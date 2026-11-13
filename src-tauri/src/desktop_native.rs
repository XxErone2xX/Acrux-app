//! Window-scoped desktop access. No global keyboard, cursor, or foreground APIs.
use crate::{
    automation::{Command, Word},
    device::Gesture,
    fishing::{Phase, Tap},
    navigation::Motion,
    now, runtime,
    vision::{Frame, Point, Region},
    DeviceTick, GameInput,
};
use std::{
    collections::{BTreeSet, VecDeque},
    io,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};

#[cfg(target_os = "windows")]
#[path = "desktop_native/windows.rs"]
mod platform;
#[cfg(target_os = "macos")]
#[path = "desktop_native/macos.rs"]
mod platform;

static STOP: AtomicBool = AtomicBool::new(false);
static RUNNING: AtomicBool = AtomicBool::new(false);
static AVAILABLE: AtomicBool = AtomicBool::new(false);
static STATUS: Mutex<Option<crate::BackgroundStatus>> = Mutex::new(None);
static WORKER: Mutex<Option<thread::JoinHandle<()>>> = Mutex::new(None);
static SNAPSHOT: Mutex<Option<mpsc::Sender<Result<Vec<u8>, String>>>> = Mutex::new(None);
static MANUAL: Mutex<VecDeque<ManualRequest>> = Mutex::new(VecDeque::new());

#[derive(Clone, Copy, Debug, PartialEq)]
struct Target {
    id: u64,
    pid: u32,
    birth: u64,
    x: f64,
    y: f64,
    width: usize,
    height: usize,
}
impl Target {
    fn geometry(self) -> (usize, usize) {
        (self.width, self.height)
    }
}

struct Image {
    data: Vec<u8>,
    width: usize,
    height: usize,
}
impl Image {
    fn frame(&self, region: Option<Region>) -> Option<Frame<'_>> {
        let r = region.unwrap_or(Region::full(self.width, self.height));
        if !r.valid((self.width, self.height)) {
            return None;
        }
        Frame::region(
            &self.data[(r.y * self.width + r.x) * 4..],
            self.width,
            self.height,
            self.width * 4,
            4,
            true,
            r,
        )
    }
    fn validate(&self) -> Result<(), String> {
        let expected = image_bytes(self.width, self.height)?;
        if self.data.len() != expected {
            return Err("게임 화면 데이터의 길이가 잘못됐습니다.".into());
        }
        // GPU-protected/minimized windows can return success with an empty bitmap.
        // Check the entire image: a small bright UI element must not be missed by sampling.
        let mut high = 0u8;
        let first = &self.data[..3];
        let mut varied = false;
        for pixel in self.data.chunks_exact(4) {
            for (channel, &v) in pixel[..3].iter().enumerate() {
                high = high.max(v);
                varied |= v.abs_diff(first[channel]) >= 3;
            }
            if high >= 8 && varied {
                return Ok(());
            }
        }
        Err(
            "게임이 검은 화면을 반환했습니다. 최소화를 해제하고 화면 캡처 권한을 확인하세요."
                .into(),
        )
    }
    fn encode(&self) -> Result<Vec<u8>, String> {
        self.validate()?;
        let mut bytes = Vec::with_capacity(self.data.len() + 8);
        bytes.extend_from_slice(&(self.width as u32).to_le_bytes());
        bytes.extend_from_slice(&(self.height as u32).to_le_bytes());
        for p in self.data.chunks_exact(4) {
            bytes.extend_from_slice(&[p[2], p[1], p[0], 255]);
        }
        Ok(bytes)
    }
}
fn image_bytes(width: usize, height: usize) -> Result<usize, String> {
    width
        .checked_mul(height)
        .and_then(|n| n.checked_mul(4))
        .filter(|&n| {
            width >= 64
                && height >= 64
                && width <= 16384
                && height <= 16384
                && n <= 128 * 1024 * 1024
        })
        .ok_or_else(|| "지원하지 않는 게임 화면 크기입니다.".into())
}

#[derive(serde::Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum ManualEvent {
    Key {
        vk: u16,
        down: bool,
    },
    Pointer {
        x: f64,
        y: f64,
        right: bool,
        down: bool,
    },
    Look {
        dx: i32,
        dy: i32,
    },
    Release,
}
struct ManualRequest {
    event: ManualEvent,
    generation: i32,
    reply: mpsc::Sender<Result<(), String>>,
    expires: Instant,
    canceled: Arc<AtomicBool>,
}
impl ManualRequest {
    fn current(&self) -> bool {
        !self.canceled.load(Ordering::Acquire) && Instant::now() < self.expires
    }
}

#[derive(Default)]
struct Held {
    keys: BTreeSet<u16>,
    buttons: BTreeSet<bool>,
    position: (i32, i32),
}
impl Held {
    fn release(&mut self, native: &mut platform::Native, target: Target) {
        // Release only to the original process/window; never fall back to the current foreground.
        for key in std::mem::take(&mut self.keys) {
            let _ = native.key(target, key, false);
        }
        for right in std::mem::take(&mut self.buttons) {
            let _ = native.button(target, self.position, right, false);
        }
    }
    fn key(
        &mut self,
        native: &mut platform::Native,
        target: Target,
        key: u16,
        down: bool,
    ) -> Result<(), String> {
        native.key(target, key, down)?;
        if down {
            self.keys.insert(key);
        } else {
            self.keys.remove(&key);
        }
        Ok(())
    }
    fn pointer(
        &mut self,
        native: &mut platform::Native,
        target: Target,
        at: (i32, i32),
        right: bool,
        down: bool,
    ) -> Result<(), String> {
        native.move_pointer(target, at, self.buttons.contains(&true))?;
        native.button(target, at, right, down)?;
        self.position = at;
        if down {
            self.buttons.insert(right);
        } else {
            self.buttons.remove(&right);
        }
        Ok(())
    }
}

pub fn start() -> io::Result<()> {
    set_status("stopped", "백그라운드 게임 연결을 시작해 주세요.", None);
    Ok(())
}
pub fn session_status() -> crate::BackgroundStatus {
    STATUS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
        .unwrap_or_else(|| crate::BackgroundStatus {
            supported: true,
            state: "stopped",
            message: "백그라운드 게임 연결을 시작해 주세요.".into(),
            width: 0,
            height: 0,
        })
}
fn set_status(state: &'static str, message: impl Into<String>, target: Option<Target>) {
    *STATUS.lock().unwrap_or_else(|e| e.into_inner()) = Some(crate::BackgroundStatus {
        supported: true,
        state,
        message: message.into(),
        width: target.map_or(0, |t| t.width),
        height: target.map_or(0, |t| t.height),
    });
}
pub fn launch_session() -> Result<crate::BackgroundStatus, String> {
    if RUNNING
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return Ok(session_status());
    }
    if let Some(old) = WORKER.lock().unwrap_or_else(|e| e.into_inner()).take() {
        let _ = old.join();
    }
    STOP.store(false, Ordering::Release);
    set_status("starting", "Roblox 창과 캡처 권한을 확인합니다.", None);
    let handle = thread::Builder::new()
        .name("acrux-window-device".into())
        .spawn(|| {
            let result = (|| {
                let mut native = platform::Native::new()?;
                {
                    let mut r = runtime();
                    r.capture = true;
                    r.input = true;
                    r.dua.background = true;
                    r.dua.hotkeys = false;
                }
                set_status(
                    "ready",
                    "기기 연결을 준비했습니다. 게임 창을 기다립니다.",
                    None,
                );
                worker(&mut native)
            })();
            AVAILABLE.store(false, Ordering::Release);
            {
                let mut r = runtime();
                r.stop("백그라운드 게임 연결이 종료됐습니다.");
                r.capture = false;
                r.input = false;
                r.dua.background = false;
            }
            for request in std::mem::take(&mut *MANUAL.lock().unwrap_or_else(|e| e.into_inner())) {
                let _ = request.reply.send(Err("게임 연결이 종료됐습니다.".into()));
            }
            if let Some(reply) = SNAPSHOT.lock().unwrap_or_else(|e| e.into_inner()).take() {
                let _ = reply.send(Err("게임 연결이 종료됐습니다.".into()));
            }
            match result {
                Ok(()) => set_status("stopped", "게임 연결을 종료했습니다.", None),
                Err(e) => set_status("error", e, None),
            }
            RUNNING.store(false, Ordering::Release);
        })
        .map_err(|e| {
            RUNNING.store(false, Ordering::Release);
            set_status("error", e.to_string(), None);
            e.to_string()
        })?;
    *WORKER.lock().unwrap_or_else(|e| e.into_inner()) = Some(handle);
    Ok(session_status())
}
pub fn stop_session() {
    STOP.store(true, Ordering::Release);
    runtime().stop("게임 연결을 종료합니다.");
}
pub fn shutdown() {
    stop_session();
    let deadline = Instant::now() + Duration::from_secs(4);
    while RUNNING.load(Ordering::Acquire) && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(20));
    }
    if !RUNNING.load(Ordering::Acquire) {
        if let Some(handle) = WORKER.lock().unwrap_or_else(|e| e.into_inner()).take() {
            let _ = handle.join();
        }
    }
}
pub fn game_available() -> bool {
    AVAILABLE.load(Ordering::Acquire)
}
pub fn snapshot() -> Result<Vec<u8>, String> {
    if !game_available() {
        return Err("실행 중인 게임 창을 먼저 연결해 주세요.".into());
    }
    let (send, receive) = mpsc::channel();
    {
        let mut q = SNAPSHOT.lock().unwrap_or_else(|e| e.into_inner());
        if q.is_some() {
            return Err("게임 화면을 이미 요청 중입니다.".into());
        }
        *q = Some(send);
    }
    receive
        .recv_timeout(Duration::from_secs(4))
        .map_err(|_| "게임 화면 확인 시간이 초과됐습니다.".to_owned())?
}
pub fn pick() -> Result<Point, String> {
    Err("앱의 게임 미리보기에서 위치를 지정하세요.".into())
}
pub fn cancel_pick() {
    if let Some(reply) = SNAPSHOT.lock().unwrap_or_else(|e| e.into_inner()).take() {
        let _ = reply.send(Err("위치 지정을 취소했습니다.".into()));
    }
}
pub fn manual(event: ManualEvent, generation: i32) -> Result<(), String> {
    match &event {
        ManualEvent::Key { vk, .. } if !platform::key_supported(*vk) => {
            return Err("지원하지 않는 키입니다.".into())
        }
        ManualEvent::Pointer { x, y, .. }
            if !x.is_finite()
                || !y.is_finite()
                || !(0.0..1.0).contains(x)
                || !(0.0..1.0).contains(y) =>
        {
            return Err("게임 화면 안의 위치를 지정하세요.".into())
        }
        ManualEvent::Look { dx, dy } if dx.unsigned_abs() > 32767 || dy.unsigned_abs() > 32767 => {
            return Err("카메라 이동량이 너무 큽니다.".into())
        }
        _ => {}
    }
    if !game_available() || STOP.load(Ordering::Acquire) {
        return Err("게임 창이 연결되지 않았습니다.".into());
    }
    {
        let r = runtime();
        if !matches!(event, ManualEvent::Release) && (r.active() || r.generation != generation) {
            return Err("매크로 실행 중에는 게임 화면을 보기만 할 수 있습니다.".into());
        }
    }
    let (send, receive) = mpsc::channel();
    let canceled = Arc::new(AtomicBool::new(false));
    {
        let mut q = MANUAL.lock().unwrap_or_else(|e| e.into_inner());
        if q.len() >= 64 {
            return Err("게임 입력을 처리 중입니다.".into());
        }
        q.push_back(ManualRequest {
            event,
            generation,
            reply: send,
            expires: Instant::now() + Duration::from_secs(2),
            canceled: canceled.clone(),
        });
    }
    let result = receive.recv_timeout(Duration::from_secs(2));
    canceled.store(true, Ordering::Release);
    result.map_err(|_| "게임 입력 확인 시간이 초과됐습니다.".to_owned())?
}

fn worker(native: &mut platform::Native) -> Result<(), String> {
    let mut selected: Option<Target> = None;
    let mut held = Held::default();
    let mut manual_generation = runtime().generation;
    let result = (|| {
        loop {
            let started = Instant::now();
            if STOP.load(Ordering::Acquire) {
                return Ok(());
            }
            let target = if let Some(target) = selected.filter(|&target| native.valid(target)) {
                target
            } else {
                if let Some(old) = selected.take() {
                    held.release(native, old);
                }
                AVAILABLE.store(false, Ordering::Release);
                runtime().no_target();
                match native.find_now()? {
                    Some(target) => {
                        native.capture(target)?.validate()?;
                        selected = Some(target);
                        AVAILABLE.store(true, Ordering::Release);
                        set_status("ready", "소유 게임 창에 연결됐습니다.", Some(target));
                        target
                    }
                    None => {
                        if let Some(reply) =
                            SNAPSHOT.lock().unwrap_or_else(|e| e.into_inner()).take()
                        {
                            let _ = reply.send(Err("게임 창을 기다리고 있습니다.".into()));
                        }
                        for request in
                            std::mem::take(&mut *MANUAL.lock().unwrap_or_else(|e| e.into_inner()))
                        {
                            let _ = request
                                .reply
                                .send(Err("게임 창이 사라져 이전 입력을 취소했습니다.".into()));
                        }
                        let next = runtime().headless_missing(now());
                        if let Some((generation, command)) = next {
                            let current = !STOP.load(Ordering::Acquire)
                                && runtime().input_current(generation);
                            if current {
                                let result = match command {
                                    Command::Launch { url } => native.launch_missing(&url),
                                    _ => Err("화면 없이 실행할 수 없는 복구 명령입니다.".into()),
                                };
                                let mut r = runtime();
                                if r.input_current(generation) {
                                    if result.is_ok() {
                                        r.device_ack();
                                    } else {
                                        r.input_rejected("소유 게임을 다시 실행하지 못했습니다.");
                                    }
                                }
                                if let Err(error) = result {
                                    set_status("ready", error, None);
                                }
                            }
                        }
                        thread::sleep(Duration::from_millis(100));
                        continue;
                    }
                }
            };
            let generation = runtime().generation;
            if manual_generation != generation {
                held.release(native, target);
                manual_generation = generation;
            }
            if let Some(reply) = SNAPSHOT.lock().unwrap_or_else(|e| e.into_inner()).take() {
                let _ = reply.send(native.capture(target).and_then(|image| image.encode()));
            }
            for request in std::mem::take(&mut *MANUAL.lock().unwrap_or_else(|e| e.into_inner())) {
                let allowed = {
                    let r = runtime();
                    matches!(request.event, ManualEvent::Release)
                        || (request.current() && !r.active() && r.generation == request.generation)
                };
                let result = if !allowed {
                    Err("실행 상태가 바뀌어 게임 입력을 취소했습니다.".into())
                } else {
                    match request.event {
                        ManualEvent::Release => {
                            held.release(native, target);
                            Ok(())
                        }
                        ManualEvent::Key { vk, down } => held.key(native, target, vk, down),
                        ManualEvent::Pointer { x, y, right, down } => {
                            held.pointer(native, target, position(Point(x, y), target), right, down)
                        }
                        ManualEvent::Look { dx, dy } => {
                            if !held.buttons.contains(&true) {
                                Err("카메라 드래그를 먼저 시작하세요.".into())
                            } else {
                                let p = (
                                    (held.position.0 + dx).clamp(0, target.width as i32 - 1),
                                    (held.position.1 + dy).clamp(0, target.height as i32 - 1),
                                );
                                native.move_pointer(target, p, true).map(|_| {
                                    held.position = p;
                                })
                            }
                        }
                    }
                };
                if result.is_err() {
                    held.release(native, target);
                }
                let _ = request.reply.send(result);
            }
            let (active, phase) = {
                let r = runtime();
                (r.active(), r.phase())
            };
            if !active {
                thread::sleep(Duration::from_millis(100));
                continue;
            }
            held.release(native, target);
            let (mut step, mut generation) = {
                let mut r = runtime();
                (r.tick(now(), target.geometry(), None), r.generation)
            };
            if let DeviceTick::Observe(region) = step {
                let need_ocr = runtime().request_ocr;
                // OCR sees this exact capture; stale text must not drive the next state.
                // The acquisition request bounds OS capture/copy time too.
                let captured_at = now();
                let image = native.capture(target)?;
                #[cfg(target_os = "windows")]
                let captured_at = captured_at.min(native.captured_at());
                image.validate()?;
                let words = if need_ocr {
                    ocr_words(&image, region, |crop| native.ocr(crop))?
                } else {
                    Vec::new()
                };
                let mut r = runtime();
                if !r.input_current(generation) {
                    continue;
                }
                if !native.valid(target) {
                    r.no_target();
                    continue;
                }
                r.request_ocr = false;
                let frame = image
                    .frame(region)
                    .ok_or("게임 화면 영역이 잘못됐습니다.")?;
                step = observed_tick(&mut r, target.geometry(), &frame, words, captured_at, now());
                generation = r.generation;
            }
            if let DeviceTick::Input(input) = step {
                if !input_evidence_current(&runtime(), now()) {
                    runtime().input_rejected("새 게임 화면을 다시 확인해야 합니다.");
                    continue;
                }
                let result = perform(native, target, input, generation);
                let mut r = runtime();
                let stale = !input_evidence_current(&r, now());
                if r.input_current(generation) {
                    match result { Ok(()) => r.device_ack(), Err(_) => r.input_rejected(if stale {"새 게임 화면을 다시 확인해야 합니다."} else {"게임 창에 입력을 전달하지 못했습니다. 연결 상태와 OS 권한을 확인하세요."}) }
                }
                if let Err(error) = result {
                    if stale {
                        continue;
                    }
                    return Err(error);
                }
            }
            let period = if phase == Phase::Reeling { 8 } else { 50 };
            thread::sleep(Duration::from_millis(period).saturating_sub(started.elapsed()));
        }
    })();
    if let Some(target) = selected {
        held.release(native, target);
    }
    result
}
fn input_evidence_current(state: &crate::Runtime, at: f64) -> bool {
    at.is_finite()
        && at >= 0.0
        && state
            .frame_expires
            .is_none_or(|deadline| deadline.is_finite() && at <= deadline)
}
fn claim_evidence(state: &mut crate::Runtime, at: f64) -> bool {
    if !input_evidence_current(state, at) {
        return false;
    }
    state.frame_expires = None;
    true
}
fn observed_tick(
    state: &mut crate::Runtime,
    geometry: (usize, usize),
    frame: &Frame<'_>,
    words: Vec<Word>,
    captured_at: f64,
    at: f64,
) -> DeviceTick {
    state.frame_expires = Some(captured_at + 0.300);
    state.ocr_words.clear();
    if !captured_at.is_finite()
        || captured_at < 0.0
        || at < captured_at
        || !input_evidence_current(state, at)
    {
        return DeviceTick::Observe(Some(frame.coverage()));
    }
    state.ocr_words = words;
    let step = state.tick(at, geometry, Some(frame));
    state.ocr_words.clear();
    step
}
fn ocr_words(
    image: &Image,
    region: Option<Region>,
    recognize: impl FnOnce(&Image) -> Result<Vec<Word>, String>,
) -> Result<Vec<Word>, String> {
    let Some(r) = region.filter(|&r| r != Region::full(image.width, image.height)) else {
        return recognize(image);
    };
    if !r.valid((image.width, image.height)) {
        return Err("OCR 화면 영역이 올바르지 않습니다.".into());
    }
    let mut data = Vec::with_capacity(r.width * r.height * 4);
    for y in r.y..r.y + r.height {
        data.extend_from_slice(
            &image.data[(y * image.width + r.x) * 4..(y * image.width + r.x + r.width) * 4],
        );
    }
    let crop = Image {
        data,
        width: r.width,
        height: r.height,
    };
    let mut words = recognize(&crop)?;
    for word in &mut words {
        let b = word.rect;
        word.rect = crate::vision::Rect(
            (r.x as f64 + b.0 * r.width as f64) / image.width as f64,
            (r.y as f64 + b.1 * r.height as f64) / image.height as f64,
            (r.x as f64 + b.2 * r.width as f64) / image.width as f64,
            (r.y as f64 + b.3 * r.height as f64) / image.height as f64,
        );
    }
    Ok(words)
}
fn current(native: &platform::Native, target: Target, generation: i32) -> bool {
    !STOP.load(Ordering::Acquire) && runtime().input_current(generation) && native.valid(target)
}
fn wait(native: &platform::Native, target: Target, generation: i32, ms: u32) -> Result<(), String> {
    let deadline = Instant::now() + Duration::from_millis(ms.into());
    loop {
        if !current(native, target, generation) {
            return Err("실행 상태나 입력 대상이 바뀌어 동작을 취소했습니다.".into());
        }
        if Instant::now() >= deadline {
            return Ok(());
        }
        thread::sleep(
            Duration::from_millis(8).min(deadline.saturating_duration_since(Instant::now())),
        );
    }
}
fn position(p: Point, target: Target) -> (i32, i32) {
    (
        (p.0 * target.width as f64)
            .floor()
            .clamp(0.0, target.width as f64 - 1.0) as i32,
        (p.1 * target.height as f64)
            .floor()
            .clamp(0.0, target.height as f64 - 1.0) as i32,
    )
}
fn direction(from: Point, to: Point, geometry: (usize, usize)) -> Vec<u16> {
    let (dx, dy) = (
        (to.0 - from.0) * geometry.0 as f64,
        (to.1 - from.1) * geometry.1 as f64,
    );
    let length = dx.hypot(dy);
    let mut keys = Vec::new();
    if length > 1e-6 {
        for (v, neg, pos) in [(dx, 0x41, 0x44), (dy, 0x57, 0x53)] {
            if v / length > 0.25 {
                keys.push(pos);
            } else if v / length < -0.25 {
                keys.push(neg);
            }
        }
    }
    keys
}
fn hold(
    native: &mut platform::Native,
    target: Target,
    keys: &[u16],
    ms: u32,
    generation: i32,
    jump: bool,
) -> Result<(), String> {
    if ms > 30_000 || !current(native, target, generation) {
        return Err("게임 입력 시간이나 상태가 잘못됐습니다.".into());
    }
    let mut held = Held::default();
    let result = (|| {
        for &key in keys {
            if !current(native, target, generation) {
                return Err("게임 입력을 취소했습니다.".into());
            }
            if held.keys.is_empty() && !claim_evidence(&mut runtime(), now()) {
                return Err("새 게임 화면을 다시 확인해야 합니다.".into());
            }
            held.key(native, target, key, true)?;
        }
        if jump {
            if held.keys.is_empty() && !claim_evidence(&mut runtime(), now()) {
                return Err("새 게임 화면을 다시 확인해야 합니다.".into());
            }
            held.key(native, target, 0x20, true)?;
        }
        let first = if jump { ms.min(50) } else { ms };
        wait(native, target, generation, first)?;
        if jump {
            held.key(native, target, 0x20, false)?;
        }
        if first < ms {
            wait(native, target, generation, ms - first)?;
        }
        Ok(())
    })();
    held.release(native, target);
    result
}
fn click(
    native: &mut platform::Native,
    target: Target,
    at: (i32, i32),
    ms: u32,
    right: bool,
    generation: i32,
) -> Result<(), String> {
    if ms > 1000 || !current(native, target, generation) || !claim_evidence(&mut runtime(), now()) {
        return Err("게임 클릭을 취소했습니다.".into());
    }
    let mut held = Held::default();
    let result = held
        .pointer(native, target, at, right, true)
        .and_then(|_| wait(native, target, generation, ms));
    held.release(native, target);
    result
}
fn drag(
    native: &mut platform::Native,
    target: Target,
    from: Point,
    to: Point,
    ms: u32,
    generation: i32,
) -> Result<(), String> {
    let mut held = Held::default();
    let result = (|| {
        if ms > 1000
            || !current(native, target, generation)
            || !claim_evidence(&mut runtime(), now())
        {
            return Err("카메라 이동을 취소했습니다.".into());
        }
        let (a, b) = (position(from, target), position(to, target));
        held.pointer(native, target, a, true, true)?;
        let steps = ms.div_ceil(12).max(1);
        for step in 1..=steps {
            wait(native, target, generation, ms / steps)?;
            let p = (
                a.0 + (b.0 - a.0) * step as i32 / steps as i32,
                a.1 + (b.1 - a.1) * step as i32 / steps as i32,
            );
            native.move_pointer(target, p, true)?;
            held.position = p;
        }
        Ok(())
    })();
    held.release(native, target);
    result
}
fn gesture(
    native: &mut platform::Native,
    target: Target,
    gesture: Gesture,
    generation: i32,
) -> Result<(), String> {
    gesture.packet(target.geometry(), 1, generation)?;
    match gesture {
        Gesture::Tap { at, ms } => {
            click(native, target, position(at, target), ms, false, generation)
        }
        Gesture::Drag { from, to, ms } => drag(native, target, from, to, ms, generation),
        Gesture::Stick { from, to, jump, ms } => hold(
            native,
            target,
            &direction(from, to, target.geometry()),
            ms,
            generation,
            jump.is_some(),
        ),
        Gesture::Pinch { from, to, ms } => {
            let distance = |p: [Point; 2]| {
                ((p[1].0 - p[0].0) * target.width as f64)
                    .hypot((p[1].1 - p[0].1) * target.height as f64)
            };
            let delta = distance(to) - distance(from);
            if delta.abs() < 1e-6 {
                return Err("확대 동작의 거리가 없습니다.".into());
            }
            hold(
                native,
                target,
                &[if delta > 0.0 { 0x49 } else { 0x4f }],
                ms,
                generation,
                false,
            )
        }
    }
}
fn perform(
    native: &mut platform::Native,
    target: Target,
    input: GameInput,
    generation: i32,
) -> Result<(), String> {
    if !current(native, target, generation) {
        return Err("게임 입력 대상이 바뀌었습니다.".into());
    }
    if !input_evidence_current(&runtime(), now()) {
        return Err("게임 화면의 입력 유효 시간이 지났습니다.".into());
    }
    match input {
        GameInput::Gesture(g) | GameInput::Command(Command::Gesture(g)) => {
            gesture(native, target, g, generation)
        }
        GameInput::Tap(Tap { x, y, duration }) => click(
            native,
            target,
            (
                x.min(target.width - 1) as i32,
                y.min(target.height - 1) as i32,
            ),
            duration.as_millis().min(1000) as u32,
            false,
            generation,
        ),
        GameInput::Motion(Motion::Tap(p, ms)) => {
            click(native, target, position(p, target), ms, false, generation)
        }
        GameInput::Motion(Motion::Click { point, right }) => click(
            native,
            target,
            position(point, target),
            40,
            right,
            generation,
        ),
        GameInput::Motion(Motion::Keys(keys, ms)) => {
            hold(native, target, keys, ms, generation, false)
        }
        GameInput::Motion(Motion::Tilt { pixels, ms }) => {
            let start = Point(0.5, 0.5);
            drag(
                native,
                target,
                start,
                Point(
                    0.5,
                    (0.5 + pixels as f64 / target.height as f64).clamp(0.0, 0.99),
                ),
                ms.min(1000),
                generation,
            )
        }
        GameInput::Motion(Motion::Stick { x, y, jump, ms, .. }) => hold(
            native,
            target,
            &direction(Point(0.0, 0.0), Point(x.into(), y.into()), (1, 1)),
            ms,
            generation,
            jump.is_some(),
        ),
        GameInput::Motion(Motion::Swipe) => drag(
            native,
            target,
            Point(0.5, 0.8),
            Point(0.5, 0.2),
            250,
            generation,
        ),
        GameInput::Motion(Motion::Pinch) => hold(native, target, &[0x4f], 300, generation, false),
        GameInput::Motion(Motion::Trace { .. }) => {
            Err("시간 기록 재생은 실시간 길찾기에서 사용하지 않습니다.".into())
        }
        GameInput::Command(Command::Key { vk, ms }) => {
            hold(native, target, &[vk], ms, generation, false)
        }
        GameInput::Command(Command::Text { text, submit }) => {
            let begun = std::cell::Cell::new(false);
            native.replace_text(target, &text, || {
                if !current(native, target, generation) {
                    return false;
                }
                if begun.get() {
                    return true;
                }
                let ready = claim_evidence(&mut runtime(), now());
                if ready {
                    begun.set(true);
                }
                ready
            })?;
            if submit {
                hold(native, target, &[0x0d], 40, generation, false)?;
            }
            Ok(())
        }
        GameInput::Command(Command::Launch { url }) => {
            if !claim_evidence(&mut runtime(), now()) {
                return Err("새 게임 화면을 다시 확인해야 합니다.".into());
            }
            native.launch(target, &url)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn regional_ocr_reads_only_requested_pixels_and_maps_words_to_viewport() {
        let mut data = Vec::new();
        for y in 0..64 {
            for x in 0..64 {
                data.extend_from_slice(&[x, y, 200, 255]);
            }
        }
        let image = Image {
            data,
            width: 64,
            height: 64,
        };
        let region = Region {
            x: 16,
            y: 8,
            width: 32,
            height: 16,
        };
        let words = ocr_words(&image, Some(region), |crop| {
            assert_eq!((crop.width, crop.height), (32, 16));
            assert_eq!(&crop.data[..4], &[16, 8, 200, 255]);
            Ok(vec![Word {
                text: "Fish".into(),
                rect: crate::vision::Rect(0.25, 0.25, 0.75, 0.75),
            }])
        })
        .unwrap();
        let b = words[0].rect;
        assert_eq!([b.0, b.1, b.2, b.3], [0.375, 0.1875, 0.625, 0.3125]);
        assert_eq!(image.frame(Some(region)).unwrap().coverage(), region);
    }
    #[test]
    fn black_and_constant_frames_fail() {
        for p in [[0, 0, 0, 255], [100, 100, 100, 255], [10, 30, 200, 255]] {
            let image = Image {
                data: p.repeat(64 * 64),
                width: 64,
                height: 64,
            };
            assert!(image.validate().is_err());
        }
        let mut image = Image {
            data: vec![0; 64 * 64 * 4],
            width: 64,
            height: 64,
        };
        image.data[24] = 255;
        assert!(image.validate().is_ok());
    }
    #[test]
    fn snapshot_is_rgba_and_regions_keep_viewport_coordinates() {
        let mut image = Image {
            data: vec![0; 64 * 64 * 4],
            width: 64,
            height: 64,
        };
        image.data[..4].copy_from_slice(&[2, 40, 200, 255]);
        assert_eq!(&image.encode().unwrap()[8..12], &[200, 40, 2, 255]);
        assert_eq!(
            image
                .frame(Some(Region {
                    x: 1,
                    y: 2,
                    width: 5,
                    height: 6
                }))
                .unwrap()
                .coverage(),
            Region {
                x: 1,
                y: 2,
                width: 5,
                height: 6
            }
        );
        assert!(image
            .frame(Some(Region {
                x: 63,
                y: 0,
                width: 2,
                height: 4
            }))
            .is_none());
    }
    #[test]
    fn normalized_sticks_map_to_camera_relative_keys() {
        assert_eq!(
            direction(Point(0.0, 0.0), Point(1.0, -1.0), (100, 100)),
            vec![0x44, 0x57]
        );
    }
}

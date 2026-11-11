use crate::{
    fishing::{Phase, Tap},
    now, runtime,
    vision::{Frame, Point, Region},
    DeviceTick, GameInput, Motion,
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

pub(crate) mod session;
use x11rb::{
    connection::Connection as _,
    protocol::{xproto::*, xtest::ConnectionExt as _},
    rust_connection::RustConnection,
};

static AVAILABLE: AtomicBool = AtomicBool::new(false);
static SESSION_STOP: AtomicBool = AtomicBool::new(false);
static SESSION_RUNNING: AtomicBool = AtomicBool::new(false);
static STOP_HANDLE: Mutex<Option<session::StopHandle>> = Mutex::new(None);
static SESSION_STATE: Mutex<Option<crate::BackgroundStatus>> = Mutex::new(None);
static SNAPSHOT: Mutex<Option<mpsc::Sender<Result<Vec<u8>, String>>>> = Mutex::new(None);
static MANUAL: Mutex<VecDeque<ManualRequest>> = Mutex::new(VecDeque::new());
#[derive(serde::Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
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
        matches!(self.event, ManualEvent::Release)
            || (!self.canceled.load(Ordering::Acquire) && Instant::now() < self.expires)
    }
}
#[derive(Default)]
struct Held {
    keys: BTreeSet<u8>,
    buttons: BTreeSet<u32>,
}
impl Held {
    fn release(&mut self, connection: &Connection) {
        for key in std::mem::take(&mut self.keys) {
            connection.key(key, false);
        }
        for button in std::mem::take(&mut self.buttons) {
            connection.button(button, false);
        }
    }
    fn apply(
        &mut self,
        connection: &Connection,
        event: &ManualEvent,
        current: impl Fn() -> bool,
    ) -> Result<(), String> {
        if !current() {
            self.release(connection);
            return Err("기한이 지난 게임 입력을 취소했습니다.".into());
        }
        let ok = match *event {
            ManualEvent::Release => {
                self.release(connection);
                true
            }
            ManualEvent::Key { vk, down } => {
                let code = connection
                    .keycode(vk)
                    .ok_or("이 키는 게임 화면에 전달할 수 없습니다.")?;
                if down {
                    if connection.focused().is_none() {
                        return Err("게임 입력 대상을 확인하지 못했습니다.".into());
                    }
                    if !current() {
                        false
                    } else if self.keys.insert(code) {
                        connection.key(code, true)
                    } else {
                        true
                    }
                } else if self.keys.contains(&code) {
                    let ok = connection.key(code, false);
                    if ok {
                        self.keys.remove(&code);
                    }
                    ok
                } else {
                    true
                }
            }
            ManualEvent::Pointer { x, y, right, down } => {
                let button = if right { 3 } else { 1 };
                if down {
                    let target = connection
                        .focused()
                        .ok_or("게임 입력 대상을 확인하지 못했습니다.")?;
                    let x = target.x + (x * target.width as f64).floor() as i32;
                    let y = target.y + (y * target.height as f64).floor() as i32;
                    if !current() || !connection.move_pointer(x, y) {
                        false
                    } else {
                        let over = connection
                            .pointer()
                            .is_some_and(|(_, _, _, child)| connection.over(child, target.window));
                        if !over || connection.focused() != Some(target) || !current() {
                            false
                        } else {
                            self.buttons.insert(button);
                            connection.button(button, true)
                        }
                    }
                } else if self.buttons.contains(&button) {
                    let ok = connection.button(button, false);
                    if ok {
                        self.buttons.remove(&button);
                    }
                    ok
                } else {
                    true
                }
            }
            ManualEvent::Look { dx, dy } => {
                if !self.buttons.contains(&3) || connection.focused().is_none() || !current() {
                    false
                } else {
                    connection.fake(MOTION_NOTIFY_EVENT, 1, 0, dx as i16, dy as i16)
                }
            }
        };
        if ok {
            Ok(())
        } else {
            self.release(connection);
            Err("독립 게임에 입력을 보내지 못했습니다.".into())
        }
    }
}
pub fn manual(event: ManualEvent, generation: i32) -> Result<(), String> {
    match &event {
        ManualEvent::Key { vk, .. } if key_symbol(*vk).is_none() => {
            return Err("지원하지 않는 키입니다.".into())
        }
        ManualEvent::Pointer { x, y, .. }
            if !x.is_finite()
                || !y.is_finite()
                || !(0.0..1.0).contains(x)
                || !(0.0..1.0).contains(y) =>
        {
            return Err("게임 화면의 입력 위치를 확인하세요.".into())
        }
        ManualEvent::Look { dx, dy } if dx.unsigned_abs() > 32767 || dy.unsigned_abs() > 32767 => {
            return Err("카메라 이동량이 너무 큽니다.".into())
        }
        _ => {}
    }
    if SESSION_STOP.load(Ordering::Acquire) || session_status().state != "ready" {
        return Err("백그라운드 게임이 연결되지 않았습니다.".into());
    }
    {
        let state = runtime();
        if !matches!(event, ManualEvent::Release)
            && (state.active() || state.generation != generation)
        {
            return Err("매크로 실행 중에는 게임 화면을 보기만 할 수 있습니다.".into());
        }
    }
    let (send, receive) = mpsc::channel();
    let canceled = Arc::new(AtomicBool::new(false));
    {
        let mut queue = MANUAL.lock().unwrap_or_else(|e| e.into_inner());
        if queue.len() >= 64 {
            return Err("이전 게임 입력을 처리 중입니다.".into());
        }
        queue.push_back(ManualRequest {
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
static WORKER: Mutex<Option<thread::JoinHandle<()>>> = Mutex::new(None);

struct Connection {
    conn: RustConnection,
    root: Window,
    screen: usize,
    clients: Atom,
    first_key: u8,
    keysyms_per_key: usize,
    keysyms: Vec<u32>,
}

impl Connection {
    #[cfg(test)]
    fn open() -> Option<Self> {
        let (conn, screen) = RustConnection::connect(None).ok()?;
        Self::from_connection(conn, screen)
    }

    fn from_connection(conn: RustConnection, screen: usize) -> Option<Self> {
        conn.xtest_get_version(2, 2).ok()?.reply().ok()?;
        let setup = conn.setup();
        let root = setup.roots.get(screen)?.root;
        let first_key = setup.min_keycode;
        let count = setup.max_keycode.checked_sub(first_key)?.checked_add(1)?;
        let keyboard = conn
            .get_keyboard_mapping(first_key, count)
            .ok()?
            .reply()
            .ok()?;
        if keyboard.keysyms_per_keycode == 0 {
            return None;
        }
        let clients = conn
            .intern_atom(false, b"_NET_CLIENT_LIST")
            .ok()?
            .reply()
            .ok()?
            .atom;
        Some(Self {
            conn,
            root,
            screen,
            clients,
            first_key,
            keysyms_per_key: keyboard.keysyms_per_keycode.into(),
            keysyms: keyboard.keysyms,
        })
    }

    fn is_sober(&self, window: Window) -> bool {
        self.conn
            .get_property(false, window, AtomEnum::WM_CLASS, AtomEnum::STRING, 0, 256)
            .ok()
            .and_then(|cookie| cookie.reply().ok())
            .is_some_and(|reply| {
                reply.format == 8 && reply.value.split(|byte| *byte == 0).any(sober_class)
            })
    }

    fn parent(&self, window: Window) -> Option<Window> {
        let parent = self.conn.query_tree(window).ok()?.reply().ok()?.parent;
        (parent != 0 && parent != window).then_some(parent)
    }

    fn target(&self, window: Window) -> Option<Target> {
        let attributes = self.conn.get_window_attributes(window).ok()?.reply().ok()?;
        if attributes.map_state != MapState::VIEWABLE {
            return None;
        }
        let geometry = self.conn.get_geometry(window).ok()?.reply().ok()?;
        if !(1..=16384).contains(&geometry.width) || !(1..=16384).contains(&geometry.height) {
            return None;
        }
        let origin = self
            .conn
            .translate_coordinates(window, self.root, 0, 0)
            .ok()?
            .reply()
            .ok()?;
        if !origin.same_screen {
            return None;
        }
        Some(Target {
            window,
            x: origin.dst_x.into(),
            y: origin.dst_y.into(),
            width: geometry.width.into(),
            height: geometry.height.into(),
        })
    }

    fn focused(&self) -> Option<Target> {
        let mut window = self.conn.get_input_focus().ok()?.reply().ok()?.focus;
        for _ in 0..32 {
            if window <= 1 || window == self.root {
                break;
            }
            if self.is_sober(window) {
                return self.target(window);
            }
            window = self.parent(window)?;
        }
        None
    }

    fn available(&self) -> bool {
        let mut windows = self
            .conn
            .get_property(false, self.root, self.clients, AtomEnum::WINDOW, 0, 4096)
            .ok()
            .and_then(|cookie| cookie.reply().ok())
            .and_then(|reply| reply.value32().map(|values| values.collect::<Vec<_>>()))
            .unwrap_or_default();
        // No window manager is needed for the adapter or its isolated fixture.
        if let Some(reply) = self
            .conn
            .query_tree(self.root)
            .ok()
            .and_then(|cookie| cookie.reply().ok())
        {
            windows.extend(reply.children);
        }
        windows.sort_unstable();
        windows.dedup();
        windows
            .into_iter()
            .any(|window| self.is_sober(window) && self.target(window).is_some())
    }

    fn keycode(&self, vk: u16) -> Option<u8> {
        let symbol = key_symbol(vk)?;
        let offset = self
            .keysyms
            .chunks_exact(self.keysyms_per_key)
            .position(|symbols| symbols.contains(&symbol))?;
        u8::try_from(usize::from(self.first_key) + offset).ok()
    }

    fn pressed(&self, vk: u16) -> bool {
        let Some(code) = self.keycode(vk) else {
            return false;
        };
        self.conn
            .query_keymap()
            .ok()
            .and_then(|cookie| cookie.reply().ok())
            .is_some_and(|reply| reply.keys[usize::from(code / 8)] & (1 << (code % 8)) != 0)
    }

    fn fake(&self, kind: u8, detail: u8, root: Window, x: i16, y: i16) -> bool {
        self.conn
            .xtest_fake_input(kind, detail, 0, root, x, y, 0)
            .ok()
            .is_some_and(|cookie| cookie.check().is_ok())
    }

    fn key(&self, code: u8, down: bool) -> bool {
        self.fake(
            if down {
                KEY_PRESS_EVENT
            } else {
                KEY_RELEASE_EVENT
            },
            code,
            0,
            0,
            0,
        )
    }

    fn button(&self, button: u32, down: bool) -> bool {
        let Ok(button) = u8::try_from(button) else {
            return false;
        };
        self.fake(
            if down {
                BUTTON_PRESS_EVENT
            } else {
                BUTTON_RELEASE_EVENT
            },
            button,
            0,
            0,
            0,
        )
    }

    fn move_pointer(&self, x: i32, y: i32) -> bool {
        let (Ok(x), Ok(y)) = (i16::try_from(x), i16::try_from(y)) else {
            return false;
        };
        self.fake(MOTION_NOTIFY_EVENT, 0, self.root, x, y)
    }

    fn relative(&self, dy: i32) -> bool {
        let Ok(dy) = i16::try_from(dy) else {
            return false;
        };
        self.fake(MOTION_NOTIFY_EVENT, 1, 0, 0, dy)
    }

    fn pointer(&self) -> Option<(i32, i32, u32, Window)> {
        let reply = self.conn.query_pointer(self.root).ok()?.reply().ok()?;
        reply.same_screen.then_some((
            reply.root_x.into(),
            reply.root_y.into(),
            u16::from(reply.mask).into(),
            reply.child,
        ))
    }

    fn over(&self, mut window: Window, target: Window) -> bool {
        for _ in 0..32 {
            if window == target {
                return true;
            }
            if window <= 1 || window == self.root {
                break;
            }
            // The root pointer child can be a window-manager frame. Follow
            // the pointer into the frame to find the actual Sober client.
            let Some(child) = self
                .conn
                .query_pointer(window)
                .ok()
                .and_then(|cookie| cookie.reply().ok())
                .map(|reply| reply.child)
                .filter(|child| *child != 0 && *child != window)
            else {
                break;
            };
            window = child;
        }
        false
    }

    fn capture(&self, target: Target) -> Option<Image> {
        self.capture_region(target, None)
    }

    fn capture_region(&self, target: Target, region: Option<Region>) -> Option<Image> {
        self.capture_checked(target, region).ok()
    }

    fn capture_checked(&self, target: Target, region: Option<Region>) -> Result<Image, String> {
        let region = region.unwrap_or(Region::full(target.width, target.height));
        if !region.valid((target.width, target.height))
            || region
                .width
                .checked_mul(region.height)
                .and_then(|v| v.checked_mul(4))
                .is_none_or(|bytes| bytes > 64 * 1024 * 1024)
        {
            return Err("게임 캡처 영역이 유효하지 않거나 너무 큽니다.".into());
        }
        let mut reply = self
            .conn
            .get_image(
                ImageFormat::Z_PIXMAP,
                target.window,
                region.x.try_into().map_err(|_| "캡처 x 범위 오류")?,
                region.y.try_into().map_err(|_| "캡처 y 범위 오류")?,
                region.width.try_into().map_err(|_| "캡처 폭 범위 오류")?,
                region
                    .height
                    .try_into()
                    .map_err(|_| "캡처 높이 범위 오류")?,
                u32::MAX,
            )
            .map_err(|error| format!("전용 화면 읽기 요청 실패: {error}"))?
            .reply()
            .map_err(|error| format!("전용 화면 읽기 실패: {error}"))?;
        let setup = self.conn.setup();
        let format = setup
            .pixmap_formats
            .iter()
            .find(|format| format.depth == reply.depth)
            .ok_or_else(|| format!("화면 깊이 {}의 형식을 찾지 못했습니다.", reply.depth))?;
        let visual = setup
            .roots
            .get(self.screen)
            .ok_or("전용 화면 번호 오류")?
            .allowed_depths
            .iter()
            .flat_map(|depth| &depth.visuals)
            .find(|visual| visual.visual_id == reply.visual)
            .ok_or_else(|| format!("화면 visual {}을 찾지 못했습니다.", reply.visual))?;
        let (stride, pixel_stride, bgra) =
            pixel_layout(region.width, format, setup.image_byte_order, visual)
                .ok_or_else(|| format!("지원하지 않는 전용 화면 형식: depth={}, bpp={}, pad={}, order={}, class={}, masks={:x}/{:x}/{:x}",
                    format.depth, format.bits_per_pixel, format.scanline_pad,
                    u8::from(setup.image_byte_order), u8::from(visual.class),
                    visual.red_mask, visual.green_mask, visual.blue_mask))?;
        if visual.class == VisualClass::DIRECT_COLOR {
            let colormap = self
                .conn
                .get_window_attributes(target.window)
                .map_err(|error| error.to_string())?
                .reply()
                .map_err(|error| error.to_string())?
                .colormap;
            // DirectColor pixels are channel indices. Read the actual ramps;
            // SDL normally initializes them to identity, but can change them.
            let pixels: [u32; 256] = std::array::from_fn(|index| (index as u32) * 0x010101);
            let colors = self
                .conn
                .query_colors(colormap, &pixels)
                .map_err(|error| error.to_string())?
                .reply()
                .map_err(|error| error.to_string())?;
            normalize_direct_color(
                &mut reply.data,
                stride,
                pixel_stride,
                region.width,
                region.height,
                bgra,
                &colors.colors,
            )?;
        }
        let image = Image {
            data: reply.data,
            width: target.width,
            height: target.height,
            stride,
            pixel_stride,
            bgra,
            region: Some(region),
        };
        image
            .frame()
            .ok_or("전용 화면 데이터 길이가 잘못됐습니다.")?;
        Ok(image)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Target {
    window: Window,
    x: i32,
    y: i32,
    width: usize,
    height: usize,
}

struct Image {
    data: Vec<u8>,
    width: usize,
    height: usize,
    stride: usize,
    pixel_stride: usize,
    bgra: bool,
    region: Option<Region>,
}

impl Image {
    fn frame(&self) -> Option<Frame<'_>> {
        if let Some(region) = self.region {
            return Frame::region(
                &self.data,
                self.width,
                self.height,
                self.stride,
                self.pixel_stride,
                self.bgra,
                region,
            );
        }
        Frame::new(
            &self.data,
            self.width,
            self.height,
            self.stride,
            self.pixel_stride,
            self.bgra,
        )
    }
}

fn pixel_layout(
    width: usize,
    format: &Format,
    order: ImageOrder,
    visual: &Visualtype,
) -> Option<(usize, usize, bool)> {
    if !(1..=16384).contains(&width)
        || order != ImageOrder::LSB_FIRST
        || ![24, 32].contains(&format.depth)
        || ![24, 32].contains(&format.bits_per_pixel)
        || ![8, 16, 32].contains(&format.scanline_pad)
        || ![VisualClass::TRUE_COLOR, VisualClass::DIRECT_COLOR].contains(&visual.class)
    {
        return None;
    }
    let bgra = match (visual.red_mask, visual.green_mask, visual.blue_mask) {
        (0xff0000, 0xff00, 0xff) => true,
        (0xff, 0xff00, 0xff0000) => false,
        _ => return None,
    };
    let pad = usize::from(format.scanline_pad);
    let stride = width
        .checked_mul(format.bits_per_pixel.into())?
        .div_ceil(pad)
        .checked_mul(pad)?
        / 8;
    Some((stride, usize::from(format.bits_per_pixel / 8), bgra))
}

fn normalize_direct_color(
    bytes: &mut [u8],
    stride: usize,
    pixel_stride: usize,
    width: usize,
    height: usize,
    bgra: bool,
    colors: &[Rgb],
) -> Result<(), String> {
    if colors.len() != 256 || pixel_stride < 3 || width == 0 || height == 0 {
        return Err("전용 화면 색상표가 유효하지 않습니다.".into());
    }
    let row = width
        .checked_sub(1)
        .and_then(|v| v.checked_mul(pixel_stride))
        .and_then(|v| v.checked_add(3))
        .ok_or("전용 화면 색상표 영역 오류")?;
    let length = height
        .checked_sub(1)
        .and_then(|v| v.checked_mul(stride))
        .and_then(|v| v.checked_add(row))
        .ok_or("전용 화면 색상표 길이 오류")?;
    if stride < row || bytes.len() < length {
        return Err("전용 화면 색상 데이터가 부족합니다.".into());
    }
    if colors.iter().enumerate().all(|(index, color)| {
        let expected = index as u16 * 257;
        color.red == expected && color.green == expected && color.blue == expected
    }) {
        return Ok(());
    }
    let channel = |value: u16| ((u32::from(value) + 128) / 257) as u8;
    for y in 0..height {
        for x in 0..width {
            let offset = y * stride + x * pixel_stride;
            let (r, b) = if bgra { (2, 0) } else { (0, 2) };
            bytes[offset + r] = channel(colors[usize::from(bytes[offset + r])].red);
            bytes[offset + 1] = channel(colors[usize::from(bytes[offset + 1])].green);
            bytes[offset + b] = channel(colors[usize::from(bytes[offset + b])].blue);
        }
    }
    Ok(())
}

fn sober_class(value: &[u8]) -> bool {
    value.eq_ignore_ascii_case(b"sober") || value.eq_ignore_ascii_case(b"org.vinegarhq.Sober")
}

fn key_symbol(vk: u16) -> Option<u32> {
    Some(match vk {
        0x08 => 0xff08,
        0x09 => 0xff09,
        0x10 => 0xffe1,
        0x11 => 0xffe3,
        0x12 => 0xffe9,
        0x25 => 0xff51,
        0x26 => 0xff52,
        0x27 => 0xff53,
        0x28 => 0xff54,
        0x1b => 0xff1b,
        0x0d => 0xff0d,
        0x20 => 0x20,
        0xbf => 0x2f,
        0x30..=0x39 => vk.into(),
        0x41..=0x5a => (vk + 0x20).into(),
        0x70..=0x7b => 0xffbe + u32::from(vk - 0x70),
        _ => return None,
    })
}

pub fn start() -> io::Result<()> {
    *SESSION_STATE.lock().unwrap_or_else(|e| e.into_inner()) =
        Some(crate::BackgroundStatus::default());
    Ok(())
}

pub fn session_status() -> crate::BackgroundStatus {
    SESSION_STATE
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
        .unwrap_or_default()
}
fn set_session(state: &'static str, message: impl Into<String>) {
    *SESSION_STATE.lock().unwrap_or_else(|e| e.into_inner()) = Some(crate::BackgroundStatus {
        supported: true,
        state,
        message: message.into(),
        width: 1280,
        height: 720,
    });
}
pub fn launch_session() -> Result<crate::BackgroundStatus, String> {
    if SESSION_RUNNING
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return Ok(session_status());
    }
    SESSION_STOP.store(false, Ordering::Release);
    set_session("starting", "사용자 입력과 분리된 게임 화면을 준비합니다.");
    let thread = thread::Builder::new()
        .name("acrux-private-game".into())
        .spawn(|| {
            let result = (|| -> Result<(), String> {
                let mut owned = session::Session::launch_with_cancel(&SESSION_STOP)?;
                *STOP_HANDLE.lock().unwrap_or_else(|e| e.into_inner()) = Some(owned.stop_handle());
                if SESSION_STOP.load(Ordering::Acquire) {
                    return Ok(());
                }
                let (conn, screen) = owned.connect_x11()?;
                let connection = Connection::from_connection(conn, screen)
                    .ok_or("독립 게임 화면에 연결하지 못했습니다.")?;
                set_session("ready", "백그라운드 게임에 연결됐습니다.");
                worker(connection, &mut owned);
                owned.stop()?;
                Ok(())
            })();
            STOP_HANDLE.lock().unwrap_or_else(|e| e.into_inner()).take();
            AVAILABLE.store(false, Ordering::Release);
            {
                let mut state = runtime();
                state.stop("백그라운드 게임 연결이 종료됐습니다.");
                state.capture = false;
                state.input = false;
                state.dua.background = false;
            }
            for request in std::mem::take(&mut *MANUAL.lock().unwrap_or_else(|e| e.into_inner())) {
                let _ = request
                    .reply
                    .send(Err("게임 세션이 종료돼 입력을 취소했습니다.".into()));
            }
            if let Some(reply) = SNAPSHOT.lock().unwrap_or_else(|e| e.into_inner()).take() {
                let _ = reply.send(Err("백그라운드 게임 연결이 종료됐습니다.".into()));
            }
            match result {
                Ok(()) => set_session("stopped", "백그라운드 게임을 열어 주세요."),
                Err(error) => set_session("error", error),
            }
            SESSION_RUNNING.store(false, Ordering::Release);
        })
        .map_err(|error| {
            SESSION_RUNNING.store(false, Ordering::Release);
            set_session("error", error.to_string());
            error.to_string()
        })?;
    *WORKER.lock().unwrap_or_else(|e| e.into_inner()) = Some(thread);
    Ok(session_status())
}
pub fn stop_session() {
    SESSION_STOP.store(true, Ordering::Release);
    let handle = STOP_HANDLE
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone();
    if let Some(handle) = handle {
        if let Err(error) = handle.stop() {
            set_session("error", error);
        }
    }
    // Kill the owned server before waiting on Runtime: a stalled server can
    // otherwise hold that mutex through an acknowledged input request.
    runtime().stop("백그라운드 게임을 닫는 중입니다.");
}
pub fn shutdown() {
    stop_session();
    let end = Instant::now() + Duration::from_secs(4);
    while SESSION_RUNNING.load(Ordering::Acquire) && Instant::now() < end {
        thread::sleep(Duration::from_millis(20));
    }
    if !SESSION_RUNNING.load(Ordering::Acquire) {
        if let Some(thread) = WORKER.lock().unwrap_or_else(|e| e.into_inner()).take() {
            let _ = thread.join();
        }
    }
}
pub fn game_available() -> bool {
    AVAILABLE.load(Ordering::Acquire)
}

fn worker(connection: Connection, session: &mut session::Session) {
    {
        let mut state = runtime();
        state.capture = true;
        state.input = true;
        state.dua.background = true;
        state.dua.hotkeys = false;
    }
    let mut previous = None;
    let mut manual_held = Held::default();
    loop {
        let started = Instant::now();
        if SESSION_STOP.load(Ordering::Acquire) {
            manual_held.release(&connection);
            return;
        }
        if !session.healthy() {
            manual_held.release(&connection);
            return;
        }
        if connection
            .conn
            .get_input_focus()
            .ok()
            .and_then(|cookie| cookie.reply().ok())
            .is_none()
        {
            AVAILABLE.store(false, Ordering::Release);
            let mut state = runtime();
            state.capture = false;
            state.input = false;
            state.stop("Sober 연결이 끊겼습니다. 앱을 다시 실행하세요.");
            return;
        }
        AVAILABLE.store(connection.available(), Ordering::Release);
        if let Some(reply) = SNAPSHOT.lock().unwrap_or_else(|e| e.into_inner()).take() {
            let result = connection
                .focused()
                .and_then(|target| connection.capture(target))
                .ok_or_else(|| "게임 화면을 찾지 못했습니다.".to_owned())
                .and_then(|image| encode_snapshot(&image));
            let _ = reply.send(result);
        }
        let requests = std::mem::take(&mut *MANUAL.lock().unwrap_or_else(|e| e.into_inner()));
        for request in requests {
            let state = runtime();
            let allowed = request.current()
                && (matches!(request.event, ManualEvent::Release)
                    || (!state.active() && state.generation == request.generation));
            let result = if allowed {
                manual_held.apply(&connection, &request.event, || request.current())
            } else {
                manual_held.release(&connection);
                Err("실행 상태가 바뀌어 게임 입력을 취소했습니다.".into())
            };
            let _ = request.reply.send(result);
        }
        let (active, phase) = {
            let state = runtime();
            (state.active(), state.phase())
        };
        if !active {
            previous = None;
            thread::sleep(Duration::from_millis(100));
            continue;
        }
        if connection.pressed(0x72) || connection.pressed(0x76) {
            runtime().stop("단축키로 중지됨");
            continue;
        }
        if connection.pressed(0x75) && runtime().arrive_desktop() {
            continue;
        }
        manual_held.release(&connection);
        let target = connection.focused();
        if previous.is_some() && previous != target.map(|target| target.window) {
            runtime().no_target();
        }
        previous = target.map(|target| target.window);
        let Some(target) = target else {
            runtime().no_target();
            thread::sleep(Duration::from_millis(100));
            continue;
        };
        let geometry = (target.width, target.height);
        let (mut step, mut generation) = {
            let mut state = runtime();
            (state.tick(now(), geometry, None), state.generation)
        };
        if let DeviceTick::Observe(region) = step {
            let image = connection.capture_region(target, region);
            let mut state = runtime();
            if !state.input_current(generation) {
                continue;
            }
            if connection.focused() != Some(target) {
                state.no_target();
                continue;
            }
            step = match image.as_ref().and_then(Image::frame) {
                Some(frame) => state.tick(now(), geometry, Some(&frame)),
                None => {
                    state.stop("Sober 화면을 캡처하지 못했습니다.");
                    DeviceTick::Wait
                }
            };
            generation = state.generation;
        }
        if let DeviceTick::Input(input) = step {
            match perform(&connection, target, input, generation) {
                Some(true) => {}
                result => {
                    let mut state = runtime();
                    if state.input_current(generation) {
                        if result.is_none() {
                            state.no_target();
                        } else {
                            state.input_rejected("Sober 입력을 보내지 못했습니다.");
                        }
                    }
                }
            }
        }
        let period = if phase == Phase::Reeling { 8 } else { 50 };
        thread::sleep(Duration::from_millis(period).saturating_sub(started.elapsed()));
    }
}

fn keep_input(connection: &Connection, target: Target, generation: i32) -> bool {
    if SESSION_STOP.load(Ordering::Acquire) || !runtime().input_current(generation) {
        return false;
    }
    if connection.pressed(0x72) || connection.pressed(0x76) {
        runtime().stop("단축키로 중지됨");
        return false;
    }
    if connection.focused() != Some(target) {
        runtime().no_target();
        return false;
    }
    runtime().input_current(generation)
}

fn wait_input(connection: &Connection, target: Target, generation: i32, duration: Duration) {
    let end = Instant::now() + duration;
    while Instant::now() < end && keep_input(connection, target, generation) {
        thread::sleep(Duration::from_millis(12).min(end.saturating_duration_since(Instant::now())));
    }
}

fn perform(
    connection: &Connection,
    target: Target,
    input: GameInput,
    generation: i32,
) -> Option<bool> {
    if !keep_input(connection, target, generation) {
        return None;
    }
    let at = |point: Point, ms: u32| Tap {
        x: ((point.0 * target.width as f64) as usize).min(target.width - 1),
        y: ((point.1 * target.height as f64) as usize).min(target.height - 1),
        duration: Duration::from_millis(ms.into()),
    };
    match input {
        GameInput::Tap(tap) => click(connection, target, tap, false, generation),
        GameInput::Motion(Motion::Tap(point, ms)) => {
            click(connection, target, at(point, ms), false, generation)
        }
        GameInput::Motion(Motion::Click { point, right }) => {
            click(connection, target, at(point, 40), right, generation)
        }
        GameInput::Motion(Motion::Keys(keys, ms)) => {
            Some(hold_keys(connection, target, keys, ms, generation))
        }
        GameInput::Motion(Motion::Tilt { pixels, ms }) => {
            Some(tilt(connection, target, pixels, ms, generation))
        }
        GameInput::Motion(_) => Some(false),
    }
}

fn click(
    connection: &Connection,
    target: Target,
    tap: Tap,
    right: bool,
    generation: i32,
) -> Option<bool> {
    let (previous_x, previous_y, _, _) = connection.pointer()?;
    let position = (
        target.x + tap.x.min(target.width - 1) as i32,
        target.y + tap.y.min(target.height - 1) as i32,
    );
    {
        let state = runtime();
        if !state.input_current(generation) {
            return None;
        }
        if !connection.move_pointer(position.0, position.1) {
            return Some(false);
        }
    }
    let (_, _, _, child) = connection.pointer()?;
    if !connection.over(child, target.window) || !keep_input(connection, target, generation) {
        return None;
    }
    let button = if right { 3 } else { 1 };
    let down = {
        let state = runtime();
        state.input_current(generation) && connection.button(button, true)
    };
    if !down {
        connection.button(button, false);
        return Some(false);
    }
    wait_input(connection, target, generation, tap.duration);
    let released = connection.button(button, false);
    if connection
        .pointer()
        .is_some_and(|(x, y, _, _)| (x, y) == position)
        && connection.focused() == Some(target)
    {
        connection.move_pointer(previous_x, previous_y);
    }
    Some(released)
}

fn hold_keys(
    connection: &Connection,
    target: Target,
    keys: &[u16],
    ms: u32,
    generation: i32,
) -> bool {
    let Some(codes) = keys
        .iter()
        .map(|key| connection.keycode(*key))
        .collect::<Option<Vec<_>>>()
    else {
        return false;
    };
    let mut held = Vec::new();
    let mut ok = true;
    for code in codes {
        if !keep_input(connection, target, generation) {
            break;
        }
        let state = runtime();
        if !state.input_current(generation) {
            break;
        }
        if !connection.key(code, true) {
            connection.key(code, false);
            ok = false;
            break;
        }
        held.push(code);
    }
    if held.len() == keys.len() {
        wait_input(
            connection,
            target,
            generation,
            Duration::from_millis(ms.into()),
        );
    }
    for code in held.into_iter().rev() {
        ok &= connection.key(code, false);
    }
    ok
}

fn tilt(connection: &Connection, target: Target, pixels: i32, ms: u32, generation: i32) -> bool {
    let Some((previous_x, previous_y, _, _)) = connection.pointer() else {
        return false;
    };
    let center = (
        target.x + target.width as i32 / 2,
        target.y + target.height as i32 / 2,
    );
    {
        let state = runtime();
        if !state.input_current(generation) {
            return true;
        }
        if !connection.move_pointer(center.0, center.1) {
            return false;
        }
    }
    if !keep_input(connection, target, generation) {
        return false;
    }
    let Some((_, _, _, child)) = connection.pointer() else {
        return false;
    };
    if !connection.over(child, target.window) {
        return false;
    }
    let Some(zoom) = connection.keycode(0x4f) else {
        return false;
    };
    let dragging = {
        let state = runtime();
        if !state.input_current(generation) {
            return true;
        }
        let zooming = ms > 0 && connection.key(zoom, true);
        (ms == 0 || zooming) && connection.button(3, true)
    };
    let mut ok = dragging;
    let mut moved = 0;
    if dragging {
        let start = Instant::now();
        let duration = Duration::from_millis(u64::from(ms.max(350)));
        while start.elapsed() < duration && keep_input(connection, target, generation) {
            let next = ((start.elapsed().as_secs_f64() / 0.35).min(1.0) * f64::from(pixels)).round()
                as i32;
            if next != moved {
                let state = runtime();
                if !state.input_current(generation) {
                    break;
                }
                ok &= connection.relative(next - moved);
                moved = next;
            }
            thread::sleep(Duration::from_millis(12));
        }
        if keep_input(connection, target, generation) && moved != pixels {
            let state = runtime();
            if state.input_current(generation) {
                ok &= connection.relative(pixels - moved);
                moved = pixels;
            }
        }
        ok &= connection.button(3, false);
    } else {
        // The server may have accepted a down event before the connection failed.
        connection.button(3, false);
    }
    if ms > 0 {
        ok &= connection.key(zoom, false);
    }
    if connection
        .pointer()
        .is_some_and(|(x, y, _, _)| (x, y) == (center.0, center.1 + moved))
        && connection.focused() == Some(target)
    {
        connection.move_pointer(previous_x, previous_y);
    }
    ok
}

pub fn cancel_pick() {
    if let Some(reply) = SNAPSHOT.lock().unwrap_or_else(|e| e.into_inner()).take() {
        let _ = reply.send(Err("위치 지정을 취소했습니다.".into()));
    }
}

pub fn snapshot() -> Result<Vec<u8>, String> {
    if session_status().state != "ready" {
        return Err("백그라운드 게임을 먼저 열어 주세요.".into());
    }
    let (send, receive) = mpsc::channel();
    {
        let mut request = SNAPSHOT.lock().unwrap_or_else(|e| e.into_inner());
        if request.is_some() {
            return Err("화면을 이미 요청 중입니다.".into());
        }
        *request = Some(send);
    }
    receive
        .recv_timeout(Duration::from_secs(3))
        .map_err(|_| "게임 화면 확인 시간이 초과됐습니다.".to_owned())?
}

fn encode_snapshot(image: &Image) -> Result<Vec<u8>, String> {
    image
        .frame()
        .ok_or("Sober 화면 데이터가 올바르지 않습니다.")?;
    if image
        .region
        .is_some_and(|r| r.x != 0 || r.y != 0 || r.width != image.width || r.height != image.height)
    {
        return Err("전체 게임 화면이 필요합니다.".into());
    }
    let bytes = image
        .width
        .checked_mul(image.height)
        .and_then(|v| v.checked_mul(4))
        .filter(|bytes| *bytes <= 256 * 1024 * 1024)
        .ok_or("Sober 화면 크기가 너무 큽니다.")?;
    let mut data = Vec::with_capacity(8 + bytes);
    data.extend_from_slice(&(image.width as u32).to_le_bytes());
    data.extend_from_slice(&(image.height as u32).to_le_bytes());
    for y in 0..image.height {
        for x in 0..image.width {
            let offset = y * image.stride + x * image.pixel_stride;
            let pixel = &image.data[offset..offset + 3];
            if image.bgra {
                data.extend_from_slice(&[pixel[2], pixel[1], pixel[0], 255]);
            } else {
                data.extend_from_slice(&[pixel[0], pixel[1], pixel[2], 255]);
            }
        }
    }
    Ok(data)
}

pub fn pick() -> Result<Point, String> {
    Err("앱의 게임 미리보기에서 위치를 지정하세요.".into())
}

/// Explicit integration check: starts one owned Sober only if none is running.
pub(crate) fn game_check() -> Result<(), String> {
    if std::env::var("ACRUX_GAME_TEST").as_deref() != Ok("1") {
        return Err("실제 게임 시작 검사는 명시적으로 활성화해야 합니다.".into());
    }
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let cancel = AtomicBool::new(false);
    let mut owned = session::Session::launch_with_cancel(&cancel)?;
    let finished = std::sync::Arc::new(AtomicBool::new(false));
    struct Finished(std::sync::Arc<AtomicBool>);
    impl Drop for Finished {
        fn drop(&mut self) {
            self.0.store(true, Ordering::Release);
        }
    }
    let _finish = Finished(finished.clone());
    let kill = owned.stop_handle();
    thread::spawn(move || {
        let until = Instant::now() + Duration::from_secs(20);
        while !finished.load(Ordering::Acquire) && Instant::now() < until {
            thread::sleep(Duration::from_millis(20));
        }
        if !finished.load(Ordering::Acquire) {
            let _ = kill.stop();
        }
    });
    let (wire, screen) = owned.connect_x11()?;
    let c = Connection::from_connection(wire, screen)
        .ok_or("전용 게임 입력 연결을 확인하지 못했습니다.")?;
    let end = Instant::now() + Duration::from_secs(15);
    let mut last_target = None;
    let mut captured = false;
    let mut distinct_colors = 0;
    let mut last_capture_error = None;
    loop {
        if !owned.healthy() {
            return Err("전용 Sober가 시작 중 종료됐습니다.".into());
        }
        if let Some(target) = c.focused() {
            last_target = Some((target.width, target.height));
            match c.capture_checked(target, None) {
                Ok(image) => {
                    captured = true;
                    let mut colors = BTreeSet::new();
                    let sample_step = (image.data.len() / image.pixel_stride / 4096).max(1);
                    for pixel in image
                        .data
                        .chunks(image.pixel_stride)
                        .step_by(sample_step)
                        .take(4096)
                    {
                        if pixel.len() >= 3 {
                            colors.insert([pixel[0], pixel[1], pixel[2]]);
                        }
                    }
                    distinct_colors = colors.len();
                    if colors.len() >= 16 {
                        let data = encode_snapshot(&image)?;
                        let mut file = std::fs::OpenOptions::new()
                            .write(true)
                            .create(true)
                            .truncate(true)
                            .mode(0o600)
                            .open("/tmp/acrux-private-game.rgba")
                            .map_err(|e| e.to_string())?;
                        file.write_all(&data).map_err(|e| e.to_string())?;
                        owned.stop()?;
                        println!(
                            "{}",
                            serde_json::json!({"soberWindowCaptured":true,"width":image.width,"height":image.height,
                        "distinctSampleColors":colors.len(),"gameplayVerified":false,"stopped":true})
                        );
                        return Ok(());
                    }
                }
                Err(error) => last_capture_error = Some(error),
            }
        }
        if Instant::now() >= end {
            let focus = c
                .conn
                .get_input_focus()
                .ok()
                .and_then(|cookie| cookie.reply().ok())
                .map(|reply| reply.focus);
            let windows = c
                .conn
                .query_tree(c.root)
                .ok()
                .and_then(|cookie| cookie.reply().ok())
                .map(|tree| {
                    tree.children
                        .into_iter()
                        .take(32)
                        .map(|window| {
                            let class = c
                                .conn
                                .get_property(
                                    false,
                                    window,
                                    AtomEnum::WM_CLASS,
                                    AtomEnum::STRING,
                                    0,
                                    64,
                                )
                                .ok()
                                .and_then(|cookie| cookie.reply().ok())
                                .map(|reply| {
                                    String::from_utf8_lossy(&reply.value).replace('\0', "/")
                                });
                            (window, class)
                        })
                        .collect::<Vec<_>>()
                });
            return Err(owned.startup_error(&format!(
                "전용 Sober의 실제 화면을 확인하지 못했습니다. 창 발견={}, 입력 대상={last_target:?}, 캡처={captured}, 표본 색상={distinct_colors}, 오류={last_capture_error:?}",
                c.available()
            )) + &format!("\n전용 화면 진단: focus={focus:?}, windows={windows:?}"));
        }
        thread::sleep(Duration::from_millis(100));
    }
}

/// Explicit development probe: private display only, never starts Sober.
pub(crate) fn probe() -> Result<(), String> {
    if std::env::var("ACRUX_SESSION_TEST").as_deref() != Ok("1") {
        return Err("독립 화면 검사는 명시적으로 활성화해야 합니다.".into());
    }
    use x11rb::{protocol::Event, wrapper::ConnectionExt as _};
    let cancel = AtomicBool::new(false);
    let mut owned = session::Session::launch_probe_with_cancel(&cancel)?;
    struct ProbeDeadline(std::sync::Arc<AtomicBool>);
    impl Drop for ProbeDeadline {
        fn drop(&mut self) {
            self.0.store(true, Ordering::Release);
        }
    }
    let done = std::sync::Arc::new(AtomicBool::new(false));
    let _deadline = ProbeDeadline(done.clone());
    let kill = owned.stop_handle();
    thread::spawn(move || {
        let end = Instant::now() + Duration::from_secs(5);
        while !done.load(Ordering::Acquire) && Instant::now() < end {
            thread::sleep(Duration::from_millis(20));
        }
        if !done.load(Ordering::Acquire) {
            let _ = kill.stop();
        }
    });
    let (wire, screen) = owned.connect_x11()?;
    let c = Connection::from_connection(wire, screen)
        .ok_or("독립 화면의 XTEST 연결을 확인하지 못했습니다.")?;
    let window = c.conn.generate_id().map_err(|e| e.to_string())?;
    c.conn
        .create_window(
            x11rb::COPY_DEPTH_FROM_PARENT,
            window,
            c.root,
            20,
            20,
            480,
            270,
            0,
            WindowClass::INPUT_OUTPUT,
            0,
            &CreateWindowAux::new()
                .background_pixel(0x2878d0)
                .event_mask(EventMask::BUTTON_PRESS | EventMask::KEY_PRESS | EventMask::EXPOSURE),
        )
        .map_err(|e| e.to_string())?
        .check()
        .map_err(|e| e.to_string())?;
    c.conn
        .change_property8(
            PropMode::REPLACE,
            window,
            AtomEnum::WM_CLASS,
            AtomEnum::STRING,
            b"sober\0org.vinegarhq.Sober\0",
        )
        .map_err(|e| e.to_string())?
        .check()
        .map_err(|e| e.to_string())?;
    c.conn
        .map_window(window)
        .map_err(|e| e.to_string())?
        .check()
        .map_err(|e| e.to_string())?;
    let ready_deadline = Instant::now() + Duration::from_secs(2);
    while c.target(window).is_none() {
        if Instant::now() >= ready_deadline {
            return Err("독립 시험 창이 표시되지 않았습니다.".into());
        }
        thread::sleep(Duration::from_millis(10));
    }
    c.conn
        .set_input_focus(InputFocus::PARENT, window, x11rb::CURRENT_TIME)
        .map_err(|e| e.to_string())?
        .check()
        .map_err(|e| e.to_string())?;
    let target = c
        .target(window)
        .ok_or("독립 시험 창이 준비되지 않았습니다.")?;
    let full = c.capture(target).ok_or("독립 시험 창 캡처 실패")?;
    let crop = c
        .capture_region(
            target,
            Some(Region {
                x: 8,
                y: 8,
                width: 40,
                height: 40,
            }),
        )
        .ok_or("독립 부분 캡처 실패")?;
    let pixel = if crop.bgra {
        [crop.data[2], crop.data[1], crop.data[0]]
    } else {
        [crop.data[0], crop.data[1], crop.data[2]]
    };
    if pixel != [0x28, 0x78, 0xd0] {
        return Err("독립 화면에서 그린 픽셀이 캡처와 다릅니다.".into());
    }
    if crop.data.len() >= full.data.len() {
        return Err("부분 캡처가 전체 프레임보다 작지 않습니다.".into());
    }
    thread::sleep(Duration::from_millis(150));
    let focus_before = c
        .conn
        .get_input_focus()
        .map_err(|e| e.to_string())?
        .reply()
        .map_err(|e| e.to_string())?
        .focus;
    c.move_pointer(target.x + 24, target.y + 24);
    let pointer_after = c.pointer();
    if !c.button(1, true) {
        return Err("독립 버튼 입력 실패".into());
    }
    c.button(1, false);
    let key = c.keycode(0x57).ok_or("독립 키 매핑 실패")?;
    if !c.key(key, true) {
        return Err("독립 키 입력 실패".into());
    }
    c.key(key, false);
    let mut button = false;
    let mut keyboard = false;
    let mut event_kinds = Vec::new();
    let end = Instant::now() + Duration::from_secs(2);
    while Instant::now() < end && !(button && keyboard) {
        match c.conn.poll_for_event().map_err(|e| e.to_string())? {
            Some(Event::ButtonPress(e)) => {
                button |= e.event == window;
                event_kinds.push(format!("button:{}", e.event));
            }
            Some(Event::KeyPress(e)) => {
                keyboard |= e.event == window;
                event_kinds.push(format!("key:{}", e.event));
            }
            Some(event) => {
                if event_kinds.len() < 8 {
                    event_kinds.push(format!("event:{}", event.response_type()));
                }
            }
            _ => thread::sleep(Duration::from_millis(5)),
        }
    }
    c.conn
        .destroy_window(window)
        .map_err(|e| e.to_string())?
        .check()
        .map_err(|e| e.to_string())?;
    owned.stop()?;
    if !button || !keyboard {
        eprintln!(
            "{}",
            serde_json::json!({"fixture":true,"window":window,"focusBefore":focus_before,"pointerAfter":pointer_after,"button":button,"keyboard":keyboard,"events":event_kinds})
        );
        return Err("독립 시험 창에서 버튼·키 이벤트를 확인하지 못했습니다.".into());
    }
    println!(
        "{}",
        serde_json::json!({"fixture":true,"soberLaunched":false,"privateEndpointValidated":true,
        "buttonReceived":button,"keyReceived":keyboard,"pixelVerified":true,"fullBytes":full.data.len(),"roiBytes":crop.data.len(),"stopped":true})
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use x11rb::{protocol::Event, wrapper::ConnectionExt as _};

    #[test]
    fn manual_input_rejects_invalid_or_overflowing_requests_before_queueing() {
        for event in [
            ManualEvent::Look {
                dx: i32::MIN,
                dy: 0,
            },
            ManualEvent::Pointer {
                x: f64::NAN,
                y: 0.5,
                right: false,
                down: true,
            },
            ManualEvent::Pointer {
                x: 1.0,
                y: 0.5,
                right: false,
                down: true,
            },
            ManualEvent::Key {
                vk: 0xffff,
                down: true,
            },
        ] {
            assert!(manual(event, 0).is_err());
        }
    }

    #[test]
    fn preview_encoding_preserves_header_colors_and_row_stride() {
        let image = Image {
            data: vec![10, 20, 30, 0, 40, 50, 60, 0, 99, 99, 99, 99],
            width: 2,
            height: 1,
            stride: 12,
            pixel_stride: 4,
            bgra: true,
            region: None,
        };
        assert_eq!(
            encode_snapshot(&image).unwrap(),
            [
                2u32.to_le_bytes().as_slice(),
                1u32.to_le_bytes().as_slice(),
                &[30, 20, 10, 255, 60, 50, 40, 255]
            ]
            .concat()
        );
        let invalid = Image {
            data: vec![],
            ..image
        };
        assert!(encode_snapshot(&invalid).is_err());
    }

    #[test]
    fn sober_identity_requires_an_exact_class() {
        for value in [b"sober".as_slice(), b"Sober", b"org.vinegarhq.Sober"] {
            assert!(sober_class(value));
        }
        for value in [b"Roblox".as_slice(), b"sober-settings", b"not-sober", b""] {
            assert!(!sober_class(value));
        }
    }

    #[test]
    fn desktop_keys_map_to_x11_symbols() {
        for (vk, symbol) in [
            (0x57, 0x77),
            (0x1b, 0xff1b),
            (0x0d, 0xff0d),
            (0xbf, 0x2f),
            (0x72, 0xffc0),
            (0x75, 0xffc3),
            (0x76, 0xffc4),
        ] {
            assert_eq!(key_symbol(vk), Some(symbol));
        }
        assert_eq!(key_symbol(0xffff), None);
    }

    #[test]
    fn direct_color_conversion_respects_channel_order_padding_and_bounds() {
        let colors: Vec<Rgb> = (0..256u16)
            .map(|index| Rgb {
                red: (255 - index) * 257,
                green: index * 257,
                blue: index * 257,
            })
            .collect();
        let mut bgra = [5, 17, 30, 99, 8, 19, 40, 98, 77, 77, 77, 77];
        normalize_direct_color(&mut bgra, 12, 4, 2, 1, true, &colors).unwrap();
        assert_eq!(bgra, [5, 17, 225, 99, 8, 19, 215, 98, 77, 77, 77, 77]);
        let mut rgb = [5, 17, 30];
        normalize_direct_color(&mut rgb, 3, 3, 1, 1, false, &colors).unwrap();
        assert_eq!(rgb, [250, 17, 30]);
        assert!(normalize_direct_color(&mut rgb, 3, 3, 1, 1, false, &colors[..255]).is_err());
        assert!(normalize_direct_color(&mut rgb, 3, 3, 2, 1, false, &colors).is_err());
        assert!(
            normalize_direct_color(&mut rgb, usize::MAX, 4, 2, usize::MAX, false, &colors).is_err()
        );
    }

    #[test]
    fn capture_layout_checks_masks_padding_and_buffer_length() {
        let mut format = Format {
            depth: 24,
            bits_per_pixel: 32,
            scanline_pad: 32,
        };
        let mut visual = Visualtype {
            visual_id: 1,
            class: VisualClass::TRUE_COLOR,
            bits_per_rgb_value: 8,
            colormap_entries: 256,
            red_mask: 0xff0000,
            green_mask: 0xff00,
            blue_mask: 0xff,
        };
        assert_eq!(
            pixel_layout(7, &format, ImageOrder::LSB_FIRST, &visual),
            Some((28, 4, true))
        );
        format.bits_per_pixel = 24;
        assert_eq!(
            pixel_layout(7, &format, ImageOrder::LSB_FIRST, &visual),
            Some((24, 3, true))
        );
        assert_eq!(
            pixel_layout(7, &format, ImageOrder::MSB_FIRST, &visual),
            None
        );
        visual.red_mask = 0xf800;
        assert_eq!(
            pixel_layout(7, &format, ImageOrder::LSB_FIRST, &visual),
            None
        );
        visual.red_mask = 0xff0000;
        format.scanline_pad = 0;
        assert_eq!(
            pixel_layout(7, &format, ImageOrder::LSB_FIRST, &visual),
            None
        );
        assert!(Image {
            data: vec![0; 20],
            width: 7,
            height: 5,
            stride: 24,
            pixel_stride: 3,
            bgra: true,
            region: None,
        }
        .frame()
        .is_none());
    }

    #[test]
    #[ignore = "requires ACRUX_X11_TEST=1 and a dedicated Xvfb display >= :90"]
    fn native_x11_capture_and_input_direct_color() {
        assert_eq!(std::env::var("ACRUX_X11_TEST").as_deref(), Ok("1"));
        let display = std::env::var("DISPLAY").unwrap();
        assert!(display
            .strip_prefix(':')
            .and_then(|s| s.split('.').next())
            .and_then(|s| s.parse::<u32>().ok())
            .is_some_and(|n| n >= 90));
        let connection = Connection::open().expect("dedicated Xvfb with XTest");
        let c = &connection.conn;
        let visual = c.setup().roots[connection.screen]
            .allowed_depths
            .iter()
            .filter(|depth| depth.depth == 24)
            .flat_map(|depth| &depth.visuals)
            .find(|visual| {
                visual.class == VisualClass::DIRECT_COLOR
                    && visual.colormap_entries == 256
                    && (visual.red_mask, visual.green_mask, visual.blue_mask)
                        == (0xff0000, 0xff00, 0xff)
            })
            .expect("Xvfb must expose the 24-bit DirectColor visual used by Sober");
        let colormap = c.generate_id().unwrap();
        c.create_colormap(
            ColormapAlloc::ALL,
            colormap,
            connection.root,
            visual.visual_id,
        )
        .unwrap()
        .check()
        .unwrap();
        let window = c.generate_id().unwrap();
        c.create_window(
            24,
            window,
            connection.root,
            10,
            10,
            64,
            48,
            0,
            WindowClass::INPUT_OUTPUT,
            visual.visual_id,
            &CreateWindowAux::new()
                .colormap(colormap)
                .background_pixel(0x123456)
                .border_pixel(0),
        )
        .unwrap()
        .check()
        .unwrap();
        c.map_window(window).unwrap().check().unwrap();
        let target = connection.target(window).expect("viewable test window");
        // Change the real server palette without repainting the window. Each
        // capture must query current ramps, rather than treating indices as RGB
        // or reusing a palette from the preceding frame.
        for invert_red in [true, false] {
            let colors: Vec<_> = (0u16..256)
                .map(|index| Coloritem {
                    pixel: u32::from(index) * 0x010101,
                    red: if invert_red { 255 - index } else { index } * 257,
                    green: if invert_red { index } else { 255 - index } * 257,
                    blue: index * 257,
                    flags: ColorFlag::RED | ColorFlag::GREEN | ColorFlag::BLUE,
                })
                .collect();
            c.store_colors(colormap, &colors).unwrap().check().unwrap();
            let image = connection
                .capture_checked(target, None)
                .expect("capture the real DirectColor window");
            let rgba = encode_snapshot(&image).unwrap();
            assert_eq!(&rgba[..4], &64u32.to_le_bytes());
            assert_eq!(&rgba[4..8], &48u32.to_le_bytes());
            assert_eq!(rgba.len(), 8 + 64 * 48 * 4);
            let expected = if invert_red {
                [0xed, 0x34, 0x56, 255]
            } else {
                [0x12, 0xcb, 0x56, 255]
            };
            assert!(
                rgba[8..].chunks_exact(4).all(|pixel| pixel == expected),
                "capture must apply the current nonidentity DirectColor ramps"
            );
        }
        c.destroy_window(window).unwrap().check().unwrap();
        c.free_colormap(colormap).unwrap().check().unwrap();
    }

    #[test]
    #[ignore = "requires ACRUX_X11_TEST=1 and a dedicated Xvfb display >= :90"]
    fn native_x11_capture_and_input() {
        assert_eq!(std::env::var("ACRUX_X11_TEST").as_deref(), Ok("1"));
        let display = std::env::var("DISPLAY").unwrap();
        assert!(display
            .strip_prefix(':')
            .and_then(|s| s.split('.').next())
            .and_then(|s| s.parse::<u32>().ok())
            .is_some_and(|n| n >= 90));
        let connection = Connection::open().expect("dedicated Xvfb with XTest");
        let c = &connection.conn;
        let window = c.generate_id().unwrap();
        c.create_window(
            0,
            window,
            connection.root,
            10,
            10,
            480,
            270,
            0,
            WindowClass::INPUT_OUTPUT,
            0,
            &CreateWindowAux::new().background_pixel(0).event_mask(
                EventMask::KEY_PRESS
                    | EventMask::KEY_RELEASE
                    | EventMask::BUTTON_PRESS
                    | EventMask::BUTTON_RELEASE,
            ),
        )
        .unwrap()
        .check()
        .unwrap();
        c.change_property8(
            PropMode::REPLACE,
            window,
            AtomEnum::WM_CLASS,
            AtomEnum::STRING,
            b"sober\0org.vinegarhq.Sober\0",
        )
        .unwrap()
        .check()
        .unwrap();
        c.map_window(window).unwrap().check().unwrap();
        c.set_input_focus(InputFocus::PARENT, window, x11rb::CURRENT_TIME)
            .unwrap()
            .check()
            .unwrap();
        let wrapper = c.generate_id().unwrap();
        c.create_window(
            x11rb::COPY_DEPTH_FROM_PARENT,
            wrapper,
            connection.root,
            10,
            10,
            500,
            290,
            0,
            WindowClass::INPUT_OUTPUT,
            0,
            &CreateWindowAux::new(),
        )
        .unwrap()
        .check()
        .unwrap();
        c.reparent_window(window, wrapper, 0, 0)
            .unwrap()
            .check()
            .unwrap();
        c.map_window(wrapper).unwrap().check().unwrap();
        c.map_window(window).unwrap().check().unwrap();
        c.change_property32(
            PropMode::REPLACE,
            connection.root,
            connection.clients,
            AtomEnum::WINDOW,
            &[window],
        )
        .unwrap()
        .check()
        .unwrap();
        c.set_input_focus(InputFocus::PARENT, window, x11rb::CURRENT_TIME)
            .unwrap()
            .check()
            .unwrap();
        assert!(
            connection.available(),
            "EWMH must find a reparented Sober client"
        );
        let gc = c.generate_id().unwrap();
        c.create_gc(gc, window, &CreateGCAux::new().foreground(0xf8f8f8))
            .unwrap()
            .check()
            .unwrap();
        let mut points = Vec::new();
        for i in 0..14 {
            for x in [110 + i, 370 - i] {
                for y in [95, 181] {
                    points.push(x11rb::protocol::xproto::Point { x, y });
                }
            }
            for x in [110, 370] {
                for y in [95 + i, 181 - i] {
                    points.push(x11rb::protocol::xproto::Point { x, y });
                }
            }
        }
        c.poly_point(CoordMode::ORIGIN, window, gc, &points)
            .unwrap()
            .check()
            .unwrap();
        c.change_gc(gc, &ChangeGCAux::new().foreground(0x144bcd))
            .unwrap()
            .check()
            .unwrap();
        c.poly_fill_rectangle(
            window,
            gc,
            &[Rectangle {
                x: 170,
                y: 164,
                width: 9,
                height: 4,
            }],
        )
        .unwrap()
        .check()
        .unwrap();
        c.free_gc(gc).unwrap().check().unwrap();
        assert!(connection.available());
        let target = connection
            .focused()
            .expect("fixture is the foreground Sober target");
        let image = connection.capture(target).expect("actual X11 capture");
        let frame = image.frame().unwrap();
        let mut fishing = crate::fishing::Fishing::default();
        fishing.calibrate(0.0).unwrap();
        assert!(fishing.frame(0.1, &frame).is_none());
        assert!(
            fishing.calibration.is_some(),
            "calibrate the actual X11 capture"
        );
        fishing.start(0.2).unwrap();
        let tap = fishing
            .frame(0.3, &frame)
            .expect("cast from captured Fish button");
        assert!((170..179).contains(&tap.x) && (164..168).contains(&tap.y));
        let generation = {
            let mut state = runtime();
            state.stop("isolated test");
            state.fishing.phase = Phase::Searching;
            state.generation
        };
        assert_eq!(
            click(&connection, target, tap, false, generation),
            Some(true)
        );
        assert!(connection.move_pointer(target.x + 20, target.y + 20));
        let before = connection.pointer().unwrap();
        assert!(connection.relative(3));
        let after = connection.pointer().unwrap();
        assert_eq!((after.0, after.1), (before.0, before.1 + 3));
        // A slow focus reply must not revive a canceled manual key-down.
        let mut held = Held::default();
        let checks = std::cell::Cell::new(0);
        assert!(held
            .apply(
                &connection,
                &ManualEvent::Key {
                    vk: 0x57,
                    down: true
                },
                || {
                    checks.set(checks.get() + 1);
                    checks.get() == 1
                },
            )
            .is_err());
        assert!(!connection.pressed(0x57));
        held.apply(
            &connection,
            &ManualEvent::Key {
                vk: 0x57,
                down: true,
            },
            || true,
        )
        .unwrap();
        assert!(connection.pressed(0x57));
        let (reply, _) = mpsc::channel();
        let expired_release = ManualRequest {
            event: ManualEvent::Release,
            generation: -1,
            reply,
            expires: Instant::now() - Duration::from_secs(1),
            canceled: Arc::new(AtomicBool::new(true)),
        };
        held.apply(&connection, &expired_release.event, || {
            expired_release.current()
        })
        .unwrap();
        assert!(!connection.pressed(0x57), "release survives request expiry");
        let stop = thread::spawn(|| {
            thread::sleep(Duration::from_millis(60));
            runtime().stop("isolated stop");
        });
        let began = Instant::now();
        assert!(hold_keys(&connection, target, &[0x57], 5000, generation));
        stop.join().unwrap();
        assert!(began.elapsed() < Duration::from_secs(1));
        assert!(!connection.pressed(0x57), "stop must release held W");
        assert_eq!(
            perform(
                &connection,
                target,
                GameInput::Motion(Motion::Keys(&[0x57], 1)),
                generation
            ),
            None
        );
        let other = c.generate_id().unwrap();
        c.create_window(
            x11rb::COPY_DEPTH_FROM_PARENT,
            other,
            connection.root,
            600,
            10,
            100,
            100,
            0,
            WindowClass::INPUT_OUTPUT,
            0,
            &CreateWindowAux::new(),
        )
        .unwrap()
        .check()
        .unwrap();
        c.map_window(other).unwrap().check().unwrap();
        for resize in [false, true] {
            let generation = {
                let mut state = runtime();
                state.stop("isolated target change");
                state.fishing.phase = Phase::Searching;
                state.generation
            };
            let change = thread::spawn(move || {
                let peer = Connection::open().unwrap();
                thread::sleep(Duration::from_millis(60));
                if resize {
                    peer.conn
                        .configure_window(window, &ConfigureWindowAux::new().width(500))
                        .unwrap()
                        .check()
                        .unwrap();
                } else {
                    peer.conn
                        .set_input_focus(InputFocus::PARENT, other, x11rb::CURRENT_TIME)
                        .unwrap()
                        .check()
                        .unwrap();
                }
            });
            let began = Instant::now();
            assert!(hold_keys(&connection, target, &[0x57], 5000, generation));
            change.join().unwrap();
            assert!(began.elapsed() < Duration::from_secs(1));
            assert!(!connection.pressed(0x57), "target changes must release W");
            assert_eq!(
                perform(
                    &connection,
                    target,
                    GameInput::Motion(Motion::Keys(&[0x57], 1)),
                    generation
                ),
                None
            );
            c.configure_window(window, &ConfigureWindowAux::new().width(480))
                .unwrap()
                .check()
                .unwrap();
            c.set_input_focus(InputFocus::PARENT, window, x11rb::CURRENT_TIME)
                .unwrap()
                .check()
                .unwrap();
        }
        {
            let mut state = runtime();
            state.stop("isolated hotkey test");
            state.fishing.phase = Phase::Searching;
        }
        let hotkey = connection.keycode(0x76).unwrap();
        assert!(connection.key(hotkey, true));
        let generation = runtime().generation;
        assert_eq!(
            perform(
                &connection,
                target,
                GameInput::Motion(Motion::Keys(&[0x57], 1)),
                generation
            ),
            None
        );
        assert!(connection.key(hotkey, false));
        assert!(!runtime().active(), "F7 must stop the macro");
        c.destroy_window(other).unwrap().check().unwrap();
        let (mut presses, mut releases, mut clicks) = (0, 0, 0);
        while let Some(event) = c.poll_for_event().unwrap() {
            match event {
                Event::KeyPress(_) => presses += 1,
                Event::KeyRelease(_) => releases += 1,
                Event::ButtonPress(_) => clicks += 1,
                _ => {}
            }
        }
        assert!(presses >= 1 && releases >= 1 && clicks >= 1);
        c.destroy_window(window).unwrap().check().unwrap();
        assert!(
            connection.capture(target).is_none(),
            "destroyed-window capture must return an error without terminating the process"
        );
        assert!(connection.focused().is_none());
        c.destroy_window(wrapper).unwrap().check().unwrap();
    }
}

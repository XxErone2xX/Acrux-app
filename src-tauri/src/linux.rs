//! Foreground Linux adapter for the same commands used by the main PC app.
#[path = "linux_clipboard.rs"]
mod clipboard;
use crate::{
    fishing::Phase,
    navigation::Motion,
    runtime,
    vision::{Frame, Point},
    GameInput,
};
use evdev::{uinput::VirtualDevice, AttributeSet, InputEvent, KeyCode, RelativeAxisCode};
use serde_json::Value;
use std::{
    collections::BTreeSet,
    io::{self, Read, Write},
    os::unix::net::UnixStream,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicU8, Ordering},
        LazyLock, Mutex,
    },
    thread,
    time::{Duration, Instant},
};
use x11rb::{
    connection::Connection as _,
    protocol::xproto::{self, ConnectionExt, ImageFormat, ImageOrder, InputFocus, VisualClass},
    rust_connection::RustConnection,
    CURRENT_TIME, NONE,
};

static AVAILABLE: AtomicBool = AtomicBool::new(false);
static OPENING: AtomicBool = AtomicBool::new(false);
static PICKING: AtomicU8 = AtomicU8::new(0);
static MESSAGE: LazyLock<Mutex<String>> = LazyLock::new(|| Mutex::new("Sober 연결 준비 중".into()));

fn message(text: impl Into<String>) {
    let text = text.into();
    let mut current = MESSAGE.lock().unwrap_or_else(|e| e.into_inner());
    if *current != text {
        eprintln!("[game] {text}");
        *current = text;
    }
}
pub fn connection_message() -> String {
    MESSAGE.lock().unwrap_or_else(|e| e.into_inner()).clone()
}
pub fn game_available() -> bool {
    AVAILABLE.load(Ordering::Acquire)
}
pub fn geometry() -> Option<(usize, usize)> {
    let desktop = Desktop::new().ok()?;
    let target = desktop.game()?;
    Some((target.width, target.height))
}
pub fn start() -> io::Result<()> {
    thread::Builder::new()
        .name("acrux-linux".into())
        .spawn(worker)
        .map(|_| ())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Target {
    window: u32,
    pid: u32,
    x: i32,
    y: i32,
    width: usize,
    height: usize,
}
struct Desktop {
    conn: RustConnection,
    root: u32,
    hypr: Option<PathBuf>,
}
fn sober(class: &str) -> bool {
    matches!(
        class.to_ascii_lowercase().as_str(),
        "sober" | "org.vinegarhq.sober" | "io.sober.sober"
    )
}
impl Desktop {
    fn new() -> Result<Self, String> {
        let (conn, screen) = RustConnection::connect(None)
            .map_err(|e| format!("게임 화면에 연결하지 못했습니다: {e}"))?;
        let root = conn.setup().roots[screen].root;
        let hypr = std::env::var_os("XDG_RUNTIME_DIR")
            .zip(std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE"))
            .map(|(dir, id)| {
                PathBuf::from(dir)
                    .join("hypr")
                    .join(id)
                    .join(".socket.sock")
            })
            .filter(|p| p.metadata().is_ok_and(|m| m.file_type().is_socket()));
        Ok(Self { conn, root, hypr })
    }
    fn atom(&self, name: &[u8]) -> Option<u32> {
        Some(self.conn.intern_atom(false, name).ok()?.reply().ok()?.atom)
    }
    fn property(&self, window: u32, name: &[u8]) -> Option<Vec<u8>> {
        Some(
            self.conn
                .get_property(
                    false,
                    window,
                    self.atom(name)?,
                    xproto::AtomEnum::ANY,
                    0,
                    4096,
                )
                .ok()?
                .reply()
                .ok()?
                .value,
        )
    }
    fn target(&self, window: u32) -> Option<Target> {
        let class = self.property(window, b"WM_CLASS")?;
        if !class
            .split(|c| *c == 0)
            .any(|s| sober(&String::from_utf8_lossy(s)))
        {
            return None;
        }
        let g = self.conn.get_geometry(window).ok()?.reply().ok()?;
        let a = self.conn.get_window_attributes(window).ok()?.reply().ok()?;
        if a.map_state != xproto::MapState::VIEWABLE || g.width < 40 || g.height < 40 {
            return None;
        }
        let p = self
            .conn
            .translate_coordinates(window, self.root, 0, 0)
            .ok()?
            .reply()
            .ok()?;
        let pid = self
            .property(window, b"_NET_WM_PID")
            .and_then(|p| {
                p.get(..4)
                    .and_then(|v| v.try_into().ok())
                    .map(u32::from_ne_bytes)
            })
            .unwrap_or(0);
        Some(Target {
            window,
            pid,
            x: p.dst_x.into(),
            y: p.dst_y.into(),
            width: g.width.into(),
            height: g.height.into(),
        })
    }
    fn game(&self) -> Option<Target> {
        let children = self.conn.query_tree(self.root).ok()?.reply().ok()?.children;
        children
            .into_iter()
            .filter_map(|w| self.target(w))
            .max_by_key(|t| t.width * t.height)
    }
    fn compositor(&self, request: &str) -> Result<Vec<u8>, String> {
        let mut stream = UnixStream::connect(self.hypr.as_ref().ok_or("창 관리 연결이 없습니다.")?)
            .map_err(|e| e.to_string())?;
        stream
            .set_read_timeout(Some(Duration::from_millis(150)))
            .map_err(|e| e.to_string())?;
        stream
            .set_write_timeout(Some(Duration::from_millis(150)))
            .map_err(|e| e.to_string())?;
        stream
            .write_all(request.as_bytes())
            .map_err(|e| e.to_string())?;
        let mut reply = Vec::new();
        stream
            .take(1024 * 1024)
            .read_to_end(&mut reply)
            .map_err(|e| e.to_string())?;
        Ok(reply)
    }
    fn foreground(&self, target: Target) -> bool {
        if self.target(target.window) != Some(target) {
            return false;
        }
        if self.hypr.is_some() {
            return self
                .compositor("j/activewindow")
                .ok()
                .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
                .is_some_and(|v| {
                    v["class"].as_str().is_some_and(sober)
                        && v["pid"].as_u64() == Some(target.pid as u64)
                });
        }
        self.conn
            .get_input_focus()
            .ok()
            .and_then(|c| c.reply().ok())
            .is_some_and(|v| v.focus == target.window)
    }
    fn focus(&self, target: Target) -> Result<(), String> {
        if self.hypr.is_some() {
            let clients: Value = serde_json::from_slice(&self.compositor("j/clients")?)
                .map_err(|e| e.to_string())?;
            let address = clients
                .as_array()
                .and_then(|a| {
                    a.iter().find(|v| {
                        v["pid"].as_u64() == Some(target.pid as u64)
                            && v["class"].as_str().is_some_and(sober)
                    })
                })
                .and_then(|v| v["address"].as_str())
                .filter(|s| {
                    s.strip_prefix("0x")
                        .is_some_and(|x| !x.is_empty() && x.chars().all(|c| c.is_ascii_hexdigit()))
                })
                .ok_or("Sober 창을 찾지 못했습니다.")?;
            self.compositor(&format!(
                "/dispatch hl.dsp.focus({{ window = 'address:{address}' }})"
            ))?;
        } else {
            self.conn
                .set_input_focus(InputFocus::PARENT, target.window, CURRENT_TIME)
                .map_err(|e| e.to_string())?
                .check()
                .map_err(|e| e.to_string())?;
            self.conn.flush().map_err(|e| e.to_string())?;
        }
        for _ in 0..20 {
            if self.foreground(target) {
                return Ok(());
            }
            thread::sleep(Duration::from_millis(10));
        }
        Err("Sober 창을 앞에 두고 다시 시도하세요.".into())
    }
    fn pointer(&self, target: Target) -> Option<Point> {
        if self.hypr.is_some() {
            let cursor: Value =
                serde_json::from_slice(&self.compositor("j/cursorpos").ok()?).ok()?;
            let active: Value =
                serde_json::from_slice(&self.compositor("j/activewindow").ok()?).ok()?;
            let position = active["at"].as_array()?;
            let size = active["size"].as_array()?;
            let x = (cursor["x"].as_f64()? - position[0].as_f64()?) / size[0].as_f64()?;
            let y = (cursor["y"].as_f64()? - position[1].as_f64()?) / size[1].as_f64()?;
            return ((0.0..1.).contains(&x) && (0.0..1.).contains(&y)).then_some(Point(x, y));
        }
        let p = self.conn.query_pointer(target.window).ok()?.reply().ok()?;
        let x = f64::from(p.win_x) / target.width as f64;
        let y = f64::from(p.win_y) / target.height as f64;
        ((0.0..1.).contains(&x) && (0.0..1.).contains(&y)).then_some(Point(x, y))
    }
    fn position(&self, target: Target, point: Point) -> Result<(), String> {
        if !self.foreground(target) {
            return Err("다른 창으로 전환됐습니다.".into());
        }
        if self.hypr.is_some() {
            let active: Value = serde_json::from_slice(&self.compositor("j/activewindow")?)
                .map_err(|e| e.to_string())?;
            let at = active["at"]
                .as_array()
                .ok_or("게임 창 위치를 확인하지 못했습니다.")?;
            let size = active["size"]
                .as_array()
                .ok_or("게임 창 크기를 확인하지 못했습니다.")?;
            let x = at[0].as_f64().ok_or("게임 창 위치 오류")?
                + point.0 * size[0].as_f64().ok_or("게임 창 크기 오류")?;
            let y = at[1].as_f64().ok_or("게임 창 위치 오류")?
                + point.1 * size[1].as_f64().ok_or("게임 창 크기 오류")?;
            self.compositor(&format!(
                "/dispatch hl.dsp.cursor.move({{ x = {}, y = {} }})",
                x.round() as i32,
                y.round() as i32
            ))?;
        } else {
            let x = target.x + (point.0 * target.width as f64) as i32;
            let y = target.y + (point.1 * target.height as f64) as i32;
            self.conn
                .warp_pointer(
                    NONE,
                    self.root,
                    0,
                    0,
                    0,
                    0,
                    x.try_into().map_err(|_| "마우스 좌표 범위 오류")?,
                    y.try_into().map_err(|_| "마우스 좌표 범위 오류")?,
                )
                .map_err(|e| e.to_string())?
                .check()
                .map_err(|e| e.to_string())?;
            self.conn.flush().map_err(|e| e.to_string())?;
        }
        let actual = self
            .pointer(target)
            .ok_or("실제 마우스 위치를 확인하지 못했습니다.")?;
        if (actual.0 - point.0).abs() * target.width as f64 > 3.
            || (actual.1 - point.1).abs() * target.height as f64 > 3.
        {
            return Err("게임에서 마우스가 지정한 위치에 도달하지 못했습니다.".into());
        }
        Ok(())
    }
    fn pressed(&self, vk: u16) -> bool {
        let Some(symbol) = keysym(vk) else {
            return false;
        };
        let setup = self.conn.setup();
        let Some(mapping) = self
            .conn
            .get_keyboard_mapping(setup.min_keycode, setup.max_keycode - setup.min_keycode + 1)
            .ok()
            .and_then(|c| c.reply().ok())
        else {
            return false;
        };
        let Some(index) = mapping
            .keysyms
            .chunks(mapping.keysyms_per_keycode as usize)
            .position(|row| row.contains(&symbol))
        else {
            return false;
        };
        let code = setup.min_keycode as usize + index;
        self.conn
            .query_keymap()
            .ok()
            .and_then(|c| c.reply().ok())
            .is_some_and(|v| v.keys[code / 8] & (1 << (code % 8)) != 0)
    }
    fn left_down(&self) -> bool {
        self.conn
            .query_pointer(self.root)
            .ok()
            .and_then(|c| c.reply().ok())
            .is_some_and(|p| p.mask.contains(xproto::KeyButMask::BUTTON1))
    }
    fn capture(&self, target: Target) -> Result<Vec<u8>, String> {
        if self.target(target.window) != Some(target) {
            return Err("게임 창 크기나 위치가 바뀌었습니다.".into());
        }
        let image = self
            .conn
            .get_image(
                ImageFormat::Z_PIXMAP,
                target.window,
                0,
                0,
                target.width as u16,
                target.height as u16,
                u32::MAX,
            )
            .map_err(|e| e.to_string())?
            .reply()
            .map_err(|e| e.to_string())?;
        let setup = self.conn.setup();
        let format = setup
            .pixmap_formats
            .iter()
            .find(|f| f.depth == image.depth)
            .ok_or("지원하지 않는 게임 화면 형식입니다.")?;
        let visual = setup
            .roots
            .iter()
            .flat_map(|r| r.allowed_depths.iter())
            .flat_map(|d| &d.visuals)
            .find(|v| v.visual_id == image.visual)
            .ok_or("게임 화면 색상 정보를 읽지 못했습니다.")?;
        if setup.image_byte_order != ImageOrder::LSB_FIRST
            || format.bits_per_pixel != 32
            || (visual.red_mask, visual.green_mask, visual.blue_mask) != (0xff0000, 0xff00, 0xff)
        {
            return Err("게임 화면 색상 형식을 지원하지 않습니다.".into());
        }
        let stride = (target.width * 32).div_ceil(format.scanline_pad as usize)
            * format.scanline_pad as usize
            / 8;
        if image.data.len() < stride * target.height {
            return Err("게임 화면 데이터가 잘렸습니다.".into());
        }
        let ramps = if visual.class == VisualClass::DIRECT_COLOR {
            let indices: Vec<u32> = (0..256).map(|i| i * 0x010101).collect();
            Some(
                self.conn
                    .query_colors(
                        self.conn
                            .get_window_attributes(target.window)
                            .map_err(|e| e.to_string())?
                            .reply()
                            .map_err(|e| e.to_string())?
                            .colormap,
                        &indices,
                    )
                    .map_err(|e| e.to_string())?
                    .reply()
                    .map_err(|e| e.to_string())?
                    .colors,
            )
        } else {
            None
        };
        let mut rgba = Vec::with_capacity(8 + target.width * target.height * 4);
        rgba.extend_from_slice(&(target.width as u32).to_le_bytes());
        rgba.extend_from_slice(&(target.height as u32).to_le_bytes());
        for row in image.data.chunks(stride).take(target.height) {
            for p in row[..target.width * 4].chunks_exact(4) {
                if let Some(r) = &ramps {
                    rgba.extend_from_slice(&[
                        (r[p[2] as usize].red >> 8) as u8,
                        (r[p[1] as usize].green >> 8) as u8,
                        (r[p[0] as usize].blue >> 8) as u8,
                        255,
                    ]);
                } else {
                    rgba.extend_from_slice(&[p[2], p[1], p[0], 255]);
                }
            }
        }
        Ok(rgba)
    }
}
use std::os::unix::fs::FileTypeExt;

struct Input {
    device: VirtualDevice,
    held: BTreeSet<u16>,
}
impl Input {
    fn new() -> io::Result<Self> {
        let mut keys = AttributeSet::new();
        for vk in 0..=255 {
            if let Some(code) = keycode(vk) {
                keys.insert(KeyCode::new(code));
            }
        }
        keys.insert(KeyCode::BTN_LEFT);
        keys.insert(KeyCode::BTN_RIGHT);
        let mut axes = AttributeSet::new();
        axes.insert(RelativeAxisCode::REL_X);
        axes.insert(RelativeAxisCode::REL_Y);
        let device = VirtualDevice::builder()?
            .name("Acrux main input")
            .with_keys(&keys)?
            .with_relative_axes(&axes)?
            .build()?;
        Ok(Self {
            device,
            held: BTreeSet::new(),
        })
    }
    fn key(&mut self, code: u16, down: bool) -> bool {
        if !down && !self.held.contains(&code) {
            return true;
        }
        let value = if down && self.held.contains(&code) {
            2
        } else {
            i32::from(down)
        };
        if self
            .device
            .emit(&[InputEvent::new(1, code, value)])
            .is_err()
        {
            return false;
        }
        if down {
            self.held.insert(code);
        } else {
            self.held.remove(&code);
        }
        true
    }
    fn relative(&mut self, x: i32, y: i32) -> bool {
        self.device
            .emit(&[InputEvent::new(2, 0, x), InputEvent::new(2, 1, y)])
            .is_ok()
    }
    fn release(&mut self) {
        for key in self.held.clone() {
            self.key(key, false);
        }
    }
}
impl Drop for Input {
    fn drop(&mut self) {
        self.release();
    }
}
fn keycode(vk: u16) -> Option<u16> {
    Some(match vk {
        0xbf => 53,
        0x30 => 11,
        0x31..=0x39 => vk - 0x31 + 2,
        0x1b => 1,
        0x0d => 28,
        0x20 => 57,
        0x10 => 42,
        0x11 => 29,
        0x09 => 15,
        0x08 => 14,
        0x25 => 105,
        0x26 => 103,
        0x27 => 106,
        0x28 => 108,
        0x70..=0x79 => 59 + vk - 0x70,
        0x7a => 87,
        0x7b => 88,
        0x41..=0x5a => [
            30, 48, 46, 32, 18, 33, 34, 35, 23, 36, 37, 38, 50, 49, 24, 25, 16, 19, 31, 20, 22, 47,
            17, 45, 21, 44,
        ][(vk - 0x41) as usize],
        _ => return None,
    })
}
fn keysym(vk: u16) -> Option<u32> {
    Some(match vk {
        0x41..=0x5a => (vk + 32).into(),
        0x70..=0x7b => 0xffbe + u32::from(vk - 0x70),
        0x1b => 0xff1b,
        0x0d => 0xff0d,
        0x20 => 0x20,
        _ => return None,
    })
}
fn current(desktop: &Desktop, target: Target, generation: i32) -> bool {
    if !runtime().input_current(generation) {
        return false;
    }
    if desktop.pressed(0x72) || desktop.pressed(0x76) {
        runtime().stop("단축키로 중지됨");
        return false;
    }
    desktop.foreground(target)
}
fn wait(desktop: &Desktop, target: Target, generation: i32, ms: u32) -> bool {
    let end = Instant::now() + Duration::from_millis(ms.into());
    while Instant::now() < end {
        if !current(desktop, target, generation) {
            return false;
        }
        thread::sleep(Duration::from_millis(8).min(end.saturating_duration_since(Instant::now())));
    }
    true
}
fn perform(
    desktop: &Desktop,
    input: &mut Input,
    target: Target,
    command: GameInput,
    generation: i32,
) -> bool {
    if !current(desktop, target, generation) {
        return false;
    }
    match command {
        GameInput::Text { text, submit } => {
            replace_text(desktop, input, target, text, submit, generation)
        }
        GameInput::Tap(tap) => click(
            desktop,
            input,
            target,
            Point(
                tap.x as f64 / target.width as f64,
                tap.y as f64 / target.height as f64,
            ),
            false,
            tap.duration.as_millis() as u32,
            generation,
        ),
        GameInput::Motion(Motion::Tap(point, ms)) => {
            click(desktop, input, target, point, false, ms, generation)
        }
        GameInput::Motion(Motion::Click { point, right }) => {
            click(desktop, input, target, point, right, 40, generation)
        }
        GameInput::Motion(Motion::Keys(keys, ms)) => {
            let codes: Vec<_> = keys.iter().filter_map(|&k| keycode(k)).collect();
            if codes.len() != keys.len() {
                return false;
            }
            for &code in &codes {
                if !input.key(code, true) {
                    input.release();
                    return false;
                }
            }
            let end = Instant::now() + Duration::from_millis(ms.into());
            let mut ok = true;
            while Instant::now() < end {
                if !current(desktop, target, generation) {
                    ok = false;
                    break;
                }
                for &code in &codes {
                    ok &= input.key(code, true);
                }
                thread::sleep(
                    Duration::from_millis(33).min(end.saturating_duration_since(Instant::now())),
                );
            }
            for &code in codes.iter().rev() {
                ok &= input.key(code, false);
            }
            ok
        }
        GameInput::Motion(Motion::KeyEvent { vk, down }) => {
            keycode(vk).is_some_and(|code| input.key(code, down))
        }
        GameInput::Motion(Motion::KeysOverlap {
            main,
            extra,
            ms,
            extra_at,
        }) => {
            let (Some(main), Some(extra)) = (keycode(main), keycode(extra)) else {
                return false;
            };
            let start = Instant::now();
            let mut secondary = false;
            let mut ok = input.key(main, true);
            while start.elapsed() < Duration::from_millis(ms.into())
                && current(desktop, target, generation)
            {
                ok &= input.key(main, true);
                if start.elapsed() >= Duration::from_millis(extra_at.into()) {
                    ok &= input.key(extra, true);
                    secondary = true;
                }
                thread::sleep(Duration::from_millis(33));
            }
            if secondary {
                ok &= input.key(extra, false);
            }
            ok &= input.key(main, false);
            ok
        }
        GameInput::Motion(Motion::Tilt { pixels, ms }) => {
            if let Err(error) = desktop.position(target, Point(0.5, 0.5)) {
                message(error);
                return false;
            }
            let mut ok = input.key(0x111, true);
            let mut moved = 0;
            while moved != pixels && current(desktop, target, generation) {
                let step = (pixels - moved).clamp(-20, 20);
                ok &= input.relative(0, step);
                moved += step;
                if !ok || !wait(desktop, target, generation, 10) {
                    break;
                }
            }
            ok &= input.key(0x111, false);
            if moved != pixels {
                return false;
            }
            if ms > 0 {
                let zoom = keycode(0x4f).unwrap();
                ok &= input.key(zoom, true);
                let end = Instant::now() + Duration::from_millis(ms.into());
                while ok && Instant::now() < end && current(desktop, target, generation) {
                    ok &= input.key(zoom, true);
                    thread::sleep(
                        Duration::from_millis(33)
                            .min(end.saturating_duration_since(Instant::now())),
                    );
                }
                ok &= input.key(zoom, false);
            }
            ok
        }
        GameInput::Motion(_) => false,
    }
}
fn replace_text(
    desktop: &Desktop,
    input: &mut Input,
    target: Target,
    text: String,
    submit: bool,
    generation: i32,
) -> bool {
    input.release();
    let clipboard = match clipboard::stage(text) {
        Ok(clipboard) => clipboard,
        Err(error) => {
            message(error);
            return false;
        }
    };
    let mut key = |vk: u16, modifiers: &[u16]| -> bool {
        if !current(desktop, target, generation) {
            return false;
        }
        let Some(code) = keycode(vk) else {
            return false;
        };
        for &modifier in modifiers {
            let Some(modifier) = keycode(modifier) else {
                input.release();
                return false;
            };
            if !input.key(modifier, true) {
                input.release();
                return false;
            }
        }
        let down = current(desktop, target, generation) && input.key(code, true);
        let held = down && wait(desktop, target, generation, 20);
        let up = input.key(code, false);
        input.release();
        down && held && up && wait(desktop, target, generation, 50)
    };
    let mut ok = key(0x41, &[0x11]) && key(0x08, &[]) && key(0x56, &[0x11]);
    if ok {
        ok = wait(desktop, target, generation, 350);
    }
    if let Err(error) = clipboard.restore() {
        message(error);
        ok = false;
    }
    if ok && submit {
        ok = key(0x0d, &[]);
    }
    input.release();
    ok
}
fn click(
    desktop: &Desktop,
    input: &mut Input,
    target: Target,
    point: Point,
    right: bool,
    ms: u32,
    generation: i32,
) -> bool {
    let settled = !right
        && desktop.pointer(target).is_some_and(|p| {
            (p.0 - point.0).abs() * target.width as f64 <= 2.
                && (p.1 - point.1).abs() * target.height as f64 <= 2.
        });
    if !settled {
        if let Err(error) = desktop.position(target, point) {
            message(error);
            return false;
        }
        thread::sleep(Duration::from_millis(10));
        if !input.relative(1, 0) || !input.relative(-1, 0) || !wait(desktop, target, generation, 30)
        {
            return false;
        }
    }
    let button = if right { 0x111 } else { 0x110 };
    let down = input.key(button, true);
    let held = down && wait(desktop, target, generation, ms);
    let up = input.key(button, false);
    down && held && up
}
fn worker() {
    let desktop = match Desktop::new() {
        Ok(d) => d,
        Err(error) => {
            message(error);
            return;
        }
    };
    let mut input = Input::new()
        .map_err(|e| {
            message(format!("입력 장치를 준비하지 못했습니다: {e}"));
            e
        })
        .ok();
    {
        let mut state = runtime();
        state.capture = true;
        state.input = input.is_some();
    }
    let mut previous_generation = runtime().generation;
    let mut recording_seen = false;
    let mut finish_held = false;
    let mut safe_held = false;
    loop {
        let began = Instant::now();
        let target = desktop.game();
        let safe_down = desktop.pressed(0x4b);
        if safe_down && !safe_held && target.is_some_and(|target| desktop.foreground(target)) {
            crate::biome::skip_safe();
        }
        safe_held = safe_down;
        AVAILABLE.store(target.is_some(), Ordering::Release);
        if input.is_some() && (target.is_some() || !OPENING.load(Ordering::Acquire)) {
            message(if target.is_some() {
                "Sober 연결됨"
            } else {
                "Sober 창을 열어 주세요."
            });
        }
        let (phase, generation) = {
            let state = runtime();
            (state.phase(), state.generation)
        };
        if generation != previous_generation || !phase.active() {
            if let Some(input) = input.as_mut() {
                input.release();
            }
            previous_generation = generation;
        }
        if phase.active() {
            if desktop.pressed(0x72) || desktop.pressed(0x76) {
                runtime().stop("단축키로 중지됨");
                continue;
            }
            let recording = runtime()
                .job
                .as_ref()
                .is_some_and(|job| job.desktop_recording());
            if recording {
                let keys: Vec<_> = [0x57, 0x41, 0x53, 0x44, 0x20]
                    .into_iter()
                    .filter(|&key| desktop.pressed(key))
                    .collect();
                let point = target
                    .filter(|t| desktop.foreground(*t))
                    .and_then(|t| desktop.pointer(t))
                    .unwrap_or(Point(f64::NAN, f64::NAN));
                runtime().record_desktop_sample(&keys, desktop.left_down(), point, crate::now());
                let finish = desktop.pressed(0x75);
                if recording_seen && finish && !finish_held {
                    runtime().arrive_desktop();
                    finish_held = finish;
                    continue;
                }
                recording_seen = true;
                finish_held = finish;
            } else {
                recording_seen = false;
                finish_held = false;
            }
            if let Some(target) = target.filter(|t| desktop.foreground(*t)) {
                let (command, command_generation) = {
                    let mut state = runtime();
                    let command = state.headless_navigation((target.width, target.height));
                    (command, state.generation)
                };
                if command_generation != previous_generation {
                    if let Some(input) = input.as_mut() {
                        input.release();
                    }
                    previous_generation = command_generation;
                }
                if let (Some(command), Some(input)) = (command, input.as_mut()) {
                    let completed = perform(&desktop, input, target, command, command_generation);
                    crate::desktop_input_ack(command_generation, completed);
                    if !completed && runtime().input_current(command_generation) {
                        input.release();
                        runtime().stop("게임 입력을 완료하지 못해 중지했습니다.");
                    }
                }
                let capture = {
                    let state = runtime();
                    state.needs_frame()
                };
                if capture {
                    match desktop.capture(target) {
                        Ok(bytes) => {
                            if let Some(frame) = Frame::new(
                                &bytes[8..],
                                target.width,
                                target.height,
                                target.width * 4,
                                4,
                                false,
                            ) {
                                let (command, command_generation) = {
                                    let command = crate::desktop_frame(&frame);
                                    (command, runtime().generation)
                                };
                                if let (Some(command), Some(input)) = (command, input.as_mut()) {
                                    let completed = perform(
                                        &desktop,
                                        input,
                                        target,
                                        command,
                                        command_generation,
                                    );
                                    crate::desktop_input_ack(command_generation, completed);
                                    if !completed && runtime().input_current(command_generation) {
                                        input.release();
                                        runtime().stop("게임 입력을 완료하지 못해 중지했습니다.");
                                    }
                                }
                            }
                        }
                        Err(error) => {
                            message(error);
                            runtime().no_target();
                        }
                    }
                }
            } else {
                if let Some(input) = input.as_mut() {
                    input.release();
                }
                if !recording {
                    runtime().no_target();
                }
            }
        }
        let navigation = runtime().job.is_some();
        thread::sleep(
            Duration::from_millis(if navigation {
                8
            } else if phase == Phase::Reeling {
                8
            } else if phase.active() {
                50
            } else {
                100
            })
            .saturating_sub(began.elapsed()),
        );
    }
}
pub fn focus_game() -> Result<(), String> {
    let desktop = Desktop::new()?;
    let target = desktop.game().ok_or("Sober 창을 먼저 열어 주세요.")?;
    desktop.focus(target)
}
pub fn snapshot() -> Result<Vec<u8>, String> {
    let desktop = Desktop::new()?;
    let target = desktop.game().ok_or("Sober 창을 먼저 열어 주세요.")?;
    desktop.capture(target)
}
pub fn open_game() -> Result<(), String> {
    if focus_game().is_ok() {
        return Ok(());
    }
    if OPENING
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return Err("Sober 실행 중입니다. 게임 창 연결을 기다려 주세요.".into());
    }
    struct Opening;
    impl Drop for Opening {
        fn drop(&mut self) {
            OPENING.store(false, Ordering::Release);
        }
    }
    let _opening = Opening;
    use std::{
        fs::OpenOptions,
        os::unix::fs::OpenOptionsExt,
        process::{Command, Stdio},
    };
    let log = runtime()
        .settings
        .as_ref()
        .and_then(|p| p.parent())
        .ok_or("실행 로그 저장소가 준비되지 않았습니다.")?
        .join("main-port-sober.log");
    let output = OpenOptions::new()
        .create(true)
        .append(true)
        .mode(0o600)
        .open(&log)
        .map_err(|e| format!("Sober 실행 로그를 열지 못했습니다: {e}"))?;
    message("Sober 실행 중 · 게임 창 연결을 기다립니다.");
    eprintln!("[game] starting Sober with visible X11 window");
    let mut child = Command::new("flatpak")
        .args([
            "run",
            "--socket=x11",
            "--nosocket=wayland",
            "--env=SDL_VIDEODRIVER=x11",
            "--env=GDK_BACKEND=x11",
            "org.vinegarhq.Sober",
            "roblox://placeId=15532962292",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::from(output.try_clone().map_err(|e| e.to_string())?))
        .stderr(Stdio::from(output))
        .spawn()
        .map_err(|e| format!("Sober를 열지 못했습니다: {e}"))?;
    let desktop = Desktop::new()?;
    let started = Instant::now();
    while started.elapsed() < Duration::from_secs(20) {
        if let Some(target) = desktop.game() {
            AVAILABLE.store(true, Ordering::Release);
            message("Sober 연결됨");
            desktop.focus(target)?;
            eprintln!(
                "[game] Sober window connected: {}x{}",
                target.width, target.height
            );
            return Ok(());
        }
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
            if !status.success() {
                let error =
                    format!("Sober 실행이 종료됐습니다 ({status}). 실행 로그를 확인하세요.");
                message(error.clone());
                return Err(error);
            }
        }
        thread::sleep(Duration::from_millis(100));
    }
    let error = "Sober의 캡처 가능한 게임 창을 찾지 못했습니다. 실행 로그를 확인하세요.";
    message(error);
    Err(error.into())
}

pub fn cancel_pick() {
    let _ = PICKING.compare_exchange(1, 2, Ordering::AcqRel, Ordering::Acquire);
}
pub fn pick() -> Result<Point, String> {
    let point = pick_area(false)?;
    Ok(Point(point[0], point[1]))
}
pub fn pick_region() -> Result<Vec<f64>, String> {
    pick_area(true)
}
fn pick_area(region: bool) -> Result<Vec<f64>, String> {
    if runtime().phase().active() {
        return Err("실행 중인 작업을 먼저 중지하세요.".into());
    }
    if PICKING
        .compare_exchange(0, 1, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return Err("위치를 이미 지정 중입니다.".into());
    }
    struct Clear;
    impl Drop for Clear {
        fn drop(&mut self) {
            PICKING.store(0, Ordering::Release);
        }
    }
    let _clear = Clear;
    let desktop = Desktop::new()?;
    let target = desktop.game().ok_or("Sober 창을 먼저 열어 주세요.")?;
    desktop.focus(target)?;
    let deadline = Instant::now() + Duration::from_secs(120);
    let mut armed = false;
    let mut first = None;
    loop {
        if PICKING.load(Ordering::Acquire) != 1 || desktop.pressed(0x1b) {
            return Err("위치 지정을 취소했습니다.".into());
        }
        if Instant::now() > deadline {
            return Err("위치 지정 시간이 지났습니다.".into());
        }
        if !desktop.foreground(target) {
            return Err("게임 창이 바뀌어 지정을 취소했습니다.".into());
        }
        let down = desktop.left_down();
        if !armed {
            armed = !down;
        } else if down && first.is_none() {
            if let Some(point) = desktop.pointer(target) {
                if !region {
                    return Ok(vec![point.0, point.1]);
                }
                first = Some(point);
            }
        } else if !down {
            if let (Some(first), Some(last)) = (first, desktop.pointer(target)) {
                let area = vec![
                    first.0.min(last.0),
                    first.1.min(last.1),
                    first.0.max(last.0),
                    first.1.max(last.1),
                ];
                if area[0] >= area[2] || area[1] >= area[3] {
                    return Err("서로 다른 두 모서리를 드래그하세요.".into());
                }
                return Ok(area);
            }
        }
        thread::sleep(Duration::from_millis(8));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn accepts_only_the_real_sober_identity() {
        assert!(sober("org.vinegarhq.Sober"));
        assert!(sober("Sober"));
        assert!(!sober("Sober helper"));
        assert!(!sober("Roblox webpage"));
    }
    #[test]
    fn movement_keys_map_to_physical_linux_codes() {
        assert_eq!(keycode(0x57), Some(17));
        assert_eq!(keycode(0x41), Some(30));
        assert_eq!(keycode(0x53), Some(31));
        assert_eq!(keycode(0x44), Some(32));
        assert_eq!(keycode(0x20), Some(57));
        assert_eq!(keycode(0), None);
    }
}

use crate::{
    fishing::{Phase, Tap},
    now, runtime,
    vision::{Frame, Point},
    DeviceTick, GameInput, Motion,
};
use std::{
    io,
    sync::atomic::{AtomicBool, AtomicU8, Ordering},
    thread,
    time::{Duration, Instant},
};
use x11rb::{
    connection::Connection as _,
    protocol::{xproto::*, xtest::ConnectionExt as _},
    rust_connection::RustConnection,
};

const SETUP: &str = "Linux 입력에는 Sober X11이 필요합니다. Sober를 X11 모드로 실행하세요.";
static AVAILABLE: AtomicBool = AtomicBool::new(false);
static PICKING: AtomicU8 = AtomicU8::new(0);

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
    fn open() -> Option<Self> {
        // Only the caller's DISPLAY/XAUTHORITY session is used. Protocol and
        // connection errors are local Result values, independent of GTK/Xlib.
        let (conn, screen) = RustConnection::connect(None).ok()?;
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
        let reply = self
            .conn
            .get_image(
                ImageFormat::Z_PIXMAP,
                target.window,
                0,
                0,
                target.width.try_into().ok()?,
                target.height.try_into().ok()?,
                u32::MAX,
            )
            .ok()?
            .reply()
            .ok()?;
        let setup = self.conn.setup();
        let format = setup
            .pixmap_formats
            .iter()
            .find(|format| format.depth == reply.depth)?;
        let visual = setup
            .roots
            .get(self.screen)?
            .allowed_depths
            .iter()
            .flat_map(|depth| &depth.visuals)
            .find(|visual| visual.visual_id == reply.visual)?;
        let (stride, pixel_stride, bgra) =
            pixel_layout(target.width, format, setup.image_byte_order, visual)?;
        let image = Image {
            data: reply.data,
            width: target.width,
            height: target.height,
            stride,
            pixel_stride,
            bgra,
        };
        image.frame()?;
        Some(image)
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
}

impl Image {
    fn frame(&self) -> Option<Frame<'_>> {
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
        || visual.class != VisualClass::TRUE_COLOR
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

fn sober_class(value: &[u8]) -> bool {
    value.eq_ignore_ascii_case(b"sober") || value.eq_ignore_ascii_case(b"org.vinegarhq.Sober")
}

fn key_symbol(vk: u16) -> Option<u32> {
    Some(match vk {
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
    if Connection::open().is_none() {
        let mut state = runtime();
        state.capture = false;
        state.input = false;
        state.stop(SETUP);
        return Ok(());
    }
    thread::Builder::new()
        .name("acrux-x11".into())
        .spawn(worker)?;
    Ok(())
}

pub fn game_available() -> bool {
    AVAILABLE.load(Ordering::Acquire)
}

fn worker() {
    let Some(connection) = Connection::open() else {
        let mut state = runtime();
        state.capture = false;
        state.input = false;
        state.stop(SETUP);
        return;
    };
    {
        let mut state = runtime();
        state.capture = true;
        state.input = true;
    }
    let mut previous = None;
    loop {
        let started = Instant::now();
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
            state.stop("Sober X11 연결이 끊겼습니다. 앱을 다시 실행하세요.");
            return;
        }
        AVAILABLE.store(connection.available(), Ordering::Release);
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
        if matches!(step, DeviceTick::Observe) {
            let image = connection.capture(target);
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
                    state.stop("Sober X11 화면을 캡처하지 못했습니다.");
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
                            state.input_rejected("Sober X11 입력을 보내지 못했습니다.");
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
    if !runtime().input_current(generation) {
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
    let _ = PICKING.compare_exchange(1, 2, Ordering::AcqRel, Ordering::Acquire);
}

pub fn pick() -> Result<Point, String> {
    if runtime().active() {
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
    let connection = Connection::open().ok_or(SETUP)?;
    let start = Instant::now();
    let mut previous = None;
    let mut ready = false;
    while start.elapsed() < Duration::from_secs(60) {
        if PICKING.load(Ordering::Acquire) != 1 || connection.pressed(0x1b) || runtime().active() {
            return Err("위치 지정을 취소했습니다.".into());
        }
        let target = connection.focused();
        if target != previous {
            ready = false;
            previous = target;
        }
        if let Some(target) = target {
            if let Some((x, y, mask, child)) = connection.pointer() {
                let down = mask & u32::from(u16::from(KeyButMask::BUTTON1)) != 0;
                let (x, y) = (x - target.x, y - target.y);
                if ready
                    && down
                    && connection.over(child, target.window)
                    && (0..target.width as i32).contains(&x)
                    && (0..target.height as i32).contains(&y)
                {
                    return Ok(Point(
                        f64::from(x) / target.width as f64,
                        f64::from(y) / target.height as f64,
                    ));
                }
                ready = !down;
            }
        }
        thread::sleep(Duration::from_millis(15));
    }
    Err("Sober X11 화면에서 위치를 선택하지 않았습니다.".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use x11rb::{protocol::Event, wrapper::ConnectionExt as _};

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
            bgra: true
        }
        .frame()
        .is_none());
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

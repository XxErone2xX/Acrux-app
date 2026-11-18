use crate::{
    fishing::{Phase, Tap},
    runtime,
    vision::{Frame, Point},
    GameInput, Motion,
};
use std::{
    ffi::c_void,
    io,
    mem::size_of,
    ptr::null_mut,
    sync::atomic::{AtomicU8, Ordering},
    thread,
    time::{Duration, Instant},
};
use windows_sys::Win32::{
    Foundation::{CloseHandle, HWND, POINT, RECT},
    Graphics::Gdi::{
        BitBlt, ClientToScreen, CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject,
        GdiFlush, GetDC, ReleaseDC, SelectObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB,
        DIB_RGB_COLORS, HBITMAP, HDC, HGDIOBJ, SRCCOPY,
    },
    System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION,
    },
    UI::{
        Input::KeyboardAndMouse::{
            GetAsyncKeyState, MapVirtualKeyW, SendInput, INPUT, INPUT_0, INPUT_KEYBOARD,
            INPUT_MOUSE, KEYBDINPUT, KEYEVENTF_EXTENDEDKEY, KEYEVENTF_KEYUP, KEYEVENTF_SCANCODE,
            KEYEVENTF_UNICODE, MAPVK_VK_TO_VSC, MOUSEEVENTF_ABSOLUTE, MOUSEEVENTF_LEFTDOWN,
            MOUSEEVENTF_LEFTUP, MOUSEEVENTF_MOVE, MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP,
            MOUSEEVENTF_VIRTUALDESK, MOUSEINPUT, VK_ESCAPE, VK_F3, VK_F6, VK_F7, VK_LBUTTON,
        },
        WindowsAndMessaging::{
            FindWindowExW, GetAncestor, GetClassNameW, GetClientRect, GetCursorPos,
            GetForegroundWindow, GetSystemMetrics, GetWindowThreadProcessId, IsIconic,
            IsWindowVisible, SetForegroundWindow, ShowWindow, WindowFromPoint, GA_ROOT,
            SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN,
            SW_RESTORE,
        },
    },
};

pub fn start() -> io::Result<()> {
    {
        let mut state = runtime();
        state.capture = true;
        state.input = true;
    }
    thread::Builder::new()
        .name("acrux-capture".into())
        .spawn(worker)?;
    Ok(())
}

fn worker() {
    let mut foreground = null_mut();
    let mut foreground_pid = 0;
    let mut is_game = false;
    let mut hotkey_held = false;
    let mut arrive_held = false;
    let mut recording_seen = false;
    let mut record_geometry = None;
    let mut surface = None;
    let mut held = HeldKeys::default();
    loop {
        let tick = Instant::now();
        let (active, generation) = {
            let state = runtime();
            (state.phase(), state.generation)
        };
        let hwnd = unsafe { GetForegroundWindow() };
        held.sync(hwnd, generation, active.active());
        if active == Phase::Idle {
            surface = None;
            hotkey_held = false;
            recording_seen = false;
            arrive_held = pressed(VK_F6);
            thread::sleep(Duration::from_millis(100));
            continue;
        }
        let hotkey = pressed(VK_F3) || pressed(VK_F7);
        if hotkey && !hotkey_held {
            runtime().stop("단축키로 중지됨");
            held.release();
            continue;
        }
        hotkey_held = hotkey;
        let mut pid = 0;
        if !hwnd.is_null() {
            unsafe { GetWindowThreadProcessId(hwnd, &mut pid) };
        }
        if hwnd != foreground || pid != foreground_pid {
            foreground = hwnd;
            foreground_pid = pid;
            is_game = pid != 0 && game_process(pid);
        }
        let geometry = is_game.then(|| client(hwnd)).flatten();
        if let Some((_, width, height)) = geometry {
            record_geometry = Some((width as usize, height as usize));
        }
        let recording = runtime()
            .job
            .as_ref()
            .is_some_and(|job| job.desktop_recording());
        if recording {
            let keys: Vec<_> = [0x57, 0x41, 0x53, 0x44, 0x20]
                .into_iter()
                .filter(|&key| pressed(key))
                .collect();
            let mut cursor = POINT::default();
            let point = geometry
                .filter(|_| unsafe { GetCursorPos(&mut cursor) } != 0)
                .map_or(Point(f64::NAN, f64::NAN), |(origin, w, h)| {
                    Point(
                        f64::from(cursor.x - origin.x) / f64::from(w),
                        f64::from(cursor.y - origin.y) / f64::from(h),
                    )
                });
            let now = crate::now();
            runtime().record_desktop_sample(&keys, pressed(VK_LBUTTON), point, now);
            let arrived = pressed(VK_F6);
            if recording_seen && arrived && !arrive_held {
                runtime().arrive_desktop();
            }
            recording_seen = true;
            arrive_held = arrived;
            // Advance the record timeout without capturing full frames.
            if let Some(geometry) = record_geometry {
                runtime().headless_navigation(geometry);
            }
            thread::sleep(Duration::from_millis(4).saturating_sub(tick.elapsed()));
            continue;
        }
        recording_seen = false;
        let arrived = pressed(VK_F6);
        if arrived && !arrive_held && runtime().arrive_desktop() {
            arrive_held = arrived;
            continue;
        }
        arrive_held = arrived;
        let Some((origin, w, h)) = geometry else {
            runtime().no_target();
            held.release();
            surface = None;
            thread::sleep(Duration::from_millis(100));
            continue;
        };
        let (mut input, mut generation, capture) = {
            let mut state = runtime();
            let input = state.headless_navigation((w as usize, h as usize));
            let capture = state.needs_frame();
            (input, state.generation, capture)
        };
        if input.is_none() && capture {
            let source = ScreenDc::new();
            if surface
                .as_ref()
                .is_none_or(|s: &Surface| (s.width, s.height) != (w, h))
            {
                surface = Surface::new(source.0, w, h);
            }
            input = surface
                .as_mut()
                .and_then(|s| s.capture(source.0, origin))
                .and_then(|frame| crate::desktop_frame(&frame));
            // Observation may complete a job or yield fishing before producing input.
            generation = runtime().generation;
        }
        held.sync(hwnd, generation, runtime().phase().active());
        if let Some(input) = input {
            if !runtime().input_current(generation) {
                continue;
            }
            let completed = perform(hwnd, origin, w, h, input, generation, &mut held);
            crate::desktop_input_ack(generation, completed == Some(true));
            match completed {
                Some(true) => {}
                Some(false) => {
                    let mut state = runtime();
                    if state.input_current(generation) {
                        state.stop("Windows에서 게임 입력을 보내지 못했습니다.");
                    }
                    held.release();
                }
                None => {
                    let mut state = runtime();
                    if state.input_current(generation) {
                        state.no_target();
                    }
                    held.release();
                }
            }
        }
        let headless_job = !runtime().needs_frame();
        let period = if headless_job {
            4
        } else if active == Phase::Reeling {
            8
        } else {
            50
        };
        thread::sleep(Duration::from_millis(period).saturating_sub(tick.elapsed()));
    }
}

#[derive(Default)]
struct HeldKeys {
    keys: Vec<u16>,
    window: HWND,
    generation: i32,
}
impl HeldKeys {
    fn sync(&mut self, window: HWND, generation: i32, active: bool) {
        if !active || self.window != window || self.generation != generation {
            self.release();
        }
        self.window = window;
        self.generation = generation;
    }
    fn event(&mut self, key: u16, down: bool) -> bool {
        if !down && !self.keys.contains(&key) {
            return true;
        }
        if !key_input(key, !down) {
            return false;
        }
        if down {
            if !self.keys.contains(&key) {
                self.keys.push(key);
            }
        } else {
            self.keys.retain(|&held| held != key);
        }
        true
    }
    fn release(&mut self) {
        for key in self.keys.drain(..).rev() {
            key_input(key, true);
        }
    }
}
impl Drop for HeldKeys {
    fn drop(&mut self) {
        self.release();
    }
}

fn game_process(pid: u32) -> bool {
    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if process.is_null() {
        return false;
    }
    let mut name = [0u16; 1024];
    let mut len = name.len() as u32;
    let ok = unsafe {
        let ok = QueryFullProcessImageNameW(process, 0, name.as_mut_ptr(), &mut len);
        CloseHandle(process);
        ok != 0
    };
    ok && String::from_utf16_lossy(&name[..len as usize])
        .rsplit('\\')
        .next()
        .is_some_and(game_executable)
}

fn game_executable(name: &str) -> bool {
    ["RobloxPlayerBeta.exe", "RobloxPlayer.exe"]
        .iter()
        .any(|candidate| name.eq_ignore_ascii_case(candidate))
}

// main's window choice: prefer a visible game client over small auxiliary windows.
fn game_window() -> Option<HWND> {
    let title: Vec<u16> = "Roblox\0".encode_utf16().collect();
    let mut window = null_mut();
    let mut best = None;
    for _ in 0..64 {
        window = unsafe { FindWindowExW(null_mut(), window, std::ptr::null(), title.as_ptr()) };
        if window.is_null() {
            break;
        }
        let mut pid = 0;
        unsafe { GetWindowThreadProcessId(window, &mut pid) };
        if pid == 0 || !game_process(pid) {
            continue;
        }
        let Some((_, width, height)) = client(window) else {
            continue;
        };
        let mut class = [0u16; 64];
        let len = unsafe { GetClassNameW(window, class.as_mut_ptr(), class.len() as i32) };
        let is_client =
            len > 0 && String::from_utf16_lossy(&class[..len as usize]) == "WINDOWSCLIENT";
        let score = (
            unsafe { IsWindowVisible(window) } != 0,
            is_client,
            i64::from(width) * i64::from(height),
        );
        if best.as_ref().is_none_or(|(old, _)| score > *old) {
            best = Some((score, window));
        }
    }
    best.map(|(_, window)| window)
}

pub fn game_available() -> bool {
    game_window().is_some()
}

pub fn geometry() -> Option<(usize, usize)> {
    let (_, width, height) = client(game_window()?)?;
    Some((width as usize, height as usize))
}

pub fn focus_game() -> Result<(), String> {
    let window = game_window().ok_or("실행 중인 Roblox 창을 찾지 못했습니다.")?;
    if unsafe { IsIconic(window) } != 0 {
        unsafe { ShowWindow(window, SW_RESTORE) };
    }
    if unsafe { GetForegroundWindow() } != window {
        unsafe { SetForegroundWindow(window) };
    }
    for _ in 0..15 {
        if unsafe { GetForegroundWindow() } == window {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(20));
    }
    Err("Windows가 창 전환을 허용하지 않았습니다. Roblox 창을 앞에 두고 다시 시도하세요.".into())
}

/// Capture only the real foreground client's pixels, never another app covering it.
pub fn snapshot() -> Result<Vec<u8>, String> {
    let window = unsafe { GetForegroundWindow() };
    let mut pid = 0;
    unsafe { GetWindowThreadProcessId(window, &mut pid) };
    if window.is_null() || pid == 0 || !game_process(pid) {
        return Err("Roblox 창을 앞에 두고 화면을 확인하세요.".into());
    }
    let (origin, width, height) = client(window).ok_or("Roblox 화면 크기를 읽지 못했습니다.")?;
    let count = (width as usize)
        .checked_mul(height as usize)
        .and_then(|n| n.checked_mul(4))
        .filter(|&n| n <= 256 * 1024 * 1024)
        .ok_or("Roblox 화면이 너무 큽니다.")?;
    let source = ScreenDc::new();
    let mut surface =
        Surface::new(source.0, width, height).ok_or("Roblox 화면 캡처를 준비하지 못했습니다.")?;
    surface
        .capture(source.0, origin)
        .ok_or("Roblox 화면을 캡처하지 못했습니다.")?;
    let unchanged = client(window).is_some_and(|(after, w, h)| {
        after.x == origin.x && after.y == origin.y && (w, h) == (width, height)
    });
    if unsafe { GetForegroundWindow() } != window || !unchanged {
        return Err("캡처 중 게임 창이 바뀌었습니다. 다시 시도하세요.".into());
    }
    let pixels = unsafe { std::slice::from_raw_parts(surface.pixels.cast::<u8>(), count) };
    let mut image = Vec::with_capacity(8 + count);
    image.extend_from_slice(&(width as u32).to_le_bytes());
    image.extend_from_slice(&(height as u32).to_le_bytes());
    for pixel in pixels.chunks_exact(4) {
        image.extend_from_slice(&[pixel[2], pixel[1], pixel[0], 255]);
    }
    Ok(image)
}

fn client(hwnd: HWND) -> Option<(POINT, i32, i32)> {
    let mut rect = RECT::default();
    let mut origin = POINT::default();
    let ok =
        unsafe { GetClientRect(hwnd, &mut rect) != 0 && ClientToScreen(hwnd, &mut origin) != 0 };
    let (width, height) = (rect.right - rect.left, rect.bottom - rect.top);
    (ok && (1..=16384).contains(&width) && (1..=16384).contains(&height))
        .then_some((origin, width, height))
}

fn perform(
    hwnd: HWND,
    origin: POINT,
    width: i32,
    height: i32,
    input: GameInput,
    generation: i32,
    held: &mut HeldKeys,
) -> Option<bool> {
    if unsafe { GetForegroundWindow() } != hwnd || !runtime().input_current(generation) {
        return None;
    }
    let at = |point: Point, ms| Tap {
        x: ((point.0 * f64::from(width)) as usize).min(width as usize - 1),
        y: ((point.1 * f64::from(height)) as usize).min(height as usize - 1),
        duration: Duration::from_millis(u64::from(ms)),
    };
    match input {
        GameInput::Text { text, submit } => {
            held.release();
            Some(replace_text(hwnd, &text, submit, generation))
        }
        GameInput::Tap(tap) => click(hwnd, origin, tap, false, generation),
        GameInput::Motion(Motion::Tap(point, ms)) => {
            click(hwnd, origin, at(point, ms), false, generation)
        }
        GameInput::Motion(Motion::Click { point, right }) => {
            click(hwnd, origin, at(point, 40), right, generation)
        }
        GameInput::Motion(Motion::Keys(keys, ms)) => Some(hold_keys(hwnd, keys, ms, generation)),
        GameInput::Motion(Motion::KeysOverlap {
            main,
            extra,
            ms,
            extra_at,
        }) => Some(hold_overlap(hwnd, main, extra, ms, extra_at, generation)),
        GameInput::Motion(Motion::KeyEvent { vk, down }) => {
            Some(keep_input(hwnd, generation) && held.event(vk, down))
        }
        GameInput::Motion(Motion::Tilt { pixels, ms }) => {
            Some(tilt(hwnd, origin, width, height, pixels, ms, generation))
        }
        GameInput::Motion(_) => Some(false),
    }
}

fn click(hwnd: HWND, origin: POINT, tap: Tap, right: bool, generation: i32) -> Option<bool> {
    let target = POINT {
        x: origin.x + tap.x as i32,
        y: origin.y + tap.y as i32,
    };
    let mut cursor = POINT::default();
    if unsafe {
        GetForegroundWindow() != hwnd
            || GetAncestor(WindowFromPoint(target), GA_ROOT) != hwnd
            || GetCursorPos(&mut cursor) == 0
    } {
        return None;
    }
    // main's reeling loop positions once, then sends button-only clicks.
    let reeling_here = !right
        && runtime().phase() == Phase::Reeling
        && cursor.x == target.x
        && cursor.y == target.y;
    if !reeling_here {
        if !move_to(target) {
            return Some(false);
        }
        thread::sleep(Duration::from_millis(30));
    }
    let (down, up) = if right {
        (MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP)
    } else {
        (MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP)
    };
    {
        let state = runtime();
        if !state.input_current(generation) || unsafe { GetForegroundWindow() } != hwnd {
            return None;
        }
        if !mouse(down, 0) {
            return Some(false);
        }
    }
    thread::sleep(tap.duration);
    Some(mouse(up, 0))
}

fn absolute_axis(point: i32, origin: i32, size: i32) -> Option<i32> {
    let offset = i64::from(point) - i64::from(origin);
    if size <= 0 || offset < 0 || offset >= i64::from(size) {
        return None;
    }
    Some((offset * 65535 / i64::from((size - 1).max(1))) as i32)
}

fn move_to(target: POINT) -> bool {
    let (left, top, width, height) = unsafe {
        (
            GetSystemMetrics(SM_XVIRTUALSCREEN),
            GetSystemMetrics(SM_YVIRTUALSCREEN),
            GetSystemMetrics(SM_CXVIRTUALSCREEN),
            GetSystemMetrics(SM_CYVIRTUALSCREEN),
        )
    };
    let (Some(x), Some(y)) = (
        absolute_axis(target.x, left, width),
        absolute_axis(target.y, top, height),
    ) else {
        return false;
    };
    if !mouse_xy(
        MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_VIRTUALDESK,
        x,
        y,
    ) {
        return false;
    }
    thread::sleep(Duration::from_millis(10));
    mouse_xy(MOUSEEVENTF_MOVE, 1, 0) && mouse_xy(MOUSEEVENTF_MOVE, -1, 0)
}

fn mouse(flags: u32, dy: i32) -> bool {
    mouse_xy(flags, 0, dy)
}

fn mouse_xy(flags: u32, dx: i32, dy: i32) -> bool {
    let input = INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT {
                dwFlags: flags,
                dx,
                dy,
                ..Default::default()
            },
        },
    };
    unsafe { SendInput(1, &input, size_of::<INPUT>() as i32) == 1 }
}

fn pressed(key: u16) -> bool {
    unsafe { GetAsyncKeyState(i32::from(key)) < 0 }
}

fn key_input(key: u16, release: bool) -> bool {
    let scan = unsafe { MapVirtualKeyW(u32::from(key), MAPVK_VK_TO_VSC) } as u16;
    if scan == 0 {
        return false;
    }
    let extended = matches!(key, 0x21..=0x28 | 0x2d | 0x2e | 0x5b | 0x5c);
    let input = INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wScan: scan,
                dwFlags: KEYEVENTF_SCANCODE
                    | if release { KEYEVENTF_KEYUP } else { 0 }
                    | if extended { KEYEVENTF_EXTENDEDKEY } else { 0 },
                ..Default::default()
            },
        },
    };
    unsafe { SendInput(1, &input, size_of::<INPUT>() as i32) == 1 }
}

fn keep_input(hwnd: HWND, generation: i32) -> bool {
    if !runtime().input_current(generation) {
        return false;
    }
    if pressed(VK_F3) || pressed(VK_F7) {
        runtime().stop("단축키로 중지됨");
        return false;
    }
    if unsafe { GetForegroundWindow() } != hwnd {
        runtime().no_target();
        return false;
    }
    runtime().input_current(generation)
}

fn unicode_input(unit: u16, release: bool) -> bool {
    let input = INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wScan: unit,
                dwFlags: KEYEVENTF_UNICODE | if release { KEYEVENTF_KEYUP } else { 0 },
                ..Default::default()
            },
        },
    };
    unsafe { SendInput(1, &input, size_of::<INPUT>() as i32) == 1 }
}

/// Replace the already-focused search/amount field; never append to its old text.
fn replace_text(hwnd: HWND, text: &str, submit: bool, generation: i32) -> bool {
    if text.encode_utf16().count() > 8192 || text.contains('\0') || !keep_input(hwnd, generation) {
        return false;
    }
    if !hold_keys(hwnd, &[0x11, 0x41], 30, generation)
        || !keep_input(hwnd, generation)
        || !hold_keys(hwnd, &[0x08], 30, generation)
    {
        return false;
    }
    let started = Instant::now();
    for character in text.chars() {
        let mut units = [0; 2];
        for &unit in character.encode_utf16(&mut units).iter() {
            if started.elapsed() >= Duration::from_secs(12) || !keep_input(hwnd, generation) {
                return false;
            }
            let down = unicode_input(unit, false);
            // Ups are always requested, including after focus/stop changed.
            let up = unicode_input(unit, true);
            if !down || !up {
                return false;
            }
        }
        // Main's Unicode typing path lets the game's text field consume each character.
        for _ in 0..2 {
            if !keep_input(hwnd, generation) {
                return false;
            }
            thread::sleep(Duration::from_millis(10));
        }
    }
    keep_input(hwnd, generation) && (!submit || hold_keys(hwnd, &[0x0d], 40, generation))
}

fn hold_keys(hwnd: HWND, keys: &[u16], ms: u32, generation: i32) -> bool {
    let mut down = 0;
    for &key in keys {
        if !keep_input(hwnd, generation) || !key_input(key, false) {
            break;
        }
        down += 1;
    }
    let mut ok = down == keys.len();
    if ok {
        let end = Instant::now() + Duration::from_millis(ms.into());
        while Instant::now() < end && keep_input(hwnd, generation) {
            thread::sleep(
                Duration::from_millis(33).min(end.saturating_duration_since(Instant::now())),
            );
            if Instant::now() >= end || !keep_input(hwnd, generation) {
                break;
            }
            for &key in keys {
                if !key_input(key, false) {
                    ok = false;
                    break;
                }
            }
            if !ok {
                break;
            }
        }
    }
    for &key in keys[..down].iter().rev() {
        ok &= key_input(key, true);
    }
    ok || !runtime().input_current(generation)
}

fn hold_overlap(
    hwnd: HWND,
    main: u16,
    extra: u16,
    ms: u32,
    extra_at: u32,
    generation: i32,
) -> bool {
    if !keep_input(hwnd, generation) || !key_input(main, false) {
        return false;
    }
    let start = Instant::now();
    let end = start + Duration::from_millis(ms.into());
    let extra_at = start + Duration::from_millis(extra_at.into());
    let mut next_repeat = start + Duration::from_millis(33);
    let mut extra_down = false;
    let mut ok = true;
    while Instant::now() < end && keep_input(hwnd, generation) {
        let now = Instant::now();
        if !extra_down && now >= extra_at {
            ok &= key_input(extra, false);
            extra_down = ok;
        }
        if now >= next_repeat {
            ok &= key_input(main, false);
            if extra_down {
                ok &= key_input(extra, false);
            }
            next_repeat = now + Duration::from_millis(33);
        }
        if !ok {
            break;
        }
        thread::sleep(Duration::from_millis(2).min(end.saturating_duration_since(Instant::now())));
    }
    if extra_down {
        ok &= key_input(extra, true);
    }
    ok &= key_input(main, true);
    ok || !runtime().input_current(generation)
}

fn tilt(
    hwnd: HWND,
    origin: POINT,
    width: i32,
    height: i32,
    pixels: i32,
    ms: u32,
    generation: i32,
) -> bool {
    let target = POINT {
        x: origin.x + width / 2,
        y: origin.y + height / 2,
    };
    if unsafe { GetAncestor(WindowFromPoint(target), GA_ROOT) } != hwnd
        || !keep_input(hwnd, generation)
    {
        return false;
    }
    if !move_to(target) {
        return false;
    }
    thread::sleep(Duration::from_millis(100));
    if !keep_input(hwnd, generation) {
        return false;
    }
    let zoom = ms > 0 && key_input(0x4f, false);
    let mut dragging = pixels > 0 && mouse(MOUSEEVENTF_RIGHTDOWN, 0);
    let mut ok = (ms == 0 || zoom) && (pixels <= 0 || dragging);
    let start = Instant::now();
    let duration = Duration::from_millis(ms.into());
    let mut next_key = start + Duration::from_millis(33);
    let mut moved = 0;
    while ok && (start.elapsed() < duration || dragging) && keep_input(hwnd, generation) {
        if dragging {
            if moved < pixels {
                let step = (pixels - moved).min(20);
                ok &= mouse(MOUSEEVENTF_MOVE, step);
                moved += step;
            } else {
                ok &= mouse(MOUSEEVENTF_RIGHTUP, 0);
                dragging = false;
            }
        }
        if zoom && Instant::now() >= next_key && start.elapsed() < duration {
            ok &= key_input(0x4f, false);
            next_key = Instant::now() + Duration::from_millis(33);
        }
        thread::sleep(Duration::from_millis(10));
    }
    if dragging {
        ok &= mouse(MOUSEEVENTF_RIGHTUP, 0);
    }
    if zoom {
        ok &= key_input(0x4f, true);
    }
    ok || !runtime().input_current(generation)
}

static PICKING: AtomicU8 = AtomicU8::new(0);

pub fn cancel_pick() {
    let _ = PICKING.compare_exchange(1, 2, Ordering::AcqRel, Ordering::Acquire);
}

pub fn pick() -> Result<Point, String> {
    let position = pick_area(false)?;
    Ok(Point(position[0], position[1]))
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
    let start = Instant::now();
    let mut window = null_mut();
    let mut game = false;
    let mut ready = false;
    let mut first: Option<Point> = None;
    while start.elapsed() < Duration::from_secs(60) {
        if PICKING.load(Ordering::Acquire) != 1 || pressed(VK_ESCAPE) {
            return Err("위치 지정을 취소했습니다.".into());
        }
        let foreground = unsafe { GetForegroundWindow() };
        if foreground != window {
            window = foreground;
            let mut pid = 0;
            if !window.is_null() {
                unsafe { GetWindowThreadProcessId(window, &mut pid) };
            }
            game = pid != 0 && game_process(pid);
            ready = false;
            first = None;
        }
        if game {
            let button = pressed(VK_LBUTTON);
            if ready && (button || first.is_some()) {
                let mut cursor = POINT::default();
                if let Some((origin, w, h)) = client(window) {
                    if unsafe {
                        GetCursorPos(&mut cursor) != 0
                            && GetAncestor(WindowFromPoint(cursor), GA_ROOT) == window
                    } {
                        let (x, y) = (cursor.x - origin.x, cursor.y - origin.y);
                        if (0..w).contains(&x) && (0..h).contains(&y) {
                            let point =
                                Point(f64::from(x) / f64::from(w), f64::from(y) / f64::from(h));
                            if !region {
                                return Ok(vec![point.0, point.1]);
                            }
                            if let Some(first) = first {
                                if !button {
                                    let values = vec![
                                        first.0.min(point.0),
                                        first.1.min(point.1),
                                        first.0.max(point.0),
                                        first.1.max(point.1),
                                    ];
                                    if values[0] == values[2] || values[1] == values[3] {
                                        return Err("서로 다른 두 모서리를 드래그하세요.".into());
                                    }
                                    return Ok(values);
                                }
                            } else if button {
                                first = Some(point);
                            }
                        }
                    }
                }
            }
            ready = first.is_some() || !button;
        }
        thread::sleep(Duration::from_millis(15));
    }
    Err("Roblox 화면에서 위치를 선택하지 않았습니다.".into())
}

struct ScreenDc(HDC);

impl ScreenDc {
    fn new() -> Self {
        Self(unsafe { GetDC(null_mut()) })
    }
}

impl Drop for ScreenDc {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { ReleaseDC(null_mut(), self.0) };
        }
    }
}

struct Surface {
    dc: HDC,
    bitmap: HBITMAP,
    original: HGDIOBJ,
    pixels: *mut c_void,
    width: i32,
    height: i32,
}

impl Surface {
    fn new(source: HDC, width: i32, height: i32) -> Option<Self> {
        if source.is_null() {
            return None;
        }
        let mut surface = Self {
            dc: unsafe { CreateCompatibleDC(source) },
            bitmap: null_mut(),
            original: null_mut(),
            pixels: null_mut(),
            width,
            height,
        };
        if surface.dc.is_null() {
            return None;
        }
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width,
                biHeight: -height,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB,
                ..Default::default()
            },
            ..Default::default()
        };
        unsafe {
            surface.bitmap = CreateDIBSection(
                source,
                &info,
                DIB_RGB_COLORS,
                &mut surface.pixels,
                null_mut(),
                0,
            );
            if surface.bitmap.is_null() || surface.pixels.is_null() {
                return None;
            }
            surface.original = SelectObject(surface.dc, surface.bitmap);
        }
        (!surface.original.is_null()).then_some(surface)
    }

    fn capture(&mut self, source: HDC, origin: POINT) -> Option<Frame<'_>> {
        unsafe {
            if source.is_null()
                || BitBlt(
                    self.dc,
                    0,
                    0,
                    self.width,
                    self.height,
                    source,
                    origin.x,
                    origin.y,
                    SRCCOPY,
                ) == 0
            {
                return None;
            }
            GdiFlush();
            let (w, h) = (self.width as usize, self.height as usize);
            Frame::new(
                std::slice::from_raw_parts(self.pixels.cast(), w * h * 4),
                w,
                h,
                w * 4,
                4,
                true,
            )
        }
    }
}

impl Drop for Surface {
    fn drop(&mut self) {
        unsafe {
            if !self.original.is_null() {
                SelectObject(self.dc, self.original);
            }
            if !self.bitmap.is_null() {
                DeleteObject(self.bitmap);
            }
            if !self.dc.is_null() {
                DeleteDC(self.dc);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn player_process_identity_does_not_accept_studio_or_similar_names() {
        for name in ["RobloxPlayerBeta.exe", "ROBLOXPLAYER.EXE"] {
            assert!(game_executable(name));
        }
        for name in [
            "RobloxStudioBeta.exe",
            "RobloxPlayerBeta.exe.old",
            "NotRobloxPlayer.exe",
            "",
        ] {
            assert!(!game_executable(name));
        }
    }

    #[test]
    fn virtual_desktop_coordinates_preserve_secondary_monitor_origins() {
        assert_eq!(absolute_axis(-1920, -1920, 3840), Some(0));
        assert_eq!(absolute_axis(1919, -1920, 3840), Some(65535));
        assert_eq!(absolute_axis(-1, -1920, 3840), Some(32758));
        assert_eq!(absolute_axis(-1921, -1920, 3840), None);
        assert_eq!(absolute_axis(1920, -1920, 3840), None);
        assert_eq!(absolute_axis(5, 5, 1), Some(0));
        assert_eq!(absolute_axis(0, 0, 0), None);
    }
}

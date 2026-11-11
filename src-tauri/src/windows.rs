use crate::{
    fishing::{Phase, Tap},
    now, runtime,
    vision::{Frame, Point},
    DeviceTick, GameInput, Motion,
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
            GetAsyncKeyState, SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, INPUT_MOUSE, KEYBDINPUT,
            KEYEVENTF_KEYUP, MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP, MOUSEEVENTF_MOVE,
            MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP, MOUSEINPUT, VK_ESCAPE, VK_F3, VK_F6, VK_F7,
            VK_LBUTTON,
        },
        WindowsAndMessaging::{
            FindWindowExW, GetAncestor, GetClientRect, GetCursorPos, GetForegroundWindow,
            GetWindowThreadProcessId, SetCursorPos, WindowFromPoint, GA_ROOT,
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
    let mut surface = None;
    loop {
        let tick = Instant::now();
        let (phase, active) = {
            let state = runtime();
            (state.phase(), state.active())
        };
        if !active {
            surface = None;
            hotkey_held = false;
            thread::sleep(Duration::from_millis(100));
            continue;
        }
        let hotkey = pressed(VK_F3) || pressed(VK_F7);
        if hotkey && !hotkey_held {
            runtime().stop("F3으로 중지됨");
            continue;
        }
        hotkey_held = hotkey;
        if pressed(VK_F6) && runtime().arrive_desktop() {
            continue;
        }

        let hwnd = unsafe { GetForegroundWindow() };
        let mut pid = 0;
        if !hwnd.is_null() {
            unsafe { GetWindowThreadProcessId(hwnd, &mut pid) };
        }
        if hwnd != foreground || pid != foreground_pid {
            if is_game {
                runtime().no_target();
                surface = None;
            }
            foreground = hwnd;
            foreground_pid = pid;
            is_game = pid != 0 && game_process(pid);
        }

        let geometry = is_game.then(|| client(hwnd)).flatten();
        let Some((origin, w, h)) = geometry else {
            runtime().no_target();
            surface = None;
            thread::sleep(Duration::from_millis(100));
            continue;
        };
        let geometry = (w as usize, h as usize);
        let (mut step, mut generation) = {
            let mut state = runtime();
            (state.tick(now(), geometry, None), state.generation)
        };
        if matches!(step, DeviceTick::Observe) {
            let source = ScreenDc::new();
            if surface
                .as_ref()
                .is_none_or(|s: &Surface| (s.width, s.height) != (w, h))
            {
                surface = Surface::new(source.0, w, h);
            }
            let frame = surface.as_mut().and_then(|s| s.capture(source.0, origin));
            let mut state = runtime();
            if !state.input_current(generation) {
                continue;
            }
            if !same_client(hwnd, origin, w, h) {
                state.no_target();
                continue;
            }
            step = match frame {
                Some(frame) => state.tick(now(), geometry, Some(&frame)),
                None => {
                    state.stop("Windows에서 게임 화면을 캡처하지 못했습니다.");
                    DeviceTick::Wait
                }
            };
            generation = state.generation;
        } else {
            // Headless movement and waits never acquire or read a screen surface.
            surface = None;
        }

        if let DeviceTick::Input(input) = step {
            if !runtime().input_current(generation) {
                continue;
            }
            match perform(hwnd, origin, w, h, input, generation) {
                Some(true) => {}
                Some(false) => {
                    let mut state = runtime();
                    if state.input_current(generation) {
                        state.input_rejected("Windows에서 게임 입력을 보내지 못했습니다.");
                    }
                }
                None => {
                    let mut state = runtime();
                    if state.input_current(generation) {
                        state.no_target();
                    }
                }
            }
        }
        let period = if phase == Phase::Reeling { 8 } else { 50 };
        thread::sleep(Duration::from_millis(period).saturating_sub(tick.elapsed()));
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
        .is_some_and(|exe| exe.eq_ignore_ascii_case("RobloxPlayerBeta.exe"))
}

pub fn game_available() -> bool {
    let title: Vec<u16> = "Roblox\0".encode_utf16().collect();
    let mut window = null_mut();
    loop {
        window = unsafe { FindWindowExW(null_mut(), window, std::ptr::null(), title.as_ptr()) };
        if window.is_null() {
            return false;
        }
        let mut pid = 0;
        unsafe { GetWindowThreadProcessId(window, &mut pid) };
        if pid != 0 && game_process(pid) {
            return true;
        }
    }
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

fn same_client(hwnd: HWND, origin: POINT, width: i32, height: i32) -> bool {
    (unsafe { GetForegroundWindow() == hwnd })
        && client(hwnd).is_some_and(|(current, w, h)| {
            current.x == origin.x && current.y == origin.y && w == width && h == height
        })
}

fn perform(
    hwnd: HWND,
    origin: POINT,
    width: i32,
    height: i32,
    input: GameInput,
    generation: i32,
) -> Option<bool> {
    if !same_client(hwnd, origin, width, height) || !runtime().input_current(generation) {
        return None;
    }
    let at = |point: Point, ms| Tap {
        x: ((point.0 * f64::from(width)) as usize).min(width as usize - 1),
        y: ((point.1 * f64::from(height)) as usize).min(height as usize - 1),
        duration: Duration::from_millis(u64::from(ms)),
    };
    match input {
        GameInput::Tap(tap) => click(hwnd, origin, tap, false, generation),
        GameInput::Motion(Motion::Tap(point, ms)) => {
            click(hwnd, origin, at(point, ms), false, generation)
        }
        GameInput::Motion(Motion::Click { point, right }) => {
            click(hwnd, origin, at(point, 40), right, generation)
        }
        GameInput::Motion(Motion::Keys(keys, ms)) => Some(hold_keys(hwnd, keys, ms, generation)),
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
    let mut previous = POINT::default();
    let valid = unsafe {
        GetForegroundWindow() == hwnd
            && GetAncestor(WindowFromPoint(target), GA_ROOT) == hwnd
            && GetCursorPos(&mut previous) != 0
    };
    if !valid {
        return None;
    }
    let (down, up) = if right {
        (MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP)
    } else {
        (MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP)
    };
    {
        let state = runtime();
        if !state.input_current(generation) {
            return None;
        }
        if unsafe { SetCursorPos(target.x, target.y) } == 0 {
            return Some(false);
        }
        if !mouse(down, 0) {
            unsafe { SetCursorPos(previous.x, previous.y) };
            return Some(false);
        }
    }
    thread::sleep(tap.duration);
    let released = mouse(up, 0);
    let mut cursor = POINT::default();
    unsafe {
        if GetCursorPos(&mut cursor) != 0 && cursor.x == target.x && cursor.y == target.y {
            SetCursorPos(previous.x, previous.y);
        }
    }
    Some(released)
}

fn mouse(flags: u32, dy: i32) -> bool {
    let input = INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT {
                dwFlags: flags,
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
    let input = INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: key,
                dwFlags: if release { KEYEVENTF_KEYUP } else { 0 },
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

fn hold_keys(hwnd: HWND, keys: &[u16], ms: u32, generation: i32) -> bool {
    let mut down = 0;
    for &key in keys {
        if !keep_input(hwnd, generation) || !key_input(key, false) {
            break;
        }
        down += 1;
    }
    if down == keys.len() {
        let end = Instant::now() + Duration::from_millis(ms.into());
        while Instant::now() < end && keep_input(hwnd, generation) {
            thread::sleep(
                Duration::from_millis(15).min(end.saturating_duration_since(Instant::now())),
            );
        }
    }
    let mut ok = down == keys.len();
    for &key in keys[..down].iter().rev() {
        ok &= key_input(key, true);
    }
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
    let mut previous = POINT::default();
    if unsafe { GetCursorPos(&mut previous) == 0 || SetCursorPos(target.x, target.y) == 0 } {
        return false;
    }
    let zoom = ms == 0 || key_input(0x4f, false);
    let dragging = zoom && mouse(MOUSEEVENTF_RIGHTDOWN, 0);
    let mut ok = dragging;
    if dragging {
        let start = Instant::now();
        let mut moved = 0;
        let duration = Duration::from_millis(u64::from(ms.max(350)));
        while start.elapsed() < duration && keep_input(hwnd, generation) {
            let next = ((start.elapsed().as_secs_f64() / 0.35).min(1.0) * f64::from(pixels)).round()
                as i32;
            if moved != next {
                ok &= mouse(MOUSEEVENTF_MOVE, next - moved);
                moved = next;
            }
            thread::sleep(Duration::from_millis(12));
        }
        if keep_input(hwnd, generation) && moved != pixels {
            ok &= mouse(MOUSEEVENTF_MOVE, pixels - moved);
        }
        ok &= mouse(MOUSEEVENTF_RIGHTUP, 0);
    }
    if zoom && ms > 0 {
        ok &= key_input(0x4f, true);
    }
    if unsafe { GetForegroundWindow() } == hwnd {
        unsafe { SetCursorPos(previous.x, previous.y) };
    }
    ok || !runtime().input_current(generation)
}

static PICKING: AtomicU8 = AtomicU8::new(0);

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
    let start = Instant::now();
    let mut window = null_mut();
    let mut game = false;
    let mut ready = false;
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
        }
        if game {
            let button = pressed(VK_LBUTTON);
            if ready && button {
                let mut cursor = POINT::default();
                if let Some((origin, w, h)) = client(window) {
                    if unsafe {
                        GetCursorPos(&mut cursor) != 0
                            && GetAncestor(WindowFromPoint(cursor), GA_ROOT) == window
                    } {
                        let (x, y) = (cursor.x - origin.x, cursor.y - origin.y);
                        if (0..w).contains(&x) && (0..h).contains(&y) {
                            return Ok(Point(
                                f64::from(x) / f64::from(w),
                                f64::from(y) / f64::from(h),
                            ));
                        }
                    }
                }
            }
            ready = !button;
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

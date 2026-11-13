//! Win32 calls are restricted to the selected Roblox HWND. Never use SendInput.
use super::{image_bytes, Image, Target, Word};
use crate::vision::Rect;
use std::{
    cell::{Cell, RefCell},
    ffi::c_void,
    mem::{size_of, zeroed},
    ptr, thread,
    time::{Duration, Instant},
};
#[path = "windows_capture.rs"]
mod capture;
#[path = "windows_session.rs"]
mod session;
use windows::{
    Graphics::Imaging::{BitmapPixelFormat, SoftwareBitmap},
    Media::Ocr::OcrEngine,
    Storage::Streams::DataWriter,
};

type Handle = isize;
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct WinRect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}
#[repr(C)]
#[derive(Default)]
struct WinPoint {
    x: i32,
    y: i32,
}
#[repr(C)]
#[derive(Default)]
struct FileTime {
    low: u32,
    high: u32,
}
#[repr(C)]
struct BitmapHeader {
    size: u32,
    width: i32,
    height: i32,
    planes: u16,
    bit_count: u16,
    compression: u32,
    size_image: u32,
    xppm: i32,
    yppm: i32,
    used: u32,
    important: u32,
}
#[repr(C)]
struct BitmapInfo {
    header: BitmapHeader,
    colors: [u32; 1],
}
#[link(name = "user32")]
unsafe extern "system" {
    fn IsWindow(hwnd: Handle) -> i32;
    fn IsWindowVisible(hwnd: Handle) -> i32;
    fn IsIconic(hwnd: Handle) -> i32;
    fn IsHungAppWindow(hwnd: Handle) -> i32;
    fn GetWindowThreadProcessId(hwnd: Handle, pid: *mut u32) -> u32;
    fn GetClientRect(hwnd: Handle, rect: *mut WinRect) -> i32;
    fn GetWindowRect(hwnd: Handle, rect: *mut WinRect) -> i32;
    fn ClientToScreen(hwnd: Handle, point: *mut WinPoint) -> i32;
    fn GetDC(hwnd: Handle) -> Handle;
    fn ReleaseDC(hwnd: Handle, dc: Handle) -> i32;
    fn PrintWindow(hwnd: Handle, dc: Handle, flags: u32) -> i32;
    fn PostMessageW(hwnd: Handle, message: u32, wparam: usize, lparam: isize) -> i32;
    fn MapVirtualKeyW(code: u32, map_type: u32) -> u32;
}
#[link(name = "dwmapi")]
unsafe extern "system" {
    fn DwmGetWindowAttribute(hwnd: Handle, attribute: u32, value: *mut c_void, size: u32) -> i32;
}
#[link(name = "gdi32")]
unsafe extern "system" {
    fn CreateCompatibleDC(dc: Handle) -> Handle;
    fn DeleteDC(dc: Handle) -> i32;
    fn CreateDIBSection(
        dc: Handle,
        info: *const BitmapInfo,
        usage: u32,
        bits: *mut *mut c_void,
        section: Handle,
        offset: u32,
    ) -> Handle;
    fn SelectObject(dc: Handle, object: Handle) -> Handle;
    fn DeleteObject(object: Handle) -> i32;
    fn GdiFlush() -> i32;
}
#[link(name = "kernel32")]
unsafe extern "system" {
    fn OpenProcess(access: u32, inherit: i32, pid: u32) -> Handle;
    fn CloseHandle(handle: Handle) -> i32;
    fn QueryFullProcessImageNameW(
        process: Handle,
        flags: u32,
        name: *mut u16,
        size: *mut u32,
    ) -> i32;
    fn GetProcessTimes(
        process: Handle,
        creation: *mut FileTime,
        exit: *mut FileTime,
        kernel: *mut FileTime,
        user: *mut FileTime,
    ) -> i32;
}
#[link(name = "combase")]
unsafe extern "system" {
    fn RoInitialize(kind: u32) -> i32;
    fn RoUninitialize();
}

pub(super) struct Native {
    initialized: bool,
    gpu: RefCell<Option<capture::Capture>>,
    session: session::Session,
    captured_at: Cell<f64>,
}
impl Native {
    pub(super) fn new() -> Result<Self, String> {
        let session = session::Session::new()?;
        // A worker owns its MTA apartment, including WinRT OCR asynchronous work.
        let hr = unsafe { RoInitialize(1) };
        if hr < 0 {
            return Err(format!("Windows OCR 초기화 실패: 0x{:08x}", hr as u32));
        }
        Ok(Self {
            initialized: true,
            gpu: RefCell::new(None),
            session,
            captured_at: Cell::new(0.0),
        })
    }
    pub(super) fn find_now(&self) -> Result<Option<Target>, String> {
        let targets = self.session.windows();
        match targets.len() {
            0 => Ok(None),
            1 => Ok(Some(targets[0])),
            _ => Err("소유 게임 창이 여러 개라 입력 대상을 정하지 못했습니다.".into()),
        }
    }
    pub(super) fn launch_missing(&self, url: &str) -> Result<(), String> {
        let url = crate::automation::game_url(url)?;
        self.gpu.borrow_mut().take();
        self.session.open(&url)
    }
    pub(super) fn valid(&self, target: Target) -> bool {
        self.session.owns(target.pid) && inspect(target.id as Handle) == Some(target)
    }
    fn owned(&self, target: Target) -> bool {
        self.session.owns(target.pid)
            && identity(target.id as Handle)
                .is_some_and(|(pid, birth)| pid == target.pid && birth == target.birth)
    }
    pub(super) fn capture(&self, target: Target) -> Result<Image, String> {
        self.captured_at.set(crate::now());
        if !self.valid(target) {
            return Err("게임 창이 변경됐거나 최소화됐습니다.".into());
        }
        let mut gpu = self.gpu.borrow_mut();
        if gpu.as_ref().is_some_and(|capture| !capture.matches(target)) {
            gpu.take();
        }
        if gpu.is_none() {
            if let Ok(image) = self.print_window(target) {
                return Ok(image);
            }
            *gpu = Some(capture::Capture::new(target)?);
        }
        let (image, captured_at) = gpu.as_mut().unwrap().image(target)?;
        self.captured_at.set(captured_at);
        if !self.valid(target) {
            return Err("GPU 캡처 중 게임 창이 변경됐습니다.".into());
        }
        Ok(image)
    }
    pub(super) fn captured_at(&self) -> f64 {
        self.captured_at.get()
    }
    fn print_window(&self, target: Target) -> Result<Image, String> {
        if !self.valid(target) {
            return Err("게임 창이 변경됐거나 최소화됐습니다.".into());
        }
        let len = image_bytes(target.width, target.height)?;
        let hwnd = target.id as Handle;
        unsafe {
            let dc = GetDC(hwnd);
            if dc == 0 {
                return Err("게임 창의 그래픽 장치에 연결하지 못했습니다.".into());
            }
            let memory = CreateCompatibleDC(dc);
            if memory == 0 {
                ReleaseDC(hwnd, dc);
                return Err("화면 캡처 메모리를 만들지 못했습니다.".into());
            }
            let info = BitmapInfo {
                header: BitmapHeader {
                    size: size_of::<BitmapHeader>() as u32,
                    width: target.width as i32,
                    height: -(target.height as i32),
                    planes: 1,
                    bit_count: 32,
                    compression: 0,
                    size_image: len as u32,
                    xppm: 0,
                    yppm: 0,
                    used: 0,
                    important: 0,
                },
                colors: [0],
            };
            let mut bits = ptr::null_mut();
            let bitmap = CreateDIBSection(dc, &info, 0, &mut bits, 0, 0);
            if bitmap == 0 || bits.is_null() {
                if bitmap != 0 {
                    DeleteObject(bitmap);
                }
                DeleteDC(memory);
                ReleaseDC(hwnd, dc);
                return Err("화면 캡처 버퍼를 만들지 못했습니다.".into());
            }
            let old = SelectObject(memory, bitmap);
            // PW_CLIENTONLY | PW_RENDERFULLCONTENT captures only this client, even when occluded.
            // GPU applications may refuse it; a nonzero return alone is never sufficient.
            ptr::write_bytes(bits as *mut u8, 0, len);
            let ok = PrintWindow(hwnd, memory, 3) != 0;
            GdiFlush();
            let data = std::slice::from_raw_parts(bits as *const u8, len).to_vec();
            SelectObject(memory, old);
            DeleteObject(bitmap);
            DeleteDC(memory);
            ReleaseDC(hwnd, dc);
            if !ok || !self.valid(target) {
                return Err(
                    "Windows가 게임 창 캡처를 거부했습니다. GPU 캡처나 최소화 상태를 확인하세요."
                        .into(),
                );
            }
            let image = Image {
                data,
                width: target.width,
                height: target.height,
            };
            image.validate()?;
            Ok(image)
        }
    }
    fn message(
        &self,
        target: Target,
        message: u32,
        wparam: usize,
        lparam: isize,
    ) -> Result<(), String> {
        if !self.owned(target) {
            return Err("원래 게임 입력 대상이 사라졌습니다.".into());
        }
        if unsafe { PostMessageW(target.id as Handle, message, wparam, lparam) } == 0 {
            return Err(
                "Windows가 게임 창 입력을 거부했습니다. 앱과 게임의 실행 권한이 다를 수 있습니다."
                    .into(),
            );
        }
        Ok(())
    }
    pub(super) fn key(&self, target: Target, vk: u16, down: bool) -> Result<(), String> {
        if !key_supported(vk) {
            return Err("지원하지 않는 Windows 키입니다.".into());
        }
        let scan = unsafe { MapVirtualKeyW(vk.into(), 0) };
        let extended = matches!(vk, 0x21..=0x28 | 0x2d | 0x2e | 0x6f | 0xa3 | 0xa5);
        let param = 1u32
            | ((scan & 255) << 16)
            | ((extended as u32) << 24)
            | if down { 0 } else { 0xc0000000 };
        self.message(
            target,
            if down { 0x100 } else { 0x101 },
            vk as usize,
            param as i32 as isize,
        )
    }
    pub(super) fn move_pointer(
        &self,
        target: Target,
        at: (i32, i32),
        right_drag: bool,
    ) -> Result<(), String> {
        self.message(
            target,
            0x200,
            if right_drag { 2 } else { 0 },
            coordinates(at),
        )
    }
    pub(super) fn button(
        &self,
        target: Target,
        at: (i32, i32),
        right: bool,
        down: bool,
    ) -> Result<(), String> {
        self.message(
            target,
            match (right, down) {
                (false, true) => 0x201,
                (false, false) => 0x202,
                (true, true) => 0x204,
                (true, false) => 0x205,
            },
            if down {
                if right {
                    2
                } else {
                    1
                }
            } else {
                0
            },
            coordinates(at),
        )
    }
    pub(super) fn replace_text(
        &self,
        target: Target,
        text: &str,
        current: impl Fn() -> bool,
    ) -> Result<(), String> {
        if text.len() > 8192 || text.contains('\0') {
            return Err("입력할 텍스트가 너무 길거나 잘못됐습니다.".into());
        }
        let mut control = false;
        let mut a = false;
        let result = (|| {
            if !current() {
                return Err("텍스트 입력을 취소했습니다.".into());
            }
            self.key(target, 0x11, true)?;
            control = true;
            self.key(target, 0x41, true)?;
            a = true;
            self.key(target, 0x41, false)?;
            a = false;
            self.key(target, 0x11, false)?;
            control = false;
            self.key(target, 0x08, true)?;
            self.key(target, 0x08, false)?;
            for c in text.encode_utf16() {
                if !current() {
                    return Err("텍스트 입력을 취소했습니다.".into());
                }
                self.message(target, 0x102, c as usize, 1)?;
            }
            Ok(())
        })();
        if a {
            let _ = self.key(target, 0x41, false);
        }
        if control {
            let _ = self.key(target, 0x11, false);
        }
        result
    }
    pub(super) fn launch(&self, target: Target, url: &str) -> Result<(), String> {
        if !self.session.owns(target.pid) {
            return Err("앱이 소유한 Roblox 프로세스가 아닙니다.".into());
        }
        self.launch_missing(url)
    }
    pub(super) fn ocr(&self, image: &Image) -> Result<Vec<Word>, String> {
        fn recognize(image: &Image) -> windows::core::Result<Vec<Word>> {
            let engine = OcrEngine::TryCreateFromUserProfileLanguages()?;
            let max = OcrEngine::MaxImageDimension()? as usize;
            // Bound OCR size without changing the underlying capture or viewport coordinates.
            let scale = (max as f64 / image.width.max(image.height) as f64).min(1.0);
            let w = (image.width as f64 * scale).round().max(1.0) as usize;
            let h = (image.height as f64 * scale).round().max(1.0) as usize;
            let mut pixels = Vec::with_capacity(w * h * 4);
            for y in 0..h {
                for x in 0..w {
                    let offset = (((y as f64 / scale) as usize).min(image.height - 1)
                        * image.width
                        + ((x as f64 / scale) as usize).min(image.width - 1))
                        * 4;
                    pixels.extend_from_slice(&image.data[offset..offset + 3]);
                    pixels.push(255);
                }
            }
            let writer = DataWriter::new()?;
            writer.WriteBytes(&pixels)?;
            let bitmap = SoftwareBitmap::CreateCopyFromBuffer(
                &writer.DetachBuffer()?,
                BitmapPixelFormat::Bgra8,
                w as i32,
                h as i32,
            )?;
            let operation = engine.RecognizeAsync(&bitmap)?;
            let deadline = Instant::now() + Duration::from_secs(3);
            while operation.Status()?.0 == 0 {
                if super::STOP.load(std::sync::atomic::Ordering::Acquire)
                    || Instant::now() > deadline
                {
                    operation.Cancel()?;
                    return Err(windows::core::Error::from_hresult(windows::core::HRESULT(
                        0x80004004u32 as i32,
                    )));
                }
                thread::sleep(Duration::from_millis(8));
            }
            let recognized = operation.GetResults()?;
            let mut result = Vec::new();
            for line in recognized.Lines()? {
                for word in line.Words()? {
                    let r = word.BoundingRect()?;
                    result.push(Word {
                        text: word.Text()?.to_string(),
                        rect: Rect(
                            r.X as f64 / w as f64,
                            r.Y as f64 / h as f64,
                            (r.X + r.Width) as f64 / w as f64,
                            (r.Y + r.Height) as f64 / h as f64,
                        ),
                    });
                }
            }
            bitmap.Close()?;
            Ok(result)
        }
        recognize(image).map_err(|e| {
            format!("Windows 텍스트 인식 실패: {e}. Windows 언어/OCR 팩을 확인하세요.")
        })
    }
}
fn client_offset(target: Target, size: (usize, usize)) -> Option<(usize, usize)> {
    if size == target.geometry() {
        return Some((0, 0));
    }
    unsafe {
        let hwnd = target.id as Handle;
        let mut point = WinPoint::default();
        if ClientToScreen(hwnd, &mut point) == 0 {
            return None;
        }
        let mut bounds = WinRect::default();
        let mut candidates = Vec::new();
        if GetWindowRect(hwnd, &mut bounds) != 0 {
            candidates.push(bounds);
        }
        if DwmGetWindowAttribute(
            hwnd,
            9,
            (&mut bounds as *mut WinRect).cast(),
            size_of::<WinRect>() as u32,
        ) >= 0
        {
            candidates.push(bounds);
        }
        for b in candidates {
            if (b.right - b.left, b.bottom - b.top) != (size.0 as i32, size.1 as i32) {
                continue;
            }
            let (x, y) = (
                usize::try_from(point.x - b.left).ok()?,
                usize::try_from(point.y - b.top).ok()?,
            );
            if x + target.width <= size.0 && y + target.height <= size.1 {
                return Some((x, y));
            }
        }
        None
    }
}
impl Drop for Native {
    fn drop(&mut self) {
        self.gpu.get_mut().take();
        if self.initialized {
            unsafe {
                RoUninitialize();
            }
        }
    }
}
pub(super) fn key_supported(vk: u16) -> bool {
    matches!(vk,0x08|0x09|0x0d|0x10..=0x14|0x1b|0x20..=0x28|0x2d|0x2e|0x30..=0x39|0x41..=0x5a|0x60..=0x6f|0x70..=0x7b|0xa0..=0xa5|0xba..=0xc0|0xdb..=0xde)
}
fn coordinates(at: (i32, i32)) -> isize {
    ((at.0 as u16 as u32) | ((at.1 as u16 as u32) << 16)) as i32 as isize
}
unsafe extern "system" fn collect(hwnd: Handle, param: isize) -> i32 {
    if let Some(target) = inspect(hwnd) {
        unsafe {
            (&mut *(param as *mut Vec<Target>)).push(target);
        }
    }
    1
}
fn identity(hwnd: Handle) -> Option<(u32, u64)> {
    unsafe {
        if IsWindow(hwnd) == 0 {
            return None;
        }
        let mut pid = 0;
        if GetWindowThreadProcessId(hwnd, &mut pid) == 0 {
            return None;
        }
        let process = OpenProcess(0x1000, 0, pid);
        if process == 0 {
            return None;
        }
        let result = (|| {
            let mut path = [0u16; 32768];
            let mut len = path.len() as u32;
            if QueryFullProcessImageNameW(process, 0, path.as_mut_ptr(), &mut len) == 0 {
                return None;
            }
            let path = String::from_utf16_lossy(&path[..len as usize]);
            if !path
                .rsplit(['\\', '/'])
                .next()?
                .eq_ignore_ascii_case("RobloxPlayerBeta.exe")
            {
                return None;
            }
            let (mut creation, mut exit, mut kernel, mut user) = (
                FileTime::default(),
                FileTime::default(),
                FileTime::default(),
                FileTime::default(),
            );
            if GetProcessTimes(process, &mut creation, &mut exit, &mut kernel, &mut user) == 0 {
                return None;
            }
            Some((pid, (creation.high as u64) << 32 | creation.low as u64))
        })();
        CloseHandle(process);
        result
    }
}
fn inspect(hwnd: Handle) -> Option<Target> {
    unsafe {
        if IsWindowVisible(hwnd) == 0 || IsIconic(hwnd) != 0 || IsHungAppWindow(hwnd) != 0 {
            return None;
        }
        let (pid, birth) = identity(hwnd)?;
        let mut rect: WinRect = zeroed();
        if GetClientRect(hwnd, &mut rect) == 0 {
            return None;
        }
        let (width, height) = (
            (rect.right - rect.left) as usize,
            (rect.bottom - rect.top) as usize,
        );
        image_bytes(width, height).ok()?;
        Some(Target {
            id: hwnd as u64,
            pid,
            birth,
            x: 0.0,
            y: 0.0,
            width,
            height,
        })
    }
}

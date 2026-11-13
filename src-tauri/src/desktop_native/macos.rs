//! Process-scoped macOS events and window-only capture. No global CGEventPost.
use super::{image_bytes, Image, Target, Word};
use crate::vision::Rect;
use block2::RcBlock;
use objc2::{
    msg_send,
    rc::{autoreleasepool, Retained},
    runtime::{AnyClass, AnyObject, Bool},
};
use objc2_foundation::{NSPoint, NSRange, NSRect, NSSize, NSString};
use std::{
    cell::{Cell, RefCell},
    ffi::{c_void, CStr},
    ptr,
    sync::{mpsc, OnceLock},
    time::Duration,
};

type Ref = *const c_void;
#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGWindowListCopyWindowInfo(option: u32, relative: u32) -> Ref;
    fn CGWindowListCreateImage(bounds: NSRect, option: u32, window: u32, image_option: u32) -> Ref;
    fn CGRectMakeWithDictionaryRepresentation(dict: Ref, rect: *mut NSRect) -> bool;
    fn CGPreflightScreenCaptureAccess() -> bool;
    fn CGImageGetWidth(image: Ref) -> usize;
    fn CGImageGetHeight(image: Ref) -> usize;
    fn CGColorSpaceCreateDeviceRGB() -> Ref;
    fn CGBitmapContextCreate(
        data: *mut c_void,
        width: usize,
        height: usize,
        bits: usize,
        stride: usize,
        color: Ref,
        info: u32,
    ) -> Ref;
    fn CGBitmapContextCreateImage(context: Ref) -> Ref;
    fn CGContextDrawImage(context: Ref, rect: NSRect, image: Ref);
    fn CGEventCreateKeyboardEvent(source: Ref, key: u16, down: bool) -> Ref;
    fn CGEventSourceCreate(state: i32) -> Ref;
    fn CGEventCreateMouseEvent(source: Ref, event_type: u32, point: NSPoint, button: u32) -> Ref;
    fn CGEventKeyboardSetUnicodeString(event: Ref, len: usize, text: *const u16);
    fn CGEventSetFlags(event: Ref, flags: u64);
    fn CGEventPostToPid(pid: i32, event: Ref);
    static kCGWindowNumber: Ref;
    static kCGWindowOwnerPID: Ref;
    static kCGWindowLayer: Ref;
    static kCGWindowBounds: Ref;
    static kCGWindowIsOnscreen: Ref;
}
#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFArrayGetCount(array: Ref) -> isize;
    fn CFArrayGetValueAtIndex(array: Ref, index: isize) -> Ref;
    fn CFDictionaryGetValue(dict: Ref, key: Ref) -> Ref;
    fn CFNumberGetValue(number: Ref, kind: i32, value: *mut c_void) -> bool;
    fn CFBooleanGetValue(boolean: Ref) -> bool;
    fn CFRetain(value: Ref) -> Ref;
    fn CFRelease(value: Ref);
}
#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn AXIsProcessTrusted() -> bool;
}
#[link(name = "System")]
unsafe extern "C" {
    fn proc_pidpath(pid: i32, buffer: *mut c_void, size: u32) -> i32;
    fn dlopen(path: *const std::ffi::c_char, flags: i32) -> *mut c_void;
}
#[link(name = "Vision", kind = "framework")]
unsafe extern "C" {}
#[link(name = "AppKit", kind = "framework")]
unsafe extern "C" {}

struct Cf(Ref);
impl Drop for Cf {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                CFRelease(self.0);
            }
        }
    }
}
fn class(name: &CStr) -> Result<&'static AnyClass, String> {
    AnyClass::get(name)
        .ok_or_else(|| format!("macOS API를 사용할 수 없습니다: {}", name.to_string_lossy()))
}
unsafe fn object(pointer: *mut AnyObject) -> Result<Retained<AnyObject>, String> {
    unsafe { Retained::from_raw(pointer) }.ok_or_else(|| "macOS 객체 생성에 실패했습니다.".into())
}
unsafe fn description(error: *mut AnyObject) -> String {
    if error.is_null() {
        return "알 수 없는 macOS 오류".into();
    }
    let text: *mut NSString = unsafe { msg_send![error, localizedDescription] };
    if text.is_null() {
        "macOS 작업에 실패했습니다.".into()
    } else {
        unsafe { (&*text).to_string() }
    }
}
fn rect(width: usize, height: usize) -> NSRect {
    NSRect::new(
        NSPoint::new(0.0, 0.0),
        NSSize::new(width as f64, height as f64),
    )
}

pub(super) struct Native {
    source: Cf,
    flags: Cell<u64>,
    pid: Cell<u32>,
    bundle: RefCell<Option<String>>,
}
impl Native {
    pub(super) fn new() -> Result<Self, String> {
        // Keep optional Objective-C classes loaded for the process lifetime,
        // including callbacks that may finish after a timed-out request.
        // Older macOS versions continue using the CoreGraphics fallback.
        static CAPTURE_FRAMEWORK: OnceLock<usize> = OnceLock::new();
        CAPTURE_FRAMEWORK.get_or_init(|| unsafe {
            dlopen(
                c"/System/Library/Frameworks/ScreenCaptureKit.framework/ScreenCaptureKit".as_ptr(),
                1,
            ) as usize
        });
        if !unsafe { CGPreflightScreenCaptureAccess() } {
            return Err("시스템 설정 → 개인정보 보호 및 보안 → 화면 기록에서 Acrux를 허용한 뒤 앱을 다시 실행하세요.".into());
        }
        if !unsafe { AXIsProcessTrusted() } {
            return Err(
                "시스템 설정 → 개인정보 보호 및 보안 → 손쉬운 사용에서 Acrux를 허용하세요.".into(),
            );
        }
        let source = Cf(unsafe { CGEventSourceCreate(-1) });
        if source.0.is_null() {
            return Err("독립 게임 입력 소스를 만들지 못했습니다.".into());
        }
        Ok(Self {
            source,
            flags: Cell::new(0),
            pid: Cell::new(0),
            bundle: RefCell::new(None),
        })
    }
    pub(super) fn find_now(&self) -> Result<Option<Target>, String> {
        let targets: Vec<_> = windows()
            .into_iter()
            .filter(|t| self.pid.get() == 0 || self.pid.get() == t.pid)
            .collect();
        match targets.len() {
            0 => Ok(None),
            1 => {
                let target = targets[0];
                self.pid.set(target.pid);
                if let Some(path) = process_path(target.pid) {
                    *self.bundle.borrow_mut() = path
                        .strip_suffix("/Contents/MacOS/Roblox")
                        .map(str::to_owned);
                }
                Ok(Some(target))
            }
            _ => Err("Roblox 게임 창이 여러 개라 입력 대상을 정하지 못했습니다.".into()),
        }
    }
    pub(super) fn valid(&self, target: Target) -> bool {
        self.pid.get() == target.pid && windows().contains(&target)
    }
    fn owned(&self, target: Target) -> bool {
        unsafe {
            if process_path(target.pid).is_none() {
                return false;
            }
            // All windows includes minimized ones: their original key-up/button-up
            // must still be deliverable while new capture/input is forbidden.
            let list = Cf(CGWindowListCopyWindowInfo(16, 0));
            if list.0.is_null() {
                return false;
            }
            for i in 0..CFArrayGetCount(list.0) {
                let dict = CFArrayGetValueAtIndex(list.0, i);
                if number(dict, kCGWindowNumber) == Some(target.id)
                    && number(dict, kCGWindowOwnerPID) == Some(target.pid as u64)
                {
                    return true;
                }
            }
            false
        }
    }

    pub(super) fn capture(&self, target: Target) -> Result<Image, String> {
        if !unsafe { CGPreflightScreenCaptureAccess() } {
            return Err("macOS 화면 기록 권한이 해제됐습니다.".into());
        }
        if !self.valid(target) {
            return Err("게임 창이 이동·최소화되거나 크기가 변경됐습니다. 다시 연결하세요.".into());
        }
        autoreleasepool(|_| unsafe {
            // SCScreenshotManager is available on macOS 14+. Its desktop-independent
            // window filter captures an occluded game without compositing other apps.
            let captured = if AnyClass::get(c"SCScreenshotManager").is_some() {
                screenshot(target)?
            } else {
                let image = CGWindowListCreateImage(rect(0, 0), 8, target.id as u32, 1);
                if image.is_null() {
                    return Err("macOS가 게임 창 캡처를 거부했습니다.".into());
                }
                Cf(image)
            };
            if !self.valid(target) {
                return Err("캡처 중 게임 창이 변경됐습니다.".into());
            }
            let image = decode(captured.0, target.width, target.height)?;
            image.validate()?;
            Ok(image)
        })
    }
    fn event(&self, target: Target, event: Ref) -> Result<(), String> {
        let event = Cf(event);
        if event.0.is_null() {
            return Err("macOS 게임 입력을 만들지 못했습니다.".into());
        }
        if !unsafe { AXIsProcessTrusted() } || !self.owned(target) {
            return Err("게임 입력 대상이나 macOS 손쉬운 사용 권한이 사라졌습니다.".into());
        }
        // This posts to the selected PID, not the session/HID event stream. It does
        // not move the physical cursor or activate the target application.
        unsafe {
            CGEventPostToPid(target.pid as i32, event.0);
        }
        Ok(())
    }
    pub(super) fn key(&self, target: Target, vk: u16, down: bool) -> Result<(), String> {
        let code = keycode(vk).ok_or("지원하지 않는 macOS 키입니다.")?;
        let flag = match vk {
            0x10 | 0xa0 | 0xa1 => 1 << 17,
            0x11 | 0xa2 | 0xa3 => 1 << 18,
            0x12 | 0xa4 | 0xa5 => 1 << 19,
            0x5b | 0x5c => 1 << 20,
            _ => 0,
        };
        self.flags.set(if down {
            self.flags.get() | flag
        } else {
            self.flags.get() & !flag
        });
        let event = unsafe { CGEventCreateKeyboardEvent(self.source.0, code, down) };
        if !event.is_null() {
            unsafe {
                CGEventSetFlags(event, self.flags.get());
            }
        }
        self.event(target, event)
    }
    pub(super) fn move_pointer(
        &self,
        target: Target,
        at: (i32, i32),
        right_drag: bool,
    ) -> Result<(), String> {
        self.event(target, unsafe {
            CGEventCreateMouseEvent(
                self.source.0,
                if right_drag { 7 } else { 5 },
                NSPoint::new(target.x + at.0 as f64, target.y + at.1 as f64),
                if right_drag { 1 } else { 0 },
            )
        })
    }
    pub(super) fn button(
        &self,
        target: Target,
        at: (i32, i32),
        right: bool,
        down: bool,
    ) -> Result<(), String> {
        self.event(target, unsafe {
            CGEventCreateMouseEvent(
                self.source.0,
                match (right, down) {
                    (false, true) => 1,
                    (false, false) => 2,
                    (true, true) => 3,
                    (true, false) => 4,
                },
                NSPoint::new(target.x + at.0 as f64, target.y + at.1 as f64),
                if right { 1 } else { 0 },
            )
        })
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
        // Command+A scoped to the game, then Unicode keyboard events. Never touch clipboard.
        if !current() {
            return Err("텍스트 입력을 취소했습니다.".into());
        }
        for down in [true, false] {
            let event = unsafe { CGEventCreateKeyboardEvent(self.source.0, 0, down) };
            if !event.is_null() {
                unsafe {
                    CGEventSetFlags(event, 1 << 20);
                }
            }
            self.event(target, event)?;
        }
        self.key(target, 0x08, true)?;
        self.key(target, 0x08, false)?;
        // Keep Unicode scalars together, including UTF-16 surrogate pairs.
        for character in text.chars() {
            if !current() {
                return Err("텍스트 입력을 취소했습니다.".into());
            }
            let mut pair = [0u16; 2];
            let units = character.encode_utf16(&mut pair);
            for down in [true, false] {
                let event = unsafe { CGEventCreateKeyboardEvent(self.source.0, 0, down) };
                if !event.is_null() {
                    unsafe {
                        CGEventKeyboardSetUnicodeString(event, units.len(), units.as_ptr());
                    }
                }
                self.event(target, event)?;
            }
        }
        Ok(())
    }
    pub(super) fn launch(&self, target: Target, url: &str) -> Result<(), String> {
        if target.pid != self.pid.get() {
            return Err("소유 Roblox 프로세스가 바뀌었습니다.".into());
        }
        self.launch_missing(url)
    }
    pub(super) fn launch_missing(&self, url: &str) -> Result<(), String> {
        let url = crate::automation::game_url(url)?;
        if !url.starts_with("roblox://") || url.len() > 8192 || url.chars().any(char::is_control) {
            return Err("Roblox 재접속 주소가 잘못됐습니다.".into());
        }
        let bundle = self
            .bundle
            .borrow()
            .clone()
            .or_else(|| {
                let mut candidates = vec![std::path::PathBuf::from("/Applications/Roblox.app")];
                if let Some(home) = std::env::var_os("HOME") {
                    candidates.push(std::path::PathBuf::from(home).join("Applications/Roblox.app"));
                }
                candidates
                    .into_iter()
                    .find(|p| p.join("Contents/MacOS/Roblox").is_file())
                    .and_then(|p| p.to_str().map(str::to_owned))
            })
            .ok_or("Roblox 설치 경로가 없습니다. Roblox를 한 번 실행한 뒤 다시 연결하세요.")?;
        autoreleasepool(|_| unsafe {
            let path = NSString::from_str(&bundle);
            let app_url: *mut AnyObject = msg_send![class(c"NSURL")?,fileURLWithPath:&*path];
            let text = NSString::from_str(&url);
            let link: *mut AnyObject = msg_send![class(c"NSURL")?,URLWithString:&*text];
            if app_url.is_null() || link.is_null() {
                return Err("Roblox 앱 경로나 실행 주소가 잘못됐습니다.".into());
            }
            let links: *mut AnyObject = msg_send![class(c"NSArray")?,arrayWithObject:link];
            let config: *mut AnyObject =
                msg_send![class(c"NSWorkspaceOpenConfiguration")?, configuration];
            let _: () = msg_send![config,setActivates:Bool::NO];
            let _: () = msg_send![config,setCreatesNewApplicationInstance:Bool::NO];
            let workspace: *mut AnyObject = msg_send![class(c"NSWorkspace")?, sharedWorkspace];
            let (send, receive) = mpsc::channel();
            let block = RcBlock::new(move |app: *mut AnyObject, error: *mut AnyObject| {
                let result = if !error.is_null() {
                    Err(description(error))
                } else if app.is_null() {
                    Err("Roblox 실행 결과가 없습니다.".into())
                } else {
                    let pid: i32 = msg_send![app, processIdentifier];
                    Ok(pid as u32)
                };
                let _ = send.send(result);
            });
            let _: () = msg_send![workspace,openURLs:links,withApplicationAtURL:app_url,configuration:config,completionHandler:&*block];
            let pid = receive
                .recv_timeout(Duration::from_secs(3))
                .map_err(|_| "Roblox 재실행 확인 시간이 초과됐습니다.".to_owned())??;
            let actual = process_path(pid)
                .and_then(|p| p.strip_suffix("/Contents/MacOS/Roblox").map(str::to_owned));
            if actual.as_deref() != Some(bundle.as_str()) {
                return Err("재실행한 앱의 프로세스가 원래 Roblox 앱과 일치하지 않습니다.".into());
            }
            self.pid.set(pid);
            *self.bundle.borrow_mut() = Some(bundle);
            Ok(())
        })
    }
    pub(super) fn ocr(&self, image: &Image) -> Result<Vec<Word>, String> {
        autoreleasepool(|_| unsafe {
            let cg = encode(image)?;
            let request = object(msg_send![class(c"VNRecognizeTextRequest")?, new])?;
            let _: () = msg_send![&*request,setRecognitionLevel:1isize];
            let _: () = msg_send![&*request,setUsesLanguageCorrection:Bool::NO];
            let options: *mut AnyObject = msg_send![class(c"NSDictionary")?, dictionary];
            let allocation: *mut AnyObject = msg_send![class(c"VNImageRequestHandler")?, alloc];
            let handler = object(msg_send![allocation,initWithCGImage:cg.0, options:options])?;
            let requests: *mut AnyObject = msg_send![class(c"NSArray")?,arrayWithObject:&*request];
            let mut error: *mut AnyObject = ptr::null_mut();
            let success: Bool = msg_send![&*handler,performRequests:requests, error:&mut error];
            if !success.as_bool() {
                return Err(format!("macOS 텍스트 인식 실패: {}", description(error)));
            }
            let observations: *mut AnyObject = msg_send![&*request, results];
            if observations.is_null() {
                return Ok(Vec::new());
            }
            let count: usize = msg_send![observations, count];
            let mut result = Vec::new();
            for i in 0..count.min(4096) {
                let observation: *mut AnyObject = msg_send![observations,objectAtIndex:i];
                let candidates: *mut AnyObject = msg_send![observation,topCandidates:1usize];
                let len: usize = msg_send![candidates, count];
                if len == 0 {
                    continue;
                }
                let candidate: *mut AnyObject = msg_send![candidates,objectAtIndex:0usize];
                let string: *mut NSString = msg_send![candidate, string];
                if string.is_null() {
                    continue;
                }
                let text = (&*string).to_string();
                let fallback: NSRect = msg_send![observation, boundingBox];
                let mut offset = 0usize;
                for part in text.split_inclusive(char::is_whitespace) {
                    let token = part.trim_end();
                    if !token.is_empty() {
                        let mut error: *mut AnyObject = ptr::null_mut();
                        let box_object: *mut AnyObject = msg_send![candidate,boundingBoxForRange:NSRange::new(offset,token.encode_utf16().count()), error:&mut error];
                        let b: NSRect = if box_object.is_null() {
                            fallback
                        } else {
                            msg_send![box_object, boundingBox]
                        };
                        result.push(Word {
                            text: token.into(),
                            rect: Rect(
                                b.origin.x,
                                1.0 - b.origin.y - b.size.height,
                                b.origin.x + b.size.width,
                                1.0 - b.origin.y,
                            ),
                        });
                    }
                    offset += part.encode_utf16().count();
                }
            }
            Ok(result)
        })
    }
}

unsafe fn screenshot(target: Target) -> Result<Cf, String> {
    unsafe {
        let (send, receive) = mpsc::channel();
        let block = RcBlock::new(move |content: *mut AnyObject, error: *mut AnyObject| {
            let result = if !error.is_null() {
                Err(description(error))
            } else if content.is_null() {
                Err("공유 가능한 게임 창을 확인하지 못했습니다.".into())
            } else {
                {
                    let retained: *mut AnyObject = msg_send![content, retain];
                    Ok(retained as usize)
                }
            };
            if let Err(mpsc::SendError(Ok(pointer))) = send.send(result) {
                let _: () = msg_send![pointer as *mut AnyObject, release];
            }
        });
        let _: () = msg_send![class(c"SCShareableContent")?,getShareableContentExcludingDesktopWindows:Bool::YES, onScreenWindowsOnly:Bool::YES, completionHandler:&*block];
        let content = object(
            receive
                .recv_timeout(Duration::from_secs(3))
                .map_err(|_| "게임 창 확인 시간이 초과됐습니다.".to_owned())??
                as *mut AnyObject,
        )?;
        let windows: *mut AnyObject = msg_send![&*content, windows];
        let count: usize = msg_send![windows, count];
        let mut selected: *mut AnyObject = ptr::null_mut();
        for i in 0..count {
            let w: *mut AnyObject = msg_send![windows,objectAtIndex:i];
            let id: u32 = msg_send![w, windowID];
            if id == target.id as u32 {
                selected = w;
                break;
            }
        }
        if selected.is_null() {
            return Err("ScreenCaptureKit에서 게임 창을 찾지 못했습니다.".into());
        }
        let allocated: *mut AnyObject = msg_send![class(c"SCContentFilter")?, alloc];
        let filter = object(msg_send![allocated,initWithDesktopIndependentWindow:selected])?;
        let config = object(msg_send![class(c"SCStreamConfiguration")?, new])?;
        let _: () = msg_send![&*config,setWidth:target.width];
        let _: () = msg_send![&*config,setHeight:target.height];
        let _: () = msg_send![&*config,setShowsCursor:Bool::NO];
        let _: () = msg_send![&*config,setIgnoreShadowsSingleWindow:Bool::YES];
        let (send, receive) = mpsc::channel();
        let block = RcBlock::new(move |image: Ref, error: *mut AnyObject| {
            let result = if !error.is_null() {
                Err(description(error))
            } else if image.is_null() {
                Err("게임 화면 캡처 결과가 비어 있습니다.".into())
            } else {
                Ok(CFRetain(image) as usize)
            };
            if let Err(mpsc::SendError(Ok(pointer))) = send.send(result) {
                CFRelease(pointer as Ref);
            }
        });
        let _: () = msg_send![class(c"SCScreenshotManager")?,captureImageWithFilter:&*filter, configuration:&*config, completionHandler:&*block];
        Ok(Cf(receive
            .recv_timeout(Duration::from_secs(3))
            .map_err(|_| "게임 창 캡처 시간이 초과됐습니다.".to_owned())??
            as Ref))
    }
}
unsafe fn decode(image: Ref, width: usize, height: usize) -> Result<Image, String> {
    unsafe {
        if CGImageGetWidth(image) == 0 || CGImageGetHeight(image) == 0 {
            return Err("macOS 화면 크기가 잘못됐습니다.".into());
        }
        let mut result = Image {
            data: vec![0; image_bytes(width, height)?],
            width,
            height,
        };
        let color = Cf(CGColorSpaceCreateDeviceRGB());
        let context = Cf(CGBitmapContextCreate(
            result.data.as_mut_ptr().cast(),
            width,
            height,
            8,
            width * 4,
            color.0,
            0x2002,
        ));
        if context.0.is_null() {
            return Err("macOS 픽셀 변환에 실패했습니다.".into());
        }
        CGContextDrawImage(context.0, rect(width, height), image);
        Ok(result)
    }
}
unsafe fn encode(image: &Image) -> Result<Cf, String> {
    unsafe {
        let color = Cf(CGColorSpaceCreateDeviceRGB());
        // The bitmap context does not mutate its input for CreateImage.
        let context = Cf(CGBitmapContextCreate(
            image.data.as_ptr() as *mut c_void,
            image.width,
            image.height,
            8,
            image.width * 4,
            color.0,
            0x2002,
        ));
        if context.0.is_null() {
            return Err("macOS 텍스트 인식 이미지를 만들지 못했습니다.".into());
        }
        let result = CGBitmapContextCreateImage(context.0);
        if result.is_null() {
            return Err("macOS 텍스트 인식 이미지가 비어 있습니다.".into());
        }
        Ok(Cf(result))
    }
}
fn number(dict: Ref, key: Ref) -> Option<u64> {
    unsafe {
        let value = CFDictionaryGetValue(dict, key);
        if value.is_null() {
            return None;
        }
        let mut n = 0i64;
        CFNumberGetValue(value, 4, (&mut n as *mut i64).cast()).then_some(n as u64)
    }
}
fn process_path(pid: u32) -> Option<String> {
    let mut path = [0u8; 4096];
    let len = unsafe { proc_pidpath(pid as i32, path.as_mut_ptr().cast(), path.len() as u32) };
    if len <= 0 {
        return None;
    }
    let nul = path.iter().position(|&c| c == 0).unwrap_or(len as usize);
    let path = String::from_utf8(path[..nul].to_vec()).ok()?;
    path.ends_with("/Roblox.app/Contents/MacOS/Roblox")
        .then_some(path)
}
fn windows() -> Vec<Target> {
    unsafe {
        let list = Cf(CGWindowListCopyWindowInfo(1 | 16, 0));
        if list.0.is_null() {
            return Vec::new();
        }
        let mut result = Vec::new();
        for i in 0..CFArrayGetCount(list.0) {
            let dict = CFArrayGetValueAtIndex(list.0, i);
            let Some(id) = number(dict, kCGWindowNumber) else {
                continue;
            };
            let Some(pid) = number(dict, kCGWindowOwnerPID) else {
                continue;
            };
            if number(dict, kCGWindowLayer) != Some(0) || process_path(pid as u32).is_none() {
                continue;
            }
            let onscreen = CFDictionaryGetValue(dict, kCGWindowIsOnscreen);
            if onscreen.is_null() || !CFBooleanGetValue(onscreen) {
                continue;
            }
            let bounds = CFDictionaryGetValue(dict, kCGWindowBounds);
            let mut b = NSRect::default();
            if bounds.is_null() || !CGRectMakeWithDictionaryRepresentation(bounds, &mut b) {
                continue;
            }
            let (width, height) = (
                b.size.width.round() as usize,
                b.size.height.round() as usize,
            );
            if image_bytes(width, height).is_err() {
                continue;
            }
            result.push(Target {
                id,
                pid: pid as u32,
                birth: id,
                x: b.origin.x,
                y: b.origin.y,
                width,
                height,
            });
        }
        result
    }
}
pub(super) fn key_supported(vk: u16) -> bool {
    keycode(vk).is_some()
}
fn keycode(vk: u16) -> Option<u16> {
    Some(match vk {
        0x41 => 0,
        0x53 => 1,
        0x44 => 2,
        0x46 => 3,
        0x48 => 4,
        0x47 => 5,
        0x5a => 6,
        0x58 => 7,
        0x43 => 8,
        0x56 => 9,
        0x42 => 11,
        0x51 => 12,
        0x57 => 13,
        0x45 => 14,
        0x52 => 15,
        0x59 => 16,
        0x54 => 17,
        0x31 => 18,
        0x32 => 19,
        0x33 => 20,
        0x34 => 21,
        0x36 => 22,
        0x35 => 23,
        0xbb => 24,
        0x39 => 25,
        0x37 => 26,
        0xbd => 27,
        0x38 => 28,
        0x30 => 29,
        0xdd => 30,
        0x4f => 31,
        0x55 => 32,
        0xdb => 33,
        0x49 => 34,
        0x50 => 35,
        0x0d => 36,
        0x4c => 37,
        0x4a => 38,
        0xde => 39,
        0x4b => 40,
        0xba => 41,
        0xdc => 42,
        0xbc => 43,
        0xbf => 44,
        0x4e => 45,
        0x4d => 46,
        0xbe => 47,
        0x09 => 48,
        0x20 => 49,
        0xc0 => 50,
        0x08 => 51,
        0x1b => 53,
        0x5b | 0x5c => 55,
        0x10 | 0xa0 => 56,
        0x14 => 57,
        0x12 | 0xa4 => 58,
        0x11 | 0xa2 => 59,
        0xa1 => 60,
        0xa5 => 61,
        0xa3 => 62,
        0x70 => 122,
        0x71 => 120,
        0x72 => 99,
        0x73 => 118,
        0x74 => 96,
        0x75 => 97,
        0x76 => 98,
        0x77 => 100,
        0x78 => 101,
        0x79 => 109,
        0x7a => 103,
        0x7b => 111,
        0x24 => 115,
        0x21 => 116,
        0x2e => 117,
        0x23 => 119,
        0x22 => 121,
        0x25 => 123,
        0x27 => 124,
        0x28 => 125,
        0x26 => 126,
        _ => return None,
    })
}

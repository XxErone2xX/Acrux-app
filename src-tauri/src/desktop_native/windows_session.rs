//! Normal Windows desktop. Existing Roblox is attached, never killed on detach.
use super::Handle;
use std::{
    collections::BTreeSet,
    ffi::c_void,
    mem::{size_of, zeroed},
    path::PathBuf,
    ptr,
    sync::atomic::Ordering,
};
#[repr(C)]
struct Startup {
    size: u32,
    reserved: *mut u16,
    desktop: *mut u16,
    title: *mut u16,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    xchars: u32,
    ychars: u32,
    fill: u32,
    flags: u32,
    show: u16,
    reserved_size: u16,
    reserved_bytes: *mut u8,
    input: Handle,
    output: Handle,
    error: Handle,
}
#[repr(C)]
struct Process {
    process: Handle,
    thread: Handle,
    pid: u32,
    tid: u32,
}
#[repr(C)]
struct BasicLimits {
    process_time: i64,
    job_time: i64,
    flags: u32,
    min_working: usize,
    max_working: usize,
    active: u32,
    affinity: usize,
    priority: u32,
    scheduling: u32,
}
#[repr(C)]
struct Limits {
    basic: BasicLimits,
    io: [u64; 6],
    process_memory: usize,
    job_memory: usize,
    peak_process: usize,
    peak_job: usize,
}
#[repr(C)]
struct ProcessEntry {
    size: u32,
    usage: u32,
    pid: u32,
    heap: usize,
    module: u32,
    threads: u32,
    parent: u32,
    priority: i32,
    flags: u32,
    name: [u16; 260],
}
#[link(name = "user32")]
unsafe extern "system" {
    fn EnumWindows(callback: unsafe extern "system" fn(Handle, isize) -> i32, param: isize) -> i32;
}
#[link(name = "kernel32")]
unsafe extern "system" {
    fn CreateProcessW(
        app: *const u16,
        args: *mut u16,
        process_security: *const c_void,
        thread_security: *const c_void,
        inherit: i32,
        flags: u32,
        environment: *const c_void,
        directory: *const u16,
        startup: *const Startup,
        info: *mut Process,
    ) -> i32;
    fn CreateJobObjectW(security: *const c_void, name: *const u16) -> Handle;
    fn SetInformationJobObject(job: Handle, class: u32, data: *const c_void, len: u32) -> i32;
    fn AssignProcessToJobObject(job: Handle, process: Handle) -> i32;
    fn IsProcessInJob(process: Handle, job: Handle, result: *mut i32) -> i32;
    fn OpenProcess(access: u32, inherit: i32, pid: u32) -> Handle;
    fn ResumeThread(thread: Handle) -> u32;
    fn TerminateProcess(process: Handle, code: u32) -> i32;
    fn CloseHandle(handle: Handle) -> i32;
    fn GetLastError() -> u32;
    fn LocalFree(memory: *mut c_void) -> *mut c_void;
    fn CreateToolhelp32Snapshot(flags: u32, pid: u32) -> Handle;
    fn Process32FirstW(snapshot: Handle, entry: *mut ProcessEntry) -> i32;
    fn Process32NextW(snapshot: Handle, entry: *mut ProcessEntry) -> i32;
}
#[link(name = "shell32")]
unsafe extern "system" {
    fn CommandLineToArgvW(command: *const u16, count: *mut i32) -> *mut *mut u16;
}
#[link(name = "advapi32")]
unsafe extern "system" {
    fn RegGetValueW(
        key: Handle,
        subkey: *const u16,
        name: *const u16,
        flags: u32,
        kind: *mut u32,
        data: *mut c_void,
        len: *mut u32,
    ) -> i32;
}
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain([0]).collect()
}
struct Owned(Handle);
impl Drop for Owned {
    fn drop(&mut self) {
        if self.0 != 0 && self.0 != -1 {
            unsafe {
                CloseHandle(self.0);
            }
        }
    }
}

pub(super) struct Session {
    attached: BTreeSet<(u32, u64)>,
    job: Owned,
    executable: PathBuf,
    arguments: Vec<String>,
}
impl Session {
    pub(super) fn new(url: &str) -> Result<Self, String> {
        let (executable, arguments) = handler()?;
        let attached = all_windows()
            .into_iter()
            .map(|target| (target.pid, target.birth))
            .collect();
        let job = Owned(unsafe { CreateJobObjectW(ptr::null(), ptr::null()) });
        if job.0 == 0 {
            return Err("게임 프로세스 그룹을 만들지 못했습니다.".into());
        }
        let mut limits: Limits = unsafe { zeroed() };
        limits.basic.flags = 0x2000; // Only processes started by Acrux die with this Job.
        if unsafe {
            SetInformationJobObject(
                job.0,
                9,
                (&limits as *const Limits).cast(),
                size_of::<Limits>() as u32,
            )
        } == 0
        {
            return Err("게임 프로세스 관리를 준비하지 못했습니다.".into());
        }
        let session = Self {
            attached,
            job,
            executable,
            arguments,
        };
        if session.attached.is_empty() {
            if !roblox_processes()?.is_empty() {
                return Err(
                    "Roblox가 시작 중이거나 최소화돼 있습니다. 게임 창을 연 뒤 다시 연결하세요."
                        .into(),
                );
            }
            session.open(url)?;
        }
        Ok(session)
    }
    pub(super) fn owns(&self, pid: u32) -> bool {
        let process = Owned(unsafe { OpenProcess(0x1000, 0, pid) });
        if process.0 == 0 {
            return false;
        }
        let mut yes = 0;
        unsafe { IsProcessInJob(process.0, self.job.0, &mut yes) != 0 && yes != 0 }
    }
    pub(super) fn windows(&self) -> Vec<super::Target> {
        all_windows()
            .into_iter()
            .filter(|target| self.accepts(*target))
            .collect()
    }
    pub(super) fn accepts(&self, target: super::Target) -> bool {
        self.owns(target.pid) || self.attached.contains(&(target.pid, target.birth))
    }
    pub(super) fn open(&self, url: &str) -> Result<(), String> {
        if super::super::STOP.load(Ordering::Acquire) || !valid_url(url) {
            return Err("Roblox 실행 주소나 실행 상태가 올바르지 않습니다.".into());
        }
        let executable = self
            .executable
            .to_str()
            .ok_or("Roblox 설치 경로를 읽지 못했습니다.")?;
        let app = wide(executable);
        let mut arguments = vec![quote_argument(executable)];
        for argument in &self.arguments {
            arguments.push(quote_argument(&url_argument(argument, url)));
        }
        let mut command = wide(&arguments.join(" "));
        let mut startup: Startup = unsafe { zeroed() };
        startup.size = size_of::<Startup>() as u32;
        startup.flags = 1;
        startup.show = 5;
        let mut info: Process = unsafe { zeroed() };
        // Start suspended, then bind the process and all descendants to the Job
        // before any Roblox code executes. Only this Job is killed on stop.
        if unsafe {
            CreateProcessW(
                app.as_ptr(),
                command.as_mut_ptr(),
                ptr::null(),
                ptr::null(),
                0,
                4,
                ptr::null(),
                ptr::null(),
                &startup,
                &mut info,
            )
        } == 0
        {
            return Err("Roblox를 실행하지 못했습니다.".into());
        }
        let process = Owned(info.process);
        let thread = Owned(info.thread);
        if unsafe { AssignProcessToJobObject(self.job.0, process.0) } == 0
            || super::super::STOP.load(Ordering::Acquire)
            || unsafe { ResumeThread(thread.0) } == u32::MAX
        {
            unsafe {
                TerminateProcess(process.0, 1);
            }
            return Err("소유 게임 프로세스의 안전한 시작을 확인하지 못했습니다.".into());
        }
        Ok(())
    }
}
fn all_windows() -> Vec<super::Target> {
    let mut targets = Vec::new();
    unsafe {
        EnumWindows(
            super::collect,
            &mut targets as *mut Vec<super::Target> as isize,
        );
    }
    targets
}
fn valid_url(url: &str) -> bool {
    url.starts_with("roblox://")
        && url.len() <= 8192
        && !url.chars().any(|c| c.is_control() || c == '"' || c == '\\')
}
fn handler() -> Result<(PathBuf, Vec<String>), String> {
    for key in [
        "roblox\\shell\\open\\command",
        "roblox-player\\shell\\open\\command",
    ] {
        let key = wide(key);
        let mut bytes = 65536u32;
        let mut value = vec![0u16; bytes as usize / 2];
        if unsafe {
            RegGetValueW(
                -2147483648isize,
                key.as_ptr(),
                ptr::null(),
                2 | 4,
                ptr::null_mut(),
                value.as_mut_ptr().cast(),
                &mut bytes,
            )
        } != 0
        {
            continue;
        }
        let n = value.iter().position(|&u| u == 0).unwrap_or(value.len());
        let command =
            String::from_utf16(&value[..n]).map_err(|_| "Roblox 실행 경로가 잘못됐습니다.")?;
        let command = wide(&command);
        let mut count = 0;
        let parsed = unsafe { CommandLineToArgvW(command.as_ptr(), &mut count) };
        if parsed.is_null() || count <= 0 {
            continue;
        }
        let mut arguments = Vec::new();
        for i in 0..count as usize {
            let arg = unsafe { *parsed.add(i) };
            let mut len = 0;
            while unsafe { *arg.add(len) } != 0 {
                len += 1;
            }
            arguments.push(String::from_utf16_lossy(unsafe {
                std::slice::from_raw_parts(arg, len)
            }));
        }
        unsafe {
            LocalFree(parsed.cast());
        }
        let path = PathBuf::from(arguments.remove(0));
        let allowed = path
            .file_name()
            .and_then(|p| p.to_str())
            .is_some_and(|name| {
                name.eq_ignore_ascii_case("RobloxPlayerBeta.exe")
                    || name.eq_ignore_ascii_case("RobloxPlayerLauncher.exe")
            });
        if allowed
            && path.is_absolute()
            && path.is_file()
            && arguments
                .iter()
                .any(|s| s.contains("%1") || s.contains("%L") || s.contains("%l"))
        {
            return Ok((path, arguments));
        }
    }
    Err(
        "Windows의 Roblox 실행 프로그램을 찾지 못했습니다. Roblox를 설치한 뒤 다시 시도하세요."
            .into(),
    )
}
/// Inverse of CommandLineToArgvW for one argv value, including trailing slashes.
fn quote_argument(value: &str) -> String {
    let mut result = String::from("\"");
    let mut slashes = 0;
    for ch in value.chars() {
        if ch == '\\' {
            slashes += 1;
            continue;
        }
        let count = if ch == '"' { slashes * 2 + 1 } else { slashes };
        result.extend(std::iter::repeat_n('\\', count));
        slashes = 0;
        result.push(ch);
    }
    result.extend(std::iter::repeat_n('\\', slashes * 2));
    result.push('"');
    result
}
fn url_argument(template: &str, url: &str) -> String {
    let mut result = String::new();
    let mut chars = template.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '%' && matches!(chars.peek(), Some('1' | 'L' | 'l')) {
            chars.next();
            result.push_str(url);
        } else {
            result.push(ch);
        }
    }
    result
}
fn roblox_processes() -> Result<Vec<u32>, String> {
    unsafe {
        let snapshot = Owned(CreateToolhelp32Snapshot(2, 0));
        if snapshot.0 == -1 {
            return Err("기존 Roblox 프로세스를 확인하지 못했습니다.".into());
        }
        let mut entry: ProcessEntry = zeroed();
        entry.size = size_of::<ProcessEntry>() as u32;
        let mut result = Vec::new();
        let mut present = Process32FirstW(snapshot.0, &mut entry) != 0;
        if !present {
            return Err("기존 Roblox 프로세스 목록을 읽지 못했습니다.".into());
        }
        while present {
            let n = entry
                .name
                .iter()
                .position(|&u| u == 0)
                .unwrap_or(entry.name.len());
            let name = String::from_utf16_lossy(&entry.name[..n]);
            if name.eq_ignore_ascii_case("RobloxPlayerBeta.exe")
                || name.eq_ignore_ascii_case("RobloxPlayerLauncher.exe")
            {
                result.push(entry.pid);
            }
            present = Process32NextW(snapshot.0, &mut entry) != 0;
            if !present && GetLastError() != 18 {
                return Err("기존 Roblox 프로세스 목록을 끝까지 읽지 못했습니다.".into());
            }
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn uri_cannot_change_command_line_or_escape_roblox_scheme() {
        assert!(valid_url("roblox://experiences/start?placeId=15532962292"));
        for url in [
            "https://roblox.com",
            "roblox://x\" --switch-desktop",
            "roblox://x\n",
            "roblox://\\x",
        ] {
            assert!(!valid_url(url));
        }
    }
    #[test]
    fn windows_argument_quoting_preserves_spaces_and_trailing_slashes() {
        assert_eq!(quote_argument(""), "\"\"");
        assert_eq!(quote_argument("a b"), "\"a b\"");
        assert_eq!(quote_argument("a\\"), "\"a\\\\\"");
        assert_eq!(quote_argument("a\"b"), "\"a\\\"b\"");
        assert_eq!(url_argument("%1", "roblox://a%L"), "roblox://a%L");
    }
    #[test]
    fn win64_ffi_structures_match_the_windows_abi() {
        assert_eq!(size_of::<Startup>(), 104);
        assert_eq!(size_of::<Process>(), 24);
        assert_eq!(size_of::<Limits>(), 144);
        assert_eq!(size_of::<ProcessEntry>(), 568);
    }
}

//! Owned, private Gamescope/Xwayland session. This module never connects to the
//! user's display and never closes a Sober instance it did not launch.
use serde::{Deserialize, Serialize};
use std::{
    env,
    ffi::{OsStr, OsString},
    fs::{self, File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    os::fd::AsRawFd,
    os::unix::{
        fs::{symlink, MetadataExt, OpenOptionsExt, PermissionsExt},
        net::UnixStream,
        process::CommandExt,
    },
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::{Duration, Instant},
};
use tempfile::TempDir;
use x11rb::{
    protocol::xproto::{Setup, SetupRequest},
    rust_connection::{DefaultStream, RustConnection},
    x11_utils::{Serialize as _, TryParse as _},
};

const APP: &str = "org.vinegarhq.Sober";
const HELPER: &str = "--acrux-private-session";
const SUPERVISOR: &str = "--acrux-private-supervisor";
const DIRECTORY: &str = "ACRUX_PRIVATE_DIRECTORY";
const TOKEN: &str = "ACRUX_PRIVATE_TOKEN";
const START_TIMEOUT: Duration = Duration::from_secs(15);
const POLL: Duration = Duration::from_millis(25);
const MIT_COOKIE: &[u8] = b"MIT-MAGIC-COOKIE-1";

#[derive(Clone, Copy, Deserialize, Serialize)]
struct Identity {
    pid: u32,
    started: u64,
    group: u32,
}

impl Identity {
    fn read(pid: u32) -> Result<Self, String> {
        let stat = fs::read_to_string(format!("/proc/{pid}/stat"))
            .map_err(|_| "세션 프로세스가 종료됐습니다.")?;
        let fields: Vec<_> = stat
            .rsplit_once(") ")
            .ok_or("프로세스 정보를 읽지 못했습니다.")?
            .1
            .split_whitespace()
            .collect();
        Ok(Self {
            pid,
            group: fields
                .get(2)
                .and_then(|s| s.parse().ok())
                .ok_or("프로세스 그룹이 올바르지 않습니다.")?,
            started: fields
                .get(19)
                .and_then(|s| s.parse().ok())
                .ok_or("프로세스 시작 정보가 올바르지 않습니다.")?,
        })
    }
    fn alive(self) -> bool {
        Self::read(self.pid)
            .is_ok_and(|current| current.started == self.started && current.group == self.group)
    }
}

#[derive(Deserialize, Serialize)]
struct Config {
    token: String,
    cookie: [u8; 16],
    owner: Identity,
    host_display: Option<u16>,
    flatpak: PathBuf,
    xwayland: PathBuf,
    gamescope: PathBuf,
    mode: Mode,
    #[serde(default)]
    url: Option<String>,
}

#[derive(Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
enum Mode {
    Sober,
    Fixture,
}

#[derive(Clone, Deserialize, Serialize)]
struct Xserver {
    token: String,
    display: u16,
    process: Identity,
}

#[derive(Clone, Deserialize, Serialize)]
struct Handshake {
    token: String,
    display: u16,
    process: Identity,
    instance: Option<String>,
}

/// An emergency stop remains usable while the worker is blocked in an X11 reply.
/// Every signal is restricted to process identities from our own startup.
#[derive(Clone)]
pub(super) struct StopHandle {
    directory: PathBuf,
    process: Identity,
    members: Vec<Identity>,
    token: String,
    flatpak: PathBuf,
    requested: Arc<AtomicBool>,
}

impl StopHandle {
    pub(super) fn stop(&self) -> Result<(), String> {
        if self.requested.swap(true, Ordering::AcqRel) {
            return Ok(());
        }
        if private_directory(&self.directory).is_ok() {
            let ready = read_json::<Handshake>(&self.directory.join("ready.json"))
                .ok()
                .filter(|ready| {
                    ready.token == self.token && ready.process.group == self.process.pid
                });
            if self.owns_group() || ready.is_some() {
                let _ = fs::write(self.directory.join("stop"), b"stop");
            }
            // Flatpak can outlive the compositor process group. The private,
            // token-bound instance ID is independent authorization to close it.
            if let Some(ready) = ready {
                if let Some(instance) = ready.instance {
                    kill_instance(&self.flatpak, &instance);
                }
            }
        }
        if let Some(group) = i32::try_from(self.process.pid)
            .ok()
            .and_then(rustix::process::Pid::from_raw)
        {
            if self.owns_group() {
                let _ = rustix::process::kill_process_group(group, rustix::process::Signal::TERM);
                thread::sleep(Duration::from_millis(50));
                if self.owns_group() {
                    let _ =
                        rustix::process::kill_process_group(group, rustix::process::Signal::KILL);
                }
            }
        }
        Ok(())
    }

    fn owns_group(&self) -> bool {
        self.process.alive() || self.members.iter().any(|process| process.alive())
    }
}

pub(super) struct Session {
    directory: TempDir,
    child: Child,
    process: Identity,
    config: Config,
    endpoint: Option<(Handshake, Xserver)>,
    stopped: bool,
}

impl Session {
    pub(super) fn launch_with_cancel(cancel: &AtomicBool) -> Result<Self, String> {
        Self::launch_mode(Mode::Sober, cancel, None, || true)
    }
    pub(super) fn launch_url_with_cancel(
        url: &str,
        cancel: &AtomicBool,
        current: impl Fn() -> bool,
    ) -> Result<Self, String> {
        if !url.starts_with("roblox://") || url.len() > 8192 || url.chars().any(char::is_control) {
            return Err("Roblox 재실행 주소가 잘못됐습니다.".into());
        }
        Self::launch_mode(Mode::Sober, cancel, Some(url.into()), current)
    }

    /// Only the explicit diagnostic command can create this non-game fixture.
    pub(super) fn launch_probe_with_cancel(cancel: &AtomicBool) -> Result<Self, String> {
        if env::var("ACRUX_SESSION_TEST").as_deref() != Ok("1") {
            return Err("전용 세션 진단은 명시적으로 허용한 실행에서만 가능합니다.".into());
        }
        Self::launch_mode(Mode::Fixture, cancel, None, || true)
    }

    fn launch_mode(
        mode: Mode,
        cancel: &AtomicBool,
        url: Option<String>,
        current: impl Fn() -> bool,
    ) -> Result<Self, String> {
        if cancel.load(Ordering::Acquire) || !current() {
            return Err("게임 세션 시작을 취소했습니다.".into());
        }
        let started = Instant::now();
        let deadline = started
            + if mode == Mode::Fixture {
                Duration::from_secs(5)
            } else {
                START_TIMEOUT
            };
        let flatpak = if mode == Mode::Sober {
            let flatpak =
                executable("flatpak").ok_or("Sober 연결에 필요한 Flatpak을 찾지 못했습니다.")?;
            ensure_no_sober(&flatpak, Some(cancel))?;
            flatpak
        } else {
            PathBuf::new()
        };
        if cancel.load(Ordering::Acquire) || !current() {
            return Err("게임 세션 시작을 취소했습니다.".into());
        }
        let app = env::current_exe().map_err(|error| error.to_string())?;
        let gamescope = gamescope_path(&app).ok_or("백그라운드 게임 실행 구성 요소 Gamescope가 없습니다. 앱에서 실행 구성 요소를 준비한 뒤 다시 시작하세요.")?;
        let xwayland =
            executable("Xwayland").ok_or("백그라운드 게임에 필요한 Xwayland를 찾지 못했습니다.")?;
        let directory = tempfile::Builder::new()
            .prefix("acrux-private-")
            .tempdir()
            .map_err(|error| error.to_string())?;
        fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))
            .map_err(|error| error.to_string())?;
        let config = Config {
            token: uuid::Uuid::new_v4().simple().to_string(),
            cookie: *uuid::Uuid::new_v4().as_bytes(),
            owner: Identity::read(std::process::id())?,
            host_display: env::var("DISPLAY")
                .ok()
                .and_then(|display| display_number(&display).ok()),
            flatpak,
            xwayland,
            gamescope: gamescope.clone(),
            mode,
            url,
        };
        write_json(&directory.path().join("config.json"), &config)?;
        let wrapper = directory.path().join("Xwayland");
        symlink(&app, &wrapper).map_err(|error| error.to_string())?;
        let log = private_file(&directory.path().join("session.log"))?;
        let mut command = Command::new(&app);
        command
            .arg(SUPERVISOR)
            .arg(directory.path())
            .arg(&config.token)
            .env(DIRECTORY, directory.path())
            .env(TOKEN, &config.token)
            .env("WLR_XWAYLAND", &wrapper)
            .env("XAUTHORITY", directory.path().join("Xauthority"))
            .env_remove("DISPLAY")
            .env_remove("WAYLAND_DISPLAY")
            .env_remove("WAYLAND_SOCKET")
            .env_remove("HYPRLAND_INSTANCE_SIGNATURE")
            .stdin(Stdio::null())
            .stdout(log.try_clone().map_err(|error| error.to_string())?)
            .stderr(log)
            .process_group(0);
        if let Some(runtime) = gamescope.parent().and_then(Path::parent).filter(|runtime| {
            app.parent()
                .is_some_and(|parent| *runtime == parent.join("runtime/usr"))
        }) {
            let mut paths = vec![runtime.join("bin")];
            if let Some(existing) = env::var_os("PATH") {
                paths.extend(env::split_paths(&existing));
            }
            command.env(
                "PATH",
                env::join_paths(paths).map_err(|error| error.to_string())?,
            );
            let libraries = runtime.join("lib");
            if libraries.is_dir() {
                let mut paths = vec![libraries];
                if let Some(existing) = env::var_os("LD_LIBRARY_PATH") {
                    paths.extend(env::split_paths(&existing));
                }
                command.env(
                    "LD_LIBRARY_PATH",
                    env::join_paths(paths).map_err(|error| error.to_string())?,
                );
            }
            let share = runtime.join("share");
            if share.is_dir() {
                let mut paths = vec![share.clone()];
                paths.extend(env::split_paths(
                    &env::var_os("XDG_DATA_DIRS")
                        .unwrap_or_else(|| OsString::from("/usr/local/share:/usr/share")),
                ));
                command.env(
                    "XDG_DATA_DIRS",
                    env::join_paths(paths).map_err(|error| error.to_string())?,
                );
                let scripts = share.join("gamescope/scripts");
                if scripts.is_dir() {
                    command.env("GAMESCOPE_SCRIPT_PATH", scripts);
                }
            }
        }
        if cancel.load(Ordering::Acquire) || !current() {
            return Err("게임 세션 시작을 취소했습니다.".into());
        }
        let mut child = command
            .spawn()
            .map_err(|error| format!("백그라운드 게임 세션을 시작하지 못했습니다. {error}"))?;
        let process = match Identity::read(child.id()) {
            Ok(process) => process,
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(error);
            }
        };
        let mut session = Self {
            directory,
            child,
            process,
            config,
            endpoint: None,
            stopped: false,
        };
        session.wait_ready(deadline, cancel, current)?;
        Ok(session)
    }

    fn wait_ready(
        &mut self,
        deadline: Instant,
        cancel: &AtomicBool,
        current: impl Fn() -> bool,
    ) -> Result<(), String> {
        loop {
            if cancel.load(Ordering::Acquire) || !current() {
                return Err("게임 세션 시작을 취소했습니다.".into());
            }
            if let Ok(error) = read_small(&self.directory.path().join("error.txt"), 4096) {
                return Err(redact(&error, &self.config).chars().take(2048).collect());
            }
            if self
                .child
                .try_wait()
                .map_err(|error| error.to_string())?
                .is_some()
            {
                return Err(self.startup_error("백그라운드 게임 구성 요소가 시작 중 종료됐습니다."));
            }
            if let (Ok(ready), Ok(server)) = (
                read_json::<Handshake>(&self.directory.path().join("ready.json")),
                read_json::<Xserver>(&self.directory.path().join("xserver.json")),
            ) {
                self.verify(&ready, &server)?;
                self.endpoint = Some((ready, server));
                self.connect_x11()?;
                return Ok(());
            }
            if Instant::now() >= deadline {
                return Err(self.startup_error("백그라운드 게임 세션의 준비 시간이 초과됐습니다."));
            }
            thread::sleep(POLL);
        }
    }

    pub(super) fn startup_error(&self, message: &str) -> String {
        let diagnostics =
            runtime_diagnostics(&self.directory.path().join("session.log"), &self.config);
        if diagnostics.is_empty() {
            message.to_owned()
        } else {
            format!("{message}\n{diagnostics}")
        }
    }

    fn verify(&self, ready: &Handshake, server: &Xserver) -> Result<(), String> {
        if ready.token != self.config.token
            || server.token != self.config.token
            || ready.display != server.display
            || self.config.host_display == Some(ready.display)
            || ready.process.group != self.process.pid
            || server.process.group != self.process.pid
            || !ready.process.alive()
            || !server.process.alive()
            || match self.config.mode {
                Mode::Sober => !ready.instance.as_deref().is_some_and(valid_instance),
                Mode::Fixture => ready.instance.is_some(),
            }
        {
            return Err("앱이 만든 전용 게임 디스플레이를 확인하지 못했습니다.".into());
        }
        Ok(())
    }

    pub(super) fn connect_x11(&self) -> Result<(RustConnection, usize), String> {
        let (ready, server) = self
            .endpoint
            .as_ref()
            .ok_or("전용 게임 디스플레이가 아직 준비되지 않았습니다.")?;
        self.verify(ready, server)?;
        let socket = UnixStream::connect(format!("/tmp/.X11-unix/X{}", ready.display))
            .map_err(|_| "전용 게임 디스플레이에 연결하지 못했습니다.")?;
        let peer =
            rustix::net::sockopt::socket_peercred(&socket).map_err(|error| error.to_string())?;
        let pid = peer.pid.as_raw_pid() as u32;
        if peer.uid != rustix::process::getuid()
            || (pid != self.process.pid && pid != server.process.pid)
        {
            return Err("전용 게임 디스플레이의 소유자가 일치하지 않습니다.".into());
        }
        let connection = authenticated_connection(socket, self.config.cookie)?;
        Ok((connection, 0))
    }

    pub(super) fn healthy(&mut self) -> bool {
        !self.stopped
            && self.child.try_wait().is_ok_and(|status| status.is_none())
            && self
                .endpoint
                .as_ref()
                .is_some_and(|(ready, server)| self.verify(ready, server).is_ok())
    }

    /// Send a protocol URL only inside the token-bound Flatpak instance we own.
    /// Never invoke the host's default URL handler or start another Flatpak app.
    pub(super) fn launch_url(
        &mut self,
        url: &str,
        cancel: &AtomicBool,
        current: impl Fn() -> bool,
    ) -> Result<(), String> {
        if !url.starts_with("roblox://")
            || url.len() > 8192
            || url.chars().any(char::is_control)
            || self.config.mode != Mode::Sober
            || !self.healthy()
            || !current()
        {
            return Err("재접속 주소나 소유 게임 세션이 올바르지 않습니다.".into());
        }
        let (ready, _) = self.endpoint.as_ref().ok_or("소유 게임 세션이 없습니다.")?;
        let instance = ready
            .instance
            .as_deref()
            .filter(|id| valid_instance(id))
            .ok_or("소유 Flatpak 인스턴스가 없습니다.")?;
        let mut command = Command::new(&self.config.flatpak);
        command
            .args([
                "enter",
                instance,
                "/usr/bin/env",
                "-u",
                "WAYLAND_DISPLAY",
                "-u",
                "WAYLAND_SOCKET",
            ])
            .arg(format!("DISPLAY=:{}", ready.display))
            .arg(format!(
                "XAUTHORITY={}",
                self.directory.path().join("Xauthority").display()
            ))
            .args([
                "SDL_VIDEO_DRIVER=x11",
                "SDL_VIDEODRIVER=x11",
                "/app/bin/sober",
                "--",
                url,
            ])
            .env_remove("DISPLAY")
            .env_remove("WAYLAND_DISPLAY")
            .env_remove("WAYLAND_SOCKET")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let mut child = command
            .spawn()
            .map_err(|e| format!("소유 게임에 재접속 주소를 전달하지 못했습니다: {e}"))?;
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            if cancel.load(Ordering::Acquire)
                || !current()
                || !self.healthy()
                || Instant::now() >= deadline
            {
                let _ = child.kill();
                let _ = child.wait();
                return Err("재접속 주소 전달을 취소했거나 시간이 초과됐습니다.".into());
            }
            match child.try_wait() {
                Ok(Some(status)) if status.success() => return Ok(()),
                Ok(Some(_)) => return Err("소유 게임이 재접속 주소를 받지 못했습니다.".into()),
                Ok(None) => thread::sleep(POLL),
                Err(error) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(error.to_string());
                }
            }
        }
    }

    pub(super) fn stop_handle(&self) -> StopHandle {
        let mut members = self
            .endpoint
            .as_ref()
            .map(|(ready, server)| vec![ready.process, server.process])
            .unwrap_or_default();
        // Startup can fail before the full handshake. Keep identities written
        // by the authenticated bootstrap so its children are still cleaned up
        // if the Gamescope group leader has already exited.
        if let Ok(server) = read_json::<Xserver>(&self.directory.path().join("xserver.json")) {
            if server.token == self.config.token && server.process.group == self.process.pid {
                members.push(server.process);
            }
        }
        if let Ok(ready) = read_json::<Handshake>(&self.directory.path().join("ready.json")) {
            if ready.token == self.config.token && ready.process.group == self.process.pid {
                members.push(ready.process);
            }
        }
        StopHandle {
            directory: self.directory.path().to_owned(),
            process: self.process,
            members,
            token: self.config.token.clone(),
            flatpak: self.config.flatpak.clone(),
            requested: Arc::new(AtomicBool::new(false)),
        }
    }

    pub(super) fn stop(&mut self) -> Result<(), String> {
        if self.stopped {
            return Ok(());
        }
        self.stopped = true;
        let handle = self.stop_handle();
        let _ = fs::write(self.directory.path().join("stop"), b"stop");
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            match self.child.try_wait() {
                Ok(Some(_)) | Err(_) => break,
                Ok(None) => {}
            }
            if Instant::now() >= deadline {
                break;
            }
            thread::sleep(POLL);
        }
        handle.stop()?;
        let _ = self.child.kill();
        let _ = self.child.wait();
        Ok(())
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

/// Called by main before Tauri creates a window or starts its runtime.
pub(crate) fn bootstrap() -> Option<i32> {
    let arguments: Vec<OsString> = env::args_os().collect();
    let is_helper = arguments.get(1).is_some_and(|argument| argument == HELPER);
    let is_supervisor = arguments
        .get(1)
        .is_some_and(|argument| argument == SUPERVISOR);
    let is_xwayland = env::var_os(DIRECTORY).is_some()
        && arguments
            .get(1)
            .and_then(|argument| argument.to_str())
            .is_some_and(|argument| display_number(argument).is_ok())
        && arguments.iter().any(|argument| argument == "-rootless");
    if !is_helper && !is_supervisor && !is_xwayland {
        return None;
    }
    let result = if is_helper || is_supervisor {
        arguments
            .get(2)
            .zip(arguments.get(3))
            .ok_or_else(|| "전용 게임 실행 정보가 없습니다.".to_owned())
            .and_then(|(directory, token)| {
                if is_supervisor {
                    run_supervisor(Path::new(directory), token)
                } else {
                    run_helper(Path::new(directory), token)
                }
            })
    } else {
        run_xwayland(&arguments[1..])
    };
    Some(match result {
        Ok(code) => code,
        Err(error) => {
            // Do not print inherited display credentials or process arguments.
            if let Some(directory) = env::var_os(DIRECTORY) {
                let path = PathBuf::from(directory);
                if private_directory(&path).is_ok() {
                    let _ = fs::write(path.join("error.txt"), error.as_bytes());
                }
            }
            1
        }
    })
}

fn load_config(directory: &Path, token: &OsStr) -> Result<Config, String> {
    private_directory(directory)?;
    let config: Config = read_json(&directory.join("config.json"))?;
    if token.to_str() != Some(config.token.as_str())
        || config.token.len() != 32
        || !config.owner.alive()
        || (config.mode == Mode::Fixture && env::var("ACRUX_SESSION_TEST").as_deref() != Ok("1"))
    {
        return Err("전용 게임 실행 권한이 만료됐습니다.".into());
    }
    Ok(config)
}

fn run_supervisor(directory: &Path, token: &OsStr) -> Result<i32, String> {
    let config = load_config(directory, token)?;
    rustix::process::setrlimit(
        rustix::process::Resource::Core,
        rustix::process::Rlimit {
            current: Some(0),
            maximum: Some(0),
        },
    )
    .map_err(|error| error.to_string())?;
    rustix::process::set_parent_process_death_signal(Some(rustix::process::Signal::TERM))
        .map_err(|error| error.to_string())?;
    if rustix::process::getppid().map(|pid| pid.as_raw_pid() as u32) != Some(config.owner.pid)
        || !config.owner.alive()
    {
        return Err("게임을 시작한 앱이 종료됐습니다.".into());
    }
    let app = env::current_exe().map_err(|error| error.to_string())?;
    let error = Command::new(config.gamescope)
        .args(["--backend", "headless", "-w", "1280", "-h", "720", "--"])
        .arg(app)
        .arg(HELPER)
        .arg(directory)
        .arg(token)
        .exec();
    Err(format!("Gamescope를 실행하지 못했습니다. {error}"))
}

fn run_xwayland(arguments: &[OsString]) -> Result<i32, String> {
    let directory = PathBuf::from(env::var_os(DIRECTORY).ok_or("전용 실행 경로가 없습니다.")?);
    let config = load_config(
        &directory,
        &env::var_os(TOKEN).ok_or("전용 실행 권한이 없습니다.")?,
    )?;
    let display = display_number(
        arguments
            .first()
            .and_then(|s| s.to_str())
            .ok_or("전용 디스플레이 번호가 없습니다.")?,
    )?;
    if config.host_display == Some(display)
        || arguments
            .iter()
            .any(|argument| argument == "-auth" || argument == "-ac")
    {
        return Err("기존 사용자 디스플레이로 연결할 수 없습니다.".into());
    }
    let authority = directory.join("Xauthority");
    write_private(&authority, &authority_record(display, config.cookie))?;
    let process = Identity::read(std::process::id())?;
    write_json(
        &directory.join("xserver.json"),
        &Xserver {
            token: config.token,
            display,
            process,
        },
    )?;
    let error = Command::new(config.xwayland)
        .args(arguments)
        .arg("-auth")
        .arg(authority)
        .env_remove(DIRECTORY)
        .env_remove(TOKEN)
        .exec();
    Err(format!("전용 Xwayland를 실행하지 못했습니다. {error}"))
}

fn run_helper(directory: &Path, token: &OsStr) -> Result<i32, String> {
    let config = load_config(directory, token)?;
    let display = display_number(
        &env::var("DISPLAY").map_err(|_| "Gamescope가 전용 디스플레이를 제공하지 않았습니다.")?,
    )?;
    let server: Xserver = read_json(&directory.join("xserver.json"))?;
    let process = Identity::read(std::process::id())?;
    if server.token != config.token
        || server.display != display
        || config.host_display == Some(display)
        || server.process.group != process.group
        || !server.process.alive()
    {
        return Err("Gamescope 전용 디스플레이를 검증하지 못했습니다.".into());
    }
    let compositor = Identity::read(process.group)?;
    if config.mode == Mode::Fixture {
        write_json(
            &directory.join("ready.json"),
            &Handshake {
                token: config.token,
                display,
                process,
                instance: None,
            },
        )?;
        let deadline = Instant::now() + Duration::from_secs(10);
        while !directory.join("stop").exists()
            && config.owner.alive()
            && compositor.alive()
            && Instant::now() < deadline
        {
            thread::sleep(POLL);
        }
        return Ok(0);
    }
    ensure_no_sober(&config.flatpak, None)?;
    let instance_file = directory.join("instance.txt");
    let output = private_file(&instance_file)?;
    let instance_fd = output.as_raw_fd();
    if instance_fd <= 2 {
        return Err("전용 게임의 실행 정보 통로를 만들지 못했습니다.".into());
    }
    let log = OpenOptions::new()
        .append(true)
        .open(directory.join("session.log"))
        .map_err(|error| error.to_string())?;
    let mut command = Command::new(&config.flatpak);
    command
        .args([
            "run",
            "--die-with-parent",
            "--socket=x11",
            "--nosocket=wayland",
            "--nosocket=fallback-x11",
            "--unset-env=WAYLAND_DISPLAY",
            "--unset-env=WAYLAND_SOCKET",
            "--env=SDL_VIDEO_DRIVER=x11",
            "--env=SDL_VIDEODRIVER=x11",
        ])
        .arg(format!("--instance-id-fd={instance_fd}"))
        .arg(format!("--filesystem={}:ro", directory.display()))
        .arg(format!("--env=DISPLAY=:{display}"))
        .arg(format!(
            "--env=XAUTHORITY={}",
            directory.join("Xauthority").display()
        ))
        .arg(APP)
        .env("DISPLAY", format!(":{display}"))
        .env("XAUTHORITY", directory.join("Xauthority"))
        .env_remove("WAYLAND_DISPLAY")
        .env_remove("WAYLAND_SOCKET")
        .env_remove("HYPRLAND_INSTANCE_SIGNATURE")
        .env_remove(DIRECTORY)
        .env_remove(TOKEN)
        .env_remove("WLR_XWAYLAND")
        .stdin(Stdio::null())
        .stdout(log.try_clone().map_err(|error| error.to_string())?)
        .stderr(log);
    if let Some(url) = config.url.as_deref() {
        if !url.starts_with("roblox://") || url.len() > 8192 || url.chars().any(char::is_control) {
            return Err("소유 게임의 재실행 주소가 잘못됐습니다.".into());
        }
        command.args(["--", url]);
    }
    // This internal helper is single-threaded. Inherit only the dedicated ID
    // descriptor; neither Flatpak's stdout nor Sober's logs can write into it.
    rustix::io::fcntl_setfd(&output, rustix::io::FdFlags::empty())
        .map_err(|error| error.to_string())?;
    let spawned = command.spawn();
    drop(output);
    let mut child =
        spawned.map_err(|error| format!("전용 Sober를 실행하지 못했습니다. {error}"))?;
    let mut instance = None;
    let started = Instant::now();
    let mut outcome = Ok(0);
    loop {
        if instance.is_none() {
            if let Ok(value) = read_small(&instance_file, 128) {
                if let Some(id) = value.lines().next().filter(|id| valid_instance(id)) {
                    instance = Some(id.to_owned());
                    if let Err(error) = write_json(
                        &directory.join("ready.json"),
                        &Handshake {
                            token: config.token.clone(),
                            display,
                            process,
                            instance: Some(id.to_owned()),
                        },
                    ) {
                        outcome = Err(error);
                        break;
                    }
                }
            }
        }
        match child.try_wait() {
            Ok(Some(status)) => {
                outcome = Ok(status.code().unwrap_or(1));
                break;
            }
            Ok(None) => {}
            Err(error) => {
                outcome = Err(error.to_string());
                break;
            }
        }
        if directory.join("stop").exists() || !config.owner.alive() || !compositor.alive() {
            break;
        }
        if instance.is_none() && started.elapsed() >= Duration::from_secs(10) {
            outcome = Err("Sober 실행 인스턴스를 확인하지 못했습니다.".into());
            break;
        }
        thread::sleep(POLL);
    }
    if let Some(instance) = instance {
        kill_instance(&config.flatpak, &instance);
    }
    let _ = child.kill();
    let _ = child.wait();
    outcome
}

fn authenticated_connection(
    mut socket: UnixStream,
    cookie: [u8; 16],
) -> Result<RustConnection, String> {
    let deadline = Instant::now() + Duration::from_secs(1);
    socket
        .set_write_timeout(Some(Duration::from_millis(250)))
        .map_err(|error| error.to_string())?;
    let request = SetupRequest {
        byte_order: if cfg!(target_endian = "little") {
            b'l'
        } else {
            b'B'
        },
        protocol_major_version: 11,
        protocol_minor_version: 0,
        authorization_protocol_name: MIT_COOKIE.to_vec(),
        authorization_protocol_data: cookie.to_vec(),
    };
    socket
        .write_all(&request.serialize())
        .map_err(|_| "전용 디스플레이 인증 요청에 실패했습니다.")?;
    let mut header = [0; 8];
    read_deadline(&mut socket, &mut header, deadline)?;
    if header[0] != 1 {
        return Err("전용 디스플레이 인증이 거부됐습니다.".into());
    }
    let length = usize::from(u16::from_ne_bytes([header[6], header[7]])) * 4;
    let mut reply = vec![0; length + 8];
    reply[..8].copy_from_slice(&header);
    read_deadline(&mut socket, &mut reply[8..], deadline)?;
    let (setup, _) =
        Setup::try_parse(&reply).map_err(|_| "전용 디스플레이 응답이 올바르지 않습니다.")?;
    if setup.roots.is_empty() {
        return Err("전용 디스플레이에 화면이 없습니다.".into());
    }
    socket
        .set_read_timeout(None)
        .map_err(|error| error.to_string())?;
    socket
        .set_write_timeout(None)
        .map_err(|error| error.to_string())?;
    let (stream, _) = DefaultStream::from_unix_stream(socket).map_err(|error| error.to_string())?;
    RustConnection::for_connected_stream(stream, setup).map_err(|error| error.to_string())
}

fn read_deadline(
    socket: &mut UnixStream,
    mut bytes: &mut [u8],
    deadline: Instant,
) -> Result<(), String> {
    while !bytes.is_empty() {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err("전용 디스플레이 응답 시간이 초과됐습니다.".into());
        }
        socket
            .set_read_timeout(Some(remaining.min(Duration::from_millis(50))))
            .map_err(|error| error.to_string())?;
        match socket.read(bytes) {
            Ok(0) => return Err("전용 디스플레이 연결이 종료됐습니다.".into()),
            Ok(length) => bytes = &mut bytes[length..],
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::WouldBlock
                        | std::io::ErrorKind::TimedOut
                        | std::io::ErrorKind::Interrupted
                ) => {}
            Err(_) => return Err("전용 디스플레이를 읽지 못했습니다.".into()),
        }
    }
    Ok(())
}

fn display_number(display: &str) -> Result<u16, String> {
    let number = display
        .strip_prefix(':')
        .ok_or("로컬 전용 디스플레이가 아닙니다.")?;
    let number = number.strip_suffix(".0").unwrap_or(number);
    if number.is_empty() || !number.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err("전용 디스플레이 번호가 올바르지 않습니다.".into());
    }
    number
        .parse()
        .map_err(|_| "전용 디스플레이 번호가 너무 큽니다.".into())
}

fn valid_instance(instance: &str) -> bool {
    !instance.is_empty()
        && instance.len() <= 20
        && instance.bytes().all(|byte| byte.is_ascii_digit())
}

fn ensure_no_sober(flatpak: &Path, cancel: Option<&AtomicBool>) -> Result<(), String> {
    let output = run_output(
        Command::new(flatpak).args(["ps", "--columns=application,instance"]),
        Duration::from_secs(2),
        cancel,
    )?;
    if String::from_utf8_lossy(&output)
        .lines()
        .any(|line| line.split_whitespace().next() == Some(APP))
    {
        return Err("기존 Sober가 실행 중입니다. 현재 게임을 종료한 뒤 앱의 백그라운드 게임 시작을 다시 눌러 주세요. 기존 게임은 자동으로 종료하지 않습니다.".into());
    }
    Ok(())
}

fn run_output(
    command: &mut Command,
    timeout: Duration,
    cancel: Option<&AtomicBool>,
) -> Result<Vec<u8>, String> {
    if cancel.is_some_and(|cancel| cancel.load(Ordering::Acquire)) {
        return Err("게임 세션 시작을 취소했습니다.".into());
    }
    let mut output = tempfile::tempfile().map_err(|error| error.to_string())?;
    let mut child = command
        .stdin(Stdio::null())
        .stdout(output.try_clone().map_err(|error| error.to_string())?)
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| error.to_string())?;
    let deadline = Instant::now() + timeout;
    loop {
        let status = match child.try_wait() {
            Ok(status) => status,
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(error.to_string());
            }
        };
        if let Some(status) = status {
            if !status.success() {
                return Err("게임 실행 정보를 확인하지 못했습니다.".into());
            }
            use std::io::{Seek, SeekFrom};
            output
                .seek(SeekFrom::Start(0))
                .map_err(|error| error.to_string())?;
            let mut data = Vec::new();
            output
                .take(1024 * 1024)
                .read_to_end(&mut data)
                .map_err(|error| error.to_string())?;
            return Ok(data);
        }
        if cancel.is_some_and(|cancel| cancel.load(Ordering::Acquire)) {
            let _ = child.kill();
            let _ = child.wait();
            return Err("게임 세션 시작을 취소했습니다.".into());
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err("게임 실행 정보 확인 시간이 초과됐습니다.".into());
        }
        thread::sleep(POLL);
    }
}

fn kill_instance(flatpak: &Path, instance: &str) {
    if valid_instance(instance) {
        let _ = run_output(
            Command::new(flatpak).args(["kill", instance]),
            Duration::from_millis(500),
            None,
        );
    }
}

fn executable(name: &str) -> Option<PathBuf> {
    env::split_paths(&env::var_os("PATH")?)
        .filter(|path| path.is_absolute())
        .map(|path| path.join(name))
        .find(|path| executable_file(path))
}

fn executable_file(path: &Path) -> bool {
    fs::metadata(path)
        .is_ok_and(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
}

fn gamescope_path(app: &Path) -> Option<PathBuf> {
    executable("gamescope").or_else(|| {
        let path = app.parent()?.join("runtime/usr/bin/gamescope");
        executable_file(&path).then_some(path)
    })
}

fn private_directory(directory: &Path) -> Result<(), String> {
    let metadata = fs::symlink_metadata(directory).map_err(|error| error.to_string())?;
    if !metadata.is_dir()
        || metadata.uid() != rustix::process::getuid().as_raw()
        || metadata.permissions().mode() & 0o077 != 0
    {
        return Err("전용 게임 연결 경로의 접근 권한이 올바르지 않습니다.".into());
    }
    Ok(())
}

fn private_file(path: &Path) -> Result<File, String> {
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .map_err(|error| error.to_string())
}

fn write_private(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let temporary = path.with_extension("tmp");
    let mut file = private_file(&temporary)?;
    file.write_all(bytes).map_err(|error| error.to_string())?;
    fs::rename(temporary, path).map_err(|error| error.to_string())
}

fn write_json(path: &Path, value: &impl Serialize) -> Result<(), String> {
    write_private(
        path,
        &serde_json::to_vec(value).map_err(|error| error.to_string())?,
    )
}

fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, String> {
    let metadata = fs::symlink_metadata(path).map_err(|error| error.to_string())?;
    if !metadata.is_file()
        || metadata.len() > 16384
        || metadata.uid() != rustix::process::getuid().as_raw()
        || metadata.permissions().mode() & 0o077 != 0
    {
        return Err("전용 게임 연결 파일이 올바르지 않습니다.".into());
    }
    serde_json::from_slice(&fs::read(path).map_err(|error| error.to_string())?)
        .map_err(|_| "전용 게임 연결 파일을 읽지 못했습니다.".into())
}

fn read_small(path: &Path, limit: u64) -> Result<String, String> {
    let file = File::open(path).map_err(|error| error.to_string())?;
    let mut value = String::new();
    file.take(limit)
        .read_to_string(&mut value)
        .map_err(|error| error.to_string())?;
    Ok(value)
}

fn redact(value: &str, config: &Config) -> String {
    let hex: String = config
        .cookie
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    let mut value = value.replace(&config.token, "[redacted]");
    if let Some(url) = config.url.as_deref().filter(|url| !url.is_empty()) {
        value = value.replace(url, "[redacted]");
        let credentials = regex::Regex::new(r"(?i)(?:[?&]|://)(?:linkCode|privateServerLinkCode|privateServerLink|code)=([A-Za-z0-9_-]+)").unwrap();
        for captures in credentials.captures_iter(url) {
            value = value.replace(&captures[1], "[redacted]");
        }
    }
    for secret in [
        hex.clone(),
        hex.to_uppercase(),
        uuid::Uuid::from_bytes(config.cookie).to_string(),
        serde_json::to_string(&config.cookie).unwrap_or_default(),
        format!("{:?}", config.cookie),
    ] {
        if !secret.is_empty() {
            value = value.replace(&secret, "[redacted]");
        }
    }
    value
}

fn runtime_diagnostics(path: &Path, config: &Config) -> String {
    let Ok(mut file) = File::open(path) else {
        return String::new();
    };
    let Ok(length) = file.metadata().map(|metadata| metadata.len()) else {
        return String::new();
    };
    let offset = length.saturating_sub(16 * 1024);
    if file.seek(SeekFrom::Start(offset)).is_err() {
        return String::new();
    }
    let mut bytes = Vec::new();
    if file.take(16 * 1024).read_to_end(&mut bytes).is_err() {
        return String::new();
    }
    let text = redact(&String::from_utf8_lossy(&bytes), config);
    let text = if offset > 0 {
        text.split_once('\n').map_or("", |(_, rest)| rest)
    } else {
        &text
    };
    let mut lines: Vec<_> = text
        .lines()
        .map(clean_log_line)
        .filter(|line| {
            let lower = line.to_ascii_lowercase();
            ![
                "cookie",
                "token",
                "password",
                "credential",
                "authorization",
                "xauthority",
                "config.json",
                "--acrux-private",
                "acrux_private_",
                "argv",
                "command line",
                "arguments:",
            ]
            .iter()
            .any(|secret| lower.contains(secret))
                && [
                    "error",
                    "failed",
                    "failure",
                    "fatal",
                    "could not",
                    "couldn't",
                    "cannot",
                    "no such file",
                    "not found",
                    "permission denied",
                    "undefined symbol",
                    "assertion",
                    "segmentation",
                    "primary child shut down",
                ]
                .iter()
                .any(|keyword| lower.contains(keyword))
        })
        .map(|line| line.chars().take(240).collect::<String>())
        .rev()
        .take(6)
        .collect();
    lines.reverse();
    lines.join("\n")
}

fn clean_log_line(line: &str) -> String {
    let mut output = String::new();
    let (mut escape, mut csi) = (false, false);
    for character in line.chars() {
        if escape {
            escape = false;
            csi = character == '[';
        } else if csi {
            if ('@'..='~').contains(&character) {
                csi = false;
            }
        } else if character == '\u{1b}' {
            escape = true;
        } else if !character.is_control() || character == '\t' {
            output.push(character);
        }
    }
    output
}

fn authority_record(display: u16, cookie: [u8; 16]) -> Vec<u8> {
    let mut data = u16::MAX.to_be_bytes().to_vec(); // FamilyWild, private local display only.
    for field in [
        b"".as_slice(),
        display.to_string().as_bytes(),
        MIT_COOKIE,
        &cookie,
    ] {
        data.extend_from_slice(&(field.len() as u16).to_be_bytes());
        data.extend_from_slice(field);
    }
    data
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(owner: Identity) -> Config {
        Config {
            token: "12345678901234567890123456789012".into(),
            cookie: [7; 16],
            owner,
            host_display: Some(1),
            flatpak: "/usr/bin/flatpak".into(),
            xwayland: "/usr/bin/Xwayland".into(),
            gamescope: "/usr/bin/gamescope".into(),
            mode: Mode::Sober,
            url: None,
        }
    }

    #[test]
    fn recovery_link_and_its_server_code_never_reach_diagnostics() {
        let mut cfg = config(Identity::read(std::process::id()).unwrap());
        cfg.url =
            Some("roblox://navigation/share_links?code=private-server-code&type=Server".into());
        let diagnostic = redact(
            &format!(
                "launch failed: {}; server code: private-server-code",
                cfg.url.as_ref().unwrap()
            ),
            &cfg,
        );
        assert!(!diagnostic.contains("private-server-code"));
        assert!(!diagnostic.contains("roblox://"));
    }

    #[test]
    fn canceled_recovery_generation_cannot_start_a_private_session() {
        let cancel = AtomicBool::new(false);
        let result = Session::launch_url_with_cancel("roblox://placeId=1", &cancel, || false);
        assert!(matches!(result, Err(error) if error == "게임 세션 시작을 취소했습니다."));
        cancel.store(true, Ordering::Release);
        let result = Session::launch_url_with_cancel("roblox://placeId=1", &cancel, || true);
        assert!(matches!(result, Err(error) if error == "게임 세션 시작을 취소했습니다."));
        assert!(Session::launch_url_with_cancel(
            "https://example.com",
            &AtomicBool::new(false),
            || true
        )
        .is_err());
    }

    #[test]
    fn only_local_display_and_exact_instance_ids_are_accepted() {
        assert_eq!(display_number(":42").unwrap(), 42);
        assert_eq!(display_number(":42.0").unwrap(), 42);
        for display in ["localhost:42", "host:1", ":1.1", ":", ":-1", ":65536"] {
            assert!(display_number(display).is_err());
        }
        assert!(valid_instance("123456789"));
        for instance in ["", APP, "--all", "12 34", "1;kill", "123456789012345678901"] {
            assert!(!valid_instance(instance));
        }
    }

    #[test]
    fn authority_file_is_private_and_contains_the_allocated_display() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("Xauthority");
        let bytes = authority_record(42, [7; 16]);
        write_private(&path, &bytes).unwrap();
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(&bytes[..2], &[255, 255]);
        assert!(bytes.windows(2).any(|value| value == b"42"));
        assert!(bytes.ends_with(&[7; 16]));
        assert_eq!(fs::read(&path).unwrap(), bytes);
    }

    #[test]
    fn identity_detects_process_reuse_and_config_requires_matching_token() {
        let directory = tempfile::tempdir().unwrap();
        // Match the production directory: tempfile's defaults follow its mode
        // policy, whereas the private bootstrap deliberately requires 0700.
        fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700)).unwrap();
        let owner = Identity::read(std::process::id()).unwrap();
        assert!(owner.alive());
        assert!(!Identity {
            started: owner.started.wrapping_add(1),
            ..owner
        }
        .alive());
        let config = config(owner);
        write_json(&directory.path().join("config.json"), &config).unwrap();
        load_config(directory.path(), OsStr::new(&config.token)).unwrap();
        assert!(load_config(directory.path(), OsStr::new("other")).is_err());
        fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o755)).unwrap();
        assert!(load_config(directory.path(), OsStr::new(&config.token)).is_err());
    }

    #[test]
    fn startup_diagnostics_keep_actionable_failures_without_credentials_or_arguments() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("session.log");
        let config = config(Identity::read(std::process::id()).unwrap());
        let cookie = "07".repeat(16);
        fs::write(
            &path,
            format!(
                "{}\n[Error] --acrux-private-supervisor /tmp/private {}\n[Error] cookie {}\n\u{1b}[31m[Error]\u{1b}[0m process: Failed to start process gamescopereaper: No such file or directory\n[Error] authentication rejected {} {}\n",
                "old log\n".repeat(4096), config.token, cookie, config.token, cookie
            ),
        )
        .unwrap();
        let diagnostics = runtime_diagnostics(&path, &config);
        assert!(diagnostics.contains("gamescopereaper: No such file or directory"));
        assert!(diagnostics.contains("[redacted]"));
        assert!(!diagnostics.contains(&config.token));
        assert!(!diagnostics.contains(&cookie));
        assert!(!diagnostics.contains("--acrux-private"));
        assert!(!diagnostics.contains('\u{1b}'));
        assert!(diagnostics.len() <= 6 * 240 + 6);
    }

    #[test]
    fn stale_stop_handles_are_safe_and_idempotent() {
        let directory = tempfile::tempdir().unwrap();
        let handle = StopHandle {
            directory: directory.path().to_owned(),
            process: Identity {
                pid: u32::MAX,
                started: 1,
                group: u32::MAX,
            },
            members: Vec::new(),
            token: "unused".into(),
            flatpak: PathBuf::new(),
            requested: Arc::new(AtomicBool::new(false)),
        };
        handle.stop().unwrap();
        handle.clone().stop().unwrap();
        assert!(!directory.path().join("stop").exists());
    }

    #[test]
    fn trusted_instance_cleanup_does_not_require_a_live_group() {
        let directory = tempfile::tempdir().unwrap();
        fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700)).unwrap();
        let process = Identity {
            pid: u32::MAX,
            started: 1,
            group: u32::MAX,
        };
        let token = "private-instance-token".to_owned();
        write_json(
            &directory.path().join("ready.json"),
            &Handshake {
                token: token.clone(),
                display: 42,
                process,
                instance: Some("123456789".into()),
            },
        )
        .unwrap();
        let handle = StopHandle {
            directory: directory.path().to_owned(),
            process,
            members: Vec::new(),
            token,
            // A harmless stand-in: this test never calls Flatpak or a game.
            flatpak: PathBuf::from("/bin/true"),
            requested: Arc::new(AtomicBool::new(false)),
        };
        assert!(!handle.owns_group());
        handle.stop().unwrap();
        assert!(directory.path().join("stop").exists());
        handle.stop().unwrap();
    }

    #[test]
    fn cancelled_preflight_does_not_spawn_a_process() {
        let error = run_output(
            &mut Command::new("/not-an-executable-acrux-test"),
            Duration::from_secs(2),
            Some(&AtomicBool::new(true)),
        )
        .unwrap_err();
        assert!(error.contains("취소"));
    }

    #[test]
    fn private_x11_handshake_read_is_bounded_without_a_server_reply() {
        let (mut client, _server) = UnixStream::pair().unwrap();
        let started = Instant::now();
        assert!(read_deadline(
            &mut client,
            &mut [0; 8],
            started + Duration::from_millis(50)
        )
        .is_err());
        assert!(started.elapsed() < Duration::from_millis(250));
    }
}

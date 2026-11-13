#[cfg(target_os = "android")]
mod android;
mod automation;
mod biome;
mod device;
mod discord;
mod fishing;
#[cfg(target_os = "linux")]
mod linux;
mod navigation;
mod navigator;
mod online;
pub mod pathfinding;
mod phone;
mod scene;
mod sdk;
mod services;
pub mod travel;
mod vision;
mod wayfinder;

#[cfg(target_os = "linux")]
use linux as desktop;
#[cfg(any(target_os = "windows", target_os = "macos"))]
mod desktop_native;
#[cfg(any(target_os = "windows", target_os = "macos"))]
use desktop_native as desktop;

use fishing::{Calibration, CalibrationPositions, Fishing, Phase, Stats, Tuning};
use navigation::{Job, Motion, Navigation, Options, Place, Task, Tick};
use serde::{Deserialize, Serialize};
use std::{
    collections::VecDeque,
    fs,
    path::PathBuf,
    sync::{LazyLock, Mutex, MutexGuard},
    time::Instant,
};
use tauri::Manager;

static STARTED: LazyLock<Instant> = LazyLock::new(Instant::now);
static RUNTIME: LazyLock<Mutex<Runtime>> = LazyLock::new(|| Mutex::new(Runtime::default()));

fn now() -> f64 {
    STARTED.elapsed().as_secs_f64()
}

fn runtime() -> MutexGuard<'static, Runtime> {
    RUNTIME
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[derive(Default)]
struct Runtime {
    fishing: Fishing,
    settings: Option<PathBuf>,
    settings_error: Option<String>,
    capture: bool,
    input: bool,
    totals: Stats,
    log: VecDeque<(i64, String)>,
    macro_since: Option<i64>,
    logged: Option<String>,
    navigation: Navigation,
    job: Option<Job>,
    empty_sales: u8,
    relocations: u8,
    measured_seconds: Option<f64>,
    generation: i32,
    dua: DuaStatus,
    resume_after_calibration: bool,
    phone: phone::State,
    device_action: Option<device::Action>,
    automation: Option<automation::Runner>,
    wayfinder: Option<wayfinder::Wayfinder>,
    macro_settings: serde_json::Value,
    macro_message: Option<String>,
    ocr_words: Vec<automation::Word>,
    request_ocr: bool,
    observed_biome: Option<(String, u64)>,
    wayfinding: wayfinder::Settings,
    game_observed: bool,
    game_disconnected: bool,
    last_automation: serde_json::Value,
    last_geometry: Option<(usize, usize)>,
    sdk_request: Option<sdk::Request>,
    fishing_settings_dirty: bool,
    target_missing: bool,
    frame_expires: Option<f64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
enum Route {
    Headless,
    Screenshot,
}

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
struct DuaStatus {
    supported: bool,
    running: bool,
    route: Option<Route>,
    reason: &'static str,
    headless_ticks: u64,
    screenshot_frames: u64,
    transitions: u64,
    background: bool,
    hotkeys: bool,
}

impl Default for DuaStatus {
    fn default() -> Self {
        Self {
            supported: cfg!(any(
                target_os = "linux",
                target_os = "windows",
                target_os = "macos",
                target_os = "android"
            )),
            running: false,
            route: None,
            reason: "DUA 대기 중",
            headless_ticks: 0,
            screenshot_frames: 0,
            transitions: 0,
            background: false,
            hotkeys: false,
        }
    }
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct BackgroundStatus {
    supported: bool,
    state: &'static str,
    message: String,
    width: usize,
    height: usize,
}
impl Default for BackgroundStatus {
    fn default() -> Self {
        Self {
            supported: cfg!(target_os = "linux"),
            state: if cfg!(target_os = "linux") {
                "stopped"
            } else {
                "unsupported"
            },
            message: if cfg!(target_os = "linux") {
                "백그라운드 게임을 열어 주세요."
            } else {
                "이 운영체제의 독립 입력 연결은 아직 준비되지 않았습니다."
            }
            .into(),
            width: 1280,
            height: 720,
        }
    }
}
fn background_status() -> BackgroundStatus {
    #[cfg(desktop)]
    return desktop::session_status();
    #[cfg(not(desktop))]
    BackgroundStatus::default()
}
pub fn session_helper() -> Option<i32> {
    #[cfg(target_os = "linux")]
    {
        if std::env::args().nth(1).as_deref() == Some("--acrux-private-game-check") {
            return Some(match linux::game_check() {
                Ok(()) => 0,
                Err(error) => {
                    eprintln!("{error}");
                    1
                }
            });
        }
        if std::env::args().nth(1).as_deref() == Some("--acrux-private-probe") {
            return Some(match linux::probe() {
                Ok(()) => 0,
                Err(error) => {
                    eprintln!("{error}");
                    1
                }
            });
        }
        return linux::session::bootstrap();
    }
    #[cfg(not(target_os = "linux"))]
    None
}

#[derive(Default, Deserialize, Serialize)]
#[serde(default)]
struct Profile {
    biome: biome::Settings,
    online: online::Settings,
    calibration: Option<Calibration>,
    tuning: Tuning,
    totals: Stats,
    navigation: Navigation,
    #[serde(rename = "discordLink")]
    discord_link: discord::Settings,
    macro_settings: serde_json::Value,
    wayfinding: wayfinder::Settings,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Status {
    platform: &'static str,
    version: &'static str,
    session: BackgroundStatus,
    capture_ready: bool,
    input_ready: bool,
    game_available: bool,
    calibrated: bool,
    positions: Option<CalibrationPositions>,
    phase: Phase,
    message: String,
    settings_error: Option<String>,
    stats: Stats,
    totals: Stats,
    tuning: Tuning,
    measuring_elapsed: Option<f64>,
    measured_seconds: Option<f64>,
    dua: DuaStatus,
    generation: i32,
    automation: serde_json::Value,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct NavigationStatus<'a> {
    options: Options,
    positions: &'a std::collections::BTreeMap<String, vision::Point>,
    fish: usize,
    sell: usize,
}

enum GameInput {
    Tap(fishing::Tap),
    Motion(Motion),
    Gesture(device::Gesture),
    Command(automation::Command),
}

enum DeviceTick {
    Input(GameInput),
    Wait,
    Observe(Option<vision::Region>),
}

impl Runtime {
    fn macro_config(&self) -> serde_json::Value {
        let mut config: serde_json::Value = if self.macro_settings.is_object() {
            self.macro_settings.clone()
        } else {
            let mut defaults: serde_json::Value =
                serde_json::from_str(include_str!("../../frontend/desktop-defaults.json"))
                    .expect("bundled macro defaults");
            defaults["useMove"] = true.into();
            defaults["useSell"] = true.into();
            defaults
        };
        for key in [
            "stick_center",
            "stick_edge",
            "jump_btn",
            "talk_btn",
            "chat_pos",
            "collection_pos",
            "collection_close",
            "dialog_pos",
        ] {
            if config["base"][key].is_null() {
                config["base"][key] = self
                    .navigation
                    .positions
                    .get(key)
                    .map(|p| serde_json::json!([p.0, p.1]))
                    .unwrap_or(serde_json::Value::Null);
            }
        }
        config
    }

    fn status(&self) -> Status {
        Status {
            platform: std::env::consts::OS,
            version: env!("CARGO_PKG_VERSION"),
            session: background_status(),
            capture_ready: self.capture,
            input_ready: self.input,
            game_available: if cfg!(target_os = "android") {
                self.phone.foreground
            } else {
                game_available()
            },
            calibrated: self.fishing.calibration.is_some(),
            positions: self.fishing.calibration.map(Calibration::positions),
            phase: self.phase(),
            message: self.message().to_owned(),
            settings_error: self.settings_error.clone(),
            stats: self.fishing.stats,
            totals: self.totals,
            tuning: self.fishing.tuning,
            measuring_elapsed: self.measuring_since().map(|start| now() - start),
            measured_seconds: self.measured_seconds,
            dua: self.dua,
            generation: self.generation,
            automation: self
                .automation
                .as_ref()
                .map(|runner| serde_json::to_value(runner.status()).unwrap_or_default())
                .unwrap_or_else(|| self.last_automation.clone()),
        }
    }

    fn load(&mut self, path: PathBuf) {
        match fs::read(&path) {
            Ok(bytes) => match serde_json::from_slice::<Profile>(&bytes) {
                Ok(profile) => {
                    biome::load(profile.biome);
                    online::load(profile.online);
                    discord::load(profile.discord_link);
                    self.fishing.calibration = profile.calibration;
                    self.totals = profile.totals;
                    self.navigation = profile.navigation;
                    self.macro_settings = profile.macro_settings;
                    self.wayfinding = profile.wayfinding;
                    self.navigation.fit_positions();
                    #[cfg(any(
                        target_os = "windows",
                        target_os = "linux",
                        target_os = "macos",
                        target_os = "android"
                    ))]
                    {
                        // Retain saved positions, but never resume retired timed
                        // travel when upgrading to the private game runtime.
                        // Old timed routes stay as data; the DUA reads the
                        // user's movement/sale choices from macro settings.
                    }
                    if let Err(error) = self.navigation.options.validate() {
                        self.navigation.options = Options::default();
                        self.settings_error = Some(error.into());
                    }
                    match profile.tuning.validate() {
                        Ok(tuning) => self.fishing.tuning = tuning,
                        Err(error) => self.settings_error = Some(error.into()),
                    }
                }
                Err(error) => {
                    self.settings_error = Some(format!("설정 파일을 읽을 수 없습니다. {error}"))
                }
            },
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                self.settings_error = Some(format!("설정 파일을 열 수 없습니다. {error}"))
            }
        }
        self.settings = Some(path);
    }

    fn save(&self, tuning: Tuning) -> Result<(), String> {
        self.save_biome_profile(tuning, biome::settings())
    }

    fn save_biome_profile(&self, tuning: Tuning, biome: biome::Settings) -> Result<(), String> {
        self.save_profile(tuning, biome, discord::settings())
    }

    fn save_profile(
        &self,
        tuning: Tuning,
        biome: biome::Settings,
        discord_link: discord::Settings,
    ) -> Result<(), String> {
        let path = self
            .settings
            .as_ref()
            .ok_or("설정 저장소가 준비되지 않았습니다.")?;
        let data = serde_json::to_vec_pretty(&Profile {
            biome,
            online: online::settings(),
            calibration: self.fishing.calibration,
            tuning,
            totals: self.totals,
            navigation: self.navigation.clone(),
            discord_link,
            macro_settings: self.macro_config(),
            wayfinding: self.wayfinding.clone(),
        })
        .map_err(|error| error.to_string())?;
        let temporary = path.with_extension("tmp");
        use std::io::Write;
        let mut options = fs::OpenOptions::new();
        options.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        options
            .open(&temporary)
            .and_then(|mut file| {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    file.set_permissions(fs::Permissions::from_mode(0o600))?;
                }
                file.write_all(&data)
            })
            .and_then(|_| fs::rename(&temporary, path))
            .map_err(|error| format!("설정을 저장하지 못했습니다. {error}"))
    }

    fn tick(
        &mut self,
        at: f64,
        geometry: (usize, usize),
        frame: Option<&vision::Frame<'_>>,
    ) -> DeviceTick {
        self.request_ocr = false;
        if frame.is_none() {
            self.frame_expires = None;
        }
        if !self.active() {
            return DeviceTick::Wait;
        }
        if geometry.0 == 0
            || geometry.1 == 0
            || frame.is_some_and(|f| (f.width, f.height) != geometry)
        {
            self.stop("게임 화면 크기를 확인할 수 없어 DUA를 중지했습니다.");
            return DeviceTick::Wait;
        }
        let resized = self.last_geometry != Some(geometry);
        self.target_missing = false;
        self.last_geometry = Some(geometry);
        if resized || self.fishing_settings_dirty {
            if let Err(error) = self
                .fishing
                .configure(&self.macro_config()["mfish"], geometry)
            {
                self.stop_automation(error);
                return DeviceTick::Wait;
            }
        }
        if self.fishing_settings_dirty {
            self.fishing_settings_dirty = false;
            match Calibration::manual(&self.macro_config()["mfish"], geometry) {
                Ok(calibration) => self.fishing.calibration = calibration,
                Err(error) => {
                    self.stop_automation(error);
                    return DeviceTick::Wait;
                }
            }
        }
        if let Some(step) = sdk::inspect(self, geometry, frame) {
            return step;
        }
        if let Some(action) = self.device_action.take() {
            let requested = match &action {
                device::Action::Queued(_, geometry) | device::Action::Pending(geometry) => {
                    *geometry
                }
            };
            if requested != geometry {
                self.stop("화면 크기가 바뀌어 기기 동작을 중지했습니다.");
                self.record();
                return DeviceTick::Wait;
            }
            self.device_action = Some(device::Action::Pending(requested));
            self.route(Route::Headless, "기기의 짧은 입력을 실행합니다.", false);
            return match action {
                device::Action::Queued(gesture, _) => {
                    DeviceTick::Input(GameInput::Gesture(gesture))
                }
                device::Action::Pending(_) => DeviceTick::Wait,
            };
        }
        if let Some(step) = self.automation_tick(at, geometry, frame) {
            return step;
        }
        if self.resume_after_calibration && self.fishing.phase == Phase::Idle {
            self.resume_after_calibration = false;
            if self.fishing.calibration.is_some() {
                if let Err(error) = self.fishing.start(at) {
                    self.stop(error);
                }
            } else {
                self.stop(self.fishing.message);
            }
            self.record();
        }
        if let Some(job) = self.job.as_mut() {
            let visual = frame.is_some();
            let tick = job.tick(at, geometry, frame);
            if matches!(tick, Tick::Observe) {
                self.route(
                    Route::Screenshot,
                    "판매 화면의 실제 상태를 확인합니다.",
                    false,
                );
                return DeviceTick::Observe(None);
            }
            self.route(
                if visual {
                    Route::Screenshot
                } else {
                    Route::Headless
                },
                if visual {
                    "판매 화면의 실제 상태를 확인합니다."
                } else {
                    "구조화된 이동 명령과 대기를 처리합니다."
                },
                visual,
            );
            let input = match tick {
                Tick::Input(motion) => DeviceTick::Input(GameInput::Motion(motion)),
                Tick::Wait => DeviceTick::Wait,
                Tick::Observe => unreachable!(),
                Tick::Done {
                    resume,
                    message,
                    sold,
                } => {
                    if let Some(count) = sold {
                        self.empty_sales = if count == 0 {
                            self.empty_sales.saturating_add(1)
                        } else {
                            0
                        };
                    }
                    if resume && self.empty_sales >= 2 {
                        self.stop(
                            "두 번 연속 팔 물고기가 없어 중지했습니다. 낚시 장소를 확인하세요.",
                        );
                        self.record();
                        return DeviceTick::Wait;
                    }
                    self.finish_tool(message);
                    if resume {
                        if let Err(error) = self.begin_fishing(at) {
                            self.stop(error);
                        }
                    } else {
                        self.dua.running = false;
                        self.dua.route = None;
                        self.dua.reason = message;
                    }
                    DeviceTick::Wait
                }
                Tick::Failed(error) => {
                    self.stop(error);
                    DeviceTick::Wait
                }
            };
            self.record();
            return input;
        }
        if !self.fishing.phase.active() {
            if self.dua.running {
                self.stop(self.fishing.message);
            }
            return DeviceTick::Wait;
        }
        let observation = self.fishing.observation(at, geometry);
        if frame.is_none() && at < observation.due_at {
            self.route(Route::Headless, "다음 관측 시각까지 기다립니다.", false);
            return DeviceTick::Wait;
        }
        self.route(Route::Screenshot, observation.reason, frame.is_some());
        let Some(frame) = frame else {
            return DeviceTick::Observe(observation.region);
        };
        let before = self.fishing.stats;
        let debug_log = self.macro_config()["mfish"]["debug_log"]
            .as_bool()
            .unwrap_or(false);
        self.fishing.configure_log(
            self.settings.as_deref().and_then(std::path::Path::parent),
            debug_log,
        );
        let captured_at = self
            .frame_expires
            .map(|expires| expires - 0.300)
            .unwrap_or(at);
        let tap = self.fishing.frame(captured_at, frame);
        if let Some(error) = self.fishing.log_error() {
            if self.log.len() == 300 {
                self.log.pop_front();
            }
            services::emit_hook("log", vec![serde_json::json!(error), serde_json::json!("")]);
            self.log
                .push_back((chrono::Utc::now().timestamp_millis(), error));
        }
        let changed = before != self.fishing.stats;
        if changed {
            self.totals.record_delta(before, self.fishing.stats);
        }
        if self.fishing.stats.inventory_increased(before) && self.automation.is_some() {
            self.automation
                .as_mut()
                .unwrap()
                .trigger(automation::Event::InventoryFull, at);
            self.fishing.stop("인벤토리 판매를 확인합니다.");
            return DeviceTick::Wait;
        }
        if self.automation.is_none()
            && self.navigation.options.use_sell
            && self.fishing.stats.inventory_increased(before)
        {
            match self.navigation.job(Task::Sell, at, true) {
                Ok(job) => {
                    self.generation = self.generation.wrapping_add(1);
                    self.job = Some(job);
                }
                Err(error) => self.stop(error),
            }
        }
        if self.fishing.needs_relocation(at) && self.automation.is_some() {
            if self.relocations >= 3 {
                self.stop("낚시 장소 확인 3회 실패 · 화면과 위치를 확인하세요.");
            } else {
                self.relocations += 1;
                self.automation
                    .as_mut()
                    .unwrap()
                    .trigger(automation::Event::Coast, at);
                self.fishing.stop("접근 가능한 해안을 다시 확인합니다.");
            }
            return DeviceTick::Wait;
        }
        if self.automation.is_none()
            && self.navigation.options.use_move
            && self.fishing.needs_relocation(at)
        {
            self.fishing.stop("낚시 장소로 다시 이동 중");
            if self.relocations >= 3 {
                self.stop("낚시 장소 이동 3회 실패 · 길과 위치를 확인하세요.");
            } else {
                self.relocations += 1;
                match self.navigation.job(Task::Fish, at, true) {
                    Ok(job) => {
                        self.generation = self.generation.wrapping_add(1);
                        self.job = Some(job);
                    }
                    Err(error) => match self.fishing.start(at) {
                        Ok(()) => self.fishing.message = error,
                        Err(error) => self.stop(error),
                    },
                }
            }
        }
        self.record();
        let calibrated = std::mem::take(&mut self.fishing.calibration_changed);
        if calibrated {
            self.sync_calibration();
        }
        if calibrated || changed {
            self.settings_error = self.save(self.fishing.tuning).err();
        }
        if self.fishing.phase == Phase::Idle
            && self.job.is_none()
            && self.automation.is_none()
            && !self.resume_after_calibration
        {
            self.stop(self.fishing.message);
        }
        // A tool handoff invalidates input computed by the preceding tool.
        if self.job.is_some() || !self.fishing.phase.active() {
            DeviceTick::Wait
        } else {
            if tap.is_some() {
                if let Some(runner) = self.automation.as_mut() {
                    runner.trigger(automation::Event::Activity, at);
                }
            }
            tap.map_or(DeviceTick::Wait, |tap| {
                DeviceTick::Input(GameInput::Tap(tap))
            })
        }
    }

    fn route(&mut self, route: Route, reason: &'static str, observed: bool) {
        if self.dua.route != Some(route) {
            self.dua.transitions += 1;
        }
        self.dua.route = Some(route);
        self.dua.reason = reason;
        match route {
            Route::Headless => self.dua.headless_ticks += 1,
            Route::Screenshot if observed => self.dua.screenshot_frames += 1,
            Route::Screenshot => {}
        }
    }

    fn active(&self) -> bool {
        self.dua.running
            || self.device_action.is_some()
            || self.sdk_request.is_some()
            || self.phase().active()
    }

    fn start_device_action(
        &mut self,
        gesture: device::Gesture,
        geometry: (usize, usize),
    ) -> Result<(), String> {
        if self.active() {
            return Err("실행 중인 작업을 먼저 중지하세요.".into());
        }
        if !self.capture || !self.input {
            return Err("화면 캡처와 입력을 먼저 준비하세요.".into());
        }
        #[cfg(target_os = "android")]
        if !self.phone.foreground {
            return Err("Roblox 화면에서 실행하세요.".into());
        }
        #[cfg(target_os = "linux")]
        validate_desktop_entry(self)?;
        gesture.packet(geometry, 1, self.generation)?;
        self.generation = self.generation.wrapping_add(1);
        self.phone.cancel();
        self.device_action = Some(device::Action::Queued(gesture, geometry));
        self.fishing.message = "기기 동작 확인 중";
        self.record();
        Ok(())
    }

    fn device_ack(&mut self) {
        self.device_ack_at(now());
    }
    fn device_ack_at(&mut self, at: f64) {
        if matches!(self.device_action, Some(device::Action::Pending(_))) {
            self.stop("기기 동작 완료");
            self.record();
        } else if let Some(navigation) = self.wayfinder.as_mut() {
            navigation.ack(true, at);
        } else if let Some(runner) = self.automation.as_mut() {
            runner.ack(true, at);
        }
    }

    fn movement_settings(&self) -> wayfinder::Settings {
        let mut settings = self.wayfinding.clone();
        if let (Some(center), Some(edge)) = (
            self.macro_point("stick_center"),
            self.macro_point("stick_edge"),
        ) {
            settings.controls = Some(navigator::Controls {
                stick_center: center,
                stick_edge: edge,
            });
        }
        settings.jump = self.macro_point("jump_btn").or(settings.jump);
        let config = self.macro_config();
        settings.player_label = config["player_label"]
            .as_str()
            .filter(|s| !s.trim().is_empty())
            .or_else(|| {
                config["biome"]["player"]
                    .as_str()
                    .filter(|s| !s.trim().is_empty())
            })
            .map(|s| s.trim().to_owned())
            .unwrap_or_else(|| biome::settings().player);
        settings
    }

    fn macro_point(&self, key: &str) -> Option<vision::Point> {
        serde_json::from_value::<vision::Point>(self.macro_settings["base"][key].clone())
            .ok()
            .filter(|p| {
                p.0.is_finite()
                    && p.1.is_finite()
                    && (0.0..=1.0).contains(&p.0)
                    && (0.0..=1.0).contains(&p.1)
            })
            .or_else(|| self.navigation.positions.get(key).copied())
    }

    fn start_travel(&mut self, destination: navigator::Destination, at: f64) -> Result<(), String> {
        let settings = self.movement_settings();
        self.wayfinder = Some(wayfinder::Wayfinder::start(destination, settings, at)?);
        self.fishing.stop("목적지로 이동 중");
        self.resume_after_calibration = false;
        self.generation = self.generation.wrapping_add(1);
        self.phone.cancel();
        Ok(())
    }

    fn automation_tick(
        &mut self,
        at: f64,
        geometry: (usize, usize),
        frame: Option<&vision::Frame<'_>>,
    ) -> Option<DeviceTick> {
        if let Some(mut navigation) = self.wayfinder.take() {
            let step = navigation.tick(
                at,
                geometry,
                frame,
                &self.ocr_words,
                self.frame_expires.map(|expires| expires - 0.300),
            );
            self.macro_message = Some(navigation.status().message.clone());
            let result = match step {
                wayfinder::Tick::Observe { region, ocr } => {
                    self.request_ocr = ocr;
                    self.route(
                        Route::Screenshot,
                        "이동할 지면과 목적지를 확인합니다.",
                        frame.is_some(),
                    );
                    DeviceTick::Observe(region)
                }
                wayfinder::Tick::Command(command) => {
                    self.route(
                        Route::Headless,
                        "관측한 경로의 짧은 동작을 실행합니다.",
                        false,
                    );
                    DeviceTick::Input(GameInput::Command(command))
                }
                wayfinder::Tick::Wait(_) => DeviceTick::Wait,
                wayfinder::Tick::Arrived => {
                    self.wayfinding.memory = navigation.memory();
                    if let Some(runner) = self.automation.as_mut() {
                        runner.travel_result(true, at);
                    }
                    self.settings_error = self.save(self.fishing.tuning).err();
                    self.record();
                    return Some(DeviceTick::Wait);
                }
                wayfinder::Tick::Stopped(error) => {
                    self.wayfinding.memory = navigation.memory();
                    if let Some(runner) = self.automation.as_mut() {
                        runner.travel_result(false, at);
                    }
                    self.stop_automation(error);
                    return Some(DeviceTick::Wait);
                }
            };
            self.wayfinder = Some(navigation);
            self.record();
            return Some(result);
        }
        sdk::before_tick(self);
        let mut runner = self.automation.take()?;
        let biome = biome::status();
        if let Some(name) = biome.current {
            let occurrence = biome.history.last().map_or(0, |(at, _)| *at as u64);
            if self.observed_biome.as_ref() != Some(&(name.clone(), occurrence)) {
                runner.trigger(
                    automation::Event::Biome {
                        name: name.clone(),
                        occurrence,
                    },
                    at,
                );
                self.observed_biome = Some((name, occurrence));
            }
        }
        if !self.ocr_words.is_empty() {
            let text = self
                .ocr_words
                .iter()
                .map(|w| w.text.as_str())
                .collect::<Vec<_>>()
                .join(" ")
                .to_lowercase();
            let in_game = (text.contains("auto") && text.contains("roll"))
                || (text.contains("자동") && (text.contains("뽑기") || text.contains("롤")))
                || self.fishing.phase == Phase::Reeling;
            if in_game {
                self.game_observed = true;
                self.game_disconnected = false;
            }
        }
        runner.trigger(
            automation::Event::Game {
                available: self.capture && self.input && !self.target_missing,
                in_game: self.game_observed,
            },
            at,
        );
        let failed_before = runner.status().failed;
        let step = runner.tick(at, geometry, frame, &self.ocr_words);
        self.macro_message = Some(runner.status().message.clone());
        for message in runner.drain_logs() {
            if self.log.len() == 300 {
                self.log.pop_front();
            }
            self.log
                .push_back((chrono::Utc::now().timestamp_millis(), message));
        }
        if runner.config() != &self.macro_settings {
            self.macro_settings = runner.config().clone();
            self.settings_error = self.save(self.fishing.tuning).err();
        }
        let finished = runner.finished();
        let failed = runner.status().failed > failed_before;
        self.automation = Some(runner);
        sdk::after_tick(
            self,
            matches!(step, automation::Tick::Done | automation::Tick::Stopped(_)),
            failed,
        );
        let result = match step {
            automation::Tick::Observe { region, ocr } => {
                self.request_ocr = ocr;
                self.fishing.stop("자동 작업 화면 확인 중");
                self.route(
                    Route::Screenshot,
                    "자동 작업의 실제 화면을 확인합니다.",
                    frame.is_some(),
                );
                DeviceTick::Observe(region)
            }
            automation::Tick::Command(command) => {
                if matches!(command, automation::Command::Launch { .. }) {
                    self.game_observed = false;
                    if let Some(runner) = self.automation.as_mut() {
                        runner.trigger(
                            automation::Event::Game {
                                available: false,
                                in_game: false,
                            },
                            at,
                        );
                    }
                }
                self.fishing.stop("자동 작업 실행 중");
                self.route(
                    Route::Headless,
                    "확인된 자동 작업의 입력을 실행합니다.",
                    false,
                );
                DeviceTick::Input(GameInput::Command(command))
            }
            automation::Tick::Travel(destination) => {
                if let Err(error) = self.start_travel(destination, at) {
                    self.stop_automation(error);
                }
                DeviceTick::Wait
            }
            automation::Tick::Fishing => {
                self.macro_message = None;
                if self.fishing.phase == Phase::Idle && !self.resume_after_calibration {
                    if let Err(error) = self.begin_fishing(at) {
                        self.stop(error);
                    }
                }
                return None;
            }
            automation::Tick::Done if finished => {
                self.stop("자동 작업 완료");
                DeviceTick::Wait
            }
            automation::Tick::Done | automation::Tick::Wait(_) => DeviceTick::Wait,
            automation::Tick::Stopped(error) => {
                self.stop_automation(error);
                DeviceTick::Wait
            }
        };
        self.record();
        Some(result)
    }

    fn device_command(&mut self, action: &str, geometry: (usize, usize)) -> Result<(), String> {
        use device::Gesture;
        use vision::Point;
        let point = |key: &str| {
            self.navigation
                .positions
                .get(key)
                .copied()
                .ok_or_else(|| format!("먼저 {key} 위치를 지정하세요."))
        };
        let gesture = match action {
            "camera_up" => Gesture::Drag {
                from: Point(0.62, 0.25),
                to: Point(0.62, 0.52),
                ms: 350,
            },
            "zoom_out" => Gesture::Pinch {
                from: [Point(0.40, 0.30), Point(0.64, 0.30)],
                to: [Point(0.49, 0.30), Point(0.55, 0.30)],
                ms: 350,
            },
            "jump" => Gesture::Tap {
                at: point("jump_btn")?,
                ms: 50,
            },
            "left" | "right" | "forward" | "back" | "moving_jump" => {
                let center = point("stick_center")?;
                let edge = point("stick_edge")?;
                let (width, height) = (geometry.0 as f64, geometry.1 as f64);
                if width < 1.0 || height < 1.0 {
                    return Err("화면 크기를 확인하세요.".into());
                }
                let radius = ((edge.0 - center.0) * width).hypot((edge.1 - center.1) * height);
                if !(8.0..=width.min(height) * 0.35).contains(&radius) {
                    return Err("조이스틱 가운데와 가장자리를 다시 지정하세요.".into());
                }
                let (dx, dy) = match action {
                    "left" => (-1.0, 0.0),
                    "right" => (1.0, 0.0),
                    "back" => (0.0, 1.0),
                    _ => (0.0, -1.0),
                };
                let x = (center.0 * width + dx * radius * 1.6).clamp(1.0, width - 1.0);
                let y = (center.1 * height + dy * radius * 1.6).clamp(1.0, height - 1.0);
                Gesture::Stick {
                    from: center,
                    to: Point(x / width, y / height),
                    jump: if action == "moving_jump" {
                        Some(point("jump_btn")?)
                    } else {
                        None
                    },
                    ms: 200,
                }
            }
            _ => return Err("알 수 없는 기기 동작입니다.".into()),
        };
        self.start_device_action(gesture, geometry)
    }

    fn macro_running(&self) -> bool {
        self.dua.running
            || self.job.as_ref().is_some_and(Job::resumes_fishing)
            || matches!(
                self.fishing.phase,
                Phase::Searching | Phase::Casting | Phase::Waiting | Phase::Reeling | Phase::Result
            )
    }

    fn start_desktop(&mut self, features: &[String], at: f64) -> Result<(), &'static str> {
        self.start_dua_with_foreground(features, at, false)
    }

    fn start_dua(&mut self, features: &[String], at: f64) -> Result<(), &'static str> {
        self.start_dua_with_foreground(features, at, cfg!(target_os = "android"))
    }

    fn start_dua_with_foreground(
        &mut self,
        features: &[String],
        at: f64,
        foreground_required: bool,
    ) -> Result<(), &'static str> {
        if features.is_empty() {
            return Err("실행할 매크로 기능을 켜 주세요.");
        }
        if features.iter().any(|feature| {
            ![
                "mfish", "mpop", "mitem", "mmerch", "mcraft", "mmatch", "mblood",
            ]
            .contains(&feature.as_str())
        }) {
            return Err("실행할 매크로 기능을 확인하세요.");
        }
        if self.active() {
            return Err("실행 중인 작업을 먼저 중지하세요.");
        }
        if !foreground_required && !self.dua.background {
            return Err("사용자 입력과 분리된 백그라운드 게임을 먼저 열어 주세요.");
        }
        if !self.capture || !self.input {
            return Err("화면 캡처와 입력을 먼저 준비하세요.");
        }
        let mut runner = automation::Runner::start_settings(self.macro_config(), features, at)
            .map_err(|_| "자동 매크로 설정의 값과 필수 위치를 확인하세요.")?;
        runner.restore_session(&self.last_automation);
        if let Some(geometry) = self.last_geometry {
            self.fishing
                .configure(&self.macro_config()["mfish"], geometry)
                .map_err(|_| "낚시 결과창의 수동 위치를 확인하세요.")?;
        }
        self.dua = DuaStatus {
            running: true,
            background: self.dua.background,
            hotkeys: self.dua.hotkeys,
            ..DuaStatus::default()
        };
        self.generation = self.generation.wrapping_add(1);
        self.phone.cancel();
        self.empty_sales = 0;
        self.relocations = 0;
        self.route(Route::Headless, "설정과 실행할 작업을 확인합니다.", false);
        self.job = None;
        self.wayfinder = None;
        self.macro_message = None;
        self.automation = Some(runner);
        if foreground_required && !self.phone.foreground {
            self.fishing.stop("Roblox가 앞에 오기를 기다립니다.");
            self.route(Route::Headless, "Roblox가 앞에 오기를 기다립니다.", false);
        }
        self.record();
        services::emit_hook("macro", vec![true.into()]);
        Ok(())
    }

    fn begin_fishing(&mut self, at: f64) -> Result<(), &'static str> {
        if self.dua.running && self.fishing.calibration.is_none() {
            self.fishing.calibrate(at)?;
            self.resume_after_calibration = true;
            Ok(())
        } else {
            self.fishing.start(at)
        }
    }

    fn reset_popping_template(&mut self, biome: &str) -> Result<serde_json::Value, String> {
        let biome = biome.to_ascii_uppercase();
        let template = automation::default_template(&biome)?;
        if self.active() {
            return Err("템플릿을 바꾸기 전에 작업을 중지하세요.".into());
        }
        let mut config = self.macro_config();
        config["mpop"]["templates"][&biome] = template.clone();
        let previous = std::mem::replace(&mut self.macro_settings, config);
        if let Err(error) = self.save(self.fishing.tuning) {
            self.macro_settings = previous;
            return Err(error);
        }
        Ok(template)
    }

    fn phase(&self) -> Phase {
        if self.device_action.is_some()
            || self.wayfinder.is_some()
            || self.automation.is_some() && self.fishing.phase == Phase::Idle && self.dua.running
        {
            return Phase::Moving;
        }
        match self.job.as_ref() {
            Some(job) if job.recording_at.is_some() && job.action != Task::Measure => {
                Phase::Recording
            }
            Some(job) if job.action == Task::Sell => Phase::Selling,
            Some(_) => Phase::Moving,
            None => self.fishing.phase,
        }
    }

    fn message(&self) -> &str {
        self.macro_message
            .as_deref()
            .unwrap_or_else(|| self.job.as_ref().map_or(self.fishing.message, Job::message))
    }

    fn stop(&mut self, message: &'static str) {
        let was_running = self.dua.running;
        if let Some(request) = self.sdk_request.take() {
            request.cancel(message);
        }
        self.dua.running = false;
        self.dua.route = None;
        self.dua.reason = message;
        self.resume_after_calibration = false;
        if let Some(mut runner) = self.automation.take() {
            runner.stop_all();
            self.macro_settings = runner.config().clone();
            self.last_automation = serde_json::to_value(runner.status()).unwrap_or_default();
            if self.settings.is_some() {
                self.settings_error = self.save(self.fishing.tuning).err();
            }
        }
        self.wayfinder = None;
        self.finish_tool(message);
        if was_running {
            services::emit_hook("macro", vec![false.into()]);
            services::emit_hook("stop", vec![]);
        }
    }

    fn stop_automation(&mut self, message: String) {
        self.stop("자동 작업을 중지했습니다.");
        self.macro_message = Some(message);
        self.record();
    }

    fn cancel_feature(&mut self, feature: &str) {
        let owns = feature == "mfish"
            && (self.fishing.phase.active() || self.resume_after_calibration)
            || self.automation.as_ref().is_some_and(|r| {
                r.status()
                    .active
                    .as_ref()
                    .is_some_and(|job| job.feature() == feature)
                    || feature == "mfish" && r.status().active.is_none() && self.wayfinder.is_some()
            });
        if let Some(runner) = self.automation.as_mut() {
            runner.stop_feature(feature);
        }
        if feature == "ret" {
            self.phone.cancel_return();
        }
        if owns {
            self.wayfinder = None;
            self.generation = self.generation.wrapping_add(1);
            self.phone.cancel();
            if let Some(request) = self.sdk_request.take() {
                request.cancel("입력 소유 작업이 중지됐습니다.");
            }
            self.fishing.stop("현재 작업을 중지했습니다.");
            self.resume_after_calibration = false;
        }
        if feature == "mfish" {
            self.fishing.stop("낚시를 중지했습니다.");
            self.resume_after_calibration = false;
        }
        self.record();
    }

    fn sync_calibration(&mut self) {
        let Some(calibration) = self.fishing.calibration else {
            return;
        };
        let positions = serde_json::to_value(calibration.positions()).unwrap_or_default();
        let mut config = self.macro_config();
        for (to, from) in [
            ("panel_region", "panel"),
            ("reel_region", "reel"),
            ("fish_btn", "fish"),
            ("bar_region", "bar"),
        ] {
            config["mfish"][to] = positions[from].clone();
        }
        config["mfish"]["cal_win"] = true.into();
        if let Some(runner) = self.automation.as_mut() {
            runner.update_fishing_calibration(&config["mfish"]);
        }
        self.macro_settings = config;
    }

    fn finish_tool(&mut self, message: &'static str) {
        if let Some(request) = self.sdk_request.take() {
            request.cancel(message);
            if let Some(runner) = self.automation.as_mut() {
                runner.stop_feature("sdk");
            }
        }
        self.generation = self.generation.wrapping_add(1);
        self.phone.cancel();
        self.device_action = None;
        self.job = None;
        self.macro_message = None;
        self.ocr_words.clear();
        self.fishing.stop(message);
    }

    fn input_rejected(&mut self, message: &'static str) {
        if !self.phase().active() {
            return;
        }
        if self.device_action.is_none() && self.automation.is_some() {
            if let Some(navigation) = self.wayfinder.as_mut() {
                navigation.ack(false, now());
            } else {
                if self.sdk_request.is_some() {
                    if let Some(request) = self.sdk_request.take() {
                        request.cancel(message);
                    }
                    self.automation.as_mut().unwrap().stop_feature("sdk");
                }
                self.automation.as_mut().unwrap().ack(false, now());
            }
            self.generation = self.generation.wrapping_add(1);
            self.phone.cancel();
            self.fishing.stop(message);
            self.macro_message = Some(message.into());
            self.record();
            return;
        }
        if self.job.as_mut().is_some_and(Job::retry_input) {
            self.generation = self.generation.wrapping_add(1);
        } else {
            let relocating = self.job.as_ref().is_some_and(Job::resumes_fishing)
                && (1..3).contains(&self.relocations);
            if relocating {
                self.finish_tool(message);
                if let Err(error) = self.fishing.start(now()) {
                    self.stop(error);
                } else {
                    self.fishing.message = message;
                }
            } else {
                self.stop(message);
            }
        }
        self.record();
    }

    fn no_target(&mut self) {
        if self.device_action.is_some() {
            self.stop("Roblox 화면이 바뀌어 기기 동작을 중지했습니다.");
            self.record();
            return;
        }
        if let Some(runner) = self.automation.as_mut() {
            self.game_observed = false;
            self.game_disconnected = true;
            runner.trigger(
                automation::Event::Game {
                    available: false,
                    in_game: false,
                },
                now(),
            );
            if self.target_missing {
                return;
            }
            self.target_missing = true;
            if self.wayfinder.take().is_some() {
                runner.travel_result(false, now());
            }
            runner.suspend(now());
            runner.resume(now());
            self.generation = self.generation.wrapping_add(1);
            self.phone.cancel();
            if let Some(request) = self.sdk_request.take() {
                request.cancel("게임 대상을 잃어 이전 요청을 취소했습니다.");
                runner.stop_feature("sdk");
            }
            self.fishing.stop("게임 연결을 기다립니다.");
            self.route(Route::Headless, "게임 연결을 다시 확인합니다.", false);
            return;
        }
        if let Some(job) = self.job.as_ref() {
            if job.action == Task::Measure
                && job.recording_at.is_some_and(|start| now() - start < 300.0)
            {
                return;
            }
            if job.started || now() > job.deadline {
                self.stop("Roblox 화면이 바뀌어 이동 · 판매를 중지했습니다.");
            }
        } else {
            if self.resume_after_calibration
                && self.fishing.phase == Phase::Idle
                && self.fishing.calibration.is_some()
            {
                self.route(Route::Headless, "Roblox가 앞에 오기를 기다립니다.", false);
                return;
            }
            self.fishing.no_target(now());
            if !self.fishing.phase.active() && self.dua.running {
                self.stop(self.fishing.message);
            } else if self.active() {
                self.route(Route::Headless, "Roblox가 앞에 오기를 기다립니다.", false);
            }
        }
    }

    fn game_line(&mut self, line: &str, at: f64) {
        if line.len() > 32768 || !at.is_finite() {
            return;
        }
        if automation::game_disconnected(line) {
            self.game_observed = false;
            self.game_disconnected = true;
        }
        if line.contains("\"state\":\"Equipped") {
            self.game_observed = true;
            self.game_disconnected = false;
        }
        if let Some(runner) = self.automation.as_mut() {
            runner.log(line, at);
        }
    }

    fn headless_missing(&mut self, at: f64) -> Option<(i32, automation::Command)> {
        if !self.dua.running || self.automation.is_none() {
            return None;
        }
        let mut runner = self.automation.take().unwrap();
        let step = runner.headless_missing(at);
        self.macro_message = Some(runner.status().message.clone());
        for message in runner.drain_logs() {
            if self.log.len() == 300 {
                self.log.pop_front();
            }
            self.log
                .push_back((chrono::Utc::now().timestamp_millis(), message));
        }
        if runner.config() != &self.macro_settings {
            self.macro_settings = runner.config().clone();
            self.settings_error = self.save(self.fishing.tuning).err();
        }
        self.automation = Some(runner);
        self.record();
        match step {
            automation::Tick::Command(command @ automation::Command::Launch { .. }) => {
                self.game_observed = false;
                if let Some(runner) = self.automation.as_mut() {
                    runner.trigger(
                        automation::Event::Game {
                            available: false,
                            in_game: false,
                        },
                        at,
                    );
                }
                self.route(Route::Headless, "게임 실행을 다시 요청합니다.", false);
                Some((self.generation, command))
            }
            automation::Tick::Stopped(error) => {
                self.stop_automation(error);
                None
            }
            _ => None,
        }
    }

    fn input_current(&self, generation: i32) -> bool {
        self.generation == generation && self.active()
    }

    fn record(&mut self) {
        if self.macro_running() {
            if self.macro_since.is_none() {
                self.macro_since = Some(chrono::Utc::now().timestamp_millis());
            }
        } else {
            self.macro_since = None;
        }
        let message = self.message().to_owned();
        if self.logged.as_deref() == Some(message.as_str()) {
            return;
        }
        if self.log.len() == 300 {
            self.log.pop_front();
        }
        self.log
            .push_back((chrono::Utc::now().timestamp_millis(), message.clone()));
        self.logged = Some(message);
        services::emit_hook(
            "log",
            vec![serde_json::json!(self.message()), serde_json::json!("")],
        );
        services::emit_hook(
            "status",
            vec![
                serde_json::json!(if self.dua.running { "running" } else { "idle" }),
                serde_json::json!(self.message()),
            ],
        );
        discord::wake();
    }

    fn control(&mut self, action: Action) -> Result<(), String> {
        if action == Action::Stop {
            self.stop("중지됨");
            return Ok(());
        }
        if action == Action::Start {
            let config = self.macro_config();
            let features: Vec<String> = [
                "mfish", "mpop", "mitem", "mmerch", "mcraft", "mmatch", "mblood",
            ]
            .into_iter()
            .filter(|key| config[*key]["enabled"].as_bool() == Some(true))
            .map(str::to_owned)
            .collect();
            return self.start_dua(&features, now()).map_err(str::to_owned);
        }
        if self.dua.running {
            return Err("실행 중인 DUA를 먼저 중지하세요.".into());
        }
        if !self.capture || !self.input {
            return Err("화면 캡처와 입력을 먼저 준비하세요.".into());
        }
        #[cfg(target_os = "android")]
        if !self.phone.foreground {
            return Err("Roblox에서 낚시 창을 연 뒤 보정하세요.".into());
        }
        if self.job.is_some() {
            return Err("실행 중인 작업을 먼저 중지하세요.".into());
        }
        match action {
            Action::Calibrate => {
                self.fishing.calibrate(now())?;
                self.generation = self.generation.wrapping_add(1);
                self.phone.cancel();
                Ok(())
            }
            Action::Start | Action::Stop => unreachable!(),
        }
        .map_err(str::to_owned)
    }

    fn measuring_since(&self) -> Option<f64> {
        self.job
            .as_ref()
            .filter(|job| job.action == Task::Measure)
            .and_then(|job| job.recording_at)
    }

    fn arrive_desktop(&mut self) -> bool {
        let Some(start) = self.measuring_since() else {
            return false;
        };
        self.measured_seconds = Some((now() - start).max(0.0));
        self.stop("이동 시간을 기록했습니다.");
        true
    }

    #[cfg(test)]
    fn task(&mut self, task: Task) -> Result<(), &'static str> {
        if cfg!(target_os = "android") {
            return Err("시간 기반 이동과 판매는 사용할 수 없습니다. 지도 관측 연결이 필요합니다.");
        }
        if !self.capture || !self.input {
            return Err("화면 캡처와 입력을 먼저 준비하세요.");
        }
        if self.active() {
            return Err("실행 중인 작업을 먼저 중지하세요.");
        }
        self.job = Some(self.navigation.job(task, now(), false)?);
        Ok(())
    }

    fn edit_navigation(
        &mut self,
        edit: impl FnOnce(&mut Navigation) -> Result<(), &'static str>,
    ) -> Result<(), String> {
        let previous = self.navigation.clone();
        edit(&mut self.navigation)?;
        if let Err(error) = self.save(self.fishing.tuning) {
            self.navigation = previous;
            return Err(error);
        }
        Ok(())
    }
}

fn game_available() -> bool {
    #[cfg(desktop)]
    return desktop::game_available();
    #[cfg(not(desktop))]
    false
}

#[derive(Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
enum Action {
    Calibrate,
    Start,
    Stop,
}

#[tauri::command]
fn status() -> Status {
    let mut state = runtime();
    state.record();
    state.status()
}

#[tauri::command]
fn control(action: Action) -> Result<Status, String> {
    let mut state = runtime();
    #[cfg(any(target_os = "linux", target_os = "windows", target_os = "macos"))]
    if action == Action::Start {
        validate_desktop_entry(&state)?;
    }
    state.control(action)?;
    Ok(state.status())
}

#[tauri::command]
fn dua_start(features: Vec<String>, generation: i32) -> Result<Status, String> {
    if !cfg!(any(
        target_os = "linux",
        target_os = "windows",
        target_os = "macos",
        target_os = "android"
    )) {
        return Err("이 운영체제의 DUA 입력 연결은 아직 준비되지 않았습니다.".into());
    }
    let mut state = runtime();
    if generation != state.generation {
        return Err("이전 실행 요청이 취소됐습니다. 다시 시작하세요.".into());
    }
    #[cfg(target_os = "linux")]
    validate_desktop_entry(&state)?;
    state.start_dua(&features, now())?;
    Ok(state.status())
}

#[tauri::command]
fn dua_stop() -> Status {
    let mut state = runtime();
    state.stop("중지됨");
    state.record();
    state.status()
}

#[tauri::command]
fn device_action(
    action: String,
    width: usize,
    height: usize,
    generation: i32,
) -> Result<Status, String> {
    if !cfg!(any(
        target_os = "android",
        target_os = "linux",
        target_os = "windows",
        target_os = "macos"
    )) {
        return Err("이 기기의 독립 입력 연결이 아직 준비되지 않았습니다.".into());
    }
    let mut state = runtime();
    if state.generation != generation {
        return Err("이전 입력 요청이 취소됐습니다.".into());
    }
    state.device_command(&action, (width, height))?;
    Ok(state.status())
}

#[tauri::command]
fn desktop_start(features: Vec<String>, generation: i32) -> Result<Status, String> {
    if !cfg!(any(
        target_os = "windows",
        target_os = "macos",
        target_os = "linux"
    )) {
        return Err("데스크톱 DUA의 게임 캡처 · 입력은 Windows와 Linux에서 지원합니다.".into());
    }
    let mut state = runtime();
    if state.generation != generation {
        return Err("이전 실행 요청이 취소됐습니다. 다시 시작하세요.".into());
    }
    #[cfg(target_os = "linux")]
    if !desktop::game_available() {
        return Err(
            "실행 중인 Sober 게임 창을 찾지 못했습니다. Sober에서 게임을 연 뒤 다시 시작하세요."
                .into(),
        );
    }
    validate_desktop_entry(&state)?;
    state.start_desktop(&features, now())?;
    Ok(state.status())
}

fn validate_desktop_entry(state: &Runtime) -> Result<(), String> {
    if !state.dua.background {
        return Err("백그라운드 게임을 먼저 열어 주세요.".into());
    }
    Ok(())
}

#[tauri::command]
fn desktop_session_start() -> Result<BackgroundStatus, String> {
    #[cfg(desktop)]
    return desktop::launch_session();
    #[cfg(not(desktop))]
    Err("이 운영체제의 독립 입력 연결은 아직 준비되지 않았습니다.".into())
}
#[tauri::command]
async fn desktop_session_stop() -> BackgroundStatus {
    #[cfg(desktop)]
    let _ = tauri::async_runtime::spawn_blocking(desktop::stop_session).await;
    background_status()
}
#[tauri::command]
async fn desktop_manual(event: serde_json::Value, generation: i32) -> Result<(), String> {
    #[cfg(desktop)]
    {
        let event = serde_json::from_value::<desktop::ManualEvent>(event)
            .map_err(|_| "게임 입력 요청이 올바르지 않습니다.".to_owned())?;
        return tauri::async_runtime::spawn_blocking(move || desktop::manual(event, generation))
            .await
            .map_err(|e| e.to_string())?;
    }
    #[cfg(not(desktop))]
    {
        let _ = (event, generation);
        Err("독립 게임 입력이 준비되지 않았습니다.".into())
    }
}
#[tauri::command]
async fn navigation_plan(
    map: pathfinding::Map,
    request: pathfinding::Request,
) -> Result<pathfinding::Route, String> {
    tauri::async_runtime::spawn_blocking(move || pathfinding::plan(&map, &request))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn desktop_snapshot() -> Result<tauri::ipc::Response, String> {
    #[cfg(desktop)]
    return tauri::async_runtime::spawn_blocking(desktop::snapshot)
        .await
        .map_err(|error| error.to_string())?
        .map(tauri::ipc::Response::new);
    #[cfg(not(desktop))]
    Err("이 환경에서는 앱 미리보기 위치 지정을 사용할 수 없습니다.".into())
}

#[tauri::command]
fn save_tuning(tuning: Tuning) -> Result<Status, String> {
    let mut state = runtime();
    if state.active() {
        return Err("실행 중인 작업을 먼저 중지하세요.".into());
    }
    let tuning = tuning.validate()?;
    let old = state.macro_settings.clone();
    let mut config = state.macro_config();
    let value = serde_json::to_value(tuning).map_err(|e| e.to_string())?;
    for (to, from) in [
        ("bite_max", "biteSeconds"),
        ("lead_ms", "leadMs"),
        ("target_pct", "targetPercent"),
        ("click_ms", "clickMs"),
        ("click_gap_ms", "clickGapMs"),
        ("cast_retry", "castRetries"),
        ("adaptive", "adaptive"),
        ("tap_above", "tapAbove"),
        ("deadband", "deadband"),
    ] {
        config["mfish"][to] = value[from].clone();
    }
    state.macro_settings = config;
    if let Err(error) = state.save(tuning) {
        state.macro_settings = old;
        return Err(error);
    }
    state.fishing.tuning = tuning;
    state.settings_error = None;
    Ok(state.status())
}

#[tauri::command]
fn navigation_status() -> serde_json::Value {
    let state = runtime();
    serde_json::to_value(NavigationStatus {
        options: state.navigation.options,
        positions: &state.navigation.positions,
        fish: state.navigation.route_parts(Place::Fish),
        sell: state.navigation.route_parts(Place::Sell),
    })
    .expect("navigation status serializes")
}

#[tauri::command]
fn save_navigation(options: Options) -> Result<(), String> {
    let mut state = runtime();
    if state.active() {
        return Err("실행 중인 작업을 먼저 중지하세요.".into());
    }
    let options = options.validate()?;
    #[cfg(target_os = "android")]
    let options = {
        let mut options = options;
        options.use_move = false;
        options.use_sell = false;
        options
    };
    state.edit_navigation(|nav| {
        nav.options = options;
        Ok(())
    })
}

#[tauri::command]
fn desktop_task(task: Task) -> Result<(), String> {
    if !cfg!(any(
        target_os = "windows",
        target_os = "macos",
        target_os = "linux"
    )) {
        return Err("Windows와 Linux에서 게임 입력을 사용할 수 있습니다.".into());
    }
    task_call(task)
}

pub(crate) fn task_call(task: Task) -> Result<(), String> {
    use serde_json::json;
    let (action, payload) = match task {
        Task::Base => ("move_base", json!({})),
        Task::Fish => ("move_place", json!({"place":"fish_spot"})),
        Task::Shop => ("move_place", json!({"place":"sell_spot"})),
        Task::Sell => ("sell_test", json!({})),
        Task::RecordFish => ("waypoint_mark", json!({"destination":"coast"})),
        Task::RecordSell => ("waypoint_mark", json!({"destination":"sell"})),
        Task::Measure => ("waypoint_mark", json!({"destination":"base"})),
        Task::StickTest | Task::JumpTest => {
            let mut state = runtime();
            let geometry = state.last_geometry.ok_or("관측한 게임 화면이 없습니다.")?;
            return state.device_command(
                if task == Task::StickTest {
                    "forward"
                } else {
                    "jump"
                },
                geometry,
            );
        }
    };
    automation_call(action.into(), payload, current_generation()).map(|_| ())
}

#[tauri::command]
async fn desktop_pick() -> Result<vision::Point, String> {
    #[cfg(desktop)]
    return tauri::async_runtime::spawn_blocking(desktop::pick)
        .await
        .map_err(|e| e.to_string())?;
    #[cfg(not(desktop))]
    Err("독립 입력 연결이 준비된 환경에서 위치를 지정할 수 있습니다.".into())
}

#[tauri::command]
fn desktop_cancel_pick() {
    #[cfg(desktop)]
    desktop::cancel_pick();
}

#[tauri::command]
fn clear_navigation() -> Result<(), String> {
    let mut state = runtime();
    if state.active() {
        return Err("실행 중인 작업을 먼저 중지하세요.".into());
    }
    state.edit_navigation(|nav| {
        nav.positions.clear();
        nav.fish.clear();
        nav.sell.clear();
        nav.fish_trace.clear();
        nav.sell_trace.clear();
        Ok(())
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
enum Reset {
    Calibration,
    Session,
    Total,
    Log,
}

#[tauri::command]
fn reset(kind: Reset) -> Result<Status, String> {
    let mut state = runtime();
    match kind {
        Reset::Session => state.fishing.stats = Stats::default(),
        Reset::Log => {
            state.log.clear();
            biome::clear_log();
        }
        Reset::Calibration | Reset::Total => {
            let previous = (state.fishing.calibration, state.totals);
            match kind {
                Reset::Calibration => {
                    state.stop("보정 초기화");
                    state.fishing.calibration = None;
                }
                Reset::Total => state.totals = Stats::default(),
                _ => unreachable!(),
            }
            if let Err(error) = state.save(state.fishing.tuning) {
                (state.fishing.calibration, state.totals) = previous;
                return Err(error);
            }
            if matches!(kind, Reset::Total) {
                state.fishing.stats = Stats::default();
            }
        }
    }
    Ok(state.status())
}

#[tauri::command]
fn fishing_log() -> Vec<String> {
    let mut lines: Vec<String> = runtime()
        .log
        .iter()
        .rev()
        .map(|(_, line)| line.clone())
        .collect();
    lines.extend(biome::status().logs);
    lines.truncate(300);
    lines
}

pub(crate) fn current_generation() -> i32 {
    runtime().generation
}

pub(crate) fn game_line(line: &str) {
    runtime().game_line(line, now());
}

#[tauri::command]
fn macro_settings() -> serde_json::Value {
    runtime().macro_config()
}

#[tauri::command]
fn save_macro_settings(mut config: serde_json::Value) -> Result<serde_json::Value, String> {
    if !config.is_object()
        || serde_json::to_vec(&config)
            .map_err(|e| e.to_string())?
            .len()
            > 2 * 1024 * 1024
    {
        return Err("매크로 설정의 형식과 크기를 확인하세요.".into());
    }
    config["macro_on"] = false.into();
    let _ = automation::Runner::new(config.clone(), now())?;
    let mut state = runtime();
    if state.active() && state.macro_config() != config {
        return Err("설정을 바꾸기 전에 실행 중인 작업을 중지하세요.".into());
    }
    config["stats"] = state.macro_config()["stats"].clone();
    let prior = state.macro_config();
    let regions_changed = ["panel_region", "reel_region", "fish_btn", "bar_region"]
        .iter()
        .any(|key| prior["mfish"][*key] != config["mfish"][*key]);
    let mut value = serde_json::to_value(state.fishing.tuning).map_err(|e| e.to_string())?;
    for (from, to) in [
        ("bite_max", "biteSeconds"),
        ("lead_ms", "leadMs"),
        ("target_pct", "targetPercent"),
        ("click_ms", "clickMs"),
        ("click_gap_ms", "clickGapMs"),
        ("cast_retry", "castRetries"),
        ("adaptive", "adaptive"),
        ("tap_above", "tapAbove"),
        ("deadband", "deadband"),
    ] {
        if !config["mfish"][from].is_null() {
            value[to] = config["mfish"][from].clone();
        }
    }
    let tuning: Tuning = serde_json::from_value(value).map_err(|e| e.to_string())?;
    let tuning = tuning.validate()?;
    if let Some(geometry) = state.last_geometry {
        let mut check = Fishing::default();
        check.configure(&config["mfish"], geometry)?;
    }
    let previous = std::mem::replace(&mut state.macro_settings, config.clone());
    let previous_tuning = std::mem::replace(&mut state.fishing.tuning, tuning);
    if let Err(error) = state.save(tuning) {
        state.macro_settings = previous;
        state.fishing.tuning = previous_tuning;
        return Err(error);
    }
    state.fishing_settings_dirty |= regions_changed;
    if let Some(geometry) = state.last_geometry {
        state.fishing.configure(&config["mfish"], geometry)?;
    }
    Ok(config)
}

#[tauri::command]
async fn pick_macro_position(
    app: tauri::AppHandle,
    feat: String,
    key: String,
    region: bool,
    generation: Option<i32>,
) -> Result<serde_json::Value, String> {
    let (expected, initial) = {
        let state = runtime();
        if state.active() {
            return Err("작업을 중지한 뒤 위치를 지정하세요.".into());
        }
        if generation.is_some_and(|g| g != state.generation) {
            return Err("이전 위치 지정 요청이 취소됐습니다.".into());
        }
        let config = state.macro_config();
        if !config[&feat].is_object() || !config[&feat].as_object().unwrap().contains_key(&key) {
            return Err("저장할 매크로 위치를 확인하세요.".into());
        }
        (state.generation, config[&feat][&key].clone())
    };
    #[cfg(target_os = "android")]
    let result = {
        let label = key.clone();
        tauri::async_runtime::spawn_blocking(move||android::pick_macro(&app,serde_json::json!({"label":label,"region":region,"initial":initial.as_array().cloned().unwrap_or_default()}))).await.map_err(|e|e.to_string())??
    };
    #[cfg(desktop)]
    let result = {
        let _ = (app, initial);
        tauri::async_runtime::spawn_blocking(move || {
            let a = desktop::pick()?;
            let pos = if region {
                let b = desktop::pick()?;
                serde_json::json!([a.0.min(b.0), a.1.min(b.1), a.0.max(b.0), a.1.max(b.1)])
            } else {
                serde_json::json!([a.0, a.1])
            };
            Ok::<_, String>(serde_json::json!({"pos":pos}))
        })
        .await
        .map_err(|e| e.to_string())??
    };
    let mut state = runtime();
    if state.active() || state.generation != expected {
        return Err("실행 상태가 바뀌어 위치를 저장하지 않았습니다.".into());
    }
    let pos = result["pos"].clone();
    let a = pos
        .as_array()
        .filter(|a| a.len() == if region { 4 } else { 2 })
        .ok_or("선택한 좌표가 올바르지 않습니다.")?;
    if a.iter().any(|v| {
        v.as_f64()
            .is_none_or(|n| !n.is_finite() || !(0.0..1.0).contains(&n))
    }) || region && (a[0].as_f64() >= a[2].as_f64() || a[1].as_f64() >= a[3].as_f64())
    {
        return Err("좌표가 화면 밖이거나 영역이 비어 있습니다.".into());
    }
    let previous = state.macro_config();
    let nav = state.navigation.clone();
    let calibration = state.fishing.calibration;
    let mut config = previous.clone();
    config[&feat][&key] = pos.clone();
    state.macro_settings = config.clone();
    if !region {
        state.navigation.positions.insert(
            key.clone(),
            vision::Point(a[0].as_f64().unwrap(), a[1].as_f64().unwrap()),
        );
    }
    if let Some(g) = result["geometry"].as_array().filter(|g| g.len() == 2) {
        state.last_geometry = g[0]
            .as_u64()
            .zip(g[1].as_u64())
            .map(|(w, h)| (w as usize, h as usize));
    }
    if feat == "mfish" {
        if let Some(geometry) = state.last_geometry {
            state.fishing.configure(&config["mfish"], geometry)?;
            if let Some(manual) = Calibration::manual(&config["mfish"], geometry)? {
                state.fishing.calibration = Some(manual);
            }
        }
    }
    if let Err(error) = state.save(state.fishing.tuning) {
        state.macro_settings = previous;
        state.navigation = nav;
        state.fishing.calibration = calibration;
        return Err(error);
    }
    Ok(serde_json::json!({"pos":pos,"config":config}))
}

#[tauri::command]
async fn automation_action(
    app: tauri::AppHandle,
    action: String,
    payload: serde_json::Value,
    generation: i32,
) -> Result<serde_json::Value, String> {
    #[cfg(target_os = "android")]
    if ![
        "ocr_info",
        "mpop_default",
        "ret_join",
        "ret_test",
        "crash_cancel",
    ]
    .contains(&action.as_str())
        && !action.ends_with("_stop")
    {
        android::setup(&app, "returnGame", None)?;
    }
    #[cfg(not(target_os = "android"))]
    let _ = app;
    tauri::async_runtime::spawn_blocking(move || automation_call(action, payload, generation))
        .await
        .map_err(|e| e.to_string())?
}

pub(crate) fn automation_call(
    action: String,
    payload: serde_json::Value,
    generation: i32,
) -> Result<serde_json::Value, String> {
    use serde_json::json;
    if action == "sdk_call" {
        return sdk::call(payload, generation);
    }
    if action == "mfish_check" {
        return sdk::call(
            json!({"module":"macro","method":"fishing_check","args":[]}),
            generation,
        );
    }
    if ["ocr_test", "mpop_ocr_test", "mpop_ocr"].contains(&action.as_str()) {
        let region = if action == "ocr_test" {
            payload["region"].clone()
        } else {
            macro_settings()["base"]["ocr_region"].clone()
        };
        if region.is_null() {
            return Err("읽을 글자 영역을 먼저 지정하세요.".into());
        }
        let value = sdk::call(
            json!({"module":"macro","method":if action=="ocr_test"{"ocr_region"}else{"ocr_inventory"},"args":[region]}),
            generation,
        )?;
        return Ok(if action == "ocr_test" {
            json!({"text":value,"engine":sdk::engine_name()})
        } else {
            value
        });
    }
    if action == "waypoint_mark" {
        return sdk::call(
            json!({"module":"macro","method":"waypoint_mark","args":[payload["destination"]]}),
            generation,
        );
    }
    let mut state = runtime();
    if generation != state.generation {
        return Err("이전 실행 요청이 취소됐습니다.".into());
    }
    if action == "waypoint_cancel" {
        if let Some(request) = state.sdk_request.take() {
            request.cancel("목적지 등록을 취소했습니다.");
        }
        return Ok(json!({"ok":true}));
    }
    if action == "ocr_info" {
        return Ok(
            json!({"engine":sdk::engine_name(),"local":true,"info":"화면 글자 읽기는 기기에서만 처리합니다."}),
        );
    }
    if action == "mpop_default" {
        let template = state.reset_popping_template(payload["biome"].as_str().unwrap_or(""))?;
        return Ok(json!({"template":template}));
    }
    if action == "crash_cancel" {
        state.cancel_feature("ret");
        state.cancel_feature("play");
        if let Some(runner) = state.automation.as_mut() {
            runner.trigger(automation::Event::CancelRecovery, now());
        }
        return Ok(json!({"ok":true}));
    }
    if action.ends_with("_stop") || action == "move_stop" {
        if action == "move_stop" {
            state.stop("중지됨");
        } else {
            state.cancel_feature(action.trim_end_matches("_stop"));
        }
        return Ok(json!({"ok":true}));
    }
    let job =
        if ["move_base", "move_base_rev", "move_place", "move_test"].contains(&action.as_str()) {
            let destination = if action == "move_base" || action == "move_base_rev" {
                navigator::Destination::Base
            } else {
                let config = state.macro_config();
                let index = payload["i"]
                    .as_u64()
                    .or_else(|| payload["index"].as_u64())
                    .unwrap_or(0) as usize;
                let name = payload["place"]
                    .as_str()
                    .or_else(|| config["move"]["places"][index]["key"].as_str())
                    .unwrap_or("fish_spot");
                match name {
                    "sell" | "sell_spot" => navigator::Destination::Sell,
                    "craft" | "craft_spot" => navigator::Destination::Craft,
                    "match" | "match_spot" => navigator::Destination::Match,
                    _ => navigator::Destination::Coast,
                }
            };
            automation::Job::Go { destination }
        } else {
            automation::Runner::action(&action, &payload)?
                .ok_or_else(|| format!("알 수 없는 매크로 작업: {action}"))?
        };
    if !state.capture || !state.input {
        return Err("화면 캡처와 입력을 먼저 준비하세요.".into());
    }
    #[cfg(not(target_os = "android"))]
    validate_desktop_entry(&state)?;
    if state.automation.is_none() {
        if state.active() {
            return Err("실행 중인 작업을 먼저 중지하세요.".into());
        }
        state.automation = Some(automation::Runner::new(state.macro_config(), now())?);
        let previous = state.last_automation.clone();
        state
            .automation
            .as_mut()
            .unwrap()
            .restore_session(&previous);
        state.dua.running = true;
        state.generation = state.generation.wrapping_add(1);
        state.phone.cancel();
        #[cfg(target_os = "android")]
        if !state.phone.foreground {
            state.automation.as_mut().unwrap().suspend(now());
        }
    }
    #[cfg(target_os = "android")]
    let phone_return = matches!(&job, automation::Job::Return { .. });
    state.automation.as_mut().unwrap().queue(job)?;
    #[cfg(target_os = "android")]
    if phone_return {
        phone::request_return(&mut state, now());
    }
    state.macro_message = Some("자동 작업을 시작합니다.".into());
    state.record();
    Ok(
        json!({"ok":true,"queued":true,"running":true,"generation":state.generation,"automation":state.status().automation}),
    )
}

#[tauri::command]
fn android_input_status(app: tauri::AppHandle) -> Result<serde_json::Value, String> {
    #[cfg(target_os = "android")]
    return android::input_status(&app);
    #[cfg(not(target_os = "android"))]
    {
        let _ = app;
        Ok(serde_json::json!({"enabled":true,"selected":true}))
    }
}

#[tauri::command]
fn android_setup(
    _app: tauri::AppHandle,
    action: String,
    options: Option<serde_json::Value>,
) -> Result<(), String> {
    #[cfg(target_os = "android")]
    return android::setup(&_app, &action, options);
    #[cfg(not(target_os = "android"))]
    {
        let _ = (action, options);
        Err("안드로이드에서만 사용할 수 있습니다.".into())
    }
}

#[tauri::command]
fn biome_status() -> biome::Status {
    biome::status()
}

#[tauri::command]
fn save_biome(settings: biome::Settings) -> Result<biome::Status, String> {
    settings.validate()?;
    let state = runtime();
    state.save_biome_profile(state.fishing.tuning, settings.clone())?;
    biome::configure(settings);
    Ok(biome::status())
}

#[tauri::command]
fn biome_test() -> Result<(), String> {
    biome::test()
}

#[tauri::command]
fn biome_transport(_app: tauri::AppHandle) -> Result<serde_json::Value, String> {
    #[cfg(target_os = "android")]
    return android::transport(&_app);
    #[cfg(not(target_os = "android"))]
    Ok(serde_json::json!({"connection": "off", "testOutput": "", "testUntil": 0}))
}

#[tauri::command]
fn online_status() -> serde_json::Value {
    #[cfg(any(target_os = "windows", target_os = "linux", target_os = "android"))]
    online::start();
    serde_json::json!({"online": online::count(), "share": online::settings().share})
}

#[tauri::command]
fn save_online(share: bool) -> Result<(), String> {
    let state = runtime();
    let previous = online::settings();
    online::load(online::Settings {
        share,
        ..previous.clone()
    });
    if let Err(error) = state.save(state.fishing.tuning) {
        online::load(previous);
        return Err(error);
    }
    Ok(())
}

#[tauri::command]
fn discord_state() -> discord::PublicState {
    discord::status()
}

#[tauri::command]
async fn discord_login(app: tauri::AppHandle) -> Result<discord::PublicState, String> {
    tauri::async_runtime::spawn_blocking(move || {
        if let Some(url) = discord::login()? {
            #[cfg(target_os = "android")]
            let opened = android::setup(&app, "discordLink", Some(serde_json::json!({"url":url})));
            #[cfg(any(target_os = "linux", target_os = "windows", target_os = "macos"))]
            let opened = {
                let _ = app;
                open::that(url).map_err(|_| "브라우저를 열지 못했습니다.".to_owned())
            };
            #[cfg(not(any(
                target_os = "android",
                target_os = "linux",
                target_os = "windows",
                target_os = "macos"
            )))]
            let opened: Result<(), String> = {
                let _ = (app, url);
                Err("지원하지 않는 플랫폼입니다.".into())
            };
            if opened.is_err() {
                discord::browser_failed();
            }
        }
        Ok(discord::status())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
fn discord_cancel() -> discord::PublicState {
    discord::cancel()
}

#[tauri::command]
async fn discord_logout() -> Result<discord::PublicState, String> {
    tauri::async_runtime::spawn_blocking(discord::logout)
        .await
        .map_err(|e| e.to_string())?
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default();
    #[cfg(target_os = "android")]
    let builder = builder.plugin(android::plugin());
    builder
        .register_uri_scheme_protocol("acruxmod", services::mod_protocol)
        .setup(|app| {
            let dir = app.path().app_config_dir()?;
            fs::create_dir_all(&dir)?;
            runtime().load(dir.join("profile.json"));
            services::init(app.handle()).map_err(std::io::Error::other)?;
            {
                let mut state = runtime();
                if state.settings_error.is_none() {
                    state.settings_error = state.save(state.fishing.tuning).err();
                }
            }
            #[cfg(desktop)]
            biome::watch_desktop();
            #[cfg(desktop)]
            desktop::start()?;
            discord::start();
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            status,
            biome_status,
            biome_transport,
            save_biome,
            biome_test,
            online_status,
            save_online,
            discord_state,
            discord_login,
            discord_cancel,
            discord_logout,
            control,
            dua_start,
            dua_stop,
            device_action,
            desktop_start,
            desktop_snapshot,
            desktop_session_start,
            desktop_session_stop,
            desktop_manual,
            navigation_plan,
            save_tuning,
            android_setup,
            reset,
            fishing_log,
            macro_settings,
            save_macro_settings,
            automation_action,
            android_input_status,
            pick_macro_position,
            services::service_action,
            navigation_status,
            save_navigation,
            clear_navigation,
            desktop_task,
            desktop_pick,
            desktop_cancel_pick
        ])
        .build(tauri::generate_context!())
        .expect("could not launch Acrux")
        .run(|_app, event| {
            if matches!(event, tauri::RunEvent::Exit) {
                runtime().stop("앱을 종료합니다.");
                #[cfg(desktop)]
                desktop::shutdown();
                if let Err(error) = services::shutdown_worker() {
                    eprintln!("{error}");
                }
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_template_reset_replaces_custom_rows_persists_and_rolls_back_on_write_failure() {
        let folder = std::env::temp_dir().join(format!("acrux-template-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&folder).unwrap();
        let path = folder.join("settings.json");
        let mut state = Runtime {
            settings: Some(path.clone()),
            ..Runtime::default()
        };
        let mut config = state.macro_config();
        config["mpop"]["templates"]["GLITCHED"] = serde_json::json!({"mode":"one","items":[]});
        config["mpop"]["templates"]["CYBERSPACE"] = serde_json::json!({"mode":"custom","items":[]});
        state.macro_settings = config;
        let expected = automation::default_template("GLITCHED").unwrap();
        assert_eq!(state.reset_popping_template("glitched").unwrap(), expected);
        let saved: Profile = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(
            saved.macro_settings["mpop"]["templates"]["GLITCHED"],
            expected
        );
        assert_eq!(
            saved.macro_settings["mpop"]["templates"]["CYBERSPACE"]["mode"],
            "custom"
        );
        state.dua.running = true;
        assert!(state.reset_popping_template("CYBERSPACE").is_err());
        state.dua.running = false;
        let blocker = folder.join("blocker");
        fs::write(&blocker, b"file").unwrap();
        state.settings = Some(blocker.join("settings.json"));
        let previous = state.macro_settings.clone();
        assert!(state.reset_popping_template("CYBERSPACE").is_err());
        assert_eq!(state.macro_settings, previous);
        fs::remove_dir_all(folder).unwrap();
    }

    #[test]
    fn private_account_settings_round_trip_without_entering_public_status() {
        let path =
            std::env::temp_dir().join(format!("acrux-account-{}.json", uuid::Uuid::new_v4()));
        let state = Runtime {
            settings: Some(path.clone()),
            ..Runtime::default()
        };
        let credentials: discord::Settings =
            serde_json::from_value(serde_json::json!({"token":"a".repeat(64),"name":"account"}))
                .unwrap();
        state
            .save_profile(
                state.fishing.tuning,
                biome::Settings::default(),
                credentials,
            )
            .unwrap();
        let saved: Profile = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(
            serde_json::to_value(saved.discord_link).unwrap()["token"],
            "a".repeat(64)
        );
        let public = serde_json::to_string(&state.status()).unwrap();
        assert!(!public.contains(&"a".repeat(64)) && !public.contains("discordLink"));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        fs::remove_file(path).unwrap();
    }

    #[cfg(any(target_os = "windows", target_os = "linux"))]
    #[test]
    fn desktop_start_rejects_a_request_from_before_stop() {
        let generation = {
            let mut state = runtime();
            *state = desktop_runtime();
            let generation = state.generation;
            state.control(Action::Stop).unwrap();
            generation
        };
        assert!(desktop_start(vec!["mfish".into()], generation)
            .err()
            .unwrap()
            .contains("취소"));
        assert!(!runtime().dua.running);
    }

    #[test]
    fn desktop_runtime_refuses_shared_input_but_accepts_observed_travel_and_sale() {
        let mut state = Runtime {
            capture: true,
            input: true,
            ..Runtime::default()
        };
        assert!(state.start_desktop(&["mfish".into()], 0.0).is_err());
        assert!(!state.active());
        state.dua.background = true;
        state.navigation.options.use_move = true;
        state.navigation.options.use_sell = true;
        assert!(validate_desktop_entry(&state).is_ok());
        state.start_desktop(&["mfish".into()], 0.1).unwrap();
        assert!(state.automation.is_some() && state.job.is_none());
        assert!(matches!(
            state.tick(0.2, (480, 270), None),
            DeviceTick::Wait
        ));
        assert!(state.wayfinder.is_some() && state.job.is_none());
        assert!(
            matches!(state.tick(0.3, (480, 270), None), DeviceTick::Observe(_)),
            "fresh screen precedes all movement; legacy timing cannot emit input"
        );
    }

    #[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
    #[test]
    fn old_profile_keeps_positions_but_cannot_enable_timed_desktop_travel() {
        let path = std::env::temp_dir().join(format!(
            "acrux-profile-upgrade-{}.json",
            uuid::Uuid::new_v4()
        ));
        let profile = serde_json::json!({
            "navigation": {
                "options": { "useMove": true, "useSell": true, "sellMax": 37 },
                "positions": { "chat_pos": [0.3, 0.4], "sell_fish_pos": [0.6, 0.7] },
                "fish": [{ "x": 1, "y": 0, "ms": 500, "jump": false }],
                "sell": [{ "x": 0, "y": -1, "ms": 700, "jump": true }]
            }
        });
        fs::write(&path, serde_json::to_vec(&profile).unwrap()).unwrap();
        let previous_biome = biome::settings();
        let previous_online = online::settings();
        let mut state = desktop_runtime();
        state.load(path.clone());
        assert!(state.settings_error.is_none());
        assert!(state.navigation.options.use_move && state.navigation.options.use_sell);
        let loaded = serde_json::to_value(&state.navigation).unwrap();
        for key in ["positions", "fish", "sell"] {
            assert_eq!(loaded[key], profile["navigation"][key]);
        }
        assert_eq!(loaded["options"]["sellMax"], 37);
        assert!(validate_desktop_entry(&state).is_ok());
        state.start_desktop(&["mfish".into()], 10.0).unwrap();
        assert!(state.dua.running && state.job.is_none());
        assert!(state.automation.is_some() && state.wayfinder.is_none());
        assert!(matches!(
            state.tick(10.1, (480, 270), None),
            DeviceTick::Wait
        ));
        assert!(matches!(
            state.tick(10.15, (480, 270), None),
            DeviceTick::Observe(_)
        ));
        let pixels = fishing_screen();
        let frame = vision::Frame::new(&pixels, 480, 270, 1920, 4, false).unwrap();
        state.frame_expires = Some(10.2 + 0.300);
        assert!(!matches!(
            state.tick(10.2, (480, 270), Some(&frame)),
            DeviceTick::Input(GameInput::Motion(_))
        ));
        assert!(
            state.wayfinder.is_none() && state.job.is_none() && state.automation.is_some(),
            "the actual Fish UI confirms coast arrival without replaying timed routes"
        );
        assert!(matches!(
            state.tick(10.3, (480, 270), None),
            DeviceTick::Observe(_)
        ));
        assert!(
            state.fishing.phase == Phase::Idle && state.fishing.calibration.is_none(),
            "arrival still requires a fresh tool observation"
        );
        state.frame_expires = Some(10.31 + 0.300);
        assert!(matches!(
            state.tick(10.31, (480, 270), Some(&frame)),
            DeviceTick::Wait
        ));
        assert!(state.fishing.calibration.is_some() && state.resume_after_calibration);
        state.save(state.fishing.tuning).unwrap();
        let mut reloaded = desktop_runtime();
        reloaded.load(path.clone());
        biome::load(previous_biome);
        online::load(previous_online);
        fs::remove_file(path).unwrap();
        assert!(reloaded.settings_error.is_none());
        assert!(reloaded.navigation.options.use_move && reloaded.navigation.options.use_sell);
        let restored = serde_json::to_value(&reloaded.navigation).unwrap();
        for key in ["positions", "fish", "sell"] {
            assert_eq!(restored[key], profile["navigation"][key]);
        }
        assert_eq!(restored["options"]["sellMax"], 37);
        assert!(validate_desktop_entry(&reloaded).is_ok());
    }

    fn desktop_runtime() -> Runtime {
        let mut state = Runtime {
            dua: DuaStatus {
                background: true,
                ..DuaStatus::default()
            },
            capture: true,
            input: true,
            ..Runtime::default()
        };
        let mut config = state.macro_config();
        config["useMove"] = false.into();
        config["useSell"] = false.into();
        state.macro_settings = config;
        state
    }

    #[test]
    fn device_action_never_rescales_a_queued_gesture_after_rotation() {
        let mut state = desktop_runtime();
        state
            .start_device_action(
                device::Gesture::Tap {
                    at: vision::Point(0.2, 0.8),
                    ms: 50,
                },
                (480, 270),
            )
            .unwrap();
        let generation = state.generation;
        assert!(matches!(
            state.tick(0.1, (270, 480), None),
            DeviceTick::Wait
        ));
        assert!(!state.active());
        assert_ne!(state.generation, generation);
        assert!(state.device_action.is_none());
    }

    #[test]
    fn losing_target_cancels_device_action_and_cannot_accept_a_late_success() {
        let mut state = desktop_runtime();
        state
            .start_device_action(
                device::Gesture::Tap {
                    at: vision::Point(0.2, 0.8),
                    ms: 50,
                },
                (480, 270),
            )
            .unwrap();
        let generation = state.generation;
        assert!(matches!(
            state.tick(0.1, (480, 270), None),
            DeviceTick::Input(GameInput::Gesture(_))
        ));
        assert!(matches!(
            state.tick(0.11, (480, 270), None),
            DeviceTick::Wait
        ));
        state.no_target();
        assert!(!state.active());
        assert!(!state.input_current(generation));
        let message = state.message().to_owned();
        state.device_ack();
        assert_eq!(state.message(), message);
    }

    fn calibrated_runtime() -> Runtime {
        let mut state = desktop_runtime();
        state.fishing.calibration = Some(
            serde_json::from_value(serde_json::json!({
                "panel": [110.0/480.0, 95.0/270.0, 370.0/480.0, 181.0/270.0],
                "reel": [100.0/480.0, 95.0/270.0, 380.0/480.0, 181.0/270.0],
                "aspect": 480.0/270.0
            }))
            .unwrap(),
        );
        state
    }

    fn fishing_screen() -> Vec<u8> {
        let mut pixels = vec![0; 480 * 270 * 4];
        let mut put = |x: usize, y: usize, color: [u8; 3]| {
            let offset = (y * 480 + x) * 4;
            pixels[offset..offset + 3].copy_from_slice(&color);
        };
        for i in 0..14 {
            for x in [110 + i, 370 - i] {
                for y in [95, 181] {
                    put(x, y, [248; 3]);
                }
            }
            for x in [110, 370] {
                for y in [95 + i, 181 - i] {
                    put(x, y, [248; 3]);
                }
            }
        }
        for y in 164..168 {
            for x in 170..179 {
                put(x, y, [20, 75, 205]);
            }
        }
        pixels
    }

    fn desktop_navigation() -> Navigation {
        Navigation::desktop(serde_json::from_value(serde_json::json!({
            "movement": {"places": [
                {"key": "fish_spot", "points": [{"pos": [0.4,0.5], "time": 0.1}]},
                {"key": "sell_spot", "points": [{"pos": [0.6,0.5], "time": 0.1}]}
            ]},
            "positions": {
                "chat_pos": [0.1,0.2], "collection_pos": [0.2,0.3], "collection_close": [0.3,0.4],
                "dialog_pos": [0.4,0.5], "sell_fish_pos": [0.5,0.6], "first_fish_pos": [0.6,0.7],
                "sell_all_pos": [0.7,0.8], "confirm_sell_pos": [0.8,0.9], "shop_close_pos": [0.9,0.8]
            },
            "sell_max": 2, "sell_delay": 0.0
        })).unwrap()).unwrap()
    }

    #[test]
    fn dua_automatically_calibrates_then_fishes_and_stop_invalidates_pending_input() {
        let mut state = desktop_runtime();
        let pixels = fishing_screen();
        let frame = vision::Frame::new(&pixels, 480, 270, 1920, 4, false).unwrap();
        state.start_desktop(&["mfish".into()], 0.0).unwrap();
        assert!(state.dua.running && state.automation.is_some());
        assert!(matches!(
            state.tick(0.1, (480, 270), None),
            DeviceTick::Observe(_)
        ));
        assert_eq!(state.dua.screenshot_frames, 0);
        assert!(matches!(
            state.tick(0.1, (480, 270), Some(&frame)),
            DeviceTick::Wait
        ));
        assert!(state.fishing.calibration.is_some() && state.resume_after_calibration);
        assert!(state.task(Task::Base).is_err());
        assert!(matches!(
            state.tick(0.2, (480, 270), None),
            DeviceTick::Observe(_)
        ));
        assert!(!state.resume_after_calibration);
        let DeviceTick::Input(GameInput::Tap(tap)) = state.tick(0.2, (480, 270), Some(&frame))
        else {
            panic!("fresh calibrated Fish must cast");
        };
        assert!((170..179).contains(&tap.x) && (164..168).contains(&tap.y));
        assert_eq!(state.dua.route, Some(Route::Screenshot));
        assert_eq!(state.dua.screenshot_frames, 2);
        let generation = state.generation;
        assert!(state.input_current(generation));
        state.control(Action::Stop).unwrap();
        assert!(
            !state.input_current(generation) && !state.dua.running && state.dua.route.is_none()
        );
        assert!(matches!(
            state.tick(1.0, (480, 270), Some(&frame)),
            DeviceTick::Wait
        ));
        assert!(state.phase() == Phase::Idle);
    }

    #[test]
    fn dua_requests_observed_coast_travel_without_replaying_legacy_timing() {
        let mut state = desktop_runtime();
        state.navigation = desktop_navigation();
        state.macro_settings["useMove"] = true.into();
        state.start_desktop(&["mfish".into()], 0.0).unwrap();
        let generation = state.generation;
        assert!(matches!(
            state.tick(0.1, (480, 270), None),
            DeviceTick::Wait
        ));
        assert!(matches!(
            state.tick(0.2, (480, 270), None),
            DeviceTick::Observe(_)
        ));
        assert!(state.wayfinder.is_some() && state.job.is_none());
        assert!(
            !state.input_current(generation),
            "handoff invalidates the previous tool generation"
        );
        assert!(
            state.fishing.calibration.is_none(),
            "fishing cannot begin before actual coast arrival"
        );
        assert!(matches!(
            state.tick(0.3, (480, 270), None),
            DeviceTick::Observe(_)
        ));
        assert!(state.request_ocr);
        assert_eq!(state.dua.route, Some(Route::Screenshot));
    }

    #[test]
    fn inventory_full_hands_off_to_sale_travel_and_keeps_inputs_generation_bound() {
        let mut state = calibrated_runtime();
        state.macro_settings["useSell"] = true.into();
        state.start_desktop(&["mfish".into()], 0.0).unwrap();
        state
            .automation
            .as_mut()
            .unwrap()
            .trigger(automation::Event::InventoryFull, 0.1);
        assert_eq!(state.automation.as_ref().unwrap().status().queued, 1);
        let generation = state.generation;
        assert!(matches!(
            state.tick(0.2, (480, 270), None),
            DeviceTick::Wait
        ));
        assert!(matches!(
            state.automation.as_ref().unwrap().status().active,
            Some(automation::Job::Sell { test: false })
        ));
        assert!(state.wayfinder.is_some() && state.job.is_none());
        assert!(!state.input_current(generation));
        assert!(
            matches!(state.tick(0.4, (480, 270), None), DeviceTick::Observe(_)),
            "selling must inspect actual ground before moving"
        );
        let generation = state.generation;
        state.control(Action::Stop).unwrap();
        assert!(!state.input_current(generation));
        assert!(state.wayfinder.is_none() && state.automation.is_none());
    }

    #[test]
    fn dua_sale_confirms_on_screen_returns_headless_and_resumes_fishing() {
        let mut state = calibrated_runtime();
        state.navigation = desktop_navigation();
        state.dua.running = true;
        state.job = Some(state.navigation.job(Task::Sell, 0.0, true).unwrap());
        let mut green = false;
        let mut confirmations = 0;
        let mut returned_to_headless = false;
        for step in 0..300 {
            let at = step as f64 * 10.0;
            let tick = state.tick(at, (480, 270), None);
            match tick {
                DeviceTick::Observe(_) => {
                    let mut pixels = vec![0; 480 * 270 * 4];
                    for (point, color) in [
                        (vision::Point(0.5, 0.6), [20, 75, 205]),
                        (vision::Point(0.7, 0.8), [20, 75, 205]),
                        (
                            vision::Point(0.8, 0.9),
                            if green { [20, 180, 20] } else { [0; 3] },
                        ),
                    ] {
                        let (x, y) = ((point.0 * 480.0) as usize, (point.1 * 270.0) as usize);
                        for yy in y - 6..y + 6 {
                            for xx in x - 10..x + 10 {
                                let offset = (yy * 480 + xx) * 4;
                                pixels[offset..offset + 3].copy_from_slice(&color);
                            }
                        }
                    }
                    let frame = vision::Frame::new(&pixels, 480, 270, 1920, 4, false).unwrap();
                    assert!(matches!(
                        state.tick(at, (480, 270), Some(&frame)),
                        DeviceTick::Wait
                    ));
                    assert_eq!(state.dua.route, Some(Route::Screenshot));
                }
                DeviceTick::Input(GameInput::Motion(motion)) => {
                    assert_eq!(state.dua.route, Some(Route::Headless));
                    returned_to_headless |= state.dua.screenshot_frames > 0;
                    if let Motion::Tap(point, _) = motion {
                        if point.0 == 0.7 && point.1 == 0.8 {
                            green = true;
                        }
                        if point.0 == 0.8 && point.1 == 0.9 {
                            assert!(green, "confirmation must follow Sell All");
                            green = false;
                            confirmations += 1;
                        }
                    }
                }
                DeviceTick::Wait => {}
                _ => panic!("sale must use structured game input"),
            }
            if state.job.is_none() {
                break;
            }
        }
        assert_eq!(confirmations, 2);
        assert!(returned_to_headless && state.dua.screenshot_frames >= 8);
        assert!(state.job.is_none());
        assert!(state.dua.running && state.phase() == Phase::Searching);
        assert!(matches!(
            state.tick(3001.0, (480, 270), None),
            DeviceTick::Observe(_)
        ));
    }

    #[test]
    fn losing_target_invalidates_travel_and_keeps_only_the_recovery_runner() {
        let mut state = desktop_runtime();
        state.macro_settings["useMove"] = true.into();
        let at = now();
        state.start_desktop(&["mfish".into()], at).unwrap();
        let pixels = fishing_screen();
        let frame = vision::Frame::new(&pixels, 480, 270, 1920, 4, false).unwrap();
        state.tick(at, (480, 270), Some(&frame));
        assert!(state.wayfinder.is_some());
        let generation = state.generation;
        state.no_target();
        assert!(state.dua.running && state.automation.is_some());
        assert!(state.wayfinder.is_none() && state.job.is_none());
        assert!(!state.input_current(generation));
        assert_eq!(state.dua.route, Some(Route::Headless));
        state.stop("test recovery canceled");
        assert!(!state.active());
    }

    #[test]
    fn dua_supports_all_seven_features_and_failed_calibration_never_starts_fishing() {
        for feature in [
            "mfish", "mpop", "mitem", "mblood", "mmerch", "mcraft", "mmatch",
        ] {
            let mut state = desktop_runtime();
            state.start_desktop(&[feature.into()], 0.0).unwrap();
            assert!(state.dua.running && state.automation.is_some());
            state.stop("next feature");
            assert!(!state.active());
        }
        let mut state = desktop_runtime();
        assert!(state.start_desktop(&[], 0.0).is_err());
        assert!(state.start_desktop(&["unknown".into()], 0.0).is_err());
        assert!(!state.active());
        state.start_desktop(&["mfish".into()], 0.0).unwrap();
        let pixels = vec![0; 480 * 270 * 4];
        let frame = vision::Frame::new(&pixels, 480, 270, 1920, 4, false).unwrap();
        state.tick(0.1, (480, 270), Some(&frame));
        assert!(state.fishing.phase == Phase::Calibrating);
        state.tick(31.0, (480, 270), Some(&frame));
        state.tick(31.1, (480, 270), None);
        assert!(
            !state.dua.running
                && !state.active()
                && state.fishing.calibration.is_none()
                && !state.resume_after_calibration
        );
    }

    #[test]
    fn dua_observed_travel_resize_stops_and_restart_rejects_old_generation() {
        let mut state = desktop_runtime();
        state.macro_settings["useMove"] = true.into();
        let pixels = fishing_screen();
        let frame = vision::Frame::new(&pixels, 480, 270, 1920, 4, false).unwrap();
        state.start_desktop(&["mfish".into()], 0.0).unwrap();
        state.tick(0.0, (480, 270), Some(&frame));
        assert!(state.wayfinder.is_some());
        assert!(matches!(
            state.tick(0.01, (480, 270), None),
            DeviceTick::Observe(_)
        ));
        let generation = state.generation;
        state.tick(0.02, (640, 360), None);
        assert!(!state.active() && !state.input_current(generation));
        state.start_desktop(&["mfish".into()], 1.0).unwrap();
        assert!(!state.input_current(generation));
        state.control(Action::Stop).unwrap();
        state.input_rejected("late input failure");
        assert!(!state.active());
    }

    #[test]
    fn input_failures_retry_sales_and_relocation_but_respect_stop() {
        let mut positions = std::collections::BTreeMap::new();
        for key in navigation::BASE.into_iter().chain(navigation::SELL).chain([
            "stick_center",
            "stick_edge",
            "talk_btn",
        ]) {
            positions.insert(key, vision::Point(0.2, 0.7));
        }
        positions.insert("stick_edge", vision::Point(0.25, 0.7));
        let navigation: Navigation = serde_json::from_value(serde_json::json!({
            "positions": positions,
            "fish": [{"x": 0, "y": -1, "ms": 500, "jump": false}],
            "sell": [{"x": 0, "y": -1, "ms": 500, "jump": false}]
        }))
        .unwrap();
        let mut state = Runtime::default();
        state.job = Some(navigation.job(Task::Sell, 0.0, true).unwrap());
        state.input_rejected("입력 취소");
        assert!(matches!(state.phase(), Phase::Selling));
        assert!(!state.input_current(0));
        state.input_rejected("입력 취소");
        assert!(matches!(state.phase(), Phase::Selling));
        state.input_rejected("입력 취소");
        assert!(matches!(state.phase(), Phase::Idle));

        state.fishing.calibration = Some(
            serde_json::from_value(serde_json::json!({
                "panel": [0.2, 0.2, 0.8, 0.8],
                "reel": [0.1, 0.2, 0.9, 0.8],
                "aspect": 1.5
            }))
            .unwrap(),
        );
        state.relocations = 1;
        state.job = Some(navigation.job(Task::Fish, 0.0, true).unwrap());
        state.input_rejected("입력 취소");
        assert!(matches!(state.phase(), Phase::Searching));
        assert_eq!(state.relocations, 1);
        state.control(Action::Stop).unwrap();
        state.input_rejected("늦게 도착한 입력 오류");
        assert!(matches!(state.phase(), Phase::Idle));
        assert_eq!(state.message(), "중지됨");
    }
    #[test]
    fn disabled_fishing_recovery_leaves_other_features_running() {
        for event in [automation::Event::InventoryFull, automation::Event::Coast] {
            let mut state = desktop_runtime();
            state
                .start_desktop(&["mfish".into(), "mmatch".into()], 0.0)
                .unwrap();
            state.automation.as_mut().unwrap().trigger(event, 0.1);
            assert_eq!(state.automation.as_ref().unwrap().status().queued, 0);
            let pixels = fishing_screen();
            let frame = vision::Frame::new(&pixels, 480, 270, 1920, 4, false).unwrap();
            state.tick(0.2, (480, 270), Some(&frame));
            assert!(state.dua.running && state.automation.is_some());
            assert!(state.fishing.phase == Phase::Idle && state.job.is_none());
            assert!(
                matches!(
                    state.automation.as_ref().unwrap().status().active,
                    Some(automation::Job::Match { .. })
                ) && state.wayfinder.is_some(),
                "the unrelated memory-match feature still starts its own journey"
            );
            assert!(
                !state.resume_after_calibration,
                "disabled fishing cannot restart while another feature waits"
            );
            assert!(
                state.log.iter().any(|(_, line)| line.contains("꺼져")),
                "the disabled recovery reason is retained in the app log"
            );
        }
    }
}

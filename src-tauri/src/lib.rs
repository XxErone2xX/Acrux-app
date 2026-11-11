#[cfg(target_os = "android")]
mod android;
mod biome;
mod fishing;
mod navigation;
mod online;
mod vision;
#[cfg(target_os = "windows")]
mod windows;

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
    log: VecDeque<&'static str>,
    logged: Option<&'static str>,
    navigation: Navigation,
    job: Option<Job>,
    empty_sales: u8,
    relocations: u8,
    measured_seconds: Option<f64>,
    generation: i32,
    dua: DuaStatus,
    resume_after_calibration: bool,
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
}

impl Default for DuaStatus {
    fn default() -> Self {
        Self {
            supported: cfg!(target_os = "windows"),
            running: false,
            route: None,
            reason: "DUA 대기 중",
            headless_ticks: 0,
            screenshot_frames: 0,
            transitions: 0,
        }
    }
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
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Status {
    platform: &'static str,
    version: &'static str,
    capture_ready: bool,
    input_ready: bool,
    game_available: bool,
    calibrated: bool,
    positions: Option<CalibrationPositions>,
    phase: Phase,
    message: &'static str,
    settings_error: Option<String>,
    stats: Stats,
    totals: Stats,
    tuning: Tuning,
    measuring_elapsed: Option<f64>,
    measured_seconds: Option<f64>,
    dua: DuaStatus,
    generation: i32,
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
}

enum DeviceTick {
    Input(GameInput),
    Wait,
    Observe,
}

impl Runtime {
    fn status(&self) -> Status {
        Status {
            platform: std::env::consts::OS,
            version: env!("CARGO_PKG_VERSION"),
            capture_ready: self.capture,
            input_ready: self.input,
            game_available: game_available(),
            calibrated: self.fishing.calibration.is_some(),
            positions: self.fishing.calibration.map(Calibration::positions),
            phase: self.phase(),
            message: self.message(),
            settings_error: self.settings_error.clone(),
            stats: self.fishing.stats,
            totals: self.totals,
            tuning: self.fishing.tuning,
            measuring_elapsed: self.measuring_since().map(|start| now() - start),
            measured_seconds: self.measured_seconds,
            dua: self.dua,
            generation: self.generation,
        }
    }

    fn load(&mut self, path: PathBuf) {
        match fs::read(&path) {
            Ok(bytes) => match serde_json::from_slice::<Profile>(&bytes) {
                Ok(profile) => {
                    biome::load(profile.biome);
                    online::load(profile.online);
                    self.fishing.calibration = profile.calibration;
                    self.totals = profile.totals;
                    self.navigation = profile.navigation;
                    self.navigation.fit_positions();
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
            navigation: if cfg!(target_os = "windows") {
                Navigation::default()
            } else {
                self.navigation.clone()
            },
        })
        .map_err(|error| error.to_string())?;
        let temporary = path.with_extension("tmp");
        fs::write(&temporary, data)
            .and_then(|_| fs::rename(&temporary, path))
            .map_err(|error| format!("설정을 저장하지 못했습니다. {error}"))
    }

    #[cfg(target_os = "android")]
    fn frame(&mut self, frame: &vision::Frame<'_>) -> Option<GameInput> {
        match self.tick(now(), (frame.width, frame.height), Some(frame)) {
            DeviceTick::Input(input) => Some(input),
            DeviceTick::Wait | DeviceTick::Observe => None,
        }
    }

    fn tick(
        &mut self,
        at: f64,
        geometry: (usize, usize),
        frame: Option<&vision::Frame<'_>>,
    ) -> DeviceTick {
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
                return DeviceTick::Observe;
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
        let reason = if self.fishing.phase == Phase::Calibrating {
            "낚시 창의 위치를 화면에서 보정합니다."
        } else {
            "던지기 · 입질 · 릴링 · 결과를 화면에서 확인합니다."
        };
        self.route(Route::Screenshot, reason, frame.is_some());
        let Some(frame) = frame else {
            return DeviceTick::Observe;
        };
        let before = self.fishing.stats;
        let tap = self.fishing.frame(at, frame);
        let changed = before != self.fishing.stats;
        if changed {
            self.totals.record_delta(before, self.fishing.stats);
        }
        if self.navigation.options.use_sell && self.fishing.stats.inventory_increased(before) {
            match self.navigation.job(Task::Sell, at, true) {
                Ok(job) => {
                    self.generation = self.generation.wrapping_add(1);
                    self.job = Some(job);
                }
                Err(error) => self.stop(error),
            }
        }
        if self.navigation.options.use_move && self.fishing.needs_relocation(at) {
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
        if std::mem::take(&mut self.fishing.calibration_changed) || changed {
            self.settings_error = self.save(self.fishing.tuning).err();
        }
        if self.fishing.phase == Phase::Idle && self.job.is_none() && !self.resume_after_calibration
        {
            self.stop(self.fishing.message);
        }
        // A tool handoff invalidates input computed by the preceding tool.
        if self.job.is_some() || !self.fishing.phase.active() {
            DeviceTick::Wait
        } else {
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
        self.dua.running || self.phase().active()
    }

    fn start_desktop(&mut self, features: &[String], at: f64) -> Result<(), &'static str> {
        if features.is_empty() {
            return Err("실행할 매크로 기능을 켜 주세요.");
        }
        if features.iter().any(|feature| feature != "mfish") {
            return Err("현재 데스크톱 DUA는 자동 낚시 · 이동 · 판매를 지원합니다. 다른 기능은 이식 전입니다.");
        }
        if self.active() {
            return Err("실행 중인 작업을 먼저 중지하세요.");
        }
        if !self.capture || !self.input {
            return Err("화면 캡처와 입력을 먼저 준비하세요.");
        }
        // Construct the first job before committing the session so invalid routes cannot half-start.
        let job = if self.navigation.options.use_move {
            Some(self.navigation.job(Task::Fish, at, true)?)
        } else {
            None
        };
        self.dua = DuaStatus {
            running: true,
            ..DuaStatus::default()
        };
        self.generation = self.generation.wrapping_add(1);
        self.empty_sales = 0;
        self.relocations = 0;
        self.route(Route::Headless, "설정과 실행할 작업을 확인합니다.", false);
        self.job = job;
        if self.job.is_none() {
            if let Err(error) = self.begin_fishing(at) {
                self.stop(error);
                return Err(error);
            }
        }
        self.record();
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

    fn phase(&self) -> Phase {
        match self.job.as_ref() {
            Some(job) if job.recording_at.is_some() && job.action != Task::Measure => {
                Phase::Recording
            }
            Some(job) if job.action == Task::Sell => Phase::Selling,
            Some(_) => Phase::Moving,
            None => self.fishing.phase,
        }
    }

    fn message(&self) -> &'static str {
        self.job.as_ref().map_or(self.fishing.message, Job::message)
    }

    fn stop(&mut self, message: &'static str) {
        self.dua.running = false;
        self.dua.route = None;
        self.dua.reason = message;
        self.resume_after_calibration = false;
        self.finish_tool(message);
    }

    fn finish_tool(&mut self, message: &'static str) {
        self.generation = self.generation.wrapping_add(1);
        self.job = None;
        self.fishing.stop(message);
    }

    fn input_rejected(&mut self, message: &'static str) {
        if !self.phase().active() {
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

    fn input_current(&self, generation: i32) -> bool {
        self.generation == generation && self.active()
    }

    fn record(&mut self) {
        let message = self.message();
        if self.logged == Some(message) {
            return;
        }
        if self.log.len() == 300 {
            self.log.pop_front();
        }
        self.log.push_back(message);
        self.logged = Some(message);
    }

    fn control(&mut self, action: Action) -> Result<(), String> {
        if action == Action::Stop {
            self.stop("중지됨");
            return Ok(());
        }
        #[cfg(target_os = "windows")]
        if action == Action::Start {
            return self
                .start_desktop(&["mfish".into()], now())
                .map_err(str::to_owned);
        }
        if self.dua.running {
            return Err("실행 중인 DUA를 먼저 중지하세요.".into());
        }
        if !self.capture || !self.input {
            return Err("화면 캡처와 입력을 먼저 준비하세요.".into());
        }
        if self.job.is_some() {
            return Err("실행 중인 작업을 먼저 중지하세요.".into());
        }
        if action == Action::Start {
            self.empty_sales = 0;
            self.relocations = 0;
        }
        if action == Action::Start && self.navigation.options.use_move {
            if self.fishing.calibration.is_none() {
                return Err("낚시 자동 보정을 먼저 실행하세요.".into());
            }
            self.job = Some(self.navigation.job(Task::Fish, now(), true)?);
            return Ok(());
        }
        match action {
            Action::Calibrate => self.fishing.calibrate(now()),
            Action::Start => self.fishing.start(now()),
            Action::Stop => unreachable!(),
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

    fn task(&mut self, task: Task) -> Result<(), &'static str> {
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
    #[cfg(target_os = "windows")]
    return windows::game_available();
    #[cfg(not(target_os = "windows"))]
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
    state.control(action)?;
    Ok(state.status())
}

#[tauri::command]
fn desktop_start(features: Vec<String>, generation: i32) -> Result<Status, String> {
    if !cfg!(target_os = "windows") {
        return Err("데스크톱 DUA의 게임 캡처 · 입력은 현재 Windows에서 지원합니다.".into());
    }
    let mut state = runtime();
    if state.generation != generation {
        return Err("이전 실행 요청이 취소됐습니다. 다시 시작하세요.".into());
    }
    state.start_desktop(&features, now())?;
    Ok(state.status())
}

#[tauri::command]
fn save_tuning(tuning: Tuning) -> Result<Status, String> {
    let mut state = runtime();
    if state.active() {
        return Err("실행 중인 작업을 먼저 중지하세요.".into());
    }
    let tuning = tuning.validate()?;
    state.save(tuning)?;
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
    state.edit_navigation(|nav| {
        nav.options = options;
        Ok(())
    })
}

#[tauri::command]
fn desktop_navigation(setup: navigation::DesktopSetup) -> Result<(), String> {
    if !cfg!(target_os = "windows") {
        return Err("Windows에서만 게임 입력을 사용할 수 있습니다.".into());
    }
    let mut state = runtime();
    if state.active() {
        return Err("실행 중인 작업을 먼저 중지하세요.".into());
    }
    state.navigation = Navigation::desktop(setup)?;
    Ok(())
}

#[tauri::command]
fn desktop_task(task: Task) -> Result<(), String> {
    if !cfg!(target_os = "windows") {
        return Err("Windows에서만 게임 입력을 사용할 수 있습니다.".into());
    }
    runtime().task(task).map_err(str::to_owned)
}

#[tauri::command]
fn desktop_waypoint(
    place: Place,
    index: usize,
    measure: bool,
    from_base: bool,
) -> Result<(), String> {
    if !cfg!(target_os = "windows") {
        return Err("Windows에서만 게임 입력을 사용할 수 있습니다.".into());
    }
    let mut state = runtime();
    if state.active() {
        return Err("실행 중인 작업을 먼저 중지하세요.".into());
    }
    state.job = Some(
        state
            .navigation
            .desktop_waypoint(place, index, measure, from_base, now())?,
    );
    state.measured_seconds = None;
    Ok(())
}

#[tauri::command]
fn desktop_arrive() -> Result<(), String> {
    if runtime().arrive_desktop() {
        Ok(())
    } else {
        Err("이동 시간을 재는 중에 눌러 주세요.".into())
    }
}

#[tauri::command]
async fn desktop_pick() -> Result<vision::Point, String> {
    #[cfg(target_os = "windows")]
    return tauri::async_runtime::spawn_blocking(windows::pick)
        .await
        .map_err(|e| e.to_string())?;
    #[cfg(not(target_os = "windows"))]
    Err("Windows에서만 화면 위치를 지정할 수 있습니다.".into())
}

#[tauri::command]
fn desktop_cancel_pick() {
    #[cfg(target_os = "windows")]
    windows::cancel_pick();
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
        .map(|line| (*line).into())
        .collect();
    lines.extend(biome::status().logs);
    lines.truncate(300);
    lines
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
    #[cfg(any(target_os = "windows", target_os = "android"))]
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default();
    #[cfg(target_os = "android")]
    let builder = builder.plugin(android::plugin());
    builder
        .setup(|app| {
            let dir = app.path().app_config_dir()?;
            fs::create_dir_all(&dir)?;
            runtime().load(dir.join("profile.json"));
            {
                let mut state = runtime();
                if state.settings_error.is_none() {
                    state.settings_error = state.save(state.fishing.tuning).err();
                }
            }
            #[cfg(target_os = "windows")]
            biome::watch_windows();
            #[cfg(target_os = "windows")]
            windows::start()?;
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
            control,
            desktop_start,
            save_tuning,
            android_setup,
            reset,
            fishing_log,
            navigation_status,
            save_navigation,
            clear_navigation,
            desktop_navigation,
            desktop_task,
            desktop_waypoint,
            desktop_arrive,
            desktop_pick,
            desktop_cancel_pick
        ])
        .run(tauri::generate_context!())
        .expect("could not launch Acrux");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_os = "windows")]
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

    fn desktop_runtime() -> Runtime {
        Runtime {
            capture: true,
            input: true,
            ..Runtime::default()
        }
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
        assert!(state.dua.running && state.resume_after_calibration);
        assert!(matches!(
            state.tick(0.1, (480, 270), None),
            DeviceTick::Observe
        ));
        assert_eq!(state.dua.screenshot_frames, 0);
        assert!(matches!(
            state.tick(0.1, (480, 270), Some(&frame)),
            DeviceTick::Wait
        ));
        assert!(state.fishing.calibration.is_some());
        assert!(state.dua.running && state.phase() == Phase::Idle);
        assert!(state.task(Task::Base).is_err());
        state.no_target();
        assert!(state.dua.running && state.resume_after_calibration);
        assert!(matches!(
            state.tick(0.2, (480, 270), None),
            DeviceTick::Observe
        ));
        assert!(!state.resume_after_calibration);
        let DeviceTick::Input(GameInput::Tap(tap)) = state.tick(0.2, (480, 270), Some(&frame))
        else {
            panic!("calibrated DUA must cast using the screenshot");
        };
        assert!((170..179).contains(&tap.x) && (164..168).contains(&tap.y));
        assert_eq!(state.dua.route, Some(Route::Screenshot));
        assert_eq!(state.dua.screenshot_frames, 2);
        let generation = state.generation;
        assert!(state.input_current(generation));
        state.control(Action::Stop).unwrap();
        assert!(!state.input_current(generation));
        assert!(!state.dua.running && state.dua.route.is_none());
        assert!(matches!(
            state.tick(1.0, (480, 270), Some(&frame)),
            DeviceTick::Wait
        ));
        assert!(state.phase() == Phase::Idle);
    }

    #[test]
    fn dua_routes_movement_to_headless_and_escalates_to_screenshot_for_fishing() {
        let mut state = desktop_runtime();
        state.navigation = desktop_navigation();
        state.start_desktop(&["mfish".into()], 0.0).unwrap();
        let mut inputs = 0;
        for step in 0..100 {
            match state.tick(step as f64 * 10.0, (480, 270), None) {
                DeviceTick::Input(GameInput::Motion(_)) => inputs += 1,
                DeviceTick::Observe => break,
                DeviceTick::Wait => {}
                _ => panic!("movement must not produce a screenshot-dependent fishing tap"),
            }
        }
        assert!(inputs > 10);
        assert!(state.dua.running && state.phase() == Phase::Calibrating);
        assert_eq!(state.dua.route, Some(Route::Screenshot));
        assert_eq!(state.dua.screenshot_frames, 0);
        assert!(state.dua.headless_ticks > 10 && state.dua.transitions >= 2);
    }

    #[test]
    fn dua_inventory_full_hands_off_to_headless_sale_and_requires_visual_confirmation() {
        let mut state = calibrated_runtime();
        state.navigation = desktop_navigation();
        state.navigation.options.use_move = false;
        state.start_desktop(&["mfish".into()], 0.0).unwrap();
        let pixels = fishing_screen();
        let frame = vision::Frame::new(&pixels, 480, 270, 1920, 4, false).unwrap();
        for at in [0.1, 3.2, 6.3, 9.4] {
            state.tick(at, (480, 270), Some(&frame));
        }
        assert!(state
            .job
            .as_ref()
            .is_some_and(|job| job.action == Task::Sell));
        assert!(state.dua.running);
        let generation = state.generation;
        let before = state.dua.screenshot_frames;
        let mut inputs = 0;
        for step in 1..100 {
            match state.tick(10.0 + step as f64 * 10.0, (480, 270), None) {
                DeviceTick::Input(GameInput::Motion(_)) => inputs += 1,
                DeviceTick::Observe => break,
                DeviceTick::Wait => {}
                _ => panic!("sale movement requires structured commands"),
            }
        }
        assert!(inputs > 10);
        assert_eq!(state.dua.screenshot_frames, before);
        assert_eq!(state.dua.route, Some(Route::Screenshot));
        assert!(state
            .job
            .as_ref()
            .is_some_and(|job| job.action == Task::Sell));
        state.control(Action::Stop).unwrap();
        assert!(!state.input_current(generation));
        assert!(state.job.is_none());
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
                DeviceTick::Observe => {
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
            DeviceTick::Observe
        ));
    }

    #[test]
    fn losing_the_target_during_headless_movement_cancels_the_session() {
        let mut state = desktop_runtime();
        state.navigation = desktop_navigation();
        state.start_desktop(&["mfish".into()], 0.0).unwrap();
        assert!(matches!(
            state.tick(0.0, (480, 270), None),
            DeviceTick::Input(_)
        ));
        let generation = state.generation;
        state.no_target();
        assert!(!state.active() && !state.input_current(generation));
        assert!(matches!(
            state.tick(0.1, (480, 270), None),
            DeviceTick::Wait
        ));
    }

    #[test]
    fn dua_refuses_unsupported_features_and_failed_calibration_does_not_start_fishing() {
        let mut state = desktop_runtime();
        assert!(state.start_desktop(&[], 0.0).is_err());
        assert!(state
            .start_desktop(&["mfish".into(), "mitem".into()], 0.0)
            .is_err());
        assert!(!state.active());
        state.start_desktop(&["mfish".into()], 0.0).unwrap();
        let pixels = vec![0; 480 * 270 * 4];
        let frame = vision::Frame::new(&pixels, 480, 270, 1920, 4, false).unwrap();
        state.tick(31.0, (480, 270), Some(&frame));
        state.tick(31.1, (480, 270), None);
        assert!(!state.dua.running && !state.active());
        assert!(state.fishing.calibration.is_none());
        assert!(!state.resume_after_calibration);
    }

    #[test]
    fn dua_movement_resize_stops_and_restart_rejects_old_generation() {
        let mut state = desktop_runtime();
        state.navigation = desktop_navigation();
        state.start_desktop(&["mfish".into()], 0.0).unwrap();
        assert!(matches!(
            state.tick(0.0, (480, 270), None),
            DeviceTick::Input(_)
        ));
        let generation = state.generation;
        state.tick(0.01, (640, 360), None);
        assert!(!state.active() && !state.input_current(generation));
        state.start_desktop(&["mfish".into()], 1.0).unwrap();
        assert!(!state.input_current(generation));
        state.control(Action::Stop).unwrap();
        state.input_rejected("늦은 입력 실패");
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
}

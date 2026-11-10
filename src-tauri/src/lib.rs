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
    measured_seconds: Option<f64>,
    generation: i32,
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

    #[cfg(any(target_os = "windows", target_os = "android"))]
    fn frame(&mut self, frame: &vision::Frame<'_>) -> Option<GameInput> {
        if let Some(job) = self.job.as_mut() {
            let tick = job.tick(now(), frame);
            let input = match tick {
                Tick::Input(motion) => Some(GameInput::Motion(motion)),
                Tick::Wait => None,
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
                        return None;
                    }
                    self.stop(message);
                    if resume {
                        if let Err(error) = self.fishing.start(now()) {
                            self.stop(error);
                        }
                    }
                    None
                }
                Tick::Failed(error) => {
                    self.stop(error);
                    None
                }
            };
            self.record();
            return input;
        }
        let before = self.fishing.stats;
        let tap = self.fishing.frame(now(), frame);
        let changed = before != self.fishing.stats;
        if changed {
            self.totals.record_delta(before, self.fishing.stats);
        }
        if self.navigation.options.use_sell && self.fishing.stats.inventory_increased(before) {
            match self.navigation.job(Task::Sell, now(), true) {
                Ok(job) => self.job = Some(job),
                Err(error) => self.stop(error),
            }
        }
        self.record();
        if std::mem::take(&mut self.fishing.calibration_changed) || changed {
            self.settings_error = self.save(self.fishing.tuning).err();
        }
        tap.map(GameInput::Tap)
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
        self.generation = self.generation.wrapping_add(1);
        self.job = None;
        self.fishing.stop(message);
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
            self.fishing.no_target(now());
        }
    }

    fn input_current(&self, generation: i32) -> bool {
        self.generation == generation && self.phase().active()
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
        if !self.capture || !self.input {
            return Err("화면 캡처와 입력을 먼저 준비하세요.".into());
        }
        if self.job.is_some() {
            return Err("실행 중인 작업을 먼저 중지하세요.".into());
        }
        if action == Action::Start {
            self.empty_sales = 0;
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
        if self.phase().active() {
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
fn save_tuning(tuning: Tuning) -> Result<Status, String> {
    let mut state = runtime();
    if state.phase().active() {
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
    if state.phase().active() {
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
    if state.phase().active() {
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
    if state.phase().active() {
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
    if state.phase().active() {
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

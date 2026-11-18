mod activities;
#[cfg(target_os = "android")]
mod android;
mod biome;
mod coordinator;
mod fishing;
mod items;
#[cfg(target_os = "linux")]
mod linux;
mod macro_step;
mod navigation;
mod ocr;
mod online;
mod vision;
#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "linux")]
use linux as desktop;
#[cfg(target_os = "windows")]
use windows as desktop;

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
    log: VecDeque<String>,
    logged: Option<String>,
    navigation: Navigation,
    job: Option<Job>,
    empty_sales: u8,
    measured_seconds: Option<f64>,
    generation: i32,
    macro_settings: serde_json::Value,
    coordinator: coordinator::Coordinator,
    fish_retry_at: f64,
    fish_backoff: f64,
    fish_started: bool,
    background_ocr: Option<u64>,
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
    macro_settings: serde_json::Value,
}

#[derive(Serialize)]
struct RecordingStatus {
    place: usize,
    elapsed: f64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Status {
    platform: &'static str,
    version: &'static str,
    generation: i32,
    recording: Option<RecordingStatus>,
    capture_ready: bool,
    input_ready: bool,
    game_available: bool,
    connection_message: String,
    calibrated: bool,
    positions: Option<CalibrationPositions>,
    phase: Phase,
    message: String,
    macro_running: bool,
    feature_statuses: serde_json::Value,
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
    Text { text: String, submit: bool },
}

#[cfg(any(target_os = "windows", target_os = "linux"))]
struct BackgroundOcr {
    request: coordinator::Observation,
    generation: i32,
    width: usize,
    height: usize,
    pixels: Vec<u8>,
    words: Result<Vec<ocr::Word>, String>,
}
#[cfg(any(target_os = "windows", target_os = "linux"))]
static OCR_INBOX: LazyLock<Mutex<Option<BackgroundOcr>>> = LazyLock::new(|| Mutex::new(None));
#[cfg(any(target_os = "windows", target_os = "linux"))]
static OCR_BUSY: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// The platform worker holds no Runtime lock while doing native OCR.
#[cfg(any(target_os = "windows", target_os = "linux"))]
fn desktop_frame(frame: &vision::Frame<'_>) -> Option<GameInput> {
    let request = {
        let state = runtime();
        state
            .coordinator
            .observation()
            .map(|request| (request, state.generation))
    };
    if let Some((request, generation)) = request {
        if request.background {
            use std::sync::atomic::Ordering;
            if !OCR_BUSY.swap(true, Ordering::AcqRel) {
                let mut pixels = Vec::with_capacity(frame.width * frame.height * 3);
                for y in 0..frame.height {
                    for x in 0..frame.width {
                        pixels.extend_from_slice(&frame.pixel(x, y));
                    }
                }
                runtime().background_ocr = Some(request.token);
                let (width, height) = (frame.width, frame.height);
                if let Err(error) = std::thread::Builder::new()
                    .name("acrux-feature-ocr".into())
                    .spawn(move || {
                        let words = vision::Frame::new(&pixels, width, height, width * 3, 3, false)
                            .ok_or_else(|| "OCR 화면을 읽지 못했습니다.".into())
                            .and_then(|frame| {
                                ocr::read_with_options(
                                    &frame,
                                    request.region,
                                    request.scale,
                                    request.pad,
                                )
                            });
                        *OCR_INBOX.lock().unwrap_or_else(|e| e.into_inner()) =
                            Some(BackgroundOcr {
                                request,
                                generation,
                                width,
                                height,
                                pixels,
                                words,
                            });
                        OCR_BUSY.store(false, Ordering::Release);
                    })
                {
                    OCR_BUSY.store(false, Ordering::Release);
                    let mut state = runtime();
                    state.background_ocr = None;
                    let config = state.macro_config();
                    let step = state.coordinator.observed(
                        request.token,
                        &config,
                        now(),
                        frame,
                        Err(format!("OCR 작업을 시작하지 못했습니다: {error}")),
                    );
                    let input = step.and_then(|step| state.macro_step(step));
                    state.macro_updates();
                    return input;
                }
            }
            return runtime().frame(frame);
        }
        let words = if request.ocr {
            ocr::read_with_options(frame, request.region, request.scale, request.pad)
        } else {
            Ok(Vec::new())
        };
        let mut state = runtime();
        if state.generation != generation {
            return None;
        }
        let config = state.macro_config();
        let step = state
            .coordinator
            .observed(request.token, &config, now(), frame, words);
        let input = step.and_then(|step| state.macro_step(step));
        state.macro_updates();
        input
    } else {
        runtime().frame(frame)
    }
}

#[cfg(any(target_os = "windows", target_os = "linux"))]
fn desktop_input_ack(generation: i32, completed: bool) {
    let mut state = runtime();
    if state.generation == generation {
        state.input_ack(completed, now());
    }
}

impl Runtime {
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    fn deliver_background_ocr(&mut self) -> Option<GameInput> {
        let Some(result) = OCR_INBOX.lock().unwrap_or_else(|e| e.into_inner()).take() else {
            return None;
        };
        if self.background_ocr == Some(result.request.token) {
            self.background_ocr = None;
        }
        if self.generation != result.generation {
            return None;
        }
        let frame = vision::Frame::new(
            &result.pixels,
            result.width,
            result.height,
            result.width * 3,
            3,
            false,
        )?;
        let config = self.macro_config();
        let step =
            self.coordinator
                .observed(result.request.token, &config, now(), &frame, result.words);
        let input = step.and_then(|step| self.macro_step(step));
        self.macro_updates();
        input
    }
    fn status(&self) -> Status {
        Status {
            platform: std::env::consts::OS,
            version: env!("CARGO_PKG_VERSION"),
            generation: self.generation,
            recording: self
                .job
                .as_ref()
                .and_then(|job| job.recording_status(now()))
                .map(|(place, elapsed)| RecordingStatus { place, elapsed }),
            capture_ready: self.capture,
            input_ready: self.input,
            game_available: game_available(),
            connection_message: {
                #[cfg(target_os = "linux")]
                {
                    linux::connection_message()
                }
                #[cfg(not(target_os = "linux"))]
                {
                    if game_available() {
                        "Roblox 연결됨"
                    } else {
                        "Roblox 창을 열어 주세요."
                    }
                    .into()
                }
            },
            calibrated: self.fishing.calibration.is_some(),
            positions: self.fishing.calibration.map(Calibration::positions),
            phase: self.phase(),
            message: self.message(),
            macro_running: self.coordinator.running,
            feature_statuses: {
                let config = self.macro_config();
                let mut statuses = self.coordinator.statuses(&config, now());
                if !statuses["mfish"]["running"].as_bool().unwrap_or(false) {
                    statuses["mfish"]["running"] = self.fishing.phase.active().into();
                    statuses["mfish"]["msg"] = self.fishing.message.into();
                    statuses["mfish"]["stats"] =
                        serde_json::to_value(self.fishing.stats).unwrap_or_default();
                }
                statuses
            },
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
                    self.macro_settings = profile.macro_settings;
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
        self.persist_profile(&Profile {
            biome,
            online: online::settings(),
            calibration: self.fishing.calibration,
            tuning,
            totals: self.totals,
            macro_settings: self.macro_settings.clone(),
            navigation: if cfg!(any(target_os = "windows", target_os = "linux")) {
                Navigation::default()
            } else {
                self.navigation.clone()
            },
        })
    }

    fn persist_profile(&self, profile: &Profile) -> Result<(), String> {
        let path = self
            .settings
            .as_ref()
            .ok_or("설정 저장소가 준비되지 않았습니다.")?;
        let data = serde_json::to_vec_pretty(profile).map_err(|error| error.to_string())?;
        let temporary = path.with_extension("tmp");
        fs::write(&temporary, data)
            .and_then(|_| fs::rename(&temporary, path))
            .map_err(|error| format!("설정을 저장하지 못했습니다. {error}"))
    }

    #[cfg(any(target_os = "windows", target_os = "linux", target_os = "android"))]
    fn frame(&mut self, frame: &vision::Frame<'_>) -> Option<GameInput> {
        if let Some(job) = self.job.as_mut() {
            let tick = job.tick(now(), frame);
            return self.navigation_tick(tick);
        }
        if self.coordinator.has_job() && self.coordinator.is_ready() {
            return None;
        }
        if self.macro_settings["mfish"].is_object() {
            if let Err(error) = self
                .fishing
                .configure_positions(&self.macro_settings["mfish"], (frame.width, frame.height))
            {
                self.stop(error);
                self.record();
                return None;
            }
        }
        let before = self.fishing.stats;
        let was_fishing = self.fishing.phase;
        let tap = self.fishing.frame(now(), frame);
        let changed = before != self.fishing.stats;
        if changed {
            self.totals.record_delta(before, self.fishing.stats);
            if !self.fishing.stats.inventory_increased(before) {
                self.fish_backoff = 0.;
                self.fish_retry_at = 0.;
            }
        }
        if self.navigation.options.use_sell && self.fishing.stats.inventory_increased(before) {
            match self.navigation.job(Task::Sell, now(), true) {
                Ok(job) => self.job = Some(job),
                Err(error) => {
                    if self.coordinator.running {
                        self.block_fishing(error);
                    } else {
                        self.stop(error);
                    }
                }
            }
        }
        if self.coordinator.running
            && was_fishing.active()
            && was_fishing != Phase::Calibrating
            && !self.fishing.phase.active()
            && self.job.is_none()
        {
            self.block_fishing(self.fishing.message);
        }
        self.record();
        if std::mem::take(&mut self.fishing.calibration_changed) || changed {
            self.settings_error = self.save(self.fishing.tuning).err();
        }
        tap.map(GameInput::Tap)
    }

    fn navigation_tick(&mut self, tick: Tick) -> Option<GameInput> {
        if self.coordinator.traveling() {
            return match tick {
                Tick::Input(motion) => Some(GameInput::Motion(motion)),
                Tick::Wait | Tick::Observe => None,
                Tick::Done { .. } => {
                    self.job = None;
                    self.generation = self.generation.wrapping_add(1);
                    self.coordinator.travel_done(true, now());
                    self.macro_updates();
                    None
                }
                Tick::Failed(error) => {
                    self.job = None;
                    self.generation = self.generation.wrapping_add(1);
                    self.coordinator.travel_done(false, now());
                    self.push_log(error.into());
                    None
                }
            };
        }
        let input = match tick {
            Tick::Input(motion) => Some(GameInput::Motion(motion)),
            Tick::Wait | Tick::Observe => None,
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
                    self.stop("두 번 연속 팔 물고기가 없어 중지했습니다. 낚시 장소를 확인하세요.");
                    self.record();
                    return None;
                }
                self.job = None;
                if self.coordinator.running {
                    self.push_log(message.into());
                } else {
                    self.stop(message);
                }
                if resume {
                    if let Err(error) = self.fishing.start(now()) {
                        self.stop(error);
                    }
                }
                None
            }
            Tick::Failed(error) => {
                if self.coordinator.running {
                    self.job = None;
                    self.block_fishing(error);
                } else {
                    self.stop(error);
                }
                None
            }
        };

        self.record();
        input
    }

    fn headless_navigation(&mut self, geometry: (usize, usize)) -> Option<GameInput> {
        #[cfg(any(target_os = "windows", target_os = "linux"))]
        if let Some(input) = self.deliver_background_ocr() {
            return Some(input);
        }
        let config = self.macro_config();
        self.coordinator.poll(&config, &biome::status(), now());
        if self.coordinator.running
            && !config["mfish"]["enabled"].as_bool().unwrap_or(false)
            && self.fishing.phase.active()
            && self.fishing.phase != Phase::Calibrating
        {
            self.fishing.stop("자동 낚시 꺼짐");
        }
        if self.coordinator.take_preempted() {
            self.generation = self.generation.wrapping_add(1);
            self.job = None;
            self.fishing.release(now());
        }
        if self.coordinator.hold() == Some(coordinator::Hold::Urgent)
            && self.job.is_some()
            && !self.coordinator.traveling()
        {
            self.job = None;
            self.generation = self.generation.wrapping_add(1);
            self.coordinator.cleanup_navigation();
            self.fishing.stop("긴급 기능을 먼저 실행합니다.");
        }
        if let Some(job) = self.job.as_mut() {
            if job.needs_frame() {
                return None;
            }
            let tick = job.tick_without_frame(now(), geometry);
            return self.navigation_tick(tick);
        }
        if let Some(hold) = self.coordinator.hold() {
            match hold {
                coordinator::Hold::Replace => {
                    if !self.coordinator.is_ready() {
                        self.fishing.stop("다른 기능을 실행하는 중");
                        self.fishing.release(now());
                    }
                    self.coordinator.ready();
                }
                coordinator::Hold::FinishCatch | coordinator::Hold::Urgent => {
                    if self.fishing.phase == Phase::Calibrating {
                        self.fishing.stop("긴급 기능을 먼저 실행합니다.");
                    }
                    self.fishing.hold(hold == coordinator::Hold::Urgent, now());
                    if self.fishing.is_yielded() {
                        self.coordinator.ready();
                    }
                }
            }
        }
        let step = self.coordinator.tick(now());
        let input = step.and_then(|step| self.macro_step(step));
        self.macro_updates();
        if input.is_some() {
            return input;
        }
        if self.coordinator.running
            && !self.coordinator.has_job()
            && config["mfish"]["enabled"].as_bool().unwrap_or(false)
            && !self.fishing.phase.active()
            && now() >= self.fish_retry_at
        {
            self.begin_fishing();
        }
        None
    }

    fn needs_frame(&self) -> bool {
        if let Some(job) = self.job.as_ref() {
            return job.needs_frame();
        }
        if let Some(request) = self.coordinator.observation() {
            if request.background && self.background_ocr == Some(request.token) {
                return self.fishing.phase.active() && !self.fishing.is_yielded();
            }
            return true;
        }
        if self.coordinator.has_job() && self.coordinator.is_ready() {
            return false;
        }
        self.fishing.phase.active() && !self.fishing.is_yielded()
    }

    fn input_ack(&mut self, completed: bool, at: f64) {
        self.coordinator.ack(completed, at);
        self.macro_updates();
    }

    fn macro_step(&mut self, step: macro_step::Step) -> Option<GameInput> {
        match step {
            macro_step::Step::Input(motion) => Some(GameInput::Motion(motion)),
            macro_step::Step::Text { value, submit } => Some(GameInput::Text {
                text: value,
                submit,
            }),
            macro_step::Step::Travel { place } => {
                match self.navigation.desktop_move_place(place, now()) {
                    Ok(job) => self.job = Some(job),
                    Err(error) => {
                        self.coordinator.travel_done(false, now());
                        self.push_log(error.into());
                    }
                }
                None
            }
            macro_step::Step::Done | macro_step::Step::Failed(_) => None,
            macro_step::Step::Observe { .. } | macro_step::Step::Wait(_) => None,
        }
    }

    fn macro_updates(&mut self) {
        for line in self.coordinator.take_logs() {
            self.push_log(line);
        }
        if let Some(patch) = self.coordinator.take_patch() {
            let old = self.macro_settings.clone();
            let mut config = self.macro_config();
            if let Some(groups) = patch.as_object() {
                for (group, value) in groups {
                    if let Some(values) = value.as_object() {
                        if !config[group].is_object() {
                            config[group] = serde_json::json!({});
                        }
                        config[group]
                            .as_object_mut()
                            .unwrap()
                            .extend(values.clone());
                    }
                }
            }
            config["macro_on"] = false.into();
            self.macro_settings = config;
            if let Err(error) = self.save(self.fishing.tuning) {
                self.macro_settings = old;
                self.settings_error = Some(error.clone());
                self.push_log(error);
            }
        }
        if let Some(restart) = self.coordinator.take_release() {
            self.fishing.release(now());
            if restart {
                self.fishing.stop("낚시 장소로 돌아갈 준비 중");
            }
        }
        self.record();
    }

    fn begin_fishing(&mut self) {
        if self.navigation.options.use_move {
            match self.navigation.job(Task::Fish, now(), true) {
                Ok(job) => {
                    self.job = Some(job);
                    self.fish_started = true;
                }
                Err(error) => self.block_fishing(error),
            }
        } else if let Err(error) = self.fishing.start(now()) {
            self.block_fishing(error);
        } else {
            self.fish_started = true;
        }
    }

    fn block_fishing(&mut self, error: &'static str) {
        self.fishing.stop(error);
        self.fish_backoff = (self.fish_backoff * 2.).clamp(30., 300.);
        self.fish_retry_at = now() + self.fish_backoff;
        self.push_log(format!(
            "{error} · {}초 뒤 다시 시도",
            self.fish_backoff as u64
        ));
    }

    fn record_desktop_sample(&mut self, keys: &[u16], left: bool, point: vision::Point, at: f64) {
        if let Some(job) = self.job.as_mut() {
            job.record_desktop(keys, left, point, at);
        }
    }

    fn macro_config(&self) -> serde_json::Value {
        let mut config: serde_json::Value =
            serde_json::from_str(include_str!("../../frontend/desktop-defaults.json"))
                .expect("packaged configuration");
        if let (Some(defaults), Some(saved)) =
            (config.as_object_mut(), self.macro_settings.as_object())
        {
            for (key, value) in saved {
                if let (Some(base), Some(group)) = (
                    defaults.get_mut(key).and_then(|v| v.as_object_mut()),
                    value.as_object(),
                ) {
                    base.extend(group.clone());
                } else {
                    defaults.insert(key.clone(), value.clone());
                }
            }
        }
        if let Some(places) = config["move"]["places"].as_array_mut() {
            for (key, name, feat, count) in [
                ("fish_spot", "낚시 장소", "mfish", 1),
                ("sell_spot", "물고기 판매 장소", "mfish", 1),
                ("craft_spot", "포션 제작 장소", "mcraft", 2),
                ("match_spot", "메모리 매치 장소", "mmatch", 1),
            ] {
                if !places
                    .iter()
                    .any(|place| place["key"].as_str() == Some(key))
                {
                    places.push(serde_json::json!({"key":key,"name":name,"feat":feat,"points":(0..count).map(|_|serde_json::json!({"pos":null,"time":null})).collect::<Vec<_>>()}));
                }
            }
        }
        let biome = biome::settings().normalized();
        if !config["biome"].is_object() {
            config["biome"] = serde_json::json!({});
        }
        config["biome"]["safe_mode"] = biome.safe_mode.into();
        config["biome"]["safe_v2"] = biome.safe_v2.into();
        config["macro_on"] = self.coordinator.running.into();
        config
    }

    fn phase(&self) -> Phase {
        match self.job.as_ref() {
            Some(job) if job.desktop_recording() => Phase::Recording,
            Some(job) if job.recording_at.is_some() && job.action != Task::Measure => {
                Phase::Recording
            }
            Some(job) if job.action == Task::Sell => Phase::Selling,
            Some(_) => Phase::Moving,
            None if self.fishing.phase.active() => self.fishing.phase,
            None if self.coordinator.active() => Phase::Moving,
            None => Phase::Idle,
        }
    }

    fn message(&self) -> String {
        if let Some(job) = self.job.as_ref() {
            return job.message().into();
        }
        self.coordinator
            .message()
            .unwrap_or_else(|| self.fishing.message.into())
    }

    fn stop(&mut self, message: &'static str) {
        self.generation = self.generation.wrapping_add(1);
        self.job = None;
        self.coordinator.stop_all(now());
        self.background_ocr = None;
        self.fishing.release(now());
        self.fishing.stop(message);
    }

    fn no_target(&mut self) {
        if self.coordinator.has_job() {
            let traveling = self.coordinator.traveling();
            self.coordinator
                .fail_current("Roblox 화면이 바뀌어 실행을 중지했습니다.".into(), now());
            if traveling {
                self.job = None;
            }
            self.generation = self.generation.wrapping_add(1);
            self.macro_updates();
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
            self.fishing.no_target(now());
        }
    }

    fn input_current(&self, generation: i32) -> bool {
        self.generation == generation && self.phase().active()
    }

    fn record(&mut self) {
        let message = self.message();
        if self.logged.as_deref() == Some(message.as_str()) {
            return;
        }
        self.push_log(message.clone());
        self.logged = Some(message);
    }

    fn push_log(&mut self, message: String) {
        if self.log.len() == 300 {
            self.log.pop_front();
        }
        eprintln!("[acrux] {message}");
        self.log.push_back(message);
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
        #[cfg(any(target_os = "windows", target_os = "linux"))]
        if action == Action::Start {
            self.fish_retry_at = 0.;
            self.fish_backoff = 0.;
            self.empty_sales = 0;
            let config = self.macro_config();
            self.coordinator.start(&config, now())?;
            return Ok(());
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
        if let Some(job) = self.job.as_mut() {
            match job.finish_desktop_record(now()) {
                Ok(Some((index, route))) => {
                    let mut config = self.macro_config();
                    let Some(place) = config["move"]["places"]
                        .as_array_mut()
                        .and_then(|places| places.get_mut(index))
                    else {
                        self.stop("길을 저장할 장소가 없습니다.");
                        return true;
                    };
                    place["route"] = route;
                    let previous = std::mem::replace(&mut self.macro_settings, config);
                    if let Err(error) = self.save(self.fishing.tuning) {
                        self.macro_settings = previous;
                        self.settings_error = Some(error);
                        self.stop("길 기록을 저장하지 못했습니다.");
                    } else {
                        self.stop("길 기록을 저장했습니다.");
                    }
                    self.record();
                    return true;
                }
                Err(error) => {
                    self.stop(error);
                    self.record();
                    return true;
                }
                Ok(None) => {}
            }
        }
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
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    return desktop::game_available();
    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
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
fn control(action: Action, generation: Option<i32>) -> Result<Status, String> {
    if action != Action::Stop {
        {
            let state = runtime();
            if generation.is_some_and(|g| g != state.generation) {
                return Err("이전 실행 요청이 취소됐습니다.".into());
            }
        }
        #[cfg(any(target_os = "windows", target_os = "linux"))]
        desktop::focus_game()?;
    }
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    let geometry = desktop::geometry();
    let mut state = runtime();
    if action != Action::Stop && generation.is_some_and(|g| g != state.generation) {
        return Err("이전 실행 요청이 취소됐습니다.".into());
    }
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    if action != Action::Stop {
        if let Some(geometry) = geometry {
            let config = state.macro_config();
            if config["mfish"]["enabled"].as_bool().unwrap_or(false) || action == Action::Calibrate
            {
                state
                    .fishing
                    .configure_positions(&config["mfish"], geometry)?;
            }
        }
    }
    state.control(action)?;
    state.record();
    Ok(state.status())
}

#[tauri::command]
fn macro_action(
    action: String,
    payload: serde_json::Value,
    generation: Option<i32>,
) -> Result<Status, String> {
    if action == "start" {
        return control(Action::Start, generation);
    }
    if action == "stop" {
        return control(Action::Stop, generation);
    }
    let stopping = action.ends_with("_stop");
    {
        let state = runtime();
        if generation.is_some_and(|expected| expected != state.generation) {
            return Err("이전 실행 요청이 취소됐습니다.".into());
        }
        if !stopping && (!state.capture || !state.input) {
            return Err("게임 캡처와 입력을 먼저 연결하세요.".into());
        }
    }
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    if !stopping {
        desktop::focus_game()?;
    }
    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    if !stopping {
        return Err("이 매크로 동작은 아직 데스크톱에만 연결돼 있습니다.".into());
    }
    let mut state = runtime();
    if generation.is_some_and(|expected| expected != state.generation) {
        return Err("이전 실행 요청이 취소됐습니다.".into());
    }
    if !stopping && state.job.is_some() {
        return Err("이동 작업이 끝난 뒤 다시 시도하세요.".into());
    }
    let cancel_current = stopping && action.strip_suffix("_stop") == state.coordinator.feature();
    let cancel_travel = cancel_current && state.coordinator.traveling();
    let config = state.macro_config();
    state
        .coordinator
        .action(&action, &payload, &config, now())?;
    if !stopping || cancel_current {
        state.generation = state.generation.wrapping_add(1);
        state.background_ocr = None;
    }
    if cancel_travel {
        state.job = None;
    }
    if action == "mfish_stop" {
        state.fishing.stop("자동 낚시 중지됨");
    }
    state.macro_updates();
    Ok(state.status())
}

#[tauri::command]
fn macro_settings() -> serde_json::Value {
    runtime().macro_config()
}

#[tauri::command]
fn save_macro_settings(mut config: serde_json::Value) -> Result<serde_json::Value, String> {
    if !config.is_object() {
        return Err("설정 형식을 확인하세요.".into());
    }
    let mut state = runtime();
    if state.phase().active() && !state.coordinator.running {
        return Err("실행 중인 작업을 먼저 중지하세요.".into());
    }
    config["macro_on"] = false.into();
    let mut tuning = serde_json::to_value(state.fishing.tuning).map_err(|e| e.to_string())?;
    for (ui, key) in [
        ("bite_max", "biteSeconds"),
        ("lead_ms", "leadMs"),
        ("target_pct", "targetPercent"),
        ("click_gap_ms", "clickGapMs"),
        ("cast_retry", "castRetries"),
    ] {
        if let Some(value) = config["mfish"].get(ui) {
            tuning[key] = value.clone();
        }
    }
    let tuning: Tuning =
        serde_json::from_value(tuning).map_err(|_| "낚시 설정의 숫자 형식을 확인하세요.")?;
    let tuning = tuning.validate()?;
    let source = &config["biome"];
    let biome = serde_json::from_value::<biome::Settings>(serde_json::json!({
        "enabled":source["enabled"].as_bool().unwrap_or(false),
        "webhooks":source.get("webhooks").cloned().unwrap_or(serde_json::json!([])),
        "psLink":source["ps_link"].as_str().unwrap_or(""),
        "player":source["player"].as_str().unwrap_or(""),
        "everyone":true,"mentions":source.get("everyone").cloned().unwrap_or(serde_json::json!({})),
        "roles":source.get("roles").cloned().unwrap_or(serde_json::json!({})),
        "safeMode":source["safe_mode"].as_bool().unwrap_or(true),
        "safeV2":source["safe_v2"].as_bool().unwrap_or(false)
    }))
    .map_err(|_| "바이옴 설정 형식을 확인하세요.")?
    .normalized();
    biome.validate()?;
    if !config["biome"].is_object() {
        config["biome"] = serde_json::json!({});
    }
    config["biome"]["safe_mode"] = biome.safe_mode.into();
    config["biome"]["safe_v2"] = biome.safe_v2.into();
    let online = online::Settings {
        share: config["online_share"].as_bool().unwrap_or(true),
        ..online::settings()
    };
    state.persist_profile(&Profile {
        biome: biome.clone(),
        online: online.clone(),
        calibration: state.fishing.calibration,
        tuning,
        totals: state.totals,
        navigation: if cfg!(any(target_os = "windows", target_os = "linux")) {
            Navigation::default()
        } else {
            state.navigation.clone()
        },
        macro_settings: config.clone(),
    })?;
    state.macro_settings = config;
    state.fishing.tuning = tuning;
    if !state.coordinator.running {
        state.generation = state.generation.wrapping_add(1);
    }
    biome::configure(biome);
    online::load(online);
    Ok(state.macro_config())
}

#[tauri::command]
fn desktop_record(place: usize) -> Result<(), String> {
    let mut state = runtime();
    if state.phase().active() {
        return Err("실행 중인 작업을 먼저 중지하세요.".into());
    }
    state.job = Some(state.navigation.desktop_record(place, now())?);
    drop(state);
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    if let Err(error) = desktop::focus_game() {
        runtime().stop("게임 창에 연결하지 못했습니다.");
        return Err(error);
    }
    Ok(())
}
#[tauri::command]
fn desktop_move_place(place: usize) -> Result<(), String> {
    let mut state = runtime();
    if state.phase().active() {
        return Err("실행 중인 작업을 먼저 중지하세요.".into());
    }
    state.job = Some(state.navigation.desktop_move_place(place, now())?);
    drop(state);
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    if let Err(error) = desktop::focus_game() {
        runtime().stop("게임 창에 연결하지 못했습니다.");
        return Err(error);
    }
    Ok(())
}
#[tauri::command]
fn desktop_route_clear(place: usize) -> Result<(), String> {
    let mut state = runtime();
    if state.phase().active() {
        return Err("실행 중인 작업을 먼저 중지하세요.".into());
    }
    let mut config = state.macro_config();
    let selected = config["move"]["places"]
        .as_array_mut()
        .and_then(|places| places.get_mut(place))
        .ok_or("지울 장소를 찾을 수 없습니다.")?;
    selected["route"] = serde_json::Value::Null;
    let old = std::mem::replace(&mut state.macro_settings, config);
    if let Err(error) = state.save(state.fishing.tuning) {
        state.macro_settings = old;
        return Err(error);
    }
    Ok(())
}
#[tauri::command]
async fn desktop_pick_region() -> Result<Vec<f64>, String> {
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    return tauri::async_runtime::spawn_blocking(desktop::pick_region)
        .await
        .map_err(|e| e.to_string())?;
    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    Err("이 환경에서는 화면 영역 지정을 지원하지 않습니다.".into())
}
#[tauri::command]
async fn desktop_snapshot(app: tauri::AppHandle) -> Result<tauri::ipc::Response, String> {
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    {
        let result = tauri::async_runtime::spawn_blocking(|| {
            desktop::focus_game()?;
            desktop::snapshot()
        })
        .await
        .map_err(|e| e.to_string());
        if let Some(window) = app.get_webview_window("main") {
            let _ = window.set_focus();
        }
        result?.map(tauri::ipc::Response::new)
    }
    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    {
        let _ = app;
        Err("이 환경에서는 게임 화면 연결을 지원하지 않습니다.".into())
    }
}
#[tauri::command]
async fn desktop_open_game() -> Result<Status, String> {
    #[cfg(target_os = "linux")]
    tauri::async_runtime::spawn_blocking(desktop::open_game)
        .await
        .map_err(|e| e.to_string())??;
    #[cfg(target_os = "windows")]
    {
        tauri::async_runtime::spawn_blocking(|| {
            if desktop::focus_game().is_ok() {
                return Ok(());
            }
            std::process::Command::new("cmd")
                .args(["/C", "start", "", "roblox://placeId=15532962292"])
                .spawn()
                .map(|_| ())
                .map_err(|e| format!("Roblox를 열지 못했습니다: {e}"))
        })
        .await
        .map_err(|e| e.to_string())??;
    }
    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    return Err("이 환경에서는 게임 실행을 지원하지 않습니다.".into());
    Ok(runtime().status())
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
    if !cfg!(any(target_os = "windows", target_os = "linux")) {
        return Err("Windows에서만 게임 입력을 사용할 수 있습니다.".into());
    }
    let mut state = runtime();
    if state.job.is_some()
        || state.coordinator.has_job()
        || (state.phase().active() && !state.coordinator.running)
    {
        return Err("실행 중인 작업을 먼저 중지하세요.".into());
    }
    state.navigation = Navigation::desktop(setup)?;
    Ok(())
}

#[tauri::command]
fn desktop_task(task: Task) -> Result<(), String> {
    if !cfg!(any(target_os = "windows", target_os = "linux")) {
        return Err("Windows에서만 게임 입력을 사용할 수 있습니다.".into());
    }
    runtime().task(task).map_err(str::to_owned)?;
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    if let Err(error) = desktop::focus_game() {
        runtime().stop("게임 창에 연결하지 못했습니다.");
        return Err(error);
    }
    Ok(())
}

#[tauri::command]
fn desktop_waypoint(
    place: Place,
    index: usize,
    measure: bool,
    from_base: bool,
) -> Result<(), String> {
    if !cfg!(any(target_os = "windows", target_os = "linux")) {
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
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    return tauri::async_runtime::spawn_blocking(desktop::pick)
        .await
        .map_err(|e| e.to_string())?;
    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    Err("이 환경에서는 화면 위치 지정을 지원하지 않습니다.".into())
}

#[tauri::command]
fn desktop_cancel_pick() {
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    desktop::cancel_pick();
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
    let mut lines: Vec<String> = runtime().log.iter().rev().cloned().collect();
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
    let settings = settings.normalized();
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default();
    #[cfg(target_os = "android")]
    let builder = builder.plugin(android::plugin());
    #[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
    let builder = builder.plugin(tauri_plugin_single_instance::init(|app, _, _| {
        if let Some(window) = app.get_webview_window("main") {
            let _ = window.unminimize();
            let _ = window.show();
            let _ = window.set_focus();
        }
    }));
    builder
        .setup(|app| {
            let dir = app.path().app_config_dir()?;
            fs::create_dir_all(&dir)?;
            runtime().load(dir.join("main-port-profile.json"));
            {
                let mut state = runtime();
                if state.settings_error.is_none() {
                    state.settings_error = state.save(state.fishing.tuning).err();
                }
            }
            #[cfg(any(target_os = "windows", target_os = "linux"))]
            biome::watch_desktop();
            #[cfg(any(target_os = "windows", target_os = "linux"))]
            desktop::start()?;
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
            macro_action,
            save_tuning,
            macro_settings,
            save_macro_settings,
            desktop_record,
            desktop_move_place,
            desktop_route_clear,
            desktop_pick_region,
            desktop_snapshot,
            desktop_open_game,
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

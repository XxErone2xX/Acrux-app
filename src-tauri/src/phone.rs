//! Phone transport: screen-bound input plus a separately claimed recovery launch.
use crate::{
    vision::{Frame, Region},
    DeviceTick, GameInput, Route, Runtime,
};

const WAIT_MS: i32 = 250;
const INPUT_GRACE: f64 = 2.0;

#[derive(Clone, Copy)]
struct Observation {
    id: i32,
    generation: i32,
    geometry: (usize, usize),
    region: Region,
    at: f64,
    ocr: bool,
}

#[derive(Clone, Copy)]
struct Input {
    id: i32,
    generation: i32,
    until: f64,
    start_before: f64,
    launch: bool,
    claimed: bool,
}

#[derive(Default)]
pub(super) struct State {
    pub(super) foreground: bool,
    geometry: Option<(usize, usize)>,
    next_id: i32,
    observation: Option<Observation>,
    input: Option<Input>,
    return_requested: bool,
    returning: bool,
}

impl State {
    pub(super) fn can_start(&self, generation: i32, id: i32, at: f64) -> bool {
        self.foreground
            && self.input.is_some_and(|input| {
                !input.launch
                    && input.generation == generation
                    && input.id == id
                    && at.is_finite()
                    && at < input.start_before
            })
    }
    pub(super) fn has_pending(&self) -> bool {
        self.input.is_some() || self.observation.is_some()
    }
    pub(super) fn cancel(&mut self) {
        self.observation = None;
        self.input = None;
        self.cancel_return();
    }
    pub(super) fn cancel_return(&mut self) {
        self.return_requested = false;
        self.returning = false;
    }

    fn id(&mut self) -> i32 {
        self.next_id = if self.next_id == i32::MAX {
            1
        } else {
            self.next_id + 1
        };
        self.next_id
    }
}

/// Only an explicit Return action can authorize recovery without a disconnect
/// log. Play and ordinary app switches never set this permission.
pub(super) fn request_return(state: &mut Runtime, at: f64) {
    if !state.dua.running || !at.is_finite() {
        return;
    }
    state.phone.return_requested = true;
    if !state.phone.foreground {
        if let Some(runner) = state.automation.as_mut() {
            if runner.status().suspended {
                runner.resume(at);
            }
        }
    }
}

/// Unlike the screen-input guard, this consumes one transport-owned launch.
/// The service supplies its current lock check immediately before startActivity.
pub(super) fn claim_launch(
    state: &mut Runtime,
    generation: i32,
    id: i32,
    unlocked: bool,
    at: f64,
) -> bool {
    if !unlocked || !state.input_current(generation) || !state.capture || !state.input {
        return false;
    }
    let Some(input) = state.phone.input.as_mut() else {
        return false;
    };
    if !input.launch
        || input.claimed
        || input.generation != generation
        || input.id != id
        || !at.is_finite()
        || at >= input.start_before
        || at >= input.until
    {
        return false;
    }
    input.claimed = true;
    true
}

fn wait(state: &Runtime, delay: i32) -> Vec<i32> {
    vec![0, state.generation, delay.clamp(8, WAIT_MS)]
}

fn request(observation: Observation) -> Vec<i32> {
    let r = observation.region;
    vec![
        1,
        observation.generation,
        8,
        r.x as i32,
        r.y as i32,
        r.width as i32,
        r.height as i32,
        observation.id,
        i32::from(observation.ocr),
    ]
}

fn stop(state: &mut Runtime, message: &'static str) {
    state.stop(message);
    state.record();
}

fn settle_return(state: &mut Runtime) {
    if state.phone.returning
        && !state.automation.as_ref().is_some_and(|runner| {
            matches!(
                runner.status().active,
                Some(crate::automation::Job::Return { .. })
            )
        })
    {
        state.phone.returning = false;
    }
}

fn input(
    state: &mut Runtime,
    step: DeviceTick,
    geometry: (usize, usize),
    at: f64,
) -> Option<Vec<i32>> {
    let DeviceTick::Input(command) = step else {
        return None;
    };
    let launch = matches!(
        &command,
        GameInput::Command(crate::automation::Command::Launch { .. })
    );
    let recovery = state.phone.return_requested || state.phone.returning || state.game_disconnected;
    if !state.input_current(state.generation) || (!state.phone.foreground && !(launch && recovery))
    {
        return None;
    }
    let id = state.phone.id();
    let packet = match command {
        GameInput::Tap(tap) => {
            let ms = tap.duration.as_millis();
            if ms == 0 || ms > 1000 || tap.x >= geometry.0 || tap.y >= geometry.1 {
                stop(state, "모바일 DUA의 터치 명령이 올바르지 않습니다.");
                return None;
            }
            vec![
                2,
                id,
                geometry.0 as i32,
                geometry.1 as i32,
                0,
                ms as i32,
                tap.x as i32,
                tap.y as i32,
                state.generation,
            ]
        }
        GameInput::Gesture(gesture) => match gesture.packet(geometry, id, state.generation) {
            Ok(packet) => packet,
            Err(_) => {
                stop(state, "모바일 DUA의 입력 명령이 올바르지 않습니다.");
                return None;
            }
        },
        GameInput::Command(command) => {
            use crate::automation::Command;
            let (kind, flag, text) = match command {
                Command::Gesture(gesture) => {
                    let packet = gesture.packet(geometry, id, state.generation).ok()?;
                    state.phone.input = Some(Input {
                        id,
                        generation: state.generation,
                        start_before: state.frame_expires.unwrap_or(at + 0.25).min(at + 0.25),
                        until: at + f64::from(packet[5]) / 1000.0 + INPUT_GRACE,
                        launch: false,
                        claimed: false,
                    });
                    return Some(packet);
                }
                Command::Text { text, submit } => (4, i32::from(submit), text),
                Command::Launch { url } => (
                    6,
                    0,
                    match crate::automation::game_url(&url) {
                        Ok(url) => url,
                        Err(error) => {
                            state.stop_automation(error);
                            return None;
                        }
                    },
                ),
                Command::Key { vk, ms } => {
                    let key = match vk {
                        0x45 => Some("talk_btn"),
                        0x20 => Some("jump_btn"),
                        0xbf => Some("chat_pos"),
                        _ => None,
                    };
                    let point = key.and_then(|key| state.macro_point(key));
                    let point = point.or_else(|| {
                        let label = match vk {
                            0x45 => "e",
                            0x46 => "f",
                            _ => return None,
                        };
                        let mut found = state
                            .ocr_words
                            .iter()
                            .filter(|word| word.text.trim().eq_ignore_ascii_case(label));
                        let word = found.next()?;
                        found.next().is_none().then(|| word.rect.at(0.5, 0.5))
                    });
                    if let Some(point) = point {
                        let packet = crate::device::Gesture::Tap {
                            at: point,
                            ms: ms.clamp(1, 1000),
                        }
                        .packet(geometry, id, state.generation)
                        .ok()?;
                        state.phone.input = Some(Input {
                            id,
                            generation: state.generation,
                            start_before: state.frame_expires.unwrap_or(at + 0.25).min(at + 0.25),
                            until: at + f64::from(packet[5]) / 1000.0 + INPUT_GRACE,
                            launch: false,
                            claimed: false,
                        });
                        return Some(packet);
                    }
                    (5, i32::from(vk), String::new())
                }
            };
            if text.contains('\0') || text.encode_utf16().count() > 2048 {
                stop(state, "입력할 글자나 실행 주소의 길이를 확인하세요.");
                return None;
            }
            let mut packet = vec![
                2,
                id,
                geometry.0 as i32,
                geometry.1 as i32,
                kind,
                1,
                flag,
                0,
                state.generation,
            ];
            packet.extend(text.encode_utf16().map(i32::from));
            packet
        }
        GameInput::Motion(_) => {
            stop(
                state,
                "모바일 DUA에서 시간 기반 이동 명령을 실행할 수 없습니다.",
            );
            return None;
        }
    };
    state.phone.input = Some(Input {
        id,
        generation: state.generation,
        start_before: if launch {
            at + 0.25
        } else {
            state.frame_expires.unwrap_or(at + 0.25).min(at + 0.25)
        },
        until: at + f64::from(packet[5]) / 1000.0 + INPUT_GRACE,
        launch,
        claimed: false,
    });
    if launch {
        state.phone.return_requested = false;
        state.phone.returning = true;
    }
    Some(packet)
}

pub(super) fn tick(state: &mut Runtime, width: usize, height: usize, at: f64) -> Vec<i32> {
    if !state.active() {
        state.phone.cancel();
        return wait(state, WAIT_MS);
    }
    let geometry = (width, height);
    if !at.is_finite() || width == 0 || height == 0 || width > 16384 || height > 16384 {
        stop(
            state,
            "휴대폰 화면 크기를 확인할 수 없어 DUA를 중지했습니다.",
        );
        return wait(state, WAIT_MS);
    }
    if state.phone.geometry.is_some_and(|old| old != geometry)
        && (state.phone.observation.is_some()
            || state.phone.input.is_some_and(|input| !input.launch))
    {
        state.phone.geometry = Some(geometry);
        stop(state, "휴대폰 화면 크기가 바뀌어 DUA를 중지했습니다.");
        return wait(state, WAIT_MS);
    }
    state.phone.geometry = Some(geometry);
    if !state.capture || !state.input {
        stop(state, "화면 캡처와 접근성 입력을 다시 준비하세요.");
        return wait(state, WAIT_MS);
    }
    if let Some(pending) = state.phone.input {
        if !state.phone.foreground && !pending.launch {
            state.phone.cancel();
            return wait(state, WAIT_MS);
        }
        if at >= pending.until {
            stop(state, "휴대폰 터치의 완료 응답이 없어 DUA를 중지했습니다.");
        } else {
            state.route(Route::Headless, "휴대폰 터치가 끝나기를 기다립니다.", false);
        }
        return wait(state, 25);
    }
    if !state.phone.foreground {
        state.phone.observation = None;
        // Losing focus alone is not evidence of losing the game. Only a real
        // disconnect log or the user's Return action may drive this route.
        if state.game_disconnected || state.phone.return_requested || state.phone.returning {
            if let Some(runner) = state.automation.as_mut() {
                if runner.status().suspended {
                    runner.resume(at);
                }
                if state.game_disconnected {
                    runner.trigger(
                        crate::automation::Event::Game {
                            available: false,
                            in_game: false,
                        },
                        at,
                    );
                }
            }
            if let Some((generation, command)) = state.headless_missing(at) {
                if generation == state.generation {
                    return input(
                        state,
                        DeviceTick::Input(GameInput::Command(command)),
                        geometry,
                        at,
                    )
                    .unwrap_or_else(|| wait(state, WAIT_MS));
                }
            }
        } else {
            if let Some(runner) = state.automation.as_mut() {
                if !runner.status().suspended {
                    runner.suspend(at);
                }
            }
            state.route(Route::Headless, "Roblox가 앞에 오기를 기다립니다.", false);
        }
        return wait(state, WAIT_MS);
    }
    if let Some(observation) = state.phone.observation {
        return request(observation);
    }
    let step = state.tick(at, geometry, None);
    settle_return(state);
    match step {
        DeviceTick::Observe(region) => {
            let region = region.unwrap_or(Region::full(width, height));
            if !region.valid(geometry) {
                stop(state, "모바일 DUA의 관측 영역이 올바르지 않습니다.");
                return wait(state, WAIT_MS);
            }
            let observation = Observation {
                id: state.phone.id(),
                generation: state.generation,
                geometry,
                region,
                at,
                ocr: state.request_ocr,
            };
            state.phone.observation = Some(observation);
            request(observation)
        }
        DeviceTick::Input(command) => input(state, DeviceTick::Input(command), geometry, at)
            .unwrap_or_else(|| wait(state, WAIT_MS)),
        DeviceTick::Wait => {
            let next = state.fishing.observation(at, geometry).due_at;
            wait(
                state,
                ((next - at).max(0.0) * 1000.0)
                    .ceil()
                    .min(f64::from(WAIT_MS)) as i32,
            )
        }
    }
}

pub(super) fn observe(
    state: &mut Runtime,
    frame: &Frame<'_>,
    generation: i32,
    observation_id: i32,
    at: f64,
) -> Option<Vec<i32>> {
    observe_with_age(state, frame, generation, observation_id, at, 0.)
}

pub(super) fn observe_with_age(
    state: &mut Runtime,
    frame: &Frame<'_>,
    generation: i32,
    observation_id: i32,
    at: f64,
    age: f64,
) -> Option<Vec<i32>> {
    let observation = state.phone.observation?;
    if generation != state.generation
        || observation.generation != generation
        || observation.id != observation_id
        || !state.phone.foreground
        || !state.active()
        || !at.is_finite()
        || at < observation.at
    {
        return None;
    }
    if (frame.width, frame.height) != observation.geometry || frame.coverage() != observation.region
    {
        stop(
            state,
            "요청한 휴대폰 화면과 관측 영역이 달라 DUA를 중지했습니다.",
        );
        return None;
    }
    if !state.capture || !state.input || state.phone.input.is_some() {
        return None;
    }
    if !age.is_finite() || !(0.0..0.25).contains(&age) {
        state.ocr_words.clear();
        return None;
    }
    state.phone.observation = None;
    state.frame_expires = Some(at + 0.300 - age);
    state.frame_captured = Some(at - age);
    let (step, processed_at) =
        crate::processed_frame_tick(state, observation.geometry, frame, at, at);
    settle_return(state);
    let result = input(state, step, observation.geometry, processed_at);
    state.ocr_words.clear();
    result
}

pub(super) fn text(
    state: &mut Runtime,
    generation: i32,
    observation_id: i32,
    words: Vec<crate::automation::Word>,
) -> bool {
    let Some(request) = state.phone.observation else {
        return false;
    };
    if !request.ocr
        || request.generation != generation
        || state.generation != generation
        || request.id != observation_id
        || !state.phone.foreground
        || words.len() > 1024
    {
        return false;
    }
    if words
        .iter()
        .any(|word| word.text.len() > 4096 || word.rect.region(request.geometry).is_none())
    {
        return false;
    }
    state.ocr_words = words;
    true
}

pub(super) fn reject_observation(state: &mut Runtime, generation: i32, observation_id: i32) {
    if state.generation == generation
        && state
            .phone
            .observation
            .is_some_and(|pending| pending.generation == generation && pending.id == observation_id)
    {
        stop(state, "휴대폰 관측 데이터를 읽지 못해 DUA를 중지했습니다.");
    }
}

pub(super) fn foreground(state: &mut Runtime, foreground: bool, at: f64) {
    if state.phone.foreground == foreground {
        return;
    }
    if !state.active() {
        state.phone.foreground = foreground;
        return;
    }
    let message = if state.active() {
        "Roblox가 앞에 오기를 기다립니다.".to_owned()
    } else {
        state.message().to_owned()
    };
    state.phone.foreground = foreground;
    if matches!(state.sdk_request, Some(crate::sdk::Request::Mark { .. })) {
        // The mark command returns to Roblox before starting camera setup.
        // That first foreground callback must retain its waiting controller;
        // it is not a fishing-resume request. Leaving later cancels the mark.
        if foreground {
            state.phone.cancel();
            state.ocr_words.clear();
            state.frame_expires = None;
            state.route(
                Route::Headless,
                "현재 장소를 기억할 새 게임 화면을 확인합니다.",
                false,
            );
        } else {
            state.stop("다른 앱으로 전환돼 장소 등록을 취소했습니다.");
        }
        state.record();
        return;
    }
    // startActivity can deliver a foreground callback before its ACK. That
    // callback must not invalidate or replay the one pending launch.
    if state.phone.input.is_some_and(|input| input.launch) {
        state.phone.observation = None;
        state.ocr_words.clear();
        state.frame_expires = None;
        state.route(Route::Headless, "게임 실행 응답을 기다립니다.", false);
        state.record();
        return;
    }
    state.resume_after_calibration = false;
    if let Some(navigation) = state.wayfinder.take() {
        state.wayfinding.memory = navigation.memory();
        if let Some(runner) = state.automation.as_mut() {
            runner.travel_result(false, at);
        }
    }
    state.finish_tool("Roblox 화면을 다시 확인합니다.");
    state.macro_message = Some(message);
    if state.dua.running {
        state.route(Route::Headless, "Roblox 화면을 다시 확인합니다.", false);
        if let Some(runner) = state.automation.as_mut() {
            if foreground {
                runner.resume(at);
            } else {
                runner.suspend(at);
            }
        } else if foreground {
            if let Err(error) = state.begin_fishing(at) {
                stop(state, error);
            }
        }
    }
    state.record();
}

pub(super) fn ack(state: &mut Runtime, generation: i32, input_id: i32, success: bool, at: f64) {
    let Some(pending) = state.phone.input else {
        return;
    };
    if pending.id != input_id || pending.generation != generation || state.generation != generation
    {
        return;
    }
    state.phone.input = None;
    if !at.is_finite() || at >= pending.until {
        stop(state, "휴대폰 터치의 완료 응답이 늦어 DUA를 중지했습니다.");
    } else if !success || pending.launch && !pending.claimed {
        stop(state, "휴대폰 터치가 취소되어 DUA를 중지했습니다.");
    } else {
        state.device_ack_at(at);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{fishing::Phase, vision::Point};

    const W: usize = 480;
    const H: usize = 270;

    fn ready() -> Runtime {
        let mut state = Runtime::default();
        state.capture = true;
        state.input = true;
        let mut config = state.macro_config();
        config["useMove"] = false.into();
        config["useSell"] = false.into();
        state.macro_settings = config;
        state
            .start_dua_with_foreground(&["mfish".into()], 0.0, true)
            .unwrap();
        state
    }

    fn recovery_config() -> serde_json::Value {
        serde_json::json!({"useMove":false,"useSell":false,
            "ret":{"ps_link":"https://www.roblox.com/games/15532962292/?privateServerLinkCode=test","start_wait":0},
            "play":{"load_wait":0}})
    }

    fn recovery_ready() -> Runtime {
        let mut state = Runtime::default();
        state.capture = true;
        state.input = true;
        state.macro_settings = recovery_config();
        state
            .start_dua_with_foreground(&["mpop".into()], 0.0, true)
            .unwrap();
        foreground(&mut state, true, 0.1);
        state.game_line("{\"state\":\"Equipped\"}", 0.2);
        state
    }

    #[test]
    fn ordinary_app_switch_never_launches_even_with_a_recovery_link() {
        let mut state = recovery_ready();
        foreground(&mut state, false, 0.3);
        let generation = state.generation;
        for (i, line) in [
            "BluetoothGatt disconnect",
            "SystemUI connectionlost",
            "Wifi disconnected",
        ]
        .iter()
        .enumerate()
        {
            state.game_line(line, 0.4 + i as f64 * 0.1);
        }
        for at in [1.0, 11.0, 60.0] {
            assert_eq!(tick(&mut state, W, H, at)[0], 0);
        }
        assert_eq!(state.generation, generation);
        assert!(!state.game_disconnected);
        assert!(
            state.game_observed,
            "unrelated system logs do not clear game evidence"
        );
        assert!(state
            .automation
            .as_ref()
            .unwrap()
            .status()
            .recovery_left
            .is_none());
        assert!(state.phone.input.is_none() && state.phone.observation.is_none());
        assert!(state.automation.as_ref().unwrap().status().suspended);
        assert_eq!(state.automation.as_ref().unwrap().status().commands, 0);
        assert_eq!(state.dua.screenshot_frames, 0);
    }

    #[test]
    fn disconnect_log_launches_once_without_pixels_and_foreground_does_not_ack_it() {
        let mut state = recovery_ready();
        foreground(&mut state, false, 0.3);
        state.game_line(
            "[FLog::Network] Client:Disconnect reason=connection_lost",
            1.0,
        );
        assert!(state.game_disconnected && !state.game_observed);
        assert_eq!(
            state.automation.as_ref().unwrap().status().recovery_left,
            Some(10.0)
        );
        assert_eq!(tick(&mut state, W, H, 10.9)[0], 0);
        let launch = tick(&mut state, W, H, 11.0);
        assert_eq!(launch[4], 6);
        assert_eq!(state.dua.screenshot_frames, 0);
        assert!(
            !state.phone.return_requested,
            "the explicit permission is consumed on issue"
        );
        assert_eq!(tick(&mut state, W, H, 11.01)[0], 0);
        assert!(!state.phone.can_start(launch[8], launch[1], 11.02));
        assert!(
            !claim_launch(&mut state, launch[8], launch[1], false, 11.02),
            "lock check cannot claim a launch"
        );
        assert!(!claim_launch(
            &mut state,
            launch[8],
            launch[1] + 1,
            true,
            11.02
        ));
        assert!(claim_launch(&mut state, launch[8], launch[1], true, 11.03));
        assert!(
            !claim_launch(&mut state, launch[8], launch[1], true, 11.04),
            "launch claim is one-shot"
        );
        foreground(&mut state, true, 11.05);
        assert_eq!(
            state.generation, launch[8],
            "startActivity foreground callback preserves its ACK"
        );
        assert_eq!(
            tick(&mut state, 640, 360, 11.06)[0],
            0,
            "launch has no viewport dependency"
        );
        assert!(state.active());
        ack(&mut state, launch[8], launch[1], true, 11.07);
        assert!(state.phone.input.is_none());
        assert!(!state.game_observed);
        assert_eq!(state.automation.as_ref().unwrap().status().completed, 0);
        assert_eq!(state.automation.as_ref().unwrap().status().commands, 1);
        // A visible app and launch ACK still require a fresh screenshot/OCR.
        let request = tick(&mut state, 640, 360, 11.08);
        assert_eq!(request[0], 1);
        assert_eq!(state.automation.as_ref().unwrap().status().completed, 0);
        let region = state.phone.observation.unwrap().region;
        let bytes = vec![0; region.width * region.height * 4];
        let frame = Frame::region(&bytes, 640, 360, region.width * 4, 4, false, region).unwrap();
        assert!(observe(&mut state, &frame, request[1], request[7], 11.09).is_none());
        assert_eq!(state.automation.as_ref().unwrap().status().completed, 0);
        let request = tick(&mut state, 640, 360, 11.10);
        assert_eq!(request[0], 1);
        state.game_line("{\"state\":\"Equipped\"}", 11.11);
        let region = state.phone.observation.unwrap().region;
        let bytes = vec![0; region.width * region.height * 4];
        let frame = Frame::region(&bytes, 640, 360, region.width * 4, 4, false, region).unwrap();
        assert!(observe(&mut state, &frame, request[1], request[7], 11.12).is_none());
        tick(&mut state, 640, 360, 11.13);
        assert_eq!(
            state.automation.as_ref().unwrap().status().completed,
            1,
            "only new entry evidence completes recovery"
        );
        assert!(!state.phone.return_requested && !state.phone.returning);
        foreground(&mut state, false, 11.14);
        assert_eq!(tick(&mut state, 640, 360, 30.0)[0], 0);
        assert_eq!(
            state.automation.as_ref().unwrap().status().commands,
            1,
            "later ordinary app switches cannot reuse recovery permission"
        );
    }

    #[test]
    fn explicit_return_can_launch_but_background_play_and_old_entry_cannot() {
        let mut state = Runtime::default();
        state.capture = true;
        state.input = true;
        state.dua.running = true;
        state.game_observed = true;
        let mut runner = crate::automation::Runner::new(recovery_config(), 0.0).unwrap();
        runner.log("{\"state\":\"Equipped\"}", 0.1);
        runner
            .queue(crate::automation::Job::Return { test: true })
            .unwrap();
        runner.suspend(0.2);
        state.automation = Some(runner);
        request_return(&mut state, 0.3);
        let launch = tick(&mut state, W, H, 0.4);
        assert_eq!(launch[4], 6);
        assert!(
            !state.game_observed,
            "old entry evidence is cleared when issuing Launch"
        );
        assert!(claim_launch(&mut state, launch[8], launch[1], true, 0.41));
        ack(&mut state, launch[8], launch[1], true, 0.42);
        assert_eq!(tick(&mut state, W, H, 0.43)[0], 0);
        assert_eq!(state.automation.as_ref().unwrap().status().completed, 0);

        let mut play = Runtime::default();
        play.capture = true;
        play.input = true;
        play.dua.running = true;
        let mut runner = crate::automation::Runner::new(recovery_config(), 0.0).unwrap();
        runner
            .queue(crate::automation::Job::Play { test: true })
            .unwrap();
        runner.suspend(0.1);
        play.automation = Some(runner);
        for at in [0.2, 20.0] {
            assert_eq!(tick(&mut play, W, H, at)[0], 0);
        }
        assert_eq!(play.automation.as_ref().unwrap().status().commands, 0);
        assert!(play.phone.input.is_none());
    }

    #[test]
    fn stop_rejects_old_launch_claim_and_ack_without_consuming_a_new_launch() {
        let mut state = recovery_ready();
        foreground(&mut state, false, 0.3);
        state
            .automation
            .as_mut()
            .unwrap()
            .queue(crate::automation::Job::Return { test: true })
            .unwrap();
        request_return(&mut state, 0.4);
        let old = tick(&mut state, W, H, 0.5);
        assert_eq!(old[4], 6);
        state.stop("screen locked");
        assert!(!claim_launch(&mut state, old[8], old[1], true, 0.51));
        ack(&mut state, old[8], old[1], true, 0.52);
        assert!(!state.active() && !state.phone.return_requested && !state.phone.returning);
        foreground(&mut state, true, 0.53);
        assert!(!state.active(), "unlock cannot resume a stopped launch");
        state.phone.foreground = false;
        state.dua.running = true;
        let mut runner = crate::automation::Runner::new(recovery_config(), 0.6).unwrap();
        runner
            .queue(crate::automation::Job::Return { test: true })
            .unwrap();
        state.automation = Some(runner);
        request_return(&mut state, 0.61);
        let new = tick(&mut state, W, H, 0.62);
        assert_eq!(new[4], 6);
        ack(&mut state, old[8], old[1], true, 0.63);
        assert_eq!(state.phone.input.unwrap().id, new[1]);
        assert!(!claim_launch(&mut state, old[8], old[1], true, 0.64));
        assert!(claim_launch(&mut state, new[8], new[1], true, 0.65));
    }

    #[test]
    fn expired_launch_claim_cannot_be_reported_as_a_successful_input() {
        let mut state = recovery_ready();
        foreground(&mut state, false, 0.3);
        state
            .automation
            .as_mut()
            .unwrap()
            .queue(crate::automation::Job::Return { test: true })
            .unwrap();
        request_return(&mut state, 0.4);
        let launch = tick(&mut state, W, H, 0.5);
        assert_eq!(launch[4], 6);
        assert!(!claim_launch(&mut state, launch[8], launch[1], true, 0.75));
        ack(&mut state, launch[8], launch[1], true, 0.76);
        assert!(
            !state.active(),
            "an unclaimed launch ACK cannot advance recovery"
        );
        assert!(!state.phone.return_requested && !state.phone.returning);
    }

    #[test]
    fn cancelling_a_queued_return_consumes_permission_without_relaunching() {
        let mut state = recovery_ready();
        foreground(&mut state, false, 0.3);
        state
            .automation
            .as_mut()
            .unwrap()
            .queue(crate::automation::Job::Return { test: true })
            .unwrap();
        request_return(&mut state, 0.4);
        assert!(state.phone.return_requested);
        state.cancel_feature("ret");
        assert!(!state.phone.return_requested && !state.phone.returning);
        assert_eq!(tick(&mut state, W, H, 0.5)[0], 0);
        assert_eq!(tick(&mut state, W, H, 20.0)[0], 0);
        assert!(state.automation.as_ref().unwrap().status().suspended);
        assert_eq!(state.automation.as_ref().unwrap().status().commands, 0);
    }

    fn pixels() -> Vec<u8> {
        let mut bytes = vec![0; W * H * 4];
        let mut put = |x: usize, y: usize, rgb: [u8; 3]| {
            bytes[(y * W + x) * 4..(y * W + x) * 4 + 3].copy_from_slice(&rgb);
        };
        for offset in 0..14 {
            for x in [110 + offset, 370 - offset] {
                for y in [95, 181] {
                    put(x, y, [248; 3]);
                }
            }
            for x in [110, 370] {
                for y in [95 + offset, 181 - offset] {
                    put(x, y, [248; 3]);
                }
            }
        }
        for y in 164..168 {
            for x in 170..179 {
                put(x, y, [20, 75, 205]);
            }
        }
        bytes
    }

    fn supply(state: &mut Runtime, request: &[i32], at: f64) -> Option<Vec<i32>> {
        assert_eq!(request[0], 1);
        let region = Region {
            x: request[3] as usize,
            y: request[4] as usize,
            width: request[5] as usize,
            height: request[6] as usize,
        };
        let full = pixels();
        let mut crop = Vec::new();
        for y in region.y..region.y + region.height {
            crop.extend_from_slice(
                &full[(y * W + region.x) * 4..(y * W + region.x + region.width) * 4],
            );
        }
        let frame = Frame::region(&crop, W, H, region.width * 4, 4, false, region).unwrap();
        observe(state, &frame, request[1], request[7], at)
    }

    fn first_tap(state: &mut Runtime) -> Vec<i32> {
        foreground(state, true, 0.1);
        let calibration = tick(state, W, H, 0.11);
        assert!(supply(state, &calibration, 0.12).is_none());
        assert!(state.fishing.phase == Phase::Idle && state.active());
        let cast = tick(state, W, H, 0.13);
        supply(state, &cast, 0.14).expect("fresh Fish observation must produce the cast")
    }

    #[test]
    fn headless_wait_needs_no_frame_and_updates_geometry_before_foreground() {
        let mut state = ready();
        let generation = state.generation;
        assert_eq!(tick(&mut state, W, H, 0.1), [0, generation, WAIT_MS]);
        assert_eq!(tick(&mut state, 640, 360, 0.2), [0, generation, WAIT_MS]);
        assert!(state.dua.running && state.active());
        assert!(state.phone.observation.is_none() && state.phone.input.is_none());
        assert_eq!(state.dua.screenshot_frames, 0);
        assert_eq!(state.dua.route, Some(Route::Headless));
    }

    #[test]
    fn automatic_calibration_handoff_and_input_ack_preserve_single_delivery() {
        let mut state = ready();
        let first = first_tap(&mut state);
        assert_eq!(first[0], 2);
        assert_eq!(&first[2..4], &[W as i32, H as i32]);
        assert_eq!(state.dua.screenshot_frames, 2);
        let generation = state.generation;
        assert_eq!(tick(&mut state, W, H, 0.15)[0], 0);
        assert_eq!(tick(&mut state, W, H, 0.16)[0], 0);
        assert_eq!(state.phone.input.unwrap().id, first[1]);
        ack(&mut state, generation, first[1] + 1, true, 0.17);
        assert!(state.phone.input.is_some());
        ack(&mut state, generation, first[1], true, 0.18);
        assert!(state.phone.input.is_none());
        let request = tick(&mut state, W, H, 3.3);
        let second = supply(&mut state, &request, 3.31).unwrap();
        assert_ne!(first[1], second[1]);
        ack(&mut state, generation, first[1], false, 3.32);
        assert!(state.dua.running);
        assert_eq!(state.phone.input.unwrap().id, second[1]);
    }

    #[test]
    fn restart_rejects_old_observations_without_consuming_the_new_request() {
        let mut state = ready();
        foreground(&mut state, true, 0.1);
        let old = tick(&mut state, W, H, 0.11);
        assert_eq!(tick(&mut state, W, H, 0.12), old);
        state.stop("test stop");
        state
            .start_dua_with_foreground(&["mfish".into()], 0.2, true)
            .unwrap();
        let current = tick(&mut state, W, H, 0.21);
        assert_ne!(old[1], current[1]);
        assert_ne!(old[7], current[7]);
        assert!(supply(&mut state, &old, 0.22).is_none());
        assert_eq!(state.phone.observation.unwrap().id, current[7]);
        assert!(supply(&mut state, &current, 0.23).is_none());
        assert!(state.fishing.calibration.is_some() && state.dua.running);
    }

    #[test]
    fn invalid_observation_stops_only_its_matching_generation_and_request() {
        let mut state = ready();
        foreground(&mut state, true, 0.1);
        let old = tick(&mut state, W, H, 0.11);
        state.stop("test restart");
        state
            .start_dua_with_foreground(&["mfish".into()], 0.2, true)
            .unwrap();
        let current = tick(&mut state, W, H, 0.21);
        reject_observation(&mut state, old[1], old[7]);
        reject_observation(&mut state, current[1], old[7]);
        reject_observation(&mut state, old[1], current[7]);
        assert!(state.dua.running);
        assert_eq!(state.phone.observation.unwrap().id, current[7]);
        reject_observation(&mut state, current[1], current[7]);
        assert!(!state.active() && state.phone.observation.is_none());
        assert_ne!(state.generation, current[1]);
        assert_eq!(
            state.message(),
            "휴대폰 관측 데이터를 읽지 못해 DUA를 중지했습니다."
        );
    }

    #[test]
    fn foreground_loss_discards_input_and_resumes_from_a_fresh_observation() {
        let mut state = ready();
        let tap = first_tap(&mut state);
        state.fishing.stats.success = 7;
        foreground(&mut state, false, 0.2);
        assert!(state.dua.running && state.fishing.phase == Phase::Idle);
        assert!(state.fishing.calibration.is_some());
        assert!(state.phone.input.is_none() && state.phone.observation.is_none());
        ack(&mut state, tap[8], tap[1], false, 0.21);
        assert!(state.dua.running);
        assert_eq!(tick(&mut state, W, H, 0.3)[0], 0);
        foreground(&mut state, true, 0.4);
        assert!(
            state.fishing.phase == Phase::Idle,
            "resume waits for a fresh screen before fishing"
        );
        assert_eq!(state.fishing.stats.success, 7);
        let fresh = tick(&mut state, W, H, 0.41);
        assert_eq!(fresh[0], 1);
        assert!(
            supply(&mut state, &fresh, 0.42).is_some(),
            "fresh screen resumes calibrated fishing"
        );
        state.stop("screen locked");
        foreground(&mut state, false, 0.5);
        foreground(&mut state, true, 0.6);
        assert!(!state.active(), "unlock never restarts a stopped macro");
    }

    #[test]
    fn resize_or_wrong_roi_stops_pending_observation_and_input_timeout_stops() {
        let mut state = ready();
        foreground(&mut state, true, 0.1);
        tick(&mut state, W, H, 0.11);
        assert_eq!(tick(&mut state, 640, 360, 0.12)[0], 0);
        assert!(!state.dua.running && state.phone.observation.is_none());

        let mut state = ready();
        let tap = first_tap(&mut state);
        tick(&mut state, W, H, 3.0);
        assert!(!state.active() && state.phone.input.is_none());
        ack(&mut state, tap[8], tap[1], true, 3.1);
        assert!(!state.active());

        let mut state = ready();
        first_tap(&mut state);
        assert_eq!(tick(&mut state, 640, 360, 0.15)[0], 0);
        assert!(!state.dua.running);

        let mut state = ready();
        let tap = first_tap(&mut state);
        ack(&mut state, tap[8], tap[1], true, 0.2);
        let request = tick(&mut state, W, H, 0.5);
        assert_eq!(request[0], 1);
        let data = pixels();
        let full = Frame::new(&data, W, H, W * 4, 4, false).unwrap();
        assert!(observe(&mut state, &full, request[1], request[7], 0.51).is_none());
        assert!(
            !state.dua.running,
            "full pixels cannot masquerade as the requested ROI"
        );
    }

    #[test]
    fn phone_supports_all_features_without_replaying_legacy_travel() {
        let mut state = Runtime::default();
        state.capture = true;
        state.input = true;
        let mut config = state.macro_config();
        config["useMove"] = false.into();
        config["useSell"] = false.into();
        state.macro_settings = config;
        state.navigation.options.use_move = true;
        state.navigation.options.use_sell = true;
        state
            .navigation
            .positions
            .insert("stick_center".into(), Point(0.2, 0.8));
        state.navigation.fish_trace = serde_json::from_value(serde_json::json!([
            {"k":"F","t":0.0,"dx":0.0,"dy":-1.0,"a":true,"b":false}
        ]))
        .unwrap();
        for (index, feature) in [
            "mfish", "mpop", "mitem", "mblood", "mmerch", "mcraft", "mmatch",
        ]
        .into_iter()
        .enumerate()
        {
            state
                .start_dua_with_foreground(&[feature.into()], index as f64, true)
                .unwrap();
            assert!(state.dua.running && state.automation.is_some());
            assert!(state.job.is_none() && state.wayfinder.is_none());
            state.stop("next feature");
        }
        // Old paths remain editable data; only the new settings choose execution.
        assert!(state.navigation.options.use_move && state.navigation.options.use_sell);
        assert_eq!(state.navigation.fish_trace.len(), 1);
        state
            .start_dua_with_foreground(&["mfish".into()], 10.0, true)
            .unwrap();
        foreground(&mut state, true, 10.1);
        let request = tick(&mut state, W, H, 10.2);
        assert_eq!(request[0], 1);
        assert!(supply(&mut state, &request, 10.21).is_none());
        assert!(state.fishing.calibration.is_some());
        assert!(state.job.is_none() && state.wayfinder.is_none());
        assert!(input(
            &mut state,
            DeviceTick::Input(GameInput::Motion(crate::Motion::Swipe)),
            (W, H),
            10.3
        )
        .is_none());
        assert!(
            !state.dua.running,
            "retired timed input cannot enter the phone transport"
        );
    }

    #[test]
    fn structured_text_keeps_utf16_and_a_failed_ack_cancels_its_generation() {
        let mut state = ready();
        foreground(&mut state, true, 0.1);
        let generation = state.generation;
        let packet = input(
            &mut state,
            DeviceTick::Input(GameInput::Command(crate::automation::Command::Text {
                text: "Acrux 한글 😀".into(),
                submit: true,
            })),
            (W, H),
            0.2,
        )
        .unwrap();
        assert_eq!(packet[4], 4);
        assert_eq!(packet[6], 1);
        assert_eq!(
            String::from_utf16(&packet[9..].iter().map(|&u| u as u16).collect::<Vec<_>>()).unwrap(),
            "Acrux 한글 😀"
        );
        ack(&mut state, generation, packet[1], false, 0.21);
        assert!(!state.active() && !state.input_current(generation));
        assert!(state.phone.input.is_none());
        ack(&mut state, generation, packet[1], true, 0.22);
        assert!(
            !state.active(),
            "late success cannot resurrect canceled text"
        );
    }

    #[test]
    fn place_registration_waits_for_first_game_entry_and_cancels_when_leaving_it() {
        let mut state = Runtime {
            capture: true,
            input: true,
            ..Runtime::default()
        };
        state.wayfinder = Some(
            crate::wayfinder::Wayfinder::start_mark(
                crate::navigator::Destination::Sell,
                state.movement_settings(),
                0.,
            )
            .unwrap(),
        );
        state.dua.running = true;
        let (tx, rx) = std::sync::mpsc::channel();
        state.sdk_request = Some(crate::sdk::Request::Mark {
            ticket: 41,
            reply: tx,
        });
        let generation = state.generation;
        foreground(&mut state, true, 0.1);
        assert_eq!(state.generation, generation);
        assert!(state.wayfinder.is_some() && state.automation.is_none());
        assert!(matches!(
            rx.try_recv(),
            Err(std::sync::mpsc::TryRecvError::Empty)
        ));
        assert!(state.fishing.phase == crate::fishing::Phase::Idle);
        foreground(&mut state, false, 0.2);
        assert!(rx.recv().unwrap().unwrap_err().contains("등록을 취소"));
        assert!(!state.dua.running && state.wayfinder.is_none());
        foreground(&mut state, true, 0.3);
        assert!(state.fishing.phase == crate::fishing::Phase::Idle);
    }

    #[test]
    fn sdk_requests_cancel_on_foreground_loss_and_never_replay_after_return() {
        use crate::{
            automation::{Command, Job},
            sdk::Request,
        };
        use std::sync::mpsc;
        let mut idle = Runtime::default();
        let idle_generation = idle.generation;
        foreground(&mut idle, true, 0.01);
        foreground(&mut idle, false, 0.02);
        assert_eq!(
            idle.generation, idle_generation,
            "idle game entry keeps the intended request generation"
        );
        for mode in 0..3 {
            let mut state = ready();
            foreground(&mut state, true, 0.1);
            let (tx, rx) = mpsc::channel();
            let mut old_packet = None;
            if mode == 0 {
                state.sdk_request = Some(Request::Inspect {
                    ticket: 41,
                    method: "ocr_boxes".into(),
                    rect: None,
                    pixels: None,
                    reply: tx,
                });
            } else {
                let command = Command::Text {
                    text: "must not replay".into(),
                    submit: false,
                };
                state
                    .automation
                    .as_mut()
                    .unwrap()
                    .queue(Job::Command {
                        command: command.clone(),
                    })
                    .unwrap();
                state.sdk_request = Some(Request::Input {
                    ticket: 42,
                    command: serde_json::to_value(command).unwrap(),
                    reply: tx,
                    seen: false,
                });
                if mode == 2 {
                    let mut packet = tick(&mut state, W, H, 0.11);
                    if packet[0] == 1 {
                        packet = supply(&mut state, &packet, 0.12).unwrap();
                    }
                    assert_eq!(packet[0], 2);
                    assert_eq!(packet[4], 4);
                    old_packet = Some(packet);
                }
            }
            let old_generation = state.generation;
            let commands = state.automation.as_ref().unwrap().status().commands;
            foreground(&mut state, false, 0.2);
            assert!(
                rx.try_recv().unwrap().is_err(),
                "the caller receives cancellation for inspect, queued and pending SDK requests"
            );
            assert_ne!(state.generation, old_generation);
            assert!(state.sdk_request.is_none() && !state.phone.has_pending());
            assert!(state.automation.as_ref().unwrap().status().suspended);
            if let Some(packet) = old_packet {
                ack(&mut state, packet[8], packet[1], true, 0.21);
            }
            assert!(
                !state.phone.has_pending(),
                "a late ACK cannot resurrect canceled SDK input"
            );
            foreground(&mut state, true, 0.3);
            let request = tick(&mut state, W, H, 0.31);
            assert_eq!(request[0], 1, "return requires a new screenshot");
            let next = supply(&mut state, &request, 0.32);
            assert!(next.as_ref().is_none_or(|packet| packet[4] != 4));
            assert_eq!(
                state.automation.as_ref().unwrap().status().commands,
                commands,
                "an old queued command cannot enter the new generation"
            );
            state.stop("test cleanup");
        }
    }

    #[test]
    fn feature_cancel_invalidates_only_its_travel_owner_and_ignores_old_ack() {
        use crate::automation::Job;
        let mut state = ready();
        foreground(&mut state, true, 0.1);
        state
            .automation
            .as_mut()
            .unwrap()
            .queue(Job::Craft {
                name: "Haste Potion I".into(),
                calibrate: false,
            })
            .unwrap();
        let request = tick(&mut state, W, H, 0.11);
        assert_eq!(request[0], 1);
        assert!(supply(&mut state, &request, 0.12).is_none());
        assert!(
            state.wayfinder.is_some(),
            "the real Craft task must own observed travel"
        );
        let request = tick(&mut state, W, H, 0.13);
        assert_eq!(request[0], 1);
        let packet = supply(&mut state, &request, 0.14).unwrap();
        assert_eq!(packet[0], 2);
        let generation = state.generation;
        state.cancel_feature("mmatch");
        assert_eq!(
            state.generation, generation,
            "stopping an unrelated feature preserves the current owner"
        );
        assert!(state.wayfinder.is_some() && state.phone.input.is_some());
        state.cancel_feature("mcraft");
        assert_ne!(state.generation, generation);
        assert!(state.wayfinder.is_none() && !state.phone.has_pending());
        assert!(
            state.dua.running,
            "stopping Craft preserves the independently enabled fishing macro"
        );
        ack(&mut state, packet[8], packet[1], true, 0.15);
        assert!(state.wayfinder.is_none() && !state.phone.has_pending());
        let request = tick(&mut state, W, H, 0.3);
        assert_eq!(request[0], 1);
        assert!(supply(&mut state, &request, 0.31).is_none());
        assert!(
            state.wayfinder.is_none(),
            "useMove=false cannot start new travel after canceling Craft"
        );
        assert!(
            state.fishing.calibration.is_some(),
            "fishing survives after a fresh real panel observation"
        );
        state.stop("test cleanup");
    }

    #[test]
    fn compositor_age_and_delayed_dispatch_share_one_input_deadline() {
        let mut state = ready();
        let first = first_tap(&mut state);
        assert!(state.phone.can_start(first[8], first[1], 0.15));
        assert!(!state.phone.can_start(first[8], first[1], 0.50));
        ack(&mut state, first[8], first[1], true, 0.16);
        let request = tick(&mut state, W, H, 3.3);
        let bytes = pixels();
        let region = state.phone.observation.unwrap().region;
        let mut crop = Vec::new();
        for y in region.y..region.y + region.height {
            crop.extend_from_slice(
                &bytes[(y * W + region.x) * 4..(y * W + region.x + region.width) * 4],
            );
        }
        let frame = Frame::region(&crop, W, H, region.width * 4, 4, false, region).unwrap();
        assert!(observe_with_age(&mut state, &frame, request[1], request[7], 3.31, 0.25).is_none());
        assert!(
            state.phone.observation.is_some(),
            "expired pixels do not advance the requested observation"
        );
        assert!(observe_with_age(&mut state, &frame, request[1], request[7], 3.32, 0.24).is_none(),
            "the captured Fish frame predates the three-second cast retry, even if OCR completed later");
        let request = tick(&mut state, W, H, 3.4);
        let packet =
            observe_with_age(&mut state, &frame, request[1], request[7], 3.42, 0.24).unwrap();
        assert!(state.phone.can_start(packet[8], packet[1], 3.47));
        assert!(
            !state.phone.can_start(packet[8], packet[1], 3.49),
            "delayed UI dispatch cannot reset the 300ms evidence deadline"
        );
    }
}

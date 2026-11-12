//! Foreground phone transport for the same rule-based DUA runtime.
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
}

#[derive(Clone, Copy)]
struct Input {
    id: i32,
    generation: i32,
    until: f64,
}

#[derive(Default)]
pub(super) struct State {
    pub(super) foreground: bool,
    geometry: Option<(usize, usize)>,
    next_id: i32,
    observation: Option<Observation>,
    input: Option<Input>,
}

impl State {
    pub(super) fn cancel(&mut self) {
        self.observation = None;
        self.input = None;
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
    ]
}

fn stop(state: &mut Runtime, message: &'static str) {
    state.stop(message);
    state.record();
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
    let GameInput::Tap(tap) = command else {
        stop(
            state,
            "모바일 DUA에서 시간 기반 이동 명령을 실행할 수 없습니다.",
        );
        return None;
    };
    if !state.input_current(state.generation) || !state.phone.foreground {
        return None;
    }
    let duration = tap.duration.as_millis();
    if duration == 0 || duration > 1000 || tap.x >= geometry.0 || tap.y >= geometry.1 {
        stop(state, "모바일 DUA의 터치 명령이 올바르지 않습니다.");
        return None;
    }
    let id = state.phone.id();
    state.phone.input = Some(Input {
        id,
        generation: state.generation,
        until: at + duration as f64 / 1000.0 + INPUT_GRACE,
    });
    Some(vec![
        2,
        id,
        geometry.0 as i32,
        geometry.1 as i32,
        0,
        duration as i32,
        tap.x as i32,
        tap.y as i32,
        state.generation,
    ])
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
        && (state.phone.observation.is_some() || state.phone.input.is_some())
    {
        state.phone.geometry = Some(geometry);
        stop(state, "휴대폰 화면 크기가 바뀌어 DUA를 중지했습니다.");
        return wait(state, WAIT_MS);
    }
    state.phone.geometry = Some(geometry);
    if !state.phone.foreground {
        state.phone.cancel();
        state.route(Route::Headless, "Roblox가 앞에 오기를 기다립니다.", false);
        return wait(state, WAIT_MS);
    }
    if !state.capture || !state.input {
        stop(state, "화면 캡처와 접근성 입력을 다시 준비하세요.");
        return wait(state, WAIT_MS);
    }
    if let Some(pending) = state.phone.input {
        if at >= pending.until {
            stop(state, "휴대폰 터치의 완료 응답이 없어 DUA를 중지했습니다.");
        } else {
            state.route(Route::Headless, "휴대폰 터치가 끝나기를 기다립니다.", false);
        }
        return wait(state, 25);
    }
    if let Some(observation) = state.phone.observation {
        return request(observation);
    }
    match state.tick(at, geometry, None) {
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
    state.phone.observation = None;
    let step = state.tick(at, observation.geometry, Some(frame));
    input(state, step, observation.geometry, at)
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
    let message = if state.active() {
        "Roblox가 앞에 오기를 기다립니다."
    } else {
        state.message()
    };
    state.phone.foreground = foreground;
    state.resume_after_calibration = false;
    state.finish_tool(message);
    if state.dua.running {
        state.route(Route::Headless, "Roblox 화면을 다시 확인합니다.", false);
        if foreground {
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
    } else if !success {
        stop(state, "휴대폰 터치가 취소되어 DUA를 중지했습니다.");
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
        state
            .start_dua_with_foreground(&["mfish".into()], 0.0, true)
            .unwrap();
        state
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
        assert!(state.fishing.phase == Phase::Searching);
        assert_eq!(state.fishing.stats.success, 7);
        assert_eq!(tick(&mut state, W, H, 0.41)[0], 1);
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
    fn phone_start_clamps_retired_travel_and_rejects_unsupported_features() {
        let mut state = Runtime::default();
        state.capture = true;
        state.input = true;
        state.navigation.options.use_move = true;
        state.navigation.options.use_sell = true;
        state
            .navigation
            .positions
            .insert("stick_center".into(), Point(0.2, 0.8));
        state.navigation.fish_trace = serde_json::from_value(serde_json::json!([
            {"k":"s","t":0.0,"dx":0.0,"dy":0.0,"a":false,"b":false}
        ]))
        .unwrap();
        assert!(state
            .start_dua_with_foreground(&["mitem".into()], 0.0, true)
            .is_err());
        state
            .start_dua_with_foreground(&["mfish".into()], 0.1, true)
            .unwrap();
        assert!(!state.navigation.options.use_move && !state.navigation.options.use_sell);
        assert!(state.job.is_none());
        assert!(state.navigation.positions.contains_key("stick_center"));
        assert_eq!(state.navigation.fish_trace.len(), 1);
        foreground(&mut state, true, 0.2);
        assert!(input(
            &mut state,
            DeviceTick::Input(GameInput::Motion(crate::Motion::Swipe)),
            (W, H),
            0.3
        )
        .is_none());
        assert!(!state.dua.running);
    }
}

//! Closed-loop navigation inside one actually observed, fixed-camera patch.
//! Coordinates and distances stay in image pixels; this module invents no world
//! height, camera projection, global map, or traversable jump connection.
use crate::{
    device::Gesture,
    scene::{self, Snapshot},
    vision::{Frame, Point, Rect, Region},
};
use serde::{Deserialize, Serialize};

const PROBE_MS: u32 = 100;
const MAX_STEP_MS: u32 = 250;
const SETTLE_SECONDS: f64 = 0.12;
const MAX_SECONDS: f64 = 45.0;
const MAX_ACTIONS: u32 = 64;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Destination {
    Base,
    Coast,
    Sell,
    Craft,
    Match,
}

/// Contacts and seeds must be observed on this very frame, after camera setup.
/// A coastal region denotes observed LAND next to water, never a water color.
/// Seeds witness one visible local surface. They are not a supplied game map.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Observed {
    pub avatar_contact: Point,
    pub avatar_bounds: Rect,
    pub goal_region: Rect,
    pub ground_seeds: Vec<Point>,
    pub included: Rect,
    pub excluded: Vec<Rect>,
    /// Observed projected footprint plus image localization error, in pixels.
    pub clearance_pixels: f64,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Controls {
    pub stick_center: Point,
    pub stick_edge: Point,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Calibration {
    /// Avatar movement relative to the observed ground, pixels / millisecond.
    pub right: [f64; 2],
    pub up: [f64; 2],
    pub error_pixels: f64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Status {
    pub destination: Destination,
    pub phase: &'static str,
    pub message: String,
    pub observations: u32,
    pub actions: u32,
    pub remaining_pixels: f64,
    pub calibration: Option<Calibration>,
    /// Local geometric approach only. The caller must check the real task UI.
    pub local_goal_reached: bool,
}

#[derive(Debug)]
pub(crate) enum Tick {
    Observe,
    Wait(u32),
    Gesture(Gesture),
    Done,
    Stopped(String),
}

#[derive(Clone, Copy)]
enum Action {
    Probe(usize, f64),
    Move {
        before_distance: f64,
        expected: [f64; 2],
    },
}

enum Phase {
    Stable,
    Ready,
    Input {
        action: Action,
        sent_at: f64,
        duration: u32,
    },
    After {
        action: Action,
        not_before: f64,
    },
    Done,
    Stopped,
}

struct GroundSample {
    rgb: [f64; 3],
    spread: f64,
}

struct Ground {
    hull: Vec<[f64; 2]>,
    samples: Vec<GroundSample>,
    excluded: Vec<Region>,
    self_mask: Region,
}

pub(crate) struct Navigator {
    state: Status,
    phase: Phase,
    controls: Controls,
    geometry: (usize, usize),
    included: Region,
    excluded: Vec<Region>,
    avatar_region: Region,
    avatar: [f64; 2],
    goal: [f64; 4],
    clearance: f64,
    // Grid already certifies production segments. Raw observations retain
    // their local surface model; do not classify a planned route a second time.
    ground: Option<Ground>,
    world: Snapshot,
    body: Option<Snapshot>,
    response: [[f64; 2]; 2],
    probes: usize,
    started: f64,
    last_now: f64,
    last_frame: f64,
    no_progress: u8,
}

impl Navigator {
    pub(crate) fn from_frame(
        destination: Destination,
        frame: &Frame<'_>,
        observed: &Observed,
        controls: Controls,
        now: f64,
    ) -> Result<Self, String> {
        Self::from_frame_inner(destination, frame, observed, controls, now, false, None)
    }

    /// The caller has already checked this segment against its terrain grid.
    /// Keep one motion controller, without repeating hue/seed-hull checks over
    /// terrain already accepted by the path planner.
    pub(crate) fn from_route_frame(
        destination: Destination,
        frame: &Frame<'_>,
        observed: &Observed,
        controls: Controls,
        now: f64,
        calibration: Option<Calibration>,
    ) -> Result<Self, String> {
        Self::from_frame_inner(
            destination,
            frame,
            observed,
            controls,
            now,
            true,
            calibration,
        )
    }

    fn from_frame_inner(
        destination: Destination,
        frame: &Frame<'_>,
        observed: &Observed,
        controls: Controls,
        now: f64,
        planned: bool,
        calibration: Option<Calibration>,
    ) -> Result<Self, String> {
        if !now.is_finite() || now < 0.0 || !frame.is_full() {
            return Err("새 전체 화면 관측이 필요합니다.".into());
        }
        let geometry = (frame.width, frame.height);
        let included = checked_region(observed.included, geometry)?;
        let avatar_region = checked_region(observed.avatar_bounds, geometry)?;
        let goal = checked_region(observed.goal_region, geometry)?;
        if observed.excluded.len() > 24
            || (!planned && !(3..=16).contains(&observed.ground_seeds.len()))
            || !point_valid(observed.avatar_contact)
            || !point_valid(controls.stick_center)
            || !point_valid(controls.stick_edge)
            || !observed.clearance_pixels.is_finite()
            || !(2.0..=128.0).contains(&observed.clearance_pixels)
        {
            return Err("현재 화면 중심·지형·입력 위치를 확인하세요.".into());
        }
        let avatar = pixel_point(observed.avatar_contact, geometry);
        if !contains(avatar_region, avatar) || !contains(included, avatar) {
            return Err("화면 중심은 현재 관측 영역 안에 있어야 합니다.".into());
        }
        if goal.width as f64 <= observed.clearance_pixels * 2.0
            || goal.height as f64 <= observed.clearance_pixels * 2.0
            || goal.x < included.x
            || goal.y < included.y
            || goal.x + goal.width > included.x + included.width
            || goal.y + goal.height > included.y + included.height
        {
            return Err("목표 접근 영역이 발판 여유보다 작거나 현재 관측 밖입니다.".into());
        }
        let mut excluded: Vec<_> = observed
            .excluded
            .iter()
            .map(|rect| checked_region(*rect, geometry))
            .collect::<Result<_, _>>()?;
        if excluded.iter().any(|r| contains(*r, avatar)) {
            return Err("화면 중심이 제외 영역에 가려져 있습니다.".into());
        }
        excluded.push(avatar_region);
        let radius = stick_radius(controls, geometry);
        if !(8.0..=(frame.width.min(frame.height) as f64 * 0.3)).contains(&radius) {
            return Err("화면에서 저장한 조이스틱 가운데와 가장자리가 필요합니다.".into());
        }
        // Both orthogonal endpoints must be real, reachable screen positions.
        stick(controls, geometry, [1.0, 0.0], PROBE_MS)?;
        stick(controls, geometry, [0.0, 1.0], PROBE_MS)?;
        let ground = if planned {
            None
        } else {
            let ground = Ground::learn(
                frame,
                &observed.ground_seeds,
                included,
                &excluded,
                avatar_region,
            )?;
            if !inside_polygon(&ground.hull, avatar) {
                return Err("현재 위치를 둘러싼 지형 관측이 부족합니다.".into());
            }
            Some(ground)
        };
        let calibration = calibration.filter(|c| {
            c.right.into_iter().chain(c.up).all(f64::is_finite)
                && conditioning([c.right, c.up]) >= 0.25
                && c.error_pixels.is_finite()
                && c.error_pixels >= 0.
                && c.error_pixels <= observed.clearance_pixels
        });
        let world = Snapshot::capture(frame, included, &excluded)?;
        // Planned camera-follow walking uses only terrain registration; do not
        // inspect or try to match player pixels. The raw tracking API retains
        // its separately observed body texture contract.
        let body = if planned {
            None
        } else {
            Snapshot::capture(frame, avatar_region, &[]).ok()
        };
        let mut navigator = Self {
            state: Status {
                destination,
                phase: "stabilizing",
                message: "고정된 시점과 현재 지면을 확인합니다.".into(),
                observations: 1,
                actions: 0,
                remaining_pixels: 0.0,
                calibration,
                local_goal_reached: false,
            },
            phase: Phase::Stable,
            controls,
            geometry,
            included,
            excluded,
            avatar_region,
            avatar,
            goal: [
                goal.x as f64,
                goal.y as f64,
                (goal.x + goal.width) as f64,
                (goal.y + goal.height) as f64,
            ],
            clearance: observed.clearance_pixels,
            ground,
            world,
            body,
            response: calibration
                .map(|c| [c.right, c.up])
                .unwrap_or([[0.0; 2]; 2]),
            probes: if calibration.is_some() { 2 } else { 0 },
            started: now,
            last_now: now,
            last_frame: now,
            no_progress: 0,
        };
        navigator.state.remaining_pixels = navigator.distance();
        Ok(navigator)
    }

    pub(crate) fn status(&self) -> Status {
        self.state.clone()
    }

    /// Call only with a fresh captured frame from the current execution/input
    /// generation. `now` is monotonic; the transport owns capture age checks.
    pub(crate) fn tick(&mut self, now: f64, frame: Option<&Frame<'_>>) -> Tick {
        self.tick_inner(now, frame, None)
    }

    /// Production observation: the player-follow camera supplies its origin
    /// and occupancy mask in this fresh frame. Walking needs neither player
    /// identity nor physical foot/ground evidence (jumping owns that evidence).
    pub(crate) fn tick_observed(
        &mut self,
        now: f64,
        frame: Option<&Frame<'_>>,
        contact: Point,
        bounds: Rect,
        error_pixels: f64,
    ) -> Tick {
        if !point_valid(contact)
            || !error_pixels.is_finite()
            || error_pixels < 0.
            || error_pixels > self.clearance * 0.5
        {
            return self.stop("현재 화면 중심의 위치 관측이 유효하지 않습니다.");
        }
        if bounds
            .region(self.geometry)
            .is_none_or(|r| !contains(r, pixel_point(contact, self.geometry)))
        {
            return self.stop("현재 화면 중심과 제외 영역이 일치하지 않습니다.");
        }
        self.tick_inner(now, frame, Some((contact, bounds, error_pixels)))
    }

    fn tick_inner(
        &mut self,
        now: f64,
        frame: Option<&Frame<'_>>,
        observed: Option<(Point, Rect, f64)>,
    ) -> Tick {
        if matches!(self.phase, Phase::Done) {
            return Tick::Done;
        }
        if matches!(self.phase, Phase::Stopped) {
            return Tick::Stopped(self.state.message.clone());
        }
        if !now.is_finite() || now < self.last_now || now - self.started > MAX_SECONDS {
            return self.stop("이동 관측 시간 제한을 넘었습니다. 위치를 다시 확인하세요.");
        }
        self.last_now = now;
        if let Phase::Input {
            sent_at, duration, ..
        } = self.phase
        {
            return if now - sent_at > duration as f64 / 1000.0 + 2.0 {
                self.stop("이동 입력 완료를 확인하지 못했습니다.")
            } else {
                Tick::Wait(20)
            };
        }
        if let Phase::After { not_before, .. } = self.phase {
            if now < not_before {
                return Tick::Wait(((not_before - now) * 1000.0).ceil().clamp(1.0, 1000.0) as u32);
            }
        }
        let Some(frame) = frame else {
            return Tick::Observe;
        };
        if now <= self.last_frame {
            return Tick::Observe;
        }
        if !frame.is_full() || (frame.width, frame.height) != self.geometry {
            return self.stop("화면 크기나 시점이 달라졌습니다. 지면을 다시 관측하세요.");
        }
        self.last_frame = now;
        self.state.observations += 1;
        let action = match self.phase {
            Phase::After { action, .. } => Some(action),
            _ => None,
        };
        let mut registration_excluded = self.excluded.clone();
        if let Some((_, bounds, _)) = observed {
            let region = bounds.region(self.geometry).unwrap();
            if let Err(e) = self.world.exclude_region(region) {
                return self.stop_error(e);
            }
            if !registration_excluded.contains(&region) {
                registration_excluded.push(region);
            }
        }
        let world = match Snapshot::capture(frame, self.included, &registration_excluded) {
            Ok(value) => value,
            Err(error) => return self.stop_error(error),
        };
        let shift = scene::displacement_limit(self.geometry);
        let (movement, error, body) = if let Some((contact, bounds, contact_error)) = observed {
            let registration = match scene::register(&self.world, &world, shift) {
                Ok(v) => v,
                Err(e) => return self.stop_error(e),
            };
            if !registration.supports(self.avatar)
                || (self.ground.is_some() && !registration.supports(self.target()))
            {
                return self
                    .stop("화면 중심 주변의 지형 특징이 부족해 이동량을 측정하지 못했습니다.");
            }
            let predicted = registration.map_point(self.avatar);
            let actual = pixel_point(contact, self.geometry);
            let movement = [actual[0] - predicted[0], actual[1] - predicted[1]];
            let error = registration.error_pixels + contact_error;
            self.avatar = actual;
            let center = registration.map_point([
                (self.goal[0] + self.goal[2]) / 2.,
                (self.goal[1] + self.goal[3]) / 2.,
            ]);
            let half = [
                (self.goal[2] - self.goal[0]) / 2.,
                (self.goal[3] - self.goal[1]) / 2.,
            ];
            let a = registration.affine;
            let determinant = a[0][0] * a[1][1] - a[0][1] * a[1][0];
            if determinant <= 0.5 {
                return self.stop("지면 변형이 커서 목표 접근 영역을 유지할 수 없습니다.");
            }
            let candidate = [
                half[0] * registration.scale[0],
                half[1] * registration.scale[1],
            ];
            let x = (a[1][1].abs() * candidate[0] + a[0][1].abs() * candidate[1]) / determinant;
            let y = (a[1][0].abs() * candidate[0] + a[0][0].abs() * candidate[1]) / determinant;
            let shrink = (half[0] / x).min(half[1] / y).min(1.);
            self.goal = [
                center[0] - candidate[0] * shrink,
                center[1] - candidate[1] * shrink,
                center[0] + candidate[0] * shrink,
                center[1] + candidate[1] * shrink,
            ];
            if let Some(ground) = &mut self.ground {
                ground.transform(&registration);
            }
            for response in &mut self.response {
                *response = registration.map_vector(*response);
            }
            self.avatar_region = bounds.region(self.geometry).unwrap();
            self.excluded.pop();
            self.excluded.push(self.avatar_region);
            if let Some(ground) = &mut self.ground {
                ground.excluded = self.excluded.clone();
                ground.self_mask = self.avatar_region;
            }
            (movement, error, None)
        } else {
            let body = match Snapshot::capture(frame, self.avatar_region, &[]) {
                Ok(v) => v,
                Err(e) => return self.stop_error(e),
            };
            let Some(previous) = self.body.as_ref() else {
                return self.stop("몸의 특징이 부족합니다. 실제 발 접점을 새 화면에서 관측하세요.");
            };
            let ground_flow = match scene::translation(&self.world, &world, shift) {
                Ok(v) => v,
                Err(e) => return self.stop_error(e),
            };
            let body_flow = match scene::translation(previous, &body, shift) {
                Ok(v) => v,
                Err(e) => return self.stop_error(e),
            };
            let movement = [body_flow.dx - ground_flow.dx, body_flow.dy - ground_flow.dy];
            self.avatar[0] += body_flow.dx;
            self.avatar[1] += body_flow.dy;
            for i in [0, 2] {
                self.goal[i] += ground_flow.dx;
            }
            for i in [1, 3] {
                self.goal[i] += ground_flow.dy;
            }
            if let Some(ground) = &mut self.ground {
                ground.translate(ground_flow.dx, ground_flow.dy);
            }
            (
                movement,
                ground_flow.error_pixels + body_flow.error_pixels,
                Some(body),
            )
        };
        let error_limit = if self.ground.is_some() {
            self.clearance * 0.5
        } else {
            self.clearance
        };
        if !error.is_finite() || error > error_limit {
            return self.stop("지형 이동량의 측정 오차가 큽니다. 다시 관측합니다.");
        }
        if action.is_none() && length(movement) > 2.0 + error {
            return self.stop("입력 없이 화면이 움직였습니다. 시점을 다시 고정하세요.");
        }
        self.world = if observed.is_some() {
            match Snapshot::capture(frame, self.included, &self.excluded) {
                Ok(v) => v,
                Err(e) => return self.stop_error(e),
            }
        } else {
            world
        };
        self.body = body;
        self.state.remaining_pixels = self.distance();
        self.phase = Phase::Ready;
        if let Some(action) = action {
            match action {
                Action::Probe(axis, sign) => {
                    if length(movement) < 2.0 + error
                        || length(movement) > self.geometry.0.min(self.geometry.1) as f64 * 0.12
                    {
                        return self.stop("짧은 입력에 따른 실제 이동량을 확인하지 못했습니다.");
                    }
                    self.response[axis] = [
                        movement[0] / (PROBE_MS as f64 * sign),
                        movement[1] / (PROBE_MS as f64 * sign),
                    ];
                    self.probes = axis + 1;
                    if self.probes == 2 {
                        if conditioning(self.response) < 0.25 {
                            return self.stop("두 이동 방향을 구분하지 못했습니다. 시점과 조이스틱을 다시 확인하세요.");
                        }
                        self.state.calibration = Some(Calibration {
                            right: self.response[0],
                            up: self.response[1],
                            error_pixels: error,
                        });
                    }
                }
                Action::Move {
                    before_distance,
                    expected,
                } => {
                    let expected_length = length(expected);
                    let actual_length = length(movement);
                    let residual = length([movement[0] - expected[0], movement[1] - expected[1]]);
                    if actual_length < 1.0 + error
                        || dot(movement, expected) <= 0.0
                        || actual_length > expected_length * 2.5 + error + 3.0
                        || residual > error + (expected_length * 0.5).max(3.0)
                    {
                        return self.stop(
                            "계획한 짧은 이동과 실제 화면 변화가 다릅니다. 재관측이 필요합니다.",
                        );
                    }
                    if self.state.remaining_pixels + 1.0 + error >= before_distance {
                        self.no_progress += 1;
                    } else {
                        self.no_progress = 0;
                    }
                    if self.no_progress >= 2 {
                        return self.stop("목표까지 진행되지 않습니다. 지형을 다시 확인하세요.");
                    }
                }
            }
        }
        if self.probes == 2 {
            self.state.calibration = Some(Calibration {
                right: self.response[0],
                up: self.response[1],
                error_pixels: error,
            });
        }
        self.next(frame)
    }

    /// Planning can take time after the frame is observed. Start the pending
    /// input timeout at actual dispatch, without changing any image evidence.
    pub(crate) fn dispatch_at(&mut self, at: f64) {
        if let Phase::Input { sent_at, .. } = &mut self.phase {
            if at.is_finite() && at >= *sent_at {
                *sent_at = at;
            }
        }
    }

    /// Acknowledgement means every finger is released. Never infer success from
    /// elapsed milliseconds or observe a pre-input buffered frame as completion.
    pub(crate) fn ack(&mut self, success: bool, now: f64) {
        let Phase::Input {
            action,
            sent_at,
            duration,
        } = self.phase
        else {
            return;
        };
        if !success
            || !now.is_finite()
            || now < sent_at
            || now - sent_at > duration as f64 / 1000.0 + 2.0
        {
            self.stop("이동 입력이 취소되거나 완료되지 않았습니다.");
            return;
        }
        self.last_now = self.last_now.max(now);
        self.phase = Phase::After {
            action,
            not_before: now + SETTLE_SECONDS,
        };
        self.state.phase = "observing";
        self.state.message = "입력 후 새 지형에서 이동량을 확인합니다.".into();
    }

    fn next(&mut self, frame: &Frame<'_>) -> Tick {
        if self
            .ground
            .as_ref()
            .is_some_and(|ground| !inside_polygon(&ground.hull, self.avatar))
        {
            return self.stop("현재 위치가 관측한 지형 영역을 벗어났습니다.");
        }
        // Arrival must fit INSIDE the actual observed approach region. The
        // controller aims further inside it, so quantized short inputs do not
        // asymptotically stop just outside its boundary.
        if self.avatar[0] - self.clearance >= self.goal[0]
            && self.avatar[0] + self.clearance <= self.goal[2]
            && self.avatar[1] - self.clearance >= self.goal[1]
            && self.avatar[1] + self.clearance <= self.goal[3]
        {
            self.phase = Phase::Done;
            self.state.phase = "arrived";
            self.state.local_goal_reached = true;
            self.state.remaining_pixels = 0.0;
            self.state.message =
                "관측한 접근 영역에 도착했습니다. 실제 기능 화면을 확인하세요.".into();
            return Tick::Done;
        }
        if self.state.actions >= MAX_ACTIONS {
            return self.stop("짧은 이동 횟수 제한을 넘었습니다. 지형을 다시 관측하세요.");
        }
        let target = self.target();
        if self
            .ground
            .as_ref()
            .is_some_and(|ground| !ground.corridor(frame, self.avatar, target, self.clearance))
        {
            return self.stop(
                "목표 사이에 확인되지 않은 지형이나 경계가 있습니다. 새 경로 관측이 필요합니다.",
            );
        }
        if self.probes < 2 {
            let guard = self.clearance + self.geometry.0.min(self.geometry.1) as f64 * 0.035;
            if self.ground.as_ref().is_some_and(|ground| {
                !ground.disk(frame, self.avatar, guard, self.avatar, self.clearance)
            }) {
                return self.stop("이동 방향을 측정할 만큼 주변 지면이 확인되지 않았습니다.");
            }
            let sign = if self.ground.is_some() {
                1.
            } else if self.probes == 0 {
                if target[0] < self.avatar[0] {
                    -1.
                } else {
                    1.
                }
            } else {
                if target[1] > self.avatar[1] {
                    -1.
                } else {
                    1.
                }
            };
            let direction = if self.probes == 0 {
                [sign, 0.]
            } else {
                [0., sign]
            };
            let gesture = match stick(self.controls, self.geometry, direction, PROBE_MS) {
                Ok(value) => value,
                Err(error) => return self.stop_error(error),
            };
            return self.emit(
                gesture,
                Action::Probe(self.probes, sign),
                "calibrating",
                "실제 화면 변화로 조이스틱 이동 방향과 속도를 측정합니다.",
            );
        }
        let desired = [target[0] - self.avatar[0], target[1] - self.avatar[1]];
        let Some(input) = inverse(self.response, desired) else {
            return self.stop("실제 이동 보정값을 사용할 수 없습니다.");
        };
        let requested_ms = length(input);
        if !requested_ms.is_finite() || requested_ms < 25.0 {
            return self
                .stop("남은 거리가 현재 입력 정밀도보다 작습니다. 더 정확한 관측이 필요합니다.");
        }
        let mut direction = [input[0] / requested_ms, input[1] / requested_ms];
        #[cfg(not(target_os = "android"))]
        if self.ground.is_none() {
            // Desktop Stick becomes digital WASD with this same threshold in
            // both native adapters. Predict that actual normalized direction,
            // rather than an analog direction the keyboard cannot produce.
            direction = direction.map(|v| {
                if v > 0.25 {
                    1.
                } else if v < -0.25 {
                    -1.
                } else {
                    0.
                }
            });
            let norm = length(direction);
            if norm > 0. {
                direction = direction.map(|v| v / norm);
            }
        }
        let velocity = [
            self.response[0][0] * direction[0] + self.response[1][0] * direction[1],
            self.response[0][1] * direction[0] + self.response[1][1] * direction[1],
        ];
        let safe_ms = (self.clearance * 0.6 / length(velocity)).min(MAX_STEP_MS as f64);
        if !safe_ms.is_finite() || safe_ms < 25.0 {
            return self
                .stop("입력의 최소 이동량이 관측 여유보다 큽니다. 시점을 다시 맞춰 주세요.");
        }
        let ms = (requested_ms * 0.65).clamp(25.0, safe_ms).round() as u32;
        let expected = [
            (self.response[0][0] * direction[0] + self.response[1][0] * direction[1]) * ms as f64,
            (self.response[0][1] * direction[0] + self.response[1][1] * direction[1]) * ms as f64,
        ];
        let gesture = match stick(self.controls, self.geometry, direction, ms) {
            Ok(value) => value,
            Err(error) => return self.stop_error(error),
        };
        self.emit(
            gesture,
            Action::Move {
                before_distance: self.state.remaining_pixels,
                expected,
            },
            "moving",
            "관측한 지면 안에서 짧게 이동하고 위치를 다시 확인합니다.",
        )
    }

    fn emit(
        &mut self,
        gesture: Gesture,
        action: Action,
        phase: &'static str,
        message: &str,
    ) -> Tick {
        self.phase = Phase::Input {
            action,
            sent_at: self.last_now,
            duration: gesture.duration_ms(),
        };
        self.state.phase = phase;
        self.state.message = message.into();
        self.state.actions += 1;
        Tick::Gesture(gesture)
    }
    fn target(&self) -> [f64; 2] {
        [0, 1].map(|axis| {
            let low = self.goal[axis];
            let high = self.goal[axis + 2];
            let center = low + (high - low) * 0.5;
            let half = ((high - low) * 0.5 - self.clearance * 1.5).max(0.);
            // A narrow approach region has one target: its center. Computing
            // bounds symmetrically also avoids min > max from rounding when
            // independently adding/subtracting the old clearance margins.
            if half <= f64::EPSILON * center.abs().max(1.) * 8. {
                center
            } else {
                self.avatar[axis].clamp(center - half, center + half)
            }
        })
    }
    fn distance(&self) -> f64 {
        let p = self.target();
        length([p[0] - self.avatar[0], p[1] - self.avatar[1]])
    }
    fn stop(&mut self, message: &str) -> Tick {
        self.phase = Phase::Stopped;
        self.state.phase = "stopped";
        self.state.message = message.into();
        Tick::Stopped(message.into())
    }
    fn stop_error(&mut self, error: String) -> Tick {
        self.stop(&error)
    }
}

impl Ground {
    fn learn(
        frame: &Frame<'_>,
        points: &[Point],
        included: Region,
        excluded: &[Region],
        self_mask: Region,
    ) -> Result<Self, String> {
        let mut seeds = Vec::new();
        let mut samples = Vec::new();
        for point in points {
            if !point_valid(*point) {
                return Err("지면 관측 위치가 화면 밖에 있습니다.".into());
            }
            let p = pixel_point(*point, (frame.width, frame.height));
            if !contains(included, p) || excluded.iter().any(|r| contains(*r, p)) {
                return Err("가려지지 않은 실제 지면의 표식이 필요합니다.".into());
            }
            seeds.push(p);
            samples.push(sample(frame, p).ok_or("지면 픽셀이 충분하지 않습니다.")?);
        }
        let hull = convex_hull(seeds.clone());
        if hull.len() < 3 || polygon_area(&hull) < 100.0 {
            return Err("주변 지면을 둘러싼 관측이 필요합니다.".into());
        }
        Ok(Self {
            hull,
            samples,
            excluded: excluded.to_vec(),
            self_mask,
        })
    }
    fn translate(&mut self, dx: f64, dy: f64) {
        for p in &mut self.hull {
            p[0] += dx;
            p[1] += dy;
        }
    }
    fn transform(&mut self, registration: &scene::Registration) {
        for point in &mut self.hull {
            *point = registration.map_point(*point);
        }
    }
    fn pixel(&self, frame: &Frame<'_>, p: [f64; 2]) -> bool {
        if !inside_polygon(&self.hull, p) {
            return false;
        }
        if self.excluded.iter().any(|r| contains(*r, p)) {
            // Only our observed body may occlude an interpolated local surface.
            // Require visible matching floor on BOTH sides on this same row;
            // static UI masks, shadows and an unseen future footprint stay unknown.
            if !contains(self.self_mask, p)
                || self
                    .excluded
                    .iter()
                    .any(|r| *r != self.self_mask && contains(*r, p))
            {
                return false;
            }
            let side = [
                [self.self_mask.x as f64 - 4., p[1]],
                [(self.self_mask.x + self.self_mask.width) as f64 + 4., p[1]],
            ];
            return side.into_iter().all(|q| {
                inside_polygon(&self.hull, q)
                    && sample_visible(frame, q, &self.excluded).is_some_and(|s| self.matches(&s))
            });
        }
        let Some(found) = sample_visible(frame, p, &self.excluded) else {
            return false;
        };
        self.matches(&found)
    }
    fn matches(&self, found: &GroundSample) -> bool {
        self.samples.iter().any(|model| {
            let delta = (0..3)
                .map(|i| (model.rgb[i] - found.rgb[i]).powi(2))
                .sum::<f64>()
                .sqrt();
            delta <= (model.spread * 1.5).clamp(10.0, 32.0)
                && found.spread <= (model.spread * 2.0 + 6.0).min(45.0)
        })
    }
    fn disk(
        &self,
        frame: &Frame<'_>,
        center: [f64; 2],
        radius: f64,
        contact: [f64; 2],
        footprint: f64,
    ) -> bool {
        let step = (radius / 5.0).max(2.0);
        let mut y = -radius;
        while y <= radius {
            let mut x = -radius;
            while x <= radius {
                if x * x + y * y <= radius * radius {
                    let p = [center[0] + x, center[1] + y];
                    // The observed contact witnesses support only below our
                    // current footprint, not an occluded future path.
                    if length([p[0] - contact[0], p[1] - contact[1]]) > footprint + 1e-7
                        && !self.pixel(frame, p)
                    {
                        return false;
                    }
                }
                x += step;
            }
            y += step;
        }
        true
    }
    fn corridor(&self, frame: &Frame<'_>, from: [f64; 2], to: [f64; 2], radius: f64) -> bool {
        let distance = length([to[0] - from[0], to[1] - from[1]]);
        let count = (distance / 4.0).ceil().max(1.0) as usize;
        if count > 4096 {
            return false;
        }
        (0..=count).all(|i| {
            let t = i as f64 / count as f64;
            self.disk(
                frame,
                [
                    from[0] + (to[0] - from[0]) * t,
                    from[1] + (to[1] - from[1]) * t,
                ],
                radius,
                from,
                radius,
            )
        })
    }
}

fn sample(frame: &Frame<'_>, p: [f64; 2]) -> Option<GroundSample> {
    sample_visible(frame, p, &[])
}
fn sample_visible(frame: &Frame<'_>, p: [f64; 2], excluded: &[Region]) -> Option<GroundSample> {
    if !p[0].is_finite() || !p[1].is_finite() || p[0] < 3.0 || p[1] < 3.0 {
        return None;
    }
    let (x, y) = (p[0].round() as usize, p[1].round() as usize);
    let mut sum = [0.0; 3];
    let mut squares = [0.0; 3];
    let mut count = 0.0;
    for dy in -2isize..=2 {
        for dx in -2isize..=2 {
            let (px, py) = (x.checked_add_signed(dx)?, y.checked_add_signed(dy)?);
            if excluded
                .iter()
                .any(|r| contains(*r, [px as f64, py as f64]))
            {
                continue;
            }
            let color = frame.pixel(px, py)?;
            count += 1.0;
            for i in 0..3 {
                let value = color[i] as f64;
                sum[i] += value;
                squares[i] += value * value;
            }
        }
    }
    if count < 13.0 {
        return None;
    }
    let rgb = sum.map(|v| v / count);
    let spread = ((0..3)
        .map(|i| (squares[i] / count - rgb[i] * rgb[i]).max(0.0))
        .sum::<f64>()
        / 3.0)
        .sqrt();
    Some(GroundSample { rgb, spread })
}
fn point_valid(p: Point) -> bool {
    p.0.is_finite() && p.1.is_finite() && (0.0..1.0).contains(&p.0) && (0.0..1.0).contains(&p.1)
}
fn checked_region(rect: Rect, geometry: (usize, usize)) -> Result<Region, String> {
    if ![rect.0, rect.1, rect.2, rect.3]
        .iter()
        .all(|v| v.is_finite() && (0.0..=1.0).contains(v))
        || rect.0 >= rect.2
        || rect.1 >= rect.3
    {
        return Err("관측 영역이 화면 밖이거나 비어 있습니다.".into());
    }
    rect.region(geometry)
        .ok_or_else(|| "관측 영역이 올바르지 않습니다.".into())
}
fn pixel_point(p: Point, (width, height): (usize, usize)) -> [f64; 2] {
    [p.0 * width as f64, p.1 * height as f64]
}
fn contains(r: Region, p: [f64; 2]) -> bool {
    p[0] >= r.x as f64
        && p[0] < (r.x + r.width) as f64
        && p[1] >= r.y as f64
        && p[1] < (r.y + r.height) as f64
}
fn stick_radius(c: Controls, geometry: (usize, usize)) -> f64 {
    let a = pixel_point(c.stick_center, geometry);
    let b = pixel_point(c.stick_edge, geometry);
    length([a[0] - b[0], a[1] - b[1]])
}
fn stick(
    c: Controls,
    geometry: (usize, usize),
    direction: [f64; 2],
    ms: u32,
) -> Result<Gesture, String> {
    let radius = stick_radius(c, geometry);
    let to = Point(
        c.stick_center.0 + direction[0] * radius / geometry.0 as f64,
        c.stick_center.1 - direction[1] * radius / geometry.1 as f64,
    );
    if !point_valid(to) || !(1..=1000).contains(&ms) {
        return Err("측정할 조이스틱 입력이 화면 밖입니다.".into());
    }
    Ok(Gesture::Stick {
        from: c.stick_center,
        to,
        jump: None,
        ms,
    })
}
fn length(p: [f64; 2]) -> f64 {
    p[0].hypot(p[1])
}
fn dot(a: [f64; 2], b: [f64; 2]) -> f64 {
    a[0] * b[0] + a[1] * b[1]
}
fn conditioning(m: [[f64; 2]; 2]) -> f64 {
    let d = m[0][0] * m[1][1] - m[0][1] * m[1][0];
    d.abs() / (length(m[0]) * length(m[1])).max(1e-12)
}
fn inverse(m: [[f64; 2]; 2], p: [f64; 2]) -> Option<[f64; 2]> {
    let d = m[0][0] * m[1][1] - m[0][1] * m[1][0];
    (d.is_finite() && d.abs() > 1e-9).then(|| {
        [
            (m[1][1] * p[0] - m[1][0] * p[1]) / d,
            (-m[0][1] * p[0] + m[0][0] * p[1]) / d,
        ]
    })
}
fn cross(o: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
    (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0])
}
fn convex_hull(mut points: Vec<[f64; 2]>) -> Vec<[f64; 2]> {
    points.sort_by(|a, b| a[0].total_cmp(&b[0]).then(a[1].total_cmp(&b[1])));
    points.dedup();
    if points.len() < 3 {
        return points;
    }
    let mut out = Vec::new();
    for p in &points {
        while out.len() >= 2 && cross(out[out.len() - 2], out[out.len() - 1], *p) <= 0.0 {
            out.pop();
        }
        out.push(*p);
    }
    let lower = out.len();
    for p in points[..points.len() - 1].iter().rev() {
        while out.len() > lower && cross(out[out.len() - 2], out[out.len() - 1], *p) <= 0.0 {
            out.pop();
        }
        out.push(*p);
    }
    out.pop();
    out
}
fn inside_polygon(polygon: &[[f64; 2]], p: [f64; 2]) -> bool {
    polygon.len() >= 3
        && (0..polygon.len())
            .all(|i| cross(polygon[i], polygon[(i + 1) % polygon.len()], p) >= -1e-6)
}
fn polygon_area(p: &[[f64; 2]]) -> f64 {
    (0..p.len())
        .map(|i| p[i][0] * p[(i + 1) % p.len()][1] - p[i][1] * p[(i + 1) % p.len()][0])
        .sum::<f64>()
        .abs()
        * 0.5
}

#[cfg(test)]
mod tests {
    use super::*;
    const W: usize = 192;
    const H: usize = 160;

    fn observed() -> Observed {
        Observed {
            avatar_contact: Point(0.45, 0.5),
            avatar_bounds: Rect(0.39, 0.43, 0.51, 0.57),
            goal_region: Rect(0.65, 0.35, 0.98, 0.75),
            ground_seeds: vec![
                Point(0.04, 0.06),
                Point(0.96, 0.06),
                Point(0.96, 0.95),
                Point(0.04, 0.95),
            ],
            included: Rect(0.0, 0.0, 1.0, 1.0),
            excluded: vec![],
            clearance_pixels: 18.0,
        }
    }
    fn controls() -> Controls {
        Controls {
            stick_center: Point(0.15, 0.85),
            stick_edge: Point(0.23, 0.85),
        }
    }
    fn noise(seed: &mut u32) -> u8 {
        *seed ^= *seed << 13;
        *seed ^= *seed >> 17;
        *seed ^= *seed << 5;
        (*seed % 61) as u8
    }
    fn scene_texture() -> Vec<u8> {
        let mut bytes = vec![0; W * H * 4];
        let mut seed = 43;
        for p in bytes.chunks_exact_mut(4) {
            let value = 65 + noise(&mut seed);
            p.copy_from_slice(&[value, value, value, 255]);
        }
        bytes
    }
    fn picture(texture: &[u8], dx: isize, dy: isize) -> Vec<u8> {
        let mut bytes = vec![0; texture.len()];
        for y in 0..H {
            for x in 0..W {
                let (sx, sy) = (x as isize - dx, y as isize - dy);
                if sx >= 0 && sy >= 0 && sx < W as isize && sy < H as isize {
                    let p = (y * W + x) * 4;
                    let q = (sy as usize * W + sx as usize) * 4;
                    bytes[p..p + 4].copy_from_slice(&texture[q..q + 4]);
                }
            }
        }
        // A separately observed, stationary avatar texture; no screen-center
        // assumption and no fabricated pose is injected into the controller.
        let avatar = observed().avatar_bounds.region((W, H)).unwrap();
        let mut seed = 281;
        for y in avatar.y..avatar.y + avatar.height {
            for x in avatar.x..avatar.x + avatar.width {
                let value = 160 + noise(&mut seed);
                let p = (y * W + x) * 4;
                bytes[p..p + 4].copy_from_slice(&[value, value, value, 255]);
            }
        }
        bytes
    }
    fn frame(bytes: &[u8]) -> Frame<'_> {
        Frame::new(bytes, W, H, W * 4, 4, false).unwrap()
    }
    fn navigator(bytes: &[u8]) -> Navigator {
        Navigator::from_frame(
            Destination::Sell,
            &frame(bytes),
            &observed(),
            controls(),
            0.0,
        )
        .unwrap()
    }

    #[test]
    fn local_goal_uses_measured_two_axis_motion_and_new_frames_until_arrival() {
        let texture = scene_texture();
        let bytes = picture(&texture, 0, 0);
        let mut nav = navigator(&bytes);
        let (mut at, mut x, mut y) = (0.2, 0.0f64, 0.0f64);
        let mut tick = nav.tick(at, Some(&frame(&bytes)));
        for _ in 0..24 {
            match tick {
                Tick::Gesture(Gesture::Stick { from, to, jump, ms }) => {
                    assert!(jump.is_none() && ms <= MAX_STEP_MS);
                    let r = stick_radius(controls(), (W, H));
                    // The fixture acts only on the actual emitted gesture and
                    // supplies resulting pixels through the real scene matcher.
                    x -= (to.0 - from.0) * W as f64 / r * ms as f64 * 0.1;
                    y -= (to.1 - from.1) * H as f64 / r * ms as f64 * 0.1;
                    at += ms as f64 / 1000.0;
                    assert!(matches!(nav.tick(at, None), Tick::Wait(_)));
                    nav.ack(true, at);
                    at += 0.2;
                    let after = picture(&texture, x.round() as isize, y.round() as isize);
                    tick = nav.tick(at, Some(&frame(&after)));
                }
                Tick::Done => break,
                other => panic!(
                    "unexpected navigation outcome: {other:?}; {:?}",
                    nav.status()
                ),
            }
        }
        assert!(matches!(tick, Tick::Done), "{:?}", nav.status());
        let status = nav.status();
        assert!(status.local_goal_reached && status.actions >= 3 && status.calibration.is_some());
        let c = status.calibration.unwrap();
        assert!((c.right[0] - 0.1).abs() < 0.01 && c.right[1].abs() < 0.01);
        assert!((c.up[1] + 0.1).abs() < 0.01 && c.up[0].abs() < 0.01);
    }

    #[test]
    fn fresh_observed_feet_follow_affine_ground_without_fabricating_body_texture() {
        let texture = scene_texture();
        let render = |dx: isize, dy: isize| {
            let mut pixels = picture(&texture, dx, dy);
            let r = observed().avatar_bounds.region((W, H)).unwrap();
            for y in r.y..r.y + r.height {
                for x in r.x..r.x + r.width {
                    let p = (y * W + x) * 4;
                    pixels[p..p + 4].copy_from_slice(&[10, 10, 10, 255]);
                }
            }
            pixels
        };
        let initial = render(0, 0);
        let mut nav = navigator(&initial);
        assert!(
            nav.body.is_none(),
            "this fixture has no matchable body texture"
        );
        let (mut at, mut x, mut y) = (0.2, 0.0f64, 0.0f64);
        let mut tick = nav.tick_observed(
            at,
            Some(&frame(&initial)),
            observed().avatar_contact,
            observed().avatar_bounds,
            0.5,
        );
        for _ in 0..24 {
            match tick {
                Tick::Gesture(Gesture::Stick { from, to, jump, ms }) => {
                    assert!(jump.is_none());
                    let r = stick_radius(controls(), (W, H));
                    x -= (to.0 - from.0) * W as f64 / r * ms as f64 * 0.1;
                    y -= (to.1 - from.1) * H as f64 / r * ms as f64 * 0.1;
                    at += ms as f64 / 1000.;
                    nav.ack(true, at);
                    at += 0.2;
                    let after = render(x.round() as isize, y.round() as isize);
                    tick = nav.tick_observed(
                        at,
                        Some(&frame(&after)),
                        observed().avatar_contact,
                        observed().avatar_bounds,
                        0.5,
                    );
                }
                Tick::Done => break,
                other => panic!("fresh observed feet: {other:?}; {:?}", nav.status()),
            }
        }
        assert!(matches!(tick, Tick::Done), "{:?}", nav.status());
        assert!(nav.status().calibration.is_some() && nav.status().local_goal_reached);
    }

    #[test]
    fn grid_certified_segment_reuses_motion_calibration_without_ground_seed_hull_or_body() {
        let texture = scene_texture();
        let initial = picture(&texture, 0, 0);
        let mut seen = observed();
        seen.ground_seeds.clear();
        seen.goal_region = Rect(0.03, 0.25, 0.31, 0.78);
        seen.clearance_pixels = 10.;
        let measured = Calibration {
            right: [0.1, 0.],
            up: [0., -0.1],
            error_pixels: 1.,
        };
        let mut nav = Navigator::from_route_frame(
            Destination::Base,
            &frame(&initial),
            &seen,
            controls(),
            0.,
            Some(measured),
        )
        .unwrap();
        assert!(nav.ground.is_none());
        assert!(nav.body.is_none());
        let Tick::Gesture(Gesture::Stick { from, to, .. }) = nav.tick_observed(
            0.2,
            Some(&frame(&initial)),
            seen.avatar_contact,
            seen.avatar_bounds,
            0.5,
        ) else {
            panic!(
                "Grid-certified segment must reach motion immediately: {:?}",
                nav.status()
            );
        };
        assert!(to.0 < from.0, "reuse measured calibration and move toward the left-hand route, never probe right again");
        assert_eq!(nav.status().phase, "moving");
        assert_eq!(nav.status().actions, 1);
    }

    #[test]
    fn camera_follow_route_measures_both_probe_axes_toward_the_target() {
        let texture = scene_texture();
        let initial = picture(&texture, 0, 0);
        let mut seen = observed();
        seen.ground_seeds.clear();
        seen.goal_region = Rect(0.03, 0.50, 0.30, 0.96);
        seen.clearance_pixels = 10.;
        let mut nav = Navigator::from_route_frame(
            Destination::Base,
            &frame(&initial),
            &seen,
            controls(),
            0.,
            None,
        )
        .unwrap();
        let (mut at, mut x, mut y) = (0.2, 0.0f64, 0.0f64);
        let mut tick = nav.tick_observed(
            at,
            Some(&frame(&initial)),
            seen.avatar_contact,
            seen.avatar_bounds,
            0.5,
        );
        for probe in 0..2 {
            let Tick::Gesture(Gesture::Stick { from, to, ms, .. }) = tick else {
                panic!(
                    "probe {probe} must be emitted: {tick:?}; {:?}",
                    nav.status()
                );
            };
            if probe == 0 {
                assert!(to.0 < from.0);
            } else {
                assert!(to.1 > from.1);
            }
            let r = stick_radius(controls(), (W, H));
            x -= (to.0 - from.0) * W as f64 / r * ms as f64 * 0.1;
            y -= (to.1 - from.1) * H as f64 / r * ms as f64 * 0.1;
            at += ms as f64 / 1000.;
            nav.ack(true, at);
            at += 0.2;
            let after = picture(&texture, x.round() as isize, y.round() as isize);
            tick = nav.tick_observed(
                at,
                Some(&frame(&after)),
                seen.avatar_contact,
                seen.avatar_bounds,
                0.5,
            );
        }
        let c = nav
            .status()
            .calibration
            .expect("two actual signed observations produce calibration");
        assert!((c.right[0] - 0.1).abs() < 0.01 && c.right[1].abs() < 0.01);
        assert!((c.up[1] + 0.1).abs() < 0.01 && c.up[0].abs() < 0.01);
    }

    #[test]
    fn moving_observed_body_and_feet_replace_the_mask_without_accumulating_holes() {
        let texture = scene_texture();
        let initial = observed();
        let render = |dx: f64, dy: f64| {
            let mut bytes = texture.clone();
            let bounds = Rect(
                initial.avatar_bounds.0 + dx / W as f64,
                initial.avatar_bounds.1 + dy / H as f64,
                initial.avatar_bounds.2 + dx / W as f64,
                initial.avatar_bounds.3 + dy / H as f64,
            );
            let r = bounds.region((W, H)).unwrap();
            for y in r.y..r.y + r.height {
                for x in r.x..r.x + r.width {
                    bytes[(y * W + x) * 4..(y * W + x) * 4 + 4].copy_from_slice(&[10, 10, 10, 255]);
                }
            }
            (
                bytes,
                bounds,
                Point(
                    initial.avatar_contact.0 + dx / W as f64,
                    initial.avatar_contact.1 + dy / H as f64,
                ),
            )
        };
        let (bytes, bounds, contact) = render(0., 0.);
        let mut nav = navigator(&bytes);
        assert!(matches!(
            nav.tick_observed(0.2, Some(&frame(&bytes)), contact, bounds, 0.5),
            Tick::Gesture(_)
        ));
        nav.ack(true, 0.3);
        let (bytes, bounds, contact) = render(10., 0.);
        assert!(
            matches!(
                nav.tick_observed(0.5, Some(&frame(&bytes)), contact, bounds, 0.5),
                Tick::Gesture(_)
            ),
            "{:?}",
            nav.status()
        );
        nav.ack(true, 0.6);
        let (bytes, bounds, contact) = render(10., -10.);
        assert!(
            matches!(
                nav.tick_observed(0.8, Some(&frame(&bytes)), contact, bounds, 0.5),
                Tick::Gesture(_)
            ),
            "{:?}",
            nav.status()
        );
        let actual = bounds.region((W, H)).unwrap();
        assert_eq!(nav.avatar_region, actual);
        assert_eq!(nav.excluded, vec![actual]);
        assert_eq!(nav.ground.as_ref().unwrap().self_mask, actual);
        let c = nav.status().calibration.unwrap();
        assert!((c.right[0] - 0.1).abs() < 0.01 && c.right[1].abs() < 0.01);
        assert!((c.up[1] + 0.1).abs() < 0.01 && c.up[0].abs() < 0.01);
    }

    #[test]
    fn acknowledgement_is_required_and_no_motion_never_becomes_a_jump_or_progress() {
        let texture = scene_texture();
        let bytes = picture(&texture, 0, 0);
        let mut nav = navigator(&bytes);
        assert!(matches!(
            nav.tick(0.2, Some(&frame(&bytes))),
            Tick::Gesture(_)
        ));
        assert!(matches!(nav.tick(0.5, Some(&frame(&bytes))), Tick::Wait(_)));
        nav.ack(true, 0.5);
        assert!(matches!(nav.tick(0.6, Some(&frame(&bytes))), Tick::Wait(_)));
        assert!(matches!(
            nav.tick(0.8, Some(&frame(&bytes))),
            Tick::Stopped(_)
        ));
        assert!(!nav.status().local_goal_reached && nav.status().actions == 1);
    }

    #[test]
    fn malformed_regions_controls_and_geometry_are_rejected_before_clamping() {
        let bytes = picture(&scene_texture(), 0, 0);
        for goal in [
            Rect(0.5, 0.5, 0.51, 0.51),
            Rect(0.5, 0.5, f64::NAN, 0.8),
            Rect(-0.1, 0.2, 0.8, 0.8),
        ] {
            let mut seen = observed();
            seen.goal_region = goal;
            assert!(Navigator::from_frame(
                Destination::Coast,
                &frame(&bytes),
                &seen,
                controls(),
                0.0
            )
            .is_err());
        }
        let mut bad = controls();
        bad.stick_center.0 = f64::NAN;
        assert!(
            Navigator::from_frame(Destination::Craft, &frame(&bytes), &observed(), bad, 0.0)
                .is_err()
        );
        let mut nav = navigator(&bytes);
        let resized = Frame::new(&bytes, W - 1, H, W * 4, 4, false).unwrap();
        assert!(matches!(nav.tick(0.2, Some(&resized)), Tick::Stopped(_)));
        let mut nav = navigator(&bytes);
        assert!(matches!(nav.tick(f64::NAN, None), Tick::Stopped(_)));
    }

    #[test]
    fn unknown_pixels_and_masks_block_a_ground_corridor_without_hue_assumptions() {
        let mut bytes = scene_texture();
        let seen = observed();
        let ground = Ground::learn(
            &frame(&bytes),
            &seen.ground_seeds,
            Region::full(W, H),
            &[],
            seen.avatar_bounds.region((W, H)).unwrap(),
        )
        .unwrap();
        assert!(ground.corridor(&frame(&bytes), [50.0, 80.0], [145.0, 80.0], 6.0));
        for y in 45..115 {
            for x in 96..105 {
                let p = (y * W + x) * 4;
                bytes[p..p + 4].copy_from_slice(&[220, 5, 5, 255]);
            }
        }
        // A new red patch is unknown even though a biome could also make water
        // blue, red, gray, etc. No color is hard-coded as water or walkable.
        assert!(!ground.corridor(&frame(&bytes), [50.0, 80.0], [145.0, 80.0], 6.0));
        let masked = Ground::learn(
            &frame(&bytes),
            &seen.ground_seeds,
            Region::full(W, H),
            &[Region {
                x: 90,
                y: 40,
                width: 20,
                height: 80,
            }],
            seen.avatar_bounds.region((W, H)).unwrap(),
        )
        .unwrap();
        assert!(!masked.corridor(&frame(&bytes), [50.0, 80.0], [145.0, 80.0], 6.0));
    }

    #[test]
    fn an_unobserved_boundary_stops_before_any_probe_or_attempted_jump() {
        let mut bytes = picture(&scene_texture(), 0, 0);
        for y in 25..135 {
            for x in 112..122 {
                let at = (y * W + x) * 4;
                bytes[at..at + 4].copy_from_slice(&[220, 5, 5, 255]);
            }
        }
        let mut nav = navigator(&bytes);
        assert!(matches!(
            nav.tick(0.2, Some(&frame(&bytes))),
            Tick::Stopped(_)
        ));
        assert_eq!(nav.status().actions, 0);
        assert!(!nav.status().local_goal_reached);
    }

    #[test]
    fn local_arrival_is_typed_and_does_not_claim_task_completion() {
        let bytes = picture(&scene_texture(), 0, 0);
        let mut seen = observed();
        seen.goal_region = Rect(0.28, 0.30, 0.62, 0.72);
        for destination in [
            Destination::Coast,
            Destination::Sell,
            Destination::Craft,
            Destination::Match,
        ] {
            let mut nav =
                Navigator::from_frame(destination, &frame(&bytes), &seen, controls(), 0.0).unwrap();
            assert!(matches!(nav.tick(0.2, Some(&frame(&bytes))), Tick::Done));
            let status = nav.status();
            assert_eq!(status.destination, destination);
            assert_eq!(status.actions, 0);
            let json = serde_json::to_value(status).unwrap();
            assert_eq!(json["localGoalReached"], true);
            assert!(json.get("taskCompleted").is_none());
        }
    }

    #[test]
    fn failed_ack_and_clock_or_duration_limits_end_the_session() {
        let bytes = picture(&scene_texture(), 0, 0);
        let mut nav = navigator(&bytes);
        assert!(matches!(
            nav.tick(0.2, Some(&frame(&bytes))),
            Tick::Gesture(_)
        ));
        nav.ack(false, 0.3);
        assert!(matches!(nav.tick(0.5, None), Tick::Stopped(_)));
        let mut nav = navigator(&bytes);
        assert!(matches!(
            nav.tick(MAX_SECONDS + 1.0, None),
            Tick::Stopped(_)
        ));
        let mut nav = navigator(&bytes);
        assert!(matches!(
            nav.tick(0.2, Some(&frame(&bytes))),
            Tick::Gesture(_)
        ));
        assert!(matches!(nav.tick(3.0, None), Tick::Stopped(_)));
    }

    #[test]
    fn actual_dispatch_rebases_only_input_timeout_after_planning_delay() {
        let texture = scene_texture();
        let bytes = picture(&texture, 0, 0);
        let mut nav = navigator(&bytes);
        assert!(matches!(
            nav.tick(0.2, Some(&frame(&bytes))),
            Tick::Gesture(_)
        ));
        let captured = nav.last_frame;
        nav.dispatch_at(3.);
        nav.dispatch_at(f64::NAN);
        nav.dispatch_at(1.);
        assert!(matches!(nav.phase, Phase::Input { sent_at: 3., .. }));
        assert_eq!(
            nav.last_frame, captured,
            "dispatch cannot refresh old capture evidence"
        );
        assert!(matches!(nav.tick(3.05, None), Tick::Wait(_)));
        nav.ack(true, 3.1);
        assert!(matches!(nav.phase, Phase::After { .. }));
        let after = picture(&texture, -10, 0);
        assert!(
            matches!(nav.tick(3.3, Some(&frame(&after))), Tick::Gesture(_)),
            "{:?}",
            nav.status()
        );
    }

    #[test]
    fn narrow_transformed_goal_has_finite_center_target_without_inverted_clamp() {
        let bytes = picture(&scene_texture(), 0, 0);
        let mut nav = navigator(&bytes);
        for epsilon in [-1e-12, 0., 1e-12] {
            nav.goal = [109., 62., 145. + epsilon, 98. + epsilon];
            let target = nav.target();
            assert!((target[0] - 127.).abs() < 1e-9);
            assert!((target[1] - 80.).abs() < 1e-9);
            assert!(nav.distance().is_finite());
        }
    }
}

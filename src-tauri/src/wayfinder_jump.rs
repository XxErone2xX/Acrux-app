//! Actual local-image jump experiments and pre-contact planning. Directions,
//! airborne ground traces and raised crossings must all have measured evidence.
use crate::{
    device::Gesture,
    navigator::{Calibration, Controls},
    vision::Point,
};
use serde::Serialize;
const MS: u32 = 100;
const FRAME_AGE: f64 = 0.30;
const FLIGHT_LIMIT: f64 = 3.;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Grounding {
    Supported,
    Airborne,
    Unknown,
    Unsupported,
}
#[derive(Clone, Copy, Debug)]
pub(crate) struct JumpObservation {
    pub foot: [f64; 2],
    pub shadow: Option<[f64; 2]>,
    pub error_pixels: f64,
    pub observed_at: f64,
    pub ground: Grounding,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FlightPoint {
    pub elapsed_seconds: f64,
    /// Ground position relative to the experiment's supported takeoff.
    pub ground: [f64; 2],
    pub height_pixels: f64,
    pub error_pixels: f64,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct JumpSample {
    /// Exact measured joystick vector: +right and +forward. Never rotated.
    pub direction: [f64; 2],
    pub endpoint: [f64; 2],
    pub input_ms: u32,
    pub flight: Vec<FlightPoint>,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct JumpCalibration {
    pub lift: [f64; 2],
    pub apex_pixels: f64,
    pub shadow_per_height: [f64; 2],
    pub supported_foot_minus_shadow: [f64; 2],
    pub samples: Vec<JumpSample>,
    pub error_pixels: f64,
    /// Observed time, never a substitute for supported landing evidence.
    pub observed_flight_seconds: f64,
}
#[derive(Debug)]
pub(crate) enum Tick {
    Observe,
    Gesture { id: u32, gesture: Gesture },
    Wait(u32),
    Measured(JumpCalibration),
    Landed,
    Stopped(String),
}
#[derive(Clone, Copy)]
enum Phase {
    Ready,
    Input { id: u32, at: f64 },
    Flight { at: f64, acknowledged_at: f64 },
    Measured,
    Stopped,
}
pub(crate) struct Jumper {
    controls: Controls,
    jump: Point,
    geometry: (usize, usize),
    walk: Calibration,
    support: Vec<[f64; 2]>,
    origin: JumpObservation,
    phase: Phase,
    stage: usize,
    directions: Vec<[f64; 2]>,
    last_now: f64,
    last_frame: f64,
    departed: bool,
    peak: [f64; 2],
    landing: Option<JumpObservation>,
    raw: Vec<JumpObservation>,
    flight: Vec<FlightPoint>,
    measured: JumpCalibration,
    failure: String,
    crossing: Option<PlannedJump>,
    landing_support: Vec<[f64; 2]>,
}
impl Jumper {
    pub(crate) fn start_directions(
        controls: Controls,
        jump: Point,
        geometry: (usize, usize),
        walk: Calibration,
        origin: JumpObservation,
        flat_support_hull: Vec<[f64; 2]>,
        directions: Vec<[f64; 2]>,
        now: f64,
    ) -> Result<Self, String> {
        let radius = ((controls.stick_edge.0 - controls.stick_center.0) * geometry.0 as f64)
            .hypot((controls.stick_edge.1 - controls.stick_center.1) * geometry.1 as f64);
        if !now.is_finite()
            || now < 0.
            || !fresh(origin, now)
            || origin.ground != Grounding::Supported
            || origin.shadow.is_none()
            || !valid_control(controls.stick_center)
            || !valid_control(controls.stick_edge)
            || !valid_control(jump)
            || geometry.0 == 0
            || geometry.1 == 0
            || geometry.0 > 16384
            || geometry.1 > 16384
            || radius < 8.
            || radius > geometry.0.min(geometry.1) as f64 * 0.3
            || !walk.error_pixels.is_finite()
            || walk.error_pixels < 0.
            || !walk.right.into_iter().chain(walk.up).all(f64::is_finite)
            || length(walk.right) < 0.005
            || length(walk.up) < 0.005
            || cross([0., 0.], walk.right, walk.up).abs()
                < length(walk.right) * length(walk.up) * 0.2
            || !valid_hull(&flat_support_hull)
            || !inside(&flat_support_hull, origin.foot, origin.error_pixels + 2.)
            || directions.is_empty()
            || directions.len() > 16
            || directions.iter().any(|d| !valid_direction(*d))
        {
            return Err("Jump needs measured controls, supported feet and shadow, a flat patch and exact requested directions.".into());
        }
        if directions
            .iter()
            .enumerate()
            .any(|(i, d)| directions[..i].contains(d))
        {
            return Err("Jump experiment directions must be distinct actual vectors.".into());
        }
        Ok(Self {
            controls,
            jump,
            geometry,
            walk,
            support: flat_support_hull,
            origin,
            phase: Phase::Ready,
            stage: 0,
            directions,
            last_now: now,
            last_frame: origin.observed_at,
            departed: false,
            peak: [0., 0.],
            landing: None,
            raw: Vec::new(),
            flight: Vec::new(),
            measured: JumpCalibration {
                lift: [0., 0.],
                apex_pixels: 0.,
                shadow_per_height: [0., 0.],
                supported_foot_minus_shadow: sub(origin.foot, origin.shadow.unwrap()),
                samples: Vec::new(),
                error_pixels: origin.error_pixels + walk.error_pixels,
                observed_flight_seconds: 0.,
            },
            failure: String::new(),
            crossing: None,
            landing_support: Vec::new(),
        })
    }
    pub(crate) fn crossing(
        controls: Controls,
        jump: Point,
        geometry: (usize, usize),
        walk: Calibration,
        origin: JumpObservation,
        candidate: &Candidate,
        measured: JumpCalibration,
        now: f64,
    ) -> Result<Self, String> {
        let Plan::Ready(plan) = measured.plan(candidate, origin.foot, origin.error_pixels) else {
            return Err("The observed crossing is not ready before its barrier.".into());
        };
        let mut runner = Self::start_directions(
            controls,
            jump,
            geometry,
            walk,
            origin,
            candidate.takeoff_support.clone(),
            vec![plan.direction],
            now,
        )?;
        runner.stage = 1;
        runner.crossing = Some(plan);
        runner.landing_support = candidate.landing_support.clone();
        runner.measured = measured;
        Ok(runner)
    }
    /// A second supporting-plane observation is useful only after an actual
    /// departure, including both supported frames that confirm the landing.
    #[cfg(test)]
    pub(crate) fn test_landing_height(&self) -> Option<f64> {
        self.crossing.map(|p| p.landing_height)
    }

    pub(crate) fn landing_expected(&self) -> bool {
        self.crossing.is_some() && matches!(self.phase, Phase::Flight { .. }) && self.departed
    }

    fn direction(&self) -> [f64; 2] {
        if let Some(plan) = self.crossing {
            plan.direction
        } else if self.stage > 0 {
            self.directions[self.stage - 1]
        } else {
            [0., 0.]
        }
    }
    fn expected(&self) -> [f64; 2] {
        let d = self.direction();
        scale(
            add(scale(self.walk.right, d[0]), scale(self.walk.up, d[1])),
            f64::from(MS),
        )
    }
    pub(crate) fn tick(&mut self, now: f64, observation: Option<JumpObservation>) -> Tick {
        if matches!(self.phase, Phase::Stopped) {
            return Tick::Stopped(self.failure.clone());
        }
        if matches!(self.phase, Phase::Measured) {
            return if self.crossing.is_some() {
                Tick::Landed
            } else {
                Tick::Measured(self.measured.clone())
            };
        }
        if !now.is_finite() || now < self.last_now {
            return self.stop("Jump clock changed.");
        }
        self.last_now = now;
        if let Phase::Input { at, .. } = self.phase {
            return if now - at > 1.5 + f64::from(self.input_ms()) / 1000. {
                self.stop("Jump input acknowledgement timed out.")
            } else {
                Tick::Wait(20)
            };
        }
        if let Phase::Flight { at, .. } = self.phase {
            if now - at > FLIGHT_LIMIT {
                return self.stop("Airborne departure and supported landing were not observed.");
            }
        }
        let Some(o) = observation else {
            return Tick::Observe;
        };
        if !fresh(o, now) {
            return self.stop("Jump foot or shadow observation is stale or invalid.");
        }
        if o.observed_at <= self.last_frame {
            return Tick::Observe;
        }
        self.last_frame = o.observed_at;
        if o.ground == Grounding::Unsupported {
            return self.stop("Jump left its observed supporting floor.");
        }
        match self.phase {
            Phase::Ready => {
                if o.ground != Grounding::Supported || o.shadow.is_none() {
                    return Tick::Observe;
                }
                let takeoff = self.crossing.map_or(self.origin.foot, |p| p.takeoff);
                if length(sub(o.foot, takeoff))
                    > o.error_pixels + self.origin.error_pixels + self.measured.error_pixels + 2.
                {
                    return self.stop("Feet moved before the planned jump experiment.");
                }
                if self.stage > 0
                    && length(sub(
                        sub(o.foot, o.shadow.unwrap()),
                        self.measured.supported_foot_minus_shadow,
                    )) > o.error_pixels + self.measured.error_pixels + 2.
                {
                    return self.stop("The supporting foot/shadow offset changed before the jump.");
                }
                self.origin = o;
                let target = self
                    .crossing
                    .map_or(add(o.foot, self.expected()), |p| p.landing);
                let support = if self.crossing.is_some() {
                    &self.landing_support
                } else {
                    &self.support
                };
                let error = self
                    .crossing
                    .map_or(self.measured.error_pixels, |p| p.error_pixels);
                if !inside(support, target, error + 4.) {
                    return self.stop("The next jump has no observed landing clearance.");
                }
                let gesture = match self.gesture() {
                    Ok(g) => g,
                    Err(e) => return self.stop(&e),
                };
                let id = self.stage as u32 + 1;
                self.phase = Phase::Input { id, at: now };
                self.departed = false;
                self.peak = [0., 0.];
                self.landing = None;
                self.raw.clear();
                self.flight = vec![FlightPoint {
                    elapsed_seconds: 0.,
                    ground: [0., 0.],
                    height_pixels: 0.,
                    error_pixels: o.error_pixels + self.measured.error_pixels,
                }];
                Tick::Gesture { id, gesture }
            }
            Phase::Flight {
                at,
                acknowledged_at,
            } => {
                if o.observed_at <= acknowledged_at {
                    return Tick::Observe;
                }
                self.measured.error_pixels = self
                    .measured
                    .error_pixels
                    .max(o.error_pixels + self.origin.error_pixels);
                let delta = sub(o.foot, self.origin.foot);
                if o.ground == Grounding::Airborne {
                    self.departed = true;
                    self.landing = None;
                    if self.stage == 0 {
                        if length(delta) > length(self.peak) {
                            self.peak = delta;
                        }
                        if o.shadow.is_some() {
                            if self.raw.len() >= 256 {
                                return self.stop(
                                    "Jump observation count exceeded its bounded experiment.",
                                );
                            }
                            self.raw.push(o);
                        }
                    } else if self.crossing.is_none() {
                        if let Some(point) = self.flight_point(o, at) {
                            if self.flight.len() >= 255 {
                                return self
                                    .stop("Jump flight trace exceeded its bounded experiment.");
                            }
                            self.flight.push(point);
                        }
                    }
                    return Tick::Observe;
                }
                if o.ground != Grounding::Supported || !self.departed {
                    self.landing = None;
                    return Tick::Observe;
                }
                let support = if self.crossing.is_some() {
                    &self.landing_support
                } else {
                    &self.support
                };
                if !inside(support, o.foot, o.error_pixels + 2.) {
                    return self.stop("Jump landed outside its observed supporting patch.");
                }
                let error = o.error_pixels + self.origin.error_pixels + 2.;
                let ground_delta = sub(
                    delta,
                    scale(
                        self.measured.lift,
                        self.crossing.map_or(0., |p| p.landing_height),
                    ),
                );
                if self.stage == 0 && length(delta) > error {
                    return self.stop("An in-place jump landed at an unexpected point.");
                }
                if self.stage > 0 {
                    let expected = self.crossing.map_or_else(
                        || self.expected(),
                        |p| {
                            sub(
                                sub(p.landing, p.takeoff),
                                scale(self.measured.lift, p.landing_height),
                            )
                        },
                    );
                    if dot(ground_delta, expected) <= 0.
                        || length(ground_delta) <= error
                        || cross([0., 0.], ground_delta, expected).abs() / length(expected)
                            > error + length(ground_delta) * 0.25
                    {
                        return self.stop("Moving jump displacement is missing or differs from its actually measured direction.");
                    }
                }
                let Some(previous) = self.landing else {
                    self.landing = Some(o);
                    return Tick::Observe;
                };
                if o.observed_at - previous.observed_at < 0.05 {
                    return Tick::Observe;
                }
                if length(sub(o.foot, previous.foot)) > error {
                    return self.stop("Jump landing did not stabilize on the observed floor.");
                }
                if let Some(plan) = self.crossing {
                    if length(sub(o.foot, plan.landing)) > error + plan.error_pixels {
                        return self.stop(
                            "Observed landing differs from the measured pre-contact endpoint.",
                        );
                    }
                    self.phase = Phase::Measured;
                    return Tick::Landed;
                }
                if self.stage == 0 {
                    if let Err(error) = self.fit_in_place() {
                        return self.stop(&error);
                    }
                } else {
                    if !self.flight.iter().any(|p| p.height_pixels > p.error_pixels) {
                        return self
                            .stop("No actual airborne foot/shadow ground trace was measured.");
                    }
                    self.flight.push(FlightPoint {
                        elapsed_seconds: o.observed_at - at,
                        ground: delta,
                        height_pixels: 0.,
                        error_pixels: error,
                    });
                    self.measured.samples.push(JumpSample {
                        direction: self.direction(),
                        endpoint: delta,
                        input_ms: MS,
                        flight: std::mem::take(&mut self.flight),
                    });
                }
                self.measured.observed_flight_seconds = self
                    .measured
                    .observed_flight_seconds
                    .max(o.observed_at - at);
                self.origin = o;
                self.stage += 1;
                if self.stage > self.directions.len() {
                    self.phase = Phase::Measured;
                    Tick::Measured(self.measured.clone())
                } else {
                    self.phase = Phase::Ready;
                    Tick::Observe
                }
            }
            _ => Tick::Observe,
        }
    }
    fn fit_in_place(&mut self) -> Result<(), String> {
        let apex = length(self.peak);
        if apex <= self.measured.error_pixels * 2. + 2. {
            return Err("Jump image lift is not observable above foot uncertainty.".into());
        }
        let lift = scale(self.peak, 1. / apex);
        let origin_shadow = self
            .origin
            .shadow
            .ok_or("The supported shadow is missing.")?;
        let mut sum = [0., 0.];
        let mut denominator = 0.;
        let mut fit = Vec::new();
        for o in &self.raw {
            let delta = sub(o.foot, self.origin.foot);
            let height = dot(delta, lift);
            if height <= o.error_pixels + self.origin.error_pixels + 1. {
                continue;
            }
            if length(sub(delta, scale(lift, height)))
                > o.error_pixels + self.origin.error_pixels + 2.
            {
                return Err("The in-place flight is not a consistent image lift.".into());
            }
            let shadow = sub(o.shadow.unwrap(), origin_shadow);
            sum = add(sum, scale(shadow, height));
            denominator += height * height;
            fit.push((shadow, height, o.error_pixels));
        }
        if fit.len() < 2 || denominator < apex * apex * 0.5 {
            return Err(
                "At least two actual airborne shadows are needed to fit jump height.".into(),
            );
        }
        let coefficient = scale(sum, 1. / denominator);
        let mut residual: f64 = 0.;
        for (shadow, height, error) in fit {
            let r = length(sub(shadow, scale(coefficient, height)));
            if r > error + self.origin.error_pixels + 2. {
                return Err("Airborne shadows do not agree with the measured image lift.".into());
            }
            residual = residual.max(r);
        }
        if length(sub(lift, coefficient)) < 1e-6 {
            return Err("Foot and shadow cannot distinguish ground motion from lift.".into());
        }
        self.measured.lift = lift;
        self.measured.apex_pixels = apex;
        self.measured.shadow_per_height = coefficient;
        self.measured.supported_foot_minus_shadow = sub(self.origin.foot, origin_shadow);
        self.measured.error_pixels += residual;
        Ok(())
    }
    fn flight_point(&self, o: JumpObservation, at: f64) -> Option<FlightPoint> {
        let shadow = o.shadow?;
        let axis = sub(self.measured.lift, self.measured.shadow_per_height);
        let size = length(axis);
        if size < 1e-6 {
            return None;
        }
        let difference = sub(
            sub(o.foot, shadow),
            self.measured.supported_foot_minus_shadow,
        );
        let height = dot(difference, axis) / dot(axis, axis);
        let observation_error =
            o.error_pixels + self.origin.error_pixels + self.measured.error_pixels;
        if height <= 0.
            || height > self.measured.apex_pixels + observation_error
            || length(sub(difference, scale(axis, height))) > observation_error + 1.
        {
            return None;
        }
        let error = o.error_pixels + self.origin.error_pixels + observation_error / size;
        if !error.is_finite() || error > 64. {
            return None;
        }
        Some(FlightPoint {
            elapsed_seconds: o.observed_at - at,
            ground: sub(
                sub(o.foot, scale(self.measured.lift, height)),
                self.origin.foot,
            ),
            height_pixels: height,
            error_pixels: error,
        })
    }
    pub(crate) fn ack(&mut self, id: u32, success: bool, at: f64) {
        let Phase::Input {
            id: pending,
            at: sent,
        } = self.phase
        else {
            return;
        };
        if id != pending {
            return;
        }
        if !success
            || !at.is_finite()
            || at < sent
            || at > sent + 1.5 + f64::from(self.input_ms()) / 1000.
        {
            self.stop("Jump input failed or its acknowledgement was invalid.");
            return;
        }
        self.phase = Phase::Flight {
            at: sent,
            acknowledged_at: at,
        };
    }
    fn input_ms(&self) -> u32 {
        self.crossing.map_or(MS, |p| p.input_ms)
    }
    fn gesture(&self) -> Result<Gesture, String> {
        let radius = ((self.controls.stick_edge.0 - self.controls.stick_center.0)
            * self.geometry.0 as f64)
            .hypot(
                (self.controls.stick_edge.1 - self.controls.stick_center.1)
                    * self.geometry.1 as f64,
            );
        let direction = self.direction();
        let to = Point(
            self.controls.stick_center.0 + direction[0] * radius / self.geometry.0 as f64,
            self.controls.stick_center.1 - direction[1] * radius / self.geometry.1 as f64,
        );
        if !valid_control(to) {
            return Err("Jump control endpoint is outside the screen.".into());
        }
        Ok(Gesture::Stick {
            from: self.controls.stick_center,
            to,
            jump: Some(self.jump),
            ms: self.input_ms(),
        })
    }
    fn stop(&mut self, reason: &str) -> Tick {
        self.phase = Phase::Stopped;
        self.failure = reason.into();
        Tick::Stopped(self.failure.clone())
    }
}
#[derive(Clone, Copy, Debug)]
pub(crate) struct RiseEvidence {
    pub base: [f64; 2],
    pub top: [f64; 2],
    pub error_pixels: f64,
}
#[derive(Clone, Copy, Debug)]
pub(crate) struct LandingRise {
    pub height_pixels: f64,
    pub error_pixels: f64,
    /// The observed upper floor projected back onto the takeoff plane.
    pub base_projection: [f64; 2],
    /// Actual visible point on the supported upper floor.
    pub actual_floor: [f64; 2],
}
#[derive(Clone, Debug)]
pub(crate) struct Candidate {
    pub takeoff: [f64; 2],
    pub landing: [f64; 2],
    pub takeoff_support: Vec<[f64; 2]>,
    pub landing_support: Vec<[f64; 2]>,
    pub same_plane: bool,
    pub near_edge: [f64; 2],
    pub far_edge: [f64; 2],
    pub rise: Option<RiseEvidence>,
    pub landing_rise: Option<LandingRise>,
    pub observed_gap: bool,
    pub error_pixels: f64,
    pub clearance_pixels: f64,
}
#[derive(Clone, Copy, Debug)]
pub(crate) struct PlannedJump {
    pub takeoff: [f64; 2],
    pub landing: [f64; 2],
    pub direction: [f64; 2],
    pub input_ms: u32,
    pub landing_height: f64,
    pub error_pixels: f64,
}
#[derive(Debug)]
pub(crate) enum Plan {
    Approach([f64; 2]),
    Ready(PlannedJump),
    NotObservable(String),
}
impl JumpCalibration {
    pub(crate) fn plan(&self, c: &Candidate, foot: [f64; 2], error_pixels: f64) -> Plan {
        let reject = |reason: &str| Plan::NotObservable(reason.into());
        let error = self.error_pixels + c.error_pixels + error_pixels;
        if !self.valid()
            || ![c.takeoff, c.landing, c.near_edge, c.far_edge, foot]
                .into_iter()
                .flatten()
                .all(f64::is_finite)
            || ![c.error_pixels, error_pixels, c.clearance_pixels]
                .into_iter()
                .all(|v| v.is_finite() && v >= 0.)
            || !valid_hull(&c.takeoff_support)
            || !valid_hull(&c.landing_support)
            || !inside(&c.takeoff_support, c.takeoff, error + 2.)
            || !inside(&c.landing_support, c.landing, error + 2.)
        {
            return reject("Takeoff, landing support or measured jump samples are not observable.");
        }
        let (base, height, height_error) = if c.same_plane {
            if c.landing_rise.is_some() {
                return reject("Same-plane support conflicts with raised-floor evidence.");
            }
            (c.landing, 0., 0.)
        } else {
            let Some(r) = c.landing_rise else {
                return reject("A raised landing needs measured two-plane evidence.");
            };
            if !r.height_pixels.is_finite()
                || !r.error_pixels.is_finite()
                || r.error_pixels < 0.
                || r.height_pixels <= r.error_pixels + error
                || r.height_pixels + r.error_pixels + error >= self.apex_pixels - self.error_pixels
                || !r
                    .base_projection
                    .into_iter()
                    .chain(r.actual_floor)
                    .all(f64::is_finite)
                || length(sub(r.actual_floor, c.landing)) > r.error_pixels + c.error_pixels + 2.
                || length(sub(
                    sub(r.actual_floor, r.base_projection),
                    scale(self.lift, r.height_pixels),
                )) > r.error_pixels + error + 2.
            {
                return reject(
                    "The raised floor is inconsistent or above measured safe jump height.",
                );
            }
            (r.base_projection, r.height_pixels, r.error_pixels)
        };
        let delta = sub(base, c.takeoff);
        let distance = length(delta);
        if distance <= error * 2. + 2. {
            return reject("Jump endpoint displacement is below observation uncertainty.");
        }
        let direction = scale(delta, 1. / distance);
        let along = |p: [f64; 2]| dot(sub(p, c.takeoff), direction);
        let near = along(c.near_edge);
        let far = along(c.far_edge);
        if near <= c.clearance_pixels + error + 2.
            || if c.landing_rise.is_some() {
                far < near - error
            } else {
                far <= near
            }
            || far >= distance - c.clearance_pixels - error - 2.
            || [c.near_edge, c.far_edge]
                .into_iter()
                .any(|p| cross([0., 0.], sub(p, c.takeoff), direction).abs() > error + 2.)
        {
            return reject(
                "The barrier is not ahead of clear takeoff and projected landing support.",
            );
        }
        let mut barrier_height = height;
        let mut barrier_error = height_error;
        if let Some(r) = c.rise {
            if !r.base.into_iter().chain(r.top).all(f64::is_finite)
                || !r.error_pixels.is_finite()
                || r.error_pixels < 0.
            {
                return reject("Obstacle rise is not observable.");
            }
            let vertical = sub(r.top, r.base);
            let h = dot(vertical, self.lift);
            barrier_height = barrier_height.max(h);
            barrier_error = barrier_error.max(r.error_pixels);
            if h <= r.error_pixels
                || cross([0., 0.], vertical, self.lift).abs() > r.error_pixels + error
                || h + r.error_pixels + error >= self.apex_pixels - self.error_pixels
            {
                return reject("Obstacle rise exceeds measured jump clearance.");
            }
        } else if !c.observed_gap && c.landing_rise.is_none() {
            return reject("No observed gap or measured obstacle rise supports this jump.");
        }
        let mut best: Option<(f64, PlannedJump)> = None;
        for sample in &self.samples {
            if !clears_barrier(
                sample,
                direction,
                near,
                far,
                barrier_height,
                error + barrier_error,
                c.clearance_pixels,
            ) {
                continue;
            }
            let Some((ground, prediction_error)) = sample.ground_at_height(height, height_error)
            else {
                continue;
            };
            let landing = add(add(c.takeoff, ground), scale(self.lift, height));
            let uncertainty = error + prediction_error + height_error;
            let mismatch = length(sub(landing, c.landing));
            if mismatch > uncertainty + 2.
                || length(sub(add(c.takeoff, ground), base)) > uncertainty + 2.
                || !inside(&c.landing_support, landing, uncertainty + 2.)
            {
                continue;
            }
            let plan = PlannedJump {
                takeoff: c.takeoff,
                landing,
                direction: sample.direction,
                input_ms: sample.input_ms,
                landing_height: height,
                error_pixels: uncertainty,
            };
            if best.as_ref().is_none_or(|(score, _)| mismatch < *score) {
                best = Some((mismatch, plan));
            }
        }
        let Some((_, plan)) = best else {
            return reject("No exact measured direction has an observed landing range or descending height bracket here.");
        };
        if along(foot) + c.clearance_pixels >= near - error {
            return reject(
                "The avatar already entered the barrier; collision is not a jump trigger.",
            );
        }
        if length(sub(foot, c.takeoff)) > error + 2. {
            return Plan::Approach(c.takeoff);
        }
        Plan::Ready(plan)
    }
    fn valid(&self) -> bool {
        self.lift
            .into_iter()
            .chain(self.shadow_per_height)
            .chain(self.supported_foot_minus_shadow)
            .all(f64::is_finite)
            && (length(self.lift) - 1.).abs() <= 0.01
            && self.error_pixels.is_finite()
            && self.error_pixels >= 0.
            && self.apex_pixels.is_finite()
            && self.apex_pixels > self.error_pixels * 2. + 2.
            && self.observed_flight_seconds.is_finite()
            && self.observed_flight_seconds > 0.
            && self.observed_flight_seconds <= FLIGHT_LIMIT
            && !self.samples.is_empty()
            && self.samples.len() <= 16
            && self.samples.iter().all(|s| {
                valid_direction(s.direction)
                    && s.endpoint.into_iter().all(f64::is_finite)
                    && (1..=1000).contains(&s.input_ms)
                    && s.flight.len() >= 3
                    && s.flight.len() <= 256
                    && s.flight.iter().all(|p| {
                        p.elapsed_seconds.is_finite()
                            && (0. ..=FLIGHT_LIMIT).contains(&p.elapsed_seconds)
                            && p.elapsed_seconds <= self.observed_flight_seconds + 1e-9
                            && p.ground.into_iter().all(f64::is_finite)
                            && p.height_pixels.is_finite()
                            && p.height_pixels >= 0.
                            && p.height_pixels <= self.apex_pixels + self.error_pixels
                            && p.error_pixels.is_finite()
                            && (0. ..=64.).contains(&p.error_pixels)
                    })
                    && s.flight
                        .windows(2)
                        .all(|p| p[0].elapsed_seconds < p[1].elapsed_seconds)
                    && s.flight.first().is_some_and(|p| {
                        p.height_pixels == 0. && length(p.ground) <= p.error_pixels + 1.
                    })
                    && s.flight.last().is_some_and(|p| {
                        p.height_pixels == 0.
                            && length(sub(p.ground, s.endpoint)) <= p.error_pixels + 1.
                    })
            })
    }
}
fn clears_barrier(
    sample: &JumpSample,
    direction: [f64; 2],
    near: f64,
    far: f64,
    height: f64,
    error: f64,
    clearance: f64,
) -> bool {
    let (lo, hi) = (near.min(far), near.max(far));
    let (mut covers_near, mut covers_far) = (false, false);
    for pair in sample.flight.windows(2) {
        let (a, b) = (&pair[0], &pair[1]);
        let (pa, pb) = (dot(a.ground, direction), dot(b.ground, direction));
        if pa.max(pb) < lo || pa.min(pb) > hi {
            continue;
        }
        if [a, b].iter().any(|p| {
            cross([0., 0.], p.ground, direction).abs() > p.error_pixels + error + clearance + 2.
        }) {
            return false;
        }
        if (pb - pa).abs() < 1e-9 {
            if a.height_pixels.min(b.height_pixels) - a.error_pixels.max(b.error_pixels)
                <= height + error
            {
                return false;
            }
            covers_near |= (pa - lo).abs() < 1e-9;
            covers_far |= (pa - hi).abs() < 1e-9;
            continue;
        }
        let height_at =
            |p: f64| a.height_pixels + (b.height_pixels - a.height_pixels) * ((p - pa) / (pb - pa));
        let first = lo.max(pa.min(pb));
        let last = hi.min(pa.max(pb));
        let uncertainty =
            a.error_pixels.max(b.error_pixels) + 0.5 * (b.height_pixels - a.height_pixels).abs();
        if height_at(first).min(height_at(last)) - uncertainty <= height + error {
            return false;
        }
        covers_near |= pa.min(pb) <= lo && pa.max(pb) >= lo;
        covers_far |= pa.min(pb) <= hi && pa.max(pb) >= hi;
    }
    covers_near && covers_far
}

impl JumpSample {
    /// Enumerate only observed endpoints: a supported endpoint on the original
    /// plane, or a conservative adjacent descending bracket on a raised plane.
    pub(crate) fn ground_at_height(
        &self,
        height: f64,
        height_error: f64,
    ) -> Option<([f64; 2], f64)> {
        if !height.is_finite()
            || height < 0.
            || !height_error.is_finite()
            || height_error < 0.
            || !self.endpoint.into_iter().all(f64::is_finite)
        {
            return None;
        }
        if height == 0. {
            return Some((self.endpoint, 0.));
        }
        if self.flight.len() < 3
            || self.flight.len() > 256
            || !self.flight.iter().all(|p| {
                p.elapsed_seconds.is_finite()
                    && p.ground.into_iter().all(f64::is_finite)
                    && p.height_pixels.is_finite()
                    && p.height_pixels >= 0.
                    && p.error_pixels.is_finite()
                    && p.error_pixels >= 0.
            })
            || !self
                .flight
                .windows(2)
                .all(|p| p[0].elapsed_seconds < p[1].elapsed_seconds)
        {
            return None;
        }
        descending_ground(self, height, height_error)
    }
}

fn descending_ground(
    sample: &JumpSample,
    height: f64,
    height_error: f64,
) -> Option<([f64; 2], f64)> {
    if height - height_error < 0. {
        return None;
    }
    let (center, error) = descending_at(sample, height)?;
    let (low, low_error) = descending_at(sample, height - height_error)?;
    let (high, high_error) = descending_at(sample, height + height_error)?;
    Some((
        center,
        error.max(low_error).max(high_error)
            + height_error
            + length(sub(center, low)).max(length(sub(center, high))),
    ))
}

fn descending_at(sample: &JumpSample, height: f64) -> Option<([f64; 2], f64)> {
    let mut best: Option<([f64; 2], f64)> = None;
    let peak = sample
        .flight
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.height_pixels.total_cmp(&b.1.height_pixels))
        .map(|(i, _)| i)?;
    for pair in sample.flight[peak..].windows(2) {
        let (a, b) = (&pair[0], &pair[1]);
        if a.height_pixels <= b.height_pixels
            || a.height_pixels < height
            || b.height_pixels > height
        {
            continue;
        }
        let span = a.height_pixels - b.height_pixels;
        let t = (a.height_pixels - height) / span;
        if !(0. ..=1.).contains(&t) {
            continue;
        }
        let ground = add(a.ground, scale(sub(b.ground, a.ground), t));
        let error =
            a.error_pixels.max(b.error_pixels) + 0.5 * length(sub(b.ground, a.ground)) + 0.5 * span;
        if best.as_ref().is_none_or(|(_, e)| error < *e) {
            best = Some((ground, error));
        }
    }
    best
}
fn fresh(o: JumpObservation, now: f64) -> bool {
    o.foot.into_iter().all(f64::is_finite)
        && o.shadow.is_none_or(|p| p.into_iter().all(f64::is_finite))
        && o.error_pixels.is_finite()
        && (0. ..=64.).contains(&o.error_pixels)
        && o.observed_at.is_finite()
        && o.observed_at >= 0.
        && o.observed_at <= now + 0.02
        && now - o.observed_at <= FRAME_AGE
}
fn valid_direction(d: [f64; 2]) -> bool {
    d.into_iter().all(f64::is_finite) && length(d) > 1e-3 && length(d) <= 1. + 1e-9
}
fn valid_control(p: Point) -> bool {
    p.0.is_finite() && p.1.is_finite() && (0. ..=1.).contains(&p.0) && (0. ..=1.).contains(&p.1)
}
fn add(a: [f64; 2], b: [f64; 2]) -> [f64; 2] {
    [a[0] + b[0], a[1] + b[1]]
}
fn sub(a: [f64; 2], b: [f64; 2]) -> [f64; 2] {
    [a[0] - b[0], a[1] - b[1]]
}
fn scale(a: [f64; 2], s: f64) -> [f64; 2] {
    [a[0] * s, a[1] * s]
}
fn dot(a: [f64; 2], b: [f64; 2]) -> f64 {
    a[0] * b[0] + a[1] * b[1]
}
fn length(a: [f64; 2]) -> f64 {
    a[0].hypot(a[1])
}
fn cross(a: [f64; 2], b: [f64; 2], c: [f64; 2]) -> f64 {
    (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
}
fn valid_hull(h: &[[f64; 2]]) -> bool {
    h.len() >= 3
        && h.len() <= 32
        && h.iter().flatten().all(|v| v.is_finite())
        && (0..h.len()).all(|i| cross(h[i], h[(i + 1) % h.len()], h[(i + 2) % h.len()]) > 0.)
}
fn inside(h: &[[f64; 2]], p: [f64; 2], margin: f64) -> bool {
    (0..h.len()).all(|i| {
        let a = h[i];
        let b = h[(i + 1) % h.len()];
        cross(a, b, p) >= margin * length(sub(b, a))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn hull(center: [f64; 2], radius: f64) -> Vec<[f64; 2]> {
        vec![
            [center[0] - radius, center[1] - radius],
            [center[0] + radius, center[1] - radius],
            [center[0] + radius, center[1] + radius],
            [center[0] - radius, center[1] + radius],
        ]
    }
    fn controls() -> Controls {
        Controls {
            stick_center: Point(0.15, 0.75),
            stick_edge: Point(0.20, 0.75),
        }
    }
    fn walk() -> Calibration {
        Calibration {
            right: [0.2, 0.],
            up: [0., -0.2],
            error_pixels: 0.1,
        }
    }
    fn observation(
        origin: [f64; 2],
        ground: [f64; 2],
        height: f64,
        state: Grounding,
        at: f64,
    ) -> JumpObservation {
        JumpObservation {
            foot: add(add(origin, ground), [0., -height]),
            shadow: Some(add(add(origin, ground), [height * 0.25, 4. + height * 0.1])),
            ground: state,
            observed_at: at,
            error_pixels: 0.1,
        }
    }
    fn supported(foot: [f64; 2], at: f64) -> JumpObservation {
        observation(foot, [0., 0.], 0., Grounding::Supported, at)
    }
    fn jumper(directions: Vec<[f64; 2]>) -> Jumper {
        Jumper::start_directions(
            controls(),
            Point(0.9, 0.8),
            (1000, 600),
            walk(),
            supported([100., 200.], 0.),
            hull([100., 200.], 90.),
            directions,
            0.,
        )
        .unwrap()
    }
    fn assert_near(a: [f64; 2], b: [f64; 2]) {
        assert!(length(sub(a, b)) < 1e-8, "{a:?} != {b:?}");
    }
    #[test]
    fn ack_departure_and_two_supported_frames_are_all_required() {
        let mut j = jumper(vec![[-1., 0.]]);
        let Tick::Gesture { id, .. } = j.tick(0.1, Some(supported([100., 200.], 0.1))) else {
            panic!()
        };
        assert!(matches!(
            j.tick(
                0.2,
                Some(observation(
                    [100., 200.],
                    [0., 0.],
                    20.,
                    Grounding::Airborne,
                    0.2
                ))
            ),
            Tick::Wait(_)
        ));
        j.ack(id + 1, true, 0.2);
        assert!(matches!(j.tick(0.3, None), Tick::Wait(_)));
        j.ack(id, true, 0.3);
        assert!(
            matches!(
                j.tick(
                    0.31,
                    Some(observation(
                        [100., 200.],
                        [0., 0.],
                        20.,
                        Grounding::Airborne,
                        0.2
                    ))
                ),
                Tick::Observe
            ),
            "a frame captured before ACK cannot prove departure"
        );
        assert!(matches!(
            j.tick(0.4, Some(supported([100., 200.], 0.4))),
            Tick::Observe
        ));
        assert!(
            matches!(j.tick(3.2, None), Tick::Stopped(_)),
            "time cannot prove departure or landing"
        );
    }
    #[test]
    fn negative_and_arbitrary_diagonal_vectors_are_measured_without_rotation() {
        let directions = vec![[-1., 0.], [-0.6, 0.8]];
        let mut j = jumper(directions.clone());
        let mut at = 0.;
        let mut foot = [100., 200.];
        let mut measured = None;
        for (stage, endpoint) in [[0., 0.], [-25., 0.], [-15., -20.]].into_iter().enumerate() {
            at += 0.1;
            let Tick::Gesture { id, gesture } = j.tick(at, Some(supported(foot, at))) else {
                panic!("no experiment at {stage}")
            };
            let Gesture::Stick { from, to, ms, .. } = gesture else {
                panic!()
            };
            assert_eq!(ms, 100);
            let d = if stage == 0 {
                [0., 0.]
            } else {
                directions[stage - 1]
            };
            assert!((to.0 - from.0 - d[0] * 0.05).abs() < 1e-8);
            assert!((to.1 - from.1 + d[1] * 50. / 600.).abs() < 1e-8);
            j.ack(id, true, at + 0.02);
            for (progress, height) in [(0.25, 10.), (0.60, 28.), (0.88, 12.)] {
                at += 0.08;
                let o = observation(
                    foot,
                    scale(endpoint, progress),
                    height,
                    Grounding::Airborne,
                    at,
                );
                assert!(matches!(j.tick(at, Some(o)), Tick::Observe));
            }
            at += 0.10;
            foot = add(foot, endpoint);
            assert!(matches!(
                j.tick(at, Some(supported(foot, at))),
                Tick::Observe
            ));
            at += 0.08;
            match j.tick(at, Some(supported(foot, at))) {
                Tick::Measured(m) => measured = Some(m),
                Tick::Observe => {}
                other => panic!("{other:?}"),
            }
        }
        let m = measured.unwrap();
        assert_near(m.lift, [0., -1.]);
        assert_near(m.shadow_per_height, [0.25, 0.1]);
        assert_near(m.supported_foot_minus_shadow, [0., -4.]);
        assert_eq!(m.samples.len(), 2);
        assert_eq!(m.samples[0].direction, [-1., 0.]);
        assert_eq!(m.samples[1].direction, [-0.6, 0.8]);
        assert_near(m.samples[0].endpoint, [-25., 0.]);
        assert_near(m.samples[1].endpoint, [-15., -20.]);
        assert_near(m.samples[1].flight[1].ground, [-3.75, -5.]);
        assert!((m.samples[1].flight[1].height_pixels - 10.).abs() < 1e-8);
        assert!(m.valid());
    }
    #[test]
    fn missing_or_inconsistent_shadows_cannot_create_a_calibration() {
        let mut origin = supported([100., 200.], 0.);
        origin.shadow = None;
        assert!(Jumper::start_directions(
            controls(),
            Point(0.9, 0.8),
            (1000, 600),
            walk(),
            origin,
            hull([100., 200.], 90.),
            vec![[1., 0.]],
            0.
        )
        .is_err());
        let mut j = jumper(vec![[1., 0.]]);
        let Tick::Gesture { id, .. } = j.tick(0.1, Some(supported([100., 200.], 0.1))) else {
            panic!()
        };
        j.ack(id, true, 0.12);
        for (at, h) in [(0.2, 12.), (0.3, 25.)] {
            let mut o = observation([100., 200.], [0., 0.], h, Grounding::Airborne, at);
            o.shadow = None;
            assert!(matches!(j.tick(at, Some(o)), Tick::Observe));
        }
        assert!(matches!(
            j.tick(0.5, Some(supported([100., 200.], 0.5))),
            Tick::Observe
        ));
        assert!(matches!(
            j.tick(0.6, Some(supported([100., 200.], 0.6))),
            Tick::Stopped(_)
        ));
        let mut invalid = jumper(vec![[1., 0.]]);
        let Tick::Gesture { id, .. } = invalid.tick(0.1, Some(supported([100., 200.], 0.1))) else {
            panic!()
        };
        invalid.ack(id, true, 0.12);
        for (at, h, offset) in [(0.2, 12., 0.), (0.3, 25., 30.)] {
            let mut o = observation([100., 200.], [0., 0.], h, Grounding::Airborne, at);
            o.shadow.as_mut().unwrap()[0] += offset;
            assert!(matches!(invalid.tick(at, Some(o)), Tick::Observe));
        }
        assert!(matches!(
            invalid.tick(0.5, Some(supported([100., 200.], 0.5))),
            Tick::Observe
        ));
        assert!(
            matches!(
                invalid.tick(0.6, Some(supported([100., 200.], 0.6))),
                Tick::Stopped(_)
            ),
            "inconsistent actual shadows cannot fit a movement model"
        );
        let mut j = jumper(vec![[1., 0.]]);
        let mut stale = supported([100., 200.], 0.1);
        stale.shadow = Some([f64::NAN, 0.]);
        assert!(matches!(j.tick(0.1, Some(stale)), Tick::Stopped(_)));
    }
    fn flight() -> Vec<FlightPoint> {
        [
            (0., [0., 0.], 0.),
            (0.05, [5., 0.], 10.),
            (0.1, [10., 0.], 18.),
            (0.15, [15., 0.], 25.),
            (0.2, [20., 0.], 30.),
            (0.25, [25., 0.], 26.),
            (0.3, [30., 0.], 20.),
            (0.4, [34., 0.], 12.),
            (0.5, [38., 0.], 4.),
            (0.6, [50., 0.], 0.),
        ]
        .into_iter()
        .map(|(elapsed_seconds, ground, height_pixels)| FlightPoint {
            elapsed_seconds,
            ground,
            height_pixels,
            error_pixels: 0.1,
        })
        .collect()
    }
    fn measured(direction: [f64; 2], endpoint: [f64; 2]) -> JumpCalibration {
        let mut points = flight();
        for p in &mut points {
            let progress = p.ground[0] / 50.;
            p.ground = scale(endpoint, progress);
        }
        JumpCalibration {
            lift: [0., -1.],
            apex_pixels: 30.,
            shadow_per_height: [0.25, 0.1],
            supported_foot_minus_shadow: [0., -4.],
            samples: vec![JumpSample {
                direction,
                endpoint,
                input_ms: 100,
                flight: points,
            }],
            error_pixels: 0.1,
            observed_flight_seconds: 0.6,
        }
    }
    fn candidate(delta: [f64; 2]) -> Candidate {
        Candidate {
            takeoff: [0., 0.],
            landing: delta,
            takeoff_support: hull([0., 0.], 25.),
            landing_support: hull(delta, 25.),
            same_plane: true,
            near_edge: scale(delta, 0.25),
            far_edge: scale(delta, 0.65),
            rise: None,
            landing_rise: None,
            observed_gap: true,
            error_pixels: 0.1,
            clearance_pixels: 3.,
        }
    }
    #[test]
    fn only_exact_actual_directions_are_available_and_reverse_is_not_synthesized() {
        let m = measured([-1., 0.], [-40., 0.]);
        let Plan::Ready(p) = m.plan(&candidate([-40., 0.]), [0., 0.], 0.1) else {
            panic!()
        };
        assert_eq!(p.direction, [-1., 0.]);
        assert!(matches!(
            m.plan(&candidate([40., 0.]), [0., 0.], 0.1),
            Plan::NotObservable(_)
        ));
        let m = measured([-0.6, -0.8], [-24., 32.]);
        let Plan::Ready(p) = m.plan(&candidate([-24., 32.]), [0., 0.], 0.1) else {
            panic!()
        };
        assert_eq!(p.direction, [-0.6, -0.8]);
        assert!(
            matches!(
                m.plan(&candidate([32., 24.]), [0., 0.], 0.1),
                Plan::NotObservable(_)
            ),
            "rotating a measured sample is forbidden"
        );
    }
    fn raised() -> Candidate {
        let mut c = candidate([32., -16.]);
        c.same_plane = false;
        c.observed_gap = false;
        c.near_edge = [15., 0.];
        c.far_edge = [21., 0.];
        c.landing_rise = Some(LandingRise {
            height_pixels: 16.,
            error_pixels: 0.1,
            base_projection: [32., 0.],
            actual_floor: [32., -16.],
        });
        c
    }
    #[test]
    fn raised_landing_uses_only_an_adjacent_measured_descending_height_crossing() {
        let m = measured([1., 0.], [50., 0.]);
        let Plan::Ready(p) = m.plan(&raised(), [0., 0.], 0.1) else {
            panic!("{:?}", m.plan(&raised(), [0., 0.], 0.1))
        };
        assert_near(p.landing, [32., -16.]);
        assert_eq!(p.direction, [1., 0.]);
        assert_eq!(p.landing_height, 16.);
        assert!(
            p.error_pixels >= 6.,
            "flight-point gaps must remain uncertainty"
        );
        assert_near(
            m.samples[0].ground_at_height(16., 0.1).unwrap().0,
            [32., 0.],
        );
        assert_eq!(
            m.samples[0].ground_at_height(0., 0.1),
            Some(([50., 0.], 0.))
        );
        let mut c = raised();
        c.landing_rise.as_mut().unwrap().height_pixels = 31.;
        assert!(matches!(m.plan(&c, [0., 0.], 0.1), Plan::NotObservable(_)));
        let mut m = measured([1., 0.], [50., 0.]);
        for p in &mut m.samples[0].flight {
            p.height_pixels = p.height_pixels.min(14.);
        }
        assert!(
            matches!(m.plan(&raised(), [0., 0.], 0.1), Plan::NotObservable(_)),
            "an unseen higher descent cannot be extrapolated"
        );
    }
    #[test]
    fn a_high_apex_does_not_prove_the_wall_was_cleared_at_its_actual_edge() {
        let m = measured([1., 0.], [50., 0.]);
        let mut c = raised();
        c.near_edge = [9., 0.];
        c.far_edge = c.near_edge;
        assert!(
            matches!(m.plan(&c, [0., 0.], 0.1), Plan::NotObservable(_)),
            "the measured trajectory is still below the raised wall here"
        );
        let mut c = candidate([50., 0.]);
        c.rise = Some(RiseEvidence {
            base: [12.5, 0.],
            top: [12.5, -29.],
            error_pixels: 0.1,
        });
        assert!(
            matches!(m.plan(&c, [0., 0.], 0.1), Plan::NotObservable(_)),
            "an observed apex cannot clear a wall crossed earlier or later"
        );
    }

    #[test]
    fn a_measured_vertical_step_does_not_require_invented_horizontal_thickness() {
        let m = measured([1., 0.], [50., 0.]);
        let mut c = raised();
        c.near_edge = [15., 0.];
        c.far_edge = c.near_edge;
        assert!(matches!(m.plan(&c, [0., 0.], 0.1), Plan::Ready(_)));
        let mut gap = candidate([50., 0.]);
        gap.near_edge = [15., 0.];
        gap.far_edge = gap.near_edge;
        assert!(matches!(
            m.plan(&gap, [0., 0.], 0.1),
            Plan::NotObservable(_)
        ));
    }

    #[test]
    fn raised_support_and_geometry_must_be_actual_and_pre_contact() {
        let m = measured([1., 0.], [50., 0.]);
        let mut c = raised();
        c.landing_rise = None;
        assert!(matches!(m.plan(&c, [0., 0.], 0.1), Plan::NotObservable(_)));
        let mut c = raised();
        c.landing_support.clear();
        assert!(matches!(m.plan(&c, [0., 0.], 0.1), Plan::NotObservable(_)));
        let mut c = raised();
        c.landing_rise.as_mut().unwrap().actual_floor[1] = -5.;
        assert!(matches!(m.plan(&c, [0., 0.], 0.1), Plan::NotObservable(_)));
        assert!(
            matches!(m.plan(&raised(), [15., 0.], 0.1), Plan::NotObservable(_)),
            "wall contact is not permission to jump"
        );
        assert!(matches!(
            m.plan(&raised(), [-8., 0.], 0.1),
            Plan::Approach(_)
        ));
    }
    #[test]
    fn raised_crossing_executes_exact_vector_and_removes_rise_before_direction_validation() {
        let mut c = raised();
        let offset = [100., 200.];
        c.takeoff = add(c.takeoff, offset);
        c.landing = add(c.landing, offset);
        c.near_edge = add(c.near_edge, offset);
        c.far_edge = add(c.far_edge, offset);
        c.takeoff_support = hull(c.takeoff, 25.);
        c.landing_support = hull(c.landing, 25.);
        let r = c.landing_rise.as_mut().unwrap();
        r.base_projection = add(r.base_projection, offset);
        r.actual_floor = add(r.actual_floor, offset);
        let mut j = Jumper::crossing(
            controls(),
            Point(0.9, 0.8),
            (1000, 600),
            walk(),
            supported(offset, 0.),
            &c,
            measured([1., 0.], [50., 0.]),
            0.,
        )
        .unwrap();
        assert!(!j.landing_expected());
        let Tick::Gesture {
            id,
            gesture: Gesture::Stick { to, .. },
        } = j.tick(0.1, Some(supported(offset, 0.1)))
        else {
            panic!()
        };
        assert!((to.0 - 0.20).abs() < 1e-8);
        assert!((to.1 - 0.75).abs() < 1e-8);
        j.ack(id, true, 0.12);
        assert!(!j.landing_expected());
        assert!(matches!(
            j.tick(
                0.2,
                Some(observation(
                    offset,
                    [20., 0.],
                    30.,
                    Grounding::Airborne,
                    0.2
                ))
            ),
            Tick::Observe
        ));
        assert!(j.landing_expected());
        assert!(matches!(
            j.tick(0.4, Some(supported(c.landing, 0.4))),
            Tick::Observe
        ));
        assert!(j.landing_expected());
        assert!(matches!(
            j.tick(0.5, Some(supported(c.landing, 0.5))),
            Tick::Landed
        ));
        assert!(!j.landing_expected());
    }
}

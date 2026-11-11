//! Feedback following of an already validated map route. No input is sent here.
//! Heading/length units and timestamp origin must come from actual observations.
//! The actuator must finish/release each bounded intent before observing again.
use crate::pathfinding::{Point, Position, Route, Segment};
use serde::{Deserialize, Serialize};
use std::f64::consts::PI;

const EPS: f64 = 1e-7;
const MAX_SEGMENTS: usize = 512;
const MAX_OBSERVATIONS: usize = 100_000;

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Pose {
    pub position: Position,
    /// Map-plane forward heading: atan2(delta_z, delta_x), in radians.
    pub heading_radians: f64,
    pub grounded: bool,
    pub observed_at: f64,
    /// A positional error bound in map units, not an AI confidence score.
    pub uncertainty: f64,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Limits {
    /// Control tolerance for a planned endpoint, distinct from a displayed ring.
    pub position_tolerance: f64,
    pub heading_tolerance_radians: f64,
    pub max_step_distance: f64,
    pub max_turn_radians: f64,
    /// Must fit inside the map planner's footprint safety allowance.
    pub max_path_deviation: f64,
    pub max_uncertainty: f64,
    pub max_pose_age_seconds: f64,
    pub max_future_seconds: f64,
    pub progress_timeout_seconds: f64,
    pub min_progress_distance: f64,
    pub landing_timeout_seconds: f64,
    pub max_duration_seconds: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Decision {
    /// No movement: caller must acquire another actual observation.
    Observe {
        reason: String,
    },
    Turn {
        heading_radians: f64,
        delta_radians: f64,
    },
    Walk {
        toward: Position,
        distance: f64,
    },
    /// Issued only once at a planned, aligned, grounded takeoff.
    Jump {
        id: u32,
        from: Position,
        to: Position,
        apex: f64,
    },
    WaitForLanding {
        id: u32,
    },
    /// Geometric arrival only; the task still verifies interaction/fishing state.
    Arrived {
        position: Position,
    },
    /// Terminal for this route. Release input and obtain a new pose/route.
    Replan {
        reason: String,
    },
    /// Terminal time/validation budget failure. Release input.
    Stop {
        reason: String,
    },
}

#[derive(Clone, Debug)]
struct JumpState {
    issued_at: f64,
    saw_airborne: bool,
}

#[derive(Clone, Debug)]
struct Progress {
    segment: usize,
    best_distance: f64,
    best_heading_error: f64,
    at: f64,
    attempted: bool,
}

#[derive(Clone, Debug)]
pub struct Travel {
    route: Route,
    limits: Limits,
    segment: usize,
    started_at: f64,
    last_now: f64,
    last_pose_at: Option<f64>,
    observations: usize,
    progress: Option<Progress>,
    jumping: Option<JumpState>,
    halted: Option<Decision>,
}

impl Travel {
    /// `now` and Pose.observedAt share the server's monotonic clock. Limits need
    /// measured map/camera/actuator calibration; this module invents none of it.
    pub fn new(route: Route, limits: Limits, now: f64) -> Result<Self, String> {
        validate_limits(limits)?;
        validate_route(&route)?;
        if !now.is_finite() || now < 0.0 {
            return Err("Invalid monotonic start time.".into());
        }
        Ok(Self {
            route,
            limits,
            segment: 0,
            started_at: now,
            last_now: now,
            last_pose_at: None,
            observations: 0,
            progress: None,
            jumping: None,
            halted: None,
        })
    }

    pub fn segment_index(&self) -> usize {
        self.segment
    }

    pub fn observe(&mut self, pose: Pose, now: f64) -> Decision {
        if let Some(decision) = &self.halted {
            return decision.clone();
        }
        if !now.is_finite()
            || now < self.last_now
            || now - self.started_at > self.limits.max_duration_seconds
        {
            return self.halt(false, "Route clock or total duration limit was exceeded.");
        }
        self.last_now = now;
        self.observations += 1;
        if self.observations > MAX_OBSERVATIONS {
            return self.halt(false, "Route observation limit was exceeded.");
        }
        if !valid_position(pose.position)
            || !pose.heading_radians.is_finite()
            || !pose.observed_at.is_finite()
            || pose.observed_at < 0.0
            || !pose.uncertainty.is_finite()
            || pose.uncertainty < 0.0
        {
            return self.halt(
                true,
                "Pose contains invalid map coordinates, heading, time, or uncertainty.",
            );
        }
        if pose.uncertainty > self.limits.max_uncertainty {
            return self.halt(true, "Position uncertainty requires relocalization.");
        }
        if pose.observed_at > now + self.limits.max_future_seconds
            || now - pose.observed_at > self.limits.max_pose_age_seconds
        {
            return self.halt(
                true,
                "Pose is stale or from a future clock; relocalize before movement.",
            );
        }
        if let Some(last) = self.last_pose_at {
            if pose.observed_at < last {
                return self.halt(true, "Out-of-order pose requires relocalization.");
            }
            if pose.observed_at == last {
                return Decision::Observe {
                    reason: "Wait for a newly captured pose before another movement.".into(),
                };
            }
        }
        self.last_pose_at = Some(pose.observed_at);

        // Only zero-length/completed endpoints can be skipped. No timed replay
        // or lookahead movement may cross the current corner or jump link.
        for _ in 0..=self.route.segments.len() {
            let Some(segment) = self.route.segments.get(self.segment).cloned() else {
                if !pose.grounded
                    || pose.position.surface != self.route.end.surface
                    || distance(pose.position.point, self.route.end.point) + pose.uncertainty
                        > self.limits.position_tolerance
                {
                    return self.halt(
                        true,
                        "Final endpoint is not confirmed on its grounded target surface.",
                    );
                }
                let decision = Decision::Arrived {
                    position: pose.position,
                };
                self.halted = Some(decision.clone());
                return decision;
            };
            match segment {
                Segment::Walk { surface, from, to } => {
                    if pose.position.surface != surface {
                        return self
                            .halt(true, "Walking pose is on the wrong surface; relocalize.");
                    }
                    if !pose.grounded {
                        return self.halt(
                            true,
                            "Unexpected airborne pose on a walking route; relocalize.",
                        );
                    }
                    if point_segment_distance(pose.position.point, from, to) + pose.uncertainty
                        > self.limits.max_path_deviation
                    {
                        return self.halt(
                            true,
                            "Pose left the validated route corridor; relocalize and replan.",
                        );
                    }
                    let remaining = distance(pose.position.point, to);
                    if remaining + pose.uncertainty <= self.limits.position_tolerance {
                        self.advance();
                        continue;
                    }
                    if remaining <= EPS {
                        return Decision::Observe {
                            reason: "Endpoint needs a tighter position observation.".into(),
                        };
                    }
                    let desired = heading(pose.position.point, to, pose.heading_radians);
                    let error = angle(desired - pose.heading_radians);
                    if !self.progress(remaining, error.abs(), now) {
                        return self.halt(true, "No observed progress; relocalize instead of jumping or repeating the route.");
                    }
                    if error.abs() > self.limits.heading_tolerance_radians {
                        return self.turn(pose.heading_radians, error);
                    }
                    return Decision::Walk {
                        toward: Position { surface, point: to },
                        distance: remaining.min(self.limits.max_step_distance),
                    };
                }
                Segment::Jump { id, from, to, apex } => {
                    if let Some(jumping) = &mut self.jumping {
                        if pose.position.surface != from.surface
                            && pose.position.surface != to.surface
                        {
                            return self
                                .halt(true, "Jump pose is on an unrelated surface; relocalize.");
                        }
                        if point_segment_distance(pose.position.point, from.point, to.point)
                            + pose.uncertainty
                            > self.limits.max_path_deviation
                        {
                            return self
                                .halt(true, "Jump left its planned flight corridor; relocalize.");
                        }
                        if pose.grounded
                            && pose.position.surface == to.surface
                            && distance(pose.position.point, to.point) + pose.uncertainty
                                <= self.limits.position_tolerance
                        {
                            self.advance();
                            continue;
                        }
                        if jumping.saw_airborne && pose.grounded {
                            return self
                                .halt(true, "Jump landed outside its validated landing region.");
                        }
                        jumping.saw_airborne |= !pose.grounded;
                        if now - jumping.issued_at > self.limits.landing_timeout_seconds {
                            return self.halt(
                                true,
                                "Planned jump did not confirm a grounded landing in time.",
                            );
                        }
                        return Decision::WaitForLanding { id };
                    }
                    if pose.position.surface != from.surface
                        || !pose.grounded
                        || distance(pose.position.point, from.point) + pose.uncertainty
                            > self.limits.position_tolerance
                    {
                        return self.halt(
                            true,
                            "Planned jump requires its grounded takeoff position; relocalize.",
                        );
                    }
                    let desired = heading(from.point, to.point, pose.heading_radians);
                    let error = angle(desired - pose.heading_radians);
                    if !self.progress(distance(pose.position.point, from.point), error.abs(), now) {
                        return self.halt(true, "Jump approach did not align; relocalize.");
                    }
                    if error.abs() > self.limits.heading_tolerance_radians {
                        return self.turn(pose.heading_radians, error);
                    }
                    self.jumping = Some(JumpState {
                        issued_at: now,
                        saw_airborne: false,
                    });
                    return Decision::Jump { id, from, to, apex };
                }
            }
        }
        self.halt(false, "Route endpoint transition limit was exceeded.")
    }

    fn advance(&mut self) {
        self.segment += 1;
        self.progress = None;
        self.jumping = None;
    }

    fn turn(&self, current: f64, error: f64) -> Decision {
        let delta = error.clamp(-self.limits.max_turn_radians, self.limits.max_turn_radians);
        Decision::Turn {
            heading_radians: angle(current + delta),
            delta_radians: delta,
        }
    }

    fn progress(&mut self, remaining: f64, heading_error: f64, now: f64) -> bool {
        let p = self.progress.get_or_insert(Progress {
            segment: self.segment,
            best_distance: remaining,
            best_heading_error: heading_error,
            at: now,
            attempted: false,
        });
        if p.segment != self.segment {
            *p = Progress {
                segment: self.segment,
                best_distance: remaining,
                best_heading_error: heading_error,
                at: now,
                attempted: false,
            };
        }
        if remaining <= p.best_distance - self.limits.min_progress_distance
            || heading_error <= p.best_heading_error - self.limits.heading_tolerance_radians * 0.5
        {
            p.best_distance = p.best_distance.min(remaining);
            p.best_heading_error = p.best_heading_error.min(heading_error);
            p.at = now;
        }
        if p.attempted && now - p.at > self.limits.progress_timeout_seconds {
            return false;
        }
        p.attempted = true;
        true
    }

    fn halt(&mut self, replan: bool, reason: &str) -> Decision {
        let decision = if replan {
            Decision::Replan {
                reason: reason.into(),
            }
        } else {
            Decision::Stop {
                reason: reason.into(),
            }
        };
        self.halted = Some(decision.clone());
        decision
    }
}

fn validate_limits(l: Limits) -> Result<(), String> {
    let distances = [
        l.position_tolerance,
        l.max_step_distance,
        l.max_path_deviation,
        l.min_progress_distance,
    ];
    let times = [
        l.max_pose_age_seconds,
        l.progress_timeout_seconds,
        l.landing_timeout_seconds,
        l.max_duration_seconds,
    ];
    if !distances
        .into_iter()
        .all(|v| v.is_finite() && v > 0.0 && v <= 10_000.0)
        || !times
            .into_iter()
            .all(|v| v.is_finite() && v > 0.0 && v <= 3600.0)
        || !l.max_uncertainty.is_finite()
        || l.max_uncertainty < 0.0
        || l.max_uncertainty > l.position_tolerance
        || l.max_uncertainty >= l.max_path_deviation
        || !l.max_future_seconds.is_finite()
        || !(0.0..=1.0).contains(&l.max_future_seconds)
        || !l.heading_tolerance_radians.is_finite()
        || !(0.0..PI).contains(&l.heading_tolerance_radians)
        || l.heading_tolerance_radians == 0.0
        || !l.max_turn_radians.is_finite()
        || l.max_turn_radians <= 0.0
        || l.max_turn_radians > PI
        || l.min_progress_distance > l.max_step_distance
        || l.max_step_distance <= l.position_tolerance
        || l.max_path_deviation < l.position_tolerance
    {
        return Err("Invalid feedback travel limits.".into());
    }
    Ok(())
}

fn validate_route(route: &Route) -> Result<(), String> {
    if route.distance_metric != "mapPlane"
        || route.segments.len() > MAX_SEGMENTS
        || !valid_position(route.end)
        || ![route.direct_distance, route.route_distance]
            .into_iter()
            .all(|v| v.is_finite() && (0.0..=1_000_000_000.0).contains(&v))
    {
        return Err("Invalid route metric, size, endpoint, or distances.".into());
    }
    let mut prior = None;
    let mut length = 0.0;
    for segment in &route.segments {
        let (from, to) = match *segment {
            Segment::Walk { surface, from, to } => (
                Position {
                    surface,
                    point: from,
                },
                Position { surface, point: to },
            ),
            Segment::Jump { from, to, apex, .. } => {
                if !apex.is_finite() || !(0.0..=10_000.0).contains(&apex) {
                    return Err("Invalid planned jump arc.".into());
                }
                (from, to)
            }
        };
        if !valid_position(from)
            || !valid_position(to)
            || prior.is_some_and(|p: Position| {
                p.surface != from.surface || distance(p.point, from.point) > EPS
            })
        {
            return Err("Route segments are invalid or discontinuous.".into());
        }
        prior = Some(to);
        length += distance(from.point, to.point);
    }
    if prior
        .is_some_and(|p| p.surface != route.end.surface || distance(p.point, route.end.point) > EPS)
        || (length - route.route_distance).abs() > EPS * length.max(1.0)
        || (route.segments.is_empty() != route.already_arrived)
    {
        return Err("Route endpoint, length, or arrival metadata is inconsistent.".into());
    }
    Ok(())
}

fn valid_position(p: Position) -> bool {
    p.point
        .into_iter()
        .all(|v| v.is_finite() && v.abs() <= 1_000_000.0)
}
fn distance(a: Point, b: Point) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt()
}
fn heading(a: Point, b: Point, fallback: f64) -> f64 {
    if distance(a, b) <= EPS {
        angle(fallback)
    } else {
        (b[1] - a[1]).atan2(b[0] - a[0])
    }
}
fn angle(value: f64) -> f64 {
    (value + PI).rem_euclid(2.0 * PI) - PI
}
fn point_segment_distance(p: Point, a: Point, b: Point) -> f64 {
    let d = [b[0] - a[0], b[1] - a[1]];
    let norm = d[0] * d[0] + d[1] * d[1];
    let t = if norm > EPS * EPS {
        (((p[0] - a[0]) * d[0] + (p[1] - a[1]) * d[1]) / norm).clamp(0.0, 1.0)
    } else {
        0.0
    };
    distance(p, [a[0] + d[0] * t, a[1] + d[1] * t])
}

#[cfg(test)]
mod tests {
    use super::*;
    fn pos(surface: u32, x: f64, z: f64) -> Position {
        Position {
            surface,
            point: [x, z],
        }
    }
    fn walk(from: Point, to: Point) -> Segment {
        Segment::Walk {
            surface: 1,
            from,
            to,
        }
    }
    fn route(segments: Vec<Segment>, end: Position) -> Route {
        let length = segments
            .iter()
            .map(|s| match s {
                Segment::Walk { from, to, .. } => distance(*from, *to),
                Segment::Jump { from, to, .. } => distance(from.point, to.point),
            })
            .sum();
        Route {
            distance_metric: "mapPlane".into(),
            direct_distance: 10.0,
            route_distance: length,
            already_arrived: segments.is_empty(),
            end,
            segments,
        }
    }
    fn limits() -> Limits {
        Limits {
            position_tolerance: 0.1,
            heading_tolerance_radians: 0.05,
            max_step_distance: 2.0,
            max_turn_radians: PI / 4.0,
            max_path_deviation: 0.5,
            max_uncertainty: 0.05,
            max_pose_age_seconds: 0.2,
            max_future_seconds: 0.01,
            progress_timeout_seconds: 2.0,
            min_progress_distance: 0.05,
            landing_timeout_seconds: 3.0,
            max_duration_seconds: 60.0,
        }
    }
    fn pose(surface: u32, x: f64, z: f64, heading_radians: f64, grounded: bool, t: f64) -> Pose {
        Pose {
            position: pos(surface, x, z),
            heading_radians,
            grounded,
            observed_at: t,
            uncertainty: 0.0,
        }
    }
    fn travel(segments: Vec<Segment>, end: Position) -> Travel {
        Travel::new(route(segments, end), limits(), 0.0).unwrap()
    }
    #[test]
    fn sharp_turn_finishes_current_segment_before_turning_and_never_crosses_corner() {
        let mut t = travel(
            vec![
                walk([0.0, 0.0], [10.0, 0.0]),
                walk([10.0, 0.0], [10.0, 5.0]),
            ],
            pos(1, 10.0, 5.0),
        );
        assert!(
            matches!(t.observe(pose(1,9.5,0.0,0.0,true,0.0),0.0),Decision::Walk{distance,..} if (distance-0.5).abs()<EPS)
        );
        assert!(
            matches!(t.observe(pose(1,10.0,0.0,0.0,true,0.1),0.1),Decision::Turn{delta_radians,..} if (delta_radians-PI/4.0).abs()<EPS)
        );
        assert_eq!(t.segment_index(), 1);
        assert!(matches!(
            t.observe(pose(1, 10.0, 0.0, PI / 2.0, true, 0.2), 0.2),
            Decision::Walk { distance: 2.0, .. }
        ));
    }
    #[test]
    fn preplanned_jump_requires_aligned_takeoff_then_grounded_landing() {
        let jump = Segment::Jump {
            id: 7,
            from: pos(1, 3.0, 0.0),
            to: pos(2, 7.0, 0.0),
            apex: 2.0,
        };
        let mut t = travel(vec![walk([0.0, 0.0], [3.0, 0.0]), jump], pos(2, 7.0, 0.0));
        assert!(matches!(
            t.observe(pose(1, 2.0, 0.0, 0.0, true, 0.0), 0.0),
            Decision::Walk { distance: 1.0, .. }
        ));
        assert!(matches!(
            t.observe(pose(1, 3.0, 0.0, PI / 2.0, true, 0.1), 0.1),
            Decision::Turn { .. }
        ));
        assert!(matches!(
            t.observe(pose(1, 3.0, 0.0, 0.0, true, 0.2), 0.2),
            Decision::Jump { id: 7, .. }
        ));
        assert_eq!(
            t.observe(pose(1, 3.0, 0.0, 0.0, true, 0.3), 0.3),
            Decision::WaitForLanding { id: 7 }
        );
        assert_eq!(
            t.observe(pose(1, 5.0, 0.0, 0.0, false, 0.4), 0.4),
            Decision::WaitForLanding { id: 7 }
        );
        assert_eq!(
            t.observe(pose(2, 7.0, 0.0, 0.0, false, 0.5), 0.5),
            Decision::WaitForLanding { id: 7 }
        );
        assert!(matches!(
            t.observe(pose(2, 7.0, 0.0, 0.0, true, 0.6), 0.6),
            Decision::Arrived { .. }
        ));
    }
    #[test]
    fn no_progress_never_invents_a_jump_or_replays_whole_route() {
        let mut t = travel(vec![walk([0.0, 0.0], [10.0, 0.0])], pos(1, 10.0, 0.0));
        assert!(matches!(
            t.observe(pose(1, 0.0, 0.0, 0.0, true, 0.0), 0.0),
            Decision::Walk { distance: 2.0, .. }
        ));
        assert!(matches!(
            t.observe(pose(1, 0.0, 0.0, 0.0, true, 1.0), 1.0),
            Decision::Walk { distance: 2.0, .. }
        ));
        assert!(matches!(
            t.observe(pose(1, 0.0, 0.0, 0.0, true, 2.1), 2.1),
            Decision::Replan { .. }
        ));
        assert!(matches!(
            t.observe(pose(1, 1.0, 0.0, 0.0, true, 2.2), 2.2),
            Decision::Replan { .. }
        ));
    }
    #[test]
    fn stale_future_out_of_order_and_repeated_poses_do_not_produce_motion() {
        for p in [
            pose(1, 0.0, 0.0, 0.0, true, 0.0),
            pose(1, 0.0, 0.0, 0.0, true, 1.1),
        ] {
            let mut t = travel(vec![walk([0.0, 0.0], [10.0, 0.0])], pos(1, 10.0, 0.0));
            assert!(matches!(t.observe(p, 1.0), Decision::Replan { .. }));
        }
        let mut t = travel(vec![walk([0.0, 0.0], [10.0, 0.0])], pos(1, 10.0, 0.0));
        t.observe(pose(1, 0.0, 0.0, 0.0, true, 0.1), 0.1);
        assert!(matches!(
            t.observe(pose(1, 0.0, 0.0, 0.0, true, 0.1), 0.1),
            Decision::Observe { .. }
        ));
        assert!(matches!(
            t.observe(pose(1, 0.0, 0.0, 0.0, true, 0.05), 0.15),
            Decision::Replan { .. }
        ));
    }
    #[test]
    fn wrong_layer_uncertainty_and_path_deviation_cannot_arrive() {
        for p in [
            pose(2, 10.0, 0.0, 0.0, true, 0.0),
            Pose {
                uncertainty: 0.1,
                ..pose(1, 10.0, 0.0, 0.0, true, 0.0)
            },
            pose(1, 5.0, 1.0, 0.0, true, 0.0),
        ] {
            let mut t = travel(vec![walk([0.0, 0.0], [10.0, 0.0])], pos(1, 10.0, 0.0));
            assert!(matches!(t.observe(p, 0.0), Decision::Replan { .. }));
        }
    }
    #[test]
    fn coast_arrives_at_planned_allowed_shore_endpoint_not_water_center() {
        let mut t = travel(vec![walk([0.0, 0.0], [8.0, -1.0])], pos(1, 8.0, -1.0));
        let h = heading([7.0, -0.875], [8.0, -1.0], 0.0);
        assert!(matches!(
            t.observe(pose(1, 7.0, -0.875, h, true, 0.0), 0.0),
            Decision::Walk { .. }
        ));
        assert!(
            matches!(t.observe(pose(1,8.0,-1.0,h,true,0.1),0.1),Decision::Arrived{position} if position==pos(1,8.0,-1.0))
        );
    }
    #[test]
    fn invalid_route_time_limits_and_jump_landing_fail_closed() {
        let mut bad = route(
            vec![walk([0.0, 0.0], [1.0, 0.0]), walk([2.0, 0.0], [3.0, 0.0])],
            pos(1, 3.0, 0.0),
        );
        assert!(Travel::new(bad.clone(), limits(), 0.0).is_err());
        bad.segments.clear();
        bad.route_distance = f64::NAN;
        assert!(Travel::new(bad, limits(), 0.0).is_err());
        let mut l = limits();
        l.max_step_distance = 0.0;
        assert!(Travel::new(route(vec![], pos(1, 0.0, 0.0)), l, 0.0).is_err());
        let mut t = travel(
            vec![Segment::Jump {
                id: 1,
                from: pos(1, 0.0, 0.0),
                to: pos(1, 3.0, 0.0),
                apex: 1.0,
            }],
            pos(1, 3.0, 0.0),
        );
        assert!(matches!(
            t.observe(pose(1, 0.0, 0.0, 0.0, true, 0.0), 0.0),
            Decision::Jump { .. }
        ));
        t.observe(pose(1, 1.0, 0.0, 0.0, false, 0.1), 0.1);
        assert!(matches!(
            t.observe(pose(1, 1.0, 0.0, 0.0, true, 0.2), 0.2),
            Decision::Replan { .. }
        ));
        let mut t = travel(vec![], pos(1, 0.0, 0.0));
        assert!(matches!(
            t.observe(pose(1, 0.0, 0.0, 0.0, true, 61.0), 61.0),
            Decision::Stop { .. }
        ));
    }
    #[test]
    fn heading_wrap_jump_timeout_and_segment_budget_are_bounded() {
        let mut t = travel(vec![walk([0.0, 0.0], [-10.0, -0.1])], pos(1, -10.0, -0.1));
        // Cross the -pi/pi seam by the short turn; do not turn almost a full circle.
        let decision = t.observe(pose(1, 0.0, 0.0, PI - 0.1, true, 0.0), 0.0);
        assert!(
            matches!(decision,Decision::Turn{delta_radians,..} if delta_radians>0.0 && delta_radians<0.2)
        );
        let mut t = travel(
            vec![Segment::Jump {
                id: 1,
                from: pos(1, 0.0, 0.0),
                to: pos(1, 3.0, 0.0),
                apex: 1.0,
            }],
            pos(1, 3.0, 0.0),
        );
        assert!(matches!(
            t.observe(pose(1, 0.0, 0.0, 0.0, true, 0.0), 0.0),
            Decision::Jump { .. }
        ));
        assert!(matches!(
            t.observe(pose(1, 0.0, 0.0, 0.0, true, 3.1), 3.1),
            Decision::Replan { .. }
        ));
        let segments = (0..=MAX_SEGMENTS)
            .map(|i| walk([i as f64, 0.0], [i as f64 + 1.0, 0.0]))
            .collect();
        assert!(Travel::new(
            route(segments, pos(1, MAX_SEGMENTS as f64 + 1.0, 0.0)),
            limits(),
            0.0
        )
        .is_err());
    }
}

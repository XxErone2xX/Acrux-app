//! Screenshot-led travel to the few game destinations. No world coordinates,
//! heights or timed routes are supplied or inferred from screen-center position.
use crate::{
    automation::{Command, Word},
    device::Gesture,
    navigator::{Calibration, Controls, Navigator, Observed},
    scene::{self, Snapshot},
    vision::{Frame, Point, Rect, Region, WindowFinder},
};
use serde::{Deserialize, Serialize};
use std::{
    cmp::Ordering,
    collections::{BinaryHeap, VecDeque},
};

#[path = "wayfinder_jump.rs"]
mod jump;

pub(crate) use crate::navigator::Destination;
const MAX_SECONDS: f64 = 240.;
const MAX_ACTIONS: u32 = 160;
const PATCH: usize = 24;

#[derive(Clone, Default, Debug, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Settings {
    pub controls: Option<Controls>,
    pub jump: Option<Point>,
    /// An actual overhead name, optionally `display name|account name`.
    pub player_label: String,
    pub excluded: Vec<Rect>,
    pub memory: Memory,
}

#[derive(Clone, Default, Debug, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Memory {
    pub anchors: Vec<Anchor>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Anchor {
    pub destination: Destination,
    pub label: String,
    pub patch: Vec<u8>,
    pub size: [f64; 2],
    pub approach: [f64; 2],
    pub aspect: f64,
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
    pub route: Vec<Point>,
    pub avatar: Option<Point>,
    pub goal: Option<Rect>,
}

pub(crate) enum Tick {
    Observe { region: Option<Region>, ocr: bool },
    Command(Command),
    Wait(u32),
    Arrived,
    Stopped(String),
}

#[derive(Clone, Copy)]
enum After {
    Camera(u8),
    Navigate,
    Interact,
    QuestExit,
    Portal,
    Jump(u32),
    Survey,
}
enum Phase {
    Camera(u8),
    Locate,
    Navigate,
    Interact,
    Jump,
    Survey,
    Input {
        after: After,
        at: f64,
        duration: u32,
    },
    Settle {
        after: After,
        until: f64,
    },
    Arrived,
    Stopped,
}

#[derive(Clone, Copy, Debug)]
struct Contact {
    point: Point,
    bounds: Rect,
    radius: f64,
    error: f64,
}

struct JumpSession {
    runner: jump::Jumper,
    reference: Snapshot,
    included: Region,
    excluded: Vec<Region>,
    unknown: u8,
    body: Contact,
    landing: Option<LandingReference>,
}
struct LandingReference {
    region: Region,
}
struct JumpModel {
    calibration: jump::JumpCalibration,
    reference: Snapshot,
    included: Region,
    excluded: Vec<Region>,
    body: Contact,
}
struct Survey {
    reference: Snapshot,
    grid: Grid,
    goal: Point,
    included: Region,
    excluded: Vec<Region>,
    body: Contact,
    anchor: Region,
}
struct RaisedPlane {
    mapping: scene::Registration,
    height: f64,
    height_error: f64,
    reference_region: Region,
    before_point: [f64; 2],
    current_point: [f64; 2],
}

pub(crate) struct Wayfinder {
    settings: Settings,
    state: Status,
    phase: Phase,
    geometry: Option<(usize, usize)>,
    started: f64,
    last_now: f64,
    last_frame: f64,
    navigator: Option<Navigator>,
    intermediate: bool,
    craft_stage: u8,
    last_goal: Option<Point>,
    pending_anchor: Option<Anchor>,
    interaction_attempts: u8,
    unknown_frames: u8,
    coast_rejected: Vec<Point>,
    walk: Option<Calibration>,
    jump_session: Option<JumpSession>,
    measured_jump: Option<JumpModel>,
    survey: Option<Survey>,
    jump_approach: bool,
    jump_bootstrap: bool,
    survey_failed: bool,
    required_direction: Option<[f64; 2]>,
}

impl Wayfinder {
    /// A user-requested mark binds a visible, destination-specific landmark to
    /// the character's ACTUAL supported feet in this same captured frame.
    /// It never turns a viewport point, guessed center or hidden landmark into
    /// a saved destination. Automatic marks still require the real task UI.
    pub(crate) fn mark(
        destination: Destination,
        settings: Settings,
        geometry: (usize, usize),
        frame: &Frame<'_>,
        words: &[Word],
    ) -> Result<Memory, String> {
        let mut finder = Self::start(destination, settings, 0.)?;
        if !frame.is_full()
            || geometry != (frame.width, frame.height)
            || words.len() > 512
            || words
                .iter()
                .any(|w| w.text.len() > 512 || !rect_valid(w.rect))
        {
            return Err(
                "목적지 등록에는 새 전체 화면과 같은 화면의 글자 관측이 필요합니다.".into(),
            );
        }
        let contact = avatar_contact(frame, words, &finder.settings.player_label)?;
        let excluded = exclusions(
            frame,
            words,
            &finder.settings.excluded,
            &finder.settings.player_label,
        );
        let grid = Grid::observe(frame, contact, &excluded)?;
        if grid
            .seeds(
                contact.point,
                [
                    contact.point.0 * frame.width as f64,
                    contact.point.1 * frame.height as f64,
                ],
                contact.radius,
            )
            .len()
            < 3
        {
            return Err("현재 발을 둘러싼 실제 지면이 충분히 보이지 않습니다.".into());
        }
        let labels: &[&str] = match destination {
            Destination::Base => &[
                "spawn",
                "spawnpoint",
                "스폰",
                "리스폰",
                "mostrolls",
                "가장많이뽑은사람",
            ],
            Destination::Coast => &["fishing", "낚시", "fish"],
            Destination::Sell => &["captainflarg", "물고기판매", "sellfish"],
            Destination::Craft => &[
                "stellasworkshop",
                "스텔라의공방",
                "questboard",
                "퀘스트보드",
                "stellaportal",
            ],
            Destination::Match => &["memorymatch", "메모리매치"],
        };
        let candidates: Vec<_> = matching_words(words, labels, false)
            .into_iter()
            .filter(|w| w.rect.1 > 0.08 && w.rect.3 < 0.82)
            .collect();
        if candidates.len() != 1 {
            return Err("이 목적지의 실제 표식을 하나로 구분하지 못했습니다. 자기 캐릭터와 목적지 표식이 함께 보이게 해 주세요.".into());
        }
        let word = &candidates[0];
        let rect = expand(word.rect, 0.06, 0.12);
        let anchor = capture_anchor(frame, rect, contact.point, destination, word.text.clone())
            .ok_or("현재 화면의 목적지 표식 이미지를 저장하지 못했습니다.")?;
        if !anchor.approach.iter().all(|v| (-1.0..=2.0).contains(v)) {
            return Err("목적지 표식과 캐릭터가 너무 멀어 현재 위치를 등록할 수 없습니다.".into());
        }
        finder.remember(anchor);
        Ok(finder.memory())
    }

    pub(crate) fn start(
        destination: Destination,
        settings: Settings,
        now: f64,
    ) -> Result<Self, String> {
        if !now.is_finite()
            || now < 0.
            || settings.player_label.len() > 160
            || settings.excluded.len() > 24
            || settings.memory.anchors.len() > 16
            || settings.excluded.iter().any(|r| !rect_valid(*r))
            || settings.jump.is_some_and(|p| !valid(p))
        {
            return Err("이동 설정과 관측 시작 시각을 확인하세요.".into());
        }
        for a in &settings.memory.anchors {
            if a.patch.len() != PATCH * PATCH
                || a.label.len() > 120
                || !a.aspect.is_finite()
                || a.aspect <= 0.
                || !a.size.iter().all(|v| v.is_finite() && *v > 0. && *v <= 1.)
                || !a
                    .approach
                    .iter()
                    .all(|v| v.is_finite() && (-1.0..=2.0).contains(v))
            {
                return Err("자동 학습한 목적지 관측 데이터가 올바르지 않습니다.".into());
            }
        }
        if settings
            .controls
            .is_some_and(|c| !valid(c.stick_center) || !valid(c.stick_edge))
        {
            return Err("저장된 조이스틱 위치를 확인하세요.".into());
        }
        Ok(Self {
            settings,
            state: Status {
                destination,
                phase: "observing",
                message: "현재 화면과 실제 목적지 표시를 확인합니다.".into(),
                observations: 0,
                actions: 0,
                remaining_pixels: 0.,
                route: Vec::new(),
                avatar: None,
                goal: None,
            },
            phase: Phase::Camera(0),
            geometry: None,
            started: now,
            last_now: now,
            last_frame: f64::NEG_INFINITY,
            navigator: None,
            intermediate: false,
            craft_stage: 0,
            last_goal: None,
            pending_anchor: None,
            interaction_attempts: 0,
            unknown_frames: 0,
            coast_rejected: Vec::new(),
            walk: None,
            jump_session: None,
            measured_jump: None,
            survey: None,
            jump_approach: false,
            jump_bootstrap: false,
            survey_failed: false,
            required_direction: None,
        })
    }
    pub(crate) fn status(&self) -> Status {
        self.state.clone()
    }
    pub(crate) fn memory(&self) -> Memory {
        self.settings.memory.clone()
    }
    pub(crate) fn ack(&mut self, success: bool, now: f64) {
        let Phase::Input {
            after,
            at,
            duration,
        } = self.phase
        else {
            return;
        };
        if !success || !now.is_finite() || now < at || now - at > duration as f64 / 1000. + 2. {
            self.stop("이동 입력이 취소됐습니다. 모든 입력을 해제하고 재관측합니다.");
            return;
        }
        if matches!(after, After::Navigate) {
            if let Some(n) = self.navigator.as_mut() {
                n.ack(true, now);
            }
        }
        if let After::Jump(id) = after {
            if let Some(session) = self.jump_session.as_mut() {
                session.runner.ack(id, success, now);
            }
        }
        self.last_now = self.last_now.max(now);
        self.phase = Phase::Settle {
            after,
            until: now
                + if matches!(after, After::Jump(_)) {
                    0.
                } else {
                    0.18
                },
        };
    }
    pub(crate) fn tick(
        &mut self,
        now: f64,
        geometry: (usize, usize),
        frame: Option<&Frame<'_>>,
        words: &[Word],
        observation: Option<(f64, f64)>,
    ) -> Tick {
        if matches!(self.phase, Phase::Arrived) {
            return Tick::Arrived;
        }
        if matches!(self.phase, Phase::Stopped) {
            return Tick::Stopped(self.state.message.clone());
        }
        if !now.is_finite()
            || now < self.last_now
            || now - self.started > MAX_SECONDS
            || self.state.actions >= MAX_ACTIONS
        {
            return self
                .stop("목적지 관측 시간·입력 제한을 넘었습니다. 현재 위치를 다시 확인하세요.");
        }
        if geometry.0 == 0
            || geometry.1 == 0
            || geometry.0 > 16384
            || geometry.1 > 16384
            || self.geometry.is_some_and(|old| old != geometry)
        {
            return self.stop("화면 크기가 바뀌어 기존 지면 관측을 사용할 수 없습니다.");
        }
        self.geometry = Some(geometry);
        self.last_now = now;
        if let Phase::Input { at, duration, .. } = self.phase {
            return if now - at > duration as f64 / 1000. + 2. {
                self.stop("짧은 이동 입력의 완료 응답이 없습니다.")
            } else {
                Tick::Wait(20)
            };
        }
        if let Phase::Settle { after, until } = self.phase {
            if now < until {
                return Tick::Wait(((until - now) * 1000.).ceil().clamp(1., 1000.) as u32);
            }
            self.phase = match after {
                After::Camera(n) => Phase::Camera(n),
                After::Navigate => Phase::Navigate,
                After::Interact => Phase::Interact,
                After::QuestExit | After::Portal => Phase::Locate,
                After::Jump(_) => Phase::Jump,
                After::Survey => Phase::Survey,
            };
        }
        let Some(frame) = frame else {
            return if matches!(self.phase, Phase::Jump) {
                self.jump_tick(None, words, now)
            } else {
                observe()
            };
        };
        if !frame.is_full() || (frame.width, frame.height) != geometry {
            return observe();
        }
        // The transport grants a processing-aware input deadline. Capture time
        // remains physical evidence for movement/jump measurements.
        let Some((captured_at, _)) = observation.filter(|(captured, deadline)| {
            captured.is_finite()
                && *captured >= 0.
                && *captured <= now + 0.02
                && deadline.is_finite()
                && now <= *deadline
                && now - *captured <= 5.3
        }) else {
            return self
                .stop("실제 화면 캡처 시각이 없거나 화면이 오래되어 이동 입력을 중지합니다.");
        };
        if captured_at <= self.last_frame {
            return observe();
        }
        if words.len() > 512
            || words
                .iter()
                .any(|w| w.text.len() > 512 || !rect_valid(w.rect))
        {
            return self.stop("화면 글자 관측의 영역이나 크기가 올바르지 않습니다.");
        }
        self.last_frame = captured_at;
        self.state.observations += 1;
        if task_ui(self.state.destination, frame, words) {
            if let Some(anchor) = self.pending_anchor.take() {
                self.remember(anchor);
            }
            self.phase = Phase::Arrived;
            self.state.phase = "arrived";
            self.state.message = "실제 목적지 기능 화면을 확인했습니다.".into();
            return Tick::Arrived;
        }
        match self.phase {
            Phase::Camera(0) => self.emit(
                Command::Gesture(Gesture::Drag {
                    from: Point(0.80, 0.25),
                    to: Point(0.80, 0.65),
                    ms: 350,
                }),
                After::Camera(1),
                350,
                "camera",
                "가려지지 않은 시점으로 카메라를 올립니다.",
            ),
            Phase::Camera(1) => self.emit(
                Command::Gesture(Gesture::Pinch {
                    from: [Point(0.62, 0.30), Point(0.88, 0.30)],
                    to: [Point(0.70, 0.30), Point(0.80, 0.30)],
                    ms: 300,
                }),
                After::Camera(2),
                300,
                "camera",
                "실제 주변 지형을 볼 수 있도록 시야를 넓힙니다.",
            ),
            Phase::Camera(_) => {
                self.phase = Phase::Locate;
                self.locate(frame, words, now)
            }
            Phase::Locate => self.locate(frame, words, now),
            Phase::Navigate => self.navigate(frame, words, now),
            Phase::Interact => self.interact(frame, words, now),
            Phase::Jump => self.jump_tick(Some(frame), words, now),
            Phase::Survey => self.survey_tick(frame, words, now),
            _ => observe(),
        }
    }
    fn emit(
        &mut self,
        command: Command,
        after: After,
        duration: u32,
        phase: &'static str,
        message: &str,
    ) -> Tick {
        self.phase = Phase::Input {
            after,
            at: self.last_now,
            duration,
        };
        self.state.actions += 1;
        self.state.phase = phase;
        self.state.message = message.into();
        Tick::Command(command)
    }
    fn stop(&mut self, message: &str) -> Tick {
        self.phase = Phase::Stopped;
        self.state.phase = "stopped";
        self.state.message = message.into();
        Tick::Stopped(message.into())
    }
    fn retry_observation(&mut self, message: &str) -> Tick {
        self.unknown_frames += 1;
        self.state.phase = "observing";
        self.state.message = message.into();
        if self.unknown_frames >= 8 {
            self.stop(message)
        } else {
            observe()
        }
    }
    fn locate(&mut self, frame: &Frame<'_>, words: &[Word], now: f64) -> Tick {
        if self.settings.player_label.trim().is_empty() {
            return self.stop("자기 캐릭터의 표시 이름을 설정하세요. 다른 플레이어나 화면 중앙을 현재 위치로 사용하지 않습니다.");
        }
        let contact = match avatar_contact(frame, words, &self.settings.player_label) {
            Ok(v) => v,
            Err(e) => return self.retry_observation(&e),
        };
        let controls=match self.settings.controls.or_else(||detect_controls(frame)) {Some(v)=>v,None=>return self.stop("실제 조이스틱 가운데와 가장자리 위치가 필요합니다. 조이스틱이 보이게 하거나 기존 입력 위치를 지정하세요.")};
        self.settings.controls = Some(controls);
        let excluded = exclusions(
            frame,
            words,
            &self.settings.excluded,
            &self.settings.player_label,
        );
        let grid = match Grid::observe(frame, contact, &excluded) {
            Ok(v) => v,
            Err(e) => return self.retry_observation(&e),
        };
        let goal = self.goal(frame, words, &grid, contact);
        let Some((goal, label, anchor_box)) = goal else {
            return self.retry_observation("현재 화면에서 목적지 표시나 안전한 해안 접근 영역을 확인하지 못했습니다. 관측된 표식이 없는 방향으로 걷지 않습니다.");
        };
        let route = grid.route(contact.point, goal, contact.radius + contact.error + 3.);
        if route.as_ref().is_some_and(|r| r.len() == 1) {
            self.state.avatar = Some(contact.point);
            self.last_goal = Some(goal);
            self.pending_anchor = anchor_box.and_then(|r| {
                capture_anchor(frame, r, contact.point, self.state.destination, label)
            });
            self.phase = Phase::Interact;
            return self.interact(frame, words, now);
        }
        if self.settings.jump.is_some()
            && route
                .as_ref()
                .is_none_or(|r| r.len() > 2 && !self.survey_failed)
        {
            if self.measured_jump.is_none() && self.walk.is_some() && self.settings.jump.is_some() {
                return self.start_jump_measurement(frame, words, contact, &grid, goal, now);
            }
            if self.measured_jump.is_some() {
                return self.start_survey(frame, words, grid, goal);
            }
            if self.walk.is_none() && self.settings.jump.is_some() {
                if let Some(p) = grid.calibration_goal(contact, goal) {
                    self.jump_bootstrap = true;
                    return self.start_segment(frame, words, now, contact, &grid, p, true, None);
                }
            }
        }
        let Some(route) = route.filter(|r| r.len() >= 2) else {
            return self.retry_observation("확인한 지면 안에 통과 가능한 경로가 없습니다. 높이·착지면이 확인되지 않은 벽이나 틈에서는 점프하지 않습니다.");
        };
        self.state.avatar = Some(contact.point);
        self.state.route = route.iter().map(|&p| grid.normalized(p)).collect();
        self.last_goal = Some(goal);
        self.state.remaining_pixels = route.windows(2).map(|p| distance(p[0], p[1])).sum();
        let waypoint = route[1];
        self.intermediate = route.len() > 2;
        let anchor = if !self.intermediate {
            anchor_box
                .and_then(|rect| capture_anchor(frame, rect, goal, self.state.destination, label))
        } else {
            None
        };
        self.start_segment(
            frame,
            words,
            now,
            contact,
            &grid,
            waypoint,
            self.intermediate,
            anchor,
        )
    }
    fn start_segment(
        &mut self,
        frame: &Frame<'_>,
        words: &[Word],
        now: f64,
        contact: Contact,
        grid: &Grid,
        waypoint: [f64; 2],
        intermediate: bool,
        anchor: Option<Anchor>,
    ) -> Tick {
        let excluded = exclusions(
            frame,
            words,
            &self.settings.excluded,
            &self.settings.player_label,
        );
        let controls = self.settings.controls.unwrap();
        self.intermediate = intermediate;
        let clearance = contact.radius + contact.error + 3.;
        let waypoint_region = grid.approach(waypoint, clearance);
        let Some(waypoint_region) = waypoint_region else {
            return self.retry_observation("모서리 뒤 접근 영역의 발판 여유가 부족합니다.");
        };
        let seeds: Vec<_> = grid
            .seeds(contact.point, waypoint, clearance)
            .into_iter()
            .filter(|p| {
                inside(grid.included(), *p)
                    && !inside(contact.bounds, *p)
                    && !excluded.iter().any(|r| inside(*r, *p))
            })
            .collect();
        if seeds.len() < 3 {
            return self.retry_observation("같은 지면의 관측 범위가 부족합니다.");
        }
        let observed = Observed {
            avatar_contact: contact.point,
            avatar_bounds: contact.bounds,
            goal_region: waypoint_region,
            ground_seeds: seeds,
            included: grid.included(),
            excluded: excluded
                .iter()
                .copied()
                .filter(|r| !inside(*r, contact.point))
                .collect(),
            clearance_pixels: clearance,
        };
        let navigator =
            match Navigator::from_frame(self.state.destination, frame, &observed, controls, now) {
                Ok(v) => v,
                Err(e) => return self.retry_observation(&e),
            };
        self.navigator = Some(navigator);
        self.phase = Phase::Navigate;
        self.unknown_frames = 0;
        self.state.goal = Some(waypoint_region);
        if !self.intermediate {
            self.pending_anchor = anchor;
        }
        self.state.phase = "localizing";
        self.state.message = "실제 발 접점과 지면을 바탕으로 모서리 경로를 따라갑니다.".into();
        observe()
    }
    fn navigate(&mut self, frame: &Frame<'_>, words: &[Word], now: f64) -> Tick {
        let contact = match avatar_contact(frame, words, &self.settings.player_label) {
            Ok(v) => v,
            Err(e) => return self.retry_observation(&e),
        };
        self.state.avatar = Some(contact.point);
        let Some(n) = self.navigator.as_mut() else {
            self.phase = Phase::Locate;
            return observe();
        };
        let tick = n.tick_observed(
            now,
            Some(frame),
            contact.point,
            contact.bounds,
            contact.error,
        );
        self.state.remaining_pixels = n.status().remaining_pixels;
        if let Some(c) = n.status().calibration {
            self.walk = Some(c);
        }
        match tick {
            crate::navigator::Tick::Gesture(g) => {
                let ms = g.duration_ms();
                self.emit(
                    Command::Gesture(g),
                    After::Navigate,
                    ms,
                    "moving",
                    "관측된 모서리 경로에서 짧게 이동하고 실제 위치를 다시 확인합니다.",
                )
            }
            crate::navigator::Tick::Observe => observe(),
            crate::navigator::Tick::Wait(ms) => Tick::Wait(ms),
            crate::navigator::Tick::Done => {
                self.navigator = None;
                self.unknown_frames = 0;
                self.survey_failed = false;
                if self.jump_approach || self.jump_bootstrap {
                    self.jump_approach = false;
                    self.jump_bootstrap = false;
                    self.phase = Phase::Locate;
                    observe()
                } else if self.intermediate {
                    self.phase = Phase::Locate;
                    observe()
                } else {
                    self.phase = Phase::Interact;
                    self.interact(frame, words, now)
                }
            }
            crate::navigator::Tick::Stopped(e) => self.stop(&e),
        }
    }
    fn start_jump_measurement(
        &mut self,
        frame: &Frame<'_>,
        words: &[Word],
        contact: Contact,
        grid: &Grid,
        goal: Point,
        now: f64,
    ) -> Tick {
        let (ground, shadow) = grounding(frame, contact);
        if ground != jump::Grounding::Supported {
            return self.retry_observation("점프 보정에 필요한 실제 발과 그림자의 접지를 확인하지 못했습니다. 시간만으로 착지나 출발을 가정하지 않습니다.");
        }
        let Some(support) = grid.flat_hull(contact.radius + contact.error + 3.) else {
            return self.retry_observation(
                "점프를 실측할 만큼 넓고 같은 지면의 이륙·착지 여유가 보이지 않습니다.",
            );
        };
        let (Some(controls), Some(button), Some(walk)) =
            (self.settings.controls, self.settings.jump, self.walk)
        else {
            return self.stop("관측한 조이스틱 보정과 점프 버튼 위치가 필요합니다.");
        };
        let included = grid.included().region((frame.width, frame.height)).unwrap();
        let mut excluded: Vec<_> = exclusions(
            frame,
            words,
            &self.settings.excluded,
            &self.settings.player_label,
        )
        .into_iter()
        .filter_map(|r| r.region((frame.width, frame.height)))
        .collect();
        // One fixed mask covers all three small experiments. Ground texture
        // around it, independently matched in each frame, tracks the plane.
        if let Some(r) = expand(
            contact.bounds,
            (contact.bounds.2 - contact.bounds.0) * 1.5,
            (contact.bounds.3 - contact.bounds.1) * 1.3,
        )
        .region((frame.width, frame.height))
        {
            excluded.push(r);
        }
        let reference = match Snapshot::capture(frame, included, &excluded) {
            Ok(s) => s,
            Err(e) => return self.retry_observation(&e),
        };
        let origin = jump::JumpObservation {
            foot: [
                contact.point.0 * frame.width as f64,
                contact.point.1 * frame.height as f64,
            ],
            shadow: shadow.map(|p| [p.0 * frame.width as f64, p.1 * frame.height as f64]),
            error_pixels: contact.error,
            observed_at: self.last_frame,
            ground,
        };
        let mut directions = Vec::new();
        let desired = [
            (goal.0 - contact.point.0) * frame.width as f64,
            (goal.1 - contact.point.1) * frame.height as f64,
        ];
        if let Some(d) = self
            .required_direction
            .take()
            .or_else(|| control_direction(walk, desired))
        {
            directions.push(d);
            directions.push([-d[0], -d[1]]);
        }
        for direction in [
            [1., 0.],
            [-1., 0.],
            [0., 1.],
            [0., -1.],
            [0.5f64.sqrt(), 0.5f64.sqrt()],
            [-0.5f64.sqrt(), -0.5f64.sqrt()],
            [-0.5f64.sqrt(), 0.5f64.sqrt()],
            [0.5f64.sqrt(), -0.5f64.sqrt()],
        ] {
            if directions.iter().all(|d| distance(*d, direction) > 0.02) {
                directions.push(direction);
            }
        }
        let runner = match jump::Jumper::start_directions(
            controls,
            button,
            (frame.width, frame.height),
            walk,
            origin,
            support,
            directions,
            now,
        ) {
            Ok(r) => r,
            Err(e) => return self.retry_observation(&e),
        };
        self.navigator = None;
        self.jump_session = Some(JumpSession {
            runner,
            reference,
            included,
            excluded,
            unknown: 0,
            body: contact,
            landing: None,
        });
        self.phase = Phase::Jump;
        self.state.phase = "jumpMeasuring";
        self.state.message =
            "보이는 평지에서 공중 분리와 착지를 직접 관측해 점프를 측정합니다.".into();
        observe()
    }

    fn jump_tick(&mut self, frame: Option<&Frame<'_>>, words: &[Word], now: f64) -> Tick {
        let captured_at = self.last_frame;
        let Some(session) = self.jump_session.as_mut() else {
            return self.stop("점프 관측 세션이 없습니다.");
        };
        let observation = frame.and_then(|frame| {
            avatar_contact(frame, words, &self.settings.player_label)
                .ok()
                .and_then(|contact| {
                    let (ground, shadow) = grounding(frame, contact);
                    let current =
                        Snapshot::capture(frame, session.included, &session.excluded).ok()?;
                    let limit = frame
                        .width
                        .max(frame.height)
                        .div_ceil(512)
                        .max(1)
                        .saturating_mul(48)
                        .min(512);
                    let registration = if ground == jump::Grounding::Supported
                        && session.runner.landing_expected()
                        && session.landing.is_some()
                    {
                        scene::register_local(
                            &session.reference,
                            &current,
                            limit,
                            session.landing.as_ref()?.region,
                        )
                        .ok()?
                    } else {
                        let anchor = foot_anchor(
                            session.body,
                            session.included,
                            (frame.width, frame.height),
                        )?;
                        scene::register_anchored(&session.reference, &current, limit, anchor)
                            .ok()?
                    };
                    let foot = registration.inverse_point([
                        contact.point.0 * frame.width as f64,
                        contact.point.1 * frame.height as f64,
                    ])?;
                    let shadow = registration.inverse_point([
                        shadow?.0 * frame.width as f64,
                        shadow?.1 * frame.height as f64,
                    ])?;
                    Some(jump::JumpObservation {
                        foot,
                        shadow: Some(shadow),
                        error_pixels: contact.error + registration.error_pixels,
                        observed_at: captured_at,
                        ground,
                    })
                })
        });
        if frame.is_some() {
            if observation.is_none_or(|o| o.ground == jump::Grounding::Unknown) {
                session.unknown += 1;
            } else {
                session.unknown = 0;
            }
            if session.unknown >= 8 {
                return self.stop("점프의 실제 발·그림자·같은 지면 관측이 반복해서 가려졌습니다. 모든 입력을 해제하고 중지합니다.");
            }
        }
        match session.runner.tick(now, observation) {
            jump::Tick::Observe => observe(),
            jump::Tick::Wait(ms) => Tick::Wait(ms),
            jump::Tick::Gesture { id, gesture } => {
                let ms = gesture.duration_ms();
                self.emit(
                    Command::Gesture(gesture),
                    After::Jump(id),
                    ms,
                    "jumpMeasuring",
                    "관측한 평지에서 짧은 점프 입력을 보내고 발과 그림자를 다시 확인합니다.",
                )
            }
            jump::Tick::Measured(measured) => {
                let session = self.jump_session.take().unwrap();
                self.measured_jump = Some(JumpModel {
                    calibration: measured,
                    reference: session.reference,
                    included: session.included,
                    excluded: session.excluded,
                    body: session.body,
                });
                self.phase = Phase::Locate;
                self.unknown_frames = 0;
                self.state.message="각 요청 방향의 실제 이륙·비행·안정된 착지로 점프를 측정했습니다. 현재 지형을 다시 확인합니다.".into();
                observe()
            }
            jump::Tick::Landed => {
                self.jump_session = None;
                self.phase = Phase::Locate;
                self.unknown_frames = 0;
                self.state.message="계획한 착지면에서 실제 공중 분리와 안정된 발 접점을 확인했습니다. 목적지를 다시 관측합니다.".into();
                observe()
            }
            jump::Tick::Stopped(e) => self.stop(&e),
        }
    }

    fn start_survey(&mut self, frame: &Frame<'_>, words: &[Word], grid: Grid, goal: Point) -> Tick {
        let body = match avatar_contact(frame, words, &self.settings.player_label) {
            Ok(v) => v,
            Err(e) => return self.retry_observation(&e),
        };
        let included = grid.included().region(grid.geometry).unwrap();
        let mut excluded: Vec<_> = exclusions(
            frame,
            words,
            &self.settings.excluded,
            &self.settings.player_label,
        )
        .into_iter()
        .filter_map(|r| r.region(grid.geometry))
        .collect();
        // A vertical body is not a ground-plane feature. The observed body in
        // the second frame is added to BOTH masks before matching the pair.
        if let Some(mask) = body.bounds.region(grid.geometry) {
            excluded.push(mask);
        }
        let reference = match Snapshot::capture(frame, included, &excluded) {
            Ok(v) => v,
            Err(e) => return self.retry_observation(&e),
        };
        let Some(anchor) = foot_anchor(body, included, grid.geometry) else {
            return self.retry_observation("현재 발을 둘러싼 바닥 정합 영역이 충분하지 않습니다.");
        };
        self.survey = Some(Survey {
            reference,
            grid,
            goal,
            included,
            excluded,
            body,
            anchor,
        });
        self.emit(
            Command::Gesture(Gesture::Drag {
                from: Point(0.80, 0.4),
                to: Point(0.80, 0.34),
                ms: 200,
            }),
            After::Survey,
            200,
            "surveying",
            "이동하기 전에 작은 카메라 각도 변화로 양쪽 발판과 장애물 가장자리를 다시 관측합니다.",
        )
    }

    fn survey_tick(&mut self, frame: &Frame<'_>, words: &[Word], now: f64) -> Tick {
        let Some(mut survey) = self.survey.take() else {
            return self.stop("장애물의 첫 관측 프레임이 없습니다.");
        };
        self.phase = Phase::Locate;
        self.survey_failed = true;
        let contact = match avatar_contact(frame, words, &self.settings.player_label) {
            Ok(v) => v,
            Err(e) => return self.retry_observation(&e),
        };
        let mut grid = match Grid::observe(
            frame,
            contact,
            &exclusions(
                frame,
                words,
                &self.settings.excluded,
                &self.settings.player_label,
            ),
        ) {
            Ok(v) => v,
            Err(e) => return self.retry_observation(&e),
        };
        let body_region = contact.bounds.region(grid.geometry).unwrap();
        if let Err(error) = survey.reference.exclude_region(body_region) {
            return self.retry_observation(&error);
        }
        if !survey.excluded.contains(&body_region) {
            survey.excluded.push(body_region);
        }
        let current = match Snapshot::capture(frame, survey.included, &survey.excluded) {
            Ok(v) => v,
            Err(e) => return self.retry_observation(&e),
        };
        let limit = frame
            .width
            .max(frame.height)
            .div_ceil(512)
            .max(1)
            .saturating_mul(48)
            .min(512);
        let registration =
            match scene::register_anchored(&survey.reference, &current, limit, survey.anchor) {
                Ok(v) => v,
                Err(e) => return self.retry_observation(&e),
            };
        let actual_foot = [
            contact.point.0 * frame.width as f64,
            contact.point.1 * frame.height as f64,
        ];
        if distance(
            registration.map_point(survey.grid.pixel(survey.grid.start)),
            actual_foot,
        ) > registration.error_pixels + contact.error + survey.grid.step as f64
        {
            return self.retry_observation("카메라 관측 사이 자기 발의 지면 위치가 바뀌었습니다. 장애물 관측을 다시 시작합니다.");
        }
        grid.confirm_floor(&mut survey.grid, &survey.reference, &current, &registration);
        let goal_before = [
            survey.goal.0 * frame.width as f64,
            survey.goal.1 * frame.height as f64,
        ];
        if !registration.supports(goal_before)
            || (distance(registration.map_point(goal_before), goal_before)
                < registration.error_pixels * 2. + 2.
                && registration.rotation_radians.abs() < 0.015
                && registration
                    .scale
                    .into_iter()
                    .all(|s| (s - 1.).abs() < 0.02))
        {
            return self.retry_observation("두 카메라 관측의 시차가 작거나 목표 발판의 지면 정합이 부족합니다. 장애물을 추측해 넘지 않습니다.");
        }
        let Some(model) = self.measured_jump.as_ref() else {
            return self.stop("실측한 점프 보정이 없습니다.");
        };
        let model_current = match Snapshot::capture(frame, model.included, &model.excluded) {
            Ok(v) => v,
            Err(e) => return self.retry_observation(&e),
        };
        let Some(model_anchor) = foot_anchor(model.body, model.included, grid.geometry) else {
            return self.retry_observation("점프 실측 바닥의 참조 영역이 부족합니다.");
        };
        let mapping =
            match scene::register_anchored(&model.reference, &model_current, limit, model_anchor) {
                Ok(v) => v,
                Err(e) => {
                    self.measured_jump = None;
                    return self.retry_observation(&e);
                }
            };
        let mut measured = model.calibration.clone();
        for sample in &mut measured.samples {
            sample.endpoint = mapping.map_vector(sample.endpoint);
            for point in &mut sample.flight {
                point.ground = mapping.map_vector(point.ground);
            }
        }
        // Ground affine scale does NOT measure vertical projection. Use the
        // same character's observed standing head-to-sole image extent instead.
        let head = |c: Contact| {
            [
                (c.bounds.0 + c.bounds.2) * frame.width as f64 / 2.,
                c.bounds.1 * frame.height as f64,
            ]
        };
        let actual_head = head(contact);
        let old_head = head(survey.body);
        if !registration.supports(old_head) {
            return self
                .retry_observation("몸 상단의 바닥 대비 시차를 지지하는 지면 특징이 부족합니다.");
        }
        let predicted_head = registration.map_point(old_head);
        let parallax_vector = [
            actual_head[0] - predicted_head[0],
            actual_head[1] - predicted_head[1],
        ];
        let head_error = registration.error_pixels + survey.body.error + contact.error;
        let parallax = (distance(actual_head, predicted_head)
            - registration.error_pixels
            - survey.body.error
            - contact.error)
            .max(0.);
        let body_height = (contact.point.1 - contact.bounds.1) * frame.height as f64;
        let model_height = (model.body.point.1 - model.body.bounds.1) * frame.height as f64;
        if body_height <= contact.error * 3. || model_height <= model.body.error * 3. {
            return self.retry_observation(
                "캐릭터의 실제 세로 크기가 오차에 비해 작아 점프 높이와 시차를 비교할 수 없습니다.",
            );
        }
        let scale = body_height / model_height;
        measured.shadow_per_height = mapping
            .map_vector(measured.shadow_per_height)
            .map(|v| v / scale);
        measured.supported_foot_minus_shadow =
            mapping.map_vector(measured.supported_foot_minus_shadow);
        for sample in &mut measured.samples {
            for point in &mut sample.flight {
                point.height_pixels *= scale;
                point.error_pixels = point.error_pixels * scale + mapping.error_pixels;
            }
        }
        let model_head = head(model.body);
        let old_axis = [
            model_head[0] - model.body.point.0 * frame.width as f64,
            -model_height,
        ];
        let old_norm = old_axis[0].hypot(old_axis[1]);
        if ((old_axis[0] * measured.lift[1] - old_axis[1] * measured.lift[0]) / old_norm).abs()
            * measured.apex_pixels
            > measured.error_pixels + model.body.error
        {
            return self.retry_observation(
                "점프 실측 방향과 같은 캐릭터의 세로 윤곽이 맞지 않아 시점을 재보정해야 합니다.",
            );
        }
        let lean = actual_head[0] - contact.point.0 * frame.width as f64;
        let lift = [lean, -body_height];
        let norm = lift[0].hypot(lift[1]);
        measured.lift = [lift[0] / norm, lift[1] / norm];
        measured.apex_pixels *= scale;
        // Contact.error already bounds the two silhouette raster boundaries.
        measured.error_pixels = measured.error_pixels * scale
            + mapping.error_pixels
            + measured.apex_pixels
                * (contact.error / body_height
                    + model.body.error / model_height
                    + lean.abs() / body_height);
        if let (jump::Grounding::Supported, Some(shadow)) = grounding(frame, contact) {
            measured.supported_foot_minus_shadow = [
                actual_foot[0] - shadow.0 * frame.width as f64,
                actual_foot[1] - shadow.1 * frame.height as f64,
            ];
        }
        let raised = observe_landing_plane(
            &survey.reference,
            &current,
            &registration,
            &survey.grid,
            &grid,
            goal_before,
            contact,
            measured.lift,
            parallax_vector,
            head_error,
            body_height,
            limit,
        );
        let goal = grid.normalized(
            raised
                .as_ref()
                .map_or_else(|| registration.map_point(goal_before), |p| p.current_point),
        );
        let Some(candidate) = grid.jump_candidate(
            &survey.grid,
            &registration,
            &measured,
            contact,
            goal,
            parallax / body_height,
            raised.as_ref(),
        ) else {
            if let Some(mut walk) = self.walk {
                walk.right = registration.map_vector(walk.right);
                walk.up = registration.map_vector(walk.up);
                let ground_goal = raised.as_ref().map_or(
                    [goal.0 * frame.width as f64, goal.1 * frame.height as f64],
                    |p| {
                        [
                            p.current_point[0] - measured.lift[0] * p.height,
                            p.current_point[1] - measured.lift[1] * p.height,
                        ]
                    },
                );
                if let Some(direction) = control_direction(
                    walk,
                    [
                        ground_goal[0] - actual_foot[0],
                        ground_goal[1] - actual_foot[1],
                    ],
                ) {
                    if measured
                        .samples
                        .iter()
                        .all(|s| distance(s.direction, direction) > 0.05)
                    {
                        self.required_direction = Some(direction);
                        self.measured_jump = None;
                        self.walk = Some(walk);
                        self.survey_failed = false;
                        return self.retry_observation("필요한 방향의 실제 점프 표본이 없어 보이는 평지에서 그 방향을 직접 측정합니다.");
                    }
                }
            }
            let reach = measured
                .samples
                .iter()
                .map(|s| s.endpoint[0].hypot(s.endpoint[1]))
                .fold(0., f64::max);
            if distance(
                actual_foot,
                [goal.0 * frame.width as f64, goal.1 * frame.height as f64],
            ) > reach * 1.2
            {
                if let Some(p) = grid.calibration_goal(contact, goal) {
                    if distance(
                        p,
                        [goal.0 * frame.width as f64, goal.1 * frame.height as f64],
                    ) + contact.radius * 2.
                        < distance(
                            actual_foot,
                            [goal.0 * frame.width as f64, goal.1 * frame.height as f64],
                        )
                    {
                        if let Some(route) = grid.route(
                            contact.point,
                            grid.normalized(p),
                            contact.radius + contact.error + 3.,
                        ) {
                            if route.len() > 1 {
                                self.jump_bootstrap = true;
                                return self.start_segment(
                                    frame, words, now, contact, &grid, route[1], true, None,
                                );
                            }
                        }
                    }
                }
            }
            return self.retry_observation("두 시점에서 동일 평면의 이륙·착지면과 장애물 양쪽 경계를 함께 입증하지 못했습니다.");
        };
        self.survey_failed = false;
        self.state.avatar = Some(contact.point);
        self.state.route = vec![
            grid.normalized(candidate.takeoff),
            grid.normalized(candidate.landing),
        ];
        self.state.goal = Some(Rect(
            candidate
                .landing_support
                .iter()
                .map(|p| p[0])
                .fold(f64::INFINITY, f64::min)
                / frame.width as f64,
            candidate
                .landing_support
                .iter()
                .map(|p| p[1])
                .fold(f64::INFINITY, f64::min)
                / frame.height as f64,
            candidate
                .landing_support
                .iter()
                .map(|p| p[0])
                .fold(f64::NEG_INFINITY, f64::max)
                / frame.width as f64,
            candidate
                .landing_support
                .iter()
                .map(|p| p[1])
                .fold(f64::NEG_INFINITY, f64::max)
                / frame.height as f64,
        ));
        match measured.plan(
            &candidate,
            [
                contact.point.0 * frame.width as f64,
                contact.point.1 * frame.height as f64,
            ],
            contact.error,
        ) {
            jump::Plan::Approach(p) => {
                let Some(route) = grid.route(
                    contact.point,
                    grid.normalized(p),
                    contact.radius + contact.error + 3.,
                ) else {
                    return self
                        .retry_observation("관측한 점프 이륙점까지 안전한 보행 경로가 없습니다.");
                };
                if route.len() < 2 {
                    return self.retry_observation("점프 이륙점 접근 경로를 다시 확인합니다.");
                }
                self.jump_approach = true;
                self.start_segment(frame, words, now, contact, &grid, route[1], true, None)
            }
            jump::Plan::Ready(_) => {
                let (ground, _) = grounding(frame, contact);
                if ground != jump::Grounding::Supported {
                    return self.retry_observation(
                        "점프 직전의 실제 발·그림자 접지를 확인하지 못했습니다.",
                    );
                }
                let (Some(controls), Some(button), Some(mut walk)) =
                    (self.settings.controls, self.settings.jump, self.walk)
                else {
                    return self.stop("실측한 이동·점프 조작 위치가 없습니다.");
                };
                walk.right = registration.map_vector(walk.right);
                walk.up = registration.map_vector(walk.up);
                let origin = jump::JumpObservation {
                    foot: [
                        contact.point.0 * frame.width as f64,
                        contact.point.1 * frame.height as f64,
                    ],
                    shadow: grounding(frame, contact)
                        .1
                        .map(|p| [p.0 * frame.width as f64, p.1 * frame.height as f64]),
                    error_pixels: contact.error,
                    observed_at: self.last_frame,
                    ground,
                };
                let runner = match jump::Jumper::crossing(
                    controls,
                    button,
                    grid.geometry,
                    walk,
                    origin,
                    &candidate,
                    measured,
                    now,
                ) {
                    Ok(v) => v,
                    Err(e) => return self.retry_observation(&e),
                };
                self.jump_session = Some(JumpSession {
                    runner,
                    reference: current,
                    included: survey.included,
                    excluded: survey.excluded,
                    unknown: 0,
                    body: contact,
                    landing: raised.as_ref().map(|plane| LandingReference {
                        region: plane.reference_region,
                    }),
                });
                self.phase = Phase::Jump;
                self.state.phase = "jumpPlanned";
                self.state.message="두 시점의 지면·가장자리와 실측 점프 범위가 일치합니다. 접촉 전에 점프하고 실제 착지를 확인합니다.".into();
                observe()
            }
            jump::Plan::NotObservable(reason) => self.retry_observation(&reason),
        }
    }

    fn goal(
        &self,
        frame: &Frame<'_>,
        words: &[Word],
        grid: &Grid,
        contact: Contact,
    ) -> Option<(Point, String, Option<Rect>)> {
        let labels = match self.state.destination {
            Destination::Base => &[
                "spawn",
                "spawnpoint",
                "스폰",
                "리스폰",
                "mostrolls",
                "가장많이뽑은사람",
            ] as &[&str],
            Destination::Coast => &["fishing", "낚시"] as &[&str],
            Destination::Sell => &["captainflarg", "flarg", "물고기판매", "sellfish"],
            Destination::Craft if self.craft_stage == 0 => &["questboard", "퀘스트보드"],
            Destination::Craft if self.craft_stage == 1 => &[
                "stellaportal",
                "stellasteleporter",
                "스텔라포탈",
                "stella'sportal",
            ],
            Destination::Craft => &["stella", "스텔라"],
            Destination::Match => &["memorymatch", "메모리매치"],
        };
        for word in matching_words(words, labels, true) {
            if word.rect.1 < 0.82 {
                if let Some(p) = grid.near_label(word.rect, contact.radius * 2.) {
                    return Some((p, word.text.clone(), Some(expand(word.rect, 0.04, 0.06))));
                }
            }
        }
        if self.state.destination == Destination::Coast {
            if let Some(p) = grid.coast(contact.point, &self.coast_rejected, contact.radius * 2.) {
                return Some((p, "observed shoreline candidate".into(), None));
            }
        }
        for anchor in self.settings.memory.anchors.iter().filter(|a| {
            a.destination == self.state.destination
                && (self.state.destination != Destination::Craft
                    || labels
                        .iter()
                        .any(|label| normalized(&a.label).contains(&normalized(label))))
        }) {
            if let Some((rect, p)) = recognize_anchor(frame, anchor) {
                if valid(p) && grid.walkable(p, contact.radius + contact.error) {
                    return Some((p, anchor.label.clone(), Some(rect)));
                }
            }
        }
        None
    }
    fn interact(&mut self, _frame: &Frame<'_>, words: &[Word], _now: f64) -> Tick {
        if self.state.destination == Destination::Base {
            if !has(
                words,
                &["spawn", "스폰", "리스폰", "mostrolls", "가장많이뽑은사람"],
            ) {
                return self.retry_observation("Spawn 기준점 표식을 다시 확인합니다.");
            }
            if let Some(anchor) = self.pending_anchor.take() {
                self.remember(anchor);
            }
            self.phase = Phase::Arrived;
            self.state.phase = "arrived";
            self.state.message = "관측한 Spawn 기준점 접근 영역에 도착했습니다.".into();
            return Tick::Arrived;
        }
        if self.state.destination == Destination::Coast {
            if let Some(p) = self.last_goal.take() {
                self.coast_rejected.push(p);
            }
            self.phase = Phase::Locate;
            self.navigator = None;
            return self.retry_observation("해안 접근 후보에서 실제 낚시 화면이 나타나지 않아 다음 관측 가능한 해안을 확인합니다.");
        }
        if self.state.destination == Destination::Craft && self.craft_stage == 0 {
            if has(words, &["dailyquests", "questboard", "퀘스트"]) {
                if let Some(w) = find(words, &["exit", "close", "닫기"]) {
                    self.craft_stage = 1;
                    self.interaction_attempts = 0;
                    self.phase = Phase::Locate;
                    return self.emit(
                        tap(center(w.rect)),
                        After::QuestExit,
                        40,
                        "portal",
                        "퀘스트 창의 실제 닫기 버튼을 확인하고 스텔라 포탈을 찾습니다.",
                    );
                }
            }
        }
        if self.interaction_attempts >= 4 {
            return self.stop("목적지 접근 뒤 실제 상호작용 화면이 열리지 않았습니다.");
        }
        if let Some(prompt) = find(
            words,
            &[
                "talk",
                "interact",
                "open",
                "대화",
                "상호작용",
                "열기",
                "e",
                "f",
            ],
        ) {
            let p = center(prompt.rect);
            if p.0 > 0.18 && p.0 < 0.90 && p.1 > 0.15 && p.1 < 0.82 {
                self.interaction_attempts += 1;
                if self.state.destination == Destination::Craft && self.craft_stage == 1 {
                    self.craft_stage = 2;
                    self.interaction_attempts = 0;
                    self.phase = Phase::Locate;
                    return self.emit(
                        tap(p),
                        After::Portal,
                        40,
                        "portal",
                        "관측한 스텔라 포탈을 사용하고 도착 화면을 다시 확인합니다.",
                    );
                }
                return self.emit(
                    tap(p),
                    After::Interact,
                    40,
                    "interacting",
                    "목적지의 실제 상호작용 표시를 누르고 기능 화면을 확인합니다.",
                );
            }
        }
        // Desktop keys are explicit commands; phone executors may only perform
        // them when their targeted transport supports them, never host fallback.
        self.interaction_attempts += 1;
        if self.state.destination == Destination::Craft && self.craft_stage == 1 {
            self.craft_stage = 2;
            self.interaction_attempts = 0;
            return self.emit(
                Command::Key { vk: 0x45, ms: 40 },
                After::Portal,
                40,
                "portal",
                "관측한 스텔라 포탈을 사용하고 도착 화면의 공방 표식을 다시 확인합니다.",
            );
        }
        self.emit(
            Command::Key {
                vk: if self.state.destination == Destination::Craft && self.craft_stage >= 2 {
                    0x46
                } else {
                    0x45
                },
                ms: 40,
            },
            After::Interact,
            40,
            "interacting",
            "접근한 목적지의 기능 화면을 실제로 확인합니다.",
        )
    }
    fn remember(&mut self, anchor: Anchor) {
        self.settings.memory.anchors.retain(|a| {
            a.destination != anchor.destination || normalized(&a.label) != normalized(&anchor.label)
        });
        if self.settings.memory.anchors.len() == 16 {
            self.settings.memory.anchors.remove(0);
        }
        self.settings.memory.anchors.push(anchor);
    }
}

fn observe() -> Tick {
    Tick::Observe {
        region: None,
        ocr: true,
    }
}
fn tap(at: Point) -> Command {
    Command::Gesture(Gesture::Tap { at, ms: 40 })
}
fn valid(p: Point) -> bool {
    p.0.is_finite() && p.1.is_finite() && (0.0..1.0).contains(&p.0) && (0.0..1.0).contains(&p.1)
}
fn rect_valid(r: Rect) -> bool {
    [r.0, r.1, r.2, r.3]
        .iter()
        .all(|v| v.is_finite() && (0.0..=1.0).contains(v))
        && r.0 < r.2
        && r.1 < r.3
}
fn center(r: Rect) -> Point {
    Point((r.0 + r.2) / 2., (r.1 + r.3) / 2.)
}
fn expand(r: Rect, x: f64, y: f64) -> Rect {
    Rect(
        (r.0 - x).max(0.),
        (r.1 - y).max(0.),
        (r.2 + x).min(1.),
        (r.3 + y).min(1.),
    )
}
fn inside(r: Rect, p: Point) -> bool {
    p.0 >= r.0 && p.0 <= r.2 && p.1 >= r.1 && p.1 <= r.3
}
fn normalized(text: &str) -> String {
    text.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}
/// OCR engines may return either lines or individual words. Join only adjacent
/// words on the same visible line, and only while they can form a known label.
/// Unrelated rows, gaps and intervening words never become a landmark/name.
fn matching_words(words: &[Word], labels: &[&str], partial: bool) -> Vec<Word> {
    let labels: Vec<_> = labels
        .iter()
        .map(|s| normalized(s))
        .filter(|s| !s.is_empty())
        .collect();
    let text: Vec<_> = words.iter().map(|w| normalized(&w.text)).collect();
    let mut matches = Vec::new();
    for (i, first) in words.iter().enumerate() {
        if !rect_valid(first.rect) || text[i].is_empty() {
            continue;
        }
        let mut current = first.clone();
        let mut joined = text[i].clone();
        for _ in 0..12 {
            if labels.iter().any(|label| {
                if partial {
                    joined.contains(label)
                } else {
                    joined == *label
                }
            }) {
                matches.push(current.clone());
            }
            if !labels
                .iter()
                .any(|label| label.len() > joined.len() && label.starts_with(&joined))
            {
                break;
            }
            let height = current.rect.3 - current.rect.1;
            let next = words
                .iter()
                .enumerate()
                .filter(|(j, word)| {
                    let r = word.rect;
                    let h = r.3 - r.1;
                    let overlap = current.rect.3.min(r.3) - current.rect.1.max(r.1);
                    *j != i
                        && !text[*j].is_empty()
                        && rect_valid(r)
                        && r.0 >= current.rect.2 - height * 0.15
                        && r.2 > current.rect.2
                        && r.0 - current.rect.2 <= height.max(h) * 0.9
                        && overlap >= height.max(h) * 0.65
                        && h >= height * 0.65
                        && h <= height * 1.5
                })
                .min_by(|(_, a), (_, b)| a.rect.0.total_cmp(&b.rect.0));
            let Some((index, next)) = next else { break };
            joined.push_str(&text[index]);
            current.text.push(' ');
            current.text.push_str(&next.text);
            current.rect = Rect(
                current.rect.0,
                current.rect.1.min(next.rect.1),
                next.rect.2,
                current.rect.3.max(next.rect.3),
            );
        }
    }
    matches
}
fn find(words: &[Word], labels: &[&str]) -> Option<Word> {
    matching_words(words, labels, false).into_iter().next()
}
fn has(words: &[Word], labels: &[&str]) -> bool {
    !matching_words(words, labels, true).is_empty()
}
fn task_ui(destination: Destination, frame: &Frame<'_>, words: &[Word]) -> bool {
    match destination {
        Destination::Base => false,
        Destination::Coast => WindowFinder::default()
            .find(frame, |r, ratio| {
                (1.6..=5.5).contains(&ratio)
                    && ((r.0 + r.2) / 2. - 0.5).abs() <= 0.15
                    && frame.button(r.at(0.2494, 0.8264)).is_some()
            })
            .is_some(),
        Destination::Sell => {
            has(words, &["sellfish", "물고기판매"])
                && has(words, &["sellall", "captainflarg", "판매", "fishinventory"])
        }
        Destination::Craft => {
            has(words, &["stellasworkshop", "스텔라의공방"])
                && has(words, &["item", "lantern", "search", "recipe", "검색"])
        }
        Destination::Match => {
            has(words, &["memorymatch", "메모리매치"])
                && has(
                    words,
                    &[
                        "chances",
                        "available",
                        "startmemorymatch",
                        "watchad",
                        "도전",
                        "시작",
                    ],
                )
        }
    }
}

fn exclusions(frame: &Frame<'_>, words: &[Word], saved: &[Rect], player: &str) -> Vec<Rect> {
    let mut values = saved.to_vec();
    values.extend([
        Rect(0., 0., 1., 0.08),
        Rect(0., 0.08, 0.12, 1.),
        Rect(0., 0.82, 1., 1.),
    ]);
    let names: Vec<_> = player.split('|').collect();
    let own_labels = matching_words(words, &names, false);
    for word in words {
        let s = normalized(&word.text);
        if word.rect.0 > 0.68
            && word.rect.1 < 0.65
            && has(
                &[Word {
                    text: word.text.clone(),
                    rect: word.rect,
                }],
                &["players", "player", "플레이어", "rolls"],
            )
        {
            values.push(Rect(word.rect.0, 0., 1., 0.78));
        }
        if !s.is_empty()
            && !own_labels
                .iter()
                .any(|label| inside(label.rect, center(word.rect)))
            && word.rect.1 > 0.1
            && word.rect.1 < 0.8
            && s.chars().any(|c| c.is_ascii_digit())
            && s.len() >= 5
        {
            values.push(expand(word.rect, 0.025, 0.055));
        }
    }
    values.truncate(23);
    values.retain(|r| r.region((frame.width, frame.height)).is_some());
    values
}

/// Locate a name-associated BODY silhouette and its visible two-foot base.
/// A glow, overhead word or screen center is never used as a foot observation.
fn avatar_contact(frame: &Frame<'_>, words: &[Word], player: &str) -> Result<Contact, String> {
    let names: Vec<_> = player.split('|').filter(|s| !s.trim().is_empty()).collect();
    let labels = matching_words(words, &names, false);
    let ambiguous = || {
        "자기 캐릭터의 표시 이름을 하나로 구분하지 못했습니다. 이름과 발이 가려지지 않게 해 주세요."
            .to_owned()
    };
    if labels.is_empty()
        || labels.iter().enumerate().any(|(i, a)| {
            labels[..i]
                .iter()
                .any(|b| normalized(&a.text) == normalized(&b.text))
        })
    {
        return Err(ambiguous());
    }
    let mut contacts = labels.iter().map(|word| contact_below_label(frame, word));
    let first = contacts.next().unwrap()?;
    for contact in contacts {
        let contact = contact?;
        // Display name and account name may occupy two overhead rows. They
        // identify one avatar only when both independently find the SAME feet.
        if distance(
            [
                first.point.0 * frame.width as f64,
                first.point.1 * frame.height as f64,
            ],
            [
                contact.point.0 * frame.width as f64,
                contact.point.1 * frame.height as f64,
            ],
        ) > first.error.max(contact.error)
        {
            return Err(ambiguous());
        }
    }
    Ok(first)
}

fn contact_below_label(frame: &Frame<'_>, word: &Word) -> Result<Contact, String> {
    let label = word
        .rect
        .region((frame.width, frame.height))
        .ok_or("표시 이름의 화면 영역이 올바르지 않습니다.")?;
    let pad = (label.height * 3)
        .max(frame.height / 40)
        .min(frame.height / 12);
    let x0 = label.x.saturating_sub(pad);
    let x1 = (label.x + label.width + pad).min(frame.width);
    let y0 = label.y + label.height;
    let y1 = (y0 + (label.height * 7).max(frame.height / 10)).min(frame.height - 4);
    if x1 <= x0 + 8 || y1 <= y0 + 8 {
        return Err("캐릭터의 몸과 발을 관측할 영역이 부족합니다.".into());
    }
    let step = (frame.height / 540).max(1);
    let w = (x1 - x0).div_ceil(step);
    let h = (y1 - y0).div_ceil(step);
    if w * h > 40000 {
        return Err("표시 이름 영역이 너무 커서 캐릭터를 구분하지 못했습니다.".into());
    }
    let mut surrounding = Vec::new();
    for y in (y0..y1).step_by(step * 3) {
        for x in [x0, (x1 - 1)] {
            if let Some(p) = frame.pixel(x, y) {
                surrounding.push(p)
            }
        }
    }
    let bg = median_rgb(&surrounding).ok_or("캐릭터 주변 지면 밝기를 읽지 못했습니다.")?;
    let luminance =
        |p: [u8; 3]| (u32::from(p[0]) * 77 + u32::from(p[1]) * 150 + u32::from(p[2]) * 29) / 256;
    if luminance(bg) < 18 {
        return Err("현재 화면이 너무 어두워 몸과 발의 접점을 구분할 수 없습니다.".into());
    }
    let mut mask = vec![false; w * h];
    for y in 0..h {
        for x in 0..w {
            if let Some(p) = frame.pixel(x0 + x * step, y0 + y * step) {
                // Clothing can be darker OR brighter than the observed floor.
                // The name association, bounded body shape, two separate soles
                // and visible floor below carry identity/contact evidence.
                mask[y * w + x] = color_distance(p, bg) > 25. && !shadow_of(p, bg);
            }
        }
    }
    let mut candidates = Vec::new();
    for start in 0..mask.len() {
        if !mask[start] {
            continue;
        }
        let mut queue = VecDeque::from([start]);
        mask[start] = false;
        let (mut minx, mut maxx, mut miny, mut maxy, mut count) = (w, 0, h, 0, 0usize);
        let mut pixels = Vec::new();
        while let Some(at) = queue.pop_front() {
            let (x, y) = (at % w, at / w);
            count += 1;
            pixels.push((x, y));
            minx = minx.min(x);
            maxx = maxx.max(x);
            miny = miny.min(y);
            maxy = maxy.max(y);
            for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                let (Some(xx), Some(yy)) = (x.checked_add_signed(dx), y.checked_add_signed(dy))
                else {
                    continue;
                };
                if xx < w && yy < h && mask[yy * w + xx] {
                    mask[yy * w + xx] = false;
                    queue.push_back(yy * w + xx);
                }
            }
        }
        let (bw, bh) = ((maxx - minx + 1) * step, (maxy - miny + 1) * step);
        if bw < frame.height / 100
            || bw > frame.height / 10
            || bh < frame.height / 45
            || bh > frame.height / 5
            || count * step * step < bw * bh / 6
            || maxy + 1 >= h
        {
            continue;
        }
        let mut bottom = vec![false; maxx - minx + 1];
        for (x, y) in &pixels {
            if *y + 2 >= maxy {
                bottom[*x - minx] = true;
            }
        }
        let mut runs = Vec::new();
        let mut begin = None;
        for (x, filled) in bottom.iter().copied().chain([false]).enumerate() {
            if filled && begin.is_none() {
                begin = Some(x)
            } else if !filled {
                if let Some(a) = begin.take() {
                    if x - a >= 2 {
                        runs.push((a, x));
                    }
                }
            }
        }
        if runs.len() != 2 || runs[1].0 - runs[0].1 > bw / step / 2 {
            continue;
        }
        let foot_x = x0 + (minx + (runs[0].0 + runs[0].1 + runs[1].0 + runs[1].1) / 4) * step;
        let foot_y = y0 + (maxy + 1) * step;
        let below = frame
            .pixel(foot_x, foot_y + step * 2)
            .ok_or("발 아래 지면 관측이 부족합니다.")?;
        if color_distance(below, bg) > 35. && !shadow_of(below, bg) {
            continue;
        }
        let point = Point(
            foot_x as f64 / frame.width as f64,
            foot_y as f64 / frame.height as f64,
        );
        let bounds = Rect(
            (x0 + minx * step) as f64 / frame.width as f64,
            (y0 + miny * step) as f64 / frame.height as f64,
            (x0 + (maxx + 1) * step) as f64 / frame.width as f64,
            (y0 + (maxy + 1) * step + 2) as f64 / frame.height as f64,
        );
        candidates.push(Contact {
            point,
            bounds,
            radius: (bw as f64 * 0.55).max(4.),
            error: (step * 3) as f64,
        });
    }
    if candidates.len() != 1 {
        return Err("자기 캐릭터의 두 발과 지면 접점이 가려져 있습니다. 오라·다른 플레이어와 구분되는 새 화면이 필요합니다.".into());
    }
    Ok(candidates[0])
}

fn detect_controls(frame: &Frame<'_>) -> Option<Controls> {
    // The actual mobile joystick has concentric circular edges. Search only
    // the control area, requiring a white ring on at least 12 of 16 rays.
    let (w, h) = (frame.width, frame.height);
    let step = (h / 120).max(3);
    for cy in (h * 65 / 100..h * 90 / 100).step_by(step) {
        for cx in (w * 6 / 100..w * 25 / 100).step_by(step) {
            for radius in (h / 35..h / 12).step_by(step) {
                let mut hits = 0;
                for n in 0..16 {
                    let a = n as f64 * std::f64::consts::TAU / 16.;
                    let x = (cx as f64 + a.cos() * radius as f64).round() as usize;
                    let y = (cy as f64 + a.sin() * radius as f64).round() as usize;
                    if frame.pixel(x, y).is_some_and(|p| {
                        p.iter().copied().min().unwrap() > 155
                            && p.iter().copied().max().unwrap() - p.iter().copied().min().unwrap()
                                < 40
                    }) {
                        hits += 1
                    }
                }
                if hits >= 13 {
                    return Some(Controls {
                        stick_center: Point(cx as f64 / w as f64, cy as f64 / h as f64),
                        stick_edge: Point((cx + radius) as f64 / w as f64, cy as f64 / h as f64),
                    });
                }
            }
        }
    }
    None
}

fn median_rgb(values: &[[u8; 3]]) -> Option<[u8; 3]> {
    if values.len() < 3 {
        return None;
    }
    let mut out = [0; 3];
    for c in 0..3 {
        let mut v: Vec<_> = values.iter().map(|p| p[c]).collect();
        let middle = v.len() / 2;
        out[c] = *v.select_nth_unstable(middle).1;
    }
    Some(out)
}
fn color_distance(a: [u8; 3], b: [u8; 3]) -> f64 {
    (0..3)
        .map(|i| (f64::from(a[i]) - f64::from(b[i])).powi(2))
        .sum::<f64>()
        .sqrt()
}

/// Local multiplicative shading evidence. A dark outfit or a colored aura is
/// not a cast shadow merely because its luminance is low.
fn shadow_of(pixel: [u8; 3], floor: [u8; 3]) -> bool {
    let norm: f64 = floor.iter().map(|v| f64::from(*v).powi(2)).sum();
    if norm < 1000. {
        return false;
    }
    let ratio = (0..3)
        .map(|i| f64::from(pixel[i]) * f64::from(floor[i]))
        .sum::<f64>()
        / norm;
    (0.40..=0.87).contains(&ratio)
        && (0..3)
            .map(|i| (f64::from(pixel[i]) - f64::from(floor[i]) * ratio).powi(2))
            .sum::<f64>()
            .sqrt()
            < 15.
}

/// Visible cast-shadow support, independent of elapsed time or the command
/// being sent. Missing/overlapping/colored shadows remain unknown.
fn grounding(frame: &Frame<'_>, contact: Contact) -> (jump::Grounding, Option<Point>) {
    use jump::Grounding;
    let g = (frame.width, frame.height);
    let Some(body) = contact.bounds.region(g) else {
        return (Grounding::Unknown, None);
    };
    let foot = [
        (contact.point.0 * g.0 as f64) as usize,
        (contact.point.1 * g.1 as f64) as usize,
    ];
    let pad = body.width * 2;
    let x0 = foot[0].saturating_sub(pad);
    let x1 = (foot[0] + pad).min(g.0 - 1);
    // A real jump can lift the visible soles by more than one body length.
    // Search actual pixels over a bounded visible column; ambiguity still
    // yields Unknown instead of attaching an arbitrary dark pixel to the feet.
    let vertical = body.height.saturating_mul(4).min(g.1);
    let y0 = foot[1].saturating_sub(vertical);
    let y1 = (foot[1] + vertical).min(g.1 - 1);
    let step = g.1.div_ceil(540).max(1);
    let (w, h) = ((x1 - x0).div_ceil(step), (y1 - y0).div_ceil(step));
    if w == 0 || h == 0 || w * h > 100000 {
        return (Grounding::Unknown, None);
    }
    let mut ring = Vec::new();
    for y in (y0..y1).step_by(step * 3) {
        for x in [x0, x1] {
            if let Some(p) = frame.pixel(x, y) {
                ring.push(p);
            }
        }
    }
    let Some(floor) = median_rgb(&ring) else {
        return (Grounding::Unknown, None);
    };
    let mut mask = vec![false; w * h];
    for y in 0..h {
        for x in 0..w {
            let (px, py) = (x0 + x * step, y0 + y * step);
            let p = Point(px as f64 / g.0 as f64, py as f64 / g.1 as f64);
            if !inside(contact.bounds, p) {
                mask[y * w + x] = frame.pixel(px, py).is_some_and(|rgb| shadow_of(rgb, floor));
            }
        }
    }
    let mut candidates = Vec::new();
    for start in 0..mask.len() {
        if !mask[start] {
            continue;
        }
        mask[start] = false;
        let mut q = VecDeque::from([start]);
        let (mut a, mut b, mut c, mut d, mut count) = (w, 0, h, 0, 0usize);
        while let Some(at) = q.pop_front() {
            let (x, y) = (at % w, at / w);
            a = a.min(x);
            b = b.max(x);
            c = c.min(y);
            d = d.max(y);
            count += 1;
            for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                if let (Some(xx), Some(yy)) = (x.checked_add_signed(dx), y.checked_add_signed(dy)) {
                    if xx < w && yy < h && mask[yy * w + xx] {
                        mask[yy * w + xx] = false;
                        q.push_back(yy * w + xx);
                    }
                }
            }
        }
        let (bw, bh) = ((b - a + 1) * step, (d - c + 1) * step);
        if bw < body.width / 2
            || bw > body.width * 3
            || bh < step * 2
            || bh > body.height * 2 / 3
            || bw < bh * 3 / 2
            || bw > bh * 8
            || count * step * step < bw * bh / 3
        {
            continue;
        }
        let cx = (x0 + (a + b) * step / 2) as f64;
        let cy = (y0 + (c + d) * step / 2) as f64;
        if (cx - foot[0] as f64).abs() > body.width as f64 {
            continue;
        }
        // An isolated shaded patch must be enclosed by visible floor. Require
        // both sides and the row below, not a single convenient dark pixel.
        let margin = step * 3;
        let probes = [
            (x0 + a * step, y0 + c * step),
            (x0 + (b + 1) * step, y0 + c * step),
            (x0 + a * step, (y0 + (d + 1) * step + margin).min(g.1 - 1)),
            (
                (x0 + (b + 1) * step).min(g.0 - 1),
                (y0 + (d + 1) * step + margin).min(g.1 - 1),
            ),
        ];
        if probes
            .into_iter()
            .filter(|&(x, y)| {
                frame
                    .pixel(x, y)
                    .is_some_and(|p| color_distance(p, floor) < 25.)
            })
            .count()
            < 3
        {
            continue;
        }
        candidates.push((cx, cy, bh as f64 / 2.));
    }
    if candidates.len() != 1 {
        return (Grounding::Unknown, None);
    }
    let (x, y, ry) = candidates[0];
    let separation = (x - foot[0] as f64).hypot(y - foot[1] as f64);
    let ground = if separation <= ry + contact.error + step as f64 * 2. {
        Grounding::Supported
    } else if separation > ry + contact.error * 2. + step as f64 * 2. {
        Grounding::Airborne
    } else {
        Grounding::Unknown
    };
    (ground, Some(Point(x / g.0 as f64, y / g.1 as f64)))
}

struct Grid {
    w: usize,
    h: usize,
    step: usize,
    geometry: (usize, usize),
    land: Vec<bool>,
    open: Vec<bool>,
    visible: Vec<bool>,
    plane: Vec<bool>,
    plausible: Vec<bool>,
    colors: Vec<[u8; 3]>,
    floor_color: [u8; 3],
    body_mask: Rect,
    start: usize,
}
#[derive(Clone, Copy)]
struct Node {
    cost: f64,
    index: usize,
}
impl PartialEq for Node {
    fn eq(&self, o: &Self) -> bool {
        self.index == o.index && self.cost == o.cost
    }
}
impl Eq for Node {}
impl Ord for Node {
    fn cmp(&self, o: &Self) -> Ordering {
        o.cost
            .total_cmp(&self.cost)
            .then_with(|| self.index.cmp(&o.index))
    }
}
impl PartialOrd for Node {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}
impl Grid {
    fn observe(frame: &Frame<'_>, contact: Contact, excluded: &[Rect]) -> Result<Self, String> {
        let geometry = (frame.width, frame.height);
        let self_mask = expand(contact.bounds, 0.004, 0.004);
        let step = geometry.0.max(geometry.1).div_ceil(320).max(3);
        let (w, h) = (geometry.0.div_ceil(step), geometry.1.div_ceil(step));
        let point = |x: usize, y: usize| {
            Point(
                ((x * step + step / 2).min(geometry.0 - 1)) as f64 / geometry.0 as f64,
                ((y * step + step / 2).min(geometry.1 - 1)) as f64 / geometry.1 as f64,
            )
        };
        let (mut colors, mut texture, mut visible) =
            (vec![[0; 3]; w * h], vec![0f64; w * h], vec![false; w * h]);
        for y in 0..h {
            for x in 0..w {
                let p = point(x, y);
                let at = y * w + x;
                if !inside(Rect(0.12, 0.08, 0.98, 0.82), p)
                    || excluded.iter().any(|r| inside(*r, p))
                    || inside(self_mask, p)
                {
                    continue;
                }
                let (px, py) = (
                    (p.0 * geometry.0 as f64) as usize,
                    (p.1 * geometry.1 as f64) as usize,
                );
                let mut values = Vec::new();
                for dy in [-2isize, 0, 2] {
                    for dx in [-2isize, 0, 2] {
                        if let (Some(xx), Some(yy)) =
                            (px.checked_add_signed(dx), py.checked_add_signed(dy))
                        {
                            if let Some(rgb) = frame.pixel(xx, yy) {
                                values.push(rgb)
                            }
                        }
                    }
                }
                if let Some(rgb) = median_rgb(&values) {
                    colors[at] = rgb;
                    texture[at] = values.iter().map(|p| color_distance(*p, rgb)).sum::<f64>()
                        / values.len() as f64;
                    visible[at] = true;
                }
            }
        }
        let mut seeds = Vec::new();
        let (px, py) = (
            contact.point.0 * geometry.0 as f64,
            contact.point.1 * geometry.1 as f64,
        );
        for n in 0..16 {
            let a = n as f64 * std::f64::consts::TAU / 16.;
            let xx = (px + a.cos() * contact.radius * 1.6).round();
            let yy = (py + a.sin() * contact.radius * 1.6).round();
            if xx >= 0. && yy >= 0. {
                let (x, y) = (xx as usize / step, yy as usize / step);
                if x < w && y < h && visible[y * w + x] {
                    seeds.push(y * w + x)
                }
            }
        }
        let models: Vec<_> = seeds.iter().map(|&i| colors[i]).collect();
        let model = median_rgb(&models).ok_or("발 주변 지면을 관측하지 못했습니다.")?;
        if model.iter().copied().max().unwrap() < 22 {
            return Err("지면이 너무 어두워 발판과 경계를 확인할 수 없습니다.".into());
        }
        let ground_texture = seeds.iter().map(|&i| texture[i]).sum::<f64>() / seeds.len() as f64;
        let compatible = |at: usize| {
            visible[at]
                && color_distance(colors[at], model) <= 38.
                && texture[at] <= (ground_texture * 2. + 10.).min(38.)
                && (ground_texture < 3. || texture[at] >= ground_texture * 0.2)
        };
        let start_y = (py as usize / step).min(h - 1);
        let start_x = (px as usize / step).min(w - 1);
        let start = start_y * w + start_x;
        let surface: Vec<_> = (0..w * h).map(&compatible).collect();
        let mut land = vec![false; w * h];
        let mut queue = VecDeque::new();
        for at in seeds.into_iter().filter(|&i| compatible(i)) {
            if !land[at] {
                land[at] = true;
                queue.push_back(at)
            }
        }
        if queue.len() < 3 {
            return Err("발 주변에서 같은 지면의 관측을 충분히 구분하지 못했습니다.".into());
        }
        while let Some(at) = queue.pop_front() {
            let (x, y) = (at % w, at / w);
            for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                let (Some(xx), Some(yy)) = (x.checked_add_signed(dx), y.checked_add_signed(dy))
                else {
                    continue;
                };
                if xx < w && yy < h {
                    let to = yy * w + xx;
                    if !land[to] && compatible(to) && color_distance(colors[at], colors[to]) < 24. {
                        land[to] = true;
                        queue.push_back(to);
                    }
                }
            }
        }
        // Interpolate only the current body's occlusion, bracketed by observed
        // matching floor on BOTH sides in this same row. Never use these cells
        // as ground seeds or infer an unseen obstacle's height from them.
        if let Some(body) = self_mask.region(geometry) {
            let (left, right) = (
                body.x.saturating_sub(step * 2) / step,
                ((body.x + body.width + step * 2) / step).min(w - 1),
            );
            for y in body.y / step..((body.y + body.height).div_ceil(step)).min(h) {
                if compatible(y * w + left)
                    && compatible(y * w + right)
                    && land[y * w + left]
                    && land[y * w + right]
                {
                    for x in left + 1..right {
                        if inside(self_mask, point(x, y)) {
                            land[y * w + x] = true;
                        }
                    }
                }
            }
        }
        // Only the actually observed current footprint may cover its own body.
        // A contact occupies raster CELLS, not mathematical points. Include the
        // touching cell so the clearance disk can leave its own observed feet
        // without mistaking the masked body for a tiny terrain corner.
        let footprint = ((contact.radius + contact.error + 3.) / step as f64).ceil() as usize + 1;
        for y in start_y.saturating_sub(footprint)..=(start_y + footprint).min(h - 1) {
            for x in start_x.saturating_sub(footprint)..=(start_x + footprint).min(w - 1) {
                if (x as isize - start_x as isize).pow(2) + (y as isize - start_y as isize).pow(2)
                    <= (footprint * footprint) as isize
                {
                    land[y * w + x] = true;
                }
            }
        }
        let plane = surface.iter().zip(&land).map(|(a, b)| *a || *b).collect();
        let plausible = (0..w * h)
            .map(|i| {
                visible[i]
                    && texture[i] <= (ground_texture * 2. + 10.).min(38.)
                    && colors[i].into_iter().max().unwrap_or(0) >= 22
            })
            .collect();
        let open = (0..w * h)
            .map(|i| {
                visible[i]
                    && !land[i]
                    && texture[i] <= ground_texture.max(4.)
                    && color_distance(colors[i], model) > 30.
            })
            .collect();
        Ok(Self {
            w,
            h,
            step,
            geometry,
            land,
            open,
            visible,
            plane,
            plausible,
            colors,
            floor_color: model,
            body_mask: self_mask,
            start,
        })
    }
    fn normalized(&self, p: [f64; 2]) -> Point {
        Point(p[0] / self.geometry.0 as f64, p[1] / self.geometry.1 as f64)
    }
    fn confirm_floor(
        &mut self,
        before: &mut Grid,
        reference: &Snapshot,
        current: &Snapshot,
        plane: &scene::Registration,
    ) {
        for i in 0..self.land.len() {
            if self.land[i]
                || !self.visible[i]
                || !self.plausible[i]
                || color_distance(self.colors[i], self.floor_color) > 38.
            {
                continue;
            }
            let p = self.pixel(i);
            let Some(source) = plane.inverse_point(p) else {
                continue;
            };
            let old = before.index(before.normalized(source));
            if before.visible[old]
                && before.plausible[old]
                && color_distance(before.colors[old], before.floor_color) <= 38.
                && scene::verify_projection(reference, current, plane, source)
            {
                self.land[i] = true;
                self.plane[i] = true;
            }
        }
        // The reverse observations matter too: an old flood's isolated cell
        // is usable only after its actual source patch matches this frame.
        for old in 0..before.land.len() {
            if before.land[old]
                || !before.visible[old]
                || !before.plausible[old]
                || color_distance(before.colors[old], before.floor_color) > 38.
            {
                continue;
            }
            let source = before.pixel(old);
            if !plane.supports(source) {
                continue;
            }
            let current_point = plane.map_point(source);
            if !valid(self.normalized(current_point)) {
                continue;
            }
            let at = self.index(self.normalized(current_point));
            if self.visible[at]
                && self.plausible[at]
                && color_distance(self.colors[at], self.floor_color) <= 38.
                && scene::verify_projection(reference, current, plane, source)
            {
                before.land[old] = true;
                before.plane[old] = true;
            }
        }
        self.confirm_body_floor(reference, current, plane, false);
        before.confirm_body_floor(reference, current, plane, true);
    }
    fn confirm_body_floor(
        &mut self,
        reference: &Snapshot,
        current: &Snapshot,
        plane: &scene::Registration,
        source_view: bool,
    ) {
        let Some(body) = self.body_mask.region(self.geometry) else {
            return;
        };
        let left = body.x.saturating_sub(self.step * 2) / self.step;
        let right = ((body.x + body.width + self.step * 2) / self.step).min(self.w - 1);
        for y in body.y / self.step..((body.y + body.height).div_ceil(self.step)).min(self.h) {
            let verified = [y * self.w + left, y * self.w + right]
                .into_iter()
                .all(|i| {
                    if !self.visible[i] || !self.land[i] {
                        return false;
                    }
                    let pixel = self.pixel(i);
                    let source = if source_view {
                        Some(pixel)
                    } else {
                        plane.inverse_point(pixel)
                    };
                    source.is_some_and(|p| scene::verify_projection(reference, current, plane, p))
                });
            if verified {
                for x in left + 1..right {
                    let i = y * self.w + x;
                    if inside(self.body_mask, self.normalized(self.pixel(i))) {
                        self.land[i] = true;
                        self.plane[i] = true;
                    }
                }
            }
        }
    }
    fn flat_hull(&self, clearance: f64) -> Option<Vec<[f64; 2]>> {
        let max = (self.geometry.0.min(self.geometry.1) as f64 * 0.25) as usize;
        let min = (clearance * 2.5).ceil() as usize;
        for radius in (min..=max).rev().step_by(self.step) {
            if self.clear(self.start, radius as f64) {
                let p = self.pixel(self.start);
                let r = radius as f64 / std::f64::consts::SQRT_2;
                return Some(vec![
                    [p[0] - r, p[1] - r],
                    [p[0] + r, p[1] - r],
                    [p[0] + r, p[1] + r],
                    [p[0] - r, p[1] + r],
                ]);
            }
        }
        None
    }
    fn pixel(&self, at: usize) -> [f64; 2] {
        [
            ((at % self.w) * self.step + self.step / 2) as f64,
            ((at / self.w) * self.step + self.step / 2) as f64,
        ]
    }
    fn index(&self, p: Point) -> usize {
        ((p.1 * self.geometry.1 as f64) as usize / self.step).min(self.h - 1) * self.w
            + ((p.0 * self.geometry.0 as f64) as usize / self.step).min(self.w - 1)
    }
    fn included(&self) -> Rect {
        Rect(0.12, 0.08, 0.98, 0.82)
    }
    fn clear(&self, at: usize, radius: f64) -> bool {
        self.clear_mask(at, radius, &self.land)
    }
    fn clear_mask(&self, at: usize, radius: f64, mask: &[bool]) -> bool {
        let (x, y) = (at % self.w, at / self.w);
        let r = (radius / self.step as f64).ceil() as isize;
        for dy in -r..=r {
            for dx in -r..=r {
                if dx * dx + dy * dy > r * r {
                    continue;
                }
                let (Some(xx), Some(yy)) = (x.checked_add_signed(dx), y.checked_add_signed(dy))
                else {
                    return false;
                };
                if xx >= self.w || yy >= self.h || !mask[yy * self.w + xx] {
                    return false;
                }
            }
        }
        true
    }
    fn walkable(&self, p: Point, r: f64) -> bool {
        self.clear(self.index(p), r)
    }
    fn near_label(&self, rect: Rect, radius: f64) -> Option<Point> {
        let q = center(rect);
        (0..self.land.len())
            .filter(|&i| self.plausible[i] && self.clear_mask(i, radius, &self.plausible))
            .filter(|&i| {
                let p = self.normalized(self.pixel(i));
                (p.0 - q.0).abs() < 0.15 && (p.1 - q.1).abs() < 0.20
            })
            .min_by(|&a, &b| {
                distance(
                    self.pixel(a),
                    [q.0 * self.geometry.0 as f64, q.1 * self.geometry.1 as f64],
                )
                .total_cmp(&distance(
                    self.pixel(b),
                    [q.0 * self.geometry.0 as f64, q.1 * self.geometry.1 as f64],
                ))
            })
            .map(|i| self.normalized(self.pixel(i)))
    }
    fn calibration_goal(&self, contact: Contact, goal: Point) -> Option<[f64; 2]> {
        let from = [
            contact.point.0 * self.geometry.0 as f64,
            contact.point.1 * self.geometry.1 as f64,
        ];
        let target = [
            goal.0 * self.geometry.0 as f64,
            goal.1 * self.geometry.1 as f64,
        ];
        let r = contact.radius + contact.error + 3.;
        (0..self.land.len())
            .filter(|&i| self.clear(i, r * 2.8))
            .filter(|&i| {
                distance(from, self.pixel(i)) > r * 3. && self.line(from, self.pixel(i), r)
            })
            .min_by(|&a, &b| {
                (distance(self.pixel(a), target) + distance(self.pixel(a), from) * 0.5).total_cmp(
                    &(distance(self.pixel(b), target) + distance(self.pixel(b), from) * 0.5),
                )
            })
            .map(|i| self.pixel(i))
    }
    fn barrier_border(&self, from: [f64; 2], search: f64) -> Option<Vec<[f64; 2]>> {
        let start = (0..self.plane.len())
            .filter(|&i| !self.plane[i] && self.visible[i])
            .filter(|&i| distance(self.pixel(i), from) <= search)
            .min_by(|&a, &b| {
                distance(self.pixel(a), from).total_cmp(&distance(self.pixel(b), from))
            })?;
        let mut seen = vec![false; self.plane.len()];
        seen[start] = true;
        let mut queue = VecDeque::from([start]);
        let mut border = Vec::new();
        let mut count = 0;
        while let Some(at) = queue.pop_front() {
            count += 1;
            if count > 8192 {
                return None;
            }
            let (x, y) = (at % self.w, at / self.w);
            let mut edge = false;
            for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                let (Some(xx), Some(yy)) = (x.checked_add_signed(dx), y.checked_add_signed(dy))
                else {
                    return None;
                };
                if xx >= self.w || yy >= self.h {
                    return None;
                }
                let i = yy * self.w + xx;
                if !self.visible[i] && !self.plane[i] {
                    return None;
                }
                if self.plane[i] {
                    edge = true;
                } else if !seen[i] {
                    seen[i] = true;
                    queue.push_back(i);
                }
            }
            if edge {
                border.push(self.pixel(at));
            }
        }
        (border.len() >= 6).then_some(border)
    }
    fn supported_patch(
        &self,
        p: [f64; 2],
        r: f64,
        before: &Grid,
        registration: &scene::Registration,
    ) -> Option<Vec<[f64; 2]>> {
        if !self.clear_mask(self.index(self.normalized(p)), r * 1.5, &self.plane) {
            return None;
        }
        let h = r;
        let hull = vec![
            [p[0] - h, p[1] - h],
            [p[0] + h, p[1] - h],
            [p[0] + h, p[1] + h],
            [p[0] - h, p[1] + h],
        ];
        for q in &hull {
            let source = registration.inverse_point(*q)?;
            if !before.clear_mask(
                before.index(before.normalized(source)),
                r * 0.3,
                &before.plane,
            ) {
                return None;
            }
        }
        Some(hull)
    }
    fn raised_pixel(&self, p: [f64; 2], before: &Grid, raised: &RaisedPlane) -> bool {
        if !valid(self.normalized(p)) {
            return false;
        }
        let at = self.index(self.normalized(p));
        let current_model = self.colors[self.index(self.normalized(raised.current_point))];
        if !self.visible[at]
            || !self.plausible[at]
            || color_distance(self.colors[at], current_model) > 38.
        {
            return false;
        }
        let Some(source) = raised.mapping.inverse_point(p) else {
            return false;
        };
        let old = before.index(before.normalized(source));
        let old_model = before.colors[before.index(before.normalized(raised.before_point))];
        before.visible[old]
            && before.plausible[old]
            && color_distance(before.colors[old], old_model) <= 38.
    }
    fn raised_patch(
        &self,
        p: [f64; 2],
        r: f64,
        before: &Grid,
        raised: &RaisedPlane,
        base: &scene::Registration,
        lift: [f64; 2],
    ) -> Option<Vec<[f64; 2]>> {
        let radius = r / (std::f64::consts::PI / 8.).cos();
        let step = self.step as f64;
        let n = (radius / step).ceil() as i32;
        for y in -n..=n {
            for x in -n..=n {
                if x * x + y * y > n * n {
                    continue;
                }
                let q = [p[0] + x as f64 * step, p[1] + y as f64 * step];
                if !self.raised_pixel(q, before, raised)
                    || base
                        .inverse_point([
                            q[0] - lift[0] * raised.height,
                            q[1] - lift[1] * raised.height,
                        ])
                        .is_none()
                {
                    return None;
                }
            }
        }
        Some(
            (0..8)
                .map(|i| {
                    let angle = i as f64 * std::f64::consts::TAU / 8.;
                    [p[0] + radius * angle.cos(), p[1] + radius * angle.sin()]
                })
                .collect(),
        )
    }
    /// Collect a crossing only when TWO actual views support both floor patches
    /// and the entire visible barrier contour. A changed color alone cannot
    /// enter this path; UI/occlusion/insufficient parallax yields no candidate.
    fn jump_candidate(
        &self,
        before: &Grid,
        registration: &scene::Registration,
        measured: &jump::JumpCalibration,
        contact: Contact,
        goal: Point,
        vertical_parallax_per_pixel: f64,
        raised: Option<&RaisedPlane>,
    ) -> Option<jump::Candidate> {
        if self.geometry != before.geometry {
            return None;
        }
        let foot = [
            contact.point.0 * self.geometry.0 as f64,
            contact.point.1 * self.geometry.1 as f64,
        ];
        let goal = [
            goal.0 * self.geometry.0 as f64,
            goal.1 * self.geometry.1 as f64,
        ];
        let uncertainty =
            measured.error_pixels + registration.error_pixels + contact.error + self.step as f64;
        let r = contact.radius + uncertainty + 3.;
        let mut best: Option<(f64, jump::Candidate)> = None;
        for index in (0..self.land.len()).step_by(2) {
            if !self.land[index] || !self.clear(index, r * 1.5) {
                continue;
            }
            let takeoff = self.pixel(index);
            if distance(takeoff, foot) > self.geometry.0.min(self.geometry.1) as f64 * 0.4 {
                continue;
            }
            let Some(takeoff_support) = self.supported_patch(takeoff, r, before, registration)
            else {
                continue;
            };
            for sample in &measured.samples {
                let height = raised.map_or(0., |p| p.height);
                let height_error = raised.map_or(0., |p| p.height_error);
                let Some((ground, prediction_error)) =
                    sample.ground_at_height(height, height_error)
                else {
                    continue;
                };
                let endpoint = [
                    ground[0] + measured.lift[0] * height,
                    ground[1] + measured.lift[1] * height,
                ];
                let range = ground[0].hypot(ground[1]);
                if range < r * 2.5 {
                    continue;
                }
                let landing = [takeoff[0] + endpoint[0], takeoff[1] + endpoint[1]];
                if !valid(self.normalized(landing))
                    || distance(landing, goal) + self.step as f64 >= distance(takeoff, goal)
                {
                    continue;
                }
                let landing_radius = r + prediction_error + height_error;
                let landing_support = if let Some(plane) = raised {
                    self.raised_patch(
                        landing,
                        landing_radius,
                        before,
                        plane,
                        registration,
                        measured.lift,
                    )
                } else {
                    self.supported_patch(landing, landing_radius, before, registration)
                };
                let Some(landing_support) = landing_support else {
                    continue;
                };
                let samples = (range / self.step as f64).ceil() as usize;
                if let Some(plane) = raised {
                    let top = |i: usize| {
                        let t = i as f64 / samples as f64;
                        [
                            takeoff[0] + ground[0] * t + measured.lift[0] * height,
                            takeoff[1] + ground[1] * t + measured.lift[1] * height,
                        ]
                    };
                    let Some(entry) =
                        (1..samples).find(|&i| self.raised_pixel(top(i), before, plane))
                    else {
                        continue;
                    };
                    // The upper boundary is observed in both views. Its base
                    // projection uses the independently measured relative rise;
                    // a vertical ledge may have zero ground-plane thickness.
                    let t = (entry as f64 - 0.5) / samples as f64;
                    let edge = [takeoff[0] + ground[0] * t, takeoff[1] + ground[1] * t];
                    let candidate = jump::Candidate {
                        takeoff,
                        landing,
                        takeoff_support: takeoff_support.clone(),
                        landing_support,
                        same_plane: false,
                        near_edge: edge,
                        far_edge: edge,
                        rise: Some(jump::RiseEvidence {
                            base: edge,
                            top: [
                                edge[0] + measured.lift[0] * height,
                                edge[1] + measured.lift[1] * height,
                            ],
                            error_pixels: height_error + self.step as f64,
                        }),
                        landing_rise: Some(jump::LandingRise {
                            height_pixels: height,
                            error_pixels: height_error,
                            base_projection: [takeoff[0] + ground[0], takeoff[1] + ground[1]],
                            actual_floor: landing,
                        }),
                        observed_gap: false,
                        error_pixels: registration.error_pixels + self.step as f64,
                        clearance_pixels: contact.radius,
                    };
                    let plan = measured.plan(&candidate, takeoff, contact.error);
                    if matches!(plan, jump::Plan::NotObservable(_)) {
                        continue;
                    }
                    let score = distance(foot, takeoff) * 1.05 + distance(landing, goal);
                    if best.as_ref().is_none_or(|(cost, _)| score < *cost) {
                        best = Some((score, candidate));
                    }
                    continue;
                }
                let blocked: Vec<_> = (1..samples)
                    .filter(|&i| {
                        let t = i as f64 / samples as f64;
                        !self.plane[self.index(self.normalized([
                            takeoff[0] + endpoint[0] * t,
                            takeoff[1] + endpoint[1] * t,
                        ]))]
                    })
                    .collect();
                let (Some(&near), Some(&far)) = (blocked.first(), blocked.last()) else {
                    continue;
                };
                if blocked.len() != far - near + 1 {
                    continue;
                }
                let at = |i: f64| {
                    [
                        takeoff[0] + endpoint[0] * i / samples as f64,
                        takeoff[1] + endpoint[1] * i / samples as f64,
                    ]
                };
                let near_edge = at(near as f64 - 0.5);
                let far_edge = at(far as f64 + 0.5);
                let middle = at((near + far) as f64 / 2.);
                let Some(contour) = self.barrier_border(middle, self.step as f64 * 2.) else {
                    continue;
                };
                let Some(source) = registration.inverse_point(middle) else {
                    continue;
                };
                let Some(old_contour) = before
                    .barrier_border(source, registration.error_pixels + self.step as f64 * 3.)
                else {
                    continue;
                };
                let mapped: Vec<_> = old_contour
                    .iter()
                    .copied()
                    .map(|p| registration.map_point(p))
                    .collect();
                let tolerance = registration.error_pixels + (self.step + before.step) as f64;
                // A wall as tall as our safe jump limit MUST be distinguishable
                // from a floor boundary at this measured view baseline. Without
                // this resolution, a stable contour is not evidence of a gap.
                if !vertical_parallax_per_pixel.is_finite()
                    || vertical_parallax_per_pixel * (measured.apex_pixels - measured.error_pixels)
                        <= tolerance * 1.5
                {
                    continue;
                }
                let nearest = |p: [f64; 2], set: &[[f64; 2]]| {
                    set.iter()
                        .map(|q| distance(p, *q))
                        .fold(f64::INFINITY, f64::min)
                };
                let stable = contour.iter().all(|p| nearest(*p, &mapped) <= tolerance)
                    && mapped.iter().all(|p| nearest(*p, &contour) <= tolerance);
                let rise = if stable {
                    None
                } else {
                    let axis = measured.lift;
                    let projection = |p: [f64; 2]| p[0] * axis[0] + p[1] * axis[1];
                    let bottom = *contour
                        .iter()
                        .min_by(|a, b| projection(**a).total_cmp(&projection(**b)))?;
                    let top = *contour
                        .iter()
                        .filter(|p| {
                            let d = [p[0] - bottom[0], p[1] - bottom[1]];
                            (d[0] * axis[1] - d[1] * axis[0]).abs() <= tolerance
                        })
                        .max_by(|a, b| projection(**a).total_cmp(&projection(**b)))?;
                    let height = projection(top) - projection(bottom);
                    // A stable ground base plus changing upper silhouette is
                    // actual occlusion evidence for a raised barrier. Its FULL
                    // projected extent is an upper bound, not a guessed height.
                    if nearest(bottom, &mapped) > tolerance
                        || nearest(top, &mapped) <= tolerance
                        || height <= tolerance * 2.
                        || height + uncertainty >= measured.apex_pixels - measured.error_pixels
                    {
                        continue;
                    }
                    Some(jump::RiseEvidence {
                        base: bottom,
                        top,
                        error_pixels: tolerance,
                    })
                };
                let candidate = jump::Candidate {
                    takeoff,
                    landing,
                    takeoff_support: takeoff_support.clone(),
                    landing_support,
                    same_plane: true,
                    near_edge,
                    far_edge,
                    rise,
                    landing_rise: None,
                    observed_gap: stable,
                    error_pixels: registration.error_pixels + self.step as f64,
                    clearance_pixels: contact.radius,
                };
                if matches!(
                    measured.plan(&candidate, takeoff, contact.error),
                    jump::Plan::NotObservable(_)
                ) {
                    continue;
                }
                let score = distance(foot, takeoff) * 1.05 + distance(landing, goal);
                if best.as_ref().is_none_or(|(cost, _)| score < *cost) {
                    best = Some((score, candidate));
                }
            }
        }
        best.map(|(_, candidate)| candidate)
    }
    fn coast(&self, from: Point, rejected: &[Point], radius: f64) -> Option<Point> {
        let mut candidates = Vec::new();
        let mut visited = vec![false; self.open.len()];
        for at in 0..self.open.len() {
            if !self.open[at] || visited[at] {
                continue;
            }
            let mut queue = VecDeque::from([at]);
            visited[at] = true;
            let mut component = Vec::new();
            while let Some(i) = queue.pop_front() {
                component.push(i);
                let (x, y) = (i % self.w, i / self.w);
                for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                    let (Some(xx), Some(yy)) = (x.checked_add_signed(dx), y.checked_add_signed(dy))
                    else {
                        continue;
                    };
                    if xx < self.w && yy < self.h {
                        let j = yy * self.w + xx;
                        if self.open[j] && !visited[j] {
                            visited[j] = true;
                            queue.push_back(j)
                        }
                    }
                }
            }
            if component.len() * self.step * self.step < self.geometry.1 * self.geometry.1 / 60 {
                continue;
            }
            for i in component {
                let (x, y) = (i % self.w, i / self.w);
                let r = (radius * 1.6 / self.step as f64).ceil() as isize;
                for (dx, dy) in [(-r, 0), (r, 0), (0, -r), (0, r)] {
                    let (Some(xx), Some(yy)) = (x.checked_add_signed(dx), y.checked_add_signed(dy))
                    else {
                        continue;
                    };
                    if xx < self.w && yy < self.h {
                        let j = yy * self.w + xx;
                        let p = self.normalized(self.pixel(j));
                        if self.clear(j, radius)
                            && !rejected
                                .iter()
                                .any(|&q| distance([p.0, p.1], [q.0, q.1]) < 0.06)
                        {
                            candidates.push(j)
                        }
                    }
                }
            }
        }
        let start = [
            from.0 * self.geometry.0 as f64,
            from.1 * self.geometry.1 as f64,
        ];
        candidates
            .into_iter()
            .min_by(|&a, &b| {
                distance(self.pixel(a), start).total_cmp(&distance(self.pixel(b), start))
            })
            .map(|i| self.normalized(self.pixel(i)))
    }
    fn line(&self, a: [f64; 2], b: [f64; 2], r: f64) -> bool {
        let n = (distance(a, b) / self.step as f64).ceil().max(1.) as usize;
        (0..=n).all(|i| {
            let t = i as f64 / n as f64;
            self.clear(
                self.index(self.normalized([a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t])),
                r,
            )
        })
    }
    fn route(&self, from: Point, to: Point, radius: f64) -> Option<Vec<[f64; 2]>> {
        let (start, goal) = (self.index(from), self.index(to));
        if !self.clear(goal, radius) {
            return None;
        }
        let mut costs = vec![f64::INFINITY; self.land.len()];
        let mut parents = vec![usize::MAX; self.land.len()];
        let mut heap = BinaryHeap::new();
        costs[start] = 0.;
        heap.push(Node {
            cost: 0.,
            index: start,
        });
        while let Some(Node { cost, index }) = heap.pop() {
            if index == goal {
                break;
            }
            if cost > costs[index] + 1e-6 {
                continue;
            }
            let (x, y) = (index % self.w, index / self.w);
            for (dx, dy) in [
                (-1, 0),
                (1, 0),
                (0, -1),
                (0, 1),
                (-1, -1),
                (-1, 1),
                (1, -1),
                (1, 1),
            ] {
                let (Some(xx), Some(yy)) = (x.checked_add_signed(dx), y.checked_add_signed(dy))
                else {
                    continue;
                };
                if xx >= self.w || yy >= self.h {
                    continue;
                }
                let j = yy * self.w + xx;
                if !self.clear(j, radius) || !self.line(self.pixel(index), self.pixel(j), radius) {
                    continue;
                }
                let next = cost
                    + if dx != 0 && dy != 0 {
                        std::f64::consts::SQRT_2
                    } else {
                        1.
                    };
                if next < costs[j] {
                    costs[j] = next;
                    parents[j] = index;
                    heap.push(Node {
                        cost: next,
                        index: j,
                    });
                }
            }
        }
        if !costs[goal].is_finite() {
            return None;
        }
        let mut raw = vec![self.pixel(goal)];
        let mut at = goal;
        while at != start {
            at = parents[at];
            if at == usize::MAX || raw.len() > self.land.len() {
                return None;
            }
            raw.push(self.pixel(at));
        }
        raw.reverse();
        raw[0] = [
            from.0 * self.geometry.0 as f64,
            from.1 * self.geometry.1 as f64,
        ];
        let mut pulled = vec![raw[0]];
        let mut at = 0;
        while at + 1 < raw.len() {
            let mut next = at + 1;
            for j in (at + 1..raw.len()).rev() {
                if self.line(raw[at], raw[j], radius) {
                    next = j;
                    break;
                }
            }
            pulled.push(raw[next]);
            at = next;
        }
        Some(pulled)
    }
    fn approach(&self, p: [f64; 2], radius: f64) -> Option<Rect> {
        let p = self.normalized(p);
        let r = radius * 1.15;
        if !self.walkable(p, r) {
            return None;
        }
        Some(Rect(
            p.0 - r / self.geometry.0 as f64,
            p.1 - r / self.geometry.1 as f64,
            p.0 + r / self.geometry.0 as f64,
            p.1 + r / self.geometry.1 as f64,
        ))
    }
    fn seeds(&self, from: Point, to: [f64; 2], clearance: f64) -> Vec<Point> {
        let a = [
            from.0 * self.geometry.0 as f64,
            from.1 * self.geometry.1 as f64,
        ];
        // The full arrival footprint must stay inside the seed hull; stopping
        // rays at the goal center makes the controller reject every approach.
        let radius = (distance(a, to) + clearance * 3.).max(40.);
        let mut result = Vec::new();
        for n in 0..12 {
            let angle = n as f64 * std::f64::consts::TAU / 12.;
            let mut far = None;
            for m in 1..=8 {
                let p = self.normalized([
                    a[0] + angle.cos() * radius * m as f64 / 8.,
                    a[1] + angle.sin() * radius * m as f64 / 8.,
                ]);
                if valid(p) && self.land[self.index(p)] {
                    // The occupied foot raster is usable only for leaving the
                    // current footprint. It is never a measured ground seed.
                    if self.visible[self.index(p)] {
                        far = Some(p)
                    }
                } else {
                    break;
                }
            }
            if let Some(p) = far {
                result.push(p)
            }
        }
        result
    }
}
fn distance(a: [f64; 2], b: [f64; 2]) -> f64 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}

fn control_direction(walk: Calibration, desired: [f64; 2]) -> Option<[f64; 2]> {
    let det = walk.right[0] * walk.up[1] - walk.up[0] * walk.right[1];
    if !det.is_finite() || det.abs() < 1e-8 {
        return None;
    }
    let direction = [
        (walk.up[1] * desired[0] - walk.up[0] * desired[1]) / det,
        (-walk.right[1] * desired[0] + walk.right[0] * desired[1]) / det,
    ];
    let norm = direction[0].hypot(direction[1]);
    (norm > 1e-6 && norm.is_finite()).then(|| [direction[0] / norm, direction[1] / norm])
}

fn foot_anchor(contact: Contact, included: Region, geometry: (usize, usize)) -> Option<Region> {
    let p = [
        contact.point.0 * geometry.0 as f64,
        contact.point.1 * geometry.1 as f64,
    ];
    let margin = [
        p[0] - included.x as f64,
        p[1] - included.y as f64,
        (included.x + included.width) as f64 - p[0],
        (included.y + included.height) as f64 - p[1],
    ]
    .into_iter()
    .fold(f64::INFINITY, f64::min)
        - 2.;
    let radius = (contact.radius * 5.)
        .max(geometry.1 as f64 * 0.15)
        .min(margin)
        .floor() as usize;
    if radius < geometry.0.max(geometry.1).div_ceil(512) * 7 {
        return None;
    }
    Some(Region {
        x: p[0] as usize - radius,
        y: p[1] as usize - radius,
        width: radius * 2,
        height: radius * 2,
    })
}

fn observed_region(center: [f64; 2], radius: f64, included: Region) -> Option<Region> {
    let left = (center[0] - radius).max(included.x as f64).floor() as usize;
    let top = (center[1] - radius).max(included.y as f64).floor() as usize;
    let right = (center[0] + radius)
        .min((included.x + included.width) as f64)
        .ceil() as usize;
    let bottom = (center[1] + radius)
        .min((included.y + included.height) as f64)
        .ceil() as usize;
    (right > left && bottom > top).then_some(Region {
        x: left,
        y: top,
        width: right - left,
        height: bottom - top,
    })
}

/// Relative elevation comes from TWO independently registered image planes.
/// The measured standing body's vertical parallax is the scale witness; there
/// is no supplied world height, camera pose, or assumed flat-height constant.
fn observe_landing_plane(
    before: &Snapshot,
    current: &Snapshot,
    base: &scene::Registration,
    before_grid: &Grid,
    grid: &Grid,
    point: [f64; 2],
    contact: Contact,
    lift: [f64; 2],
    vertical: [f64; 2],
    vertical_error: f64,
    body_height: f64,
    limit: usize,
) -> Option<RaisedPlane> {
    let length = vertical[0].hypot(vertical[1]);
    if length <= vertical_error * 1.5 || !base.supports(point) {
        return None;
    }
    let included = grid.included().region(grid.geometry)?;
    let radius = (contact.radius * 5.).max(grid.geometry.1 as f64 * 0.10);
    let region = observed_region(point, radius, included)?;
    let mapping = scene::register_anchored(before, current, limit, region).ok()?;
    if !mapping.supports(point) {
        return None;
    }
    let orientation_error = (base.error_pixels + mapping.error_pixels)
        / (region.width.min(region.height) as f64 * 0.3)
        + 0.04;
    if (0..2).any(|i| {
        (0..2).any(|j| (base.affine[i][j] - mapping.affine[i][j]).abs() > orientation_error)
    }) {
        return None;
    }
    let actual = mapping.map_point(point);
    let projected = base.map_point(point);
    if !valid(grid.normalized(actual))
        || !grid.plausible[grid.index(grid.normalized(actual))]
        || !before_grid.plausible[before_grid.index(before_grid.normalized(point))]
    {
        return None;
    }
    let difference = [actual[0] - projected[0], actual[1] - projected[1]];
    let height = (difference[0] * vertical[0] + difference[1] * vertical[1]) / (length * length)
        * body_height;
    let residual = (difference[0] * vertical[1] - difference[1] * vertical[0]).abs() / length;
    let measurement = base.error_pixels + mapping.error_pixels;
    if residual > measurement + height.abs() * vertical_error / body_height {
        return None;
    }
    let mut error = (measurement + height.abs() * vertical_error / body_height) * body_height
        / (length - vertical_error)
        + height.abs() * contact.error / body_height;
    if !height.is_finite() || height <= error + contact.error {
        return None;
    }
    let mut bounds = [
        f64::INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
    ];
    for source in &mapping.support_hull {
        let p = mapping.map_point(*source);
        let b = base.map_point(*source);
        let local_height = ((p[0] - b[0]) * vertical[0] + (p[1] - b[1]) * vertical[1])
            / (length * length)
            * body_height;
        error = error.max(
            (local_height - height).abs() + measurement * body_height / (length - vertical_error),
        );
        bounds[0] = bounds[0].min(p[0]);
        bounds[1] = bounds[1].min(p[1]);
        bounds[2] = bounds[2].max(p[0]);
        bounds[3] = bounds[3].max(p[1]);
    }
    let center = [(bounds[0] + bounds[2]) * 0.5, (bounds[1] + bounds[3]) * 0.5];
    let extent = (bounds[2] - bounds[0]).max(bounds[3] - bounds[1]) * 0.5 + grid.step as f64 * 6.;
    let reference_region = observed_region(center, extent, included)?;
    // The projected base point is only used inside actual base-plane support.
    let base_point = [actual[0] - lift[0] * height, actual[1] - lift[1] * height];
    base.inverse_point(base_point)?;
    Some(RaisedPlane {
        mapping,
        height,
        height_error: error,
        reference_region,
        before_point: point,
        current_point: actual,
    })
}

fn capture_anchor(
    frame: &Frame<'_>,
    rect: Rect,
    approach: Point,
    destination: Destination,
    label: String,
) -> Option<Anchor> {
    let region = rect.region((frame.width, frame.height))?;
    if region.width < 12 || region.height < 12 {
        return None;
    }
    let mut patch = Vec::with_capacity(PATCH * PATCH);
    for y in 0..PATCH {
        for x in 0..PATCH {
            let p = frame.pixel(
                region.x + x * region.width / PATCH,
                region.y + y * region.height / PATCH,
            )?;
            patch.push(
                ((77 * u32::from(p[0]) + 150 * u32::from(p[1]) + 29 * u32::from(p[2])) / 256) as u8,
            )
        }
    }
    Some(Anchor {
        destination,
        label,
        patch,
        size: [rect.2 - rect.0, rect.3 - rect.1],
        approach: [
            (approach.0 - rect.0) / (rect.2 - rect.0),
            (approach.1 - rect.1) / (rect.3 - rect.1),
        ],
        aspect: frame.width as f64 / frame.height as f64,
    })
}
fn recognize_anchor(frame: &Frame<'_>, anchor: &Anchor) -> Option<(Rect, Point)> {
    if (frame.width as f64 / frame.height as f64 / anchor.aspect - 1.).abs() > 0.04 {
        return None;
    }
    let (w, h) = (
        (anchor.size[0] * frame.width as f64).round() as usize,
        (anchor.size[1] * frame.height as f64).round() as usize,
    );
    if w < 12 || h < 12 || w > frame.width / 2 || h > frame.height / 2 {
        return None;
    }
    let mean = anchor.patch.iter().map(|&v| v as f64).sum::<f64>() / (PATCH * PATCH) as f64;
    let mut best = (f64::INFINITY, 0, 0);
    let mut candidates = Vec::new();
    let step = (frame.height / 90).max(5);
    for y in (frame.height / 10..frame.height * 4 / 5 - h).step_by(step) {
        for x in (frame.width / 8..frame.width - w).step_by(step) {
            let mut pixels = Vec::with_capacity(PATCH * PATCH);
            for yy in 0..PATCH {
                for xx in 0..PATCH {
                    let p = frame.pixel(x + xx * w / PATCH, y + yy * h / PATCH)?;
                    pixels.push(
                        ((77 * u32::from(p[0]) + 150 * u32::from(p[1]) + 29 * u32::from(p[2]))
                            / 256) as f64,
                    )
                }
            }
            let current = pixels.iter().sum::<f64>() / pixels.len() as f64;
            let error = pixels
                .iter()
                .zip(&anchor.patch)
                .map(|(a, &b)| (a - current - (b as f64 - mean)).abs())
                .sum::<f64>()
                / pixels.len() as f64;
            if error < best.0 {
                best = (error, x, y)
            }
            candidates.push((error, x, y));
        }
    }
    let second = candidates
        .iter()
        .filter(|(_, x, y)| x.abs_diff(best.1) > w / 2 || y.abs_diff(best.2) > h / 2)
        .map(|v| v.0)
        .fold(f64::INFINITY, f64::min);
    if best.0 > 12. || second - best.0 < 4. {
        return None;
    }
    let rect = Rect(
        best.1 as f64 / frame.width as f64,
        best.2 as f64 / frame.height as f64,
        (best.1 + w) as f64 / frame.width as f64,
        (best.2 + h) as f64 / frame.height as f64,
    );
    Some((
        rect,
        Point(
            rect.0 + anchor.approach[0] * (rect.2 - rect.0),
            rect.1 + anchor.approach[1] * (rect.3 - rect.1),
        ),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    const W: usize = 480;
    const H: usize = 270;
    fn floor() -> Vec<u8> {
        let mut b = vec![0; W * H * 4];
        let mut seed = 67u32;
        for y in 0..H {
            for x in 0..W {
                seed ^= seed << 13;
                seed ^= seed >> 17;
                seed ^= seed << 5;
                let d = (seed % 26) as u8;
                let at = (y * W + x) * 4;
                b[at..at + 4].copy_from_slice(&[70 + d, 130 + d, 60 + d, 255]);
            }
        }
        b
    }
    fn paint(b: &mut [u8], r: (usize, usize, usize, usize), p: [u8; 3]) {
        for y in r.1..r.3 {
            for x in r.0..r.2 {
                let at = (y * W + x) * 4;
                b[at..at + 3].copy_from_slice(&p);
            }
        }
    }
    fn body(b: &mut [u8]) {
        paint(b, (145, 100, 169, 126), [8; 3]);
        paint(b, (147, 125, 155, 145), [8; 3]);
        paint(b, (160, 125, 168, 145), [8; 3]);
    }
    fn frame(b: &[u8]) -> Frame<'_> {
        Frame::new(b, W, H, W * 4, 4, false).unwrap()
    }
    fn word(text: &str, r: (usize, usize, usize, usize)) -> Word {
        Word {
            text: text.into(),
            rect: Rect(
                r.0 as f64 / W as f64,
                r.1 as f64 / H as f64,
                r.2 as f64 / W as f64,
                r.3 as f64 / H as f64,
            ),
        }
    }
    fn own() -> Word {
        word("Alice", (142, 88, 172, 99))
    }

    #[test]
    fn actual_name_body_and_two_feet_are_required_away_from_screen_center() {
        let mut b = floor();
        body(&mut b);
        let f = frame(&b);
        let contact = avatar_contact(&f, &[own()], "Alice").unwrap();
        assert!(contact.point.0 < 0.4 && contact.point.1 > 0.5);
        assert!((contact.point.1 * H as f64 - 145.).abs() < 2.);
        assert!(avatar_contact(&f, &[own(), own()], "Alice").is_err());
        assert!(avatar_contact(&f, &[word("Bob", (142, 88, 172, 99))], "Alice").is_err());
        paint(&mut b, (140, 100, 175, 150), [245; 3]);
        assert!(
            avatar_contact(&frame(&b), &[own()], "Alice").is_err(),
            "a glow cannot become a foot contact"
        );
    }

    #[test]
    fn bright_clothing_uses_visible_body_shape_and_mark_requires_real_landmark() {
        let mut b = floor();
        body(&mut b);
        for pixel in b.chunks_exact_mut(4) {
            if pixel[..3] == [8; 3] {
                pixel[..3].copy_from_slice(&[230, 220, 210]);
            }
        }
        assert!(avatar_contact(&frame(&b), &[own()], "Alice").is_ok());
        let settings = Settings {
            player_label: "Alice".into(),
            ..Settings::default()
        };
        let words = [own(), word("Captain Flarg", (190, 120, 260, 135))];
        let memory = Wayfinder::mark(
            Destination::Sell,
            settings.clone(),
            (W, H),
            &frame(&b),
            &words,
        )
        .unwrap();
        assert_eq!(memory.anchors.len(), 1);
        assert_eq!(memory.anchors[0].destination, Destination::Sell);
        assert!(Wayfinder::mark(
            Destination::Match,
            settings.clone(),
            (W, H),
            &frame(&b),
            &words
        )
        .is_err());
        assert!(
            Wayfinder::mark(Destination::Sell, settings, (W + 1, H), &frame(&b), &words).is_err()
        );
    }

    #[test]
    fn split_ocr_words_identify_names_register_places_and_find_the_same_goal() {
        let mut b = floor();
        body(&mut b);
        let f = frame(&b);
        // Windows.Media.Ocr emits these individual word boxes, not a line.
        let words = [
            word("Alice", (142, 88, 156, 99)),
            word("Smith", (159, 88, 172, 99)),
            word("Captain", (190, 120, 227, 135)),
            word("Flarg", (232, 121, 260, 135)),
        ];
        let contact = avatar_contact(&f, &words, "Alice Smith").unwrap();
        assert!((contact.point.1 * H as f64 - 145.).abs() < 2.);
        let settings = Settings {
            player_label: "Alice Smith".into(),
            ..Settings::default()
        };
        let memory =
            Wayfinder::mark(Destination::Sell, settings.clone(), (W, H), &f, &words).unwrap();
        assert_eq!(memory.anchors[0].label, "Captain Flarg");
        let finder = Wayfinder::start(Destination::Sell, settings, 0.).unwrap();
        let grid = Grid::observe(&f, contact, &exclusions(&f, &words, &[], "Alice Smith")).unwrap();
        assert_eq!(
            finder.goal(&f, &words, &grid, contact).unwrap().1,
            "Captain Flarg"
        );
    }

    #[test]
    fn split_words_cannot_join_other_rows_distant_words_or_skip_an_intervening_word() {
        for words in [
            vec![
                word("Captain", (190, 120, 227, 135)),
                word("Flarg", (232, 141, 260, 155)),
            ],
            vec![
                word("Captain", (190, 120, 227, 135)),
                word("Flarg", (270, 120, 300, 135)),
            ],
            vec![
                word("Captain", (190, 120, 227, 135)),
                word("Other", (231, 120, 250, 135)),
                word("Flarg", (254, 120, 280, 135)),
            ],
        ] {
            assert!(matching_words(&words, &["captainflarg"], false).is_empty());
        }
    }

    #[test]
    fn display_and_account_rows_must_independently_identify_the_same_visible_feet() {
        let mut b = floor();
        body(&mut b);
        let f = frame(&b);
        let names = [
            word("Alice Smith", (142, 77, 172, 87)),
            word("@alice_account", (135, 88, 179, 99)),
        ];
        let contact = avatar_contact(&f, &names, "Alice Smith|alice_account").unwrap();
        assert!((contact.point.1 * H as f64 - 145.).abs() < 2.);
        assert!(avatar_contact(
            &f,
            &[names[0].clone(), names[0].clone()],
            "Alice Smith|alice_account"
        )
        .is_err());
        let other = word("@alice_account", (310, 88, 354, 99));
        assert!(
            avatar_contact(&f, &[names[0].clone(), other], "Alice Smith|alice_account").is_err()
        );
    }

    #[test]
    fn split_names_do_not_turn_their_numeric_word_into_an_obstacle() {
        let b = floor();
        let words = [
            word("Alice", (142, 88, 157, 99)),
            word("12345", (160, 88, 179, 99)),
        ];
        assert_eq!(exclusions(&frame(&b), &words, &[], "Alice 12345").len(), 3);
        assert_eq!(exclusions(&frame(&b), &words, &[], "Bob").len(), 4);
    }

    #[test]
    fn split_ocr_task_titles_confirm_only_the_matching_destination() {
        let b = floor();
        let f = frame(&b);
        let sell = [
            word("Sell", (180, 40, 210, 55)),
            word("Fish", (215, 40, 245, 55)),
            word("Sell", (180, 80, 210, 95)),
            word("All", (215, 80, 245, 95)),
        ];
        let craft = [
            word("Stella's", (180, 40, 222, 55)),
            word("Workshop", (227, 40, 280, 55)),
            word("Item", (180, 80, 210, 95)),
        ];
        let game = [
            word("Memory", (180, 40, 222, 55)),
            word("Match", (227, 40, 260, 55)),
            word("Available", (180, 80, 245, 95)),
        ];
        assert!(task_ui(Destination::Sell, &f, &sell));
        assert!(task_ui(Destination::Craft, &f, &craft));
        assert!(task_ui(Destination::Match, &f, &game));
        assert!(!task_ui(Destination::Sell, &f, &craft));
        assert!(!task_ui(Destination::Craft, &f, &game));
    }

    #[test]
    fn visible_shadow_separation_distinguishes_airborne_from_supported_and_missing() {
        let make = |lift: usize| {
            let mut b = floor();
            for y in 138..154 {
                for x in 136..178 {
                    if ((x as f64 - 157.) / 21.).powi(2) + ((y as f64 - 146.) / 8.).powi(2) <= 1. {
                        let at = (y * W + x) * 4;
                        for p in &mut b[at..at + 3] {
                            *p = (*p as f64 * 0.65) as u8;
                        }
                    }
                }
            }
            paint(&mut b, (145, 100 - lift, 169, 126 - lift), [8; 3]);
            paint(&mut b, (147, 125 - lift, 155, 145 - lift), [8; 3]);
            paint(&mut b, (160, 125 - lift, 168, 145 - lift), [8; 3]);
            b
        };
        let b = make(0);
        let f = frame(&b);
        let contact = avatar_contact(&f, &[own()], "Alice").unwrap();
        assert_eq!(grounding(&f, contact).0, jump::Grounding::Supported);
        let b = make(20);
        let f = frame(&b);
        let label = word("Alice", (142, 68, 172, 79));
        let contact = avatar_contact(&f, &[label], "Alice").unwrap();
        assert_eq!(grounding(&f, contact).0, jump::Grounding::Airborne);
        let mut b = floor();
        body(&mut b);
        let f = frame(&b);
        let contact = avatar_contact(&f, &[own()], "Alice").unwrap();
        assert_eq!(grounding(&f, contact).0, jump::Grounding::Unknown);
    }

    #[test]
    fn observed_ground_routes_around_a_wall_before_any_input() {
        let mut b = floor();
        body(&mut b);
        paint(&mut b, (235, 60, 260, 180), [15; 3]);
        let f = frame(&b);
        let contact = avatar_contact(&f, &[own()], "Alice").unwrap();
        let excluded = exclusions(&f, &[own()], &[], "Alice");
        let grid = Grid::observe(&f, contact, &excluded).unwrap();
        let goal = Point(350. / W as f64, 145. / H as f64);
        assert!(!grid.line(
            [contact.point.0 * W as f64, contact.point.1 * H as f64],
            [350., 145.],
            8.
        ));
        let route = grid.route(contact.point, goal, 8.).unwrap();
        assert!(
            route.len() > 2,
            "the taut route must retain a blocking corner"
        );
        for pair in route.windows(2) {
            assert!(grid.line(pair[0], pair[1], 8.));
        }
        assert!(route.windows(2).map(|p| distance(p[0], p[1])).sum::<f64>() > 190.);
        assert!(route.iter().any(|p| p[1] > 180.));
    }

    #[test]
    fn coast_candidates_use_observed_land_and_open_texture_not_blue_pixels() {
        let mut b = floor();
        body(&mut b);
        paint(&mut b, (380, 0, W, H), [180, 40, 40]);
        let f = frame(&b);
        let contact = avatar_contact(&f, &[own()], "Alice").unwrap();
        let grid = Grid::observe(&f, contact, &exclusions(&f, &[own()], &[], "Alice")).unwrap();
        let shore = grid.coast(contact.point, &[], 12.).unwrap();
        assert!(shore.0 * (W as f64) < 380. && shore.0 * (W as f64) > 320.);
        assert!(grid.walkable(shore, 12.));
        assert!(
            !task_ui(Destination::Coast, &f, &[]),
            "a coastline candidate is not fishing success"
        );
    }

    #[test]
    fn navigation_uses_processing_deadline_without_changing_capture_timestamp() {
        let b = floor();
        let f = frame(&b);
        let mut finder = Wayfinder::start(Destination::Sell, Settings::default(), 0.).unwrap();
        let step = finder.tick(1.0, (W, H), Some(&f), &[], Some((0.1, 1.3)));
        assert!(matches!(step, Tick::Command(_)));
        assert_eq!(finder.last_frame, 0.1);
        let mut expired = Wayfinder::start(Destination::Sell, Settings::default(), 0.).unwrap();
        assert!(matches!(
            expired.tick(1.0, (W, H), Some(&f), &[], Some((0.1, 0.4))),
            Tick::Stopped(_)
        ));
    }

    #[test]
    fn camera_waits_for_ack_and_never_replays_after_failure() {
        let b = floor();
        let f = frame(&b);
        let mut finder = Wayfinder::start(Destination::Sell, Settings::default(), 0.).unwrap();
        assert!(matches!(
            finder.tick(0.1, (W, H), None, &[], Some((0.1, 0.1 + 0.300))),
            Tick::Observe { .. }
        ));
        assert!(matches!(
            finder.tick(0.2, (W, H), Some(&f), &[], Some((0.2, 0.2 + 0.300))),
            Tick::Command(Command::Gesture(Gesture::Drag { .. }))
        ));
        assert!(matches!(
            finder.tick(0.3, (W, H), Some(&f), &[], Some((0.3, 0.3 + 0.300))),
            Tick::Wait(_)
        ));
        finder.ack(false, 0.4);
        assert!(matches!(
            finder.tick(0.5, (W, H), Some(&f), &[], Some((0.5, 0.5 + 0.300))),
            Tick::Stopped(_)
        ));
        assert_eq!(finder.state.actions, 1);
        assert!(matches!(
            finder.tick(0.6, (W + 1, H), Some(&f), &[], Some((0.6, 0.6 + 0.300))),
            Tick::Stopped(_)
        ));
    }

    #[test]
    fn automatic_name_and_ground_observation_reaches_the_shared_motion_controller() {
        let mut b = floor();
        body(&mut b);
        let f = frame(&b);
        let words = [own(), word("Captain Flarg", (330, 120, 400, 135))];
        let settings = Settings {
            controls: Some(Controls {
                stick_center: Point(0.15, 0.78),
                stick_edge: Point(0.20, 0.78),
            }),
            player_label: "Alice".into(),
            ..Settings::default()
        };
        let mut finder = Wayfinder::start(Destination::Sell, settings, 0.).unwrap();
        finder.phase = Phase::Locate;
        let first = finder.tick(0.1, (W, H), Some(&f), &words, Some((0.1, 0.1 + 0.300)));
        assert!(
            matches!(first, Tick::Observe { .. }),
            "{}",
            finder.state.message
        );
        assert!(finder.navigator.is_some(), "{}", finder.state.message);
        let next = finder.tick(0.4, (W, H), Some(&f), &words, Some((0.4, 0.4 + 0.300)));
        assert!(
            matches!(
                next,
                Tick::Command(Command::Gesture(Gesture::Stick { jump: None, .. }))
            ),
            "{}",
            finder.state.message
        );
        assert!(finder.state.avatar.unwrap().0 < 0.4);
        finder.ack(false, 0.45);
        assert!(matches!(
            finder.tick(0.7, (W, H), Some(&f), &words, Some((0.7, 0.7 + 0.300))),
            Tick::Stopped(_)
        ));
        assert!(
            finder.memory().anchors.is_empty(),
            "approach alone must not teach a confirmed destination"
        );
    }

    #[test]
    fn base_needs_geometric_approach_and_real_task_ui_is_destination_specific() {
        let b = floor();
        let f = frame(&b);
        let labels = [word("Most Rolls", (250, 30, 350, 50))];
        assert!(!task_ui(Destination::Base, &f, &labels));
        let mut finder = Wayfinder::start(Destination::Base, Settings::default(), 0.).unwrap();
        assert!(
            matches!(
                finder.tick(0.1, (W, H), Some(&f), &labels, Some((0.1, 0.1 + 0.300))),
                Tick::Command(_)
            ),
            "a visible leaderboard alone is not arrival"
        );
        let craft = [
            word("Stella's Workshop", (180, 40, 280, 60)),
            word("Item", (170, 70, 200, 85)),
        ];
        assert!(task_ui(Destination::Craft, &f, &craft));
        assert!(!task_ui(Destination::Sell, &f, &craft));
        let mut ready = Wayfinder::start(Destination::Craft, Settings::default(), 0.).unwrap();
        assert!(matches!(
            ready.tick(0.1, (W, H), Some(&f), &craft, Some((0.1, 0.1 + 0.300))),
            Tick::Arrived
        ));
        assert_eq!(
            ready.state.actions, 0,
            "an already open real task UI must bypass camera/input"
        );
    }

    #[test]
    fn memory_is_committed_only_after_the_real_interaction_ui() {
        let b = floor();
        let f = frame(&b);
        let a = capture_anchor(
            &f,
            Rect(0.35, 0.3, 0.5, 0.5),
            Point(0.4, 0.55),
            Destination::Sell,
            "Captain Flarg".into(),
        )
        .unwrap();
        let mut finder = Wayfinder::start(Destination::Sell, Settings::default(), 0.).unwrap();
        finder.pending_anchor = Some(a);
        assert!(finder.memory().anchors.is_empty());
        let ui = [
            word("Sell Fish", (170, 150, 230, 170)),
            word("Captain Flarg", (160, 90, 260, 110)),
        ];
        assert!(matches!(
            finder.tick(0.1, (W, H), Some(&f), &ui, Some((0.1, 0.1 + 0.300))),
            Tick::Arrived
        ));
        assert_eq!(finder.memory().anchors.len(), 1);
        assert_eq!(finder.memory().anchors[0].label, "Captain Flarg");
    }

    fn scene_avatar(bytes: &mut [u8], foot: [usize; 2], floor: [usize; 2]) -> Word {
        scene_avatar_height(bytes, foot, floor, 45)
    }
    fn scene_avatar_height(
        bytes: &mut [u8],
        foot: [usize; 2],
        floor: [usize; 2],
        height: usize,
    ) -> Word {
        for y in floor[1] - 8..floor[1] + 9 {
            for x in floor[0] - 21..floor[0] + 22 {
                if ((x as f64 - floor[0] as f64) / 21.).powi(2)
                    + ((y as f64 - floor[1] as f64) / 8.).powi(2)
                    <= 1.
                {
                    let at = (y * W + x) * 4;
                    for p in &mut bytes[at..at + 3] {
                        *p = (*p as f64 * 0.65) as u8;
                    }
                }
            }
        }
        paint(
            bytes,
            (
                foot[0] - 12,
                foot[1] - height,
                foot[0] + 12,
                foot[1] - height * 19 / 45,
            ),
            [8; 3],
        );
        paint(
            bytes,
            (
                foot[0] - 10,
                foot[1] - height * 20 / 45,
                foot[0] - 2,
                foot[1],
            ),
            [8; 3],
        );
        paint(
            bytes,
            (
                foot[0] + 3,
                foot[1] - height * 20 / 45,
                foot[0] + 11,
                foot[1],
            ),
            [8; 3],
        );
        word(
            "Alice",
            (
                foot[0] - 15,
                foot[1] - height - 12,
                foot[0] + 15,
                foot[1] - height - 1,
            ),
        )
    }
    fn camera_view(source: &[u8], angle: f64, pivot: [f64; 2]) -> Vec<u8> {
        let mut out = vec![0; source.len()];
        let (c, s) = (angle.cos(), angle.sin());
        for y in 0..H {
            for x in 0..W {
                let (dx, dy) = (x as f64 - pivot[0], (y as f64 - pivot[1]) / 0.94);
                let (u, v) = (
                    (c * dx + s * dy + pivot[0]).round() as isize,
                    (-s * dx + c * dy + pivot[1]).round() as isize,
                );
                if u >= 0 && v >= 0 && u < W as isize && v < H as isize {
                    let p = (y * W + x) * 4;
                    let q = (v as usize * W + u as usize) * 4;
                    out[p..p + 4].copy_from_slice(&source[q..q + 4]);
                }
            }
        }
        out
    }
    fn measured_sample(direction: [f64; 2], endpoint: [f64; 2], apex: f64) -> jump::JumpSample {
        jump::JumpSample {
            direction,
            endpoint,
            input_ms: 100,
            flight: [
                (0., 0., 0.),
                (0.1, 0.2, 0.5),
                (0.2, 0.4, 0.9),
                (0.3, 0.6, 1.),
                (0.4, 0.8, 0.8),
                (0.5, 0.93, 0.35),
                (0.6, 1., 0.),
            ]
            .into_iter()
            .map(|(t, g, h)| jump::FlightPoint {
                elapsed_seconds: t,
                ground: [endpoint[0] * g, endpoint[1] * g],
                height_pixels: apex * h,
                error_pixels: 0.5,
            })
            .collect(),
        }
    }
    #[test]
    fn two_actual_image_views_generate_a_crossing_and_ack_airborne_landing_complete_it() {
        for initial_height in [45, 20] {
            let mut terrain = floor();
            paint(&mut terrain, (230, 65, 245, 207), [160, 55, 45]);
            let mut before = terrain.clone();
            let name = scene_avatar_height(&mut before, [157, 145], [157, 146], initial_height);
            let settings = Settings {
                controls: Some(Controls {
                    stick_center: Point(0.15, 0.78),
                    stick_edge: Point(0.20, 0.78),
                }),
                jump: Some(Point(0.9, 0.8)),
                player_label: "Alice".into(),
                ..Settings::default()
            };
            let f = frame(&before);
            let contact = avatar_contact(&f, &[name.clone()], "Alice").unwrap();
            let excluded = exclusions(&f, &[name.clone()], &[], "Alice");
            let grid = Grid::observe(&f, contact, &excluded).unwrap();
            let included = grid.included().region((W, H)).unwrap();
            let regions: Vec<_> = excluded.iter().filter_map(|r| r.region((W, H))).collect();
            let reference = Snapshot::capture(&f, included, &regions).unwrap();
            let mut finder = Wayfinder::start(Destination::Sell, settings, 0.).unwrap();
            finder.geometry = Some((W, H));
            finder.last_now = 0.1;
            finder.phase = Phase::Locate;
            finder.walk = Some(Calibration {
                right: [1.4, 0.],
                up: [0., -1.4],
                error_pixels: 0.5,
            });
            finder.measured_jump = Some(JumpModel {
                calibration: jump::JumpCalibration {
                    lift: [0., -1.],
                    apex_pixels: 24.,
                    shadow_per_height: [0., 0.],
                    supported_foot_minus_shadow: [0., -1.],
                    samples: vec![
                        measured_sample([1., 0.], [150., 0.], 24.),
                        measured_sample([0., 1.], [0., -90.], 24.),
                    ],
                    error_pixels: 0.5,
                    observed_flight_seconds: 0.7,
                },
                reference,
                included,
                excluded: regions,
                body: contact,
            });
            let labels = [name.clone(), word("Captain Flarg", (330, 138, 400, 151))];
            assert!(
                matches!(
                    finder.tick(0.1, (W, H), Some(&f), &labels, Some((0.1, 0.1 + 0.300))),
                    Tick::Command(_)
                ),
                "{}",
                finder.state.message
            );
            assert!(
                matches!(
                    finder.tick(
                        0.2,
                        (W, H),
                        Some(&f),
                        &[name.clone()],
                        Some((0.2, 0.2 + 0.300))
                    ),
                    Tick::Wait(_)
                ),
                "camera input requires an ACK"
            );
            finder.ack(true, 0.23);
            let after_terrain = camera_view(&terrain, 0., [157., 145.]);
            let mut after = after_terrain.clone();
            let name = scene_avatar(&mut after, [157, 145], [157, 146]);
            let result = finder.tick(
                0.5,
                (W, H),
                Some(&frame(&after)),
                &[name.clone()],
                Some((0.5, 0.5 + 0.300)),
            );
            assert!(
                matches!(result, Tick::Observe { .. }),
                "{}",
                finder.state.message
            );
            if initial_height == 45 {
                assert!(
                    matches!(finder.phase, Phase::Locate),
                    "an unresolved vertical baseline cannot distinguish a tall wall from a gap: {}",
                    finder.state.message
                );
                assert!(finder.jump_session.is_none());
                continue;
            }
            assert!(
                matches!(finder.phase, Phase::Jump),
                "{}",
                finder.state.message
            );
            let landing = finder.state.route[1];
            let land = [
                (landing.0 * W as f64).round() as usize,
                (landing.1 * H as f64).round() as usize,
            ];
            assert!(
                land[0] > 245,
                "the observed landing must be across the actual barrier"
            );
            let result = finder.tick(
                0.6,
                (W, H),
                Some(&frame(&after)),
                &[name],
                Some((0.6, 0.6 + 0.300)),
            );
            assert!(
                matches!(
                    result,
                    Tick::Command(Command::Gesture(Gesture::Stick { jump: Some(_), .. }))
                ),
                "{}",
                finder.state.message
            );
            finder.ack(true, 0.7);
            let mut air = after_terrain.clone();
            let name = scene_avatar(&mut air, [190, 100], [190, 146]);
            assert!(
                matches!(
                    finder.tick(
                        0.8,
                        (W, H),
                        Some(&frame(&air)),
                        &[name],
                        Some((0.8, 0.8 + 0.300))
                    ),
                    Tick::Observe { .. }
                ),
                "{}",
                finder.state.message
            );
            let mut landed = after_terrain;
            let name = scene_avatar(&mut landed, land, [land[0], land[1] + 1]);
            assert!(
                matches!(
                    finder.tick(
                        1.2,
                        (W, H),
                        Some(&frame(&landed)),
                        &[name.clone()],
                        Some((1.2, 1.2 + 0.300))
                    ),
                    Tick::Observe { .. }
                ),
                "{}",
                finder.state.message
            );
            assert!(
                matches!(
                    finder.tick(
                        1.3,
                        (W, H),
                        Some(&frame(&landed)),
                        &[name],
                        Some((1.3, 1.3 + 0.300))
                    ),
                    Tick::Observe { .. }
                ),
                "{}",
                finder.state.message
            );
            assert!(
                matches!(finder.phase, Phase::Locate),
                "{}",
                finder.state.message
            );
            assert_eq!(
                finder.state.actions, 2,
                "one view probe and one pre-contact crossing"
            );
            assert!(
                finder.memory().anchors.is_empty(),
                "a landing is not a confirmed shop UI"
            );
        }
    }

    #[test]
    fn unchanged_view_or_uncommanded_avatar_motion_cannot_create_a_jump_candidate() {
        for displacement in [0, 30] {
            let terrain = floor();
            let mut before = terrain.clone();
            let name = scene_avatar(&mut before, [157, 145], [157, 146]);
            let f = frame(&before);
            let contact = avatar_contact(&f, &[name.clone()], "Alice").unwrap();
            let grid =
                Grid::observe(&f, contact, &exclusions(&f, &[name.clone()], &[], "Alice")).unwrap();
            let settings = Settings {
                player_label: "Alice".into(),
                ..Settings::default()
            };
            let mut finder = Wayfinder::start(Destination::Sell, settings, 0.).unwrap();
            finder.last_now = 0.1;
            assert!(matches!(
                finder.start_survey(&f, &[name], grid, Point(350. / W as f64, 145. / H as f64)),
                Tick::Command(_)
            ));
            finder.ack(true, 0.23);
            let mut after = terrain;
            let name = scene_avatar(
                &mut after,
                [157 + displacement, 145],
                [157 + displacement, 146],
            );
            assert!(matches!(
                finder.tick(
                    0.5,
                    (W, H),
                    Some(&frame(&after)),
                    &[name],
                    Some((0.5, 0.5 + 0.300))
                ),
                Tick::Observe { .. }
            ));
            assert!(matches!(finder.phase, Phase::Locate));
            assert!(finder.jump_session.is_none());
            assert_eq!(
                finder.state.actions, 1,
                "observing never acknowledges or emits a crossing"
            );
            assert!(
                finder.state.message.contains(if displacement == 0 {
                    "시차"
                } else {
                    "발의 지면 위치"
                }),
                "{}",
                finder.state.message
            );
        }
    }

    #[test]
    fn two_observed_planes_plan_and_confirm_a_raised_landing() {
        const A: usize = 1280;
        const B: usize = 810;
        fn picture() -> Vec<u8> {
            let mut out = vec![0; A * B * 4];
            for y in 0..B {
                for x in 0..A {
                    let mut v = ((y / 3) * (A / 3 + 1) + x / 3 + 67) as u32;
                    v ^= v << 13;
                    v ^= v >> 17;
                    v ^= v << 5;
                    let d = (v % 26) as u8;
                    let at = (y * A + x) * 4;
                    out[at..at + 4].copy_from_slice(&[70 + d, 130 + d, 60 + d, 255]);
                }
            }
            out
        }
        fn f(b: &[u8]) -> Frame<'_> {
            Frame::new(b, A, B, A * 4, 4, false).unwrap()
        }
        fn text(s: &str, r: [usize; 4]) -> Word {
            Word {
                text: s.into(),
                rect: Rect(
                    r[0] as f64 / A as f64,
                    r[1] as f64 / B as f64,
                    r[2] as f64 / A as f64,
                    r[3] as f64 / B as f64,
                ),
            }
        }
        fn avatar(b: &mut [u8], foot: [usize; 2], shadow: [usize; 2], h: usize) -> Word {
            for y in shadow[1] - 8..shadow[1] + 9 {
                for x in shadow[0] - 21..shadow[0] + 22 {
                    if ((x as f64 - shadow[0] as f64) / 21.).powi(2)
                        + ((y as f64 - shadow[1] as f64) / 8.).powi(2)
                        <= 1.
                    {
                        for v in &mut b[(y * A + x) * 4..(y * A + x) * 4 + 3] {
                            *v = (*v as f64 * 0.65) as u8;
                        }
                    }
                }
            }
            for r in [
                [
                    foot[0] - 12,
                    foot[1] - h,
                    foot[0] + 12,
                    foot[1] - h * 19 / 45,
                ],
                [foot[0] - 10, foot[1] - h * 20 / 45, foot[0] - 2, foot[1]],
                [foot[0] + 3, foot[1] - h * 20 / 45, foot[0] + 11, foot[1]],
            ] {
                for y in r[1]..r[3] {
                    for x in r[0]..r[2] {
                        b[(y * A + x) * 4..(y * A + x) * 4 + 3].fill(8);
                    }
                }
            }
            text(
                "Alice",
                [
                    foot[0] - 20,
                    foot[1] - h - 18,
                    foot[0] + 20,
                    foot[1] - h - 2,
                ],
            )
        }
        let base = picture();
        let mut upper = base.clone();
        for y in 80..660 {
            for x in 600..1180 {
                let at = (y * A + x) * 4;
                let d = base[at] - 70;
                upper[at..at + 3].copy_from_slice(&[125 + d, 115 + d, 90 + d]);
            }
        }
        for (cx, cy) in [(630, 125), (1150, 125), (630, 640), (1150, 640)] {
            for y in cy - 12..cy + 13 {
                for x in cx - 12..cx + 13 {
                    let mut seed = (y * A + x + 9753) as u32;
                    seed ^= seed << 13;
                    seed ^= seed >> 17;
                    seed ^= seed << 5;
                    let v = (seed % 120 + 50) as u8;
                    let p = (y * A + x) * 4;
                    upper[p..p + 3].copy_from_slice(&[v, v, v]);
                }
            }
        }
        let mut before = upper.clone();
        let name = avatar(&mut before, [300, 410], [300, 411], 40);
        let contact = avatar_contact(&f(&before), &[name.clone()], "Alice").unwrap();
        assert_eq!(
            grounding(&f(&before), contact).0,
            jump::Grounding::Supported
        );
        let settings = Settings {
            controls: Some(Controls {
                stick_center: Point(0.15, 0.78),
                stick_edge: Point(0.20, 0.78),
            }),
            jump: Some(Point(0.9, 0.8)),
            player_label: "Alice".into(),
            ..Settings::default()
        };
        let included = Rect(0.12, 0.08, 0.98, 0.82).region((A, B)).unwrap();
        let mut excluded: Vec<_> = exclusions(&f(&before), &[name.clone()], &[], "Alice")
            .iter()
            .filter_map(|r| r.region((A, B)))
            .collect();
        excluded.push(contact.bounds.region((A, B)).unwrap());
        let reference = Snapshot::capture(&f(&before), included, &excluded).unwrap();
        let sample = jump::JumpSample {
            direction: [1., 0.],
            endpoint: [600., 0.],
            input_ms: 100,
            flight: (0..=40)
                .map(|i| {
                    let t = i as f64 / 40.;
                    jump::FlightPoint {
                        elapsed_seconds: t * 0.7,
                        ground: [600. * t, 0.],
                        height_pixels: 400. * t * (1. - t),
                        error_pixels: 0.5,
                    }
                })
                .collect(),
        };
        let mut finder = Wayfinder::start(Destination::Sell, settings, 0.).unwrap();
        finder.phase = Phase::Locate;
        finder.walk = Some(Calibration {
            right: [6., 0.],
            up: [0., -6.],
            error_pixels: 0.5,
        });
        finder.measured_jump = Some(JumpModel {
            calibration: jump::JumpCalibration {
                lift: [0., -1.],
                apex_pixels: 100.,
                shadow_per_height: [0., 0.],
                supported_foot_minus_shadow: [0., -1.],
                samples: vec![sample],
                error_pixels: 0.5,
                observed_flight_seconds: 0.7,
            },
            reference,
            included,
            excluded,
            body: contact,
        });
        let words = [name, text("Captain Flarg", [800, 365, 870, 380])];
        assert!(
            matches!(
                finder.tick(
                    0.1,
                    (A, B),
                    Some(&f(&before)),
                    &words,
                    Some((0.1, 0.1 + 0.300))
                ),
                Tick::Command(_)
            ),
            "{}",
            finder.state.message
        );
        finder.ack(true, 0.31);
        let height = 90.;
        let vertical_shift = height * (40. / 95. - 1.);
        let mut terrain = vec![0; A * B * 4];
        for y in 0..B {
            for x in 0..A {
                let base_y = (y as f64 - 12.).round() as isize;
                if base_y >= 0 && base_y < B as isize {
                    let a = (base_y as usize * A + x) * 4;
                    let b = (y * A + x) * 4;
                    terrain[b..b + 4].copy_from_slice(&base[a..a + 4]);
                }
                let top_y = (y as f64 - 12. - vertical_shift).round() as isize;
                if (600..1180).contains(&x) && top_y >= 80 && top_y < 660 {
                    let a = (top_y as usize * A + x) * 4;
                    let b = (y * A + x) * 4;
                    terrain[b..b + 4].copy_from_slice(&upper[a..a + 4]);
                }
            }
        }
        let mut after = terrain.clone();
        let name = avatar(&mut after, [300, 422], [300, 423], 95);
        let result = finder.tick(
            0.6,
            (A, B),
            Some(&f(&after)),
            &[name.clone()],
            Some((0.6, 0.6 + 0.300)),
        );
        assert!(
            matches!(result, Tick::Observe { .. }),
            "{}",
            finder.state.message
        );
        assert!(
            matches!(finder.phase, Phase::Jump),
            "{}",
            finder.state.message
        );
        let measured_height = finder
            .jump_session
            .as_ref()
            .unwrap()
            .runner
            .test_landing_height()
            .unwrap();
        assert!(
            (measured_height - height).abs() < 3.,
            "known fixture rise {height} versus actually reconstructed {measured_height}"
        );
        assert!(
            finder.jump_session.as_ref().unwrap().landing.is_some(),
            "higher floor needs its OWN tracked plane"
        );
        let landing = finder.state.route[1];
        let point = [
            (landing.0 * A as f64).round() as usize,
            (landing.1 * B as f64).round() as usize,
        ];
        assert!(
            point[0] > 600 && point[1] < 410,
            "the planned actual floor is above takeoff"
        );
        let result = finder.tick(
            0.7,
            (A, B),
            Some(&f(&after)),
            &[name],
            Some((0.7, 0.7 + 0.300)),
        );
        assert!(
            matches!(
                result,
                Tick::Command(Command::Gesture(Gesture::Stick { jump: Some(_), .. }))
            ),
            "{}",
            finder.state.message
        );
        finder.ack(true, 0.8);
        let mut air = terrain.clone();
        let name = avatar(&mut air, [500, 242], [500, 423], 95);
        assert!(
            matches!(
                finder.tick(
                    0.9,
                    (A, B),
                    Some(&f(&air)),
                    &[name],
                    Some((0.9, 0.9 + 0.300))
                ),
                Tick::Observe { .. }
            ),
            "{}",
            finder.state.message
        );
        let mut landed = terrain;
        let name = avatar(&mut landed, point, [point[0], point[1] + 1], 95);
        assert!(
            matches!(
                finder.tick(
                    1.3,
                    (A, B),
                    Some(&f(&landed)),
                    &[name.clone()],
                    Some((1.3, 1.3 + 0.300))
                ),
                Tick::Observe { .. }
            ),
            "{}",
            finder.state.message
        );
        assert!(
            matches!(
                finder.tick(
                    1.4,
                    (A, B),
                    Some(&f(&landed)),
                    &[name],
                    Some((1.4, 1.4 + 0.300))
                ),
                Tick::Observe { .. }
            ),
            "{}",
            finder.state.message
        );
        assert!(
            matches!(finder.phase, Phase::Locate),
            "{}",
            finder.state.message
        );
        assert!(
            finder.memory().anchors.is_empty(),
            "raised landing still cannot declare the shop open"
        );
    }
}

use crate::vision::{Frame, Point};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, VecDeque};

pub const BASE: [&str; 5] = [
    "menu_btn",
    "reset_btn",
    "reset_confirm",
    "collection_pos",
    "collection_close",
];
pub const SELL: [&str; 6] = [
    "dialog_pos",
    "sell_fish_pos",
    "first_fish_pos",
    "sell_all_pos",
    "confirm_sell_pos",
    "shop_close_pos",
];

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Place {
    Fish,
    Sell,
}

#[derive(Clone, Copy, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Options {
    pub use_move: bool,
    pub use_sell: bool,
    reset_wait: f64,
    wa: f64,
    a: f64,
    aw: f64,
    pause: f64,
    d: f64,
    tilt: u32,
    zoom: u32,
    margin: f64,
    sell_max: u32,
    sell_delay: f64,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            use_move: false,
            use_sell: false,
            reset_wait: 3.5,
            wa: 3.5,
            a: 3.0,
            aw: 2.0,
            pause: 0.5,
            d: 1.0,
            tilt: 3,
            zoom: 4,
            margin: 0.3,
            sell_max: 100,
            sell_delay: 0.0,
        }
    }
}

impl Options {
    pub fn validate(self) -> Result<Self, &'static str> {
        let walks = [self.wa, self.a, self.aw, self.d];
        if !(0.5..=30.0).contains(&self.reset_wait)
            || walks.iter().any(|time| !(0.0..=30.0).contains(time))
            || !(0.0..=10.0).contains(&self.pause)
            || self.tilt > 10
            || self.zoom > 10
            || !(0.0..=5.0).contains(&self.margin)
            || !(1..=500).contains(&self.sell_max)
            || !(0.0..=5.0).contains(&self.sell_delay)
        {
            return Err("이동 · 판매 설정의 범위를 확인하세요.");
        }
        Ok(self)
    }
}

#[derive(Clone, Default, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Navigation {
    pub options: Options,
    pub positions: BTreeMap<String, Point>,
    pub fish: Vec<Stride>,
    pub sell: Vec<Stride>,
    pub fish_trace: Vec<TraceEvent>,
    pub sell_trace: Vec<TraceEvent>,
    #[serde(skip)]
    pub desktop: Option<DesktopMove>,
}

#[derive(Clone, Copy, Deserialize, Serialize)]
pub struct Stride {
    x: i8,
    y: i8,
    ms: u32,
    jump: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TraceEvent {
    pub k: char,
    pub t: f64,
    pub dx: f64,
    pub dy: f64,
    pub a: bool,
    pub b: bool,
}

pub fn normalize_trace(mut events: Vec<TraceEvent>) -> Result<Vec<TraceEvent>, &'static str> {
    if events.len() > 30000
        || events.iter().any(|event| {
            !matches!(event.k, 'F' | 'M' | 'U' | 'X')
                || !event.t.is_finite()
                || !event.dx.is_finite()
                || !event.dy.is_finite()
                || event.dx.abs() > 1.01
                || event.dy.abs() > 1.01
        })
    {
        return Err("길 기록 데이터가 올바르지 않습니다.");
    }
    let first = events
        .iter()
        .position(|event| event.k == 'F')
        .ok_or("이동 동작을 기록한 뒤 도착을 눌러 주세요.")?;
    events.drain(..first);
    let start = events[0].t;
    let mut last: f64 = 0.0;
    let mut stick = false;
    let mut jump = false;
    for event in &mut events {
        event.t = ((event.t - start) * 10.0).round().max(last * 10.0) / 10.0;
        last = event.t;
        match event.k {
            'F' => {
                stick = event.a;
                jump = event.b;
            }
            'U' => {
                stick &= !event.a;
                jump &= !event.b;
            }
            'X' => {
                stick = false;
                jump = false;
            }
            _ => {}
        }
    }
    if last > 300000.0 {
        return Err("길 기록 시간이 너무 깁니다.");
    }
    if stick || jump {
        events.push(TraceEvent {
            k: 'X',
            t: last,
            dx: 0.0,
            dy: 0.0,
            a: false,
            b: false,
        });
    }
    if !events.iter().any(|event| event.k == 'F' && event.a) {
        return Err("이동 동작을 먼저 기록하세요.");
    }
    Ok(events)
}

impl Navigation {
    pub fn trace(&mut self, place: Place) -> &mut Vec<TraceEvent> {
        match place {
            Place::Fish => &mut self.fish_trace,
            Place::Sell => &mut self.sell_trace,
        }
    }
    pub fn route_parts(&self, place: Place) -> usize {
        let (trace, route) = match place {
            Place::Fish => (&self.fish_trace, &self.fish),
            Place::Sell => (&self.sell_trace, &self.sell),
        };
        if trace.is_empty() {
            route.len()
        } else {
            trace
                .iter()
                .filter(|event| event.k == 'F' && event.a)
                .count()
        }
    }
    fn trace_motion(&self, events: Vec<TraceEvent>) -> Result<Motion, &'static str> {
        let Motion::Stick { center, edge, .. } = self.stick(0, -1, 1, false)? else {
            unreachable!()
        };
        Ok(Motion::Trace {
            center,
            edge,
            jump: self.positions.get("jump_btn").copied(),
            events,
        })
    }

    pub fn route(&mut self, place: Place) -> &mut Vec<Stride> {
        match place {
            Place::Fish => &mut self.fish,
            Place::Sell => &mut self.sell,
        }
    }

    fn points(&self, keys: &[&str]) -> Option<Vec<Point>> {
        keys.iter()
            .map(|key| self.positions.get(*key).copied())
            .collect()
    }

    pub fn fit_positions(&mut self) {
        for point in self.positions.values_mut() {
            point.0 = point.0.clamp(0.0, 0.9999);
            point.1 = point.1.clamp(0.0, 0.9999);
        }
    }

    pub fn set_point(&mut self, key: &str, mut point: Point) -> Result<(), &'static str> {
        let allowed = BASE.contains(&key)
            || SELL.contains(&key)
            || ["stick_center", "stick_edge", "jump_btn", "talk_btn"].contains(&key);
        if !allowed || !point.0.is_finite() || !point.1.is_finite() {
            return Err("저장할 위치를 확인하세요.");
        }
        point.0 = point.0.clamp(0.0, 0.9999);
        point.1 = point.1.clamp(0.0, 0.9999);
        if key == "stick_center" {
            self.positions.remove("stick_edge");
        }
        self.positions.insert(key.into(), point);
        Ok(())
    }

    fn stick(&self, x: i8, y: i8, ms: u32, jump: bool) -> Result<Motion, &'static str> {
        let center = *self
            .positions
            .get("stick_center")
            .ok_or("조이스틱 가운데를 먼저 지정하세요.")?;
        let edge = *self
            .positions
            .get("stick_edge")
            .ok_or("조이스틱 가장자리를 먼저 지정하세요.")?;
        if (center.0 - edge.0).hypot(center.1 - edge.1) < 0.004 {
            return Err("조이스틱 가운데와 가장자리를 다시 지정하세요.");
        }
        let jump = if jump {
            Some(
                *self
                    .positions
                    .get("jump_btn")
                    .ok_or("점프 버튼을 먼저 지정하세요.")?,
            )
        } else {
            None
        };
        Ok(Motion::Stick {
            center,
            edge,
            x,
            y,
            jump,
            ms,
        })
    }

    fn align(&self, queue: &mut VecDeque<Step>) -> Result<(), &'static str> {
        if self.desktop.is_some() {
            let points = self
                .points(&["chat_pos", "collection_pos", "collection_close"])
                .ok_or("채팅 · 도감 버튼 · 도감 Exit를 먼저 지정하세요.")?;
            key(queue, &[0xbf], 40, 0.5);
            for point in points {
                tap(queue, point, 0.5);
            }
            key(queue, &[0xbf], 40, 0.5);
            key(queue, &[0x0d], 40, 0.3);
            return Ok(());
        }
        let points = self
            .points(&BASE[3..])
            .ok_or("도감 버튼과 도감 Exit 위치를 먼저 지정하세요.")?;
        tap(queue, points[0], 0.9);
        tap(queue, points[1], 0.5);
        Ok(())
    }

    fn base(&self, queue: &mut VecDeque<Step>) -> Result<(), &'static str> {
        if let Some(desktop) = &self.desktop {
            key(queue, &[0x1b], 40, 0.5);
            key(queue, &[0x52], 40, 0.5);
            key(queue, &[0x0d], 40, desktop.reset_wait);
            self.align(queue)?;
            key(queue, &[0x57], millis(desktop.w_time), 0.0);
            key(queue, &[0x57, 0x41], millis(desktop.wa_time), 0.0);
            let tail = desktop.a_time.min(desktop.w2_time);
            key(queue, &[0x41], millis(desktop.a_time - tail), 0.0);
            key(queue, &[0x41, 0x57], millis(tail), 0.0);
            key(queue, &[0x53, 0x44], millis(desktop.sd_time), 0.5);
            queue.push_back(Step::Input(
                Motion::Tilt {
                    pixels: desktop.tilt_px,
                    ms: millis(desktop.o_time),
                },
                0.3,
            ));
            return Ok(());
        }
        let reset = self
            .points(&BASE[..3])
            .ok_or("기준 장소 메뉴 · 재설정 · 확인 위치를 먼저 지정하세요.")?;
        for (point, wait) in reset.into_iter().zip([0.8, 0.8, self.options.reset_wait]) {
            tap(queue, point, wait);
        }
        for _ in 0..self.options.zoom {
            queue.push_back(Step::Input(Motion::Pinch, 0.6));
        }
        self.align(queue)?;
        let mut events = Vec::new();
        let mut time = 0.0;
        let mut held = false;
        for (x, y, seconds) in [
            (-1.0, -1.0, self.options.wa),
            (-1.0, 0.0, self.options.a),
            (-1.0, -1.0, self.options.aw),
            (0.0, 0.0, self.options.pause),
            (1.0, 0.0, self.options.d),
        ] {
            if seconds == 0.0 {
                continue;
            }
            let magnitude: f64 = x * x + y * y;
            let moving = magnitude > 0.0;
            let length = magnitude.sqrt().max(1.0);
            events.push(TraceEvent {
                k: if moving {
                    if held {
                        'M'
                    } else {
                        'F'
                    }
                } else {
                    'X'
                },
                t: time,
                dx: x / length,
                dy: y / length,
                a: moving,
                b: false,
            });
            held = moving;
            time += seconds * 1000.0;
        }
        events.push(TraceEvent {
            k: 'X',
            t: time,
            dx: 0.0,
            dy: 0.0,
            a: false,
            b: false,
        });
        queue.push_back(Step::Input(self.trace_motion(events)?, 0.0));
        queue.push_back(Step::Wait(0.5));
        for _ in 0..self.options.tilt {
            queue.push_back(Step::Input(Motion::Swipe, 0.4));
        }
        queue.push_back(Step::Wait(0.3));
        Ok(())
    }

    fn walk(&self, place: Place, queue: &mut VecDeque<Step>) -> Result<(), &'static str> {
        if let Some(desktop) = &self.desktop {
            let route = desktop.waypoints(place);
            if route.is_empty() {
                return Err("이동할 장소의 지점을 먼저 지정하세요.");
            }
            for waypoint in route {
                let point = waypoint
                    .pos
                    .filter(valid_point)
                    .ok_or("이동 지점 위치를 먼저 지정하세요.")?;
                let seconds = waypoint
                    .time
                    .filter(|s| (0.0..=300.0).contains(s))
                    .ok_or("이동 지점의 시간을 먼저 저장하세요.")?;
                queue.push_back(Step::Input(
                    Motion::Click {
                        point,
                        right: desktop.button != "left",
                    },
                    seconds + desktop.margin,
                ));
            }
            return Ok(());
        }
        let trace = match place {
            Place::Fish => &self.fish_trace,
            Place::Sell => &self.sell_trace,
        };
        if !trace.is_empty() {
            queue.push_back(Step::Input(
                self.trace_motion(normalize_trace(trace.clone())?)?,
                self.options.margin,
            ));
            return Ok(());
        }
        let route = match place {
            Place::Fish => &self.fish,
            Place::Sell => &self.sell,
        };
        if route.is_empty() {
            return Err(match place {
                Place::Fish => "낚시 장소 길을 먼저 기록하세요.",
                Place::Sell => "판매 장소 길을 먼저 기록하세요.",
            });
        }
        for s in route {
            let motion = if s.x == 0 && s.y == 0 && s.jump {
                Motion::Tap(
                    *self
                        .positions
                        .get("jump_btn")
                        .ok_or("점프 버튼을 먼저 지정하세요.")?,
                    s.ms,
                )
            } else {
                self.stick(s.x, s.y, s.ms, s.jump)?
            };
            queue.push_back(Step::Input(motion, 0.0));
        }
        queue.push_back(Step::Wait(self.options.margin));
        Ok(())
    }

    pub fn desktop_waypoint(
        &self,
        place: Place,
        index: usize,
        measure: bool,
        from_base: bool,
        now: f64,
    ) -> Result<Job, &'static str> {
        let desktop = self
            .desktop
            .as_ref()
            .ok_or("Windows 이동 설정이 없습니다.")?;
        let route = desktop.waypoints(place);
        let waypoint = route.get(index).ok_or("이동 지점을 찾을 수 없습니다.")?;
        let mut job = Job::new(if measure { Task::Measure } else { Task::Base }, now, false);
        if from_base {
            self.base(&mut job.steps)?;
            for earlier in &route[..index] {
                let point = earlier
                    .pos
                    .filter(valid_point)
                    .ok_or("앞 지점의 위치를 먼저 지정하세요.")?;
                let time = earlier
                    .time
                    .filter(|s| (0.0..=300.0).contains(s))
                    .ok_or("앞 지점의 시간을 먼저 재세요.")?;
                job.steps.push_back(Step::Input(
                    Motion::Click {
                        point,
                        right: desktop.button != "left",
                    },
                    time + desktop.margin,
                ));
            }
        }
        if measure {
            let point = waypoint
                .pos
                .filter(valid_point)
                .ok_or("지점 위치를 먼저 지정하세요.")?;
            job.steps.push_back(Step::Input(
                Motion::Click {
                    point,
                    right: desktop.button != "left",
                },
                0.0,
            ));
        }
        Ok(job)
    }

    pub fn job(&self, action: Task, now: f64, resume: bool) -> Result<Job, &'static str> {
        let mut steps = VecDeque::new();
        let mut job = Job::new(action, now, resume);
        match action {
            Task::Measure => return Err("먼저 시간을 잴 지점을 지정하세요."),
            Task::Base => self.base(&mut steps)?,
            Task::Fish | Task::Shop => {
                self.base(&mut steps)?;
                self.walk(
                    if action == Task::Fish {
                        Place::Fish
                    } else {
                        Place::Sell
                    },
                    &mut steps,
                )?;
            }
            Task::RecordFish | Task::RecordSell => {
                self.stick(0, -1, 320, false)?;
                self.base(&mut steps)?;
            }
            Task::StickTest => {
                for (x, y) in [(0, -1), (-1, 0), (0, 1), (1, 0)] {
                    steps.push_back(Step::Input(self.stick(x, y, 800, false)?, 0.4));
                }
            }
            Task::JumpTest => {
                let point = *self
                    .positions
                    .get("jump_btn")
                    .ok_or("점프 버튼을 먼저 지정하세요.")?;
                tap(&mut steps, point, 0.5);
                steps.push_back(Step::Input(self.stick(0, -1, 600, true)?, 0.0));
            }
            Task::Sell => {
                let [dialog, sell_fish, first, all, confirm, close]: [Point; 6] = self
                    .points(&SELL)
                    .ok_or("판매 메뉴에서 판매 위치 6개를 먼저 지정하세요.")?
                    .try_into()
                    .unwrap();
                let mut back = VecDeque::new();
                self.base(&mut back)?;
                self.walk(Place::Fish, &mut back)?;
                self.base(&mut steps)?;
                self.walk(Place::Sell, &mut steps)?;
                self.align(&mut steps)?;
                if self.desktop.is_some() {
                    key(&mut steps, &[0x45], 40, 0.35);
                } else {
                    let talk = *self
                        .positions
                        .get("talk_btn")
                        .ok_or("대화 버튼을 먼저 지정하세요.")?;
                    tap(&mut steps, talk, 0.35);
                }
                steps.push_back(Step::Check(Check::Dialog(0)));
                job.sale = Some(Sale {
                    dialog,
                    sell_fish,
                    first,
                    all,
                    confirm,
                    close,
                    back,
                    delay: self.options.sell_delay,
                    max: self.options.sell_max,
                    sold: 0,
                });
            }
        }
        if action == Task::Sell {
            job.sale_start = steps.clone();
        }
        job.steps = steps;
        Ok(job)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Task {
    Base,
    Fish,
    Shop,
    RecordFish,
    RecordSell,
    StickTest,
    JumpTest,
    Sell,
    Measure,
}

impl Task {
    pub fn record(self) -> Option<Place> {
        match self {
            Self::RecordFish => Some(Place::Fish),
            Self::RecordSell => Some(Place::Sell),
            _ => None,
        }
    }
}

#[derive(Clone)]
pub enum Motion {
    Trace {
        center: Point,
        edge: Point,
        jump: Option<Point>,
        events: Vec<TraceEvent>,
    },
    Tap(Point, u32),
    Swipe,
    Pinch,
    Keys(&'static [u16], u32),
    Click {
        point: Point,
        right: bool,
    },
    Tilt {
        pixels: i32,
        ms: u32,
    },
    Stick {
        center: Point,
        edge: Point,
        x: i8,
        y: i8,
        jump: Option<Point>,
        ms: u32,
    },
}

impl Motion {
    fn seconds(&self) -> f64 {
        f64::from(match self {
            Self::Trace { events, .. } => events.last().map_or(0, |event| event.t.ceil() as u32),
            Self::Tap(_, ms) | Self::Stick { ms, .. } => *ms,
            Self::Keys(_, ms) | Self::Tilt { ms, .. } => *ms,
            Self::Click { .. } => 40,
            Self::Swipe => 250,
            Self::Pinch => 300,
        }) / 1000.0
    }
}

#[derive(Clone)]
enum Step {
    Input(Motion, f64),
    Wait(f64),
    Check(Check),
}
#[derive(Clone, Copy)]
enum Check {
    Dialog(u8),
    Shop,
    Before,
    Confirm { before: f64, tries: u8, until: f64 },
    Closed { before: f64, tries: u8 },
}

struct Sale {
    dialog: Point,
    sell_fish: Point,
    first: Point,
    all: Point,
    confirm: Point,
    close: Point,
    back: VecDeque<Step>,
    delay: f64,
    max: u32,
    sold: u32,
}

pub enum Tick {
    Input(Motion),
    Wait,
    Done {
        resume: bool,
        message: &'static str,
        sold: Option<u32>,
    },
    Failed(&'static str),
}

pub struct Job {
    geometry: Option<(usize, usize)>,
    pub action: Task,
    pub started: bool,
    pub ready_at: f64,
    pub deadline: f64,
    pub recording_at: Option<f64>,
    steps: VecDeque<Step>,
    sale: Option<Sale>,
    sale_start: VecDeque<Step>,
    attempts: u8,
    resume: bool,
}

impl Job {
    fn new(action: Task, now: f64, resume: bool) -> Self {
        Self {
            action,
            geometry: None,
            started: false,
            ready_at: now,
            deadline: now + 30.0,
            recording_at: None,
            steps: VecDeque::new(),
            sale: None,
            sale_start: VecDeque::new(),
            attempts: 1,
            resume,
        }
    }

    pub fn message(&self) -> &'static str {
        match (self.action, self.recording_at) {
            (Task::Measure, Some(_)) => "시간 재는 중 · 도착하면 F6을 누르세요.",
            (_, Some(_)) => "길 기록 중 · 이동 버튼을 누르고 도착하면 저장하세요.",
            (Task::Sell, _) if self.attempts == 2 => "판매 실패 · 처음부터 다시 이동 중 (2/3)",
            (Task::Sell, _) if self.attempts == 3 => "판매 실패 · 처음부터 다시 이동 중 (3/3)",
            (Task::Sell, _) => "판매 · 이동 또는 판매 중",
            (Task::StickTest, _) => "조이스틱 테스트 중",
            (Task::JumpTest, _) => "점프 테스트 중",
            _ => "이동 중",
        }
    }

    pub fn tick(&mut self, now: f64, frame: &Frame<'_>) -> Tick {
        self.started = true;
        let geometry = (frame.width, frame.height);
        if self.geometry.is_some_and(|previous| previous != geometry) {
            return Tick::Failed("화면 크기가 바뀌어 이동 · 판매를 중지했습니다.");
        }
        self.geometry = Some(geometry);
        if self.recording_at.is_some_and(|start| now - start >= 300.0) {
            return Tick::Failed("길 기록 시간이 지나 중지했습니다.");
        }
        if now < self.ready_at {
            return Tick::Wait;
        }
        let Some(step) = self.steps.pop_front() else {
            if self.action.record().is_some() || self.action == Task::Measure {
                self.recording_at.get_or_insert(now);
                return Tick::Wait;
            }
            return Tick::Done {
                resume: self.resume,
                sold: self.sale.as_ref().map(|sale| sale.sold),
                message: match self.action {
                    Task::Sell => "판매 후 낚시 장소로 돌아왔습니다.",
                    _ => "이동 동작이 끝났습니다.",
                },
            };
        };
        match step {
            Step::Input(motion, after) => {
                self.ready_at = now + motion.seconds() + after;
                Tick::Input(motion)
            }
            Step::Wait(seconds) => {
                self.ready_at = now + seconds;
                Tick::Wait
            }
            Step::Check(check) => self.check(now, frame, check),
        }
    }

    fn check(&mut self, now: f64, frame: &Frame<'_>, check: Check) -> Tick {
        let sale = self
            .sale
            .as_mut()
            .expect("sale steps require sale positions");
        match check {
            Check::Dialog(tries) => {
                if frame.sale_color(sale.sell_fish, false) >= 0.035 {
                    tap(&mut self.steps, sale.sell_fish, 0.8 + sale.delay);
                    self.steps.push_back(Step::Check(Check::Shop));
                } else if tries < 16 {
                    tap(&mut self.steps, sale.dialog, 0.35);
                    self.steps.push_back(Step::Check(Check::Dialog(tries + 1)));
                } else {
                    return self
                        .retry_sale("판매 3회 실패 · 대화창과 Sell Fish 위치를 확인하세요.");
                }
            }
            Check::Shop => {
                if frame.sale_color(sale.all, false) < 0.035 {
                    return self.retry_sale("판매 3회 실패 · Sell All과 상점 위치를 확인하세요.");
                }
                tap(&mut self.steps, sale.first, 0.5 + sale.delay);
                self.steps.push_back(Step::Check(Check::Before));
            }
            Check::Before => {
                let before = frame.sale_color(sale.confirm, true);
                tap(&mut self.steps, sale.all, 0.2 + sale.delay);
                self.steps.push_back(Step::Check(Check::Confirm {
                    before,
                    tries: 1,
                    until: now + 2.5 + sale.delay,
                }));
            }
            Check::Confirm {
                before,
                tries,
                until,
            } => {
                if confirmed(frame, sale.confirm, before) {
                    tap(&mut self.steps, sale.confirm, 1.2 + sale.delay);
                    self.steps
                        .push_back(Step::Check(Check::Closed { before, tries: 1 }));
                } else if now < until {
                    self.steps.push_back(Step::Check(check));
                } else if tries < 3 && frame.sale_color(sale.all, false) >= 0.035 {
                    tap(&mut self.steps, sale.all, 0.2 + sale.delay);
                    self.steps.push_back(Step::Check(Check::Confirm {
                        before,
                        tries: tries + 1,
                        until: now + 2.5 + sale.delay,
                    }));
                } else if frame.sale_color(sale.all, false) >= 0.035 {
                    self.end_sale();
                } else {
                    return self
                        .retry_sale("판매 3회 실패 · 판매 장소 길과 상점 위치를 확인하세요.");
                }
            }
            Check::Closed { before, tries } => {
                if confirmed(frame, sale.confirm, before) {
                    if tries >= 3 {
                        return self.retry_sale("판매 3회 실패 · 확인 버튼 위치를 확인하세요.");
                    }
                    tap(&mut self.steps, sale.confirm, 1.2 + sale.delay);
                    self.steps.push_back(Step::Check(Check::Closed {
                        before,
                        tries: tries + 1,
                    }));
                } else {
                    sale.sold += 1;
                    if sale.sold >= sale.max {
                        self.end_sale();
                    } else {
                        self.steps.push_back(Step::Check(Check::Shop));
                    }
                }
            }
        }
        Tick::Wait
    }

    pub fn retry_input(&mut self) -> bool {
        if self.action != Task::Sell || self.attempts >= 3 {
            return false;
        }
        self.attempts += 1;
        self.steps = self.sale_start.clone();
        self.ready_at = 0.0;
        true
    }

    pub fn resumes_fishing(&self) -> bool {
        self.action == Task::Fish && self.resume
    }

    fn retry_sale(&mut self, error: &'static str) -> Tick {
        if self.retry_input() {
            Tick::Wait
        } else {
            Tick::Failed(error)
        }
    }

    fn end_sale(&mut self) {
        let sale = self.sale.as_mut().unwrap();
        tap(&mut self.steps, sale.close, 0.5 + sale.delay);
        self.steps.extend(sale.back.iter().cloned());
    }
}

fn tap(queue: &mut VecDeque<Step>, point: Point, after: f64) {
    queue.push_back(Step::Input(Motion::Tap(point, 150), after));
}

fn confirmed(frame: &Frame<'_>, point: Point, baseline: f64) -> bool {
    let color = frame.sale_color(point, true);
    color >= 0.04 && color >= baseline + 0.03
}

fn millis(seconds: f64) -> u32 {
    (seconds * 1000.0).round() as u32
}

fn key(queue: &mut VecDeque<Step>, keys: &'static [u16], ms: u32, after: f64) {
    if ms > 0 {
        queue.push_back(Step::Input(Motion::Keys(keys, ms), after));
    } else if after > 0.0 {
        queue.push_back(Step::Wait(after));
    }
}

fn valid_point(point: &Point) -> bool {
    (0.0..1.0).contains(&point.0) && (0.0..1.0).contains(&point.1)
}

#[derive(Clone, Deserialize)]
#[serde(default)]
pub struct DesktopMove {
    reset_wait: f64,
    w_time: f64,
    wa_time: f64,
    a_time: f64,
    w2_time: f64,
    sd_time: f64,
    o_time: f64,
    tilt_px: i32,
    margin: f64,
    button: String,
    places: Vec<DesktopPlace>,
}

impl Default for DesktopMove {
    fn default() -> Self {
        Self {
            reset_wait: 3.5,
            w_time: 1.0,
            wa_time: 7.0,
            a_time: 1.0,
            w2_time: 0.25,
            sd_time: 0.25,
            o_time: 2.5,
            tilt_px: 800,
            margin: 0.3,
            button: "right".into(),
            places: Vec::new(),
        }
    }
}

#[derive(Clone, Deserialize)]
struct DesktopPlace {
    key: String,
    points: Vec<Waypoint>,
}

#[derive(Clone, Deserialize)]
pub struct Waypoint {
    pos: Option<Point>,
    time: Option<f64>,
}

impl DesktopMove {
    fn waypoints(&self, place: Place) -> &[Waypoint] {
        let key = match place {
            Place::Fish => "fish_spot",
            Place::Sell => "sell_spot",
        };
        self.places
            .iter()
            .find(|place| place.key == key)
            .map_or(&[], |place| &place.points)
    }

    fn validate(&self) -> bool {
        [
            self.reset_wait,
            self.w_time,
            self.wa_time,
            self.a_time,
            self.w2_time,
            self.sd_time,
            self.o_time,
        ]
        .into_iter()
        .all(|v| (0.0..=30.0).contains(&v))
            && (0..=3000).contains(&self.tilt_px)
            && (0.0..=5.0).contains(&self.margin)
    }
}

#[derive(Deserialize)]
pub struct DesktopSetup {
    pub movement: DesktopMove,
    pub positions: BTreeMap<String, Option<Point>>,
    pub sell_max: u32,
    pub sell_delay: f64,
}

impl Navigation {
    pub fn desktop(setup: DesktopSetup) -> Result<Self, &'static str> {
        if !setup.movement.validate() {
            return Err("이동 시간이나 카메라 설정을 확인하세요.");
        }
        let options = Options {
            sell_max: setup.sell_max,
            sell_delay: setup.sell_delay,
            ..Options::default()
        }
        .validate()?;
        let positions = setup
            .positions
            .into_iter()
            .filter_map(|(k, point)| point.filter(valid_point).map(|p| (k, p)))
            .collect();
        let mut navigation = Self {
            options,
            positions,
            desktop: Some(setup.movement),
            ..Self::default()
        };
        navigation.options.use_move = navigation.job(Task::Fish, 0.0, true).is_ok();
        navigation.options.use_sell = navigation.job(Task::Sell, 0.0, true).is_ok();
        Ok(navigation)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saved_and_picked_positions_are_kept_on_screen() {
        let mut nav: Navigation = serde_json::from_str(
            r#"{"positions":{"stick_center":[-0.2,1.5],"jump_btn":[1.2,-1.0]}}"#,
        )
        .unwrap();
        nav.fit_positions();
        assert_eq!(nav.positions["stick_center"].0, 0.0);
        assert_eq!(nav.positions["stick_center"].1, 0.9999);
        nav.set_point("jump_btn", Point(1.0, -0.1)).unwrap();
        assert_eq!(nav.positions["jump_btn"].0, 0.9999);
        assert_eq!(nav.positions["jump_btn"].1, 0.0);
        assert!(nav.set_point("jump_btn", Point(f64::NAN, 0.1)).is_err());
    }

    #[test]
    fn failed_sales_restart_the_full_route_and_stop_on_the_third_failure() {
        let mut nav = Navigation::default();
        for key in BASE
            .into_iter()
            .chain(SELL)
            .chain(["stick_center", "stick_edge", "talk_btn"])
        {
            nav.positions.insert(key.into(), Point(0.2, 0.7));
        }
        nav.positions.insert("stick_edge".into(), Point(0.25, 0.7));
        nav.sell.push(Stride {
            x: 0,
            y: -1,
            ms: 500,
            jump: false,
        });
        nav.fish.push(Stride {
            x: 0,
            y: -1,
            ms: 500,
            jump: false,
        });
        let mut job = nav.job(Task::Sell, 0.0, true).unwrap();
        let pixels = vec![0; 100 * 100 * 4];
        let frame = Frame::new(&pixels, 100, 100, 400, 4, false).unwrap();
        job.ready_at = 1000.0;
        assert!(job.retry_input());
        assert_eq!(job.ready_at, 0.0);
        assert!(job.retry_input());
        assert!(!job.retry_input());
        let mut job = nav.job(Task::Sell, 0.0, true).unwrap();
        let mut failed = false;
        for step in 0..500 {
            if matches!(job.tick(step as f64 * 10.0, &frame), Tick::Failed(_)) {
                failed = true;
                break;
            }
        }
        assert!(failed);
        assert_eq!(job.attempts, 3);
        assert!(!job.sale_start.is_empty());
    }

    #[test]
    fn sale_confirmation_requires_a_new_green_button() {
        let mut pixels = vec![0; 100 * 100 * 4];
        let point = Point(0.5, 0.5);
        let empty = Frame::new(&pixels, 100, 100, 400, 4, false).unwrap();
        assert!(!confirmed(&empty, point, 0.0));
        for y in 48..52 {
            for x in 46..54 {
                pixels[(y * 100 + x) * 4 + 1] = 180;
            }
        }
        let opened = Frame::new(&pixels, 100, 100, 400, 4, false).unwrap();
        assert!(confirmed(&opened, point, 0.0));
        assert!(!confirmed(&opened, point, opened.sale_color(point, true)));
    }

    #[test]
    fn a_running_route_stops_when_the_capture_dimensions_change() {
        let mut nav = Navigation::default();
        nav.set_point("stick_center", Point(0.12, 0.8)).unwrap();
        nav.set_point("stick_edge", Point(0.18, 0.8)).unwrap();
        let pixels = vec![0; 32 * 32 * 4];
        let first = Frame::new(&pixels, 32, 32, 128, 4, false).unwrap();
        let rotated = Frame::new(&pixels, 16, 32, 128, 4, false).unwrap();
        let mut job = nav.job(Task::StickTest, 0.0, false).unwrap();
        assert!(matches!(
            job.tick(0.0, &first),
            Tick::Input(Motion::Stick { .. })
        ));
        assert!(matches!(job.tick(0.9, &rotated), Tick::Failed(_)));
    }
    #[test]
    fn trace_roundtrip_preserves_press_release_times_and_old_routes() {
        let trace = normalize_trace(vec![
            TraceEvent {
                k: 'X',
                t: 50.0,
                dx: 0.0,
                dy: 0.0,
                a: false,
                b: false,
            },
            TraceEvent {
                k: 'F',
                t: 100.0,
                dx: 0.0,
                dy: -1.0,
                a: true,
                b: false,
            },
            TraceEvent {
                k: 'U',
                t: 1367.4,
                dx: 0.0,
                dy: 0.0,
                a: true,
                b: false,
            },
        ])
        .unwrap();
        assert_eq!(trace.len(), 2);
        assert_eq!(trace[0].t, 0.0);
        assert_eq!(trace[1].t, 1267.4);
        let mut nav = Navigation::default();
        nav.fish_trace = trace;
        let restored: Navigation =
            serde_json::from_slice(&serde_json::to_vec(&nav).unwrap()).unwrap();
        assert_eq!(restored.fish_trace[1].t, 1267.4);
        assert_eq!(restored.route_parts(Place::Fish), 1);
        let old: Navigation =
            serde_json::from_str(r#"{"fish":[{"x":0,"y":-1,"ms":500,"jump":false}]}"#).unwrap();
        assert_eq!(old.route_parts(Place::Fish), 1);
        assert!(old.fish_trace.is_empty());
    }
    #[test]
    fn trace_rejects_invalid_data_and_releases_held_fingers() {
        let trace = normalize_trace(vec![TraceEvent {
            k: 'F',
            t: 0.0,
            dx: 0.0,
            dy: -1.0,
            a: true,
            b: true,
        }])
        .unwrap();
        assert_eq!(trace.last().unwrap().k, 'X');
        assert!(normalize_trace(vec![TraceEvent {
            k: 'F',
            t: f64::NAN,
            dx: 0.0,
            dy: -1.0,
            a: true,
            b: false
        }])
        .is_err());
    }
}

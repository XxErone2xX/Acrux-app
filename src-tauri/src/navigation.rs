use crate::ocr::Word;
#[cfg(target_os = "android")]
use crate::vision::Rect;
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
    pub(crate) reset_wait: f64,
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

    pub fn set_point(&mut self, key: &str, point: Point) -> Result<(), &'static str> {
        let allowed = BASE.contains(&key)
            || SELL.contains(&key)
            || [
                "stick_center",
                "stick_edge",
                "jump_btn",
                "talk_btn",
                "game_menu",
                "private_server_commands",
                "ps_menu_close",
                "ps_panel_close",
            ]
            .contains(&key);
        if !allowed || !(0.0..1.0).contains(&point.0) || !(0.0..1.0).contains(&point.1) {
            return Err("저장할 위치를 확인하세요.");
        }
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
        if self.desktop.is_some() {
            return self.desktop_base(queue, false);
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
            let key = match place {
                Place::Fish => "fish_spot",
                Place::Sell => "sell_spot",
            };
            let index = desktop
                .places
                .iter()
                .position(|place| place.key == key)
                .ok_or("이동할 장소를 찾을 수 없습니다.")?;
            return self.desktop_walk(index, queue);
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

    fn desktop_base(&self, queue: &mut VecDeque<Step>, reverse: bool) -> Result<(), &'static str> {
        let desktop = self.desktop.as_ref().ok_or("PC 이동 설정이 없습니다.")?;
        queue.push_back(Step::Wait(0.2));
        key(queue, &[0x1b], 40, 0.5);
        key(queue, &[0x52], 40, 0.5);
        key(queue, &[0x0d], 40, desktop.reset_wait.max(0.5));
        self.align(queue)?;
        if !reverse {
            key(queue, &[0x57], millis(desktop.w_time), 0.0);
            key(queue, &[0x57, 0x41], millis(desktop.wa_time), 0.0);
            overlap(queue, 0x41, desktop.a_time, 0x57, desktop.w2_time);
            key(queue, &[0x53, 0x44], millis(desktop.sd_time), 0.5);
        }
        queue.push_back(Step::Input(
            Motion::Tilt {
                pixels: desktop.tilt_px,
                ms: millis(desktop.o_time),
            },
            0.3,
        ));
        if reverse {
            key(queue, &[0x53, 0x44], millis(desktop.rsd_time), 0.0);
            overlap(queue, 0x53, desktop.rs_time, 0x41, desktop.ra_time);
            queue.push_back(Step::Wait(0.5));
        }
        Ok(())
    }

    fn desktop_walk(&self, index: usize, queue: &mut VecDeque<Step>) -> Result<(), &'static str> {
        let desktop = self.desktop.as_ref().ok_or("PC 이동 설정이 없습니다.")?;
        let place = desktop
            .places
            .get(index)
            .ok_or("이동할 장소를 찾을 수 없습니다.")?;
        let route = place
            .route
            .as_ref()
            .ok_or("장소의 길을 먼저 기록하세요.")?
            .normalized()?;
        queue.push_back(Step::Replay(route, desktop.margin));
        Ok(())
    }

    pub fn desktop_move_place(&self, index: usize, now: f64) -> Result<Job, &'static str> {
        let desktop = self.desktop.as_ref().ok_or("PC 이동 설정이 없습니다.")?;
        let place = desktop
            .places
            .get(index)
            .ok_or("이동할 장소를 찾을 수 없습니다.")?;
        let mut job = Job::new(Task::Base, now, false);
        self.desktop_base(&mut job.steps, place.feat == "mcraft")?;
        self.desktop_walk(index, &mut job.steps)?;
        Ok(job)
    }

    pub fn desktop_record(&self, index: usize, now: f64) -> Result<Job, &'static str> {
        let desktop = self.desktop.as_ref().ok_or("PC 이동 설정이 없습니다.")?;
        let place = desktop
            .places
            .get(index)
            .ok_or("길을 기록할 장소를 찾을 수 없습니다.")?;
        let mut job = Job::new(Task::Measure, now, false);
        self.desktop_base(&mut job.steps, place.feat == "mcraft")?;
        job.desktop_record = Some(DesktopRecording {
            index,
            ..DesktopRecording::default()
        });
        Ok(job)
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
            Task::BaseRev => self.desktop_base(&mut steps, true)?,
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
            Task::CalibrateSale => {
                job.deadline = now + 120.;
                job.sale_draft = Some(self.positions.clone());
                steps.push_back(Step::Check(Check::Calibrate(CalibrationStage::Dialog)));
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
                    #[cfg(target_os = "android")]
                    ocr_at: f64::NEG_INFINITY,
                });
            }
        }
        job.steps = steps;
        Ok(job)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Task {
    Base,
    BaseRev,
    Fish,
    Shop,
    RecordFish,
    RecordSell,
    StickTest,
    JumpTest,
    Sell,
    CalibrateSale,
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
    KeysOverlap {
        main: u16,
        extra: u16,
        ms: u32,
        extra_at: u32,
    },
    KeyEvent {
        vk: u16,
        down: bool,
    },
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
            Self::Keys(_, ms) | Self::KeysOverlap { ms, .. } | Self::Tilt { ms, .. } => *ms,
            Self::KeyEvent { .. } => 0,
            Self::Click { .. } => 40,
            Self::Swipe => 250,
            Self::Pinch => 300,
        }) / 1000.0
    }
}

enum Step {
    Input(Motion, f64),
    Wait(f64),
    Check(Check),
    Replay(DesktopRoute, f64),
}
#[derive(Clone, Copy)]
enum Check {
    Dialog(u8),
    Shop,
    Before,
    Confirm { before: f64, tries: u8, until: f64 },
    Closed { before: f64, tries: u8 },
    Calibrate(CalibrationStage),
}

#[derive(Clone, Copy)]
enum CalibrationStage {
    Dialog,
    Shop(u8),
    Confirm(u8),
    Close(u8),
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
    #[cfg(target_os = "android")]
    ocr_at: f64,
}

pub enum Tick {
    Input(Motion),
    Observe,
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
    sale_draft: Option<BTreeMap<String, Point>>,
    resume: bool,
    desktop_record: Option<DesktopRecording>,
    replay: Option<Replay>,
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
            sale_draft: None,
            resume,
            desktop_record: None,
            replay: None,
        }
    }

    pub fn message(&self) -> &'static str {
        if self.desktop_recording() {
            return "길 기록 중 · WASD·Space로 걸어가고 도착하면 F6 또는 도착을 누르세요.";
        }
        match (self.action, self.recording_at) {
            (Task::Measure, Some(_)) => "시간 재는 중 · 도착하면 F6을 누르세요.",
            (_, Some(_)) => "길 기록 중 · 이동 버튼을 누르고 도착하면 저장하세요.",
            (Task::Sell, _) => "판매 · 이동 또는 판매 중",
            (Task::CalibrateSale, _) => match self.steps.front() {
                Some(Step::Check(Check::Calibrate(CalibrationStage::Dialog))) => {
                    "Captain Flarg 앞에서 대화 버튼을 눌러 대화를 여세요"
                }
                _ => "판매 화면 보정 중",
            },
            (Task::StickTest, _) => "조이스틱 테스트 중",
            (Task::JumpTest, _) => "점프 테스트 중",
            _ => "이동 중",
        }
    }

    pub fn desktop_recording(&self) -> bool {
        self.desktop_record.is_some() && self.recording_at.is_some()
    }

    pub fn recording_status(&self, now: f64) -> Option<(usize, f64)> {
        Some((
            self.desktop_record.as_ref()?.index,
            (now - self.recording_at?).max(0.),
        ))
    }

    pub fn record_desktop(&mut self, keys: &[u16], left: bool, point: Point, now: f64) {
        if !self.desktop_recording() || !now.is_finite() {
            return;
        }
        if let Some(record) = self.desktop_record.as_mut() {
            record.sample(keys, left, point, now);
        }
    }

    pub fn finish_desktop_record(
        &mut self,
        now: f64,
    ) -> Result<Option<(usize, serde_json::Value)>, &'static str> {
        if !self.desktop_recording() {
            return Ok(None);
        }
        if !now.is_finite() || now - self.recording_at.unwrap() > 300. {
            return Err("길 기록 시간이 지나 저장하지 않았습니다.");
        }
        let record = self.desktop_record.as_mut().unwrap();
        let route = record.finish(now)?;
        Ok(Some((
            record.index,
            serde_json::to_value(route).map_err(|_| "길 기록을 저장하지 못했습니다.")?,
        )))
    }

    pub fn take_sale_draft(&mut self) -> Option<BTreeMap<String, Point>> {
        self.sale_draft.take()
    }

    pub fn needs_frame(&self) -> bool {
        self.replay.is_none() && matches!(self.steps.front(), Some(Step::Check(_)))
    }

    pub fn tick(&mut self, now: f64, frame: &Frame<'_>) -> Tick {
        self.tick_inner(now, (frame.width, frame.height), Some(frame), &[])
    }

    #[cfg(target_os = "android")]
    pub fn sale_ocr(&mut self, now: f64, frame: &Frame<'_>) -> Option<Rect> {
        if now < self.ready_at {
            return None;
        }
        let Step::Check(check) = self.steps.front()? else {
            return None;
        };
        if matches!(check, Check::Calibrate(_)) {
            return Some(Rect(0.05, 0.12, 0.89, 0.94));
        }
        let sale = self.sale.as_mut()?;
        if now < sale.ocr_at + 0.5 {
            return None;
        }
        let region = match *check {
            Check::Dialog(_) if frame.sale_color(sale.sell_fish, false) < 0.035 => {
                Frame::centered(sale.sell_fish, 0.50, 0.30)
            }
            Check::Shop if frame.sale_color(sale.all, false) < 0.035 => {
                Frame::centered(sale.all, 0.52, 0.28)
            }
            Check::Confirm { before, .. } | Check::Closed { before, .. }
                if !confirmed(frame, sale.confirm, before) =>
            {
                Rect(
                    0.18_f64.min(sale.all.0 - 0.1),
                    0.23,
                    0.82_f64.max(sale.all.0 + 0.1),
                    0.76_f64.max(sale.all.1 + 0.08),
                )
            }
            _ => return None,
        };
        sale.ocr_at = now;
        Some(Rect(
            region.0.max(0.),
            region.1.max(0.),
            region.2.min(1.),
            region.3.min(1.),
        ))
    }

    #[cfg(target_os = "android")]
    pub fn tick_with_words(&mut self, now: f64, frame: &Frame<'_>, words: &[Word]) -> Tick {
        self.tick_inner(now, (frame.width, frame.height), Some(frame), words)
    }

    pub fn tick_without_frame(&mut self, now: f64, geometry: (usize, usize)) -> Tick {
        self.tick_inner(now, geometry, None, &[])
    }

    fn tick_inner(
        &mut self,
        now: f64,
        geometry: (usize, usize),
        frame: Option<&Frame<'_>>,
        words: &[Word],
    ) -> Tick {
        if !now.is_finite() || now < 0. || geometry.0 == 0 || geometry.1 == 0 {
            return Tick::Failed("이동 시각이나 화면 크기를 확인할 수 없습니다.");
        }
        self.started = true;
        if self.geometry.is_some_and(|previous| previous != geometry) {
            return Tick::Failed("화면 크기가 바뀌어 이동 · 판매를 중지했습니다.");
        }
        self.geometry = Some(geometry);
        if self.recording_at.is_some_and(|start| now - start >= 300.)
            || self
                .desktop_record
                .as_ref()
                .is_some_and(|record| record.ev.len() > 30000)
        {
            return Tick::Failed("길 기록 시간이 지나 중지했습니다.");
        }
        if now < self.ready_at {
            return Tick::Wait;
        }
        if let Some(replay) = self.replay.as_mut() {
            if let Some(event) = replay.route.ev.get(replay.next) {
                if now < replay.started + event.at() {
                    return Tick::Wait;
                }
                replay.next += 1;
                return Tick::Input(event.motion());
            }
            if now < replay.started + replay.route.duration() {
                return Tick::Wait;
            }
            self.ready_at = now + replay.margin;
            self.replay = None;
            return Tick::Wait;
        }
        if self.needs_frame() && frame.is_none() {
            return Tick::Observe;
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
                    Task::CalibrateSale => "판매 위치 보정을 완료했습니다.",
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
            Step::Check(check) => self.check(
                now,
                frame.expect("visual steps require a frame"),
                check,
                words,
            ),
            Step::Replay(route, margin) => {
                self.replay = Some(Replay {
                    route,
                    next: 0,
                    started: now,
                    margin,
                });
                self.tick_inner(now, geometry, frame, words)
            }
        }
    }

    fn check(&mut self, now: f64, frame: &Frame<'_>, check: Check, words: &[Word]) -> Tick {
        if let Check::Calibrate(stage) = check {
            return self.calibrate_sale(now, frame, words, stage);
        }
        let sale = self
            .sale
            .as_mut()
            .expect("sale steps require sale positions");
        match check {
            Check::Dialog(tries) => {
                let button = (frame.sale_color(sale.sell_fish, false) >= 0.035)
                    .then_some(sale.sell_fish)
                    .or_else(|| sale_label(words, "sellflsh", sale.sell_fish));
                if let Some(button) = button {
                    tap(&mut self.steps, button, 0.8 + sale.delay);
                    self.steps.push_back(Step::Check(Check::Shop));
                } else if tries < 16 {
                    tap(&mut self.steps, sale.dialog, 0.35);
                    self.steps.push_back(Step::Check(Check::Dialog(tries + 1)));
                } else {
                    return Tick::Failed(
                        "Sell Fish를 찾지 못했습니다. 대화창과 버튼 위치를 확인하세요.",
                    );
                }
            }
            Check::Shop => {
                if frame.sale_color(sale.all, false) < 0.035 {
                    let Some(button) = sell_all_label(words, sale.all) else {
                        return Tick::Failed("Sell All을 찾지 못했습니다. 상점 위치를 확인하세요.");
                    };
                    sale.all = button;
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
                let confirm = (confirmed(frame, sale.confirm, before))
                    .then_some(sale.confirm)
                    .or_else(|| confirmation_labels(words));
                let shop = frame.sale_color(sale.all, false) >= 0.035
                    || sell_all_label(words, sale.all).is_some();
                if let Some(point) = confirm {
                    sale.confirm = point;
                    tap(&mut self.steps, point, 1.2 + sale.delay);
                    self.steps
                        .push_back(Step::Check(Check::Closed { before, tries: 1 }));
                } else if now < until {
                    self.steps.push_back(Step::Check(check));
                } else if tries < 3 && shop {
                    tap(&mut self.steps, sale.all, 0.2 + sale.delay);
                    self.steps.push_back(Step::Check(Check::Confirm {
                        before,
                        tries: tries + 1,
                        until: now + 2.5 + sale.delay,
                    }));
                } else if shop {
                    self.end_sale();
                } else {
                    return Tick::Failed("판매 도중 상점을 찾지 못했습니다.");
                }
            }
            Check::Calibrate(_) => unreachable!(),
            Check::Closed { before, tries } => {
                let confirm = confirmed(frame, sale.confirm, before)
                    .then_some(sale.confirm)
                    .or_else(|| confirmation_labels(words));
                if let Some(point) = confirm {
                    if tries >= 3 {
                        return Tick::Failed(
                            "판매 확인창이 닫히지 않았습니다. 확인 버튼 위치를 확인하세요.",
                        );
                    }
                    tap(&mut self.steps, point, 1.2 + sale.delay);
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

    fn calibrate_sale(
        &mut self,
        now: f64,
        frame: &Frame<'_>,
        words: &[Word],
        stage: CalibrationStage,
    ) -> Tick {
        let positions = self.sale_draft.as_mut().unwrap();
        if now >= self.deadline {
            return Tick::Failed(
                "판매 위치 보정 시간이 지났습니다. Captain Flarg 앞에서 다시 시도하세요.",
            );
        }
        let word = |key: &str| {
            words
                .iter()
                .find(|word| sale_text(&word.text).contains(key))
        };
        let center = |word: &Word| word.rect.at(0.5, 0.5);
        let mut enqueue = |stage| self.steps.push_back(Step::Check(Check::Calibrate(stage)));
        self.ready_at = now + 0.55;
        match stage {
            CalibrationStage::Dialog => {
                if let Some(dialog) = word("cllcktosklp").or_else(|| word("whotdoyou")) {
                    let point = center(dialog);
                    positions.insert("dialog_pos".into(), point);
                    if word("sellflsh").is_none() {
                        enqueue(CalibrationStage::Dialog);
                        return Tick::Input(Motion::Tap(point, 40));
                    }
                }
                if let Some(button) = word("sellflsh").map(center).filter(|point| point.1 > 0.45) {
                    positions.insert("sell_fish_pos".into(), button);
                    enqueue(CalibrationStage::Shop(0));
                    return Tick::Input(Motion::Tap(button, 40));
                }
                enqueue(CalibrationStage::Dialog);
            }
            CalibrationStage::Shop(tries) => {
                let title = word("flshlngshop").map(center);
                let all = title.and_then(|title| {
                    let near = Point(0.26, (title.1 + 0.45).min(0.84));
                    sell_all_label(words, near).filter(|p| p.0 < 0.55 && p.1 > title.1 + 0.2)
                });
                if let (Some(title), Some(all)) = (title, all) {
                    let close = words
                        .iter()
                        .filter(|word| matches!(word.text.trim(), "X" | "x" | "×" | "✕" | "✖"))
                        .map(center)
                        .filter(|p| p.0 > title.0 && (p.1 - title.1).abs() < 0.08)
                        .max_by(|a, b| a.0.total_cmp(&b.0));
                    let first = words
                        .iter()
                        .filter_map(|word| {
                            let p = center(word);
                            let text = sale_text(&word.text);
                            (p.0 > 0.07
                                && p.0 < 0.72
                                && p.1 > title.1 + 0.10
                                && p.1 < all.1 - 0.035
                                && text.len() >= 3
                                && ![
                                    "sell",
                                    "buy",
                                    "shop",
                                    "flshlng",
                                    "sort",
                                    "seorch",
                                    "lnventory",
                                    "prlce",
                                ]
                                .iter()
                                .any(|key| text.contains(key)))
                            .then_some(Point(
                                p.0,
                                p.1 - (word.rect.3 - word.rect.1).clamp(0.01, 0.03),
                            ))
                        })
                        .min_by(|a, b| {
                            if (a.1 - b.1).abs() <= 0.025 {
                                a.0.total_cmp(&b.0)
                            } else {
                                a.1.total_cmp(&b.1)
                            }
                        });
                    let Some(first) = first.or_else(|| positions.get("first_fish_pos").copied())
                    else {
                        return Tick::Failed(
                            "첫 물고기 칸을 찾지 못했습니다. 물고기가 있을 때 다시 보정하세요.",
                        );
                    };
                    positions.insert("first_fish_pos".into(), first);
                    positions.insert("sell_all_pos".into(), all);
                    if let Some(close) = close {
                        positions.insert("shop_close_pos".into(), close);
                    }
                    tap(&mut self.steps, first, 0.5);
                    tap(&mut self.steps, all, 0.5);
                    self.steps
                        .push_back(Step::Check(Check::Calibrate(CalibrationStage::Confirm(0))));
                } else if tries < 8 {
                    enqueue(CalibrationStage::Shop(tries + 1));
                } else {
                    return Tick::Failed("Fishing Shop과 Sell All을 찾지 못했습니다.");
                }
            }
            CalibrationStage::Confirm(tries) => {
                if let Some((sell, cancel)) = sale_confirmation(words) {
                    positions.insert("confirm_sell_pos".into(), sell);
                    enqueue(CalibrationStage::Close(0));
                    return Tick::Input(Motion::Tap(cancel, 40));
                }
                if tries < 8 {
                    enqueue(CalibrationStage::Confirm(tries + 1));
                    if matches!(tries, 2 | 5) && word("flshlngshop").is_some() {
                        let all = positions["sell_all_pos"];
                        return Tick::Input(Motion::Tap(all, if tries == 2 { 70 } else { 150 }));
                    }
                } else if word("flshlngshop").is_some()
                    && word("sellconflrm").is_none()
                    && word("reollywonttosell").is_none()
                {
                    enqueue(CalibrationStage::Close(0));
                } else {
                    return Tick::Failed(
                        "판매 확인창을 확인하지 못했습니다. 화면을 직접 닫고 다시 시도하세요.",
                    );
                }
            }
            CalibrationStage::Close(tries) => {
                if sale_confirmation(words).is_some() {
                    return Tick::Failed(
                        "판매 확인창이 닫히지 않았습니다. 취소를 누른 뒤 다시 시도하세요.",
                    );
                }
                if word("flshlngshop").is_some() {
                    if let Some(close) = positions
                        .get("shop_close_pos")
                        .copied()
                        .filter(|_| tries < 2)
                    {
                        enqueue(CalibrationStage::Close(tries + 1));
                        return Tick::Input(Motion::Tap(close, 40));
                    }
                }
                // A blank OCR is accepted only after the configured close control was used.
                if tries == 0 && words.is_empty() && frame.width > 0 {
                    enqueue(CalibrationStage::Close(1));
                    return Tick::Wait;
                }
            }
        }
        Tick::Wait
    }

    fn end_sale(&mut self) {
        let sale = self.sale.as_mut().unwrap();
        tap(&mut self.steps, sale.close, 0.5 + sale.delay);
        self.steps.append(&mut sale.back);
    }
}

fn tap(queue: &mut VecDeque<Step>, point: Point, after: f64) {
    queue.push_back(Step::Input(Motion::Tap(point, 150), after));
}

fn sale_text(text: &str) -> String {
    text.chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|character| match character.to_ascii_lowercase() {
            'a' => 'o',
            'i' | '1' => 'l',
            ch => ch,
        })
        .collect()
}

fn sale_label(words: &[Word], key: &str, near: Point) -> Option<Point> {
    words
        .iter()
        .filter_map(|word| (sale_text(&word.text) == key).then(|| word.rect.at(0.5, 0.5)))
        .filter(|point| (point.0 - near.0).abs() < 0.21 && (point.1 - near.1).abs() < 0.13)
        .min_by(|a, b| {
            ((a.0 - near.0).abs() + (a.1 - near.1).abs())
                .total_cmp(&((b.0 - near.0).abs() + (b.1 - near.1).abs()))
        })
}

fn sell_all_label(words: &[Word], near: Point) -> Option<Point> {
    sale_label(words, "selloll", near).or_else(|| {
        let sell = sale_label(words, "sell", near)?;
        let all = sale_label(words, "oll", near)?;
        ((0. ..0.12).contains(&(all.0 - sell.0)) && (all.1 - sell.1).abs() <= 0.03)
            .then_some(Point((sell.0 + all.0) * 0.5, (sell.1 + all.1) * 0.5))
    })
}

fn confirmation_labels(words: &[Word]) -> Option<Point> {
    sale_confirmation(words).map(|(sell, _)| sell)
}

fn sale_confirmation(words: &[Word]) -> Option<(Point, Point)> {
    words.iter().find_map(|word| {
        if sale_text(&word.text) != "concel" {
            return None;
        }
        let cancel = word.rect.at(0.5, 0.5);
        words
            .iter()
            .filter_map(|word| {
                let sell = word.rect.at(0.5, 0.5);
                (sale_text(&word.text) == "sell"
                    && (sell.1 - cancel.1).abs() <= 0.045
                    && (0.035..0.35).contains(&(cancel.0 - sell.0)))
                .then_some(sell)
            })
            .max_by(|a, b| a.0.total_cmp(&b.0))
            .map(|sell| (sell, cancel))
    })
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

fn overlap(
    queue: &mut VecDeque<Step>,
    main: u16,
    main_seconds: f64,
    extra: u16,
    extra_seconds: f64,
) {
    let seconds = main_seconds.max(extra_seconds);
    if seconds > 0. {
        queue.push_back(Step::Input(
            Motion::KeysOverlap {
                main,
                extra,
                ms: millis(seconds),
                extra_at: millis((main_seconds - extra_seconds).max(0.)),
            },
            0.,
        ));
    }
}

const REC_KEYS: [(u16, &str); 5] = [
    (0x57, "w"),
    (0x41, "a"),
    (0x53, "s"),
    (0x44, "d"),
    (0x20, "space"),
];

#[derive(Clone, Deserialize, Serialize)]
#[serde(untagged)]
enum DesktopEvent {
    Key(f64, String, u8),
    Click(f64, String, Point),
}
impl DesktopEvent {
    fn at(&self) -> f64 {
        match self {
            Self::Key(at, ..) | Self::Click(at, ..) => *at,
        }
    }
    fn motion(&self) -> Motion {
        match self {
            Self::Key(_, name, down) => Motion::KeyEvent {
                vk: REC_KEYS.iter().find(|(_, key)| name == key).unwrap().0,
                down: *down == 1,
            },
            Self::Click(_, _, point) => Motion::Click {
                point: *point,
                right: false,
            },
        }
    }
}

#[derive(Clone, Deserialize, Serialize)]
struct DesktopRoute {
    ev: Vec<DesktopEvent>,
    len: f64,
}
impl DesktopRoute {
    fn duration(&self) -> f64 {
        self.len.max(self.ev.last().map_or(0., DesktopEvent::at))
    }
    fn normalized(&self) -> Result<Self, &'static str> {
        if self.ev.is_empty()
            || self.ev.len() > 30000
            || !self.len.is_finite()
            || !(0.0..=300.).contains(&self.len)
        {
            return Err("장소의 길을 먼저 기록하세요.");
        }
        let mut route = self.clone();
        let mut last = 0.;
        let mut held = [false; 5];
        for event in &route.ev {
            let at = event.at();
            if !at.is_finite() || at < last || at < 0. || at > self.len + 0.005 {
                return Err("길 기록의 입력 시각이 올바르지 않습니다.");
            }
            last = at;
            match event {
                DesktopEvent::Key(_, name, value) => {
                    let index = REC_KEYS
                        .iter()
                        .position(|(_, key)| name == key)
                        .ok_or("길 기록에 지원하지 않는 키가 있습니다.")?;
                    if *value > 1 {
                        return Err("길 기록의 키 누름 값이 올바르지 않습니다.");
                    }
                    held[index] = *value == 1;
                }
                DesktopEvent::Click(_, name, point) => {
                    if name != "click" || !valid_point(point) {
                        return Err("길 기록의 클릭 위치가 올바르지 않습니다.");
                    }
                }
            }
        }
        let end = route.duration();
        for (index, active) in held.into_iter().enumerate() {
            if active {
                route
                    .ev
                    .push(DesktopEvent::Key(end, REC_KEYS[index].1.into(), 0));
            }
        }
        Ok(route)
    }
}

struct Replay {
    route: DesktopRoute,
    next: usize,
    started: f64,
    margin: f64,
}
#[derive(Default)]
struct DesktopRecording {
    index: usize,
    initialized: bool,
    held: [bool; 5],
    left: bool,
    first: Option<f64>,
    ev: Vec<DesktopEvent>,
}
impl DesktopRecording {
    fn sample(&mut self, keys: &[u16], left: bool, point: Point, now: f64) {
        let held = REC_KEYS.map(|(vk, _)| keys.contains(&vk));
        if !self.initialized {
            self.held = held;
            self.left = left;
            self.initialized = true;
            return;
        }
        for (index, down) in held.into_iter().enumerate() {
            if self.held[index] == down {
                continue;
            }
            self.held[index] = down;
            if self.first.is_none() && !down {
                continue;
            }
            let first = *self.first.get_or_insert(now);
            self.ev.push(DesktopEvent::Key(
                ((now - first) * 1000.).round() / 1000.,
                REC_KEYS[index].1.into(),
                u8::from(down),
            ));
        }
        if left != self.left {
            self.left = left;
            if left && valid_point(&point) {
                let first = *self.first.get_or_insert(now);
                self.ev.push(DesktopEvent::Click(
                    ((now - first) * 1000.).round() / 1000.,
                    "click".into(),
                    point,
                ));
            }
        }
    }
    fn finish(&mut self, now: f64) -> Result<DesktopRoute, &'static str> {
        let first = self
            .first
            .ok_or("아무 입력도 없어 길을 저장하지 않았습니다.")?;
        if now < first || now - first > 300. || self.ev.is_empty() {
            return Err("길 기록의 시간을 확인하세요.");
        }
        let end = ((now - first) * 1000.).round() / 1000.;
        let mut ev = self.ev.clone();
        for (index, down) in self.held.into_iter().enumerate() {
            if down
                && ev.iter().any(
                    |event| matches!(event,DesktopEvent::Key(_,name,_) if name==REC_KEYS[index].1),
                )
            {
                ev.push(DesktopEvent::Key(end, REC_KEYS[index].1.into(), 0));
            }
        }
        DesktopRoute {
            ev,
            len: (end * 100.).round() / 100.,
        }
        .normalized()
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
    rsd_time: f64,
    rs_time: f64,
    ra_time: f64,
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
            rsd_time: 1.,
            rs_time: 6.,
            ra_time: 1.,
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
    #[serde(default)]
    feat: String,
    #[serde(default)]
    points: Vec<Waypoint>,
    #[serde(default)]
    route: Option<DesktopRoute>,
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
            self.rsd_time,
            self.rs_time,
            self.ra_time,
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

    fn desktop_nav(route: serde_json::Value, feat: &str) -> Navigation {
        Navigation::desktop(serde_json::from_value(serde_json::json!({
            "movement":{"places":[{"key":"fish_spot","feat":feat,"route":route,"points":[{"pos":[0.7,0.4],"time":1.}]}]},
            "positions":{"chat_pos":[0.1,0.1],"collection_pos":[0.2,0.2],"collection_close":[0.3,0.3]},
            "sell_max":1,"sell_delay":0.
        })).unwrap()).unwrap()
    }

    #[test]
    fn pc_routes_replay_absolute_timestamps_with_simultaneous_keys_and_left_clicks() {
        let route:DesktopRoute=serde_json::from_value(serde_json::json!({"ev":[[0.,"w",1],[0.1,"a",1],[0.2,"click",[0.7,0.4]],[0.3,"w",0],[0.3,"a",0]],"len":0.3})).unwrap();
        let mut job = Job::new(Task::Base, 10., false);
        job.steps
            .push_back(Step::Replay(route.normalized().unwrap(), 0.3));
        assert!(matches!(
            job.tick_without_frame(10., (1920, 1080)),
            Tick::Input(Motion::KeyEvent {
                vk: 0x57,
                down: true
            })
        ));
        assert!(matches!(
            job.tick_without_frame(10.05, (1920, 1080)),
            Tick::Wait
        ));
        assert!(matches!(
            job.tick_without_frame(10.12, (1920, 1080)),
            Tick::Input(Motion::KeyEvent {
                vk: 0x41,
                down: true
            })
        ));
        assert!(matches!(
            job.tick_without_frame(10.21, (1920, 1080)),
            Tick::Input(Motion::Click { right: false, .. })
        ));
        assert!(matches!(
            job.tick_without_frame(10.3, (1920, 1080)),
            Tick::Input(Motion::KeyEvent {
                vk: 0x57,
                down: false
            })
        ));
        assert!(matches!(
            job.tick_without_frame(10.3, (1920, 1080)),
            Tick::Input(Motion::KeyEvent {
                vk: 0x41,
                down: false
            })
        ));
        assert!(matches!(
            job.tick_without_frame(10.3, (1920, 1080)),
            Tick::Wait
        ));
        assert!(matches!(
            job.tick_without_frame(10.61, (1920, 1080)),
            Tick::Done { .. }
        ));
    }

    #[test]
    fn pc_recording_ignores_initial_holds_and_closes_remaining_keys_on_arrival() {
        let mut job = Job::new(Task::Measure, 100., false);
        job.desktop_record = Some(DesktopRecording {
            index: 2,
            ..DesktopRecording::default()
        });
        assert!(matches!(
            job.tick_without_frame(100., (1920, 1080)),
            Tick::Wait
        ));
        let point = Point(0.7, 0.4);
        job.record_desktop(&[0x57], true, point, 100.);
        job.record_desktop(&[], false, point, 100.1);
        assert!(job.finish_desktop_record(100.2).is_err());
        job.record_desktop(&[0x57], false, point, 101.);
        job.record_desktop(&[0x57, 0x41], false, point, 101.25);
        job.record_desktop(&[0x57, 0x41], true, point, 101.375);
        job.record_desktop(&[0x41], false, point, 101.5);
        let (index, route) = job.finish_desktop_record(102.).unwrap().unwrap();
        assert_eq!(index, 2);
        assert_eq!(
            route,
            serde_json::json!({"ev":[[0.,"w",1],[0.25,"a",1],[0.375,"click",[0.7,0.4]],[0.5,"w",0],[1.,"a",0]],"len":1.})
        );
        assert_eq!(job.recording_status(102.), Some((2, 2.)));
    }

    #[test]
    fn pc_recording_starts_from_the_same_normal_or_reverse_base_as_main() {
        let recorded = serde_json::json!({"ev":[[0.,"w",1],[1.,"w",0]],"len":1.});
        let normal = desktop_nav(recorded.clone(), "mfish")
            .desktop_record(0, 0.)
            .unwrap();
        assert!(normal.steps.iter().any(|step| matches!(
            step,
            Step::Input(
                Motion::KeysOverlap {
                    main: 0x41,
                    extra: 0x57,
                    ms: 1000,
                    extra_at: 750
                },
                _
            )
        )));
        let reverse = desktop_nav(recorded, "mcraft")
            .desktop_record(0, 0.)
            .unwrap();
        let tilt = reverse
            .steps
            .iter()
            .position(|step| matches!(step, Step::Input(Motion::Tilt { .. }, _)))
            .unwrap();
        let walk = reverse
            .steps
            .iter()
            .position(|step| matches!(step, Step::Input(Motion::Keys(&[0x53, 0x44], 1000), _)))
            .unwrap();
        assert!(tilt < walk);
        assert!(reverse.steps.iter().any(|step| matches!(
            step,
            Step::Input(
                Motion::KeysOverlap {
                    main: 0x53,
                    extra: 0x41,
                    ms: 6000,
                    extra_at: 5000
                },
                _
            )
        )));
        assert!(matches!(normal.steps.front(),Some(Step::Wait(delay)) if *delay==0.2));
    }

    #[test]
    fn pc_missing_or_invalid_routes_never_fall_back_to_timed_click_points() {
        let nav = desktop_nav(serde_json::Value::Null, "mfish");
        assert!(nav.desktop_move_place(0, 0.).is_err());
        assert!(nav.desktop_record(0, 0.).is_ok());
        for route in [
            serde_json::json!({"ev":[[0.,"q",1]],"len":1.}),
            serde_json::json!({"ev":[[0.,"w",2]],"len":1.}),
            serde_json::json!({"ev":[[0.5,"w",1],[0.1,"w",0]],"len":1.}),
            serde_json::json!({"ev":[[0.,"click",[1.,0.5]]],"len":1.}),
        ] {
            let route: DesktopRoute = serde_json::from_value(route).unwrap();
            assert!(route.normalized().is_err());
        }
    }

    #[test]
    fn mobile_sale_calibration_only_taps_cancel_in_the_confirmation() {
        let pixels = vec![0; 320 * 180 * 3];
        let frame = Frame::new(&pixels, 320, 180, 320 * 3, 3, false).unwrap();
        let word = |text: &str, x, y| Word {
            text: text.into(),
            rect: crate::vision::Rect(x, y, x, y),
        };
        let mut nav = Navigation::default();
        nav.set_point("confirm_sell_pos", Point(0.4, 0.6)).unwrap();
        let mut job = nav.job(Task::CalibrateSale, 0., false).unwrap();
        let _ = job.tick_inner(0., (320, 180), Some(&frame), &[word("Sell Fish", 0.5, 0.7)]);
        let shop = [
            word("Fishing Shop", 0.35, 0.24),
            word("Moonfish", 0.22, 0.46),
            word("Sell All", 0.26, 0.7),
            word("X", 0.68, 0.24),
        ];
        assert!(matches!(
            job.tick_inner(1., (320, 180), Some(&frame), &shop),
            Tick::Wait
        ));
        assert!(matches!(
            job.tick_inner(2., (320, 180), Some(&frame), &shop),
            Tick::Input(_)
        ));
        assert!(matches!(
            job.tick_inner(3., (320, 180), Some(&frame), &shop),
            Tick::Input(_)
        ));
        let modal = [word("Sell", 0.44, 0.6), word("Cancel", 0.58, 0.6)];
        assert!(
            matches!(job.tick_inner(4., (320, 180), Some(&frame), &modal),
            Tick::Input(Motion::Tap(Point(x, y), _)) if x == 0.58 && y == 0.6)
        );
        assert!(matches!(nav.positions["confirm_sell_pos"], Point(0.4, 0.6)));
        assert!(matches!(
            job.take_sale_draft().unwrap()["confirm_sell_pos"],
            Point(0.44, 0.6)
        ));
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

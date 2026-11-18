use crate::vision::{Bar, Button, Catch, Frame, Point, Rect, WindowFinder};
use serde::{Deserialize, Serialize};
use std::{collections::VecDeque, time::Duration};

// Shared with the main Python/Android engine: a slow cast can take over 1.5s.
const CAST_WAIT: f64 = 3.0;

#[derive(Clone, Copy, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Tuning {
    bite_seconds: f64,
    lead_ms: f64,
    target_percent: f64,
    deadband: f64,
    click_ms: u32,
    click_gap_ms: u32,
    cast_retries: u32,
    adaptive: bool,
    tap_above: bool,
}

impl Default for Tuning {
    fn default() -> Self {
        Self {
            bite_seconds: 60.0,
            lead_ms: 0.0,
            target_percent: 0.0,
            deadband: 0.0,
            click_ms: if cfg!(target_os = "android") { 30 } else { 25 },
            click_gap_ms: 45,
            cast_retries: 3,
            adaptive: cfg!(target_os = "android"),
            tap_above: cfg!(target_os = "android"),
        }
    }
}

impl Tuning {
    pub fn validate(self) -> Result<Self, &'static str> {
        if !(1.0..=600.0).contains(&self.bite_seconds)
            || !(0.0..=500.0).contains(&self.lead_ms)
            || !(0.0..=60.0).contains(&self.target_percent)
            || !(0.0..=0.1).contains(&self.deadband)
            || !(5..=300).contains(&self.click_ms)
            || !(10..=500).contains(&self.click_gap_ms)
            || !(1..=10).contains(&self.cast_retries)
        {
            return Err("낚시 설정의 범위를 확인하세요.");
        }
        Ok(self)
    }
}

#[derive(Clone, Copy, Deserialize, Serialize)]
pub struct Calibration {
    panel: Rect,
    reel: Rect,
    aspect: f64,
    #[serde(default)]
    result: Option<Rect>,
    #[serde(default)]
    fish_position: Option<Point>,
    #[serde(default)]
    bar_region: Option<Rect>,
    #[serde(default)]
    close_position: Option<Point>,
    #[serde(default)]
    title_position: Option<Point>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CalibrationPositions {
    panel: Option<Rect>,
    reel: Option<Rect>,
    fish: Point,
    bar: Rect,
    result: Option<Rect>,
    close: Option<Point>,
    title: Option<Point>,
}

impl Calibration {
    fn from_panel(panel: Rect, frame: &Frame<'_>) -> Self {
        let extra = (498.0 / 437.0 - 1.0) / 2.0;
        Self {
            panel,
            reel: panel.inset(-extra, 0.0, 1.0 + extra, 1.0),
            aspect: frame.width as f64 / frame.height as f64,
            result: None,
            fish_position: None,
            bar_region: None,
            close_position: None,
            title_position: None,
        }
    }

    fn fish(self) -> Point {
        self.fish_position
            .unwrap_or_else(|| self.panel.at(0.2494, 0.8264))
    }
    fn bar(self) -> Rect {
        self.bar_region
            .unwrap_or_else(|| self.reel.inset(0.0944, 0.2746, 0.9116, 0.4648))
    }

    pub fn positions(self) -> CalibrationPositions {
        CalibrationPositions {
            panel: valid_rect(self.panel).then_some(self.panel),
            reel: valid_rect(self.reel).then_some(self.reel),
            fish: self.fish(),
            bar: self.bar(),
            result: self.result,
            close: self
                .close_position
                .or_else(|| self.result.map(|rect| rect.at(0.9222, 0.0579))),
            title: self
                .title_position
                .or_else(|| self.result.map(|rect| rect.at(0.4972, 0.0913))),
        }
    }

    fn reel_tap(self, above: bool) -> Point {
        let bar = self.bar();
        if above {
            Point(
                (bar.0 + bar.2) / 2.0,
                ((if valid_rect(self.reel) {
                    self.reel.1
                } else {
                    bar.1
                }) - (bar.3 - bar.1) * 1.5)
                    .max(0.15),
            )
        } else {
            Point(
                (bar.0 + bar.2) / 2.0,
                (bar.3 + (bar.3 - bar.1) * 3.8).min(0.98),
            )
        }
    }

    fn moved(self, frame: &Frame<'_>) -> bool {
        (frame.width as f64 / frame.height as f64 / self.aspect - 1.0).abs() > 0.04
    }

    fn diamond_moved(self, frame: &Frame<'_>) -> bool {
        if !valid_rect(self.panel) || !valid_rect(self.reel) {
            return false;
        }
        let radius = ((self.panel.2 - self.panel.0) * frame.width as f64 * 0.012) as usize;
        frame.diamond(self.reel.at(0.9317, 0.8310), radius.max(3))
            && !frame.diamond(self.panel.at(0.9268, 0.8264), radius.max(3))
    }
}

fn valid_point(point: Point) -> bool {
    [point.0, point.1]
        .into_iter()
        .all(|value| value.is_finite() && (0.0..=1.0).contains(&value))
}

fn valid_rect(rect: Rect) -> bool {
    valid_point(Point(rect.0, rect.1))
        && valid_point(Point(rect.2, rect.3))
        && rect.0 < rect.2
        && rect.1 < rect.3
}

#[derive(Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Stats {
    success: u32,
    junk: u32,
    fail: u32,
    full: u32,
}

impl Stats {
    pub fn inventory_increased(self, before: Self) -> bool {
        self.full > before.full
    }

    pub fn record_delta(&mut self, before: Self, after: Self) {
        self.success += after.success - before.success;
        self.junk += after.junk - before.junk;
        self.fail += after.fail - before.fail;
        self.full += after.full - before.full;
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Phase {
    Idle,
    Calibrating,
    Searching,
    Casting,
    Waiting,
    Reeling,
    Result,
    Moving,
    Selling,
    Recording,
}

impl Phase {
    pub fn active(self) -> bool {
        self != Self::Idle
    }
}

#[derive(Clone, Copy)]
pub struct Tap {
    pub x: usize,
    pub y: usize,
    pub duration: Duration,
}

#[derive(Clone, Copy)]
enum CalibrationStep {
    Find,
    Cast,
    Bite,
    ReelFrame,
    Reel,
    Result,
}

pub struct Fishing {
    pub phase: Phase,
    pub message: &'static str,
    pub stats: Stats,
    pub calibration: Option<Calibration>,
    pub tuning: Tuning,
    pub calibration_changed: bool,
    phase_at: f64,
    last_scan: f64,
    last_progress: f64,
    last_cast: f64,
    casts: u32,
    fish_confirmation: Option<f64>,
    hold_cancel: Option<bool>,
    held_at: Option<f64>,
    cancel_at: Option<f64>,
    calibration_step: CalibrationStep,
    calibration_window: Option<Rect>,
    configured_positions: Option<(serde_json::Value, (usize, usize))>,
    result_baseline: Option<Catch>,
    bar_seen: u32,
    diamond_seen: bool,
    gone_at: Option<f64>,
    result: Option<Catch>,
    window: WindowFinder,
    bar: Bar,
    controller: ReelControl,
}

impl Default for Fishing {
    fn default() -> Self {
        Self {
            phase: Phase::Idle,
            message: "대기 중",
            stats: Stats::default(),
            calibration: None,
            tuning: Tuning::default(),
            calibration_changed: false,
            phase_at: 0.0,
            last_scan: f64::NEG_INFINITY,
            last_progress: 0.0,
            last_cast: f64::NEG_INFINITY,
            casts: 0,
            fish_confirmation: None,
            hold_cancel: None,
            held_at: None,
            cancel_at: None,
            calibration_step: CalibrationStep::Find,
            calibration_window: None,
            configured_positions: None,
            result_baseline: None,
            bar_seen: 0,
            diamond_seen: false,
            gone_at: None,
            result: None,
            window: WindowFinder::default(),
            bar: Bar::default(),
            controller: ReelControl::default(),
        }
    }
}

impl Fishing {
    fn phase(&mut self, phase: Phase, now: f64, message: &'static str) {
        if self.phase != phase {
            self.phase_at = now;
            self.phase = phase;
        }
        self.message = message;
    }

    /// Apply the same normalized coordinates used by main's manual setup.
    /// This is configuration, not evidence that automatic calibration ran.
    pub fn configure_positions(
        &mut self,
        source: &serde_json::Value,
        geometry: (usize, usize),
    ) -> Result<(), &'static str> {
        if !(1..=16384).contains(&geometry.0) || !(1..=16384).contains(&geometry.1) {
            return Err("낚시 위치를 적용할 실제 게임 화면 크기가 필요합니다.");
        }
        if !source.is_object() {
            return Err("낚시 위치 설정을 확인하세요.");
        }
        if self.phase == Phase::Calibrating {
            return Ok(());
        }
        let keys = [
            "panel_region",
            "reel_region",
            "result_region",
            "fish_btn",
            "bar_region",
            "close_pos",
            "title_pos",
        ];
        let signature = serde_json::Value::Object(
            keys.into_iter()
                .map(|key| (key.to_owned(), source[key].clone()))
                .collect(),
        );
        if self
            .configured_positions
            .as_ref()
            .is_some_and(|(previous, size)| *size == geometry && *previous == signature)
        {
            return Ok(());
        }
        let rect = |key: &str| -> Result<Option<Rect>, &'static str> {
            let value = &source[key];
            if value.is_null() {
                return Ok(None);
            }
            let rect: Rect = serde_json::from_value(value.clone())
                .map_err(|_| "낚시 영역 좌표를 확인하세요.")?;
            if !valid_rect(rect) {
                return Err("낚시 영역은 화면 안의 서로 다른 두 모서리로 지정하세요.");
            }
            Ok(Some(rect))
        };
        let point = |key: &str| -> Result<Option<Point>, &'static str> {
            let value = &source[key];
            if value.is_null() {
                return Ok(None);
            }
            let point: Point = serde_json::from_value(value.clone())
                .map_err(|_| "낚시 버튼 좌표를 확인하세요.")?;
            if !valid_point(point) {
                return Err("낚시 버튼은 게임 화면 안에서 지정하세요.");
            }
            Ok(Some(point))
        };
        // Validate the complete patch before changing an existing calibration.
        let panel = rect("panel_region")?;
        let reel = rect("reel_region")?;
        let result = rect("result_region")?;
        let fish = point("fish_btn")?;
        let bar = rect("bar_region")?;
        let close = point("close_pos")?;
        let title = point("title_pos")?;
        if panel.is_none()
            && reel.is_none()
            && result.is_none()
            && fish.is_none()
            && bar.is_none()
            && close.is_none()
            && title.is_none()
        {
            self.configured_positions = Some((signature, geometry));
            return Ok(());
        }
        let mut next = self.calibration.unwrap_or(Calibration {
            panel: Rect(0., 0., 0., 0.),
            reel: Rect(0., 0., 0., 0.),
            aspect: geometry.0 as f64 / geometry.1 as f64,
            result: None,
            fish_position: None,
            bar_region: None,
            close_position: None,
            title_position: None,
        });
        if let Some(panel) = panel {
            next.panel = panel;
            let extra = (498.0 / 437.0 - 1.0) / 2.0;
            next.reel = panel.inset(-extra, 0., 1. + extra, 1.);
            next.fish_position = Some(panel.at(0.2494, 0.8264));
            next.bar_region = Some(next.reel.inset(0.0944, 0.2746, 0.9116, 0.4648));
        }
        if let Some(reel) = reel {
            next.reel = reel;
            next.bar_region = Some(reel.inset(0.0944, 0.2746, 0.9116, 0.4648));
        }
        if let Some(result) = result {
            next.result = Some(result);
            next.close_position = Some(result.at(0.9222, 0.0579));
            next.title_position = Some(result.at(0.4972, 0.0913));
        }
        if let Some(fish) = fish {
            next.fish_position = Some(fish);
        }
        if let Some(bar) = bar {
            next.bar_region = Some(bar);
        }
        if let Some(close) = close {
            next.close_position = Some(close);
        }
        if let Some(title) = title {
            next.title_position = Some(title);
        }
        next.aspect = geometry.0 as f64 / geometry.1 as f64;
        if (next.fish_position.is_some() || valid_rect(next.panel))
            && (next.bar_region.is_some() || valid_rect(next.reel))
        {
            self.calibration = Some(next);
        }
        self.configured_positions = Some((signature, geometry));
        Ok(())
    }

    pub fn calibrate(&mut self, now: f64) -> Result<(), &'static str> {
        if self.phase.active() {
            return Err("실행 중인 작업을 먼저 중지하세요.");
        }
        self.reset_cycle();
        self.calibration_step = CalibrationStep::Find;
        self.bar = Bar::default();
        self.controller = ReelControl::default();
        self.phase(Phase::Calibrating, now, "Roblox에서 낚시 창을 열어 주세요.");
        Ok(())
    }

    pub fn start(&mut self, now: f64) -> Result<(), &'static str> {
        if self.phase.active() {
            return Err("실행 중인 작업을 먼저 중지하세요.");
        }
        if self.calibration.is_none() {
            return Err("낚시 자동 보정을 먼저 실행하세요.");
        }
        self.reset_cycle();
        self.hold_cancel = None;
        self.held_at = None;
        self.cancel_at = None;
        self.last_progress = now;
        self.diamond_seen = false;
        self.phase(Phase::Searching, now, "Roblox 화면을 기다리는 중");
        Ok(())
    }

    fn reset_cycle(&mut self) {
        self.last_scan = f64::NEG_INFINITY;
        self.last_cast = f64::NEG_INFINITY;
        self.casts = 0;
        self.fish_confirmation = None;
        self.bar_seen = 0;
        self.gone_at = None;
        self.result = None;
    }

    pub fn stop(&mut self, message: &'static str) {
        self.phase(Phase::Idle, 0.0, message);
        self.hold_cancel = None;
        self.held_at = None;
        self.cancel_at = None;
        self.window = WindowFinder::default();
        self.bar = Bar::default();
    }

    /// The coordinator owns the single screen job. Keep ticking fishing until
    /// is_yielded(): ordinary items finish this catch; urgent popping cancels
    /// the bite wait through Exit, while an active reel/result still completes.
    pub fn hold(&mut self, cancel: bool, now: f64) {
        self.hold_cancel = Some(self.hold_cancel.unwrap_or(false) || cancel);
        if !self.phase.active() && now.is_finite() {
            self.held_at = Some(now);
        }
    }

    pub fn is_yielded(&self) -> bool {
        !self.phase.active() || self.held_at.is_some()
    }

    pub fn release(&mut self, now: f64) {
        self.hold_cancel = None;
        self.cancel_at = None;
        if let Some(since) = self.held_at.take() {
            let elapsed = (now - since).max(0.0);
            if elapsed.is_finite() && self.phase.active() {
                self.phase_at += elapsed;
                self.last_cast += elapsed;
                self.last_progress = now;
                self.fish_confirmation = None;
                self.message = "낚시 화면 확인 중";
            }
        }
    }

    pub fn no_target(&mut self, now: f64) {
        if self.phase == Phase::Calibrating && now - self.phase_at > 30.0 {
            self.stop("Roblox를 열고 자동 보정을 다시 실행하세요.");
        } else if self.phase.active() && self.phase != Phase::Calibrating {
            if self.phase != Phase::Searching {
                self.reset_cycle();
            }
            self.last_progress = now;
            self.phase(
                Phase::Searching,
                now,
                "Roblox가 앞에 오면 낚시를 이어갑니다.",
            );
        }
    }

    fn find_panel(&mut self, frame: &Frame<'_>) -> Option<Calibration> {
        self.window
            .find(frame, |rect, ratio| {
                (2.7..=3.25).contains(&ratio) && frame.button(rect.at(0.2494, 0.8264)).is_some()
            })
            .map(|rect| Calibration::from_panel(rect, frame))
    }

    fn result_window(&mut self, frame: &Frame<'_>) -> Option<Rect> {
        self.window.find(frame, |rect, ratio| {
            (0.35..=1.6).contains(&ratio)
                && rect.3 - rect.1 >= 0.25
                && ((rect.0 + rect.2) / 2.0 - 0.5).abs() <= 0.15
        })
    }

    fn tap(frame: &Frame<'_>, point: Point, ms: u32) -> Tap {
        let (x, y) = frame.position(point);
        Tap {
            x,
            y,
            duration: Duration::from_millis(ms.into()),
        }
    }

    fn finish_result(&mut self, now: f64) {
        // Main treats an unread fast-closing result as success. Explicit fail
        // wins, then success, then junk (a fading success can look gray).
        match self.result.take().unwrap_or(Catch::Success) {
            Catch::Fail => self.stats.fail += 1,
            Catch::Junk => self.stats.junk += 1,
            Catch::Success => self.stats.success += 1,
        }
        self.reset_cycle();
        self.phase(Phase::Searching, now, "낚시 화면 확인 중");
    }

    fn calibration_step(&mut self, step: CalibrationStep, now: f64, message: &'static str) {
        self.calibration_step = step;
        self.calibration_window = None;
        self.last_scan = f64::NEG_INFINITY;
        self.phase_at = now;
        self.message = message;
        self.fish_confirmation = None;
    }

    fn stable_calibration_window(&mut self, current: Option<Rect>) -> Option<Rect> {
        let stable = self
            .calibration_window
            .zip(current)
            .filter(|(previous, current)| {
                [
                    previous.0 - current.0,
                    previous.1 - current.1,
                    previous.2 - current.2,
                    previous.3 - current.3,
                ]
                .into_iter()
                .all(|difference| difference.abs() <= 0.002)
            })
            .map(|(_, current)| current);
        self.calibration_window = current;
        stable
    }

    fn begin_calibration_reel(&mut self, now: f64) {
        self.bar.reset();
        self.controller = ReelControl::default();
        self.gone_at = None;
        self.calibration_step(
            CalibrationStep::Reel,
            now,
            "자동 보정: 미니게임 확인·릴링 중",
        );
    }

    fn calibration_frame(&mut self, now: f64, frame: &Frame<'_>) -> Option<Tap> {
        if matches!(self.calibration_step, CalibrationStep::Find) {
            if now - self.phase_at > 30.0 {
                self.stop(
                    "낚시 창을 찾지 못했습니다. Fish 또는 Exit 버튼이 보일 때 다시 보정하세요.",
                );
            } else if now - self.last_scan >= 0.25 {
                self.last_scan = now;
                if let Some(mut calibration) = self.find_panel(frame) {
                    calibration.result = self.calibration.and_then(|previous| previous.result);
                    calibration.close_position = self
                        .calibration
                        .and_then(|previous| previous.close_position);
                    calibration.title_position = self
                        .calibration
                        .and_then(|previous| previous.title_position);
                    self.calibration = Some(calibration);
                    self.calibration_changed = true;
                    self.calibration_step(
                        CalibrationStep::Cast,
                        now,
                        "자동 보정: 낚싯대 던지는 중",
                    );
                }
            }
            return None;
        }
        let mut calibration = self.calibration?;
        let fish = frame.button(calibration.fish());
        let bar = self.bar.read(frame, calibration.bar());
        let biting = bar || calibration.diamond_moved(frame);
        match self.calibration_step {
            CalibrationStep::Find => unreachable!(),
            CalibrationStep::Cast => {
                if fish == Some(Button::Exit) || biting {
                    self.calibration_step(
                        CalibrationStep::Bite,
                        now,
                        "자동 보정: 입질 기다리는 중",
                    );
                } else if fish == Some(Button::Fish) && now - self.last_cast >= CAST_WAIT {
                    if !self.fish_confirmation.is_some_and(|at| now > at) {
                        self.fish_confirmation = Some(now);
                        return None;
                    }
                    self.fish_confirmation = None;
                    if self.casts >= 3 {
                        self.stop("Fish 버튼이 반응하지 않아 대기 창만 맞췄습니다. 인벤토리를 확인하세요.");
                    } else {
                        self.casts += 1;
                        self.last_cast = now;
                        return Some(Self::tap(frame, calibration.fish(), 40));
                    }
                } else if fish.is_none() {
                    self.fish_confirmation = None;
                    if now - self.phase_at > 3.0 * CAST_WAIT + 3.0 {
                        self.stop("Fish 또는 Exit 버튼을 확인하지 못해 자동 보정을 중지했습니다.");
                    }
                }
            }
            CalibrationStep::Bite => {
                if biting {
                    self.calibration_step(
                        CalibrationStep::ReelFrame,
                        now,
                        "자동 보정: 미니게임 창이 멈추기를 기다리는 중",
                    );
                } else if now - self.phase_at > self.tuning.bite_seconds + 10.0 {
                    self.stop("입질이 없어 대기 창만 맞췄습니다. 자동 보정을 다시 실행하세요.");
                    if fish == Some(Button::Exit) {
                        return Some(Self::tap(frame, calibration.fish(), 40));
                    }
                }
            }
            CalibrationStep::ReelFrame => {
                if now - self.last_scan >= 0.1 {
                    self.last_scan = now;
                    let center = (calibration.panel.0 + calibration.panel.2) / 2.0;
                    let current = self.window.find(frame, |rect, ratio| {
                        (3.3..=3.9).contains(&ratio)
                            && ((rect.0 + rect.2) / 2.0 - center).abs() < 0.15
                    });
                    if let Some(reel) = self.stable_calibration_window(current) {
                        calibration.reel = reel;
                        calibration.bar_region = None;
                        self.calibration = Some(calibration);
                        self.calibration_changed = true;
                        self.begin_calibration_reel(now);
                    } else if now - self.phase_at >= 3.0 {
                        // Main retains its panel-derived fallback if the actual
                        // minigame window cannot settle within three seconds.
                        self.begin_calibration_reel(now);
                    }
                }
            }
            CalibrationStep::Reel => {
                if bar && now - self.phase_at < 15.0 {
                    self.gone_at = None;
                    let (marker, zones, width) = self.bar.track(frame, calibration.bar());
                    if self
                        .controller
                        .click(now, marker, zones, width, self.tuning)
                    {
                        return Some(Self::tap(
                            frame,
                            calibration.reel_tap(self.tuning.tap_above),
                            self.tuning.click_ms,
                        ));
                    }
                } else if now - *self.gone_at.get_or_insert(now) >= 0.6
                    || now - self.phase_at >= 15.0
                {
                    self.last_scan = f64::NEG_INFINITY;
                    self.calibration_step(
                        CalibrationStep::Result,
                        now,
                        "자동 보정: 결과창 확인 중",
                    );
                }
            }
            CalibrationStep::Result => {
                if fish.is_some() {
                    self.stop("자동 보정 완료");
                } else if now - self.last_scan >= 0.1 {
                    self.last_scan = now;
                    let current = self.result_window(frame);
                    if let Some(result) = self.stable_calibration_window(current) {
                        calibration.result = Some(result);
                        calibration.close_position = Some(result.at(0.9222, 0.0579));
                        calibration.title_position = Some(result.at(0.4972, 0.0913));
                        self.calibration = Some(calibration);
                        self.calibration_changed = true;
                        return Some(Self::tap(frame, frame.result_close(result), 40));
                    }
                    if now - self.phase_at >= 10.0 {
                        self.stop(
                            "결과창을 확인하지 못했습니다. 대기 창·미니게임 창은 맞췄습니다.",
                        );
                    }
                }
            }
        }
        None
    }

    pub fn frame(&mut self, now: f64, frame: &Frame<'_>) -> Option<Tap> {
        if !self.phase.active() {
            return None;
        }
        if self.phase == Phase::Calibrating {
            return self.calibration_frame(now, frame);
        }
        if let Some(since) = self.held_at {
            if now - since >= 600.0 {
                self.release(now);
            } else {
                return None;
            }
        }

        let mut calibration = self.calibration?;
        if self.hold_cancel.is_none() && now - self.last_progress > self.tuning.bite_seconds + 60.0
        {
            self.stop("낚시 화면을 찾을 수 없어 중지했습니다.");
            return None;
        }
        let fish = frame.button(calibration.fish());
        if calibration.moved(frame) || (self.phase == Phase::Searching && fish.is_none()) {
            if now - self.last_scan >= 1.5 {
                self.last_scan = now;
                if let Some(updated) = self.find_panel(frame) {
                    calibration = updated;
                    self.calibration = Some(updated);
                    self.calibration_changed = true;
                }
            }
            if calibration.moved(frame) {
                self.phase(
                    Phase::Searching,
                    now,
                    "화면 비율이 바뀌었습니다. 낚시 창을 다시 찾는 중",
                );
                return None;
            }
        }
        let fish = frame.button(calibration.fish());

        if self.phase == Phase::Result {
            if fish.is_some() || now - self.phase_at >= 10.0 {
                self.finish_result(now);
            } else if now - self.last_scan >= 0.1 {
                self.last_scan = now;
                let window = self.result_window(frame).or(calibration.result);
                let title = calibration
                    .title_position
                    .or_else(|| window.map(|rect| rect.at(0.4972, 0.0913)));
                if let Some(seen) = title
                    .and_then(|point| frame.title(point))
                    .filter(|seen| window.is_some() || Some(*seen) != self.result_baseline)
                {
                    self.result = match (self.result, seen) {
                        (Some(Catch::Fail), _) | (_, Catch::Fail) => Some(Catch::Fail),
                        (Some(Catch::Success), _) | (_, Catch::Success) => Some(Catch::Success),
                        (_, Catch::Junk) => Some(Catch::Junk),
                    };
                }
                if let Some(close) = calibration
                    .close_position
                    .or_else(|| window.map(|rect| frame.result_close(rect)))
                {
                    return Some(Self::tap(frame, close, 40));
                }
            }
            return None;
        }

        let area = calibration.bar();
        let bar = self.bar.read(frame, area);
        let diamond = calibration.diamond_moved(frame);
        self.diamond_seen |= diamond;
        self.bar_seen = if bar && (diamond || !self.diamond_seen) {
            self.bar_seen + 1
        } else {
            0
        };
        if self.phase != Phase::Reeling && (self.bar_seen >= 3 || bar && diamond) {
            self.bar.reset();
            self.controller = ReelControl::default();
            self.gone_at = None;
            self.casts = 0;
            self.result_baseline = calibration
                .title_position
                .and_then(|point| frame.title(point));
            self.phase(Phase::Reeling, now, "릴링 중");
        }
        if self.phase == Phase::Reeling {
            self.last_progress = now;
            if bar && (diamond || !self.diamond_seen) {
                self.gone_at = None;
                if now - self.phase_at < 15.0 {
                    let (marker, zones, width) = self.bar.track(frame, area);
                    if self
                        .controller
                        .click(now, marker, zones, width, self.tuning)
                    {
                        return Some(Self::tap(
                            frame,
                            calibration.reel_tap(self.tuning.tap_above),
                            self.tuning.click_ms,
                        ));
                    }
                    return None;
                }
            } else if now - *self.gone_at.get_or_insert(now) < 0.6 && now - self.phase_at < 15.0 {
                return None;
            }
            self.last_scan = f64::NEG_INFINITY;
            self.phase(Phase::Result, now, "낚시 결과창 닫는 중");
            return None;
        }

        if let Some(cancel) = self.hold_cancel {
            if self.cancel_at.is_some_and(|at| now - at < 0.8) {
                return None;
            }
            match fish {
                Some(Button::Fish) => {
                    self.held_at = Some(now);
                    self.fish_confirmation = None;
                    self.message = "다른 기능에 자리 양보 중";
                    return None;
                }
                Some(Button::Exit) if cancel => {
                    self.cancel_at = Some(now);
                    return Some(Self::tap(frame, calibration.fish(), 40));
                }
                _ => {}
            }
        }

        match fish {
            Some(Button::Fish) if now - self.last_cast >= CAST_WAIT => {
                // Main reads the button again before retrying or declaring a
                // full inventory. A later frame supplies that second read.
                if !self.fish_confirmation.is_some_and(|at| now > at) {
                    self.fish_confirmation = Some(now);
                    return None;
                }
                self.fish_confirmation = None;
                if self.casts >= self.tuning.cast_retries {
                    self.stats.full += 1;
                    self.stop("Fish 버튼이 반응하지 않습니다. 인벤토리를 확인하세요.");
                } else {
                    self.casts += 1;
                    self.last_cast = now;
                    self.phase(Phase::Casting, now, "낚싯대 던지는 중");
                    return Some(Self::tap(frame, calibration.fish(), 40));
                }
            }
            Some(Button::Exit) => {
                self.fish_confirmation = None;
                self.casts = 0;
                self.last_cast = f64::NEG_INFINITY;
                self.phase(Phase::Waiting, now, "입질 기다리는 중");
                self.last_progress = now;
                if now - self.phase_at >= self.tuning.bite_seconds {
                    self.phase(Phase::Searching, now, "입질이 없어 다시 던지는 중");
                    self.last_cast = now - CAST_WAIT + 1.0;
                    return Some(Self::tap(frame, calibration.fish(), 40));
                }
            }
            None if !bar => {
                self.fish_confirmation = None;
                self.phase(Phase::Searching, now, "낚시 화면 확인 중");
            }
            _ => {}
        }
        None
    }
}

struct ReelControl {
    history: VecDeque<(f64, f64)>,
    odd: Option<f64>,
    zone: Option<(f64, f64, f64)>,
    velocity: f64,
    last_click: Option<f64>,
    rise_delay: f64,
    rising: bool,
    edge_k: f64,
    side: i8,
}

impl Default for ReelControl {
    fn default() -> Self {
        Self {
            history: VecDeque::new(),
            odd: None,
            zone: None,
            velocity: 0.,
            last_click: None,
            rise_delay: 0.,
            rising: true,
            edge_k: 1.,
            side: 0,
        }
    }
}

impl ReelControl {
    fn track_zone(
        &mut self,
        now: f64,
        candidates: &[(f64, f64)],
        marker: f64,
        w: f64,
    ) -> Option<(f64, f64)> {
        if let Some((width, center, last)) = self.zone {
            let reach = width * 0.6 + 300.0 * (now - last);
            let distance = |a: f64, b: f64| ((a + b) / 2.0 - center).abs();
            if let Some(&(a, b)) = candidates
                .iter()
                .filter(|&&(a, b)| b - a >= width * 0.7 && distance(a, b) <= reach)
                .min_by(|&&(a, b), &&(c, d)| distance(a, b).total_cmp(&distance(c, d)))
            {
                self.zone = Some((
                    width * 0.8 + (b - a).min(width * 1.3) * 0.2,
                    (a + b) / 2.0,
                    now,
                ));
                return Some((a, b));
            }
            if let Some(right) = candidates
                .iter()
                .filter_map(|&(a, b)| {
                    ((a - marker).abs() <= (w * 0.02).max(6.0)
                        && b > marker
                        && (b - center - width / 2.0).abs() <= reach)
                        .then_some(b)
                })
                .max_by(f64::total_cmp)
            {
                self.zone = Some((width, right - width / 2.0, now));
                return Some((right - width, right));
            }
            if now - last <= 1.0 {
                return Some((center - width / 2.0, center + width / 2.0));
            }
            self.zone = None;
            return None;
        }
        let &(a, b) = candidates
            .iter()
            .filter(|&&(a, b)| b - a >= (w * 0.06).max(8.0))
            .max_by(|&&(a, b), &&(c, d)| (b - a).total_cmp(&(d - c)))?;
        self.zone = Some((b - a, (a + b) / 2.0, now));
        Some((a, b))
    }

    fn click(
        &mut self,
        now: f64,
        mut marker: Option<f64>,
        zones: &[(f64, f64)],
        width: f64,
        tuning: Tuning,
    ) -> bool {
        if let Some(m) = marker {
            let jump = width * 0.25;
            let outlier = self.history.len() >= 2
                && self
                    .history
                    .iter()
                    .rev()
                    .take(2)
                    .all(|&(_, x)| (x - m).abs() > jump);
            if outlier && !self.odd.is_some_and(|x| (x - m).abs() <= jump * 0.5) {
                self.odd = Some(m);
                marker = None;
            } else {
                if outlier {
                    self.history.clear();
                }
                self.odd = None;
                if self.history.len() == 5 {
                    self.history.pop_front();
                }
                self.history.push_back((now, m));
            }
        }

        if let (Some(&(a, x)), Some(&(b, y))) = (self.history.front(), self.history.back()) {
            if self.history.len() >= 3 && b > a {
                self.velocity = 0.5 * self.velocity + 0.5 * (y - x) / (b - a);
                if let Some(click) = self
                    .last_click
                    .filter(|&t| !self.rising && b > t && self.velocity > width * 0.05)
                {
                    self.rising = true;
                    let delay = b - click;
                    self.rise_delay = if self.rise_delay == 0.0 {
                        delay
                    } else {
                        self.rise_delay * 0.7 + delay * 0.3
                    };
                }
            }
            marker = marker.or_else(|| (now - b < 0.1).then_some(y + self.velocity * (now - b)));
        }
        let Some(m) = marker else { return false };
        let Some((left, right)) = self.track_zone(now, zones, m, width) else {
            return false;
        };
        let side = if m > right {
            1
        } else if m < left - width * 0.01 {
            -1
        } else {
            0
        };
        if side != self.side {
            if side == 1 && self.last_click.is_some_and(|at| now - at < 0.8) {
                self.edge_k = (self.edge_k * 0.7 - 0.05).max(0.0);
            } else if side == -1 {
                self.edge_k = (self.edge_k * 1.15 + 0.05).min(1.6);
            }
            self.side = side;
        }
        let target = left
            + (right - left) * tuning.target_percent / 100.0
            + (width * 0.04).min((right - left) * 0.35) * self.edge_k;
        let settle = if tuning.adaptive {
            (self.rise_delay * 1.6).clamp(0.2, 0.5)
        } else {
            0.2
        };
        if let Some(at) = self.last_click {
            let elapsed = now - at;
            let falling_outside = m < left && self.velocity <= 0.0 && elapsed >= 0.06;
            if elapsed * 1000.0 < f64::from(tuning.click_gap_ms)
                || (!self.rising && elapsed < settle && !falling_outside)
            {
                return false;
            }
        }
        let max_rise = (2.0 * (target - m).max(0.0))
            .min(width * 0.5)
            .max(width * 0.05);
        let predicted = m
            + self.velocity.min(0.0) * self.rise_delay.min(0.1)
            + self.velocity * tuning.lead_ms / 1000.0;
        let click = (self.history.len() < 3 || self.velocity <= max_rise)
            && m.max(predicted) <= (left + right) / 2.0
            && predicted <= target - tuning.deadband * width;
        if click {
            self.last_click = Some(now);
            self.rising = false;
        }
        click
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const W: usize = 480;
    const H: usize = 270;
    const STRIDE: usize = W * 4 + 32;

    fn put(bytes: &mut [u8], x: usize, y: usize, pixel: [u8; 3]) {
        let p = y * STRIDE + x * 4;
        bytes[p..p + 3].copy_from_slice(&pixel);
    }

    fn window(bytes: &mut [u8], x0: usize, y0: usize, x1: usize, y1: usize) {
        for i in 0..14 {
            for x in [x0 + i, x1 - i] {
                for y in [y0, y1] {
                    put(bytes, x, y, [248; 3]);
                }
            }
            for x in [x0, x1] {
                for y in [y0 + i, y1 - i] {
                    put(bytes, x, y, [248; 3]);
                }
            }
        }
    }

    fn frame(bytes: &[u8]) -> Frame<'_> {
        Frame::new(bytes, W, H, STRIDE, 4, false).unwrap()
    }

    fn idle_pixels(exit: bool) -> Vec<u8> {
        let mut bytes = vec![0; STRIDE * H];
        window(&mut bytes, 110, 95, 370, 181);
        for y in 164..168 {
            for x in 170..179 {
                put(
                    &mut bytes,
                    x,
                    y,
                    if exit { [240, 30, 30] } else { [20, 75, 205] },
                );
            }
        }
        bytes
    }

    fn ready() -> Fishing {
        let bytes = idle_pixels(false);
        let mut engine = Fishing::default();
        engine.calibration = Some(Calibration::from_panel(
            Rect(
                110. / W as f64,
                95. / H as f64,
                370. / W as f64,
                181. / H as f64,
            ),
            &frame(&bytes),
        ));
        engine.start(0.).unwrap();
        engine
    }

    #[test]
    fn full_inventory_requires_three_three_second_casts_and_fresh_fish_frames() {
        let mut engine = ready();
        let bytes = idle_pixels(false);
        for time in [0.2, 3.4, 6.6] {
            assert!(engine.frame(time - 0.1, &frame(&bytes)).is_none());
            let tap = engine.frame(time, &frame(&bytes)).unwrap();
            assert!((170..179).contains(&tap.x) && (164..168).contains(&tap.y));
        }
        assert!(engine.frame(9.8, &frame(&bytes)).is_none());
        assert_eq!(
            engine.stats.full, 0,
            "a previously read Fish cannot immediately become full"
        );
        assert!(engine.frame(9.9, &frame(&bytes)).is_none());
        assert!(engine.phase == Phase::Idle);
        assert_eq!(engine.stats.full, 1);
    }

    #[test]
    fn slow_cast_and_existing_exit_never_receive_a_second_canceling_tap() {
        let fish = idle_pixels(false);
        let exit = idle_pixels(true);
        let mut engine = ready();
        assert!(engine.frame(0.1, &frame(&fish)).is_none());
        assert!(engine.frame(0.2, &frame(&fish)).is_some());
        for time in [1.8, 2.5] {
            assert!(engine.frame(time, &frame(&fish)).is_none());
        }
        assert!(engine.frame(2.7, &frame(&exit)).is_none());
        assert!(engine.phase == Phase::Waiting);
        assert_eq!(engine.stats.full, 0);
        let mut existing = ready();
        assert!(existing.frame(0.1, &frame(&exit)).is_none());
        assert!(existing.phase == Phase::Waiting);
        assert_eq!(existing.casts, 0);
    }

    #[test]
    fn exit_in_the_fresh_confirmation_frame_prevents_false_full() {
        let mut engine = ready();
        engine.tuning.cast_retries = 1;
        let fish = idle_pixels(false);
        assert!(engine.frame(0.1, &frame(&fish)).is_none());
        assert!(engine.frame(0.2, &frame(&fish)).is_some());
        assert!(engine.frame(3.4, &frame(&fish)).is_none());
        assert!(engine.frame(3.5, &frame(&idle_pixels(true))).is_none());
        assert!(engine.phase == Phase::Waiting);
        assert_eq!(engine.stats.full, 0);
    }

    #[test]
    fn calibration_keeps_the_same_slow_cast_wait_and_does_not_tap_unknown_or_exit() {
        let fish = idle_pixels(false);
        let exit = idle_pixels(true);
        let mut engine = Fishing::default();
        engine.calibrate(0.).unwrap();
        assert!(engine.frame(0.1, &frame(&fish)).is_none());
        assert!(
            engine.phase == Phase::Calibrating,
            "finding the panel alone is not a completed calibration"
        );
        assert!(engine.frame(0.2, &frame(&fish)).is_none());
        assert!(engine.frame(0.3, &frame(&fish)).is_some());
        for time in [1.9, 2.7] {
            assert!(engine.frame(time, &frame(&fish)).is_none());
        }
        assert!(engine.frame(2.9, &frame(&exit)).is_none());
        assert!(matches!(engine.calibration_step, CalibrationStep::Bite));
        let mut existing = Fishing::default();
        existing.calibrate(0.).unwrap();
        for time in [0.1, 0.2, 0.3] {
            assert!(existing.frame(time, &frame(&exit)).is_none());
        }
        assert!(matches!(existing.calibration_step, CalibrationStep::Bite));
        let mut unknown = Fishing::default();
        unknown.calibrate(0.).unwrap();
        unknown.frame(0.1, &frame(&fish));
        let blank = vec![0; STRIDE * H];
        for time in [0.2, 1., 3., 10., 13.] {
            assert!(unknown.frame(time, &frame(&blank)).is_none());
        }
        assert!(unknown.phase == Phase::Idle);
    }

    #[test]
    fn calibration_runs_through_the_actual_reel_and_result_without_changing_stats() {
        let mut engine = Fishing::default();
        let fish = idle_pixels(false);
        let exit = idle_pixels(true);
        engine.calibrate(0.).unwrap();
        engine.frame(0.1, &frame(&exit));
        engine.frame(0.2, &frame(&exit));
        let mut reel = vec![0; STRIDE * H];
        window(&mut reel, 92, 95, 388, 181);
        for y in 120..133 {
            for x in 220..251 {
                put(&mut reel, x, y, [205, 75, 125]);
            }
        }
        for y in 165..168 {
            for x in 367..371 {
                put(&mut reel, x, y, [245; 3]);
            }
        }
        engine.frame(1., &frame(&reel));
        assert!(matches!(
            engine.calibration_step,
            CalibrationStep::ReelFrame
        ));
        engine.frame(1.1, &frame(&reel));
        assert!(matches!(
            engine.calibration_step,
            CalibrationStep::ReelFrame
        ));
        engine.frame(1.25, &frame(&reel));
        assert!(matches!(engine.calibration_step, CalibrationStep::Reel));
        engine.frame(1.3, &frame(&reel));
        let mut result = vec![0; STRIDE * H];
        window(&mut result, 185, 70, 290, 214);
        engine.frame(2., &frame(&result));
        engine.frame(2.7, &frame(&result));
        assert!(matches!(engine.calibration_step, CalibrationStep::Result));
        assert!(engine.frame(2.8, &frame(&result)).is_none());
        assert!(engine.calibration.unwrap().result.is_none());
        assert!(engine.frame(2.95, &frame(&result)).is_some());
        assert!(engine.calibration.unwrap().result.is_some());
        engine.frame(3., &frame(&fish));
        assert!(engine.phase == Phase::Idle);
        assert_eq!(engine.message, "자동 보정 완료");
        assert!(engine.stats == Stats::default());
    }

    #[test]
    fn result_closing_uses_main_default_success_and_remains_running_after_timeout() {
        let mut engine = ready();
        engine.phase(Phase::Result, 1., "결과창 닫는 중");
        let blank = vec![0; STRIDE * H];
        engine.frame(11.1, &frame(&blank));
        assert_eq!(engine.stats.success, 1);
        assert!(engine.phase == Phase::Searching);
    }

    #[test]
    fn finds_displaced_x_on_a_bright_result_window() {
        let mut bytes = vec![65; STRIDE * H];
        window(&mut bytes, 185, 70, 290, 214);
        for i in 0..7 {
            for x in [265 + i, 271 - i] {
                put(&mut bytes, x, 86 + i, [235; 3]);
            }
        }
        let frame = frame(&bytes);
        let mut engine = Fishing::default();
        let result = engine.result_window(&frame).unwrap();
        assert_eq!(frame.position(frame.result_close(result)), (268, 89));
    }

    #[test]
    fn reel_controller_waits_inside_the_zone_and_avoids_the_right_half() {
        let mut controller = ReelControl::default();
        let tuning = Tuning::default();
        let zone = [(35.0, 65.0)];
        assert!(controller.click(0.0, Some(15.0), &zone, 100.0, tuning));
        assert!(!controller.click(0.1, Some(37.0), &zone, 100.0, tuning));
        assert!(!controller.click(0.25, Some(55.0), &zone, 100.0, tuning));
    }
    #[test]
    fn reel_clicks_before_the_left_edge_but_never_past_the_middle() {
        let tuning = Tuning::default();
        for (marker, expected) in [(114., true), (120., false), (150., false)] {
            let mut controller = ReelControl::default();
            assert_eq!(
                controller.click(1., Some(marker), &[(100., 180.)], 400., tuning),
                expected
            );
        }
    }

    #[test]
    fn reel_reclicks_every_sixty_ms_when_the_marker_keeps_falling_outside() {
        let mut controller = ReelControl::default();
        let mut clicks = Vec::new();
        for i in 0..40 {
            let time = 1. + (i as f64) * 0.01;
            if controller.click(
                time,
                Some(90. - i as f64),
                &[(100., 180.)],
                400.,
                Tuning::default(),
            ) {
                clicks.push(time);
            }
        }
        assert!(
            clicks.len() >= 5,
            "continued fall must not be limited to one input every 200ms: {clicks:?}"
        );
        assert!(clicks.windows(2).all(|w| w[1] - w[0] >= 0.059));
    }
    #[test]
    fn manual_points_control_actual_inputs_without_claiming_panel_measurement() {
        let mut engine = Fishing::default();
        let manual = serde_json::json!({"fish_btn":[0.2,0.5],"bar_region":[0.3,0.5,0.6,0.6],"close_pos":[0.7,0.3],"title_pos":[0.6,0.2]});
        engine.configure_positions(&manual, (W, H)).unwrap();
        assert!(!engine.calibration_changed);
        let positions = serde_json::to_value(engine.calibration.unwrap().positions()).unwrap();
        assert!(positions["panel"].is_null() && positions["reel"].is_null());
        assert_eq!(positions["fish"], serde_json::json!([0.2, 0.5]));
        engine.start(0.).unwrap();
        let mut bytes = vec![0; STRIDE * H];
        for y in 133..137 {
            for x in 92..101 {
                put(&mut bytes, x, y, [20, 75, 205]);
            }
        }
        assert!(engine.frame(0.1, &frame(&bytes)).is_none());
        let tap = engine.frame(0.2, &frame(&bytes)).unwrap();
        assert_eq!((tap.x, tap.y), (96, 135));
        engine.phase(Phase::Result, 1., "결과창 닫는 중");
        let tap = engine.frame(1.1, &frame(&vec![0; STRIDE * H])).unwrap();
        assert_eq!((tap.x, tap.y), (336, 81));
    }

    #[test]
    fn manual_regions_derive_layout_preserve_auto_result_and_reject_partial_bad_updates() {
        let mut engine = ready();
        engine.stop("test");
        engine.calibration.as_mut().unwrap().result = Some(Rect(0.3, 0.2, 0.7, 0.8));
        let patch =
            serde_json::json!({"panel_region":[0.2,0.3,0.7,0.5],"reel_region":[0.15,0.3,0.75,0.5]});
        engine.configure_positions(&patch, (W, H)).unwrap();
        let positions = serde_json::to_value(engine.calibration.unwrap().positions()).unwrap();
        assert_eq!(positions["panel"], patch["panel_region"]);
        assert_eq!(positions["reel"], patch["reel_region"]);
        assert_eq!(positions["result"], serde_json::json!([0.3, 0.2, 0.7, 0.8]));
        assert_eq!(
            positions["bar"],
            serde_json::json!([
                0.15 + 0.6 * 0.0944,
                0.3 + 0.2 * 0.2746,
                0.15 + 0.6 * 0.9116,
                0.3 + 0.2 * 0.4648
            ])
        );
        let before = serde_json::to_value(engine.calibration).unwrap();
        let invalid = serde_json::json!({"fish_btn":[0.4,0.4],"bar_region":[0.9,0.4,0.3,0.5]});
        assert!(engine.configure_positions(&invalid, (W, H)).is_err());
        assert_eq!(serde_json::to_value(engine.calibration).unwrap(), before);
        assert!(engine.configure_positions(&patch, (0, H)).is_err());
        // Identical persisted coordinates must not overwrite a later actual
        // automatic measurement on every captured frame.
        engine.calibration.as_mut().unwrap().panel = Rect(0.25, 0.3, 0.75, 0.5);
        engine.configure_positions(&patch, (W, H)).unwrap();
        assert_eq!(engine.calibration.unwrap().panel.0, 0.25);
    }

    #[test]
    fn moving_calibration_windows_are_not_committed_until_two_observations_agree() {
        let mut engine = ready();
        engine.phase = Phase::Calibrating;
        engine.calibration_step(CalibrationStep::ReelFrame, 0., "test");
        let mut first = vec![0; STRIDE * H];
        window(&mut first, 92, 95, 388, 181);
        let mut moving = vec![0; STRIDE * H];
        window(&mut moving, 92, 98, 388, 184);
        engine.frame(0.1, &frame(&first));
        engine.frame(0.25, &frame(&moving));
        assert!(matches!(
            engine.calibration_step,
            CalibrationStep::ReelFrame
        ));
        engine.frame(0.4, &frame(&moving));
        assert!(matches!(engine.calibration_step, CalibrationStep::Reel));
        assert!((engine.calibration.unwrap().reel.1 - 98. / H as f64).abs() < 1e-9);
    }
}

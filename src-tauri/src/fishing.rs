use crate::vision::{Bar, Button, Catch, Frame, Point, Rect, WindowFinder};
use serde::{Deserialize, Serialize};
use std::{collections::VecDeque, time::Duration};

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
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CalibrationPositions {
    panel: Rect,
    reel: Rect,
    fish: Point,
    bar: Rect,
}

impl Calibration {
    fn from_panel(panel: Rect, frame: &Frame<'_>) -> Self {
        let extra = (498.0 / 437.0 - 1.0) / 2.0;
        Self {
            panel,
            reel: panel.inset(-extra, 0.0, 1.0 + extra, 1.0),
            aspect: frame.width as f64 / frame.height as f64,
        }
    }

    fn fish(self) -> Point {
        self.panel.at(0.2494, 0.8264)
    }
    fn bar(self) -> Rect {
        self.reel.inset(0.0944, 0.2746, 0.9116, 0.4648)
    }

    pub fn positions(self) -> CalibrationPositions {
        CalibrationPositions {
            panel: self.panel,
            reel: self.reel,
            fish: self.fish(),
            bar: self.bar(),
        }
    }

    fn reel_tap(self, above: bool) -> Point {
        let bar = self.bar();
        if above {
            Point(
                (bar.0 + bar.2) / 2.0,
                (self.reel.1 - (bar.3 - bar.1) * 1.5).max(0.15),
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
        let radius = ((self.panel.2 - self.panel.0) * frame.width as f64 * 0.012) as usize;
        frame.diamond(self.reel.at(0.9317, 0.8310), radius.max(3))
            && !frame.diamond(self.panel.at(0.9268, 0.8264), radius.max(3))
    }
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

    pub fn calibrate(&mut self, now: f64) -> Result<(), &'static str> {
        if self.phase.active() {
            return Err("실행 중인 작업을 먼저 중지하세요.");
        }
        self.last_scan = f64::NEG_INFINITY;
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
        self.last_progress = now;
        self.diamond_seen = false;
        self.phase(Phase::Searching, now, "Roblox 화면을 기다리는 중");
        Ok(())
    }

    fn reset_cycle(&mut self) {
        self.last_scan = f64::NEG_INFINITY;
        self.last_cast = f64::NEG_INFINITY;
        self.casts = 0;
        self.bar_seen = 0;
        self.gone_at = None;
        self.result = None;
    }

    pub fn stop(&mut self, message: &'static str) {
        self.phase(Phase::Idle, 0.0, message);
        self.window = WindowFinder::default();
        self.bar = Bar::default();
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
                (1.6..=5.5).contains(&ratio) && frame.button(rect.at(0.2494, 0.8264)).is_some()
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

    pub fn frame(&mut self, now: f64, frame: &Frame<'_>) -> Option<Tap> {
        if !self.phase.active() {
            return None;
        }
        if self.phase == Phase::Calibrating {
            if now - self.phase_at > 30.0 {
                self.stop(
                    "낚시 창을 찾지 못했습니다. Fish 또는 Exit 버튼이 보일 때 다시 보정하세요.",
                );
            } else if now - self.last_scan >= 0.25 {
                self.last_scan = now;
                if let Some(calibration) = self.find_panel(frame) {
                    self.calibration = Some(calibration);
                    self.calibration_changed = true;
                    self.stop("자동 보정 완료");
                }
            }
            return None;
        }

        let mut calibration = self.calibration?;
        if now - self.last_progress > self.tuning.bite_seconds + 60.0 {
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
            if fish.is_some() {
                match self.result.take() {
                    Some(Catch::Fail) => self.stats.fail += 1,
                    Some(Catch::Junk) => self.stats.junk += 1,
                    Some(Catch::Success) => self.stats.success += 1,
                    None => {}
                }
                self.reset_cycle();
                self.phase(Phase::Searching, now, "낚시 화면 확인 중");
                return None;
            }
            if now - self.phase_at >= 10.0 {
                self.stop("낚시 결과창을 닫지 못했습니다.");
            } else if now - self.last_scan >= 0.35 {
                self.last_scan = now;
                if let Some(window) = self.result_window(frame) {
                    self.result = frame.title(window.at(0.4972, 0.0913)).or(self.result);
                    return Some(Self::tap(frame, frame.result_close(window), 40));
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

        match fish {
            Some(Button::Fish) if now - self.last_cast >= 1.5 => {
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
                self.casts = 0;
                self.last_cast = f64::NEG_INFINITY;
                self.phase(Phase::Waiting, now, "입질 기다리는 중");
                self.last_progress = now;
                if now - self.phase_at >= self.tuning.bite_seconds {
                    self.phase(Phase::Searching, now, "입질이 없어 다시 던지는 중");
                    self.last_cast = now - 0.5;
                    return Some(Self::tap(frame, calibration.fish(), 40));
                }
            }
            None if !bar => {
                self.phase(Phase::Searching, now, "낚시 화면 확인 중");
            }
            _ => {}
        }
        None
    }
}

#[derive(Default)]
struct ReelControl {
    history: VecDeque<(f64, f64)>,
    odd: Option<f64>,
    zone: Option<(f64, f64, f64)>,
    velocity: f64,
    last_click: Option<f64>,
    rise_delay: f64,
    rising: bool,
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
        let settle = if tuning.adaptive {
            (self.rise_delay * 1.6).clamp(0.2, 0.5)
        } else {
            0.2
        };
        if let Some(click) = self.last_click {
            let elapsed = now - click;
            if elapsed * 1000.0 < f64::from(tuning.click_gap_ms)
                || (!self.rising && elapsed < settle)
            {
                return false;
            }
        }
        let target = left + (right - left) * tuning.target_percent / 100.0;
        let max_rise = (2.0 * (target - m).max(0.0))
            .min(width * 0.5)
            .max(width * 0.05);
        let predicted = m + self.velocity * tuning.lead_ms / 1000.0;
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

    #[test]
    fn calibrates_and_stops_after_unanswered_casts_with_padded_rows() {
        let mut engine = Fishing::default();
        assert!(engine.start(0.0).is_err());
        let mut bytes = vec![0; STRIDE * H];
        window(&mut bytes, 110, 95, 370, 181);
        for y in 164..168 {
            for x in 170..179 {
                put(&mut bytes, x, y, [20, 75, 205]);
            }
        }
        let frame = frame(&bytes);
        engine.calibrate(0.0).unwrap();
        assert!(engine.frame(0.1, &frame).is_none());
        assert!(engine.calibration_changed);
        engine.start(0.2).unwrap();
        for time in [0.3, 1.9, 3.5] {
            let tap = engine.frame(time, &frame).unwrap();
            assert!((170..179).contains(&tap.x) && (164..168).contains(&tap.y));
        }
        assert!(engine.frame(5.1, &frame).is_none());
        assert!(engine.phase == Phase::Idle);
        assert_eq!(engine.stats.full, 1);
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
    fn reel_controller_settles_between_clicks_and_avoids_the_right_half() {
        let mut controller = ReelControl::default();
        let tuning = Tuning::default();
        let zone = [(35.0, 65.0)];
        assert!(controller.click(0.0, Some(15.0), &zone, 100.0, tuning));
        assert!(!controller.click(0.1, Some(16.0), &zone, 100.0, tuning));
        assert!(!controller.click(0.25, Some(55.0), &zone, 100.0, tuning));
    }
}

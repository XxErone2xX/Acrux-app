//! Mobile touch prediction uses observations in fractions of the reel bar width.
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

#[derive(Clone, Copy, Deserialize, Serialize)]
#[serde(default)]
pub(crate) struct Learning {
    fall: f64,
    jump: f64,
    delay: f64,
    tap_slot: u8,
}

impl Default for Learning {
    fn default() -> Self {
        Self {
            fall: 0.29,
            jump: 0.087,
            delay: 0.15,
            tap_slot: 0,
        }
    }
}

impl Learning {
    pub(crate) fn checked(self) -> Self {
        if (0.03..=1.5).contains(&self.fall)
            && (0.02..=0.3).contains(&self.jump)
            && (0.03..=0.6).contains(&self.delay)
            && self.tap_slot <= 2
        {
            self
        } else {
            Self::default()
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Feedback {
    Pending,
    Seen,
    Lost,
}

#[derive(Default)]
pub(crate) struct Pilot {
    pub(crate) learning: Learning,
    pub(crate) moved: bool,
    touches: VecDeque<(f64, Feedback)>,
    samples: VecDeque<(f64, f64)>,
    first: Option<(f64, f64)>,
    last: Option<(f64, f64)>,
    live: bool,
    odd: bool,
    drift: f64,
    zone: Option<(f64, f64)>,
    last_fit: f64,
    slot: u8,
    slot_started: Option<f64>,
    slot_sent: u8,
    slot_seen: bool,
    switches: u8,
}

impl Pilot {
    pub(crate) fn new(learning: Learning) -> Self {
        Self {
            slot: learning.tap_slot,
            learning,
            ..Self::default()
        }
    }

    pub(crate) fn click(
        &mut self,
        at: f64,
        marker: f64,
        (left, right): (f64, f64),
        min_gap: f64,
        lead: f64,
        deadband: f64,
    ) -> bool {
        let (first_at, first_marker) = *self.first.get_or_insert((at, marker));
        self.moved |= (marker - first_marker).abs() >= 0.012;
        self.live |= self.moved || at - first_at >= 2.5;
        if !self.live {
            return false;
        }
        if self
            .last
            .is_some_and(|(time, previous)| at - time < 0.25 && (marker - previous).abs() > 0.4)
        {
            if !self.odd {
                self.odd = true;
                return false;
            }
            self.samples.clear();
            self.last = None;
        }
        self.odd = false;

        let Learning {
            fall, jump, delay, ..
        } = self.learning;
        while self.touches.front().is_some_and(|tap| at - tap.0 > 4.1) {
            self.touches.pop_front();
        }
        for (time, state) in &mut self.touches {
            if *state == Feedback::Pending && at - *time > delay + 0.2 {
                *state = Feedback::Lost;
            }
        }
        let wall = !(0.01..0.99).contains(&marker);
        if let Some((time, previous)) = self.last.filter(|(time, _)| at > *time) {
            let dt = at - time;
            let change = marker - previous;
            if change > (jump * 0.35).max(0.012) {
                let expected = ((change + fall * dt) / jump).round().clamp(1., 10.) as usize;
                let midpoint = (at + time) / 2.;
                for _ in 0..expected {
                    let Some(tap) = self
                        .touches
                        .iter_mut()
                        .filter(|tap| tap.1 == Feedback::Pending && at - tap.0 >= 0.015)
                        .min_by(|a, b| {
                            (midpoint - a.0 - delay)
                                .abs()
                                .total_cmp(&(midpoint - b.0 - delay).abs())
                        })
                    else {
                        break;
                    };
                    tap.1 = Feedback::Seen;
                    if self.slot_started.is_some_and(|start| tap.0 >= start) {
                        self.slot_seen = true;
                        self.learning.tap_slot = self.slot;
                    }
                    let gain = change + fall * dt;
                    if expected == 1 && !wall && (0.02..=0.3).contains(&gain) {
                        self.learning.jump = jump * 0.7 + gain * 0.3;
                    }
                }
            } else if !wall
                && dt >= 0.03
                && !self.touches.iter().any(|&(sent, state)| {
                    state != Feedback::Lost && (time - 0.05..=at + 0.05).contains(&(sent + delay))
                })
            {
                let speed = -change / dt;
                if (0.03..=1.5).contains(&speed) {
                    self.learning.fall = fall * 0.8 + speed * 0.2;
                }
            }
        }
        self.last = Some((at, marker));
        if wall {
            self.samples.clear();
        } else {
            self.samples.push_back((at, marker));
            while self
                .samples
                .front()
                .is_some_and(|(time, _)| at - time > 3. || self.samples.len() > 256)
            {
                self.samples.pop_front();
            }
            if self.samples.len() >= 12 && at - self.last_fit >= 0.15 {
                self.last_fit = at;
                self.fit_delay();
            }
        }

        let center = (left + right) / 2.;
        if let Some((time, previous)) = self.zone.filter(|(time, _)| at > *time && at - time < 0.3)
        {
            self.drift =
                (self.drift * 0.7 + (center - previous) / (at - time) * 0.3).clamp(-0.6, 0.6);
        }
        self.zone = Some((at, center));
        let Learning {
            fall, jump, delay, ..
        } = self.learning;
        let pending = self
            .touches
            .iter()
            .filter(|&&(time, state)| state == Feedback::Pending && at - time < delay + 0.06)
            .count() as f64;
        let horizon = (delay + lead).min(0.6);
        let predicted = (marker - fall * horizon + jump * pending).max(0.);
        let target = center + self.drift * horizon.min(0.3) - jump / 2.
            + ((right - left - jump).max(0.) * 0.15).min(0.02)
            - deadband;
        if predicted >= target
            || self
                .touches
                .back()
                .is_some_and(|tap| at - tap.0 < min_gap.max(0.07))
        {
            return false;
        }
        if !self.slot_seen
            && self.slot_sent >= 6
            && self.switches < 2
            && self.slot_started.is_some_and(|start| at - start >= 1.)
        {
            self.slot = (self.slot + 1) % 3;
            self.switches += 1;
            self.slot_started = None;
            self.slot_sent = 0;
        }
        self.slot_started.get_or_insert(at);
        self.slot_sent = self.slot_sent.saturating_add(1);
        self.touches.push_back((at, Feedback::Pending));
        true
    }

    #[cfg(target_os = "android")]
    pub(crate) fn marker(&self) -> Option<f64> {
        self.last.map(|(_, marker)| marker)
    }

    pub(crate) fn tap_slot(&self) -> u8 {
        self.slot
    }

    pub(crate) fn summary(&self) -> String {
        let Learning {
            fall, jump, delay, ..
        } = self.learning;
        format!(
            "릴링 · 터치 지연 {:.0} ms · 낙하 {:.2}/s · 상승 {:.3}",
            delay * 1000.,
            fall,
            jump
        )
    }

    fn fit_delay(&mut self) {
        let (Some(&(start, _)), Some(&(end, _))) = (self.samples.front(), self.samples.back())
        else {
            return;
        };
        let current = self.learning.delay;
        let taps: Vec<_> = self
            .touches
            .iter()
            .filter(|&&(time, state)| state != Feedback::Lost && time + 0.6 > start)
            .map(|tap| tap.0)
            .collect();
        if taps.len() < 2 {
            return;
        }
        let mut best = (f64::INFINITY, current);
        let mut current_error = f64::INFINITY;
        for step in 3..=60 {
            let candidate = f64::from(step) / 100.;
            if taps
                .iter()
                .filter(|&&tap| (start..=end).contains(&(tap + candidate)))
                .count()
                < 2
            {
                continue;
            }
            let mut touches = taps.iter().copied().peekable();
            let mut absorbed = 0;
            let mut sum = 0.;
            let mut squares = 0.;
            for &(at, marker) in &self.samples {
                while touches.next_if(|tap| *tap + candidate <= at).is_some() {
                    absorbed += 1;
                }
                let residual = marker + self.learning.fall * (at - start)
                    - self.learning.jump * f64::from(absorbed);
                sum += residual;
                squares += residual * residual;
            }
            let error = (squares - sum * sum / self.samples.len() as f64).max(0.);
            if step == (current * 100.).round() as u32 {
                current_error = error;
            }
            if error < best.0
                || error == best.0 && (candidate - current).abs() < (best.1 - current).abs()
            {
                best = (error, candidate);
            }
        }
        if (best.1 - current).abs() <= 0.035 {
            self.learning.delay = current * 0.6 + best.1 * 0.4;
        } else if best.0 < current_error * 0.8 {
            self.learning.delay = best.1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn waits_for_ready_and_accounts_for_an_unseen_touch() {
        let mut pilot = Pilot::default();
        let zone = (0.4, 0.6);
        assert!(!pilot.click(1., 0.32, zone, 0.045, 0., 0.));
        assert!(!pilot.click(2., 0.32, zone, 0.045, 0., 0.));
        assert!(pilot.click(2.1, 0.44, zone, 0.045, 0., 0.));
        assert!(!pilot.click(2.18, 0.43, zone, 0.045, 0., 0.));
        assert!(pilot.moved);
    }
}

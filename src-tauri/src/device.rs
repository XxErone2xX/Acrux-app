use crate::vision::Point;
use serde::{Deserialize, Serialize};

/// One bounded action. A new observation is required after it completes.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub(crate) enum Gesture {
    Tap {
        at: Point,
        ms: u32,
    },
    Drag {
        from: Point,
        to: Point,
        ms: u32,
    },
    Pinch {
        from: [Point; 2],
        to: [Point; 2],
        ms: u32,
    },
    Stick {
        from: Point,
        to: Point,
        jump: Option<Point>,
        ms: u32,
    },
}

pub(crate) enum Action {
    Queued(Gesture, (usize, usize)),
    Pending((usize, usize)),
}

impl Gesture {
    pub(crate) fn duration_ms(&self) -> u32 {
        match self {
            Self::Tap { ms, .. }
            | Self::Drag { ms, .. }
            | Self::Pinch { ms, .. }
            | Self::Stick { ms, .. } => *ms,
        }
    }

    pub(crate) fn packet(
        &self,
        (width, height): (usize, usize),
        id: i32,
        generation: i32,
    ) -> Result<Vec<i32>, String> {
        let ms = self.duration_ms();
        if width == 0 || height == 0 || width > 16384 || height > 16384 || !(1..=1000).contains(&ms)
        {
            return Err("화면 크기와 입력 시간을 확인하세요.".into());
        }
        let point = |p: Point| -> Result<[i32; 2], String> {
            if !p.0.is_finite()
                || !p.1.is_finite()
                || !(0.0..=1.0).contains(&p.0)
                || !(0.0..=1.0).contains(&p.1)
            {
                return Err("입력 위치가 화면 밖에 있습니다.".into());
            }
            Ok([
                ((p.0 * width as f64).round() as usize).min(width - 1) as i32,
                ((p.1 * height as f64).round() as usize).min(height - 1) as i32,
            ])
        };
        let (kind, from, extra) = match *self {
            Self::Tap { at, .. } => (0, at, Vec::new()),
            Self::Drag { from, to, .. } => (1, from, point(to)?.to_vec()),
            Self::Pinch { from, to, .. } => {
                let mut extra = point(to[0])?.to_vec();
                extra.extend(point(from[1])?);
                extra.extend(point(to[1])?);
                (2, from[0], extra)
            }
            Self::Stick { from, to, jump, .. } => {
                let mut extra = point(to)?.to_vec();
                extra.extend(jump.map(point).transpose()?.unwrap_or([-1, -1]));
                (3, from, extra)
            }
        };
        let [x, y] = point(from)?;
        let mut packet = vec![
            2,
            id,
            width as i32,
            height as i32,
            kind,
            ms as i32,
            x,
            y,
            generation,
        ];
        packet.extend(extra);
        Ok(packet)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn multi_finger_packets_preserve_viewport_generation_and_optional_jump() {
        let stick = Gesture::Stick {
            from: Point(0.2, 0.7),
            to: Point(0.3, 0.6),
            jump: Some(Point(0.9, 0.8)),
            ms: 200,
        };
        assert_eq!(
            stick.packet((1000, 500), 12, 7).unwrap(),
            vec![2, 12, 1000, 500, 3, 200, 200, 350, 7, 300, 300, 900, 400]
        );
        let pinch = Gesture::Pinch {
            from: [Point(0.6, 0.5), Point(0.9, 0.5)],
            to: [Point(0.7, 0.5), Point(0.8, 0.5)],
            ms: 300,
        };
        assert_eq!(
            pinch.packet((1000, 500), 13, 8).unwrap(),
            vec![2, 13, 1000, 500, 2, 300, 600, 250, 8, 700, 250, 900, 250, 800, 250]
        );
    }

    #[test]
    fn malformed_actions_never_become_device_packets() {
        for (at, ms) in [
            (Point(f64::NAN, 0.5), 100),
            (Point(-0.1, 0.5), 100),
            (Point(0.5, 0.5), 1001),
            (Point(0.5, 0.5), 0),
        ] {
            assert!(Gesture::Tap { at, ms }.packet((1000, 500), 1, 1).is_err());
        }
        assert!(Gesture::Tap {
            at: Point(0.5, 0.5),
            ms: 100
        }
        .packet((0, 500), 1, 1)
        .is_err());
    }
}

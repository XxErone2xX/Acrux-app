use super::*;

/// Recovery uses a new, transport-owned session and fresh in-game evidence.
/// A visible client or a successful launch ACK is never proof of joining.
pub(super) struct Recovery {
    launch: bool,
    test: bool,
    phase: u8,
    due: f64,
    deadline: f64,
    pub clicks: u64,
    baseline: u64,
}
impl Recovery {
    pub fn new(
        cfg: &Config,
        launch: bool,
        test: bool,
        now: f64,
        epoch: u64,
    ) -> Result<Self, String> {
        if launch {
            valid_link(&link(cfg))?;
        }
        Ok(Self {
            launch,
            test,
            phase: 0,
            due: now,
            deadline: now + 90.0,
            clicks: 0,
            baseline: epoch,
        })
    }
    pub fn tick(
        &mut self,
        cfg: &mut Config,
        now: f64,
        available: bool,
        in_game: bool,
        epoch: u64,
        frame: Option<&Frame<'_>>,
        words: &[Word],
    ) -> Tick {
        if now < self.due {
            return wait(self.due - now);
        }
        if self.phase == 0 {
            self.phase = 1;
            if self.launch {
                self.deadline = now + 90.0;
                return Tick::Command(Command::Launch { url: link(cfg) });
            }
        }
        if self.phase == 1 {
            if !available {
                if now >= self.deadline {
                    return Tick::Stopped("90초 동안 Roblox 실행을 확인하지 못했습니다.".into());
                }
                return wait(0.5);
            }
            self.phase = 2;
            self.due = now + cfg.number("play", "load_wait", 5.0);
            self.deadline = self.due + cfg.number("play", "max_time", 300.0).clamp(10.0, 3600.0);
            return wait(self.due - now);
        }
        if self.phase == 3 {
            return Tick::Done;
        }
        if self.phase == 4 {
            self.phase = 3;
            return Tick::Done;
        }
        // An epoch records an actual new Equipped/log/observed gameplay event.
        // Existing state from the client being replaced is deliberately ignored.
        if in_game && epoch > self.baseline {
            self.phase = 4;
            self.due = now
                + if self.test {
                    0.0
                } else {
                    cfg.number("ret", "start_wait", 7.5)
                };
            return wait(self.due - now);
        }
        if now >= self.deadline {
            return Tick::Stopped(
                "게임 입장 근거가 확인되지 않아 Play 클릭을 중지했습니다.".into(),
            );
        }
        if !available {
            return wait(0.5);
        }
        if frame.is_none() {
            return observe(true, None);
        }
        let locate = |labels: &[&str]| {
            let found: Vec<_> = words
                .iter()
                .filter(|w| {
                    labels
                        .iter()
                        .any(|label| super::label(&w.text, label, true))
                        && w.rect.0 >= 0.02
                        && w.rect.2 <= 0.98
                        && w.rect.1 >= 0.12
                        && w.rect.3 <= 0.95
                })
                .collect();
            (found.len() == 1).then(|| found[0])
        };
        let (field, word) = if let Some(word) = locate(&["Click to skip", "Skip"]) {
            ("skip_pos", word)
        } else if let Some(word) = locate(&["Play"]) {
            ("pos", word)
        } else {
            return wait(0.25);
        };
        let observed = center(word.rect);
        let at = cfg
            .point("play", field)
            .filter(|p| within(word.rect.inset(-0.25, -0.5, 1.25, 1.5), *p))
            .unwrap_or(observed);
        cfg.patch("play", field, json!([at.0, at.1]));
        self.clicks += 1;
        self.due = now + cfg.number("play", "interval", 0.15).clamp(0.05, 10.0);
        tap(at, 30)
    }
}
fn link(cfg: &Config) -> String {
    let primary = cfg.text("ret", "ps_link");
    if primary.is_empty() {
        cfg.text("biome", "ps_link")
    } else {
        primary
    }
}
fn valid_link(url: &str) -> Result<(), String> {
    let rest = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .ok_or("복귀할 Roblox 비공개 서버 링크가 필요합니다.")?;
    let host = rest.split('/').next().unwrap_or("").to_ascii_lowercase();
    if !matches!(host.as_str(), "roblox.com" | "www.roblox.com")
        || url.len() > 4096
        || url.chars().any(char::is_control)
    {
        return Err("복귀 링크는 Roblox 도메인의 서버 링크여야 합니다.".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn new_game_evidence_is_required_after_launch() {
        let mut cfg=Config::validate(json!({"play":{"pos":[0.4,0.5],"skip_pos":[0.5,0.6],"load_wait":0},"ret":{"ps_link":"https://www.roblox.com/games/15532962292/?privateServerLinkCode=abc","start_wait":0}})).unwrap();
        let mut r = Recovery::new(&cfg, true, false, 0.0, 3).unwrap();
        assert!(matches!(
            r.tick(&mut cfg, 0.0, true, true, 3, None, &[]),
            Tick::Command(Command::Launch { .. })
        ));
        assert!(matches!(
            r.tick(&mut cfg, 1.0, true, true, 3, None, &[]),
            Tick::Wait(_)
        ));
        assert!(matches!(
            r.tick(&mut cfg, 2.0, true, true, 3, None, &[]),
            Tick::Observe { .. }
        ));
        assert!(matches!(
            r.tick(&mut cfg, 3.0, true, true, 4, None, &[]),
            Tick::Wait(_)
        ));
        assert!(matches!(
            r.tick(&mut cfg, 4.0, true, true, 4, None, &[]),
            Tick::Done
        ));
        assert!(valid_link("https://www.roblox.com.attacker.test/x").is_err());
    }
}

use crate::{
    macro_step::Step,
    navigation::Motion,
    ocr::Word,
    vision::{Frame, Point, Rect},
};
use serde_json::{json, Value};

const MENU: Point = Point(33. / 1920., 673. / 1080.);
const MENU_LABELS: [(&str, f64); 6] = [
    ("seoson", 441.),
    ("ochlev", 494.),
    ("flshlng", 539.),
    ("settlng", 586.),
    ("robux", 633.),
    ("develop", 679.),
];

#[derive(Clone, Copy)]
enum Stage {
    Reset(usize),
    Open(u8),
    Read(u8),
    Close(Point, bool),
    Verify(bool),
    OpenPanel(u8),
    ReadPanel(u8),
    ClosePanel(u8),
    VerifyPanel(u8),
    Done,
}

struct MobilePositions {
    reset: [Point; 3],
    command: Point,
    menu_close: Point,
    panel_close: Point,
}

pub(crate) struct ScreenJob {
    stage: Stage,
    menu: Option<Point>,
    mobile: Option<MobilePositions>,
    reset_wait: f64,
    due: f64,
    next: Option<(Stage, f64)>,
    failures: usize,
    own: Option<bool>,
    reason: &'static str,
    logs: Vec<String>,
}

impl ScreenJob {
    pub(crate) fn new(config: &Value, check_server: bool, at: f64) -> Self {
        let menu = serde_json::from_value::<Point>(config["base"]["menu_pos"].clone())
            .ok()
            .filter(|point| (0. ..1.).contains(&point.0) && (0. ..1.).contains(&point.1));
        Self {
            stage: Stage::Reset(0),
            mobile: None,
            menu: check_server.then_some(menu.unwrap_or(MENU)),
            reset_wait: config["move"]["reset_wait"]
                .as_f64()
                .unwrap_or(3.5)
                .clamp(0.5, 30.),
            due: at,
            next: None,
            failures: 0,
            own: None,
            reason: "",
            logs: Vec::new(),
        }
    }

    pub(crate) fn mobile(
        config: &Value,
        positions: &std::collections::BTreeMap<String, Point>,
        at: f64,
    ) -> Option<Self> {
        let point = |key| positions.get(key).copied();
        let menu = point("game_menu")?;
        let mut job = Self::new(config, true, at);
        job.menu = Some(menu);
        job.mobile = Some(MobilePositions {
            reset: [
                point("menu_btn")?,
                point("reset_btn")?,
                point("reset_confirm")?,
            ],
            command: point("private_server_commands")?,
            menu_close: point("ps_menu_close")?,
            panel_close: point("ps_panel_close")?,
        });
        Some(job)
    }

    pub(crate) fn reset_due(&self, at: f64) -> bool {
        at >= self.due && matches!(self.stage, Stage::Reset(_))
    }

    fn input(&mut self, motion: Motion, next: Stage, wait: f64) -> Step {
        self.next = Some((next, wait));
        Step::Input(motion)
    }

    pub(crate) fn tick(&mut self, at: f64, frame: Option<&Frame<'_>>, words: &[Word]) -> Step {
        if at < self.due {
            return Step::Wait(self.due - at);
        }
        match self.stage {
            Stage::Reset(index) => {
                let (keys, next, wait): (&'static [u16], _, _) = match index {
                    0 => (&[0x1b], Stage::Reset(1), 0.5),
                    1 => (&[0x52], Stage::Reset(2), 0.5),
                    _ => (
                        &[0x0d],
                        self.menu.map_or(Stage::Done, |_| Stage::Open(0)),
                        self.reset_wait,
                    ),
                };
                let motion = self
                    .mobile
                    .as_ref()
                    .map_or(Motion::Keys(keys, 40), |mobile| {
                        Motion::Tap(mobile.reset[index.min(2)], 40)
                    });
                self.input(motion, next, wait)
            }
            Stage::Open(attempt) => self.input(
                Motion::Tap(self.menu.unwrap_or(MENU), 40),
                Stage::Read(attempt),
                0.9,
            ),
            Stage::Read(_) | Stage::Verify(_) | Stage::ReadPanel(_) | Stage::VerifyPanel(_)
                if frame.is_none() =>
            {
                Step::Observe {
                    region: Some(Rect(0.16, 0.18, 0.84, 0.92)),
                    ocr: true,
                }
            }
            Stage::Read(attempt) => {
                let menu = measure(words, frame.unwrap());
                if let Some((own, close)) = menu {
                    if self.mobile.is_some() {
                        self.stage = Stage::OpenPanel(0);
                        return self.tick(at, None, &[]);
                    }
                    if !own && attempt % 2 == 0 {
                        self.stage = Stage::Read(attempt + 1);
                        self.due = at + 0.5;
                        return Step::Wait(0.5);
                    }
                    self.own = Some(own);
                    self.stage = Stage::Close(close, false);
                } else if attempt % 2 == 0 {
                    self.stage = Stage::Read(attempt + 1);
                    self.due = at + 0.6;
                    return Step::Wait(0.6);
                } else if attempt < 3 {
                    self.stage = Stage::Open(attempt + 1);
                } else {
                    self.reason = "menu";
                    self.stage = Stage::Done;
                }
                self.tick(at, None, &[])
            }
            Stage::Close(point, fallback) => {
                self.input(Motion::Tap(point, 40), Stage::Verify(fallback), 0.6)
            }
            Stage::Verify(fallback) => {
                if measure(words, frame.unwrap()).is_some() {
                    if !fallback {
                        self.stage = Stage::Close(self.menu.unwrap_or(MENU), true);
                        return self.tick(at, None, &[]);
                    }
                    self.logs.push(
                        "서버 확인 후 메뉴를 닫지 못했습니다. 게임에서 메뉴를 닫아 주세요.".into(),
                    );
                }
                self.stage = Stage::Done;
                Step::Done
            }
            Stage::OpenPanel(attempt) => {
                let command = self.mobile.as_ref().unwrap().command;
                self.input(Motion::Tap(command, 40), Stage::ReadPanel(attempt), 0.5)
            }
            Stage::ReadPanel(attempt) => {
                if panel_visible(words) {
                    self.own = Some(true);
                    self.stage = Stage::ClosePanel(0);
                } else if attempt < 5 {
                    self.stage = if attempt == 2 {
                        Stage::OpenPanel(attempt + 1)
                    } else {
                        Stage::ReadPanel(attempt + 1)
                    };
                    self.due = at + 0.45;
                    return Step::Wait(0.45);
                } else if measure(words, frame.unwrap()).is_some() {
                    self.own = Some(false);
                    self.stage = Stage::Close(self.mobile.as_ref().unwrap().menu_close, false);
                } else {
                    self.reason = "menu";
                    self.stage = Stage::Done;
                }
                self.tick(at, None, &[])
            }
            Stage::ClosePanel(attempt) => {
                let close = self.mobile.as_ref().unwrap().panel_close;
                self.input(Motion::Tap(close, 40), Stage::VerifyPanel(attempt), 0.6)
            }
            Stage::VerifyPanel(attempt) => {
                if panel_visible(words) {
                    if attempt == 0 {
                        self.stage = Stage::ClosePanel(1);
                        return self.tick(at, None, &[]);
                    }
                    self.logs
                        .push("서버 확인 후 관리창을 닫지 못했습니다.".into());
                } else if measure(words, frame.unwrap()).is_some() {
                    self.stage = Stage::Close(self.mobile.as_ref().unwrap().menu_close, false);
                    return self.tick(at, None, &[]);
                }
                self.stage = Stage::Done;
                Step::Done
            }
            Stage::Done => Step::Done,
        }
    }

    pub(crate) fn ack(&mut self, ok: bool, at: f64) {
        let Some((next, wait)) = self.next.take() else {
            return;
        };
        if ok {
            self.stage = next;
            self.due = at + wait;
        } else if self.menu.is_none() {
            self.failures += 1;
            if self.failures < 5 {
                self.stage = Stage::Reset(0);
                self.due = at + (self.failures * 2) as f64;
                if self.failures == 1 {
                    self.logs
                        .push("시작 재설정에 실패했습니다. 잠시 후 다시 시도합니다.".into());
                }
            } else {
                self.logs
                    .push("시작 재설정이 5회 실패해 재설정 없이 실행합니다.".into());
                self.stage = Stage::Done;
                self.due = at;
            }
        } else {
            self.own = None;
            self.reason = "error";
            self.stage = Stage::Done;
            self.due = at;
        }
    }

    pub(crate) fn status(&self) -> Value {
        let message = if self.menu.is_none() {
            "시작 재설정 중"
        } else {
            match (self.stage, self.own) {
                (Stage::Done, Some(true)) => "내 비공개 서버입니다.",
                (Stage::Done, Some(false)) => "내 비공개 서버가 아닙니다.",
                (Stage::Done, None) => "비공개 서버를 확인하지 못했습니다.",
                _ => "비공개 서버 확인 중",
            }
        };
        json!({"msg":message, "own":self.own, "why":self.reason})
    }

    pub(crate) fn take_logs(&mut self) -> Vec<String> {
        std::mem::take(&mut self.logs)
    }
}

fn center(rect: Rect) -> Point {
    rect.at(0.5, 0.5)
}

fn label(text: &str) -> String {
    text.chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|character| match character.to_ascii_lowercase() {
            'a' => 'o',
            'i' | '1' => 'l',
            ch => ch,
        })
        .collect()
}

fn panel_visible(words: &[Word]) -> bool {
    let labels: String = words
        .iter()
        .filter(|word| {
            let Point(x, y) = center(word.rect);
            (0.22..0.78).contains(&x) && (0.2..0.8).contains(&y)
        })
        .map(|word| label(&word.text))
        .collect();
    let headings = [
        "server information",
        "server logs",
        "server control",
        "player control",
        "player list",
    ];
    let matches = headings
        .iter()
        .filter(|heading| labels.contains(&label(heading)))
        .count();
    matches >= 2 || matches == 1 && labels.contains(&label("Private Server Management"))
}

fn measure(words: &[Word], frame: &Frame<'_>) -> Option<(bool, Point)> {
    let labels: Vec<_> = words
        .iter()
        .map(|word| (label(&word.text), center(word.rect)))
        .collect();
    let mut rows = Vec::new();
    for (key, reference) in MENU_LABELS {
        if let Some((_, point)) = labels
            .iter()
            .filter(|(text, point)| {
                text.contains(key)
                    && (point.0 - 0.5).abs() * (frame.width as f64) < frame.height as f64 * 0.28
            })
            .min_by(|(_, a), (_, b)| {
                (a.1 - reference / 1080.)
                    .abs()
                    .total_cmp(&(b.1 - reference / 1080.).abs())
            })
        {
            rows.push((reference, point.1));
        }
    }
    let (&(first, top), &(last, bottom)) = (rows.first()?, rows.last()?);
    if last <= first || bottom <= top {
        return None;
    }
    let height = frame.height as f64;
    let width = frame.width as f64;
    let scale = (bottom - top) * height / (last - first);
    let title = labels.iter().find(|(text, point)| {
        matches!(text.as_str(), "menu" | "menux")
            && point.1 < top
            && (point.0 - 0.5).abs() * width < height * 0.2
    });
    let (cx, ty) = title.map_or((0.5, top - (first - 389.) * scale / height), |(_, p)| {
        (p.0, p.1)
    });
    let own = labels.iter().any(|(text, point)| {
        text.contains("prlv")
            && (text.contains("serv") || text.contains("comm"))
            && point.1 > bottom
            && (point.0 - cx).abs() * width < height * 0.25
    });
    Some((
        own,
        Point(
            (cx + 163. * scale / width).clamp(0., 1.),
            (ty - scale / height).clamp(0., 1.),
        ),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checks_private_commands_only_below_a_recognized_menu() {
        let pixels = vec![0; 192 * 108 * 3];
        let frame = Frame::new(&pixels, 192, 108, 192 * 3, 3, false).unwrap();
        let word = |text: &str, x, y| Word {
            text: text.into(),
            rect: Rect(x, y, x, y),
        };
        let mut words = vec![
            word("Menu", 0.5, 0.36),
            word("Season", 0.5, 0.41),
            word("Settings", 0.5, 0.54),
            word("Privote Server Commonds", 0.8, 0.67),
        ];
        assert_eq!(measure(&words, &frame).map(|m| m.0), Some(false));
        words.push(word("Private Server Commands", 0.5, 0.68));
        assert_eq!(measure(&words, &frame).map(|m| m.0), Some(true));
        assert!(measure(&words[2..], &frame).is_none());
    }

    #[test]
    fn rejoin_accepts_roblox_private_links_and_rejects_other_hosts() {
        let share = "roblox://navigation/share_links?code=abc_123&type=Server";
        assert_eq!(
            roblox_deep_link("https://www.roblox.com/share?code=abc_123&type=Server").as_deref(),
            Some(share)
        );
        assert_eq!(roblox_deep_link(share).as_deref(), Some(share));
        let private = "roblox://placeId=123&linkCode=abc";
        assert_eq!(roblox_deep_link(private).as_deref(), Some(private));
        assert_eq!(
            roblox_deep_link("https://www.roblox.com/games/123/title?privateServerLinkCode=abc")
                .as_deref(),
            Some(private)
        );
        assert!(
            roblox_deep_link("https://www.roblox.com.example.org/share?code=abc&type=Server")
                .is_none()
        );
        assert!(
            roblox_deep_link("https://www.roblox.com/share?code=abc%26bad&type=Server").is_none()
        );
    }

    #[test]
    fn gives_up_after_five_startup_failures() {
        let mut job = ScreenJob::new(&json!({}), false, 0.);
        let mut at = 0.;
        for attempt in 1..=5 {
            assert!(matches!(job.tick(at, None, &[]), Step::Input(_)));
            job.ack(false, at);
            if attempt < 5 {
                assert!(matches!(job.tick(at, None, &[]), Step::Wait(_)));
                at += f64::from(attempt * 2);
            }
        }
        assert!(matches!(job.tick(at, None, &[]), Step::Done));
    }
}

pub(crate) fn roblox_deep_link(value: &str) -> Option<String> {
    let url = reqwest::Url::parse(value.trim()).ok()?;
    if !(matches!(url.scheme(), "https" | "roblox")
        && url.username().is_empty()
        && url.password().is_none()
        && url.port().is_none())
    {
        return None;
    }
    if url.scheme() == "https" && !matches!(url.host_str(), Some("roblox.com" | "www.roblox.com")) {
        return None;
    }
    let code_ok = |value: &str| {
        (1..=128).contains(&value.len())
            && value
                .bytes()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == b'-' || ch == b'_')
    };
    // Roblox's placeId deep link puts the fields after :// without a query delimiter.
    let query = url.query().or_else(|| {
        value
            .trim()
            .strip_prefix("roblox://placeId=")
            .and_then(|_| value.trim().strip_prefix("roblox://"))
    })?;
    let pairs: Vec<_> = reqwest::Url::parse(&format!("https://www.roblox.com/?{query}"))
        .ok()?
        .query_pairs()
        .into_owned()
        .collect();
    let field = |key: &str| {
        pairs
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case(key))
            .map(|(_, value)| value.as_str())
    };
    let place = field("placeId").or_else(|| {
        url.path()
            .strip_prefix("/games/")
            .and_then(|tail| tail.split('/').next())
    });
    if let (Some(place), Some(code)) = (
        place.filter(|place| {
            (1..=20).contains(&place.len()) && place.bytes().all(|ch| ch.is_ascii_digit())
        }),
        field("privateServerLinkCode")
            .or_else(|| field("privateServerLink"))
            .or_else(|| field("linkCode"))
            .filter(|code| code_ok(code)),
    ) {
        return Some(format!("roblox://placeId={place}&linkCode={code}"));
    }
    matches!(url.path(), "/share" | "/share/" | "/share_links")
        .then(|| field("code").filter(|code| code_ok(code)))
        .flatten()
        .filter(|_| field("type").is_none_or(|kind| kind.eq_ignore_ascii_case("server")))
        .map(|code| format!("roblox://navigation/share_links?code={code}&type=Server"))
}

#[cfg(any(target_os = "windows", target_os = "linux"))]
pub(crate) struct Play {
    points: Vec<Point>,
    index: usize,
    next: f64,
    interval: f64,
    load_wait: f64,
}

#[cfg(any(target_os = "windows", target_os = "linux"))]
impl Play {
    pub(crate) fn new(config: &Value) -> Self {
        let point = |key| {
            serde_json::from_value::<Point>(config["play"][key].clone())
                .ok()
                .filter(|p| (0. ..1.).contains(&p.0) && (0. ..1.).contains(&p.1))
        };
        let points = [point("pos"), point("skip_pos")]
            .into_iter()
            .flatten()
            .collect();
        Self {
            points,
            index: 0,
            next: 0.,
            interval: config["play"]["interval"]
                .as_f64()
                .unwrap_or(0.15)
                .clamp(0.1, 30.),
            load_wait: config["ret"]["start_wait"]
                .as_f64()
                .unwrap_or(7.5)
                .clamp(0., 60.),
        }
    }

    pub(crate) fn tap(&mut self, at: f64) -> Option<Motion> {
        if self.points.is_empty() {
            return None;
        }
        if self.next == 0. {
            self.next = at + self.load_wait;
        }
        if at < self.next {
            return None;
        }
        let point = self.points[self.index];
        self.index = (self.index + 1) % self.points.len();
        self.next = at + self.interval;
        Some(Motion::Tap(point, 30))
    }
}

#[cfg(any(target_os = "windows", target_os = "linux"))]
pub(crate) fn wait_for_join(generation: i32, original: Option<std::path::PathBuf>) {
    use std::{
        fs::File,
        io::{Read, Seek, SeekFrom},
        thread,
        time::Duration,
    };
    let mut cursor = None;
    loop {
        thread::sleep(Duration::from_millis(250));
        {
            let state = crate::runtime();
            if state.generation != generation || crate::now() >= state.returning_until {
                break;
            }
        }
        let Some(path) = crate::biome::latest_desktop_log() else {
            continue;
        };
        if original.as_ref() == Some(&path) {
            continue;
        }
        let Ok(mut file) = File::open(&path) else {
            continue;
        };
        let Ok(metadata) = file.metadata() else {
            continue;
        };
        let end = metadata.len();
        let offset = cursor
            .as_ref()
            .filter(|(key, _)| *key == path)
            .map_or(0, |(_, offset)| *offset)
            .min(end)
            .max(end.saturating_sub(131_072));
        if offset >= end || file.seek(SeekFrom::Start(offset)).is_err() {
            continue;
        }
        let mut bytes = Vec::new();
        if file.take(131_072).read_to_end(&mut bytes).is_err() {
            continue;
        }
        let Some(last_line) = bytes.iter().rposition(|&ch| ch == b'\n') else {
            continue;
        };
        let text = String::from_utf8_lossy(&bytes[..=last_line]);
        if text.contains(r#""state":"Equipped"#) || text.contains(r#"\"state\":\"Equipped"#) {
            let mut state = crate::runtime();
            if state.generation == generation && crate::now() < state.returning_until {
                state.returning_until = 0.;
                state.return_play = None;
                state.push_log("비공개 서버 입장을 확인했습니다.".into());
            }
            return;
        }
        cursor = Some((path, offset + last_line as u64 + 1));
    }
    let mut state = crate::runtime();
    if state.generation == generation && state.returning_until > 0. {
        state.returning_until = 0.;
        state.return_play = None;
        state.push_log("비공개 서버 입장 확인 시간이 지났습니다.".into());
    }
}

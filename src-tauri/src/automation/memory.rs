//! Deterministic 5x4 memory matching. Image signatures distinguish icons and
//! OCR digits distinguish quantities of the same icon; neither chooses actions
//! through a language/vision agent.
use super::*;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone)]
struct Card {
    at: Point,
    image: Rect,
    text: Rect,
}
#[derive(Clone)]
struct Board {
    cards: Vec<Card>,
    close: Point,
    title: Point,
    k: f64,
    chances: Rect,
}
fn offset(p: Point, x: f64, y: f64, k: f64, g: (usize, usize)) -> Option<Point> {
    let p = Point(p.0 + x * k / g.0 as f64, p.1 + y * k / g.1 as f64);
    valid_point(p).then_some(p)
}
fn area(p: Point, x: f64, y: f64, w: f64, h: f64, k: f64, g: (usize, usize)) -> Option<Rect> {
    let a = offset(p, x - w / 2.0, y - h / 2.0, k, g)?;
    let b = offset(p, x + w / 2.0, y + h / 2.0, k, g)?;
    Some(Rect(a.0, a.1, b.0, b.1))
}
fn board_at(title: Point, k: f64, g: (usize, usize)) -> Option<Board> {
    if !k.is_finite() || !(0.25..4.0).contains(&k) {
        return None;
    }
    let mut cards = Vec::with_capacity(20);
    for row in 0..4 {
        for col in 0..5 {
            let x = -136.0 + 112.75 * col as f64;
            let y = 94.0 + 121.3 * row as f64;
            cards.push(Card {
                at: offset(title, x, y, k, g)?,
                image: area(title, x, y, 84.0, 92.0, k, g)?,
                text: area(title, x, y + 38.0, 104.0, 34.0, k, g)?,
            });
        }
    }
    Some(Board {
        cards,
        close: offset(title, 0.0, 549.0, k, g)?,
        title,
        k: k * 1080.0 / g.1 as f64,
        chances: area(title, -288.0, 74.0, 110.0, 66.0, k, g)?,
    })
}
fn board_layout(words: &[Word], g: (usize, usize)) -> Option<Board> {
    let ch = words.iter().find(|w| label(&w.text, "Chances", false))?;
    let cy = center(ch.rect).1;
    let t = words
        .iter()
        .filter(|w| label(&w.text, "Memory Match", false) && center(w.rect).1 < cy)
        .max_by(|a, b| a.rect.1.total_cmp(&b.rect.1))?;
    let t = center(t.rect);
    board_at(t, (cy - t.1) * g.1 as f64 / 114.0, g)
}
fn saved_board(cfg: &Config, g: (usize, usize)) -> Option<Board> {
    let title = cfg.point("mmatch", "board_title")?;
    let k = cfg.group("mmatch")["board_k"].as_f64()? * g.1 as f64 / 1080.0;
    board_at(title, k, g)
}
fn save_board(cfg: &mut Config, b: &Board) {
    patch_point(cfg, "mmatch", "board_title", b.title);
    patch_point(cfg, "mmatch", "board_close_pos", b.close);
    cfg.patch("mmatch", "board_k", json!(b.k));
    cfg.patch("mmatch", "cal_board", json!(true));
}

#[derive(Clone, Debug)]
struct Face {
    signature: Vec<f64>,
    mean: [f64; 3],
    bright: f64,
    digits: String,
    back: bool,
    green: bool,
}
fn face(frame: &Frame<'_>, card: &Card, words: &[Word]) -> Option<Face> {
    if !frame.contains(card.image) {
        return None;
    }
    let mut signature = Vec::with_capacity(300);
    for y in 0..10 {
        for x in 0..10 {
            let p = card
                .image
                .at((x as f64 + 0.5) / 10.0, (y as f64 + 0.5) / 10.0 * 0.62);
            let rgb = frame.pixel(
                (p.0 * frame.width as f64) as usize,
                (p.1 * frame.height as f64) as usize,
            )?;
            signature.extend(rgb.map(f64::from));
        }
    }
    let (mut dark, mut white, mut green, mut bright, mut count) =
        (0usize, 0usize, 0usize, 0usize, 0usize);
    let mut sum = [0.0; 3];
    let mut light = 0;
    // Fixed sample lattice bounds work and memory independently of display size.
    for y in 0..24 {
        for x in 0..24 {
            let nx = (x as f64 + 0.5) / 24.0;
            let ny = (y as f64 + 0.5) / 24.0;
            let p = card.image.at(nx, ny);
            let rgb = frame.pixel(
                (p.0 * frame.width as f64) as usize,
                (p.1 * frame.height as f64) as usize,
            )?;
            count += 1;
            if *rgb.iter().max()? < 80 {
                dark += 1;
            }
            if (0.25..0.75).contains(&nx) && (0.25..0.75).contains(&ny) && *rgb.iter().min()? > 200
            {
                white += 1;
            }
            if rgb[1] > 110
                && i16::from(rgb[1]) > i16::from(rgb[0]) + 30
                && i16::from(rgb[1]) > i16::from(rgb[2]) + 30
            {
                green += 1;
            }
            if (0.15..0.85).contains(&nx) && (0.08..0.62).contains(&ny) {
                light += 1;
                if *rgb.iter().max()? > 90 {
                    bright += 1;
                    for i in 0..3 {
                        sum[i] += f64::from(rgb[i]);
                    }
                }
            }
        }
    }
    let digits = digits(&read(words, Some(card.text)));
    Some(Face {
        signature,
        mean: sum.map(|v| v / (bright.max(1) as f64)),
        bright: bright as f64 / light.max(1) as f64,
        digits,
        back: dark as f64 / count as f64 > 0.4
            && (0.01..0.5).contains(&(white as f64 / (count as f64 / 4.0))),
        green: green as f64 / count as f64 > 0.5,
    })
}
fn digits(s: &str) -> String {
    s.chars()
        .map(|c| if matches!(c, 'O' | 'o') { '0' } else { c })
        .filter(char::is_ascii_digit)
        .take(16)
        .collect()
}
fn difference(a: &Face, b: &Face) -> f64 {
    a.signature
        .iter()
        .zip(&b.signature)
        .map(|(a, b)| (a - b).abs())
        .sum::<f64>()
        / 300.0
}
fn same(a: &Face, b: &Face) -> bool {
    a.mean.iter().zip(b.mean).all(|(a, b)| (a - b).abs() < 28.0)
        && (a.bright - b.bright).abs() < 0.12
        && difference(a, b) < 10.0
        && (a.digits.is_empty() || b.digits.is_empty() || a.digits == b.digits)
}
fn hidden(frame: &Frame<'_>, board: &Board) -> Option<Vec<Face>> {
    let cards: Vec<_> = board
        .cards
        .iter()
        .map(|c| face(frame, c, &[]))
        .collect::<Option<_>>()?;
    let first = cards.first()?;
    cards
        .iter()
        .all(|c| c.back && !c.green && difference(c, first) < 8.0)
        .then_some(cards)
}

pub(super) fn parse_wait(raw: &str) -> Option<f64> {
    let raw = raw
        .replace("시간", "h")
        .replace("時間", "h")
        .replace('분', "m")
        .replace('分', "m")
        .replace('초', "s")
        .replace('秒', "s");
    let lower = raw.to_ascii_lowercase();
    let rest = lower.rsplit_once("fter").map_or(lower.as_str(), |(_, s)| s);
    let s: String = rest
        .chars()
        .map(|c| match c {
            'o' => '0',
            'i' | 'l' | '|' => '1',
            c => c,
        })
        .collect();
    let mut n = String::new();
    let (mut h, mut m, mut sec) = (None, None, None);
    for c in s.chars() {
        if c.is_ascii_digit() {
            n.push(c);
            if n.len() > 4 {
                n.clear();
            }
        } else if c.is_whitespace() {
        } else {
            let value = n.parse::<u32>().ok();
            match c {
                'h' => h = value,
                'm' => m = value,
                's' => sec = value,
                _ => {}
            }
            n.clear();
        }
    }
    if let (Some(h), Some(m)) = (h, m) {
        if h <= 99 && m < 60 {
            return Some(f64::from(h * 3600 + m * 60));
        }
    }
    if let (Some(m), Some(s)) = (m, sec) {
        if m <= 99 && s < 60 {
            return Some(f64::from(m * 60 + s));
        }
    }
    None
}
struct Notice {
    start: bool,
    wait: f64,
    at: Point,
    close: Point,
    ad: Option<Point>,
    title: Point,
    k: f64,
}
fn notice(words: &[Word], g: (usize, usize)) -> Option<Notice> {
    let head = words.iter().find(|w| {
        let n = ocr_norm(&w.text);
        label(&w.text, "Notification", false)
            || n.contains("flcotlon")
            || (n.starts_with("notl") && n.contains("cot"))
    });
    let avail = words
        .iter()
        .find_map(|w| parse_wait(&w.text).map(|s| (w, s)));
    let ad = words.iter().find(|w| {
        let n = ocr_norm(&w.text);
        label(&w.text, "Watch AD", false)
            || n.contains("wotchod")
            || n.contains("lowerthecool")
            || n.contains("cooltlmefor")
    });
    let low = if let Some((w, _)) = avail {
        w
    } else {
        words
            .iter()
            .filter(|w| {
                let n = ocr_norm(&w.text);
                (label(&w.text, "Start Memory Match", false)
                    || n.ends_with("tortmemorymotch")
                    || (n.contains("ovolloble") && n.contains("reody")))
                    && head.is_none_or(|h| {
                        (center(w.rect).1 - center(h.rect).1) * g.1 as f64
                            > 157.0 * 0.65 * g.1 as f64 / 1080.0
                    })
                    && ad.is_none_or(|a| {
                        let d = (center(a.rect).1 - center(w.rect).1) * g.1 as f64;
                        (0.0..100.0 * g.1 as f64 / 1080.0).contains(&d)
                    })
            })
            .max_by(|a, b| a.rect.1.total_cmp(&b.rect.1))?
    };
    let at = center(low.rect);
    let k = head
        .map(|h| (at.1 - center(h.rect).1) * g.1 as f64 / 157.0)
        .filter(|k| (0.25..4.0).contains(k))
        .unwrap_or(g.1 as f64 / 1080.0);
    let title = head
        .map(|w| center(w.rect))
        .or_else(|| offset(at, 0.0, -157.0, k, g))?;
    let close = offset(title, 271.0, -3.0, k, g)?;
    Some(Notice {
        start: avail.is_none(),
        wait: avail.map_or(0.0, |(_, s)| s),
        at,
        close,
        ad: ad.map(|w| center(w.rect)),
        title,
        k,
    })
}
fn save_notice(cfg: &mut Config, n: &Notice, g: (usize, usize)) {
    for (key, p) in [
        ("note_close_pos", Some(n.close)),
        ("note_btn_pos", Some(n.at)),
        ("ad_pos", n.ad),
    ] {
        if let Some(p) = p {
            patch_point(cfg, "mmatch", key, p);
        }
    }
    if let Some(r) = area(n.title, 0.0, 105.0, 620.0, 270.0, n.k, g) {
        cfg.patch("mmatch", "note_region", json!([r.0, r.1, r.2, r.3]));
    }
    cfg.patch("mmatch", "cal_note", json!(true));
}

/// Pixel evidence of Roblox's rewarded-ad panel and its pause/play indicator.
fn ad_panel(frame: &Frame<'_>) -> Option<(Point, bool)> {
    let g = (frame.width, frame.height);
    let mut dark = 0usize;
    let mut count = 0usize;
    for y in 0..16 {
        for x in 0..6 {
            let p = Point(
                0.05 + (x as f64 + 0.5) / 6.0 * 0.13,
                0.28 + (y as f64 + 0.5) / 16.0 * 0.46,
            );
            let rgb = frame.pixel((p.0 * g.0 as f64) as usize, (p.1 * g.1 as f64) as usize)?;
            count += 1;
            if rgb.iter().all(|v| *v < 12) {
                dark += 1;
            }
        }
    }
    if dark * 10 < count * 9 {
        return None;
    }
    let scale = g.1 as f64 / 1080.0;
    let expected = Point(
        (1513.0 / 1920.0 * g.0 as f64
            + (g.0 as f64 / g.1 as f64 - 16.0 / 9.0).max(0.0) * g.1 as f64 * 0.03)
            / g.0 as f64,
        239.0 / 1080.0,
    );
    let radius = (40.0 * scale).max(12.0) as isize;
    let (cx, cy) = (
        (expected.0 * g.0 as f64) as isize,
        (expected.1 * g.1 as f64) as isize,
    );
    let (mut sx, mut sy, mut white) = (0usize, 0usize, 0usize);
    for dy in -radius..=radius {
        for dx in -radius..=radius {
            let (x, y) = (cx + dx, cy + dy);
            if x < 0 || y < 0 {
                continue;
            }
            if frame
                .pixel(x as usize, y as usize)
                .is_some_and(|rgb| rgb.iter().all(|v| *v > 180))
            {
                sx += x as usize;
                sy += y as usize;
                white += 1;
            }
        }
    }
    if (white as f64) < 60.0 * scale * scale {
        return None;
    }
    let close = Point(
        sx as f64 / white as f64 / g.0 as f64,
        sy as f64 / white as f64 / g.1 as f64,
    );
    let icon = offset(close, 3.0, 582.0, scale, g)?;
    let mut columns = Vec::new();
    for x in -10..=10 {
        let mut has = false;
        for y in -8..=8 {
            let px = (icon.0 * g.0 as f64 + x as f64 * scale) as usize;
            let py = (icon.1 * g.1 as f64 + y as f64 * scale) as usize;
            if frame
                .pixel(px, py)
                .is_some_and(|rgb| rgb.iter().all(|v| *v > 200))
            {
                has = true;
                break;
            }
        }
        columns.push(has);
    }
    let first = columns.iter().position(|v| *v)?;
    let last = columns.iter().rposition(|v| *v)?;
    if last <= first {
        return None;
    }
    let ended = columns[first..=last].iter().all(|v| *v);
    Some((close, ended))
}

pub(super) struct Memory {
    calibrate: bool,
    board_only: bool,
    phase: u8,
    due: f64,
    deadline: f64,
    started: f64,
    board: Option<Board>,
    backs: Vec<Face>,
    known: BTreeMap<usize, Face>,
    done: BTreeSet<usize>,
    bad: BTreeSet<(usize, usize)>,
    first: usize,
    second: usize,
    previous: Option<Face>,
    stable: u8,
    retries: u8,
    presses: u32,
    chances: u32,
    zero: u8,
    rounds: u32,
    cooldown: f64,
    ad_before: f64,
    ad_start: f64,
    ad_ended: u8,
    ad_fail: u8,
    pub played: u64,
    pub ads: u64,
    pub next_at: f64,
    pub logs: Vec<String>,
}
impl Memory {
    pub fn new(calibrate: bool, board_only: bool, now: f64) -> Self {
        Self {
            calibrate,
            board_only,
            phase: if board_only {
                3
            } else if calibrate {
                1
            } else {
                0
            },
            due: now,
            deadline: now + 120.0,
            started: now,
            board: None,
            backs: Vec::new(),
            known: BTreeMap::new(),
            done: BTreeSet::new(),
            bad: BTreeSet::new(),
            first: 0,
            second: 0,
            previous: None,
            stable: 0,
            retries: 0,
            presses: 0,
            chances: 10,
            zero: 0,
            rounds: 0,
            cooldown: 0.0,
            ad_before: 0.0,
            ad_start: 0.0,
            ad_ended: 0,
            ad_fail: 0,
            played: 0,
            ads: 0,
            next_at: now + 600.0,
            logs: Vec::new(),
        }
    }
    pub fn tick(
        &mut self,
        cfg: &mut Config,
        now: f64,
        g: (usize, usize),
        frame: Option<&Frame<'_>>,
        words: &[Word],
    ) -> Tick {
        if now < self.due {
            return wait(self.due - now);
        }
        if self.phase == 0 {
            self.phase = 1;
            return Tick::Travel(Destination::Match);
        }
        if self.phase == 1 {
            self.phase = 2;
            self.deadline = now + if self.calibrate { 120.0 } else { 15.0 };
            self.due = now + cfg.number("mmatch", "e_wait", 2.0);
            return key(0x45);
        }
        let Some(frame) = frame else {
            return observe(true, None);
        };
        match self.phase {
            2 => {
                let Some(n) = notice(words, g) else {
                    return self.timeout(now, "메모리 매치 알림 창을 확인하지 못했습니다.");
                };
                save_notice(cfg, &n, g);
                if self.calibrate {
                    self.phase = 30;
                    self.deadline = now + 4.0;
                    self.due = now + 0.5;
                    return tap(n.close, 40);
                }
                if self.phase == 2 && self.ad_before > 0.0 {
                    if n.start || self.ad_before - n.wait > 1800.0 {
                        self.ads += 1;
                        self.logs.push("광고 뒤 실제 쿨타임 감소 확인".into());
                        self.ad_fail = 0;
                    } else {
                        self.ad_fail += 1;
                    }
                    self.ad_before = 0.0;
                }
                self.cooldown = n.wait;
                if self.rounds >= 20 {
                    self.next_at = now + n.wait + 60.0;
                    self.phase = 30;
                    self.deadline = now + 4.0;
                    return tap(n.close, 40);
                }
                if n.start {
                    self.phase = 3;
                    self.started = now;
                    self.deadline = now + 30.0;
                    self.due = now + 1.5;
                    self.rounds += 1;
                    self.board = None;
                    self.known.clear();
                    self.done.clear();
                    self.bad.clear();
                    self.presses = 0;
                    self.chances = 10;
                    self.zero = 0;
                    return tap(n.at, 60);
                }
                if let Some(ad) = n.ad {
                    if self.ads < 8 && self.ad_fail < 2 {
                        self.ad_before = n.wait;
                        self.ad_start = now;
                        self.phase = 20;
                        self.deadline = now + 15.0;
                        self.due = now + 1.0;
                        self.ad_ended = 0;
                        return tap(ad, 40);
                    }
                }
                self.next_at = now + n.wait + 60.0;
                self.phase = 30;
                self.deadline = now + 4.0;
                self.due = now + 0.5;
                tap(n.close, 40)
            }
            3 => {
                let lay = board_layout(words, g).or_else(|| saved_board(cfg, g));
                let Some(board) = lay else {
                    return self.timeout(now, "Memory Match · CHANCES 보드를 확인하지 못했습니다.");
                };
                if self.board_only {
                    if board_layout(words, g).is_none() {
                        return self
                            .timeout(now, "현재 화면의 실제 보드 글자를 확인하지 못했습니다.");
                    }
                    save_board(cfg, &board);
                    self.logs
                        .push("실제 보드 글자로 위치 보정 완료 · 카드 입력 없음".into());
                    return Tick::Done;
                }
                if now - self.started < 5.5 {
                    return wait(5.5 - (now - self.started));
                }
                let Some(backs) = hidden(frame, &board) else {
                    return self.timeout(now, "20장 카드의 동일한 뒷면을 확인하지 못했습니다.");
                };
                save_board(cfg, &board);
                self.board = Some(board);
                self.backs = backs;
                self.phase = 4;
                self.deadline = now + 600.0;
                self.tick(cfg, now, g, Some(frame), words)
            }
            4 => {
                if self.done.len() == 20
                    || self.presses >= 40
                    || self.zero >= 2
                    || now >= self.deadline
                    || find(words, &["Close"], true).is_some()
                {
                    self.phase = 10;
                    self.deadline = now + 8.0;
                    return self.tick(cfg, now, g, Some(frame), words);
                }
                let board = self.board.as_ref().unwrap();
                let chances = digits(&read(words, Some(board.chances)))
                    .parse::<u32>()
                    .ok()
                    .filter(|v| *v <= 10 && *v <= self.chances && self.chances - *v <= 1);
                if let Some(n) = chances {
                    self.chances = n;
                    if n == 0 {
                        self.zero += 1;
                    } else {
                        self.zero = 0;
                    }
                    if self.zero >= 2 {
                        self.phase = 10;
                        self.deadline = now + 8.0;
                        return self.tick(cfg, now, g, Some(frame), words);
                    }
                }
                let unexpected = board
                    .cards
                    .iter()
                    .enumerate()
                    .filter(|(i, c)| {
                        !self.done.contains(i)
                            && face(frame, c, words).is_some_and(|f| !f.back && !f.green)
                    })
                    .count();
                if unexpected >= 3 {
                    return Tick::Stopped(
                        "예상 밖으로 카드가 3장 이상 열려 재관측이 필요합니다.".into(),
                    );
                }
                let pair = self.known.iter().find_map(|(a, fa)| {
                    self.known
                        .iter()
                        .find(|(b, fb)| a < *b && !self.bad.contains(&(*a, **b)) && same(fa, fb))
                        .map(|(b, _)| (*a, *b))
                });
                let candidate = pair
                    .map(|p| p.0)
                    .or_else(|| {
                        (0..20).find(|i| !self.done.contains(i) && !self.known.contains_key(i))
                    })
                    .or_else(|| (0..20).find(|i| !self.done.contains(i)));
                let Some(first) = candidate else {
                    return Tick::Stopped("남은 메모리 카드 후보가 없습니다.".into());
                };
                self.first = first;
                self.second = pair.map_or(usize::MAX, |p| p.1);
                self.retries = 0;
                self.previous = None;
                self.stable = 0;
                self.phase = 5;
                self.started = now;
                self.due = now + cfg.number("mmatch", "flip_wait", 0.35).max(0.15);
                self.presses += 1;
                tap(board.cards[first].at, 100)
            }
            5 | 7 => {
                let index = if self.phase == 5 {
                    self.first
                } else {
                    self.second
                };
                let board = self.board.as_ref().unwrap();
                let Some(f) = face(frame, &board.cards[index], words) else {
                    return observe(true, None);
                };
                if !f.back && difference(&f, &self.backs[index]) > 12.0 {
                    if self
                        .previous
                        .as_ref()
                        .is_some_and(|p| difference(p, &f) < 4.0)
                    {
                        self.stable += 1;
                    } else {
                        self.stable = 0;
                    }
                    self.previous = Some(f.clone());
                    if self.stable >= 1 {
                        self.known.insert(index, f);
                        self.previous = None;
                        self.stable = 0;
                        if self.phase == 5 {
                            if self.second == usize::MAX {
                                self.second = self
                                    .known
                                    .iter()
                                    .find(|(i, f)| {
                                        **i != index
                                            && !self.done.contains(i)
                                            && !self.bad.contains(&(index.min(**i), index.max(**i)))
                                            && same(self.known.get(&index).unwrap(), f)
                                    })
                                    .map(|(i, _)| *i)
                                    .or_else(|| {
                                        (0..20).find(|i| {
                                            *i != index
                                                && !self.done.contains(i)
                                                && !self.known.contains_key(i)
                                        })
                                    })
                                    .or_else(|| {
                                        (0..20).find(|i| {
                                            *i != index
                                                && !self.done.contains(i)
                                                && !self
                                                    .bad
                                                    .contains(&(index.min(*i), index.max(*i)))
                                        })
                                    })
                                    .unwrap_or(usize::MAX);
                            }
                            if self.second == usize::MAX {
                                return Tick::Stopped(
                                    "안전하게 뒤집을 두 번째 카드가 없습니다.".into(),
                                );
                            }
                            self.phase = 7;
                            self.started = now;
                            self.retries = 0;
                            self.presses += 1;
                            self.due = now + cfg.number("mmatch", "flip_wait", 0.35).max(0.15);
                            return tap(board.cards[self.second].at, 100);
                        }
                        self.phase = 8;
                        self.started = now;
                        self.due = now + 0.15;
                        return observe(true, None);
                    }
                } else {
                    self.previous = None;
                    self.stable = 0;
                }
                if now - self.started >= 3.5 {
                    return Tick::Stopped("카드 뒤집기 결과가 안정되지 않았습니다.".into());
                }
                if f.back
                    && now - self.started >= f64::from(self.retries + 1)
                    && self.retries < 2
                    && self.presses < 40
                {
                    self.retries += 1;
                    self.presses += 1;
                    self.due = now + 0.35;
                    return tap(board.cards[index].at, 100);
                }
                self.due = now + 0.15;
                observe(true, None)
            }
            8 => {
                let board = self.board.as_ref().unwrap();
                let (Some(a), Some(b)) = (
                    face(frame, &board.cards[self.first], words),
                    face(frame, &board.cards[self.second], words),
                ) else {
                    return observe(true, None);
                };
                if a.green && b.green {
                    self.done.insert(self.first);
                    self.done.insert(self.second);
                    self.known.remove(&self.first);
                    self.known.remove(&self.second);
                    self.phase = 4;
                    return self.tick(cfg, now, g, Some(frame), words);
                }
                if a.back
                    && b.back
                    && difference(&a, &self.backs[self.first]) < 8.0
                    && difference(&b, &self.backs[self.second]) < 8.0
                {
                    self.bad
                        .insert((self.first.min(self.second), self.first.max(self.second)));
                    self.phase = 4;
                    return self.tick(cfg, now, g, Some(frame), words);
                }
                if now - self.started > 4.0 {
                    return Tick::Stopped("카드 짝 결과(초록/뒷면)를 확인하지 못했습니다.".into());
                }
                self.due = now + 0.15;
                observe(true, None)
            }
            10 => {
                if let Some(button) = find(words, &["Close"], true) {
                    self.phase = 11;
                    self.deadline = now + 5.0;
                    self.due = now + 0.5;
                    return tap(center(button.rect), 60);
                }
                self.timeout(now, "메모리 매치 종료 Close 버튼을 확인하지 못했습니다.")
            }
            11 => {
                if board_layout(words, g).is_none() && find(words, &["Close"], true).is_none() {
                    self.played += 1;
                    self.logs.push(format!(
                        "메모리 매치 종료 확인 · {}쌍 · 입력 {}회",
                        self.done.len() / 2,
                        self.presses
                    ));
                    self.phase = 1;
                    self.due = now + 1.0;
                    return wait(1.0);
                }
                self.timeout(now, "메모리 매치 보드가 닫히지 않았습니다.")
            }
            20 => {
                if ad_panel(frame).is_some() {
                    self.phase = 21;
                    self.deadline = now + 90.0;
                    self.due = now + 0.5;
                    return observe(false, None);
                }
                self.timeout(now, "광고 패널의 실제 픽셀을 확인하지 못했습니다.")
            }
            21 => {
                let Some((close, ended)) = ad_panel(frame) else {
                    return Tick::Stopped("광고 화면을 잃어 닫기 위치를 추정하지 않습니다.".into());
                };
                if ended {
                    self.ad_ended += 1;
                } else {
                    self.ad_ended = 0;
                }
                if self.ad_ended >= 2 && now - self.ad_start >= 10.0 {
                    self.phase = 22;
                    self.deadline = now + 8.0;
                    self.due = now + 1.0;
                    return tap(close, 60);
                }
                if now >= self.deadline {
                    return Tick::Stopped(
                        "90초 동안 광고 종료 아이콘을 확인하지 못했습니다.".into(),
                    );
                }
                self.due = now + 0.5;
                observe(false, None)
            }
            22 => {
                if ad_panel(frame).is_none() {
                    self.phase = 1;
                    self.due = now + 3.0;
                    return wait(3.0);
                }
                self.timeout(now, "광고가 닫히지 않아 중지했습니다.")
            }
            _ => {
                if notice(words, g).is_none() {
                    Tick::Done
                } else {
                    self.timeout(now, "메모리 매치 알림 창이 닫히지 않았습니다.")
                }
            }
        }
    }
    fn timeout(&self, now: f64, message: &str) -> Tick {
        if now >= self.deadline {
            Tick::Stopped(message.into())
        } else {
            observe(true, None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fake(mean: [f64; 3], digits: &str) -> Face {
        Face {
            signature: vec![mean[0]; 300],
            mean,
            bright: 0.5,
            digits: digits.into(),
            back: false,
            green: false,
        }
    }
    #[test]
    fn same_icon_different_quantity_is_not_a_pair() {
        let a = fake([100.0, 140.0, 190.0], "100");
        let mut b = a.clone();
        b.digits = "200".into();
        assert!(!same(&a, &b));
        b.digits = "100".into();
        assert!(same(&a, &b));
        b.signature.fill(140.0);
        assert!(!same(&a, &b));
    }
    #[test]
    fn cooldown_ocr_has_units_and_bounds() {
        assert_eq!(parse_wait("Available after O3h O4m Left"), Some(11040.0));
        assert_eq!(parse_wait("Available after iih53m Left"), Some(42780.0));
        assert_eq!(parse_wait("Available after 02m 30s"), Some(150.0));
        assert_eq!(parse_wait("Available after 03h 99m"), None);
        assert_eq!(parse_wait("Start Memory Match"), None);
    }
    #[test]
    fn board_coordinates_are_measured_and_never_clamped_offscreen() {
        let words = vec![
            Word {
                text: "Memory Match".into(),
                rect: Rect(0.43, 0.19, 0.57, 0.21),
            },
            Word {
                text: "CHANCES".into(),
                rect: Rect(0.3, 0.2955, 0.4, 0.3155),
            },
        ];
        let b = board_layout(&words, (1920, 1080)).unwrap();
        assert_eq!(b.cards.len(), 20);
        assert!((b.cards[0].at.0 - (0.5 - 136.0 / 1920.0)).abs() < 0.001);
        assert!(board_at(Point(0.98, 0.9), 1.0, (1920, 1080)).is_none());
    }
    #[test]
    fn hidden_and_green_pixel_evidence_are_distinct() {
        let mut bytes = vec![30u8; 100 * 100 * 3];
        for y in 40..60 {
            for x in 40..60 {
                let p = (y * 100 + x) * 3;
                bytes[p..p + 3].fill(240);
            }
        }
        let frame = Frame::new(&bytes, 100, 100, 300, 3, false).unwrap();
        let card = Card {
            at: Point(0.5, 0.5),
            image: Rect(0.1, 0.1, 0.9, 0.9),
            text: Rect(0.1, 0.8, 0.9, 0.9),
        };
        let f = face(&frame, &card, &[]).unwrap();
        assert!(f.back && !f.green);
        for p in bytes.chunks_exact_mut(3) {
            p.copy_from_slice(&[20, 200, 20]);
        }
        let frame = Frame::new(&bytes, 100, 100, 300, 3, false).unwrap();
        let f = face(&frame, &card, &[]).unwrap();
        assert!(f.green && !f.back);
    }
}

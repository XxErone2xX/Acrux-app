use super::*;
use inventory::{Inventory, Item, Report};

const KNOWN: &[&str] = &[
    "Mixed Potion",
    "Speed Potion",
    "Lucky Potion",
    "Fortune Spoid I",
    "Fortune Spoid II",
    "Fortune Spoid III",
    "Void Coin",
    "Lucky Penny",
    "Gear A",
    "Gear B",
    "Strange Potion I",
    "Strange Potion II",
    "Oblivion Potion",
    "Heavenly Potion",
    "Potion of Bound",
    "Random Potion Sack",
    "Stella's Candle",
    "Rune of Everything",
    "Rune of Wind",
    "Rune of Frost",
    "Rune of Rainstorm",
    "Rune of Hell",
    "Rune of Galaxy",
    "Rune of Corruption",
    "Rune of Nothing",
    "Merchant Tracker",
];

fn offset(p: Point, dx: f64, dy: f64, k: f64, g: (usize, usize)) -> Option<Point> {
    let q = Point(p.0 + dx * k / g.0 as f64, p.1 + dy * k / g.1 as f64);
    valid_point(q).then_some(q)
}
fn patch_rect(cfg: &mut Config, group: &str, key: &str, r: Rect) {
    cfg.patch(group, key, json!([r.0, r.1, r.2, r.3]));
}
fn rect_offsets(p: Point, a: [f64; 4], k: f64, g: (usize, usize)) -> Option<Rect> {
    let lo = offset(p, a[0], a[1], k, g)?;
    let hi = offset(p, a[2], a[3], k, g)?;
    rect(&json!([lo.0, lo.1, hi.0, hi.1]))
}
fn dialog(words: &[Word]) -> Option<Point> {
    find(words, &["Click to skip", "to skip"], false).map(|w| center(w.rect))
}
fn open_choice(words: &[Word], fallback: Option<Point>) -> Option<Point> {
    if let Some(w) = find(words, &["Open"], true) {
        return Some(center(w.rect));
    }
    let who = find(words, &["Who are you"], false);
    let leave = find(words, &["Leave"], true);
    if let (Some(w), Some(l)) = (who, leave) {
        let w = center(w.rect);
        let l = center(l.rect);
        let p = Point(2.0 * w.0 - l.0, w.1);
        if l.0 > w.0 && valid_point(p) {
            return Some(p);
        }
    }
    if who.is_some() || leave.is_some() {
        fallback
    } else {
        None
    }
}
fn merchant_layout(cfg: &mut Config, words: &[Word], g: (usize, usize)) -> Option<[Point; 5]> {
    let max = center(find(words, &["Set to Max"], true)?.rect);
    let title = words
        .iter()
        .filter(|w| {
            (ocr_norm(&w.text).contains("sshop") || label(&w.text, "Shop", false))
                && center(w.rect).1 < max.1
        })
        .max_by(|a, b| a.rect.1.total_cmp(&b.rect.1));
    let purchase = find(words, &["Purchase"], true);
    let k = if let Some(t) = title {
        (max.1 - center(t.rect).1) * g.1 as f64 / 266.0
    } else if let Some(p) = purchase {
        (center(p.rect).1 - max.1) * g.1 as f64 / 48.0
    } else {
        g.0 as f64 / 1920.0
    };
    if !(0.2..3.0).contains(&k) {
        return None;
    }
    let close = offset(max, 468.0, -267.0, k, g)?;
    let purchase = offset(max, -164.0, 48.0, k, g)?;
    let mut slots = [Point(0.0, 0.0); 5];
    for (i, p) in slots.iter_mut().enumerate() {
        *p = offset(max, -376.0 + 190.0 * i as f64, 106.0, k, g)?;
    }
    let item = rect_offsets(max, [-241.0, -242.0, 469.0, -210.0], k, g)?;
    for (key, p) in [
        ("max_pos", max),
        ("purchase_pos", purchase),
        ("close_pos", close),
        ("first_slot", slots[0]),
        ("second_slot", slots[1]),
    ] {
        patch_point(cfg, "mmerch", key, p);
    }
    patch_rect(cfg, "mmerch", "item_region", item);
    cfg.patch("mmerch", "cal_shop", json!(true));
    Some(slots)
}
fn saved_merchant_layout(cfg: &Config) -> Option<[Point; 5]> {
    for key in ["max_pos", "purchase_pos", "close_pos"] {
        cfg.point("mmerch", key)?;
    }
    cfg.rect("mmerch", "item_region")?;
    let first = cfg.point("mmerch", "first_slot")?;
    let second = cfg.point("mmerch", "second_slot")?;
    let slots = std::array::from_fn(|i| {
        Point(
            first.0 + (second.0 - first.0) * i as f64,
            first.1 + (second.1 - first.1) * i as f64,
        )
    });
    slots.iter().all(|p| valid_point(*p)).then_some(slots)
}
fn match_item<'a>(raw: &str, wanted: &'a [String]) -> Option<(&'a str, f64)> {
    let item = raw
        .lines()
        .next()
        .unwrap_or("")
        .split('|')
        .next()
        .unwrap_or("")
        .trim();
    let best = KNOWN
        .iter()
        .copied()
        .chain(wanted.iter().map(String::as_str))
        .map(|n| (n, similarity(&ocr_norm(item), &ocr_norm(n))))
        .max_by(|a, b| a.1.total_cmp(&b.1))?;
    wanted
        .iter()
        .find(|n| n.as_str() == best.0)
        .map(|n| (n.as_str(), best.1))
}

pub(super) struct Merchant {
    name: String,
    calibrate: bool,
    check_only: bool,
    wanted: Vec<String>,
    inventory: Option<Inventory>,
    phase: u8,
    due: f64,
    deadline: f64,
    slots: Option<[Point; 5]>,
    slot: usize,
    current: String,
    before: String,
    pub detected_arrivals: Vec<String>,
    pub bought: BTreeMap<String, u64>,
    pub report: Report,
}
impl Merchant {
    pub fn new(
        cfg: &Config,
        name: String,
        calibrate: bool,
        check_only: bool,
        now: f64,
    ) -> Result<Self, String> {
        if !["Mari", "Jester", "Rin"].contains(&name.as_str()) {
            return Err("상인 이름은 Mari · Jester · Rin 중 하나여야 합니다.".into());
        }
        let prefix = format!("{name}_");
        let wanted: Vec<_> = cfg.group("mmerch")["buy"]
            .as_object()
            .into_iter()
            .flatten()
            .filter(|(k, v)| k.starts_with(&prefix) && v.as_f64().is_some_and(|n| n > 0.0))
            .map(|(k, _)| k[prefix.len()..].to_string())
            .collect();
        if !calibrate && !check_only && wanted.is_empty() {
            return Err(format!("{name}에게 구매할 아이템을 선택하세요."));
        }
        if check_only && cfg.rect("mmerch", "chat_region").is_none() {
            return Err("상인 채팅 글자 영역을 먼저 지정하세요.".into());
        }
        if !calibrate
            && !check_only
            && !cfg.flag("mmerch", "auto_cal", true)
            && saved_merchant_layout(cfg).is_none()
        {
            return Err("상점 자동 보정이 꺼져 있습니다. 칸·아이템 이름 영역·구매·닫기 위치를 먼저 지정하세요.".into());
        }
        let inventory = if calibrate || check_only {
            None
        } else {
            Some(Inventory::new(
                cfg,
                vec![Item {
                    name: "Merchant Teleporter".into(),
                    amount: Some(1),
                    min_have: 1,
                }],
                true,
                false,
                true,
            )?)
        };
        Ok(Self {
            name,
            calibrate,
            check_only,
            wanted,
            inventory,
            phase: 0,
            due: now,
            deadline: now + 120.0,
            slots: None,
            slot: 0,
            current: String::new(),
            before: String::new(),
            detected_arrivals: Vec::new(),
            bought: BTreeMap::new(),
            report: Report::default(),
        })
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
        if self.check_only {
            let Some(chat) = cfg.rect("mmerch", "chat_region") else {
                return Tick::Stopped("상인 채팅 글자 영역을 먼저 지정하세요.".into());
            };
            if !frame.is_some_and(|f| f.contains(chat)) {
                return observe(true, chat.region(g));
            }
            let raw = read(words, Some(chat));
            let arrivals = arrivals(&raw);
            self.detected_arrivals = arrivals.keys().cloned().collect();
            self.report
                .logs
                .push(format!("상인 채팅 검사: {arrivals:?}"));
            return Tick::Done;
        }
        if self.phase == 0 {
            if let Some(inv) = self.inventory.as_mut() {
                match inv.tick(cfg, now, g, frame, words) {
                    Tick::Done => {
                        self.report = inv.report.clone();
                        if inv
                            .report
                            .used
                            .get("Merchant Teleporter")
                            .copied()
                            .unwrap_or(0)
                            == 0
                        {
                            return Tick::Stopped(
                                "Merchant Teleporter 사용을 확인하지 못했습니다.".into(),
                            );
                        }
                        self.inventory = None;
                        self.phase = 1;
                        self.due = now + cfg.number("mmerch", "teleport_wait", 3.0);
                        return wait(self.due - now);
                    }
                    other => return other,
                }
            }
            self.phase = 1;
        }
        if self.phase == 1 {
            self.phase = 2;
            self.deadline = now + if self.calibrate { 120.0 } else { 15.0 };
            return key(0x45);
        }
        if frame.is_none() {
            return observe(true, None);
        }
        match self.phase {
            2 => {
                if let Some(at) = open_choice(words, cfg.point("mmerch", "open_pos")) {
                    let at = if self.calibrate || cfg.flag("mmerch", "auto_cal", true) {
                        patch_point(cfg, "mmerch", "open_pos", at);
                        at
                    } else {
                        cfg.point("mmerch", "open_pos").unwrap_or(at)
                    };
                    self.phase = 3;
                    self.deadline = now + 8.0;
                    self.due = now + 0.5;
                    return tap(at, 40);
                }
                if let Some(at) = dialog(words).or_else(|| {
                    find(words, &[&self.name], false).and_then(|_| cfg.point("base", "dialog_pos"))
                }) {
                    self.due = now + 0.35;
                    return tap(at, 40);
                }
                if now >= self.deadline {
                    return Tick::Stopped("상인의 실제 Open 선택지를 찾지 못했습니다.".into());
                }
                observe(true, None)
            }
            3 => {
                let slots = if self.calibrate || cfg.flag("mmerch", "auto_cal", true) {
                    merchant_layout(cfg, words, g)
                } else if find(words, &["Set to Max"], true).is_some()
                    && find(words, &["Purchase"], true).is_some()
                {
                    saved_merchant_layout(cfg)
                } else {
                    None
                };
                if let Some(slots) = slots {
                    self.slots = Some(slots);
                    if self.calibrate {
                        self.phase = 9;
                    } else {
                        self.phase = 4;
                    }
                    return self.tick(cfg, now, g, frame, words);
                }
                if now >= self.deadline {
                    return Tick::Stopped("Set to Max와 상점 배치 확인에 실패했습니다.".into());
                }
                observe(true, None)
            }
            4 => {
                if self.slot >= 5 {
                    self.phase = 9;
                    return self.tick(cfg, now, g, frame, words);
                }
                if find(words, &["Set to Max"], true).is_none() {
                    return Tick::Stopped("상점이 사라져 구매하지 않습니다.".into());
                }
                let Some(slots) = self.slots else {
                    return Tick::Stopped("상점 배치가 없습니다.".into());
                };
                self.phase = 5;
                self.deadline = now + 4.0;
                self.due = now + 0.5;
                tap(slots[self.slot], 40)
            }
            5 => {
                let raw = read(words, cfg.rect("mmerch", "item_region"));
                if let Some((name, score)) = match_item(&raw, &self.wanted) {
                    if score >= cfg.number("pop", "match_threshold", 70.0) {
                        self.current = name.into();
                        self.before = read(words, None);
                        self.phase = 6;
                        self.due = now + 0.3;
                        return tap(cfg.point("mmerch", "max_pos").unwrap(), 40);
                    }
                }
                self.report.logs.push(format!(
                    "{} 칸 {} 구매 안 함 · 실제 이름 '{raw}'",
                    self.name,
                    self.slot + 1
                ));
                self.slot += 1;
                self.phase = 4;
                self.tick(cfg, now, g, frame, words)
            }
            6 => {
                // Name must still match immediately before issuing a purchase.
                let raw = read(words, cfg.rect("mmerch", "item_region"));
                if !match_item(&raw, &self.wanted).is_some_and(|(n, s)| {
                    n == self.current && s >= cfg.number("pop", "match_threshold", 70.0)
                }) {
                    return Tick::Stopped("구매 직전 아이템 이름이 달라져 중지했습니다.".into());
                }
                let Some(at) = find(words, &["Purchase"], true).map(|w| center(w.rect)) else {
                    return Tick::Stopped("실제 Purchase 버튼을 확인하지 못했습니다.".into());
                };
                let at = if self.calibrate || cfg.flag("mmerch", "auto_cal", true) {
                    at
                } else {
                    let Some(saved) = cfg.point("mmerch", "purchase_pos") else {
                        return Tick::Stopped("저장된 상점 Purchase 위치가 없습니다.".into());
                    };
                    saved
                };
                self.phase = 7;
                self.deadline = now + 8.0;
                self.due = now + 0.5;
                tap(at, 40)
            }
            7 => {
                let raw = read(words, None);
                let n = ocr_norm(&raw);
                let bought = (n.contains("purchosed")
                    || n.contains("successfullybought")
                    || n.contains("youhovebought")
                    || raw.contains("구매 완료")
                    || raw.contains("구매했습니다")
                    || raw.contains("購入しました")
                    || raw.contains("購入完了"))
                    && raw != self.before;
                let unavailable = label(&raw, "Not enough", false)
                    || n.contains("notenough")
                    || n.contains("soldout")
                    || n.contains("outofstock")
                    || n.contains("lnsufflclent")
                    || raw.contains("품절")
                    || raw.contains("売り切れ");
                if bought || unavailable {
                    if bought {
                        *self.bought.entry(self.current.clone()).or_default() += 1;
                        self.report
                            .logs
                            .push(format!("{} 구매 결과 확인", self.current));
                    } else {
                        self.report.logs.push(format!(
                            "{} 구매 불가 · 품절/재화 부족 화면 확인",
                            self.current
                        ));
                    }
                    self.phase = 8;
                    self.due = now + 0.35;
                    if let Some(at) = dialog(words) {
                        return tap(at, 40);
                    }
                    return self.tick(cfg, now, g, frame, words);
                }
                if let Some(at) = dialog(words) {
                    self.due = now + 0.35;
                    return tap(at, 40);
                }
                if now >= self.deadline {
                    return Tick::Stopped(format!(
                        "{} 구매 결과를 확인하지 못했습니다. 다시 구매하지 않습니다.",
                        self.current
                    ));
                }
                observe(true, None)
            }
            8 => {
                if find(words, &["Set to Max"], true).is_some() {
                    self.slot += 1;
                    self.phase = 4;
                    return self.tick(cfg, now, g, frame, words);
                }
                if now >= self.deadline {
                    return Tick::Stopped("구매 후 상점 복귀를 확인하지 못했습니다.".into());
                }
                if let Some(at) = dialog(words) {
                    self.due = now + 0.35;
                    return tap(at, 40);
                }
                observe(true, None)
            }
            9 => {
                self.phase = 10;
                self.deadline = now + 4.0;
                self.due = now + 0.5;
                tap(cfg.point("mmerch", "close_pos").unwrap(), 40)
            }
            _ => {
                if find(words, &["Set to Max"], true).is_none() {
                    Tick::Done
                } else if now >= self.deadline {
                    Tick::Stopped("상점이 닫히지 않았습니다.".into())
                } else {
                    observe(true, None)
                }
            }
        }
    }
}

pub(super) fn arrivals(raw: &str) -> BTreeMap<String, u64> {
    let n = ocr_norm(raw);
    let mut out = BTreeMap::new();
    for (name, needle) in [
        ("Mari", "morlhosorrlved"),
        ("Jester", "jesterhosorrlved"),
        ("Rin", "rlnhosorrlved"),
    ] {
        let needle = needle.replace(' ', "");
        let count = n.matches(&needle).count() as u64;
        if count > 0 {
            out.insert(name.to_string(), count);
        }
    }
    out
}

const WORDS: &[&str] = &[
    "Potion",
    "Fortune",
    "Haste",
    "Heavenly",
    "Godlike",
    "Godly",
    "Warp",
    "Zeus",
    "Poseidon",
    "Hades",
    "Jewelry",
    "Zombie",
    "Rage",
    "Diver",
    "Bound",
    "Oblivion",
    "Transcendent",
    "Lucky",
    "Speed",
    "Mixed",
    "Super",
    "Tidal",
    "Shifter",
    "Popping",
    "Stella",
    "Candle",
    "Celestial",
    "Exotic",
    "Powered",
    "Quartz",
    "Strange",
    "Random",
    "Sack",
    "Divine",
    "Aurora",
    "Of",
    "The",
    "I",
    "II",
    "III",
    "IV",
    "V",
    "X",
];
pub(super) fn potion_name(name: &str) -> String {
    let mut parts: Vec<String> = name
        .split_whitespace()
        .map(|w| {
            WORDS
                .iter()
                .find(|x| ocr_norm(x) == ocr_norm(w))
                .copied()
                .unwrap_or(w)
                .to_string()
        })
        .collect();
    if let Some(last) = parts.last_mut() {
        if let Some(i) = ["1", "2", "3", "4", "5"].iter().position(|x| last == x) {
            *last = ["I", "II", "III", "IV", "V"][i].into();
        }
    }
    parts.join(" ")
}
pub(super) fn crafted(raw: &str) -> Option<String> {
    for line in raw.lines() {
        let lower = line.to_ascii_lowercase();
        let Some(start) = lower.find("your") else {
            continue;
        };
        let end = lower.find(" was ").or_else(|| lower.find(" wos "));
        if let Some(end) = end {
            if end > start + 4
                && ocr_norm(&line[end..]).contains("outomotlc")
                && ocr_norm(&line[end..]).contains("crofted")
            {
                let name = potion_name(line[start + 4..end].trim_matches([' ', '.', '!']));
                if !name.is_empty() {
                    return Some(name);
                }
            }
        }
    }
    None
}
fn craft_shop(cfg: &mut Config, words: &[Word], g: (usize, usize)) -> bool {
    let Some(title) = find(words, &["Workshop"], false) else {
        return false;
    };
    let p = center(title.rect);
    let k = match (
        find(words, &["Item"], true),
        find(words, &["Lantern"], true),
    ) {
        (Some(i), Some(l)) => (center(l.rect).0 - center(i.rect).0).abs() * g.0 as f64 / 239.0,
        _ => g.1 as f64 / 1080.0,
    };
    if !(0.2..4.0).contains(&k) {
        return false;
    }
    let (Some(search), Some(close), Some(list)) = (
        offset(p, 7.0, 83.0, k, g),
        offset(p, 217.0, -2.0, k, g),
        rect_offsets(p, [-225.0, 110.0, 225.0, 735.0], k, g),
    ) else {
        return false;
    };
    patch_point(cfg, "mcraft", "search_pos", search);
    patch_point(cfg, "mcraft", "shop_close_pos", close);
    patch_rect(cfg, "mcraft", "list_region", list);
    true
}
fn add_layout(cfg: &mut Config, words: &[Word], g: (usize, usize)) -> bool {
    let (Some(title), Some(add)) = (
        find(words, &["Ingredients"], false),
        find(words, &["Everything"], false),
    ) else {
        return false;
    };
    let p = center(title.rect);
    let a = center(add.rect);
    let k = if a.1 > p.1 {
        (a.1 - p.1) * g.1 as f64 / 295.0
    } else {
        g.1 as f64 / 1080.0
    };
    if !(0.2..4.0).contains(&k) {
        return false;
    }
    let (Some(craft), Some(close)) = (offset(a, 240.0, 0.0, k, g), offset(p, 226.0, -2.0, k, g))
    else {
        return false;
    };
    patch_point(cfg, "mcraft", "add_all_pos", a);
    patch_point(cfg, "mcraft", "craft_pos", craft);
    patch_point(cfg, "mcraft", "add_close_pos", close);
    cfg.patch("mcraft", "cal_shop", json!(true));
    true
}
fn pick_card<'a>(words: &'a [Word], name: &str, area: Option<Rect>) -> Option<&'a Word> {
    let mut parts: Vec<_> = name.split_whitespace().collect();
    let roman = parts
        .last()
        .and_then(|s| ["I", "II", "III", "IV", "V"].iter().position(|r| s == r));
    if roman.is_some() {
        parts.pop();
    }
    let base = ocr_norm(&parts.join(" "));
    let mut found: Vec<_> = words
        .iter()
        .filter(|w| {
            area.is_none_or(|a| within(a, center(w.rect))) && {
                let n = ocr_norm(&w.text);
                if roman.is_some() {
                    n.starts_with(&base) && n.len() <= base.len() + 4
                } else {
                    n == base || similarity(&n, &base) >= 90.0
                }
            }
        })
        .collect();
    found.sort_by(|a, b| a.rect.1.total_cmp(&b.rect.1));
    found.get(roman.unwrap_or(0)).copied()
}

pub(super) struct Craft {
    name: String,
    calibrate: bool,
    phase: u8,
    due: f64,
    deadline: f64,
    before: String,
    pub completed: bool,
    pub logs: Vec<String>,
    fallback: bool,
}
impl Craft {
    pub fn new(name: String, calibrate: bool, now: f64) -> Result<Self, String> {
        if !calibrate && (name.trim().is_empty() || name.len() > 256) {
            return Err("제작할 포션 이름이 필요합니다.".into());
        }
        Ok(Self {
            name: potion_name(&name),
            calibrate,
            phase: if calibrate { 1 } else { 0 },
            due: now,
            deadline: now + 120.0,
            before: String::new(),
            completed: false,
            logs: Vec::new(),
            fallback: false,
        })
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
            return Tick::Travel(Destination::Craft);
        }
        if self.phase == 1 {
            self.phase = 2;
            self.deadline = now + if self.calibrate { 120.0 } else { 15.0 };
            self.due = now + cfg.number("mcraft", "f_wait", 2.5);
            return key(0x46);
        }
        if frame.is_none() {
            return observe(true, None);
        }
        match self.phase {
            2 => {
                if craft_shop(cfg, words, g) {
                    self.phase = if self.calibrate { 5 } else { 3 };
                    return self.tick(cfg, now, g, frame, words);
                }
                self.timeout(now, "Stella's Workshop을 확인하지 못했습니다.")
            }
            3 => {
                self.phase = 4;
                self.due = now + 0.3;
                tap(cfg.point("mcraft", "search_pos").unwrap(), 40)
            }
            4 => {
                self.phase = 5;
                self.deadline = now + 5.0;
                self.due = now + 0.5;
                let query = if self.fallback {
                    self.name
                        .split(['a', 'o', 'i', 'l', 'A', 'O', 'I', 'L'])
                        .max_by_key(|s| s.len())
                        .filter(|s| s.len() >= 3)
                        .unwrap_or(&self.name)
                        .to_string()
                } else {
                    self.name.clone()
                };
                text(query)
            }
            5 => {
                let card = if self.calibrate {
                    let area = cfg.rect("mcraft", "list_region");
                    words
                        .iter()
                        .filter(|w| {
                            area.is_some_and(|r| within(r, center(w.rect)))
                                && ocr_norm(&w.text).len() >= 6
                        })
                        .min_by(|a, b| a.rect.1.total_cmp(&b.rect.1))
                } else {
                    pick_card(words, &self.name, cfg.rect("mcraft", "list_region"))
                };
                if let Some(card) = card {
                    self.phase = 6;
                    self.deadline = now + 4.0;
                    self.due = now + 0.5;
                    return tap(center(card.rect), 40);
                }
                if now >= self.deadline && !self.calibrate && !self.fallback {
                    self.fallback = true;
                    self.phase = 3;
                    return self.tick(cfg, now, g, frame, words);
                }
                self.timeout(now, "실제 제작 목록에서 지정한 포션을 찾지 못했습니다.")
            }
            6 => {
                if let Some(button) = find(words, &["pen Recipe"], false) {
                    let p = center(button.rect);
                    patch_point(cfg, "mcraft", "open_recipe_pos", p);
                    self.phase = 7;
                    self.deadline = now + 5.0;
                    self.due = now + 0.4;
                    return tap(p, 40);
                }
                self.timeout(now, "Open Recipe를 확인하지 못했습니다.")
            }
            7 => {
                if add_layout(cfg, words, g) {
                    if self.calibrate {
                        self.phase = 12;
                    } else {
                        self.phase = 8;
                    }
                    return self.tick(cfg, now, g, frame, words);
                }
                self.timeout(now, "Add Ingredients 창을 확인하지 못했습니다.")
            }
            8 => {
                if find(words, &["Ingredients"], false).is_none() {
                    return Tick::Stopped("재료 창이 사라져 재료를 넣지 않습니다.".into());
                }
                let Some(p) = find(words, &["Everything"], false).map(|w| center(w.rect)) else {
                    return Tick::Stopped("Add Everything 버튼이 없습니다.".into());
                };
                self.before = read(words, None);
                self.phase = 9;
                self.deadline = now + 5.0;
                self.due = now + 0.5;
                tap(p, 40)
            }
            9 => {
                let raw = read(words, None);
                if raw == self.before && !has_ingredient_count(words) {
                    return self.timeout(
                        now,
                        "재료 추가 결과를 확인하지 못했습니다. 다시 넣지 않습니다.",
                    );
                }
                if find(words, &["Not enough", "Missing ingredients"], false).is_some() {
                    return Tick::Stopped("제작 재료가 부족합니다.".into());
                }
                let Some(p) = find(words, &["Craft"], true)
                    .map(|w| center(w.rect))
                    .or_else(|| {
                        find(words, &["Ingredients"], false)
                            .and_then(|_| cfg.point("mcraft", "craft_pos"))
                    })
                else {
                    return Tick::Stopped("Craft 버튼이 없습니다.".into());
                };
                self.before = raw;
                self.phase = 10;
                self.deadline = now + 7.0;
                self.due = now + 0.6;
                tap(p, 40)
            }
            10 => {
                let raw = read(words, None);
                let n = ocr_norm(&raw);
                let success = n.contains("successfullycrofted")
                    || n.contains("outocroft")
                    || n.contains("potioncrofted")
                    || raw.contains("제작 완료")
                    || raw.contains("제작했습니다")
                    || raw.contains("作成しました")
                    || raw.contains("作成完了")
                    || (raw != self.before
                        && find(words, &["Everything"], false).is_some()
                        && !has_ingredient_count(words));
                if !success {
                    return self.timeout(
                        now,
                        "제작 결과를 확인하지 못했습니다. Craft를 재전송하지 않습니다.",
                    );
                }
                self.completed = true;
                self.phase = 11;
                self.deadline = now + 5.0;
                self.due = now + 0.5;
                self.before = raw;
                let Some(p) = find(words, &["Everything"], false).map(|w| center(w.rect)) else {
                    return Tick::Stopped("제작 후 재료 재충전 창이 없습니다.".into());
                };
                tap(p, 40)
            }
            11 => {
                if read(words, None) == self.before && !has_ingredient_count(words) {
                    return self
                        .timeout(now, "다음 자동 제작용 재료 재충전을 확인하지 못했습니다.");
                }
                self.logs
                    .push(format!("{} 제작/재료 재충전 결과 확인", self.name));
                self.phase = 12;
                self.tick(cfg, now, g, frame, words)
            }
            12 => {
                self.phase = 13;
                self.due = now + 0.4;
                self.deadline = now + 4.0;
                tap(cfg.point("mcraft", "add_close_pos").unwrap(), 40)
            }
            13 => {
                if find(words, &["Ingredients"], false).is_some() {
                    return self.timeout(now, "재료 창이 닫히지 않았습니다.");
                }
                self.phase = 14;
                self.due = now + 0.4;
                self.deadline = now + 4.0;
                tap(cfg.point("mcraft", "shop_close_pos").unwrap(), 40)
            }
            _ => {
                if find(words, &["Workshop"], false).is_none() {
                    Tick::Done
                } else {
                    self.timeout(now, "제작 창이 닫히지 않았습니다.")
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
fn has_ingredient_count(words: &[Word]) -> bool {
    words.iter().any(|w| {
        w.text.split_whitespace().any(|s| {
            s.split_once('/').is_some_and(|(a, b)| {
                a.parse::<u64>().is_ok_and(|n| n > 0) && b.parse::<u64>().is_ok()
            })
        })
    })
}

pub(super) struct Sale {
    phase: u8,
    due: f64,
    deadline: f64,
    before: String,
    pub sold: u64,
    calibrate: bool,
}
impl Sale {
    pub fn new(calibrate: bool, now: f64) -> Self {
        Self {
            phase: if calibrate { 1 } else { 0 },
            due: now,
            deadline: now + 120.0,
            before: String::new(),
            sold: 0,
            calibrate,
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
            return Tick::Travel(Destination::Sell);
        }
        if self.phase == 1 {
            self.phase = 2;
            self.deadline = now + if self.calibrate { 120.0 } else { 15.0 };
            self.due = now + cfg.number("mfish", "e_wait", 1.0);
            return key(0x45);
        }
        if frame.is_none() {
            return observe(true, None);
        }
        match self.phase {
            2 => {
                if let Some(button) = find(words, &["Sell Fish"], false) {
                    let p = center(button.rect);
                    patch_point(cfg, "mfish", "sell_fish_pos", p);
                    self.phase = 3;
                    self.deadline = now + 7.0;
                    self.due = now + 0.6;
                    return tap(p, 40);
                }
                if let Some(p) = dialog(words) {
                    self.due = now + 0.35;
                    return tap(p, 40);
                }
                self.timeout(now, "Sell Fish 선택지를 찾지 못했습니다.")
            }
            3 => {
                if sale_layout(cfg, words, g) {
                    self.phase = 4;
                    return self.tick(cfg, now, g, frame, words);
                }
                self.timeout(
                    now,
                    "Fishing Shop · Sell All · 실제 목록을 확인하지 못했습니다.",
                )
            }
            4 => {
                if self.sold >= cfg.number("mfish", "sell_max", 100.0).clamp(1.0, 1000.0) as u64 {
                    return Tick::Stopped("설정한 최대 판매 횟수에 도달했습니다.".into());
                }
                if find(words, &["Fishing Shop"], false).is_none() {
                    return Tick::Stopped("판매 상점이 사라졌습니다.".into());
                }
                self.phase = 5;
                self.due = now + 0.5 + cfg.number("mfish", "sell_delay", 0.0);
                tap(cfg.point("mfish", "first_fish_pos").unwrap(), 40)
            }
            5 => {
                let Some(at) = find(words, &["Sell All"], true).map(|w| center(w.rect)) else {
                    return Tick::Stopped("Sell All 버튼을 확인하지 못했습니다.".into());
                };
                self.phase = 6;
                self.deadline = now + 3.0;
                self.due = now + 0.4;
                tap(at, 40)
            }
            6 => {
                if let Some((sell, cancel)) = confirm(words) {
                    patch_point(cfg, "mfish", "confirm_sell_pos", sell);
                    cfg.patch("mfish", "cal_sell", json!(true));
                    self.before = read(words, None);
                    self.phase = if self.calibrate { 9 } else { 7 };
                    self.deadline = now + 5.0;
                    self.due = now + 0.5;
                    return tap(if self.calibrate { cancel } else { sell }, 40);
                }
                if now >= self.deadline {
                    if find(words, &["No fish", "Empty", "Nothing to sell"], false).is_some() {
                        self.phase = 9;
                        return self.tick(cfg, now, g, frame, words);
                    }
                    return Tick::Stopped("판매 확인창 또는 빈 목록을 확인하지 못했습니다. 판매 완료로 처리하지 않습니다.".into());
                }
                observe(true, None)
            }
            7 => {
                if confirm(words).is_none()
                    && find(words, &["Fishing Shop"], false).is_some()
                    && read(words, None) != self.before
                {
                    self.sold += 1;
                    self.phase = 4;
                    self.due = now + 0.3;
                    return wait(0.3);
                }
                self.timeout(
                    now,
                    "판매 확인 입력 뒤 결과를 확인하지 못했습니다. 다시 판매하지 않습니다.",
                )
            }
            9 => {
                if confirm(words).is_some() {
                    return self.timeout(now, "판매 확인창이 닫히지 않았습니다.");
                }
                self.phase = 10;
                self.deadline = now + 4.0;
                self.due = now + 0.5;
                tap(cfg.point("mfish", "shop_close_pos").unwrap(), 40)
            }
            _ => {
                if find(words, &["Fishing Shop"], false).is_none() {
                    Tick::Done
                } else {
                    self.timeout(now, "판매 상점이 닫히지 않았습니다.")
                }
            }
        }
    }
    fn timeout(&self, now: f64, s: &str) -> Tick {
        if now >= self.deadline {
            Tick::Stopped(s.into())
        } else {
            observe(true, None)
        }
    }
}
fn confirm(words: &[Word]) -> Option<(Point, Point)> {
    let c = center(find(words, &["Cancel"], true)?.rect);
    let s = words
        .iter()
        .filter(|w| {
            label(&w.text, "Sell", true)
                && (center(w.rect).1 - c.1).abs() < 0.04
                && center(w.rect).0 < c.0
        })
        .max_by(|a, b| a.rect.0.total_cmp(&b.rect.0))?;
    Some((center(s.rect), c))
}
fn sale_layout(cfg: &mut Config, words: &[Word], g: (usize, usize)) -> bool {
    let (Some(title), Some(sa), Some(buy)) = (
        find(words, &["Fishing Shop"], false),
        find(words, &["Sell All"], true),
        find(words, &["Buy"], true),
    ) else {
        return false;
    };
    let t = center(title.rect);
    let s = center(sa.rect);
    let b = center(buy.rect);
    let Some(tab) = words.iter().find(|w| {
        label(&w.text, "Sell", true)
            && (center(w.rect).1 - b.1).abs() < 0.03
            && center(w.rect).0 > b.0
    }) else {
        return false;
    };
    let tab = center(tab.rect);
    let half = (tab.0 - b.0) * 0.5;
    let left = b.0 - half;
    let right = tab.0 + half;
    let aspect = g.0 as f64 / g.1 as f64;
    let mut names: Vec<_> = words
        .iter()
        .filter(|w| {
            let p = center(w.rect);
            left < p.0
                && p.0 < right
                && b.1 + 0.02 < p.1
                && p.1 < s.1 + 0.05
                && ocr_norm(&w.text).len() >= 3
        })
        .collect();
    names.sort_by(|a, b| a.rect.1.total_cmp(&b.rect.1));
    let Some(first) = names.first() else {
        if find(words, &["No fish", "Empty", "Nothing to sell"], false).is_none() {
            return false;
        }
        let close = Point(right - 0.85 * (title.rect.3 - title.rect.1) / aspect, t.1);
        if !valid_point(close) {
            return false;
        }
        patch_point(cfg, "mfish", "shop_close_pos", close);
        return false;
    };
    let top = center(first.rect).1;
    let height = first.rect.3 - first.rect.1;
    let pitch = names
        .iter()
        .map(|w| center(w.rect).1 - top)
        .filter(|v| *v > height * 1.5)
        .min_by(f64::total_cmp)
        .unwrap_or(height * 3.0);
    let col = names
        .iter()
        .filter(|w| (center(w.rect).1 - top).abs() < height)
        .map(|w| center(w.rect).0)
        .min_by(f64::total_cmp)
        .unwrap();
    let fish = Point(col, top - 0.3 * pitch);
    let close = Point(right - 0.85 * (title.rect.3 - title.rect.1) / aspect, t.1);
    if !valid_point(fish) || !valid_point(close) {
        return false;
    }
    patch_point(cfg, "mfish", "first_fish_pos", fish);
    patch_point(cfg, "mfish", "shop_close_pos", close);
    patch_point(cfg, "mfish", "sell_all_pos", s);
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    fn w(text: &str, x: f64, y: f64) -> Word {
        Word {
            text: text.into(),
            rect: Rect(x - 0.02, y - 0.01, x + 0.02, y + 0.01),
        }
    }
    fn merchant_config(auto_cal: bool) -> Value {
        json!({
            "base": {
                "inventory_pos": [0.1, 0.1], "items_pos": [0.2, 0.1],
                "search_pos": [0.3, 0.1], "item_pos": [0.4, 0.1],
                "amount_pos": [0.5, 0.1], "use_pos": [0.6, 0.1],
                "ocr_region": [0.1, 0.1, 0.9, 0.9]
            },
            "mmerch": {
                "auto_cal": auto_cal, "buy": {"Mari_Lucky Potion": 1},
                "open_pos": [0.14, 0.4], "max_pos": [0.38, 0.58],
                "purchase_pos": [0.35, 0.7], "close_pos": [0.85, 0.2],
                "first_slot": [0.2, 0.8], "second_slot": [0.3, 0.8],
                "item_region": [0.1, 0.1, 0.8, 0.3],
                "chat_region": [0.05, 0.05, 0.7, 0.35]
            }
        })
    }
    fn tapped(result: Tick) -> Point {
        match result {
            Tick::Command(Command::Gesture(Gesture::Tap { at, .. })) => at,
            other => panic!("expected a verified tap, got {other:?}"),
        }
    }
    fn assert_point(actual: Point, expected: Point) {
        assert!((actual.0 - expected.0).abs() < 1e-9);
        assert!((actual.1 - expected.1).abs() < 1e-9);
    }
    #[test]
    fn merchant_manual_layout_preserves_coordinates_and_requires_actual_shop() {
        let mut cfg = Config(merchant_config(false));
        let saved = cfg.group("mmerch").clone();
        let mut merchant = Merchant::new(&cfg, "Mari".into(), false, false, 0.).unwrap();
        let pixels = vec![40; 1920 * 1080 * 3];
        let frame = Frame::new(&pixels, 1920, 1080, 1920 * 3, 3, false).unwrap();
        merchant.phase = 2;
        assert_point(
            tapped(merchant.tick(
                &mut cfg,
                0.,
                (1920, 1080),
                Some(&frame),
                &[w("Open", 0.72, 0.42)],
            )),
            Point(0.14, 0.4),
        );
        assert_eq!(cfg.group("mmerch"), &saved);
        assert!(matches!(
            merchant.tick(&mut cfg, 1., (1920, 1080), None, &[]),
            Tick::Observe { .. }
        ));
        assert!(matches!(
            merchant.tick(
                &mut cfg,
                1.,
                (1920, 1080),
                Some(&frame),
                &[w("Set to Max", 0.55, 0.55)]
            ),
            Tick::Observe { .. }
        ));
        let shop = [
            w("Mari's Shop", 0.55, 0.3),
            w("Set to Max", 0.55, 0.55),
            w("Purchase", 0.48, 0.6),
        ];
        assert_point(
            tapped(merchant.tick(&mut cfg, 1., (1920, 1080), Some(&frame), &shop)),
            Point(0.2, 0.8),
        );
        assert_eq!(cfg.group("mmerch"), &saved);
        merchant.phase = 6;
        merchant.current = "Lucky Potion".into();
        merchant.due = 0.;
        assert_point(
            tapped(merchant.tick(
                &mut cfg,
                2.,
                (1920, 1080),
                Some(&frame),
                &[w("Lucky Potion", 0.4, 0.18), w("Purchase", 0.48, 0.6)],
            )),
            Point(0.35, 0.7),
        );
        assert_eq!(cfg.group("mmerch"), &saved);
        assert!(
            merchant.bought.is_empty(),
            "a purchase click is not a confirmed purchase"
        );
        merchant.phase = 9;
        merchant.due = 0.;
        assert_point(
            tapped(merchant.tick(&mut cfg, 3., (1920, 1080), Some(&frame), &shop)),
            Point(0.85, 0.2),
        );
        assert_eq!(cfg.group("mmerch"), &saved);
        let mut without_open = cfg.clone();
        without_open.patch("mmerch", "open_pos", Value::Null);
        let mut observed_open =
            Merchant::new(&without_open, "Mari".into(), false, false, 0.).unwrap();
        observed_open.phase = 2;
        assert_point(
            tapped(observed_open.tick(
                &mut without_open,
                0.,
                (1920, 1080),
                Some(&frame),
                &[w("Open", 0.72, 0.42)],
            )),
            Point(0.72, 0.42),
        );
        assert!(
            without_open.group("mmerch")["open_pos"].is_null(),
            "an observed Open remains transient when auto calibration is disabled"
        );
        cfg.patch("mmerch", "purchase_pos", Value::Null);
        assert!(
            Merchant::new(&cfg, "Mari".into(), false, false, 0.).is_err(),
            "reject incomplete saved coordinates before consuming a teleporter"
        );
    }
    #[test]
    fn merchant_auto_and_explicit_calibration_use_observed_positions() {
        let pixels = vec![40; 1920 * 1080 * 3];
        let frame = Frame::new(&pixels, 1920, 1080, 1920 * 3, 3, false).unwrap();
        let shop = [
            w("Mari's Shop", 0.55, 0.3),
            w("Set to Max", 0.55, 0.55),
            w("Purchase", 0.48, 0.6),
        ];
        for (auto, explicit) in [(true, false), (false, true)] {
            let mut cfg = Config(if explicit {
                json!({"mmerch":{"auto_cal":false}})
            } else {
                merchant_config(auto)
            });
            let mut merchant = Merchant::new(&cfg, "Mari".into(), explicit, false, 0.).unwrap();
            merchant.phase = 2;
            assert_point(
                tapped(merchant.tick(
                    &mut cfg,
                    0.,
                    (1920, 1080),
                    Some(&frame),
                    &[w("Open", 0.72, 0.42)],
                )),
                Point(0.72, 0.42),
            );
            assert_point(cfg.point("mmerch", "open_pos").unwrap(), Point(0.72, 0.42));
            let at = tapped(merchant.tick(&mut cfg, 1., (1920, 1080), Some(&frame), &shop));
            assert_point(cfg.point("mmerch", "max_pos").unwrap(), Point(0.55, 0.55));
            assert!(cfg.flag("mmerch", "cal_shop", false));
            assert_eq!(cfg.flag("mmerch", "auto_cal", true), auto);
            assert_point(
                at,
                cfg.point("mmerch", if explicit { "close_pos" } else { "first_slot" })
                    .unwrap(),
            );
            assert!(merchant.bought.is_empty());
            if explicit {
                assert_eq!(
                    merchant.phase, 10,
                    "explicit calibration closes without buying"
                );
            }
        }
    }
    #[test]
    fn manual_merchant_chat_check_queues_selected_arrivals_once_without_auto_monitor() {
        let mut config = merchant_config(false);
        config["mmerch"]["buy"] = json!({"Mari_Void Coin":0,"Jester_Gear A":1,"Rin_Gear A":1});
        let mut runner = Runner::new(config.clone(), 0.).unwrap();
        assert!(!runner.enabled("mmerch"));
        runner
            .queue(Job::Merchant {
                name: "Mari".into(),
                calibrate: false,
                check_only: true,
            })
            .unwrap();
        assert!(matches!(
            runner.tick(0., (1920, 1080), None, &[]),
            Tick::Observe {
                ocr: true,
                region: Some(_)
            }
        ));
        let words = [
            w("Mari has arrived", 0.3, 0.12),
            w("Jester has arrived", 0.3, 0.2),
            w("Jester has arrived", 0.3, 0.24),
            w("Rin has arrived", 0.8, 0.7),
        ];
        let pixels = vec![40; 1920 * 1080 * 3];
        let frame = Frame::new(&pixels, 1920, 1080, 1920 * 3, 3, false).unwrap();
        // OCR text alone cannot replace pixels covering the saved chat area.
        let crop = Region {
            x: 1500,
            y: 700,
            width: 100,
            height: 100,
        };
        let cropped = Frame::region(&pixels, 1920, 1080, 300, 3, false, crop).unwrap();
        assert!(matches!(
            runner.tick(0.1, (1920, 1080), Some(&cropped), &words),
            Tick::Observe {
                region: Some(_),
                ..
            }
        ));
        assert!(runner.queue.is_empty());
        assert!(matches!(
            runner.tick(0.2, (1920, 1080), Some(&frame), &words),
            Tick::Done
        ));
        assert_eq!(runner.status.completed, 1);
        assert_eq!(runner.queue.len(), 1);
        assert!(
            matches!(&runner.queue[0].job,Job::Merchant { name, calibrate:false,check_only:false } if name=="Jester")
        );
        assert_eq!(runner.status.queued, 1);
        assert!(runner.status.running);
        assert!(matches!(
            runner.tick(0.3, (1920, 1080), Some(&frame), &[]),
            Tick::Command(Command::Gesture(Gesture::Tap { .. }))
        ));
        assert!(
            matches!(&runner.active.as_ref().unwrap().job,Job::Merchant { name,check_only:false,.. } if name=="Jester")
        );
        assert_eq!(
            runner.status.commands, 1,
            "the follow-up starts the real inventory/teleporter executor"
        );
        config["mmerch"]["chat_region"] = Value::Null;
        let mut no_chat = Runner::new(config, 0.).unwrap();
        assert!(no_chat
            .queue(Job::Merchant {
                name: "Mari".into(),
                calibrate: false,
                check_only: true
            })
            .is_err());
    }
    #[test]
    fn cancelled_merchant_chat_check_does_not_start_a_purchase() {
        let mut runner = Runner::new(merchant_config(false), 0.).unwrap();
        runner
            .queue(Job::Merchant {
                name: "Mari".into(),
                calibrate: false,
                check_only: true,
            })
            .unwrap();
        runner.tick(0., (1920, 1080), None, &[]);
        if let Task::Merchant(task) = &mut runner.active.as_mut().unwrap().task {
            task.detected_arrivals = vec!["Mari".into()];
        } else {
            panic!("merchant chat check must be active");
        }
        runner.finish_active(false, Some("cancelled".into()));
        assert!(runner.queue.is_empty());
        assert_eq!(runner.status.commands, 0);
        assert_eq!(runner.status.failed, 1);
    }
    #[test]
    fn merchant_name_competition_does_not_buy_wrong_gear_or_potion() {
        let desired = vec!["Gear A".into(), "Lucky Potion".into()];
        assert!(match_item("Gear B | Common", &desired).is_none());
        assert!(match_item("Lucky Penny | Rare", &desired).is_none());
        assert_eq!(match_item("Gear A | Common", &desired).unwrap().0, "Gear A");
        assert_eq!(
            arrivals("Mari has arrived\nJester has arrived\nMari has arrived").get("Mari"),
            Some(&2)
        );
    }
    #[test]
    fn recipe_roman_tier_and_actual_buttons_are_required() {
        let words = vec![
            w("Fortune Potion l", 0.5, 0.3),
            w("Fortune Potion ll", 0.5, 0.4),
            w("Fortune Potion lll", 0.5, 0.5),
        ];
        assert_eq!(
            center(pick_card(&words, "Fortune Potion II", None).unwrap().rect).1,
            0.4
        );
        assert_eq!(potion_name("fortune potion 2"), "Fortune Potion II");
        assert_eq!(
            crafted("Your Heovenly Potion II was automatically crafted!"),
            Some("Heavenly Potion II".into())
        );
        assert!(confirm(&[w("Sell All", 0.4, 0.6), w("Cancel", 0.6, 0.6)]).is_none());
        assert!(confirm(&[w("Sell", 0.4, 0.6), w("Cancel", 0.6, 0.6)]).is_some());
    }
}

//! The main PC inventory sequence shared by popping, item timers and merchant teleporters.
//! Input is acknowledged by the desktop worker before the following delay starts.
use crate::{
    macro_step::Step,
    navigation::Motion,
    ocr::Word,
    vision::{Frame, Point, Rect},
};
use regex::Regex;
use serde_json::{json, Value};
use std::{collections::HashMap, sync::OnceLock};
use unicode_normalization::UnicodeNormalization;

pub(crate) const POP_BIOMES: &[&str] = &["CYBERSPACE", "GLITCHED", "DREAMSPACE"];
pub(crate) const BLOOD_BIOMES: &[&str] = &["PUMPKINMOON", "GRAVEYARD"];
pub(crate) const TIMED_ITEMS: &[(&str, &str, f64)] = &[
    ("strange", "Strange Controller", 10.5),
    ("randomizer", "Biome Randomizer", 18.0),
];

#[derive(Clone, Debug)]
pub(crate) struct UseRecord {
    pub key: String,
    pub name: String,
    pub amount: u64,
    pub at: f64,
}

struct Item {
    key: String,
    name: String,
    amount: Option<u64>, // None is the main template's ALL.
    min_have: u64,
}

#[derive(Clone, Copy)]
struct Positions {
    inventory: Point,
    items: Point,
    search: Point,
    item: Point,
    amount: Point,
    use_button: Point,
    ocr: Rect,
}

impl Positions {
    fn read(config: &Value) -> Result<Self, String> {
        let base = &config["base"];
        let p = |key, label| {
            read_point(&base[key]).ok_or_else(|| format!("공통 위치 지정 필요: {label}"))
        };
        Ok(Self {
            inventory: p("inventory_pos", "Inventory 버튼")?,
            items: p("items_pos", "Items 버튼")?,
            search: p("search_pos", "Search 버튼")?,
            item: p("item_pos", "아이템 칸")?,
            amount: p("amount_pos", "수량 입력칸")?,
            use_button: p("use_pos", "Use 버튼")?,
            ocr: read_rect(&base["ocr_region"])
                .ok_or_else(|| "공통 위치 지정 필요: OCR 영역".to_string())?,
        })
    }
}

struct Delays {
    inventory: f64,
    items: f64,
    search: f64,
    typing: f64,
    item_click: f64,
    dbl_gap: f64,
    after_dbl: f64,
    after_enter: f64,
}

impl Delays {
    fn read(config: &Value) -> Self {
        let d = &config["pop"]["delays"];
        let get = |key, fallback| finite_number(&d[key]).unwrap_or(fallback).max(0.0);
        Self {
            inventory: get("inventory", 0.5),
            items: get("items", 0.5),
            search: get("search", 0.5),
            typing: get("typing", 0.5),
            item_click: get("item_click", 0.35),
            dbl_gap: get("dbl_gap", 0.1),
            after_dbl: get("after_dbl", 0.5),
            after_enter: get("after_enter", 0.35),
        }
    }
}

#[derive(Clone, Copy)]
enum Phase {
    OpenInventory,
    OpenItems,
    Search,
    TypeSearch,
    RequestOcr,
    ReadOcr,
    SelectItem,
    AmountFirst,
    AmountSecond,
    TypeAmount,
    Use,
    AfterItem,
    Close,
    Done,
    Failed,
}

struct Pending {
    next: Phase,
    delay: f64,
    record: Option<UseRecord>,
}

pub(crate) struct ItemUse {
    feature: String,
    feature_config: Value,
    pos: Positions,
    delays: Delays,
    threshold: f64,
    items: Vec<Item>,
    one: bool,
    dry: bool,
    close_inventory: bool,
    phase: Phase,
    pending: Option<Pending>,
    ready_at: f64,
    now: f64,
    index: usize,
    attempt: u8,
    amount: u64,
    completed: Vec<UseRecord>,
    retry_at: Option<f64>,
    message: String,
    error: Option<String>,
    logs: Vec<String>,
}

impl ItemUse {
    /// A finite main-app batch. The coordinator supplies due timer keys and owns
    /// biome deduplication, screen priority, fishing yield and historical totals.
    pub(crate) fn new(
        config: &Value,
        feature: &str,
        biome: Option<&str>,
        keys: &[String],
        test: bool,
        now: f64,
    ) -> Result<Self, String> {
        let cfg = &config[feature];
        let (items, one, dry) = match feature {
            "mitem" => {
                let items: Vec<_> = keys
                    .iter()
                    .map(|key| {
                        let (_, name, _) = TIMED_ITEMS
                            .iter()
                            .find(|(k, _, _)| k == key)
                            .ok_or_else(|| "사용할 아이템을 확인하세요.".to_string())?;
                        Ok(Item {
                            key: key.clone(),
                            name: (*name).to_string(),
                            amount: Some(1),
                            min_have: 1,
                        })
                    })
                    .collect::<Result<_, String>>()?;
                (items, false, false)
            }
            "mpop" => {
                let cur = biome
                    .filter(|b| POP_BIOMES.contains(b))
                    .ok_or_else(|| "팝핑할 바이옴을 확인하세요.".to_string())?;
                let tpl = &cfg["templates"][cur];
                let items = tpl["items"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|it| {
                        let name = it["name"].as_str()?.trim();
                        (!name.is_empty()).then(|| Item {
                            key: name.to_string(),
                            name: name.to_string(),
                            amount: item_amount(&it["amount"]),
                            min_have: positive_integer(&it["min_have"]).unwrap_or(1),
                        })
                    })
                    .collect();
                (items, tpl["mode"].as_str() == Some("one"), false)
            }
            "mblood" => {
                if !test && !biome.is_some_and(|b| BLOOD_BIOMES.contains(&b)) {
                    return Err("블러드 레인을 만들 바이옴을 확인하세요.".to_string());
                }
                (
                    vec![Item {
                        key: "blood".to_string(),
                        name: "Cursed Rune Fragment".to_string(),
                        amount: Some(1),
                        min_have: 2,
                    }],
                    false,
                    test,
                )
            }
            _ => return Err("지원하지 않는 아이템 기능입니다.".to_string()),
        };
        if items.is_empty() {
            return Err(if feature == "mpop" {
                "템플릿에 사용할 포션이 없습니다."
            } else {
                "사용할 아이템이 꺼져 있습니다."
            }
            .to_string());
        }
        let delay = if feature != "mitem" && !test {
            finite_number(&cfg["start_delay"])
                .unwrap_or(1.0)
                .clamp(0.0, 60.0)
        } else {
            0.0
        };
        Self::batch(
            config,
            feature,
            items,
            one,
            dry,
            cfg["close_inventory"].as_bool().unwrap_or(true),
            now,
            delay,
        )
    }

    /// Merchant Teleporter uses the same search, name/count checks and Use flow.
    pub(crate) fn single(
        config: &Value,
        name: &str,
        amount: u32,
        min_have: u32,
        close_inventory: bool,
        now: f64,
    ) -> Result<Self, String> {
        if name.trim().is_empty() {
            return Err("사용할 아이템 이름이 없습니다.".to_string());
        }
        Self::batch(
            config,
            "single",
            vec![Item {
                key: name.trim().to_string(),
                name: name.trim().to_string(),
                amount: Some(u64::from(amount.max(1))),
                min_have: u64::from(min_have.max(1)),
            }],
            false,
            false,
            close_inventory,
            now,
            0.0,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn batch(
        config: &Value,
        feature: &str,
        items: Vec<Item>,
        one: bool,
        dry: bool,
        close_inventory: bool,
        now: f64,
        start_delay: f64,
    ) -> Result<Self, String> {
        if !now.is_finite() {
            return Err("아이템 실행 시간을 확인하세요.".to_string());
        }
        Ok(Self {
            feature: feature.to_string(),
            feature_config: config[feature].clone(),
            pos: Positions::read(config)?,
            delays: Delays::read(config),
            threshold: finite_number(&config["pop"]["match_threshold"])
                .unwrap_or(70.0)
                .clamp(1.0, 100.0),
            items,
            one,
            dry,
            close_inventory,
            phase: Phase::OpenInventory,
            pending: None,
            ready_at: now + start_delay,
            now,
            index: 0,
            attempt: 0,
            amount: 0,
            completed: Vec::new(),
            retry_at: None,
            message: if start_delay > 0.0 {
                format!("시작 전 대기 {start_delay}초")
            } else {
                "Inventory 열기".to_string()
            },
            error: None,
            logs: Vec::new(),
        })
    }

    pub(crate) fn tick(&mut self, now: f64, frame: Option<&Frame<'_>>, words: &[Word]) -> Step {
        if !now.is_finite() {
            self.fail("아이템 실행 시간을 확인하세요.");
        } else {
            self.now = self.now.max(now);
        }
        if let Some(error) = &self.error {
            return Step::Failed(error.clone());
        }
        // No repeated click or fabricated completion while native input is running.
        if self.pending.is_some() {
            return Step::Wait(0.01);
        }
        if self.now < self.ready_at {
            return Step::Wait(self.ready_at - self.now);
        }
        loop {
            match self.phase {
                Phase::OpenInventory => {
                    self.message = "Inventory 열기".to_string();
                    return self.click(self.pos.inventory, Phase::OpenItems, self.delays.inventory);
                }
                Phase::OpenItems => {
                    return self.click(self.pos.items, Phase::Search, self.delays.items);
                }
                Phase::Search => {
                    if self.index >= self.items.len() {
                        if self.feature == "mitem" && self.completed.is_empty() {
                            self.retry_at = Some(self.now + 60.0);
                            self.logs
                                .push("쓴 아이템 없음 · 1분 뒤 다시 시도".to_string());
                        }
                        self.phase = if self.close_inventory {
                            Phase::Close
                        } else {
                            Phase::Done
                        };
                        if self.close_inventory && self.feature != "mitem" {
                            self.ready_at = self.now + self.delays.after_enter;
                            if self.ready_at > self.now {
                                return Step::Wait(self.delays.after_enter);
                            }
                        }
                        continue;
                    }
                    self.attempt += 1;
                    self.message = format!(
                        "{} 검색{}",
                        self.items[self.index].name,
                        if self.attempt == 2 {
                            " (재시도)"
                        } else {
                            ""
                        }
                    );
                    return self.click(self.pos.search, Phase::TypeSearch, self.delays.search);
                }
                Phase::TypeSearch => {
                    return self.text(
                        self.items[self.index].name.clone(),
                        Phase::RequestOcr,
                        self.delays.typing,
                    );
                }
                Phase::RequestOcr => {
                    // OCR passed with earlier UI actions belongs to an older screen.
                    self.phase = Phase::ReadOcr;
                    return self.observe();
                }
                Phase::ReadOcr => {
                    if !frame.is_some_and(|f| f.width > 0 && f.height > 0) {
                        return self.observe();
                    }
                    let raw = words
                        .iter()
                        .filter(|word| inside(self.pos.ocr, word.rect))
                        .map(|word| word.text.as_str())
                        .collect::<Vec<_>>()
                        .join(" ");
                    let (name, count) = parse_ocr(&raw);
                    let item = &self.items[self.index];
                    let score = similarity(&name, &item.name);
                    self.logs.push(format!(
                        "OCR: '{}' · {} 일치율 {score}%",
                        raw.chars().take(40).collect::<String>(),
                        item.name
                    ));
                    if score < self.threshold {
                        if self.attempt < 2 {
                            self.phase = Phase::Search;
                        } else {
                            self.logs.push(format!(
                                "{} — 일치율 {}% 미만, 스킵",
                                item.name, self.threshold
                            ));
                            self.phase = Phase::AfterItem;
                        }
                        continue;
                    }
                    let count = count.or_else(|| (!contains_digit(&raw)).then_some(1));
                    let Some(count) = count else {
                        self.logs
                            .push(format!("{} — 개수를 읽지 못해 스킵", item.name));
                        self.phase = Phase::AfterItem;
                        continue;
                    };
                    if count < item.min_have {
                        self.logs.push(format!(
                            "{} — 보유 {count}개 (필요 {}개), 스킵",
                            item.name, item.min_have
                        ));
                        self.phase = Phase::AfterItem;
                        continue;
                    }
                    self.amount = item.amount.map_or(count, |amount| amount.min(count));
                    if self.dry {
                        self.logs.push(format!(
                            "{} 보유 {count}개 — 테스트라 사용 안 함 (실제로는 {}개 사용)",
                            item.name, self.amount
                        ));
                        self.phase = Phase::AfterItem;
                    } else {
                        self.message = format!("{} {}개 사용", item.name, self.amount);
                        self.phase = Phase::SelectItem;
                    }
                }
                Phase::SelectItem => {
                    return self.click(self.pos.item, Phase::AmountFirst, self.delays.item_click);
                }
                Phase::AmountFirst => {
                    return self.click(self.pos.amount, Phase::AmountSecond, self.delays.dbl_gap);
                }
                Phase::AmountSecond => {
                    return self.click(self.pos.amount, Phase::TypeAmount, self.delays.after_dbl);
                }
                Phase::TypeAmount => {
                    return self.text(self.amount.to_string(), Phase::Use, self.delays.after_enter);
                }
                Phase::Use => {
                    let item = &self.items[self.index];
                    self.pending = Some(Pending {
                        next: Phase::AfterItem,
                        delay: 0.0,
                        record: Some(UseRecord {
                            key: item.key.clone(),
                            name: item.name.clone(),
                            amount: self.amount,
                            at: self.now,
                        }),
                    });
                    return Step::Input(Motion::Tap(self.pos.use_button, 40));
                }
                Phase::AfterItem => {
                    self.index += 1;
                    self.attempt = 0;
                    self.phase = Phase::Search;
                    if self.feature == "mitem" {
                        self.ready_at = self.now + self.delays.after_enter;
                        if self.ready_at > self.now {
                            return Step::Wait(self.delays.after_enter);
                        }
                    }
                }
                Phase::Close => {
                    self.message = "Inventory 닫기".to_string();
                    return self.click(self.pos.inventory, Phase::Done, 0.0);
                }
                Phase::Done => {
                    self.message = "대기".to_string();
                    return Step::Done;
                }
                Phase::Failed => {
                    return Step::Failed(self.error.clone().unwrap_or_default());
                }
            }
        }
    }

    /// Call only after the native action has actually completed. OCR responses are
    /// supplied to tick following Observe and do not use this input acknowledgement.
    pub(crate) fn ack(&mut self, success: bool, now: f64) {
        let Some(pending) = self.pending.take() else {
            return;
        };
        if !success || !now.is_finite() {
            self.fail("아이템 입력을 완료하지 못했습니다.");
            return;
        }
        self.now = self.now.max(now);
        if let Some(mut record) = pending.record {
            record.at = self.now;
            self.logs
                .push(format!("{} {}개 사용", record.name, record.amount));
            self.completed.push(record);
            if self.one {
                self.index = self.items.len();
            }
        }
        self.phase = pending.next;
        self.ready_at = self.now + pending.delay;
    }

    fn observe(&self) -> Step {
        Step::Observe {
            region: Some(self.pos.ocr),
            ocr: true,
        }
    }

    fn click(&mut self, point: Point, next: Phase, delay: f64) -> Step {
        self.pending = Some(Pending {
            next,
            delay,
            record: None,
        });
        Step::Input(Motion::Tap(point, 40))
    }

    fn text(&mut self, value: String, next: Phase, delay: f64) -> Step {
        self.pending = Some(Pending {
            next,
            delay,
            record: None,
        });
        Step::Text {
            value,
            submit: true,
        }
    }

    fn fail(&mut self, message: &str) {
        self.phase = Phase::Failed;
        self.pending = None;
        self.message = message.to_string();
        self.error = Some(message.to_string());
        if self.feature == "mitem" {
            self.retry_at = Some(self.now + 60.0);
        }
    }

    pub(crate) fn used(&self) -> u32 {
        self.completed.len().min(u32::MAX as usize) as u32
    }

    pub(crate) fn completed(&self) -> &[UseRecord] {
        &self.completed
    }

    pub(crate) fn retry_at(&self) -> Option<f64> {
        self.retry_at
    }

    pub(crate) fn take_logs(&mut self) -> Vec<String> {
        std::mem::take(&mut self.logs)
    }

    pub(crate) fn status(&self) -> Value {
        let mut status = json!({
            "running": !matches!(self.phase, Phase::Done | Phase::Failed),
            "msg": self.message,
        });
        if self.feature == "mblood" {
            status["used"] = json!(self.used());
        } else if self.feature == "mitem" {
            let mut items = serde_json::Map::new();
            for &(key, _, default_min) in TIMED_ITEMS {
                let minutes = finite_number(&self.feature_config[format!("{key}_min")])
                    .unwrap_or(default_min)
                    .clamp(1.0, 240.0);
                let last = self.completed.iter().rev().find(|r| r.key == key);
                let next = last.map_or(self.now, |r| r.at + minutes * 60.0);
                items.insert(
                    key.to_string(),
                    json!({
                        "on": self.feature_config[key].as_bool().unwrap_or(true),
                        "used": self.completed.iter().filter(|r| r.key == key).count(),
                        "left": (next.max(self.retry_at.unwrap_or(self.now)) - self.now).max(0.0),
                    }),
                );
            }
            status["items"] = Value::Object(items);
        }
        status
    }
}

fn finite_number(value: &Value) -> Option<f64> {
    value
        .as_f64()
        .or_else(|| value.as_str()?.parse().ok())
        .filter(|n| n.is_finite())
}

fn positive_integer(value: &Value) -> Option<u64> {
    // Python int accepts numeric floats, but not a decimal string such as "2.5".
    if let Some(text) = value.as_str() {
        return text
            .trim()
            .parse::<i128>()
            .ok()
            .map(|n| n.clamp(1, u64::MAX as i128) as u64);
    }
    value
        .as_f64()
        .filter(|n| n.is_finite())
        .map(|n| (n.max(1.0).trunc() as u64).max(1))
}

fn item_amount(value: &Value) -> Option<u64> {
    // The main config normalizer treats absent/invalid amounts as ALL.
    if value
        .as_str()
        .is_some_and(|s| s.eq_ignore_ascii_case("ALL"))
    {
        None
    } else {
        positive_integer(value)
    }
}

fn read_point(value: &Value) -> Option<Point> {
    let a = value.as_array()?;
    if a.len() != 2 {
        return None;
    }
    let x = finite_number(&a[0])?;
    let y = finite_number(&a[1])?;
    ((0.0..=1.0).contains(&x) && (0.0..=1.0).contains(&y)).then_some(Point(x, y))
}

fn read_rect(value: &Value) -> Option<Rect> {
    let a = value.as_array()?;
    if a.len() != 4 {
        return None;
    }
    let r = Rect(
        finite_number(&a[0])?,
        finite_number(&a[1])?,
        finite_number(&a[2])?,
        finite_number(&a[3])?,
    );
    (r.0 >= 0.0 && r.1 >= 0.0 && r.2 <= 1.0 && r.3 <= 1.0 && r.0 < r.2 && r.1 < r.3).then_some(r)
}

fn inside(region: Rect, word: Rect) -> bool {
    let [x0, y0, x1, y1] = [word.0, word.1, word.2, word.3];
    if ![x0, y0, x1, y1].iter().all(|n| n.is_finite()) || x0 >= x1 || y0 >= y1 {
        return false;
    }
    let (x, y) = ((x0 + x1) * 0.5, (y0 + y1) * 0.5);
    x >= region.0 && x <= region.2 && y >= region.1 && y <= region.3
}

fn zero_width(c: char) -> bool {
    matches!(
        c,
        '\u{200b}'..='\u{200f}' | '\u{2060}' | '\u{feff}' | '\u{ad}'
    )
}

fn normalized(s: &str) -> String {
    s.chars()
        .filter(|c| !zero_width(*c))
        .collect::<String>()
        .nfkc()
        .flat_map(char::to_lowercase)
        .collect()
}

fn norm_name(s: &str) -> String {
    normalized(s)
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect()
}

fn clean_name(name: &str) -> String {
    static JUNK: OnceLock<Regex> = OnceLock::new();
    let junk =
        JUNK.get_or_init(|| Regex::new(r"^[0-9SsOoIl|]*[x×X][0-9SsOoIl|,.]*$|^[0-9,.]+$").unwrap());
    name.split_whitespace()
        .filter(|token| {
            !junk.is_match(token)
                || (!token.chars().any(|c| c.is_ascii_digit())
                    && token.chars().any(|c| c != ',' && c != '.'))
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn parse_ocr(text: &str) -> (String, Option<u64>) {
    static COUNT: OnceLock<Regex> = OnceLock::new();
    static LOOSE: OnceLock<Regex> = OnceLock::new();
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let count = COUNT.get_or_init(|| Regex::new(r"[x×X]\s*([0-9][0-9,.]*)\s*$").unwrap());
    let loose = LOOSE.get_or_init(|| Regex::new(r"\s[x×X]\s*([0-9lIiO|o,.]+)\s*$").unwrap());
    let found = count
        .captures(&text)
        .map(|m| (m, false))
        .or_else(|| loose.captures(&text).map(|m| (m, true)));
    if let Some((m, fix)) = found {
        let digits: String = m[1]
            .chars()
            .map(|c| match (fix, c) {
                (true, 'l' | 'I' | 'i' | '|') => '1',
                (true, 'O' | 'o') => '0',
                _ => c,
            })
            .filter(char::is_ascii_digit)
            .collect();
        return (
            clean_name(&text[..m.get(0).unwrap().start()]),
            digits.parse().ok(),
        );
    }
    (clean_name(&text), None)
}

fn contains_digit(text: &str) -> bool {
    static DIGIT: OnceLock<Regex> = OnceLock::new();
    DIGIT
        .get_or_init(|| Regex::new(r"\d").unwrap())
        .is_match(text)
}

fn name_words(text: &str) -> Vec<String> {
    normalized(text)
        .split(|c: char| {
            !(c.is_ascii_lowercase() || c.is_ascii_digit() || ('가'..='힣').contains(&c))
        })
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect()
}

fn similarity(ocr_name: &str, target: &str) -> f64 {
    let (a, b) = (norm_name(ocr_name), norm_name(target));
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    if a == b {
        return 100.0;
    }
    let (ow, tw) = (name_words(ocr_name), name_words(target));
    if !tw.is_empty() && ow.len() > tw.len() && ow.windows(tw.len()).any(|w| w == tw) {
        return 0.0;
    }
    (sequence_ratio(&a, &b) * 1000.0).round() / 10.0
}

/// difflib.SequenceMatcher's matching-block ratio, including its popular-letter
/// anchor rule for long OCR strings. Extra item-name words are rejected above.
fn sequence_ratio(a: &str, b: &str) -> f64 {
    let a: Vec<_> = a.chars().collect();
    let b: Vec<_> = b.chars().collect();
    let mut positions: HashMap<char, Vec<usize>> = HashMap::new();
    for (j, &c) in b.iter().enumerate() {
        positions.entry(c).or_default().push(j);
    }
    if b.len() >= 200 {
        positions.retain(|_, p| p.len() <= b.len() / 100 + 1);
    }
    let mut pending = vec![(0, a.len(), 0, b.len())];
    let mut matches = 0;
    while let Some((alo, ahi, blo, bhi)) = pending.pop() {
        let (mut ai, mut bj, mut size) = (alo, blo, 0);
        let mut lengths: HashMap<usize, usize> = HashMap::new();
        for (i, c) in a.iter().enumerate().take(ahi).skip(alo) {
            let mut next = HashMap::new();
            if let Some(js) = positions.get(c) {
                for &j in js.iter().filter(|&&j| j >= blo && j < bhi) {
                    let k = j
                        .checked_sub(1)
                        .and_then(|p| lengths.get(&p))
                        .copied()
                        .unwrap_or(0)
                        + 1;
                    next.insert(j, k);
                    if k > size {
                        (ai, bj, size) = (i + 1 - k, j + 1 - k, k);
                    }
                }
            }
            lengths = next;
        }
        while ai > alo && bj > blo && a[ai - 1] == b[bj - 1] {
            ai -= 1;
            bj -= 1;
            size += 1;
        }
        while ai + size < ahi && bj + size < bhi && a[ai + size] == b[bj + size] {
            size += 1;
        }
        if size > 0 {
            matches += size;
            if alo < ai && blo < bj {
                pending.push((alo, ai, blo, bj));
            }
            if ai + size < ahi && bj + size < bhi {
                pending.push((ai + size, ahi, bj + size, bhi));
            }
        }
    }
    if a.is_empty() && b.is_empty() {
        1.0
    } else {
        2.0 * matches as f64 / (a.len() + b.len()) as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> Value {
        json!({
            "base": {
                "inventory_pos": [0.1,0.1], "items_pos": [0.2,0.1],
                "search_pos": [0.3,0.1], "item_pos": [0.4,0.1],
                "amount_pos": [0.5,0.1], "use_pos": [0.6,0.1],
                "ocr_region": [0.2,0.2,0.8,0.8]
            },
            "pop": {"match_threshold": 70, "delays": {
                "inventory": 0, "items": 0, "search": 0, "typing": 0,
                "item_click": 0, "dbl_gap": 0, "after_dbl": 0, "after_enter": 0
            }},
            "mitem": {"strange": true, "randomizer": true},
            "mpop": {"templates": {"CYBERSPACE": {"mode":"one", "items":[
                {"name":"Warp Potion", "amount":5, "min_have":5},
                {"name":"Transcendent Potion", "amount":1, "min_have":1},
                {"name":"Warp Potion", "amount":"ALL", "min_have":1}
            ]}}}
        })
    }

    fn word(text: &str) -> Word {
        Word {
            text: text.to_string(),
            rect: Rect(0.3, 0.3, 0.6, 0.4),
        }
    }

    // Runs actual Step/ACK sequencing. OCR is only supplied after an Observe;
    // each replacement screen is consumed once rather than reused after a click.
    fn run(job: &mut ItemUse, screens: &[&str]) -> Vec<String> {
        let bytes = [0_u8; 12];
        let frame = Frame::new(&bytes, 2, 2, 6, 3, false).unwrap();
        let mut events = Vec::new();
        let mut next_screen = 0;
        let mut response: Option<Vec<Word>> = None;
        let mut now = 0.0;
        for _ in 0..200 {
            let step = if let Some(words) = response.take() {
                job.tick(now, Some(&frame), &words)
            } else {
                job.tick(now, None, &[])
            };
            match step {
                Step::Input(Motion::Tap(Point(x, y), _)) => {
                    events.push(format!("tap:{x},{y}"));
                    job.ack(true, now);
                }
                Step::Text { value, submit } => {
                    assert!(submit);
                    events.push(format!("text:{value}"));
                    job.ack(true, now);
                }
                Step::Observe {
                    region: Some(_),
                    ocr: true,
                } => {
                    events.push("ocr".to_string());
                    response = Some(vec![word(screens.get(next_screen).copied().unwrap_or(""))]);
                    next_screen += 1;
                }
                Step::Wait(delay) => now += delay.max(0.01),
                Step::Done => return events,
                Step::Failed(error) => panic!("{error}"),
                _ => panic!("unexpected inventory action"),
            }
        }
        panic!("inventory batch did not terminate");
    }

    #[test]
    fn canonical_ocr_counts_and_roman_item_names() {
        for (raw, name, count) in [
            ("Warp Potion x23", "Warp Potion", Some(23)),
            ("Warp Potion × 1,234", "Warp Potion", Some(1234)),
            ("Warp Potion x1O", "Warp Potion", Some(10)),
            ("Warp Potion xl3", "Warp Potion", Some(13)),
            ("Warp Potion S6X 23 x2", "Warp Potion", Some(2)),
            ("Heavenly Potion II", "Heavenly Potion II", None),
            ("Heavenly Potion Ⅱ", "Heavenly Potion Ⅱ", None),
            ("Warp Potion", "Warp Potion", None),
        ] {
            assert_eq!(parse_ocr(raw), (name.to_string(), count), "{raw}");
        }
        assert!(!contains_digit("Heavenly Potion Ⅱ"));
        assert!(contains_digit("Warp Potion ２"));
        assert_eq!(item_amount(&json!("2.5")), None);
        assert_eq!(item_amount(&json!(2.5)), Some(2));
    }

    #[test]
    fn canonical_name_matching_rejects_other_items() {
        assert_eq!(similarity("Ｗａｒｐ\u{200b} Potion", "Warp Potion"), 100.0);
        assert_eq!(similarity("Super Warp Potion", "Warp Potion"), 0.0);
        assert_eq!(similarity("Warp Potion X", "Warp Potion"), 0.0);
        assert_eq!(similarity("Heavenly Potion II", "Heavenly Potion"), 0.0);
        assert_eq!(similarity("Warp Potlon", "Warp Potion"), 90.0);
        assert_eq!(similarity("tide", "diet"), 25.0); // difflib, not edit distance.
    }

    #[test]
    fn wrong_name_searches_twice_without_use_and_retries_later() {
        let mut job = ItemUse::new(
            &config(),
            "mitem",
            None,
            &["strange".to_string()],
            true,
            0.0,
        )
        .unwrap();
        let events = run(
            &mut job,
            &[
                "Super Strange Controller x99",
                "Super Strange Controller x99",
            ],
        );
        assert_eq!(
            events
                .iter()
                .filter(|e| *e == "text:Strange Controller")
                .count(),
            2
        );
        assert!(!events
            .iter()
            .any(|e| e == "tap:0.4,0.1" || e == "tap:0.6,0.1"));
        assert_eq!(job.used(), 0);
        assert_eq!(job.retry_at(), Some(60.0));
        assert_eq!(events.last().unwrap(), "tap:0.1,0.1");
        assert_eq!(job.status()["running"], false);
        assert!(job.take_logs().iter().any(|log| log.contains("스킵")));
    }

    #[test]
    fn valid_sequence_double_clicks_amount_and_waits_for_use_ack() {
        let mut job = ItemUse::single(&config(), "Merchant Teleporter", 1, 1, true, 0.0).unwrap();
        let bytes = [0_u8; 12];
        let frame = Frame::new(&bytes, 2, 2, 6, 3, false).unwrap();
        let mut response = false;
        let mut events = Vec::new();
        for _ in 0..50 {
            let step = job.tick(
                0.0,
                response.then_some(&frame),
                &[word("Merchant Teleporter x8")],
            );
            response = false;
            match step {
                Step::Input(Motion::Tap(Point(x, y), _)) => {
                    events.push(format!("tap:{x},{y}"));
                    assert_eq!(job.used(), 0);
                    if x == 0.6 {
                        // A scheduled Use is not a successful click.
                        assert!(matches!(job.tick(0.0, None, &[]), Step::Wait(_)));
                        job.ack(true, 3.0);
                        assert_eq!(job.used(), 1);
                        assert_eq!(job.completed()[0].amount, 1);
                        assert_eq!(job.completed()[0].at, 3.0);
                        break;
                    }
                    job.ack(true, 0.0);
                }
                Step::Text { value, .. } => {
                    events.push(format!("text:{value}"));
                    job.ack(true, 0.0);
                }
                Step::Observe { .. } => response = true,
                _ => panic!("unexpected step before Use"),
            }
        }
        assert_eq!(
            events,
            [
                "tap:0.1,0.1",
                "tap:0.2,0.1",
                "tap:0.3,0.1",
                "text:Merchant Teleporter",
                "tap:0.4,0.1",
                "tap:0.5,0.1",
                "tap:0.5,0.1",
                "text:1",
                "tap:0.6,0.1"
            ]
        );
        assert!(matches!(job.tick(3.0, None, &[]), Step::Input(_)));
        job.ack(true, 3.0);
        assert!(matches!(job.tick(3.0, None, &[]), Step::Done));
    }

    #[test]
    fn main_mode_one_falls_back_then_stops_after_first_used_item() {
        let mut job = ItemUse::new(&config(), "mpop", Some("CYBERSPACE"), &[], true, 0.0).unwrap();
        let events = run(&mut job, &["Warp Potion x4", "Transcendent Potion x3"]);
        assert_eq!(job.used(), 1);
        assert_eq!(job.completed()[0].name, "Transcendent Potion");
        assert_eq!(job.completed()[0].amount, 1);
        assert_eq!(events.iter().filter(|e| *e == "tap:0.6,0.1").count(), 1);
        assert_eq!(
            events
                .iter()
                .filter(|e| e.starts_with("text:Warp Potion"))
                .count(),
            1
        );
    }

    #[test]
    fn template_all_uses_available_count_and_clamps_requested_amount() {
        let mut cfg = config();
        cfg["mpop"]["templates"]["CYBERSPACE"] = json!({"mode":"all", "items":[
            {"name":"Warp Potion", "amount":"ALL"},
            {"name":"Transcendent Potion", "amount":999}
        ]});
        let mut job = ItemUse::new(&cfg, "mpop", Some("CYBERSPACE"), &[], true, 0.0).unwrap();
        let events = run(&mut job, &["Warp Potion x23", "Transcendent Potion x2"]);
        assert!(events.contains(&"text:23".to_string()));
        assert!(events.contains(&"text:2".to_string()));
        assert_eq!(
            job.completed().iter().map(|r| r.amount).collect::<Vec<_>>(),
            [23, 2]
        );
    }

    #[test]
    fn blood_test_reads_count_without_consuming_fragments() {
        let mut job = ItemUse::new(&config(), "mblood", None, &[], true, 0.0).unwrap();
        let events = run(&mut job, &["Cursed Rune Fragment x20"]);
        assert!(!events
            .iter()
            .any(|e| e == "tap:0.4,0.1" || e == "tap:0.6,0.1"));
        assert_eq!(job.used(), 0);
        assert_eq!(job.status()["used"], 0);
        assert!(job
            .take_logs()
            .iter()
            .any(|l| l.contains("테스트라 사용 안 함")));

        let mut job =
            ItemUse::new(&config(), "mblood", Some("GRAVEYARD"), &[], false, 0.0).unwrap();
        let events = run(&mut job, &["Cursed Rune Fragment"]);
        assert!(!events.iter().any(|e| e == "tap:0.6,0.1"));
        assert_eq!(job.used(), 0); // A single fragment must be retained.
    }

    #[test]
    fn single_implicit_count_is_one_but_unread_digits_are_skipped() {
        let mut job = ItemUse::single(&config(), "Merchant Teleporter", 1, 1, true, 0.0).unwrap();
        run(&mut job, &["Merchant Teleporter"]);
        assert_eq!(job.used(), 1);
        let mut job = ItemUse::single(&config(), "Merchant Teleporter", 1, 1, true, 0.0).unwrap();
        run(&mut job, &["Merchant Teleporter 12"]);
        assert_eq!(job.used(), 0);
        assert!(job
            .take_logs()
            .iter()
            .any(|l| l.contains("개수를 읽지 못해")));
    }

    #[test]
    fn delays_start_after_input_completion_and_ocr_requires_fresh_request() {
        let mut cfg = config();
        cfg["pop"]["delays"]["inventory"] = json!(0.5);
        let mut job = ItemUse::single(&cfg, "Warp Potion", 1, 1, false, 0.0).unwrap();
        assert!(matches!(job.tick(0.0, None, &[]), Step::Input(_)));
        assert!(matches!(job.tick(1.0, None, &[]), Step::Wait(_)));
        job.ack(true, 2.0);
        assert!(matches!(job.tick(2.1, None, &[]), Step::Wait(t) if (t-0.4).abs()<1e-6));
        assert!(matches!(job.tick(2.5, None, &[]), Step::Input(_)));
        job.ack(true, 2.5);
        assert!(matches!(job.tick(2.5, None, &[]), Step::Input(_)));
        job.ack(true, 2.5);
        assert!(matches!(job.tick(2.5, None, &[]), Step::Text { .. }));
        job.ack(true, 2.5);
        let bytes = [0_u8; 12];
        let frame = Frame::new(&bytes, 2, 2, 6, 3, false).unwrap();
        assert!(matches!(
            job.tick(2.5, Some(&frame), &[word("Warp Potion x5")]),
            Step::Observe { .. }
        ));
        assert_eq!(job.used(), 0);
        assert!(matches!(
            job.tick(2.5, Some(&frame), &[word("Warp Potion x5")]),
            Step::Input(_)
        ));
    }

    #[test]
    fn failed_input_never_records_use_and_invalid_positions_never_start() {
        let mut job = ItemUse::single(&config(), "Warp Potion", 1, 1, true, 0.0).unwrap();
        assert!(matches!(job.tick(0.0, None, &[]), Step::Input(_)));
        job.ack(false, 0.1);
        assert!(matches!(job.tick(0.1, None, &[]), Step::Failed(_)));
        assert_eq!(job.used(), 0);
        let mut cfg = config();
        cfg["base"]["use_pos"] = json!([-0.1, 0.1]);
        assert!(ItemUse::single(&cfg, "Warp Potion", 1, 1, true, 0.0).is_err());
    }

    #[test]
    fn unrelated_ocr_outside_item_region_cannot_trigger_use() {
        let mut job = ItemUse::single(&config(), "Warp Potion", 1, 1, false, 0.0).unwrap();
        let bytes = [0_u8; 12];
        let frame = Frame::new(&bytes, 2, 2, 6, 3, false).unwrap();
        let words = [Word {
            text: "Warp Potion x99".to_string(),
            rect: Rect(0.81, 0.81, 0.9, 0.9),
        }];
        for _ in 0..40 {
            match job.tick(0.0, Some(&frame), &words) {
                Step::Input(Motion::Tap(Point(x, _), _)) => {
                    assert_ne!(x, 0.4);
                    assert_ne!(x, 0.6);
                    job.ack(true, 0.0);
                }
                Step::Text { .. } => job.ack(true, 0.0),
                Step::Observe { .. } => {}
                Step::Done => {
                    assert_eq!(job.used(), 0);
                    return;
                }
                _ => panic!("unexpected step"),
            }
        }
        panic!("batch did not finish");
    }
}

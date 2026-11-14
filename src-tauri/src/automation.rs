//! Rule-based macro ownership and OCR-confirmed task execution.
//! The transport supplies fresh pixels/words and acknowledges each command;
//! this runner never focuses a host window or performs timed route replay.
#[path = "automation/inventory.rs"]
mod inventory;
#[path = "automation/memory.rs"]
mod memory;
#[path = "automation/recovery.rs"]
mod recovery;
#[path = "automation/shops.rs"]
mod shops;

use crate::{
    device::Gesture,
    navigator::Destination,
    vision::{Frame, Point, Rect, Region},
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, VecDeque};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Word {
    pub text: String,
    pub rect: Rect,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(
    tag = "kind",
    content = "data",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub(crate) enum Command {
    Gesture(Gesture),
    /// Replace the focused editable field, then optionally submit it. The
    /// transport must not append text to a previously entered search/amount.
    Text {
        text: String,
        submit: bool,
    },
    Key {
        vk: u16,
        ms: u32,
    },
    /// Only the owned Roblox session/client is eligible for replacement.
    Launch {
        url: String,
    },
}

#[derive(Debug)]
pub(crate) enum Tick {
    Observe { ocr: bool, region: Option<Region> },
    Command(Command),
    Travel(Destination),
    Wait(u32),
    Fishing,
    Done,
    Stopped(String),
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub(crate) enum Job {
    Pop {
        biome: String,
        test: bool,
    },
    Items {
        keys: Vec<String>,
        test: bool,
    },
    Blood {
        test: bool,
    },
    Merchant {
        name: String,
        calibrate: bool,
        check_only: bool,
    },
    Craft {
        name: String,
        calibrate: bool,
    },
    Match {
        calibrate: bool,
        board_only: bool,
    },
    Sell {
        test: bool,
    },
    Return {
        test: bool,
    },
    Play {
        test: bool,
    },
    InventoryOcr,
    SellCalibrate,
    Go {
        destination: Destination,
    },
    Command {
        command: Command,
    },
    Afk,
}

impl Job {
    pub(crate) fn feature(&self) -> &'static str {
        match self {
            Self::Pop { .. } => "mpop",
            Self::Items { .. } | Self::InventoryOcr => "mitem",
            Self::Blood { .. } => "mblood",
            Self::Merchant { .. } => "mmerch",
            Self::Craft { .. } => "mcraft",
            Self::Match { .. } => "mmatch",
            Self::Sell { .. } | Self::SellCalibrate => "mfish",
            Self::Return { .. } => "ret",
            Self::Play { .. } => "play",
            Self::Afk => "afk",
            Self::Go { .. } => "move",
            Self::Command { .. } => "sdk",
        }
    }
    fn label(&self) -> &'static str {
        match self {
            Self::Pop { .. } => "자동 팝핑",
            Self::Items { .. } => "아이템 사용",
            Self::InventoryOcr => "인벤토리 확인",
            Self::Blood { .. } => "블러드 레인",
            Self::Merchant { .. } => "상인 구매",
            Self::Craft { .. } => "포션 제작",
            Self::Match { .. } => "메모리 매치",
            Self::Sell { .. } => "물고기 판매",
            Self::SellCalibrate => "판매 보정",
            Self::Return { .. } => "게임 복귀",
            Self::Play { .. } => "게임 입장",
            Self::Afk => "자리 비움 방지",
            Self::Go { .. } => "목적지 이동",
            Self::Command { .. } => "모드 작업",
        }
    }
    fn priority(&self) -> u8 {
        match self {
            Self::Return { .. } | Self::Play { .. } => 4,
            Self::Pop { .. } => 3,
            Self::Blood { .. } => 2,
            Self::Afk => 0,
            _ => 1,
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) enum Event {
    Biome { name: String, occurrence: u64 },
    Merchant { name: String },
    Crafted { name: String },
    Game { available: bool, in_game: bool },
    InventoryFull,
    Return,
    CancelRecovery,
    Activity,
    Coast,
}

#[derive(Clone)]
pub(super) struct Config(Value);
impl Config {
    pub(super) fn group(&self, key: &str) -> &Value {
        &self.0[key]
    }
    pub(super) fn number(&self, group: &str, key: &str, default: f64) -> f64 {
        self.0[group][key].as_f64().unwrap_or(default)
    }
    pub(super) fn flag(&self, group: &str, key: &str, default: bool) -> bool {
        self.0[group][key].as_bool().unwrap_or(default)
    }
    pub(super) fn text(&self, group: &str, key: &str) -> String {
        self.0[group][key].as_str().unwrap_or("").trim().to_string()
    }
    pub(super) fn point(&self, group: &str, key: &str) -> Option<Point> {
        point(&self.0[group][key])
    }
    pub(super) fn rect(&self, group: &str, key: &str) -> Option<Rect> {
        rect(&self.0[group][key])
    }
    pub(super) fn patch(&mut self, group: &str, key: &str, value: Value) {
        if !self.0[group].is_object() {
            self.0[group] = json!({});
        }
        self.0[group][key] = value;
    }
    pub(super) fn delay(&self, key: &str) -> f64 {
        let default = match key {
            "item_click" | "after_enter" => 0.35,
            "dbl_gap" => 0.1,
            _ => 0.5,
        };
        self.0["pop"]["delays"][key].as_f64().unwrap_or(default)
    }
    fn validate(value: Value) -> Result<Self, String> {
        if !value.is_object() {
            return Err("매크로 설정은 JSON 객체여야 합니다.".into());
        }
        // Preserve unknown keys; validate actual fields used by this runner.
        let mut cfg = Self(value);
        for group in [
            "base", "mfish", "mpop", "mitem", "mmerch", "mcraft", "mmatch", "mblood", "pop",
            "play", "ret",
        ] {
            if !cfg.0[group].is_null() && !cfg.0[group].is_object() {
                return Err(format!("{group} 설정 형식을 확인하세요."));
            }
            if let Some(object) = cfg.0[group].as_object() {
                for (key, v) in object {
                    if key.ends_with("_pos")
                        || matches!(
                            key.as_str(),
                            "first_slot"
                                | "second_slot"
                                | "board_title"
                                | "stick_center"
                                | "stick_edge"
                                | "jump_btn"
                                | "talk_btn"
                                | "fish_btn"
                        )
                    {
                        if !v.is_null() && point(v).is_none() {
                            return Err(format!("{group}.{key} 위치가 올바르지 않습니다."));
                        }
                    }
                    if key.ends_with("_region") && !v.is_null() && rect(v).is_none() {
                        return Err(format!("{group}.{key} 영역이 올바르지 않습니다."));
                    }
                    if (key.ends_with("_wait")
                        || key.ends_with("_delay")
                        || key.ends_with("_min")
                        || matches!(
                            key.as_str(),
                            "check_sec" | "interval" | "max_time" | "start_delay" | "flip_wait"
                        ))
                        && !v.is_null()
                    {
                        let n = v
                            .as_f64()
                            .ok_or_else(|| format!("{group}.{key} 숫자를 확인하세요."))?;
                        if !n.is_finite() || !(0.0..=86400.0).contains(&n) {
                            return Err(format!("{group}.{key} 범위를 확인하세요."));
                        }
                    }
                }
            }
        }
        for (key, v) in cfg.0["pop"]["delays"].as_object().into_iter().flatten() {
            if !v
                .as_f64()
                .is_some_and(|n| n.is_finite() && (0.0..=30.0).contains(&n))
            {
                return Err(format!("pop.delays.{key} 범위를 확인하세요."));
            }
        }
        if !cfg.number("pop", "match_threshold", 70.0).is_finite()
            || !(0.0..=100.0).contains(&cfg.number("pop", "match_threshold", 70.0))
        {
            return Err("이름 일치율 범위를 확인하세요.".into());
        }
        let seeded = cfg.flag("mpop", "seeded", false);
        if cfg.0["mpop"].is_null() {
            cfg.0["mpop"] = json!({});
        }
        if cfg.0["mpop"]["templates"].is_null() {
            cfg.0["mpop"]["templates"] = json!({});
        }
        for biome in ["GLITCHED", "DREAMSPACE", "CYBERSPACE"] {
            let t = &cfg.0["mpop"]["templates"][biome];
            let empty = t["items"].as_array().is_none_or(Vec::is_empty);
            if (!seeded || t.is_null()) && empty {
                cfg.0["mpop"]["templates"][biome] = default_template(biome)?;
            }
        }
        cfg.patch("mpop", "seeded", json!(true));
        Ok(cfg)
    }
}

pub(super) fn point(value: &Value) -> Option<Point> {
    let a = value.as_array()?;
    if a.len() != 2 {
        return None;
    }
    let p = Point(a[0].as_f64()?, a[1].as_f64()?);
    valid_point(p).then_some(p)
}
pub(super) fn valid_point(p: Point) -> bool {
    p.0.is_finite() && p.1.is_finite() && (0.0..1.0).contains(&p.0) && (0.0..1.0).contains(&p.1)
}
pub(super) fn rect(value: &Value) -> Option<Rect> {
    let a = value.as_array()?;
    if a.len() != 4 {
        return None;
    }
    let r = Rect(
        a[0].as_f64()?,
        a[1].as_f64()?,
        a[2].as_f64()?,
        a[3].as_f64()?,
    );
    ([r.0, r.1, r.2, r.3]
        .iter()
        .all(|v| v.is_finite() && (0.0..=1.0).contains(v))
        && r.0 < r.2
        && r.1 < r.3)
        .then_some(r)
}
pub(super) fn center(r: Rect) -> Point {
    Point((r.0 + r.2) * 0.5, (r.1 + r.3) * 0.5)
}
pub(super) fn within(r: Rect, p: Point) -> bool {
    p.0 >= r.0 && p.0 <= r.2 && p.1 >= r.1 && p.1 <= r.3
}
pub(super) fn normalized(s: &str) -> String {
    s.chars()
        .map(|c| {
            if ('\u{ff01}'..='\u{ff5e}').contains(&c) {
                char::from_u32(c as u32 - 0xfee0).unwrap_or(c)
            } else {
                c
            }
        })
        .flat_map(char::to_lowercase)
        .filter(|c| c.is_alphanumeric())
        .collect()
}
pub(super) fn ocr_norm(s: &str) -> String {
    normalized(s)
        .chars()
        .filter(|c| c.is_alphabetic())
        .map(|c| match c {
            'a' => 'o',
            'i' => 'l',
            c => c,
        })
        .collect()
}
/// Android logcat also contains other applications' messages. This is Roblox's
/// client marker; generic network/Bluetooth disconnect text is not game evidence.
pub(crate) fn game_disconnected(line: &str) -> bool {
    line.contains("Client:Disconnect")
}
/// UI aliases apply only to fixed interface labels. Item names, quantities and
/// user text keep their original spelling and are never translated for matching.
pub(super) fn label(raw: &str, key: &str, exact: bool) -> bool {
    let n = ocr_norm(raw);
    let k = ocr_norm(key);
    if n.is_empty() || k.is_empty() {
        return false;
    }
    let aliases: &[&str] = match key.to_ascii_lowercase().as_str() {
        "play" => &["플레이", "시작", "プレイ", "開始"],
        "skip" | "click to skip" | "to skip" => &[
            "건너뛰기",
            "클릭하여 건너뛰기",
            "클릭해서 건너뛰기",
            "클릭하여 스킵",
            "눌러서 건너뛰기",
            "スキップ",
            "タップしてスキップ",
            "クリックしてスキップ",
        ],
        "inventory" => &["인벤토리", "インベントリ", "所持品"],
        "items" | "item" => &["아이템", "アイテム"],
        "search" => &["검색", "検索"],
        "use" => &["사용", "使う", "使用"],
        "sell" => &["판매", "売却", "売る"],
        "sell all" => &["모두 판매", "전부 판매", "すべて売却", "全て売る"],
        "sell fish" => &["물고기 판매", "생선 판매", "魚を売る", "魚を売却"],
        "confirm" => &["확인", "確認"],
        "cancel" => &["취소", "キャンセル"],
        "close" => &["닫기", "閉じる"],
        "quantity" | "amount" => &["수량", "数量", "個数"],
        "craft" => &["제작", "作成", "クラフト"],
        "pen recipe" | "open recipe" => &["레시피 열기", "조합법 열기", "レシピを開く"],
        "everything" | "add everything" => &["모두 추가", "전부 추가", "すべて追加", "全て追加"],
        "ingredients" => &["재료 추가", "재료", "材料を追加", "材料"],
        "notification" => &["알림", "通知"],
        "start memory match" => &[
            "메모리 매치 시작",
            "메모리 매칭 시작",
            "メモリーマッチ開始",
            "メモリーマッチを開始",
        ],
        "memory match" => &["메모리 매치", "메모리 매칭", "メモリーマッチ"],
        "chances" => &["기회", "남은 기회", "チャンス", "残り回数"],
        "available after" => &[
            "이후 이용 가능",
            "후 이용 가능",
            "후 사용 가능",
            "利用可能まで",
            "利用可能になるまで",
        ],
        "watch ad" => &["광고 보기", "광고 시청", "広告を見る", "広告を視聴"],
        "disconnected" => &["연결 끊김", "연결 해제", "接続が切断", "切断されました"],
        "open" => &["열기", "開く"],
        "leave" => &["떠나기", "나가기", "離れる", "退出"],
        "who are you" => &["당신은 누구", "누구세요", "あなたは誰"],
        "set to max" => &["최대로 설정", "최대 수량", "最大に設定", "最大数に設定"],
        "purchase" => &["구매", "購入"],
        "shop" => &["상점", "ショップ", "店"],
        "workshop" => &[
            "스텔라의 작업장",
            "스텔라 작업장",
            "스텔라의 공방",
            "ステラの工房",
            "ステラのワークショップ",
        ],
        "lantern" => &["랜턴", "ランタン"],
        "fishing shop" => &["낚시 상점", "낚시 가게", "釣りショップ", "釣具店"],
        "buy" => &["구매", "購入"],
        "no items found" | "no results" => &[
            "아이템 없음",
            "검색 결과 없음",
            "결과 없음",
            "アイテムが見つかりません",
            "結果なし",
        ],
        "no fish" | "empty" | "nothing to sell" => &[
            "물고기 없음",
            "비어 있음",
            "판매할 물고기가 없습니다",
            "魚がありません",
            "空です",
            "売却するものがありません",
        ],
        "not enough" | "missing ingredients" => &[
            "부족합니다",
            "재료 부족",
            "충분하지 않습니다",
            "不足しています",
            "材料不足",
        ],
        _ => &[],
    };
    std::iter::once(k)
        .chain(aliases.iter().map(|s| ocr_norm(s)))
        .any(|k| {
            if exact {
                n.starts_with(&k) && n.chars().count() <= k.chars().count() + 2
            } else {
                n.contains(&k)
            }
        })
}
pub(super) fn find<'a>(words: &'a [Word], keys: &[&str], exact: bool) -> Option<&'a Word> {
    words
        .iter()
        .find(|w| keys.iter().any(|key| label(&w.text, key, exact)))
}
pub(super) fn read(words: &[Word], area: Option<Rect>) -> String {
    let mut selected: Vec<_> = words
        .iter()
        .filter(|w| area.is_none_or(|r| within(r, center(w.rect))))
        .collect();
    selected.sort_by(|a, b| {
        a.rect
            .1
            .total_cmp(&b.rect.1)
            .then(a.rect.0.total_cmp(&b.rect.0))
    });
    selected
        .iter()
        .map(|w| w.text.as_str())
        .collect::<Vec<_>>()
        .join("\n")
}
pub(super) fn observe(ocr: bool, region: Option<Region>) -> Tick {
    Tick::Observe { ocr, region }
}
pub(super) fn tap(at: Point, ms: u32) -> Tick {
    Tick::Command(Command::Gesture(Gesture::Tap { at, ms }))
}
pub(super) fn key(vk: u16) -> Tick {
    Tick::Command(Command::Key { vk, ms: 60 })
}
pub(super) fn text(value: String) -> Tick {
    Tick::Command(Command::Text {
        text: value,
        submit: true,
    })
}
pub(super) fn wait(seconds: f64) -> Tick {
    Tick::Wait((seconds * 1000.0).ceil().clamp(8.0, 1000.0) as u32)
}
pub(super) fn patch_point(cfg: &mut Config, group: &str, key: &str, p: Point) {
    if valid_point(p) {
        cfg.patch(group, key, json!([p.0, p.1]));
    }
}

/// Equivalent matching-block ratio for short OCR names (no token reordering).
pub(super) fn similarity(a: &str, b: &str) -> f64 {
    let aw: Vec<_> = a
        .split_whitespace()
        .map(normalized)
        .filter(|s| !s.is_empty())
        .collect();
    let bw: Vec<_> = b
        .split_whitespace()
        .map(normalized)
        .filter(|s| !s.is_empty())
        .collect();
    if !bw.is_empty() && aw.len() > bw.len() && aw.windows(bw.len()).any(|w| w == bw) {
        return 0.0;
    }
    let a: Vec<_> = normalized(a).chars().take(256).collect();
    let b: Vec<_> = normalized(b).chars().take(256).collect();
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    fn matches(a: &[char], b: &[char]) -> usize {
        if a.is_empty() || b.is_empty() {
            return 0;
        }
        let (mut best, mut ai, mut bi) = (0, 0, 0);
        let mut prev = vec![0; b.len() + 1];
        for (i, x) in a.iter().enumerate() {
            let mut next = vec![0; b.len() + 1];
            for (j, y) in b.iter().enumerate() {
                if x == y {
                    next[j + 1] = prev[j] + 1;
                    if next[j + 1] > best {
                        best = next[j + 1];
                        ai = i + 1 - best;
                        bi = j + 1 - best;
                    }
                }
            }
            prev = next;
        }
        if best == 0 {
            0
        } else {
            best + matches(&a[..ai], &b[..bi]) + matches(&a[ai + best..], &b[bi + best..])
        }
    }
    200.0 * matches(&a, &b) as f64 / (a.len() + b.len()) as f64
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Status {
    pub running: bool,
    pub suspended: bool,
    pub active: Option<Job>,
    pub queued: usize,
    pub phase: String,
    pub message: String,
    pub commands: u64,
    pub completed: u64,
    pub failed: u64,
    pub used: BTreeMap<String, u64>,
    pub bought: BTreeMap<String, u64>,
    pub crafted: u64,
    pub played: u64,
    pub ads: u64,
    pub sold: u64,
    pub next_match: f64,
    pub clicks: u64,
    pub clock: f64,
    pub item_next: BTreeMap<String, f64>,
    pub recovery_left: Option<f64>,
}
impl Default for Status {
    fn default() -> Self {
        Self {
            running: false,
            suspended: false,
            active: None,
            queued: 0,
            phase: "idle".into(),
            message: "대기".into(),
            commands: 0,
            completed: 0,
            failed: 0,
            used: BTreeMap::new(),
            bought: BTreeMap::new(),
            crafted: 0,
            played: 0,
            ads: 0,
            sold: 0,
            next_match: 0.0,
            clicks: 0,
            clock: 0.,
            item_next: BTreeMap::new(),
            recovery_left: None,
        }
    }
}

enum Task {
    Inventory(inventory::Inventory),
    Merchant(shops::Merchant),
    Craft(shops::Craft),
    Memory(memory::Memory),
    Sale(shops::Sale),
    Recovery(recovery::Recovery),
    Go {
        destination: Destination,
        started: bool,
    },
    Command {
        command: Option<Command>,
    },
    InventoryOcr {
        requested: bool,
    },
}
struct Active {
    job: Job,
    task: Task,
    started: f64,
}
struct Queued {
    job: Job,
    at: f64,
}

/// One input owner for all macro features. Every emitted command needs an ACK
/// and fresh capture before the next decision; the transport owns generation,
/// actual capture age, foreground permission, and command cancellation.
pub(crate) struct Runner {
    cfg: Config,
    features: BTreeSet<String>,
    continuous: bool,
    queue: VecDeque<Queued>,
    active: Option<Active>,
    status: Status,
    logs: VecDeque<String>,
    now: f64,
    pending: bool,
    pending_at: f64,
    travel: bool,
    fresh: bool,
    suspended: bool,
    uncertain: bool,
    available: bool,
    in_game: bool,
    game_epoch: u64,
    missing_since: Option<f64>,
    recovery_cancelled: bool,
    biome: String,
    occurrence: u64,
    pop_seen: BTreeSet<(String, u64)>,
    blood_seen: BTreeSet<u64>,
    item_next: BTreeMap<String, f64>,
    merchant_seen: BTreeMap<String, f64>,
    merchant_counts: BTreeMap<String, u64>,
    merchant_lower: BTreeMap<String, u8>,
    merchant_seeded: bool,
    craft_seen: BTreeMap<String, f64>,
    last_monitor: f64,
    monitor_wait: bool,
    last_activity: f64,
    coast_needed: bool,
    coast_wait: bool,
}
use std::collections::BTreeSet;
impl Runner {
    pub(crate) fn new(config: Value, now: f64) -> Result<Self, String> {
        if !now.is_finite() {
            return Err("실행 시각이 올바르지 않습니다.".into());
        }
        Ok(Self {
            cfg: Config::validate(config)?,
            features: BTreeSet::new(),
            continuous: false,
            queue: VecDeque::new(),
            active: None,
            status: Status::default(),
            logs: VecDeque::new(),
            now,
            pending: false,
            pending_at: now,
            travel: false,
            fresh: false,
            suspended: false,
            uncertain: false,
            available: false,
            in_game: false,
            game_epoch: 0,
            missing_since: None,
            recovery_cancelled: false,
            biome: String::new(),
            occurrence: 0,
            pop_seen: BTreeSet::new(),
            blood_seen: BTreeSet::new(),
            item_next: BTreeMap::new(),
            merchant_seen: BTreeMap::new(),
            merchant_counts: BTreeMap::new(),
            merchant_lower: BTreeMap::new(),
            merchant_seeded: false,
            craft_seen: BTreeMap::new(),
            last_monitor: now - 600.0,
            monitor_wait: false,
            last_activity: now,
            coast_needed: false,
            coast_wait: false,
        })
    }
    pub(crate) fn start_settings(
        config: Value,
        features: &[String],
        now: f64,
    ) -> Result<Self, String> {
        let mut runner = Self::new(config, now)?;
        for feature in features {
            if ![
                "mfish", "mpop", "mitem", "mblood", "mmerch", "mcraft", "mmatch", "afk", "ret",
                "play",
            ]
            .contains(&feature.as_str())
            {
                return Err(format!("알 수 없는 매크로 기능: {feature}"));
            }
            runner.features.insert(feature.clone());
        }
        if runner.features.is_empty() {
            return Err("실행할 매크로 기능을 선택하세요.".into());
        }
        runner.continuous = true;
        runner.coast_needed = runner.enabled("mfish") && runner.option("useMove", true);
        runner.status.running = true;
        Ok(runner)
    }
    pub(crate) fn config(&self) -> &Value {
        &self.cfg.0
    }
    pub(crate) fn set_mod_settings(&mut self, id: &str, settings: Value) {
        if !self.cfg.0["mod_settings"].is_object() {
            self.cfg.0["mod_settings"] = serde_json::json!({});
        }
        self.cfg.0["mod_settings"][id] = settings;
    }
    pub(crate) fn restore_session(&mut self, previous: &Value) {
        for key in ["used", "bought"] {
            let values: BTreeMap<String, u64> = previous[key]
                .as_object()
                .into_iter()
                .flatten()
                .filter_map(|(k, v)| v.as_u64().map(|n| (k.clone(), n)))
                .collect();
            if key == "used" {
                self.status.used = values;
            } else {
                self.status.bought = values;
            }
        }
        self.status.crafted = previous["crafted"].as_u64().unwrap_or(0);
        self.status.played = previous["played"].as_u64().unwrap_or(0);
        self.status.ads = previous["ads"].as_u64().unwrap_or(0);
        self.status.sold = previous["sold"].as_u64().unwrap_or(0);
        self.status.clicks = previous["clicks"].as_u64().unwrap_or(0);
        self.status.commands = previous["commands"].as_u64().unwrap_or(0);
        self.status.completed = previous["completed"].as_u64().unwrap_or(0);
        self.status.failed = previous["failed"].as_u64().unwrap_or(0);
        self.item_next = previous["itemNext"]
            .as_object()
            .into_iter()
            .flatten()
            .filter_map(|(k, v)| {
                v.as_f64()
                    .filter(|n| n.is_finite() && *n >= 0.)
                    .map(|n| (k.clone(), n))
            })
            .collect();
        self.status.item_next = self.item_next.clone();
    }
    pub(crate) fn update_fishing_calibration(&mut self, settings: &Value) {
        for key in [
            "panel_region",
            "reel_region",
            "fish_btn",
            "bar_region",
            "cal_win",
        ] {
            self.cfg.patch("mfish", key, settings[key].clone());
        }
    }
    pub(crate) fn status(&self) -> &Status {
        &self.status
    }
    pub(crate) fn logs(&self) -> Vec<String> {
        self.logs.iter().cloned().collect()
    }
    pub(crate) fn drain_logs(&mut self) -> Vec<String> {
        self.logs.drain(..).collect()
    }
    pub(crate) fn finished(&self) -> bool {
        !self.continuous
            && self.active.is_none()
            && self.queue.is_empty()
            && !self.pending
            && !self.travel
            && !self.coast_wait
    }
    fn enabled(&self, key: &str) -> bool {
        self.features.contains(key)
    }
    fn option(&self, key: &str, default: bool) -> bool {
        self.cfg.0[key].as_bool() != Some(false)
            && self.cfg.0["mfish"][key].as_bool() != Some(false)
            && (self.cfg.0[key]
                .as_bool()
                .or_else(|| self.cfg.0["mfish"][key].as_bool())
                .unwrap_or(default))
    }
    fn note(&mut self, line: String) {
        self.status.message = line.clone();
        if self.logs.len() >= 300 {
            self.logs.pop_front();
        }
        self.logs.push_back(line);
    }
    fn clock(&mut self, now: f64) -> bool {
        if !now.is_finite() || now < self.now {
            return false;
        }
        self.now = now;
        true
    }
    pub(crate) fn queue(&mut self, job: Job) -> Result<(), String> {
        self.validate_job(&job)?;
        if self.queue.len() >= 64 {
            return Err("대기 중인 작업은 최대 64개입니다.".into());
        }
        if self.active.as_ref().is_some_and(|a| same_job(&a.job, &job))
            || self.queue.iter().any(|q| same_job(&q.job, &job))
        {
            return Err("같은 작업이 이미 실행/대기 중입니다.".into());
        }
        let priority = job.priority();
        let index = self
            .queue
            .iter()
            .position(|q| q.job.priority() < priority)
            .unwrap_or(self.queue.len());
        self.queue.insert(index, Queued { job, at: self.now });
        self.status.queued = self.queue.len();
        self.status.running = true;
        Ok(())
    }
    fn validate_job(&self, job: &Job) -> Result<(), String> {
        match job {
            Job::Pop { biome, .. } => {
                if !["CYBERSPACE", "GLITCHED", "DREAMSPACE"].contains(&biome.as_str()) {
                    return Err("팝핑 바이옴을 확인하세요.".into());
                }
                let (items, one) = inventory::template(&self.cfg, biome)?;
                if items.is_empty() {
                    return Err(format!("{biome} 팝핑 템플릿에 아이템이 없습니다."));
                }
                inventory::Inventory::new(&self.cfg, items, one, false, true).map(|_| ())
            }
            Job::Items { keys, .. } => {
                inventory::Inventory::new(&self.cfg, self.items(keys)?, false, false, true)
                    .map(|_| ())
            }
            Job::Blood { test } => inventory::Inventory::new(
                &self.cfg,
                vec![inventory::Item {
                    name: "Cursed Rune Fragment".into(),
                    amount: Some(1),
                    min_have: 2,
                }],
                true,
                *test,
                true,
            )
            .map(|_| ()),
            Job::Merchant {
                name,
                calibrate,
                check_only,
            } => shops::Merchant::new(&self.cfg, name.clone(), *calibrate, *check_only, self.now)
                .map(|_| ()),
            Job::Craft { name, calibrate } => shops::Craft::new(
                if name.trim().is_empty() {
                    self.cfg.text("mcraft", "potion")
                } else {
                    name.clone()
                },
                *calibrate,
                self.now,
            )
            .map(|_| ()),
            Job::Match { .. }
            | Job::Sell { .. }
            | Job::SellCalibrate
            | Job::Go { .. }
            | Job::Afk => Ok(()),
            Job::Return { test } => {
                recovery::Recovery::new(&self.cfg, true, *test, self.now, self.game_epoch)
                    .map(|_| ())
            }
            Job::Play { test } => {
                recovery::Recovery::new(&self.cfg, false, *test, self.now, self.game_epoch)
                    .map(|_| ())
            }
            Job::InventoryOcr => {
                if self.cfg.rect("base", "ocr_region").is_some() {
                    Ok(())
                } else {
                    Err("인벤토리 OCR 영역이 필요합니다.".into())
                }
            }
            Job::Command { command } => validate_command(command),
        }
    }
    fn items(&self, keys: &[String]) -> Result<Vec<inventory::Item>, String> {
        let selected: Vec<_> = if keys.is_empty() {
            ["strange", "randomizer"]
                .into_iter()
                .filter(|k| self.cfg.flag("mitem", k, true))
                .map(str::to_string)
                .collect()
        } else {
            keys.to_vec()
        };
        if selected.is_empty() {
            return Err("사용할 주기 아이템을 선택하세요.".into());
        }
        let mut result = Vec::new();
        for key in selected {
            let name = match key.as_str() {
                "strange" => "Strange Controller",
                "randomizer" => "Biome Randomizer",
                _ => return Err(format!("알 수 없는 주기 아이템: {key}")),
            };
            result.push(inventory::Item {
                name: name.into(),
                amount: Some(1),
                min_have: 1,
            });
        }
        Ok(result)
    }
    pub(crate) fn trigger(&mut self, event: Event, now: f64) {
        if !self.clock(now) {
            return;
        }
        match event {
            Event::Biome { name, occurrence } => {
                self.biome = name.clone();
                self.occurrence = occurrence;
                if self.enabled("mpop")
                    && self.cfg.group("mpop")["biomes_on"][&name]
                        .as_bool()
                        .unwrap_or(true)
                    && ["CYBERSPACE", "GLITCHED", "DREAMSPACE"].contains(&name.as_str())
                    && self.pop_seen.insert((name.clone(), occurrence))
                {
                    self.enqueue(Job::Pop {
                        biome: name.clone(),
                        test: false,
                    });
                }
                if self.enabled("mblood")
                    && ["PUMPKINMOON", "GRAVEYARD"].contains(&name.as_str())
                    && self.blood_seen.insert(occurrence)
                {
                    self.enqueue(Job::Blood { test: false });
                }
                if self.pop_seen.len() > 512 {
                    self.pop_seen.retain(|(_, n)| *n == occurrence);
                }
                if self.blood_seen.len() > 512 {
                    self.blood_seen.retain(|n| *n == occurrence);
                }
            }
            Event::Merchant { name } => {
                if self.enabled("mmerch")
                    && now
                        - self
                            .merchant_seen
                            .get(&name)
                            .copied()
                            .unwrap_or(now - 600.0)
                        >= 600.0
                {
                    self.merchant_seen.insert(name.clone(), now);
                    self.enqueue(Job::Merchant {
                        name,
                        calibrate: false,
                        check_only: false,
                    });
                }
            }
            Event::Crafted { name } => {
                let name = if self.cfg.text("mcraft", "potion").is_empty() {
                    shops::potion_name(&name)
                } else {
                    shops::potion_name(&self.cfg.text("mcraft", "potion"))
                };
                if self.enabled("mcraft")
                    && now - self.craft_seen.get(&name).copied().unwrap_or(now - 60.0) >= 60.0
                {
                    self.craft_seen.insert(name.clone(), now);
                    self.enqueue(Job::Craft {
                        name,
                        calibrate: false,
                    });
                }
                self.craft_seen.retain(|_, at| now - *at < 600.0);
            }
            Event::Game { available, in_game } => {
                if in_game && !self.in_game {
                    self.game_epoch = self.game_epoch.wrapping_add(1);
                }
                self.available = available;
                self.in_game = in_game;
                if in_game {
                    self.missing_since = None;
                } else if self.missing_since.is_none() {
                    self.missing_since = Some(now);
                }
                self.status.recovery_left = if self.continuous && !self.recovery_cancelled {
                    self.missing_since.map(|at| (10. - (now - at)).max(0.))
                } else {
                    None
                };
            }
            Event::InventoryFull => {
                if self.enabled("mfish") && self.option("useSell", true) {
                    self.enqueue(Job::Sell { test: false });
                } else if self.enabled("mfish") {
                    self.stop_feature("mfish");
                    self.note(
                        "인벤토리가 가득 찼습니다. 자동 판매가 꺼져 낚시를 중지합니다.".into(),
                    );
                }
            }
            Event::Return => {
                self.recovery_cancelled = false;
                self.enqueue(Job::Return { test: false });
            }
            Event::Coast => {
                self.coast_needed = self.enabled("mfish") && self.option("useMove", true);
                if self.enabled("mfish") && !self.coast_needed {
                    self.stop_feature("mfish");
                    self.note(
                        "낚시 창이 응답하지 않습니다. 자동 이동이 꺼져 낚시를 중지합니다.".into(),
                    );
                }
            }
            Event::CancelRecovery => {
                self.recovery_cancelled = true;
                self.status.recovery_left = None;
                self.stop_feature("ret");
                self.stop_feature("play");
            }
            Event::Activity => self.last_activity = now,
        }
    }
    fn enqueue(&mut self, job: Job) {
        if let Err(error) = self.queue(job) {
            if !error.contains("이미") {
                self.note(error);
            }
        }
    }
    pub(crate) fn log(&mut self, line: &str, now: f64) {
        if line.len() > 32768 || !self.clock(now) {
            return;
        }
        if line.contains("\"state\":\"Equipped") {
            self.trigger(
                Event::Game {
                    available: true,
                    in_game: true,
                },
                now,
            );
        }
        if let Some(name) = shops::crafted(line) {
            self.trigger(Event::Crafted { name }, now);
        }
        for (name, _) in shops::arrivals(line) {
            self.trigger(Event::Merchant { name }, now);
        }
        if game_disconnected(line) {
            self.trigger(
                Event::Game {
                    available: false,
                    in_game: false,
                },
                now,
            );
        }
    }
    pub(crate) fn stop_feature(&mut self, feature: &str) {
        self.features.remove(feature);
        if feature == "mfish" {
            self.coast_needed = false;
            self.coast_wait = false;
        }
        self.queue.retain(|q| q.job.feature() != feature);
        if self
            .active
            .as_ref()
            .is_some_and(|a| a.job.feature() == feature)
        {
            self.finish_active(false, Some("사용자가 작업을 중지했습니다.".into()));
            self.pending = false;
            self.travel = false;
        }
        self.status.queued = self.queue.len();
    }
    pub(crate) fn stop_all(&mut self) {
        self.finish_active(false, Some("사용자가 실행을 중지했습니다.".into()));
        self.queue.clear();
        self.pending = false;
        self.travel = false;
        self.coast_wait = false;
        self.continuous = false;
        self.status.running = false;
        self.status.suspended = false;
        self.status.queued = 0;
    }
    pub(crate) fn suspend(&mut self, now: f64) {
        if !self.clock(now) {
            return;
        }
        self.suspended = true;
        self.status.suspended = true;
        self.uncertain = self.pending;
        self.pending = false;
        self.fresh = true;
        self.note("다른 앱에서는 입력 대기 · 복귀 후 새 화면을 확인합니다.".into());
    }
    pub(crate) fn resume(&mut self, now: f64) {
        if !self.clock(now) {
            return;
        }
        self.suspended = false;
        self.status.suspended = false;
        self.fresh = true;
        if self.uncertain {
            self.uncertain = false;
            self.finish_active(false,Some("앱 전환 중 입력 결과가 불확실해 해당 작업을 중단했습니다. 중복 소모하지 않습니다.".into()));
        }
        self.note("새 게임 화면 확인 후 실행을 이어갑니다.".into());
    }
    pub(crate) fn ack(&mut self, success: bool, now: f64) {
        if !self.clock(now) || !self.pending {
            return;
        }
        self.pending = false;
        self.fresh = true;
        if !success {
            self.finish_active(
                false,
                Some("입력 완료가 확인되지 않아 현재 작업을 중지했습니다.".into()),
            );
        } else {
            self.last_activity = now;
        }
    }
    pub(crate) fn travel_result(&mut self, success: bool, now: f64) {
        if !self.clock(now) {
            return;
        }
        if self.coast_wait {
            self.coast_wait = false;
            if !success {
                self.coast_needed = true;
                self.note("해안 도착을 확인하지 못해 낚시를 시작하지 않습니다.".into());
                self.status.failed += 1;
            } else {
                self.coast_needed = false;
                self.fresh = true;
            }
            return;
        }
        if !self.travel {
            return;
        }
        self.travel = false;
        self.fresh = true;
        if !success {
            self.finish_active(
                false,
                Some("실제 목적지 도착을 확인하지 못해 작업을 중지했습니다.".into()),
            );
        }
    }
    fn monitor(&mut self, words: &[Word], now: f64) {
        if self.enabled("mmerch") {
            let raw = read(words, self.cfg.rect("mmerch", "chat_region"));
            let counts = shops::arrivals(&raw);
            if !self.merchant_seeded {
                let lines: Vec<_> = raw.lines().collect();
                let recent = shops::arrivals(&lines[lines.len().saturating_sub(4)..].join("\n"));
                for (n, count) in &counts {
                    self.merchant_counts.insert(
                        n.clone(),
                        count.saturating_sub(recent.get(n).copied().unwrap_or(0)),
                    );
                }
                self.merchant_seeded = true;
            }
            for name in ["Mari", "Jester", "Rin"] {
                let current = counts.get(name).copied().unwrap_or(0);
                let previous = self.merchant_counts.get(name).copied().unwrap_or(0);
                if current > previous {
                    self.trigger(Event::Merchant { name: name.into() }, now);
                }
                if current >= previous {
                    self.merchant_counts.insert(name.into(), current);
                    self.merchant_lower.insert(name.into(), 0);
                } else {
                    let lower = self.merchant_lower.entry(name.into()).or_default();
                    *lower += 1;
                    if *lower >= 2 {
                        self.merchant_counts.insert(name.into(), current);
                        *lower = 0;
                    }
                }
            }
        }
        if self.enabled("mcraft") {
            if let Some(name) = shops::crafted(&read(words, self.cfg.rect("base", "notice_region")))
            {
                self.trigger(Event::Crafted { name }, now);
            }
        }
    }
    fn schedule(&mut self, now: f64) {
        if self.enabled("mitem") {
            let mut due = Vec::new();
            for (key, default) in [("strange", 10.5), ("randomizer", 18.0)] {
                if self.cfg.flag("mitem", key, true)
                    && now >= self.item_next.get(key).copied().unwrap_or(now)
                {
                    due.push(key.to_string());
                    let interval = self.cfg.number("mitem", &format!("{key}_min"), default) * 60.0;
                    self.item_next.insert(key.into(), now + interval.max(60.0));
                }
            }
            if !due.is_empty() {
                self.enqueue(Job::Items {
                    keys: due,
                    test: false,
                });
            }
            self.status.item_next = self.item_next.clone();
        }
        if self.enabled("mmatch") && now >= self.status.next_match {
            self.status.next_match = now + 600.0;
            self.enqueue(Job::Match {
                calibrate: false,
                board_only: false,
            });
        }
        if self.continuous
            && !self.recovery_cancelled
            && self.missing_since.is_some_and(|at| now - at >= 10.0)
            && !self
                .active
                .as_ref()
                .is_some_and(|a| matches!(a.job, Job::Return { .. } | Job::Play { .. }))
        {
            self.recovery_cancelled = true;
            self.enqueue(Job::Return { test: false });
        }
        if self.continuous
            && self.active.is_none()
            && self.queue.is_empty()
            && now - self.last_activity >= 300.0
        {
            self.enqueue(Job::Afk);
            self.last_activity = now;
        }
        self.queue.retain(|q| match q.job {
            Job::Merchant {
                calibrate: false,
                check_only: false,
                ..
            } => now - q.at <= 180.0,
            Job::Craft {
                calibrate: false, ..
            } => now - q.at <= 300.0,
            _ => true,
        });
    }
    fn begin(&mut self, job: Job, now: f64) -> Result<(), String> {
        let task = match &job {
            Job::Pop { biome, .. } => {
                let (items, one) = inventory::template(&self.cfg, biome)?;
                Task::Inventory(inventory::Inventory::new(
                    &self.cfg,
                    items,
                    one,
                    false,
                    self.cfg.flag("mpop", "close_inventory", true),
                )?)
            }
            Job::Items { keys, .. } => Task::Inventory(inventory::Inventory::new(
                &self.cfg,
                self.items(keys)?,
                false,
                false,
                self.cfg.flag("mitem", "close_inventory", true),
            )?),
            Job::Blood { test } => Task::Inventory(inventory::Inventory::new(
                &self.cfg,
                vec![inventory::Item {
                    name: "Cursed Rune Fragment".into(),
                    amount: Some(1),
                    min_have: 2,
                }],
                true,
                *test,
                self.cfg.flag("mblood", "close_inventory", true),
            )?),
            Job::Merchant {
                name,
                calibrate,
                check_only,
            } => Task::Merchant(shops::Merchant::new(
                &self.cfg,
                name.clone(),
                *calibrate,
                *check_only,
                now,
            )?),
            Job::Craft { name, calibrate } => Task::Craft(shops::Craft::new(
                if name.trim().is_empty() {
                    self.cfg.text("mcraft", "potion")
                } else {
                    name.clone()
                },
                *calibrate,
                now,
            )?),
            Job::Match {
                calibrate,
                board_only,
            } => Task::Memory(memory::Memory::new(*calibrate, *board_only, now)),
            Job::Sell { .. } => Task::Sale(shops::Sale::new(false, now)),
            Job::SellCalibrate => Task::Sale(shops::Sale::new(true, now)),
            Job::Return { test } => Task::Recovery(recovery::Recovery::new(
                &self.cfg,
                true,
                *test,
                now,
                self.game_epoch,
            )?),
            Job::Play { test } => Task::Recovery(recovery::Recovery::new(
                &self.cfg,
                false,
                *test,
                now,
                self.game_epoch,
            )?),
            Job::Go { destination } => Task::Go {
                destination: *destination,
                started: false,
            },
            Job::Command { command } => Task::Command {
                command: Some(command.clone()),
            },
            Job::Afk => Task::Command {
                command: Some(Command::Key { vk: 0x20, ms: 80 }),
            },
            Job::InventoryOcr => Task::InventoryOcr { requested: false },
        };
        self.status.phase = job.feature().into();
        self.status.active = Some(job.clone());
        self.status.message = format!("{} 실행 중", job.label());
        let delay = match job {
            Job::Pop { .. } => self.cfg.number("mpop", "start_delay", 1.0),
            Job::Blood { .. } => self.cfg.number("mblood", "start_delay", 1.0),
            _ => 0.0,
        };
        self.active = Some(Active {
            job,
            task,
            started: now + delay,
        });
        Ok(())
    }
    pub(crate) fn tick(
        &mut self,
        now: f64,
        g: (usize, usize),
        frame: Option<&Frame<'_>>,
        words: &[Word],
    ) -> Tick {
        if !self.clock(now) {
            return Tick::Stopped("실행 시각이 역행하거나 올바르지 않습니다.".into());
        }
        self.status.clock = now;
        if g.0 == 0
            || g.1 == 0
            || g.0 > 16384
            || g.1 > 16384
            || frame.is_some_and(|f| (f.width, f.height) != g)
            || words.len() > 4096
            || words.iter().any(|w| {
                w.text.len() > 8192
                    || rect(&json!([w.rect.0, w.rect.1, w.rect.2, w.rect.3])).is_none()
            })
        {
            return Tick::Stopped("화면/OCR 데이터 형식이 올바르지 않습니다.".into());
        }
        if self.suspended {
            return wait(0.25);
        }
        if self.pending {
            if now - self.pending_at > 15.0 {
                self.pending = false;
                self.finish_active(false, Some("입력 응답 시간이 초과됐습니다.".into()));
                return Tick::Stopped("입력 응답 시간이 초과됐습니다.".into());
            }
            return wait(0.05);
        }
        if !self.available && self.continuous {
            self.schedule(now);
        }
        let recovering = self
            .active
            .as_ref()
            .is_some_and(|a| matches!(a.job, Job::Return { .. } | Job::Play { .. }))
            || self
                .queue
                .front()
                .is_some_and(|q| matches!(q.job, Job::Return { .. } | Job::Play { .. }));
        if !self.available && self.continuous && !recovering {
            return wait(0.25);
        }
        if self.travel || self.coast_wait {
            return wait(0.1);
        }
        if self.fresh && (!recovering || self.available) {
            if frame.is_none() {
                return observe(true, None);
            }
            self.fresh = false;
        }
        if self.monitor_wait {
            if frame.is_none() {
                return observe(true, None);
            }
            self.monitor_wait = false;
            self.monitor(words, now);
            self.last_monitor = now;
        }
        self.schedule(now);
        // Urgent work takes ownership only between acknowledged commands. A
        // partially entered/purchased task is not replayed after preemption.
        if self.active.as_ref().is_some_and(|a| {
            self.queue
                .front()
                .is_some_and(|q| q.job.priority() > a.job.priority())
        }) {
            self.finish_active(
                false,
                Some(
                    "긴급 바이옴/복귀 작업에 입력을 양보했습니다. 미확인 소모를 반복하지 않습니다."
                        .into(),
                ),
            );
        }
        if self.active.is_none() {
            if let Some(q) = self.queue.pop_front() {
                if let Err(error) = self.begin(q.job, now) {
                    self.status.failed += 1;
                    self.note(error);
                    self.status.queued = self.queue.len();
                    return Tick::Done;
                }
            }
        }
        self.status.queued = self.queue.len();
        if self.active.is_none() {
            self.status.active = None;
            self.status.phase = "idle".into();
            self.status.running = self.continuous;
            if self.enabled("mfish") && self.coast_needed {
                self.coast_wait = true;
                self.status.phase = "travel".into();
                return Tick::Travel(Destination::Coast);
            }
            let interval = if self.enabled("mcraft") {
                2.0
            } else {
                self.cfg.number("mmerch", "check_sec", 15.0).max(1.0)
            };
            if (self.enabled("mmerch") || self.enabled("mcraft"))
                && now - self.last_monitor >= interval
            {
                self.monitor_wait = true;
                return observe(true, None);
            }
            return if self.enabled("mfish") {
                Tick::Fishing
            } else if self.finished() {
                Tick::Done
            } else {
                wait(0.25)
            };
        }
        if now < self.active.as_ref().unwrap().started {
            return wait(self.active.as_ref().unwrap().started - now);
        }
        if now - self.active.as_ref().unwrap().started > 3600.0 {
            let message = "작업의 최대 실행 시간(1시간)을 초과했습니다.".to_string();
            self.finish_active(false, Some(message.clone()));
            return Tick::Stopped(message);
        }
        let result = {
            let a = self.active.as_mut().unwrap();
            match &mut a.task {
                Task::Inventory(t) => t.tick(&self.cfg, now, g, frame, words),
                Task::Merchant(t) => t.tick(&mut self.cfg, now, g, frame, words),
                Task::Craft(t) => t.tick(&mut self.cfg, now, g, frame, words),
                Task::Memory(t) => t.tick(&mut self.cfg, now, g, frame, words),
                Task::Sale(t) => t.tick(&mut self.cfg, now, g, frame, words),
                Task::Recovery(t) => t.tick(
                    &mut self.cfg,
                    now,
                    self.available,
                    self.in_game,
                    self.game_epoch,
                    frame,
                    words,
                ),
                Task::Go {
                    destination,
                    started,
                } => {
                    if !*started {
                        *started = true;
                        Tick::Travel(*destination)
                    } else {
                        Tick::Done
                    }
                }
                Task::Command { command } => command.take().map_or(Tick::Done, Tick::Command),
                Task::InventoryOcr { requested } => {
                    if !*requested || frame.is_none() {
                        *requested = true;
                        observe(
                            true,
                            self.cfg
                                .rect("base", "ocr_region")
                                .and_then(|r| r.region(g)),
                        )
                    } else {
                        self.status.message = read(words, self.cfg.rect("base", "ocr_region"));
                        Tick::Done
                    }
                }
            }
        };
        match result {
            Tick::Command(ref command) => {
                self.status.phase = "input".into();
                if let Err(error) = validate_command(command) {
                    self.finish_active(false, Some(error.clone()));
                    return Tick::Stopped(error);
                }
                self.pending = true;
                self.pending_at = now;
                self.status.commands += 1;
            }
            Tick::Travel(_) => {
                self.travel = true;
                self.status.phase = "travel".into();
            }
            Tick::Done => self.finish_active(true, None),
            Tick::Stopped(ref error) => self.finish_active(false, Some(error.clone())),
            Tick::Observe { .. } => self.status.phase = "observe".into(),
            Tick::Wait(_) => self.status.phase = "wait".into(),
            _ => {}
        }
        result
    }
    /// Recovery can start a missing client without inventing a viewport or
    /// accepting any click/keystroke before a real window is observed again.
    pub(crate) fn headless_missing(&mut self, now: f64) -> Tick {
        if !self.clock(now) {
            return Tick::Stopped("복구 시각이 올바르지 않습니다.".into());
        }
        self.status.clock = now;
        if self.suspended {
            return wait(0.25);
        }
        if self.pending {
            if now - self.pending_at > 15. {
                self.pending = false;
                self.finish_active(false, Some("게임 실행 응답 시간이 초과됐습니다.".into()));
                return Tick::Stopped("게임 실행 응답 시간이 초과됐습니다.".into());
            }
            return wait(0.1);
        }
        self.schedule(now);
        let recovering = |job: &Job| matches!(job, Job::Return { .. } | Job::Play { .. });
        if !self.active.as_ref().is_some_and(|a| recovering(&a.job)) {
            let Some(index) = self.queue.iter().position(|q| recovering(&q.job)) else {
                return wait(0.25);
            };
            self.finish_active(
                false,
                Some("게임 재접속을 위해 미확인 작업을 중단합니다.".into()),
            );
            self.travel = false;
            self.coast_wait = false;
            let job = self.queue.remove(index).unwrap().job;
            if let Err(error) = self.begin(job, now) {
                return Tick::Stopped(error);
            }
        }
        let result = match &mut self.active.as_mut().unwrap().task {
            Task::Recovery(task) => {
                task.tick(&mut self.cfg, now, false, false, self.game_epoch, None, &[])
            }
            _ => unreachable!(),
        };
        match &result {
            Tick::Command(Command::Launch { .. }) => {
                self.pending = true;
                self.pending_at = now;
                self.status.commands += 1;
                self.fresh = true;
            }
            Tick::Stopped(error) => self.finish_active(false, Some(error.clone())),
            Tick::Done => self.finish_active(true, None),
            Tick::Command(_) => {
                return Tick::Stopped("실제 화면 없는 입력 요청을 거절했습니다.".into())
            }
            _ => {}
        }
        result
    }
    fn finish_active(&mut self, success: bool, error: Option<String>) {
        let previous = self.stat_counts();
        let Some(a) = self.active.take() else {
            if let Some(e) = error {
                self.note(e);
            }
            return;
        };
        let return_coast = matches!(
            a.job,
            Job::Merchant {
                check_only: false,
                calibrate: false,
                ..
            } | Job::Craft {
                calibrate: false,
                ..
            } | Job::Match {
                calibrate: false,
                board_only: false
            } | Job::Sell { .. }
                | Job::Return { .. }
        );
        let mut merchant_followups = Vec::new();
        match a.task {
            Task::Inventory(t) => {
                for (k, n) in t.report.used {
                    *self.status.used.entry(k).or_default() += n;
                }
                for line in t.report.logs {
                    self.note(line);
                }
                if matches!(a.job, Job::Items { .. }) && t.report.attempted.is_empty() {
                    for key in ["strange", "randomizer"] {
                        if self.item_next.contains_key(key) {
                            self.item_next.insert(key.into(), self.now + 60.0);
                        }
                    }
                }
            }
            Task::Merchant(t) => {
                if success
                    && matches!(
                        a.job,
                        Job::Merchant {
                            check_only: true,
                            ..
                        }
                    )
                {
                    merchant_followups = t.detected_arrivals;
                }
                for (k, n) in t.bought {
                    *self.status.bought.entry(k).or_default() += n;
                }
                for (k, n) in t.report.used {
                    *self.status.used.entry(k).or_default() += n;
                }
                for line in t.report.logs {
                    self.note(line);
                }
            }
            Task::Craft(t) => {
                if t.completed {
                    self.status.crafted += 1;
                }
                for line in t.logs {
                    self.note(line);
                }
            }
            Task::Memory(t) => {
                self.status.played += t.played;
                self.status.ads += t.ads;
                self.status.next_match = t.next_at;
                for line in t.logs {
                    self.note(line);
                }
            }
            Task::Sale(t) => self.status.sold += t.sold,
            Task::Recovery(t) => self.status.clicks += t.clicks,
            _ => {}
        }
        if success {
            self.status.completed += 1;
            self.last_activity = self.now;
            if !matches!(a.job, Job::InventoryOcr) {
                self.note(format!("{} 작업 완료 · 실제 화면 확인", a.job.label()));
            }
        } else {
            self.status.failed += 1;
            if let Some(error) = error {
                self.note(error);
            }
        }
        if return_coast && self.enabled("mfish") && self.option("useMove", true) {
            self.coast_needed = true;
        }
        self.status.active = None;
        self.status.phase = "idle".into();
        // An explicit chat check treats the visible arrival lines as new, even
        // when automatic merchant monitoring is disabled. Normal queue
        // validation still applies before any teleporter can be consumed.
        for name in merchant_followups {
            let prefix = format!("{name}_");
            if self.cfg.group("mmerch")["buy"]
                .as_object()
                .is_some_and(|buy| {
                    buy.iter().any(|(key, value)| {
                        key.starts_with(&prefix) && value.as_f64().is_some_and(|n| n > 0.0)
                    })
                })
            {
                self.enqueue(Job::Merchant {
                    name,
                    calibrate: false,
                    check_only: false,
                });
            }
        }
        self.status.running = self.continuous || !self.queue.is_empty();
        let counts = self.stat_counts();
        let mut changed = false;
        if !self.cfg.0["stats"].is_object() {
            self.cfg.0["stats"] = json!({});
        }
        for (key, count) in counts {
            let delta = count.saturating_sub(previous.get(&key).copied().unwrap_or(0));
            if delta > 0 {
                let total = self.cfg.0["stats"][&key].as_u64().unwrap_or(0);
                self.cfg.0["stats"][&key] = json!(total.saturating_add(delta));
                changed = true;
            }
        }
        if changed && self.cfg.0["stats"]["since"].is_null() {
            self.cfg.0["stats"]["since"] = json!(chrono::Utc::now().timestamp());
        }
    }
    fn stat_counts(&self) -> BTreeMap<String, u64> {
        BTreeMap::from([
            (
                "item_strange".into(),
                self.status
                    .used
                    .get("Strange Controller")
                    .copied()
                    .unwrap_or(0),
            ),
            (
                "item_randomizer".into(),
                self.status
                    .used
                    .get("Biome Randomizer")
                    .copied()
                    .unwrap_or(0),
            ),
            (
                "blood_used".into(),
                self.status
                    .used
                    .get("Cursed Rune Fragment")
                    .copied()
                    .unwrap_or(0),
            ),
            ("merch_bought".into(), self.status.bought.values().sum()),
            ("crafted".into(), self.status.crafted),
            ("match_played".into(), self.status.played),
            ("match_ads".into(), self.status.ads),
        ])
    }
    /// Existing web API names mapped to the same production input scheduler.
    pub(crate) fn action(action: &str, payload: &Value) -> Result<Option<Job>, String> {
        let name = payload["name"].as_str().unwrap_or("").to_string();
        let biome = payload["biome"]
            .as_str()
            .unwrap_or("CYBERSPACE")
            .to_ascii_uppercase();
        let job = match action {
            "mpop_test" => Job::Pop { biome, test: true },
            "mitem_test" => Job::Items {
                keys: Vec::new(),
                test: true,
            },
            "mblood_test" => Job::Blood { test: true },
            "mpop_ocr_test" | "mpop_ocr" => Job::InventoryOcr,
            "mmerch_check" => Job::Merchant {
                name: if name.is_empty() { "Mari".into() } else { name },
                calibrate: false,
                check_only: true,
            },
            "mmerch_buy_test" => Job::Merchant {
                name,
                calibrate: false,
                check_only: false,
            },
            "mmerch_autocal" => Job::Merchant {
                name: if name.is_empty() { "Mari".into() } else { name },
                calibrate: true,
                check_only: false,
            },
            "mcraft_test" => Job::Craft {
                name,
                calibrate: false,
            },
            "mcraft_autocal" => Job::Craft {
                name,
                calibrate: true,
            },
            "mmatch_test" => Job::Match {
                calibrate: false,
                board_only: false,
            },
            "mmatch_autocal" => Job::Match {
                calibrate: true,
                board_only: false,
            },
            "mmatch_boardcal" => Job::Match {
                calibrate: true,
                board_only: true,
            },
            "sell_test" => Job::Sell { test: true },
            "sell_autocal" => Job::SellCalibrate,
            "ret_join" => Job::Return { test: false },
            "ret_test" => Job::Return { test: true },
            "play_test" => Job::Play { test: true },
            "go" => Job::Go {
                destination: serde_json::from_value(payload["destination"].clone())
                    .map_err(|_| "이동 목적지를 확인하세요.")?,
            },
            "command" => Job::Command {
                command: serde_json::from_value(payload["command"].clone())
                    .map_err(|_| "입력 명령 형식을 확인하세요.")?,
            },
            _ => return Ok(None),
        };
        Ok(Some(job))
    }
}
fn same_job(a: &Job, b: &Job) -> bool {
    match (a, b) {
        (Job::Pop { biome: a, .. }, Job::Pop { biome: b, .. }) => a == b,
        (Job::Merchant { name: a, .. }, Job::Merchant { name: b, .. })
        | (Job::Craft { name: a, .. }, Job::Craft { name: b, .. }) => a == b,
        (Job::Go { destination: a }, Job::Go { destination: b }) => a == b,
        _ => std::mem::discriminant(a) == std::mem::discriminant(b),
    }
}
fn validate_command(command: &Command) -> Result<(), String> {
    match command {
        Command::Gesture(g) => {
            if g.duration_ms() == 0 || g.duration_ms() > 1000 {
                return Err("입력 길이는 1~1000ms여야 합니다.".into());
            }
            let points: Vec<Point> = match g {
                Gesture::Tap { at, .. } => vec![*at],
                Gesture::Drag { from, to, .. } => vec![*from, *to],
                Gesture::Pinch { from, to, .. } => from.iter().chain(to).copied().collect(),
                Gesture::Stick { from, to, jump, .. } => {
                    let mut p = vec![*from, *to];
                    p.extend(*jump);
                    p
                }
            };
            if points.into_iter().any(|p| !valid_point(p)) {
                return Err("입력 위치가 화면 밖이거나 올바르지 않습니다.".into());
            }
        }
        Command::Text { text, .. } => {
            if text.is_empty() || text.len() > 4096 || text.chars().any(|c| c.is_control()) {
                return Err("입력 문자열을 확인하세요.".into());
            }
        }
        Command::Key { vk, ms } => {
            if *vk == 0 || *vk > 255 || *ms == 0 || *ms > 1000 {
                return Err("키 입력 형식이 올바르지 않습니다.".into());
            }
        }
        Command::Launch { url } => {
            game_url(url)?;
        }
    }
    Ok(())
}

pub(crate) fn lobby_text(text: &str) -> bool {
    let words: std::collections::BTreeSet<_> = text.split_whitespace().collect();
    (words.contains("home")
        && (words.contains("friends") || words.contains("marketplace") || words.contains("avatar")))
        || (words.contains("홈") && (words.contains("친구") || words.contains("아바타")))
}

pub(crate) fn game_url(value: &str) -> Result<String, String> {
    if value.len() > 4096 || value.chars().any(char::is_control) {
        return Err("Roblox 서버 링크를 확인하세요.".into());
    }
    if value.starts_with("roblox://") {
        if regex::Regex::new(r"^roblox://(?:placeId=[0-9]{1,20}(?:&linkCode=[A-Za-z0-9_-]{1,200})?|navigation/share_links\?code=[A-Za-z0-9_-]{1,200}&type=Server)$").unwrap().is_match(value){return Ok(value.into());}
        return Err("Roblox 접속 주소 형식을 확인하세요.".into());
    }
    let url = reqwest::Url::parse(value).map_err(|_| "Roblox 서버 링크를 확인하세요.")?;
    if !["https", "http"].contains(&url.scheme())
        || ![Some("roblox.com"), Some("www.roblox.com")].contains(&url.host_str())
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_some()
    {
        return Err("Roblox 도메인의 서버 링크가 필요합니다.".into());
    }
    let query: std::collections::BTreeMap<_, _> = url.query_pairs().into_owned().collect();
    let code = |key: &str| {
        query.get(key).filter(|s| {
            !s.is_empty()
                && s.len() <= 200
                && s.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
        })
    };
    let parts: Vec<_> = url.path().split('/').filter(|s| !s.is_empty()).collect();
    if parts.first() == Some(&"games") {
        let place = parts
            .get(1)
            .filter(|s| !s.is_empty() && s.len() <= 20 && s.bytes().all(|b| b.is_ascii_digit()))
            .ok_or("접속할 게임 번호가 없습니다.")?;
        let code = code("privateServerLinkCode")
            .or_else(|| code("privateServerLink"))
            .or_else(|| code("linkCode"));
        return Ok(format!(
            "roblox://placeId={place}{}",
            code.map(|c| format!("&linkCode={c}")).unwrap_or_default()
        ));
    }
    if parts.first() == Some(&"share")
        && query
            .get("type")
            .is_some_and(|s| s.eq_ignore_ascii_case("Server"))
    {
        if let Some(code) = code("code") {
            return Ok(format!(
                "roblox://navigation/share_links?code={code}&type=Server"
            ));
        }
    }
    Err("게임 또는 비공개 서버 접속 링크가 필요합니다.".into())
}

pub(crate) fn inventory_text(text: &str) -> Value {
    let (name, count) = inventory::parse(text);
    json!({"text":text,"name":name,"count":count})
}

/// The stable app's first-use templates. Explicit, seeded user lists are kept.
pub(crate) fn default_template(biome: &str) -> Result<Value, String> {
    match biome {
        "GLITCHED" | "DREAMSPACE" => Ok(
            json!({"mode":"all","items":[{"name":"Warp Potion","amount":1,"min_have":1},{"name":"Oblivion Potion","amount":"ALL","min_have":1},{"name":"Godlike Potion","amount":"ALL","min_have":1},{"name":"Heavenly Potion","amount":"ALL","min_have":1},{"name":"Potion of Bound","amount":"ALL","min_have":1},{"name":"Popping Potion","amount":"ALL","min_have":1}]}),
        ),
        "CYBERSPACE" => Ok(
            json!({"mode":"one","items":[{"name":"Warp Potion","amount":5,"min_have":5},{"name":"Transcendent Potion","amount":1,"min_have":1},{"name":"Tidal Shifter Potion","amount":1,"min_have":2},{"name":"Warp Potion","amount":"ALL","min_have":1}]}),
        ),
        _ => Err("알 수 없는 바이옴 템플릿입니다.".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_visible_lobby_starts_recovery_but_a_missing_ocr_label_does_not_prove_lobby() {
        assert!(lobby_text("home friends avatar"));
        assert!(!lobby_text("go home buy potion"));
        assert!(!lobby_text("auto roll inventory"));
        let mut runner = Runner::new(json!({}), 0.0).unwrap();
        runner.continuous = true;
        runner.trigger(
            Event::Game {
                available: true,
                in_game: false,
            },
            1.0,
        );
        assert_eq!(runner.missing_since, Some(1.0));
        runner.trigger(
            Event::Game {
                available: true,
                in_game: false,
            },
            9.0,
        );
        assert_eq!(runner.missing_since, Some(1.0));
        runner.trigger(
            Event::Game {
                available: true,
                in_game: true,
            },
            10.0,
        );
        assert!(runner.missing_since.is_none());
    }

    #[test]
    fn missing_client_recovery_launches_without_a_viewport_and_ack_is_not_game_entry() {
        let config = json!({"ret":{"ps_link":"https://www.roblox.com/games/15532962292/?privateServerLinkCode=test","start_wait":0},"play":{"load_wait":0}});
        let mut runner = Runner::start_settings(config, &["mpop".into()], 0.).unwrap();
        runner.trigger(
            Event::Game {
                available: false,
                in_game: false,
            },
            0.,
        );
        assert_eq!(runner.status().recovery_left, Some(10.));
        assert!(matches!(runner.headless_missing(9.9), Tick::Wait(_)));
        assert!(matches!(
            runner.headless_missing(10.),
            Tick::Command(Command::Launch { .. })
        ));
        runner.ack(true, 10.1);
        assert!(matches!(runner.headless_missing(10.2), Tick::Wait(_)));
        assert_eq!(runner.status().completed, 0);
        runner.trigger(Event::CancelRecovery, 10.3);
        assert!(runner.status().recovery_left.is_none());
        assert!(!matches!(runner.headless_missing(30.), Tick::Command(_)));
    }
    #[test]
    fn server_links_are_canonical_and_never_launch_an_unrelated_handler() {
        assert_eq!(
            game_url("https://www.roblox.com/games/15532962292/name?privateServerLinkCode=abc_1")
                .unwrap(),
            "roblox://placeId=15532962292&linkCode=abc_1"
        );
        assert_eq!(
            game_url("https://www.roblox.com/share?code=a_b&type=Server").unwrap(),
            "roblox://navigation/share_links?code=a_b&type=Server"
        );
        for url in [
            "https://roblox.com.evil.test/games/123",
            "https://roblox.com@evil.test/games/123",
            "https://www.roblox.com/account/settings",
            "roblox://evil",
            "file:///tmp/a",
            "https://www.roblox.com:999/games/123",
        ] {
            assert!(game_url(url).is_err(), "{url}");
        }
    }
    fn cfg() -> Value {
        json!({"base":{"inventory_pos":[0.1,0.1],"items_pos":[0.2,0.2],"search_pos":[0.3,0.3],"item_pos":[0.4,0.4],"amount_pos":[0.5,0.5],"use_pos":[0.6,0.6],"ocr_region":[0.1,0.1,0.9,0.9]},"mitem":{"strange":true,"randomizer":false},"mpop":{"start_delay":0.0},"mblood":{"start_delay":0.0},"unknownMod":{"private":"preserved"}})
    }
    fn word(s: &str) -> Word {
        Word {
            text: s.into(),
            rect: Rect(0.4, 0.4, 0.6, 0.6),
        }
    }
    #[test]
    fn scheduler_owns_one_command_until_ack_and_fresh_observation() {
        let mut r = Runner::new(cfg(), 0.0).unwrap();
        r.queue(Job::Command {
            command: Command::Key { vk: 32, ms: 80 },
        })
        .unwrap();
        assert!(matches!(r.tick(0.0, (10, 10), None, &[]), Tick::Command(_)));
        assert!(matches!(r.tick(0.5, (10, 10), None, &[]), Tick::Wait(_)));
        assert_eq!(r.status.commands, 1);
        assert!(!r.finished());
        r.ack(true, 1.0);
        assert!(matches!(
            r.tick(1.1, (10, 10), None, &[]),
            Tick::Observe { .. }
        ));
        let bytes = vec![40; 300];
        let frame = Frame::new(&bytes, 10, 10, 30, 3, false).unwrap();
        assert!(matches!(
            r.tick(1.2, (10, 10), Some(&frame), &[]),
            Tick::Done
        ));
        assert!(r.finished());
    }
    #[test]
    fn continuous_fishing_waits_for_actual_coast_arrival() {
        let mut r = Runner::start_settings(cfg(), &["mfish".into()], 0.0).unwrap();
        r.trigger(
            Event::Game {
                available: true,
                in_game: true,
            },
            0.,
        );
        assert!(matches!(
            r.tick(0.0, (10, 10), None, &[]),
            Tick::Travel(Destination::Coast)
        ));
        assert!(matches!(r.tick(1.0, (10, 10), None, &[]), Tick::Wait(_)));
        r.travel_result(true, 1.1);
        assert!(matches!(
            r.tick(1.2, (10, 10), None, &[]),
            Tick::Observe { .. }
        ));
        let bytes = vec![0; 300];
        let f = Frame::new(&bytes, 10, 10, 30, 3, false).unwrap();
        assert!(matches!(
            r.tick(1.3, (10, 10), Some(&f), &[]),
            Tick::Fishing
        ));
        assert!(!r.finished());
    }
    #[test]
    fn rare_biome_priority_and_occurrence_dedup_preserve_fifo() {
        let mut r = Runner::start_settings(cfg(), &["mpop".into(), "mblood".into()], 0.0).unwrap();
        r.queue(Job::Afk).unwrap();
        r.trigger(
            Event::Biome {
                name: "PUMPKINMOON".into(),
                occurrence: 1,
            },
            0.0,
        );
        r.trigger(
            Event::Biome {
                name: "CYBERSPACE".into(),
                occurrence: 2,
            },
            0.0,
        );
        r.trigger(
            Event::Biome {
                name: "CYBERSPACE".into(),
                occurrence: 2,
            },
            0.0,
        );
        assert_eq!(r.queue.len(), 3);
        assert!(matches!(r.queue[0].job, Job::Pop { .. }));
        assert!(matches!(r.queue[1].job, Job::Blood { .. }));
        assert!(matches!(r.queue[2].job, Job::Afk));
        r.trigger(
            Event::Biome {
                name: "NORMAL".into(),
                occurrence: 3,
            },
            0.0,
        );
        assert_eq!(r.queue.len(), 3);
    }
    #[test]
    fn uncertain_command_is_not_replayed_after_foreground_loss() {
        let mut r = Runner::new(cfg(), 0.0).unwrap();
        r.queue(Job::Command {
            command: Command::Text {
                text: "1".into(),
                submit: true,
            },
        })
        .unwrap();
        assert!(matches!(r.tick(0.0, (10, 10), None, &[]), Tick::Command(_)));
        r.suspend(0.1);
        r.resume(0.2);
        assert!(r.active.is_none());
        assert_eq!(r.status.commands, 1);
        assert_eq!(r.status.failed, 1);
        assert!(r.status.used.is_empty());
        r.ack(true, 0.3);
        assert_eq!(r.status.commands, 1);
    }
    #[test]
    fn settings_migration_preserves_user_data_and_deleted_templates() {
        let mut value = cfg();
        value["mpop"]["seeded"] = true.into();
        value["mpop"]["templates"] = json!({"CYBERSPACE":{"mode":"one","items":[]}});
        let r = Runner::new(value, 0.0).unwrap();
        assert_eq!(r.config()["unknownMod"]["private"], "preserved");
        assert_eq!(
            r.config()["mpop"]["templates"]["CYBERSPACE"]["items"],
            json!([])
        );
        assert_eq!(
            r.config()["mpop"]["templates"]["GLITCHED"]["items"]
                .as_array()
                .unwrap()
                .len(),
            6
        );
        assert!(Runner::new(json!({"base":{"use_pos":[1.2,0.4]}}), 0.0).is_err());
        assert!(Runner::new(cfg(), f64::NAN).is_err());
    }
    #[test]
    fn fixed_ui_labels_are_multilingual_but_item_names_are_not_translated() {
        assert!(find(&[word("검색")], &["Search"], true).is_some());
        assert!(find(&[word("インベントリ")], &["Inventory"], true).is_some());
        assert!(find(&[word("모두 판매")], &["Sell All"], true).is_some());
        assert!(find(&[word("レシピを開く")], &["pen Recipe"], false).is_some());
        assert_eq!(normalized("Ｗａｒｐ Potion"), "warppotion");
        assert!(similarity("워프 포션", "Warp Potion") < 70.0);
    }
    #[test]
    fn commands_roundtrip_and_reject_outside_coordinates() {
        let c = Command::Gesture(Gesture::Tap {
            at: Point(0.4, 0.5),
            ms: 30,
        });
        let encoded = serde_json::to_value(&c).unwrap();
        assert_eq!(encoded["kind"], "gesture");
        assert_eq!(encoded["data"]["kind"], "tap");
        assert!(serde_json::from_value::<Command>(encoded).is_ok());
        assert!(validate_command(&Command::Gesture(Gesture::Tap {
            at: Point(f64::NAN, 0.5),
            ms: 30
        }))
        .is_err());
        assert!(validate_command(&Command::Key { vk: 32, ms: 1001 }).is_err());
        assert!(validate_command(&Command::Launch {
            url: "https://www.roblox.com.evil.test/games/x".into()
        })
        .is_err());
    }
    #[test]
    fn merchant_and_craft_events_deduplicate_observations() {
        let mut r =
            Runner::start_settings(cfg(), &["mmerch".into(), "mcraft".into()], 0.0).unwrap();
        r.cfg.patch("mmerch", "buy", json!({"Mari_Gear A":1}));
        r.trigger(
            Event::Merchant {
                name: "Mari".into(),
            },
            0.0,
        );
        r.trigger(
            Event::Merchant {
                name: "Mari".into(),
            },
            1.0,
        );
        r.trigger(
            Event::Crafted {
                name: "Fortune Potion II".into(),
            },
            1.0,
        );
        r.trigger(
            Event::Crafted {
                name: "Fortune Potion II".into(),
            },
            2.0,
        );
        assert_eq!(r.queue.len(), 2);
        r.stop_feature("mcraft");
        assert_eq!(r.queue.len(), 1);
    }
}

//! The original PC merchant, potion-crafting and memory-match jobs.
//! Game observations and input execution stay in the desktop worker.
use crate::{
    items::ItemUse,
    macro_step::Step,
    navigation::Motion,
    ocr::Word,
    vision::{Frame, Point, Rect},
};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

pub(crate) enum Activity {
    Merchant(Merchant),
    Craft(Crafter),
    Memory(memory::Memory),
    SellCal(SellCalibration),
}

impl Activity {
    pub(crate) fn new(feature: &str, config: &Value, now: f64) -> Result<Self, String> {
        match feature {
            "mmerch" => Self::merchant(config, "Mari", now),
            "mcraft" => Self::craft(
                config,
                config["mcraft"]["potion"].as_str().unwrap_or(""),
                now,
            ),
            "mmatch" => Self::memory(config, false, now),
            _ => Err("지원하지 않는 작업입니다.".into()),
        }
    }
    pub(crate) fn merchant(config: &Value, name: &str, now: f64) -> Result<Self, String> {
        Ok(Self::Merchant(Merchant::new(config, name, now)?))
    }
    pub(crate) fn craft(config: &Value, name: &str, now: f64) -> Result<Self, String> {
        Ok(Self::Craft(Crafter::new(config, name, now)?))
    }
    pub(crate) fn memory(config: &Value, here: bool, now: f64) -> Result<Self, String> {
        Ok(Self::Memory(memory::Memory::new(config, here, now)?))
    }
    pub(crate) fn calibrate(feature: &str, config: &Value, now: f64) -> Result<Self, String> {
        match feature {
            "sell" => Ok(Self::SellCal(SellCalibration::new(now))),
            "mmerch" => Ok(Self::Merchant(Merchant::calibrate(config, now))),
            "mcraft" => Ok(Self::Craft(Crafter::calibrate(config, now))),
            "mmatch" | "mmatch_board" => Ok(Self::Memory(memory::Memory::calibrate(
                config,
                feature == "mmatch_board",
                now,
            )?)),
            _ => Err("지원하지 않는 자동 보정입니다.".into()),
        }
    }
    pub(crate) fn tick(&mut self, now: f64, frame: Option<&Frame<'_>>, words: &[Word]) -> Step {
        match self {
            Self::Merchant(job) => job.tick(now, frame, words),
            Self::Craft(job) => job.tick(now, frame, words),
            Self::Memory(job) => job.tick(now, frame, words),
            Self::SellCal(job) => job.tick(now, frame, words),
        }
    }
    pub(crate) fn ack(&mut self, ok: bool, now: f64) {
        match self {
            Self::Merchant(job) => job.ack(ok, now),
            Self::Craft(job) => job.script.ack(ok, now),
            Self::Memory(job) => job.ack(ok, now),
            Self::SellCal(job) => job.script.ack(ok, now),
        }
    }
    pub(crate) fn status(&self) -> Value {
        match self {
            Self::Merchant(job) => {
                let running = !job.finished && job.script.error.is_none();
                json!({"running":running,"msg":job.script.error.as_ref().unwrap_or(&job.message),"job":if running {json!("buy")}else{Value::Null},"bought":job.bought})
            }
            Self::Craft(job) => {
                let running = !job.finished && job.script.error.is_none();
                json!({"running":running,"msg":job.script.error.as_ref().unwrap_or(&job.message),"name":if running {json!(job.name)}else{Value::Null},"crafted":job.crafted})
            }
            Self::Memory(job) => job.status(),
            Self::SellCal(job) => json!({
                "running": !job.finished && job.script.error.is_none(),
                "msg": job.error.as_ref().or(job.script.error.as_ref()).unwrap_or(&job.message),
                "notes": job.notes,
            }),
        }
    }
    pub(crate) fn next_at(&self) -> Option<f64> {
        if let Self::Memory(job) = self {
            Some(job.next_at())
        } else {
            None
        }
    }
    /// OCR preprocessing used by main for card amounts and the large chances digit.
    pub(crate) fn ocr_options(&self) -> (u32, u32) {
        if let Self::Memory(job) = self {
            job.ocr_options()
        } else {
            (1, 0)
        }
    }
    pub(crate) fn take_logs(&mut self) -> Vec<String> {
        if let Self::Merchant(job) = self {
            job.teleporter
                .as_mut()
                .map_or_else(Vec::new, ItemUse::take_logs)
        } else {
            Vec::new()
        }
    }
    /// Read once after completion; merge the nested group into the saved settings.
    pub(crate) fn config_patch(&mut self) -> Option<Value> {
        match self {
            Self::Merchant(job) => job.patch.take().map(|patch| json!({"mmerch":patch})),
            Self::Craft(job) => job.patch.take().map(|patch| json!({"mcraft":patch})),
            Self::Memory(job) => job.config_patch().map(|patch| json!({"mmatch":patch})),
            Self::SellCal(job) => job.patch.take(),
        }
    }
}

fn norm(text: &str) -> String {
    text.to_ascii_lowercase()
        .chars()
        .filter(char::is_ascii_lowercase)
        .map(|c| match c {
            'a' => 'o',
            'i' => 'l',
            _ => c,
        })
        .collect()
}
fn center(word: &Word) -> Point {
    word.rect.at(0.5, 0.5)
}
fn point(config: &Value, key: &str) -> Option<Point> {
    let value = config.get(key)?.as_array()?;
    if value.len() != 2 {
        return None;
    }
    let point = Point(value[0].as_f64()?, value[1].as_f64()?);
    [point.0, point.1]
        .iter()
        .all(|v| v.is_finite() && (0.0..=1.0).contains(v))
        .then_some(point)
}
fn region(config: &Value, key: &str) -> Option<Rect> {
    let value = config.get(key)?.as_array()?;
    if value.len() != 4 {
        return None;
    }
    let r = Rect(
        value[0].as_f64()?,
        value[1].as_f64()?,
        value[2].as_f64()?,
        value[3].as_f64()?,
    );
    ([r.0, r.1, r.2, r.3]
        .iter()
        .all(|v| v.is_finite() && (0.0..=1.0).contains(v))
        && r.0 < r.2
        && r.1 < r.3)
        .then_some(r)
}
fn number(config: &Value, key: &str, default: f64) -> f64 {
    config[key]
        .as_f64()
        .filter(|n| n.is_finite())
        .unwrap_or(default)
}
fn words_text(words: &[Word]) -> String {
    words
        .iter()
        .map(|w| w.text.as_str())
        .collect::<Vec<_>>()
        .join("\n")
}
fn place(config: &Value, feature: &str, key: &str) -> Option<usize> {
    config["move"]["places"].as_array()?.iter().position(|p| {
        p["feat"] == feature
            && p["key"] == key
            && p["route"]["ev"].as_array().is_some_and(|ev| !ev.is_empty())
    })
}
fn dims(frame: Option<&Frame<'_>>) -> Option<(f64, f64)> {
    frame.map(|f| (f.width as f64, f.height as f64))
}
fn offset(anchor: Point, dx: f64, dy: f64, k: f64, size: (f64, f64)) -> Point {
    Point(
        (anchor.0 + dx * k / size.0).clamp(0., 1.),
        (anchor.1 + dy * k / size.1).clamp(0., 1.),
    )
}
fn put_point(config: &mut Value, key: &str, p: Point) {
    config[key] = json!([p.0, p.1]);
}
fn put_region(config: &mut Value, key: &str, r: Rect) {
    config[key] = json!([r.0, r.1, r.2, r.3]);
}
fn text_point(words: &[Word], key: &str, exact: bool) -> Option<Point> {
    let key = norm(key);
    words.iter().find_map(|word| {
        let text = norm(&word.text);
        if !(if exact {
            text.starts_with(&key) && text.len() <= key.len() + 2
        } else {
            text.contains(&key)
        }) {
            return None;
        }
        if text.len() <= key.len() + 2 {
            return Some(center(word));
        }
        let indices: Vec<_> = word
            .text
            .chars()
            .enumerate()
            .filter_map(|(i, c)| c.is_ascii_alphabetic().then_some(i))
            .collect();
        let index = text.find(&key)?;
        let first = *indices.get(index)?;
        let last = *indices.get(index + key.len() - 1)? + 1;
        Some(word.rect.at(
            (first + last) as f64 / 2. / word.text.chars().count() as f64,
            0.5,
        ))
    })
}

// Two jobs contain short fixed input sequences. Acknowledgement owns the clock:
// no search, dialog or animation wait starts while input is still executing.
#[derive(Default)]
struct Script {
    actions: VecDeque<(Step, f64)>,
    awaiting: bool,
    after: f64,
    until: f64,
    error: Option<String>,
}
impl Script {
    fn input(&mut self, input: Motion, wait: f64) {
        self.actions.push_back((Step::Input(input), wait));
    }
    fn tap(&mut self, p: Point, wait: f64) {
        self.input(Motion::Tap(p, 25), wait);
    }
    fn key(&mut self, key: &'static [u16], wait: f64) {
        self.input(Motion::Keys(key, 40), wait);
    }
    fn text(&mut self, value: String, submit: bool, wait: f64) {
        self.actions.push_back((Step::Text { value, submit }, wait));
    }
    fn reset(&mut self) {
        for key in [&[0x1b][..], &[0x52][..], &[0x0d][..]] {
            self.key(key, 0.5);
        }
    }
    fn tick(&mut self, now: f64) -> Option<Step> {
        if let Some(error) = &self.error {
            return Some(Step::Failed(error.clone()));
        }
        if self.awaiting {
            return Some(Step::Wait(0.01));
        }
        if now < self.until {
            return Some(Step::Wait(self.until - now));
        }
        if let Some((action, after)) = self.actions.pop_front() {
            self.awaiting = true;
            self.after = after;
            Some(action)
        } else {
            None
        }
    }
    fn ack(&mut self, ok: bool, now: f64) {
        if !self.awaiting {
            return;
        }
        self.awaiting = false;
        if ok {
            self.until = now + self.after;
        } else {
            self.actions.clear();
            self.error = Some("게임 입력을 완료하지 못했습니다.".into());
        }
    }
}

enum SellCalStage {
    WaitDialog,
    SettleDialog,
    Dialog,
    Shop,
    SettleShop,
    SelectFirst,
    SellAll,
    Confirm,
    Close,
    Verify,
}

/// sell.autocal: observe Captain Flarg's dialogue and shop, then cancel the
/// confirmation. No selling input is issued while measuring these positions.
pub(crate) struct SellCalibration {
    stage: SellCalStage,
    script: Script,
    observing: bool,
    observe_region: Option<Rect>,
    deadline: f64,
    attempts: u8,
    dialog: Option<Point>,
    title: Option<Word>,
    sell_all: Option<Word>,
    geometry: Option<Value>,
    patch: Option<Value>,
    notes: Vec<String>,
    message: String,
    error: Option<String>,
    finished: bool,
}

impl SellCalibration {
    fn new(now: f64) -> Self {
        Self {
            stage: SellCalStage::WaitDialog,
            script: Script::default(),
            observing: false,
            observe_region: None,
            deadline: now + 120.,
            attempts: 0,
            dialog: None,
            title: None,
            sell_all: None,
            geometry: None,
            patch: None,
            notes: Vec::new(),
            message: "Captain Flarg 앞에서 E 를 눌러 주세요".into(),
            error: None,
            finished: false,
        }
    }

    fn observe(&mut self, region: Option<Rect>) -> Step {
        self.observing = true;
        self.observe_region = region;
        Step::Observe { region, ocr: true }
    }

    fn fail(&mut self, message: &str) -> Step {
        self.finished = true;
        self.error = Some(message.into());
        Step::Failed(message.into())
    }

    fn save_point(&mut self, group: &str, key: &str, p: Point) {
        let patch = self.patch.get_or_insert_with(|| json!({}));
        if !patch[group].is_object() {
            patch[group] = json!({});
        }
        put_point(&mut patch[group], key, p);
    }

    fn tick(&mut self, now: f64, frame: Option<&Frame<'_>>, words: &[Word]) -> Step {
        if self.finished {
            return self
                .error
                .as_ref()
                .map_or(Step::Done, |e| Step::Failed(e.clone()));
        }
        if self.observing && frame.is_none() {
            return Step::Observe {
                region: self.observe_region,
                ocr: true,
            };
        }
        if let Some(step) = self.script.tick(now) {
            return step;
        }
        loop {
            match self.stage {
                SellCalStage::WaitDialog => {
                    if !self.observing {
                        return self.observe(Some(Rect(0., 0.4, 1., 1.)));
                    }
                    self.observing = false;
                    let dialog = sell_dialog_point(words);
                    if dialog.is_some() || text_point(words, "sellfish", false).is_some() {
                        self.dialog = dialog;
                        self.message = "대화창 찾음 — 이제 만지지 마세요".into();
                        self.stage = SellCalStage::SettleDialog;
                        self.script.until = now + 0.6;
                        return Step::Wait(0.6);
                    }
                    if now >= self.deadline {
                        return self
                            .fail("대화창이 안 보입니다. Captain Flarg 앞에서 E 를 눌러 주세요.");
                    }
                    self.script.until = now + 0.4;
                    return Step::Wait(0.4);
                }
                SellCalStage::SettleDialog => {
                    if !self.observing {
                        return self.observe(Some(Rect(0., 0.4, 1., 1.)));
                    }
                    self.observing = false;
                    self.dialog = sell_dialog_point(words).or(self.dialog);
                    if let Some(p) = self.dialog {
                        self.save_point("base", "dialog_pos", p);
                    }
                    self.deadline = now + 12.;
                    self.stage = SellCalStage::Dialog;
                    // These words are the fresh post-settle dialogue observation.
                    self.observing = true;
                }
                SellCalStage::Dialog => {
                    if !self.observing {
                        return self.observe(Some(Rect(0., 0.4, 1., 1.)));
                    }
                    self.observing = false;
                    if let Some(p) = text_point(words, "sellfish", false) {
                        self.save_point("mfish", "sell_fish_pos", sell_ratio(p.0, p.1));
                        if self.dialog.is_none() {
                            self.notes.push("대화창은 못 찾음".into());
                        }
                        self.message = "상점 여는 중".into();
                        self.script.tap(p, 0.6);
                        self.attempts = 0;
                        self.stage = SellCalStage::Shop;
                        return self.script.tick(now).unwrap();
                    }
                    if now >= self.deadline {
                        return self.fail("[Sell Fish] 버튼을 찾지 못했습니다.");
                    }
                    let Some(dialog) = self.dialog else {
                        return self.fail("대화창을 찾지 못했습니다.");
                    };
                    self.script.tap(dialog, 0.35);
                    return self.script.tick(now).unwrap();
                }
                SellCalStage::Shop => {
                    if !self.observing {
                        return self.observe(None);
                    }
                    self.observing = false;
                    self.title = sell_word(words, "fishingshop", false);
                    self.sell_all = sell_word(words, "sellall", true);
                    if self.title.is_some() && self.sell_all.is_some() {
                        self.stage = SellCalStage::SettleShop;
                        self.script.until = now + 0.4;
                        return Step::Wait(0.4);
                    }
                    self.attempts += 1;
                    if self.attempts >= 8 {
                        return self.fail("상점(Fishing Shop · Sell All)을 찾지 못했습니다.");
                    }
                    self.script.until = now + 0.6;
                    return Step::Wait(0.6);
                }
                SellCalStage::SettleShop => {
                    if !self.observing {
                        return self.observe(None);
                    }
                    self.observing = false;
                    self.title = sell_word(words, "fishingshop", false).or(self.title.take());
                    self.sell_all = sell_word(words, "sellall", true).or(self.sell_all.take());
                    let Some((width, height)) = dims(frame).filter(|(w, h)| *w > 0. && *h > 0.)
                    else {
                        return self.fail("상점을 측정할 실제 게임 화면 크기가 없습니다.");
                    };
                    let Some(geometry) = sell_shop_layout(
                        words,
                        self.title.as_ref().unwrap(),
                        self.sell_all.as_ref().unwrap(),
                        width / height,
                    ) else {
                        return self.fail("상점 Buy · Sell 탭을 찾지 못했습니다.");
                    };
                    self.save_point(
                        "mfish",
                        "sell_all_pos",
                        center(self.sell_all.as_ref().unwrap()),
                    );
                    for key in ["first_fish_pos", "shop_close_pos"] {
                        self.save_point("mfish", key, point(&geometry, key).unwrap());
                    }
                    self.geometry = Some(geometry);
                    self.message = "확인창 확인 중 (팔지는 않음)".into();
                    self.stage = SellCalStage::SelectFirst;
                }
                SellCalStage::SelectFirst => {
                    self.script.tap(
                        point(self.geometry.as_ref().unwrap(), "first_fish_pos").unwrap(),
                        0.6,
                    );
                    self.stage = SellCalStage::SellAll;
                    return self.script.tick(now).unwrap();
                }
                SellCalStage::SellAll => {
                    self.script
                        .tap(center(self.sell_all.as_ref().unwrap()), 0.6);
                    self.attempts = 0;
                    self.stage = SellCalStage::Confirm;
                    return self.script.tick(now).unwrap();
                }
                SellCalStage::Confirm => {
                    if !self.observing {
                        return self.observe(None);
                    }
                    self.observing = false;
                    if let Some((sell, cancel)) = sell_confirmation(words) {
                        self.save_point("mfish", "confirm_sell_pos", sell);
                        self.script.tap(cancel, 0.6);
                        self.stage = SellCalStage::Close;
                        return self.script.tick(now).unwrap();
                    }
                    self.attempts += 1;
                    if self.attempts >= 4 {
                        // Main explicitly labels this geometry estimate instead of
                        // claiming a confirmation dialog was actually observed.
                        self.save_point(
                            "mfish",
                            "confirm_sell_pos",
                            point(self.geometry.as_ref().unwrap(), "confirm_sell_pos").unwrap(),
                        );
                        self.notes
                            .push("확인창이 안 떠서(물고기 없음?) 확인 Sell 은 계산한 자리".into());
                        self.stage = SellCalStage::Close;
                        continue;
                    }
                    self.script.until = now + 0.6;
                    return Step::Wait(0.6);
                }
                SellCalStage::Close => {
                    self.message = "상점 닫는 중".into();
                    self.script.tap(
                        point(self.geometry.as_ref().unwrap(), "shop_close_pos").unwrap(),
                        0.8,
                    );
                    self.stage = SellCalStage::Verify;
                    return self.script.tick(now).unwrap();
                }
                SellCalStage::Verify => {
                    if !self.observing {
                        return self.observe(None);
                    }
                    self.observing = false;
                    if text_point(words, "fishingshop", false).is_some() {
                        self.notes
                            .push("상점이 안 닫힘 — 상점 닫기 X 를 직접 지정해 주세요".into());
                    }
                    self.patch.as_mut().unwrap()["mfish"]["cal_sell"] = true.into();
                    self.finished = true;
                    self.message = "판매 자동 보정 완료".into();
                    return Step::Done;
                }
            }
        }
    }
}

fn sell_ratio(x: f64, y: f64) -> Point {
    Point(
        (x.clamp(0., 1.) * 10000.).round() / 10000.,
        (y.clamp(0., 1.) * 10000.).round() / 10000.,
    )
}

fn sell_word(words: &[Word], key: &str, exact: bool) -> Option<Word> {
    let key = norm(key);
    words.iter().find_map(|word| {
        let text = norm(&word.text);
        if !(if exact {
            text.starts_with(&key) && text.len() <= key.len() + 2
        } else {
            text.contains(&key)
        }) {
            return None;
        }
        let mut found = word.clone();
        if text.len() > key.len() + 2 {
            let indices: Vec<_> = word
                .text
                .chars()
                .enumerate()
                .filter_map(|(i, c)| c.is_ascii_alphabetic().then_some(i))
                .collect();
            let i = text.find(&key)?;
            let (a, z) = (*indices.get(i)?, *indices.get(i + key.len() - 1)? + 1);
            let len = word.text.chars().count() as f64;
            found.rect.0 = word.rect.at(a as f64 / len, 0.).0;
            found.rect.2 = word.rect.at(z as f64 / len, 0.).0;
        }
        Some(found)
    })
}

fn sell_dialog_point(words: &[Word]) -> Option<Point> {
    let line = sell_word(words, "orrr", false).or_else(|| sell_word(words, "whotdoyou", false));
    let skip = sell_word(words, "clicktoskip", false).or_else(|| sell_word(words, "toskip", false));
    let reference = line.as_ref().or(skip.as_ref())?;
    let y = center(reference).1;
    let mut ys = vec![y];
    if let Some(name_y) = words
        .iter()
        .filter(|w| norm(&w.text).contains("coptolnflorg") && center(w).1 < y)
        .map(|w| center(w).1)
        .max_by(f64::total_cmp)
    {
        ys.push(name_y);
    }
    if line.is_some() {
        if let Some(skip) = skip {
            ys.push(center(&skip).1);
        }
    }
    Some(sell_ratio(0.5, ys.iter().sum::<f64>() / ys.len() as f64))
}

fn sell_shop_layout(words: &[Word], title: &Word, sell_all: &Word, aspect: f64) -> Option<Value> {
    let t = center(title);
    let sa = center(sell_all);
    let buy = words
        .iter()
        .find(|w| norm(&w.text) == "buy" && center(w).1 > t.1)
        .map(center)?;
    let tab = words
        .iter()
        .find(|w| {
            norm(&w.text) == "sell" && (center(w).1 - buy.1).abs() < 0.03 && center(w).0 > buy.0
        })
        .map(center)?;
    let half = (tab.0 - buy.0) / 2.;
    let (left, right) = (buy.0 - half, tab.0 + half);
    let span = sa.1 - t.1;
    if !aspect.is_finite() || aspect <= 0. || span <= 0. {
        return None;
    }
    let mut names: Vec<_> = words
        .iter()
        .filter(|w| {
            let p = center(w);
            left < p.0
                && p.0 < right
                && buy.1 + 0.02 < p.1
                && p.1 < sa.1 + 0.05
                && norm(&w.text).len() >= 3
        })
        .collect();
    names.sort_by(|a, b| center(a).1.total_cmp(&center(b).1));
    let first = if let Some(name) = names.first() {
        let top = center(name).1;
        let height = name.rect.3 - name.rect.1;
        let mut rows: Vec<_> = names
            .iter()
            .map(|w| (center(w).1 * 100.).round() / 100.)
            .collect();
        rows.sort_by(f64::total_cmp);
        rows.dedup();
        let pitch = rows
            .into_iter()
            .map(|y| y - top)
            .find(|d| *d > height * 1.5)
            .unwrap_or_else(|| {
                let mut xs: Vec<_> = names
                    .iter()
                    .filter(|w| (center(w).1 - top).abs() < height)
                    .map(|w| center(w).0)
                    .collect();
                xs.sort_by(f64::total_cmp);
                if xs.len() > 1 {
                    (xs[1] - xs[0]) * aspect
                } else {
                    height * 3.
                }
            });
        sell_ratio(
            names.iter().map(|w| center(w).0).min_by(f64::total_cmp)?,
            top - 0.3 * pitch,
        )
    } else {
        let w = (right - left) / 5.;
        sell_ratio(left + w * 0.55, buy.1 + w * aspect * 0.8)
    };
    let x1 = title.rect.0 - 0.005;
    let mut out = json!({});
    put_point(&mut out, "first_fish_pos", first);
    put_point(
        &mut out,
        "shop_close_pos",
        sell_ratio(right - 0.85 * (title.rect.3 - title.rect.1) / aspect, t.1),
    );
    put_point(
        &mut out,
        "confirm_sell_pos",
        sell_ratio((x1 + right) / 2. - 0.155 * (right - x1), t.1 + 0.66 * span),
    );
    Some(out)
}

fn sell_confirmation(words: &[Word]) -> Option<(Point, Point)> {
    let cancel = center(&sell_word(words, "cancel", true)?);
    let sell = words
        .iter()
        .filter(|w| {
            norm(&w.text) == "sell"
                && (center(w).1 - cancel.1).abs() < 0.04
                && center(w).0 < cancel.0
        })
        .map(center)
        .max_by(|a, b| a.0.total_cmp(&b.0))?;
    Some((sell_ratio(sell.0, sell.1), sell_ratio(cancel.0, cancel.1)))
}

fn sequence_ratio(a: &str, b: &str) -> f64 {
    // difflib.SequenceMatcher's matching-block ratio; these short item names do
    // not reach Python's 200-character automatic-junk threshold.
    fn matched(a: &[u8], b: &[u8]) -> usize {
        let (mut ai, mut bi, mut best) = (0, 0, 0);
        let mut previous = vec![0; b.len() + 1];
        for (i, &x) in a.iter().enumerate() {
            let mut current = vec![0; b.len() + 1];
            for (j, &y) in b.iter().enumerate() {
                if x == y {
                    current[j + 1] = previous[j] + 1;
                    if current[j + 1] > best {
                        best = current[j + 1];
                        ai = i + 1 - best;
                        bi = j + 1 - best;
                    }
                }
            }
            previous = current;
        }
        if best == 0 {
            0
        } else {
            best + matched(&a[..ai], &b[..bi]) + matched(&a[ai + best..], &b[bi + best..])
        }
    }
    if a.is_empty() && b.is_empty() {
        1.
    } else {
        2. * matched(a.as_bytes(), b.as_bytes()) as f64 / (a.len() + b.len()) as f64
    }
}

const MERCHANT_ITEMS: &[&str] = &[
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
fn merchant_item(text: &str) -> String {
    let first = text
        .lines()
        .next()
        .unwrap_or("")
        .split('|')
        .next()
        .unwrap_or("");
    let rarities = [
        "common",
        "uncommon",
        "rare",
        "epic",
        "legendary",
        "mythic",
        "exalted",
        "divine",
        "special",
        "unique",
        "limited",
    ];
    first
        .split_whitespace()
        .take_while(|word| !rarities.contains(&word.to_ascii_lowercase().as_str()))
        .filter(|word| {
            let s = word.to_ascii_lowercase();
            !s.chars()
                .all(|c| c.is_ascii_digit() || c == ',' || c == '.')
                && !(s.contains('x')
                    && s.chars().any(|c| c.is_ascii_digit())
                    && s.chars().all(|c| "0123456789soIl|x,.".contains(c)))
        })
        .collect::<Vec<_>>()
        .join(" ")
}
fn match_merchant(text: &str, wanted: &BTreeSet<String>) -> Option<(String, f64)> {
    let needle = norm(text);
    if needle.is_empty() {
        return None;
    }
    let mut candidates: Vec<&str> = MERCHANT_ITEMS.to_vec();
    candidates.extend(
        wanted
            .iter()
            .map(String::as_str)
            .filter(|n| !MERCHANT_ITEMS.contains(n)),
    );
    candidates
        .into_iter()
        .map(|n| (n.to_owned(), sequence_ratio(&needle, &norm(n)) * 100.))
        .max_by(|a, b| a.1.total_cmp(&b.1))
}
fn open_choice(words: &[Word], fallback: Option<Point>) -> Option<Point> {
    if let Some(p) = text_point(words, "open", true) {
        return Some(p);
    }
    let who = text_point(words, "whoareyou", false);
    let leave = text_point(words, "leave", true);
    if let (Some(w), Some(l)) = (who, leave) {
        if l.0 > w.0 {
            return Some(Point(2. * w.0 - l.0, w.1));
        }
    }
    if who.is_some() || leave.is_some() {
        fallback
    } else {
        None
    }
}
fn merchant_layout(words: &[Word], size: (f64, f64)) -> Option<(Value, Vec<String>)> {
    let anchor = text_point(words, "settomax", true)?;
    let title = words
        .iter()
        .find(|w| norm(&w.text).contains("sshop") && center(w).1 < anchor.1);
    let purchase = words
        .iter()
        .find(|w| norm(&w.text).starts_with("purchose") && center(w).1 > anchor.1);
    let k = if let Some(t) = title {
        (anchor.1 - center(t).1) * size.1 / 266.
    } else if let Some(p) = purchase {
        (center(p).1 - anchor.1) * size.1 / 48.
    } else {
        size.0 / 1920.
    };
    if !(0.2..3.).contains(&k) {
        return None;
    }
    let mut out = json!({});
    for (key, x, y) in [
        ("close_pos", 468., -267.),
        ("max_pos", 0., 0.),
        ("purchase_pos", -164., 48.),
        ("first_slot", -376., 106.),
        ("second_slot", -186., 106.),
    ] {
        put_point(&mut out, key, offset(anchor, x, y, k, size));
    }
    let a = offset(anchor, -241., -242., k, size);
    let b = offset(anchor, 469., -210., k, size);
    put_region(&mut out, "item_region", Rect(a.0, a.1, b.0, b.1));
    let mut slots: Vec<Vec<(f64, String)>> = vec![vec![]; 5];
    for word in words {
        let p = center(word);
        let row = anchor.1 * size.1 + 106. * k;
        let x0 = anchor.0 * size.0 - 376. * k;
        if (p.1 * size.1 - row).abs() > 22. * k {
            continue;
        }
        let i = ((p.0 * size.0 - x0) / (190. * k)).round() as isize;
        if (0..5).contains(&i) && (p.0 * size.0 - (x0 + i as f64 * 190. * k)).abs() < 92. * k {
            slots[i as usize].push((p.0, word.text.clone()));
        }
    }
    let labels = slots
        .into_iter()
        .map(|mut words| {
            words.sort_by(|a, b| a.0.total_cmp(&b.0));
            words
                .into_iter()
                .map(|(_, s)| s)
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect();
    Some((out, labels))
}

pub(crate) struct MerchantWatch {
    counts: Option<BTreeMap<String, usize>>,
    lower: BTreeMap<String, usize>,
    seen: BTreeMap<String, f64>,
}
impl MerchantWatch {
    pub(crate) fn new(test: bool) -> Self {
        Self {
            counts: test.then(BTreeMap::new),
            lower: BTreeMap::new(),
            seen: BTreeMap::new(),
        }
    }
    pub(crate) fn check(&mut self, words: &[Word], now: f64) -> Option<String> {
        let text = words_text(words);
        let arrivals = merchant_arrivals(&text);
        if self.counts.is_none() {
            let lines = text.lines().collect::<Vec<_>>();
            let recent = merchant_arrivals(&lines[lines.len().saturating_sub(4)..].join("\n"));
            self.counts = Some(
                arrivals
                    .iter()
                    .map(|(n, c)| (n.clone(), c - recent.get(n).copied().unwrap_or(0)))
                    .collect(),
            );
        }
        let counts = self.counts.as_mut().unwrap();
        let found = arrivals
            .iter()
            .filter(|(name, count)| {
                **count > counts.get(*name).copied().unwrap_or(0)
                    && self.seen.get(*name).is_none_or(|at| now - at >= 600.)
            })
            .min_by_key(|(name, _)| {
                norm(&text)
                    .find(match name.as_str() {
                        "Mari" => "morlhosorrlved",
                        "Jester" => "jesterhosorrlved",
                        _ => "rlnhosorrlved",
                    })
                    .unwrap_or(usize::MAX)
            })
            .map(|(name, _)| name.clone());
        let names: BTreeSet<_> = arrivals.keys().chain(counts.keys()).cloned().collect();
        for name in names {
            let count = arrivals.get(&name).copied().unwrap_or(0);
            if count >= counts.get(&name).copied().unwrap_or(0) {
                counts.insert(name.clone(), count);
                self.lower.insert(name, 0);
            } else {
                let lower = self.lower.entry(name.clone()).or_default();
                *lower += 1;
                if *lower >= 2 {
                    counts.insert(name, count);
                    *lower = 0;
                }
            }
        }
        if let Some(name) = &found {
            self.seen.insert(name.clone(), now);
        }
        found
    }
}
fn merchant_arrivals(text: &str) -> BTreeMap<String, usize> {
    let text = norm(text);
    let mut result = BTreeMap::new();
    for (name, needle) in [
        ("Mari", "morlhosorrlved"),
        ("Jester", "jesterhosorrlved"),
        ("Rin", "rlnhosorrlved"),
    ] {
        let count = text.matches(needle).count();
        if count > 0 {
            result.insert(name.into(), count);
        }
    }
    result
}

enum MerchantStage {
    Teleport,
    WaitDialog,
    Interact,
    Dialog,
    Calibrate,
    Slot,
    ReadItem,
    Purchase,
    Close,
    Reset,
    Finish,
}
pub(crate) struct Merchant {
    cfg: Value,
    base: Value,
    name: String,
    wanted: BTreeSet<String>,
    threshold: f64,
    teleporter: Option<ItemUse>,
    stage: MerchantStage,
    script: Script,
    observing: bool,
    observe_region: Option<Rect>,
    deadline: f64,
    slot: usize,
    labels: Vec<String>,
    bought: u32,
    message: String,
    finished: bool,
    error: Option<String>,
    patch: Option<Value>,
    calibrating: bool,
}
impl Merchant {
    fn new(config: &Value, name: &str, now: f64) -> Result<Self, String> {
        if !["Mari", "Jester", "Rin"].contains(&name) {
            return Err("상인 이름을 확인하세요.".into());
        }
        let cfg = config["mmerch"].clone();
        let base = config["base"].clone();
        if region(&cfg, "chat_region").is_none() {
            return Err("상인 채팅 글자 영역을 지정하세요.".into());
        }
        if point(&base, "dialog_pos").is_none() {
            return Err("대화창 공통 위치를 지정하세요.".into());
        }
        let wanted = cfg["buy"]
            .as_object()
            .into_iter()
            .flat_map(|b| b.keys())
            .filter_map(|key| key.strip_prefix(&format!("{name}_")).map(str::to_owned))
            .collect::<BTreeSet<_>>();
        let threshold = number(&config["pop"], "match_threshold", 70.);
        if !(0.0..=100.).contains(&threshold)
            || !(0.0..=300.).contains(&number(&cfg, "teleport_wait", 3.))
        {
            return Err("상인 대기 시간과 일치율 범위를 확인하세요.".into());
        }
        if !cfg["auto_cal"].as_bool().unwrap_or(true) {
            for key in [
                "first_slot",
                "second_slot",
                "max_pos",
                "purchase_pos",
                "close_pos",
            ] {
                if point(&cfg, key).is_none() {
                    return Err(format!("상점 {key} 위치를 지정하세요."));
                }
            }
            if region(&cfg, "item_region").is_none() {
                return Err("상점 아이템 이름 영역을 지정하세요.".into());
            }
        }
        let teleporter = ItemUse::single(config, "Merchant Teleporter", 1, 1, true, now)?;
        Ok(Self {
            cfg,
            base,
            name: name.into(),
            wanted,
            threshold,
            teleporter: Some(teleporter),
            stage: MerchantStage::Teleport,
            script: Script::default(),
            observing: false,
            observe_region: None,
            deadline: now,
            slot: 0,
            labels: vec![],
            bought: 0,
            message: "Merchant Teleporter 사용".into(),
            finished: false,
            error: None,
            patch: None,
            calibrating: false,
        })
    }
    fn calibrate(config: &Value, now: f64) -> Self {
        Self {
            cfg: config["mmerch"]
                .as_object()
                .map(|v| Value::Object(v.clone()))
                .unwrap_or_else(|| json!({})),
            base: config["base"]
                .as_object()
                .map(|v| Value::Object(v.clone()))
                .unwrap_or_else(|| json!({})),
            name: String::new(),
            wanted: BTreeSet::new(),
            threshold: 70.,
            teleporter: None,
            stage: MerchantStage::WaitDialog,
            script: Script::default(),
            observing: false,
            observe_region: None,
            deadline: now + 120.,
            slot: 0,
            labels: vec![],
            bought: 0,
            message: "상인 앞에서 E 를 눌러 주세요".into(),
            finished: false,
            error: None,
            patch: None,
            calibrating: true,
        }
    }
    fn ack(&mut self, ok: bool, now: f64) {
        if matches!(self.stage, MerchantStage::Teleport) {
            if let Some(teleporter) = self.teleporter.as_mut() {
                teleporter.ack(ok, now);
            }
        } else {
            self.script.ack(ok, now);
        }
    }
    fn finish(&mut self, error: Option<String>) {
        self.error = error;
        if !self.calibrating {
            self.message = "리셋".into();
            self.script.reset();
        }
        self.stage = MerchantStage::Finish;
    }
    fn observe(&mut self, region: Option<Rect>) -> Step {
        self.observing = true;
        self.observe_region = region;
        Step::Observe { region, ocr: true }
    }
    fn tick(&mut self, now: f64, frame: Option<&Frame<'_>>, words: &[Word]) -> Step {
        if self.finished {
            return if let Some(error) = &self.error {
                Step::Failed(error.clone())
            } else {
                Step::Done
            };
        }
        if self.observing && frame.is_none() {
            return Step::Observe {
                region: self.observe_region,
                ocr: true,
            };
        }
        if matches!(self.stage, MerchantStage::Teleport) {
            if self.wanted.is_empty() {
                self.finished = true;
                self.message = format!("{} — 살 아이템이 없습니다.", self.name);
                return Step::Done;
            }
            let teleporter = self.teleporter.as_mut().unwrap();
            match teleporter.tick(now, frame, words) {
                Step::Done => {
                    if teleporter.used() == 0 {
                        self.finished = true;
                        self.error = Some("Merchant Teleporter를 사용하지 못했습니다.".into());
                        return Step::Failed(self.error.clone().unwrap());
                    }
                    self.stage = MerchantStage::Interact;
                    self.script.until = now + number(&self.cfg, "teleport_wait", 3.);
                }
                Step::Failed(error) => {
                    self.finished = true;
                    self.error = Some(error.clone());
                    self.message = error.clone();
                    return Step::Failed(error);
                }
                other => return other,
            }
        }
        if let Some(step) = self.script.tick(now) {
            return step;
        }
        loop {
            match self.stage {
                MerchantStage::Teleport => unreachable!(),
                MerchantStage::WaitDialog => {
                    if !self.observing {
                        return self.observe(Some(Rect(0., 0.4, 1., 1.)));
                    }
                    self.observing = false;
                    let skip = text_point(words, "clicktoskip", false)
                        .or_else(|| text_point(words, "toskip", false));
                    if open_choice(words, None).is_some() || skip.is_some() {
                        if point(&self.base, "dialog_pos").is_none() {
                            if let Some(p) = skip {
                                put_point(&mut self.base, "dialog_pos", p);
                            }
                        }
                        self.message = "대화창 찾음 — 이제 만지지 마세요".into();
                        self.deadline = now + 12.;
                        self.stage = MerchantStage::Dialog;
                        continue;
                    }
                    if now >= self.deadline {
                        self.finish(Some(
                            "대화창이 안 보입니다. 상인 앞에서 E 를 눌러 주세요.".into(),
                        ));
                        continue;
                    }
                    self.script.until = now + 0.4;
                    return Step::Wait(0.4);
                }
                MerchantStage::Interact => {
                    self.message = format!("{} 대화 (E)", self.name);
                    for _ in 0..5 {
                        self.script.key(&[0x45], 0.2);
                    }
                    self.deadline = 0.;
                    self.stage = MerchantStage::Dialog;
                    return self.script.tick(now).unwrap();
                }
                MerchantStage::Dialog => {
                    if self.deadline == 0. {
                        self.deadline = now + 12.;
                    }
                    if !self.observing {
                        return self.observe(Some(Rect(0., 0.4, 1., 1.)));
                    }
                    self.observing = false;
                    let fallback = if self.calibrating {
                        None
                    } else {
                        point(&self.cfg, "open_pos")
                    };
                    let choice = open_choice(words, fallback)
                        .or_else(|| (now >= self.deadline).then_some(fallback).flatten());
                    if let Some(p) = choice {
                        if self.calibrating {
                            let patch = self.patch.get_or_insert_with(|| json!({}));
                            put_point(patch, "open_pos", p);
                        }
                        self.message = "상점 여는 중".into();
                        self.script.until = now + 0.15;
                        self.script.tap(p, if self.calibrating { 0.6 } else { 1. });
                        self.stage = MerchantStage::Calibrate;
                        self.deadline = 0.;
                        return self.script.tick(now).unwrap();
                    }
                    if now >= self.deadline {
                        self.finish(Some("상점 Open 선택지를 찾지 못했습니다.".into()));
                        continue;
                    }
                    let Some(dialog) = point(&self.base, "dialog_pos") else {
                        self.finish(Some("대화창 클릭 위치를 찾지 못했습니다.".into()));
                        continue;
                    };
                    self.script.tap(dialog, 0.35);
                    return self.script.tick(now).unwrap();
                }
                MerchantStage::Calibrate => {
                    self.message = "상점 위치 자동 보정".into();
                    if !self.calibrating && !self.cfg["auto_cal"].as_bool().unwrap_or(true) {
                        self.stage = MerchantStage::Slot;
                        continue;
                    }
                    if self.deadline == 0. {
                        self.deadline = now + if self.calibrating { 6. } else { 5. };
                    }
                    if !self.observing {
                        return self.observe(None);
                    }
                    self.observing = false;
                    if let Some((layout, labels)) =
                        dims(frame).and_then(|size| merchant_layout(words, size))
                    {
                        self.cfg
                            .as_object_mut()
                            .unwrap()
                            .extend(layout.as_object().unwrap().clone());
                        let patch = self.patch.get_or_insert_with(|| json!({}));
                        patch
                            .as_object_mut()
                            .unwrap()
                            .extend(layout.as_object().unwrap().clone());
                        if self.calibrating {
                            patch["cal_shop"] = true.into();
                        }
                        self.labels = labels;
                        self.stage = if self.calibrating {
                            MerchantStage::Close
                        } else {
                            MerchantStage::Slot
                        };
                        continue;
                    }
                    if now >= self.deadline {
                        if !self.calibrating
                            && [
                                "first_slot",
                                "second_slot",
                                "max_pos",
                                "purchase_pos",
                                "close_pos",
                            ]
                            .iter()
                            .all(|k| point(&self.cfg, k).is_some())
                            && region(&self.cfg, "item_region").is_some()
                        {
                            self.stage = MerchantStage::Slot;
                            continue;
                        }
                        self.finish(Some(
                            "Set to Max 버튼을 찾지 못했습니다. 상점 위치를 지정하세요.".into(),
                        ));
                        continue;
                    }
                    self.script.until = now + 0.4;
                    return Step::Wait(0.4);
                }
                MerchantStage::Slot => {
                    if self.slot >= 5 {
                        self.stage = MerchantStage::Close;
                        continue;
                    }
                    if let Some(label) = self.labels.get(self.slot).filter(|s| !s.is_empty()) {
                        if !match_merchant(label, &self.wanted).is_some_and(|(name, score)| {
                            self.wanted.contains(&name) && score >= self.threshold
                        }) {
                            self.slot += 1;
                            continue;
                        }
                    }
                    let (Some(a), Some(b)) = (
                        point(&self.cfg, "first_slot"),
                        point(&self.cfg, "second_slot"),
                    ) else {
                        self.finish(Some("상점 칸 위치가 없습니다.".into()));
                        continue;
                    };
                    let pos = Point(
                        a.0 + (b.0 - a.0) * self.slot as f64,
                        a.1 + (b.1 - a.1) * self.slot as f64,
                    );
                    if [pos.0, pos.1].iter().any(|v| !(0.0..=1.).contains(v)) {
                        self.finish(Some("상점 칸 위치가 화면을 벗어났습니다.".into()));
                        continue;
                    }
                    self.message = format!("상점 {}번 칸 확인", self.slot + 1);
                    self.script.tap(pos, 0.5);
                    self.stage = MerchantStage::ReadItem;
                    return self.script.tick(now).unwrap();
                }
                MerchantStage::ReadItem => {
                    if !self.observing {
                        return self.observe(region(&self.cfg, "item_region"));
                    }
                    self.observing = false;
                    let item = merchant_item(&words_text(words));
                    if let Some((name, score)) = match_merchant(&item, &self.wanted)
                        .filter(|(n, s)| self.wanted.contains(n) && *s >= self.threshold)
                    {
                        self.message = format!("{name} 구매 ({score:.1}%)");
                        self.script.tap(point(&self.cfg, "max_pos").unwrap(), 0.3);
                        self.script
                            .tap(point(&self.cfg, "purchase_pos").unwrap(), 0.);
                        self.deadline = 0.;
                        self.stage = MerchantStage::Purchase;
                        return self.script.tick(now).unwrap();
                    }
                    self.slot += 1;
                    self.stage = MerchantStage::Slot;
                }
                MerchantStage::Purchase => {
                    if self.deadline == 0. {
                        self.deadline = now + 3.3;
                    }
                    if now >= self.deadline {
                        self.bought += 1;
                        self.slot += 1;
                        self.stage = MerchantStage::Slot;
                        continue;
                    }
                    self.script
                        .tap(point(&self.base, "dialog_pos").unwrap(), 0.1);
                    return self.script.tick(now).unwrap();
                }
                MerchantStage::Close => {
                    self.message = "상점 닫기".into();
                    self.script.tap(
                        point(&self.cfg, "close_pos").unwrap(),
                        if self.calibrating { 0.6 } else { 0.8 },
                    );
                    self.stage = if self.calibrating {
                        MerchantStage::Finish
                    } else {
                        MerchantStage::Reset
                    };
                    return self.script.tick(now).unwrap();
                }
                MerchantStage::Reset => {
                    self.finish(None);
                }
                MerchantStage::Finish => {
                    self.finished = true;
                    self.message = self.error.clone().unwrap_or_else(|| "대기".into());
                    return if let Some(error) = &self.error {
                        Step::Failed(error.clone())
                    } else {
                        Step::Done
                    };
                }
            }
            if let Some(step) = self.script.tick(now) {
                return step;
            }
        }
    }
}

const CRAFT_WORDS: &[&str] = &[
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
const ROMAN: &[&str] = &["I", "II", "III", "IV", "V"];
pub(crate) fn typed_name(text: &str) -> String {
    let mut words = text
        .split_whitespace()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let len = words.len();
    if len > 1 {
        let last = &mut words[len - 1];
        if let Ok(n) = last.parse::<usize>() {
            if (1..=5).contains(&n) {
                *last = ROMAN[n - 1].into();
            }
        } else if ROMAN.contains(&last.to_ascii_uppercase().as_str()) {
            *last = last.to_ascii_uppercase();
        }
    }
    words
        .into_iter()
        .enumerate()
        .map(|(i, w)| {
            if i + 1 == len && ROMAN.contains(&w.as_str()) {
                return w;
            }
            CRAFT_WORDS
                .iter()
                .find(|known| known.len() > 2 && known.eq_ignore_ascii_case(&w))
                .map_or(w.clone(), |s| (*s).into())
        })
        .collect::<Vec<_>>()
        .join(" ")
}
fn fixed_name(text: &str) -> String {
    text.split_whitespace()
        .map(|word| {
            let n = norm(word);
            if !n.is_empty() && n.len() <= 3 && n.chars().all(|c| c == 'l') {
                "I".repeat(n.len())
            } else {
                CRAFT_WORDS
                    .iter()
                    .find(|known| norm(known) == n)
                    .map_or_else(|| word.to_owned(), |s| (*s).into())
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}
pub(crate) fn crafted_name(text: &str) -> Option<String> {
    static RE: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
        regex::Regex::new(r"(?i)y[o0]ur\s*([^\n]+?)\s*w[ao]s\s*[ao]ut[ao]m[ao]t[il1][ck][ao][il1]{1,2}y\s*cr[ao]fted").unwrap()
    });
    let captures = RE.captures(text)?;
    let raw = captures.get(1)?.as_str().trim_matches([' ', '.', '!']);
    let mut name = String::new();
    let mut lower = false;
    for ch in raw.chars() {
        if !raw.contains(' ') && lower && ch.is_ascii_uppercase() {
            name.push(' ');
        }
        name.push(ch);
        lower = ch.is_ascii_lowercase();
    }
    let name = fixed_name(&name);
    (!name.is_empty()).then_some(name)
}

/// Main checks the blue Auto Crafted notification before reading its name;
/// Auto Added notifications must never start a crafting trip.
pub(crate) fn craft_notice(config: &Value, frame: &Frame<'_>, words: &[Word]) -> Option<String> {
    let rect = region(&config["base"], "notice_region")?;
    let (x0, y0) = frame.position(Point(rect.0, rect.1));
    let (x1, y1) = frame.position(Point(rect.2, rect.3));
    let mut blue = 0usize;
    let mut total = 0usize;
    for y in y0..y1 {
        for x in x0..x1 {
            let [r, g, b] = frame.pixel(x, y).map(i16::from);
            blue += usize::from(b > 200 && b - r > 60 && g > 130);
            total += 1;
        }
    }
    if total == 0 || blue as f64 / (total as f64) < 0.006 {
        return None;
    }
    let text = words_text(words);
    if !norm(&text).contains("crofted") {
        return None;
    }
    let typed = typed_name(config["mcraft"]["potion"].as_str().unwrap_or(""));
    if typed.is_empty() {
        crafted_name(&text)
    } else {
        Some(typed)
    }
}
fn split_roman(name: &str) -> (String, usize) {
    let words = name.split_whitespace().collect::<Vec<_>>();
    if words.len() > 1 {
        if let Some(i) = ROMAN.iter().position(|n| Some(n) == words.last()) {
            return (words[..words.len() - 1].join(" "), i + 1);
        }
    }
    (name.into(), 0)
}
fn safe_query(name: &str) -> String {
    let part = name
        .split(['a', 'o', 'i', 'l', 'A', 'O', 'I', 'L'])
        .max_by_key(|s| s.len())
        .unwrap_or("")
        .trim();
    if part.len() >= 3 {
        part.into()
    } else {
        name.into()
    }
}
fn same_name(a: &str, b: &str) -> bool {
    let (a, b) = (norm(a), norm(b));
    !a.is_empty() && (a == b || (a.len().abs_diff(b.len()) <= 1 && sequence_ratio(&a, &b) >= 0.9))
}
fn pick_card(words: &[Word], name: &str) -> Option<Point> {
    let (base, n) = split_roman(name);
    if n == 0 {
        return words.iter().find(|w| same_name(&w.text, name)).map(center);
    }
    let base = norm(&base);
    let mut cards = words
        .iter()
        .filter(|w| {
            let t = norm(&w.text);
            t.starts_with(&base) && t.len() <= base.len() + 4
        })
        .collect::<Vec<_>>();
    cards.sort_by(|a, b| center(a).1.total_cmp(&center(b).1));
    cards.get(n - 1).map(|w| center(w))
}

#[cfg(test)]
mod pc_activity_tests {
    use super::*;
    fn word(text: &str, x: f64, y: f64) -> Word {
        Word {
            text: text.into(),
            rect: Rect(x - 0.03, y - 0.01, x + 0.03, y + 0.01),
        }
    }
    #[test]
    fn pc_shop_matching_checks_known_unwanted_items_and_layout_scale() {
        let wanted = BTreeSet::from(["Lucky Potion".into(), "Gear A".into()]);
        assert_eq!(
            match_merchant("Lucky Penny", &wanted).unwrap().0,
            "Lucky Penny"
        );
        assert_eq!(match_merchant("Gear B", &wanted).unwrap().0, "Gear B");
        assert_eq!(merchant_item("Mixed Potion | Common"), "Mixed Potion");
        let words = [
            word("Set to Max", 1341. / 1920., 614. / 1080.),
            word("Mari's Shop", 0.5, 348. / 1080.),
        ];
        let (layout, _) = merchant_layout(&words, (1920., 1080.)).unwrap();
        let first = point(&layout, "first_slot").unwrap();
        assert!((first.0 - 965. / 1920.).abs() < 1e-9);
        assert!((first.1 - 720. / 1080.).abs() < 1e-9);
    }
    #[test]
    fn pc_craft_name_and_roman_card_selection_match_main() {
        assert_eq!(typed_name(" fortune potion 2 "), "Fortune Potion II");
        assert_eq!(
            crafted_name("Your Heovenly Potion ll was automatically crafted!"),
            Some("Heavenly Potion II".into())
        );
        assert_eq!(crafted_name("Potion Auto Added"), None);
        let cards = [
            word("Fortune Potion lll", 0.5, 0.6),
            word("Fortune Potion l", 0.5, 0.4),
            word("Fortune Potion ll", 0.5, 0.5),
        ];
        assert!((pick_card(&cards, "Fortune Potion II").unwrap().1 - 0.5).abs() < 1e-9);
        assert!(!same_name("Super Warp Potion", "Warp Potion"));
    }
    #[test]
    fn pc_script_delays_start_after_ack_and_failures_stop_the_sequence() {
        let mut script = Script::default();
        script.key(&[0x45], 0.5);
        script.key(&[0x46], 0.2);
        assert!(matches!(script.tick(1.), Some(Step::Input(_))));
        assert!(matches!(script.tick(10.), Some(Step::Wait(_))));
        script.ack(true, 10.);
        assert!(matches!(script.tick(10.4), Some(Step::Wait(_))));
        assert!(matches!(script.tick(10.5), Some(Step::Input(_))));
        script.ack(false, 11.);
        assert!(matches!(script.tick(20.), Some(Step::Failed(_))));
        assert!(script.actions.is_empty());
    }
    #[test]
    fn pc_merchant_chat_counts_debounce_missing_lines_and_enforce_cooldown() {
        let line = [word("[Merchant]: Mari has arrived", 0.3, 0.2)];
        let mut watch = MerchantWatch::new(false);
        assert_eq!(watch.check(&line, 100.), Some("Mari".into()));
        assert_eq!(watch.check(&line, 200.), None);
        assert_eq!(watch.check(&[], 800.), None);
        assert_eq!(watch.check(&line, 801.), None);
        watch.check(&[], 802.);
        watch.check(&[], 803.);
        assert_eq!(watch.check(&line, 804.), Some("Mari".into()));
    }
    #[test]
    fn pc_craft_executes_fill_craft_refill_then_reset() {
        let config = json!({"mcraft":{"potion":"Fortune Potion II","f_wait":2.5,"search_pos":[0.4,0.2],"list_region":[0.2,0.3,0.8,0.9],"shop_close_pos":[0.8,0.1],"open_recipe_pos":[0.8,0.6],"add_all_pos":[0.4,0.7],"craft_pos":[0.6,0.7],"add_close_pos":[0.8,0.2]},"move":{"places":[{"feat":"mcraft","key":"craft_spot","route":{"ev":[[0.,"w",1],[1.,"w",0]],"len":1.}}]}});
        let mut craft = Crafter::new(&config, "Fortune Potion II", 0.).unwrap();
        let pixels = vec![0; 100 * 100 * 3];
        let frame = Frame::new(&pixels, 100, 100, 300, 3, false).unwrap();
        let results = [
            word("Fortune Potion I", 0.5, 0.4),
            word("Fortune Potion II", 0.5, 0.5),
        ];
        let mut now = 0.;
        let mut observation = false;
        let mut taps = vec![];
        let mut keys = vec![];
        let mut searches = vec![];
        for _ in 0..100 {
            let step = craft.tick(
                now,
                observation.then_some(&frame),
                if observation { &results } else { &[] },
            );
            observation = false;
            match step {
                Step::Travel { place } => {
                    assert_eq!(place, 0);
                    craft.script.ack(true, now);
                }
                Step::Observe { .. } => observation = true,
                Step::Input(Motion::Tap(p, _)) => {
                    taps.push((p.0, p.1));
                    craft.script.ack(true, now);
                }
                Step::Input(Motion::Keys(k, _)) => {
                    keys.push(k.to_vec());
                    craft.script.ack(true, now);
                }
                Step::Text { value, submit } => {
                    assert!(submit);
                    searches.push(value);
                    craft.script.ack(true, now);
                }
                Step::Wait(delay) => now += delay.max(0.001),
                Step::Done => break,
                Step::Failed(error) => panic!("{error}"),
                _ => panic!("unexpected craft action"),
            }
        }
        assert!(craft.finished);
        assert_eq!(craft.crafted, 1);
        assert_eq!(searches, vec!["Fortune Potion"]);
        assert_eq!(
            &taps[taps.len() - 3..],
            &[(0.4, 0.7), (0.6, 0.7), (0.4, 0.7)]
        );
        assert_eq!(
            &keys[keys.len() - 3..],
            &[vec![0x1b], vec![0x52], vec![0x0d]]
        );
    }
    #[test]
    fn pc_craft_notification_requires_blue_crafted_not_auto_added() {
        let config =
            json!({"base":{"notice_region":[0.,0.,1.,1.]},"mcraft":{"potion":"Fortune Potion I"}});
        let blue = [80, 180, 240].repeat(100);
        let frame = Frame::new(&blue, 10, 10, 30, 3, false).unwrap();
        assert!(craft_notice(&config, &frame, &[word("Auto Added", 0.5, 0.5)]).is_none());
        assert_eq!(
            craft_notice(&config, &frame, &[word("Auto Crafted", 0.5, 0.5)]),
            Some("Fortune Potion I".into())
        );
        let black = [0, 0, 0].repeat(100);
        let frame = Frame::new(&black, 10, 10, 30, 3, false).unwrap();
        assert!(craft_notice(&config, &frame, &[word("Auto Crafted", 0.5, 0.5)]).is_none());
    }
}
fn craft_shop_layout(words: &[Word], size: (f64, f64)) -> Option<Value> {
    let title = words.iter().find(|w| norm(&w.text).contains("workshop"))?;
    let t = center(title);
    let item = words.iter().find(|w| norm(&w.text) == "ltem");
    let lantern = words.iter().find(|w| norm(&w.text).starts_with("lontern"));
    let k = if let (Some(i), Some(l)) = (item, lantern) {
        (center(l).0 - center(i).0).abs() * size.0 / 239.
    } else {
        size.1 / 1080.
    };
    if k <= 0. || !k.is_finite() {
        return None;
    }
    let mut layout = json!({});
    put_point(&mut layout, "search_pos", offset(t, 7., 83., k, size));
    put_point(&mut layout, "shop_close_pos", offset(t, 217., -2., k, size));
    let a = offset(t, -225., 110., k, size);
    let b = offset(t, 225., 735., k, size);
    put_region(&mut layout, "list_region", Rect(a.0, a.1, b.0, b.1));
    Some(layout)
}
fn craft_add_layout(words: &[Word], size: (f64, f64)) -> Option<Value> {
    let title = words
        .iter()
        .find(|w| norm(&w.text).contains("ngredlents"))?;
    let add = words
        .iter()
        .find(|w| norm(&w.text).contains("everythlng"))?;
    let t = center(title);
    let a = center(add);
    let k = if a.1 > t.1 {
        (a.1 - t.1) * size.1 / 295.
    } else {
        size.1 / 1080.
    };
    let mut layout = json!({});
    put_point(&mut layout, "add_all_pos", a);
    put_point(&mut layout, "craft_pos", offset(a, 240., 0., k, size));
    put_point(&mut layout, "add_close_pos", offset(t, 226., -2., k, size));
    Some(layout)
}

enum CraftStage {
    Travel,
    CalShop,
    CalCard,
    CalClose,
    Open,
    Search,
    Results,
    Recipe,
    Add,
    Fill,
    Reset,
    Finish,
}
pub(crate) struct Crafter {
    cfg: Value,
    name: String,
    place: usize,
    stage: CraftStage,
    script: Script,
    observing: bool,
    observe_region: Option<Rect>,
    deadline: f64,
    attempt: u8,
    query_attempt: u8,
    calibrated: bool,
    message: String,
    finished: bool,
    error: Option<String>,
    crafted: u32,
    patch: Option<Value>,
    calibrating: bool,
}
impl Crafter {
    fn new(config: &Value, name: &str, now: f64) -> Result<Self, String> {
        let name = typed_name(name);
        if name.is_empty() || name.chars().count() > 80 || name.chars().any(char::is_control) {
            return Err("제작할 포션을 입력하세요.".into());
        }
        let place = place(config, "mcraft", "craft_spot")
            .ok_or("포션 제작 장소의 길을 먼저 기록하세요.")?;
        let cfg = config["mcraft"]
            .as_object()
            .map(|v| Value::Object(v.clone()))
            .unwrap_or_else(|| json!({}));
        if !(0.5..=300.).contains(&number(&cfg, "f_wait", 2.5)) {
            return Err("제작 창 대기 시간을 확인하세요.".into());
        }
        let calibrated = point(&cfg, "search_pos").is_some()
            && point(&cfg, "shop_close_pos").is_some()
            && region(&cfg, "list_region").is_some();
        Ok(Self {
            cfg,
            name,
            place,
            stage: CraftStage::Travel,
            script: Script {
                until: now,
                ..Script::default()
            },
            observing: false,
            observe_region: None,
            deadline: 0.,
            attempt: 0,
            query_attempt: 0,
            calibrated,
            message: "포션 제작 장소로 가는 중".into(),
            finished: false,
            error: None,
            crafted: 0,
            patch: None,
            calibrating: false,
        })
    }
    fn calibrate(config: &Value, now: f64) -> Self {
        Self {
            cfg: config["mcraft"]
                .as_object()
                .map(|v| Value::Object(v.clone()))
                .unwrap_or_else(|| json!({})),
            name: String::new(),
            place: 0,
            stage: CraftStage::CalShop,
            script: Script::default(),
            observing: false,
            observe_region: None,
            deadline: now + 120.,
            attempt: 0,
            query_attempt: 0,
            calibrated: false,
            message: "Stella 앞에서 F 를 눌러 주세요".into(),
            finished: false,
            error: None,
            crafted: 0,
            patch: None,
            calibrating: true,
        }
    }
    fn observe(&mut self, region: Option<Rect>) -> Step {
        self.observing = true;
        self.observe_region = region;
        Step::Observe { region, ocr: true }
    }
    fn fail(&mut self, message: &str) {
        self.error = Some(message.into());
        self.stage = if self.calibrating {
            CraftStage::Finish
        } else {
            CraftStage::Reset
        };
    }
    fn merge(&mut self, layout: Value) {
        self.cfg
            .as_object_mut()
            .unwrap()
            .extend(layout.as_object().unwrap().clone());
        if self.calibrating {
            let patch = self.patch.get_or_insert_with(|| json!({}));
            patch
                .as_object_mut()
                .unwrap()
                .extend(layout.as_object().unwrap().clone());
        }
    }
    fn tick(&mut self, now: f64, frame: Option<&Frame<'_>>, words: &[Word]) -> Step {
        if self.finished {
            return if let Some(error) = &self.error {
                Step::Failed(error.clone())
            } else {
                Step::Done
            };
        }
        if self.observing && frame.is_none() {
            return Step::Observe {
                region: self.observe_region,
                ocr: true,
            };
        }
        if let Some(step) = self.script.tick(now) {
            return step;
        }
        loop {
            match self.stage {
                CraftStage::Travel => {
                    self.script
                        .actions
                        .push_back((Step::Travel { place: self.place }, 0.));
                    self.stage = CraftStage::Open;
                    self.deadline = 0.;
                    return self.script.tick(now).unwrap();
                }
                CraftStage::CalShop => {
                    if !self.observing {
                        return self.observe(None);
                    }
                    self.observing = false;
                    if let Some(layout) =
                        dims(frame).and_then(|size| craft_shop_layout(words, size))
                    {
                        self.merge(layout);
                        self.message = "제작 창 찾음 — 이제 만지지 마세요".into();
                        self.script.until = now + 0.5;
                        self.stage = CraftStage::CalCard;
                        return Step::Wait(0.5);
                    }
                    if now >= self.deadline {
                        self.fail("제작 창이 안 보입니다. Stella 앞에서 F 를 눌러 주세요.");
                        continue;
                    }
                    self.script.until = now + 0.4;
                    return Step::Wait(0.4);
                }
                CraftStage::CalCard => {
                    if !self.observing {
                        return self.observe(region(&self.cfg, "list_region"));
                    }
                    self.observing = false;
                    let first = words
                        .iter()
                        .filter(|w| norm(&w.text).len() >= 6)
                        .min_by(|a, b| center(a).1.total_cmp(&center(b).1))
                        .map(center);
                    if let Some(first) = first {
                        self.script.tap(first, 0.6);
                        self.stage = CraftStage::Recipe;
                        return self.script.tick(now).unwrap();
                    }
                    self.fail("제작 목록이 비어 있습니다.");
                }
                CraftStage::CalClose => {
                    self.message = "창 닫는 중".into();
                    self.script
                        .tap(point(&self.cfg, "add_close_pos").unwrap(), 0.4);
                    self.script
                        .tap(point(&self.cfg, "shop_close_pos").unwrap(), 0.3);
                    self.patch.get_or_insert_with(|| json!({}))["cal_shop"] = true.into();
                    self.stage = CraftStage::Finish;
                    return self.script.tick(now).unwrap();
                }
                CraftStage::Open => {
                    self.message = "F (Stella's Workshop)".into();
                    if self.deadline == 0. {
                        self.script.key(&[0x46], 0.);
                        self.attempt += 1;
                        self.deadline = -1.;
                        return self.script.tick(now).unwrap();
                    }
                    if self.deadline < 0. {
                        self.deadline = now
                            + if self.attempt == 1 {
                                number(&self.cfg, "f_wait", 2.5)
                            } else if self.calibrated {
                                2.5
                            } else {
                                3.
                            };
                    }
                    if !self.observing {
                        return self.observe(if self.calibrated && self.attempt <= 2 {
                            region(&self.cfg, "list_region")
                        } else {
                            None
                        });
                    }
                    self.observing = false;
                    let layout = dims(frame).and_then(|size| craft_shop_layout(words, size));
                    if self.calibrated
                        && self.attempt <= 2
                        && words.iter().any(|w| norm(&w.text).len() >= 4)
                    {
                        self.stage = CraftStage::Search;
                        continue;
                    }
                    if let Some(layout) = layout {
                        self.merge(layout);
                        self.stage = CraftStage::Search;
                        continue;
                    }
                    if now >= self.deadline {
                        if self.attempt == 1 {
                            self.deadline = 0.;
                            continue;
                        }
                        if self.calibrated && self.attempt == 2 {
                            self.attempt = 3;
                            self.deadline = now + 1.;
                            continue;
                        }
                        self.fail("제작 창(Stella's Workshop)이 열리지 않았습니다. 제작 장소를 확인하세요.");
                        continue;
                    }
                    self.script.until = now
                        + if self.calibrated && self.attempt <= 2 {
                            0.25
                        } else {
                            0.4
                        };
                    return Step::Wait(self.script.until - now);
                }
                CraftStage::Search => {
                    let Some(search) = point(&self.cfg, "search_pos") else {
                        self.fail("제작 검색창을 찾지 못했습니다.");
                        continue;
                    };
                    let (base, _) = split_roman(&self.name);
                    let query = if self.query_attempt == 0 {
                        base
                    } else {
                        safe_query(&base)
                    };
                    self.message = format!("{} 검색", self.name);
                    self.script.tap(search, 0.15);
                    self.script.key(&[0x11, 0x41], 0.05);
                    self.script.text(query, true, 0.3);
                    self.deadline = 0.;
                    self.stage = CraftStage::Results;
                    return self.script.tick(now).unwrap();
                }
                CraftStage::Results => {
                    if self.deadline == 0. {
                        self.deadline = now + 2.;
                    }
                    if !self.observing {
                        return self.observe(region(&self.cfg, "list_region"));
                    }
                    self.observing = false;
                    if let Some(card) = pick_card(words, &self.name) {
                        self.script.tap(card, 0.35);
                        self.stage = CraftStage::Recipe;
                        return self.script.tick(now).unwrap();
                    }
                    if now >= self.deadline {
                        let (base, _) = split_roman(&self.name);
                        if self.query_attempt == 0 && safe_query(&base) != base {
                            self.query_attempt = 1;
                            self.stage = CraftStage::Search;
                            continue;
                        }
                        if let Some(close) = point(&self.cfg, "shop_close_pos") {
                            self.script.tap(close, 0.);
                        }
                        self.fail("제작 목록에서 포션을 찾지 못했습니다.");
                        continue;
                    }
                    self.script.until = now + 0.3;
                    return Step::Wait(0.3);
                }
                CraftStage::Recipe => {
                    self.message = "Open Recipe".into();
                    let saved = if self.calibrating {
                        None
                    } else {
                        point(&self.cfg, "open_recipe_pos")
                    };
                    if saved.is_none() && !self.observing {
                        return self.observe(None);
                    }
                    self.observing = false;
                    let found = saved.or_else(|| text_point(words, "penrecipe", false));
                    if let Some(p) = found {
                        if self.calibrating {
                            self.merge(json!({"open_recipe_pos":[p.0,p.1]}));
                        }
                        self.script.tap(p, 0.5);
                        self.stage = CraftStage::Add;
                        self.deadline = 0.;
                        return self.script.tick(now).unwrap();
                    }
                    self.fail("Open Recipe 버튼을 찾지 못했습니다.");
                }
                CraftStage::Add => {
                    if !self.calibrating
                        && ["add_all_pos", "craft_pos", "add_close_pos"]
                            .iter()
                            .all(|key| point(&self.cfg, key).is_some())
                    {
                        self.stage = CraftStage::Fill;
                        continue;
                    }
                    if self.deadline == 0. {
                        self.deadline = now + if self.calibrating { 4. } else { 3. };
                    }
                    if !self.observing {
                        return self.observe(None);
                    }
                    self.observing = false;
                    if let Some(layout) = dims(frame).and_then(|size| craft_add_layout(words, size))
                    {
                        self.merge(layout);
                        self.stage = if self.calibrating {
                            CraftStage::CalClose
                        } else {
                            CraftStage::Fill
                        };
                        continue;
                    }
                    if now >= self.deadline {
                        self.fail("Add Ingredients 창을 찾지 못했습니다.");
                        continue;
                    }
                    self.script.until = now + 0.3;
                    return Step::Wait(0.3);
                }
                CraftStage::Fill => {
                    self.message = "Add Everything → Craft → Add Everything".into();
                    self.script
                        .tap(point(&self.cfg, "add_all_pos").unwrap(), 0.25);
                    self.script
                        .tap(point(&self.cfg, "craft_pos").unwrap(), 0.25);
                    self.script
                        .tap(point(&self.cfg, "add_all_pos").unwrap(), 0.4);
                    self.stage = CraftStage::Reset;
                    return self.script.tick(now).unwrap();
                }
                CraftStage::Reset => {
                    if self.error.is_none() {
                        self.crafted += 1;
                    }
                    self.message = "리셋".into();
                    self.script.reset();
                    self.stage = CraftStage::Finish;
                    return self.script.tick(now).unwrap();
                }
                CraftStage::Finish => {
                    self.finished = true;
                    self.message = self.error.clone().unwrap_or_else(|| "대기".into());
                    return if let Some(error) = &self.error {
                        Step::Failed(error.clone())
                    } else {
                        Step::Done
                    };
                }
            }
            if let Some(step) = self.script.tick(now) {
                return step;
            }
        }
    }
}

mod memory {
    use super::*;

    #[derive(Clone)]
    struct Board {
        cards: Vec<Point>,
        boxes: Vec<Rect>,
        texts: Vec<Rect>,
        close: Point,
        title: Point,
        k: f64,
        chances: Rect,
    }

    #[derive(Clone)]
    struct Notice {
        start: bool,
        seconds: Option<f64>,
        button: Point,
        close: Point,
        ad: Option<Point>,
        head: Option<Point>,
        has_head: bool,
        scale: f64,
    }

    #[derive(Clone)]
    struct Signature {
        small: [f64; 300],
        mean: [f64; 3],
    }

    #[derive(Clone)]
    struct Card {
        mean: Option<[f64; 3]>,
        fraction: f64,
        amount: String,
    }

    #[derive(Clone)]
    enum State {
        Travel(usize),
        Open {
            attempt: u8,
            refresh: bool,
        },
        Notice {
            attempt: u8,
            refresh: bool,
            full: bool,
            deadline: f64,
        },
        SettleNotice {
            refresh: bool,
            full: bool,
        },
        Decide,
        Start,
        StartRetry {
            attempt: u8,
        },
        FindBoard {
            attempt: u8,
            deadline: f64,
            extended: bool,
        },
        Ready {
            deadline: f64,
        },
        Check,
        Chances,
        Choose,
        RevealClick {
            index: usize,
            first: bool,
            retry: bool,
        },
        Reveal {
            index: usize,
            first: bool,
        },
        Amount {
            index: usize,
            first: bool,
            card: Card,
        },
        Settle {
            deadline: f64,
        },
        CloseFind {
            deadline: f64,
        },
        CloseClick {
            attempt: u8,
        },
        CloseVerify {
            attempt: u8,
        },
        Refresh {
            deadline: f64,
        },
        AdClick,
        AdStart {
            deadline: f64,
        },
        AdWatch {
            started: f64,
        },
        AdClose {
            attempt: u8,
        },
        FinishNotice,
        CalNotice {
            deadline: f64,
            settled: bool,
        },
        CalCloseVerify {
            attempt: u8,
        },
        CalBoard,
        Reset(u8),
        Done,
        Failed(String),
    }

    pub(super) struct Memory {
        cfg: Value,
        here: bool,
        state: State,
        pending: Option<(State, f64)>,
        due: f64,
        observing: bool,
        notice: Option<Notice>,
        board: Option<Board>,
        estimate: Option<Board>,
        hidden: Vec<Signature>,
        known: Vec<Option<Card>>,
        done: [bool; 20],
        bad: BTreeSet<(usize, usize)>,
        first: Option<usize>,
        second: Option<usize>,
        previous: Option<Signature>,
        reveal_at: f64,
        reveal_presses: u8,
        flipped: bool,
        stuck: u8,
        presses: usize,
        chances: i32,
        zero: u8,
        board_started: f64,
        round_started: f64,
        rounds: usize,
        ads_attempted: usize,
        ad_fail: usize,
        ad_before: Option<f64>,
        ad_pending: bool,
        ad_ended: usize,
        ad_close: Point,
        close: Point,
        played: usize,
        ads: usize,
        next_at: f64,
        clock: f64,
        seed: u64,
        msg: String,
        patch: Option<Value>,
    }

    impl Memory {
        pub(super) fn new(config: &Value, here: bool, now: f64) -> Result<Self, String> {
            let cfg = config.get("mmatch").cloned().unwrap_or_else(|| json!({}));
            for key in ["e_wait", "flip_wait"] {
                if let Some(value) = cfg.get(key) {
                    let n = value
                        .as_f64()
                        .ok_or_else(|| format!("메모리 매치 {key} 숫자를 확인하세요."))?;
                    if !n.is_finite() || n < if key == "e_wait" { 0.5 } else { 0.1 } {
                        return Err("메모리 매치 대기 시간을 확인하세요.".into());
                    }
                }
            }
            let state = if here {
                State::Open {
                    attempt: 0,
                    refresh: false,
                }
            } else {
                let place = config["move"]["places"]
                    .as_array()
                    .and_then(|p| p.iter().position(|p| p["feat"] == "mmatch"))
                    .ok_or("메모리 매치 장소를 먼저 지정하세요.")?;
                State::Travel(place)
            };
            Ok(Self {
                cfg,
                here,
                state,
                pending: None,
                due: now,
                observing: false,
                notice: None,
                board: None,
                estimate: None,
                hidden: Vec::new(),
                known: vec![None; 20],
                done: [false; 20],
                bad: BTreeSet::new(),
                first: None,
                second: None,
                previous: None,
                reveal_at: now,
                reveal_presses: 0,
                flipped: true,
                stuck: 0,
                presses: 0,
                chances: 10,
                zero: 0,
                board_started: now,
                round_started: now,
                rounds: 0,
                ads_attempted: 0,
                ad_fail: 0,
                ad_before: None,
                ad_pending: false,
                ad_ended: 0,
                ad_close: Point(0.0, 0.0),
                close: Point(0.0, 0.0),
                played: 0,
                ads: 0,
                next_at: 0.0,
                clock: now,
                seed: now.to_bits() | 1,
                msg: "메모리 매치 장소로 가는 중".into(),
                patch: None,
            })
        }

        pub(super) fn calibrate(
            config: &Value,
            board_only: bool,
            now: f64,
        ) -> Result<Self, String> {
            let mut memory = Self::new(config, true, now)?;
            memory.state = if board_only {
                State::CalBoard
            } else {
                State::CalNotice {
                    deadline: now + 120.0,
                    settled: false,
                }
            };
            memory.msg = if board_only {
                "카드 판 확인 중"
            } else {
                "메모리 매치 보드 앞에서 E 를 눌러 주세요"
            }
            .into();
            Ok(memory)
        }

        pub(super) fn ack(&mut self, ok: bool, now: f64) {
            let Some((mut next, delay)) = self.pending.take() else {
                return;
            };
            if !ok {
                self.state =
                    State::Failed("메모리 매치 입력 또는 이동을 완료하지 못했습니다.".into());
                self.next_at = now + 600.0;
                return;
            }
            match &mut next {
                State::Notice { deadline, .. } => {
                    *deadline = now + number(&self.cfg, "e_wait", 2.0) + 1.5
                }
                State::FindBoard { deadline, .. } => {
                    self.board_started = now;
                    *deadline = now + 6.0;
                }
                State::Reveal { .. } => {
                    if self.reveal_presses == 1 {
                        self.reveal_at = now;
                    }
                }
                State::AdStart { deadline } => *deadline = now + 15.0,
                State::AdWatch { started } => *started = now,
                _ => {}
            }
            self.state = next;
            self.due = now + delay;
            self.observing = false;
        }

        pub(super) fn status(&self) -> Value {
            json!({"running": !matches!(self.state, State::Done | State::Failed(_)),
                "msg": self.msg, "played": self.played, "ads": self.ads,
                "next_in": (self.next_at - self.clock).max(0.0) as u64,
                "presses": self.presses, "chances": self.chances,
                "matched": self.done.iter().filter(|&&v| v).count() / 2})
        }

        pub(super) fn config_patch(&self) -> Option<Value> {
            self.patch.clone()
        }

        pub(super) fn next_at(&self) -> f64 {
            self.next_at
        }
        pub(super) fn ocr_options(&self) -> (u32, u32) {
            match self.state {
                State::Amount { .. } => (3, 12),
                State::Chances => (1, 30),
                _ => (1, 0),
            }
        }

        fn action(&mut self, step: Step, next: State, delay: f64) -> Step {
            self.pending = Some((next, delay));
            self.observing = false;
            step
        }

        fn observe(
            &mut self,
            frame: Option<&Frame<'_>>,
            region: Option<Rect>,
            ocr: bool,
        ) -> Option<Step> {
            if !self.observing || frame.is_none() {
                self.observing = true;
                Some(Step::Observe { region, ocr })
            } else {
                self.observing = false;
                None
            }
        }

        fn later(&mut self, state: State, now: f64, delay: f64) -> Step {
            self.state = state;
            self.due = now + delay;
            self.observing = false;
            Step::Wait(delay)
        }

        fn finish(&mut self, now: f64, seconds: f64) -> Step {
            self.next_at = now + seconds;
            self.msg = "메모리 매치 끝".into();
            self.later(
                if self.here {
                    State::Done
                } else {
                    State::Reset(0)
                },
                now,
                0.0,
            )
        }

        fn end_round(&mut self, now: f64) -> Step {
            self.played += 1;
            self.msg = "Close".into();
            self.close = point(&self.cfg, "board_close_pos")
                .unwrap_or_else(|| self.board.as_ref().unwrap().close);
            self.later(
                State::CloseFind {
                    deadline: now + 8.0,
                },
                now,
                0.0,
            )
        }

        fn random_index(&mut self, values: &[usize]) -> usize {
            self.seed ^= self.seed << 13;
            self.seed ^= self.seed >> 7;
            self.seed ^= self.seed << 17;
            values[(self.seed as usize) % values.len()]
        }

        fn reveal_failed(&mut self, first: bool, now: f64) -> Step {
            self.flipped = false;
            if first && self.second.is_none() {
                self.stuck += 1;
                if self.stuck >= 3 {
                    return self.end_round(now);
                }
                return self.later(State::Check, now, 0.0);
            }
            self.later(
                State::Settle {
                    deadline: now + 4.0,
                },
                now,
                0.15,
            )
        }

        pub(super) fn tick(&mut self, now: f64, frame: Option<&Frame<'_>>, words: &[Word]) -> Step {
            self.clock = now;
            if self.pending.is_some() {
                return Step::Wait(0.01);
            }
            if now < self.due {
                return Step::Wait(self.due - now);
            }
            match self.state.clone() {
                State::Travel(place) => self.action(
                    Step::Travel { place },
                    State::Open {
                        attempt: 0,
                        refresh: false,
                    },
                    0.0,
                ),
                State::Open { attempt, refresh } => {
                    self.msg = "E (메모리 매치 확인)".into();
                    let full = region(&self.cfg, "note_region").is_none();
                    self.action(
                        Step::Input(Motion::Keys(&[0x45], 40)),
                        State::Notice {
                            attempt,
                            refresh,
                            full,
                            deadline: 0.0,
                        },
                        0.0,
                    )
                }
                State::Notice {
                    attempt,
                    refresh,
                    full,
                    deadline,
                } => {
                    let roi = if full {
                        None
                    } else {
                        region(&self.cfg, "note_region")
                    };
                    if let Some(step) = self.observe(frame, roi, true) {
                        return step;
                    }
                    let f = frame.unwrap();
                    if let Some(st) = read_notice(words, f.width, f.height, &self.cfg) {
                        self.notice = Some(st);
                        return self.later(
                            State::SettleNotice { refresh, full },
                            now,
                            if refresh {
                                3.0
                            } else {
                                number(&self.cfg, "click_wait", 1.0)
                            },
                        );
                    }
                    if !full {
                        return self.later(
                            State::Notice {
                                attempt,
                                refresh,
                                full: true,
                                deadline,
                            },
                            now,
                            0.0,
                        );
                    }
                    if now >= deadline {
                        if attempt == 0 {
                            return self.later(
                                State::Open {
                                    attempt: 1,
                                    refresh,
                                },
                                now,
                                0.0,
                            );
                        }
                        return self.finish(now, 600.0);
                    }
                    self.later(
                        State::Notice {
                            attempt,
                            refresh,
                            full: region(&self.cfg, "note_region").is_none(),
                            deadline,
                        },
                        now,
                        0.4,
                    )
                }
                State::SettleNotice { refresh, full } => {
                    let roi = if full {
                        None
                    } else {
                        region(&self.cfg, "note_region")
                    };
                    if let Some(step) = self.observe(frame, roi, true) {
                        return step;
                    }
                    let f = frame.unwrap();
                    if let Some(st) = read_notice(words, f.width, f.height, &self.cfg) {
                        self.notice = Some(st);
                    }
                    if refresh && self.ad_pending {
                        let st = self.notice.as_ref().unwrap();
                        let got = st.start
                            || self
                                .ad_before
                                .zip(st.seconds)
                                .is_some_and(|(a, b)| b < a - 1800.0);
                        if got {
                            self.ads += 1;
                            self.ad_fail = 0;
                        } else {
                            self.ad_fail += 1;
                        }
                        self.ad_pending = false;
                    }
                    self.later(State::Decide, now, 0.0)
                }
                State::Decide => {
                    self.rounds += 1;
                    let Some(st) = self.notice.clone() else {
                        return self.finish(now, 600.0);
                    };
                    if self.rounds > 20 {
                        self.next_at = now + 600.0;
                        return self.later(State::FinishNotice, now, 0.0);
                    }
                    if st.start {
                        self.msg = "Start Memory Match".into();
                        return self.later(State::Start, now, 1.5);
                    }
                    if st.ad.is_some() && self.ads_attempted < 8 && self.ad_fail < 2 {
                        self.ads_attempted += 1;
                        self.ad_before = st.seconds;
                        self.ad_pending = true;
                        self.msg = format!("광고 보는 중 ({}번째)", self.ads_attempted);
                        return self.later(State::AdClick, now, 0.0);
                    }
                    self.next_at = now + st.seconds.map_or(3600.0, |s| s + 60.0);
                    self.later(State::FinishNotice, now, 0.0)
                }
                State::Start => {
                    if let Some(step) = self.observe(frame, None, false) {
                        return step;
                    }
                    let f = frame.unwrap();
                    let st = self.notice.as_ref().unwrap();
                    self.estimate = if self.cfg["cal_board"].as_bool().unwrap_or(false) {
                        saved_board(&self.cfg, f.width, f.height)
                    } else {
                        None
                    };
                    if self.estimate.is_none() {
                        self.estimate = st.head.map(|head| {
                            board_at(
                                head.0 * f.width as f64,
                                head.1 * f.height as f64 - 163.0 * st.scale,
                                st.scale,
                                f.width,
                                f.height,
                            )
                        });
                    }
                    self.action(
                        Step::Input(Motion::Tap(st.button, 40)),
                        State::FindBoard {
                            attempt: 0,
                            deadline: 0.0,
                            extended: false,
                        },
                        0.5,
                    )
                }
                State::StartRetry { attempt } => {
                    let button = self.notice.as_ref().unwrap().button;
                    self.action(
                        Step::Input(Motion::Tap(button, 40)),
                        State::FindBoard {
                            attempt,
                            deadline: 0.0,
                            extended: false,
                        },
                        0.5,
                    )
                }
                State::FindBoard {
                    attempt,
                    deadline,
                    extended,
                } => {
                    if let Some(step) = self.observe(frame, None, true) {
                        return step;
                    }
                    let f = frame.unwrap();
                    let notice = read_notice(words, f.width, f.height, &self.cfg);
                    let mut lay = board_layout(words, f.width, f.height).or_else(|| {
                        self.estimate
                            .as_ref()
                            .filter(|b| notice.is_none() && all_hidden(f, b))
                            .cloned()
                    });
                    if lay.is_none()
                        && now >= deadline
                        && !extended
                        && attempt < 2
                        && notice.as_ref().is_some_and(|n| n.start)
                    {
                        self.notice = notice;
                        return self.later(
                            State::StartRetry {
                                attempt: attempt + 1,
                            },
                            now,
                            1.0,
                        );
                    }
                    if lay.is_none() && now >= deadline && !extended && self.estimate.is_some() {
                        return self.later(
                            State::FindBoard {
                                attempt,
                                deadline: now + 15.0,
                                extended: true,
                            },
                            now,
                            0.7,
                        );
                    }
                    if lay.is_none() && now >= deadline {
                        lay = saved_board(&self.cfg, f.width, f.height);
                    }
                    if let Some(found) = lay {
                        self.board = Some(if self.cfg["cal_board"].as_bool().unwrap_or(false) {
                            saved_board(&self.cfg, f.width, f.height).unwrap_or(found)
                        } else {
                            found
                        });
                        self.msg = "카드 판 준비 확인 중".into();
                        return self.later(
                            State::Ready {
                                deadline: now.max(self.board_started + 5.5) + 30.0,
                            },
                            now,
                            (self.board_started + 5.5 - now).max(0.0),
                        );
                    }
                    if now >= deadline {
                        return self.finish(now, 600.0);
                    }
                    self.later(
                        State::FindBoard {
                            attempt,
                            deadline,
                            extended,
                        },
                        now,
                        if extended { 0.7 } else { 0.5 },
                    )
                }
                State::Ready { deadline } => {
                    if let Some(step) = self.observe(frame, None, false) {
                        return step;
                    }
                    let f = frame.unwrap();
                    let b = self.board.as_ref().unwrap();
                    if all_hidden(f, b) {
                        self.hidden = b.boxes.iter().map(|&r| signature(f, r)).collect();
                        self.known.fill(None);
                        self.done.fill(false);
                        self.bad.clear();
                        self.presses = 0;
                        self.chances = 10;
                        self.zero = 0;
                        self.stuck = 0;
                        self.round_started = now;
                        return self.later(State::Check, now, 1.0);
                    }
                    if now >= deadline {
                        return self.finish(now, 600.0);
                    }
                    self.later(State::Ready { deadline }, now, 0.5)
                }
                State::Check => {
                    let b = self.board.as_ref().unwrap();
                    let Point(x, y) = point(&self.cfg, "board_close_pos").unwrap_or(b.close);
                    let k = b.k;
                    let roi = Rect(
                        (x - 0.08 * k).max(0.0),
                        (y - 0.04 * k).max(0.0),
                        (x + 0.08 * k).min(1.0),
                        (y + 0.04 * k).min(1.0),
                    );
                    if let Some(step) = self.observe(frame, Some(roi), true) {
                        return step;
                    }
                    if close_word(words).is_some()
                        || self.presses >= 40
                        || now - self.round_started >= 600.0
                    {
                        return self.end_round(now);
                    }
                    let f = frame.unwrap();
                    let b = self.board.as_ref().unwrap();
                    let mut up = 0;
                    for i in 0..20 {
                        if self.done[i] {
                            continue;
                        }
                        if green_ratio(f, b.boxes[i]) > 0.5 {
                            self.done[i] = true;
                            self.known[i] = None;
                        } else if sig_diff(&signature(f, b.boxes[i]), &self.hidden[i]) > 12.0 {
                            up += 1;
                        }
                    }
                    if up >= 3 || self.done.iter().all(|&v| v) {
                        return self.end_round(now);
                    }
                    self.later(State::Chances, now, 0.0)
                }
                State::Chances => {
                    let roi = self.board.as_ref().unwrap().chances;
                    if let Some(step) = self.observe(frame, Some(roi), true) {
                        return step;
                    }
                    let seen = digits(&words_text(words))
                        .parse::<i32>()
                        .ok()
                        .filter(|&n| n <= 10);
                    self.zero = if seen == Some(0) { self.zero + 1 } else { 0 };
                    if let Some(n) =
                        seen.filter(|&n| (n - self.chances).abs() <= 1 || self.zero >= 2)
                    {
                        self.chances = n;
                    }
                    if self.chances <= 0 {
                        return self.end_round(now);
                    }
                    self.msg = format!(
                        "카드 짝 맞추기 · 남은 기회 {} · 맞춤 {}",
                        self.chances,
                        self.done.iter().filter(|&&v| v).count() / 2
                    );
                    self.later(State::Choose, now, 0.3)
                }
                State::Choose => {
                    self.first = None;
                    self.second = None;
                    self.flipped = true;
                    for a in 0..20 {
                        for b in a + 1..20 {
                            if !self.bad.contains(&(a, b))
                                && same_card(self.known[a].as_ref(), self.known[b].as_ref())
                            {
                                self.first = Some(a);
                                self.second = Some(b);
                                break;
                            }
                        }
                        if self.first.is_some() {
                            break;
                        }
                    }
                    if self.first.is_none() {
                        let unknown: Vec<_> = (0..20)
                            .filter(|&i| !self.done[i] && self.known[i].is_none())
                            .collect();
                        if unknown.is_empty() {
                            return self.end_round(now);
                        }
                        self.first = Some(self.random_index(&unknown));
                    }
                    self.later(
                        State::RevealClick {
                            index: self.first.unwrap(),
                            first: true,
                            retry: false,
                        },
                        now,
                        0.0,
                    )
                }
                State::RevealClick {
                    index,
                    first,
                    retry,
                } => {
                    if !retry {
                        self.previous = None;
                        self.reveal_presses = 0;
                    }
                    self.reveal_presses += 1;
                    self.presses += 1;
                    self.action(
                        Step::Input(Motion::Tap(self.board.as_ref().unwrap().cards[index], 100)),
                        State::Reveal { index, first },
                        0.1,
                    )
                }
                State::Reveal { index, first } => {
                    if let Some(step) = self.observe(frame, None, false) {
                        return step;
                    }
                    let f = frame.unwrap();
                    let roi = self.board.as_ref().unwrap().boxes[index];
                    let sig = signature(f, roi);
                    if now - self.reveal_at >= 3.5 {
                        return self.reveal_failed(first, now);
                    }
                    if sig_diff(&sig, &self.hidden[index]) <= 12.0 {
                        self.previous = None;
                        if now - self.reveal_at > self.reveal_presses as f64
                            && self.reveal_presses < 3
                        {
                            return self.later(
                                State::RevealClick {
                                    index,
                                    first,
                                    retry: true,
                                },
                                now,
                                0.0,
                            );
                        }
                    } else {
                        if self
                            .previous
                            .as_ref()
                            .is_some_and(|s| sig_diff(&sig, s) < 4.0)
                        {
                            let card = feature(f, roi);
                            return self.later(State::Amount { index, first, card }, now, 0.0);
                        }
                        self.previous = Some(sig);
                    }
                    self.later(State::Reveal { index, first }, now, 0.1)
                }
                State::Amount {
                    index,
                    first,
                    mut card,
                } => {
                    let roi = self.board.as_ref().unwrap().texts[index];
                    if let Some(step) = self.observe(frame, Some(roi), true) {
                        return step;
                    }
                    card.amount = digits(&words_text(words));
                    self.known[index] = Some(card);
                    self.stuck = 0;
                    if first {
                        if self.second.is_none() {
                            self.second = (0..20).find(|&i| {
                                i != index
                                    && !self.done[i]
                                    && !self.bad.contains(&(i.min(index), i.max(index)))
                                    && same_card(self.known[index].as_ref(), self.known[i].as_ref())
                            });
                        }
                        if self.second.is_none() {
                            let unknown: Vec<_> = (0..20)
                                .filter(|&i| i != index && !self.done[i] && self.known[i].is_none())
                                .collect();
                            let rest = if unknown.is_empty() {
                                (0..20)
                                    .filter(|&i| {
                                        i != index && !self.done[i] && self.known[i].is_some()
                                    })
                                    .collect()
                            } else {
                                unknown
                            };
                            if rest.is_empty() {
                                return self.end_round(now);
                            }
                            self.second = Some(self.random_index(&rest));
                        }
                        return self.later(
                            State::RevealClick {
                                index: self.second.unwrap(),
                                first: false,
                                retry: false,
                            },
                            now,
                            0.0,
                        );
                    }
                    self.later(
                        State::Settle {
                            deadline: now + 4.0,
                        },
                        now,
                        0.15,
                    )
                }
                State::Settle { deadline } => {
                    if let Some(step) = self.observe(frame, None, false) {
                        return step;
                    }
                    let f = frame.unwrap();
                    let b = self.board.as_ref().unwrap();
                    let (Some(a), Some(c)) = (self.first, self.second) else {
                        return self.later(State::Check, now, 1.0);
                    };
                    if [a, c].iter().all(|&i| green_ratio(f, b.boxes[i]) > 0.5) {
                        for i in [a, c] {
                            self.done[i] = true;
                            self.known[i] = None;
                        }
                        return self.later(State::Check, now, 0.0);
                    }
                    let hidden = [a, c]
                        .iter()
                        .all(|&i| sig_diff(&signature(f, b.boxes[i]), &self.hidden[i]) < 8.0);
                    if hidden || now >= deadline {
                        if self.flipped {
                            self.chances -= 1;
                            self.bad.insert((a.min(c), a.max(c)));
                        }
                        return self.later(State::Check, now, if self.flipped { 0.8 } else { 1.0 });
                    }
                    self.later(State::Settle { deadline }, now, 0.15)
                }
                State::CloseFind { deadline } => {
                    if let Some(step) = self.observe(frame, None, true) {
                        return step;
                    }
                    if let Some(w) = close_word(words) {
                        if point(&self.cfg, "board_close_pos").is_none() {
                            self.close = center(w);
                        }
                        return self.later(State::CloseClick { attempt: 0 }, now, 1.0);
                    }
                    if now >= deadline {
                        return self.later(State::CloseClick { attempt: 0 }, now, 1.0);
                    }
                    self.later(State::CloseFind { deadline }, now, 0.5)
                }
                State::CloseClick { attempt } => self.action(
                    Step::Input(Motion::Tap(self.close, 40)),
                    State::CloseVerify { attempt },
                    1.0,
                ),
                State::CloseVerify { attempt } => {
                    if let Some(step) = self.observe(frame, None, true) {
                        return step;
                    }
                    let f = frame.unwrap();
                    let close = close_word(words);
                    if (close.is_none() && board_layout(words, f.width, f.height).is_none())
                        || attempt >= 2
                    {
                        return self.later(
                            State::Refresh {
                                deadline: now + 3.0,
                            },
                            now,
                            1.5,
                        );
                    }
                    if let Some(w) = close {
                        self.close = center(w);
                    }
                    self.later(
                        State::CloseClick {
                            attempt: attempt + 1,
                        },
                        now,
                        0.0,
                    )
                }
                State::Refresh { deadline } => {
                    if let Some(step) = self.observe(frame, None, true) {
                        return step;
                    }
                    let f = frame.unwrap();
                    if let Some(st) = read_notice(words, f.width, f.height, &self.cfg) {
                        if now < deadline {
                            return self.action(
                                Step::Input(Motion::Tap(st.close, 40)),
                                State::Refresh { deadline },
                                0.4,
                            );
                        }
                    }
                    self.later(
                        State::Open {
                            attempt: 0,
                            refresh: true,
                        },
                        now,
                        1.0,
                    )
                }
                State::AdClick => self.action(
                    Step::Input(Motion::Tap(self.notice.as_ref().unwrap().ad.unwrap(), 40)),
                    State::AdStart { deadline: 0.0 },
                    0.5,
                ),
                State::AdStart { deadline } => {
                    if let Some(step) = self.observe(frame, None, false) {
                        return step;
                    }
                    if let Some((close, _)) = ad_screen(frame.unwrap()) {
                        self.ad_close = close;
                        self.ad_ended = 0;
                        return self.later(State::AdWatch { started: now }, now, 1.0);
                    }
                    if now >= deadline {
                        return self.later(
                            State::Refresh {
                                deadline: now + 6.0,
                            },
                            now,
                            3.0,
                        );
                    }
                    self.later(State::AdStart { deadline }, now, 0.5)
                }
                State::AdWatch { started } => {
                    if let Some(step) = self.observe(frame, None, false) {
                        return step;
                    }
                    let elapsed = now - started;
                    self.msg = format!("광고 보는 중 · {}초", elapsed as u64);
                    if let Some((close, state)) = ad_screen(frame.unwrap()) {
                        self.ad_close = close;
                        self.ad_ended = if state == Some(true) {
                            self.ad_ended + 1
                        } else {
                            0
                        };
                        if (self.ad_ended >= 2 && elapsed >= 10.0)
                            || (state.is_none() && elapsed >= 45.0)
                            || elapsed >= 90.0
                        {
                            return self.later(State::AdClose { attempt: 0 }, now, 0.0);
                        }
                    } else if elapsed > 3.0 {
                        return self.later(
                            State::Refresh {
                                deadline: now + 7.0,
                            },
                            now,
                            4.0,
                        );
                    }
                    self.later(State::AdWatch { started }, now, 1.0)
                }
                State::AdClose { attempt } => {
                    if let Some(step) = self.observe(frame, None, false) {
                        return step;
                    }
                    if attempt >= 3 || ad_screen(frame.unwrap()).is_none() {
                        return self.later(
                            State::Refresh {
                                deadline: now + 7.0,
                            },
                            now,
                            4.0,
                        );
                    }
                    self.action(
                        Step::Input(Motion::Tap(self.ad_close, 40)),
                        State::AdClose {
                            attempt: attempt + 1,
                        },
                        2.5,
                    )
                }
                State::FinishNotice => {
                    let next = if self.here {
                        State::Done
                    } else {
                        State::Reset(0)
                    };
                    if let Some(st) = &self.notice {
                        return self.action(Step::Input(Motion::Tap(st.close, 40)), next, 0.4);
                    }
                    self.later(next, now, 0.0)
                }
                State::CalNotice { deadline, settled } => {
                    if let Some(step) = self.observe(frame, None, true) {
                        return step;
                    }
                    let f = frame.unwrap();
                    if let Ok(mut patch) = notice_calibration(words, f.width, f.height) {
                        patch["cal_note"] = true.into();
                        self.patch = Some(patch);
                    }
                    if self.patch.is_some() {
                        if !settled {
                            self.msg = "메모리 매치 창 찾음 — 이제 만지지 마세요".into();
                            return self.later(
                                State::CalNotice {
                                    deadline,
                                    settled: true,
                                },
                                now,
                                0.8,
                            );
                        }
                        self.msg = "창 닫는 중".into();
                        let close = point(self.patch.as_ref().unwrap(), "note_close_pos").unwrap();
                        return self.action(
                            Step::Input(Motion::Tap(close, 40)),
                            State::CalCloseVerify { attempt: 0 },
                            0.6,
                        );
                    }
                    if now >= deadline {
                        self.state = State::Failed(
                            "메모리 매치 창이 안 보입니다. 보드 앞에서 E 를 눌러 주세요.".into(),
                        );
                        return self.tick(now, None, &[]);
                    }
                    self.later(State::CalNotice { deadline, settled }, now, 0.4)
                }
                State::CalCloseVerify { attempt } => {
                    let patch = self.patch.as_ref().unwrap();
                    let roi = region(patch, "note_region");
                    if let Some(step) = self.observe(frame, roi, true) {
                        return step;
                    }
                    let f = frame.unwrap();
                    if attempt >= 1 || read_notice(words, f.width, f.height, &json!({})).is_none() {
                        return self.later(State::Done, now, 0.0);
                    }
                    let close = point(self.patch.as_ref().unwrap(), "note_close_pos").unwrap();
                    self.action(
                        Step::Input(Motion::Tap(close, 40)),
                        State::CalCloseVerify {
                            attempt: attempt + 1,
                        },
                        0.6,
                    )
                }
                State::CalBoard => {
                    if let Some(step) = self.observe(frame, None, true) {
                        return step;
                    }
                    let f = frame.unwrap();
                    match board_calibration(words, f.width, f.height) {
                        Ok(patch) => {
                            self.patch = Some(patch);
                            self.later(State::Done, now, 0.0)
                        }
                        Err(error) => {
                            self.state = State::Failed(error.clone());
                            Step::Failed(error)
                        }
                    }
                }
                State::Reset(index) => {
                    self.msg = "리셋".into();
                    let keys: &'static [u16] = match index {
                        0 => &[0x1b],
                        1 => &[0x52],
                        2 => &[0x0d],
                        _ => return self.later(State::Done, now, 0.0),
                    };
                    self.action(
                        Step::Input(Motion::Keys(keys, 40)),
                        State::Reset(index + 1),
                        0.5,
                    )
                }
                State::Done => {
                    self.msg = "대기".into();
                    Step::Done
                }
                State::Failed(error) => {
                    self.msg = error.clone();
                    Step::Failed(error)
                }
            }
        }
    }

    fn r(x: f64, y: f64, w: usize, h: usize) -> Point {
        Point(
            (x / w as f64).clamp(0.0, 1.0),
            (y / h as f64).clamp(0.0, 1.0),
        )
    }
    fn rr(x0: f64, y0: f64, x1: f64, y1: f64, w: usize, h: usize) -> Rect {
        let a = r(x0, y0, w, h);
        let b = r(x1, y1, w, h);
        Rect(a.0, a.1, b.0, b.1)
    }
    fn board_at(tx: f64, ty: f64, k: f64, w: usize, h: usize) -> Board {
        let mut cards = Vec::new();
        let mut boxes = Vec::new();
        let mut texts = Vec::new();
        for row in 0..4 {
            for col in 0..5 {
                let x = tx + (-136.0 + 112.75 * col as f64) * k;
                let y = ty + (94.0 + 121.3 * row as f64) * k;
                cards.push(r(x, y, w, h));
                boxes.push(rr(
                    x - 42.0 * k,
                    y - 46.0 * k,
                    x + 42.0 * k,
                    y + 46.0 * k,
                    w,
                    h,
                ));
                texts.push(rr(
                    x - 52.0 * k,
                    y + 21.0 * k,
                    x + 52.0 * k,
                    y + 55.0 * k,
                    w,
                    h,
                ));
            }
        }
        Board {
            cards,
            boxes,
            texts,
            close: r(tx, ty + 549.0 * k, w, h),
            title: r(tx, ty, w, h),
            k: k * 1080.0 / h as f64,
            chances: rr(
                tx - 343.0 * k,
                ty + 41.0 * k,
                tx - 233.0 * k,
                ty + 107.0 * k,
                w,
                h,
            ),
        }
    }
    fn board_layout(words: &[Word], w: usize, h: usize) -> Option<Board> {
        let ch = words.iter().find(|b| norm(&b.text).ends_with("honces"))?;
        let t = words
            .iter()
            .filter(|b| norm(&b.text).ends_with("emorymotch") && center(b).1 < center(ch).1)
            .max_by(|a, b| center(a).1.total_cmp(&center(b).1))?;
        let k = (center(ch).1 - center(t).1) * h as f64 / 114.0;
        (0.25 < k && k < 4.0)
            .then(|| board_at(center(t).0 * w as f64, center(t).1 * h as f64, k, w, h))
    }
    fn saved_board(c: &Value, w: usize, h: usize) -> Option<Board> {
        let title = point(c, "board_title")?;
        let k = c["board_k"].as_f64()?;
        if !k.is_finite() || k <= 0.0 {
            return None;
        }
        let mut b = board_at(
            title.0 * w as f64,
            title.1 * h as f64,
            k * h as f64 / 1080.0,
            w,
            h,
        );
        if let Some(close) = point(c, "board_close_pos") {
            b.close = close;
        }
        Some(b)
    }
    fn read_notice(words: &[Word], w: usize, h: usize, cfg: &Value) -> Option<Notice> {
        let head = words.iter().find(|b| {
            let n = norm(&b.text);
            n.contains("flcotlon") || (n.starts_with("notl") && n.contains("cot"))
        });
        let ad = words.iter().find(|b| {
            let n = norm(&b.text);
            n.contains("wotchod") || n.contains("lowerthecool") || n.contains("cooltlmefor")
        });
        let avail = words.iter().find_map(|b| {
            b.text
                .to_lowercase()
                .contains("fter")
                .then(|| parse_wait(&b.text).map(|s| (b, s)))
                .flatten()
        });
        let mut starts: Vec<_> = words
            .iter()
            .filter(|b| {
                let n = norm(&b.text);
                n.ends_with("tortmemorymotch") || (n.contains("ovolloble") && n.contains("reody"))
            })
            .collect();
        starts.retain(|b| {
            head.is_none_or(|hd| {
                (center(b).1 - center(hd).1) * h as f64 > 157.0 * 0.65 * h as f64 / 1080.0
            })
        });
        starts.retain(|b| {
            ad.is_none_or(|a| {
                let d = (center(a).1 - center(b).1) * h as f64;
                d >= 0.0 && d < 100.0 * h as f64 / 1080.0
            })
        });
        starts.sort_by(|a, b| center(a).1.total_cmp(&center(b).1));
        let button = if avail.is_none() {
            starts.last().copied()
        } else {
            None
        };
        let low = button.or_else(|| avail.map(|(b, _)| b))?;
        let lowp = center(low);
        let mut k = h as f64 / 1080.0;
        let hp = if let Some(head) = head {
            let p = center(head);
            let guess = (lowp.1 - p.1) * h as f64 / 157.0;
            if 0.25 < guess && guess < 4.0 {
                k = guess;
            }
            p
        } else {
            Point(lowp.0, lowp.1 - 157.0 * k / h as f64)
        };
        Some(Notice {
            start: button.is_some(),
            seconds: avail.map(|(_, s)| s),
            button: point(cfg, "note_btn_pos").unwrap_or(lowp),
            close: point(cfg, "note_close_pos")
                .unwrap_or_else(|| r(hp.0 * w as f64 + 271.0 * k, hp.1 * h as f64 - 3.0 * k, w, h)),
            ad: ad.map(|a| point(cfg, "ad_pos").unwrap_or_else(|| center(a))),
            head: Some(hp),
            has_head: head.is_some(),
            scale: k,
        })
    }
    fn parse_wait(text: &str) -> Option<f64> {
        let text = text.to_lowercase();
        let text = text.rsplit_once("fter").map_or(text.as_str(), |(_, s)| s);
        let s: String = text
            .chars()
            .map(|c| match c {
                'o' => '0',
                'i' | 'l' | '|' => '1',
                _ => c,
            })
            .collect();
        let mut nums = Vec::<(u32, char)>::new();
        let mut number = String::new();
        for c in s.chars() {
            if c.is_ascii_digit() {
                number.push(c);
            } else if c.is_whitespace() {
            } else {
                if !number.is_empty() && number.len() <= 2 {
                    if let Ok(n) = number.parse() {
                        nums.push((n, c));
                    }
                }
                number.clear();
            }
        }
        nums.windows(2).find_map(|v| match (v[0], v[1]) {
            ((a, 'h'), (b, 'm')) => Some((a * 3600 + b * 60) as f64),
            ((a, 'm'), (b, 's')) => Some((a * 60 + b) as f64),
            _ => None,
        })
    }
    fn digits(text: &str) -> String {
        text.chars()
            .map(|c| if c == 'O' || c == 'o' { '0' } else { c })
            .filter(char::is_ascii_digit)
            .collect()
    }
    fn close_word(words: &[Word]) -> Option<&Word> {
        words
            .iter()
            .find(|w| matches!(norm(&w.text).as_str(), "close" | "cose" | "lose" | "clse"))
    }

    fn bounds(frame: &Frame<'_>, rect: Rect) -> (usize, usize, usize, usize) {
        let x0 = (rect.0 * frame.width as f64)
            .floor()
            .clamp(0.0, (frame.width - 1) as f64) as usize;
        let y0 = (rect.1 * frame.height as f64)
            .floor()
            .clamp(0.0, (frame.height - 1) as f64) as usize;
        let x1 = (rect.2 * frame.width as f64)
            .floor()
            .clamp((x0 + 1) as f64, frame.width as f64) as usize;
        let y1 = (rect.3 * frame.height as f64)
            .floor()
            .clamp((y0 + 1) as f64, frame.height as f64) as usize;
        (x0, y0, x1, y1)
    }
    fn average(frame: &Frame<'_>, x0: usize, y0: usize, x1: usize, y1: usize) -> [f64; 3] {
        let mut sum = [0.0; 3];
        let mut n = 0.0;
        for y in y0..y1 {
            for x in x0..x1 {
                let p = frame.pixel(x, y);
                for c in 0..3 {
                    sum[c] += p[c] as f64;
                }
                n += 1.0;
            }
        }
        if n > 0.0 {
            for c in &mut sum {
                *c /= n;
            }
        }
        sum
    }
    fn signature(frame: &Frame<'_>, rect: Rect) -> Signature {
        let (x0, y0, x1, y1) = bounds(frame, rect);
        let width = x1 - x0;
        let height = ((y1 - y0) as f64 * 0.62)
            .floor()
            .max(2.0)
            .min((y1 - y0) as f64) as usize;
        let mut small = [0.0; 300];
        // INTER_AREA: integrate each source pixel's overlap with the 10x10 output cells.
        for oy in 0..10 {
            for ox in 0..10 {
                let l = ox as f64 * width as f64 / 10.0;
                let r = (ox + 1) as f64 * width as f64 / 10.0;
                let t = oy as f64 * height as f64 / 10.0;
                let b = (oy + 1) as f64 * height as f64 / 10.0;
                let mut sum = [0.0; 3];
                let mut weight = 0.0;
                for y in t.floor() as usize..(b.ceil() as usize).min(height) {
                    for x in l.floor() as usize..(r.ceil() as usize).min(width) {
                        let q = (r.min((x + 1) as f64) - l.max(x as f64)).max(0.0)
                            * (b.min((y + 1) as f64) - t.max(y as f64)).max(0.0);
                        let p = frame.pixel(x0 + x, y0 + y);
                        for c in 0..3 {
                            sum[c] += p[c] as f64 * q;
                        }
                        weight += q;
                    }
                }
                for c in 0..3 {
                    small[(oy * 10 + ox) * 3 + c] = (sum[c] / weight.max(f64::EPSILON)).round();
                }
            }
        }
        let mean = average(
            frame,
            x0 + width / 4,
            y0 + height / 4,
            x0 + (3 * width / 4).max(width / 4 + 1),
            y0 + (3 * height / 4).max(height / 4 + 1),
        );
        Signature { small, mean }
    }
    fn sig_diff(a: &Signature, b: &Signature) -> f64 {
        a.small
            .iter()
            .zip(&b.small)
            .map(|(a, b)| (a - b).abs())
            .sum::<f64>()
            / 300.0
    }
    fn feature(frame: &Frame<'_>, rect: Rect) -> Card {
        let (x0, y0, x1, y1) = bounds(frame, rect);
        let (w, h) = (x1 - x0, y1 - y0);
        let left = x0 + (w as f64 * 0.15) as usize;
        let right = x0
            + ((w as f64 * 0.85) as usize)
                .max((w as f64 * 0.15) as usize + 1)
                .min(w);
        let top = y0 + (h as f64 * 0.08) as usize;
        let bottom = y0
            + ((h as f64 * 0.62) as usize)
                .max((h as f64 * 0.08) as usize + 1)
                .min(h);
        let mut sum = [0.0; 3];
        let mut count = 0;
        for y in top..bottom {
            for x in left..right {
                let p = frame.pixel(x, y);
                if *p.iter().max().unwrap() > 90 {
                    for c in 0..3 {
                        sum[c] += p[c] as f64;
                    }
                    count += 1;
                }
            }
        }
        Card {
            mean: (count > 0).then(|| sum.map(|n| n / count as f64)),
            fraction: count as f64 / ((right - left) * (bottom - top)).max(1) as f64,
            amount: String::new(),
        }
    }
    fn same_card(a: Option<&Card>, b: Option<&Card>) -> bool {
        let (Some(a), Some(b)) = (a, b) else {
            return false;
        };
        let (Some(am), Some(bm)) = (a.mean, b.mean) else {
            return false;
        };
        am.iter().zip(&bm).all(|(a, b)| (a - b).abs() < 28.0)
            && (a.fraction - b.fraction).abs() < 0.12
            && (a.amount == b.amount || a.amount.is_empty() || b.amount.is_empty())
    }
    fn card_back(frame: &Frame<'_>, rect: Rect) -> bool {
        let (x0, y0, x1, y1) = bounds(frame, rect);
        let (w, h) = (x1 - x0, y1 - y0);
        if w < 6 || h < 6 {
            return false;
        }
        let mut dark = 0;
        let mut white = 0;
        for y in y0..y1 {
            for x in x0..x1 {
                let p = frame.pixel(x, y);
                dark += usize::from(*p.iter().max().unwrap() < 80);
                if x >= x0 + w / 4 && x < x0 + 3 * w / 4 && y >= y0 + h / 4 && y < y0 + 3 * h / 4 {
                    white += usize::from(*p.iter().min().unwrap() > 200);
                }
            }
        }
        let q = white as f64 / ((3 * w / 4 - w / 4) * (3 * h / 4 - h / 4)) as f64;
        dark as f64 / (w * h) as f64 > 0.4 && q > 0.01 && q < 0.5
    }
    fn green_ratio(frame: &Frame<'_>, rect: Rect) -> f64 {
        let (x0, y0, x1, y1) = bounds(frame, rect);
        let mut n = 0;
        for y in y0..y1 {
            for x in x0..x1 {
                let [r, g, b] = frame.pixel(x, y).map(i16::from);
                n += usize::from(g > 110 && g > r + 30 && g > b + 30);
            }
        }
        n as f64 / ((x1 - x0) * (y1 - y0)) as f64
    }
    fn all_hidden(frame: &Frame<'_>, b: &Board) -> bool {
        if !b.boxes.iter().all(|&r| card_back(frame, r)) {
            return false;
        }
        let sigs: Vec<_> = b.boxes.iter().map(|&r| signature(frame, r)).collect();
        let mut reference = Signature {
            small: [0.0; 300],
            mean: [0.0; 3],
        };
        let median = |mut v: Vec<f64>| {
            v.sort_by(f64::total_cmp);
            (v[v.len() / 2 - 1] + v[v.len() / 2]) / 2.0
        };
        for i in 0..300 {
            reference.small[i] = median(sigs.iter().map(|s| s.small[i]).collect());
        }
        for i in 0..3 {
            reference.mean[i] = median(sigs.iter().map(|s| s.mean[i]).collect());
        }
        sigs.iter().all(|s| {
            let [r, g, b] = s.mean;
            sig_diff(s, &reference) < 8.0 && !(g > 110.0 && g > r + 30.0 && g > b + 30.0)
        })
    }
    // Some(true) = triangle/ended; Some(false) = two pause bars/playing.
    fn ad_screen(frame: &Frame<'_>) -> Option<(Point, Option<bool>)> {
        let (w, h) = (frame.width, frame.height);
        if w < 100 || h < 100 {
            return None;
        }
        let s = h as f64 / 1080.0;
        let side = average(
            frame,
            (w as f64 * 0.05) as usize,
            (h as f64 * 0.28) as usize,
            (w as f64 * 0.18) as usize,
            (h as f64 * 0.74) as usize,
        );
        if side.iter().sum::<f64>() / 3.0 > 12.0 {
            return None;
        }
        let ex = w as f64 * 1513.0 / 1920.0
            + ((w as f64 / h as f64 - 16.0 / 9.0) * h as f64 * 0.03).max(0.0);
        let ey = h as f64 * 239.0 / 1080.0;
        let radius = (40.0 * s).floor().max(12.0);
        let (mut sx, mut sy, mut n) = (0.0, 0.0, 0usize);
        for y in (ey - radius).max(0.0) as usize..((ey + radius) as usize).min(h) {
            for x in (ex - radius).max(0.0) as usize..((ex + radius) as usize).min(w) {
                if *frame.pixel(x, y).iter().min().unwrap() > 180 {
                    sx += x as f64;
                    sy += y as f64;
                    n += 1;
                }
            }
        }
        if (n as f64) < 60.0 * s * s {
            return None;
        }
        let (cx, cy) = (sx / n as f64, sy / n as f64);
        let (ix, iy) = (cx + 3.0 * s, cy + 582.0 * s);
        let (a, b) = ((20.0 * s).floor().max(4.0), (16.0 * s).floor().max(4.0));
        let cols: Vec<usize> = ((ix - a).max(0.0) as usize..((ix + a) as usize).min(w))
            .map(|x| {
                ((iy - b).max(0.0) as usize..((iy + b) as usize).min(h))
                    .filter(|&y| *frame.pixel(x, y).iter().min().unwrap() > 200)
                    .count()
            })
            .collect();
        let idx: Vec<_> = cols
            .iter()
            .enumerate()
            .filter_map(|(i, &n)| (n >= 2).then_some(i))
            .collect();
        let state = (idx.len() >= 3).then(|| {
            !cols[*idx.first().unwrap()..=*idx.last().unwrap()]
                .iter()
                .any(|&n| n < 2)
        });
        Some((r(cx, cy, w, h), state))
    }

    pub(super) fn notice_calibration(words: &[Word], w: usize, h: usize) -> Result<Value, String> {
        let st = read_notice(words, w, h, &json!({})).ok_or("메모리 매치 창이 안 보입니다.")?;
        if !st.has_head {
            return Err("Notification 제목을 찾지 못했습니다.".into());
        }
        let head = st.head.unwrap();
        let (hx, hy, k) = (head.0 * w as f64, head.1 * h as f64, st.scale);
        let p = |p: Point| json!([p.0, p.1]);
        let area = rr(
            hx - 310.0 * k,
            hy - 30.0 * k,
            hx + 310.0 * k,
            hy + 240.0 * k,
            w,
            h,
        );
        Ok(
            json!({"note_close_pos":p(st.close),"note_btn_pos":p(st.button),"ad_pos":p(st.ad.unwrap_or_else(||r(hx,hy+206.0*k,w,h))),"note_region":[area.0,area.1,area.2,area.3],"board_title":p(r(hx,hy-163.0*k,w,h)),"board_k":k*1080.0/h as f64,"board_close_pos":p(r(hx,hy+386.0*k,w,h))}),
        )
    }
    pub(super) fn board_calibration(words: &[Word], w: usize, h: usize) -> Result<Value, String> {
        let b = board_layout(words, w, h).ok_or("메모리 매치 카드 판이 안 보입니다.")?;
        Ok(
            json!({"board_title":[b.title.0,b.title.1],"board_k":b.k,"board_close_pos":[b.close.0,b.close.1],"cal_board":true}),
        )
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        fn word(text: &str, x: f64, y: f64) -> Word {
            Word {
                text: text.into(),
                rect: Rect(x - 0.02, y - 0.01, x + 0.02, y + 0.01),
            }
        }
        #[test]
        fn memory_notice_prefers_cooldown_to_always_visible_start_title() {
            let words = [
                word("Notification", 0.5, 0.3),
                word("Start Memory Match?", 0.5, 0.35),
                word("Available after O3h O4m Left", 0.5, 0.445),
                word("Watch AD to lower the cooltime for 3 hours!", 0.5, 0.49),
            ];
            let st = read_notice(&words, 1920, 1080, &json!({})).unwrap();
            assert!(!st.start);
            assert_eq!(st.seconds, Some(11040.0));
            assert!(st.ad.is_some());
            assert_eq!(parse_wait("Available after iih53m Left"), Some(42780.0));
        }
        #[test]
        fn memory_same_picture_with_different_amounts_is_not_a_pair() {
            let a = Card {
                mean: Some([30.0, 80.0, 220.0]),
                fraction: 0.3,
                amount: "100".into(),
            };
            let mut b = a.clone();
            b.amount = "200".into();
            assert!(!same_card(Some(&a), Some(&b)));
            b.amount.clear();
            assert!(same_card(Some(&a), Some(&b)));
        }
        #[test]
        fn memory_star_back_and_green_matched_card_are_distinct() {
            let mut bytes = vec![20u8; 40 * 40 * 3];
            for y in 17..23 {
                for x in 17..23 {
                    bytes[(y * 40 + x) * 3..(y * 40 + x) * 3 + 3].fill(240);
                }
            }
            let f = Frame::new(&bytes, 40, 40, 120, 3, false).unwrap();
            assert!(card_back(&f, Rect(0.0, 0.0, 1.0, 1.0)));
            assert_eq!(
                sig_diff(
                    &signature(&f, Rect(0.0, 0.0, 1.0, 1.0)),
                    &signature(&f, Rect(0.0, 0.0, 1.0, 1.0))
                ),
                0.0
            );
            let green = [30, 180, 30].repeat(1600);
            let f = Frame::new(&green, 40, 40, 120, 3, false).unwrap();
            assert_eq!(green_ratio(&f, Rect(0.0, 0.0, 1.0, 1.0)), 1.0);
            assert!(!card_back(&f, Rect(0.0, 0.0, 1.0, 1.0)));
        }
        #[test]
        fn memory_input_needs_ack_before_observation_deadline_starts() {
            let mut m = Memory::new(&json!({"mmatch":{}}), true, 1.0).unwrap();
            assert!(matches!(m.tick(1.0, None, &[]), Step::Input(_)));
            assert!(matches!(m.tick(20.0, None, &[]), Step::Wait(_)));
            m.ack(true, 20.0);
            assert!(matches!(m.state,State::Notice{deadline,..}if deadline==23.5));
        }
        #[test]
        fn memory_calibration_does_not_start_a_game_or_view_an_ad() {
            let mut m = Memory::calibrate(&json!({}), false, 1.0).unwrap();
            let words = [
                word("Notification", 0.5, 0.3),
                word("Start Memory Match", 0.5, 0.445),
                word("Watch AD to lower the cooltime for 3 hours!", 0.5, 0.49),
            ];
            let pixels = vec![20u8; 1920 * 1080 * 3];
            let f = Frame::new(&pixels, 1920, 1080, 1920 * 3, 3, false).unwrap();
            assert!(matches!(
                m.tick(1.0, None, &[]),
                Step::Observe { ocr: true, .. }
            ));
            assert!(matches!(m.tick(1.0, Some(&f), &words), Step::Wait(_)));
            assert!(m.config_patch().is_some());
            assert!(matches!(
                m.tick(2.0, None, &[]),
                Step::Observe { ocr: true, .. }
            ));
            match m.tick(2.0, Some(&f), &words) {
                Step::Input(Motion::Tap(p, _)) => {
                    assert!(p.0 > 0.6);
                    assert!(p.1 < 0.31);
                }
                _ => panic!("calibration must only close the notice"),
            }
            assert_eq!(m.ads, 0);
            assert_eq!(m.played, 0);
        }
        #[test]
        fn memory_failed_pair_is_excluded_and_reveals_have_a_press_limit() {
            let mut m = Memory::new(&json!({}), true, 1.0).unwrap();
            let card = Card {
                mean: Some([20.0, 90.0, 230.0]),
                fraction: 0.4,
                amount: "100".into(),
            };
            m.known[0] = Some(card.clone());
            m.known[1] = Some(card);
            m.bad.insert((0, 1));
            m.state = State::Choose;
            assert!(matches!(m.tick(1.0, None, &[]), Step::Wait(_)));
            assert_ne!(m.first, Some(0));
            m.board = Some(board_at(700.0, 100.0, 1.0, 1920, 1080));
            m.presses = 40;
            m.state = State::Check;
            let pixels = vec![20u8; 1920 * 1080 * 3];
            let f = Frame::new(&pixels, 1920, 1080, 1920 * 3, 3, false).unwrap();
            assert!(matches!(m.tick(2.0, None, &[]), Step::Observe { .. }));
            assert!(matches!(m.tick(2.0, Some(&f), &[]), Step::Wait(_)));
            assert_eq!(m.played, 1);
        }
    }
}

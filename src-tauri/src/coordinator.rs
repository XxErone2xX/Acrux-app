//! Main PC's screen ownership and feature scheduling. Native IO stays in the worker.
use crate::{
    activities::{self, Activity, MerchantWatch},
    biome,
    items::{self, ItemUse},
    macro_step::Step,
    ocr::Word,
    vision::{Frame, Rect},
};
use serde_json::{json, Value};
use std::collections::{BTreeMap, VecDeque};

pub(crate) const FEATURES: &[&str] = &[
    "mpop", "mfish", "mitem", "mmerch", "mcraft", "mmatch", "mblood",
];

#[derive(Clone, Copy)]
pub(crate) struct Observation {
    pub region: Rect,
    pub ocr: bool,
    pub scale: u32,
    pub pad: u32,
    pub token: u64,
    pub background: bool,
}
enum Purpose {
    Job,
    Merchant(bool),
    Craft,
}
enum Runner {
    Items(ItemUse),
    Activity(Activity),
}
impl Runner {
    fn tick(&mut self, now: f64, frame: Option<&Frame<'_>>, words: &[Word]) -> Step {
        match self {
            Self::Items(j) => j.tick(now, frame, words),
            Self::Activity(j) => j.tick(now, frame, words),
        }
    }
    fn ack(&mut self, ok: bool, now: f64) {
        match self {
            Self::Items(j) => j.ack(ok, now),
            Self::Activity(j) => j.ack(ok, now),
        }
    }
    fn status(&self) -> Value {
        match self {
            Self::Items(j) => j.status(),
            Self::Activity(j) => j.status(),
        }
    }
    fn logs(&mut self) -> Vec<String> {
        match self {
            Self::Items(j) => j.take_logs(),
            Self::Activity(j) => j.take_logs(),
        }
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Hold {
    FinishCatch,
    Urgent,
    Replace,
}
struct Active {
    feature: &'static str,
    runner: Runner,
    hold: Hold,
    ready: bool,
    manual: bool,
    calibration: bool,
    cleanup: Option<u8>,
    cleanup_due: f64,
}
struct Pending {
    name: String,
    at: f64,
}
struct Urgent {
    feature: &'static str,
    biome: String,
    at: f64,
}

pub(crate) struct Coordinator {
    pub running: bool,
    active: Option<Active>,
    request: Option<(Observation, Purpose)>,
    next_token: u64,
    awaiting_input: bool,
    awaiting_cleanup: bool,
    traveling: bool,
    states: BTreeMap<String, Value>,
    merchant_watch: MerchantWatch,
    merchant_pending: Option<Pending>,
    craft_pending: Option<Pending>,
    craft_seen: BTreeMap<String, f64>,
    merchant_at: f64,
    craft_at: f64,
    memory_at: f64,
    item_retry: f64,
    item_last: BTreeMap<String, f64>,
    item_used: BTreeMap<String, u64>,
    last_biome: Option<(i64, String)>,
    blood_seen: Option<(i64, String)>,
    urgent: Option<Urgent>,
    preempted: bool,
    released: Option<bool>,
    patch: Option<Value>,
    logs: VecDeque<String>,
}
impl Default for Coordinator {
    fn default() -> Self {
        Self {
            running: false,
            active: None,
            request: None,
            next_token: 0,
            awaiting_input: false,
            awaiting_cleanup: false,
            traveling: false,
            states: BTreeMap::new(),
            merchant_watch: MerchantWatch::new(false),
            merchant_pending: None,
            craft_pending: None,
            craft_seen: BTreeMap::new(),
            merchant_at: f64::NEG_INFINITY,
            craft_at: f64::NEG_INFINITY,
            memory_at: 0.,
            item_retry: 0.,
            item_last: BTreeMap::new(),
            item_used: BTreeMap::new(),
            last_biome: None,
            blood_seen: None,
            urgent: None,
            preempted: false,
            released: None,
            patch: None,
            logs: VecDeque::new(),
        }
    }
}
impl Coordinator {
    pub fn active(&self) -> bool {
        self.active.is_some() || self.request.is_some() || self.running
    }
    pub fn has_job(&self) -> bool {
        self.active.is_some()
    }
    pub fn feature(&self) -> Option<&'static str> {
        self.active.as_ref().map(|a| a.feature)
    }
    pub fn traveling(&self) -> bool {
        self.traveling
    }
    pub fn waiting_input(&self) -> bool {
        self.awaiting_input
    }
    pub fn observation(&self) -> Option<Observation> {
        self.request.as_ref().map(|(r, _)| *r)
    }
    pub fn hold(&self) -> Option<Hold> {
        self.active.as_ref().map(|a| a.hold)
    }
    pub fn ready(&mut self) {
        if let Some(active) = self.active.as_mut() {
            active.ready = true;
        }
    }
    pub fn is_ready(&self) -> bool {
        self.active.as_ref().is_none_or(|a| a.ready)
    }
    pub fn take_preempted(&mut self) -> bool {
        std::mem::take(&mut self.preempted)
    }
    pub fn take_release(&mut self) -> Option<bool> {
        self.released.take()
    }
    pub fn take_patch(&mut self) -> Option<Value> {
        self.patch.take()
    }
    pub fn take_logs(&mut self) -> Vec<String> {
        self.logs.drain(..).collect()
    }
    pub fn cleanup_navigation(&mut self) {
        if let Some(active) = self.active.as_mut() {
            active.cleanup = Some(0);
        }
    }
    fn log(&mut self, text: impl Into<String>) {
        self.logs.push_back(text.into());
        while self.logs.len() > 300 {
            self.logs.pop_front();
        }
    }
    pub fn start(&mut self, config: &Value, at: f64) -> Result<(), String> {
        if self.active.is_some() {
            return Err("실행 중인 작업을 먼저 중지하세요.".into());
        }
        if !FEATURES.iter().any(|f| enabled(config, f)) {
            return Err("실행할 기능을 켜 주세요.".into());
        }
        self.running = true;
        self.last_biome = biome::status().history.last().cloned();
        self.merchant_at = at - 15.;
        self.craft_at = at - 2.;
        self.log("매크로 켜짐");
        Ok(())
    }
    pub fn stop_all(&mut self, at: f64) {
        self.running = false;
        self.cancel_current("중지됨", at);
        self.request = None;
        self.urgent = None;
        self.merchant_pending = None;
        self.craft_pending = None;
        self.log("매크로 꺼짐");
    }
    pub fn stop_feature(&mut self, feature: &str, at: f64) {
        if self.active.as_ref().is_some_and(|a| a.feature == feature) {
            self.cancel_current("중지됨", at);
        }
        match feature {
            "mmerch" => self.merchant_pending = None,
            "mcraft" => self.craft_pending = None,
            _ => {}
        }
    }
    fn install(
        &mut self,
        feature: &'static str,
        runner: Runner,
        hold: Hold,
        manual: bool,
        calibration: bool,
    ) {
        let state = self
            .states
            .entry(feature.into())
            .or_insert_with(|| json!({}));
        state["running"] = true.into();
        state["msg"] = "낚시가 끝나기를 기다리는 중".into();
        state["error"] = Value::Null;
        self.active = Some(Active {
            feature,
            runner,
            hold,
            ready: false,
            manual,
            calibration,
            cleanup: None,
            cleanup_due: 0.,
        });
        self.request = None;
        self.awaiting_input = false;
        self.traveling = false;
    }
    pub fn action(
        &mut self,
        action: &str,
        payload: &Value,
        config: &Value,
        at: f64,
    ) -> Result<(), String> {
        if action == "start" {
            return self.start(config, at);
        }
        if action == "stop" {
            self.stop_all(at);
            return Ok(());
        }
        if let Some(feature) = action
            .strip_suffix("_stop")
            .filter(|f| FEATURES.contains(f))
        {
            self.stop_feature(feature, at);
            return Ok(());
        }
        if self.active.is_some() {
            return Err("다른 동작이 도는 중입니다.".into());
        }
        if action == "mmerch_check" {
            let region = read_region(&config["mmerch"]["chat_region"])
                .ok_or("채팅 글자 영역을 지정하세요.")?;
            self.merchant_watch = MerchantWatch::new(true);
            let state = self
                .states
                .entry("mmerch".into())
                .or_insert_with(|| json!({}));
            state["running"] = true.into();
            state["msg"] = "채팅 확인".into();
            state["job"] = "check".into();
            state["error"] = Value::Null;
            self.observe(region, true, 1, 0, Purpose::Merchant(true));
            return Ok(());
        }
        let (feature, runner, hold, calibration) = match action {
            "mitem_test" => {
                let keys = items::TIMED_ITEMS
                    .iter()
                    .filter(|(key, _, _)| config["mitem"][*key].as_bool().unwrap_or(true))
                    .map(|(key, _, _)| (*key).to_owned())
                    .collect::<Vec<_>>();
                if keys.is_empty() {
                    return Err("사용할 아이템이 꺼져 있습니다.".into());
                }
                (
                    "mitem",
                    Runner::Items(ItemUse::new(config, "mitem", None, &keys, true, at)?),
                    Hold::FinishCatch,
                    false,
                )
            }
            "mpop_test" => {
                let name = payload
                    .get("biome")
                    .or_else(|| payload.get("name"))
                    .and_then(Value::as_str)
                    .unwrap_or("CYBERSPACE");
                (
                    "mpop",
                    Runner::Items(ItemUse::new(config, "mpop", Some(name), &[], true, at)?),
                    Hold::Urgent,
                    false,
                )
            }
            "mblood_test" => (
                "mblood",
                Runner::Items(ItemUse::new(
                    config,
                    "mblood",
                    Some("PUMPKINMOON"),
                    &[],
                    true,
                    at,
                )?),
                Hold::Urgent,
                false,
            ),
            "mmerch_buy_test" => {
                let name = payload["name"]
                    .as_str()
                    .filter(|n| ["Mari", "Jester", "Rin"].contains(n))
                    .unwrap_or("Mari");
                (
                    "mmerch",
                    Runner::Activity(Activity::merchant(config, name, at)?),
                    Hold::Replace,
                    false,
                )
            }
            "mcraft_test" => {
                let name = payload["name"]
                    .as_str()
                    .filter(|n| !n.trim().is_empty())
                    .or_else(|| config["mcraft"]["potion"].as_str())
                    .unwrap_or("");
                (
                    "mcraft",
                    Runner::Activity(Activity::craft(config, name, at)?),
                    Hold::Replace,
                    false,
                )
            }
            "mmatch_test" => (
                "mmatch",
                Runner::Activity(Activity::memory(config, true, at)?),
                Hold::Replace,
                false,
            ),
            "mmerch_autocal" => (
                "mmerch",
                Runner::Activity(Activity::calibrate("mmerch", config, at)?),
                Hold::Replace,
                true,
            ),
            "mcraft_autocal" => (
                "mcraft",
                Runner::Activity(Activity::calibrate("mcraft", config, at)?),
                Hold::Replace,
                true,
            ),
            "mmatch_autocal" => (
                "mmatch",
                Runner::Activity(Activity::calibrate("mmatch", config, at)?),
                Hold::Replace,
                true,
            ),
            "mmatch_boardcal" => (
                "mmatch",
                Runner::Activity(Activity::calibrate("mmatch_board", config, at)?),
                Hold::Replace,
                true,
            ),
            "sell_autocal" | "mfish_sell_autocal" => (
                "mfish",
                Runner::Activity(Activity::calibrate("sell", config, at)?),
                Hold::Replace,
                true,
            ),
            _ => return Err("지원하지 않는 매크로 동작입니다.".into()),
        };
        self.install(feature, runner, hold, true, calibration);
        Ok(())
    }
    fn observe(&mut self, region: Rect, ocr: bool, scale: u32, pad: u32, purpose: Purpose) {
        self.next_token = self.next_token.wrapping_add(1);
        let background = !matches!(&purpose, Purpose::Job);
        self.request = Some((
            Observation {
                region,
                ocr,
                scale,
                pad,
                token: self.next_token,
                background,
            },
            purpose,
        ));
    }
    pub fn poll(&mut self, config: &Value, biome: &biome::Status, at: f64) {
        if self
            .merchant_pending
            .as_ref()
            .is_some_and(|p| at - p.at > 180.)
        {
            self.merchant_pending = None;
        }
        if self
            .craft_pending
            .as_ref()
            .is_some_and(|p| at - p.at > 300.)
        {
            self.craft_pending = None;
        }
        if self
            .active
            .as_ref()
            .is_some_and(|a| !a.manual && (!self.running || !enabled(config, a.feature)))
        {
            self.cancel_current("기능이 꺼져 중지했습니다.", at);
        }
        if !self.running {
            return;
        }
        let occurrence = biome.history.last().cloned();
        let own = biome.watching && biome.owner == "ok";
        if occurrence != self.last_biome {
            self.last_biome = occurrence.clone();
            if own && enabled(config, "mpop") {
                if let Some((_, name)) = &occurrence {
                    if items::POP_BIOMES.contains(&name.as_str())
                        && config["mpop"]["biomes_on"][name].as_bool().unwrap_or(true)
                    {
                        self.urgent = Some(Urgent {
                            feature: "mpop",
                            biome: name.clone(),
                            at,
                        });
                    }
                }
            }
        }
        if own
            && enabled(config, "mblood")
            && self.urgent.is_none()
            && !self
                .active
                .as_ref()
                .is_some_and(|a| a.feature == "mpop" || a.feature == "mblood")
        {
            if let Some((_, name)) = &occurrence {
                if items::BLOOD_BIOMES.contains(&name.as_str()) && occurrence != self.blood_seen {
                    self.blood_seen = occurrence.clone();
                    self.urgent = Some(Urgent {
                        feature: "mblood",
                        biome: name.clone(),
                        at,
                    });
                }
            }
        }
        if let Some(urgent) = self.urgent.as_ref() {
            if !enabled(config, urgent.feature)
                || (urgent.feature == "mblood"
                    && biome.current.as_deref() != Some(urgent.biome.as_str()))
            {
                self.urgent = None;
                return;
            }
            let delay = finite(&config[urgent.feature]["start_delay"])
                .unwrap_or(1.)
                .max(0.);
            if at - urgent.at < delay {
                return;
            }
            // Preserve an ad already on screen until its own native state machine closes it.
            if self.active.as_ref().is_some_and(|a| {
                a.feature == "mmatch"
                    && a.runner.status()["msg"]
                        .as_str()
                        .is_some_and(|m| m.starts_with("광고"))
            }) {
                return;
            }
            let urgent = self.urgent.take().unwrap();
            let cleanup = self.active.is_some() || self.traveling;
            if self.active.is_some() {
                self.cancel_current("긴급 팝핑을 먼저 실행합니다.", at);
                self.preempted = true;
            }
            match ItemUse::new(config, urgent.feature, Some(&urgent.biome), &[], false, at) {
                Ok(job) => {
                    self.install(
                        urgent.feature,
                        Runner::Items(job),
                        Hold::Urgent,
                        false,
                        false,
                    );
                    if cleanup {
                        self.active.as_mut().unwrap().cleanup = Some(0);
                    }
                    self.log(format!("{} — {} 시작", urgent.biome, urgent.feature));
                }
                Err(error) => self.failure(urgent.feature, error, at),
            }
        }
        if self.active.is_some() || self.request.is_some() {
            return;
        }
        // These are deliberately the original fixed priorities, not a new queue order.
        let job = if enabled(config, "mmerch") && self.merchant_pending.is_some() {
            let p = self.merchant_pending.take().unwrap();
            Some(("mmerch", Activity::merchant(config, &p.name, at)))
        } else if enabled(config, "mcraft") && self.craft_pending.is_some() {
            let p = self.craft_pending.take().unwrap();
            Some(("mcraft", Activity::craft(config, &p.name, at)))
        } else if enabled(config, "mmatch") && at >= self.memory_at {
            self.memory_at = at + 600.;
            Some(("mmatch", Activity::memory(config, false, at)))
        } else {
            None
        };
        if let Some((feature, result)) = job {
            match result {
                Ok(job) => {
                    self.install(feature, Runner::Activity(job), Hold::Replace, false, false)
                }
                Err(error) => self.failure(feature, error, at),
            }
            return;
        }
        if enabled(config, "mitem") && at >= self.item_retry {
            let keys = items::TIMED_ITEMS
                .iter()
                .filter(|(key, _, minutes)| {
                    config["mitem"][*key].as_bool().unwrap_or(true)
                        && self.item_last.get(*key).is_none_or(|last| {
                            at - last
                                >= finite(&config["mitem"][format!("{key}_min")])
                                    .unwrap_or(*minutes)
                                    .clamp(1., 240.)
                                    * 60.
                        })
                })
                .map(|(key, _, _)| (*key).to_owned())
                .collect::<Vec<_>>();
            if !keys.is_empty() {
                match ItemUse::new(config, "mitem", None, &keys, false, at) {
                    Ok(job) => {
                        self.install("mitem", Runner::Items(job), Hold::FinishCatch, false, false)
                    }
                    Err(error) => {
                        self.item_retry = at + 60.;
                        self.failure("mitem", error, at);
                    }
                }
                return;
            }
        }
        if enabled(config, "mmerch")
            && at - self.merchant_at
                >= finite(&config["mmerch"]["check_sec"])
                    .unwrap_or(15.)
                    .max(1.)
        {
            self.merchant_at = at;
            if let Some(region) = read_region(&config["mmerch"]["chat_region"]) {
                self.observe(region, true, 1, 0, Purpose::Merchant(false));
            } else {
                self.failure("mmerch", "채팅 글자 영역을 지정하세요.".into(), at);
            }
            return;
        }
        if enabled(config, "mcraft") && self.craft_pending.is_none() && at - self.craft_at >= 2. {
            self.craft_at = at;
            if let Some(region) = read_region(&config["base"]["notice_region"]) {
                self.observe(region, true, 1, 0, Purpose::Craft);
            } else {
                self.failure("mcraft", "공통 위치의 알림 영역을 지정하세요.".into(), at);
            }
        }
    }
    pub fn tick(&mut self, at: f64) -> Option<Step> {
        if self.request.is_some() || self.awaiting_input || self.traveling {
            return None;
        }
        let active = self.active.as_mut()?;
        if !active.ready {
            return None;
        }
        if let Some(index) = active.cleanup {
            if at < active.cleanup_due {
                return None;
            }
            let keys = match index {
                0 => &[0x1b][..],
                1 => &[0x52][..],
                2 => &[0x0d][..],
                _ => {
                    active.cleanup = None;
                    return self.tick(at);
                }
            };
            self.awaiting_cleanup = true;
            self.awaiting_input = true;
            return Some(Step::Input(crate::navigation::Motion::Keys(keys, 40)));
        }
        let step = active.runner.tick(at, None, &[]);
        self.process(step, at)
    }
    fn process(&mut self, step: Step, at: f64) -> Option<Step> {
        match step {
            Step::Observe { region, ocr } => {
                let (scale, pad) = self
                    .active
                    .as_ref()
                    .and_then(|a| {
                        if let Runner::Activity(j) = &a.runner {
                            Some(j.ocr_options())
                        } else {
                            None
                        }
                    })
                    .unwrap_or((1, 0));
                self.observe(
                    region.unwrap_or(Rect(0., 0., 1., 1.)),
                    ocr,
                    scale,
                    pad,
                    Purpose::Job,
                );
                None
            }
            Step::Wait(_) => None,
            Step::Input(_) | Step::Text { .. } => {
                self.awaiting_input = true;
                Some(step)
            }
            Step::Travel { .. } => {
                self.traveling = true;
                Some(step)
            }
            Step::Done => {
                self.finish(None, at);
                Some(Step::Done)
            }
            Step::Failed(error) => {
                self.finish(Some(error.clone()), at);
                Some(Step::Failed(error))
            }
        }
    }
    pub fn observed(
        &mut self,
        token: u64,
        config: &Value,
        at: f64,
        frame: &Frame<'_>,
        words: Result<Vec<Word>, String>,
    ) -> Option<Step> {
        if self.request.as_ref().is_none_or(|(r, _)| r.token != token) {
            return None;
        }
        let (_, purpose) = self.request.take().unwrap();
        let words = match words {
            Ok(words) => words,
            Err(error) => {
                match purpose {
                    Purpose::Job => {
                        self.finish(Some(error.clone()), at);
                        return Some(Step::Failed(error));
                    }
                    Purpose::Merchant(manual) => {
                        self.failure("mmerch", error, at);
                        if manual {
                            self.states.get_mut("mmerch").unwrap()["job"] = Value::Null;
                        }
                    }
                    Purpose::Craft => self.failure("mcraft", error, at),
                }
                return None;
            }
        };
        match purpose {
            Purpose::Job => {
                let active = self.active.as_mut()?;
                let step = active.runner.tick(at, Some(frame), &words);
                self.process(step, at)
            }
            Purpose::Merchant(manual) => {
                let found = self.merchant_watch.check(&words, at);
                let state = self
                    .states
                    .entry("mmerch".into())
                    .or_insert_with(|| json!({}));
                state["running"] = false.into();
                state["msg"] = found
                    .as_ref()
                    .map_or("상인 도착 없음".to_owned(), |n| {
                        format!("상인 도착 — {n}")
                    })
                    .into();
                state["job"] = Value::Null;
                state["error"] = Value::Null;
                state["completedAt"] = at.into();
                if let Some(name) = found {
                    if config["mmerch"]["buy"].as_object().is_some_and(|buy| {
                        buy.keys().any(|key| key.starts_with(&format!("{name}_")))
                    }) {
                        if manual {
                            match Activity::merchant(config, &name, at) {
                                Ok(job) => self.install(
                                    "mmerch",
                                    Runner::Activity(job),
                                    Hold::Replace,
                                    true,
                                    false,
                                ),
                                Err(error) => self.failure("mmerch", error, at),
                            }
                        } else {
                            self.merchant_pending = Some(Pending { name, at });
                        }
                    }
                }
                None
            }
            Purpose::Craft => {
                if let Some(name) = activities::craft_notice(config, frame, &words) {
                    if self
                        .craft_seen
                        .get(&name)
                        .is_none_or(|last| at - last > 60.)
                    {
                        self.craft_seen.retain(|_, last| at - *last < 600.);
                        self.craft_seen.insert(name.clone(), at);
                        self.log(format!("Auto Crafted 알림 — {name}"));
                        self.craft_pending = Some(Pending { name, at });
                    }
                }
                None
            }
        }
    }
    pub fn ack(&mut self, ok: bool, at: f64) {
        if !self.awaiting_input {
            return;
        }
        self.awaiting_input = false;
        if self.awaiting_cleanup {
            self.awaiting_cleanup = false;
            if let Some(active) = self.active.as_mut() {
                if ok {
                    active.cleanup = active.cleanup.map(|i| i + 1);
                    active.cleanup_due = at + 0.5;
                } else {
                    self.finish(Some("열린 게임 창을 정리하지 못했습니다.".into()), at);
                }
            }
            return;
        }
        if let Some(active) = self.active.as_mut() {
            active.runner.ack(ok, at);
        }
    }
    pub fn travel_done(&mut self, ok: bool, at: f64) {
        self.traveling = false;
        if let Some(active) = self.active.as_mut() {
            active.runner.ack(ok, at);
        }
    }
    pub fn fail_current(&mut self, error: String, at: f64) {
        self.finish(Some(error), at);
    }
    fn cancel_current(&mut self, message: &str, at: f64) {
        if self.active.is_some() {
            self.finish(Some(message.into()), at);
        }
        self.request = None;
        self.awaiting_input = false;
        self.awaiting_cleanup = false;
        self.traveling = false;
    }
    fn failure(&mut self, feature: &str, error: String, at: f64) {
        self.log(error.clone());
        let state = self
            .states
            .entry(feature.into())
            .or_insert_with(|| json!({}));
        state["running"] = false.into();
        state["msg"] = error.clone().into();
        state["error"] = error.into();
        state["completedAt"] = at.into();
    }
    fn finish(&mut self, error: Option<String>, at: f64) {
        let Some(mut active) = self.active.take() else {
            return;
        };
        let mut status = active.runner.status();
        let previous = self
            .states
            .get(active.feature)
            .cloned()
            .unwrap_or_else(|| json!({}));
        for counter in ["bought", "crafted", "played", "ads", "used"] {
            if let Some(current) = status[counter].as_u64() {
                status[counter] = (previous[counter].as_u64().unwrap_or(0) + current).into();
            }
        }
        match &mut active.runner {
            Runner::Items(job) => {
                for record in job.completed() {
                    self.item_last.insert(record.key.clone(), record.at);
                    *self.item_used.entry(record.key.clone()).or_default() += 1;
                }
                if active.feature == "mitem" {
                    self.item_retry = job.retry_at().unwrap_or(at);
                }
            }
            Runner::Activity(job) => {
                if let Some(next) = job.next_at() {
                    self.memory_at = next.max(if error.is_some() { at + 600. } else { at });
                }
                self.patch = job.config_patch();
            }
        }
        for line in active.runner.logs() {
            self.log(line);
        }
        status["running"] = false.into();
        status["completedAt"] = at.into();
        status["error"] = error.clone().map_or(Value::Null, Value::String);
        if let Some(error) = error {
            status["msg"] = error.clone().into();
            self.log(error);
        }
        self.states.insert(active.feature.into(), status);
        self.released = Some(active.hold == Hold::Replace && !active.calibration);
        self.request = None;
        self.awaiting_input = false;
        self.awaiting_cleanup = false;
        self.traveling = false;
    }
    pub fn statuses(&self, config: &Value, at: f64) -> Value {
        let mut result = json!({});
        for &feature in FEATURES {
            result[feature] = self
                .states
                .get(feature)
                .cloned()
                .unwrap_or_else(|| json!({"running":false,"msg":"대기","error":null}));
        }
        if let Some(active) = &self.active {
            let mut state = active.runner.status();
            if !active.ready {
                state["msg"] = if active.hold == Hold::FinishCatch {
                    "현재 낚시가 끝나기를 기다리는 중"
                } else {
                    "낚시를 멈추고 실행 준비 중"
                }
                .into();
            }
            state["running"] = true.into();
            state["error"] = Value::Null;
            let previous = self.states.get(active.feature);
            for counter in ["bought", "crafted", "played", "ads", "used"] {
                if let Some(current) = state[counter].as_u64() {
                    state[counter] =
                        (current + previous.and_then(|s| s[counter].as_u64()).unwrap_or(0)).into();
                }
            }
            result[active.feature] = state;
        }
        if matches!(
            self.request.as_ref().map(|(_, p)| p),
            Some(Purpose::Merchant(true))
        ) {
            result["mmerch"]["running"] = true.into();
            result["mmerch"]["msg"] = "채팅 확인".into();
        }
        result["mmatch"]["next_in"] = (self.memory_at - at).max(0.).into();
        for &(key, _, default) in items::TIMED_ITEMS {
            let interval = finite(&config["mitem"][format!("{key}_min")])
                .unwrap_or(default)
                .clamp(1., 240.)
                * 60.;
            let deadline = self
                .item_last
                .get(key)
                .map_or(at, |last| last + interval)
                .max(self.item_retry);
            if !result["mitem"]["items"].is_object() {
                result["mitem"]["items"] = json!({});
            }
            result["mitem"]["items"][key] = json!({"on":config["mitem"][key].as_bool().unwrap_or(true),"used":self.item_used.get(key).copied().unwrap_or(0),"left":(deadline-at).max(0.)});
        }
        result
    }
    pub fn message(&self) -> Option<String> {
        self.active.as_ref().map(|active| {
            if !active.ready {
                "낚시가 끝나기를 기다리는 중".into()
            } else {
                active.runner.status()["msg"]
                    .as_str()
                    .unwrap_or("실행 중")
                    .into()
            }
        })
    }
}
fn enabled(config: &Value, feature: &str) -> bool {
    config[feature]["enabled"].as_bool().unwrap_or(false)
}
fn finite(value: &Value) -> Option<f64> {
    value.as_f64().filter(|n| n.is_finite())
}
fn read_region(value: &Value) -> Option<Rect> {
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
        .all(|n| n.is_finite() && (0.0..=1.).contains(n))
        && r.0 < r.2
        && r.1 < r.3)
        .then_some(r)
}

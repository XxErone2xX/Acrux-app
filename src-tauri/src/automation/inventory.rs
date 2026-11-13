use super::*;

#[derive(Clone, Debug)]
pub(super) struct Item {
    pub name: String,
    pub amount: Option<u64>,
    pub min_have: u64,
}
#[derive(Default, Clone)]
pub(super) struct Report {
    pub used: BTreeMap<String, u64>,
    pub inspected: BTreeMap<String, u64>,
    pub attempted: Vec<String>,
    pub logs: Vec<String>,
}

pub(super) struct Inventory {
    items: Vec<Item>,
    one: bool,
    dry: bool,
    close: bool,
    phase: u8,
    index: usize,
    retry: u8,
    due: f64,
    deadline: f64,
    count: u64,
    amount: u64,
    pub report: Report,
}
impl Inventory {
    pub(super) fn new(
        cfg: &Config,
        items: Vec<Item>,
        one: bool,
        dry: bool,
        close: bool,
    ) -> Result<Self, String> {
        for key in [
            "inventory_pos",
            "items_pos",
            "search_pos",
            "item_pos",
            "amount_pos",
            "use_pos",
        ] {
            if cfg.point("base", key).is_none() {
                return Err(format!("공통 인벤토리 {key} 위치가 필요합니다."));
            }
        }
        if cfg.rect("base", "ocr_region").is_none() {
            return Err("공통 인벤토리 OCR 영역이 필요합니다.".into());
        }
        if items.len() > 64
            || items.iter().any(|i| {
                i.name.trim().is_empty()
                    || i.name.len() > 256
                    || i.min_have == 0
                    || i.amount == Some(0)
            })
        {
            return Err("아이템 이름·수량·최소 보유량을 확인하세요.".into());
        }
        Ok(Self {
            items,
            one,
            dry,
            close,
            phase: 0,
            index: 0,
            retry: 0,
            due: 0.0,
            deadline: 0.0,
            count: 0,
            amount: 0,
            report: Report::default(),
        })
    }
    pub(super) fn tick(
        &mut self,
        cfg: &Config,
        now: f64,
        geometry: (usize, usize),
        frame: Option<&Frame<'_>>,
        words: &[Word],
    ) -> Tick {
        if now < self.due {
            return wait(self.due - now);
        }
        if self.items.is_empty() {
            return Tick::Done;
        }
        if self.phase == 0 {
            if frame.is_none() {
                return observe(true, None);
            }
            self.deadline = now + 10.0;
            if find(words, &["Search"], true).is_some() && find(words, &["Items"], true).is_some() {
                self.phase = 3;
            } else {
                return self.click(cfg, "inventory_pos", 1, now, cfg.delay("inventory"));
            }
        }
        if self.phase == 1 {
            if frame.is_none() {
                return observe(true, None);
            }
            if find(words, &["Inventory", "Items", "Search"], false).is_none() {
                if now > self.deadline {
                    return Tick::Stopped("인벤토리가 열렸는지 확인하지 못했습니다.".into());
                }
                return observe(true, None);
            }
            return self.click(cfg, "items_pos", 2, now, cfg.delay("items"));
        }
        if self.phase == 2 {
            if frame.is_none() {
                return observe(true, None);
            }
            if find(words, &["Search"], false).is_none() {
                if now > self.deadline {
                    return Tick::Stopped("Items 검색 화면을 확인하지 못했습니다.".into());
                }
                return observe(true, None);
            }
            self.phase = 3;
        }
        if self.index >= self.items.len() {
            return self.finish(cfg, now, frame, words);
        }
        let item = self.items[self.index].clone();
        match self.phase {
            3 => self.click(cfg, "search_pos", 4, now, cfg.delay("search")),
            4 => {
                self.phase = 5;
                self.due = now + cfg.delay("typing");
                text(item.name.clone())
            }
            5 => {
                if frame.is_none() {
                    return observe(
                        true,
                        cfg.rect("base", "ocr_region")
                            .and_then(|r| r.region(geometry)),
                    );
                }
                let raw = read(words, cfg.rect("base", "ocr_region"));
                let (name, count) = parse(&raw);
                let score = similarity(&name, &item.name);
                if score < cfg.number("pop", "match_threshold", 70.0) || count.is_none() {
                    self.retry += 1;
                    if self.retry < 2 {
                        self.phase = 3;
                        return self.tick(cfg, now, geometry, None, &[]);
                    }
                    self.report.logs.push(format!(
                        "{} 검색 확인 실패 · OCR '{raw}' · 일치율 {score:.1}% · 사용 안 함",
                        item.name
                    ));
                    self.advance();
                    return self.tick(cfg, now, geometry, frame, words);
                }
                let count = count.unwrap();
                self.report.inspected.insert(item.name.clone(), count);
                if count < item.min_have {
                    self.report.logs.push(format!(
                        "{} 보유 {count}개 · 최소 {}개 미달 · 사용 안 함",
                        item.name, item.min_have
                    ));
                    self.advance();
                    return self.tick(cfg, now, geometry, frame, words);
                }
                self.count = count;
                self.amount = item.amount.unwrap_or(count).min(count);
                if self.dry {
                    self.report.logs.push(format!(
                        "{} 보유 {count}개 · 검사만 수행하고 사용하지 않음",
                        item.name
                    ));
                    self.advance();
                    return self.tick(cfg, now, geometry, frame, words);
                }
                self.click(cfg, "item_pos", 6, now, cfg.delay("item_click"))
            }
            6 => self.click(cfg, "amount_pos", 7, now, cfg.delay("dbl_gap")),
            7 => self.click(cfg, "amount_pos", 8, now, cfg.delay("after_dbl")),
            8 => {
                self.phase = 9;
                self.due = now + cfg.delay("after_enter");
                text(self.amount.to_string())
            }
            9 => {
                self.report.attempted.push(item.name);
                self.deadline = now + 3.0;
                self.click(cfg, "use_pos", 10, now, cfg.delay("after_enter"))
            }
            10 => {
                if frame.is_none() {
                    return observe(true, None);
                }
                let raw = read(words, cfg.rect("base", "ocr_region"));
                let (name, count) = parse(&raw);
                let reduced = similarity(&name, &item.name)
                    >= cfg.number("pop", "match_threshold", 70.0)
                    && count.is_some_and(|n| n <= self.count.saturating_sub(self.amount));
                let empty = self.amount == self.count
                    && find(words, &["No items found", "No results"], false).is_some();
                let notice = words.iter().any(|w| {
                    let n = ocr_norm(&w.text);
                    (n.contains("used") || n.contains("octivoted"))
                        && similarity(&w.text, &item.name) >= 40.0
                });
                if reduced || empty || notice {
                    *self.report.used.entry(item.name.clone()).or_default() += self.amount;
                    self.report
                        .logs
                        .push(format!("{} {}개 사용 결과 확인", item.name, self.amount));
                    if self.one {
                        self.index = self.items.len();
                    } else {
                        self.advance();
                    }
                    self.phase = 3;
                    return self.tick(cfg, now, geometry, frame, words);
                }
                if now >= self.deadline {
                    return Tick::Stopped(format!("{} 사용 입력 뒤 수량/알림 변화를 확인하지 못했습니다. 중복 사용하지 않습니다.",item.name));
                }
                self.due = now + 0.2;
                observe(true, None)
            }
            _ => self.finish(cfg, now, frame, words),
        }
    }
    fn advance(&mut self) {
        self.index += 1;
        self.retry = 0;
        self.phase = 3;
    }
    fn click(&mut self, cfg: &Config, key: &str, next: u8, now: f64, delay: f64) -> Tick {
        let Some(at) = cfg.point("base", key) else {
            return Tick::Stopped(format!("인벤토리 {key} 위치가 없습니다."));
        };
        self.phase = next;
        self.due = now + delay;
        tap(at, 40)
    }
    fn finish(
        &mut self,
        cfg: &Config,
        now: f64,
        frame: Option<&Frame<'_>>,
        words: &[Word],
    ) -> Tick {
        if !self.close {
            return Tick::Done;
        }
        if frame.is_none() {
            return observe(true, None);
        }
        let visible = find(words, &["Search"], false).is_some();
        if !visible {
            return Tick::Done;
        }
        if self.phase != 12 {
            self.deadline = now + 3.0;
            return self.click(cfg, "inventory_pos", 12, now, 0.35);
        }
        if now >= self.deadline {
            return Tick::Stopped("인벤토리가 닫혔는지 확인하지 못했습니다.".into());
        }
        observe(true, None)
    }
}

pub(super) fn parse(text: &str) -> (String, Option<u64>) {
    let raw = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let split = raw
        .char_indices()
        .rev()
        .find(|(_, c)| matches!(c, 'x' | 'X' | '×'))
        .map(|(i, _)| i);
    if let Some(at) = split {
        let tail = &raw[at..];
        let digits: String = tail
            .chars()
            .skip(1)
            .filter_map(|c| match c {
                '0'..='9' => Some(c),
                'l' | 'I' | 'i' | '|' => Some('1'),
                'O' | 'o' => Some('0'),
                ',' | '.' | ' ' => None,
                _ => Some('?'),
            })
            .collect();
        if !digits.is_empty() && !digits.contains('?') {
            return (raw[..at].trim().to_string(), digits.parse().ok());
        }
    }
    let count = (!raw.chars().any(|c| c.is_ascii_digit())).then_some(1);
    (raw, count)
}

pub(super) fn template(cfg: &Config, biome: &str) -> Result<(Vec<Item>, bool), String> {
    let template = &cfg.group("mpop")["templates"][biome];
    let mut items = Vec::new();
    for it in template["items"].as_array().into_iter().flatten() {
        let name = it["name"].as_str().unwrap_or("").trim();
        if name.is_empty() {
            continue;
        }
        let amount = if it["amount"]
            .as_str()
            .is_some_and(|s| s.eq_ignore_ascii_case("ALL"))
        {
            None
        } else {
            Some(
                it["amount"]
                    .as_u64()
                    .or_else(|| it["amount"].as_str().and_then(|s| s.parse().ok()))
                    .unwrap_or(1),
            )
        };
        let min_have = it["min_have"].as_u64().unwrap_or(1).max(1);
        items.push(Item {
            name: name.into(),
            amount,
            min_have,
        });
    }
    if items.len() > 64 {
        return Err("한 템플릿의 아이템은 최대 64개입니다.".into());
    }
    Ok((items, template["mode"].as_str() == Some("one")))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn inventory_count_and_name_are_checked_before_consumption() {
        assert_eq!(
            parse("Warp Potion x1,234"),
            ("Warp Potion".into(), Some(1234))
        );
        assert_eq!(parse("Warp Potion xlO"), ("Warp Potion".into(), Some(10)));
        assert_eq!(
            parse("Heavenly Potion II"),
            ("Heavenly Potion II".into(), Some(1))
        );
        assert_eq!(similarity("Super Warp Potion", "Warp Potion"), 0.0);
        assert_eq!(similarity("Warp Potion X", "Warp Potion"), 0.0);
        assert_eq!(similarity("Warp Potion", "Warp Potion"), 100.0);
    }
}

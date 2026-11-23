use chrono::{SecondsFormat, Utc};
use regex::Regex;
use reqwest::{blocking::Client, redirect::Policy, Url};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::{HashMap, VecDeque},
    sync::{mpsc, LazyLock, Mutex},
    time::{Duration, Instant},
};

const TARGETS: &[&str] = &[
    "WINDY",
    "SNOWY",
    "RAINY",
    "SANDSTORM",
    "HELL",
    "STARFALL",
    "HEAVEN",
    "CORRUPTION",
    "NULL",
    "GLITCHED",
    "DREAMSPACE",
    "CYBERSPACE",
    "SINGULARITY",
    "LOSTFOREST",
    "RADIANTFOREST",
    "PUMPKINMOON",
    "GRAVEYARD",
    "BLOODRAIN",
];
const RARE: &[&str] = &["GLITCHED", "DREAMSPACE", "CYBERSPACE"];
const SAFE_SECONDS: u64 = 60;
static JOIN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"! Joining game '([0-9a-fA-F-]{36})' place (\d+)").unwrap());
static RPC: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#""largeImage"\s*:\s*\{[^}]*"hoverText"\s*:\s*"([^"]+)""#).unwrap()
});
static STATE: LazyLock<Mutex<State>> = LazyLock::new(|| Mutex::new(State::default()));
static QUEUES: LazyLock<Mutex<HashMap<String, mpsc::Sender<Value>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

#[derive(Clone, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    pub enabled: bool,
    pub webhooks: Vec<String>,
    pub ps_link: String,
    pub everyone: bool,
    pub mentions: HashMap<String, bool>,
    pub roles: HashMap<String, String>,
    pub player: String,
    pub safe_mode: bool,
    pub safe_v2: bool,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            enabled: false,
            webhooks: vec![],
            ps_link: String::new(),
            everyone: true,
            mentions: HashMap::new(),
            roles: HashMap::new(),
            player: String::new(),
            safe_mode: true,
            safe_v2: false,
        }
    }
}
impl Settings {
    pub fn normalized(mut self) -> Self {
        if !self.safe_v2 {
            self.safe_mode = true;
            self.safe_v2 = true;
        }
        self
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.webhooks.len() > 32
            || self
                .webhooks
                .iter()
                .any(|url| !url.is_empty() && !is_hook(url))
        {
            return Err("디스코드 웹후크 주소를 확인하세요.".into());
        }
        if self.ps_link.len() > 2048 || self.player.len() > 100 {
            return Err("바이옴 설정이 너무 깁니다.".into());
        }
        Ok(())
    }
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub settings: Settings,
    pub current: Option<String>,
    pub watching: bool,
    pub message: String,
    pub logs: Vec<String>,
    pub history: Vec<(i64, String)>,
    pub owner: String,
    pub job: Option<String>,
    pub safe: bool,
    pub safe_left: u64,
    pub held: Option<String>,
}
struct State {
    settings: Settings,
    current: Option<String>,
    watching: bool,
    message: String,
    primary: i32,
    disconnected: bool,
    announce: bool,
    since: Instant,
    logs: VecDeque<String>,
    history: VecDeque<(i64, String)>,
    owner: String,
    file: Option<String>,
    job: Option<String>,
    job_since: Option<Instant>,
    skip_job: Option<String>,
    skip_file: Option<String>,
    skip_requested: bool,
    held: Option<String>,
    announced: Option<String>,
}
impl Default for State {
    fn default() -> Self {
        Self {
            settings: Settings::default(),
            current: None,
            watching: false,
            message: "꺼짐".into(),
            primary: 0,
            disconnected: false,
            announce: false,
            since: Instant::now(),
            logs: VecDeque::new(),
            history: VecDeque::new(),
            owner: String::new(),
            file: None,
            job: None,
            job_since: None,
            skip_job: None,
            skip_file: None,
            skip_requested: false,
            held: None,
            announced: None,
        }
    }
}
fn lock() -> std::sync::MutexGuard<'static, State> {
    STATE.lock().unwrap_or_else(|e| e.into_inner())
}
pub fn clear_log() {
    lock().logs.clear();
}
pub fn platform_log(message: String) {
    lock().log(message);
}
pub fn settings() -> Settings {
    lock().settings.clone()
}
pub fn status() -> Status {
    let mut state = lock();
    state.release_held();
    let left = state.safe_left();
    Status {
        settings: state.settings.clone(),
        current: state.current.clone(),
        watching: state.watching,
        message: state.message.clone(),
        logs: state.logs.iter().rev().cloned().collect(),
        history: state.history.iter().cloned().collect(),
        owner: state.owner.clone(),
        job: state.job.as_ref().map(|job| job.chars().take(8).collect()),
        safe: state.settings.safe_mode,
        safe_left: left,
        held: state.held.clone().filter(|_| left > 0),
    }
}
pub fn load(settings: Settings) {
    let mut state = lock();
    if settings.validate().is_ok() {
        state.settings = settings.normalized();
    }
    if state.settings.enabled {
        state.message = "로그 연결 대기 중".into();
    }
}
pub fn configure(settings: Settings) {
    let mut state = lock();
    let changed = settings.enabled != state.settings.enabled;
    if settings.player != state.settings.player {
        state.held = None;
    }
    state.settings = settings.normalized();
    if !state.settings.safe_mode {
        state.held = None;
    }
    if !changed {
        return;
    }
    // Main keeps biome tracking alive when only webhook notifications are toggled.
    state.announce = false;
    state.since = Instant::now();
    let enabled = state.settings.enabled;
    state.message = if enabled {
        "로그 연결 대기 중"
    } else {
        "꺼짐"
    }
    .into();
    let payload = state.embed(
        format!(
            "> ## Biome Macro {}\nJoin discord to download macro\nhttps://discord.gg/cc9APrrnNv",
            if enabled { "Enabled" } else { "Disabled" }
        ),
        0xffffff,
    );
    state.send(payload);
    state.log(
        if enabled {
            "바이옴 매크로 켜짐"
        } else {
            "바이옴 매크로 꺼짐"
        }
        .into(),
    );
}
pub fn connection(watching: bool, message: String) {
    let mut state = lock();
    state.watching = watching;
    state.message = message;
}
pub fn line(line: &str, source: i32, snapshot: bool) {
    let mut state = lock();
    state.ingest(line, source, snapshot);
    state.release_held();
}
pub fn skip_safe() -> bool {
    let mut state = lock();
    if state.safe_left() == 0 {
        return false;
    }
    state.skip_job = state.job.clone();
    state.skip_file = state.file.clone();
    state.skip_requested = true;
    state.log("안전 모드 타이머 건너뜀 (K)".into());
    state.release_held();
    true
}
impl State {
    fn safe_left(&self) -> u64 {
        if !self.settings.safe_mode {
            return 0;
        }
        if self.skip_requested
            && self.skip_file == self.file
            && (self.skip_job.is_none() || self.skip_job == self.job)
        {
            return 0;
        }
        match (&self.job, self.job_since) {
            (Some(_), Some(since)) => (SAFE_SECONDS as f64 - since.elapsed().as_secs_f64())
                .max(0.0)
                .ceil() as u64,
            _ => SAFE_SECONDS,
        }
    }

    fn server_line(&mut self, line: &str) {
        let Some(join) = JOIN.captures(line) else {
            return;
        };
        let job = join[1].to_ascii_lowercase();
        if self.job.as_deref() == Some(job.as_str()) {
            return;
        }
        if self.skip_requested
            && self.skip_file == self.file
            && self.skip_job.is_none()
            && self.job.is_none()
        {
            self.skip_job = Some(job.clone());
        }
        self.job = Some(job.clone());
        self.job_since = Some(Instant::now());
        if self.settings.safe_mode {
            self.log(format!(
                "서버 접속 (서버 ID {}) — 안전 모드: 60초 동안 그대로면 바이옴 알림 보냄",
                &job[..8]
            ));
        }
    }

    fn release_held(&mut self) {
        if !self.settings.safe_mode {
            self.held = None;
            return;
        }
        if self.safe_left() > 0 {
            return;
        }
        let Some(held) = self.held.take() else {
            return;
        };
        let owner_ok = self.owner == "ok" || (cfg!(target_os = "android") && self.watching);
        if self.current.as_deref() != Some(held.as_str())
            || !self.settings.enabled
            || !owner_ok
            || !self.settings.webhooks.iter().any(|hook| !hook.is_empty())
        {
            return;
        }
        self.log(format!("안전 모드 통과 — {} 알림 보냄", show(&held)));
        self.announced = Some(held.clone());
        self.send(self.biome(&held, true));
    }

    fn ingest(&mut self, line: &str, source: i32, snapshot: bool) {
        self.server_line(line);
        if line.contains("Client:Disconnect") {
            if !snapshot && !self.disconnected {
                if self.settings.enabled && self.safe_left() == 0 {
                    let payload = self.embed(
                        "> ## Player has left the game (Disconnected/Left)".into(),
                        0xff0000,
                    );
                    self.send(payload);
                }
                self.log("로블록스 접속 끊김".into());
            }
            self.disconnected = true;
            if snapshot {
                self.current = None;
            }
            return;
        }
        let Some(name) = parse(line) else {
            return;
        };
        if !snapshot {
            if self.primary == 0 {
                self.primary = source;
            }
            if self.primary != source {
                return;
            }
        }
        if self.current.as_deref() == Some(&name) {
            return;
        }
        let previous = self.current.replace(name.clone());
        self.disconnected = false;
        self.history
            .push_back((Utc::now().timestamp(), name.clone()));
        while self.history.len() > 50 {
            self.history.pop_front();
        }
        self.log(format!(
            "바이옴: {} → {}",
            previous.as_deref().unwrap_or("-"),
            show(&name)
        ));
        if self
            .held
            .as_deref()
            .is_some_and(|held| held != name.as_str())
        {
            self.held = None;
        }
        if self.settings.enabled
            && self.settings.webhooks.iter().any(|hook| !hook.is_empty())
            && (!snapshot || previous.is_some() || self.announce)
        {
            if let Some(previous) = previous.filter(|name| {
                TARGETS.contains(&name.as_str())
                    && (!self.settings.safe_mode
                        || self.announced.as_deref() == Some(name.as_str()))
            }) {
                let payload = self.biome(&previous, false);
                self.send(payload);
            }
            if TARGETS.contains(&name.as_str()) {
                if self.settings.safe_mode && self.safe_left() > 0 {
                    self.held = Some(name.clone());
                    self.log(format!(
                        "안전 모드 — {} 알림 대기 ({}초 · K 로 건너뛰기)",
                        show(&name),
                        self.safe_left()
                    ));
                } else {
                    self.announced = Some(name.clone());
                    let payload = self.biome(&name, true);
                    self.send(payload);
                }
            }
        }
        self.announce = false;
    }

    fn log(&mut self, message: String) {
        self.logs.push_back(message);
        while self.logs.len() > 300 {
            self.logs.pop_front();
        }
    }
    fn embed(&self, description: String, color: u32) -> Value {
        json!({"embeds": [{"description": description, "color": color, "timestamp": Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true), "footer": {"text": format!("Acrux{} V{}", if cfg!(target_os = "android") { " Mobile" } else { "" }, env!("CARGO_PKG_VERSION"))}}], "allowed_mentions": {"parse": ["everyone", "roles"]}})
    }
    fn biome(&self, name: &str, started: bool) -> Value {
        let word = if started { "Started" } else { "Ended" };
        let mut payload = self.embed(
            format!(
                "> ## Biome {word} - {}\n> \n> **{word}:** <t:{}:R>",
                show(name),
                Utc::now().timestamp()
            ),
            0xffffff,
        );
        let seconds = self.since.elapsed().as_secs();
        let mut fields = vec![
            json!({"name": "Uptime", "value": format!("{}hr {}mn {}sc", seconds / 3600, seconds % 3600 / 60, seconds % 60), "inline": true}),
        ];
        if started {
            fields.push(json!({"name": "Private Server Link", "value": if self.settings.ps_link.is_empty() { "None" } else { &self.settings.ps_link }, "inline": false}));
        }
        payload["embeds"][0]["fields"] = fields.into();
        if TARGETS[..13].contains(&name) {
            payload["embeds"][0]["thumbnail"] = json!({"url": format!("https://raw.githubusercontent.com/rngenesis0-coder/Acrux-macro/refs/heads/main/{name}.png")});
        }
        if started {
            let mention = self
                .settings
                .mentions
                .get(name)
                .copied()
                .unwrap_or(self.settings.everyone && RARE.contains(&name));
            let role = self
                .settings
                .roles
                .get(name)
                .filter(|role| !role.is_empty() && role.chars().all(|c| c.is_ascii_digit()));
            payload["content"] = json!(format!(
                "{}{}",
                if mention { "@everyone " } else { "" },
                role.map(|role| format!("<@&{role}>")).unwrap_or_default()
            ));
        }
        payload
    }
    fn send(&self, payload: Value) {
        for hook in self.settings.webhooks.iter().filter(|url| !url.is_empty()) {
            enqueue(hook, payload.clone());
        }
    }
}
fn show(name: &str) -> &str {
    match name {
        "LOSTFOREST" => "LOST FOREST",
        "RADIANTFOREST" => "RADIANT FOREST",
        "PUMPKINMOON" => "PUMPKIN MOON",
        "BLOODRAIN" => "BLOOD RAIN",
        name => name,
    }
}
pub fn parse(line: &str) -> Option<String> {
    if !line.contains("BloxstrapRPC") {
        return None;
    }
    let unescaped = line.replace("\\\"", "\"");
    let captures = RPC.captures(&unescaped)?;
    let raw: String = captures[1]
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_uppercase())
        .collect();
    let name = raw
        .strip_prefix("THE")
        .filter(|name| TARGETS.contains(name))
        .unwrap_or(&raw);
    if name.is_empty() {
        return None;
    }
    if TARGETS.contains(&name) {
        return Some(name.into());
    }
    let best = TARGETS
        .iter()
        .map(|target| (*target, distance(name, target)))
        .min_by_key(|(_, distance)| *distance);
    Some(
        best.filter(|(target, distance)| name.len() >= 5 && *distance <= (target.len() / 5).max(1))
            .map(|(target, _)| target)
            .unwrap_or(name)
            .into(),
    )
}
fn distance(a: &str, b: &str) -> usize {
    let mut previous: Vec<usize> = (0..=b.len()).collect();
    for (i, a) in a.bytes().enumerate() {
        let mut next = vec![i + 1];
        for (j, b) in b.bytes().enumerate() {
            next.push(
                (next[j] + 1)
                    .min(previous[j + 1] + 1)
                    .min(previous[j] + usize::from(a != b)),
            );
        }
        previous = next;
    }
    previous[b.len()]
}
pub fn client() -> Result<Client, reqwest::Error> {
    let builder = Client::builder();
    #[cfg(target_os = "android")]
    let builder = builder.tls_backend_preconfigured(
        rustls::ClientConfig::builder()
            .with_root_certificates(rustls::RootCertStore {
                roots: webpki_roots::TLS_SERVER_ROOTS.to_vec(),
            })
            .with_no_client_auth(),
    );
    builder
        .timeout(Duration::from_secs(8))
        .redirect(Policy::none())
        .user_agent("Acrux/0.1")
        .build()
}
fn is_hook(value: &str) -> bool {
    static HOOK: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"^https://(?:(?:ptb|canary)\.)?discord(?:app)?\.com/api/(?:v\d+/)?webhooks/\d+/[A-Za-z0-9_-]+/?(?:\?[A-Za-z0-9_=&-]*)?$").unwrap()
    });
    HOOK.is_match(value) && Url::parse(value).is_ok()
}
fn enqueue(hook: &str, payload: Value) {
    if !is_hook(hook) {
        return;
    }
    let mut queues = QUEUES.lock().unwrap_or_else(|e| e.into_inner());
    let sender = queues.entry(hook.into()).or_insert_with(|| {
        let (sender, receiver) = mpsc::channel::<Value>();
        let url = hook.to_owned();
        std::thread::spawn(move || {
            let Ok(client) = client() else {
                return;
            };
            for payload in receiver {
                if let Err(error) = post(&client, &url, &payload) {
                    lock().log(error);
                }
            }
        });
        sender
    });
    let _ = sender.send(payload);
}
fn post(client: &Client, url: &str, payload: &Value) -> Result<(), String> {
    for attempt in 0..4 {
        let response = client
            .post(url)
            .json(payload)
            .send()
            .map_err(|_| "웹후크 연결 실패".to_owned())?;
        if response.status().is_success() {
            return Ok(());
        }
        if response.status().as_u16() != 429 || attempt == 3 {
            return Err(format!("웹후크 전송 실패 ({})", response.status().as_u16()));
        }
        let header = response
            .headers()
            .get("Retry-After")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<f64>().ok());
        let body = response
            .json::<Value>()
            .ok()
            .and_then(|v| v["retry_after"].as_f64());
        let wait = body
            .or(header)
            .filter(|v| v.is_finite())
            .unwrap_or(1.5)
            .clamp(0.5, 10.0);
        std::thread::sleep(Duration::from_secs_f64(wait));
    }
    Ok(())
}
pub fn test() -> Result<(), String> {
    let (hooks, mut payload) = {
        let state = lock();
        let mut payload = state.embed(
            "> ## Webhook Test\n> Webhook connection test".into(),
            0xffffff,
        );
        payload["embeds"][0]["fields"] = json!([{"name": "Private Server Link", "value": if state.settings.ps_link.is_empty() { "Not set" } else { &state.settings.ps_link }}]);
        (state.settings.webhooks.clone(), payload)
    };
    payload["allowed_mentions"] = json!({"parse": []});
    if !hooks.iter().any(|url| !url.is_empty()) {
        return Err("웹후크를 먼저 입력하세요.".into());
    }
    // Use the same queues as live notifications so a test cannot reorder alerts.
    for hook in hooks.iter().filter(|url| !url.is_empty()) {
        enqueue(hook, payload.clone());
    }
    Ok(())
}

#[cfg(any(target_os = "windows", target_os = "linux"))]
fn log_directories() -> Vec<std::path::PathBuf> {
    use std::path::PathBuf;
    #[cfg(target_os = "windows")]
    {
        std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .map(|path| path.join("Roblox/logs"))
            .into_iter()
            .collect()
    }
    #[cfg(target_os = "linux")]
    {
        let home = std::env::var_os("HOME").map(PathBuf::from);
        let mut paths = Vec::new();
        if let Some(home) = &home {
            paths.push(home.join(".var/app/org.vinegarhq.Sober/data/sober/appData/logs"));
        }
        if let Some(data) = std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(|| home.map(|path| path.join(".local/share")))
        {
            paths.push(data.join("sober/appData/logs"));
        }
        paths
    }
}

#[cfg(any(target_os = "windows", target_os = "linux"))]
pub(crate) fn latest_desktop_log() -> Option<std::path::PathBuf> {
    use std::fs;
    log_directories()
        .into_iter()
        .filter_map(|path| fs::read_dir(path).ok())
        .flatten()
        .filter_map(Result::ok)
        .filter(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .to_ascii_lowercase()
                .ends_with(".log")
        })
        .max_by_key(|entry| entry.metadata().and_then(|meta| meta.modified()).ok())
        .map(|entry| entry.path())
}

#[cfg(any(target_os = "windows", target_os = "linux"))]
pub fn watch_desktop() {
    std::thread::spawn(|| {
        use std::{
            fs,
            io::{Read, Seek, SeekFrom},
            path::PathBuf,
        };
        let mut file: Option<(PathBuf, u64)> = None;
        let mut pending = String::new();
        let mut player = String::new();
        let mut owner: Option<bool> = None;
        let mut initial: Option<String> = None;
        let user = Regex::new(
            r#"(?i)Players\.([^.']+)\.PlayerGui:WaitForChild\((?:"|\\")TutorialCursor(?:"|\\")\)"#,
        )
        .unwrap();
        loop {
            let config = settings();
            let requested_player = config.player.trim().trim_start_matches('@').trim();
            if player != requested_player {
                file = None;
                pending.clear();
                owner = None;
                initial = None;
                let mut state = lock();
                state.current = None;
                state.primary = 0;
                state.disconnected = false;
                state.watching = false;
                state.owner.clear();
                state.file = None;
                state.held = None;
            }
            player = requested_player.to_owned();
            if player.is_empty() {
                lock().owner = "noname".into();
                connection(false, "플레이어 이름 입력 필요".into());
            }
            if !player.is_empty() {
                lock().release_held();
                if let Some(path) = latest_desktop_log() {
                    if let Ok(mut reader) = fs::File::open(&path) {
                        let length = reader.metadata().map(|m| m.len()).unwrap_or(0);
                        let fresh = file
                            .as_ref()
                            .is_none_or(|(previous, offset)| previous != &path || *offset > length);
                        let previous = file
                            .as_ref()
                            .filter(|(previous, offset)| previous == &path && *offset <= length)
                            .map_or(0, |(_, offset)| *offset);
                        if fresh {
                            pending.clear();
                            owner = None;
                            initial = None;
                            lock().file = Some(path.to_string_lossy().into_owned());
                        }
                        if reader.seek(SeekFrom::Start(previous)).is_ok() {
                            let mut bytes = Vec::new();
                            if reader.read_to_end(&mut bytes).is_ok() {
                                pending.push_str(&String::from_utf8_lossy(&bytes));
                                let lines: Vec<String> = pending
                                    .split_inclusive('\n')
                                    .take_while(|line| line.ends_with('\n'))
                                    .map(str::to_owned)
                                    .collect();
                                let consumed: usize = lines.iter().map(String::len).sum();
                                pending.drain(..consumed);
                                {
                                    let mut state = lock();
                                    for entry in &lines {
                                        state.server_line(entry);
                                    }
                                }
                                if owner != Some(true) {
                                    if let Some(name) = lines.iter().find_map(|entry| {
                                        user.captures(entry).map(|c| c[1].trim().to_owned())
                                    }) {
                                        owner = Some(name.eq_ignore_ascii_case(&player));
                                    }
                                }
                                lock().owner = match owner {
                                    Some(true) => "ok",
                                    Some(false) => "other",
                                    None => "",
                                }
                                .into();
                                if fresh || owner != Some(true) {
                                    if let Some(last) = lines.iter().rev().find(|entry| {
                                        parse(entry).is_some()
                                            || entry.contains("Client:Disconnect")
                                    }) {
                                        initial = Some(last.clone());
                                    }
                                }
                                if owner == Some(true) {
                                    if let Some(last) = initial.take() {
                                        line(&last, 1, true);
                                    } else {
                                        for entry in lines {
                                            if parse(&entry).is_some()
                                                || entry.contains("Client:Disconnect")
                                            {
                                                line(&entry, 1, false);
                                            }
                                        }
                                    }
                                }
                                file = Some((path, previous + bytes.len() as u64));
                                connection(true, "감시 중".into());
                            }
                        }
                    }
                } else {
                    connection(false, "Roblox 로그 대기 중".into());
                }
            }
            std::thread::sleep(Duration::from_millis(500));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_original_rpc_and_escaped_rpc() {
        assert_eq!(
            parse(r#"[BloxstrapRPC] {"largeImage":{"hoverText":"The Lost Forest"}}"#).as_deref(),
            Some("LOSTFOREST")
        );
        assert_eq!(
            parse(r#"[BloxstrapRPC] {\"largeImage\":{\"hoverText\":\"Sand Storm\"}}"#).as_deref(),
            Some("SANDSTORM")
        );
        assert_eq!(parse("unrelated hoverText RAINY"), None);
    }
    #[test]
    fn webhook_validation_rejects_foreign_hosts() {
        assert!(is_hook("https://discord.com/api/webhooks/123/abc_DEF-9"));
        assert!(!is_hook(
            "https://discord.com.example.org/api/webhooks/123/abc"
        ));
        assert!(!is_hook(
            "https://discord.com/api/webhooks/123/abc#fragment"
        ));
    }
    #[test]
    fn reconnect_and_secondary_source_cannot_replay_old_biomes() {
        let mut state = State::default();
        state.settings.enabled = true;
        state.ingest(
            r#"[BloxstrapRPC] {"largeImage":{"hoverText":"RAINY"}}"#,
            1,
            true,
        );
        state.ingest(
            r#"[BloxstrapRPC] {"largeImage":{"hoverText":"RAINY"}}"#,
            1,
            false,
        );
        state.ingest(
            r#"[BloxstrapRPC] {"largeImage":{"hoverText":"HELL"}}"#,
            1,
            false,
        );
        state.ingest(
            r#"[BloxstrapRPC] {"largeImage":{"hoverText":"RAINY"}}"#,
            2,
            false,
        );
        state.ingest(
            r#"[BloxstrapRPC] {"largeImage":{"hoverText":"HELL"}}"#,
            1,
            false,
        );
        assert_eq!(state.current.as_deref(), Some("HELL"));
        assert_eq!(state.history.len(), 2);
    }
    #[test]
    fn rare_mentions_apply_only_to_start_notifications() {
        let mut state = State::default();
        state.settings.ps_link = "https://www.roblox.com/share?code=example".into();
        assert_eq!(state.biome("GLITCHED", true)["content"], "@everyone ");
        assert!(state.biome("GLITCHED", false).get("content").is_none());
        state.settings.everyone = false;
        assert_eq!(state.biome("GLITCHED", true)["content"], "");
    }
}

//! Optional account linking for the website's macro bot. Credentials never cross IPC.
use regex::Regex;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    io::Read,
    sync::{Condvar, LazyLock, Mutex, MutexGuard, Once},
    time::{Duration, Instant},
};

const API: &str = "https://acruxmacro.com/api";
const LINK_PAGE: &str = "https://acruxmacro.com/link/?code=";
const RESPONSE_LIMIT: u64 = 64 * 1024;

#[derive(Clone, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct Settings {
    token: String,
    name: String,
}

#[derive(Clone, Serialize)]
pub struct PublicState {
    linked: bool,
    name: String,
    code: Option<String>,
    error: String,
}

struct Pending {
    code: String,
    poll: String,
    until: Instant,
    next: Instant,
}

struct Connection {
    settings: Settings,
    pending: Option<Pending>,
    starting: bool,
    revision: u64,
    error: String,
    next_push: Instant,
    cooldown: Instant,
    last_running: Option<bool>,
}

impl Default for Connection {
    fn default() -> Self {
        Self {
            settings: Settings::default(),
            pending: None,
            starting: false,
            revision: 0,
            error: String::new(),
            next_push: Instant::now(),
            cooldown: Instant::now(),
            last_running: None,
        }
    }
}

impl Connection {
    fn public(&self) -> PublicState {
        PublicState {
            linked: !self.settings.token.is_empty(),
            name: self.settings.name.clone(),
            code: self
                .pending
                .as_ref()
                .filter(|p| Instant::now() < p.until)
                .map(|p| p.code.clone()),
            error: self.error.clone(),
        }
    }
    fn cancel(&mut self) {
        self.revision = self.revision.wrapping_add(1);
        self.pending = None;
        self.starting = false;
        self.error.clear();
    }
    fn current(&self, revision: u64, token: &str) -> bool {
        self.revision == revision && self.settings.token == token
    }
    fn due(&self, running: bool, now: Instant) -> bool {
        now >= self.cooldown && (now >= self.next_push || self.last_running != Some(running))
    }
    fn pushed(&mut self, running: bool, response: &Result<(u16, Value), String>, now: Instant) {
        let body = response
            .as_ref()
            .ok()
            .filter(|(status, _)| *status == 200)
            .map(|(_, body)| body);
        let skipped = body.is_some_and(|body| body["skipped"] == true);
        if body.is_some_and(|body| body["ok"] == true) && !skipped {
            let seconds = body
                .and_then(|body| body["next"].as_u64())
                .unwrap_or(60)
                .clamp(10, 300);
            self.next_push = now + Duration::from_secs(seconds);
            self.cooldown = now + Duration::from_secs(4);
            self.last_running = Some(running);
        } else {
            let seconds = if skipped { 4 } else { 60 };
            self.next_push = now + Duration::from_secs(seconds);
            self.cooldown = self.next_push;
        }
    }
}

static STATE: LazyLock<Mutex<Connection>> = LazyLock::new(|| Mutex::new(Connection::default()));
static WAKE: Condvar = Condvar::new();

fn lock() -> MutexGuard<'static, Connection> {
    STATE.lock().unwrap_or_else(|e| e.into_inner())
}

pub fn load(mut settings: Settings) {
    if !hex_token(&settings.token) {
        settings = Settings::default();
    }
    settings.name = settings.name.chars().take(40).collect();
    let mut state = lock();
    state.cancel();
    state.settings = settings;
    state.next_push = Instant::now();
    state.cooldown = Instant::now();
    state.last_running = None;
}
pub fn settings() -> Settings {
    lock().settings.clone()
}
pub fn status() -> PublicState {
    lock().public()
}
pub fn wake() {
    WAKE.notify_one();
}
pub fn cancel() -> PublicState {
    let mut state = lock();
    state.cancel();
    let result = state.public();
    drop(state);
    wake();
    result
}

fn hex_token(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
}
fn link_code(value: &str) -> bool {
    value.len() == 8
        && value
            .bytes()
            .all(|c| b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789".contains(&c))
}

fn request(
    client: &Client,
    base: &str,
    path: &str,
    body: Value,
    token: Option<&str>,
) -> Result<(u16, Value), String> {
    let mut request = client.post(format!("{base}{path}")).json(&body);
    if let Some(token) = token {
        request = request.bearer_auth(token);
    }
    let response = request
        .send()
        .map_err(|_| "연결 서버에 접속할 수 없습니다.")?;
    let status = response.status().as_u16();
    if status >= 400 || status == 204 {
        return decode_response(status, &[]);
    }
    let mut bytes = Vec::new();
    response
        .take(RESPONSE_LIMIT + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "연결 서버 응답을 읽지 못했습니다.")?;
    decode_response(status, &bytes)
}

fn decode_response(status: u16, bytes: &[u8]) -> Result<(u16, Value), String> {
    if status >= 400 || status == 204 {
        return Ok((status, Value::Null));
    }
    if bytes.len() > RESPONSE_LIMIT as usize {
        return Err("연결 서버 응답이 너무 큽니다.".into());
    }
    let data = serde_json::from_slice(bytes).map_err(|_| "연결 서버 응답이 올바르지 않습니다.")?;
    Ok((status, data))
}

fn pending(data: &Value, now: Instant) -> Result<Pending, String> {
    let code = data["code"]
        .as_str()
        .filter(|v| link_code(v))
        .ok_or("연결 코드를 읽지 못했습니다.")?;
    let poll = data["poll"]
        .as_str()
        .filter(|v| hex_token(v))
        .ok_or("연결 요청을 확인하지 못했습니다.")?;
    let seconds = data["expires"].as_u64().unwrap_or(600).clamp(1, 600);
    Ok(Pending {
        code: code.into(),
        poll: poll.into(),
        until: now + Duration::from_secs(seconds),
        next: now,
    })
}

/// Returns only a locally constructed, fixed-origin page for the platform browser.
pub fn login() -> Result<Option<String>, String> {
    let revision = {
        let mut state = lock();
        if !state.settings.token.is_empty() {
            return Ok(None);
        }
        if let Some(pending) = state.pending.as_ref().filter(|p| Instant::now() < p.until) {
            return Ok(Some(format!("{LINK_PAGE}{}", pending.code)));
        }
        if state.starting {
            return Err("이미 연결을 준비하고 있습니다.".into());
        }
        state.cancel();
        state.starting = true;
        state.revision
    };
    let result = (|| {
        let client = crate::biome::client().map_err(|_| "연결을 준비하지 못했습니다.")?;
        let platform = if cfg!(target_os = "android") {
            "mobile"
        } else {
            "pc"
        };
        let (status, response) = request(
            &client,
            API,
            "/link/start",
            json!({"platform":platform}),
            None,
        )?;
        if status != 200 {
            return Err("연결 코드를 받지 못했습니다.".into());
        }
        Ok((client, pending(&response, Instant::now())?))
    })();
    let mut state = lock();
    if state.revision != revision {
        return Err("연결 요청이 취소됐습니다.".into());
    }
    state.starting = false;
    match result {
        Ok((client, pending)) => {
            let url = format!("{LINK_PAGE}{}", pending.code);
            state.pending = Some(pending);
            state.error.clear();
            drop(state);
            start_worker(client);
            wake();
            Ok(Some(url))
        }
        Err(error) => {
            state.error = error;
            Err(state.error.clone())
        }
    }
}

pub fn browser_failed() {
    lock().error = "연결 페이지를 열지 못했습니다. 디스코드 로그인을 다시 눌러 주세요.".into();
}

pub fn start() {
    if lock().settings.token.is_empty() {
        return;
    }
    if let Ok(client) = crate::biome::client() {
        start_worker(client);
    }
}
fn start_worker(client: Client) {
    static STARTED: Once = Once::new();
    STARTED.call_once(|| {
        std::thread::spawn(move || worker(client));
    });
}

/// Save under Runtime -> account lock ordering; the profile writer takes the
/// supplied settings, so cancellation cannot race a consumed poll into storage.
fn save_link(revision: u64, poll: &str, next: Settings) -> bool {
    let runtime = crate::runtime();
    let mut state = lock();
    if state.revision != revision
        || !state
            .pending
            .as_ref()
            .is_some_and(|p| p.poll == poll && Instant::now() < p.until)
    {
        return false;
    }
    if runtime
        .save_profile(
            runtime.fishing.tuning,
            crate::biome::settings(),
            next.clone(),
        )
        .is_err()
    {
        state.pending = None;
        state.error = "디스코드 연결을 저장하지 못했습니다. 다시 연결해 주세요.".into();
        return false;
    }
    state.settings = next;
    state.pending = None;
    state.error.clear();
    state.next_push = Instant::now();
    state.cooldown = Instant::now();
    state.last_running = None;
    true
}

fn disconnect(expected: Option<(u64, &str)>) -> (String, Result<(), String>) {
    let runtime = crate::runtime();
    let mut state = lock();
    if expected.is_some_and(|(revision, token)| !state.current(revision, token)) {
        return (String::new(), Ok(()));
    }
    let token = state.settings.token.clone();
    state.cancel();
    state.settings = Settings::default();
    state.last_running = None;
    let result = runtime.save_profile(
        runtime.fishing.tuning,
        crate::biome::settings(),
        Settings::default(),
    );
    if result.is_err() {
        state.error = "연결은 중지됐지만 설정을 저장하지 못했습니다.".into();
    }
    drop(state);
    wake();
    (
        token,
        result.map_err(|_| "연결은 중지됐지만 설정을 저장하지 못했습니다.".into()),
    )
}
pub fn logout() -> Result<PublicState, String> {
    let (token, saved) = disconnect(None);
    if !token.is_empty() {
        if let Ok(client) = crate::biome::client() {
            let _ = request(&client, API, "/macro/unlink", json!({}), Some(&token));
        }
    }
    saved?;
    Ok(status())
}

fn wait(duration: Option<Duration>) {
    let state = lock();
    if let Some(duration) = duration {
        drop(
            WAKE.wait_timeout(state, duration)
                .unwrap_or_else(|e| e.into_inner()),
        );
    } else if state.pending.is_none() && state.settings.token.is_empty() {
        drop(WAKE.wait(state).unwrap_or_else(|e| e.into_inner()));
    }
}

fn worker(client: Client) {
    loop {
        let (revision, token, poll) = {
            let mut state = lock();
            let now = Instant::now();
            if state.pending.as_ref().is_some_and(|p| now >= p.until) {
                state.pending = None;
                state.error = "연결 시간이 지났습니다. 다시 연결해 주세요.".into();
            }
            let poll = state.pending.as_mut().filter(|p| now >= p.next).map(|p| {
                p.next = now + Duration::from_secs(2);
                p.poll.clone()
            });
            (state.revision, state.settings.token.clone(), poll)
        };
        if let Some(poll) = poll {
            let response = request(&client, API, "/link/poll", json!({"poll":poll}), None);
            if let Ok((status, body)) = response {
                if status == 200 && body["status"] == "ok" {
                    let token = body["token"].as_str().filter(|t| hex_token(t));
                    if let Some(token) = token {
                        let next = Settings {
                            token: token.into(),
                            name: body["user"]["name"]
                                .as_str()
                                .unwrap_or("")
                                .chars()
                                .take(40)
                                .collect(),
                        };
                        if !save_link(revision, &poll, next) {
                            let _ = request(&client, API, "/macro/unlink", json!({}), Some(token));
                        }
                    }
                } else if status == 400 || body["status"] == "expired" {
                    let mut state = lock();
                    if state.revision == revision
                        && state.pending.as_ref().is_some_and(|p| p.poll == poll)
                    {
                        state.pending = None;
                        state.error = "연결 시간이 지났습니다. 다시 연결해 주세요.".into();
                    }
                }
            }
        } else if !token.is_empty() {
            let running = crate::runtime().macro_running();
            let due = {
                let state = lock();
                state.current(revision, &token) && state.due(running, Instant::now())
            };
            if due {
                let payload = {
                    let mut runtime = crate::runtime();
                    snapshot(&mut runtime, crate::biome::status())
                };
                let running = payload["running"].as_bool().unwrap_or(false);
                let response = request(&client, API, "/macro/state", payload, Some(&token));
                if matches!(&response, Ok((401, _))) {
                    let _ = disconnect(Some((revision, &token)));
                } else {
                    let mut state = lock();
                    if state.current(revision, &token) {
                        state.pushed(running, &response, Instant::now());
                    }
                }
            }
        }
        let idle = {
            let state = lock();
            state.pending.is_none() && state.settings.token.is_empty()
        };
        wait(if idle {
            None
        } else {
            Some(Duration::from_secs(1))
        });
    }
}

fn safe_text(value: &str, limit: usize) -> String {
    static LINKS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)https?://\S+").unwrap());
    LINKS
        .replace_all(value, "[link]")
        .chars()
        .filter(|c| !c.is_control())
        .take(limit)
        .collect()
}
fn stats(stats: crate::Stats) -> Value {
    json!({"fish_success":stats.success,"fish_junk":stats.junk,"fish_fail":stats.fail,"fish_full":stats.full})
}
fn snapshot(runtime: &mut crate::Runtime, biome: crate::biome::Status) -> Value {
    runtime.record();
    let running = runtime.macro_running();
    let mut features = Vec::new();
    if running {
        features.push("자동 낚시");
    }
    if biome.settings.enabled {
        features.push("바이옴 알림");
    }
    let logs: Vec<_> = runtime
        .log
        .iter()
        .rev()
        .take(30)
        .rev()
        .map(|(time, message)| {
            let time = chrono::DateTime::from_timestamp_millis(*time)
                .map(|t| {
                    t.with_timezone(&chrono::Local)
                        .format("%H:%M:%S")
                        .to_string()
                })
                .unwrap_or_default();
            json!([time, safe_text(message, 200)])
        })
        .collect();
    json!({"v":env!("CARGO_PKG_VERSION"),"running":running,"action":safe_text(runtime.message(),160),
        "started":runtime.macro_since,"biome":biome.current.map(|b|safe_text(&b,40)),"features":features,
        "stats":{"session":stats(runtime.fishing.stats),"all":stats(runtime.totals)},"logs":logs})
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn skipped_transitions_are_retried_after_the_server_gap_and_errors_back_off() {
        let now = Instant::now();
        let mut state = Connection::default();
        state.pushed(true, &Ok((200, json!({"ok":true,"next":15}))), now);
        assert_eq!(state.last_running, Some(true));
        assert!(!state.due(false, now + Duration::from_secs(3)));
        assert!(state.due(false, now + Duration::from_secs(4)));
        let later = now + Duration::from_secs(4);
        state.pushed(
            false,
            &Ok((200, json!({"ok":true,"skipped":true,"next":60}))),
            later,
        );
        assert_eq!(state.last_running, Some(true));
        assert!(!state.due(false, later + Duration::from_secs(3)));
        assert!(state.due(false, later + Duration::from_secs(4)));
        state.pushed(false, &Err("network".into()), later);
        assert!(!state.due(false, later + Duration::from_secs(59)));
        assert!(state.due(false, later + Duration::from_secs(60)));
        state.pushed(false, &Ok((200, json!({"ok":true,"next":60}))), later);
        assert_eq!(state.last_running, Some(false));
    }
    #[test]
    fn authentication_rejection_survives_non_json_error_bodies() {
        for body in [b"".as_slice(), b"<html>unauthorized</html>", b"\xff"] {
            let (status, value) = decode_response(401, body).unwrap();
            assert_eq!(status, 401);
            assert!(value.is_null());
        }
        assert!(decode_response(204, b"").unwrap().1.is_null());
        assert!(decode_response(200, b"bad json").is_err());
        assert!(decode_response(200, &vec![b' '; RESPONSE_LIMIT as usize + 1]).is_err());
        assert_eq!(
            decode_response(200, br#"{"next":15}"#).unwrap().1["next"],
            15
        );
    }
    #[test]
    fn link_responses_cannot_open_another_origin_or_persist_poll_values() {
        let now = Instant::now();
        let poll = "a".repeat(64);
        let response =
            json!({"code":"ABCDEFGH","poll":poll,"url":"file:///etc/passwd","expires":90000});
        let p = pending(&response, now).unwrap();
        assert_eq!(
            format!("{LINK_PAGE}{}", p.code),
            "https://acruxmacro.com/link/?code=ABCDEFGH"
        );
        assert_eq!(p.until.duration_since(now), Duration::from_secs(600));
        assert!(pending(&json!({"code":"bad/url!","poll":"a".repeat(64)}), now).is_err());
        assert!(pending(&json!({"code":"ABCDEFGH","poll":"secret"}), now).is_err());
        let settings = serde_json::to_string(&Settings {
            token: "a".repeat(64),
            name: "name".into(),
        })
        .unwrap();
        assert!(!settings.contains("poll") && !settings.contains("code"));
    }
    #[test]
    fn cancel_and_logout_invalidate_late_responses_and_public_state_hides_credentials() {
        let mut state = Connection::default();
        let old = state.revision;
        state.pending = Some(
            pending(
                &json!({"code":"ABCDEFGH","poll":"b".repeat(64)}),
                Instant::now(),
            )
            .unwrap(),
        );
        state.cancel();
        assert_ne!(state.revision, old);
        assert!(state.pending.is_none());
        state.settings = Settings {
            token: "a".repeat(64),
            name: "test".into(),
        };
        let encoded = serde_json::to_string(&state.public()).unwrap();
        assert!(
            !encoded.contains("token")
                && !encoded.contains(&"a".repeat(64))
                && !encoded.contains("poll")
        );
        assert!(!state.current(old, &"a".repeat(64)));
        let current = state.revision;
        state.cancel();
        state.settings = Settings::default();
        assert!(!state.current(current, &"a".repeat(64)));
    }
    #[test]
    fn payload_uses_actual_start_time_and_never_serializes_configuration() {
        let mut runtime = crate::Runtime::default();
        runtime.fishing.phase = crate::Phase::Waiting;
        runtime.log.push_back((
            1_700_000_000_000,
            "URL https://discord.com/api/webhooks/123/secret".into(),
        ));
        let mut biome = crate::biome::Status {
            settings: crate::biome::Settings::default(),
            current: Some("Glitched".into()),
            watching: true,
            message: String::new(),
            logs: vec![],
            history: vec![],
            owner: String::new(),
        };
        biome
            .settings
            .webhooks
            .push("private-webhook-configuration".into());
        let before = chrono::Utc::now().timestamp_millis();
        let body = snapshot(&mut runtime, biome);
        let started = body["started"].as_i64().unwrap();
        assert!((before..=chrono::Utc::now().timestamp_millis()).contains(&started));
        assert_eq!(body["v"], env!("CARGO_PKG_VERSION"));
        assert_eq!(body["running"], true);
        let text = body.to_string();
        assert!(
            !text.contains("private-webhook-configuration")
                && !text.contains("/webhooks/")
                && !text.contains("calibration")
        );
        assert!(body["logs"]
            .as_array()
            .unwrap()
            .iter()
            .any(|line| line[1] == "URL [link]"));
    }
    #[test]
    fn only_supported_stats_and_sanitized_bounded_logs_are_sent() {
        let value = stats(crate::Stats {
            success: 1,
            junk: 2,
            fail: 3,
            full: 4,
        });
        assert_eq!(
            value,
            json!({"fish_success":1,"fish_junk":2,"fish_fail":3,"fish_full":4})
        );
        let message = safe_text(
            "failed HTTPS://discord.com/api/webhooks/123/private?x=y\nnext",
            200,
        );
        assert_eq!(message, "failed [link]next");
        assert_eq!(safe_text(&"한".repeat(1000), 200).chars().count(), 200);
    }
}

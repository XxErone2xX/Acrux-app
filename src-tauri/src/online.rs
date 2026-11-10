use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    sync::{LazyLock, Mutex},
    time::Duration,
};

#[derive(Clone, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    pub share: bool,
    pub install_id: String,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            share: true,
            install_id: uuid::Uuid::new_v4().simple().to_string(),
        }
    }
}
#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct State {
    settings: Settings,
    online: Option<u64>,
}
static STATE: LazyLock<Mutex<State>> = LazyLock::new(|| Mutex::new(State::default()));
pub fn load(mut settings: Settings) {
    settings.install_id = uuid::Uuid::parse_str(&settings.install_id)
        .unwrap_or_else(|_| uuid::Uuid::new_v4())
        .simple()
        .to_string();
    STATE.lock().unwrap().settings = settings;
}
pub fn settings() -> Settings {
    STATE.lock().unwrap().settings.clone()
}
pub fn count() -> Option<u64> {
    STATE.lock().unwrap().online
}
pub fn start() {
    static STARTED: std::sync::Once = std::sync::Once::new();
    STARTED.call_once(|| {
        std::thread::spawn(|| {
            let Ok(client) = crate::biome::client() else {
                return;
            };
            let mut last_ping = None;
            let mut interval = Duration::from_secs(300);
            loop {
                let settings = settings();
                if settings.share
                    && last_ping.is_none_or(|at: std::time::Instant| at.elapsed() >= interval)
                {
                    let result = client
                        .post("https://acruxmacro.com/api/ping")
                        .json(&json!({
                            "id": settings.install_id,
                            "app": if cfg!(target_os = "android") { "mobile" } else { "pc" },
                        }))
                        .send()
                        .and_then(|response| response.error_for_status())
                        .and_then(|response| response.json::<Value>());
                    if let Ok(result) = result {
                        interval = Duration::from_secs(
                            result["next"].as_u64().unwrap_or(300).clamp(60, 3600),
                        );
                        last_ping = Some(std::time::Instant::now());
                    }
                }
                let count = client
                    .get("https://acruxmacro.com/api/count")
                    .send()
                    .and_then(|response| response.error_for_status())
                    .and_then(|response| response.json::<Value>())
                    .ok()
                    .and_then(|result| result["online"].as_u64());
                STATE.lock().unwrap().online = count;
                std::thread::sleep(Duration::from_secs(60));
            }
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn installation_ids_match_the_existing_server_contract() {
        let id = Settings::default().install_id;
        assert_eq!(id.len(), 32);
        assert!(id
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)));
    }
}

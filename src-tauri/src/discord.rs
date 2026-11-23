use crate::{now, runtime, Runtime};
use reqwest::{blocking::Client, StatusCode, Url};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    sync::{Condvar, LazyLock, Mutex},
    thread,
    time::{Duration, Instant},
};

const API: &str = "https://acruxmacro.com/api";
const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Clone, Default, Deserialize, Serialize)]
#[serde(default)]
pub(crate) struct Account {
    token: String,
    name: String,
}

#[derive(Default)]
struct Flow {
    revision: u64,
    pending: Option<Pending>,
    error: String,
}
struct Pending {
    until: Instant,
    code: String,
    url: String,
}
static FLOW: LazyLock<Mutex<Flow>> = LazyLock::new(|| Mutex::new(Flow::default()));
static WAKE: Condvar = Condvar::new();

fn post(
    client: &Client,
    path: &str,
    body: Value,
    token: Option<&str>,
    seconds: u64,
) -> Result<(StatusCode, Value), String> {
    let mut request = client
        .post(format!("{API}{path}"))
        .timeout(Duration::from_secs(seconds))
        .json(&body);
    if let Some(token) = token {
        request = request.bearer_auth(token);
    }
    let response = request.send().map_err(|_| "서버에 연결할 수 없습니다.")?;
    let status = response.status();
    let data = response.json().unwrap_or(Value::Null);
    Ok((status, data))
}

#[tauri::command]
pub(crate) fn discord_state() -> Value {
    let app = runtime();
    let flow = FLOW.lock().unwrap();
    let pending = flow
        .pending
        .as_ref()
        .filter(|pending| pending.until > Instant::now());
    json!({
        "linked": !app.discord.token.is_empty(),
        "name": app.discord.name,
        "code": pending.map(|pending| &pending.code),
        "error": flow.error
    })
}

#[tauri::command]
pub(crate) async fn discord_login() -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(begin_link)
        .await
        .map_err(|error| error.to_string())?
}

#[tauri::command]
pub(crate) async fn discord_open_page(app: tauri::AppHandle) -> Result<(), String> {
    let url = FLOW
        .lock()
        .unwrap()
        .pending
        .as_ref()
        .filter(|pending| pending.until > Instant::now())
        .map(|pending| pending.url.clone())
        .ok_or("유효한 연결 코드가 없습니다.")?;
    tauri::async_runtime::spawn_blocking(move || crate::open_url(&app, &url))
        .await
        .map_err(|error| error.to_string())?
}

fn begin_link() -> Result<Value, String> {
    let revision = {
        let mut flow = FLOW.lock().unwrap();
        if let Some(pending) = flow
            .pending
            .as_ref()
            .filter(|pending| pending.until > Instant::now())
        {
            return Ok(json!({"code": pending.code, "url": pending.url, "ok": true}));
        }
        flow.revision = flow.revision.wrapping_add(1);
        flow.error.clear();
        flow.revision
    };
    let client = crate::biome::client().map_err(|_| "서버에 연결할 수 없습니다.")?;
    let (status, data) = post(
        &client,
        "/link/start",
        json!({"platform": if cfg!(target_os = "android") { "mobile" } else { "pc" }}),
        None,
        10,
    )?;
    let code = data["code"]
        .as_str()
        .filter(|code| code.len() == 8 && code.bytes().all(|ch| ch.is_ascii_alphanumeric()));
    let poll = data["poll"]
        .as_str()
        .filter(|poll| !poll.is_empty() && poll.len() <= 256);
    let url = data["url"]
        .as_str()
        .and_then(|url| Url::parse(url).ok())
        .filter(|url| {
            url.scheme() == "https"
                && url.host_str() == Some("acruxmacro.com")
                && url.username().is_empty()
                && url.password().is_none()
                && url.port().is_none()
                && matches!(url.path(), "/link" | "/link/")
        });
    let (Some(code), Some(poll), Some(url)) = (code, poll, url) else {
        return Err("Discord 연결 코드를 받지 못했습니다.".into());
    };
    if status != StatusCode::OK {
        return Err("Discord 연결 코드를 받지 못했습니다.".into());
    }
    let until =
        Instant::now() + Duration::from_secs(data["expires"].as_u64().unwrap_or(600).clamp(1, 600));
    let poll = poll.to_owned();
    {
        let mut flow = FLOW.lock().unwrap();
        if flow.revision != revision {
            return Err("연결을 취소했습니다.".into());
        }
        flow.pending = Some(Pending {
            until,
            code: code.into(),
            url: url.to_string(),
        });
    }
    thread::spawn(move || {
        while Instant::now() < until {
            thread::sleep(Duration::from_secs(2));
            if FLOW.lock().unwrap().revision != revision {
                return;
            }
            let Ok((status, data)) = post(&client, "/link/poll", json!({"poll": poll}), None, 10)
            else {
                continue;
            };
            if status == StatusCode::OK && data["status"] == "ok" {
                let Some(token) = data["token"]
                    .as_str()
                    .filter(|token| !token.is_empty() && token.len() <= 512)
                else {
                    break;
                };
                let mut app = runtime();
                let mut flow = FLOW.lock().unwrap();
                if flow.revision != revision {
                    return;
                }
                let account = Account {
                    token: token.into(),
                    name: data["user"]["name"]
                        .as_str()
                        .unwrap_or_default()
                        .chars()
                        .take(40)
                        .collect(),
                };
                flow.pending = None;
                flow.error = set_account(&mut app, account).err().unwrap_or_default();
                WAKE.notify_one();
                return;
            }
            if data["status"] == "expired" || status == StatusCode::BAD_REQUEST {
                break;
            }
        }
        let mut flow = FLOW.lock().unwrap();
        if flow.revision == revision {
            flow.pending = None;
            flow.error = "연결 시간이 지났습니다. 다시 시도해 주세요.".into();
        }
    });
    Ok(json!({"code": code, "url": url.as_str(), "ok": true}))
}

fn set_account(app: &mut Runtime, account: Account) -> Result<(), String> {
    let previous = std::mem::replace(&mut app.discord, account);
    if let Err(error) = app.save(app.fishing.tuning) {
        app.discord = previous;
        return Err(error);
    }
    Ok(())
}

#[tauri::command]
pub(crate) fn discord_cancel() -> Value {
    let mut flow = FLOW.lock().unwrap();
    flow.revision = flow.revision.wrapping_add(1);
    flow.pending = None;
    flow.error.clear();
    json!({"ok": true})
}

#[tauri::command]
pub(crate) fn discord_logout() -> Result<Value, String> {
    discord_cancel();
    let token = {
        let mut app = runtime();
        let token = app.discord.token.clone();
        set_account(&mut app, Account::default())?;
        token
    };
    if !token.is_empty() {
        thread::spawn(move || {
            if let Ok(client) = crate::biome::client() {
                let _ = post(&client, "/macro/unlink", json!({}), Some(&token), 10);
            }
        });
    }
    Ok(json!({"ok": true}))
}

pub(crate) fn start() {
    thread::spawn(|| {
        let Ok(client) = crate::biome::client() else {
            return;
        };
        loop {
            let token = runtime().discord.token.clone();
            if token.is_empty() {
                let _result = WAKE.wait_timeout(FLOW.lock().unwrap(), Duration::from_secs(5));
                continue;
            }
            match post(
                &client,
                "/macro/wait",
                json!({"v": VERSION}),
                Some(&token),
                70,
            ) {
                Ok((StatusCode::OK, data)) => {
                    let request = &data["req"];
                    let Some(id) = request["id"]
                        .as_str()
                        .filter(|id| !id.is_empty() && id.len() <= 32)
                    else {
                        continue;
                    };
                    let need = request["need"].as_str().unwrap_or("status");
                    if !matches!(need, "status" | "stats" | "logs" | "ps_check" | "ps_return")
                        || runtime().discord.token != token
                    {
                        continue;
                    }
                    let (client, id, need) = (client.clone(), id.to_owned(), need.to_owned());
                    let lines = request["lines"].as_u64().unwrap_or(50).clamp(1, 100) as usize;
                    thread::spawn(move || {
                        let response = reply(&need, lines, |phase| {
                            let (client, id, token, phase) =
                                (client.clone(), id.clone(), token.clone(), phase.to_owned());
                            thread::spawn(move || {
                                let _ = post(
                                    &client,
                                    "/macro/progress",
                                    json!({"id": id, "phase": phase}),
                                    Some(&token),
                                    3,
                                );
                            });
                        });
                        if runtime().discord.token == token {
                            let _ = post(
                                &client,
                                "/macro/reply",
                                json!({"id": id, "state": response}),
                                Some(&token),
                                10,
                            );
                        }
                    });
                }
                Ok((StatusCode::UNAUTHORIZED, _)) => {
                    let mut app = runtime();
                    if app.discord.token == token {
                        if let Err(error) = set_account(&mut app, Account::default()) {
                            app.discord = Account::default();
                            app.settings_error = Some(error);
                        }
                    }
                }
                _ => thread::sleep(Duration::from_secs(15)),
            }
        }
    });
}

fn reply(need: &str, lines: usize, progress: impl Fn(&str)) -> Value {
    let running = {
        let app = runtime();
        app.coordinator.running || cfg!(target_os = "android") && app.phase().active()
    };
    let mut response = json!({"v": VERSION, "running": running});
    match need {
        "ps_check" => response["ps"] = crate::check_private_server(progress),
        "ps_return" => response["ps"] = crate::return_private_server(),
        "stats" => {
            let app = runtime();
            let stats = |value: crate::Stats| {
                json!({
                    "fish_success": value.success, "fish_junk": value.junk,
                    "fish_fail": value.fail, "fish_full": value.full
                })
            };
            response["stats"] =
                json!({"session": stats(app.fishing.stats), "all": stats(app.totals)});
        }
        "logs" => {
            static URL: LazyLock<regex::Regex> =
                LazyLock::new(|| regex::Regex::new(r"(?i)(?:https?|roblox)://\S+").unwrap());
            let app = runtime();
            response["logs"] = Value::Array(
                app.log
                    .iter()
                    .skip(app.log.len().saturating_sub(lines))
                    .map(|(at, text)| {
                        json!([
                            at,
                            URL.replace_all(text, "[링크]")
                                .chars()
                                .take(200)
                                .collect::<String>()
                        ])
                    })
                    .collect(),
            );
        }
        "status" => {
            let app = runtime();
            let config = app.macro_config();
            let features: Vec<_> = [
                ("biome", "바이옴 알림"),
                ("mfish", "자동 낚시"),
                ("mpop", "레어 바이옴 팝핑"),
                ("mblood", "블러드 레인"),
                ("mitem", "오토 아이템"),
                ("mmerch", "상인 구매"),
                ("mcraft", "포션 제작"),
                ("mmatch", "메모리 매치"),
            ]
            .into_iter()
            .filter_map(|(key, name)| (config[key]["enabled"] == true).then_some(name))
            .collect();
            response["features"] = json!(features);
            response["action"] = json!(if running {
                app.message()
            } else {
                "꺼짐".to_owned()
            });
            response["biome"] = json!(crate::biome::status().current);
            response["started"] = json!(app.macro_since.filter(|_| running).map(|at| {
                chrono::Utc::now().timestamp_millis() - ((now() - at) * 1000.) as i64
            }));
        }
        _ => {}
    }
    response
}

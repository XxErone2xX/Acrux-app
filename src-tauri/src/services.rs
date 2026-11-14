//! App management and the optional desktop Python extension host. Game I/O stays native.
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{BufRead, BufReader, Cursor, Read, Write},
    path::{Component, Path, PathBuf},
    process::{Child, ChildStdin, Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc, Mutex, OnceLock,
    },
    time::{Duration, Instant},
};
use tauri::{AppHandle, Manager};

const MAX_PACKAGE: usize = 30 * 1024 * 1024;
const MAX_MESSAGE: usize = 16 * 1024 * 1024;
const REPO: &str = "XxErone2xX/Acrux-app";
const RESERVED: &[&str] = &[
    "diagnostic",
    "state",
    "poll",
    "set_config",
    "restart",
    "open_web",
    "open_folder",
    "uninstall",
    "mods_list",
    "mods_install",
    "mods_toggle",
    "mods_remove",
    "mods_folder",
    "changelog",
    "ocr_info",
    "ocr_test",
    "mpos_point",
    "mpos_region",
    "pick_point",
    "pick_region",
    "pick_cancel",
    "mfish_autocal",
    "mfish_check",
    "mfish_stop",
    "mpop_default",
    "mpop_test",
    "mpop_stop",
    "mpop_ocr_test",
    "mitem_test",
    "mitem_stop",
    "mmerch_autocal",
    "mmerch_check",
    "mmerch_buy_test",
    "mmerch_stop",
    "mcraft_autocal",
    "mcraft_test",
    "mcraft_stop",
    "mmatch_autocal",
    "mmatch_boardcal",
    "mmatch_test",
    "mmatch_stop",
    "mblood_test",
    "mblood_stop",
    "ret_join",
    "ret_test",
    "play_test",
    "play_stop",
    "play_pos",
    "crash_cancel",
    "move_base",
    "move_base_rev",
    "move_place",
    "move_test",
    "move_get",
    "move_pick",
    "move_arrive",
    "move_stop",
    "move_quest_pick",
    "sell_autocal",
    "sell_test",
    "webhook_test",
    "go",
    "waypoint_mark",
    "waypoint_cancel",
    "update_check",
    "update_download",
    "update_install",
    "update_state",
];

struct State {
    root: PathBuf,
    worker: Mutex<Option<Worker>>,
    worker_process: Mutex<Option<Arc<Mutex<Child>>>>,
    worker_error: Mutex<Option<String>>,
    loaded: Mutex<Vec<Value>>,
    apis: Mutex<Vec<String>>,
    releases: Mutex<Option<(Instant, Value)>>,
    closing: AtomicBool,
}
static STATE: OnceLock<Arc<State>> = OnceLock::new();
static DIAGNOSTIC_PATH: OnceLock<PathBuf> = OnceLock::new();
static DIAGNOSTIC_WRITE: Mutex<()> = Mutex::new(());

pub(crate) fn init_diagnostics(root: &Path) {
    let _ = DIAGNOSTIC_PATH.set(root.join("diagnostics.log"));
    diagnostic(&format!(
        "Acrux {} · {} startup",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS
    ));
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        diagnostic(&format!("panic: {info}"));
        previous(info);
    }));
}

pub(crate) fn diagnostic(message: &str) {
    let Some(path) = DIAGNOSTIC_PATH.get() else {
        return;
    };
    // A panic may originate inside this writer. Never block the panic hook.
    let Ok(_guard) = DIAGNOSTIC_WRITE.try_lock() else {
        return;
    };
    if fs::metadata(path).is_ok_and(|m| m.len() > 512 * 1024) {
        let _ = fs::rename(path, path.with_extension("previous.log"));
    }
    if let Ok(mut file) = fs::OpenOptions::new().create(true).append(true).open(path) {
        let text: String = message.chars().take(4096).collect();
        let _ = writeln!(file, "{} {text}", chrono::Utc::now().to_rfc3339());
    }
}

fn lock<T>(value: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    value.lock().unwrap_or_else(|e| e.into_inner())
}

pub(crate) fn init(app: &AppHandle) -> Result<(), String> {
    let root = app.path().app_config_dir().map_err(|e| e.to_string())?;
    fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    let state = Arc::new(State {
        root,
        worker: Mutex::new(None),
        worker_process: Mutex::new(None),
        worker_error: Mutex::new(None),
        loaded: Mutex::new(Vec::new()),
        apis: Mutex::new(Vec::new()),
        releases: Mutex::new(None),
        closing: AtomicBool::new(false),
    });
    if STATE.set(state.clone()).is_ok() {
        #[cfg(desktop)]
        std::thread::spawn(move || {
            if metadata(&state.root.join("mods"), &crate::macro_settings())
                .iter()
                .any(|item| item["enabled"] == true)
            {
                if let Err(error) = start_worker(&state) {
                    *lock(&state.worker_error) = Some(error);
                }
            }
        });
    }
    Ok(())
}

fn state() -> Result<&'static Arc<State>, String> {
    STATE
        .get()
        .ok_or_else(|| "지원 서비스가 준비되지 않았습니다.".into())
}
fn valid_id(id: &str) -> bool {
    regex::Regex::new(r"^[a-z][a-z0-9_]{1,30}$")
        .unwrap()
        .is_match(id)
        && id != "sniping"
}
fn relative(path: &str) -> Result<PathBuf, String> {
    if path.is_empty() || path.contains('\\') || path.contains(':') {
        return Err("잘못된 파일 경로입니다.".into());
    }
    let path = Path::new(path);
    if path
        .components()
        .any(|p| !matches!(p, Component::Normal(_)))
    {
        return Err("폴더 밖을 가리키는 파일 경로입니다.".into());
    }
    Ok(path.to_owned())
}
fn read_bounded(path: &Path, max: usize) -> Result<Vec<u8>, String> {
    let file = fs::File::open(path).map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    file.take(max as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > max {
        return Err("파일이 너무 큽니다.".into());
    }
    Ok(bytes)
}
fn atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let tmp = path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
    fs::write(&tmp, bytes).map_err(|e| e.to_string())?;
    fs::rename(&tmp, path).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        e.to_string()
    })
}
fn manifest(path: &Path) -> Result<Value, String> {
    let bytes = read_bounded(path, 128 * 1024)?;
    let text = std::str::from_utf8(&bytes)
        .map_err(|e| e.to_string())?
        .trim_start_matches('\u{feff}');
    let value: Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
    if !valid_id(value["id"].as_str().unwrap_or("")) {
        return Err("모드 ID가 올바르지 않습니다.".into());
    }
    let entry = value["entry"].as_str().unwrap_or("main");
    if !regex::Regex::new(r"^[a-z_][a-z0-9_]*$")
        .unwrap()
        .is_match(entry)
    {
        return Err("모드 entry가 올바르지 않습니다.".into());
    }
    Ok(value)
}
fn metadata(folder: &Path, config: &Value) -> Vec<Value> {
    let mut dirs: Vec<_> = fs::read_dir(folder)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|e| e.path())
        .collect();
    dirs.sort();
    dirs.into_iter().filter_map(|path| {
        if path.is_symlink() || !path.is_dir() { return None; }
        let info = manifest(&path.join("mod.json")).ok()?;
        let id = info["id"].as_str()?;
        if path.file_name()?.to_str()? != id { return None; }
        Some(json!({"id": id, "name": info["name"].as_str().unwrap_or(id), "version": info["version"].as_str().unwrap_or(""),
            "type": if info["type"] == "mod" {"mod"} else {"plugin"}, "author": info["author"].as_str().unwrap_or(""),
            "description": info["description"].as_str().unwrap_or(""), "enabled": config["mods"][id]["enabled"].as_bool().unwrap_or(true),
            "loaded": false, "error": "", "ui": {}}))
    }).collect()
}
fn install(bytes: &[u8], folder: &Path) -> Result<Value, String> {
    if bytes.len() > MAX_PACKAGE {
        return Err("모드 파일은 30MiB까지 설치할 수 있습니다.".into());
    }
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|e| e.to_string())?;
    if zip.len() > 4096 {
        return Err("모드 안의 파일이 너무 많습니다.".into());
    }
    let mut manifests = Vec::new();
    let mut total = 0u64;
    for i in 0..zip.len() {
        let item = zip.by_index(i).map_err(|e| e.to_string())?;
        if item.is_symlink() {
            return Err("모드 압축 파일의 심볼릭 링크는 설치할 수 없습니다.".into());
        }
        total = total
            .checked_add(item.size())
            .ok_or("모드 크기가 올바르지 않습니다.")?;
        if total > MAX_PACKAGE as u64 {
            return Err("압축을 푼 모드가 30MiB를 넘습니다.".into());
        }
        relative(item.name().trim_end_matches('/'))?;
        if !item.is_dir()
            && item.name().ends_with("mod.json")
            && item.name().matches('/').count() <= 1
        {
            manifests.push(item.name().to_owned());
        }
    }
    manifests.sort_by_key(|s| s.len());
    let name = manifests.first().ok_or("mod.json이 없습니다.")?.clone();
    let prefix = name.strip_suffix("mod.json").unwrap();
    let mut info_bytes = Vec::new();
    zip.by_name(&name)
        .map_err(|e| e.to_string())?
        .take(128 * 1024 + 1)
        .read_to_end(&mut info_bytes)
        .map_err(|e| e.to_string())?;
    if info_bytes.len() > 128 * 1024 {
        return Err("mod.json이 너무 큽니다.".into());
    }
    let info: Value = serde_json::from_slice(
        info_bytes
            .strip_prefix(&[0xef, 0xbb, 0xbf])
            .unwrap_or(&info_bytes),
    )
    .map_err(|e| e.to_string())?;
    let id = info["id"]
        .as_str()
        .filter(|id| valid_id(id))
        .ok_or("모드 ID가 올바르지 않습니다.")?;
    fs::create_dir_all(folder).map_err(|e| e.to_string())?;
    let tmp = folder.join(format!(".{id}.{}.tmp", uuid::Uuid::new_v4()));
    fs::create_dir(&tmp).map_err(|e| e.to_string())?;
    let unpack = (|| {
        for i in 0..zip.len() {
            let mut item = zip.by_index(i).map_err(|e| e.to_string())?;
            if item.is_dir() || !item.name().starts_with(prefix) {
                continue;
            }
            let relative = relative(&item.name()[prefix.len()..])?;
            let dest = tmp.join(relative);
            fs::create_dir_all(dest.parent().unwrap()).map_err(|e| e.to_string())?;
            let mut file = fs::File::create(&dest).map_err(|e| e.to_string())?;
            let copied = std::io::copy(&mut item.by_ref().take(MAX_PACKAGE as u64 + 1), &mut file)
                .map_err(|e| e.to_string())?;
            if copied > MAX_PACKAGE as u64 {
                return Err("모드 파일이 너무 큽니다.".into());
            }
        }
        let info = manifest(&tmp.join("mod.json"))?;
        if !tmp
            .join(format!("{}.py", info["entry"].as_str().unwrap_or("main")))
            .is_file()
        {
            return Err("모드 setup(ctx)를 실행할 Python 파일이 없습니다.".into());
        }
        Ok(())
    })();
    if let Err(error) = unpack {
        let _ = fs::remove_dir_all(&tmp);
        return Err(error);
    }
    let target = folder.join(id);
    let old = folder.join(format!(".{id}.{}.old", uuid::Uuid::new_v4()));
    if target.exists() {
        if target.is_symlink() {
            let _ = fs::remove_dir_all(&tmp);
            return Err("모드 폴더가 심볼릭 링크입니다.".into());
        }
        fs::rename(&target, &old).map_err(|e| e.to_string())?;
    }
    if let Err(error) = fs::rename(&tmp, &target) {
        let _ = fs::rename(&old, &target);
        let _ = fs::remove_dir_all(&tmp);
        return Err(error.to_string());
    }
    if old.exists() {
        fs::remove_dir_all(old).map_err(|e| e.to_string())?;
    }
    Ok(info)
}
fn byte_payload(payload: &Value) -> Result<Vec<u8>, String> {
    let values = payload["bytes"]
        .as_array()
        .ok_or("설치할 파일을 선택하세요.")?;
    if values.len() > MAX_PACKAGE {
        return Err("파일이 30MiB를 넘습니다.".into());
    }
    values
        .iter()
        .map(|v| {
            v.as_u64()
                .filter(|v| *v <= 255)
                .map(|v| v as u8)
                .ok_or_else(|| "파일 데이터가 올바르지 않습니다.".into())
        })
        .collect()
}
fn save_mod(id: &str, enabled: Option<bool>) -> Result<(), String> {
    let mut config = crate::macro_settings();
    if !config["mods"].is_object() {
        config["mods"] = json!({});
    }
    if let Some(enabled) = enabled {
        config["mods"][id] = json!({"enabled": enabled});
    } else {
        config["mods"].as_object_mut().unwrap().remove(id);
    }
    crate::save_macro_settings(config).map(|_| ())
}

fn trusted_url(url: &str) -> bool {
    reqwest::Url::parse(url).is_ok_and(|u| {
        u.scheme() == "https"
            && u.username().is_empty()
            && u.password().is_none()
            && matches!(
                u.host_str(),
                Some(
                    "api.github.com"
                        | "github.com"
                        | "objects.githubusercontent.com"
                        | "release-assets.githubusercontent.com"
                        | "raw.githubusercontent.com"
                )
            )
    })
}
fn fetch(url: &str, max: usize) -> Result<Vec<u8>, String> {
    let client = crate::biome::client().map_err(|e| e.to_string())?;
    let mut url = url.to_owned();
    for _ in 0..6 {
        if !trusted_url(&url) {
            return Err("공식 배포 주소가 아닙니다.".into());
        }
        let response = client
            .get(&url)
            .header("Accept", "application/vnd.github+json")
            .timeout(Duration::from_secs(180))
            .send()
            .map_err(|e| e.to_string())?;
        if response.status().is_redirection() {
            url = response
                .headers()
                .get(reqwest::header::LOCATION)
                .and_then(|s| s.to_str().ok())
                .ok_or("배포 리디렉션 주소가 없습니다.")?
                .into();
            continue;
        }
        let response = response.error_for_status().map_err(|e| e.to_string())?;
        let mut bytes = Vec::new();
        response
            .take(max as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        if bytes.len() > max {
            return Err("다운로드 크기 제한을 넘었습니다.".into());
        }
        return Ok(bytes);
    }
    Err("다운로드 리디렉션이 너무 많습니다.".into())
}
fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn expected_hash(bytes: &[u8]) -> Result<String, String> {
    let value = std::str::from_utf8(bytes)
        .map_err(|e| e.to_string())?
        .split_whitespace()
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    if value.len() != 64 || !value.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("SHA256 검증 파일이 올바르지 않습니다.".into());
    }
    Ok(value)
}
fn notes(body: &str) -> Value {
    let (mut ko, mut en) = (Vec::new(), Vec::new());
    let mut language = "";
    for line in body.lines() {
        if line.trim_start().starts_with("## ") && line.trim_end().ends_with("업데이트") {
            language = "ko";
        } else if line.trim_start().starts_with("## ") && line.trim_end().ends_with("Update") {
            language = "en";
        } else if language == "ko" {
            ko.push(line);
        } else if language == "en" {
            en.push(line);
        }
    }
    if ko.is_empty() && en.is_empty() {
        ko.extend(body.lines());
        en.extend(body.lines());
    }
    json!({"ko":ko.join("\n").trim(),"en":en.join("\n").trim()})
}
// A dedicated channel prevents legacy main workflows from replacing dev binaries.
const DEV_RELEASE: &str = "dev-alpha";
fn releases(state: &State, refresh: bool) -> Result<Value, String> {
    if !refresh {
        if let Some((at, value)) = &*lock(&state.releases) {
            if at.elapsed() < Duration::from_secs(600) {
                return Ok(value.clone());
            }
        }
    }
    let value: Value = serde_json::from_slice(&fetch(
        &format!("https://api.github.com/repos/{REPO}/releases/tags/{DEV_RELEASE}"),
        4 * 1024 * 1024,
    )?)
    .map_err(|e| e.to_string())?;
    validate_dev_release(&value)?;
    *lock(&state.releases) = Some((Instant::now(), value.clone()));
    Ok(value)
}
fn validate_dev_release(value: &Value) -> Result<(), String> {
    if value["tag_name"] != DEV_RELEASE || value["draft"] != false || !value["assets"].is_array() {
        return Err("개발 버전 릴리스 응답이 올바르지 않습니다.".into());
    }
    Ok(())
}
fn changelog(state: &State, refresh: bool) -> Result<Value, String> {
    let release = releases(state, refresh)?;
    let mut entry = notes(release["body"].as_str().unwrap_or(""));
    entry["version"] = release["name"].clone();
    entry["date"] = json!(release["published_at"]
        .as_str()
        .unwrap_or("")
        .chars()
        .take(10)
        .collect::<String>());
    Ok(json!({"entries":[entry],"current":env!("CARGO_PKG_VERSION")}))
}
fn release_asset(state: &State, refresh: bool) -> Result<Value, String> {
    let release = releases(state, refresh)?;
    let filename = if cfg!(target_os = "android") {
        "Acrux-Alpha.apk"
    } else if cfg!(target_os = "windows") {
        "Acrux-Alpha.exe"
    } else if cfg!(target_os = "macos") {
        "Acrux-macos-universal.zip"
    } else {
        "Acrux-Dev-Linux"
    };
    let assets = release["assets"]
        .as_array()
        .ok_or("배포 파일 목록이 없습니다.")?;
    let asset = assets
        .iter()
        .find(|a| a["name"] == filename)
        .ok_or("현재 운영체제의 알파 배포 파일이 아직 없습니다.")?;
    let checksum = assets
        .iter()
        .find(|a| a["name"] == format!("{filename}.sha256"))
        .ok_or("배포 파일의 SHA256이 없습니다.")?;
    Ok(
        json!({"name":filename,"version":release["name"],"notes":release["body"],"url":asset["browser_download_url"],"shaUrl":checksum["browser_download_url"],"updatedAt":asset["updated_at"]}),
    )
}

#[cfg(target_os = "android")]
fn native(app: &AppHandle, action: &str, payload: Value) -> Result<Value, String> {
    use tauri::plugin::PluginHandle;
    app.state::<PluginHandle<tauri::Wry>>()
        .run_mobile_plugin(
            "serviceAction",
            json!({"action":action,"payload":payload.to_string()}),
        )
        .map_err(|e| e.to_string())
}
#[cfg(not(target_os = "android"))]
fn open_path(path: &Path) -> Result<(), String> {
    open::that(path).map_err(|e| e.to_string())
}
fn link(site: &str) -> Result<&'static str, String> {
    match site {
        "discord" => Ok("https://discord.gg/cyK26gsjCC"),
        "roblox" => Ok("https://www.roblox.com/ko/users/11743130211/profile"),
        "donate" => Ok("https://ko-fi.com/acrux_"),
        "web" => Ok("https://acruxmacro.com"),
        _ => Err("알 수 없는 지원 링크입니다.".into()),
    }
}

pub(crate) fn mod_protocol(
    context: tauri::UriSchemeContext<'_, tauri::Wry>,
    request: tauri::http::Request<Vec<u8>>,
) -> tauri::http::Response<Vec<u8>> {
    let result = (|| {
        let path = request.uri().path().trim_start_matches('/');
        let (id, file) = path.split_once('/').ok_or("파일 경로가 없습니다.")?;
        if !valid_id(id) {
            return Err("잘못된 모드 ID입니다.".to_owned());
        }
        let root = context
            .app_handle()
            .path()
            .app_config_dir()
            .map_err(|e| e.to_string())?
            .join("mods")
            .join(id)
            .canonicalize()
            .map_err(|e| e.to_string())?;
        let file = root
            .join(relative(file)?)
            .canonicalize()
            .map_err(|e| e.to_string())?;
        if !file.starts_with(&root) {
            return Err("모드 폴더 밖의 파일입니다.".into());
        }
        let content = match file.extension().and_then(|e| e.to_str()).unwrap_or("") {
            "html" => "text/html; charset=utf-8",
            "js" => "text/javascript; charset=utf-8",
            "css" => "text/css; charset=utf-8",
            "png" => "image/png",
            "gif" => "image/gif",
            "svg" => "image/svg+xml",
            "json" => "application/json",
            _ => return Err("지원하지 않는 모드 화면 파일입니다.".into()),
        };
        Ok((content, read_bounded(&file, MAX_PACKAGE)?))
    })();
    match result {
        Ok((content, bytes)) => tauri::http::Response::builder()
            .header("Content-Type", content)
            .header("Access-Control-Allow-Origin", "*")
            .header("X-Content-Type-Options", "nosniff")
            .body(bytes)
            .unwrap(),
        Err(error) => tauri::http::Response::builder()
            .status(404)
            .body(error.into_bytes())
            .unwrap(),
    }
}

struct Worker {
    child: Arc<Mutex<Child>>,
    stdin: ChildStdin,
    messages: mpsc::Receiver<Result<Value, String>>,
    next: u64,
}
impl Drop for Worker {
    fn drop(&mut self) {
        let mut child = lock(&self.child);
        let _ = child.kill();
        let _ = child.wait();
    }
}
impl Worker {
    fn request(&mut self, op: &str, payload: Value) -> Result<Value, String> {
        self.request_for(op, payload, Duration::from_secs(60))
    }
    fn request_for(
        &mut self,
        op: &str,
        payload: Value,
        timeout: Duration,
    ) -> Result<Value, String> {
        if lock(&self.child)
            .try_wait()
            .map_err(|e| e.to_string())?
            .is_some()
        {
            return Err("Python 모드 실행기가 종료됐습니다.".into());
        }
        self.next += 1;
        let id = self.next;
        writeln!(self.stdin, "{}", json!({"id":id,"op":op,"payload":payload}))
            .map_err(|e| e.to_string())?;
        self.stdin.flush().map_err(|e| e.to_string())?;
        let deadline = Instant::now() + timeout;
        loop {
            let message = self
                .messages
                .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                .map_err(|_| "Python 모드 응답 시간이 초과됐습니다.")??;
            if message["kind"] == "call" {
                let result = worker_call(
                    message["action"].as_str().unwrap_or(""),
                    message["payload"].clone(),
                );
                let reply = match result {
                    Ok(value) => json!({"kind":"response","id":message["id"],"value":value}),
                    Err(error) => json!({"kind":"response","id":message["id"],"error":error}),
                };
                writeln!(self.stdin, "{reply}").map_err(|e| e.to_string())?;
                self.stdin.flush().map_err(|e| e.to_string())?;
                continue;
            }
            if message["kind"] != "result" || message["id"].as_u64() != Some(id) {
                return Err("모드 응답의 요청 ID가 일치하지 않습니다.".into());
            }
            if let Some(error) = message["error"].as_str() {
                return Err(error.into());
            }
            return Ok(message["value"].clone());
        }
    }
}
fn worker_call(action: &str, payload: Value) -> Result<Value, String> {
    match action {
        "config" => Ok(crate::macro_settings()),
        "state" => {
            let runtime = crate::runtime();
            let mut status = serde_json::to_value(runtime.status()).map_err(|e| e.to_string())?;
            status["config"] = runtime.macro_config();
            status["macro_on"] = json!(runtime.active());
            Ok(status)
        }
        "save_config" => crate::save_macro_settings(payload["config"].clone()),
        "log" => {
            extension_log(format!(
                "[모드] {}",
                payload["message"]
                    .as_str()
                    .unwrap_or("")
                    .chars()
                    .take(4000)
                    .collect::<String>()
            ));
            Ok(Value::Null)
        }
        "status" => {
            extension_log(format!(
                "[모드 상태] {}",
                payload["text"]
                    .as_str()
                    .unwrap_or("")
                    .chars()
                    .take(4000)
                    .collect::<String>()
            ));
            Ok(Value::Null)
        }
        "automation" => {
            if state()?.closing.load(Ordering::Acquire) {
                return Err("앱 종료 중에는 새 모드 입력을 실행하지 않습니다.".into());
            }
            let generation = { crate::runtime().generation };
            crate::automation_call(
                payload["action"].as_str().unwrap_or("").into(),
                payload["payload"].clone(),
                generation,
            )
        }
        _ => Err("알 수 없는 모드 실행기 요청입니다.".into()),
    }
}
fn extension_log(message: String) {
    {
        let mut runtime = crate::runtime();
        if runtime.log.len() == 300 {
            runtime.log.pop_front();
        }
        runtime
            .log
            .push_back((chrono::Utc::now().timestamp_millis(), message.clone()));
    }
    crate::biome::platform_log(message);
    crate::discord::wake();
}
fn quiet(command: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let _ = command;
}
fn command_output(command: &mut Command, timeout: Duration) -> Result<Vec<u8>, String> {
    quiet(command);
    command.stdout(Stdio::piped()).stderr(Stdio::null());
    let mut child = command.spawn().map_err(|e| e.to_string())?;
    let at = Instant::now();
    loop {
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
            let mut bytes = Vec::new();
            child
                .stdout
                .take()
                .unwrap()
                .take(1024 * 1024)
                .read_to_end(&mut bytes)
                .map_err(|e| e.to_string())?;
            return if status.success() {
                Ok(bytes)
            } else {
                Err("Python 실행 명령이 실패했습니다.".into())
            };
        }
        if at.elapsed() > timeout {
            let _ = child.kill();
            let _ = child.wait();
            return Err("Python 실행 시간이 초과됐습니다.".into());
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}
fn python_candidates(root: &Path) -> Vec<PathBuf> {
    let mut paths = vec![
        root.join("mod-runtime/python/python.exe"),
        root.join("mod-runtime/python/bin/python3"),
    ];
    if let Some(path) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path) {
            if dir.to_string_lossy().contains("WindowsApps") {
                continue;
            }
            for name in if cfg!(windows) {
                vec!["python.exe", "python3.exe"]
            } else {
                vec!["python3", "python"]
            } {
                let file = dir.join(name);
                if file.is_file() {
                    paths.push(file);
                }
            }
        }
    }
    paths
}
fn python(root: &Path) -> Result<PathBuf, String> {
    for file in python_candidates(root) {
        if file.is_file()
            && command_output(
                Command::new(&file).args([
                    "-I",
                    "-c",
                    "import sys; assert sys.version_info >= (3, 10); print(sys.executable)",
                ]),
                Duration::from_secs(4),
            )
            .is_ok()
        {
            return Ok(file);
        }
    }
    prepare_python(root)?;
    python_candidates(root)
        .into_iter()
        .find(|p| p.is_file())
        .ok_or_else(|| "자동 준비한 Python 실행 파일을 찾지 못했습니다.".into())
}
fn prepare_python(root: &Path) -> Result<(), String> {
    let target = match (std::env::consts::OS, std::env::consts::ARCH) {
        ("windows", "x86_64") => "x86_64-pc-windows-msvc",
        ("windows", "aarch64") => "aarch64-pc-windows-msvc",
        ("linux", "x86_64") => "x86_64-unknown-linux-gnu",
        ("linux", "aarch64") => "aarch64-unknown-linux-gnu",
        ("macos", "x86_64") => "x86_64-apple-darwin",
        ("macos", "aarch64") => "aarch64-apple-darwin",
        _ => return Err("이 PC의 Python 배포 대상이 없습니다.".into()),
    };
    let release: Value = serde_json::from_slice(&fetch(
        "https://api.github.com/repos/astral-sh/python-build-standalone/releases/latest",
        4 * 1024 * 1024,
    )?)
    .map_err(|e| e.to_string())?;
    let asset = release["assets"]
        .as_array()
        .ok_or("Python 배포 파일이 없습니다.")?
        .iter()
        .find(|a| {
            let n = a["name"].as_str().unwrap_or("");
            n.starts_with("cpython-3.13.")
                && n.contains(&format!("-{target}-install_only_stripped.tar.gz"))
        })
        .ok_or("이 PC에 맞는 공식 Python 파일을 찾지 못했습니다.")?;
    let url = asset["browser_download_url"]
        .as_str()
        .ok_or("Python 다운로드 주소가 없습니다.")?;
    let bytes = fetch(url, 128 * 1024 * 1024)?;
    let expected = if let Some(hash) = asset["digest"]
        .as_str()
        .and_then(|s| s.strip_prefix("sha256:"))
    {
        hash.to_owned()
    } else {
        let checksum = release["assets"]
            .as_array()
            .unwrap()
            .iter()
            .find(|a| a["name"] == format!("{}.sha256", asset["name"].as_str().unwrap()))
            .ok_or("Python 검증 파일을 찾지 못했습니다.")?;
        expected_hash(&fetch(
            checksum["browser_download_url"]
                .as_str()
                .ok_or("Python 검증 주소가 없습니다.")?,
            4096,
        )?)?
    };
    if digest(&bytes) != expected {
        return Err("Python 배포 파일의 SHA256이 일치하지 않습니다.".into());
    }
    let runtime = root.join("mod-runtime");
    fs::create_dir_all(&runtime).map_err(|e| e.to_string())?;
    let tmp = runtime.join(format!(".python-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&tmp).map_err(|e| e.to_string())?;
    let unpack = (|| {
        let decoder = flate2::read::GzDecoder::new(Cursor::new(bytes));
        let mut archive = tar::Archive::new(decoder);
        archive.unpack(&tmp).map_err(|e| e.to_string())?;
        let from = tmp.join("python");
        if !from.is_dir() {
            return Err("Python 압축 형식이 올바르지 않습니다.".into());
        }
        let to = runtime.join("python");
        if to.exists() {
            fs::remove_dir_all(&to).map_err(|e| e.to_string())?;
        }
        fs::rename(from, to).map_err(|e| e.to_string())
    })();
    let _ = fs::remove_dir_all(tmp);
    unpack
}
#[cfg(desktop)]
fn start_worker(state: &State) -> Result<(), String> {
    if state.closing.load(Ordering::Acquire) {
        return Err("앱을 종료하고 있습니다.".into());
    }
    let mut held = lock(&state.worker);
    if held.is_some() {
        return Ok(());
    }
    let folder = state.root.join("mod-runtime/sdk");
    fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
    for (name, source) in [
        ("mod_worker.py", include_str!("../resources/mod_worker.py")),
        ("mods.py", include_str!("../../src/mods.py")),
        (
            "defaults.json",
            include_str!("../../frontend/desktop-defaults.json"),
        ),
    ] {
        let path = folder.join(name);
        if fs::read(&path).ok().as_deref() != Some(source.as_bytes()) {
            atomic(&path, source.as_bytes())?;
        }
    }
    let executable = python(&state.root)?;
    let mut command = Command::new(executable);
    command
        .arg("-I")
        .arg("-u")
        .arg(folder.join("mod_worker.py"))
        .env("PYTHONNOUSERSITE", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    quiet(&mut command);
    if state.closing.load(Ordering::Acquire) {
        return Err("앱을 종료하고 있습니다.".into());
    }
    let mut child = command.spawn().map_err(|e| e.to_string())?;
    let stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let (sender, messages) = mpsc::channel();
    std::thread::spawn(move || {
        let mut reader = BufReader::new(stdout);
        loop {
            let mut line = Vec::new();
            let read = reader
                .by_ref()
                .take(MAX_MESSAGE as u64 + 1)
                .read_until(b'\n', &mut line);
            match read {
                Ok(0) => {
                    let _ = sender.send(Err("Python 모드 실행기가 종료됐습니다.".into()));
                    break;
                }
                Ok(_) if line.len() <= MAX_MESSAGE => {
                    let parsed = serde_json::from_slice(&line).map_err(|e| e.to_string());
                    if sender.send(parsed).is_err() {
                        break;
                    }
                }
                _ => {
                    let _ = sender.send(Err("모드 실행기 응답이 너무 크거나 손상됐습니다.".into()));
                    break;
                }
            }
        }
    });
    let child = Arc::new(Mutex::new(child));
    *lock(&state.worker_process) = Some(child.clone());
    let mut worker = Worker {
        child,
        stdin,
        messages,
        next: 0,
    };
    let loaded=worker.request("load",json!({"data":state.root,"sdk":folder.join("mods.py"),"defaults":folder.join("defaults.json"),"config":crate::macro_settings(),"reserved":RESERVED}))?;
    if let Some(config) = loaded.get("config") {
        crate::save_macro_settings(config.clone())?;
    }
    *lock(&state.loaded) = loaded["mods"].as_array().cloned().unwrap_or_default();
    *lock(&state.apis) = loaded["apis"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect();
    *held = Some(worker);
    *lock(&state.worker_error) = None;
    Ok(())
}
fn mod_list(state: &State) -> Value {
    let mut mods = metadata(&state.root.join("mods"), &crate::macro_settings());
    let worker_error = lock(&state.worker_error).clone();
    for item in &mut mods {
        if let Some(loaded) = lock(&state.loaded).iter().find(|m| m["id"] == item["id"]) {
            item["loaded"] = loaded["loaded"].clone();
            item["error"] = loaded["error"].clone();
            item["ui"] = loaded["ui"].clone();
        }
        if item["enabled"] == true && item["loaded"] != true {
            if let Some(error) = worker_error.as_ref() {
                item["error"] = json!(error);
            }
        }
    }
    let base = if cfg!(windows) {
        "http://acruxmod.localhost"
    } else {
        "acruxmod://localhost"
    };
    for item in &mut mods {
        if let Some(ui) = item["ui"].as_object_mut() {
            for value in ui.values_mut() {
                if let Some(path) = value.as_str() {
                    *value = json!(format!("{base}/{}", path.trim_start_matches("/mods/")));
                }
            }
        }
    }
    json!({"error":if mods.is_empty(){worker_error}else{None},"mods":mods,"folder":state.root.join("mods"),"apis":lock(&state.apis).clone()})
}
#[cfg(desktop)]
thread_local! {
    static DELIVERING_HOOK: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Call after stopping Runtime, outside its mutex. Stop/shutdown hooks may save
/// their settings, but cannot submit new game input while the host is closing.
pub(crate) fn shutdown_worker() -> Result<(), String> {
    let Ok(state) = state() else { return Ok(()) };
    state.closing.store(true, Ordering::Release);
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut held = loop {
        match state.worker.try_lock() {
            Ok(held) => break held,
            Err(std::sync::TryLockError::Poisoned(error)) => break error.into_inner(),
            Err(std::sync::TryLockError::WouldBlock) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(10))
            }
            Err(_) => {
                if let Some(process) = lock(&state.worker_process).take() {
                    let mut child = lock(&process);
                    let _ = child.kill();
                    let _ = child.wait();
                }
                return Err("모드 종료 훅의 제한 시간이 지나 실행기를 종료했습니다.".into());
            }
        }
    };
    let result = if let Some(worker) = held.as_mut() {
        worker
            .request_for(
                "shutdown",
                json!({}),
                deadline.saturating_duration_since(Instant::now()),
            )
            .map(|_| ())
    } else {
        Ok(())
    };
    // Drop kills and reaps the child even when an extension ignores shutdown.
    *held = None;
    lock(&state.worker_process).take();
    result
}

/// Runtime callers only enqueue. One bounded dispatcher owns native extension events;
/// extension logs never recursively generate another extension log event.
pub(crate) fn emit_hook(event: &str, args: Vec<Value>) {
    #[cfg(desktop)]
    {
        if event == "ready"
            || STATE
                .get()
                .is_some_and(|state| state.closing.load(Ordering::Acquire))
            || DELIVERING_HOOK.with(|active| active.get())
        {
            return;
        }
        static EVENTS: OnceLock<mpsc::SyncSender<(String, Vec<Value>)>> = OnceLock::new();
        let events = EVENTS.get_or_init(|| {
            let (sender, receiver) = mpsc::sync_channel::<(String, Vec<Value>)>(64);
            std::thread::Builder::new().name("acrux-mod-events".into()).spawn(move || {
                while let Ok((event, args)) = receiver.recv() {
                    DELIVERING_HOOK.with(|active| active.set(true));
                    let result = (|| {
                        let state = state()?;
                        let mut worker = lock(&state.worker);
                        if let Some(worker) = worker.as_mut() {
                            worker.request("event", json!({"event":event,"args":args,"config":crate::macro_settings()}))?;
                        }
                        Ok::<(), String>(())
                    })();
                    if let Err(error) = result { if let Ok(state) = state() { *lock(&state.worker_error) = Some(error); } }
                    DELIVERING_HOOK.with(|active| active.set(false));
                }
            }).expect("start extension event dispatcher");
            sender
        });
        let _ = events.try_send((event.to_owned(), args));
    }
    #[cfg(not(desktop))]
    let _ = (event, args);
}

/// Call outside Runtime's mutex. An extension may synchronously request native I/O.
pub(crate) fn mod_hook(event: &str, args: Vec<Value>) -> Result<Value, String> {
    let Some(state) = STATE.get() else {
        return Ok(json!({"results":[],"args":args}));
    };
    let mut worker = match state.worker.try_lock() {
        Ok(w) => w,
        Err(std::sync::TryLockError::Poisoned(e)) => e.into_inner(),
        Err(_) => return Err("모드 실행기가 다른 요청을 처리 중입니다.".into()),
    };
    let Some(worker) = worker.as_mut() else {
        return Ok(json!({"results":[],"args":args}));
    };
    worker.request("event", json!({"event":event,"args":args}))
}

fn update_check(state: &State, refresh: bool) -> Result<Value, String> {
    let mut release = release_asset(state, refresh)?;
    let wanted = expected_hash(&fetch(
        release["shaUrl"].as_str().ok_or("검증 주소가 없습니다.")?,
        4096,
    )?)?;
    release["sha256"] = json!(wanted);
    release["current"] = json!(env!("CARGO_PKG_VERSION"));
    #[cfg(desktop)]
    {
        let current = std::env::current_exe().map_err(|e| e.to_string())?;
        let executable_hash = digest(&read_bounded(&current, 512 * 1024 * 1024)?);
        #[cfg(target_os = "macos")]
        let installed = fs::read(state.root.join("updates/installed.json"))
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
            .is_some_and(|receipt| {
                receipt["assetSha256"] == wanted && receipt["executableSha256"] == executable_hash
            });
        #[cfg(not(target_os = "macos"))]
        let installed = executable_hash == wanted;
        release["available"] = json!(!installed);
    }
    Ok(release)
}
#[cfg(desktop)]
fn update_download(state: &State) -> Result<Value, String> {
    let mut release = update_check(state, true)?;
    let bytes = fetch(
        release["url"].as_str().ok_or("다운로드 주소가 없습니다.")?,
        512 * 1024 * 1024,
    )?;
    if digest(&bytes) != release["sha256"].as_str().unwrap_or("") {
        return Err("업데이트 파일 SHA256이 일치하지 않습니다.".into());
    }
    let folder = state.root.join("updates");
    fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
    let file = folder.join(release["name"].as_str().ok_or("파일 이름이 없습니다.")?);
    atomic(&file, &bytes)?;
    release["path"] = json!(file);
    release["ready"] = json!(true);
    atomic(
        &folder.join("download.json"),
        serde_json::to_string(&release).unwrap().as_bytes(),
    )?;
    Ok(release)
}
#[cfg(desktop)]
fn update_install(app: &AppHandle, state: &State) -> Result<Value, String> {
    let metadata: Value = serde_json::from_slice(&read_bounded(
        &state.root.join("updates/download.json"),
        64 * 1024,
    )?)
    .map_err(|e| e.to_string())?;
    let source = PathBuf::from(
        metadata["path"]
            .as_str()
            .ok_or("받은 업데이트가 없습니다.")?,
    );
    let root = state
        .root
        .join("updates")
        .canonicalize()
        .map_err(|e| e.to_string())?;
    let source = source.canonicalize().map_err(|e| e.to_string())?;
    if source.parent() != Some(root.as_path())
        || digest(&read_bounded(&source, 512 * 1024 * 1024)?)
            != metadata["sha256"].as_str().unwrap_or("")
    {
        return Err("받은 업데이트를 다시 확인하세요.".into());
    }
    let current = std::env::current_exe().map_err(|e| e.to_string())?;
    #[cfg(windows)]
    {
        let quote = |s: &Path| format!("'{}'", s.to_string_lossy().replace('\'', "''"));
        let script = state.root.join("updates/install.ps1");
        let pid = std::process::id();
        let text=format!("$ErrorActionPreference='Stop'\nwhile(Get-Process -Id {pid} -ErrorAction SilentlyContinue){{Start-Sleep -Milliseconds 100}}\nCopy-Item -LiteralPath {} -Destination {} -Force\nStart-Process -FilePath {}\n",quote(&source),quote(&current),quote(&current));
        atomic(&script, text.as_bytes())?;
        let mut command = Command::new("powershell.exe");
        command
            .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-File"])
            .arg(script);
        quiet(&mut command);
        command.spawn().map_err(|e| e.to_string())?;
        let app = app.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(400));
            crate::runtime().stop("앱 업데이트");
            app.exit(0);
        });
    }
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::fs::PermissionsExt;
        let next = current.with_extension("update-new");
        fs::copy(&source, &next).map_err(|e| e.to_string())?;
        fs::set_permissions(&next, fs::Permissions::from_mode(0o755)).map_err(|e| e.to_string())?;
        let backup = current.with_extension("update-old");
        if backup.exists() {
            fs::remove_file(&backup).map_err(|e| e.to_string())?;
        }
        fs::rename(&current, &backup).map_err(|e| e.to_string())?;
        if let Err(error) = fs::rename(&next, &current) {
            let _ = fs::rename(&backup, &current);
            return Err(error.to_string());
        }
        crate::runtime().stop("앱 업데이트");
        app.request_restart();
    }
    #[cfg(target_os = "macos")]
    {
        let app_root = current
            .ancestors()
            .nth(3)
            .filter(|p| p.extension().is_some_and(|e| e == "app"))
            .ok_or("실행 중인 macOS 앱 묶음을 찾지 못했습니다.")?;
        let unpacked = state.root.join("updates/macos");
        if unpacked.exists() {
            fs::remove_dir_all(&unpacked).map_err(|e| e.to_string())?;
        }
        fs::create_dir(&unpacked).map_err(|e| e.to_string())?;
        let status = Command::new("/usr/bin/ditto")
            .args(["-x", "-k"])
            .arg(&source)
            .arg(&unpacked)
            .status()
            .map_err(|e| e.to_string())?;
        if !status.success() {
            return Err("macOS 업데이트 압축을 풀지 못했습니다.".into());
        }
        let next = unpacked.join("Acrux.app");
        let id = command_output(
            Command::new("/usr/libexec/PlistBuddy")
                .args(["-c", "Print CFBundleIdentifier"])
                .arg(next.join("Contents/Info.plist")),
            Duration::from_secs(5),
        )?;
        if String::from_utf8_lossy(&id).trim() != app.config().identifier {
            return Err("다른 앱의 업데이트 파일입니다.".into());
        }
        let backup = app_root.with_extension("app.previous");
        if backup.exists() {
            fs::remove_dir_all(&backup).map_err(|e| e.to_string())?;
        }
        fs::rename(app_root, &backup).map_err(|e| e.to_string())?;
        if let Err(error) = fs::rename(&next, app_root) {
            let _ = fs::rename(&backup, app_root);
            return Err(error.to_string());
        }
        let executable_hash = digest(&read_bounded(
            &app_root.join("Contents/MacOS/acrux"),
            512 * 1024 * 1024,
        )?);
        atomic(
            &state.root.join("updates/installed.json"),
            serde_json::to_vec(
                &json!({"assetSha256":metadata["sha256"],"executableSha256":executable_hash}),
            )
            .map_err(|e| e.to_string())?
            .as_slice(),
        )?;
        crate::runtime().stop("앱 업데이트");
        app.request_restart();
    }
    Ok(json!({"ok":true,"restart":true}))
}

#[tauri::command]
pub(crate) async fn service_action(
    app: AppHandle,
    action: String,
    payload: Value,
) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || service(&app, &action, payload))
        .await
        .map_err(|e| e.to_string())?
}
fn service(app: &AppHandle, action: &str, payload: Value) -> Result<Value, String> {
    let state = state()?;
    match action {
        "diagnostic" => {
            diagnostic(payload["message"].as_str().unwrap_or("UI error"));
            Ok(json!({"ok":true}))
        }
        "runtime_metadata" => {
            let runtime = crate::runtime();
            let logs: Vec<_> = runtime
                .log
                .iter()
                .map(|(at, line)| json!({"time":*at as f64/1000.0,"msg":line}))
                .collect();
            Ok(json!({"logs":logs,"stats":runtime.macro_config()["stats"]}))
        }
        "changelog" => changelog(state, payload["refresh"].as_bool().unwrap_or(false)),
        "open_web" => {
            let url = link(payload["site"].as_str().unwrap_or(""))?;
            #[cfg(target_os = "android")]
            return native(app, "openWeb", json!({"url":url}));
            #[cfg(desktop)]
            {
                open::that(url).map_err(|e| e.to_string())?;
                Ok(json!({"ok":true}))
            }
        }
        "open_folder" | "mods_folder" => {
            let folder = if action == "mods_folder" {
                state.root.join("mods")
            } else {
                state.root.clone()
            };
            fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
            #[cfg(target_os = "android")]
            return native(app, "openFolder", json!({"path":folder}));
            #[cfg(desktop)]
            {
                open_path(&folder)?;
                Ok(json!({"ok":true,"path":folder}))
            }
        }
        "restart" => {
            crate::runtime().stop("앱 다시 시작");
            #[cfg(target_os = "android")]
            return native(app, "restart", json!({}));
            #[cfg(desktop)]
            {
                let app = app.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(Duration::from_millis(350));
                    app.request_restart();
                });
                Ok(json!({"ok":true}))
            }
        }
        "uninstall" => {
            #[cfg(target_os = "android")]
            return native(app, "uninstall", json!({}));
            #[cfg(desktop)]
            {
                if payload["confirm"] != true {
                    return Err("앱 데이터 삭제를 먼저 확인하세요.".into());
                }
                crate::runtime().stop("앱 데이터 삭제");
                let parent = state
                    .root
                    .parent()
                    .ok_or("앱 데이터 경로가 올바르지 않습니다.")?;
                if state.root.file_name().and_then(|s| s.to_str())
                    != Some(app.config().identifier.as_str())
                    || parent == state.root
                {
                    return Err("앱 데이터 삭제 대상이 올바르지 않습니다.".into());
                }
                let _ = shutdown_worker();
                fs::remove_dir_all(&state.root).map_err(|e| e.to_string())?;
                let app = app.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(Duration::from_millis(350));
                    app.exit(0);
                });
                Ok(json!({"ok":true}))
            }
        }
        "update_check" => {
            let release = update_check(state, true)?;
            #[cfg(target_os = "android")]
            return native(app, "updateCheck", release);
            #[cfg(desktop)]
            Ok(release)
        }
        "update_download" => {
            #[cfg(target_os = "android")]
            {
                let release = update_check(state, true)?;
                return native(app, "updateDownload", release);
            }
            #[cfg(desktop)]
            update_download(state)
        }
        "update_install" => {
            #[cfg(target_os = "android")]
            return native(app, "updateInstall", json!({}));
            #[cfg(desktop)]
            update_install(app, state)
        }
        "update_state" => {
            #[cfg(target_os = "android")]
            return native(app, "updateState", json!({}));
            #[cfg(desktop)]
            Ok(fs::read(state.root.join("updates/download.json"))
                .ok()
                .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
                .unwrap_or_else(
                    || json!({"busy":false,"available":false,"current":env!("CARGO_PKG_VERSION")}),
                ))
        }
        "mods_list" => {
            #[cfg(desktop)]
            if metadata(&state.root.join("mods"), &crate::macro_settings())
                .iter()
                .any(|item| item["enabled"] == true)
            {
                if let Err(error) = start_worker(state) {
                    *lock(&state.worker_error) = Some(error);
                }
            }
            Ok(mod_list(state))
        }
        "mods_install" => {
            let bytes = if let Some(path) = payload["path"].as_str() {
                read_bounded(Path::new(path), MAX_PACKAGE)?
            } else {
                byte_payload(&payload)?
            };
            #[cfg(desktop)]
            {
                python(&state.root)?;
            }
            let info = install(&bytes, &state.root.join("mods"))?;
            let id = info["id"].as_str().unwrap();
            save_mod(id, Some(true))?;
            let mut result = mod_list(state);
            result["ok"] = json!(true);
            result["name"] = info["name"].clone();
            result["restart"] = json!(true);
            Ok(result)
        }
        "mods_toggle" | "mods_remove" => {
            let id = payload["id"]
                .as_str()
                .filter(|s| valid_id(s))
                .ok_or("모드 ID가 올바르지 않습니다.")?;
            let folder = state.root.join("mods").join(id);
            if folder.is_symlink() || !folder.is_dir() {
                return Err("설치되지 않은 모드입니다.".into());
            }
            if action == "mods_remove" {
                fs::remove_dir_all(folder).map_err(|e| e.to_string())?;
                save_mod(id, None)?;
            } else {
                save_mod(
                    id,
                    Some(payload["on"].as_bool().ok_or("켜기 상태가 없습니다.")?),
                )?;
            }
            let mut result = mod_list(state);
            result["ok"] = json!(true);
            result["restart"] = json!(true);
            Ok(result)
        }
        "mods_call" => {
            let name = payload["name"]
                .as_str()
                .ok_or("모드 API 이름이 없습니다.")?;
            if !lock(&state.apis).iter().any(|s| s == name) {
                return Err("등록된 모드 API가 아닙니다.".into());
            }
            let mut held = lock(&state.worker);
            held.as_mut()
                .ok_or("모드 실행기가 준비되지 않았습니다.")?
                .request("call", json!({"name":name,"payload":payload["payload"]}))
        }
        "mods_hook" => mod_hook(
            payload["event"].as_str().unwrap_or(""),
            payload["args"].as_array().cloned().unwrap_or_default(),
        ),
        _ => Err("알 수 없는 지원 기능입니다.".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn development_updates_never_accept_the_legacy_alpha_or_stable_release() {
        let mut release = json!({"tag_name":"dev-alpha","draft":false,"assets":[]});
        assert!(validate_dev_release(&release).is_ok());
        for tag in ["alpha", "v3.2.3", "mobile-latest"] {
            release["tag_name"] = json!(tag);
            assert!(validate_dev_release(&release).is_err());
        }
        release["tag_name"] = json!(DEV_RELEASE);
        release["draft"] = true.into();
        assert!(validate_dev_release(&release).is_err());
    }

    use zip::write::SimpleFileOptions;
    struct Folder(PathBuf);
    impl Folder {
        fn new() -> Self {
            let path =
                std::env::temp_dir().join(format!("acrux-mod-test-{}", uuid::Uuid::new_v4()));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for Folder {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn archive(entries: &[(&str, &str)]) -> Vec<u8> {
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for (name, body) in entries {
            zip.start_file(
                *name,
                SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored),
            )
            .unwrap();
            zip.write_all(body.as_bytes()).unwrap();
        }
        zip.finish().unwrap().into_inner()
    }
    #[test]
    fn installs_original_sdk_package_and_preserves_valid_install_on_invalid_update() {
        let root = Folder::new();
        let bytes = archive(&[
            (
                "sample/mod.json",
                r#"{"id":"sample","name":"Sample","entry":"main"}"#,
            ),
            ("sample/main.py", "def setup(ctx): pass\n"),
            ("sample/ui/main.js", "window.sample = true;"),
        ]);
        assert_eq!(install(&bytes, &root.0).unwrap()["id"], "sample");
        assert!(root.0.join("sample/ui/main.js").is_file());
        let invalid = archive(&[("mod.json", r#"{"id":"sample"}"#)]);
        assert!(install(&invalid, &root.0).is_err());
        assert_eq!(
            fs::read_to_string(root.0.join("sample/main.py")).unwrap(),
            "def setup(ctx): pass\n"
        );
    }
    #[test]
    fn packages_cannot_escape_their_folder_or_reactivate_retired_module() {
        let root = Folder::new();
        let traversal = archive(&[
            ("mod.json", r#"{"id":"sample"}"#),
            ("main.py", ""),
            ("../outside.py", "escaped"),
        ]);
        assert!(install(&traversal, &root.0).is_err());
        assert!(!root.0.parent().unwrap().join("outside.py").exists());
        let retired = archive(&[("mod.json", r#"{"id":"sniping"}"#), ("main.py", "")]);
        assert!(install(&retired, &root.0).is_err());
        assert!(!root.0.join("sniping").exists());
    }
    #[test]
    fn checksum_and_file_input_reject_corruption_instead_of_accepting_partial_bytes() {
        let hash = digest(b"verified release");
        assert_eq!(
            expected_hash(format!("{}  Acrux-Alpha.exe\n", hash.to_uppercase()).as_bytes())
                .unwrap(),
            hash
        );
        assert!(expected_hash(b"1234 release").is_err());
        assert_eq!(
            byte_payload(&json!({"bytes":[0,128,255]})).unwrap(),
            vec![0, 128, 255]
        );
        for values in [json!([-1]), json!([256]), json!([1.5]), json!(["1"])] {
            assert!(byte_payload(&json!({"bytes":values})).is_err());
        }
    }
}

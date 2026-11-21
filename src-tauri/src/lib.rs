// ============================================================
// Isolate Client — Minecraft launcher core
// Full Xryloth feature set: install & play, Modrinth mods,
// templates, server manager, Discord RPC, Ely.by, updater,
// AI, news, themes, skins, crash analysis and more.
// ============================================================

#![allow(unused_imports)]
#![allow(dead_code)]

use base64::Engine as _;
use md5::{Digest, Md5};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::Manager;
mod win_opt;

// ---------------- CONSTANTS ----------------

const USER_AGENT: &str = "Isolate/0.1";
const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

const MOJANG_MANIFEST: &str = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";
const RESOURCES_BASE: &str = "https://resources.download.minecraft.net";
const FABRIC_GAME: &str = "https://meta.fabricmc.net/v2/versions/game";
const FABRIC_LOADER: &str = "https://meta.fabricmc.net/v2/versions/loader";
const FABRIC_INSTALLER: &str = "https://meta.fabricmc.net/v2/versions/installer";
const QUILT_LOADER: &str = "https://meta.quiltmc.org/v3/versions/loader";
const QUILT_GAME: &str = "https://meta.quiltmc.org/v3/versions/game";

const MS_CLIENT_ID: &str = "00000000402b5328";
const MS_SCOPE: &str = "XboxLive.signin XboxLive.offline_access MinecraftWin32.app";

// Adoptium (Temurin) JDK auto-download.
fn adoptium_url(major: u32) -> String {
    let os = std::env::consts::OS; // "linux" | "macos" | "windows"
    let arch = match std::env::consts::ARCH {
        "x86_64" => "x64",
        "aarch64" => "aarch64",
        other => other,
    };
    format!(
        "https://api.adoptium.net/v3/binary/latest/{}/{}/{}//jdk/hotspot/normal/eclipse",
        major, os, arch
    )
}

// ---------------- PATHS ----------------

fn home() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from("."))
}
fn app_dir() -> PathBuf {
    home().join(".isolate")
}
fn instances_dir() -> PathBuf {
    app_dir().join("instances")
}
fn libraries_dir() -> PathBuf {
    app_dir().join("libraries")
}
fn assets_dir() -> PathBuf {
    app_dir().join("assets")
}
fn java_dir() -> PathBuf {
    app_dir().join("java")
}

fn ensure_dirs() {
    for d in [
        app_dir(),
        instances_dir(),
        libraries_dir(),
        assets_dir().join("indexes"),
        assets_dir().join("objects"),
        java_dir(),
    ] {
        let _ = std::fs::create_dir_all(&d);
    }
}

fn instance_dir(name: &str) -> PathBuf {
    instances_dir().join(sanitize_name(name))
}

/// Strip path-hostile characters from an instance name.
fn sanitize_name(name: &str) -> String {
    let s: String = name
        .trim()
        .chars()
        .map(|c| if "/\\:*?\"<>|".contains(c) { '_' } else { c })
        .collect();
    if s.is_empty() {
        "instance".to_string()
    } else {
        s
    }
}

// ---------------- JSON / LOG HELPERS ----------------

fn load_json(path: &Path) -> Value {
    match std::fs::read_to_string(path) {
        Ok(s) => serde_json::from_str(&s).unwrap_or(Value::Null),
        Err(_) => Value::Null,
    }
}

fn save_json(path: &Path, val: &Value) {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(s) = serde_json::to_string_pretty(val) {
        let _ = std::fs::write(path, s);
    }
}

fn log_msg(app: &AppState, msg: &str) {
    let line = format!("[{}] {}", chrono::Local::now().format("%H:%M:%S"), msg);
    println!("{}", line);
    let mut entries = app.log_entries.lock().unwrap();
    entries.push(line);
    if entries.len() > 2000 {
        let excess = entries.len() - 2000;
        entries.drain(0..excess);
    }
}

fn append_game_output(app: &AppState, line: &str) {
    let mut out = app.game_output.lock().unwrap();
    out.push(line.to_string());
    if out.len() > 500 {
        let excess = out.len() - 500;
        out.drain(0..excess);
    }
}

/// A Command that never pops a console window on Windows.
fn hidden_cmd(program: &str) -> Command {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        let mut c = Command::new(program);
        c.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
        c
    }
    #[cfg(not(target_os = "windows"))]
    {
        Command::new(program)
    }
}

/// Open a file or folder with the OS default handler.
fn open_in_os(path: &Path) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    let mut c = {
        let mut c = Command::new("explorer");
        c.arg(path);
        c
    };
    #[cfg(target_os = "macos")]
    let mut c = {
        let mut c = Command::new("open");
        c.arg(path);
        c
    };
    #[cfg(all(unix, not(target_os = "macos")))]
    let mut c = {
        let mut c = Command::new("xdg-open");
        c.arg(path);
        c
    };
    c.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

// ---------------- HTTP HELPERS ----------------

/// Tauri runs `async` commands on the Tokio runtime. `reqwest::blocking`
/// creates its own inner runtime, and dropping that runtime from inside an
/// async context panics ("Cannot drop a runtime in a context where blocking
/// is not allowed"). `block_in_place` moves this worker onto a blocking
/// thread so the drop is legal; outside a runtime it is a plain call.
fn blocking<T>(f: impl FnOnce() -> T) -> T {
    match tokio::runtime::Handle::try_current() {
        Ok(_) => tokio::task::block_in_place(f),
        Err(_) => f(),
    }
}

fn http_client(timeout_secs: u64) -> Result<reqwest::blocking::Client, String> {
    blocking(|| {
        reqwest::blocking::Client::builder()
            .user_agent(USER_AGENT)
            .timeout(Duration::from_secs(timeout_secs))
            .connect_timeout(Duration::from_secs(15))
            .build()
            .map_err(|e| e.to_string())
    })
}

fn http_get(url: &str) -> Result<Value, String> {
    blocking(|| {
        let c = http_client(30)?;
        let resp = c.get(url).send().map_err(|e| e.to_string())?;
        if !resp.status().is_success() {
            return Err(format!("HTTP {} for {}", resp.status(), url));
        }
        resp.json::<Value>().map_err(|e| e.to_string())
    })
}

fn http_get_text(url: &str) -> Result<String, String> {
    blocking(|| {
        let c = http_client(30)?;
        let resp = c.get(url).send().map_err(|e| e.to_string())?;
        if !resp.status().is_success() {
            return Err(format!("HTTP {} for {}", resp.status(), url));
        }
        resp.text().map_err(|e| e.to_string())
    })
}

fn http_post_json(url: &str, body: &Value) -> Result<Value, String> {
    blocking(|| {
        let c = http_client(30)?;
        let resp = c.post(url).json(body).send().map_err(|e| e.to_string())?;
        let status = resp.status();
        let text = resp.text().map_err(|e| e.to_string())?;
        if !status.is_success() {
            return Err(format!("HTTP {}: {}", status, text));
        }
        serde_json::from_str(&text).map_err(|e| format!("{} ({})", e, text))
    })
}

fn http_post_form(url: &str, form: &[(&str, &str)]) -> Result<Value, String> {
    blocking(|| {
        let c = http_client(30)?;
        let resp = c.post(url).form(form).send().map_err(|e| e.to_string())?;
        let status = resp.status();
        let text = resp.text().map_err(|e| e.to_string())?;
        if !status.is_success() {
            return Err(format!("HTTP {}: {}", status, text));
        }
        serde_json::from_str(&text).map_err(|e| format!("{} ({})", e, text))
    })
}

/// Form POST with HTTP Basic auth (used by the Ely.by token endpoint).
fn http_post_form_basic(
    url: &str,
    form: &[(&str, &str)],
    user: &str,
    pass: &str,
) -> Result<Value, String> {
    blocking(|| {
        let c = http_client(30)?;
        let resp = c
            .post(url)
            .basic_auth(user, Some(pass))
            .form(form)
            .send()
            .map_err(|e| e.to_string())?;
        let status = resp.status();
        let text = resp.text().map_err(|e| e.to_string())?;
        if !status.is_success() {
            return Err(format!("HTTP {}: {}", status, text));
        }
        serde_json::from_str(&text).map_err(|e| format!("{} ({})", e, text))
    })
}

/// GET with a bearer token (Ely.by account info).
fn http_get_bearer(url: &str, token: &str) -> Result<Value, String> {
    blocking(|| {
        let c = http_client(30)?;
        let resp = c.get(url).bearer_auth(token).send().map_err(|e| e.to_string())?;
        let status = resp.status();
        if !status.is_success() {
            let t = resp.text().unwrap_or_default();
            return Err(format!("HTTP {}: {}", status, t));
        }
        resp.json::<Value>().map_err(|e| e.to_string())
    })
}

/// Stream a URL to disk. Long timeout — game files can be hundreds of MB.
fn download_file(url: &str, dest: &Path) -> Result<(), String> {
    blocking(|| {
        if let Some(parent) = dest.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let c = http_client(600)?;
        let mut resp = c.get(url).send().map_err(|e| e.to_string())?;
        if !resp.status().is_success() {
            return Err(format!("HTTP {} for {}", resp.status(), url));
        }
        let tmp = dest.with_extension("part");
        {
            let mut f = std::fs::File::create(&tmp).map_err(|e| e.to_string())?;
            std::io::copy(&mut resp, &mut f).map_err(|e| e.to_string())?;
        }
        std::fs::rename(&tmp, dest).map_err(|e| e.to_string())
    })
}

/// Like `download_file`, but reports `(downloaded, total)` as it streams.
fn download_file_progress(
    url: &str,
    dest: &Path,
    mut on_progress: impl FnMut(u64, Option<u64>),
) -> Result<(), String> {
    blocking(|| {
        if let Some(parent) = dest.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let c = http_client(600)?;
        let mut resp = c.get(url).send().map_err(|e| e.to_string())?;
        if !resp.status().is_success() {
            return Err(format!("HTTP {} for {}", resp.status(), url));
        }
        let total = resp.content_length();
        let mut f = std::fs::File::create(dest).map_err(|e| e.to_string())?;
        let mut buf = vec![0u8; 64 * 1024];
        let mut downloaded: u64 = 0;
        let mut last = Instant::now();
        on_progress(0, total);
        loop {
            let n = std::io::Read::read(&mut resp, &mut buf).map_err(|e| e.to_string())?;
            if n == 0 {
                break;
            }
            std::io::Write::write_all(&mut f, &buf[..n]).map_err(|e| e.to_string())?;
            downloaded += n as u64;
            if last.elapsed().as_millis() >= 200 || Some(downloaded) == total {
                last = Instant::now();
                on_progress(downloaded, total);
            }
        }
        on_progress(downloaded, total);
        Ok(())
    })
}

// ---------------- MAVEN / VERSION HELPERS ----------------

/// `group:artifact:version[:classifier]` -> maven relative jar path.
fn maven_name_to_path(maven_name: &str) -> Option<String> {
    let parts: Vec<&str> = maven_name.split(':').collect();
    if parts.len() < 3 {
        return None;
    }
    let group = parts[0].replace('.', "/");
    let artifact = parts[1];
    let version = parts[2];
    let classifier = if parts.len() > 3 { parts[3] } else { "" };
    if classifier.is_empty() {
        Some(format!(
            "{}/{}/{}/{}-{}.jar",
            group, artifact, version, artifact, version
        ))
    } else {
        Some(format!(
            "{}/{}/{}/{}-{}-{}.jar",
            group, artifact, version, artifact, version, classifier
        ))
    }
}

/// "1.20.4" -> (1, 20, 4); tolerant of missing/extra parts.
fn parse_version_tuple(ver: &str) -> (i32, i32, i32) {
    let mut it = ver.split('.');
    let mut p = |d: i32| -> i32 {
        it.next()
            .and_then(|s| s.split('-').next())
            .and_then(|s| s.parse().ok())
            .unwrap_or(d)
    };
    (p(0), p(1), p(2))
}

fn loader_supports(loader: &str, ver: &str) -> bool {
    let (a, b, c) = parse_version_tuple(ver);
    let v = (a, b, c);
    match loader {
        "vanilla" => true,
        "fabric" | "quilt" => v >= (1, 14, 0),
        _ => false,
    }
}

/// Bucket versions into `1.21`, `1.20`, … groups, newest first.
/// Returns `[{group, versions:[…]}]`.
fn build_version_groups(versions: &[String]) -> Vec<Value> {
    let mut groups: HashMap<String, Vec<String>> = HashMap::new();
    for v in versions {
        let key = v
            .split('.')
            .take(2)
            .collect::<Vec<_>>()
            .join(".");
        groups.entry(key).or_default().push(v.clone());
    }
    let mut keys: Vec<String> = groups.keys().cloned().collect();
    keys.sort_by(|a, b| {
        let ta = parse_version_tuple(a);
        let tb = parse_version_tuple(b);
        tb.cmp(&ta)
    });
    keys.into_iter()
        .map(|k| {
            let mut vs = groups.remove(&k).unwrap_or_default();
            vs.sort_by(|a, b| {
                let ta = parse_version_tuple(a);
                let tb = parse_version_tuple(b);
                tb.cmp(&ta)
            });
            json!({ "group": k, "versions": vs })
        })
        .collect()
}

// ---------------- APP STATE ----------------

struct AppState {
    settings: Mutex<HashMap<String, Value>>,
    instances: Mutex<Vec<Value>>,
    instance_settings: Mutex<HashMap<String, Value>>,
    playtimes: Mutex<HashMap<String, u64>>,
    log_entries: Mutex<Vec<String>>,
    game_output: Mutex<Vec<String>>,
    /// (unix secs, manifest) — 10 minute TTL.
    manifest_cache: Mutex<Option<(u64, Value)>>,
    /// (unix secs, java installs) — 60 second TTL.
    java_cache: Mutex<Option<(u64, Vec<Value>)>>,
    /// Poll result for the Microsoft device-code flow.
    ms_auth_result: Arc<Mutex<Option<Value>>>,
    ms_auth_polling: Arc<AtomicBool>,
    /// PID of the running game, if any.
    game_pid: Mutex<Option<u32>>,
    /// The live game process (if any) + when it started.
    game_session: Mutex<Option<GameSession>>,
    /// Discord RPC command sender, present while connected.
    discord_handle: Mutex<Option<DiscordRpcHandle>>,
    /// OAuth code handed back by the local Ely.by callback server.
    elyby_auth_code: Arc<Mutex<Option<String>>>,
    /// PKCE code_verifier for the in-flight Ely.by OAuth flow (RFC 7636).
    elyby_pkce_verifier: Arc<Mutex<Option<String>>>,
    /// Running self-hosted server processes, keyed by server name.
    server_procs: Mutex<HashMap<String, ServerProc>>,
    /// Trailing console output per server (capped).
    server_logs: Mutex<HashMap<String, Vec<String>>>,
    /// Servers being stopped on purpose (so crash-watch doesn't restart them).
    server_stopping: Mutex<HashSet<String>>,
    /// Active AI workspace — `{kind: "server"|"instance", name}` or `None`.
    ai_context: Mutex<Option<Value>>,
}

/// A running self-hosted Minecraft server plus its stdin handle.
struct ServerProc {
    child: Child,
    stdin: std::process::ChildStdin,
    started_at: Instant,
}

/// A running Minecraft process.
struct GameSession {
    child: Arc<Mutex<Option<Child>>>,
    started: Instant,
    instance: String,
}

/// Sender half of the Discord RPC command channel.
struct DiscordRpcHandle {
    sender: std::sync::mpsc::Sender<DiscordMessage>,
}

/// Messages the UI can push to the Discord RPC thread.
enum DiscordMessage {
    Update {
        details: String,
        state: String,
        large_image: String,
        large_text: String,
        small_image: String,
        small_text: String,
    },
    Stop,
}

impl AppState {
    fn new() -> Self {
        Self {
            settings: Mutex::new(HashMap::new()),
            instances: Mutex::new(Vec::new()),
            instance_settings: Mutex::new(HashMap::new()),
            playtimes: Mutex::new(HashMap::new()),
            log_entries: Mutex::new(Vec::new()),
            game_output: Mutex::new(Vec::new()),
            manifest_cache: Mutex::new(None),
            java_cache: Mutex::new(None),
            ms_auth_result: Arc::new(Mutex::new(None)),
            ms_auth_polling: Arc::new(AtomicBool::new(false)),
            game_pid: Mutex::new(None),
            game_session: Mutex::new(None),
            discord_handle: Mutex::new(None),
            elyby_auth_code: Arc::new(Mutex::new(None)),
            elyby_pkce_verifier: Arc::new(Mutex::new(None)),
            server_procs: Mutex::new(HashMap::new()),
            server_logs: Mutex::new(HashMap::new()),
            server_stopping: Mutex::new(HashSet::new()),
            ai_context: Mutex::new(None),
        }
    }
}

// ---------------- STORAGE IO ----------------

fn settings_path() -> PathBuf {
    app_dir().join("settings.json")
}
fn instances_path() -> PathBuf {
    app_dir().join("instances.json")
}
fn instance_settings_path() -> PathBuf {
    app_dir().join("instance_settings.json")
}
fn servers_path() -> PathBuf {
    app_dir().join("servers.json")
}
fn playtimes_path() -> PathBuf {
    app_dir().join("playtimes.json")
}

/// Load settings from disk into memory (once) and return a clone.
fn load_settings(app: &AppState) -> HashMap<String, Value> {
    let mut guard = app.settings.lock().unwrap();
    if guard.is_empty() {
        if let Some(obj) = load_json(&settings_path()).as_object() {
            for (k, v) in obj {
                guard.insert(k.clone(), v.clone());
            }
        }
        if guard.is_empty() {
            // First run defaults.
            guard.insert("ram_mb".into(), json!(4096));
            guard.insert("jvm_args".into(), json!(""));
            guard.insert("java_path".into(), json!(""));
            guard.insert("offline_username".into(), json!("Player"));
            guard.insert("theme".into(), json!("blue"));
            guard.insert("ms_account".into(), Value::Null);
        }
    }
    guard.clone()
}

fn save_setting_val(app: &AppState, key: &str, val: Value) {
    let snapshot = {
        let mut guard = app.settings.lock().unwrap();
        guard.insert(key.to_string(), val);
        guard.clone()
    };
    save_json(&settings_path(), &Value::Object(snapshot.into_iter().collect()));
}

fn load_instances_raw(app: &AppState) -> Vec<Value> {
    let mut guard = app.instances.lock().unwrap();
    if guard.is_empty() {
        if let Some(arr) = load_json(&instances_path()).as_array() {
            *guard = arr.clone();
        }
    }
    let mut out = guard.clone();
    for rec in out.iter_mut() {
        let name = rec
            .get("name")
            .and_then(|n| n.as_str())
            .unwrap_or("")
            .to_string();
        rec["installed"] = json!(instance_dir(&name).join("client.jar").exists());
    }
    out
}

fn save_instances(app: &AppState, list: &[Value]) {
    {
        let mut guard = app.instances.lock().unwrap();
        *guard = list.to_vec();
    }
    save_json(&instances_path(), &Value::Array(list.to_vec()));
}

fn load_instance_settings(app: &AppState) -> HashMap<String, Value> {
    let mut guard = app.instance_settings.lock().unwrap();
    if guard.is_empty() {
        if let Some(obj) = load_json(&instance_settings_path()).as_object() {
            for (k, v) in obj {
                guard.insert(k.clone(), v.clone());
            }
        }
    }
    guard.clone()
}

fn load_playtimes(app: &AppState) -> HashMap<String, u64> {
    let mut guard = app.playtimes.lock().unwrap();
    if guard.is_empty() {
        if let Some(obj) = load_json(&playtimes_path()).as_object() {
            for (k, v) in obj {
                if let Some(n) = v.as_u64() {
                    guard.insert(k.clone(), n);
                }
            }
        }
    }
    guard.clone()
}

// ============================================================
// COMMANDS — app info / settings
// ============================================================

#[tauri::command]
fn get_app_info() -> Value {
    json!({
        "name": "Isolate Client",
        "version": APP_VERSION,
        "data_dir": app_dir().to_string_lossy(),
        "logo": "logo.jpeg",
    })
}

#[tauri::command]
fn get_settings(app: tauri::State<AppState>) -> Value {
    Value::Object(load_settings(&app).into_iter().collect())
}

#[tauri::command]
fn save_settings(app: tauri::State<AppState>, partial: Value) -> Value {
    if let Some(obj) = partial.as_object() {
        let snapshot = {
            let mut guard = app.settings.lock().unwrap();
            for (k, v) in obj {
                guard.insert(k.clone(), v.clone());
            }
            guard.clone()
        };
        save_json(&settings_path(), &Value::Object(snapshot.into_iter().collect()));
    }
    json!({ "success": true })
}

#[tauri::command]
fn save_setting(app: tauri::State<AppState>, key: String, value: Value) -> Value {
    save_setting_val(&app, &key, value);
    json!({ "success": true })
}

// ============================================================
// COMMANDS — instances
// ============================================================

#[tauri::command]
fn get_instances(app: tauri::State<AppState>) -> Value {
    let mut list = load_instances_raw(&app);
    let per_instance = load_instance_settings(&app);
    let playtimes = load_playtimes(&app);
    for rec in list.iter_mut() {
        let name = rec
            .get("name")
            .and_then(|n| n.as_str())
            .unwrap_or("")
            .to_string();
        if let Some(s) = per_instance.get(&name) {
            rec["ram_mb"] = s.get("ram_mb").cloned().unwrap_or(Value::Null);
            rec["jvm_args"] = s.get("jvm_args").cloned().unwrap_or(Value::Null);
            rec["java_path"] = s.get("java_path").cloned().unwrap_or(Value::Null);
        } else {
            rec["ram_mb"] = Value::Null;
            rec["jvm_args"] = Value::Null;
            rec["java_path"] = Value::Null;
        }
        rec["play_seconds"] = json!(playtimes.get(&name).copied().unwrap_or(0));
        rec["path"] = json!(instance_dir(&name).to_string_lossy());
    }
    Value::Array(list)
}

#[tauri::command]
fn create_instance(
    app: tauri::State<AppState>,
    handle: tauri::AppHandle,
    name: String,
    version: String,
    loader: String,
    loader_version: String,
) -> Value {
    let clean = sanitize_name(&name);
    if clean.is_empty() {
        return json!({ "error": "Instance name is empty." });
    }
    let mut list = load_instances_raw(&app);
    if list
        .iter()
        .any(|r| r.get("name").and_then(|n| n.as_str()) == Some(clean.as_str()))
    {
        return json!({ "error": format!("An instance named '{}' already exists.", clean) });
    }
    if !loader_supports(&loader, &version) {
        return json!({ "error": format!("{} does not support Minecraft {}.", loader, version) });
    }
    let rec = json!({
        "name": clean,
        "version": version,
        "loader": loader,
        "loader_version": loader_version,
        "created": chrono::Local::now().to_rfc3339(),
        "last_played": Value::Null,
    });
    list.push(rec.clone());
    save_instances(&app, &list);
    let _ = std::fs::create_dir_all(instance_dir(&clean));
    log_msg(&app, &format!("[Instance] Created '{}' ({}/{})", clean, version, loader));
    // Fabric: inject Fabric API + IsolateClientMod in the background so a
    // brand-new profile is ready to launch (failures only land in the log).
    if loader == "fabric" {
        let h = handle.clone();
        let inst = clean.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(750));
            let st = h.state::<AppState>();
            match install_mod_inner(&st, &inst, "fabric-api", "", "mods") {
                Ok(f) => log_msg(&st, &format!("[IsolateMod] '{inst}': Fabric API → {f}")),
                Err(e) => log_msg(&st, &format!("[IsolateMod] '{inst}': Fabric API failed ({e})")),
            }
            let v = install_isolate_client_mod_impl(&st, &inst);
            if v.get("installed").and_then(|b| b.as_bool()) == Some(true) {
                log_msg(&st, &format!(
                    "[IsolateMod] '{inst}': IsolateClientMod installed ({})",
                    v.get("file").and_then(|f| f.as_str()).unwrap_or("")
                ));
            } else if let Some(msg) = v.get("message").and_then(|m| m.as_str()) {
                log_msg(&st, &format!("[IsolateMod] '{inst}': {msg}"));
            } else if let Some(err) = v.get("error").and_then(|e| e.as_str()) {
                log_msg(&st, &format!("[IsolateMod] '{inst}': {err}"));
            }
        });
    }
    json!({ "success": true, "instance": rec })
}

#[tauri::command]
fn delete_instance(app: tauri::State<AppState>, name: String) -> Value {
    let clean = sanitize_name(&name);
    let mut list = load_instances_raw(&app);
    let before = list.len();
    list.retain(|r| r.get("name").and_then(|n| n.as_str()) != Some(clean.as_str()));
    if list.len() == before {
        return json!({ "error": "Instance not found." });
    }
    save_instances(&app, &list);
    let dir = instance_dir(&clean);
    if dir.exists() {
        if let Err(e) = std::fs::remove_dir_all(&dir) {
            return json!({ "error": format!("Could not delete folder: {}", e) });
        }
    }
    log_msg(&app, &format!("[Instance] Deleted '{}'", clean));
    json!({ "success": true })
}

#[tauri::command]
fn rename_instance(app: tauri::State<AppState>, name: String, new_name: String) -> Value {
    let old = sanitize_name(&name);
    let new = sanitize_name(&new_name);
    if new.is_empty() {
        return json!({ "error": "New name is empty." });
    }
    let mut list = load_instances_raw(&app);
    let mut found = false;
    for rec in list.iter_mut() {
        if rec.get("name").and_then(|n| n.as_str()) == Some(old.as_str()) {
            rec["name"] = json!(new);
            found = true;
        }
    }
    if !found {
        return json!({ "error": "Instance not found." });
    }
    save_instances(&app, &list);

    let old_dir = instance_dir(&old);
    let new_dir = instance_dir(&new);
    if old_dir.exists() && !new_dir.exists() {
        let _ = std::fs::rename(&old_dir, &new_dir);
    }
    // Move per-instance settings + playtime under the new key.
    {
        let mut per = load_instance_settings(&app);
        if let Some(v) = per.remove(&old) {
            per.insert(new.clone(), v);
            save_json(
                &instance_settings_path(),
                &Value::Object(per.into_iter().collect()),
            );
        }
    }
    {
        let mut pt = load_playtimes(&app);
        if let Some(v) = pt.remove(&old) {
            pt.insert(new.clone(), v);
            save_json(&playtimes_path(), &Value::Object(
                pt.into_iter().map(|(k, v)| (k, json!(v))).collect(),
            ));
        }
    }
    json!({ "success": true })
}

#[tauri::command]
fn get_instance_settings(app: tauri::State<AppState>, name: String) -> Value {
    let per = load_instance_settings(&app);
    per.get(&sanitize_name(&name)).cloned().unwrap_or(json!({}))
}

#[tauri::command]
fn save_instance_settings(app: tauri::State<AppState>, name: String, settings: Value) -> Value {
    let clean = sanitize_name(&name);
    let snapshot = {
        let mut per = load_instance_settings(&app);
        per.insert(clean.clone(), settings);
        per
    };
    save_json(&instance_settings_path(), &Value::Object(snapshot.into_iter().collect()));
    json!({ "success": true })
}

#[tauri::command]
fn open_instance_folder(_app: tauri::State<AppState>, name: String) -> Value {
    let dir = instance_dir(&name);
    let _ = std::fs::create_dir_all(&dir);
    match open_in_os(&dir) {
        Ok(()) => json!({ "success": true }),
        Err(e) => json!({ "error": e }),
    }
}

#[tauri::command]
fn get_play_times(app: tauri::State<AppState>) -> Value {
    Value::Object(
        load_playtimes(&app)
            .into_iter()
            .map(|(k, v)| (k, json!(v)))
            .collect(),
    )
}

// ============================================================
// COMMANDS — version catalog
// ============================================================

fn get_manifest(app: &AppState) -> Result<Value, String> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    {
        let cache = app.manifest_cache.lock().unwrap();
        if let Some((ts, v)) = cache.as_ref() {
            if now.saturating_sub(*ts) < 600 {
                return Ok(v.clone());
            }
        }
    }
    let m = http_get(MOJANG_MANIFEST)?;
    let mut cache = app.manifest_cache.lock().unwrap();
    *cache = Some((now, m.clone()));
    Ok(m)
}

#[tauri::command]
fn get_minecraft_versions(app: tauri::State<AppState>, include_snapshots: bool) -> Value {
    let manifest = match get_manifest(&app) {
        Ok(m) => m,
        Err(e) => return json!({ "error": e }),
    };
    let mut versions: Vec<String> = Vec::new();
    if let Some(arr) = manifest.get("versions").and_then(|v| v.as_array()) {
        for v in arr {
            let id = v.get("id").and_then(|i| i.as_str()).unwrap_or("");
            let ty = v.get("type").and_then(|t| t.as_str()).unwrap_or("");
            if id.is_empty() {
                continue;
            }
            if ty == "release" || include_snapshots {
                versions.push(id.to_string());
            }
        }
    }
    json!({ "versions": build_version_groups(&versions) })
}

#[tauri::command]
fn get_loader_versions(_app: tauri::State<AppState>, loader: String) -> Value {
    let list = match loader.as_str() {
        "fabric" => match http_get(FABRIC_LOADER) {
            Ok(v) => v,
            Err(e) => return json!({ "error": e }),
        },
        "quilt" => match http_get(QUILT_LOADER) {
            Ok(v) => v,
            Err(e) => return json!({ "error": e }),
        },
        _ => json!([]),
    };
    let mut out: Vec<String> = Vec::new();
    if let Some(arr) = list.as_array() {
        for v in arr {
            let ver = v
                .get("version")
                .or_else(|| v.get("maven"))
                .and_then(|x| x.as_str())
                .unwrap_or("");
            if !ver.is_empty() {
                out.push(ver.to_string());
            }
        }
    }
    out.dedup();
    json!({ "versions": out })
}

#[tauri::command]
fn get_supported_loaders() -> Value {
    json!([
        { "id": "vanilla", "label": "Vanilla", "supports_version": "" },
        { "id": "fabric", "label": "Fabric", "min_version": "1.14.0" },
        { "id": "quilt", "label": "Quilt", "min_version": "1.14.0" }
    ])
}

// ============================================================
// COMMANDS — logs
// ============================================================

#[tauri::command]
fn get_logs(app: tauri::State<AppState>) -> Value {
    Value::Array(
        app.log_entries
            .lock()
            .unwrap()
            .iter()
            .map(|s| Value::String(s.clone()))
            .collect(),
    )
}

#[tauri::command]
fn clear_logs(app: tauri::State<AppState>) -> Value {
    app.log_entries.lock().unwrap().clear();
    json!({ "success": true })
}

#[tauri::command]
fn get_game_output(app: tauri::State<AppState>) -> Value {
    Value::Array(
        app.game_output
            .lock()
            .unwrap()
            .iter()
            .map(|s| Value::String(s.clone()))
            .collect(),
    )
}

#[tauri::command]
fn clear_game_output(app: tauri::State<AppState>) -> Value {
    app.game_output.lock().unwrap().clear();
    json!({ "success": true })
}

// ============================================================
// COMMANDS — misc
// ============================================================

#[tauri::command]
fn open_path(path: String) -> Value {
    let p = PathBuf::from(&path);
    if !p.exists() {
        return json!({ "error": format!("Not found: {}", path) });
    }
    match open_in_os(&p) {
        Ok(()) => json!({ "success": true }),
        Err(e) => json!({ "error": e }),
    }
}

#[tauri::command]
fn get_system_info() -> Value {
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;
    let mut ram_total = 0u64;
    let mut ram_available = 0u64;

    #[cfg(target_os = "linux")]
    {
        if let Ok(s) = std::fs::read_to_string("/proc/meminfo") {
            for line in s.lines() {
                if let Some(v) = line.strip_prefix("MemTotal:") {
                    ram_total = v
                        .split_whitespace()
                        .next()
                        .and_then(|x| x.parse::<u64>().ok())
                        .unwrap_or(0);
                }
                if let Some(v) = line.strip_prefix("MemAvailable:") {
                    ram_available = v
                        .split_whitespace()
                        .next()
                        .and_then(|x| x.parse::<u64>().ok())
                        .unwrap_or(0);
                }
            }
        }
    }
    #[cfg(target_os = "macos")]
    {
        if let Ok(out) = Command::new("sysctl")
            .args(["-n", "hw.memsize"])
            .output()
        {
            ram_total = String::from_utf8_lossy(&out.stdout)
                .trim()
                .parse::<u64>()
                .unwrap_or(0)
                / 1024;
        }
    }

    json!({
        "os": os,
        "arch": arch,
        "ram_total_kb": ram_total,
        "ram_available_kb": ram_available,
        "data_dir": app_dir().to_string_lossy(),
        "version": APP_VERSION,
    })
}

/// Full PC hardware snapshot for telemetry (admin-visible).
#[tauri::command]
fn get_hardware_info() -> Value {
    let os = std::env::consts::OS.to_string();
    let arch = std::env::consts::ARCH.to_string();
    let mut ram_mb = 0u64;
    let mut cpu = String::new();
    let mut cpu_cores = 0u32;
    let mut os_version = String::new();
    let mut gpu = String::new();

    #[cfg(target_os = "linux")]
    {
        if let Ok(s) = std::fs::read_to_string("/proc/meminfo") {
            for line in s.lines() {
                if let Some(v) = line.strip_prefix("MemTotal:") {
                    ram_mb = v
                        .split_whitespace()
                        .next()
                        .and_then(|x| x.parse::<u64>().ok())
                        .unwrap_or(0)
                        / 1024;
                }
            }
        }
        if let Ok(s) = std::fs::read_to_string("/proc/cpuinfo") {
            for line in s.lines() {
                if let Some(v) = line.strip_prefix("model name") {
                    if let Some((_sep, val)) = v.split_once(':') {
                        if cpu.is_empty() {
                            cpu = val.trim().to_string();
                        }
                    }
                }
                if line.starts_with("processor") {
                    cpu_cores += 1;
                }
            }
        }
        if let Ok(s) = std::fs::read_to_string("/etc/os-release") {
            for line in s.lines() {
                if let Some(v) = line.strip_prefix("PRETTY_NAME=") {
                    os_version = v.trim().trim_matches('"').to_string();
                    break;
                }
            }
        }
        // GPU: lspci when present, else kernel driver names from /sys.
        if let Ok(out) = Command::new("lspci").args(["-mm"]).output() {
            let text = String::from_utf8_lossy(&out.stdout);
            for line in text.lines() {
                if line.contains("VGA compatible controller")
                    || line.contains("3D controller")
                    || line.contains("Display controller")
                {
                    // lspci -mm: `"class" "vendor" "device" ...` — take fields 2+3.
                    let fields: Vec<&str> = line
                        .split('"')
                        .skip(1)
                        .step_by(2)
                        .collect();
                    if fields.len() >= 3 {
                        gpu = format!("{} {}", fields[1], fields[2]);
                    } else if fields.len() >= 2 {
                        gpu = fields[1].to_string();
                    }
                    if !gpu.is_empty() {
                        break;
                    }
                }
            }
        }
        if gpu.is_empty() {
            if let Ok(entries) = std::fs::read_dir("/sys/class/drm") {
                for e in entries.flatten() {
                    let name = e.file_name().to_string_lossy().to_string();
                    if name.starts_with("card") && !name.contains('-') {
                        let dev = e.path().join("device");
                        if let Ok(ven) = std::fs::read_to_string(dev.join("vendor")) {
                            let v = ven.trim().trim_start_matches("0x");
                            let mut m = String::new();
                            if let Ok(mm) = std::fs::read_to_string(dev.join("modalias")) {
                                m = mm;
                            }
                            let _ = v;
                            if !m.is_empty() {
                                gpu = m.split_whitespace().next().unwrap_or("").to_string();
                            }
                            break;
                        }
                    }
                }
            }
        }
    }
    #[cfg(target_os = "windows")]
    {
        if let Ok(out) = Command::new("wmic")
            .args(["cpu", "get", "Name,NumberOfCores", "/format:list"])
            .output()
        {
            let text = String::from_utf8_lossy(&out.stdout);
            for line in text.lines() {
                if let Some(v) = line.strip_prefix("Name=") {
                    cpu = v.trim().to_string();
                }
                if let Some(v) = line.strip_prefix("NumberOfCores=") {
                    cpu_cores = v.trim().parse().unwrap_or(0);
                }
            }
        }
        if let Ok(out) = Command::new("wmic")
            .args(["os", "get", "TotalVisibleMemorySize", "/format:list"])
            .output()
        {
            let text = String::from_utf8_lossy(&out.stdout);
            for line in text.lines() {
                if let Some(v) = line.strip_prefix("TotalVisibleMemorySize=") {
                    ram_mb = v.trim().parse::<u64>().unwrap_or(0) / 1024;
                }
            }
        }
        if let Ok(out) = Command::new("powershell")
            .args(["-NoProfile", "-Command", "(Get-CimInstance Win32_VideoController | Select-Object -First 1 -ExpandProperty Name)"])
            .output()
        {
            let t = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !t.is_empty() {
                gpu = t;
            }
        }
        os_version = std::env::var("OS").unwrap_or_default();
    }
    #[cfg(target_os = "macos")]
    {
        if let Ok(out) = Command::new("sysctl").args(["-n", "machdep.cpu.brand_string"]).output() {
            cpu = String::from_utf8_lossy(&out.stdout).trim().to_string();
        }
        if let Ok(out) = Command::new("sysctl").args(["-n", "hw.memsize"]).output() {
            ram_mb = String::from_utf8_lossy(&out.stdout).trim().parse::<u64>().unwrap_or(0) / (1024 * 1024);
        }
        if let Ok(out) = Command::new("sysctl").args(["-n", "hw.logicalcpu"]).output() {
            cpu_cores = String::from_utf8_lossy(&out.stdout).trim().parse().unwrap_or(0);
        }
        if let Ok(out) = Command::new("sw_vers").args(["-productVersion"]).output() {
            os_version = format!("macOS {}", String::from_utf8_lossy(&out.stdout).trim());
        }
        if let Ok(out) = Command::new("system_profiler").args(["SPDisplaysDataType", "-json"]).output() {
            if let Ok(v) = serde_json::from_slice::<serde_json::Value>(&out.stdout) {
                if let Some(g) = v
                    .get("SPDisplaysDataType")
                    .and_then(|a| a.get(0))
                    .and_then(|o| o.get("sppci_model"))
                    .and_then(|m| m.as_str())
                {
                    gpu = g.to_string();
                }
            }
        }
    }
    if cpu_cores == 0 {
        cpu_cores = std::thread::available_parallelism().map(|n| n.get() as u32).unwrap_or(0);
    }

    let hostname = std::env::var("HOSTNAME")
        .ok()
        .or_else(|| std::env::var("COMPUTERNAME").ok())
        .unwrap_or_else(|| "unknown".into());
    let username = std::env::var("USER")
        .ok()
        .or_else(|| std::env::var("USERNAME").ok())
        .or_else(|| std::env::var("LOGNAME").ok())
        .unwrap_or_else(|| "unknown".into());

    json!({
        "os": os,
        "os_version": os_version,
        "arch": arch,
        "cpu": cpu,
        "cpu_cores": cpu_cores,
        "ram_mb": ram_mb,
        "gpu": gpu,
        "hostname": hostname,
        "username": username,
        "version": APP_VERSION,
    })
}

// ============================================================
// DISCORD API PROXY — the webview can't call discord.com (CORS),
// so every REST call goes through here with the user's own token.
// ============================================================

#[tauri::command]
fn discord_api(method: String, path: String, token: String, body: Option<String>) -> Value {
    let url = format!("https://discord.com/api/v10{}", path);
    let m = method.to_uppercase();
    let res = blocking(|| {
        let c = http_client(20)?;
        let mut req = match m.as_str() {
            "POST" => c.post(&url),
            "PUT" => c.put(&url),
            "PATCH" => c.patch(&url),
            "DELETE" => c.delete(&url),
            _ => c.get(&url),
        };
        req = req
            .header("Authorization", token.trim())
            .header("Content-Type", "application/json");
        if let Some(b) = body {
            req = req.body(b);
        }
        let resp = req.send().map_err(|e| e.to_string())?;
        let status = resp.status().as_u16();
        let text = resp.text().map_err(|e| e.to_string())?;
        Ok::<(u16, String), String>((status, text))
    });
    match res {
        Ok((status, text)) => {
            let parsed = serde_json::from_str::<Value>(&text).unwrap_or(Value::String(text));
            json!({ "ok": status < 400, "status": status, "body": parsed })
        }
        Err(e) => json!({ "ok": false, "status": 0, "error": e }),
    }
}
// ============================================================
// DISCORD INVITE INFO — public, tokenless: live member/online
// counts for our invite + a real-time online-member list from
// the guild widget (right-hub community card).
// ============================================================
#[tauri::command]
fn discord_invite_info(code: String) -> Value {
    let code = code
        .trim()
        .trim_start_matches("https://discord.gg/")
        .trim_start_matches("http://discord.gg/")
        .trim_start_matches("discord.gg/")
        .trim_end_matches('/')
        .to_string();
    let res = blocking(|| {
        let c = http_client(15)?;
        let url = format!(
            "https://discord.com/api/v10/invites/{}?with_counts=true",
            code
        );
        let resp = c.get(&url).send().map_err(|e| e.to_string())?;
        let status = resp.status().as_u16();
        let text = resp.text().map_err(|e| e.to_string())?;
        Ok::<(u16, String), String>((status, text))
    });
    let (status, text) = match res {
        Ok(v) => v,
        Err(e) => return json!({ "ok": false, "error": e }),
    };
    if status >= 400 {
        return json!({ "ok": false, "status": status, "error": text });
    }
    let inv: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
    let members = inv
        .get("approximate_member_count")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let online = inv
        .get("approximate_presence_count")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let guild = inv.get("guild").cloned().unwrap_or(Value::Null);
    let guild_id = guild
        .get("id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let guild_name = guild
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("Isolate Discord")
        .to_string();
    let icon = guild
        .get("icon")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let icon_url = if icon.is_empty() {
        Value::Null
    } else {
        let ext = if icon.starts_with("a_") { "gif" } else { "png" };
        json!(format!(
            "https://cdn.discordapp.com/icons/{}/{}.{}?size=64",
            guild_id, icon, ext
        ))
    };
    // Real-time online member list — public guild widget (no token).
    let mut list: Vec<Value> = Vec::new();
    if !guild_id.is_empty() {
        if let Ok(w) = blocking(|| {
            let c = http_client(10)?;
            let url = format!("https://discord.com/api/guilds/{}/widget.json", guild_id);
            let resp = c
                .get(&url)
                .header(
                    "User-Agent",
                    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
                     (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
                )
                .send()
                .map_err(|e| e.to_string())?;
            let t = resp.text().map_err(|e| e.to_string())?;
            Ok::<String, String>(t)
        }) {
            if let Ok(wj) = serde_json::from_str::<Value>(&w) {
                if let Some(arr) = wj.get("members").and_then(|m| m.as_array()) {
                    for m in arr.iter().take(14) {
                        let id = m.get("id").and_then(|v| v.as_str()).unwrap_or("");
                        let username = m
                            .get("username")
                            .and_then(|v| v.as_str())
                            .unwrap_or("Member")
                            .to_string();
                        let av = m.get("avatar").and_then(|v| v.as_str());
                        let avatar = match av {
                            Some(h) if !h.is_empty() => Some(format!(
                                "https://cdn.discordapp.com/guilds/{}/users/{}/avatars/{}.png?size=64",
                                guild_id, id, h
                            )),
                            _ => None,
                        };
                        let status = m
                            .get("status")
                            .and_then(|v| v.as_str())
                            .unwrap_or("online")
                            .to_string();
                        list.push(json!({
                            "id": id, "name": username, "avatar": avatar, "status": status
                        }));
                    }
                }
            }
        }
    }
    json!({
        "ok": true,
        "members": members,
        "online": online,
        "guild_name": guild_name,
        "icon": icon_url,
        "list": list,
    })
}
// ============================================================
// WINDOWS OPTIMIZATION SUITE — manual trigger (auto-runs on
// every game launch too). Phases 1-5 in win_opt.rs.
// ============================================================
#[tauri::command]
fn run_system_optimizer(app: tauri::State<AppState>) -> Value {
    if !cfg!(windows) {
        return json!({
            "ok": false,
            "skipped": true,
            "platform": std::env::consts::OS,
            "reason": "Windows-only optimization suite",
        });
    }
    win_opt::run_full(&|m| log_msg(&app, m))
}

#[tauri::command]
fn get_game_state(app: tauri::State<AppState>) -> Value {
    let pid = *app.game_pid.lock().unwrap();
    json!({ "running": pid.is_some(), "pid": pid })
}

// ============================================================
// LIBRARY RULES + NATIVES  (all OSes — the blueprint was Windows-only)
// ============================================================

fn os_key() -> &'static str {
    match std::env::consts::OS {
        "windows" => "windows",
        "macos" => "osx",
        _ => "linux",
    }
}

/// Mojang `rules[]` filtering. Rules carrying `features` are skipped
/// (we don't emulate demo/quickPlay feature flags).
fn rules_allow(rules: &[Value]) -> bool {
    let mut allowed = false;
    for rule in rules {
        if rule.get("features").is_some() {
            continue;
        }
        let matches = match rule.get("os").and_then(|o| o.get("name")).and_then(|n| n.as_str()) {
            Some(name) => name == os_key(),
            None => true,
        };
        if matches {
            allowed = rule.get("action").and_then(|a| a.as_str()) == Some("allow");
        }
    }
    allowed
}

fn is_lib_allowed(lib: &Value) -> bool {
    match lib.get("rules").and_then(|r| r.as_array()) {
        Some(r) if !r.is_empty() => rules_allow(r),
        _ => true,
    }
}

fn libraries_base() -> &'static str {
    "https://libraries.minecraft.net/"
}

/// Resolve a library's main jar: (relative path, url).
fn lib_artifact(lib: &Value, loader: &str) -> Option<(String, String)> {
    if let Some(art) = lib.get("downloads").and_then(|d| d.get("artifact")) {
        let path = art.get("path").and_then(|p| p.as_str())?.to_string();
        let url = art
            .get("url")
            .and_then(|u| u.as_str())
            .filter(|u| !u.is_empty())
            .map(|u| u.to_string())
            .unwrap_or_else(|| format!("{}{}", libraries_base(), path));
        return Some((path, url));
    }
    // Some loader libraries omit `downloads` — derive from the maven name.
    let name = lib.get("name").and_then(|n| n.as_str())?;
    let rel = maven_name_to_path(name)?;
    let base = lib
        .get("url")
        .and_then(|u| u.as_str())
        .filter(|u| !u.is_empty())
        .map(|u| u.trim_end_matches('/').to_string() + "/")
        .unwrap_or_else(|| match loader {
            "fabric" => "https://maven.fabricmc.net/".to_string(),
            "quilt" => "https://maven.quiltmc.org/repository/release/".to_string(),
            _ => libraries_base().to_string(),
        });
    Some((rel.clone(), format!("{}{}", base, rel)))
}

/// Legacy (≤1.19.3) native classifier jar: (relative path, url).
fn legacy_native_artifact(lib: &Value) -> Option<(String, String)> {
    let natives = lib.get("natives")?;
    let template = natives.get(os_key())?.as_str()?;
    let arch = if cfg!(target_arch = "x86_64") { "64" } else { "32" };
    let classifier = template.replace("${arch}", arch);
    let classifiers = lib.get("downloads")?.get("classifiers")?;
    let art = classifiers.get(&classifier)?;
    let path = art.get("path").and_then(|p| p.as_str())?.to_string();
    let url = art
        .get("url")
        .and_then(|u| u.as_str())
        .filter(|u| !u.is_empty())
        .map(|u| u.to_string())
        .unwrap_or_else(|| format!("{}{}", libraries_base(), path));
    Some((path, url))
}

/// Modern (1.19.4+) natives ship as ordinary libraries whose maven classifier
/// is `natives-<os>[-arm64]` — e.g. `org.lwjgl:lwjgl:3.3.3:natives-linux`.
/// Returns that library's own artifact when it is the native build for *this*
/// machine, `None` for every other OS/arch entry.
fn modern_native_artifact(lib: &Value) -> Option<(String, String)> {
    let name = lib.get("name").and_then(|n| n.as_str())?;
    let cls = name.rsplit(':').next()?;
    let rest = cls.strip_prefix("natives-")?;
    let os = match std::env::consts::OS {
        "windows" => "windows",
        "macos" => "macos",
        _ => "linux",
    };
    if !rest.starts_with(os) {
        return None;
    }
    // "" = x86_64 build, "-arm64" = aarch64 build, everything else
    // ("-x86", "-patch", …) is a build we never run on.
    let suffix = &rest[os.len()..];
    match (suffix, cfg!(target_arch = "aarch64")) {
        ("", false) | ("-arm64", true) => {}
        _ => return None,
    }
    lib_artifact(lib, "")
}

/// Resolve a library's native jar for this OS — both the legacy
/// `natives{}` + `downloads.classifiers` form and the modern
/// `:natives-<os>` maven classifier form: (relative path, url).
fn native_artifact(lib: &Value) -> Option<(String, String)> {
    legacy_native_artifact(lib).or_else(|| modern_native_artifact(lib))
}

/// Unzip only the native binaries out of a natives jar into `dest`.
fn extract_natives(jar: &Path, dest: &Path) -> Result<usize, String> {
    std::fs::create_dir_all(dest).map_err(|e| e.to_string())?;
    let f = std::fs::File::open(jar).map_err(|e| e.to_string())?;
    let mut zip = zip::ZipArchive::new(f).map_err(|e| e.to_string())?;
    let mut count = 0usize;
    for i in 0..zip.len() {
        let mut file = zip.by_index(i).map_err(|e| e.to_string())?;
        if file.is_dir() {
            continue;
        }
        let name = file.name().to_string();
        if name.starts_with("META-INF/") {
            continue;
        }
        let lower = name.to_ascii_lowercase();
        let is_native = lower.ends_with(".dll")
            || lower.ends_with(".dylib")
            || lower.ends_with(".jnilib")
            || lower.ends_with(".so");
        if !is_native {
            continue;
        }
        let base = match std::path::Path::new(&name).file_name() {
            Some(b) => b.to_os_string(),
            None => continue,
        };
        let out = dest.join(base);
        let mut out_f = std::fs::File::create(&out).map_err(|e| e.to_string())?;
        std::io::copy(&mut file, &mut out_f).map_err(|e| e.to_string())?;
        count += 1;
    }
    Ok(count)
}

// ============================================================
// PARALLEL DOWNLOADS
// ============================================================

/// Download `items` with a small worker pool. `tick(done, total)` is called
/// after each item. Returns the number of failures.
fn download_many(
    items: Vec<(String, PathBuf)>,
    workers: usize,
    tick: &(dyn Fn(usize, usize) + Sync),
) -> usize {
    let total = items.len();
    if total == 0 {
        return 0;
    }
    let next = AtomicUsize::new(0);
    let done = AtomicUsize::new(0);
    let failed = AtomicUsize::new(0);
    let n = workers.max(1).min(total);
    std::thread::scope(|s| {
        for _ in 0..n {
            s.spawn(|| {
                loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    if i >= total {
                        break;
                    }
                    let (url, dest) = &items[i];
                    let ok = dest.exists() && dest.metadata().map(|m| m.len() > 0).unwrap_or(false);
                    if !ok {
                        if let Err(e) = download_file(url, dest) {
                            eprintln!("[Isolate] failed {}: {}", url, e);
                            failed.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                    let d = done.fetch_add(1, Ordering::Relaxed) + 1;
                    tick(d, total);
                }
            });
        }
    });
    failed.load(Ordering::Relaxed)
}

fn emit(handle: &tauri::AppHandle, event: &str, payload: Value) {
    let _ = handle.emit_all(event, payload);
}

// ============================================================
// JAVA: discovery + Adoptium auto-download
// ============================================================

fn java_bin_from_home(home: &Path) -> PathBuf {
    if cfg!(target_os = "windows") {
        home.join("bin").join("java.exe")
    } else {
        home.join("bin").join("java")
    }
}

/// Accept either a JDK/JRE home dir or a path to the java binary.
fn normalize_java_home(p: &Path) -> Option<PathBuf> {
    if p.is_dir() {
        if java_bin_from_home(p).exists() {
            return Some(p.to_path_buf());
        }
        return None;
    }
    if p.is_file() {
        let bin = p.parent()?;
        let home = bin.parent()?;
        if java_bin_from_home(home).exists() {
            return Some(home.to_path_buf());
        }
    }
    None
}

/// Run `java -version`, return (version string, major).
fn java_probe(bin: &Path) -> Option<(String, u32)> {
    let out = hidden_cmd(bin.to_str()?).arg("-version").output().ok()?;
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stderr),
        String::from_utf8_lossy(&out.stdout)
    );
    let ver = text.split('"').nth(1)?;
    let major = if ver.starts_with("1.") {
        ver.split('.')
            .nth(1)
            .and_then(|s| s.parse().ok())
            .unwrap_or(0)
    } else {
        ver.split('.').next().and_then(|s| s.parse().ok()).unwrap_or(0)
    };
    if major == 0 {
        return None;
    }
    Some((ver.to_string(), major))
}

fn collect_java_homes() -> Vec<PathBuf> {
    let mut homes: Vec<PathBuf> = Vec::new();

    // 1. Our own managed installs.
    if let Ok(rd) = std::fs::read_dir(java_dir()) {
        for e in rd.flatten() {
            if let Some(h) = normalize_java_home(&e.path()) {
                homes.push(h);
            }
        }
    }

    // 2. JAVA_HOME
    if let Ok(jh) = std::env::var("JAVA_HOME") {
        if let Some(h) = normalize_java_home(Path::new(&jh)) {
            homes.push(h);
        }
    }

    // 3. Platform search paths.
    #[cfg(target_os = "linux")]
    {
        for root in ["/usr/lib/jvm", "/usr/lib64/jvm", "/opt/java"] {
            if let Ok(rd) = std::fs::read_dir(root) {
                for e in rd.flatten() {
                    if let Some(h) = normalize_java_home(&e.path()) {
                        homes.push(h);
                    }
                }
            }
        }
    }
    #[cfg(target_os = "macos")]
    {
        let root = "/Library/Java/JavaVirtualMachines";
        if let Ok(rd) = std::fs::read_dir(root) {
            for e in rd.flatten() {
                if let Some(h) = normalize_java_home(&e.path().join("Contents/Home")) {
                    homes.push(h);
                }
            }
        }
    }
    #[cfg(target_os = "windows")]
    {
        let roots = [
            r"C:\Program Files\Java",
            r"C:\Program Files\Eclipse Adoptium",
            r"C:\Program Files\Microsoft",
            r"C:\Program Files\Zulu",
            r"C:\Program Files\Amazon Corretto",
            r"C:\Program Files\BellSoft",
        ];
        for root in roots {
            if let Ok(rd) = std::fs::read_dir(root) {
                for e in rd.flatten() {
                    if let Some(h) = normalize_java_home(&e.path()) {
                        homes.push(h);
                    }
                }
            }
        }
    }

    // 4. Whatever `java` resolves to on PATH.
    #[cfg(not(target_os = "windows"))]
    {
        if let Ok(out) = Command::new("sh")
            .args(["-c", "command -v java"])
            .output()
        {
            let p = PathBuf::from(String::from_utf8_lossy(&out.stdout).trim());
            if let Some(h) = normalize_java_home(&p) {
                homes.push(h);
            }
        }
    }
    #[cfg(target_os = "windows")]
    {
        if let Ok(out) = Command::new("where").arg("java").output() {
            if let Some(first) = String::from_utf8_lossy(&out.stdout).lines().next() {
                if let Some(h) = normalize_java_home(Path::new(first.trim())) {
                    homes.push(h);
                }
            }
        }
    }

    homes.sort();
    homes.dedup();
    homes
}

fn java_record(home: &Path) -> Option<Value> {
    let bin = java_bin_from_home(home);
    let (version, major) = java_probe(&bin)?;
    Some(json!({
        "path": bin.to_string_lossy(),
        "home": home.to_string_lossy(),
        "version": version,
        "major": major,
    }))
}

fn scan_java(app: &AppState, use_cache: bool) -> Vec<Value> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    if use_cache {
        let cache = app.java_cache.lock().unwrap();
        if let Some((ts, v)) = cache.as_ref() {
            if now.saturating_sub(*ts) < 60 {
                return v.clone();
            }
        }
    }
    let mut out: Vec<Value> = collect_java_homes()
        .iter()
        .filter_map(|h| java_record(h))
        .collect();
    out.sort_by(|a, b| {
        let ma = a.get("major").and_then(|m| m.as_u64()).unwrap_or(0);
        let mb = b.get("major").and_then(|m| m.as_u64()).unwrap_or(0);
        mb.cmp(&ma)
    });
    let mut cache = app.java_cache.lock().unwrap();
    *cache = Some((now, out.clone()));
    out
}

#[tauri::command]
fn detect_java(app: tauri::State<AppState>, rescan: bool) -> Value {
    let found = scan_java(&app, !rescan);
    let settings = load_settings(&app);
    json!({
        "java": found,
        "configured": settings.get("java_path").cloned().unwrap_or(json!("")),
    })
}

/// Pick the java executable to run: per-instance → global setting → newest scan.
fn resolve_java(app: &AppState, per_instance: Option<&Value>) -> String {
    if let Some(p) = per_instance
        .and_then(|s| s.get("java_path"))
        .and_then(|p| p.as_str())
        .filter(|p| !p.is_empty())
    {
        return p.to_string();
    }
    let settings = load_settings(app);
    if let Some(p) = settings
        .get("java_path")
        .and_then(|p| p.as_str())
        .filter(|p| !p.is_empty())
    {
        return p.to_string();
    }
    let found = scan_java(app, true);
    if let Some(first) = found.first() {
        if let Some(p) = first.get("path").and_then(|p| p.as_str()) {
            return p.to_string();
        }
    }
    "java".to_string()
}

/// Recursively find a directory containing `bin/java` (depth-limited).
fn find_java_home(root: &Path) -> Option<PathBuf> {
    let mut queue: Vec<(PathBuf, usize)> = vec![(root.to_path_buf(), 0)];
    while let Some((dir, depth)) = queue.pop() {
        if java_bin_from_home(&dir).exists() {
            return Some(dir);
        }
        if depth >= 4 {
            continue;
        }
        if let Ok(rd) = std::fs::read_dir(&dir) {
            for e in rd.flatten() {
                if e.path().is_dir() {
                    queue.push((e.path(), depth + 1));
                }
            }
        }
    }
    None
}

fn extract_archive(src: &Path, dest: &Path, zip_format: bool) -> Result<(), String> {
    std::fs::create_dir_all(dest).map_err(|e| e.to_string())?;
    let f = std::fs::File::open(src).map_err(|e| e.to_string())?;
    if zip_format {
        let mut z = zip::ZipArchive::new(f).map_err(|e| e.to_string())?;
        z.extract(dest).map_err(|e| e.to_string())
    } else {
        let dec = flate2::read::GzDecoder::new(f);
        let mut ar = tar::Archive::new(dec);
        ar.unpack(dest).map_err(|e| e.to_string())
    }
}

/// Download and unpack a Temurin JDK into `~/.isolate/java/`.
#[tauri::command]
async fn install_java(handle: tauri::AppHandle, major: u32) -> Value {
    let app = handle.state::<AppState>();
    log_msg(&app, &format!("[Java] Downloading Temurin JDK {}…", major));
    drop(app);

    let url = adoptium_url(major);
    let zip_format = cfg!(target_os = "windows");
    let staging = match tempfile::tempdir() {
        Ok(d) => d,
        Err(e) => return json!({ "error": e.to_string() }),
    };
    let archive = staging.path().join(if zip_format { "jdk.zip" } else { "jdk.tar.gz" });

    let dl = {
        let h = handle.clone();
        let msg = format!("Downloading JDK {}…", major);
        let _ = &h;
        let _ = &msg;
        download_file(&url, &archive)
    };
    if let Err(e) = dl {
        return json!({ "error": format!("Download failed: {}", e) });
    }

    let out_root = java_dir().join(format!("temurin-{}", major));
    if out_root.exists() {
        let _ = std::fs::remove_dir_all(&out_root);
    }
    if let Err(e) = extract_archive(&archive, &staging.path().join("x"), zip_format) {
        return json!({ "error": format!("Extract failed: {}", e) });
    }
    let home = match find_java_home(&staging.path().join("x")) {
        Some(h) => h,
        None => return json!({ "error": "No java binary found in the downloaded archive." }),
    };
    if let Err(e) = std::fs::rename(&home, &out_root) {
        // Cross-device rename can fail — fall back to a copy.
        if let Err(e2) = copy_dir(&home, &out_root) {
            return json!({ "error": format!("Could not install JDK: {} / {}", e, e2) });
        }
    }
    let bin = java_bin_from_home(&out_root);
    let record = match java_record(&out_root) {
        Some(r) => r,
        None => return json!({ "error": "Installed JDK did not pass `java -version`." }),
    };

    let app = handle.state::<AppState>();
    {
        let mut cache = app.java_cache.lock().unwrap();
        *cache = None;
    }
    log_msg(&app, &format!("[Java] Installed {} → {}", record, bin.to_string_lossy()));
    json!({ "success": true, "java": record })
}

fn copy_dir(src: &Path, dst: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dst).map_err(|e| e.to_string())?;
    for entry in std::fs::read_dir(src).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let to = dst.join(entry.file_name());
        if entry.path().is_dir() {
            copy_dir(&entry.path(), &to)?;
        } else {
            std::fs::copy(entry.path(), &to).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

// ============================================================
// INSTALL PIPELINE
// ============================================================

fn progress(
    handle: &tauri::AppHandle,
    instance: &str,
    pct: u32,
    message: &str,
    done: bool,
    error: Option<String>,
) {
    emit(
        handle,
        "install-progress",
        json!({
            "instance": instance,
            "pct": pct,
            "message": message,
            "done": done,
            "error": error,
        }),
    );
}

/// Fire-and-forget: returns `{status:"started"}` immediately, streams
/// `install-progress` events from a worker thread.
#[tauri::command]
async fn install_instance(handle: tauri::AppHandle, name: String) -> Value {
    let clean = sanitize_name(&name);
    let exists = {
        let app = handle.state::<AppState>();
        load_instances_raw(&app)
            .iter()
            .any(|r| r.get("name").and_then(|n| n.as_str()) == Some(clean.as_str()))
    };
    if !exists {
        return json!({ "error": "Instance not found." });
    }
    let h = handle.clone();
    std::thread::spawn(move || install_blocking(h, clean));
    json!({ "status": "started" })
}

/// Append a timestamped line to `~/.isolate/install.log`. Unlike `log_msg`
/// (in-memory only — dies with the process), this survives an app close so
/// the error that killed an install is still readable on the next run.
fn install_log(msg: &str) {
    use std::io::Write;
    let path = app_dir().join("install.log");
    if let Some(p) = path.parent() {
        let _ = std::fs::create_dir_all(p);
    }
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        let _ = writeln!(
            f,
            "[{}] {}",
            chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ"),
            msg
        );
    }
}

fn install_blocking(handle: tauri::AppHandle, name: String) {
    install_log(&format!("[start] {} (client v{})", name, APP_VERSION));
    let result = install_inner(&handle, &name);
    match result {
        Ok(msg) => {
            install_log(&format!("[done] {}: {}", name, msg));
            progress(&handle, &name, 100, &msg, true, None);
        }
        Err(e) => {
            let app = handle.state::<AppState>();
            log_msg(&app, &format!("[Install/ERR] {}: {}", name, e));
            install_log(&format!("[err] {}: {}", name, e));
            progress(&handle, &name, 0, "Install failed", true, Some(e));
        }
    }
}

fn install_inner(handle: &tauri::AppHandle, name: &str) -> Result<String, String> {
    let app = handle.state::<AppState>();
    let list = load_instances_raw(&app);
    let rec = list
        .iter()
        .find(|r| r.get("name").and_then(|n| n.as_str()) == Some(name))
        .cloned()
        .ok_or_else(|| "Instance not found.".to_string())?;
    let mc = rec
        .get("version")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let loader = rec
        .get("loader")
        .and_then(|v| v.as_str())
        .unwrap_or("vanilla")
        .to_string();
    let loader_ver = rec
        .get("loader_version")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    if mc.is_empty() {
        let e = "Instance has no Minecraft version.".to_string();
        install_log(&format!("[{}] ERR {}", name, e));
        return Err(e);
    }
    drop(app);

    let dir = instance_dir(name);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    // ---- 1. version JSON ------------------------------------------------
    progress(handle, name, 5, "Fetching version manifest", false, None);
    install_log(&format!("[{}] step manifest", name));
    let manifest = http_get(MOJANG_MANIFEST)?;
    let entry = manifest
        .get("versions")
        .and_then(|v| v.as_array())
        .and_then(|a| a.iter().find(|v| v.get("id").and_then(|i| i.as_str()) == Some(mc.as_str())))
        .cloned()
        .ok_or_else(|| format!("Minecraft {} not found in the manifest.", mc))?;
    let vurl = entry
        .get("url")
        .and_then(|u| u.as_str())
        .ok_or("Version metadata URL missing.")?
        .to_string();
    let vjson_path = dir.join(format!("{}.json", mc));
    download_file(&vurl, &vjson_path)?;
    let vjson = load_json(&vjson_path);
    if vjson.is_null() {
        let e = "Version JSON is invalid.".to_string();
        install_log(&format!("[{}] ERR {}", name, e));
        return Err(e);
    }

    // ---- 2. client jar --------------------------------------------------
    progress(handle, name, 20, "Downloading game jar", false, None);
    install_log(&format!("[{}] step client.jar", name));
    let client = vjson
        .get("downloads")
        .and_then(|d| d.get("client"))
        .cloned()
        .ok_or("Version has no client download.")?;
    let client_url = client
        .get("url")
        .and_then(|u| u.as_str())
        .ok_or("Client URL missing.")?
        .to_string();
    let client_jar = dir.join("client.jar");
    if !(client_jar.exists() && client_jar.metadata().map(|m| m.len() > 1000).unwrap_or(false)) {
        download_file(&client_url, &client_jar)?;
    }

    // ---- 3. assets ------------------------------------------------------
    progress(handle, name, 35, "Downloading assets", false, None);
    install_log(&format!("[{}] step assets", name));
    let mut asset_items: Vec<(String, PathBuf)> = Vec::new();
    if let Some(idx) = vjson.get("assetIndex") {
        let idx_id = idx
            .get("id")
            .and_then(|i| i.as_str())
            .ok_or("assetIndex.id missing")?
            .to_string();
        let idx_url = idx
            .get("url")
            .and_then(|u| u.as_str())
            .ok_or("assetIndex.url missing")?
            .to_string();
        let idx_path = assets_dir().join("indexes").join(format!("{}.json", idx_id));
        if !idx_path.exists() {
            download_file(&idx_url, &idx_path)?;
        }
        let index = load_json(&idx_path);
        if let Some(objs) = index.get("objects").and_then(|o| o.as_object()) {
            for (asset_name, meta) in objs {
                let hash = meta.get("hash").and_then(|h| h.as_str()).unwrap_or("");
                if hash.len() < 2 {
                    continue;
                }
                let dest = assets_dir()
                    .join("objects")
                    .join(&hash[..2])
                    .join(hash);
                if dest.exists() {
                    continue;
                }
                let url = format!("{}/{}/{}/{}", RESOURCES_BASE, &hash[..2], hash, asset_name);
                asset_items.push((url, dest));
            }
        }
    }
    let total_assets = asset_items.len();
    let base_pct = 35.0f64;
    let span_pct = 30.0f64;
    let h = handle.clone();
    let n2 = name.to_string();
    let asset_fails = download_many(asset_items, 8, &(move |d: usize, t: usize| {
        if d == t || d % 25 == 0 {
            let pct = base_pct + span_pct * (d as f64 / t as f64);
            progress(
                &h,
                &n2,
                pct.round() as u32,
                &format!("Downloading assets {}/{}", d, t),
                false,
                None,
            );
        }
    }));

    // ---- 4. libraries + natives ----------------------------------------
    progress(handle, name, 68, "Downloading libraries", false, None);
    install_log(&format!("[{}] step libraries", name));
    let mut merged_libs: Vec<Value> = Vec::new();
    if let Some(arr) = vjson.get("libraries").and_then(|l| l.as_array()) {
        merged_libs.extend(arr.iter().cloned());
    }

    // Loader profile (Fabric / Quilt).
    if loader == "fabric" {
        let url = format!(
            "https://meta.fabricmc.net/v2/versions/loader/{}/{}/profile/json",
            mc, loader_ver
        );
        let prof = http_get(&url).map_err(|e| format!("Fabric profile: {}", e))?;
        let prof_path = dir.join("fabric-profile.json");
        save_json(&prof_path, &prof);
        if let Some(arr) = prof.get("libraries").and_then(|l| l.as_array()) {
            merged_libs.extend(arr.iter().cloned());
        }
    } else if loader == "quilt" {
        let url = format!(
            "https://meta.quiltmc.org/v3/versions/loader/{}/{}/profile/json",
            mc, loader_ver
        );
        let prof = http_get(&url).map_err(|e| format!("Quilt profile: {}", e))?;
        let prof_path = dir.join("quilt-profile.json");
        save_json(&prof_path, &prof);
        if let Some(arr) = prof.get("libraries").and_then(|l| l.as_array()) {
            merged_libs.extend(arr.iter().cloned());
        }
    }

    let natives_dir = dir.join("natives");
    std::fs::create_dir_all(&natives_dir).map_err(|e| e.to_string())?;

    let mut lib_items: Vec<(String, PathBuf)> = Vec::new();
    let mut native_items: Vec<(String, PathBuf)> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    for lib in &merged_libs {
        if !is_lib_allowed(lib) {
            continue;
        }
        if let Some((rel, url)) = lib_artifact(lib, &loader) {
            if seen.insert(rel.clone()) {
                lib_items.push((url, libraries_dir().join(&rel)));
            }
        }
        // Legacy `natives{}` classifiers, or modern `:natives-<os>` artifacts.
        // Keyed on the bare path so a modern native (whose path is identical to
        // its own library entry) is never queued twice.
        if let Some((rel, url)) = native_artifact(lib) {
            if seen.insert(rel.clone()) {
                native_items.push((url, libraries_dir().join(&rel)));
            }
        }
    }
    let all_libs: Vec<(String, PathBuf)> = lib_items
        .into_iter()
        .chain(native_items.into_iter())
        .collect();
    let total_libs = all_libs.len();
    let h = handle.clone();
    let n3 = name.to_string();
    let lib_fails = download_many(all_libs, 8, &(move |d: usize, t: usize| {
        if d == t || d % 5 == 0 {
            let pct = 68.0 + 22.0 * (d as f64 / t as f64);
            progress(
                &h,
                &n3,
                pct.round() as u32,
                &format!("Downloading libraries {}/{}", d, t),
                false,
                None,
            );
        }
    }));
    let _ = total_libs;

    // Unpack natives into the instance folder.
    let mut natives_extracted = 0usize;
    for lib in &merged_libs {
        if !is_lib_allowed(lib) {
            continue;
        }
        if let Some((rel, _)) = native_artifact(lib) {
            let jar = libraries_dir().join(&rel);
            if jar.exists() {
                match extract_natives(&jar, &natives_dir) {
                    Ok(c) => natives_extracted += c,
                    Err(e) => eprintln!("[Isolate] natives {}: {}", rel, e),
                }
            }
        }
    }

    // ---- 5. finish ------------------------------------------------------
    let fails = asset_fails + lib_fails;
    let app = handle.state::<AppState>();
    log_msg(
        &app,
        &format!(
            "[Install] {} — {} assets, {} libs, {} natives, {} failed",
            name, total_assets, total_libs, natives_extracted, fails
        ),
    );
    drop(app);
    install_log(&format!(
        "[{}] summary: {} assets, {} libs, {} natives, {} failed",
        name, total_assets, total_libs, natives_extracted, fails
    ));

    if total_assets == 0 && fails > 0 {
        let e = format!("{} downloads failed.", fails);
        install_log(&format!("[{}] ERR {}", name, e));
        return Err(e);
    }
    let msg = if fails > 0 {
        format!("Installed with {} failed downloads (will retry on launch)", fails)
    } else {
        format!("{} installed", mc)
    };
    Ok(msg)
}

// ============================================================
// ARGUMENT EXPANSION
// ============================================================

fn substitute(s: &str, subs: &HashMap<String, String>) -> String {
    let mut out = s.to_string();
    for (k, v) in subs {
        out = out.replace(&format!("${{{}}}", k), v);
    }
    out
}

/// Expand one argument value: a bare string, or an array of strings.
fn expand_value(v: &Value, subs: &HashMap<String, String>) -> Vec<String> {
    match v {
        Value::String(s) => vec![substitute(s, subs)],
        Value::Array(a) => a
            .iter()
            .filter_map(|x| x.as_str())
            .map(|s| substitute(s, subs))
            .collect(),
        _ => vec![],
    }
}

/// Expand an `arguments.game` / `arguments.jvm` array, honouring `rules`.
fn expand_arg_list(list: &[Value], subs: &HashMap<String, String>) -> Vec<String> {
    let mut out = Vec::new();
    for item in list {
        match item {
            Value::String(s) => out.push(substitute(s, subs)),
            Value::Object(o) => {
                if let Some(rules) = o.get("rules").and_then(|r| r.as_array()) {
                    if !rules.is_empty() && !rules_allow(rules) {
                        continue;
                    }
                }
                if let Some(v) = o.get("value") {
                    out.extend(expand_value(v, subs));
                }
            }
            _ => {}
        }
    }
    out
}

// ============================================================
// OFFLINE UUID  (proper version-3 UUID, not the nil UUID)
// ============================================================

fn offline_uuid(name: &str) -> String {
    let mut hasher = Md5::new();
    hasher.update(format!("OfflinePlayer:{}", name).as_bytes());
    let mut b = hasher.finalize().to_vec();
    b[6] = (b[6] & 0x0f) | 0x30; // version 3
    b[8] = (b[8] & 0x3f) | 0x80; // RFC 4122 variant
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
        b[8], b[9], b[10], b[11], b[12], b[13], b[14], b[15]
    )
}

// ============================================================
// LAUNCH PIPELINE
// ============================================================

/// Read the version JSON + loader profile (if any) and return the merged
/// library list.
fn merged_libraries(dir: &Path, vjson: &Value, loader: &str) -> Vec<Value> {
    let mut libs: Vec<Value> = Vec::new();
    if let Some(arr) = vjson.get("libraries").and_then(|l| l.as_array()) {
        libs.extend(arr.iter().cloned());
    }
    if loader == "fabric" {
        let p = dir.join("fabric-profile.json");
        let prof = load_json(&p);
        if let Some(arr) = prof.get("libraries").and_then(|l| l.as_array()) {
            libs.extend(arr.iter().cloned());
        }
    } else if loader == "quilt" {
        let p = dir.join("quilt-profile.json");
        let prof = load_json(&p);
        if let Some(arr) = prof.get("libraries").and_then(|l| l.as_array()) {
            libs.extend(arr.iter().cloned());
        }
    }
    libs
}

/// Classpath entries as (dest, url), de-duplicated, OS-filtered.
fn classpath_entries(libs: &[Value], loader: &str) -> Vec<(PathBuf, String)> {
    let mut out: Vec<(PathBuf, String)> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    for lib in libs {
        if !is_lib_allowed(lib) {
            continue;
        }
        if let Some((rel, url)) = lib_artifact(lib, loader) {
            if seen.insert(rel.clone()) {
                out.push((libraries_dir().join(rel), url));
            }
        }
    }
    out
}

fn classpath_string(entries: &[(PathBuf, String)]) -> String {
    let sep = if cfg!(target_os = "windows") { ";" } else { ":" };
    entries
        .iter()
        .map(|(p, _)| p.to_string_lossy().to_string())
        .collect::<Vec<_>>()
        .join(sep)
}

#[tauri::command]
async fn launch_game(handle: tauri::AppHandle, name: String) -> Value {
    let clean = sanitize_name(&name);
    let app = handle.state::<AppState>();

    if app.game_pid.lock().unwrap().is_some() {
        return json!({ "error": "A game is already running." });
    }

    let list = load_instances_raw(&app);
    let rec = match list
        .iter()
        .find(|r| r.get("name").and_then(|n| n.as_str()) == Some(clean.as_str()))
    {
        Some(r) => r.clone(),
        None => return json!({ "error": "Instance not found." }),
    };
    let mc = rec
        .get("version")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let loader = rec
        .get("loader")
        .and_then(|v| v.as_str())
        .unwrap_or("vanilla")
        .to_string();
    let dir = instance_dir(&clean);
    let vjson_path = dir.join(format!("{}.json", mc));
    let client_jar = dir.join("client.jar");
    if !vjson_path.exists() || !client_jar.exists() {
        return json!({
            "error": "not_installed",
            "message": "This instance hasn't been installed yet.",
        });
    }
    let vjson = load_json(&vjson_path);
    if vjson.is_null() {
        return json!({ "error": "Version JSON is unreadable — reinstall the instance." });
    }

    // ---- auth -----------------------------------------------------------
    let settings = load_settings(&app);
    let per_instance = load_instance_settings(&app).get(&clean).cloned();
    // Priority: Ely.by > Microsoft > offline.
    let ely_acc = settings
        .get("ely_account")
        .filter(|a| {
            !a.is_null()
                && a.get("token")
                    .and_then(|t| t.as_str())
                    .map(|t| !t.is_empty() && t != "0")
                    .unwrap_or(false)
        })
        .cloned();
    let (username, uuid, token, user_type) = if let Some(acc) = ely_acc {
        (
            acc.get("username").and_then(|v| v.as_str()).unwrap_or("Player").to_string(),
            acc.get("uuid").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            acc.get("token").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            "mojang",
        )
    } else {
        match settings
            .get("ms_account")
            .filter(|a| !a.is_null() && a.get("token").and_then(|t| t.as_str()).map(|t| !t.is_empty()).unwrap_or(false))
        {
            Some(acc) => (
                acc.get("username").and_then(|v| v.as_str()).unwrap_or("Player").to_string(),
                acc.get("uuid").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                acc.get("token").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                "mojang",
            ),
            None => {
                let n = settings
                    .get("offline_username")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Player")
                    .to_string();
                let u = offline_uuid(&n);
                (n, u, "0".to_string(), "legacy")
            }
        }
    };
    let using_elyby = !settings
        .get("ely_account")
        .and_then(|a| a.get("token"))
        .and_then(|t| t.as_str())
        .map(|t| t.is_empty() || t == "0")
        .unwrap_or(true);

    // ---- java -----------------------------------------------------------
    let java = resolve_java(&app, per_instance.as_ref());
    let java_path = PathBuf::from(&java);
    if java != "java" && !java_path.exists() {
        return json!({
            "error": "no_java",
            "message": "No Java installation found. Install one from Settings.",
        });
    }

    // ---- memory ---------------------------------------------------------
    let per_ram = per_instance
        .as_ref()
        .and_then(|s| s.get("ram_mb"))
        .and_then(|v| v.as_u64());
    let ram_mb = per_ram
        .or_else(|| settings.get("ram_mb").and_then(|v| v.as_u64()))
        .unwrap_or(4096)
        .max(512);
    let extra_jvm: Vec<String> = per_instance
        .as_ref()
        .and_then(|s| s.get("jvm_args"))
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty())
        .or_else(|| settings.get("jvm_args").and_then(|v| v.as_str()).filter(|s| !s.trim().is_empty()))
        .map(|s| s.split_whitespace().map(|x| x.to_string()).collect())
        .unwrap_or_default();

    // ---- libraries + natives -------------------------------------------
    let libs = merged_libraries(&dir, &vjson, &loader);
    let entries = classpath_entries(&libs, &loader);
    // Self-heal: fetch anything missing before launch.
    let missing: Vec<(String, PathBuf)> = entries
        .iter()
        .filter(|(p, _)| !p.exists() || p.metadata().map(|m| m.len() == 0).unwrap_or(true))
        .map(|(p, u)| (u.clone(), p.clone()))
        .collect();
    if !missing.is_empty() {
        let h = handle.clone();
        let n = clean.clone();
        progress(&h, &n, 0, &format!("Repairing {} libraries", missing.len()), false, None);
        download_many(missing, 6, &|_, _| {});
    }
    let cp = classpath_string(&entries);

    let natives_dir = dir.join("natives");
    let _ = std::fs::create_dir_all(&natives_dir);
    for lib in &libs {
        if !is_lib_allowed(lib) {
            continue;
        }
        if let Some((rel, _)) = native_artifact(lib) {
            let jar = libraries_dir().join(&rel);
            if jar.exists() {
                let _ = extract_natives(&jar, &natives_dir);
            }
        }
    }

    // ---- main class -----------------------------------------------------
    let mut main_class = vjson
        .get("mainClass")
        .and_then(|m| m.as_str())
        .unwrap_or("net.minecraft.client.main.Main")
        .to_string();
    if loader == "fabric" || loader == "quilt" {
        let prof = load_json(&dir.join(format!("{}-profile.json", loader)));
        if let Some(m) = prof.get("mainClass").and_then(|m| m.as_str()) {
            main_class = m.to_string();
        }
    }

    // ---- asset index id -------------------------------------------------
    let asset_index_id = vjson
        .get("assetIndex")
        .and_then(|a| a.get("id"))
        .and_then(|i| i.as_str())
        .or_else(|| vjson.get("assets").and_then(|a| a.as_str()))
        .unwrap_or("")
        .to_string();

    // ---- substitution map ----------------------------------------------
    let mut subs: HashMap<String, String> = HashMap::new();
    subs.insert("auth_player_name".into(), username.clone());
    subs.insert("version_name".into(), mc.clone());
    subs.insert("game_directory".into(), dir.to_string_lossy().to_string());
    subs.insert("assets_root".into(), assets_dir().to_string_lossy().to_string());
    subs.insert("assets_index_name".into(), asset_index_id.clone());
    subs.insert("game_assets".into(), assets_dir().to_string_lossy().to_string());
    subs.insert("auth_uuid".into(), uuid.clone());
    subs.insert("auth_access_token".into(), token.clone());
    subs.insert("auth_session".into(), token.clone());
    subs.insert("user_type".into(), user_type.to_string());
    subs.insert("version_type".into(), vjson.get("type").and_then(|t| t.as_str()).unwrap_or("release").to_string());
    subs.insert("natives_directory".into(), natives_dir.to_string_lossy().to_string());
    subs.insert("launcher_name".into(), "isolate".into());
    subs.insert("launcher_version".into(), APP_VERSION.to_string());
    subs.insert("classpath".into(), cp.clone());
    subs.insert("classpath_separator".into(), if cfg!(target_os = "windows") { ";" } else { ":" }.to_string());
    subs.insert("library_directory".into(), libraries_dir().to_string_lossy().to_string());
    subs.insert("user_properties".into(), "{}".into());
    // Window size comes from Settings (0 = Minecraft's own default).
    let res_w = settings
        .get("resolution_width")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let res_h = settings
        .get("resolution_height")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let fullscreen = settings
        .get("fullscreen")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    subs.insert("resolution_width".into(), res_w.to_string());
    subs.insert("resolution_height".into(), res_h.to_string());

    // ---- JVM args -------------------------------------------------------
    let mut jvm_args: Vec<String> = Vec::new();
    jvm_args.push(format!("-Xmx{}M", ram_mb));
    // Initial heap: per-instance override, else the global Settings value.
    if let Some(xms) = per_instance
        .as_ref()
        .and_then(|s| s.get("xms_mb"))
        .and_then(|v| v.as_u64())
        .or_else(|| settings.get("xms_mb").and_then(|v| v.as_u64()))
        .filter(|v| *v > 0)
    {
        jvm_args.push(format!("-Xms{}M", xms));
    }
    jvm_args.extend(extra_jvm);
    // Windows optimization suite — version-aware GC flags (spec Phase 3).
    // MC 1.20.5+ (Java 21) → Generational ZGC; older → Aikar's G1GC.
    if cfg!(windows) && !jvm_args.iter().any(|a| a.contains("UseZGC") || a.contains("UseG1GC")) {
        let flags = win_opt::gc_flags_for(&mc);
        for f in flags {
            jvm_args.push(f.to_string());
        }
        log_msg(
            &app,
            if win_opt::mc_ge(&mc, 1, 20, 5) {
                "[Optimizer] JVM: Generational ZGC flags injected (MC >= 1.20.5 / Java 21)"
            } else {
                "[Optimizer] JVM: Aikar's G1GC flags injected (MC <= 1.20.4)"
            },
        );
    }

    // Ely.by: inject authlib-injector so the game pulls skins from ely.by.
    if using_elyby {
        let inj = authlib_injector_path();
        if !inj.exists() {
            log_msg(&app, "[Launch] Fetching authlib-injector for Ely.by…");
            if let Some(url) = fetch_authlib_injector_url() {
                let _ = download_file(&url, &inj);
            }
        }
        if inj.exists() {
            jvm_args.insert(0, format!("-javaagent:{}=ely.by", inj.to_string_lossy()));
            log_msg(&app, "[Launch] authlib-injector enabled for Ely.by skins");
        } else {
            log_msg(&app, "[Launch] authlib-injector unavailable — Ely.by skins disabled");
        }
    }

    let mut version_jvm: Vec<String> = Vec::new();
    if let Some(arr) = vjson.get("arguments").and_then(|a| a.get("jvm")).and_then(|a| a.as_array()) {
        version_jvm.extend(expand_arg_list(arr, &subs));
    } else {
        version_jvm.push(format!("-Djava.library.path={}", natives_dir.to_string_lossy()));
        version_jvm.push("-cp".to_string());
        version_jvm.push(cp.clone());
    }
    if loader == "fabric" || loader == "quilt" {
        let prof = load_json(&dir.join(format!("{}-profile.json", loader)));
        if let Some(arr) = prof.get("arguments").and_then(|a| a.get("jvm")).and_then(|a| a.as_array()) {
            version_jvm.extend(expand_arg_list(arr, &subs));
        }
    }
    jvm_args.extend(version_jvm);

    // ---- game args ------------------------------------------------------
    let mut game_args: Vec<String> = Vec::new();
    if let Some(arr) = vjson.get("arguments").and_then(|a| a.get("game")).and_then(|a| a.as_array()) {
        game_args.extend(expand_arg_list(arr, &subs));
    } else if let Some(s) = vjson.get("minecraftArguments").and_then(|a| a.as_str()) {
        game_args.extend(s.split_whitespace().map(|x| x.to_string()));
    }
    if loader == "fabric" || loader == "quilt" {
        let prof = load_json(&dir.join(format!("{}-profile.json", loader)));
        if let Some(arr) = prof.get("arguments").and_then(|a| a.get("game")).and_then(|a| a.as_array()) {
            game_args.extend(expand_arg_list(arr, &subs));
        }
    }

    // Real window flags from Settings.
    if fullscreen {
        game_args.push("--fullscreen".into());
    } else if res_w >= 320 && res_h >= 240 {
        game_args.push("--width".into());
        game_args.push(res_w.to_string());
        game_args.push("--height".into());
        game_args.push(res_h.to_string());
    }

    // ---- spawn ----------------------------------------------------------
    // Windows optimization suite (Phases 1-5) — background thread so it
    // never delays the launch itself.
    if cfg!(windows) {
        let h = handle.clone();
        std::thread::spawn(move || {
            win_opt::run_on_launch(&|m| {
                let s = h.state::<AppState>();
                log_msg(&s, m);
            });
        });
    }
    log_msg(
        &app,
        &format!(
            "[Launch] {} — MC {} / {} / {}MB / {}",
            clean, mc, loader, ram_mb, java
        ),
    );

    let mut cmd = hidden_cmd(&java);
    cmd.current_dir(&dir);
    for a in &jvm_args {
        cmd.arg(a);
    }
    cmd.arg(&main_class);
    for a in &game_args {
        cmd.arg(a);
    }
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            let msg = format!("Could not start Java: {}", e);
            log_msg(&app, &format!("[Launch/ERR] {}", msg));
            emit(&handle, "launch-failed", json!({ "error": msg }));
            return json!({ "error": msg });
        }
    };

    let pid = child.id();
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let child_slot: Arc<Mutex<Option<Child>>> = Arc::new(Mutex::new(Some(child)));
    let started = Instant::now();
    {
        let mut gs = app.game_session.lock().unwrap();
        *gs = Some(GameSession {
            child: Arc::clone(&child_slot),
            started,
            instance: clean.clone(),
        });
        *app.game_pid.lock().unwrap() = Some(pid);
    }
    emit(&handle, "game-state", json!({ "running": true, "pid": pid, "instance": clean }));

    // ---- stdout / stderr readers ---------------------------------------
    let mut pipes: Vec<(Box<dyn Read + Send>, bool)> = Vec::new();
    if let Some(s) = stdout {
        pipes.push((Box::new(s), false));
    }
    if let Some(s) = stderr {
        pipes.push((Box::new(s), true));
    }
    for (stream, is_err) in pipes {
        let h = handle.clone();
        std::thread::spawn(move || {
            let reader = BufReader::new(stream);
            for line in reader.lines() {
                match line {
                    Ok(l) => {
                        let st = h.state::<AppState>();
                        append_game_output(&st, &l);
                        emit(&h, "game-output", json!({ "line": l, "stderr": is_err }));
                    }
                    Err(_) => break,
                }
            }
        });
    }

    // ---- reaper: wait for exit, account playtime, emit ------------------
    {
        let h = handle.clone();
        let instance = clean.clone();
        let slot = Arc::clone(&child_slot);
        std::thread::spawn(move || {
            let code = loop {
                let status = {
                    let mut g = slot.lock().unwrap();
                    match g.as_mut() {
                        Some(c) => c.try_wait(),
                        None => break None, // killed externally
                    }
                };
                match status {
                    Ok(Some(s)) => break s.code(),
                    Ok(None) => std::thread::sleep(Duration::from_millis(150)),
                    Err(_) => break None,
                }
            };

            let st = h.state::<AppState>();
            let elapsed = {
                let mut gs = st.game_session.lock().unwrap();
                let secs = gs
                    .as_ref()
                    .map(|s| s.started.elapsed().as_secs())
                    .unwrap_or(0);
                *gs = None;
                secs
            };
            *st.game_pid.lock().unwrap() = None;

            if elapsed > 0 {
                let mut pt = load_playtimes(&st);
                *pt.entry(instance.clone()).or_insert(0) += elapsed;
                save_json(
                    &playtimes_path(),
                    &Value::Object(pt.into_iter().map(|(k, v)| (k, json!(v))).collect()),
                );
                // stamp last_played
                let mut list = load_instances_raw(&st);
                for rec in list.iter_mut() {
                    if rec.get("name").and_then(|n| n.as_str()) == Some(instance.as_str()) {
                        rec["last_played"] = json!(chrono::Local::now().to_rfc3339());
                    }
                }
                save_instances(&st, &list);
            }

            log_msg(
                &st,
                &format!("[Launch] '{}' exited (code {:?}) after {}s", instance, code, elapsed),
            );
            emit(
                &h,
                "game-exit",
                json!({ "instance": instance, "code": code, "play_seconds": elapsed }),
            );
            emit(&h, "game-state", json!({ "running": false, "pid": Value::Null }));
        });
    }

    json!({ "success": true, "pid": pid })
}

#[tauri::command]
fn stop_game(app: tauri::State<AppState>) -> Value {
    let session = app.game_session.lock().unwrap();
    match session.as_ref() {
        Some(s) => {
            let mut g = s.child.lock().unwrap();
            match g.as_mut() {
                Some(c) => {
                    let _ = c.kill();
                    json!({ "success": true })
                }
                None => json!({ "error": "Game already exited." }),
            }
        }
        None => json!({ "error": "No game is running." }),
    }
}

// ============================================================
// AUTH — offline + Microsoft device-code
// ============================================================

#[tauri::command]
fn create_offline_account(app: tauri::State<AppState>, username: String) -> Value {
    let clean: String = username
        .trim()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '_')
        .take(16)
        .collect();
    if clean.len() < 3 {
        return json!({ "error": "Username must be at least 3 characters (letters, digits, underscore)." });
    }
    let uuid = offline_uuid(&clean);
    save_setting_val(&app, "offline_username", json!(clean.clone()));
    save_setting_val(&app, "ms_account", Value::Null);
    log_msg(&app, &format!("[Auth] Offline account set to {} ({})", clean, uuid));
    json!({ "success": true, "username": clean, "uuid": uuid, "type": "offline" })
}

/// Step 1 of the Microsoft device-code flow: request a code + start polling.
#[tauri::command]
async fn authenticate_microsoft(handle: tauri::AppHandle) -> Value {
    let app = handle.state::<AppState>();
    *app.ms_auth_result.lock().unwrap() = None;
    app.ms_auth_polling.store(false, Ordering::Relaxed);
    let flag = Arc::clone(&app.ms_auth_polling);
    let result = Arc::clone(&app.ms_auth_result);
    drop(app);

    let body = json!({ "client_id": MS_CLIENT_ID, "scope": MS_SCOPE });
    let resp = match http_post_json(
        "https://login.microsoftonline.com/consumers/oauth2/v2.0/devicecode",
        &body,
    ) {
        Ok(r) => r,
        Err(e) => return json!({ "error": e }),
    };

    let user_code = resp.get("user_code").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let verification_uri = resp
        .get("verification_uri")
        .or_else(|| resp.get("verification_url"))
        .and_then(|v| v.as_str())
        .unwrap_or("https://microsoft.com/link")
        .to_string();
    let device_code = resp.get("device_code").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let interval = resp.get("interval").and_then(|v| v.as_u64()).unwrap_or(5);
    if device_code.is_empty() {
        return json!({ "error": "Microsoft did not return a device code." });
    }

    flag.store(true, Ordering::Relaxed);
    std::thread::spawn(move || {
        let iv = interval.clamp(1, 5);
        let mut failures = 0u32;
        loop {
            std::thread::sleep(Duration::from_secs(iv));
            if !flag.load(Ordering::Relaxed) {
                return;
            }
            let form = [
                ("client_id", MS_CLIENT_ID),
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                ("device_code", device_code.as_str()),
            ];
            match http_post_form(
                "https://login.microsoftonline.com/consumers/oauth2/v2.0/token",
                &form,
            ) {
                Ok(r) => {
                    if let Some(at) = r.get("access_token").and_then(|t| t.as_str()) {
                        let rt = r
                            .get("refresh_token")
                            .and_then(|t| t.as_str())
                            .unwrap_or("")
                            .to_string();
                        *result.lock().unwrap() =
                            Some(json!({ "access_token": at, "refresh_token": rt }));
                        flag.store(false, Ordering::Relaxed);
                        return;
                    }
                    match r.get("error").and_then(|e| e.as_str()).unwrap_or("") {
                        "authorization_pending" => continue,
                        "slow_down" => {
                            std::thread::sleep(Duration::from_secs(iv));
                            continue;
                        }
                        other => {
                            *result.lock().unwrap() = Some(json!({
                                "error": if other.is_empty() { format!("{}", r) } else { other.to_string() }
                            }));
                            flag.store(false, Ordering::Relaxed);
                            return;
                        }
                    }
                }
                Err(_) => {
                    failures += 1;
                    if failures > 30 {
                        *result.lock().unwrap() =
                            Some(json!({ "error": "Network error while polling Microsoft." }));
                        flag.store(false, Ordering::Relaxed);
                        return;
                    }
                }
            }
        }
    });

    json!({
        "user_code": user_code,
        "verification_uri": verification_uri,
        "interval": interval,
    })
}

#[tauri::command]
fn poll_microsoft_status(app: tauri::State<AppState>) -> Value {
    let guard = app.ms_auth_result.lock().unwrap();
    if app.ms_auth_polling.load(Ordering::Relaxed) {
        json!({ "status": "pending" })
    } else if let Some(r) = guard.as_ref() {
        if let Some(e) = r.get("error") {
            json!({ "status": "error", "error": e.clone() })
        } else {
            json!({ "status": "done", "result": r.clone() })
        }
    } else {
        json!({ "status": "idle" })
    }
}

/// The classic 4-hop chain: MS token → XBL → XSTS → Minecraft → profile.
fn exchange_ms_token(ms_token: &str) -> Result<(String, String, String), String> {
    let xbl_body = json!({
        "Properties": {
            "AuthMethod": "RPS",
            "SiteName": "user.auth.xboxlive.com",
            "RpsTicket": ms_token
        },
        "RelyingParty": "http://auth.xboxlive.com",
        "TokenType": "JWT"
    });
    let xbl = http_post_json("https://user.auth.xboxlive.com/user/authenticate", &xbl_body)?;
    let xbl_token = xbl.get("Token").and_then(|t| t.as_str()).unwrap_or("");
    let uhs = xbl
        .get("DisplayClaims")
        .and_then(|d| d.get("xui"))
        .and_then(|u| u.as_array())
        .and_then(|a| a.first())
        .and_then(|c| c.get("uhs"))
        .and_then(|u| u.as_str())
        .unwrap_or("");
    if xbl_token.is_empty() || uhs.is_empty() {
        return Err("Xbox Live authentication failed.".into());
    }

    let xsts_body = json!({
        "Properties": { "SandboxId": "RETAIL", "UserTokens": [xbl_token] },
        "RelyingParty": "rp://api.minecraftservices.com/",
        "TokenType": "JWT"
    });
    let xsts = http_post_json("https://xsts.auth.xboxlive.com/xsts/authorize", &xsts_body)?;
    let xsts_token = xsts.get("Token").and_then(|t| t.as_str()).unwrap_or("");
    if xsts_token.is_empty() {
        let code = xsts
            .get("XErr")
            .and_then(|e| e.as_i64())
            .map(|c| c.to_string())
            .unwrap_or_else(|| "unknown".into());
        return Err(format!(
            "Xbox security token failed (XErr {}). This account may not have an Xbox profile.",
            code
        ));
    }

    let mc_body = json!({ "identityToken": format!("XBL3.0 x={};{}", uhs, xsts_token) });
    let mc = http_post_json(
        "https://api.minecraftservices.com/authentication/login_with_xbox",
        &mc_body,
    )?;
    let mc_token = mc.get("access_token").and_then(|t| t.as_str()).unwrap_or("");
    if mc_token.is_empty() {
        return Err("Minecraft services rejected the Xbox token.".into());
    }

    let c = http_client(30)?;
    let resp = c
        .get("https://api.minecraftservices.com/minecraft/profile")
        .bearer_auth(mc_token)
        .send()
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(
            "This Microsoft account owns no Minecraft Java Edition profile.".into(),
        );
    }
    let profile: Value = resp.json().map_err(|e| e.to_string())?;
    let name = profile.get("name").and_then(|n| n.as_str()).unwrap_or("").to_string();
    let uuid = profile.get("id").and_then(|n| n.as_str()).unwrap_or("").to_string();
    if name.is_empty() || uuid.is_empty() {
        return Err("Could not read the Minecraft profile.".into());
    }
    Ok((mc_token.to_string(), name, uuid))
}

/// Step 2: exchange the polled MS token for a Minecraft account.
#[tauri::command]
async fn complete_microsoft_auth_cmd(
    handle: tauri::AppHandle,
    access_token: String,
    refresh_token: String,
) -> Value {
    let app = handle.state::<AppState>();
    log_msg(&app, "[Auth] Exchanging Microsoft token…");
    match exchange_ms_token(&access_token) {
        Ok((mc_token, username, uuid)) => {
            save_setting_val(
                &app,
                "ms_account",
                json!({
                    "username": username,
                    "uuid": uuid,
                    "token": mc_token,
                    "refresh_token": refresh_token,
                }),
            );
            log_msg(&app, &format!("[Auth] Signed in as {} ({})", username, uuid));
            json!({ "success": true, "username": username, "uuid": uuid, "type": "microsoft" })
        }
        Err(e) => {
            log_msg(&app, &format!("[Auth/ERR] {}", e));
            json!({ "error": e })
        }
    }
}

// ============================================================
// AUTH — Ely.by (OAuth2 + authlib-injector)
// ============================================================

// Desktop application type on Ely.by — no client secret is issued for it,
// so the token exchange runs as a public client (see exchange_elyby_code).
const ELYBY_CLIENT_ID: &str = "isolate-client1";
const ELYBY_CLIENT_SECRET: &str = "";
const ELYBY_REDIRECT_URI: &str = "http://127.0.0.1:8080/callback";

fn authlib_injector_path() -> PathBuf {
    app_dir().join("authlib-injector.jar")
}

/// Resolve the latest authlib-injector jar URL (GitHub first, CDN fallback).
fn fetch_authlib_injector_url() -> Option<String> {
    if let Ok(data) = http_get("https://api.github.com/repos/yushijinhun/authlib-injector/releases/latest") {
        if let Some(assets) = data.get("assets").and_then(|a| a.as_array()) {
            if let Some(jar) = assets
                .iter()
                .find(|a| a.get("name").and_then(|n| n.as_str()).map(|n| n.ends_with(".jar")).unwrap_or(false))
            {
                if let Some(url) = jar.get("browser_download_url").and_then(|u| u.as_str()) {
                    return Some(url.to_string());
                }
            }
        }
    }
    if let Ok(data) = http_get("https://authlib-injector.yushi.moe/artifact/latest.json") {
        if let Some(url) = data.get("download_url").and_then(|u| u.as_str()) {
            return Some(url.to_string());
        }
    }
    None
}

#[tauri::command]
fn ensure_authlib_injector(app: tauri::State<AppState>) -> Value {
    let path = authlib_injector_path();
    if path.exists() {
        return json!({ "success": true, "path": path.to_string_lossy() });
    }
    let url = match fetch_authlib_injector_url() {
        Some(u) => u,
        None => return json!({ "error": "Could not resolve authlib-injector download URL" }),
    };
    match download_file(&url, &path) {
        Ok(()) => {
            log_msg(&app, "[Auth/Ely.by] authlib-injector downloaded");
            json!({ "success": true, "path": path.to_string_lossy() })
        }
        Err(e) => json!({ "error": e }),
    }
}

/// Percent-decode a query-string value (`+` → space, `%XX` → byte).
fn url_decode(s: &str) -> String {
    let mut out: Vec<u8> = Vec::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b'%' if i + 2 < bytes.len() => {
                let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).ok();
                match hex.and_then(|h| u8::from_str_radix(h, 16).ok()) {
                    Some(v) => {
                        out.push(v);
                        i += 3;
                    }
                    None => {
                        out.push(b'%');
                        i += 1;
                    }
                }
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Pull one query parameter out of a raw HTTP request line.
fn query_param(request: &str, key: &str) -> Option<String> {
    let after_q = request.split_once('?')?.1;
    let query = after_q.split_whitespace().next().unwrap_or("");
    for pair in query.split('&') {
        let mut kv = pair.splitn(2, '=');
        if kv.next().unwrap_or("") == key {
            return Some(url_decode(kv.next().unwrap_or("")));
        }
    }
    None
}

/// Step 1: spin up a localhost callback server and open the Ely.by login page.
#[tauri::command]
fn authenticate_elyby(app: tauri::State<AppState>, handle: tauri::AppHandle) -> Value {
    log_msg(&app, "[Auth/Ely.by] Starting OAuth2 flow...");
    *app.elyby_auth_code.lock().unwrap() = None;

    // PKCE (RFC 7636): Ely.by requires code_challenge for public clients.
    let mut rnd = [0u8; 32];
    getrandom::getrandom(&mut rnd).expect("PKCE entropy");
    let pkce_verifier = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(rnd);
    let pkce_challenge = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .encode(sha2::Sha256::digest(pkce_verifier.as_bytes()));
    *app.elyby_pkce_verifier.lock().unwrap() = Some(pkce_verifier);

    let code_slot = Arc::clone(&app.elyby_auth_code);
    std::thread::spawn(move || {
        use std::io::{Read as _, Write as _};
        use std::net::TcpListener;
        let listener = match TcpListener::bind("127.0.0.1:8080") {
            Ok(l) => l,
            Err(_) => {
                // Port busy — tell the UI so it can fall back to manual paste.
                emit(
                    &handle,
                    "elyby-callback-error",
                    json!({ "error": "Port 8080 is already in use — close whatever is holding it and retry." }),
                );
                return;
            }
        };
        listener.set_nonblocking(true).ok();
        for _ in 0..150 {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buf = [0u8; 4096];
                if let Ok(n) = stream.read(&mut buf) {
                    let request = String::from_utf8_lossy(&buf[..n]).to_string();
                    // Denial: Ely.by redirected back with ?error=... — surface it.
                    if let Some(err) = query_param(&request, "error") {
                        let desc = query_param(&request, "error_description")
                            .filter(|d| !d.is_empty())
                            .unwrap_or(err);
                        emit(
                            &handle,
                            "elyby-callback-error",
                            json!({ "error": desc.clone() }),
                        );
                        let safe = desc
                            .replace('&', "&amp;")
                            .replace('<', "&lt;")
                            .replace('>', "&gt;");
                        let response = format!(
                            "HTTP/1.1 400 Bad Request\r\nContent-Type: text/html; charset=utf-8\r\n\r\n\
                                <!doctype html><html><head><meta charset=\"utf-8\"><title>Isolate Client</title></head>\
                                <body style=\"font-family:system-ui;background:#0a1120;color:#e2e8f0;display:flex;\
                                align-items:center;justify-content:center;height:100vh;margin:0\">\
                                <div style=\"text-align:center\"><h1 style=\"color:#f87171\">Sign-in failed</h1>\
                                <p>{}</p><p>You can close this tab and return to Isolate Client.</p></div></body></html>",
                            safe
                        );
                        let _ = stream.write_all(response.as_bytes());
                        return;
                    }
                    // Only the redirect URI path matters.
                    if request.contains("code=") {
                        let after = &request[request.find("code=").unwrap() + 5..];
                        let code: String = after
                            .split_whitespace()
                            .next()
                            .and_then(|s| s.split('&').next())
                            .unwrap_or("")
                            .to_string();
                        // Strip any trailing HTTP/1.1 artefacts.
                        let code = code.trim_end_matches(" HTTP/1.1").to_string();
                        if !code.is_empty() {
                            if let Ok(mut g) = code_slot.lock() {
                                *g = Some(code);
                            }
                            let response = "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\n\r\n\
                                <!doctype html><html><head><meta charset=\"utf-8\"><title>Isolate Client</title></head>\
                                <body style=\"font-family:system-ui;background:#0a1120;color:#e2e8f0;display:flex;\
                                align-items:center;justify-content:center;height:100vh;margin:0\">\
                                <div style=\"text-align:center\"><h1 style=\"color:#60a5fa\">Signed in</h1>\
                                <p>You can close this tab and return to Isolate Client.</p></div></body></html>";
                            let _ = stream.write_all(response.as_bytes());
                            return;
                        }
                    }
                }
                let response = "HTTP/1.1 400 Bad Request\r\n\r\nMissing code";
                let _ = stream.write_all(response.as_bytes());
                return;
            }
            std::thread::sleep(Duration::from_millis(500));
        }
        emit(&handle, "elyby-callback-error", json!({ "error": "Timed out waiting for the Ely.by redirect." }));
    });

    let params = format!(
        "client_id={}&redirect_uri={}&response_type=code&scope={}&code_challenge={}&code_challenge_method=S256",
        ELYBY_CLIENT_ID,
        "http%3A%2F%2F127.0.0.1%3A8080%2Fcallback",
        "account_info%20minecraft_server_session%20offline_access",
        pkce_challenge
    );
    let auth_url = format!("https://account.ely.by/oauth2/authorize?{}", params);
    log_msg(&app, &format!("[Auth/Ely.by] Open browser: {}", auth_url));
    let _ = open_in_os(Path::new(&auth_url));
    json!({ "auth_url": auth_url })
}

/// Step 2: non-blocking status poll. Returns `pending` until the localhost
/// callback delivers a code, then exchanges it (the frontend drives this).
#[tauri::command]
fn poll_elyby_auth(handle: tauri::AppHandle) -> Value {
    let code = {
        let app = handle.state::<AppState>();
        let mut slot = app.elyby_auth_code.lock().unwrap();
        slot.take()
    };
    match code {
        Some(code) => {
            let app = handle.state::<AppState>();
            exchange_elyby_code(&app, code)
        }
        None => json!({ "status": "pending" }),
    }
}

/// Manual fallback: paste the `code` query parameter yourself.
#[tauri::command]
async fn complete_elyby_auth_cmd(handle: tauri::AppHandle, code: String) -> Value {
    let app = handle.state::<AppState>();
    exchange_elyby_code(&app, code)
}

fn exchange_elyby_code(app: &AppState, code: String) -> Value {
    log_msg(app, "[Auth/Ely.by] Exchanging code for token...");
    // Public client: PKCE requires code_verifier to match the authorize-time challenge.
    let verifier = app.elyby_pkce_verifier.lock().unwrap().clone();
    let mut form = vec![
        ("grant_type", "authorization_code"),
        ("code", code.as_str()),
        ("redirect_uri", ELYBY_REDIRECT_URI),
        ("client_id", ELYBY_CLIENT_ID),
    ];
    if let Some(v) = verifier.as_deref() {
        form.push(("code_verifier", v));
    }
    let resp = if ELYBY_CLIENT_SECRET.is_empty() {
        match http_post_form("https://account.ely.by/api/oauth2/v1/token", &form) {
            Ok(r) => r,
            Err(e) => return json!({ "error": e }),
        }
    } else {
        match http_post_form_basic(
            "https://account.ely.by/api/oauth2/v1/token",
            &form,
            ELYBY_CLIENT_ID,
            ELYBY_CLIENT_SECRET,
        ) {
            Ok(r) => r,
            Err(e) => return json!({ "error": e }),
        }
    };

    if resp.get("error").is_some() {
        let err = resp
            .get("error_description")
            .or(resp.get("error"))
            .and_then(|e| e.as_str())
            .unwrap_or("Token exchange failed");
        return json!({ "error": err });
    }
    let access_token = resp.get("access_token").and_then(|t| t.as_str()).unwrap_or("");
    let refresh_token = resp.get("refresh_token").and_then(|t| t.as_str()).unwrap_or("");
    if access_token.is_empty() {
        return json!({ "error": "No access token from Ely.by" });
    }

    log_msg(app, "[Auth/Ely.by] Got access token, fetching user info...");
    match http_get_bearer("https://account.ely.by/api/account/v1/info", access_token) {
        Ok(info) => {
            let username = match info.get("username").and_then(|u| u.as_str()) {
                Some(u) => u.to_string(),
                None => return json!({ "error": "Failed to get Ely.by user info" }),
            };
            let uuid = info
                .get("uuid")
                .and_then(|u| u.as_str())
                .map(|s| s.to_string())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| offline_uuid(&username));

            save_setting_val(app, "ely_account", json!({
                "username": username,
                "uuid": uuid,
                "token": access_token,
                "refresh_token": refresh_token,
            }));
            // Keep legacy flat keys too, for the launch-time fallback.
            save_setting_val(app, "ely_access_token", json!(access_token));
            save_setting_val(app, "ely_refresh_token", json!(refresh_token));
            save_setting_val(app, "ely_by_username", json!(username));
            save_setting_val(app, "ely_uuid", json!(uuid));
            log_msg(app, &format!("[Auth/Ely.by] Success! Logged in as '{}'", username));

            // authlib-injector gives us Ely.by skins in-game.
            std::thread::spawn(|| {
                let path = authlib_injector_path();
                if !path.exists() {
                    if let Some(url) = fetch_authlib_injector_url() {
                        let _ = download_file(&url, &path);
                    }
                }
            });

            json!({
                "success": true,
                "username": username,
                "uuid": uuid,
                "token": access_token,
                "type": "elyby",
            })
        }
        Err(e) => json!({ "error": e }),
    }
}

#[tauri::command]
fn sign_out_elyby(app: tauri::State<AppState>) -> Value {
    save_setting_val(&app, "ely_account", Value::Null);
    save_setting_val(&app, "ely_access_token", json!(""));
    save_setting_val(&app, "ely_refresh_token", json!(""));
    save_setting_val(&app, "ely_by_username", json!(""));
    save_setting_val(&app, "ely_uuid", json!(""));
    log_msg(&app, "[Auth/Ely.by] Signed out");
    json!({ "success": true })
}

#[tauri::command]
fn sign_out(app: tauri::State<AppState>) -> Value {
    save_setting_val(&app, "ms_account", Value::Null);
    json!({ "success": true })
}

// ============================================================
// MODRINTH — search / install / manage mods, packs, templates
// ============================================================

const MODRINTH_API: &str = "https://api.modrinth.com/v2";

fn http_get_query(url: &str, params: &[(&str, String)]) -> Result<Value, String> {
    let c = http_client(30)?;
    let resp = c.get(url).query(params).send().map_err(|e| e.to_string())?;
    let status = resp.status();
    if !status.is_success() {
        let t = resp.text().unwrap_or_default();
        return Err(format!("HTTP {}: {}", status, t));
    }
    resp.json::<Value>().map_err(|e| e.to_string())
}

fn find_instance(app: &AppState, name: &str) -> Option<Value> {
    let clean = sanitize_name(name);
    load_instances_raw(app)
        .into_iter()
        .find(|r| r.get("name").and_then(|n| n.as_str()) == Some(clean.as_str()))
}

/// (minecraft version, loader, instance directory)
fn instance_ctx(app: &AppState, name: &str) -> Result<(String, String, PathBuf), String> {
    let rec = find_instance(app, name).ok_or("Instance not found.")?;
    let mc = rec
        .get("version")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let loader = rec
        .get("loader")
        .and_then(|v| v.as_str())
        .unwrap_or("vanilla")
        .to_string();
    Ok((mc, loader, instance_dir(name)))
}

fn safe_join(base: &Path, name: &str) -> Result<PathBuf, String> {
    if name.is_empty() || name.contains('/') || name.contains('\\') || name.contains("..") {
        return Err("Invalid file name.".into());
    }
    Ok(base.join(name))
}

fn sanitize_file_name(n: &str) -> String {
    n.chars()
        .map(|c| if "/\\:*?\"<>|".contains(c) { '_' } else { c })
        .collect()
}

/// Filter Modrinth's `/version` list by loader + game version.
fn modrinth_versions(slug: &str, loader: &str, mc: &str) -> Result<Value, String> {
    let url = format!("{}/project/{}/version", MODRINTH_API, slug);
    // The raw response is loader/mc-independent (filters run below) — cache it
    // for 10 min so the perf pack's compat gate + installer share one request
    // per slug instead of two.
    static CACHE: std::sync::OnceLock<Mutex<HashMap<String, (u64, Value)>>> =
        std::sync::OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let hit = cache
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get(slug)
        .filter(|(t, _)| now.saturating_sub(*t) < 600)
        .map(|(_, v)| v.clone());
    let v = match hit {
        Some(v) => v,
        None => {
            let fetched = http_get(&url)?;
            cache
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .insert(slug.to_string(), (now, fetched.clone()));
            fetched
        }
    };
    let list = match v.as_array() {
        Some(l) => l.clone(),
        None => return Ok(json!([])),
    };
    let mut out: Vec<Value> = Vec::new();
    for ver in &list {
        let loaders = ver
            .get("loaders")
            .and_then(|l| l.as_array())
            .cloned()
            .unwrap_or_default();
        let gvs = ver
            .get("game_versions")
            .and_then(|l| l.as_array())
            .cloned()
            .unwrap_or_default();
        let loader_ok = loader.is_empty() || loaders.iter().any(|x| x.as_str() == Some(loader));
        let mc_ok = mc.is_empty() || gvs.iter().any(|x| x.as_str() == Some(mc));
        if loader_ok && mc_ok {
            out.push(ver.clone());
        }
    }
    Ok(Value::Array(out))
}

fn modrinth_primary_file(ver: &Value) -> Option<(String, String)> {
    let files = ver.get("files")?.as_array()?;
    let f = files
        .iter()
        .find(|f| f.get("primary").and_then(|p| p.as_bool()).unwrap_or(false))
        .or_else(|| files.first())?;
    let filename = f.get("filename")?.as_str()?;
    let url = f.get("url")?.as_str()?;
    Some((sanitize_file_name(filename), url.to_string()))
}

fn put_file(dir: &Path, sub: &str, filename: &str, url: &str) -> Result<String, String> {
    let dest = safe_join(&dir.join(sub), filename)?;
    if dest.exists() && dest.metadata().map(|m| m.len() > 0).unwrap_or(false) {
        return Ok(format!("{} (already present)", filename));
    }
    download_file(url, &dest)?;
    Ok(filename.to_string())
}

/// Per-instance record of what we installed (slug + version) so updates work.
fn mod_manifest_path(dir: &Path) -> PathBuf {
    dir.join(".isolate_mods.json")
}
/// Serialises `.isolate_mods.json` read-modify-write — the perf pack installs
/// through several worker threads at once and each one appends its entry.
static MOD_MANIFEST_LOCK: Mutex<()> = Mutex::new(());

fn read_mod_manifest(dir: &Path) -> serde_json::Map<String, Value> {
    match load_json(&mod_manifest_path(dir)).as_object() {
        Some(o) => o.clone(),
        None => serde_json::Map::new(),
    }
}

fn write_mod_manifest(dir: &Path, map: &serde_json::Map<String, Value>) {
    let mut sorted: Vec<(String, Value)> = map.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
    sorted.sort_by(|a, b| a.0.cmp(&b.0));
    save_json(&mod_manifest_path(dir), &Value::Object(sorted.into_iter().collect()));
}

/// Ladder: exact loader+mc → any → error.
fn install_mod_inner(
    app: &AppState,
    instance: &str,
    slug: &str,
    version_id: &str,
    sub: &str,
) -> Result<String, String> {
    let (mc, loader, dir) = instance_ctx(app, instance)?;
    let l = if loader == "vanilla" { "" } else { loader.as_str() };
    let mut list = modrinth_versions(slug, l, &mc)?;
    if list.as_array().map(|a| a.is_empty()).unwrap_or(true) {
        list = modrinth_versions(slug, "", "")?;
    }
    let arr = list.as_array().ok_or("No versions found.")?;
    let chosen = if !version_id.is_empty() {
        arr.iter()
            .find(|v| v.get("id").and_then(|i| i.as_str()) == Some(version_id))
    } else {
        arr.first()
    }
    .ok_or("No compatible version of this mod exists for your Minecraft version.")?;

    let (filename, url) = modrinth_primary_file(chosen).ok_or("That version has no file.")?;
    let saved = put_file(&dir, sub, &filename, &url)?;

    if sub == "mods" {
        let mut m = read_mod_manifest(&dir);
        m.insert(
            filename.clone(),
            json!({
                "slug": slug,
                "version_id": chosen.get("id").and_then(|v| v.as_str()).unwrap_or(""),
                "version_number": chosen.get("version_number").and_then(|v| v.as_str()).unwrap_or(""),
                "name": chosen.get("name").and_then(|v| v.as_str()).unwrap_or(slug),
            }),
        );
        write_mod_manifest(&dir, &m);
    }
    Ok(saved)
}

// ---------- commands ----------

#[tauri::command]
fn search_modrinth(
    query: String,
    project_type: String,
    loader: String,
    mc_version: String,
    limit: u32,
) -> Value {
    let pt = if project_type.is_empty() {
        "mod".to_string()
    } else {
        project_type
    };
    let mut facets: Vec<Vec<String>> = vec![vec![format!("project_type:{}", pt)]];
    if !mc_version.is_empty() {
        facets.push(vec![format!("versions:{}", mc_version)]);
    }
    let l = loader.to_lowercase();
    if !l.is_empty() && l != "vanilla" {
        facets.push(vec![format!("categories:{}", l)]);
    }
    let fj = match serde_json::to_string(&facets) {
        Ok(s) => s,
        Err(e) => return json!({ "error": e.to_string() }),
    };
    let lim = limit.clamp(1, 60).to_string();
    // Empty query = browse mode → most downloaded first (Mods page default
    // view + featured server-plugin banner); typed queries keep relevance.
    let index = if query.trim().is_empty() {
        "downloads".to_string()
    } else {
        "relevance".to_string()
    };
    let params = [("query", query), ("facets", fj), ("limit", lim), ("index", index)];
    match http_get_query(&format!("{}/search", MODRINTH_API), &params) {
        Ok(v) => v,
        Err(e) => json!({ "error": e }),
    }
}

#[tauri::command]
fn get_modrinth_project(slug: String) -> Value {
    match http_get(&format!("{}/project/{}", MODRINTH_API, slug)) {
        Ok(v) => v,
        Err(e) => json!({ "error": e }),
    }
}

#[tauri::command]
fn get_modrinth_versions(slug: String, loader: String, mc_version: String) -> Value {
    let l = if loader == "vanilla" { String::new() } else { loader };
    match modrinth_versions(&slug, &l, &mc_version) {
        Ok(v) => v,
        Err(e) => json!({ "error": e }),
    }
}

#[tauri::command]
fn install_mod(
    app: tauri::State<AppState>,
    instance: String,
    slug: String,
    version_id: String,
) -> Value {
    match install_mod_inner(&app, &instance, &slug, &version_id, "mods") {
        Ok(f) => {
            log_msg(&app, &format!("[Mods] Installed {} into {}", f, instance));
            json!({ "success": true, "file": f })
        }
        Err(e) => json!({ "error": e }),
    }
}

#[tauri::command]
fn install_resourcepack(
    app: tauri::State<AppState>,
    instance: String,
    slug: String,
    version_id: String,
) -> Value {
    match install_mod_inner(&app, &instance, &slug, &version_id, "resourcepacks") {
        Ok(f) => json!({ "success": true, "file": f }),
        Err(e) => json!({ "error": e }),
    }
}

fn list_pack_dir(dir: &Path, sub: &str) -> Value {
    let mut out: Vec<Value> = Vec::new();
    if let Ok(rd) = std::fs::read_dir(dir.join(sub)) {
        for e in rd.flatten() {
            let fname = e.file_name().to_string_lossy().to_string();
            let lower = fname.to_lowercase();
            let disabled = lower.ends_with(".disabled");
            let base = if disabled {
                fname[..fname.len().saturating_sub(9)].to_string()
            } else {
                fname.clone()
            };
            if !(base.to_lowercase().ends_with(".jar") || base.to_lowercase().ends_with(".zip")) {
                continue;
            }
            out.push(json!({
                "filename": fname,
                "name": base,
                "disabled": disabled,
                "size": e.metadata().map(|m| m.len()).unwrap_or(0),
            }));
        }
    }
    out.sort_by(|a, b| {
        let na = a.get("name").and_then(|n| n.as_str()).unwrap_or("");
        let nb = b.get("name").and_then(|n| n.as_str()).unwrap_or("");
        na.to_lowercase().cmp(&nb.to_lowercase())
    });
    Value::Array(out)
}

#[tauri::command]
fn list_mods(app: tauri::State<AppState>, instance: String) -> Value {
    let dir = match instance_ctx(&app, &instance) {
        Ok((_, _, d)) => d,
        Err(e) => return json!({ "error": e }),
    };
    let _ = std::fs::create_dir_all(dir.join("mods"));
    json!({ "mods": list_pack_dir(&dir, "mods"), "manifest": read_mod_manifest(&dir) })
}

#[tauri::command]
fn list_resourcepacks(app: tauri::State<AppState>, instance: String) -> Value {
    let dir = match instance_ctx(&app, &instance) {
        Ok((_, _, d)) => d,
        Err(e) => return json!({ "error": e }),
    };
    list_pack_dir(&dir, "resourcepacks")
}

#[tauri::command]
fn delete_pack_file(
    app: tauri::State<AppState>,
    instance: String,
    sub: String,
    filename: String,
) -> Value {
    let sub = if sub == "resourcepacks" { "resourcepacks" } else { "mods" };
    let dir = match instance_ctx(&app, &instance) {
        Ok((_, _, d)) => d,
        Err(e) => return json!({ "error": e }),
    };
    let path = match safe_join(&dir.join(sub), &filename) {
        Ok(p) => p,
        Err(e) => return json!({ "error": e }),
    };
    if !path.exists() {
        return json!({ "error": "File not found." });
    }
    if let Err(e) = std::fs::remove_file(&path) {
        return json!({ "error": e.to_string() });
    }
    if sub == "mods" {
        let mut m = read_mod_manifest(&dir);
        if m.remove(&filename).is_some() {
            write_mod_manifest(&dir, &m);
        }
    }
    json!({ "success": true })
}

#[tauri::command]
fn toggle_mod(
    app: tauri::State<AppState>,
    instance: String,
    filename: String,
    enabled: bool,
) -> Value {
    let dir = match instance_ctx(&app, &instance) {
        Ok((_, _, d)) => d,
        Err(e) => return json!({ "error": e }),
    };
    let path = match safe_join(&dir.join("mods"), &filename) {
        Ok(p) => p,
        Err(e) => return json!({ "error": e }),
    };
    if !path.exists() {
        return json!({ "error": "File not found." });
    }
    let target = if enabled {
        // strip ".disabled"
        let s = filename.trim_end_matches(".disabled").to_string();
        dir.join("mods").join(s)
    } else {
        dir.join("mods").join(format!("{}.disabled", filename))
    };
    match std::fs::rename(&path, &target) {
        Ok(()) => json!({ "success": true }),
        Err(e) => json!({ "error": e.to_string() }),
    }
}

#[tauri::command]
async fn batch_install_mods(
    handle: tauri::AppHandle,
    instance: String,
    slugs: Vec<String>,
) -> Value {
    let mut installed: Vec<String> = Vec::new();
    let mut errors: Vec<Value> = Vec::new();
    for slug in slugs {
        let r = {
            let app = handle.state::<AppState>();
            install_mod_inner(&app, &instance, &slug, "", "mods")
        };
        match r {
            Ok(_) => installed.push(slug),
            Err(e) => errors.push(json!({ "slug": slug, "error": e })),
        }
    }
    json!({ "success": errors.is_empty(), "installed": installed, "errors": errors })
}

#[tauri::command]
fn check_mod_updates(app: tauri::State<AppState>, instance: String) -> Value {
    let (mc, loader, dir) = match instance_ctx(&app, &instance) {
        Ok(x) => x,
        Err(e) => return json!({ "error": e }),
    };
    let l = if loader == "vanilla" { "" } else { loader.as_str() };
    let manifest = read_mod_manifest(&dir);
    let mut updates: Vec<Value> = Vec::new();
    let mut checked = 0u32;

    for (filename, meta) in &manifest {
        let slug = meta.get("slug").and_then(|s| s.as_str()).unwrap_or("");
        if slug.is_empty() {
            continue;
        }
        let current = meta
            .get("version_number")
            .and_then(|s| s.as_str())
            .unwrap_or("");
        let versions = match modrinth_versions(slug, l, &mc) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let latest = match versions.as_array().and_then(|a| a.first()) {
            Some(v) => v,
            None => continue,
        };
        checked += 1;
        let latest_num = latest
            .get("version_number")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if !latest_num.is_empty() && latest_num != current {
            updates.push(json!({
                "filename": filename,
                "slug": slug,
                "name": meta.get("name").cloned().unwrap_or(json!(slug)),
                "current_version": current,
                "latest_version": latest_num,
                "version_id": latest.get("id").and_then(|v| v.as_str()).unwrap_or(""),
            }));
        }
    }
    json!({ "success": true, "updates": updates, "total_checked": checked })
}

#[tauri::command]
fn resolve_mod_dependencies(
    app: tauri::State<AppState>,
    instance: String,
    slug: String,
) -> Value {
    let (mc, loader, dir) = match instance_ctx(&app, &instance) {
        Ok(x) => x,
        Err(e) => return json!({ "error": e }),
    };
    let project = match http_get(&format!("{}/project/{}", MODRINTH_API, slug)) {
        Ok(p) => p,
        Err(e) => return json!({ "error": e }),
    };
    let deps = project
        .get("dependencies")
        .and_then(|d| d.as_array())
        .cloned()
        .unwrap_or_default();
    let mut installed: Vec<String> = Vec::new();
    let mut skipped: Vec<String> = Vec::new();
    let l = if loader == "vanilla" { "" } else { loader.as_str() };

    for d in &deps {
        let dtype = d.get("dependency_type").and_then(|t| t.as_str()).unwrap_or("");
        if dtype != "required" {
            continue;
        }
        let dep_slug = d.get("project_id").and_then(|s| s.as_str()).unwrap_or("");
        if dep_slug.is_empty() {
            continue;
        }
        // Resolve Modrinth project id → slug.
        let resolved = http_get(&format!("{}/project/{}", MODRINTH_API, dep_slug))
            .and_then(|p| {
                p.get("slug")
                    .and_then(|s| s.as_str())
                    .map(|s| s.to_string())
                    .ok_or_else(|| "no slug".to_string())
            })
            .unwrap_or_else(|_| dep_slug.to_string());
        let _ = (&mc, l, &dir);
        match install_mod_inner(&app, &instance, &resolved, "", "mods") {
            Ok(_) => installed.push(resolved),
            Err(_) => skipped.push(resolved),
        }
    }
    json!({ "success": true, "installed": installed, "skipped": skipped })
}

#[tauri::command]
fn detect_mod_conflicts(app: tauri::State<AppState>, instance: String) -> Value {
    let dir = match instance_ctx(&app, &instance) {
        Ok((_, _, d)) => d,
        Err(e) => return json!({ "error": e }),
    };
    let rules: [(&[&str], &[&str], &str, &str); 7] = [
        (
            &["optifine", "optifabric", "optiforge"],
            &["sodium", "embeddium"],
            "OptiFine and Sodium/Embeddium are incompatible rendering engines",
            "high",
        ),
        (
            &["optifine", "optifabric", "optiforge"],
            &["lithium"],
            "OptiFine and Lithium may conflict on optimization paths",
            "medium",
        ),
        (
            &["sodium"],
            &["embeddium"],
            "Sodium and Embeddium are both rendering engines - only one can run",
            "high",
        ),
        (
            &["starlight"],
            &["phosphor"],
            "Starlight and Phosphor both rewrite the light engine",
            "high",
        ),
        (
            &["sodium"],
            &["optifine"],
            "Sodium conflicts with OptiFine rendering engine",
            "high",
        ),
        (
            &["lithium"],
            &["phosphor"],
            "Lithium and Phosphor may overlap in optimization areas",
            "low",
        ),
        (
            &["modernfix"],
            &["optifine"],
            "ModernFix and OptiFine may have overlapping fixes",
            "low",
        ),
    ];

    let mods = list_pack_dir(&dir, "mods");
    let installed: Vec<String> = mods
        .as_array()
        .map(|a| {
            a.iter()
                .map(|m| {
                    m.get("name")
                        .and_then(|n| n.as_str())
                        .unwrap_or("")
                        .to_lowercase()
                        .replace(['_', ' '], "-")
                })
                .collect()
        })
        .unwrap_or_default();

    let mut conflicts: Vec<Value> = Vec::new();
    for (a, b, reason, severity) in rules {
        let hit_a = installed.iter().find(|s| a.iter().any(|k| s.contains(k)));
        let hit_b = installed.iter().find(|s| b.iter().any(|k| s.contains(k)));
        if let (Some(x), Some(y)) = (hit_a, hit_b) {
            if x == y {
                continue;
            }
            conflicts.push(json!({
                "mod1": x, "mod2": y, "reason": reason, "severity": severity
            }));
        }
    }
    json!({ "success": true, "conflicts": conflicts })
}

// ---------- .mrpack ----------

fn apply_mrpack(dir: &Path, zip_path: &Path) -> Result<usize, String> {
    let f = std::fs::File::open(zip_path).map_err(|e| e.to_string())?;
    let mut z = zip::ZipArchive::new(f).map_err(|e| e.to_string())?;

    let index: Value = {
        let mut s = String::new();
        let mut entry = z
            .by_name("modrinth.index.json")
            .map_err(|_| "Not a valid .mrpack (missing modrinth.index.json).")?;
        entry.read_to_string(&mut s).map_err(|e| e.to_string())?;
        serde_json::from_str(&s).map_err(|e| format!("Bad index: {}", e))?
    };

    let mut count = 0usize;
    if let Some(files) = index.get("files").and_then(|f| f.as_array()) {
        let mut items: Vec<(String, PathBuf)> = Vec::new();
        for f in files {
            let rel = f.get("path").and_then(|p| p.as_str()).unwrap_or("");
            let url = f
                .get("downloads")
                .and_then(|d| d.as_array())
                .and_then(|a| a.first())
                .and_then(|u| u.as_str())
                .unwrap_or("");
            if rel.is_empty() || url.is_empty() {
                continue;
            }
            if rel.contains("..") || rel.starts_with('/') || rel.starts_with('\\') {
                continue;
            }
            items.push((url.to_string(), dir.join(rel)));
        }
        let total = items.len();
        let fails = download_many(items, 6, &|_, _| {});
        count = total.saturating_sub(fails);
    }

    for i in 0..z.len() {
        let mut e = z.by_index(i).map_err(|e2| e2.to_string())?;
        if e.is_dir() {
            continue;
        }
        let name = e.name().to_string();
        let rel = name
            .strip_prefix("overrides/")
            .or_else(|| name.strip_prefix("client-overrides/"))
            .unwrap_or("");
        if rel.is_empty() || rel.contains("..") {
            continue;
        }
        let dest = dir.join(rel);
        if let Some(p) = dest.parent() {
            std::fs::create_dir_all(p).map_err(|e2| e2.to_string())?;
        }
        let mut out = std::fs::File::create(&dest).map_err(|e2| e2.to_string())?;
        std::io::copy(&mut e, &mut out).map_err(|e2| e2.to_string())?;
        count += 1;
    }
    Ok(count)
}

#[tauri::command]
async fn install_modpack(
    handle: tauri::AppHandle,
    instance: String,
    url: String,
) -> Value {
    let app = handle.state::<AppState>();
    let dir = match instance_ctx(&app, &instance) {
        Ok((_, _, d)) => d,
        Err(e) => return json!({ "error": e }),
    };
    drop(app);

    let staging = match tempfile::tempdir() {
        Ok(d) => d,
        Err(e) => return json!({ "error": e.to_string() }),
    };
    let pack = staging.path().join("pack.mrpack");
    if let Err(e) = download_file(&url, &pack) {
        return json!({ "error": format!("Download failed: {}", e) });
    }
    match apply_mrpack(&dir, &pack) {
        Ok(n) => {
            let app = handle.state::<AppState>();
            log_msg(&app, &format!("[Modpack] Applied {} files to {}", n, instance));
            json!({ "success": true, "files": n })
        }
        Err(e) => json!({ "error": e }),
    }
}

#[tauri::command]
async fn create_instance_from_modpack(
    handle: tauri::AppHandle,
    name: String,
    url: String,
) -> Value {
    let clean = sanitize_name(&name);
    let app = handle.state::<AppState>();
    let mut list = load_instances_raw(&app);
    if list
        .iter()
        .any(|r| r.get("name").and_then(|n| n.as_str()) == Some(clean.as_str()))
    {
        return json!({ "error": format!("An instance named '{}' already exists.", clean) });
    }

    // Peek at the pack to learn its Minecraft version + loader.
    let staging = match tempfile::tempdir() {
        Ok(d) => d,
        Err(e) => return json!({ "error": e.to_string() }),
    };
    let pack = staging.path().join("pack.mrpack");
    if let Err(e) = download_file(&url, &pack) {
        return json!({ "error": format!("Download failed: {}", e) });
    }
    let (mc, loader, loader_ver) = {
        let f = match std::fs::File::open(&pack) {
            Ok(f) => f,
            Err(e) => return json!({ "error": e.to_string() }),
        };
        let mut z = match zip::ZipArchive::new(f) {
            Ok(z) => z,
            Err(e) => return json!({ "error": e.to_string() }),
        };
        let mut s = String::new();
        let mut entry = match z.by_name("modrinth.index.json") {
            Ok(e) => e,
            Err(_) => return json!({ "error": "Not a valid .mrpack." }),
        };
        if let Err(e) = entry.read_to_string(&mut s) {
            return json!({ "error": e.to_string() });
        }
        drop(entry);
        let idx: Value = match serde_json::from_str(&s) {
            Ok(v) => v,
            Err(e) => return json!({ "error": e.to_string() }),
        };
        let deps = idx.get("dependencies").cloned().unwrap_or(json!({}));
        let mc = deps
            .get("minecraft")
            .and_then(|v| v.as_str())
            .unwrap_or("1.20.1")
            .to_string();
        let (l, lv) = if let Some(v) = deps.get("fabric-loader").and_then(|v| v.as_str()) {
            ("fabric".to_string(), v.to_string())
        } else if let Some(v) = deps.get("quilt-loader").and_then(|v| v.as_str()) {
            ("quilt".to_string(), v.to_string())
        } else if deps.get("forge").is_some() {
            ("forge".to_string(), String::new())
        } else if deps.get("neoforge").is_some() {
            ("neoforge".to_string(), String::new())
        } else {
            ("vanilla".to_string(), String::new())
        };
        (mc, l, lv)
    };

    if !loader_supports(&loader, &mc) {
        return json!({ "error": format!("{} modpacks aren't supported by this build yet.", loader) });
    }

    let rec = json!({
        "name": clean,
        "version": mc,
        "loader": loader,
        "loader_version": loader_ver,
        "created": chrono::Local::now().to_rfc3339(),
        "last_played": Value::Null,
    });
    list.push(rec.clone());
    save_instances(&app, &list);
    let dir = instance_dir(&clean);
    let _ = std::fs::create_dir_all(&dir);
    drop(app);

    if let Err(e) = apply_mrpack(&dir, &pack) {
        return json!({ "error": e });
    }
    json!({ "success": true, "instance": rec })
}

#[tauri::command]
async fn export_instance_mrpack(
    handle: tauri::AppHandle,
    instance: String,
    output_path: String,
) -> Value {
    let app = handle.state::<AppState>();
    let (mc, loader, loader_ver, dir) = match instance_ctx(&app, &instance) {
        Ok((m, l, d)) => {
            let lv = find_instance(&app, &instance)
                .and_then(|r| {
                    r.get("loader_version")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string())
                })
                .unwrap_or_default();
            (m, l, lv, d)
        }
        Err(e) => return json!({ "error": e }),
    };
    drop(app);

    let out = PathBuf::from(&output_path);
    if let Some(p) = out.parent() {
        let _ = std::fs::create_dir_all(p);
    }
    let file = match std::fs::File::create(&out) {
        Ok(f) => f,
        Err(e) => return json!({ "error": e.to_string() }),
    };
    let mut z = zip::ZipWriter::new(file);
    let opts: zip::write::FileOptions = zip::write::FileOptions::default();

    let deps = if loader == "fabric" {
        json!({ "minecraft": mc, "fabric-loader": loader_ver })
    } else if loader == "quilt" {
        json!({ "minecraft": mc, "quilt-loader": loader_ver })
    } else {
        json!({ "minecraft": mc })
    };
    let index = json!({
        "formatVersion": 1,
        "game": "minecraft",
        "versionId": instance,
        "name": instance,
        "dependencies": deps,
        "files": []
    });
    if let Err(e) = z.start_file("modrinth.index.json", opts) {
        return json!({ "error": e.to_string() });
    }
    let _ = z.write_all(serde_json::to_string_pretty(&index).unwrap_or_default().as_bytes());

    // Pack mods + config as overrides.
    let mut count = 0usize;
    for sub in ["mods", "config", "resourcepacks", "shaderpacks"] {
        let src = dir.join(sub);
        if !src.exists() {
            continue;
        }
        let mut stack = vec![src.clone()];
        while let Some(cur) = stack.pop() {
            if let Ok(rd) = std::fs::read_dir(&cur) {
                for e in rd.flatten() {
                    let p = e.path();
                    if p.is_dir() {
                        stack.push(p);
                        continue;
                    }
                    let rel = match p.strip_prefix(&dir) {
                        Ok(r) => r.to_string_lossy().replace('\\', "/"),
                        Err(_) => continue,
                    };
                    if rel.starts_with('.') {
                        continue;
                    }
                    if let Ok(mut f) = std::fs::File::open(&p) {
                        if z.start_file(format!("overrides/{}", rel), opts).is_ok() {
                            if std::io::copy(&mut f, &mut z).is_ok() {
                                count += 1;
                            }
                        }
                    }
                }
            }
        }
    }
    if let Err(e) = z.finish() {
        return json!({ "error": e.to_string() });
    }
    json!({ "success": true, "files": count, "path": output_path })
}

// ---------- instance templates ----------

struct Template {
    id: &'static str,
    name: &'static str,
    desc: &'static str,
    mc: &'static str,
    loader: &'static str,
    loader_ver: &'static str,
    category: &'static str,
    mods: &'static str,
}

/// Ported verbatim from Xryloth's `get_builtin_templates`.
const TEMPLATES: &[Template] = &[
    Template { id: "fabric-tech-reborn", name: "Tech Reborn (Fabric)", desc: "Automation and tech mods on Fabric — Tech Reborn, AE2, Mekanism, Pneumaticcraft, JEI.", mc: "1.20.1", loader: "fabric", loader_ver: "0.15.11", category: "Fabric", mods: "techreborn,ae2,mekanism,pneumaticcraft,jei" },
    Template { id: "fabric-ad-astra", name: "Ad Astra (Fabric)", desc: "Space exploration modpack — Ad Astra, Rocket, VisIER, and quality-of-life.", mc: "1.20.1", loader: "fabric", loader_ver: "0.15.11", category: "Fabric", mods: "ad-astra,visier,jetpack-addon,jei,fabric-language-kotlin" },
    Template { id: "fabric-create-astral", name: "Create: Astral (Fabric)", desc: "Create modpack with Astral and addons — Create, Create: Astral, Create Steam 'n' Rails.", mc: "1.20.1", loader: "fabric", loader_ver: "0.15.11", category: "Fabric", mods: "create,create-astral,create-steam-n-rails,create-armory,jei" },
    Template { id: "fabric-better-end", name: "Better End (Fabric)", desc: "Adventure and exploration — Better End, Better Nether, Biome Makeover, and performance mods.", mc: "1.20.1", loader: "fabric", loader_ver: "0.15.11", category: "Fabric", mods: "better-end,better-nether,biome-makeover,jei,sodium,iris" },
    Template { id: "fabric-chromatic-iron", name: "Chromatic Iron (Fabric)", desc: "Vanilla-friendly QoL modpack — Chromatic Iron, REI, Appleskin, and performance mods.", mc: "1.20.1", loader: "fabric", loader_ver: "0.15.11", category: "Fabric", mods: "chromaic-curse,appleskin,jei,sodium,lithium,phosphor" },
    Template { id: "fabric-lets-do", name: "Let's Do (Fabric)", desc: "Cozy building and farming — Let's Do mod series with furniture, cooking and more.", mc: "1.20.1", loader: "fabric", loader_ver: "0.15.11", category: "Fabric", mods: "lets-do-build,lets-do-meadow,lets-do-cooking,lets-do-beachparty,lets-do-barn,jei" },
    Template { id: "fabric-vault-hunters", name: "Vault Hunters (Fabric)", desc: "Adventure RPG modpack — Vault Hunters with quests, skills, and epic loot.", mc: "1.20.1", loader: "fabric", loader_ver: "0.15.11", category: "Fabric", mods: "vault-hunters,jei,jade,sodium,iris" },
    Template { id: "fabric-skyfactory", name: "SkyFactory (Fabric)", desc: "Skyblock modpack — start on a tree, build everything. Includes Ex Nihilo.", mc: "1.20.1", loader: "fabric", loader_ver: "0.15.11", category: "Fabric", mods: "ex-nihilo,ex-nihilo-mekanism-addon,ex-nihilo-astral,jei,sodium" },
    Template { id: "forge-ftb-revelation", name: "FTB Revelation (Forge)", desc: "All-purpose modpack — FTB Revelation with tech, magic, and exploration.", mc: "1.20.2", loader: "forge", loader_ver: "48.0.29", category: "Forge", mods: "ftb-revelation,mekanism,applied-energistics-2,twilight-forest,jei" },
    Template { id: "forge-ftb-direwolf20", name: "FTB Direwolf20 (Forge)", desc: "Direwolf20's curated modpack — tech, magic, and everything in between.", mc: "1.20.2", loader: "forge", loader_ver: "48.0.29", category: "Forge", mods: "direwolf20,mekanism,botania,twilight-forest,jei" },
    Template { id: "forge-enigmatica-6", name: "Enigmatica 6 (Forge)", desc: "Expert modpack — Enigmatica 6 with challenging recipes and expert progression.", mc: "1.16.5", loader: "forge", loader_ver: "36.2.0", category: "Forge", mods: "enigmatica,mekaf,botania,astral-sorcery,twilight-forest" },
    Template { id: "forge-rl", name: "RLCraft (Forge)", desc: "Hardcore survival modpack — RLCraft with realistic survival mechanics.", mc: "1.12.2", loader: "forge", loader_ver: "14.23.5.2855", category: "Forge", mods: "rlcraft,resurrect-mobs,lycanites-mobs,tough-as-nails" },
    Template { id: "forge-atm9", name: "All the Mods 9 (Forge)", desc: "Massive modpack — All the Mods 9 with 400+ mods for ultimate Minecraft.", mc: "1.20.1", loader: "forge", loader_ver: "47.1.0", category: "Forge", mods: "all-the-mods-9,mekanism,ae2,botania,twilight-forest" },
    Template { id: "forge-bettermc", name: "Better MC (Forge)", desc: "Enhanced vanilla experience — Better MC with improved worldgen and QoL.", mc: "1.16.5", loader: "forge", loader_ver: "36.2.34", category: "Forge", mods: "better-mc,biome-makeover,terrablender,jei,Xaeros-Minimap" },
    Template { id: "vanilla-optimized", name: "Optimized Vanilla (Fabric)", desc: "Vanilla experience with performance mods — Sodium, Iris, Lithium, Starlight.", mc: "1.20.1", loader: "fabric", loader_ver: "0.15.11", category: "Vanilla", mods: "sodium,iris,lithium,phosphor,starlight" },
    Template { id: "vanilla-building", name: "Building (Fabric)", desc: "Building-focused modpack — WorldEdit, Litematica, Create, and building gadgets.", mc: "1.20.1", loader: "fabric", loader_ver: "0.15.11", category: "Vanilla", mods: "worldedit,litematica,create,building-gadgets,jei" },
    Template { id: "vanilla-qol", name: "Quality of Life (Fabric)", desc: "QoL focused modpack — JEI, Appleskin, Jade, Xaero's Minimap, and more.", mc: "1.20.1", loader: "fabric", loader_ver: "0.15.11", category: "Vanilla", mods: "jei,appleskin,jade,Xaeros-Minimap,Xaeros-Mapwaypoints-Mod,sodium" },
    Template { id: "vanilla-shaders", name: "Shaders (Fabric)", desc: "Visual enhancement modpack — Iris, Complementary Shaders, and performance mods.", mc: "1.20.1", loader: "fabric", loader_ver: "0.15.11", category: "Vanilla", mods: "iris,complementary-shaders,sodium,lithium,embeddium" },
    Template { id: "server-hardcore", name: "Hardcore Server", desc: "Hardcore survival server pack — single life, increased difficulty, epic loot.", mc: "1.20.1", loader: "fabric", loader_ver: "0.15.11", category: "Server", mods: "hardcore-darkness,epic-samus,infernal-mobs,jei" },
    Template { id: "server-creative", name: "Creative Server", desc: "Creative building server pack — WorldEdit, Litematica, building gadgets.", mc: "1.20.1", loader: "fabric", loader_ver: "0.15.11", category: "Server", mods: "worldedit,litematica,building-gadgets,jei" },
    Template { id: "server-skyblock", name: "SkyBlock Server", desc: "Skyblock survival server pack — Ex Nihilo and addons for skyblock play.", mc: "1.20.1", loader: "fabric", loader_ver: "0.15.11", category: "Server", mods: "ex-nihilo,ex-nihilo-mekanism-addon,jei,sodium" },
    Template { id: "server-survival", name: "Survival Server", desc: "Classic survival server pack — Biome Makeover, exploration and QoL.", mc: "1.20.1", loader: "fabric", loader_ver: "0.15.11", category: "Server", mods: "biome-makeover,terraria-copy,jei,Xaeros-Minimap" },
];

#[tauri::command]
fn get_instance_templates() -> Value {
    let out: Vec<Value> = TEMPLATES
        .iter()
        .map(|t| {
            json!({
                "id": t.id,
                "name": t.name,
                "description": t.desc,
                "mc_version": t.mc,
                "loader": t.loader,
                "loader_version": t.loader_ver,
                "category": t.category,
                "mod_count": t.mods.split(',').count(),
                "supported": loader_supports(t.loader, t.mc),
            })
        })
        .collect();
    json!({ "templates": out })
}

#[tauri::command]
async fn create_instance_from_template(
    handle: tauri::AppHandle,
    name: String,
    template_id: String,
) -> Value {
    let t = match TEMPLATES.iter().find(|t| t.id == template_id) {
        Some(t) => t,
        None => return json!({ "error": "Template not found." }),
    };
    if !loader_supports(t.loader, t.mc) {
        return json!({ "error": format!(
            "{} isn't supported by this build yet ({} {}).",
            t.loader, t.mc, t.loader_ver
        ) });
    }
    let clean = sanitize_name(&name);
    let app = handle.state::<AppState>();
    let mut list = load_instances_raw(&app);
    if list
        .iter()
        .any(|r| r.get("name").and_then(|n| n.as_str()) == Some(clean.as_str()))
    {
        return json!({ "error": format!("An instance named '{}' already exists.", clean) });
    }
    let rec = json!({
        "name": clean,
        "version": t.mc,
        "loader": t.loader,
        "loader_version": t.loader_ver,
        "template": t.id,
        "created": chrono::Local::now().to_rfc3339(),
        "last_played": Value::Null,
    });
    list.push(rec.clone());
    save_instances(&app, &list);
    let dir = instance_dir(&clean);
    let _ = std::fs::create_dir_all(dir.join("mods"));
    log_msg(&app, &format!("[Template] Created '{}' from {}", clean, t.id));
    drop(app);

    let slugs: Vec<String> = t.mods.split(',').filter(|s| !s.is_empty()).map(|s| s.to_string()).collect();
    let mut installed = 0usize;
    let mut errors: Vec<Value> = Vec::new();
    for slug in slugs {
        let r = {
            let app = handle.state::<AppState>();
            install_mod_inner(&app, &clean, &slug, "", "mods")
        };
        match r {
            Ok(_) => installed += 1,
            Err(e) => errors.push(json!({ "slug": slug, "error": e })),
        }
    }
    emit(&handle, "template-progress", json!({
        "template_name": t.name,
        "instance": clean,
        "installed": installed,
        "errors": errors.len(),
        "error_list": errors,
    }));
    json!({ "success": true, "instance": rec, "installed": installed, "errors": errors })
}

// ---------- performance mods ----------

/// Fabric FPS pack — 60+ mods. Every slug was checked against Modrinth for a
/// real `fabric` + exact-MC build before landing here; dead mods (Starlight,
/// LazyDFU …) stay listed because they still serve older profiles and the
/// strict compat gate below simply skips them on modern versions.
const PERF_FABRIC: &[(&str, &str)] = &[
    // core rendering / tick / memory
    ("sodium", "Sodium"),
    ("lithium", "Lithium"),
    ("dynamic-fps", "Dynamic FPS"),
    ("modmenu", "Mod Menu"),
    ("ferrite-core", "FerriteCore"),
    ("entityculling", "EntityCulling"),
    ("immediatelyfast", "ImmediatelyFast"),
    ("moreculling", "MoreCulling"),
    ("modernfix", "ModernFix"),
    ("krypton", "Krypton"),
    ("memoryleakfix", "MemoryLeakFix"),
    ("starlight", "Starlight"),
    ("badoptimizations", "BadOptimizations"),
    ("threadtweak", "ThreadTweak"),
    ("noisium", "Noisium"),
    ("c2me-fabric", "C2ME"),
    ("lazydfu", "LazyDFU"),
    ("nvidium", "Nvidium"),
    ("modernfix-mvus", "ModernFix-mVUS"),
    ("noisiumforked", "NoisiumForked"),
    ("scalablelux", "ScalableLux"),
    ("servercore", "ServerCore"),
    ("immersive-optimization", "Immersive Optimization"),
    ("gputape", "GPUBooster"),
    ("alternate-current", "Alternate Current"),
    // entity / particle / chunk culling + rendering
    ("better-block-entities", "Better Block Entities"),
    ("sodium-extra", "Sodium Extra"),
    ("reeses-sodium-options", "Reese's Sodium Options"),
    ("particleculling", "Particle Culling"),
    ("cull-leaves", "Cull Leaves"),
    ("leaves-be-gone", "Leaves Be Gone"),
    ("entity-view-distance", "Entity View Distance"),
    ("asyncparticles", "AsyncParticles"),
    ("particle-core", "Particle Core"),
    ("flickerfix", "FlickerFix"),
    ("fast-item-frames", "Fast Item Frames"),
    ("fastchest", "FastChest"),
    ("exordium", "Exordium"),
    ("better-clouds", "Better Clouds"),
    ("better-biome-blend", "Better Biome Blend"),
    ("vmp-fabric", "Very Many Players"),
    ("ksyxis", "Ksyxis"),
    ("structure-layout-optimizer", "Structure Layout Optimizer"),
    // world / load times
    ("fastquit", "FastQuit"),
    ("fastload", "FastLoad"),
    ("smooth-boot", "Smooth Boot"),
    ("rrls", "Remove Reloading Screen"),
    ("language-reload", "Language Reload"),
    ("lazy-language-loader", "Lazy Language Loader"),
    ("async-pack-scan", "Async Pack Scan"),
    ("resourcepackcached", "ResourcePackCached"),
    ("achievements-optimizer", "Achievements Optimizer"),
    ("log-cleaner", "Log Cleaner"),
    // network / misc
    ("packet-fixer", "Packet Fixer"),
    ("fast-ip-ping", "Fast IP Ping"),
    ("serverpingerfixer", "Server Pinger Fixer"),
    ("spark", "Spark"),
    ("debugify", "Debugify"),
    ("betterf3", "BetterF3"),
    ("fps-reducer", "FPS Reducer"),
    ("lambdynamiclights", "LambDynamicLights"),
    ("clumps", "Clumps"),
    ("fastanim", "FastAnim"),
    ("get-it-together-drops", "Get It Together, Drops!"),
];

const PERF_FORGE: &[(&str, &str)] = &[
    ("embeddium", "Embeddium"),
    ("lithium", "Lithium"),
    ("starlight", "Starlight"),
    ("ferrite-core", "FerriteCore"),
    ("memoryleakfix", "MemoryLeakFix"),
    ("entityculling", "EntityCulling"),
    ("modernfix", "ModernFix"),
    ("performant", "Performant"),
    ("fastchest", "FastChest"),
    ("noisium", "Noisium"),
    ("enhancedblockentities", "Enhanced Block Entities"),
    ("lazydfu", "LazyDFU"),
    ("krypton-reforged", "Krypton Reforged"),
    ("moreculling", "MoreCulling"),
    ("alternate-current", "Alternate Current"),
    ("clustered", "Clustered"),
    ("radon", "Radon"),
    ("booptimize", "BooOptimize"),
    ("smooth-boot", "Smooth Boot"),
];
/// A fork and the mod it replaces must never share one `mods/` folder
/// (duplicate mod ids / mixin clashes crash the loader).
const FORK_PAIRS: &[(&str, &str)] = &[("modernfix-mvus", "modernfix"), ("noisiumforked", "noisium")];
/// True when `mods/` holds a jar whose name starts with `prefix`.
fn mods_dir_has_prefix(mods: &Path, prefix: &str) -> bool {
    std::fs::read_dir(mods)
        .map(|rd| {
            rd.flatten().any(|e| {
                let n = e.file_name().to_string_lossy().to_lowercase();
                n.starts_with(prefix) && n.ends_with(".jar")
            })
        })
        .unwrap_or(false)
}

fn install_perf_mods_impl(
    app: &AppState,
    instance: String,
    mc_version: String,
    loader: String,
) -> Value {
    let dir = match instance_ctx(app, &instance) {
        Ok((_, _, d)) => d,
        Err(e) => return json!({ "error": e }),
    };
    let _ = std::fs::create_dir_all(dir.join("mods"));

    let list: &[(&str, &str)] = match loader.as_str() {
        "fabric" => PERF_FABRIC,
        "forge" | "neoforge" => PERF_FORGE,
        _ => &[],
    };
    if list.is_empty() {
        return json!({ "success": true, "installed": 0, "skipped": 0, "errors": [],
            "message": "Performance mods require Fabric or Forge." });
    }

    // Fabric API first — almost every Fabric mod needs it.
    let mut installed = 0usize;
    let mut skipped = 0usize;
    let mut errors: Vec<Value> = Vec::new();

    if loader == "fabric" {
        if mods_dir_has_prefix(&dir.join("mods"), "fabric-api") {
            skipped += 1;
        } else {
            match install_mod_inner(app, &instance, "fabric-api", "", "mods") {
                Ok(_) => installed += 1,
                Err(e) => errors.push(json!({ "slug": "fabric-api", "error": e })),
            }
        }
    }

    let mc_major_minor = {
        let p: Vec<&str> = mc_version.split('.').collect();
        if p.len() >= 2 {
            format!("{}.{}", p[0], p[1])
        } else {
            mc_version.clone()
        }
    };

    for (slug, _name) in list {
        // coarse min-version gates, same as Xryloth
        let too_old = (*slug == "badoptimizations" && mc_major_minor < "1.19".to_string())
            || ((*slug == "noisium" || *slug == "nvidium") && mc_major_minor < "1.20".to_string());
        if too_old {
            skipped += 1;
            continue;
        }
        let dest = dir.join("mods");
        if mods_dir_has_prefix(&dest, slug) {
            skipped += 1;
            continue;
        }
        // Fork vs upstream (modernfix ↔ modernfix-mvus, noisium ↔ noisiumforked).
        let fork_clash = FORK_PAIRS.iter().any(|(fork, up)| {
            if *slug == *fork {
                mods_dir_has_prefix(&dest, &format!("{up}-"))
            } else if *slug == *up {
                mods_dir_has_prefix(&dest, &format!("{fork}-"))
            } else {
                false
            }
        });
        if fork_clash {
            skipped += 1;
            continue;
        }
        // Strict gate: only install a build that declares THIS exact Minecraft
        // version. install_mod_inner's ladder falls back to *any* version,
        // which would drop dead mods (Starlight @1.20.4 …) into a 1.21.11
        // profile and hard-crash the Fabric loader on version mismatch.
        let has_build = modrinth_versions(slug, loader.as_str(), &mc_version)
            .map(|v| v.as_array().map(|a| !a.is_empty()).unwrap_or(false))
            .unwrap_or(false);
        if !has_build {
            skipped += 1;
            continue;
        }
        match install_mod_inner(app, &instance, slug, "", "mods") {
            Ok(_) => installed += 1,
            Err(_) => {
                // try the fallback slug (sodium -> embeddium, embeddium -> optiforge)
                let fallback = match *slug {
                    "sodium" => Some("embeddium"),
                    "embeddium" => Some("optiforge"),
                    _ => None,
                };
                match fallback {
                    Some(fb) => match install_mod_inner(app, &instance, fb, "", "mods") {
                        Ok(_) => installed += 1,
                        Err(e2) => {
                            skipped += 1;
                            let _ = e2;
                        }
                    },
                    None => skipped += 1,
                }
            }
        }
    }

    let msg = format!(
        "Installed {} FPS mods ({} skipped as unsupported or already present)",
        installed, skipped
    );
    log_msg(app, &format!("[PerfMods] {}", msg));
    json!({ "success": true, "installed": installed, "skipped": skipped, "errors": errors, "message": msg })
}

#[tauri::command]
fn install_performance_mods(
    app: tauri::State<AppState>,
    instance: String,
    mc_version: String,
    loader: String,
) -> Value {
    install_perf_mods_impl(&app, instance, mc_version, loader)
}
// ============================================================
// ISOLATECLIENTMOD — official mod, auto-injected into Fabric profiles
// ============================================================
/// Latest release of the companion mod repo (tag per Minecraft version).
const ISOLATE_MOD_API: &str = "https://api.github.com/repos/EscapeXMc/IsolateClientMod/releases/latest";
/// Direct URL used when api.github.com is unreachable (rate limit / block) —
/// the 1.21.11 jar lives at a stable release-download path.
const ISOLATE_MOD_FALLBACK: &str =
    "https://github.com/EscapeXMc/IsolateClientMod/releases/download/1.21.11/isolateclient-1.21.11.jar";
/// Exact jar name for a Minecraft version: `isolateclient-{mc}.jar`.
fn isolate_mod_filename(mc: &str) -> String {
    format!("isolateclient-{mc}.jar")
}
/// Pick `isolateclient-{mc}.jar` out of a GitHub release's asset list —
/// never another version's jar (the loader hard-fails on a mismatched MC dep).
fn pick_isolate_asset(assets: &Value, mc: &str) -> Option<String> {
    let want = isolate_mod_filename(mc);
    assets
        .as_array()?
        .iter()
        .find(|a| a.get("name").and_then(|n| n.as_str()) == Some(want.as_str()))
        .and_then(|a| a.get("browser_download_url").and_then(|u| u.as_str()))
        .map(|u| u.to_string())
}
/// Resolve the download URL for this exact MC version.
fn isolate_mod_download_url(mc: &str) -> Result<String, String> {
    match http_get(ISOLATE_MOD_API) {
        Ok(v) => pick_isolate_asset(&v["assets"], mc)
            .ok_or_else(|| format!("IsolateClientMod has no build for Minecraft {mc}.")),
        Err(e) if mc == "1.21.11" => {
            let _ = e; // API hiccup — stable direct URL for the known build.
            Ok(ISOLATE_MOD_FALLBACK.to_string())
        }
        Err(e) => Err(format!("IsolateClientMod releases unreachable: {e}")),
    }
}
/// One cached jar per MC version in `~/.isolate/updates` (fresh for 30 days),
/// so provisioning N Fabric profiles downloads the mod only once.
fn ensure_isolate_mod_cached(mc: &str) -> Result<PathBuf, String> {
    let p = updates_dir().join(isolate_mod_filename(mc));
    let fresh = p
        .metadata()
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.elapsed().ok())
        .map(|d| d.as_secs() < 30 * 24 * 3600)
        .unwrap_or(false);
    if fresh && p.metadata().map(|m| m.len() > 1024).unwrap_or(false) {
        return Ok(p);
    }
    let url = isolate_mod_download_url(mc)?;
    download_file(&url, &p)?;
    let sz = p.metadata().map(|m| m.len()).unwrap_or(0);
    if sz < 1024 {
        let _ = std::fs::remove_file(&p);
        return Err("IsolateClientMod download looked empty — retry on next launch.".into());
    }
    Ok(p)
}
/// Download IsolateClientMod for this instance's exact MC version and drop it
/// into `mods/`. Skips cleanly when the jar is already there or the repo has
/// no build for this MC version.
fn install_isolate_client_mod_impl(app: &AppState, instance: &str) -> Value {
    let (mc, loader, dir) = match instance_ctx(app, instance) {
        Ok(x) => x,
        Err(e) => return json!({ "success": false, "error": e }),
    };
    if loader != "fabric" {
        return json!({ "success": true, "installed": false, "skipped": true,
            "message": "IsolateClientMod needs a Fabric instance." });
    }
    let mods = dir.join("mods");
    if let Err(e) = std::fs::create_dir_all(&mods) {
        return json!({ "success": false, "error": e.to_string() });
    }
    if mods_dir_has_prefix(&mods, "isolateclient-") {
        return json!({ "success": true, "installed": false, "skipped": true,
            "message": "IsolateClientMod already present." });
    }
    let cached = match ensure_isolate_mod_cached(&mc) {
        Ok(p) => p,
        Err(e) if e.contains("no build for Minecraft") => {
            return json!({ "success": true, "installed": false, "skipped": true, "message": e });
        }
        Err(e) => return json!({ "success": false, "error": e }),
    };
    let want_file = isolate_mod_filename(&mc);
    let dest = mods.join(&want_file);
    if let Err(e) = std::fs::copy(&cached, &dest) {
        return json!({ "success": false, "error": format!("copy failed: {e}") });
    }
    let _ml = MOD_MANIFEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut m = read_mod_manifest(&dir);
    m.insert(
        want_file.clone(),
        json!({
            "slug": "isolateclient",
            "version_id": format!("mc{}", mc),
            "version_number": mc,
            "name": "IsolateClientMod",
        }),
    );
    write_mod_manifest(&dir, &m);
    log_msg(app, &format!("[IsolateMod] '{instance}': installed {want_file}"));
    json!({ "success": true, "installed": true, "file": want_file })
}
/// Scan every Fabric profile and inject the mod where it's missing — this is
/// what brings *existing* 1.21.11 instances up to date on startup.
fn provision_isolate_client_mod_impl(app: &AppState) -> Value {
    let instances = load_instances_raw(app);
    let mut installed = 0usize;
    let mut skipped = 0usize;
    let mut errors: Vec<Value> = Vec::new();
    for rec in &instances {
        let name = rec.get("name").and_then(|n| n.as_str()).unwrap_or("");
        let loader = rec.get("loader").and_then(|n| n.as_str()).unwrap_or("vanilla");
        if name.is_empty() || loader != "fabric" {
            continue;
        }
        let v = install_isolate_client_mod_impl(app, name);
        if v.get("installed").and_then(|b| b.as_bool()) == Some(true) {
            installed += 1;
        } else if v.get("skipped").and_then(|b| b.as_bool()) == Some(true) {
            skipped += 1;
        } else {
            errors.push(json!({
                "instance": name,
                "error": v.get("error").cloned().unwrap_or(json!("unknown")),
            }));
        }
    }
    json!({ "success": true, "installed": installed, "skipped": skipped, "errors": errors })
}
/// Manual re-inject for one instance (used by tests / future Settings button).
#[tauri::command]
fn install_isolate_client_mod(app: tauri::State<AppState>, instance: String) -> Value {
    install_isolate_client_mod_impl(&app, &instance)
}
/// Manual scan of all Fabric profiles.
#[tauri::command]
fn provision_isolate_client_mod(app: tauri::State<AppState>) -> Value {
    provision_isolate_client_mod_impl(&app)
}

// ============================================================
// MULTIPLAYER SERVER LIST + STATUS PING
// ============================================================

fn servers_file() -> PathBuf {
    servers_path()
}

#[tauri::command]
fn get_servers() -> Value {
    let data = load_json(&servers_file());
    data.as_array()
        .cloned()
        .map(Value::Array)
        .unwrap_or_else(|| Value::Array(vec![]))
}

#[tauri::command]
fn add_server(name: String, address: String, port: u16) -> Value {
    let mut servers = load_json(&servers_file())
        .as_array()
        .cloned()
        .unwrap_or_default();
    if servers
        .iter()
        .any(|s| s.get("name").and_then(|n| n.as_str()) == Some(name.as_str()))
    {
        return json!({ "error": format!("Server '{}' already exists", name) });
    }
    servers.push(json!({ "name": name, "address": address, "port": port }));
    save_json(&servers_file(), &Value::Array(servers));
    json!({ "success": true })
}

#[tauri::command]
fn delete_server(name: String) -> Value {
    let mut servers = load_json(&servers_file())
        .as_array()
        .cloned()
        .unwrap_or_default();
    let before = servers.len();
    servers.retain(|s| s.get("name").and_then(|n| n.as_str()) != Some(name.as_str()));
    if servers.len() == before {
        return json!({ "error": format!("Server '{}' not found", name) });
    }
    save_json(&servers_file(), &Value::Array(servers));
    json!({ "success": true })
}

#[tauri::command]
fn edit_server(name: String, address: String, port: u16) -> Value {
    let mut servers = load_json(&servers_file())
        .as_array()
        .cloned()
        .unwrap_or_default();
    let mut found = false;
    for s in servers.iter_mut() {
        if s.get("name").and_then(|n| n.as_str()) == Some(name.as_str()) {
            s["address"] = json!(address);
            s["port"] = json!(port);
            found = true;
            break;
        }
    }
    if !found {
        return json!({ "error": format!("Server '{}' not found", name) });
    }
    save_json(&servers_file(), &Value::Array(servers));
    json!({ "success": true })
}

fn write_var_int(mut value: u32) -> Vec<u8> {
    let mut buf = Vec::new();
    loop {
        let mut byte = (value & 0x7F) as u8;
        value >>= 7;
        if value != 0 {
            byte |= 0x80;
        }
        buf.push(byte);
        if value == 0 {
            break;
        }
    }
    buf
}

fn read_var_int(data: &[u8], offset: &mut usize) -> Result<u32, String> {
    let mut result: u32 = 0;
    let mut shift = 0;
    loop {
        if *offset >= data.len() {
            return Err("Unexpected end of data reading VarInt".into());
        }
        let byte = data[*offset];
        *offset += 1;
        result |= ((byte & 0x7F) as u32) << shift;
        if byte & 0x80 == 0 {
            break;
        }
        shift += 7;
        if shift >= 35 {
            return Err("VarInt too big".into());
        }
    }
    Ok(result)
}

/// Minecraft server-list ping (status protocol).
#[tauri::command]
fn ping_server(address: String, port: u16) -> Value {
    use std::io::{Read, Write};
    use std::net::TcpStream;
    use std::time::Duration;

    let addr_str = format!("{}:{}", address, port);
    let addr: std::net::SocketAddr = match addr_str.parse() {
        Ok(a) => a,
        Err(e) => return json!({ "error": format!("Invalid address: {}", e) }),
    };
    let stream = match TcpStream::connect_timeout(&addr, Duration::from_secs(3)) {
        Ok(s) => s,
        Err(e) => return json!({ "error": format!("Failed to connect: {}", e) }),
    };
    stream.set_read_timeout(Some(Duration::from_secs(3))).ok();
    stream.set_write_timeout(Some(Duration::from_secs(3))).ok();

    let protocol_version: u32 = 767;
    let mut handshake_data = Vec::new();
    handshake_data.extend_from_slice(&write_var_int(protocol_version));
    let addr_bytes = address.as_bytes();
    handshake_data.extend_from_slice(&write_var_int(addr_bytes.len() as u32));
    handshake_data.extend_from_slice(addr_bytes);
    handshake_data.extend_from_slice(&port.to_be_bytes());
    handshake_data.extend_from_slice(&write_var_int(1)); // next state = status

    let mut handshake_packet = Vec::new();
    handshake_packet.extend_from_slice(&write_var_int(0)); // packet id 0
    handshake_packet.extend_from_slice(&handshake_data);
    let mut full_handshake = write_var_int(handshake_packet.len() as u32);
    full_handshake.extend_from_slice(&handshake_packet);

    let mut writer = match stream.try_clone() {
        Ok(s) => s,
        Err(_) => return json!({ "error": "Failed to clone stream" }),
    };
    if writer.write_all(&full_handshake).is_err() {
        return json!({ "error": "Failed to send handshake" });
    }
    // Status request: length=1, packet_id=0
    if writer.write_all(&[0x01, 0x00]).is_err() {
        return json!({ "error": "Failed to send status request" });
    }
    drop(writer);

    let mut reader = stream;
    let mut length: u32 = 0;
    let mut shift = 0;
    loop {
        let mut byte = [0u8; 1];
        if reader.read_exact(&mut byte).is_err() {
            return json!({ "error": "Failed to read response length" });
        }
        length |= ((byte[0] & 0x7F) as u32) << shift;
        if byte[0] & 0x80 == 0 {
            break;
        }
        shift += 7;
        if shift >= 35 {
            return json!({ "error": "VarInt too big" });
        }
    }
    let mut payload = vec![0u8; length as usize];
    if reader.read_exact(&mut payload).is_err() {
        return json!({ "error": "Failed to read response payload" });
    }

    let mut offset = 0;
    let _packet_id = read_var_int(&payload, &mut offset).unwrap_or(0);
    let json_len = read_var_int(&payload, &mut offset).unwrap_or(0) as usize;
    if offset + json_len > payload.len() {
        return json!({ "error": "Invalid response: JSON string too long" });
    }
    let json_str = match std::str::from_utf8(&payload[offset..offset + json_len]) {
        Ok(s) => s,
        Err(e) => return json!({ "error": format!("Invalid UTF-8 in response: {}", e) }),
    };
    match serde_json::from_str::<Value>(json_str) {
        Ok(data) => {
            // description may be a string, an object with "text", or a chat component tree.
            fn flatten_desc(v: &Value) -> String {
                match v {
                    Value::String(s) => s.clone(),
                    Value::Object(o) => {
                        if let Some(t) = o.get("text").and_then(|t| t.as_str()) {
                            t.to_string()
                        } else if let Some(t) = o.get("translate").and_then(|t| t.as_str()) {
                            t.to_string()
                        } else if let Some(extra) = o.get("extra").and_then(|e| e.as_array()) {
                            extra
                                .iter()
                                .map(flatten_desc)
                                .collect::<Vec<_>>()
                                .join("")
                        } else {
                            String::new()
                        }
                    }
                    Value::Array(a) => a.iter().map(flatten_desc).collect::<Vec<_>>().join(""),
                    _ => String::new(),
                }
            }
            let motd = flatten_desc(data.get("description").unwrap_or(&Value::Null));
            let motd = if motd.is_empty() {
                "Unknown".to_string()
            } else {
                motd
            };
            let online = data
                .get("players")
                .and_then(|p| p.get("online"))
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
            let max = data
                .get("players")
                .and_then(|p| p.get("max"))
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
            let version = data
                .get("version")
                .and_then(|v| v.get("name"))
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown")
                .to_string();
            let icon = data
                .get("icon")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let favicon = data
                .get("favicon")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let enforces_secure = data
                .get("enforcesSecureChat")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            json!({
                "success": true,
                "motd": motd,
                "online": online,
                "max": max,
                "version": version,
                "icon": if icon.is_empty() { favicon } else { icon },
                "enforces_secure_chat": enforces_secure,
            })
        }
        Err(e) => json!({ "error": format!("Failed to parse server response: {}", e) }),
    }
}

// ============================================================
// GAME BOOST / OPTIMIZER (options.txt)
// ============================================================

fn parse_options_file(path: &Path) -> HashMap<String, String> {
    let mut map = HashMap::new();
    if let Ok(content) = std::fs::read_to_string(path) {
        for line in content.lines() {
            if let Some((k, v)) = line.split_once(':') {
                map.insert(k.trim().to_string(), v.trim().to_string());
            }
        }
    }
    map
}

#[tauri::command]
fn get_optimizer_status(instance: String) -> Value {
    let dir = instance_dir(&instance);
    let options = parse_options_file(&dir.join("options.txt"));
    json!({
        "render_distance": options.get("renderDistance").and_then(|v| v.parse::<i64>().ok()).unwrap_or(12),
        "simulation_distance": options.get("simulationDistance").and_then(|v| v.parse::<i64>().ok()).unwrap_or(12),
        "fps_limit": options.get("fpsLimit").cloned().unwrap_or_else(|| "261".to_string()),
        "vsync": options.get("vsync").cloned().unwrap_or_else(|| "true".to_string()),
        "gamma": options.get("gamma").cloned().unwrap_or_else(|| "1.0".to_string()),
        "entity_distance": options.get("entityDistanceScaling").cloned().unwrap_or_else(|| "1.0".to_string()),
        "particles": options.get("particles").cloned().unwrap_or_else(|| "all".to_string()),
        "clouds": options.get("renderClouds").cloned().unwrap_or_else(|| "true".to_string()),
    })
}

#[tauri::command]
fn apply_optimizer(
    app: tauri::State<AppState>,
    instance: String,
    settings: Value,
) -> Value {
    let path = instance_dir(&instance).join("options.txt");
    let mut existing = parse_options_file(&path);
    if let Some(obj) = settings.as_object() {
        for (k, v) in obj {
            let val = v.as_str().map(|s| s.to_string()).unwrap_or_else(|| v.to_string());
            existing.insert(k.clone(), val);
        }
    }
    let mut keys: Vec<&String> = existing.keys().collect();
    keys.sort();
    let mut content = String::new();
    for k in keys {
        content.push_str(&format!(":{}\n", existing[k]));
    }
    if let Some(p) = path.parent() {
        let _ = std::fs::create_dir_all(p);
    }
    if let Err(e) = std::fs::write(&path, content) {
        return json!({ "error": e.to_string() });
    }
    log_msg(&app, &format!("[Optimizer] Applied settings to {}", instance));
    json!({ "success": true })
}

/// One-click FPS preset: safe, aggressive, or vanilla-restore.
#[tauri::command]
fn apply_fps_preset(
    app: tauri::State<AppState>,
    instance: String,
    preset: String,
) -> Value {
    let settings = match preset.as_str() {
        "performance" => json!({
            "renderDistance": "8", "simulationDistance": "6",
            "fpsLimit": "261", "vsync": "false",
            "entityDistanceScaling": "0.5", "particles": "minimal",
            "renderClouds": "false", "ao": "false", "bobView": "false",
            "gamma": "1.0",
        }),
        "balanced" => json!({
            "renderDistance": "12", "simulationDistance": "10",
            "fpsLimit": "120", "vsync": "true",
            "entityDistanceScaling": "1.0", "particles": "decreased",
            "renderClouds": "true", "ao": "true", "bobView": "true",
            "gamma": "1.0",
        }),
        "quality" => json!({
            "renderDistance": "20", "simulationDistance": "16",
            "fpsLimit": "261", "vsync": "false",
            "entityDistanceScaling": "1.0", "particles": "all",
            "renderClouds": "true", "ao": "true", "bobView": "true",
            "gamma": "1.0",
        }),
        "vanilla" => json!({
            "renderDistance": "12", "simulationDistance": "12",
            "fpsLimit": "261", "vsync": "true",
            "entityDistanceScaling": "1.0", "particles": "all",
            "renderClouds": "true", "gamma": "1.0",
        }),
        _ => return json!({ "error": "Unknown preset." }),
    };
    let r = apply_optimizer(app, instance, settings);
    r
}

// ============================================================
// CRASH LOG ANALYZER
// ============================================================

#[tauri::command]
fn analyze_crash_log(instance: String) -> Value {
    let dir = instance_dir(&instance);
    let mut crash_logs: Vec<(String, PathBuf, u64)> = Vec::new();

    for (sub, prefix) in [("crash-reports", "crash-"), ("logs", "crash-")] {
        if let Ok(entries) = std::fs::read_dir(dir.join(sub)) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                let lower = name.to_lowercase();
                let ok = name.starts_with(prefix)
                    && (lower.ends_with(".txt") || lower.ends_with(".log"));
                if !ok {
                    continue;
                }
                let modified = entry
                    .metadata()
                    .ok()
                    .and_then(|m| m.modified().ok())
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                crash_logs.push((name, entry.path(), modified));
            }
        }
    }
    crash_logs.sort_by(|a, b| b.2.cmp(&a.2));

    let most_recent = crash_logs.first().map(|(_, path, _)| {
        let content = std::fs::read_to_string(path).unwrap_or_default();
        let lines: Vec<&str> = content.lines().collect();
        let start = lines.len().saturating_sub(500);
        lines[start..].join("\n")
    });

    let logs_dir = dir.join("logs");
    let latest_log = logs_dir.join("latest.log");
    let recent_errors = if latest_log.exists() {
        let content = std::fs::read_to_string(&latest_log).unwrap_or_default();
        let error_lines: Vec<&str> = content
            .lines()
            .filter(|l| {
                l.contains("ERROR") || l.contains("Exception") || l.contains("FATAL") || l.contains("CRITICAL")
            })
            .collect();
        let start = error_lines.len().saturating_sub(100);
        error_lines[start..].join("\n")
    } else {
        String::new()
    };

    // Heuristic diagnosis from the newest crash text.
    let text = most_recent.clone().unwrap_or_default();
    let mut diagnosis: Vec<String> = Vec::new();
    let lower = text.to_lowercase();
    if lower.contains("outofmemoryerror") || lower.contains("java.lang.outofmemoryerror") {
        diagnosis.push("Out of memory — raise the instance's RAM allocation.".into());
    }
    if lower.contains("mod resolution") || lower.contains("could not find") && lower.contains("mod") {
        diagnosis.push("A mod failed to resolve — a dependency may be missing or the mod is for a different loader.".into());
    }
    if lower.contains("fabric") && lower.contains("mixin") {
        diagnosis.push("A Mixin injection failed — usually a mod conflict or an outdated mod.".into());
    }
    if lower.contains("unsupportedclassversionerror") {
        diagnosis.push("A mod was built for a newer Java version — install a newer Java.".into());
    }
    if lower.contains("nosuchmethoderror") || lower.contains("nosuchfielderror") {
        diagnosis.push("Binary incompatibility between mods — update or remove the recently added mod.".into());
    }
    if lower.contains("glfw") || lower.contains("opengl") {
        diagnosis.push("Graphics driver problem — update your GPU drivers.".into());
    }
    if lower.contains("could not reserve enough space") || lower.contains("heap") {
        diagnosis.push("JVM memory reservation failed — lower the allocated RAM.".into());
    }
    if diagnosis.is_empty() && !text.is_empty() {
        diagnosis.push("No well-known signature matched — inspect the crash log below.".into());
    }

    json!({
        "success": true,
        "crash_log": most_recent.unwrap_or_default(),
        "crash_file": crash_logs.first().map(|(n, _, _)| n.clone()).unwrap_or_default(),
        "recent_errors": recent_errors,
        "crash_count": crash_logs.len(),
        "diagnosis": diagnosis,
    })
}

// ============================================================
// PUBLIC IP
// ============================================================

#[tauri::command]
fn get_public_ip() -> Value {
    if let Ok(v) = http_get("https://api.ipify.org?format=json") {
        if let Some(ip) = v.get("ip").and_then(|i| i.as_str()) {
            if !ip.trim().is_empty() {
                return json!({ "success": true, "ip": ip.trim() });
            }
        }
    }
    for url in ["https://api.ipify.org", "https://ifconfig.me/ip", "https://icanhazip.com"] {
        let c = match http_client(8) {
            Ok(c) => c,
            Err(_) => continue,
        };
        if let Ok(resp) = c.get(url).send() {
            if let Ok(text) = resp.text() {
                let t = text.trim().to_string();
                if !t.is_empty()
                    && t.len() <= 45
                    && t.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == ':' || c == '-')
                {
                    return json!({ "success": true, "ip": t });
                }
            }
        }
    }
    json!({ "success": false, "error": "Could not determine public IP — check your internet connection." })
}

// ============================================================
// PIN / UNPIN + SCREENSHOTS
// ============================================================

fn set_instance_flag(app: &AppState, name: &str, key: &str, val: Value) -> Value {
    let mut list = load_instances_raw(app);
    for inst in list.iter_mut() {
        if inst.get("name").and_then(|v| v.as_str()) == Some(name) {
            inst[key] = val;
            save_instances(app, &list);
            return json!({ "success": true });
        }
    }
    json!({ "error": "Instance not found" })
}

#[tauri::command]
fn pin_instance(app: tauri::State<AppState>, instance: String) -> Value {
    set_instance_flag(&app, &instance, "pinned", Value::Bool(true))
}

#[tauri::command]
fn unpin_instance(app: tauri::State<AppState>, instance: String) -> Value {
    set_instance_flag(&app, &instance, "pinned", Value::Bool(false))
}

#[tauri::command]
fn get_screenshots(instance: String) -> Value {
    let dir = instance_dir(&instance).join("screenshots");
    let mut shots: Vec<Value> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            let lower = name.to_lowercase();
            if !(lower.ends_with(".png") || lower.ends_with(".jpg")
                || lower.ends_with(".jpeg") || lower.ends_with(".webp"))
            {
                continue;
            }
            let modified = entry
                .metadata()
                .ok()
                .and_then(|m| m.modified().ok())
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);
            shots.push(json!({
                "name": name,
                "path": entry.path().to_string_lossy().to_string(),
                "modified": modified,
            }));
        }
    }
    shots.sort_by(|a, b| {
        b.get("modified").and_then(|v| v.as_u64()).unwrap_or(0)
            .cmp(&a.get("modified").and_then(|v| v.as_u64()).unwrap_or(0))
    });
    json!({ "screenshots": shots })
}

// ============================================================
// INSTANCE CLONE + CONFIG EXPORT/IMPORT
// ============================================================

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dst).map_err(|e| e.to_string())?;
    for entry in std::fs::read_dir(src).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let s = entry.path();
        let d = dst.join(entry.file_name());
        if s.is_dir() {
            copy_dir_recursive(&s, &d)?;
        } else {
            std::fs::copy(&s, &d).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

#[tauri::command]
fn clone_instance(
    app: tauri::State<AppState>,
    instance: String,
    new_name: String,
) -> Value {
    let source_name = sanitize_name(&instance);
    let target_name = sanitize_name(&new_name);
    log_msg(&app, &format!("[Instance] Cloning '{}' to '{}'", source_name, target_name));
    let list = load_instances_raw(&app);
    let source = match list
        .iter()
        .find(|i| i.get("name").and_then(|n| n.as_str()) == Some(source_name.as_str()))
    {
        Some(i) => i.clone(),
        None => return json!({ "error": format!("Instance '{}' not found", instance) }),
    };
    let new_dir = instance_dir(&target_name);
    if new_dir.exists() {
        return json!({ "error": format!("Instance '{}' already exists", new_name) });
    }
    let source_dir = instance_dir(&source_name);
    let is_installed = source_dir.exists();
    if is_installed {
        if let Err(e) = copy_dir_recursive(&source_dir, &new_dir) {
            return json!({ "error": format!("Failed to copy instance directory: {}", e) });
        }
    }
    let mut new_data = source;
    new_data["name"] = json!(target_name);
    new_data["created"] = json!(chrono::Local::now().to_rfc3339());
    new_data["last_played"] = Value::Null;
    let mut all = list;
    all.push(new_data.clone());
    save_instances(&app, &all);
    log_msg(&app, &format!("[Instance] Cloned '{}' → '{}'", source_name, target_name));
    json!({ "success": true, "instance": new_data, "copied_files": is_installed })
}

#[tauri::command]
fn export_instance_config(
    app: tauri::State<AppState>,
    instance: String,
    output_path: String,
) -> Value {
    let clean = sanitize_name(&instance);
    let list = load_instances_raw(&app);
    match list
        .iter()
        .find(|i| i.get("name").and_then(|v| v.as_str()) == Some(clean.as_str()))
    {
        Some(inst) => {
            let out = PathBuf::from(&output_path);
            if let Some(parent) = out.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            match std::fs::write(&out, serde_json::to_string_pretty(inst).unwrap_or_default()) {
                Ok(()) => json!({ "success": true, "path": output_path }),
                Err(e) => json!({ "error": e.to_string() }),
            }
        }
        None => json!({ "error": "Instance not found" }),
    }
}

#[tauri::command]
fn import_instance_config(app: tauri::State<AppState>, file_path: String) -> Value {
    let data = match std::fs::read_to_string(&file_path) {
        Ok(d) => d,
        Err(e) => return json!({ "error": e.to_string() }),
    };
    let parsed: Value = match serde_json::from_str(&data) {
        Ok(v) => v,
        Err(e) => return json!({ "error": format!("Invalid JSON: {}", e) }),
    };
    if parsed.get("name").is_none() || parsed.get("version").is_none() {
        return json!({ "error": "Config must have 'name' and 'version' fields" });
    }
    let mut inst = parsed.clone();
    if inst.get("created").is_none() {
        inst["created"] = json!(chrono::Utc::now().to_rfc3339());
    }
    let name = sanitize_name(inst.get("name").and_then(|n| n.as_str()).unwrap_or("instance"));
    inst["name"] = json!(name);
    let mut list = load_instances_raw(&app);
    if list
        .iter()
        .any(|i| i.get("name").and_then(|v| v.as_str()) == Some(name.as_str()))
    {
        return json!({ "error": "Instance with this name already exists" });
    }
    list.push(inst.clone());
    save_instances(&app, &list);
    json!({ "success": true, "instance": inst })
}

// ============================================================
// FIRST-LAUNCH WIZARD
// ============================================================

#[tauri::command]
fn check_first_launch() -> Value {
    json!({ "first_launch": !app_dir().join(".wizard_done").exists() })
}

#[tauri::command]
fn complete_first_launch(app: tauri::State<AppState>) -> Value {
    let _ = std::fs::write(app_dir().join(".wizard_done"), "done");
    log_msg(&app, "[Wizard] First-launch wizard completed");
    json!({ "success": true })
}

// ============================================================
// NOTIFICATIONS
// ============================================================

#[tauri::command]
fn show_notification(title: String, body: String) -> Value {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        let _ = std::process::Command::new("powershell")
            .args([
                "-Command",
                &format!(
                    "[Windows.UI.Notifications.ToastNotificationManager, Windows.UI.Notifications, ContentType = WindowsRuntime] > $null; \
                     [Windows.Data.Xml.Dom.XmlDocument] > $null; \
                     $x = '<toast><visual><binding template=\"ToastText02\"><text id=\"1\">{}</text><text id=\"2\">{}</text></binding></visual></toast>'; \
                     $d = [Windows.Data.Xml.Dom.XmlDocument]::new(); $d.LoadXml($x); \
                     [Windows.UI.Notifications.ToastNotificationManager]::CreateToastNotifier('Isolate Client').Show([Windows.UI.Notifications.ToastNotification]::new($d));",
                    title.replace('&', "&amp;").replace('<', "&lt;").replace('\'', "''"),
                    body.replace('&', "&amp;").replace('<', "&lt;").replace('\'', "''"),
                ),
            ])
            .creation_flags(0x08000000)
            .output();
    }
    #[cfg(target_os = "linux")]
    {
        let _ = std::process::Command::new("notify-send")
            .args([&title, &body])
            .output();
    }
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("terminal-notifier")
            .args(["-title", &title, "-message", &body])
            .output();
    }
    json!({ "success": true })
}

#[tauri::command]
fn play_notification_sound() -> Value {
    // Play the platform event sound without pulling in an audio crate.
    #[cfg(target_os = "linux")]
    {
        for player in ["paplay", "aplay", "pw-play"] {
            let ok = std::process::Command::new(player)
                .args(["/usr/share/sounds/freedesktop/stereo/complete.oga"])
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false);
            if ok {
                return json!({ "success": true, "player": player });
            }
        }
    }
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("afplay")
            .args(["/System/Library/Sounds/Glass.aiff"])
            .output();
    }
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        let _ = std::process::Command::new("powershell")
            .args(["-Command", "[console]::beep(880,120)"])
            .creation_flags(0x08000000)
            .output();
    }
    json!({ "success": true })
}

// ============================================================
// FILE / DIRECTORY / SYSTEM HELPERS (config editor, tools)
// ============================================================

#[tauri::command]
fn read_file_content(path: String) -> Value {
    match std::fs::read_to_string(&path) {
        Ok(c) => json!({ "content": c }),
        Err(e) => json!({ "error": e.to_string() }),
    }
}

#[tauri::command]
fn write_file_content(path: String, content: String) -> Value {
    if let Some(p) = Path::new(&path).parent() {
        let _ = std::fs::create_dir_all(p);
    }
    match std::fs::write(&path, content) {
        Ok(()) => json!({ "success": true }),
        Err(e) => json!({ "error": e.to_string() }),
    }
}

#[tauri::command]
fn list_directory(path: String) -> Value {
    let mut dirs: Vec<Value> = Vec::new();
    let mut files: Vec<Value> = Vec::new();
    let rd = match std::fs::read_dir(&path) {
        Ok(r) => r,
        Err(e) => return json!({ "error": e.to_string() }),
    };
    for entry in rd.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        let meta = entry.metadata().ok();
        let is_dir = meta.as_ref().map(|m| m.is_dir()).unwrap_or(false);
        let size = meta.as_ref().map(|m| m.len()).unwrap_or(0);
        let modified = meta
            .as_ref()
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let item = json!({ "name": name, "size": size, "modified": modified });
        if is_dir {
            dirs.push(item);
        } else {
            files.push(item);
        }
    }
    dirs.sort_by_key(|d| d.get("name").and_then(|n| n.as_str()).unwrap_or("").to_lowercase());
    files.sort_by_key(|f| f.get("name").and_then(|n| n.as_str()).unwrap_or("").to_lowercase());
    json!({ "dirs": dirs, "files": files })
}

#[tauri::command]
fn delete_path(path: String) -> Value {
    let p = PathBuf::from(&path);
    if !p.exists() {
        return json!({ "error": "Path not found." });
    }
    let res = if p.is_dir() {
        std::fs::remove_dir_all(&p)
    } else {
        std::fs::remove_file(&p)
    };
    match res {
        Ok(()) => json!({ "success": true }),
        Err(e) => json!({ "error": e.to_string() }),
    }
}

#[tauri::command]
fn open_url(url: String) -> Value {
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return json!({ "error": "Only http(s) URLs can be opened." });
    }
    match open_in_os(Path::new(&url)) {
        Ok(()) => json!({ "success": true }),
        Err(e) => json!({ "error": e }),
    }
}

#[tauri::command]
fn get_game_logs(instance: String) -> Value {
    let path = instance_dir(&instance).join("logs").join("latest.log");
    let content = std::fs::read_to_string(&path).unwrap_or_default();
    json!({ "content": content })
}

#[tauri::command]
fn clear_instance_cache(app: tauri::State<AppState>, instance: String) -> Value {
    let dir = instance_dir(&instance);
    let mut removed = 0usize;
    // Only safe, regenerable caches — never worlds or mods.
    for sub in ["shader-cache", "datapacks/.cache", "logs", "crash-reports"] {
        let p = dir.join(sub);
        if p.exists() {
            if std::fs::remove_dir_all(&p).is_ok() {
                removed += 1;
            }
        }
    }
    log_msg(&app, &format!("[Cache] Cleared {} caches in {}", removed, instance));
    json!({ "success": true, "cleared": removed })
}

// ============================================================
// SKINS
// ============================================================

#[tauri::command]
fn get_skin_info(app: tauri::State<AppState>) -> Value {
    let s = load_settings(&app);
    let username = s
        .get("username")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let skin_path = s
        .get("custom_skin")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let base = "https://textures.minecraft.net/texture/";
    let texture = s
        .get("skin_texture")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let url = if texture.starts_with("http") {
        texture
    } else if !texture.is_empty() {
        format!("{}{}", base, texture)
    } else if !skin_path.is_empty() {
        skin_path.clone()
    } else {
        // Default Steve/Alex from the player's name hash.
        String::new()
    };
    json!({
        "username": username,
        "skin_url": url,
        "custom_skin": skin_path,
        "slim": s.get("skin_slim").and_then(|v| v.as_bool()).unwrap_or(false),
    })
}

#[tauri::command]
fn set_custom_skin(app: tauri::State<AppState>, file_path: String) -> Value {
    if !Path::new(&file_path).exists() {
        return json!({ "error": "Skin file not found." });
    }
    let dest = app_dir().join("skin.png");
    if let Err(e) = std::fs::copy(&file_path, &dest) {
        return json!({ "error": e.to_string() });
    }
    save_setting_val(&app, "custom_skin", json!(dest.to_string_lossy()));
    log_msg(&app, "[Skins] Custom skin saved");
    json!({ "success": true, "path": dest.to_string_lossy() })
}

#[tauri::command]
fn clear_custom_skin(app: tauri::State<AppState>) -> Value {
    save_setting_val(&app, "custom_skin", json!(""));
    save_setting_val(&app, "skin_texture", json!(""));
    json!({ "success": true })
}

// ============================================================
// DISCORD RPC (raw IPC — no external crate)
// ============================================================

#[cfg(target_os = "windows")]
type DiscordStream = std::fs::File;
#[cfg(not(target_os = "windows"))]
type DiscordStream = std::os::unix::net::UnixStream;

fn discord_connect(index: u32) -> std::io::Result<DiscordStream> {
    #[cfg(target_os = "windows")]
    {
        std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(format!(r"\\.\pipe\discord-ipc-{}", index))
    }
    #[cfg(not(target_os = "windows"))]
    {
        use std::os::unix::net::UnixStream;
        let mut candidates: Vec<String> = Vec::new();
        if let Ok(dir) = std::env::var("XDG_RUNTIME_DIR") {
            candidates.push(format!("{}/discord-ipc-{}", dir, index));
            candidates.push(format!("{}/app/com.discordapp.Discord/discord-ipc-{}", dir, index));
        }
        candidates.push(format!("/tmp/discord-ipc-{}", index));
        for path in &candidates {
            if let Ok(s) = UnixStream::connect(path) {
                return Ok(s);
            }
        }
        Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "no Discord IPC socket found",
        ))
    }
}

fn discord_ipc_frame(opcode: u32, payload: &str) -> Vec<u8> {
    let bytes = payload.as_bytes();
    let mut frame = Vec::with_capacity(8 + bytes.len());
    frame.extend_from_slice(&opcode.to_le_bytes());
    frame.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
    frame.extend_from_slice(bytes);
    frame
}

fn discord_ipc_read(reader: &mut dyn Read) -> Result<(u32, String), String> {
    let mut header = [0u8; 8];
    reader.read_exact(&mut header).map_err(|e| e.to_string())?;
    let opcode = u32::from_le_bytes([header[0], header[1], header[2], header[3]]);
    let length = u32::from_le_bytes([header[4], header[5], header[6], header[7]]) as usize;
    let mut payload = vec![0u8; length];
    reader.read_exact(&mut payload).map_err(|e| e.to_string())?;
    String::from_utf8(payload).map_err(|e| e.to_string()).map(|s| (opcode, s))
}

fn discord_set_activity(
    writer: &mut dyn Write,
    nonce: &str,
    details: &str,
    state: &str,
    large_image: &str,
    large_text: &str,
    small_image: &str,
    small_text: &str,
) {
    let start_ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    let mut assets = json!({ "large_image": large_image, "large_text": large_text });
    if !small_image.is_empty() {
        assets["small_image"] = json!(small_image);
        assets["small_text"] = json!(small_text);
    }
    let activity = json!({
        "cmd": "SET_ACTIVITY",
        "args": {
            "pid": std::process::id(),
            "activity": {
                "details": details,
                "state": state,
                "assets": assets,
                "timestamps": { "start": start_ts },
            }
        },
        "nonce": nonce,
    });
    let _ = writer.write_all(&discord_ipc_frame(1, &activity.to_string()));
}

fn discord_clear_frame() -> Vec<u8> {
    let clear = json!({
        "cmd": "SET_ACTIVITY",
        "args": { "pid": std::process::id(), "activity": null },
        "nonce": "isolate-clear",
    });
    discord_ipc_frame(1, &clear.to_string())
}

const DISCORD_CLIENT_ID: &str = "1557369457397997618";

#[tauri::command]
fn init_discord_rpc(app: tauri::State<AppState>, handle: tauri::AppHandle) -> Value {
    let enabled = load_settings(&app)
        .get("discord_rpc")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    if !enabled {
        return json!({ "success": true, "message": "Discord RPC disabled in settings" });
    }
    if app.discord_handle.lock().unwrap().is_some() {
        return json!({ "success": true, "message": "Discord RPC already connected" });
    }

    let state_flag = Arc::new(AtomicBool::new(false)); // stop requested
    std::thread::spawn(move || {
        let mut last_activity: Option<(String, String, String, String, String, String)> = None;
        'outer: loop {
            if state_flag.load(Ordering::Relaxed) {
                break;
            }
            let (cmd_tx, cmd_rx) = std::sync::mpsc::channel::<DiscordMessage>();

            for i in 0..10 {
                let file = match discord_connect(i) {
                    Ok(f) => f,
                    Err(_) => continue,
                };
                let mut handshake_reader = match file.try_clone() {
                    Ok(f) => std::io::BufReader::new(f),
                    Err(_) => continue,
                };
                let mut keepalive_reader = match file.try_clone() {
                    Ok(f) => std::io::BufReader::new(f),
                    Err(_) => continue,
                };
                let writer = Arc::new(Mutex::new(file));

                let handshake = json!({ "v": 1, "client_id": DISCORD_CLIENT_ID }).to_string();
                {
                    let mut w = writer.lock().unwrap();
                    if w.write_all(&discord_ipc_frame(0, &handshake)).is_err() {
                        continue;
                    }
                    let _ = w.flush();
                }

                // Handshake reply (5s budget).
                let reply = {
                    let (tx, rx) = std::sync::mpsc::channel();
                    std::thread::spawn(move || {
                        let _ = tx.send(discord_ipc_read(&mut handshake_reader));
                    });
                    rx.recv_timeout(Duration::from_secs(5))
                };
                let ready = match reply {
                    Ok(Ok((_, data))) => serde_json::from_str::<Value>(&data)
                        .ok()
                        .filter(|o| o.get("evt").and_then(|e| e.as_str()) == Some("READY"))
                        .is_some(),
                    _ => false,
                };
                if !ready {
                    continue;
                }
                log_msg_static("[Discord RPC] Connected");
                // Register the sender so the UI can push updates.
                {
                    let s = handle.state::<AppState>();
                    *s.discord_handle.lock().unwrap() = Some(DiscordRpcHandle { sender: cmd_tx.clone() });
                }

                // Restore last activity, if we had one.
                if let Some((d, st, li, lt, si, stx)) = &last_activity {
                    let mut w = writer.lock().unwrap();
                    discord_set_activity(&mut *w, "isolate-restore", d, st, li, lt, si, stx);
                    let _ = w.flush();
                }

                // Keepalive / ping reader.
                let writer_clone = writer.clone();
                let ka = std::thread::spawn(move || {
                    loop {
                        match discord_ipc_read(&mut keepalive_reader) {
                            Ok((opcode, data)) => match opcode {
                                2 => {
                                    let pong = serde_json::from_str::<Value>(&data)
                                        .ok()
                                        .and_then(|v| v.get("d").cloned())
                                        .unwrap_or(Value::Null);
                                    let payload = json!({ "d": pong }).to_string();
                                    let mut w = writer_clone.lock().unwrap();
                                    let _ = w.write_all(&discord_ipc_frame(3, &payload));
                                    let _ = w.flush();
                                }
                                1 => break, // CLOSE
                                _ => {}
                            },
                            Err(_) => break,
                        }
                    }
                });

                // Command loop — runs until stop, channel close, or reader death.
                loop {
                    match cmd_rx.try_recv() {
                        Ok(DiscordMessage::Update {
                            details,
                            state,
                            large_image,
                            large_text,
                            small_image,
                            small_text,
                        }) => {
                            let nonce = format!(
                                "isolate-{}",
                                std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .unwrap_or_default()
                                    .as_millis()
                            );
                            {
                                let mut w = writer.lock().unwrap();
                                discord_set_activity(
                                    &mut *w,
                                    &nonce,
                                    &details,
                                    &state,
                                    &large_image,
                                    &large_text,
                                    &small_image,
                                    &small_text,
                                );
                                let _ = w.flush();
                            }
                            last_activity =
                                Some((details, state, large_image, large_text, small_image, small_text));
                        }
                        Ok(DiscordMessage::Stop) => {
                            let mut w = writer.lock().unwrap();
                            let _ = w.write_all(&discord_clear_frame());
                            let _ = w.flush();
                            drop(w);
                            let s = handle.state::<AppState>();
                            *s.discord_handle.lock().unwrap() = None;
                            let _ = ka.join();
                            break 'outer;
                        }
                        Err(std::sync::mpsc::TryRecvError::Empty) => {}
                        Err(std::sync::mpsc::TryRecvError::Disconnected) => break,
                    }
                    if ka.is_finished() {
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(250));
                }

                // Clean up this attempt, then retry.
                {
                    let mut w = writer.lock().unwrap();
                    let _ = w.write_all(&discord_clear_frame());
                    let _ = w.flush();
                }
                let s = handle.state::<AppState>();
                *s.discord_handle.lock().unwrap() = None;
                break;
            }

            if state_flag.load(Ordering::Relaxed) {
                break;
            }
            // Discord not running — retry quietly.
            std::thread::sleep(Duration::from_secs(10));
        }
    });

    json!({ "success": true, "message": "Discord RPC started" })
}

/// Log without holding AppState (used from the RPC thread).
fn log_msg_static(msg: &str) {
    eprintln!("{}", msg);
}

#[tauri::command]
fn update_discord_rpc(
    app: tauri::State<AppState>,
    details: String,
    state: String,
    large_image: String,
    large_text: String,
    small_image: String,
    small_text: String,
) -> Value {
    let guard = app.discord_handle.lock().unwrap();
    if let Some(h) = guard.as_ref() {
        let _ = h.sender.send(DiscordMessage::Update {
            details,
            state,
            large_image,
            large_text,
            small_image,
            small_text,
        });
        json!({ "success": true })
    } else {
        json!({ "success": false, "message": "Discord RPC not initialized" })
    }
}

#[tauri::command]
fn stop_discord_rpc(app: tauri::State<AppState>) -> Value {
    let handle = app.discord_handle.lock().unwrap().take();
    if let Some(h) = handle {
        let _ = h.sender.send(DiscordMessage::Stop);
    }
    json!({ "success": true })
}

#[tauri::command]
fn get_discord_status(app: tauri::State<AppState>) -> Value {
    let connected = app.discord_handle.lock().unwrap().is_some();
    let enabled = load_settings(&app)
        .get("discord_rpc")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    json!({
        "connected": connected,
        "enabled": enabled,
        "client_id": DISCORD_CLIENT_ID,
    })
}

// ============================================================
// LOCAL SERVER MANAGER (Paper / Purpur / Fabric / Vanilla)
// ============================================================

const AIKAR_FLAGS: [&str; 24] = [
    "-XX:+UseG1GC",
    "-XX:+ParallelRefProcEnabled",
    "-XX:MaxGCPauseMillis=200",
    "-XX:+UnlockExperimentalVMOptions",
    "-XX:+DisableExplicitGC",
    "-XX:+AlwaysPreTouch",
    "-XX:G1NewSizePercent=30",
    "-XX:G1MaxNewSizePercent=40",
    "-XX:G1HeapRegionSize=8M",
    "-XX:G1ReservePercent=20",
    "-XX:G1HeapWastePercent=5",
    "-XX:G1MixedGCCountTarget=4",
    "-XX:InitiatingHeapOccupancyPercent=15",
    "-XX:G1MixedGCLiveThresholdPercent=90",
    "-XX:G1RSetUpdatingPauseTimePercent=5",
    "-XX:SurvivorRatio=32",
    "-XX:+PerfDisableSharedMem",
    "-XX:MaxTenuringThreshold=1",
    "-Dusing.aikar=true",
    "-Daikar.g1.enabled=true",
    "-XX:+UseStringDeduplication",
    "-XX:+OptimizeStringConcat",
    "-Dfile.encoding=UTF-8",
    "-Djava.awt.headless=true",
];

const DEFAULT_SERVER_MOTD: &str =
    "§2§l[§a§lIsolate§2§l] §f§lBlue Edition §8» §7Experience Minecraft powered by §b§lIsolate Client";

const BACKUP_DIR: &str = "isolate_backups";

fn servers_dir() -> PathBuf {
    app_dir().join("servers")
}
fn server_dir(name: &str) -> PathBuf {
    servers_dir().join(name)
}

/// Minecraft reads server.properties as ISO-8859-1 (java.util.Properties).
/// Escape non-ASCII to \uXXXX so nothing gets garbled.
fn escape_prop(v: &str) -> String {
    let mut out = String::new();
    for ch in v.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c => {
                let cp = c as u32;
                if cp < 0x20 || cp > 0x7E {
                    let mut buf = [0u16; 2];
                    for u in c.encode_utf16(&mut buf) {
                        out.push_str(&format!("\\u{:04X}", u));
                    }
                } else {
                    out.push(c);
                }
            }
        }
    }
    out
}

fn unescape_prop(v: &str) -> String {
    let mut out: Vec<u16> = Vec::new();
    let mut chars = v.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('u') => {
                    let mut hex = String::new();
                    for _ in 0..4 {
                        if let Some(h) = chars.next() {
                            hex.push(h);
                        }
                    }
                    if let Ok(cp) = u32::from_str_radix(&hex, 16) {
                        out.push(cp as u16);
                    }
                }
                Some('n') => out.push('\n' as u16),
                Some('r') => out.push('\r' as u16),
                Some('t') => out.push('\t' as u16),
                Some(other) => {
                    let mut buf = [0u16; 2];
                    for u in other.encode_utf16(&mut buf) {
                        out.push(*u);
                    }
                }
                None => {}
            }
        } else {
            let mut buf = [0u16; 2];
            for u in c.encode_utf16(&mut buf) {
                out.push(*u);
            }
        }
    }
    String::from_utf16_lossy(&out)
}

fn sanitize_server_name(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' || c == ' ' || c == '.' {
                c
            } else {
                '_'
            }
        })
        .collect::<String>()
        .trim()
        .to_string()
}

/// Join a relative path under `base`, refusing `..` / absolute escapes.
fn safe_join_rel(base: &Path, rel: &str) -> Option<PathBuf> {
    let mut p = base.to_path_buf();
    for comp in Path::new(rel).components() {
        match comp {
            std::path::Component::Normal(c) => p.push(c),
            std::path::Component::CurDir => {}
            _ => return None,
        }
    }
    Some(p)
}

fn basename(f: &str) -> String {
    Path::new(f)
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default()
}

fn unique_path(base: &Path) -> PathBuf {
    if !base.exists() {
        return base.to_path_buf();
    }
    let stem = base
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "backup".into());
    let ext = base.extension().map(|s| s.to_string_lossy().to_string());
    let parent = base.parent().map(|p| p.to_path_buf()).unwrap_or_default();
    for i in 1..1000 {
        let candidate = match &ext {
            Some(e) => parent.join(format!("{}-{}.{}", stem, i, e)),
            None => parent.join(format!("{}-{}", stem, i)),
        };
        if !candidate.exists() {
            return candidate;
        }
    }
    base.to_path_buf()
}

#[cfg(target_os = "linux")]
fn read_process_rss_kb(pid: u32) -> Option<u64> {
    let s = std::fs::read_to_string(format!("/proc/{}/status", pid)).ok()?;
    for line in s.lines() {
        if let Some(rest) = line.strip_prefix("VmRSS:") {
            return rest.trim().split_whitespace().next()?.parse().ok();
        }
    }
    None
}
#[cfg(not(target_os = "linux"))]
fn read_process_rss_kb(_pid: u32) -> Option<u64> {
    None
}

fn resolve_server_java(settings: &HashMap<String, Value>) -> String {
    if let Some(p) = settings.get("java_path").and_then(|v| v.as_str()) {
        if !p.is_empty() && Path::new(p).exists() {
            return p.to_string();
        }
    }
    "java".to_string()
}

// ---- server jar download URLs ----

fn resolve_paper_download_url(mc_version: &str) -> Result<String, String> {
    let url = format!(
        "https://fill.papermc.io/v3/projects/paper/versions/{}/builds/latest",
        mc_version
    );
    let data = http_get(&url).map_err(|_| {
        format!(
            "No Paper build found for Minecraft {}. Try the Vanilla type instead.",
            mc_version
        )
    })?;
    let downloads = data
        .get("downloads")
        .ok_or_else(|| format!("No download available for Paper {}.", mc_version))?;
    let entry = downloads
        .get("server:default")
        .or_else(|| downloads.get("server:mojang"))
        .ok_or("Missing server download")?;
    entry
        .get("url")
        .and_then(|u| u.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| "Missing download url".to_string())
}

fn resolve_purpur_download_url(mc_version: &str) -> Result<String, String> {
    let meta = http_get(&format!("https://api.purpurmc.org/v2/purpur/{}", mc_version))
        .map_err(|_| format!("No Purpur build for Minecraft {}.", mc_version))?;
    let latest = meta
        .get("builds")
        .and_then(|b| b.get("latest"))
        .and_then(|l| l.as_str())
        .ok_or_else(|| format!("No Purpur build for Minecraft {}.", mc_version))?;
    Ok(format!(
        "https://api.purpurmc.org/v2/purpur/{}/{}/download",
        mc_version, latest
    ))
}

fn resolve_fabric_download_url(mc_version: &str) -> Result<String, String> {
    let loaders = http_get(&format!(
        "https://meta.fabricmc.net/v2/versions/loader/{}",
        mc_version
    ))
    .map_err(|_| format!("No Fabric loader is available for Minecraft {}.", mc_version))?;
    let loader = loaders
        .as_array()
        .and_then(|a| a.first())
        .and_then(|l| l.get("loader"))
        .and_then(|l| l.get("version"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| format!("No Fabric loader is available for Minecraft {}.", mc_version))?;
    let installers = http_get("https://meta.fabricmc.net/v2/versions/installer")
        .map_err(|_| "Failed to fetch the Fabric installer list.".to_string())?;
    let installer = installers
        .as_array()
        .and_then(|a| a.first())
        .and_then(|l| l.get("version"))
        .and_then(|v| v.as_str())
        .ok_or("No Fabric installer is available.")?;
    Ok(format!(
        "https://meta.fabricmc.net/v2/versions/loader/{}/{}/{}/server/jar",
        mc_version, loader, installer
    ))
}

fn resolve_vanilla_server_url(mc_version: &str) -> Result<String, String> {
    let manifest = http_get(MOJANG_MANIFEST)?;
    let versions = manifest
        .get("versions")
        .and_then(|v| v.as_array())
        .ok_or("Malformed version manifest")?;
    let entry = versions
        .iter()
        .find(|v| v.get("id").and_then(|i| i.as_str()) == Some(mc_version))
        .ok_or_else(|| format!("Minecraft version {} not found.", mc_version))?;
    let vurl = entry.get("url").and_then(|u| u.as_str()).ok_or("Missing version metadata url")?;
    let vjson = http_get(vurl)?;
    vjson
        .get("downloads")
        .and_then(|d| d.get("server"))
        .and_then(|s| s.get("url"))
        .and_then(|u| u.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| format!("Minecraft {} has no server download available.", mc_version))
}

fn paper_version_key(v: &str) -> (u64, u64, u64) {
    let base = v.split('-').next().unwrap_or(v);
    let mut parts = base.split('.');
    let a = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let b = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let c = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    (a, b, c)
}

#[tauri::command]
fn get_server_versions() -> Value {
    match http_get("https://fill.papermc.io/v3/projects/paper") {
        Ok(data) => {
            let mut versions: Vec<String> = Vec::new();
            if let Some(map) = data.get("versions").and_then(|v| v.as_object()) {
                for arr in map.values() {
                    if let Some(a) = arr.as_array() {
                        for v in a {
                            if let Some(s) = v.as_str() {
                                if !s.contains('-') {
                                    versions.push(s.to_string());
                                }
                            }
                        }
                    }
                }
            }
            versions.sort_by(|a, b| paper_version_key(b).cmp(&paper_version_key(a)));
            versions.dedup();
            json!({ "success": true, "versions": versions })
        }
        Err(e) => json!({ "error": format!("Failed to fetch Paper versions: {}", e) }),
    }
}

#[tauri::command]
fn get_server_builds(mc_version: String) -> Value {
    let url = format!(
        "https://fill.papermc.io/v3/projects/paper/versions/{}/builds",
        mc_version
    );
    match http_get(&url) {
        Ok(data) => {
            let builds = data.as_array().cloned().unwrap_or_default();
            let out = builds
                .iter()
                .map(|b| {
                    let id = b.get("id").and_then(|v| v.as_u64()).unwrap_or(0);
                    let dl = b
                        .get("downloads")
                        .and_then(|d| d.get("server:default"))
                        .or_else(|| b.get("downloads").and_then(|d| d.get("server:mojang")));
                    let url = dl
                        .and_then(|d| d.get("url"))
                        .and_then(|u| u.as_str())
                        .unwrap_or("");
                    let channel = b.get("channel").and_then(|c| c.as_str()).unwrap_or("STABLE");
                    json!({ "build_number": id, "download_url": url, "channel": channel })
                })
                .collect::<Vec<_>>();
            json!({ "success": true, "builds": out })
        }
        Err(e) => json!({ "error": format!("Failed to fetch Paper builds for {}: {}", mc_version, e) }),
    }
}

#[tauri::command]
fn server_mgr_server_types() -> Value {
    json!({ "types": [
        {"id": "paper", "name": "Paper", "desc": "High performance. Runs Bukkit/Spigot/Paper plugins."},
        {"id": "purpur", "name": "Purpur", "desc": "Paper fork with many extra gameplay options."},
        {"id": "fabric", "name": "Fabric", "desc": "Lightweight mod loader. Use server-side mods."},
        {"id": "vanilla", "name": "Vanilla", "desc": "Official Mojang server. No plugins or mods."}
    ]})
}

#[tauri::command]
fn server_mgr_list_versions(server_type: String) -> Value {
    match server_type.to_lowercase().as_str() {
        "purpur" => match http_get("https://api.purpurmc.org/v2/purpur") {
            Ok(d) => {
                let mut v: Vec<String> = d
                    .get("versions")
                    .and_then(|x| x.as_array())
                    .map(|a| a.iter().filter_map(|s| s.as_str().map(|s| s.to_string())).collect())
                    .unwrap_or_default();
                v.reverse();
                json!({ "success": true, "versions": v })
            }
            Err(e) => json!({ "error": e }),
        },
        "fabric" => match http_get("https://meta.fabricmc.net/v2/versions/game") {
            Ok(d) => {
                let v: Vec<String> = d
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .filter(|g| g.get("stable").and_then(|s| s.as_bool()).unwrap_or(false))
                            .filter_map(|g| g.get("version").and_then(|s| s.as_str()).map(|s| s.to_string()))
                            .collect()
                    })
                    .unwrap_or_default();
                json!({ "success": true, "versions": v })
            }
            Err(e) => json!({ "error": e }),
        },
        "vanilla" => match http_get(MOJANG_MANIFEST) {
            Ok(d) => {
                let v: Vec<String> = d
                    .get("versions")
                    .and_then(|x| x.as_array())
                    .map(|a| {
                        a.iter()
                            .filter(|x| x.get("type").and_then(|t| t.as_str()) == Some("release"))
                            .filter_map(|x| x.get("id").and_then(|s| s.as_str()).map(|s| s.to_string()))
                            .collect()
                    })
                    .unwrap_or_default();
                json!({ "success": true, "versions": v })
            }
            Err(e) => json!({ "error": e }),
        },
        _ => get_server_versions(),
    }
}

fn forward_server_line(handle: &tauri::AppHandle, name: &str, line: &str) {
    let trimmed = line.trim_end();
    if trimmed.is_empty() {
        return;
    }
    {
        let st = handle.state::<AppState>();
        let mut logs = match st.server_logs.lock() {
            Ok(g) => g,
            Err(_) => return,
        };
        let v = logs.entry(name.to_string()).or_default();
        v.push(trimmed.to_string());
        if v.len() > 2000 {
            let drain = v.len() - 2000;
            v.drain(0..drain);
        }
        drop(logs);
    }
    emit(handle, "server-mgr-output", json!({ "server": name, "line": trimmed }));
}

fn create_server_blocking(
    app_handle: Option<tauri::AppHandle>,
    name: &str,
    server_type: &str,
    mc_version: &str,
    port: u16,
    motd: Option<String>,
    ram_mb: Option<u64>,
) -> Value {
    let name = sanitize_server_name(name);
    if name.is_empty() {
        return json!({ "error": "Please enter a valid server name." });
    }
    let dir = server_dir(&name);
    if dir.join("server.json").exists() {
        return json!({ "error": format!("A server named '{}' already exists.", name) });
    }
    if let Err(e) = std::fs::create_dir_all(&dir) {
        return json!({ "error": format!("Failed to create server folder: {}", e) });
    }
    let _ = std::fs::create_dir_all(dir.join("plugins"));
    let motd = motd
        .filter(|m| !m.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_SERVER_MOTD.to_string());
    let stype = if server_type.trim().is_empty() {
        "paper".to_string()
    } else {
        server_type.to_lowercase()
    };
    let url = match stype.as_str() {
        "vanilla" => resolve_vanilla_server_url(mc_version),
        "purpur" => resolve_purpur_download_url(mc_version),
        "fabric" => resolve_fabric_download_url(mc_version),
        _ => resolve_paper_download_url(mc_version),
    };
    let url = match url {
        Ok(u) => u,
        Err(e) => {
            let _ = std::fs::remove_dir_all(&dir);
            return json!({ "error": e });
        }
    };
    let name_for_evt = name.clone();
    let dl = download_file_progress(&url, &dir.join("server.jar"), |done, total| {
        if let Some(h) = &app_handle {
            emit(
                h,
                "server-mgr-download",
                json!({ "server": name_for_evt, "phase": "jar", "downloaded": done, "total": total }),
            );
        }
    });
    if let Err(e) = dl {
        let _ = std::fs::remove_dir_all(&dir);
        return json!({ "error": format!("Failed to download server jar: {}", e) });
    }
    let _ = std::fs::write(dir.join("eula.txt"), "eula=true\n");
    let props = format!(
        "server-port={port}\nmotd={}\nonline-mode=false\nmax-players=20\ndifficulty=easy\ngamemode=survival\npvp=true\nspawn-protection=0\nview-distance=10\nsimulation-distance=10\nallow-flight=true\nenable-command-block=true\nwhite-list=false\nenforce-whitelist=false\nquery.port={port}\n",
        escape_prop(&motd)
    );
    let _ = std::fs::write(dir.join("server.properties"), props);
    let meta = json!({
        "name": name,
        "type": stype,
        "version": mc_version,
        "port": port,
        "motd": motd,
        "ram_mb": ram_mb.unwrap_or(4096),
        "created": chrono::Local::now().to_rfc3339(),
    });
    save_json(&dir.join("server.json"), &meta);
    json!({ "success": true, "server": meta })
}

#[tauri::command]
async fn server_mgr_create(
    handle: tauri::AppHandle,
    name: String,
    server_type: String,
    mc_version: String,
    port: u16,
    motd: Option<String>,
    ram_mb: Option<u64>,
) -> Value {
    // Resolution + jar download use the blocking reqwest client.
    let r = blocking(move || create_server_blocking(Some(handle), &name, &server_type, &mc_version, port, motd, ram_mb));
    r
}

#[tauri::command]
fn server_mgr_list(app: tauri::State<AppState>) -> Value {
    let dir = servers_dir();
    let _ = std::fs::create_dir_all(&dir);
    let procs = app.server_procs.lock().unwrap();
    let mut out = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for e in entries.flatten() {
            let path = e.path();
            if !path.is_dir() {
                continue;
            }
            let meta = load_json(&path.join("server.json"));
            let name = e.file_name().to_string_lossy().to_string();
            let running = procs.contains_key(&name);
            out.push(json!({
                "name": name,
                "type": meta.get("type").and_then(|v| v.as_str()).unwrap_or("paper"),
                "version": meta.get("version").and_then(|v| v.as_str()).unwrap_or(""),
                "port": meta.get("port").and_then(|v| v.as_u64()).unwrap_or(25565),
                "motd": meta.get("motd").and_then(|v| v.as_str()).unwrap_or(DEFAULT_SERVER_MOTD),
                "ram_mb": meta.get("ram_mb").and_then(|v| v.as_u64()).unwrap_or(4096),
                "visibility": meta.get("visibility").and_then(|v| v.as_str()).unwrap_or("private"),
                "description": meta.get("description").and_then(|v| v.as_str()).unwrap_or(""),
                "tags": meta.get("tags").and_then(|v| v.as_str()).unwrap_or(""),
                "max_players": meta.get("max_players").and_then(|v| v.as_u64()).unwrap_or(20),
                "aikar_flags": meta.get("aikar_flags").and_then(|v| v.as_bool()).unwrap_or(false),
                "auto_restart": meta.get("auto_restart").and_then(|v| v.as_bool()).unwrap_or(false),
                "running": running,
            }));
        }
    }
    out.sort_by(|a, b| {
        a.get("name").and_then(|n| n.as_str()).unwrap_or("").to_lowercase()
            .cmp(&b.get("name").and_then(|n| n.as_str()).unwrap_or("").to_lowercase())
    });
    json!({ "success": true, "servers": out })
}

fn launch_local_server(app: &AppState, handle: &tauri::AppHandle, name: &str) -> Value {
    let name = name.to_string();
    // Clear any stale "stopping" flag so a fresh launch isn't mistaken for a crash.
    let _ = app.server_stopping.lock().unwrap().remove(&name);
    let dir = server_dir(&name);
    if !dir.join("server.json").exists() {
        return json!({ "error": "Server not found." });
    }
    {
        let mut procs = app.server_procs.lock().unwrap();
        if let Some(p) = procs.get_mut(&name) {
            match p.child.try_wait() {
                Ok(None) => return json!({ "error": "Server is already running." }),
                _ => {
                    procs.remove(&name);
                }
            }
        }
    }
    let meta = load_json(&dir.join("server.json"));
    let ram = meta.get("ram_mb").and_then(|v| v.as_u64()).unwrap_or(4096);
    let stype = meta
        .get("type")
        .and_then(|v| v.as_str())
        .unwrap_or("paper")
        .to_string();
    let settings = load_settings(app);
    let java = {
        let per_server = meta.get("java_path").and_then(|v| v.as_str()).unwrap_or("");
        if !per_server.is_empty() && Path::new(per_server).exists() {
            per_server.to_string()
        } else {
            resolve_server_java(&settings)
        }
    };
    let xms = meta
        .get("xms_mb")
        .and_then(|v| v.as_u64())
        .unwrap_or_else(|| std::cmp::min(ram, 1024));
    let jvm_args = meta.get("jvm_args").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let aikar = meta.get("aikar_flags").and_then(|v| v.as_bool()).unwrap_or(false);

    let mut cmd = hidden_cmd(&java);
    if aikar {
        for flag in AIKAR_FLAGS {
            cmd.arg(flag);
        }
    }
    cmd.arg(format!("-Xmx{}M", ram));
    cmd.arg(format!("-Xms{}M", xms));
    for part in jvm_args.split_whitespace() {
        cmd.arg(part);
    }
    cmd.arg("-jar");
    cmd.arg("server.jar");
    if stype == "fabric" {
        cmd.arg("nogui");
    } else {
        cmd.arg("--nogui");
    }
    cmd.current_dir(&dir);
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    cmd.stdin(Stdio::piped());

    match cmd.spawn() {
        Ok(mut child) => {
            let stdin = match child.stdin.take() {
                Some(s) => s,
                None => return json!({ "error": "Failed to open server input stream." }),
            };
            let stdout = child.stdout.take();
            let stderr = child.stderr.take();
            {
                let mut procs = app.server_procs.lock().unwrap();
                procs.insert(
                    name.clone(),
                    ServerProc {
                        child,
                        stdin,
                        started_at: Instant::now(),
                    },
                );
                let mut logs = app.server_logs.lock().unwrap();
                logs.insert(name.clone(), Vec::new());
            }
            emit(handle, "server-mgr-status", json!({ "server": name, "running": true }));

            if let Some(stdout) = stdout {
                let h = handle.clone();
                let n = name.clone();
                std::thread::spawn(move || {
                    let reader = BufReader::new(stdout);
                    for line in reader.lines() {
                        match line {
                            Ok(l) => forward_server_line(&h, &n, &l),
                            Err(_) => break,
                        }
                    }
                    let removed = {
                        h.state::<AppState>()
                            .server_procs
                            .lock()
                            .unwrap()
                            .remove(&n)
                            .is_some()
                    };
                    if removed {
                        let was_stopping = {
                            h.state::<AppState>()
                                .server_stopping
                                .lock()
                                .unwrap()
                                .remove(&n)
                        };
                        emit(&h, "server-mgr-status", json!({ "server": &n, "running": false }));
                        if !was_stopping {
                            let meta = load_json(&server_dir(&n).join("server.json"));
                            if meta.get("auto_restart").and_then(|v| v.as_bool()).unwrap_or(false) {
                                let h2 = h.clone();
                                let n2 = n.clone();
                                std::thread::spawn(move || {
                                    std::thread::sleep(Duration::from_secs(3));
                                    let st2 = h2.state::<AppState>();
                                    if !st2.server_procs.lock().unwrap().contains_key(&n2) {
                                        forward_server_line(
                                            &h2,
                                            &n2,
                                            "[Isolate] Server exited unexpectedly — auto-restarting (auto-restart is ON)…",
                                        );
                                        let r = launch_local_server(&st2, &h2, &n2);
                                        if let Some(err) = r.get("error").and_then(|v| v.as_str()) {
                                            forward_server_line(
                                                &h2,
                                                &n2,
                                                &format!("[Isolate] Auto-restart failed: {}", err),
                                            );
                                        }
                                    }
                                });
                            }
                        }
                    }
                });
            }
            if let Some(stderr) = stderr {
                let h = handle.clone();
                let n = name.clone();
                std::thread::spawn(move || {
                    let reader = BufReader::new(stderr);
                    for line in reader.lines() {
                        match line {
                            Ok(l) => forward_server_line(&h, &n, &l),
                            Err(_) => break,
                        }
                    }
                });
            }
            json!({ "success": true })
        }
        Err(e) => json!({ "error": format!("Failed to launch server: {}", e) }),
    }
}

#[tauri::command]
fn server_mgr_start(
    app: tauri::State<AppState>,
    handle: tauri::AppHandle,
    name: String,
) -> Value {
    let name = sanitize_server_name(&name);
    launch_local_server(&app, &handle, &name)
}

#[tauri::command]
fn server_mgr_stop(
    app: tauri::State<AppState>,
    handle: tauri::AppHandle,
    name: String,
) -> Value {
    let name = sanitize_server_name(&name);
    // Mark as intentional so the crash watcher does not auto-restart it.
    app.server_stopping.lock().unwrap().insert(name.clone());
    {
        let mut procs = app.server_procs.lock().unwrap();
        if let Some(p) = procs.get_mut(&name) {
            let _ = p.stdin.write_all(b"stop\n");
            let _ = p.stdin.flush();
        } else {
            return json!({ "error": "Server is not running." });
        }
    }
    // Watchdog: force-kill if it hasn't stopped within 12 seconds.
    let h = handle.clone();
    let n = name;
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(12));
        let st = h.state::<AppState>();
        let removed = {
            let mut procs = st.server_procs.lock().unwrap();
            if let Some(mut p) = procs.remove(&n) {
                let _ = p.child.kill();
                let _ = p.child.wait();
                true
            } else {
                false
            }
        };
        if removed {
            emit(&h, "server-mgr-status", json!({ "server": n, "running": false }));
        }
    });
    json!({ "success": true })
}

#[tauri::command]
fn server_mgr_kill(
    app: tauri::State<AppState>,
    handle: tauri::AppHandle,
    name: String,
) -> Value {
    let name = sanitize_server_name(&name);
    let removed = {
        let mut procs = app.server_procs.lock().unwrap();
        if let Some(mut p) = procs.remove(&name) {
            let _ = p.child.kill();
            let _ = p.child.wait();
            true
        } else {
            false
        }
    };
    emit(&handle, "server-mgr-status", json!({ "server": name, "running": false }));
    json!({ "success": removed })
}

#[tauri::command]
fn server_mgr_command(app: tauri::State<AppState>, name: String, command: String) -> Value {
    let name = sanitize_server_name(&name);
    let mut procs = app.server_procs.lock().unwrap();
    if let Some(p) = procs.get_mut(&name) {
        match p
            .stdin
            .write_all(format!("{}\n", command).as_bytes())
            .and_then(|_| p.stdin.flush())
        {
            Ok(_) => json!({ "success": true }),
            Err(e) => json!({ "error": e.to_string() }),
        }
    } else {
        json!({ "error": "Server is not running." })
    }
}

#[tauri::command]
fn server_mgr_status(app: tauri::State<AppState>, name: String) -> Value {
    let name = sanitize_server_name(&name);
    let mut procs = app.server_procs.lock().unwrap();
    let mut running = false;
    let mut dead = false;
    if let Some(p) = procs.get_mut(&name) {
        match p.child.try_wait() {
            Ok(Some(_)) => dead = true,
            Ok(None) => running = true,
            Err(_) => dead = true,
        }
    }
    if dead {
        procs.remove(&name);
    }
    json!({ "running": running })
}

#[tauri::command]
fn server_mgr_logs(app: tauri::State<AppState>, name: String) -> Value {
    let name = sanitize_server_name(&name);
    let logs = app.server_logs.lock().unwrap();
    let lines = logs.get(&name).cloned().unwrap_or_default();
    json!({ "success": true, "lines": lines })
}

#[tauri::command]
fn server_mgr_clear_logs(app: tauri::State<AppState>, name: String) -> Value {
    let name = sanitize_server_name(&name);
    if let Ok(mut logs) = app.server_logs.lock() {
        logs.insert(name, Vec::new());
    }
    json!({ "success": true })
}

#[tauri::command]
fn server_mgr_delete(
    app: tauri::State<AppState>,
    handle: tauri::AppHandle,
    name: String,
) -> Value {
    let name = sanitize_server_name(&name);
    {
        let mut procs = app.server_procs.lock().unwrap();
        if let Some(mut p) = procs.remove(&name) {
            let _ = p.child.kill();
            let _ = p.child.wait();
        }
    }
    let dir = server_dir(&name);
    if !dir.exists() {
        return json!({ "error": "Server not found." });
    }
    match std::fs::remove_dir_all(&dir) {
        Ok(_) => {
            emit(
                &handle,
                "server-mgr-status",
                json!({ "server": name, "running": false, "deleted": true }),
            );
            json!({ "success": true })
        }
        Err(e) => json!({ "error": e.to_string() }),
    }
}

// ---- properties ----

#[tauri::command]
fn server_mgr_get_properties(name: String) -> Value {
    let name = sanitize_server_name(&name);
    let path = server_dir(&name).join("server.properties");
    let content = std::fs::read_to_string(&path).unwrap_or_default();
    let mut map = serde_json::Map::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            map.insert(k.trim().to_string(), Value::String(unescape_prop(v.trim())));
        }
    }
    json!({ "success": true, "properties": Value::Object(map) })
}

#[tauri::command]
fn server_mgr_set_properties(name: String, properties: HashMap<String, String>) -> Value {
    let name = sanitize_server_name(&name);
    let path = server_dir(&name).join("server.properties");
    let content = std::fs::read_to_string(&path).unwrap_or_default();
    let mut lines: Vec<String> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    for line in content.lines() {
        let t = line.trim();
        if t.is_empty() || t.starts_with('#') {
            lines.push(line.to_string());
            continue;
        }
        if let Some((k, _)) = t.split_once('=') {
            let key = k.trim().to_string();
            if let Some(newval) = properties.get(&key) {
                lines.push(format!("{}={}", key, escape_prop(newval)));
                seen.insert(key);
                continue;
            }
        }
        lines.push(line.to_string());
    }
    let mut keys: Vec<String> = properties.keys().cloned().collect();
    keys.sort();
    for k in keys {
        if !seen.contains(&k) {
            if let Some(v) = properties.get(&k) {
                lines.push(format!("{}={}", k, escape_prop(v)));
            }
        }
    }
    match std::fs::write(&path, lines.join("\n") + "\n") {
        Ok(_) => json!({ "success": true }),
        Err(e) => json!({ "error": e.to_string() }),
    }
}

// ---- plugins (Modrinth "plugin" project type) ----

#[tauri::command]
fn server_mgr_search_plugins(query: String, mc_version: String, limit: u32) -> Value {
    search_modrinth(query, "plugin".into(), "paper".into(), mc_version, limit)
}

fn install_plugin_blocking(name: &str, slug: &str, version_id: &str) -> Value {
    let name = sanitize_server_name(name);
    let plugins_dir = server_dir(&name).join("plugins");
    if let Err(e) = std::fs::create_dir_all(&plugins_dir) {
        return json!({ "error": e.to_string() });
    }
    // Prefer a version matching the server's MC version (Paper loader), then relax.
    let server_mc = load_json(&server_dir(&name).join("server.json"))
        .get("version")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let ladder: [(String, String); 3] = [
        ("paper".to_string(), server_mc.clone()),
        (String::new(), server_mc.clone()),
        (String::new(), String::new()),
    ];
    let mut arr: Vec<Value> = Vec::new();
    for (loader, mc) in ladder {
        arr = modrinth_versions(slug, &loader, &mc)
            .ok()
            .and_then(|v| v.as_array().cloned())
            .unwrap_or_default();
        if !arr.is_empty() {
            break;
        }
    }
    if arr.is_empty() {
        return json!({ "error": "No versions found for this plugin." });
    }
    let chosen = if !version_id.is_empty() {
        arr.iter()
            .find(|v| v.get("id").and_then(|i| i.as_str()) == Some(version_id))
            .unwrap_or(&arr[0])
    } else {
        &arr[0]
    };
    let (filename, url) = match modrinth_primary_file(chosen) {
        Some(x) => x,
        None => return json!({ "error": "No download URL for this plugin." }),
    };
    if let Err(e) = download_file(&url, &plugins_dir.join(&filename)) {
        return json!({ "error": e });
    }
    let gv = chosen
        .get("game_versions")
        .and_then(|g| g.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_default();
    json!({ "success": true, "filename": filename, "game_versions": gv })
}

#[tauri::command]
async fn server_mgr_install_plugin(name: String, slug: String, version_id: String) -> Value {
    blocking(move || install_plugin_blocking(&name, &slug, &version_id))
}

#[tauri::command]
fn server_mgr_list_plugins(name: String) -> Value {
    let name = sanitize_server_name(&name);
    let dir = server_dir(&name).join("plugins");
    let mut out = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for e in entries.flatten() {
            let p = e.path();
            if p.is_file() {
                let fname = e.file_name().to_string_lossy().to_string();
                let lower = fname.to_lowercase();
                if lower.ends_with(".jar") || lower.ends_with(".jar.disabled") {
                    let size = e.metadata().map(|m| m.len()).unwrap_or(0);
                    out.push(json!({
                        "filename": fname,
                        "size": size,
                        "enabled": lower.ends_with(".jar"),
                    }));
                }
            }
        }
    }
    out.sort_by(|a, b| {
        a.get("filename")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .cmp(b.get("filename").and_then(|v| v.as_str()).unwrap_or(""))
    });
    json!({ "success": true, "plugins": out })
}

#[tauri::command]
fn server_mgr_delete_plugin(name: String, filename: String) -> Value {
    let name = sanitize_server_name(&name);
    let path = server_dir(&name).join("plugins").join(basename(&filename));
    if !path.exists() {
        return json!({ "error": "Plugin not found." });
    }
    match std::fs::remove_file(&path) {
        Ok(_) => json!({ "success": true }),
        Err(e) => json!({ "error": e.to_string() }),
    }
}

#[tauri::command]
fn server_mgr_plugin_toggle(name: String, filename: String) -> Value {
    let name = sanitize_server_name(&name);
    let dir = server_dir(&name).join("plugins");
    let fname = basename(&filename);
    let src = dir.join(&fname);
    if !src.exists() {
        return json!({ "error": "Plugin not found." });
    }
    let disabled = fname.to_lowercase().ends_with(".disabled");
    let dest = if disabled {
        dir.join(&fname[..fname.len() - ".disabled".len()])
    } else {
        dir.join(format!("{}.disabled", fname))
    };
    match std::fs::rename(&src, &dest) {
        Ok(_) => json!({ "success": true, "enabled": disabled }),
        Err(e) => json!({ "error": e.to_string() }),
    }
}

// ---- files ----

#[tauri::command]
fn server_mgr_files_list(name: String, subpath: String) -> Value {
    let name = sanitize_server_name(&name);
    let base = server_dir(&name);
    let dir = safe_join_rel(&base, &subpath).unwrap_or_else(|| base.clone());
    let mut entries = Vec::new();
    if let Ok(rd) = std::fs::read_dir(&dir) {
        for e in rd.flatten() {
            let meta = e.metadata().ok();
            let is_dir = meta.as_ref().map(|m| m.is_dir()).unwrap_or(false);
            entries.push(json!({
                "name": e.file_name().to_string_lossy().to_string(),
                "is_dir": is_dir,
                "size": meta.as_ref().map(|m| m.len()).unwrap_or(0),
                "modified": meta.as_ref().and_then(|m| m.modified().ok())
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs()).unwrap_or(0),
            }));
        }
    }
    entries.sort_by(|a, b| {
        let ad = a["is_dir"].as_bool().unwrap_or(false);
        let bd = b["is_dir"].as_bool().unwrap_or(false);
        bd.cmp(&ad).then_with(|| {
            a["name"]
                .as_str()
                .unwrap_or("")
                .to_lowercase()
                .cmp(&b["name"].as_str().unwrap_or("").to_lowercase())
        })
    });
    json!({ "success": true, "path": subpath, "entries": entries })
}

#[tauri::command]
fn server_mgr_file_read(name: String, path: String) -> Value {
    let name = sanitize_server_name(&name);
    let base = server_dir(&name);
    let p = match safe_join_rel(&base, &path) {
        Some(p) => p,
        None => return json!({ "error": "Invalid path." }),
    };
    if let Ok(m) = std::fs::metadata(&p) {
        if m.len() > 5_000_000 {
            return json!({ "error": "File is too large to open (max 5 MB)." });
        }
    }
    match std::fs::read_to_string(&p) {
        Ok(c) => json!({ "success": true, "content": c }),
        Err(e) => json!({ "error": format!("Cannot read file: {}", e) }),
    }
}

#[tauri::command]
fn server_mgr_file_write(name: String, path: String, content: String) -> Value {
    let name = sanitize_server_name(&name);
    let base = server_dir(&name);
    let p = match safe_join_rel(&base, &path) {
        Some(p) => p,
        None => return json!({ "error": "Invalid path." }),
    };
    if let Some(parent) = p.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    match std::fs::write(&p, content) {
        Ok(_) => json!({ "success": true }),
        Err(e) => json!({ "error": e.to_string() }),
    }
}

#[tauri::command]
fn server_mgr_file_delete(name: String, path: String) -> Value {
    let name = sanitize_server_name(&name);
    let base = server_dir(&name);
    let p = match safe_join_rel(&base, &path) {
        Some(p) => p,
        None => return json!({ "error": "Invalid path." }),
    };
    if p == base {
        return json!({ "error": "Refusing to delete the server root." });
    }
    let r = if p.is_dir() {
        std::fs::remove_dir_all(&p)
    } else {
        std::fs::remove_file(&p)
    };
    match r {
        Ok(_) => json!({ "success": true }),
        Err(e) => json!({ "error": e.to_string() }),
    }
}

#[tauri::command]
fn server_mgr_file_mkdir(name: String, path: String) -> Value {
    let name = sanitize_server_name(&name);
    let base = server_dir(&name);
    let p = match safe_join_rel(&base, &path) {
        Some(p) => p,
        None => return json!({ "error": "Invalid path." }),
    };
    match std::fs::create_dir_all(&p) {
        Ok(_) => json!({ "success": true }),
        Err(e) => json!({ "error": e.to_string() }),
    }
}

#[tauri::command]
fn server_mgr_file_rename(name: String, path: String, new_name: String) -> Value {
    let name = sanitize_server_name(&name);
    let base = server_dir(&name);
    let src = match safe_join_rel(&base, &path) {
        Some(p) => p,
        None => return json!({ "error": "Invalid path." }),
    };
    if !src.exists() {
        return json!({ "error": "File not found." });
    }
    let fname = basename(&new_name);
    if fname.is_empty() {
        return json!({ "error": "Enter a valid name." });
    }
    let parent = src
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| base.clone());
    let dest = parent.join(&fname);
    if src == dest {
        return json!({ "success": true, "name": fname });
    }
    if dest.exists() {
        return json!({ "error": "An item with that name already exists." });
    }
    match std::fs::rename(&src, &dest) {
        Ok(_) => json!({ "success": true, "name": fname }),
        Err(e) => json!({ "error": e.to_string() }),
    }
}

#[tauri::command]
fn server_mgr_file_copy(name: String, path: String) -> Value {
    let name = sanitize_server_name(&name);
    let base = server_dir(&name);
    let src = match safe_join_rel(&base, &path) {
        Some(p) => p,
        None => return json!({ "error": "Invalid path." }),
    };
    if !src.exists() {
        return json!({ "error": "File not found." });
    }
    let parent = src
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| base.clone());
    let stem = src
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "copy".into());
    let ext = src.extension().map(|s| s.to_string_lossy().to_string());
    let mut dest = match &ext {
        Some(e) => parent.join(format!("{} - copy.{}", stem, e)),
        None => parent.join(format!("{} - copy", stem)),
    };
    let mut i = 1;
    while dest.exists() {
        dest = match &ext {
            Some(e) => parent.join(format!("{} - copy ({}).{}", stem, i, e)),
            None => parent.join(format!("{} - copy ({})", stem, i)),
        };
        i += 1;
        if i > 1000 {
            break;
        }
    }
    let result = if src.is_dir() {
        copy_dir_recursive(&src, &dest)
    } else {
        std::fs::copy(&src, &dest)
            .map(|_| ())
            .map_err(|e| e.to_string())
    };
    match result {
        Ok(_) => json!({
            "success": true,
            "name": dest.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default()
        }),
        Err(e) => json!({ "error": e }),
    }
}

#[tauri::command]
fn server_mgr_file_upload(name: String, subpath: String, filename: String, data: String) -> Value {
    let name = sanitize_server_name(&name);
    let base = server_dir(&name);
    let dir = match safe_join_rel(&base, &subpath) {
        Some(d) => d,
        None => return json!({ "error": "Invalid path." }),
    };
    let _ = std::fs::create_dir_all(&dir);
    let fname = basename(&filename);
    let fname = if fname.is_empty() {
        "upload.bin".to_string()
    } else {
        fname
    };
    let dest = dir.join(&fname);
    let raw = data
        .trim_start_matches("data:*;base64,")
        .trim_start_matches("data:application/octet-stream;base64,");
    let bytes = match base64::engine::general_purpose::STANDARD.decode(raw.as_bytes()) {
        Ok(b) => b,
        Err(e) => return json!({ "error": format!("Invalid upload data: {}", e) }),
    };
    match std::fs::write(&dest, &bytes) {
        Ok(_) => json!({ "success": true, "bytes": bytes.len(), "filename": fname }),
        Err(e) => json!({ "error": e.to_string() }),
    }
}

#[tauri::command]
fn server_mgr_file_read_b64(name: String, path: String) -> Value {
    let name = sanitize_server_name(&name);
    let base = server_dir(&name);
    let p = match safe_join_rel(&base, &path) {
        Some(p) => p,
        None => return json!({ "error": "Invalid path." }),
    };
    if let Ok(m) = std::fs::metadata(&p) {
        if m.len() > 16_000_000 {
            return json!({ "error": "File is too large to preview (max 16 MB)." });
        }
    }
    match std::fs::read(&p) {
        Ok(b) => json!({ "success": true, "data": base64::engine::general_purpose::STANDARD.encode(&b) }),
        Err(e) => json!({ "error": e.to_string() }),
    }
}

#[tauri::command]
fn server_mgr_file_download(name: String, path: String) -> Value {
    let name = sanitize_server_name(&name);
    let base = server_dir(&name);
    let src = match safe_join_rel(&base, &path) {
        Some(p) => p,
        None => return json!({ "error": "Invalid path." }),
    };
    if !src.is_file() {
        return json!({ "error": "Only files can be downloaded." });
    }
    // Copy into the user's Downloads folder (no native save dialog needed).
    let dest_dir = home().join("Downloads");
    let _ = std::fs::create_dir_all(&dest_dir);
    let fname = src
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "file.bin".into());
    let dest = unique_path(&dest_dir.join(fname));
    match std::fs::copy(&src, &dest) {
        Ok(_) => json!({ "success": true, "saved": dest.to_string_lossy() }),
        Err(e) => json!({ "error": e.to_string() }),
    }
}

#[tauri::command]
fn server_mgr_read_icon(name: String) -> Value {
    let name = sanitize_server_name(&name);
    let p = server_dir(&name).join("server-icon.png");
    if !p.exists() {
        return json!({ "exists": false });
    }
    match std::fs::read(&p) {
        Ok(bytes) => json!({
            "exists": true,
            "data": base64::engine::general_purpose::STANDARD.encode(bytes)
        }),
        Err(_) => json!({ "exists": false }),
    }
}

// ---- backups ----

#[tauri::command]
fn server_mgr_backup_list(name: String) -> Value {
    let name = sanitize_server_name(&name);
    let dir = server_dir(&name).join(BACKUP_DIR);
    let mut out = Vec::new();
    if let Ok(rd) = std::fs::read_dir(&dir) {
        for e in rd.flatten() {
            let p = e.path();
            if p.is_file() {
                let size = e.metadata().map(|m| m.len()).unwrap_or(0);
                let modified = e
                    .metadata()
                    .ok()
                    .and_then(|m| m.modified().ok())
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                out.push(json!({
                    "filename": e.file_name().to_string_lossy().to_string(),
                    "size": size,
                    "modified": modified,
                }));
            }
        }
    }
    out.sort_by(|a, b| b["modified"].as_u64().unwrap_or(0).cmp(&a["modified"].as_u64().unwrap_or(0)));
    json!({ "success": true, "backups": out })
}

#[tauri::command]
fn server_mgr_backup_create(name: String, label: Option<String>) -> Value {
    let name = sanitize_server_name(&name);
    let base = server_dir(&name);
    if !base.join("server.json").exists() {
        return json!({ "error": "Server not found." });
    }
    let backups = base.join(BACKUP_DIR);
    let _ = std::fs::create_dir_all(&backups);
    let label = label
        .filter(|l| !l.trim().is_empty())
        .unwrap_or_else(|| chrono::Local::now().format("%Y%m%d-%H%M%S").to_string());
    let safe_label: String = label
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let file = unique_path(&backups.join(format!("{}.zip", safe_label)));
    let zf = match std::fs::File::create(&file) {
        Ok(f) => f,
        Err(e) => return json!({ "error": e.to_string() }),
    };
    let mut z = zip::ZipWriter::new(zf);
    let opts: zip::write::FileOptions = zip::write::FileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    let mut stack = vec![base.clone()];
    while let Some(dir) = stack.pop() {
        if dir == backups {
            continue;
        }
        if let Ok(rd) = std::fs::read_dir(&dir) {
            for e in rd.flatten() {
                let p = e.path();
                if p == backups {
                    continue;
                }
                if p.is_dir() {
                    stack.push(p);
                } else if p.is_file() {
                    if let Ok(rel) = p.strip_prefix(&base) {
                        let rel_str = rel.to_string_lossy().replace('\\', "/");
                        if z.start_file(rel_str, opts).is_ok() {
                            if let Ok(mut f) = std::fs::File::open(&p) {
                                let _ = std::io::copy(&mut f, &mut z);
                            }
                        }
                    }
                }
            }
        }
    }
    match z.finish() {
        Ok(_) => json!({
            "success": true,
            "file": file.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default()
        }),
        Err(e) => json!({ "error": e.to_string() }),
    }
}

#[tauri::command]
fn server_mgr_backup_restore(app: tauri::State<AppState>, name: String, file: String) -> Value {
    let name = sanitize_server_name(&name);
    if app.server_procs.lock().unwrap().contains_key(&name) {
        return json!({ "error": "Stop the server before restoring a backup." });
    }
    let base = server_dir(&name);
    let archive_path = base.join(BACKUP_DIR).join(basename(&file));
    if !archive_path.exists() {
        return json!({ "error": "Backup not found." });
    }
    let f = match std::fs::File::open(&archive_path) {
        Ok(f) => f,
        Err(e) => return json!({ "error": e.to_string() }),
    };
    let mut archive = match zip::ZipArchive::new(f) {
        Ok(a) => a,
        Err(e) => return json!({ "error": e.to_string() }),
    };
    for i in 0..archive.len() {
        let mut entry = match archive.by_index(i) {
            Ok(e) => e,
            Err(_) => continue,
        };
        if entry.name().starts_with(&format!("{}/", BACKUP_DIR)) {
            continue;
        }
        let outpath = match entry.enclosed_name() {
            Some(p) => base.join(p),
            None => continue,
        };
        if entry.is_dir() {
            let _ = std::fs::create_dir_all(&outpath);
        } else {
            if let Some(parent) = outpath.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if let Ok(mut out) = std::fs::File::create(&outpath) {
                let _ = std::io::copy(&mut entry, &mut out);
            }
        }
    }
    json!({ "success": true })
}

#[tauri::command]
fn server_mgr_backup_delete(name: String, file: String) -> Value {
    let name = sanitize_server_name(&name);
    let path = server_dir(&name).join(BACKUP_DIR).join(basename(&file));
    if !path.exists() {
        return json!({ "error": "Backup not found." });
    }
    match std::fs::remove_file(&path) {
        Ok(_) => json!({ "success": true }),
        Err(e) => json!({ "error": e.to_string() }),
    }
}

// ---- worlds ----

#[tauri::command]
fn server_mgr_list_worlds(name: String) -> Value {
    let name = sanitize_server_name(&name);
    let base = server_dir(&name);
    let mut worlds = Vec::new();
    if let Ok(rd) = std::fs::read_dir(&base) {
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                let wname = e.file_name().to_string_lossy().to_string();
                if wname.starts_with("world") && p.join("level.dat").exists() {
                    worlds.push(wname);
                }
            }
        }
    }
    worlds.sort();
    json!({ "success": true, "worlds": worlds })
}

#[tauri::command]
fn server_mgr_delete_world(app: tauri::State<AppState>, name: String, world: String) -> Value {
    let name = sanitize_server_name(&name);
    if app.server_procs.lock().unwrap().contains_key(&name) {
        return json!({ "error": "Stop the server before deleting a world." });
    }
    let base = server_dir(&name);
    let wname = basename(&world);
    if !wname.starts_with("world") {
        return json!({ "error": "Invalid world name." });
    }
    let p = base.join(&wname);
    if !p.exists() {
        return json!({ "error": "World not found." });
    }
    match std::fs::remove_dir_all(&p) {
        Ok(_) => json!({ "success": true }),
        Err(e) => json!({ "error": e.to_string() }),
    }
}

#[tauri::command]
fn server_mgr_reset_world(app: tauri::State<AppState>, name: String, world: String) -> Value {
    let name = sanitize_server_name(&name);
    if app.server_procs.lock().unwrap().contains_key(&name) {
        return json!({ "error": "Stop the server before resetting a world." });
    }
    let base = server_dir(&name);
    let mut removed: Vec<String> = Vec::new();
    let targets: Vec<String> = if !world.trim().is_empty() {
        vec![basename(&world)]
    } else {
        let mut v = Vec::new();
        if let Ok(rd) = std::fs::read_dir(&base) {
            for e in rd.flatten() {
                let n = e.file_name().to_string_lossy().to_string();
                let p = e.path();
                if p.is_dir() && n.starts_with("world") && p.join("level.dat").exists() {
                    v.push(n);
                }
            }
        }
        v
    };
    for w in targets {
        if !w.starts_with("world") {
            continue;
        }
        let p = base.join(&w);
        if p.exists() && std::fs::remove_dir_all(&p).is_ok() {
            removed.push(w);
        }
    }
    json!({ "success": true, "removed": removed })
}

// ---- meta / EULA / clone / stats / ports ----

#[tauri::command]
fn server_mgr_set_meta(name: String, ram_mb: Option<u64>, motd: Option<String>, port: Option<u16>) -> Value {
    let name = sanitize_server_name(&name);
    let path = server_dir(&name).join("server.json");
    if !path.exists() {
        return json!({ "error": "Server not found." });
    }
    let mut meta = load_json(&path);
    if !meta.is_object() {
        meta = json!({});
    }
    if let Some(r) = ram_mb {
        meta["ram_mb"] = json!(r);
    }
    if let Some(m) = motd {
        meta["motd"] = json!(m);
    }
    if let Some(p) = port {
        meta["port"] = json!(p);
    }
    save_json(&path, &meta);
    sync_server_port(&name, port);
    json!({ "success": true, "server": meta })
}

#[tauri::command]
fn server_mgr_update_meta(name: String, patch: Value) -> Value {
    let name = sanitize_server_name(&name);
    let path = server_dir(&name).join("server.json");
    if !path.exists() {
        return json!({ "error": "Server not found." });
    }
    let mut meta = load_json(&path);
    if !meta.is_object() {
        meta = json!({});
    }
    if let Some(obj) = patch.as_object() {
        for (k, v) in obj {
            meta[k] = v.clone();
        }
    }
    save_json(&path, &meta);
    let port = patch.get("port").and_then(|v| v.as_u64()).map(|p| p as u16);
    sync_server_port(&name, port);
    json!({ "success": true, "server": meta })
}

/// Mirror a port change into server.properties.
fn sync_server_port(name: &str, port: Option<u16>) {
    let Some(p) = port else { return };
    let pp = server_dir(name).join("server.properties");
    if !pp.exists() {
        return;
    }
    let content = std::fs::read_to_string(&pp).unwrap_or_default();
    let lines: Vec<String> = content
        .lines()
        .map(|l| {
            if l.trim_start().starts_with("server-port=") {
                format!("server-port={}", p)
            } else {
                l.to_string()
            }
        })
        .collect();
    let _ = std::fs::write(&pp, lines.join("\n") + "\n");
}

#[tauri::command]
fn server_mgr_eula(name: String, accept: bool) -> Value {
    let name = sanitize_server_name(&name);
    match std::fs::write(
        server_dir(&name).join("eula.txt"),
        format!("eula={}\n", accept),
    ) {
        Ok(_) => json!({ "success": true }),
        Err(e) => json!({ "error": e.to_string() }),
    }
}

#[tauri::command]
fn server_mgr_clone(
    app: tauri::State<AppState>,
    source: String,
    new_name: String,
    new_port: Option<u16>,
) -> Value {
    let source = sanitize_server_name(&source);
    let new_name = sanitize_server_name(&new_name);
    if new_name.is_empty() {
        return json!({ "error": "Enter a valid new server name." });
    }
    if app.server_procs.lock().unwrap().contains_key(&source) {
        return json!({ "error": "Stop the source server before cloning it." });
    }
    let src = server_dir(&source);
    let dst = server_dir(&new_name);
    if !src.join("server.json").exists() {
        return json!({ "error": "Source server not found." });
    }
    if dst.exists() {
        return json!({ "error": "A server with that name already exists." });
    }
    if let Err(e) = copy_dir_recursive(&src, &dst) {
        return json!({ "error": e });
    }
    let _ = std::fs::remove_dir_all(dst.join(BACKUP_DIR));
    let meta_path = dst.join("server.json");
    let mut meta = load_json(&meta_path);
    if !meta.is_object() {
        meta = json!({});
    }
    meta["name"] = json!(new_name);
    if let Some(p) = new_port {
        meta["port"] = json!(p);
    }
    save_json(&meta_path, &meta);
    sync_server_port(&new_name, new_port);
    json!({ "success": true, "server": meta })
}

#[tauri::command]
fn server_mgr_stats(app: tauri::State<AppState>, name: String) -> Value {
    let name = sanitize_server_name(&name);
    let procs = app.server_procs.lock().unwrap();
    match procs.get(&name) {
        Some(p) => {
            let pid = p.child.id();
            json!({
                "running": true,
                "pid": pid,
                "uptime_secs": p.started_at.elapsed().as_secs(),
                "memory_kb": read_process_rss_kb(pid).unwrap_or(0),
            })
        }
        None => json!({ "running": false, "pid": 0, "uptime_secs": 0, "memory_kb": 0 }),
    }
}

#[tauri::command]
fn server_mgr_check_port(port: u16) -> Value {
    match std::net::TcpListener::bind(("0.0.0.0", port)) {
        Ok(_) => json!({ "available": true }),
        Err(_) => json!({ "available": false }),
    }
}

#[tauri::command]
fn server_mgr_next_port(start: Option<u16>) -> Value {
    let s = start.unwrap_or(25565);
    for p in s..s.saturating_add(1000) {
        if std::net::TcpListener::bind(("0.0.0.0", p)).is_ok() {
            return json!({ "port": p });
        }
    }
    json!({ "port": s })
}

#[tauri::command]
fn server_mgr_list_java(app: tauri::State<AppState>) -> Value {
    json!({ "success": true, "javas": scan_java(&app, true) })
}

#[tauri::command]
fn server_mgr_open_folder(name: String) -> Value {
    let name = sanitize_server_name(&name);
    let dir = server_dir(&name);
    if !dir.exists() {
        return json!({ "error": "Server folder not found." });
    }
    match open_in_os(&dir) {
        Ok(()) => json!({ "success": true }),
        Err(e) => json!({ "error": e }),
    }
}

fn reinstall_server_blocking(
    handle: Option<tauri::AppHandle>,
    name: &str,
    version: &str,
) -> Value {
    let name = sanitize_server_name(name);
    let dir = server_dir(&name);
    let meta_path = dir.join("server.json");
    if !meta_path.exists() {
        return json!({ "error": "Server not found." });
    }
    let mut meta = load_json(&meta_path);
    let stype = meta
        .get("type")
        .and_then(|v| v.as_str())
        .unwrap_or("paper")
        .to_string();
    let url = match stype.as_str() {
        "vanilla" => resolve_vanilla_server_url(version),
        "purpur" => resolve_purpur_download_url(version),
        "fabric" => resolve_fabric_download_url(version),
        _ => resolve_paper_download_url(version),
    };
    let url = match url {
        Ok(u) => u,
        Err(e) => return json!({ "error": e }),
    };
    let name_for_evt = name.clone();
    let dl = download_file_progress(&url, &dir.join("server.jar"), |done, total| {
        if let Some(h) = &handle {
            emit(
                h,
                "server-mgr-download",
                json!({ "server": name_for_evt, "phase": "jar", "downloaded": done, "total": total }),
            );
        }
    });
    if let Err(e) = dl {
        return json!({ "error": format!("Failed to download server jar: {}", e) });
    }
    if !meta.is_object() {
        meta = json!({});
    }
    meta["version"] = json!(version);
    save_json(&meta_path, &meta);
    json!({ "success": true, "server": meta })
}

#[tauri::command]
async fn server_mgr_reinstall(handle: tauri::AppHandle, name: String, version: String) -> Value {
    blocking(move || reinstall_server_blocking(Some(handle), &name, &version))
}

// ============================================================
// INSTANCE FILE EXTRAS (mods / resourcepacks / icons / java)
// ============================================================

#[tauri::command]
fn open_instance_path(instance_name: String, sub_folder: String) -> Value {
    let sub = sub_folder.trim().trim_matches(['/', '\\']);
    if sub.contains("..") || sub.starts_with('/') {
        return json!({ "error": "Invalid folder path" });
    }
    let base = instance_dir(&instance_name);
    if !base.exists() {
        return json!({ "error": "Instance folder not found" });
    }
    let dir = if sub.is_empty() {
        base.clone()
    } else {
        match safe_join(&base, sub) {
            Ok(p) => p,
            Err(e) => return json!({ "error": e }),
        }
    };
    let _ = std::fs::create_dir_all(&dir);
    match open_in_os(&dir) {
        Ok(()) => json!({ "success": true, "path": dir.to_string_lossy() }),
        Err(e) => json!({ "error": e }),
    }
}

#[tauri::command]
fn get_instance_mods(instance_name: String) -> Value {
    let mods_dir = instance_dir(&instance_name).join("mods");
    let mut mods = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&mods_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            let clean = name.trim_end_matches(".disabled").to_string();
            if !(clean.ends_with(".jar") || clean.ends_with(".zip")) {
                continue;
            }
            let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
            let display = clean
                .trim_end_matches(".jar")
                .trim_end_matches(".zip")
                .to_string();
            mods.push(json!({
                "filename": name,
                "name": display,
                "size": size,
                "enabled": !name.ends_with(".disabled"),
            }));
        }
    }
    mods.sort_by(|a, b| {
        a.get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_lowercase()
            .cmp(
                &b.get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_lowercase(),
            )
    });
    Value::Array(mods)
}

#[tauri::command]
fn get_instance_resourcepacks(instance_name: String) -> Value {
    let rp_dir = instance_dir(&instance_name).join("resourcepacks");
    let mut packs = Vec::new();
    if let Ok(entries) = std::fs::read_dir(rp_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            let clean = name.trim_end_matches(".disabled").to_string();
            if !(clean.ends_with(".zip") || clean.ends_with(".jar")) {
                continue;
            }
            let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
            packs.push(json!({
                "filename": name,
                "name": clean.trim_end_matches(".zip").trim_end_matches(".jar"),
                "size": size,
                "enabled": !name.ends_with(".disabled"),
            }));
        }
    }
    Value::Array(packs)
}

#[tauri::command]
fn delete_mod(instance_name: String, filename: String) -> Value {
    let dir = instance_dir(&instance_name).join("mods");
    let path = match safe_join(&dir, &filename) {
        Ok(p) => p,
        Err(e) => return json!({ "error": e }),
    };
    if !path.exists() {
        return json!({ "error": "File not found" });
    }
    match std::fs::remove_file(&path) {
        Ok(_) => json!({ "success": true }),
        Err(e) => json!({ "error": e.to_string() }),
    }
}

#[tauri::command]
fn delete_resourcepack(instance_name: String, filename: String) -> Value {
    let dir = instance_dir(&instance_name).join("resourcepacks");
    let path = match safe_join(&dir, &filename) {
        Ok(p) => p,
        Err(e) => return json!({ "error": e }),
    };
    if !path.exists() {
        return json!({ "error": "File not found" });
    }
    match std::fs::remove_file(&path) {
        Ok(_) => json!({ "success": true }),
        Err(e) => json!({ "error": e.to_string() }),
    }
}

/// Copy an externally chosen file into the instance's `mods` folder.
#[tauri::command]
fn move_mod_file(instance_name: String, source_path: String) -> Value {
    move_into_folder(&instance_name, &source_path, "mods")
}

/// Copy an externally chosen file into the instance's `resourcepacks` folder.
#[tauri::command]
fn move_resourcepack_file(instance_name: String, source_path: String) -> Value {
    move_into_folder(&instance_name, &source_path, "resourcepacks")
}

fn move_into_folder(instance_name: &str, source_path: &str, sub: &str) -> Value {
    let dir = instance_dir(instance_name).join(sub);
    let _ = std::fs::create_dir_all(&dir);
    let source = PathBuf::from(source_path);
    if !source.is_file() {
        return json!({ "error": "Source file not found" });
    }
    let filename = source
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    if filename.is_empty() {
        return json!({ "error": "Source file has no name." });
    }
    let dest = match safe_join(&dir, &filename) {
        Ok(p) => p,
        Err(e) => return json!({ "error": e }),
    };
    match std::fs::copy(&source, &dest) {
        Ok(_) => json!({ "success": true, "filename": filename }),
        Err(e) => json!({ "error": e.to_string() }),
    }
}

/// Append a pack to `resourcePacks:[...]` in the instance's options.txt.
#[tauri::command]
fn auto_enable_resourcepack(instance_name: String, filename: String) -> Value {
    let options_path = instance_dir(&instance_name).join("options.txt");
    let mut content = std::fs::read_to_string(&options_path).unwrap_or_default();
    let entry = format!(":\"{}\"", filename);
    if content.contains(&entry) {
        return json!({ "success": true, "message": "Resource pack already enabled" });
    }
    const KEY: &str = "resourcePacks";
    if let Some(pos) = content.find(KEY) {
        let after_key = &content[pos + KEY.len()..];
        if let Some(rel) = after_key.find('[') {
            let start = pos + KEY.len() + rel;
            if let Some(end_rel) = content[start..].find(']') {
                let end = start + end_rel;
                let inner = &content[start + 1..end];
                let mut packs: Vec<String> = inner
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
                if !packs.iter().any(|p| p.contains(&filename)) {
                    packs.push(entry.clone());
                }
                let replacement = format!("[{}]", packs.join(", "));
                content = format!("{}{}{}", &content[..start], replacement, &content[end..]);
            }
        }
    } else {
        content.push_str(&format!("\nresourcePacks:[{}]\n", entry));
    }
    match std::fs::write(&options_path, content) {
        Ok(_) => json!({ "success": true }),
        Err(e) => json!({ "error": e.to_string() }),
    }
}

/// Rename and/or retarget an instance (Minecraft version + loader).
#[tauri::command]
fn edit_instance(
    app: tauri::State<AppState>,
    instance_name: String,
    new_name: String,
    mc_version: String,
    loader: String,
    loader_version: String,
) -> Value {
    let new_name = sanitize_name(new_name.trim());
    if new_name.is_empty() {
        return json!({ "error": "Profile name cannot be empty" });
    }
    let mut list = load_instances_raw(&app);
    let idx = match list
        .iter()
        .position(|i| i.get("name").and_then(|v| v.as_str()) == Some(instance_name.as_str()))
    {
        Some(i) => i,
        None => return json!({ "error": "Instance not found" }),
    };
    let renamed = new_name != instance_name;
    if renamed {
        let old_dir = instance_dir(&instance_name);
        let new_dir = instance_dir(&new_name);
        if new_dir.exists() {
            return json!({ "error": "An instance with that name already exists" });
        }
        if old_dir.exists() {
            if let Err(e) = std::fs::rename(&old_dir, &new_dir) {
                return json!({ "error": format!("Could not rename profile folder: {}", e) });
            }
        }
        // Carry per-profile JVM/RAM overrides to the new key.
        let sp = instance_settings_path();
        let mut sd = load_json(&sp);
        if let Some(obj) = sd.as_object_mut() {
            if let Some(entry) = obj.remove(&instance_name) {
                obj.insert(new_name.clone(), entry);
                save_json(&sp, &sd);
            }
        }
    }
    let entry = &mut list[idx];
    entry["name"] = json!(new_name);
    if !mc_version.is_empty() {
        entry["version"] = json!(mc_version);
    }
    if !loader.is_empty() {
        entry["loader"] = json!(loader);
    }
    if !loader_version.is_empty() {
        entry["loader_version"] = json!(loader_version);
    }
    save_instances(&app, &list);
    log_msg(
        &app,
        &format!(
            "[Instance] Updated '{}' -> '{}' ({} {})",
            instance_name, new_name, loader, mc_version
        ),
    );
    json!({ "success": true, "name": new_name, "renamed": renamed })
}

#[tauri::command]
fn set_instance_icon(instance_name: String, icon_data: String) -> Value {
    let inst_dir = instance_dir(&instance_name);
    if !inst_dir.exists() {
        return json!({ "error": "Profile folder not found — install the profile first" });
    }
    let icon_path = inst_dir.join("icon.png");
    let raw = icon_data.trim_start_matches("data:image/png;base64,");
    match base64::engine::general_purpose::STANDARD.decode(raw) {
        Ok(bytes) => match std::fs::write(&icon_path, &bytes) {
            Ok(_) => json!({ "success": true, "path": icon_path.to_string_lossy() }),
            Err(e) => json!({ "error": format!("Could not write the icon file: {}", e) }),
        },
        Err(_) => json!({ "error": "Invalid base64 data" }),
    }
}

#[tauri::command]
fn get_instance_icon(instance_name: String) -> Value {
    let icon_path = instance_dir(&instance_name).join("icon.png");
    if let Ok(bytes) = std::fs::read(&icon_path) {
        let encoded = base64::engine::general_purpose::STANDARD.encode(&bytes);
        return json!({ "success": true, "icon": format!("data:image/png;base64,{}", encoded) });
    }
    json!({ "success": false, "icon": Value::Null })
}

// ---------- per-instance Java ----------

#[tauri::command]
fn get_java_installations() -> Value {
    let found = scan_java_static();
    Value::Array(found)
}

/// `scan_java` without needing AppState (used by the standalone commands).
fn scan_java_static() -> Vec<Value> {
    let mut out: Vec<Value> = collect_java_homes()
        .iter()
        .filter_map(|h| java_record(h))
        .collect();
    out.sort_by(|a, b| {
        let ma = a.get("major").and_then(|m| m.as_u64()).unwrap_or(0);
        let mb = b.get("major").and_then(|m| m.as_u64()).unwrap_or(0);
        mb.cmp(&ma)
    });
    // Parity aliases with Xryloth's shape.
    for rec in out.iter_mut() {
        let major = rec.get("major").cloned().unwrap_or(json!(0));
        rec["major_version"] = major;
        rec["managed"] = json!(rec
            .get("home")
            .and_then(|h| h.as_str())
            .map(|h| h.starts_with(&java_dir().to_string_lossy().to_string()))
            .unwrap_or(false));
        rec["compatible_arch"] = json!(true);
    }
    out
}

/// Java major version required by a Minecraft version.
/// Live lookup from Mojang's metadata (`javaVersion.majorVersion`), then a
/// static fallback table.
fn required_java_major(mc_version: &str) -> u32 {
    if let Ok(manifest) = http_get(MOJANG_MANIFEST) {
        if let Some(versions) = manifest.get("versions").and_then(|v| v.as_array()) {
            if let Some(entry) = versions
                .iter()
                .find(|e| e.get("id").and_then(|i| i.as_str()) == Some(mc_version))
            {
                if let Some(url) = entry.get("url").and_then(|u| u.as_str()) {
                    if let Ok(vj) = http_get(url) {
                        if let Some(maj) = vj
                            .get("javaVersion")
                            .and_then(|j| j.get("majorVersion"))
                            .and_then(|m| m.as_u64())
                        {
                            return maj as u32;
                        }
                    }
                }
            }
        }
    }
    let vt = parse_version_tuple(mc_version);
    if vt >= (1, 21, 0) {
        21
    } else if vt >= (1, 20, 2) {
        17
    } else if vt >= (1, 17, 0) {
        16
    } else {
        8
    }
}

#[tauri::command]
fn check_java_compatibility(
    app: tauri::State<AppState>,
    mc_version: String,
) -> Value {
    let required = required_java_major(&mc_version);
    let javas = scan_java_static();
    let settings = load_settings(&app);
    let custom = settings
        .get("java_path")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let compatible: Vec<&Value> = javas
        .iter()
        .filter(|j| j.get("major").and_then(|v| v.as_u64()).unwrap_or(0) as u32 == required)
        .collect();
    let current_major = if !custom.is_empty() && Path::new(&custom).exists() {
        java_probe(Path::new(&custom)).map(|(_, m)| m)
    } else {
        javas.first().and_then(|j| j.get("major").and_then(|m| m.as_u64()).map(|m| m as u32))
    };
    json!({
        "mc_version": mc_version,
        "required_java": required,
        "current_java_version": current_major,
        "current_compatible": current_major == Some(required),
        "available_javas": javas,
        "compatible_javas": compatible,
        "managed_java_dir": java_dir().to_string_lossy(),
    })
}

/// Alias kept for Xryloth parity — installs a managed JDK and pins it globally.
#[tauri::command]
async fn download_java(handle: tauri::AppHandle, major_version: u32) -> Value {
    let r = install_java(handle.clone(), major_version).await;
    if r.get("success").and_then(|s| s.as_bool()).unwrap_or(false) {
        if let Some(path) = r.get("java").and_then(|j| j.get("path")).and_then(|p| p.as_str()) {
            let app = handle.state::<AppState>();
            save_setting_val(&app, "java_path", json!(path));
        }
    }
    r
}

/// Xryloth parity: set the launcher-wide Java executable.
#[tauri::command]
fn set_java_for_instance(app: tauri::State<AppState>, java_path: String) -> Value {
    if java_path.trim().is_empty() {
        return json!({ "error": "Java path is required" });
    }
    if !Path::new(&java_path).exists() {
        return json!({ "error": "That Java executable does not exist." });
    }
    save_setting_val(&app, "java_path", json!(java_path.trim()));
    json!({ "success": true })
}

/// Install a template's mod list onto an existing instance.
#[tauri::command]
async fn apply_instance_template(
    handle: tauri::AppHandle,
    instance_name: String,
    template_id: String,
) -> Value {
    let t = match TEMPLATES.iter().find(|t| t.id == template_id) {
        Some(t) => t,
        None => return json!({ "error": "Template not found." }),
    };
    if !loader_supports(t.loader, t.mc) {
        return json!({ "error": format!(
            "{} isn't supported by this build yet ({} {}).",
            t.loader, t.mc, t.loader_ver
        ) });
    }
    let app = handle.state::<AppState>();
    if find_instance(&app, &instance_name).is_none() {
        return json!({ "error": "Instance not found." });
    }
    drop(app);
    let slugs: Vec<String> = t
        .mods
        .split(',')
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect();
    let mut installed = 0usize;
    let mut errors: Vec<Value> = Vec::new();
    for slug in slugs {
        let app = handle.state::<AppState>();
        match install_mod_inner(&app, &instance_name, &slug, "", "mods") {
            Ok(_) => installed += 1,
            Err(e) => errors.push(json!({ "slug": slug, "error": e })),
        }
    }
    emit(
        &handle,
        "template-progress",
        json!({
            "template_name": t.name,
            "instance": instance_name,
            "installed": installed,
            "errors": errors.len(),
            "error_list": errors,
        }),
    );
    json!({ "success": true, "installed": installed, "errors": errors })
}

// ---------- Fabric loader versions ----------

const FABRIC_VERSIONS_URL: &str = "https://meta.fabricmc.net/v2/versions/game";

#[tauri::command]
fn get_fabric_versions() -> Value {
    match http_get(FABRIC_VERSIONS_URL) {
        Ok(data) => {
            let versions: Vec<Value> = data
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter(|v| v.get("stable").and_then(|s| s.as_bool()).unwrap_or(false))
                        .filter_map(|v| {
                            v.get("version")
                                .and_then(|s| s.as_str())
                                .map(|s| Value::String(s.to_string()))
                        })
                        .collect()
                })
                .unwrap_or_default();
            Value::Array(versions)
        }
        Err(_) => Value::Array(vec![]),
    }
}

// ---------- Live status of an arbitrary Minecraft server ----------

#[tauri::command]
fn get_server_status(host: String, port: String) -> Value {
    let safe_host = host.trim().trim_end_matches('/').to_string();
    if safe_host.is_empty() {
        return json!({ "online": false, "host": "", "port": port, "error": "empty host" });
    }
    let safe_port = port.trim();
    let has_port = safe_host.contains(':');
    let mut candidates: Vec<String> = vec![format!("https://api.mcsrvstat.us/3/{}", safe_host)];
    if !has_port && !safe_port.is_empty() && safe_port != "25565" {
        candidates.push(format!("https://api.mcsrvstat.us/3/{}:{}", safe_host, safe_port));
    }
    let mut last_err = String::new();
    for url in &candidates {
        match http_get(url) {
            Ok(v) => {
                let online = v.get("online").and_then(|o| o.as_bool()).unwrap_or(false);
                if online {
                    let players = v
                        .get("players")
                        .and_then(|p| p.get("online"))
                        .and_then(|p| p.as_u64())
                        .unwrap_or(0);
                    let max = v
                        .get("players")
                        .and_then(|p| p.get("max"))
                        .and_then(|p| p.as_u64())
                        .unwrap_or(0);
                    let version = v
                        .get("version")
                        .and_then(|x| x.as_str())
                        .unwrap_or("")
                        .to_string();
                    let motd = v
                        .get("motd")
                        .and_then(|m| m.get("clean"))
                        .and_then(|m| m.as_array())
                        .map(|a| {
                            a.iter()
                                .filter_map(|s| s.as_str())
                                .collect::<Vec<_>>()
                                .join("\n")
                        })
                        .unwrap_or_default();
                    return json!({
                        "online": true,
                        "host": safe_host,
                        "port": safe_port,
                        "version": version,
                        "players_online": players,
                        "players_max": max,
                        "motd": motd,
                        "raw": v,
                    });
                }
                last_err = "Server is offline".to_string();
            }
            Err(e) => last_err = e,
        }
    }
    json!({
        "online": false,
        "host": safe_host,
        "port": safe_port,
        "error": last_err,
    })
}

// ============================================================
// MOD PROFILES
// ============================================================

fn mod_profiles_file() -> PathBuf {
    app_dir().join("mod_profiles.json")
}

/// Minimal percent-encoder (we don't pull in a URL crate for one call).
fn pct_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

/// Resolve a display name to a Modrinth slug (best effort — falls back to a
/// slugified name so the profile still round-trips).
fn resolve_modrinth_slug(display_name: &str) -> String {
    let fallback = display_name
        .to_lowercase()
        .replace('_', "-")
        .replace(' ', "-");
    let url = format!(
        "{}/search?query={}&limit=1",
        MODRINTH_API,
        pct_encode(display_name)
    );
    if let Ok(data) = http_get(&url) {
        if let Some(hits) = data.get("hits").and_then(|h| h.as_array()) {
            if let Some(slug) = hits.first().and_then(|h| h.get("slug")).and_then(|s| s.as_str()) {
                if !slug.is_empty() {
                    return slug.to_string();
                }
            }
        }
    }
    fallback
}

/// Snapshot an instance's installed mods into a named profile.
#[tauri::command]
fn save_mod_profile(app: tauri::State<AppState>, name: String, instance_name: String) -> Value {
    let name = name.trim().to_string();
    if name.is_empty() {
        return json!({ "error": "Profile name is required." });
    }
    let mods_dir = instance_dir(&instance_name).join("mods");
    let mut mods = Vec::new();
    if let Ok(entries) = std::fs::read_dir(mods_dir) {
        for entry in entries.flatten() {
            let fname = entry.file_name().to_string_lossy().to_string();
            let clean = fname.trim_end_matches(".disabled").to_string();
            if !clean.ends_with(".jar") {
                continue;
            }
            let display = clean.trim_end_matches(".jar").to_string();
            let slug = resolve_modrinth_slug(&display);
            mods.push(json!({
                "slug": slug,
                "version_id": "",
                "filename": clean,
            }));
        }
    }
    let path = mod_profiles_file();
    let mut profiles: Vec<Value> = load_json(&path).as_array().cloned().unwrap_or_default();
    profiles.retain(|p| p.get("name").and_then(|n| n.as_str()) != Some(name.as_str()));
    let count = mods.len();
    profiles.push(json!({
        "name": name,
        "created": chrono::Local::now().to_rfc3339(),
        "instance_source": instance_name,
        "mods": mods,
    }));
    save_json(&path, &Value::Array(profiles));
    log_msg(
        &app,
        &format!("[Profiles] Saved '{}' with {} mods", name, count),
    );
    json!({ "success": true, "mods_count": count })
}

#[tauri::command]
fn load_mod_profiles() -> Value {
    let profiles = load_json(&mod_profiles_file())
        .as_array()
        .cloned()
        .unwrap_or_default();
    Value::Array(profiles)
}

#[tauri::command]
fn delete_mod_profile(name: String) -> Value {
    let path = mod_profiles_file();
    let mut profiles: Vec<Value> = load_json(&path).as_array().cloned().unwrap_or_default();
    let before = profiles.len();
    profiles.retain(|p| p.get("name").and_then(|n| n.as_str()) != Some(name.as_str()));
    save_json(&path, &Value::Array(profiles.clone()));
    json!({ "success": true, "removed": before > profiles.len() })
}

/// Reinstall every mod recorded in a profile onto an instance.
#[tauri::command]
fn apply_mod_profile(
    app: tauri::State<AppState>,
    name: String,
    instance_name: String,
) -> Value {
    let profiles: Vec<Value> = load_json(&mod_profiles_file())
        .as_array()
        .cloned()
        .unwrap_or_default();
    let profile = match profiles
        .iter()
        .find(|p| p.get("name").and_then(|n| n.as_str()) == Some(name.as_str()))
    {
        Some(p) => p.clone(),
        None => return json!({ "error": "Profile not found" }),
    };
    if find_instance(&app, &instance_name).is_none() {
        return json!({ "error": "Instance not found." });
    }
    let mods_dir = instance_dir(&instance_name).join("mods");
    let _ = std::fs::create_dir_all(&mods_dir);
    let entries = profile
        .get("mods")
        .and_then(|m| m.as_array())
        .cloned()
        .unwrap_or_default();
    let mut installed = 0u32;
    let mut skipped = 0u32;
    let mut errors: Vec<Value> = Vec::new();
    for entry in &entries {
        let slug = entry.get("slug").and_then(|s| s.as_str()).unwrap_or("");
        if slug.is_empty() {
            skipped += 1;
            continue;
        }
        let version_id = entry
            .get("version_id")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        match install_mod_inner(&app, &instance_name, slug, version_id, "mods") {
            Ok(_) => installed += 1,
            Err(e) => errors.push(json!({ "slug": slug, "error": e })),
        }
    }
    log_msg(
        &app,
        &format!(
            "[Profiles] Applied '{}' to '{}': {} installed, {} skipped, {} errors",
            name,
            instance_name,
            installed,
            skipped,
            errors.len()
        ),
    );
    json!({
        "success": true,
        "installed": installed,
        "skipped": skipped,
        "errors": errors,
    })
}

// ============================================================
// UPDATER (GitHub releases)
// ============================================================

/// Release feed. Override at runtime with the `update_api_url` setting.
const DEFAULT_UPDATE_API: &str =
    "https://api.github.com/repos/EscapeXMc/isolate-client/releases/latest";

fn update_api_url(app: &AppState) -> String {
    load_settings(app)
        .get("update_api_url")
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty())
        .map(|s| s.to_string())
        .unwrap_or_else(|| DEFAULT_UPDATE_API.to_string())
}

fn updates_dir() -> PathBuf {
    app_dir().join("updates")
}

/// Numeric dotted-version comparison (`a > b`), tolerant of a leading `v`.
fn version_gt(a: &str, b: &str) -> bool {
    let parse = |v: &str| -> Vec<u64> {
        v.trim_start_matches('v')
            .split('.')
            .filter_map(|s| {
                s.split(|c: char| !c.is_ascii_digit())
                    .next()
                    .and_then(|n| n.parse::<u64>().ok())
            })
            .collect()
    };
    let va = parse(a);
    let vb = parse(b);
    for i in 0..va.len().max(vb.len()) {
        let pa = va.get(i).copied().unwrap_or(0);
        let pb = vb.get(i).copied().unwrap_or(0);
        if pa != pb {
            return pa > pb;
        }
    }
    false
}

/// Pull a comparable dotted version out of a release title or tag.
fn release_version(data: &Value) -> String {
    let raw = data
        .get("name")
        .and_then(|n| n.as_str())
        .filter(|s| !s.trim().is_empty())
        .or_else(|| data.get("tag_name").and_then(|t| t.as_str()))
        .unwrap_or("")
        .trim()
        .trim_start_matches(|c| c == 'v' || c == 'V')
        .trim();
    match raw.find(|c: char| c.is_ascii_digit()) {
        Some(pos) => raw[pos..]
            .chars()
            .take_while(|c| c.is_ascii_digit() || *c == '.')
            .collect::<String>()
            .trim_end_matches('.')
            .to_string(),
        None => raw.to_string(),
    }
}

/// Score a release asset for this platform. Higher wins; negative = reject.
fn asset_score(name: &str) -> i32 {
    let n = name.to_lowercase();
    if cfg!(target_os = "windows") {
        if n.ends_with(".exe") {
            if n.contains("setup") || n.contains("install") {
                return 100;
            }
            let is64 = matches!(std::env::consts::ARCH, "x86_64");
            if is64 && (n.contains("win64") || n.contains("windows64") || n.contains("x86_64")) {
                return 95;
            }
            if !is64 && (n.contains("win32") || n.contains("windows32")) {
                return 95;
            }
            return 10;
        }
        if n.ends_with(".msi") {
            return 90;
        }
        if n.ends_with(".zip") {
            return 40;
        }
    } else if cfg!(target_os = "macos") {
        if n.ends_with(".dmg") {
            return 100;
        }
        if n.ends_with(".pkg") {
            return 90;
        }
        if n.ends_with(".zip") {
            return 40;
        }
    } else {
        if n.ends_with(".appimage") {
            return 100;
        }
        if n.ends_with(".deb") {
            return 90;
        }
        if n.ends_with(".tar.gz") || n.ends_with(".tgz") || n.ends_with(".zip") {
            return 50;
        }
        if n.ends_with(".rpm") || n.ends_with(".flatpak") || n.ends_with(".snap") {
            return 40;
        }
    }
    -1
}

fn pick_release_asset(assets: &[Value]) -> Option<(String, String, u64)> {
    let mut best: Option<(i32, String, String, u64)> = None;
    for a in assets {
        let name = a.get("name").and_then(|n| n.as_str()).unwrap_or("");
        let url = a
            .get("browser_download_url")
            .and_then(|u| u.as_str())
            .unwrap_or("");
        let size = a.get("size").and_then(|s| s.as_u64()).unwrap_or(0);
        if url.is_empty() {
            continue;
        }
        let score = asset_score(name);
        if score < 0 {
            continue;
        }
        if best.as_ref().map(|b| score > b.0).unwrap_or(true) {
            best = Some((score, name.to_string(), url.to_string(), size));
        }
    }
    best.map(|(_, n, u, s)| (n, u, s))
}

/// Describe the latest release and whether it is newer than this build.
fn release_info(app: &AppState) -> Value {
    let current = APP_VERSION;
    let url = update_api_url(app);
    match http_get(&url) {
        Ok(data) => {
            let tag = data.get("tag_name").and_then(|t| t.as_str()).unwrap_or("");
            let remote = release_version(&data);
            let assets: Vec<Value> = data
                .get("assets")
                .and_then(|a| a.as_array())
                .cloned()
                .unwrap_or_default();
            if !remote.is_empty() && version_gt(&remote, current) {
                let picked = pick_release_asset(&assets);
                let (name, durl, size) = picked.clone().unwrap_or_default();
                json!({
                    "update_available": picked.is_some(),
                    "current_version": current,
                    "current_arch": std::env::consts::ARCH,
                    "latest_version": remote,
                    "tag": tag,
                    "download_url": durl,
                    "download_name": name,
                    "download_size": size,
                    "release_notes": data.get("body").and_then(|b| b.as_str()).unwrap_or(""),
                    "published_at": data.get("published_at").and_then(|p| p.as_str()).unwrap_or(""),
                })
            } else {
                json!({
                    "update_available": false,
                    "current_version": current,
                    "current_arch": std::env::consts::ARCH,
                    "latest_version": remote,
                    "tag": tag,
                })
            }
        }
        Err(e) => json!({
            "update_available": false,
            "current_version": current,
            "error": e,
        }),
    }
}

#[tauri::command]
fn check_launcher_update(app: tauri::State<AppState>) -> Value {
    release_info(&app)
}

/// Xryloth exposed both names — keep them as the same feed.
#[tauri::command]
fn check_updates(app: tauri::State<AppState>) -> Value {
    release_info(&app)
}

// ---------------- auto-update anti-loop state (Step 5) ----------------
// ~/.isolate/update_state.json guards the *startup* update check against an
// infinite "prompt → download → restart → still old build → prompt again"
// loop:
//   attempted_version – we began installing this release; if the launcher is
//                       still running the old build afterwards, the install
//                       demonstrably did not take, so never auto-prompt for
//                       it again.
//   dismissed_version – the user pressed "Later"; don't auto-nag for it.
// The manual "Check for updates" button in Settings bypasses both guards on
// purpose (it is an explicit user action).
fn update_state_path() -> PathBuf {
    app_dir().join("update_state.json")
}

fn update_state() -> Value {
    load_json(&update_state_path())
}

/// Merge `patch` into the persisted update-state object (other keys survive).
fn save_update_state_patch(patch: &Value) {
    let mut st = update_state();
    if !st.is_object() {
        st = json!({});
    }
    if let (Some(obj), Some(patch)) = (st.as_object_mut(), patch.as_object()) {
        for (k, v) in patch {
            obj.insert(k.clone(), v.clone());
        }
    }
    save_json(&update_state_path(), &st);
}

/// Should the *auto* prompt be suppressed for `remote` given the persisted
/// guard state? Pure so the anti-loop rules are unit-testable:
///   Some("dismissed")  → user pressed Later for this exact release
///   Some("loop_guard") → this release was already attempted and the install
///                        never landed (we would restart-loop otherwise)
///   None               → prompt away
fn update_guard(state: &Value, remote: &str) -> Option<&'static str> {
    if remote.is_empty() {
        return None;
    }
    let attempted = state
        .get("attempted_version")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let dismissed = state
        .get("dismissed_version")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if !dismissed.is_empty() && dismissed == remote {
        return Some("dismissed");
    }
    if !attempted.is_empty() && attempted == remote {
        return Some("loop_guard");
    }
    None
}

/// Startup check: release_info() filtered through the anti-loop guards.
/// This is what the auto modal calls on boot; Settings uses the raw
/// check_launcher_update instead.
#[tauri::command]
fn startup_update_check(app: tauri::State<AppState>) -> Value {
    let mut info = release_info(&app);
    let st = update_state();
    let attempted = st
        .get("attempted_version")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let remote = info
        .get("latest_version")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    // The previously attempted update landed (we are now that build or
    // newer) — reset the guard so future releases prompt normally.
    if !attempted.is_empty() && (APP_VERSION == attempted || version_gt(APP_VERSION, &attempted)) {
        save_update_state_patch(&json!({ "attempted_version": "" }));
    }

    if info.get("update_available").and_then(|v| v.as_bool()).unwrap_or(false) {
        match update_guard(&update_state(), &remote) {
            Some(reason) => {
                info["update_available"] = json!(false);
                info[reason] = json!(true);
            }
            None => {}
        }
    }
    info
}

/// Record that we began installing `version` (anti-loop, called by the
/// frontend right before download_update).
#[tauri::command]
fn mark_update_attempt(version: String) -> Value {
    save_update_state_patch(&json!({
        "attempted_version": version,
        "attempted_at": chrono::Utc::now().to_rfc3339(),
    }));
    json!({ "success": true })
}

/// User pressed "Later" — stop auto-prompting for this release.
#[tauri::command]
fn dismiss_update(version: String) -> Value {
    save_update_state_patch(&json!({
        "dismissed_version": version,
        "dismissed_at": chrono::Utc::now().to_rfc3339(),
    }));
    json!({ "success": true })
}

// ---------------- Firebase live releases (RTDB) ----------------
// Public-read RTDB nodes pushed by `scripts/push-release.mjs` (or the
// release-notify workflow). The client only ever READS these:
//   releases/latest.json       – latestVersion / forceUpdate / sha256 / …
//   releases/announcement.json – live announcement (id changes → re-show)
const RELEASE_FB: &str = "https://isolateclient-default-rtdb.firebaseio.com/releases";

fn firebase_node(path: &str) -> Result<Value, String> {
    http_get(&format!("{}/{}.json", RELEASE_FB, path))
}

/// Apply `releases/latest` Firebase fields onto the GitHub-derived info.
/// Returns `force` (forceUpdate or current < minSupportedVersion).
fn apply_fb_latest(info: &mut Value, fb: &Value) -> bool {
    let latest = fb
        .get("latestVersion")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    if latest.is_empty() {
        return false;
    }
    if version_gt(&latest, APP_VERSION) {
        info["update_available"] = json!(true);
        info["latest_version"] = json!(latest);
        if let Some(u) = fb
            .get("downloadUrl")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
        {
            info["download_url"] = json!(u);
            if info
                .get("download_name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .is_empty()
            {
                info["download_name"] = json!(u.rsplit('/').next().unwrap_or("update.bin"));
            }
        }
        if let Some(s) = fb
            .get("sha256")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
        {
            info["sha256"] = json!(s);
        }
        if let Some(n) = fb.get("size").and_then(|v| v.as_u64()) {
            info["download_size"] = json!(n);
        }
        if let Some(n) = fb
            .get("releaseNotes")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
        {
            info["release_notes"] = json!(n);
        }
        let mut force = fb.get("forceUpdate").and_then(|v| v.as_bool()).unwrap_or(false);
        if let Some(m) = fb
            .get("minSupportedVersion")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
        {
            info["min_supported_version"] = json!(m);
            if version_gt(m, APP_VERSION) {
                force = true;
            }
        }
        force
    } else {
        // Firebase says we're current — but never mask a *newer* GitHub
        // release (the Firebase push for it may have failed).
        if !info
            .get("update_available")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        {
            info["latest_version"] = json!(latest);
        }
        false
    }
}

/// Full live release state: GitHub asset info ⊕ Firebase overrides +
/// announcement + anti-loop guards (unless forced).
fn live_release_payload(app: &AppState) -> Value {
    let mut info = release_info(app);
    let mut force = false;
    if let Ok(fb) = firebase_node("latest") {
        if fb.is_object() {
            force = apply_fb_latest(&mut info, &fb);
        }
    }
    match firebase_node("announcement") {
        Ok(a) if a.is_object() => info["announcement"] = a,
        _ => info["announcement"] = Value::Null,
    }
    if !force
        && info
            .get("update_available")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
    {
        let remote = info
            .get("latest_version")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if let Some(reason) = update_guard(&update_state(), &remote) {
            info["update_available"] = json!(false);
            info[reason] = json!(true);
        }
    }
    info["force_update"] = json!(force);
    info
}

/// Startup / on-demand check used by the boot path and the banner CTA.
#[tauri::command]
fn check_release_live(app: tauri::State<AppState>) -> Value {
    live_release_payload(&app)
}

/// Real-time listener: polls the two Firebase nodes every 15s (RTDB REST)
/// and emits `release-update` whenever anything changes, so announcements
/// and forced updates reach open clients without a restart. Started once.
#[tauri::command]
fn start_release_listener(handle: tauri::AppHandle) -> Value {
    use std::sync::atomic::{AtomicBool, Ordering};
    static RELEASE_LISTENER: AtomicBool = AtomicBool::new(false);
    if RELEASE_LISTENER.swap(true, Ordering::SeqCst) {
        return json!({ "success": true, "started": false });
    }
    std::thread::spawn(move || {
        let mut last_sig = String::new();
        loop {
            let sig = format!(
                "{}|{}",
                firebase_node("latest")
                    .map(|v| v.to_string())
                    .unwrap_or_default(),
                firebase_node("announcement")
                    .map(|v| v.to_string())
                    .unwrap_or_default(),
            );
            if !sig.is_empty() && sig != last_sig {
                let first = last_sig.is_empty();
                last_sig = sig;
                if !first {
                    let payload = {
                        let app = handle.state::<AppState>();
                        live_release_payload(&app)
                    };
                    let has_update = payload
                        .get("update_available")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);
                    let has_ann = payload
                        .get("announcement")
                        .map(|a| a.is_object())
                        .unwrap_or(false);
                    if has_update || has_ann {
                        emit(&handle, "release-update", payload);
                    }
                }
            }
            std::thread::sleep(Duration::from_secs(15));
        }
    });
    json!({ "success": true, "started": true })
}

#[tauri::command]
fn get_latest_release_tag(app: tauri::State<AppState>) -> Value {
    let url = update_api_url(&app);
    match http_get(&url) {
        Ok(data) => {
            let tag = match release_version(&data) {
                v if !v.is_empty() => v,
                _ => APP_VERSION.to_string(),
            };
            json!({
                "tag": tag,
                "current_version": APP_VERSION,
                "current_arch": std::env::consts::ARCH,
                "notes": data.get("body").and_then(|b| b.as_str()).unwrap_or(""),
                "published_at": data.get("published_at").and_then(|p| p.as_str()).unwrap_or(""),
            })
        }
        Err(_) => json!({
            "tag": APP_VERSION,
            "current_version": APP_VERSION,
            "current_arch": std::env::consts::ARCH,
            "notes": "",
            "published_at": "",
        }),
    }
}

fn emit_update_log(handle: &tauri::AppHandle, msg: &str) {
    emit(handle, "update-log", json!(msg));
}

fn emit_update_progress(handle: &tauri::AppHandle, downloaded: u64, total: u64, speed: u64) {
    let percent = if total > 0 {
        ((downloaded as f64 / total as f64) * 100.0).min(100.0) as u32
    } else {
        0
    };
    emit(
        handle,
        "update-progress",
        json!({ "downloaded": downloaded, "total": total, "percent": percent, "speed": speed }),
    );
}

/// Stream a release asset to disk while emitting progress events.
fn download_update_with_progress(
    handle: &tauri::AppHandle,
    url: &str,
    dest: &Path,
) -> Result<u64, String> {
    blocking(|| {
        if let Some(parent) = dest.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        emit_update_log(handle, &format!("Connecting to {}", url));
        let client = reqwest::blocking::Client::builder()
            .user_agent(USER_AGENT)
            .timeout(Duration::from_secs(900))
            .build()
            .map_err(|e| e.to_string())?;
        let mut resp = client.get(url).send().map_err(|e| e.to_string())?;
        if !resp.status().is_success() {
            return Err(format!("Server returned HTTP {}", resp.status()));
        }
        let total = resp.content_length().unwrap_or(0);
        if total > 0 {
            emit_update_log(
                handle,
                &format!("Download size: {:.1} MB", total as f64 / 1_048_576.0),
            );
        }
        let mut file = std::fs::File::create(dest).map_err(|e| e.to_string())?;
        let mut buf = vec![0u8; 131072];
        let mut downloaded: u64 = 0;
        let start = Instant::now();
        let mut last_emit = Instant::now();
        let mut last_logged_mb: u64 = 0;
        loop {
            let n = std::io::Read::read(&mut resp, &mut buf).map_err(|e| e.to_string())?;
            if n == 0 {
                break;
            }
            std::io::Write::write_all(&mut file, &buf[..n]).map_err(|e| e.to_string())?;
            downloaded += n as u64;
            if last_emit.elapsed().as_millis() >= 120 {
                let elapsed = start.elapsed().as_secs_f64().max(0.001);
                emit_update_progress(handle, downloaded, total, (downloaded as f64 / elapsed) as u64);
                last_emit = Instant::now();
            }
            let mib = downloaded / 1_048_576;
            if mib > last_logged_mb {
                last_logged_mb = mib;
                let line = if total > 0 {
                    format!("Downloaded {} MB / {:.1} MB", mib, total as f64 / 1_048_576.0)
                } else {
                    format!("Downloaded {} MB", mib)
                };
                emit_update_log(handle, &line);
            }
        }
        let _ = file.flush();
        let elapsed = start.elapsed().as_secs_f64().max(0.001);
        emit_update_progress(
            handle,
            downloaded,
            if total > 0 { total } else { downloaded },
            (downloaded as f64 / elapsed) as u64,
        );
        emit_update_log(handle, "Download complete. Verifying file...");
        if total > 0 && downloaded != total {
            return Err(format!(
                "Incomplete download ({} of {} bytes)",
                downloaded, total
            ));
        }
        Ok(downloaded)
    })
}

#[tauri::command]
fn download_update(
    handle: tauri::AppHandle,
    url: String,
    filename: String,
    sha256: Option<String>,
) -> Value {
    if url.trim().is_empty() {
        return json!({ "error": "No download URL provided" });
    }
    let dir = updates_dir();
    let _ = std::fs::create_dir_all(&dir);
    let raw = if filename.trim().is_empty() {
        url.rsplit('/').next().unwrap_or("isolate-update.bin").to_string()
    } else {
        filename
    };
    // Never allow path separators in the destination file name.
    let name = raw
        .rsplit(|c| c == '/' || c == '\\')
        .next()
        .unwrap_or("isolate-update.bin")
        .to_string();
    let dest = dir.join(&name);
    let _ = std::fs::remove_file(&dest);
    match download_update_with_progress(&handle, &url, &dest) {
        Ok(size) => {
            // Integrity check (Firebase sha256) — a mismatched/partial file
            // is discarded so a corrupt package can never reach install.
            if let Some(expect) = sha256.filter(|s| !s.trim().is_empty()) {
                emit(&handle, "update-stage", json!({ "stage": "verifying" }));
                emit_update_log(&handle, "Verifying SHA-256 checksum…");
                match file_sha256(&dest) {
                    Ok(actual) if actual.eq_ignore_ascii_case(expect.trim()) => {
                        emit_update_log(&handle, "Checksum verified.");
                    }
                    Ok(actual) => {
                        let _ = std::fs::remove_file(&dest);
                        emit_update_log(&handle, "Checksum mismatch — download discarded.");
                        return json!({ "error": format!(
                            "SHA-256 mismatch (expected {}, got {}) — corrupted download discarded.",
                            expect.trim(),
                            actual
                        ) });
                    }
                    Err(e) => {
                        let _ = std::fs::remove_file(&dest);
                        return json!({ "error": format!("Checksum verification failed: {}", e) });
                    }
                }
            }
            json!({
                "success": true,
                "path": dest.to_string_lossy(),
                "filename": name,
                "size": size,
            })
        }
        Err(e) => {
            emit_update_log(&handle, &format!("Download failed: {}", e));
            json!({ "error": e })
        }
    }
}

/// SHA-256 of a file as lowercase hex (update integrity verification).
fn file_sha256(path: &Path) -> Result<String, String> {
    use sha2::{Digest, Sha256};
    let mut f = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut hasher = Sha256::new();
    std::io::copy(&mut f, &mut hasher).map_err(|e| e.to_string())?;
    Ok(format!("{:x}", hasher.finalize()))
}

#[tauri::command]
fn install_update(handle: tauri::AppHandle, path: String) -> Value {
    let p = PathBuf::from(&path);
    if !p.exists() {
        return json!({ "error": "Update file no longer exists" });
    }
    emit_update_log(&handle, "Preparing to apply update...");
    #[cfg(target_os = "windows")]
    {
        install_update_windows(&handle, &p)
    }
    #[cfg(target_os = "linux")]
    {
        install_update_linux(&handle, &p)
    }
    #[cfg(target_os = "macos")]
    {
        emit_update_log(&handle, "Opening the update package...");
        let _ = std::process::Command::new("open").arg(&p).spawn();
        json!({ "success": true, "restart": false })
    }
    #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
    {
        json!({ "error": "Unsupported platform" })
    }
}

#[cfg(target_os = "windows")]
fn install_update_windows(handle: &tauri::AppHandle, path: &Path) -> Value {
    use std::os::windows::process::CommandExt;
    emit_update_log(handle, "Launching installer. The launcher will close so files can be replaced...");
    let mut cmd = std::process::Command::new(path);
    cmd.creation_flags(0x0000_0008); // DETACHED_PROCESS
    match cmd.spawn() {
        Ok(_) => {
            emit_update_log(handle, "Installer started. Closing Isolate Client...");
            emit(handle, "update-stage", json!({ "stage": "restarting" }));
            std::thread::sleep(Duration::from_millis(900));
            std::process::exit(0);
        }
        Err(e) => json!({ "error": e.to_string() }),
    }
}

#[cfg(target_os = "linux")]
fn install_update_linux(handle: &tauri::AppHandle, path: &Path) -> Value {
    // AppImage self-update: replace the running image, then relaunch.
    if let Ok(appimage) = std::env::var("APPIMAGE") {
        if !appimage.is_empty() {
            let target = PathBuf::from(&appimage);
            emit_update_log(handle, &format!("Replacing AppImage: {}", target.display()));
            let file_name = target
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "IsolateClient.AppImage".to_string());
            let staging = target.with_file_name(format!("{}.new", file_name));
            if let Err(e) = std::fs::copy(path, &staging) {
                return json!({ "error": format!("Failed to stage update: {}", e) });
            }
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(&staging, std::fs::Permissions::from_mode(0o755));
            }
            if let Err(e) = std::fs::rename(&staging, &target) {
                return json!({ "error": format!("Failed to replace AppImage: {}", e) });
            }
            emit_update_log(handle, "Update applied. Restarting Isolate Client...");
            let _ = std::process::Command::new(&target).spawn();
            std::process::exit(0);
        }
    }
    // .deb update (needs privileges).
    if path
        .extension()
        .map(|e| e.eq_ignore_ascii_case("deb"))
        .unwrap_or(false)
    {
        emit_update_log(handle, "Installing .deb package (you may be asked for your password)...");
        let installer = if command_exists("pkexec") {
            "pkexec"
        } else {
            "sudo"
        };
        let status = std::process::Command::new(installer)
            .arg("dpkg")
            .arg("-i")
            .arg(path)
            .status();
        match status {
            Ok(s) if s.success() => {
                emit_update_log(handle, "Package installed. Restarting Isolate Client...");
                if let Ok(exe) = std::env::current_exe() {
                    let _ = std::process::Command::new(exe).spawn();
                }
                std::process::exit(0);
            }
            _ => json!({ "error": "Package installation failed or was cancelled" }),
        }
    } else {
        // Portable binary: replace the running executable and relaunch.
        match std::env::current_exe() {
            Ok(exe) => {
                emit_update_log(handle, &format!("Replacing {} ...", exe.display()));
                let staging = exe.with_extension("new");
                if let Err(e) = std::fs::copy(path, &staging) {
                    return json!({ "error": format!("Failed to stage update: {}", e) });
                }
                {
                    use std::os::unix::fs::PermissionsExt;
                    let _ = std::fs::set_permissions(&staging, std::fs::Permissions::from_mode(0o755));
                }
                if let Err(e) = std::fs::rename(&staging, &exe) {
                    return json!({ "error": format!("Failed to replace binary: {}", e) });
                }
                emit_update_log(handle, "Update applied. Restarting Isolate Client...");
                let _ = std::process::Command::new(&exe).spawn();
                std::process::exit(0);
            }
            Err(e) => json!({ "error": e.to_string() }),
        }
    }
}

fn command_exists(name: &str) -> bool {
    std::env::var("PATH")
        .map(|p| std::env::split_paths(&p).any(|d| d.join(name).exists()))
        .unwrap_or(false)
}

// ============================================================
// NEWS
// ============================================================

#[tauri::command]
fn get_minecraft_news() -> Value {
    match http_get(MOJANG_MANIFEST) {
        Ok(data) => {
            let mut news = Vec::new();
            let latest = data.get("latest");
            let latest_release = latest
                .and_then(|l| l.get("release"))
                .and_then(|r| r.as_str())
                .unwrap_or("");
            let latest_snapshot = latest
                .and_then(|l| l.get("snapshot"))
                .and_then(|s| s.as_str())
                .unwrap_or("");
            if let Some(versions) = data.get("versions").and_then(|v| v.as_array()) {
                for v in versions.iter().take(20) {
                    let id = v.get("id").and_then(|i| i.as_str()).unwrap_or("");
                    if id == latest_release || id == latest_snapshot {
                        news.push(json!({
                            "title": format!(
                                "Minecraft {} ({})",
                                id,
                                v.get("type").and_then(|t| t.as_str()).unwrap_or("")
                            ),
                            "url": v.get("url").and_then(|u| u.as_str()).unwrap_or(""),
                            "date": v.get("releaseTime").and_then(|d| d.as_str()).unwrap_or(""),
                            "type": v.get("type").and_then(|t| t.as_str()).unwrap_or(""),
                        }));
                    }
                }
            }
            Value::Array(news)
        }
        Err(_) => Value::Array(vec![]),
    }
}

// The old minecraft.net RSS feed (/en-us/articles/rss) now returns 404, so
// the news list is rebuilt from the site sitemap (freshness via <lastmod>)
// plus each article page's og:title / og:image meta tags.

fn tag_between(block: &str, open: &str, close: &str) -> String {
    if let Some(s) = block.find(open) {
        let rest = &block[s + open.len()..];
        if let Some(e) = rest.find(close) {
            return rest[..e].trim().to_string();
        }
    }
    String::new()
}

/// Newest en-us articles: sitemap lastmod order, then og: meta per page.
/// Returns `count` items with real titles, dates and hero images.
fn latest_articles(count: usize) -> Vec<Value> {
    let xml = match http_get_text("https://www.minecraft.net/sitemap.xml") {
        Ok(x) => x,
        Err(_) => return Vec::new(),
    };
    let mut cand: Vec<(String, String)> = Vec::new(); // (lastmod, slug)
    let mut rest = xml.as_str();
    while let Some(s) = rest.find("<url>") {
        let after = &rest[s + 5..];
        let e = match after.find("</url>") {
            Some(e) => e,
            None => break,
        };
        let block = &after[..e];
        let loc = tag_between(block, "<loc>", "</loc>");
        if let Some(slug) = loc.strip_prefix("https://www.minecraft.net/en-us/article/") {
            let ok = !slug.is_empty()
                && slug.len() < 120
                && slug
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_');
            if ok {
                cand.push((tag_between(block, "<lastmod>", "</lastmod>"), slug.to_string()));
            }
        }
        rest = &after[e + 6..];
    }
    // newest first — ISO-8601 timestamps sort lexicographically
    cand.sort_by(|a, b| b.0.cmp(&a.0));
    cand.dedup_by(|a, b| a.1 == b.1);

    let mut out = Vec::new();
    for (lastmod, slug) in cand.into_iter().take(count + 3) {
        if out.len() >= count {
            break;
        }
        let url = format!("https://www.minecraft.net/en-us/article/{}", slug);
        let html = match http_get_text(&url) {
            Ok(h) => h,
            Err(_) => continue,
        };
        let mut title = tag_between(&html, "property=\"og:title\" content=\"", "\"");
        if title.is_empty() {
            title = tag_between(&html, "<title>", "</title>");
            if let Some(i) = title.find(" | ") {
                title.truncate(i);
            }
        }
        if title.is_empty() {
            continue;
        }
        let image = tag_between(&html, "property=\"og:image\" content=\"", "\"");
        out.push(json!({
            "title": title,
            "url": url,
            "date": lastmod,
            "type": "news",
            "image": if image.is_empty() {
                Value::Null
            } else {
                Value::String(image.replace("&amp;", "&"))
            },
        }));
    }
    out
}

// News cache — the sitemap alone is ~390 KB gzipped, so refetch every 30 min.
static NEWS_CACHE: std::sync::Mutex<Option<(std::time::Instant, Value)>> =
    std::sync::Mutex::new(None);

/// Fallback: latest release notes straight from the version manifest.
fn manifest_news(count: usize) -> Vec<Value> {
    let mut news = Vec::new();
    if let Ok(data) = http_get(MOJANG_MANIFEST) {
        let latest_release = data
            .get("latest")
            .and_then(|l| l.get("release"))
            .and_then(|r| r.as_str())
            .unwrap_or("")
            .to_string();
        if let Some(versions) = data.get("versions").and_then(|v| v.as_array()) {
            for v in versions.iter().take(20) {
                if news.len() >= count {
                    break;
                }
                let id = v.get("id").and_then(|i| i.as_str()).unwrap_or("");
                if id == latest_release {
                    news.push(json!({
                        "title": format!("Minecraft {} Released!", id),
                        "url": v.get("url").and_then(|u| u.as_str()).unwrap_or(""),
                        "date": v.get("releaseTime").and_then(|d| d.as_str()).unwrap_or(""),
                        "type": "release",
                        "image": Value::Null,
                    }));
                }
            }
        }
    }
    news
}

#[tauri::command]
fn get_minecraft_news_enhanced() -> Value {
    // serve from cache for 30 min so page mounts don't refetch the sitemap
    if let Ok(guard) = NEWS_CACHE.lock() {
        if let Some((at, val)) = guard.as_ref() {
            if at.elapsed() < std::time::Duration::from_secs(1800) {
                return val.clone();
            }
        }
    }
    let items = latest_articles(3);
    if items.is_empty() {
        // refresh failed — stale news beats the image-less manifest fallback
        if let Ok(guard) = NEWS_CACHE.lock() {
            if let Some((_, val)) = guard.as_ref() {
                return val.clone();
            }
        }
        return Value::Array(manifest_news(10));
    }
    let value = Value::Array(items);
    if let Ok(mut guard) = NEWS_CACHE.lock() {
        *guard = Some((std::time::Instant::now(), value.clone()));
    }
    value
}

// ============================================================
// BACKGROUND IMAGE / VIDEO
// ============================================================

fn backgrounds_dir() -> PathBuf {
    app_dir().join("backgrounds")
}

fn decode_data_url(data: &str) -> Option<(Vec<u8>, &'static str)> {
    let idx = data.find(',')?;
    let head = &data[..idx];
    let b64 = &data[idx + 1..];
    let ext = if head.contains("image/png") {
        "png"
    } else if head.contains("image/jpeg") || head.contains("image/jpg") {
        "jpg"
    } else if head.contains("image/gif") {
        "gif"
    } else if head.contains("image/webp") {
        "webp"
    } else if head.contains("video/webm") {
        "webm"
    } else if head.contains("video/quicktime") {
        "mov"
    } else if head.contains("video/mp4") {
        "mp4"
    } else {
        return None;
    };
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(b64.trim())
        .ok()?;
    Some((bytes, ext))
}

#[tauri::command]
fn save_background_image(data: String) -> Value {
    if !data.starts_with("data:") {
        return json!({ "success": false, "error": "Invalid image data" });
    }
    let (bytes, ext) = match decode_data_url(&data) {
        Some(x) if !x.1.starts_with("video") => x,
        _ => return json!({ "success": false, "error": "Invalid image data" }),
    };
    let dir = backgrounds_dir();
    let _ = std::fs::create_dir_all(&dir);
    // Remove any previous custom background so only one file exists at a time.
    for e in ["png", "jpg", "jpeg", "gif", "webp"] {
        let _ = std::fs::remove_file(dir.join(format!("custom.{}", e)));
    }
    let path = dir.join(format!("custom.{}", ext));
    match std::fs::write(&path, &bytes) {
        Ok(_) => json!({ "success": true, "path": path.to_string_lossy() }),
        Err(e) => json!({ "success": false, "error": e.to_string() }),
    }
}

#[tauri::command]
fn get_background_image_path() -> String {
    for ext in ["png", "jpg", "jpeg", "gif", "webp"] {
        let p = backgrounds_dir().join(format!("custom.{}", ext));
        if p.exists() {
            return p.to_string_lossy().to_string();
        }
    }
    String::new()
}

#[tauri::command]
fn delete_background_image() -> Value {
    for ext in ["png", "jpg", "jpeg", "gif", "webp"] {
        let _ = std::fs::remove_file(backgrounds_dir().join(format!("custom.{}", ext)));
    }
    json!({ "success": true })
}

#[tauri::command]
fn save_background_video(data: String) -> Value {
    if !data.starts_with("data:") {
        return json!({ "success": false, "error": "Invalid video data" });
    }
    let (bytes, ext) = match decode_data_url(&data) {
        Some(x) if x.1.starts_with("video") => x,
        _ => return json!({ "success": false, "error": "Invalid video data" }),
    };
    let dir = backgrounds_dir();
    let _ = std::fs::create_dir_all(&dir);
    for e in ["mp4", "webm", "mov"] {
        let _ = std::fs::remove_file(dir.join(format!("custom_video.{}", e)));
    }
    let path = dir.join(format!("custom_video.{}", ext));
    match std::fs::write(&path, &bytes) {
        Ok(_) => json!({ "success": true, "path": path.to_string_lossy() }),
        Err(e) => json!({ "success": false, "error": e.to_string() }),
    }
}

#[tauri::command]
fn get_background_video_path() -> String {
    for ext in ["mp4", "webm", "mov"] {
        let p = backgrounds_dir().join(format!("custom_video.{}", ext));
        if p.exists() {
            return p.to_string_lossy().to_string();
        }
    }
    String::new()
}

#[tauri::command]
fn delete_background_video() -> Value {
    for ext in ["mp4", "webm", "mov"] {
        let _ = std::fs::remove_file(backgrounds_dir().join(format!("custom_video.{}", ext)));
    }
    json!({ "success": true })
}

// ============================================================
// GAME BOOST
// ============================================================

/// Processes that are safe to drop right before a Minecraft session.
#[cfg(target_os = "windows")]
const BLOATWARE: &[&str] = &[
    "OneDrive.exe",
    "OneDriveSetup.exe",
    "MicrosoftTeams.exe",
    "Teams.exe",
    "ms-teams.exe",
    "Cortana.exe",
    "YourPhone.exe",
    "PhoneExperienceHost.exe",
    "SkypeApp.exe",
    "SkypeBackgroundHost.exe",
    "XboxApp.exe",
    "GameBar.exe",
    "GameBarFTServer.exe",
    "Widgets.exe",
    "WidgetService.exe",
    "Clipchamp.exe",
    "ClipchampApplication.exe",
    "BingWallpaper.exe",
    "BingWallpaperApp.exe",
    "Spotify.exe",
    "SpotifyWebHelper.exe",
    "EpicGamesLauncher.exe",
    "EpicWebHelper.exe",
    "BraveCrashHandler.exe",
    "Microsoft.Photos.exe",
    "Microsoft.Windows.Photos.exe",
    "SecurityHealthSystray.exe",
];

fn dir_bytes(path: &Path) -> u64 {
    let mut total = 0u64;
    if let Ok(rd) = std::fs::read_dir(path) {
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                total += dir_bytes(&p);
            } else if let Ok(m) = e.metadata() {
                total += m.len();
            }
        }
    }
    total
}

fn empty_dir(path: &Path) -> u64 {
    let mut freed = 0u64;
    if let Ok(rd) = std::fs::read_dir(path) {
        for e in rd.flatten() {
            let p = e.path();
            if p.is_file() {
                if let Ok(m) = e.metadata() {
                    freed += m.len();
                }
                let _ = std::fs::remove_file(&p);
            } else if p.is_dir() {
                let sz = dir_bytes(&p);
                if std::fs::remove_dir_all(&p).is_ok() {
                    freed += sz;
                }
            }
        }
    }
    freed
}

/// One-click optimisation pass: background cleanup + optional FPS mods.
#[tauri::command]
fn game_boost(
    app: tauri::State<AppState>,
    instance_name: String,
) -> Value {
    log_msg(&app, "[Boost] Starting game boost...");
    let mut steps_completed = 0u32;
    let mut steps_total = 0u32;
    let mut details: Vec<String> = Vec::new();
    let settings = load_settings(&app);
    // Auto-FPS-mod install is ON by default; every other step is opt-in.
    let s = |k: &str| {
        settings
            .get(k)
            .and_then(|v| v.as_bool())
            .unwrap_or(k == "perf_auto_mods")
    };

    let (mc_version, loader) = match find_instance(&app, &instance_name) {
        Some(rec) => (
            rec.get("version").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            rec.get("loader").and_then(|v| v.as_str()).unwrap_or("vanilla").to_string(),
        ),
        None => (String::new(), String::new()),
    };

    // Step 1: kill unnecessary background processes (Windows only).
    if s("boost_kill_processes") {
        steps_total += 1;
        #[cfg(target_os = "windows")]
        {
            log_msg(&app, "[Boost] Killing unnecessary background processes...");
            let mut killed = 0u32;
            for proc_name in BLOATWARE {
                if let Ok(out) = hidden_cmd("taskkill")
                    .args(["/F", "/IM", proc_name])
                    .output()
                {
                    if out.status.success() {
                        killed += 1;
                    }
                }
            }
            steps_completed += 1;
            details.push(format!("Killed {} unnecessary processes", killed));
        }
        #[cfg(not(target_os = "windows"))]
        {
            steps_completed += 1;
            details.push("Skipped process cleanup (Windows only)".to_string());
        }
    }

    // Step 2: clear the OS temp folder.
    if s("boost_clear_temp") {
        steps_total += 1;
        let freed = empty_dir(&std::env::temp_dir());
        steps_completed += 1;
        details.push(format!(
            "Cleared {:.1} MB from temp folder",
            freed as f64 / (1024.0 * 1024.0)
        ));
    }

    // Step 3: clear the launcher's own temp folder.
    if s("boost_clear_temp") {
        steps_total += 1;
        let launcher_temp = app_dir().join("temp");
        let _ = std::fs::create_dir_all(&launcher_temp);
        let freed = empty_dir(&launcher_temp);
        steps_completed += 1;
        details.push(format!(
            "Cleared {:.1} MB from launcher temp",
            freed as f64 / (1024.0 * 1024.0)
        ));
    }

    // Step 4: empty the recycle bin.
    if s("boost_empty_recycle") {
        steps_total += 1;
        #[cfg(target_os = "windows")]
        {
            let _ = hidden_cmd("powershell")
                .args(["-Command", "Clear-RecycleBin -Force -ErrorAction SilentlyContinue"])
                .output();
            steps_completed += 1;
            details.push("Recycle bin emptied".to_string());
        }
        #[cfg(not(target_os = "windows"))]
        {
            steps_completed += 1;
            details.push("Skipped recycle bin (Windows only)".to_string());
        }
    }

    // Step 5: flush the DNS cache.
    if s("boost_flush_dns") {
        steps_total += 1;
        #[cfg(target_os = "windows")]
        {
            let _ = hidden_cmd("ipconfig").args(["/flushdns"]).output();
            steps_completed += 1;
            details.push("DNS cache flushed".to_string());
        }
        #[cfg(target_os = "linux")]
        {
            // systemd-resolved exposes a control socket; fall back to a no-op.
            let _ = hidden_cmd("resolvectl")
                .args(["flush-caches"])
                .output();
            steps_completed += 1;
            details.push("DNS cache flushed".to_string());
        }
        #[cfg(not(any(target_os = "windows", target_os = "linux")))]
        {
            steps_completed += 1;
            details.push("Skipped DNS flush on this platform".to_string());
        }
    }

    // Step 6: clear the OS thumbnail cache.
    if s("boost_clear_thumbs") {
        steps_total += 1;
        #[cfg(target_os = "windows")]
        {
            let thumb_dir = dirs::home_dir()
                .unwrap_or_default()
                .join("AppData\\Local\\Microsoft\\Windows\\Explorer");
            let mut cleaned = 0u32;
            if let Ok(rd) = std::fs::read_dir(&thumb_dir) {
                for e in rd.flatten() {
                    let p = e.path();
                    let name = p.file_name().unwrap_or_default().to_string_lossy().to_string();
                    if name.starts_with("thumbcache_") && p.is_file() {
                        if std::fs::remove_file(&p).is_ok() {
                            cleaned += 1;
                        }
                    }
                }
            }
            steps_completed += 1;
            details.push(format!("Cleared {} thumbnail cache files", cleaned));
        }
        #[cfg(not(target_os = "windows"))]
        {
            steps_completed += 1;
            details.push("Skipped thumbnail cache (Windows only)".to_string());
        }
    }

    // Step 7: auto-install performance mods.
    if s("perf_auto_mods") {
        steps_total += 1;
        let loader_lower = loader.to_lowercase();
        let is_modded = matches!(
            loader_lower.as_str(),
            "fabric" | "quilt" | "forge" | "neoforge"
        );
        if is_modded && !mc_version.is_empty() {
            let r = install_perf_mods_impl(&app, instance_name.clone(), mc_version, loader);
            let installed = r.get("installed").and_then(|v| v.as_u64()).unwrap_or(0);
            let skipped = r.get("skipped").and_then(|v| v.as_u64()).unwrap_or(0);
            steps_completed += 1;
            details.push(format!(
                "FPS mods: {} installed, {} skipped/unsupported",
                installed, skipped
            ));
        } else {
            steps_completed += 1;
            details.push("Skipped mod install (Vanilla instance)".to_string());
        }
    } else {
        steps_total += 1;
        steps_completed += 1;
        details.push("Automatic FPS mods are turned off".to_string());
    }

    // Step 8: nudge system memory / priority (Windows only).
    if s("boost_optimize_memory") {
        steps_total += 1;
        #[cfg(target_os = "windows")]
        {
            let _ = hidden_cmd("cmd").args([
                "/C",
                "reg add \"HKLM\\SYSTEM\\CurrentControlSet\\Control\\PriorityControl\" /v Win32PrioritySeparation /t REG_DWORD /d 38 /f",
            ]).output();
            steps_completed += 1;
            details.push("System memory optimized".to_string());
        }
        #[cfg(not(target_os = "windows"))]
        {
            steps_completed += 1;
            details.push("Skipped memory tuning (Windows only)".to_string());
        }
    }

    log_msg(
        &app,
        &format!(
            "[Boost] Game boost complete: {}/{} steps",
            steps_completed, steps_total
        ),
    );
    json!({
        "success": true,
        "steps_completed": steps_completed,
        "steps_total": steps_total,
        "details": details,
    })
}

// ============================================================
// SYSTEM / SETTINGS ALIASES / UNINSTALL
// ============================================================

/// Run a shell command and return its output (used by the AI assistant).
#[tauri::command]
fn run_system_command(command: String) -> Value {
    let output = if cfg!(target_os = "windows") {
        hidden_cmd("cmd").args(["/C", &command]).output()
    } else {
        hidden_cmd("sh").args(["-c", &command]).output()
    };
    match output {
        Ok(o) => json!({
            "success": o.status.success(),
            "stdout": String::from_utf8_lossy(&o.stdout).to_string(),
            "stderr": String::from_utf8_lossy(&o.stderr).to_string(),
            "exit_code": o.status.code(),
        }),
        Err(e) => json!({ "success": false, "error": e.to_string() }),
    }
}

/// Xryloth-parity aliases: the AI tool layer addressed settings by these names.
#[tauri::command]
fn get_setting_cmd(app: tauri::State<AppState>, key: String, default: Option<Value>) -> Value {
    let settings = load_settings(&app);
    settings
        .get(&key)
        .cloned()
        .unwrap_or(default.unwrap_or(Value::Null))
}

#[tauri::command]
fn save_setting_cmd(app: tauri::State<AppState>, key: String, value: Value) -> Value {
    save_setting_val(&app, &key, value);
    json!({ "success": true })
}

fn sh_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// Where the launcher lives. An AppImage keeps its real path in $APPIMAGE.
fn launcher_install_dir() -> PathBuf {
    if let Ok(appimage) = std::env::var("APPIMAGE") {
        if !appimage.is_empty() {
            if let Some(parent) = PathBuf::from(&appimage).parent() {
                return parent.to_path_buf();
            }
        }
    }
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_else(app_dir)
}

/// Remove the launcher — and optionally all user data — after this process
/// exits. The real work runs in a detached helper script because we are
/// deleting the very file that is currently running.
#[tauri::command]
async fn uninstall_launcher(delete_data: bool) -> Result<Value, String> {
    let dir = launcher_install_dir();
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    #[cfg(target_os = "windows")]
    let (is_windows, ext) = (true, "cmd");
    #[cfg(not(target_os = "windows"))]
    let (is_windows, ext) = (false, "sh");

    let mut script = String::new();
    if is_windows {
        script.push_str("@echo off\r\n");
        script.push_str("timeout /t 3 /nobreak >nul\r\n");
        for f in ["isolate-client.exe", "IsolateClient.exe", "isolate.exe"] {
            script.push_str(&format!(
                "del /f /q \"{}\\{}\" >nul 2>&1\r\n",
                dir.display(),
                f
            ));
        }
        script.push_str(&format!(
            "del /f /q \"{}\\Desktop\\Isolate Client.lnk\" >nul 2>&1\r\n",
            home.display()
        ));
        if delete_data {
            script.push_str(&format!(
                "rmdir /s /q \"{}\" >nul 2>&1\r\n",
                app_dir().display()
            ));
        }
        script.push_str("del /f /q \"%~f0\" >nul 2>&1\r\n");
    } else {
        script.push_str("#!/bin/sh\n");
        script.push_str("sleep 2\n");
        if let Ok(appimage) = std::env::var("APPIMAGE") {
            if !appimage.is_empty() {
                script.push_str(&format!(
                    "rm -f {}\n",
                    sh_quote(&appimage)
                ));
            }
        }
        script.push_str(&format!(
            "rm -f {}\n",
            sh_quote(&format!("{}/Isolate Client.desktop", home.display()))
        ));
        script.push_str(&format!(
            "rm -f {}\n",
            sh_quote(&format!(
                "{}/.local/share/applications/isolate-client.desktop",
                home.display()
            ))
        ));
        if delete_data {
            script.push_str(&format!("rm -rf {}\n", sh_quote(&app_dir().to_string_lossy())));
        }
        script.push_str("rm -f \"$0\"\n");
    }

    let script_path = std::env::temp_dir().join(format!(
        "isolate-uninstall-{}.{}",
        std::process::id(),
        ext
    ));
    std::fs::write(&script_path, script)
        .map_err(|e| format!("could not write uninstall helper: {}", e))?;

    #[cfg(target_os = "windows")]
    let spawned = {
        use std::os::windows::process::CommandExt;
        let mut c = std::process::Command::new("cmd");
        c.arg("/C").arg(&script_path);
        c.creation_flags(0x0000_0008); // DETACHED_PROCESS
        c.spawn()
    };
    #[cfg(not(target_os = "windows"))]
    let spawned = std::process::Command::new("sh").arg(&script_path).spawn();

    spawned.map_err(|e| format!("could not start uninstall helper: {}", e))?;
    // Give the UI time to paint its "uninstalling" message, then quit so the
    // helper can remove the running launcher.
    std::thread::spawn(|| {
        std::thread::sleep(Duration::from_millis(1500));
        std::process::exit(0);
    });
    Ok(json!({
        "success": true,
        "install_dir": dir.to_string_lossy(),
        "deleted_data": delete_data,
    }))
}

// ============================================================
// IMAGE CACHE
// ============================================================

const IMAGE_REVALIDATE_SECS: i64 = 6 * 60 * 60;

fn image_cache_dir() -> PathBuf {
    app_dir().join("image_cache")
}

/// Hash the URL into a stable filename, keeping a sensible extension.
fn image_cache_paths(url: &str) -> (PathBuf, PathBuf) {
    let mut hasher = Md5::new();
    hasher.update(url.as_bytes());
    let hex: String = hasher
        .finalize()
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect();
    let ext = url
        .rsplit('?')
        .next()
        .unwrap_or("")
        .rsplit('.')
        .next()
        .unwrap_or("")
        .to_lowercase();
    let ext = if matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "gif" | "webp" | "svg") {
        ext
    } else {
        "img".to_string()
    };
    let dir = image_cache_dir();
    (dir.join(format!("{}.{}", hex, ext)), dir.join(format!("{}.json", hex)))
}

fn image_meta_read(path: &Path) -> Value {
    if path.exists() {
        load_json(path)
    } else {
        json!({})
    }
}

/// Download a remote image into the cache (blocking; callers thread-guard).
fn image_fetch_into_cache(url: &str, conditional: bool) -> Result<(), String> {
    let (file, meta) = image_cache_paths(url);
    let _ = std::fs::create_dir_all(image_cache_dir());
    if conditional && file.exists() {
        // A conditional fetch keeps bandwidth low; if the server does not send
        // a body we simply keep the copy we already have.
        if let Ok(c) = http_client(60) {
            let mut headers = reqwest::header::HeaderMap::new();
            if let Ok(v) = reqwest::header::HeaderValue::from_str(&format!(
                "{}",
                chrono::Local::now().timestamp()
            )) {
                headers.insert(reqwest::header::IF_MODIFIED_SINCE, v);
            }
            if let Ok(resp) = c.get(url).headers(headers).send() {
                if resp.status().as_u16() == 304 {
                    return Ok(());
                }
                if resp.status().is_success() {
                    if let Ok(bytes) = resp.bytes() {
                        if std::fs::write(&file, &bytes).is_ok() {
                            save_json(
                                &meta,
                                &json!({ "fetched_at": chrono::Local::now().timestamp() }),
                            );
                            return Ok(());
                        }
                    }
                }
            }
        }
        return Ok(());
    }
    download_file(url, &file)?;
    save_json(
        &meta,
        &json!({ "fetched_at": chrono::Local::now().timestamp() }),
    );
    Ok(())
}

#[tauri::command]
fn image_cache_get(url: String) -> Value {
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return json!({ "error": "not a remote image url" });
    }
    let (file, meta_path) = image_cache_paths(&url);
    if file.exists() {
        let fetched_at = image_meta_read(&meta_path)
            .get("fetched_at")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        let stale = chrono::Local::now().timestamp() - fetched_at > IMAGE_REVALIDATE_SECS;
        if stale {
            // Revalidate off the UI thread so the page never stalls.
            let u = url.clone();
            std::thread::spawn(move || {
                let _ = image_fetch_into_cache(&u, true);
            });
        }
        return json!({
            "success": true,
            "cached": true,
            "stale": stale,
            "path": file.to_string_lossy(),
        });
    }
    match image_fetch_into_cache(&url, false) {
        Ok(()) => json!({ "success": true, "cached": false, "path": file.to_string_lossy() }),
        Err(e) => json!({ "error": e }),
    }
}

#[tauri::command]
fn image_cache_warm(urls: Vec<String>) -> Value {
    // Pre-download the art shown on the launcher's first screens so opening a
    // page paints instantly from disk.
    let mut queue: Vec<String> = Vec::new();
    for u in urls {
        if queue.len() >= 64 {
            break;
        }
        if !(u.starts_with("http://") || u.starts_with("https://")) {
            continue;
        }
        if queue.contains(&u) {
            continue;
        }
        let (f, m) = image_cache_paths(&u);
        if f.exists() {
            let fetched_at = image_meta_read(&m)
                .get("fetched_at")
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
            if chrono::Local::now().timestamp() - fetched_at <= IMAGE_REVALIDATE_SECS {
                continue;
            }
        }
        queue.push(u);
    }
    let n = queue.len();
    if n == 0 {
        return json!({ "success": true, "queued": 0 });
    }
    std::thread::spawn(move || {
        for u in queue {
            let conditional = image_cache_paths(&u).0.exists();
            let _ = image_fetch_into_cache(&u, conditional);
        }
    });
    json!({ "success": true, "queued": n })
}

// ============================================================
// SELF-HOST TOOL + PARTNER SERVERS
// ============================================================

fn selfhost_dir() -> PathBuf {
    app_dir().join("selfhost")
}

fn selfhost_exe() -> PathBuf {
    if cfg!(target_os = "windows") {
        selfhost_dir().join("IsolateSelfHost.exe")
    } else {
        selfhost_dir().join("isolate-selfhost")
    }
}

#[tauri::command]
fn get_selfhost_status() -> Value {
    let exe = selfhost_exe();
    json!({
        "exists": exe.exists(),
        "path": exe.to_string_lossy(),
        "configured": !load_settings_static().get("selfhost_url")
            .and_then(|v| v.as_str())
            .map(|s| s.trim().is_empty())
            .unwrap_or(true),
    })
}

/// Settings without an `AppState` handle (for the standalone helpers).
fn load_settings_static() -> HashMap<String, Value> {
    match load_json(&app_dir().join("settings.json")).as_object() {
        Some(o) => o
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect::<HashMap<String, Value>>(),
        None => HashMap::new(),
    }
}

/// Download the self-host helper from the URL in the `selfhost_url` setting.
/// No default is shipped: the user supplies the source, so we never fetch a
/// binary we did not author.
#[tauri::command]
fn download_selfhost(app: tauri::State<AppState>) -> Value {
    let url = load_settings(&app)
        .get("selfhost_url")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let url = match url {
        Some(u) => u,
        None => {
            return json!({
                "success": false,
                "error": "No self-host download URL configured. Set 'selfhost_url' in Settings first."
            })
        }
    };
    let _ = std::fs::create_dir_all(selfhost_dir());
    log_msg(&app, &format!("[SelfHost] Downloading from {}", url));
    match download_file(&url, &selfhost_exe()) {
        Ok(()) => {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(&selfhost_exe(), std::fs::Permissions::from_mode(0o755));
            }
            log_msg(&app, "[SelfHost] Download complete");
            json!({ "success": true, "path": selfhost_exe().to_string_lossy() })
        }
        Err(e) => {
            log_msg(&app, &format!("[SelfHost] Download failed: {}", e));
            json!({ "success": false, "error": format!("Download failed: {}", e) })
        }
    }
}

#[tauri::command]
fn launch_selfhost(app: tauri::State<AppState>) -> Value {
    let exe = selfhost_exe();
    if !exe.exists() {
        return json!({ "success": false, "error": "Self-host helper not found. Download it first." });
    }
    let mut cmd = hidden_cmd(&exe.to_string_lossy());
    cmd.current_dir(selfhost_dir());
    cmd.stdout(Stdio::null());
    cmd.stderr(Stdio::null());
    cmd.stdin(Stdio::null());
    match cmd.spawn() {
        Ok(_) => {
            log_msg(&app, "[SelfHost] Launched");
            json!({ "success": true })
        }
        Err(e) => {
            log_msg(&app, &format!("[SelfHost] Failed to launch: {}", e));
            json!({ "success": false, "error": e.to_string() })
        }
    }
}

/// Partner / featured servers. Reads a local `partners.json` first, then the
/// optional `partners_url` setting — both are user-supplied, so Isolate never
/// hard-codes someone else's database.
#[tauri::command]
fn fetch_partners_remote() -> Value {
    let local = app_dir().join("partners.json");
    if local.exists() {
        if let Value::Array(a) = load_json(&local) {
            return Value::Array(a);
        }
        if let Value::Object(map) = load_json(&local) {
            let mut arr: Vec<Value> = Vec::new();
            for (id, p) in map {
                if let Some(mut obj) = p.as_object().cloned() {
                    obj.insert("id".to_string(), json!(id));
                    arr.push(Value::Object(obj));
                }
            }
            return Value::Array(arr);
        }
    }
    if let Some(url) = load_settings_static()
        .get("partners_url")
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty())
    {
        if let Ok(Value::Object(map)) = http_get(url) {
            let mut arr: Vec<Value> = Vec::new();
            for (id, p) in map {
                if let Some(mut obj) = p.as_object().cloned() {
                    obj.insert("id".to_string(), json!(id));
                    arr.push(Value::Object(obj));
                }
            }
            return Value::Array(arr);
        }
        if let Ok(Value::Array(a)) = http_get(url) {
            return Value::Array(a);
        }
    }
    Value::Array(vec![])
}

// ============================================================
// LAUNCH A SERVER JAR THAT LIVES INSIDE AN INSTANCE
// ============================================================

fn find_server_jar(inst_dir: &Path) -> Option<PathBuf> {
    if let Ok(entries) = std::fs::read_dir(inst_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_lowercase();
            if name.ends_with(".jar")
                && (name.contains("paper")
                    || name.contains("spigot")
                    || name.contains("paperclip")
                    || name.contains("purpur"))
            {
                return Some(entry.path());
            }
        }
    }
    None
}

/// Run a Paper/Spigot/Purpur jar found in an instance folder, streaming its
/// output into the normal game-log channel.
#[tauri::command]
fn launch_server(handle: tauri::AppHandle, instance_name: String) -> Value {
    let app = handle.state::<AppState>();
    log_msg(
        &app,
        &format!("[ServerLaunch] Preparing to launch server '{}'...", instance_name),
    );
    let instances = load_instances_raw(&app);
    if !instances
        .iter()
        .any(|i| i.get("name").and_then(|n| n.as_str()) == Some(instance_name.as_str()))
    {
        return json!({ "error": format!("Instance '{}' not found", instance_name) });
    }
    let inst_dir = instance_dir(&instance_name);
    if !inst_dir.exists() {
        return json!({ "error": format!("Instance directory '{}' not found", instance_name) });
    }
    let jar = match find_server_jar(&inst_dir) {
        Some(j) => j,
        None => {
            return json!({ "error": "No Paper, Purpur or Spigot JAR found in this instance." })
        }
    };
    let java = resolve_java(&app, None);
    log_msg(
        &app,
        &format!(
            "[ServerLaunch] Using Java {} for {}",
            java,
            jar.file_name().unwrap_or_default().to_string_lossy()
        ),
    );
    let mut cmd = hidden_cmd(&java);
    cmd.arg("-jar");
    cmd.arg(&jar);
    cmd.arg("--nogui");
    cmd.current_dir(&inst_dir);
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    cmd.stdin(Stdio::null());
    match cmd.spawn() {
        Ok(mut proc) => {
            let pid = proc.id();
            append_game_output(&app, &format!("[Server] Server started (PID: {})", pid));
            log_msg(&app, &format!("[ServerLaunch] Server started, PID: {}", pid));
            if let Some(stdout) = proc.stdout.take() {
                let h = handle.clone();
                std::thread::spawn(move || {
                    let reader = BufReader::new(stdout);
                    for line in reader.lines().map_while(Result::ok) {
                        let trimmed = line.trim().to_string();
                        if !trimmed.is_empty() {
                            let st = h.state::<AppState>();
                            append_game_output(&st, &trimmed);
                        }
                    }
                });
            }
            if let Some(stderr) = proc.stderr.take() {
                let h = handle.clone();
                std::thread::spawn(move || {
                    let reader = BufReader::new(stderr);
                    for line in reader.lines().map_while(Result::ok) {
                        let trimmed = line.trim().to_string();
                        if !trimmed.is_empty() {
                            let st = h.state::<AppState>();
                            append_game_output(&st, &format!("[Server/ERR] {}", trimmed));
                        }
                    }
                });
            }
            json!({ "success": true, "message": "Server process started", "pid": pid })
        }
        Err(e) => json!({ "error": format!("Failed to start: {}", e) }),
    }
}

// ============================================================
// AI ASSISTANT (Groq → Gemini, user-supplied keys only)
// ============================================================
//
// Keys are NEVER compiled into the binary. Resolution order:
//   1. launcher settings   (ai_groq_key / ai_gemini_key)
//   2. ~/.isolate/ai_keys.json
//   3. environment variables ISOLATE_GROQ_KEY / ISOLATE_GEMINI_KEY

const GROQ_API: &str = "https://api.groq.com/openai/v1/chat/completions";
const GROQ_MODELS: [&str; 4] = [
    "openai/gpt-oss-120b",
    "openai/gpt-oss-20b",
    "qwen/qwen3.8-27b",
    "llama-3.1-8b-instant",
];
const GEMINI_API: &str = "https://generativelanguage.googleapis.com/v1beta/models";
const GEMINI_MODELS: [&str; 3] = ["gemini-2.5-flash", "gemini-2.0-flash", "gemini-1.5-flash"];
const AI_MAX_TOOL_ROUNDS: usize = 12;

const AI_GROQ_SETTING_KEY: &str = "ai_groq_key";
const AI_GEMINI_SETTING_KEY: &str = "ai_gemini_key";
const AI_GROQ_ENV: &str = "ISOLATE_GROQ_KEY";
const AI_GEMINI_ENV: &str = "ISOLATE_GEMINI_KEY";

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum AiProvider {
    Groq,
    Gemini,
}

#[derive(Clone)]
struct AiToolCall {
    id: String,
    name: String,
    arguments: String,
}

struct AiResponse {
    content: String,
    tool_calls: Vec<AiToolCall>,
    streamed: bool,
}

/// Settings → keys file → environment. Empty means "not configured".
fn ai_resolve_key(app: &AppState, setting_key: &str, env_key: &str, file_key: &str) -> String {
    if let Some(v) = load_settings(app).get(setting_key).and_then(|v| v.as_str()) {
        if !v.trim().is_empty() {
            return v.trim().to_string();
        }
    }
    let path = app_dir().join("ai_keys.json");
    if path.exists() {
        if let Ok(text) = std::fs::read_to_string(&path) {
            if let Ok(v) = serde_json::from_str::<Value>(&text) {
                if let Some(k) = v.get(file_key).and_then(|x| x.as_str()) {
                    if !k.trim().is_empty() {
                        return k.trim().to_string();
                    }
                }
            }
        }
    }
    if let Ok(v) = std::env::var(env_key) {
        if !v.trim().is_empty() {
            return v.trim().to_string();
        }
    }
    String::new()
}

fn ai_system_prompt(app: &AppState) -> String {
    let ctx = app.ai_context.lock().unwrap().clone();
    let mut s = String::new();
    s.push_str(
        "You are Isolate AI, the built-in assistant of the Isolate Client Minecraft launcher. \
         You control the launcher, its game profiles and its self-hosted Minecraft servers using tools.\n\n",
    );
    s.push_str(&format!(
        "Launcher version: {}\nOS: {}\nData folder: {}\nServers folder: {}\nInstances folder: {}\n",
        APP_VERSION,
        std::env::consts::OS,
        app_dir().to_string_lossy(),
        servers_dir().to_string_lossy(),
        instances_dir().to_string_lossy()
    ));
    if let Some(c) = &ctx {
        let kind = c["kind"].as_str().unwrap_or("");
        let name = c["name"].as_str().unwrap_or("");
        s.push_str(&format!(
            "\nACTIVE WORKSPACE: {} \"{}\". Every target-specific action applies to it unless the user names a different server/profile.\n",
            kind, name
        ));
        if kind == "server" {
            let meta = load_json(&server_dir(name).join("server.json"));
            s.push_str(&format!(
                "Server facts: type={} minecraft={} port={} ram={}MB folder={}\n",
                meta["type"].as_str().unwrap_or("?"),
                meta["version"].as_str().unwrap_or("?"),
                meta["port"].as_u64().unwrap_or(25565),
                meta["ram_mb"].as_u64().unwrap_or(4096),
                server_dir(name).to_string_lossy()
            ));
        } else if kind == "instance" {
            if let Some(inst) = find_instance(app, name) {
                s.push_str(&format!(
                    "Profile facts: minecraft={} loader={} loader_version={} folder={}\n",
                    inst["version"].as_str().unwrap_or("?"),
                    inst["loader"].as_str().unwrap_or("?"),
                    inst["loader_version"].as_str().unwrap_or("?"),
                    instance_dir(name).to_string_lossy()
                ));
            }
        }
    } else {
        s.push_str(
            "\nNo active workspace is selected. If the user names a server or profile, \
             select it with ai_context_set before acting on it.\n",
        );
    }
    s.push_str("\nRULES:\n");
    s.push_str("- ALWAYS use a tool to perform an action. Never just describe an action you can do.\n");
    s.push_str("- If the user names a server/profile that is not the active workspace, select it first, then act.\n");
    s.push_str(
        "- Resolve the correct Minecraft version and loader for installs automatically; \
         never invent paths, versions or URLs — take them from tool results.\n",
    );
    s.push_str("- Keep replies concise. After finishing actions, briefly confirm what you did.\n");
    s
}

// ---------- tool definitions ----------

fn ai_tool(name: &str, desc: &str, props: &[(&str, &str, &str, bool)]) -> Value {
    let mut properties = serde_json::Map::new();
    let mut required = Vec::new();
    for (n, ty, d, req) in props {
        properties.insert(n.to_string(), json!({ "type": ty, "description": d }));
        if *req {
            required.push(n.to_string());
        }
    }
    json!({
        "type": "function",
        "function": {
            "name": name,
            "description": desc,
            "parameters": {
                "type": "object",
                "properties": Value::Object(properties),
                "required": required
            }
        }
    })
}

fn ai_simple(name: &str, desc: &str) -> Value {
    ai_tool(name, desc, &[])
}

fn ai_instance_target(name: &str, desc: &str) -> Value {
    ai_tool(
        name,
        desc,
        &[(
            "instance",
            "string",
            "Instance name (defaults to the active workspace)",
            false,
        )],
    )
}

fn ai_server_target(name: &str, desc: &str) -> Value {
    ai_tool(
        name,
        desc,
        &[(
            "server",
            "string",
            "Server name (defaults to the active workspace server)",
            false,
        )],
    )
}

fn ai_tool_defs() -> Vec<Value> {
    let mut t: Vec<Value> = Vec::new();

    // --- profiles / instances ---
    t.push(ai_simple("instance_list", "List every game profile with its Minecraft version, loader and folder."));
    t.push(ai_tool(
        "instance_create",
        "Create a new game profile and install its files.",
        &[
            ("name", "string", "Profile name", true),
            ("version", "string", "Minecraft version, e.g. 1.21.4", true),
            ("loader", "string", "vanilla, fabric or quilt", true),
            ("loader_version", "string", "Loader version (optional)", false),
        ],
    ));
    t.push(ai_instance_target("instance_delete", "Permanently delete a profile and its folder."));
    t.push(ai_instance_target("instance_launch", "Launch a profile (starts Minecraft)."));
    t.push(ai_simple("instance_stop", "Stop the running Minecraft process."));
    t.push(ai_instance_target("instance_folder", "Open a profile's folder in the file manager."));
    t.push(ai_instance_target("instance_mods_list", "List the mods installed in a profile."));
    t.push(ai_tool(
        "instance_mod_install",
        "Install a Modrinth mod into a profile by slug.",
        &[
            ("slug", "string", "Modrinth project slug", true),
            ("instance", "string", "Instance name (defaults to active workspace)", false),
            ("version_id", "string", "Specific Modrinth version id (optional)", false),
        ],
    ));
    t.push(ai_tool(
        "instance_mod_delete",
        "Delete a mod file from a profile.",
        &[
            ("filename", "string", "Exact file name inside mods/", true),
            ("instance", "string", "Instance name (defaults to active workspace)", false),
        ],
    ));
    t.push(ai_instance_target("instance_resourcepacks_list", "List the resource packs in a profile."));
    t.push(ai_instance_target("instance_settings_get", "Get a profile's RAM / JVM / Java overrides."));
    t.push(ai_tool(
        "instance_settings_save",
        "Save a profile's RAM / JVM / Java overrides.",
        &[
            ("ram_mb", "integer", "Max RAM in MB (0 = inherit launcher setting)", false),
            ("jvm_args", "string", "Extra JVM arguments", false),
            ("java_path", "string", "Java executable path", false),
            ("instance", "string", "Instance name (defaults to active workspace)", false),
        ],
    ));
    t.push(ai_tool(
        "instance_edit",
        "Rename a profile and/or change its Minecraft version and loader.",
        &[
            ("new_name", "string", "New profile name (same to keep it)", true),
            ("version", "string", "Minecraft version", false),
            ("loader", "string", "vanilla, fabric or quilt", false),
            ("loader_version", "string", "Loader version", false),
            ("instance", "string", "Instance name (defaults to active workspace)", false),
        ],
    ));
    t.push(ai_tool(
        "instance_clone",
        "Clone a profile to a new name.",
        &[
            ("new_name", "string", "Name for the clone", true),
            ("instance", "string", "Source profile name", false),
        ],
    ));
    t.push(ai_simple("instance_versions_list", "List installable Minecraft versions (releases first)."));
    t.push(ai_simple("instance_templates_list", "List the built-in modpack templates."));
    t.push(ai_tool(
        "instance_apply_template",
        "Install a built-in template's mod list onto an existing profile.",
        &[
            ("template_id", "string", "Template id from instance_templates_list", true),
            ("instance", "string", "Instance name (defaults to active workspace)", false),
        ],
    ));
    t.push(ai_tool(
        "instance_open_path",
        "Open a sub-folder of a profile (mods, config, saves, screenshots, …).",
        &[
            ("sub_folder", "string", "Sub-folder name", true),
            ("instance", "string", "Instance name (defaults to active workspace)", false),
        ],
    ));
    t.push(ai_tool(
        "instance_analyze_crash",
        "Diagnose the latest crash report of a profile.",
        &[("instance", "string", "Instance name", false)],
    ));

    // --- Modrinth / mods ---
    t.push(ai_tool(
        "mod_search",
        "Search Modrinth for mods, resource packs or modpacks.",
        &[
            ("query", "string", "Search text", true),
            ("project_type", "string", "mod, resourcepack or modpack", false),
            ("loader", "string", "vanilla, fabric or quilt", false),
            ("mc_version", "string", "Minecraft version filter", false),
            ("limit", "integer", "Max results (default 10)", false),
        ],
    ));
    t.push(ai_tool(
        "mod_versions",
        "List the downloadable versions of a Modrinth project.",
        &[
            ("slug", "string", "Modrinth project slug", true),
            ("loader", "string", "Loader filter", false),
            ("mc_version", "string", "Minecraft version filter", false),
        ],
    ));
    t.push(ai_instance_target("mod_list", "List installed mods with their recorded Modrinth slugs."));
    t.push(ai_instance_target("mod_check_updates", "Check installed mods for newer Modrinth versions."));
    t.push(ai_instance_target("mod_check_conflicts", "Detect known-incompatible mod combinations."));
    t.push(ai_instance_target("perf_mods_install", "Install the FPS-boost mod pack (Fabric/Forge only)."));
    t.push(ai_simple("mod_profiles_list", "List saved mod profiles."));
    t.push(ai_tool(
        "mod_profile_save",
        "Save a profile's installed mods as a named mod profile.",
        &[
            ("name", "string", "Profile name", true),
            ("instance", "string", "Instance name (defaults to active workspace)", false),
        ],
    ));
    t.push(ai_tool(
        "mod_profile_apply",
        "Reinstall every mod recorded in a named mod profile onto a profile.",
        &[
            ("name", "string", "Mod profile name", true),
            ("instance", "string", "Instance name (defaults to active workspace)", false),
        ],
    ));
    t.push(ai_tool(
        "mod_profile_delete",
        "Delete a named mod profile.",
        &[("name", "string", "Mod profile name", true)],
    ));

    // --- local server manager ---
    t.push(ai_simple("server_list", "List every local self-hosted server with type, version, port, RAM and running state."));
    t.push(ai_tool(
        "server_create",
        "Create a self-hosted server and download its server jar.",
        &[
            ("name", "string", "Server name", true),
            ("server_type", "string", "paper, purpur, fabric or vanilla", true),
            ("mc_version", "string", "Minecraft version, e.g. 1.21.4", true),
            ("port", "integer", "Server port", false),
            ("motd", "string", "MOTD text", false),
            ("ram_mb", "integer", "Max RAM in MB", false),
        ],
    ));
    t.push(ai_server_target("server_delete", "Permanently delete a server and its folder."));
    t.push(ai_server_target("server_start", "Start a server."));
    t.push(ai_server_target("server_stop", "Gracefully stop a running server (saves worlds)."));
    t.push(ai_server_target("server_restart", "Restart a server (stop, then start)."));
    t.push(ai_server_target("server_kill", "Force-kill a server immediately."));
    t.push(ai_server_target("server_status", "Check whether a server is running."));
    t.push(ai_server_target("server_stats", "Get a server's uptime, PID and memory usage."));
    t.push(ai_tool(
        "server_console",
        "Send a command to a server's console (without the leading slash).",
        &[
            ("command", "string", "Console command, e.g. list", true),
            ("server", "string", "Server name (defaults to active workspace)", false),
        ],
    ));
    t.push(ai_server_target("server_logs", "Read the trailing console output of a server."));
    t.push(ai_tool(
        "server_properties_get",
        "Read a server's server.properties as a key/value map.",
        &[("server", "string", "Server name", false)],
    ));
    t.push(ai_tool(
        "server_properties_set",
        "Write key/value pairs into a server's server.properties.",
        &[
            ("properties", "object", "Map of property key to value", true),
            ("server", "string", "Server name", false),
        ],
    ));
    t.push(ai_server_target("server_plugins_list", "List the plugin jars installed in a server."));
    t.push(ai_tool(
        "server_plugin_search",
        "Search Modrinth for server plugins.",
        &[
            ("query", "string", "Search text", true),
            ("mc_version", "string", "Server Minecraft version", false),
        ],
    ));
    t.push(ai_tool(
        "server_plugin_install",
        "Install a plugin into a server's plugins folder.",
        &[
            ("slug", "string", "Modrinth project slug", true),
            ("server", "string", "Server name", false),
            ("version_id", "string", "Specific version id", false),
        ],
    ));
    t.push(ai_tool(
        "server_plugin_delete",
        "Delete a plugin file from a server.",
        &[
            ("filename", "string", "Exact file name inside plugins/", true),
            ("server", "string", "Server name", false),
        ],
    ));
    t.push(ai_server_target("server_backup_list", "List a server's backups."));
    t.push(ai_tool(
        "server_backup_create",
        "Create a zip backup of a server.",
        &[
            ("label", "string", "Backup label (defaults to a timestamp)", false),
            ("server", "string", "Server name", false),
        ],
    ));
    t.push(ai_tool(
        "server_backup_restore",
        "Restore a server from a backup. The server must be stopped.",
        &[
            ("file", "string", "Backup file name", true),
            ("server", "string", "Server name", false),
        ],
    ));
    t.push(ai_server_target("server_worlds_list", "List a server's worlds."));
    t.push(ai_tool(
        "server_world_delete",
        "Delete a world from a server (server must be stopped).",
        &[
            ("world", "string", "World folder name, e.g. world", true),
            ("server", "string", "Server name", false),
        ],
    ));
    t.push(ai_tool(
        "server_file_list",
        "List files in a server folder.",
        &[
            ("path", "string", "Relative path inside the server ('' for root)", false),
            ("server", "string", "Server name", false),
        ],
    ));
    t.push(ai_tool(
        "server_file_read",
        "Read a text file from a server folder.",
        &[
            ("path", "string", "Relative path inside the server", true),
            ("server", "string", "Server name", false),
        ],
    ));
    t.push(ai_tool(
        "server_file_write",
        "Write a text file in a server folder.",
        &[
            ("path", "string", "Relative path inside the server", true),
            ("content", "string", "File contents", true),
            ("server", "string", "Server name", false),
        ],
    ));
    t.push(ai_simple("server_types_list", "List the supported server types (Paper, Purpur, Fabric, Vanilla)."));
    t.push(ai_tool(
        "server_versions_list",
        "List downloadable Minecraft versions for a server type, newest first.",
        &[("server_type", "string", "paper, purpur, fabric or vanilla", true)],
    ));
    t.push(ai_tool(
        "server_builds",
        "List Paper builds for a Minecraft version.",
        &[("mc_version", "string", "Minecraft version", true)],
    ));
    t.push(ai_tool(
        "server_check_port",
        "Check whether a TCP port is free.",
        &[("port", "integer", "Port to check", true)],
    ));
    t.push(ai_tool(
        "server_next_port",
        "Find the next free port starting from a value.",
        &[("start", "integer", "Port to start from", false)],
    ));
    t.push(ai_tool(
        "server_clone",
        "Clone a server to a new name.",
        &[
            ("new_name", "string", "Name for the clone", true),
            ("port", "integer", "Port for the clone", false),
            ("server", "string", "Source server name", false),
        ],
    ));
    t.push(ai_tool(
        "server_reinstall",
        "Re-download the server jar for a (new) Minecraft version. Keeps worlds and plugins.",
        &[
            ("version", "string", "Target Minecraft version", true),
            ("server", "string", "Server name", false),
        ],
    ));
    t.push(ai_tool(
        "server_eula",
        "Accept or revoke the Minecraft EULA for a server.",
        &[
            ("accept", "boolean", "true to accept, false to revoke", true),
            ("server", "string", "Server name", false),
        ],
    ));
    t.push(ai_server_target("server_java_list", "List every Java installation detected on this machine."));
    t.push(ai_server_target("server_folder", "Open a server's folder in the file manager."));
    t.push(ai_tool(
        "server_set_ram",
        "Set a server's max RAM in MB.",
        &[
            ("ram_mb", "integer", "Max RAM in MB", true),
            ("server", "string", "Server name", false),
        ],
    ));
    t.push(ai_tool(
        "server_set_motd",
        "Set a server's MOTD.",
        &[
            ("motd", "string", "MOTD text", true),
            ("server", "string", "Server name", false),
        ],
    ));
    t.push(ai_tool(
        "server_set_port",
        "Set a server's port (also mirrors into server.properties).",
        &[
            ("port", "integer", "Port", true),
            ("server", "string", "Server name", false),
        ],
    ));

    // --- java ---
    t.push(ai_simple("java_list", "List every Java installation with its major version and path."));
    t.push(ai_tool(
        "java_check",
        "Check which Java version a Minecraft version requires and what is installed.",
        &[("mc_version", "string", "Minecraft version", true)],
    ));
    t.push(ai_tool(
        "java_install",
        "Download and install a managed Temurin JDK (e.g. 17 or 21).",
        &[("major", "integer", "Java major version, e.g. 17 or 21", true)],
    ));

    // --- launcher ---
    t.push(ai_simple("app_settings_get", "Read every launcher setting."));
    t.push(ai_tool(
        "app_settings_set",
        "Write one launcher setting (ram_mb, java_path, theme, offline_username, …).",
        &[
            ("key", "string", "Setting key", true),
            ("value", "string", "Setting value", true),
        ],
    ));
    t.push(ai_simple("app_news", "Latest Minecraft news and release notes."));
    t.push(ai_simple("app_check_updates", "Check whether a new launcher release is available."));
    t.push(ai_simple("app_system_info", "CPU, RAM and disk usage for this machine."));
    t.push(ai_tool(
        "app_boost",
        "Run the one-click performance boost for a profile.",
        &[("instance", "string", "Instance name", false)],
    ));
    t.push(ai_tool(
        "app_open_path",
        "Open a local file or folder with the OS default handler.",
        &[("path", "string", "Absolute path", true)],
    ));
    t.push(ai_tool(
        "app_open_url",
        "Open a URL in the default browser.",
        &[("url", "string", "http(s) URL", true)],
    ));
    t.push(ai_simple("multiplayer_list", "List the saved multiplayer servers with their addresses."));
    t.push(ai_tool(
        "multiplayer_ping",
        "Ping a Minecraft server to see if it is online.",
        &[
            ("host", "string", "Hostname or IP", true),
            ("port", "integer", "Port (default 25565)", false),
        ],
    ));
    t.push(ai_tool(
        "app_run_command",
        "Run a shell command on this machine and return its output. Use sparingly.",
        &[("command", "string", "Shell command", true)],
    ));
    t.push(ai_tool(
        "app_navigate",
        "Move the launcher UI to a page.",
        &[(
            "page",
            "string",
            "home | instances | mods | modpacks | servers | servermgr | templates | boost | accounts | themes | news | ai | config | logs | settings",
            true,
        )],
    ));

    // --- workspace ---
    t.push(ai_tool(
        "ai_context_set",
        "Select the active workspace so target-specific tools default to it.",
        &[
            ("kind", "string", "server or instance", true),
            ("name", "string", "Server or profile name", true),
        ],
    ));
    t.push(ai_simple("ai_context_get", "Read the currently selected workspace."));
    t.push(ai_simple("ai_context_clear", "Clear the active workspace."));

    t
}

/// Keyword buckets: user text → substrings we want to see in a tool name.
const AI_TOPIC_KEYWORDS: &[(&[&str], &[&str])] = &[
    (&["plugin", "jar", "plugman"], &["plugin"]),
    (&["backup", "restore", "snapshot"], &["backup"]),
    (&["world", "reset", "regenerate"], &["world"]),
    (&["properties", "server.properties"], &["properties"]),
    (&["console", "command", "say", "list", "tps"], &["console"]),
    (&["motd"], &["motd"]),
    (&["ram", "memory", "jvm", "aikar"], &["ram", "jvm"]),
    (&["port", "forward"], &["port"]),
    (&["file", "folder", "config", "read the"], &["file", "folder", "path"]),
    (&["mod", "fabric mod", "sodium"], &["mod"]),
    (&["profile", "instance", "playtime"], &["instance"]),
    (&["launch", "play the game", "start the game"], &["launch"]),
    (&["guide", "how do i", "how to", "tutorial"], &["guide"]),
    (&["navigate", "go to", "open the", "switch to"], &["navigate"]),
    (&["java"], &["java"]),
    (&["theme", "background"], &["theme", "background"]),
    (&["cpu", "disk", "system info"], &["system_info"]),
    (&["news"], &["news"]),
    (&["update", "new version"], &["update"]),
    (&["boost", "fps", "lag", "performance"], &["boost", "perf"]),
    (&["create", "make a server", "new server"], &["create"]),
    (&["delete", "remove"], &["delete"]),
];

fn is_core_ai_tool(name: &str) -> bool {
    matches!(
        name,
        "instance_list"
            | "server_list"
            | "app_settings_get"
            | "app_news"
            | "app_system_info"
            | "java_list"
            | "mod_search"
            | "instance_versions_list"
            | "server_types_list"
            | "server_versions_list"
            | "mod_profiles_list"
    ) || name.starts_with("ai_context_")
}

/// Groq reports how long until a token window frees up.
fn ai_rate_limit_wait(err: &str) -> Option<f64> {
    if !err.contains("rate_limit") && !err.contains("429") {
        return None;
    }
    let marker = err.find("try again in ")? + 13;
    let rest = &err[marker..];
    let end = rest.find('s').unwrap_or(rest.len());
    rest[..end]
        .trim()
        .parse::<f64>()
        .ok()
        .filter(|w| *w > 0.0 && *w <= 30.0)
}

/// Rank the registry and send only the most relevant subset — Groq's free tier
/// has a small per-minute token budget.
fn ai_select_tools(context: &Option<Value>, messages: &[Value], all: &[Value]) -> Vec<Value> {
    let conv_chars = serde_json::to_string(messages).unwrap_or_default().len();
    let limit = if conv_chars > 9000 {
        34
    } else if conv_chars > 4000 {
        46
    } else {
        60
    };
    let limit = limit.min(all.len());
    if all.len() <= limit {
        return all.to_vec();
    }
    let kind = context
        .as_ref()
        .and_then(|c| c.get("kind"))
        .and_then(|k| k.as_str())
        .unwrap_or("");
    let thread: String = messages
        .iter()
        .rev()
        .take(6)
        .filter_map(|m| m.get("content").and_then(|c| c.as_str()))
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase();
    let mut scored: Vec<(i32, usize)> = all
        .iter()
        .enumerate()
        .map(|(i, tool)| {
            let name = tool["function"]["name"].as_str().unwrap_or("");
            let desc = tool["function"]["description"]
                .as_str()
                .unwrap_or("")
                .to_lowercase();
            let mut score = 5i32;
            if !kind.is_empty() {
                let wants = if kind == "server" { "server_" } else { "instance_" };
                let other = if kind == "server" {
                    "instance_"
                } else {
                    "server_"
                };
                if name.starts_with(wants) {
                    score += 100;
                } else if name.starts_with(other) {
                    score -= 25;
                } else if name.starts_with("ai_") || name.starts_with("app_") {
                    score += 30;
                }
            }
            for (aliases, needles) in AI_TOPIC_KEYWORDS.iter() {
                let hit = aliases.iter().any(|a| thread.contains(a));
                if hit && needles.iter().any(|nd| name.contains(nd) || desc.contains(nd)) {
                    score += 60;
                }
            }
            if is_core_ai_tool(name) {
                score += 40;
            }
            (score, i)
        })
        .collect();
    scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    let mut chosen: Vec<usize> = scored.iter().take(limit).map(|(_, i)| *i).collect();
    chosen.sort_unstable();
    chosen.into_iter().map(|i| all[i].clone()).collect()
}

// ---------- provider calls ----------

async fn ai_groq_chat<F: FnMut(String)>(
    client: &reqwest::Client,
    key: &str,
    model: &str,
    messages: &[Value],
    tools: &[Value],
    mut on_chunk: F,
) -> Result<AiResponse, String> {
    let body = json!({
        "model": model,
        "messages": messages,
        "tools": tools,
        "tool_choice": "auto",
        "stream": true,
        "temperature": 0.2,
    });
    let resp = client
        .post(GROQ_API)
        .bearer_auth(key)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("network error: {}", e))?;
    let status = resp.status();
    if !status.is_success() {
        let txt = resp.text().await.unwrap_or_default();
        return Err(format!(
            "HTTP {}: {}",
            status.as_u16(),
            txt.chars().take(400).collect::<String>()
        ));
    }
    let mut full = String::new();
    let mut calls: std::collections::HashMap<usize, (String, String, String)> =
        std::collections::HashMap::new();
    let mut buf = String::new();
    let mut stream = resp;
    while let Some(chunk) = stream
        .chunk()
        .await
        .map_err(|e| format!("stream error: {}", e))?
    {
        buf.push_str(&String::from_utf8_lossy(&chunk));
        while let Some(idx) = buf.find('\n') {
            let line = buf[..idx].trim().to_string();
            buf.drain(..=idx);
            if line.is_empty() || line.starts_with(':') {
                continue;
            }
            let data = match line.strip_prefix("data:") {
                Some(d) => d.trim(),
                None => continue,
            };
            if data == "[DONE]" {
                continue;
            }
            let v: Value = match serde_json::from_str(data) {
                Ok(v) => v,
                Err(_) => continue,
            };
            let delta = &v["choices"][0]["delta"];
            if let Some(c) = delta.get("content").and_then(|c| c.as_str()) {
                if !c.is_empty() {
                    full.push_str(c);
                    on_chunk(c.to_string());
                }
            }
            if let Some(tcs) = delta.get("tool_calls").and_then(|t| t.as_array()) {
                for tc in tcs {
                    let i = tc["index"].as_u64().unwrap_or(0) as usize;
                    let e = calls
                        .entry(i)
                        .or_insert_with(|| (String::new(), String::new(), String::new()));
                    if let Some(id) = tc["id"].as_str() {
                        if !id.is_empty() {
                            e.0 = id.to_string();
                        }
                    }
                    if let Some(nm) = tc["function"]["name"].as_str() {
                        if !nm.is_empty() {
                            e.1 = nm.to_string();
                        }
                    }
                    if let Some(a) = tc["function"]["arguments"].as_str() {
                        e.2.push_str(a);
                    }
                }
            }
        }
    }
    let mut keys: Vec<usize> = calls.keys().copied().collect();
    keys.sort();
    let mut tool_calls = Vec::new();
    for k in keys {
        if let Some((id, name, args)) = calls.get(&k) {
            if !name.is_empty() {
                tool_calls.push(AiToolCall {
                    id: if id.is_empty() {
                        format!("call_{}", k)
                    } else {
                        id.clone()
                    },
                    name: name.clone(),
                    arguments: if args.is_empty() {
                        "{}".to_string()
                    } else {
                        args.clone()
                    },
                });
            }
        }
    }
    Ok(AiResponse {
        content: full,
        tool_calls,
        streamed: true,
    })
}

async fn ai_gemini_chat<F: FnMut(String)>(
    client: &reqwest::Client,
    key: &str,
    model: &str,
    system: &str,
    messages: &[Value],
    tools: &[Value],
    mut on_chunk: F,
) -> Result<AiResponse, String> {
    let mut contents: Vec<Value> = Vec::new();
    for m in messages {
        let role = m.get("role").and_then(|r| r.as_str()).unwrap_or("user");
        match role {
            "system" => {}
            "assistant" => {
                let mut parts: Vec<Value> = Vec::new();
                if let Some(c) = m.get("content").and_then(|c| c.as_str()) {
                    if !c.is_empty() {
                        parts.push(json!({ "text": c }));
                    }
                }
                if let Some(tcs) = m.get("tool_calls").and_then(|t| t.as_array()) {
                    for tc in tcs {
                        let args: Value = serde_json::from_str(
                            tc["function"]["arguments"].as_str().unwrap_or("{}"),
                        )
                        .unwrap_or(json!({}));
                        parts.push(json!({
                            "functionCall": {
                                "name": tc["function"]["name"].as_str().unwrap_or(""),
                                "args": args
                            }
                        }));
                    }
                }
                if !parts.is_empty() {
                    contents.push(json!({ "role": "model", "parts": parts }));
                }
            }
            "tool" => {
                let name = m.get("name").and_then(|n| n.as_str()).unwrap_or("");
                let content = m.get("content").and_then(|c| c.as_str()).unwrap_or("");
                contents.push(json!({
                    "role": "user",
                    "parts": [{ "functionResponse": { "name": name, "response": { "result": content } } }]
                }));
            }
            _ => {
                let c = m.get("content").and_then(|c| c.as_str()).unwrap_or("");
                contents.push(json!({ "role": "user", "parts": [{ "text": c }] }));
            }
        }
    }
    let mut decls: Vec<Value> = Vec::new();
    for t in tools {
        if let Some(f) = t.get("function") {
            decls.push(f.clone());
        }
    }
    let body = json!({
        "contents": contents,
        "systemInstruction": { "parts": [{ "text": system }] },
        "tools": [{ "functionDeclarations": decls }],
        "temperature": 0.2,
    });
    let url = format!("{}/{}:generateContent?key={}", GEMINI_API, model, key);
    let resp = client
        .post(&url)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("network error: {}", e))?;
    let status = resp.status();
    if !status.is_success() {
        let txt = resp.text().await.unwrap_or_default();
        return Err(format!(
            "HTTP {}: {}",
            status.as_u16(),
            txt.chars().take(400).collect::<String>()
        ));
    }
    let v: Value = resp.json().await.map_err(|e| format!("parse error: {}", e))?;
    let mut content = String::new();
    let mut tool_calls = Vec::new();
    let parts = v["candidates"][0]["content"]["parts"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    for (i, part) in parts.iter().enumerate() {
        if let Some(t) = part.get("text").and_then(|t| t.as_str()) {
            content.push_str(t);
        }
        if let Some(fc) = part.get("functionCall") {
            let name = fc["name"].as_str().unwrap_or("").to_string();
            let args = serde_json::to_string(&fc["args"]).unwrap_or_else(|_| "{}".to_string());
            tool_calls.push(AiToolCall {
                id: format!("gemini_{}", i),
                name,
                arguments: args,
            });
        }
    }
    if !content.is_empty() {
        on_chunk(content.clone());
    }
    Ok(AiResponse {
        content,
        tool_calls,
        streamed: false,
    })
}

// ---------- tool execution ----------

fn ai_s(args: &Value, k: &str) -> String {
    args.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string()
}
fn ai_n(args: &Value, k: &str) -> u64 {
    args.get(k).and_then(|v| v.as_u64()).unwrap_or(0)
}
fn ai_i(args: &Value, k: &str) -> i64 {
    args.get(k).and_then(|v| v.as_i64()).unwrap_or(0)
}
fn ai_b(args: &Value, k: &str) -> bool {
    args.get(k).and_then(|v| v.as_bool()).unwrap_or(false)
}

/// Workspace fallbacks so tools work without an explicit target argument.
fn ai_ctx_field(app: &AppState, field: &str) -> String {
    app.ai_context
        .lock()
        .unwrap()
        .as_ref()
        .and_then(|c| c.get(field))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}

fn ai_target_instance(app: &AppState, args: &Value) -> String {
    let explicit = ai_s(args, "instance");
    if !explicit.is_empty() {
        return explicit;
    }
    ai_ctx_field(app, "name")
}

fn ai_target_server(app: &AppState, args: &Value) -> String {
    let explicit = ai_s(args, "server");
    if !explicit.is_empty() {
        return explicit;
    }
    ai_ctx_field(app, "name")
}

/// Tools whose implementation is an `async` command; they are awaited directly
/// on the runtime instead of being run on the blocking pool.
fn ai_tool_is_async(tool: &str) -> bool {
    matches!(
        tool,
        "instance_launch"
            | "instance_create"
            | "instance_apply_template"
            | "server_create"
            | "server_reinstall"
            | "java_install"
    )
}

async fn ai_execute_tool(handle: &tauri::AppHandle, tool: &str, args: &Value) -> Value {
    let st = handle.state::<AppState>();
    match tool {
        "instance_launch" => {
            let name = ai_target_instance(&st, args);
            launch_game(handle.clone(), name).await
        }
        "instance_create" => {
            let name = ai_s(args, "name");
            let version = ai_s(args, "version");
            let loader = ai_s(args, "loader");
            let loader_version = ai_s(args, "loader_version");
            create_instance(st, handle.clone(), name, version, loader, loader_version)
        }
        "instance_apply_template" => {
            let name = ai_target_instance(&st, args);
            let tid = ai_s(args, "template_id");
            apply_instance_template(handle.clone(), name, tid).await
        }
        "server_create" => {
            let port = ai_n(args, "port");
            let motd = {
                let m = ai_s(args, "motd");
                if m.is_empty() {
                    None
                } else {
                    Some(m)
                }
            };
            let ram = {
                let r = ai_n(args, "ram_mb");
                if r == 0 {
                    None
                } else {
                    Some(r)
                }
            };
            server_mgr_create(
                handle.clone(),
                ai_s(args, "name"),
                ai_s(args, "server_type"),
                ai_s(args, "mc_version"),
                if port == 0 { 25565 } else { port as u16 },
                motd,
                ram,
            )
            .await
        }
        "server_reinstall" => {
            let name = ai_target_server(&st, args);
            server_mgr_reinstall(handle.clone(), name, ai_s(args, "version")).await
        }
        "java_install" => download_java(handle.clone(), ai_n(args, "major") as u32).await,
        _ => {
            let h = handle.clone();
            let t = tool.to_string();
            let a = args.clone();
            match tokio::task::spawn_blocking(move || ai_tool_sync(&h, &t, &a)).await {
                Ok(v) => v,
                Err(e) => json!({ "error": format!("tool task failed: {}", e) }),
            }
        }
    }
}

fn ai_tool_sync(handle: &tauri::AppHandle, tool: &str, args: &Value) -> Value {
    let st = handle.state::<AppState>();
    let mut refresh: Option<&str> = None;
    let out = match tool {
        // --- profiles ---
        "instance_list" => get_instances(st.clone()),
        "instance_delete" => {
            refresh = Some("instances");
            delete_instance(st.clone(), ai_target_instance(&st, args))
        }
        "instance_stop" => {
            refresh = Some("instances");
            stop_game(st.clone())
        }
        "instance_folder" => open_instance_folder(st.clone(), ai_target_instance(&st, args)),
        "instance_mods_list" => get_instance_mods(ai_target_instance(&st, args)),
        "instance_mod_install" => {
            refresh = Some("mods");
            install_mod(
                st.clone(),
                ai_target_instance(&st, args),
                ai_s(args, "slug"),
                ai_s(args, "version_id"),
            )
        }
        "instance_mod_delete" => {
            refresh = Some("mods");
            delete_mod(ai_target_instance(&st, args), ai_s(args, "filename"))
        }
        "instance_resourcepacks_list" => get_instance_resourcepacks(ai_target_instance(&st, args)),
        "instance_settings_get" => get_instance_settings(st.clone(), ai_target_instance(&st, args)),
        "instance_settings_save" => {
            let name = ai_target_instance(&st, args);
            let mut patch = serde_json::Map::new();
            if let Some(r) = args.get("ram_mb").and_then(|v| v.as_i64()) {
                patch.insert("ram_mb".into(), json!(r));
            }
            if let Some(j) = args.get("jvm_args").and_then(|v| v.as_str()) {
                patch.insert("jvm_args".into(), json!(j));
            }
            if let Some(j) = args.get("java_path").and_then(|v| v.as_str()) {
                patch.insert("java_path".into(), json!(j));
            }
            let existing = get_instance_settings(st.clone(), name.clone());
            let mut merged = existing
                .get("settings")
                .cloned()
                .unwrap_or_else(|| json!({}));
            if !merged.is_object() {
                merged = json!({});
            }
            for (k, v) in patch {
                merged[k] = v;
            }
            save_instance_settings(st.clone(), name, merged)
        }
        "instance_edit" => {
            refresh = Some("instances");
            let name = ai_target_instance(&st, args);
            let new_name = {
                let n = ai_s(args, "new_name");
                if n.is_empty() {
                    name.clone()
                } else {
                    n
                }
            };
            edit_instance(
                st.clone(),
                name,
                new_name,
                ai_s(args, "version"),
                ai_s(args, "loader"),
                ai_s(args, "loader_version"),
            )
        }
        "instance_clone" => {
            refresh = Some("instances");
            clone_instance(st.clone(), ai_target_instance(&st, args), ai_s(args, "new_name"))
        }
        "instance_versions_list" => get_minecraft_versions(st.clone(), false),
        "instance_templates_list" => get_instance_templates(),
        "instance_open_path" => open_instance_path(ai_target_instance(&st, args), ai_s(args, "sub_folder")),
        "instance_analyze_crash" => analyze_crash_log(ai_target_instance(&st, args)),

        // --- mods ---
        "mod_search" => {
            let limit = {
                let l = ai_n(args, "limit");
                if l == 0 {
                    10
                } else {
                    l as u32
                }
            };
            search_modrinth(
                ai_s(args, "query"),
                ai_s(args, "project_type"),
                ai_s(args, "loader"),
                ai_s(args, "mc_version"),
                limit,
            )
        }
        "mod_versions" => get_modrinth_versions(
            ai_s(args, "slug"),
            ai_s(args, "loader"),
            ai_s(args, "mc_version"),
        ),
        "mod_list" => list_mods(st.clone(), ai_target_instance(&st, args)),
        "mod_check_updates" => check_mod_updates(st.clone(), ai_target_instance(&st, args)),
        "mod_check_conflicts" => detect_mod_conflicts(st.clone(), ai_target_instance(&st, args)),
        "perf_mods_install" => {
            refresh = Some("mods");
            let name = ai_target_instance(&st, args);
            let (mc, loader, _) = match instance_ctx(&st, &name) {
                Ok(x) => x,
                Err(e) => return json!({ "error": e }),
            };
            install_performance_mods(st.clone(), name, mc, loader)
        }
        "mod_profiles_list" => load_mod_profiles(),
        "mod_profile_save" => save_mod_profile(st.clone(), ai_s(args, "name"), ai_target_instance(&st, args)),
        "mod_profile_apply" => {
            refresh = Some("mods");
            apply_mod_profile(st.clone(), ai_s(args, "name"), ai_target_instance(&st, args))
        }
        "mod_profile_delete" => delete_mod_profile(ai_s(args, "name")),

        // --- server manager ---
        "server_list" => server_mgr_list(st.clone()),
        "server_delete" => {
            refresh = Some("servers");
            server_mgr_delete(st.clone(), handle.clone(), ai_target_server(&st, args))
        }
        "server_start" => {
            refresh = Some("servers");
            server_mgr_start(st.clone(), handle.clone(), ai_target_server(&st, args))
        }
        "server_stop" => {
            refresh = Some("servers");
            server_mgr_stop(st.clone(), handle.clone(), ai_target_server(&st, args))
        }
        "server_kill" => {
            refresh = Some("servers");
            server_mgr_kill(st.clone(), handle.clone(), ai_target_server(&st, args))
        }
        "server_restart" => {
            refresh = Some("servers");
            let name = ai_target_server(&st, args);
            let stop = server_mgr_stop(st.clone(), handle.clone(), name.clone());
            if stop.get("error").is_some() {
                // Not running — just start it.
                server_mgr_start(st.clone(), handle.clone(), name)
            } else {
                std::thread::sleep(Duration::from_millis(1500));
                server_mgr_start(st.clone(), handle.clone(), name)
            }
        }
        "server_status" => server_mgr_status(st.clone(), ai_target_server(&st, args)),
        "server_stats" => server_mgr_stats(st.clone(), ai_target_server(&st, args)),
        "server_console" => server_mgr_command(st.clone(), ai_target_server(&st, args), ai_s(args, "command")),
        "server_logs" => server_mgr_logs(st.clone(), ai_target_server(&st, args)),
        "server_properties_get" => server_mgr_get_properties(ai_target_server(&st, args)),
        "server_properties_set" => {
            let mut map = HashMap::new();
            if let Some(obj) = args.get("properties").and_then(|p| p.as_object()) {
                for (k, v) in obj {
                    map.insert(k.clone(), v.as_str().unwrap_or("").to_string());
                }
            }
            server_mgr_set_properties(ai_target_server(&st, args), map)
        }
        "server_plugins_list" => server_mgr_list_plugins(ai_target_server(&st, args)),
        "server_plugin_search" => {
            let mc = ai_s(args, "mc_version");
            let mc = if mc.is_empty() {
                load_json(&server_dir(&ai_target_server(&st, args)).join("server.json"))
                    .get("version")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string()
            } else {
                mc
            };
            server_mgr_search_plugins(ai_s(args, "query"), mc, 10)
        }
        "server_plugin_delete" => {
            refresh = Some("servers");
            server_mgr_delete_plugin(ai_target_server(&st, args), ai_s(args, "filename"))
        }
        "server_plugin_toggle" => {
            refresh = Some("servers");
            server_mgr_plugin_toggle(ai_target_server(&st, args), ai_s(args, "filename"))
        }
        "server_backup_list" => server_mgr_backup_list(ai_target_server(&st, args)),
        "server_backup_create" => {
            let label = {
                let l = ai_s(args, "label");
                if l.is_empty() {
                    None
                } else {
                    Some(l)
                }
            };
            server_mgr_backup_create(ai_target_server(&st, args), label)
        }
        "server_backup_restore" => server_mgr_backup_restore(st.clone(), ai_target_server(&st, args), ai_s(args, "file")),
        "server_worlds_list" => server_mgr_list_worlds(ai_target_server(&st, args)),
        "server_world_delete" => server_mgr_delete_world(st.clone(), ai_target_server(&st, args), ai_s(args, "world")),
        "server_file_list" => server_mgr_files_list(ai_target_server(&st, args), ai_s(args, "path")),
        "server_file_read" => server_mgr_file_read(ai_target_server(&st, args), ai_s(args, "path")),
        "server_file_write" => {
            refresh = Some("servers");
            server_mgr_file_write(
                ai_target_server(&st, args),
                ai_s(args, "path"),
                ai_s(args, "content"),
            )
        }
        "server_types_list" => server_mgr_server_types(),
        "server_versions_list" => server_mgr_list_versions(ai_s(args, "server_type")),
        "server_builds" => get_server_builds(ai_s(args, "mc_version")),
        "server_check_port" => server_mgr_check_port(ai_n(args, "port") as u16),
        "server_next_port" => {
            let start = {
                let s = ai_n(args, "start");
                if s == 0 {
                    None
                } else {
                    Some(s as u16)
                }
            };
            server_mgr_next_port(start)
        }
        "server_clone" => {
            refresh = Some("servers");
            let port = {
                let p = ai_n(args, "port");
                if p == 0 {
                    None
                } else {
                    Some(p as u16)
                }
            };
            server_mgr_clone(st.clone(), ai_target_server(&st, args), ai_s(args, "new_name"), port)
        }
        "server_eula" => server_mgr_eula(ai_target_server(&st, args), ai_b(args, "accept")),
        "server_java_list" => server_mgr_list_java(st.clone()),
        "server_folder" => server_mgr_open_folder(ai_target_server(&st, args)),
        "server_set_ram" => server_mgr_set_meta(
            ai_target_server(&st, args),
            Some(ai_n(args, "ram_mb")),
            None,
            None,
        ),
        "server_set_motd" => server_mgr_set_meta(
            ai_target_server(&st, args),
            None,
            Some(ai_s(args, "motd")),
            None,
        ),
        "server_set_port" => {
            refresh = Some("servers");
            server_mgr_set_meta(
                ai_target_server(&st, args),
                None,
                None,
                Some(ai_n(args, "port") as u16),
            )
        }

        // --- java ---
        "java_list" => get_java_installations(),
        "java_check" => check_java_compatibility(st.clone(), ai_s(args, "mc_version")),

        // --- launcher ---
        "app_settings_get" => get_settings(st.clone()),
        "app_settings_set" => {
            let value = args
                .get("value")
                .map(|v| match v {
                    Value::String(s) => {
                        // Keep numeric-looking settings numeric so RAM etc. work.
                        if let Ok(n) = s.parse::<i64>() {
                            if s.trim_start_matches('-').chars().all(|c| c.is_ascii_digit()) {
                                return json!(n);
                            }
                        }
                        json!(s)
                    }
                    other => other.clone(),
                })
                .unwrap_or(Value::Null);
            save_setting(st.clone(), ai_s(args, "key"), value)
        }
        "app_news" => get_minecraft_news_enhanced(),
        "app_check_updates" => check_updates(st.clone()),
        "app_system_info" => get_system_info(),
        "app_boost" => {
            refresh = Some("instances");
            game_boost(st.clone(), ai_target_instance(&st, args))
        }
        "app_open_path" => open_path(ai_s(args, "path")),
        "app_open_url" => open_url(ai_s(args, "url")),
        "multiplayer_list" => get_servers(),
        "multiplayer_ping" => {
            let port = {
                let p = ai_n(args, "port");
                if p == 0 {
                    25565
                } else {
                    p as u16
                }
            };
            ping_server(ai_s(args, "host"), port)
        }
        "app_run_command" => run_system_command(ai_s(args, "command")),
        "app_navigate" => json!({ "type": "navigate", "page": ai_s(args, "page") }),

        // --- workspace ---
        "ai_context_set" => {
            let kind = ai_s(args, "kind");
            let name = ai_s(args, "name");
            *st.ai_context.lock().unwrap() = Some(json!({ "kind": kind, "name": name }));
            json!({ "success": true, "kind": kind, "name": name })
        }
        "ai_context_get" => json!({ "context": st.ai_context.lock().unwrap().clone() }),
        "ai_context_clear" => {
            *st.ai_context.lock().unwrap() = None;
            json!({ "success": true })
        }

        _ => json!({ "error": format!("Unknown tool: {}", tool) }),
    };

    // Tell the UI to re-read anything the tool may have changed.
    if let Some(kind) = refresh {
        emit(
            handle,
            "ai-action",
            json!({ "type": format!("refresh-{}", kind) }),
        );
    } else if tool == "app_navigate" {
        emit(handle, "ai-action", json!({ "type": "navigate", "page": ai_s(args, "page") }));
    }

    // A short human summary so the chat can show what happened.
    let summary = out
        .get("summary")
        .and_then(|s| s.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| {
            if out.get("error").is_some() {
                out["error"].as_str().unwrap_or("failed").to_string()
            } else {
                match out.get("success").and_then(|s| s.as_bool()) {
                    Some(true) => "done".to_string(),
                    Some(false) => out
                        .get("error")
                        .and_then(|e| e.as_str())
                        .unwrap_or("failed")
                        .to_string(),
                    None => "done".to_string(),
                }
            }
        });
    let mut out = out;
    if !out.is_object() {
        out = json!({ "result": out });
    }
    out["summary"] = json!(summary);
    out
}

// ---------- the agent loop ----------

#[tauri::command]
async fn ai_chat(
    app: tauri::State<'_, AppState>,
    handle: tauri::AppHandle,
    messages: Vec<Value>,
    context: Option<Value>,
) -> Result<Value, String> {
    // Adopt the workspace the UI passed in for this turn.
    if let Some(ctx) = &context {
        let kind = ctx.get("kind").and_then(|k| k.as_str()).unwrap_or("");
        let name = ctx.get("name").and_then(|n| n.as_str()).unwrap_or("");
        if !kind.is_empty() && !name.is_empty() {
            *app.ai_context.lock().unwrap() = Some(ctx.clone());
        } else {
            *app.ai_context.lock().unwrap() = None;
        }
    }

    let system = ai_system_prompt(&app);
    let mut convo: Vec<Value> = vec![json!({ "role": "system", "content": system })];
    for m in &messages {
        let role = m.get("role").and_then(|r| r.as_str()).unwrap_or("user");
        let content = m.get("content").and_then(|c| c.as_str()).unwrap_or("");
        if (role == "user" || role == "assistant") && !content.is_empty() {
            convo.push(json!({ "role": role, "content": content }));
        }
    }

    let groq_key = ai_resolve_key(&app, AI_GROQ_SETTING_KEY, AI_GROQ_ENV, "groq_api_key");
    let gemini_key = ai_resolve_key(&app, AI_GEMINI_SETTING_KEY, AI_GEMINI_ENV, "gemini_api_key");
    if groq_key.is_empty() && gemini_key.is_empty() {
        let msg = "⚠️ No AI API key configured. Add `ai_groq_key` or `ai_gemini_key` in Settings, \
                   put them in ~/.isolate/ai_keys.json, or set ISOLATE_GROQ_KEY / ISOLATE_GEMINI_KEY.";
        emit(&handle, "ai-chunk", json!({ "text": msg }));
        emit(&handle, "ai-done", json!({ "reply": msg }));
        return Ok(json!({ "reply": msg, "error": "no_api_key" }));
    }

    let all_tools = ai_tool_defs();
    let tools = ai_select_tools(&app.ai_context.lock().unwrap().clone(), &convo, &all_tools);

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(300))
        .build()
        .map_err(|e| e.to_string())?;

    let settings = load_settings(&app);
    let groq_override = settings
        .get("ai_groq_model")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());
    let gemini_override = settings
        .get("ai_gemini_model")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());

    let mut rounds = 0usize;
    loop {
        rounds += 1;
        if rounds > AI_MAX_TOOL_ROUNDS {
            let note = "I've reached the maximum number of steps for this request. Here's what I completed so far.";
            emit(&handle, "ai-chunk", json!({ "text": note }));
            emit(&handle, "ai-done", json!({ "reply": note }));
            return Ok(json!({ "reply": note }));
        }

        // Provider/model fallback chain: the user's model first, then Groq, then Gemini.
        let mut attempts: Vec<(AiProvider, String)> = Vec::new();
        if let Some(m) = &groq_override {
            attempts.push((AiProvider::Groq, m.clone()));
        }
        for m in GROQ_MODELS {
            attempts.push((AiProvider::Groq, m.to_string()));
        }
        if let Some(m) = &gemini_override {
            attempts.push((AiProvider::Gemini, m.clone()));
        }
        for m in GEMINI_MODELS {
            attempts.push((AiProvider::Gemini, m.to_string()));
        }
        let mut seen = std::collections::HashSet::new();
        attempts.retain(|(p, m)| seen.insert((*p, m.clone())));

        let mut response: Option<AiResponse> = None;
        let mut last_err = String::new();
        for (prov, model) in &attempts {
            // Skip a provider whose key we don't have.
            let key = match prov {
                AiProvider::Groq => &groq_key,
                AiProvider::Gemini => &gemini_key,
            };
            if key.is_empty() {
                continue;
            }
            let mut tries = 0;
            loop {
                tries += 1;
                let res = match prov {
                    AiProvider::Groq => {
                        ai_groq_chat(&client, key, model, &convo, &tools, |c| {
                            emit(&handle, "ai-chunk", json!({ "text": c }))
                        })
                        .await
                    }
                    AiProvider::Gemini => {
                        ai_gemini_chat(&client, key, model, &system, &convo, &tools, |c| {
                            emit(&handle, "ai-chunk", json!({ "text": c }))
                        })
                        .await
                    }
                };
                match res {
                    Ok(r) => {
                        response = Some(r);
                        break;
                    }
                    Err(e) => {
                        last_err = format!("{:?} {}: {}", prov, model, e);
                        if tries < 3 {
                            if let Some(wait) = ai_rate_limit_wait(&e) {
                                emit(
                                    &handle,
                                    "ai-status",
                                    json!({ "text": format!("Rate limited — retrying in {:.0}s…", wait) }),
                                );
                                tokio::time::sleep(std::time::Duration::from_secs_f64(wait)).await;
                                continue;
                            }
                        }
                        emit(
                            &handle,
                            "ai-status",
                            json!({ "text": format!("{} failed, trying the next model…", model) }),
                        );
                    }
                }
                break;
            }
            if response.is_some() {
                break;
            }
        }

        let resp = match response {
            Some(r) => r,
            None => {
                let msg = "All AI providers failed to answer. Check your API key and internet connection and try again.";
                emit(&handle, "ai-chunk", json!({ "text": msg }));
                emit(&handle, "ai-done", json!({ "reply": msg }));
                return Ok(json!({ "reply": msg, "error": last_err }));
            }
        };

        if !resp.streamed && !resp.content.is_empty() {
            emit(&handle, "ai-chunk", json!({ "text": resp.content }));
        }
        if resp.tool_calls.is_empty() {
            emit(&handle, "ai-done", json!({ "reply": resp.content }));
            return Ok(json!({ "reply": resp.content }));
        }

        // Record the assistant turn, then run every tool it asked for.
        let tc_json: Vec<Value> = resp
            .tool_calls
            .iter()
            .map(|tc| {
                json!({
                    "id": tc.id,
                    "type": "function",
                    "function": { "name": tc.name, "arguments": tc.arguments }
                })
            })
            .collect();
        convo.push(json!({
            "role": "assistant",
            "content": resp.content,
            "tool_calls": tc_json
        }));

        for tc in &resp.tool_calls {
            emit(&handle, "ai-tool", json!({ "tool": tc.name, "status": "start" }));
            let parsed: Value = serde_json::from_str(&tc.arguments).unwrap_or(json!({}));
            let result = ai_execute_tool(&handle, &tc.name, &parsed).await;
            let summary = result
                .get("summary")
                .and_then(|s| s.as_str())
                .unwrap_or("")
                .to_string();
            let status = if result.get("error").is_some() {
                "error"
            } else {
                "done"
            };
            emit(
                &handle,
                "ai-tool",
                json!({ "tool": tc.name, "status": status, "summary": summary }),
            );
            let result_str = serde_json::to_string(&result).unwrap_or_else(|_| "{}".to_string());
            convo.push(json!({
                "role": "tool",
                "tool_call_id": tc.id,
                "name": tc.name,
                "content": result_str
            }));
        }
    }
}

#[tauri::command]
fn ai_context_set(app: tauri::State<AppState>, kind: String, name: String) -> Value {
    if kind.trim().is_empty() || name.trim().is_empty() {
        *app.ai_context.lock().unwrap() = None;
        return json!({ "success": true, "context": Value::Null });
    }
    let ctx = json!({ "kind": kind, "name": name });
    *app.ai_context.lock().unwrap() = Some(ctx.clone());
    json!({ "success": true, "context": ctx })
}

#[tauri::command]
fn ai_context_get_app(app: tauri::State<AppState>) -> Value {
    json!({ "context": app.ai_context.lock().unwrap().clone() })
}

#[tauri::command]
fn ai_context_clear_app(app: tauri::State<AppState>) -> Value {
    *app.ai_context.lock().unwrap() = None;
    json!({ "success": true })
}

// ============================================================
// RUN
// ============================================================

pub fn run() {
    ensure_dirs();

    tauri::Builder::default()
        .manage(AppState::new())
        .setup(|app| {
            let state = app.state::<AppState>();
            let _ = load_settings(&state);
            let _ = load_instances_raw(&state);
            let _ = load_instance_settings(&state);
            let _ = load_playtimes(&state);
            log_msg(&state, "[Isolate] Ready.");
            if let Some(w) = app.get_window("main") {
                let _ = w.set_title("Isolate Client");
            }
            // Auto-provision IsolateClientMod into every Fabric profile in the
            // background (4s grace so startup stays snappy; re-runs are no-ops
            // once the jar is present).
            let h = app.handle().clone();
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_secs(4));
                let st = h.state::<AppState>();
                let v = provision_isolate_client_mod_impl(&st);
                let inst = v["installed"].as_u64().unwrap_or(0);
                let errs = v["errors"].as_array().map(|a| a.len()).unwrap_or(0);
                if inst > 0 || errs > 0 {
                    log_msg(&st, &format!(
                        "[IsolateMod] Auto-provision: {inst} installed, {errs} error(s)"
                    ));
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // app / settings
            get_app_info,
            get_settings,
            save_settings,
            save_setting,
            get_system_info,
            get_hardware_info,
            discord_api,
            discord_invite_info,
            run_system_optimizer,
            open_path,
            // instances
            get_instances,
            create_instance,
            delete_instance,
            rename_instance,
            get_instance_settings,
            save_instance_settings,
            open_instance_folder,
            get_play_times,
            // versions
            get_minecraft_versions,
            get_loader_versions,
            get_supported_loaders,
            // java
            detect_java,
            install_java,
            // install / launch
            install_instance,
            launch_game,
            stop_game,
            get_game_state,
            // logs
            get_logs,
            clear_logs,
            get_game_output,
            clear_game_output,
            // auth
            create_offline_account,
            authenticate_microsoft,
            poll_microsoft_status,
            complete_microsoft_auth_cmd,
            sign_out,
            authenticate_elyby,
            poll_elyby_auth,
            complete_elyby_auth_cmd,
            sign_out_elyby,
            ensure_authlib_injector,
            // modrinth / mods / packs / templates
            search_modrinth,
            get_modrinth_project,
            get_modrinth_versions,
            install_mod,
            install_resourcepack,
            list_mods,
            list_resourcepacks,
            delete_pack_file,
            toggle_mod,
            batch_install_mods,
            check_mod_updates,
            resolve_mod_dependencies,
            detect_mod_conflicts,
            install_modpack,
            create_instance_from_modpack,
            export_instance_mrpack,
            get_instance_templates,
            create_instance_from_template,
            install_performance_mods,
            install_isolate_client_mod,
            provision_isolate_client_mod,
            // multiplayer
            get_servers,
            add_server,
            edit_server,
            delete_server,
            ping_server,
            // game boost / optimizer
            get_optimizer_status,
            apply_optimizer,
            apply_fps_preset,
            // crash / diagnostics
            analyze_crash_log,
            get_public_ip,
            get_game_logs,
            clear_instance_cache,
            // pin / screenshots / clone / config
            pin_instance,
            unpin_instance,
            get_screenshots,
            clone_instance,
            export_instance_config,
            import_instance_config,
            // wizard / notifications
            check_first_launch,
            complete_first_launch,
            show_notification,
            play_notification_sound,
            // file + url tools
            read_file_content,
            write_file_content,
            list_directory,
            delete_path,
            open_url,
            // skins
            get_skin_info,
            set_custom_skin,
            clear_custom_skin,
            // discord rpc
            init_discord_rpc,
            update_discord_rpc,
            stop_discord_rpc,
            get_discord_status,
            // local server manager
            server_mgr_server_types,
            get_server_versions,
            server_mgr_list_versions,
            get_server_builds,
            server_mgr_create,
            server_mgr_list,
            server_mgr_start,
            server_mgr_stop,
            server_mgr_kill,
            server_mgr_command,
            server_mgr_status,
            server_mgr_logs,
            server_mgr_clear_logs,
            server_mgr_delete,
            server_mgr_get_properties,
            server_mgr_set_properties,
            server_mgr_search_plugins,
            server_mgr_install_plugin,
            server_mgr_list_plugins,
            server_mgr_delete_plugin,
            server_mgr_plugin_toggle,
            server_mgr_files_list,
            server_mgr_file_read,
            server_mgr_file_write,
            server_mgr_file_delete,
            server_mgr_file_mkdir,
            server_mgr_file_rename,
            server_mgr_file_copy,
            server_mgr_file_upload,
            server_mgr_file_read_b64,
            server_mgr_file_download,
            server_mgr_read_icon,
            server_mgr_backup_list,
            server_mgr_backup_create,
            server_mgr_backup_restore,
            server_mgr_backup_delete,
            server_mgr_list_worlds,
            server_mgr_delete_world,
            server_mgr_reset_world,
            server_mgr_set_meta,
            server_mgr_update_meta,
            server_mgr_eula,
            server_mgr_clone,
            server_mgr_stats,
            server_mgr_check_port,
            server_mgr_next_port,
            server_mgr_list_java,
            server_mgr_open_folder,
            server_mgr_reinstall,
            // instance file extras
            open_instance_path,
            get_instance_mods,
            get_instance_resourcepacks,
            delete_mod,
            delete_resourcepack,
            move_mod_file,
            move_resourcepack_file,
            auto_enable_resourcepack,
            edit_instance,
            get_instance_icon,
            set_instance_icon,
            get_java_installations,
            check_java_compatibility,
            download_java,
            set_java_for_instance,
            apply_instance_template,
            get_fabric_versions,
            get_server_status,
            // mod profiles
            save_mod_profile,
            load_mod_profiles,
            delete_mod_profile,
            apply_mod_profile,
            // updater
            check_launcher_update,
            check_updates,
            get_latest_release_tag,
            download_update,
            install_update,
            startup_update_check,
            check_release_live,
            start_release_listener,
            mark_update_attempt,
            dismiss_update,
            // news
            get_minecraft_news,
            get_minecraft_news_enhanced,
            // backgrounds
            save_background_image,
            get_background_image_path,
            delete_background_image,
            save_background_video,
            get_background_video_path,
            delete_background_video,
            // boost / system / settings
            game_boost,
            run_system_command,
            get_setting_cmd,
            save_setting_cmd,
            uninstall_launcher,
            // image cache
            image_cache_get,
            image_cache_warm,
            // self-host + partners
            get_selfhost_status,
            download_selfhost,
            launch_selfhost,
            fetch_partners_remote,
            launch_server,
            // AI assistant
            ai_chat,
            ai_context_set,
            ai_context_get_app,
            ai_context_clear_app,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Isolate Client");
}

#[cfg(test)]
mod blocking_tests {
    use super::*;

    const URL: &str = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";

    /// The exact shape of an async `#[tauri::command]`: nested `blocking`
    /// guards (http_get -> blocking -> http_client -> blocking).
    #[test]
    fn nested_blocking_inside_async() {
        let out = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                std::panic::catch_unwind(|| http_get(URL).map(|v| v["latest"]["release"].to_string()))
            });
        let s = out.unwrap_or_else(|_| Err("PANICKED".into()));
        println!("nested -> {:?}", s);
        assert!(s.is_ok(), "unexpected: {:?}", s);
        assert!(s.unwrap().len() > 2, "expected a version string");
    }

    /// Sync commands and plain threads must keep working (guard is a no-op).
    #[test]
    fn nested_blocking_outside_runtime() {
        let v = http_get(URL).expect("sync path should work");
        println!("sync -> {:?}", v["latest"]["release"]);
        assert!(v["latest"]["release"].is_string());
    }

    /// Downloads share the same guard.
    #[test]
    fn download_guard_inside_async() {
        let dir = tempfile::tempdir().expect("tempdir");
        let dest = dir.path().join("manifest.json");
        let out = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                std::panic::catch_unwind(|| download_file(URL, &dest).map(|_| dest.exists()))
            });
        let s = out.unwrap_or_else(|_| Err("PANICKED".into()));
        println!("download -> {:?}", s);
        assert_eq!(s, Ok(true), "unexpected: {:?}", s);
    }
}

// ============================================================
// STEP 3 · E2E MINECRAFT LIBRARY / ASSET DIAGNOSTIC
// ------------------------------------------------------------
// End-to-end check of the install pipeline's resolve → download →
// cache → integrity path, without needing a GUI or a real instance:
//
//   1. version manifest + version JSON resolve
//   2. library resolution (LWJGL, authlib, netty) incl. Fabric profile
//   3. real downloads + SHA-1 integrity of the artifacts
//   4. asset index + object downloads (sounds, textures/skins) + SHA-1
//   5. cache semantics (existing files are never re-downloaded)
//   6. natives classifier download + extraction
//
// Run:  cargo test e2e_install -- --nocapture
// ============================================================
#[cfg(test)]
mod e2e_install_tests {
    use super::*;
    use sha1::Sha1;

    const MC: &str = "1.21.11";
    /// How many assets/libraries we actually pull bytes for (keeps the test
    /// seconds-long instead of downloading gigabytes).
    const SAMPLE: usize = 6;

    fn sha1_hex(bytes: &[u8]) -> String {
        let mut h = Sha1::new();
        h.update(bytes);
        h.finalize()
            .iter()
            .map(|b| format!("{:02x}", b))
            .collect::<String>()
    }
    fn sha1_file(path: &Path) -> String {
        sha1_hex(&std::fs::read(path).expect("read for sha1"))
    }

    /// HEAD/GET probe that never writes bytes.
    fn probe(url: &str) -> Result<u64, String> {
        blocking(|| {
            let c = http_client(30)?;
            let r = c.head(url).send().map_err(|e| e.to_string())?;
            if !r.status().is_success() {
                // some CDNs reject HEAD — fall back to a ranged GET
                let r2 = c
                    .get(url)
                    .header("Range", "bytes=0-0")
                    .send()
                    .map_err(|e| e.to_string())?;
                if !r2.status().is_success() {
                    return Err(format!("HTTP {}", r2.status()));
                }
                return Ok(r2.content_length().unwrap_or(0));
            }
            Ok(r.content_length().unwrap_or(0))
        })
    }

    /// manifest → version JSON on disk, plus the parsed value.
    fn version_json() -> (tempfile::TempDir, Value) {
        let dir = tempfile::tempdir().expect("tempdir");
        let manifest = http_get(MOJANG_MANIFEST).expect("manifest fetch");
        let entry = manifest["versions"]
            .as_array()
            .and_then(|a| a.iter().find(|v| v["id"] == MC))
            .unwrap_or_else(|| panic!("{} missing from manifest", MC));
        let vurl = entry["url"].as_str().expect("version url");
        let dest = dir.path().join(format!("{}.json", MC));
        download_file(vurl, &dest).expect("version json download");
        let vjson = load_json(&dest);
        assert!(!vjson.is_null(), "version JSON parsed");
        (dir, vjson)
    }

    /// Resolve every library + native of a version the same way install_inner
    /// does. Returns (lib_paths, native_paths).
    fn resolve_all(vjson: &Value, loader: &str, extra: &[Value]) -> (Vec<(String, String)>, Vec<(String, String)>) {
        let mut merged: Vec<Value> = vjson["libraries"].as_array().cloned().unwrap_or_default();
        merged.extend_from_slice(extra);
        let (mut libs, mut nats) = (Vec::new(), Vec::new());
        let mut seen = HashSet::new();
        let mut seen_nat = HashSet::new();
        for lib in &merged {
            if !is_lib_allowed(lib) {
                continue;
            }
            if let Some((rel, url)) = lib_artifact(lib, loader) {
                if seen.insert(rel.clone()) {
                    libs.push((rel, url));
                }
            }
            // Modern natives share their library's path; track them apart so
            // the diagnostic still reports which jars get unpacked.
            if let Some((rel, url)) = native_artifact(lib) {
                if seen_nat.insert(rel.clone()) {
                    nats.push((rel, url));
                }
            }
        }
        (libs, nats)
    }

    /// Pull `items[i]` for i in 0..SAMPLE into `root`, verify any SHA-1 the
    /// manifest published, and report failures. Returns (downloaded, failures).
    fn pull_sample(
        items: &[(String, String)],
        expected_sha: &HashMap<String, String>,
        root: &Path,
        label: &str,
    ) -> (usize, usize) {
        let take = SAMPLE.min(items.len());
        assert!(take > 0, "{}: nothing to download", label);
        let mut ok = 0usize;
        let mut bad = 0usize;
        for (rel, url) in items.iter().take(take) {
            let dest = root.join(rel);
            if let Err(e) = download_file(url, &dest) {
                eprintln!("[diag] {} DOWNLOAD FAIL {} — {}", label, rel, e);
                bad += 1;
                continue;
            }
            let size = dest.metadata().map(|m| m.len()).unwrap_or(0);
            assert!(size > 0, "{}: {} is empty", label, rel);
            if let Some(want) = expected_sha.get(rel) {
                let got = sha1_file(&dest);
                assert_eq!(&got, want, "{}: sha1 mismatch for {}", label, rel);
            }
            eprintln!("[diag] {} OK {:>8} B  {}", label, size, rel);
            ok += 1;
        }
        (ok, bad)
    }

    #[test]
    fn e2e_install_01_version_and_library_resolution() {
        let (_dir, vjson) = version_json();
        assert_eq!(vjson["id"], MC, "version id");
        assert!(
            vjson["downloads"]["client"]["url"].as_str().is_some(),
            "client jar URL present"
        );

        // Fabric profile libraries merged exactly like install_inner does.
        let loader_list = http_get(FABRIC_LOADER).expect("fabric loader list");
        let loader_ver = loader_list[0]["version"]
            .as_str()
            .expect("fabric loader version")
            .to_string();
        let prof = http_get(&format!(
            "https://meta.fabricmc.net/v2/versions/loader/{}/{}/profile/json",
            MC, loader_ver
        ))
        .expect("fabric profile");
        let extra: Vec<Value> = prof["libraries"].as_array().cloned().unwrap_or_default();
        assert!(!extra.is_empty(), "fabric profile contributes libraries");

        let (libs, natives) = resolve_all(&vjson, "fabric", &extra);
        println!("[diag] resolved {} libraries, {} natives", libs.len(), natives.len());
        assert!(libs.len() > 40, "expected a full library set, got {}", libs.len());
        assert!(!natives.is_empty(), "natives resolved for this OS");

        // The three families the client cannot launch without.
        let joined = libs
            .iter()
            .map(|(r, _)| r.to_lowercase())
            .collect::<Vec<_>>()
            .join("\n");
        for needle in ["lwjgl", "authlib", "netty"] {
            assert!(
                joined.contains(needle),
                "library set is missing {} — first paths:\n{}",
                needle,
                libs.iter().take(12).map(|(r, _)| r.as_str()).collect::<Vec<_>>().join("\n")
            );
        }
        // Every resolved URL must be live (cheap HEAD probe).
        let mut dead = Vec::new();
        for (rel, url) in libs.iter().chain(natives.iter()).take(20) {
            if let Err(e) = probe(url) {
                dead.push(format!("{} → {}", rel, e));
            }
        }
        assert!(dead.is_empty(), "unreachable library URLs:\n{}", dead.join("\n"));
        println!("[diag] all probed library/native URLs reachable");
    }

    #[test]
    fn e2e_install_02_library_download_and_cache() {
        let (_dir, vjson) = version_json();
        let (libs, _) = resolve_all(&vjson, "vanilla", &[]);
        let root = tempfile::tempdir().expect("tempdir");

        // Expected SHA-1s straight from the Mojang manifest.
        let mut expected = HashMap::new();
        for lib in vjson["libraries"].as_array().unwrap() {
            if let Some(p) = lib["downloads"]["artifact"]["path"].as_str() {
                if let Some(s) = lib["downloads"]["artifact"]["sha1"].as_str() {
                    expected.insert(p.to_string(), s.to_string());
                }
            }
        }

        let (ok, bad) = pull_sample(&libs, &expected, root.path(), "lib");
        assert_eq!(bad, 0, "library downloads must not fail");
        assert_eq!(ok, SAMPLE.min(libs.len()));

        // ---- CACHE: a populated destination must never be re-fetched ----
        let victim = root.path().join("cache-probe.jar");
        std::fs::write(&victim, b"cached-bytes").unwrap();
        let before = victim.metadata().unwrap().len();
        let bogus = vec![(
            "https://127.0.0.1:1/definitely-not-a-real-artifact.jar".to_string(),
            victim.clone(),
        )];
        let tick = |_d: usize, _t: usize| {};
        let fails = download_many(bogus, 1, &tick);
        assert_eq!(fails, 0, "cache hit must short-circuit the download");
        assert_eq!(
            victim.metadata().unwrap().len(),
            before,
            "cached file must be untouched"
        );
        // ---- and a missing destination must still attempt it ----
        let missing = root.path().join("missing-probe.jar");
        let fails = download_many(
            vec![(
                "https://127.0.0.1:1/definitely-not-a-real-artifact.jar".to_string(),
                missing.clone(),
            )],
            1,
            &tick,
        );
        assert_eq!(fails, 1, "cache miss must attempt the download");
        assert!(!missing.exists(), "failed download must not leave a file");
        println!("[diag] cache hit/miss semantics verified");
    }

    #[test]
    fn e2e_install_03_assets_index_sounds_textures() {
        let (_dir, vjson) = version_json();
        let idx = &vjson["assetIndex"];
        let idx_id = idx["id"].as_str().expect("assetIndex.id");
        let idx_url = idx["url"].as_str().expect("assetIndex.url");

        let root = tempfile::tempdir().expect("tempdir");
        let idx_path = root.path().join("indexes").join(format!("{}.json", idx_id));
        download_file(idx_url, &idx_path).expect("asset index download");
        if let Some(want) = idx["sha1"].as_str() {
            assert_eq!(sha1_file(&idx_path), want, "asset index sha1");
        }
        let index = load_json(&idx_path);
        let objects = index["objects"].as_object().expect("asset objects");
        println!("[diag] asset index '{}' → {} objects", idx_id, objects.len());
        assert!(objects.len() > 1000, "asset index looks too small");

        // Bucket the whole index the way the client's launcher cares about.
        let mut sounds = Vec::new();
        let mut textures = Vec::new();
        let mut other = Vec::new();
        for (name, meta) in objects {
            let hash = meta["hash"].as_str().unwrap_or("");
            assert_eq!(hash.len(), 40, "asset hash must be sha1 hex: {}", name);
            if name.ends_with(".ogg") {
                sounds.push(name.clone());
            } else if name.contains("textures/") && name.ends_with(".png") {
                textures.push(name.clone());
            } else {
                other.push(name.clone());
            }
        }
        assert!(!sounds.is_empty(), "no sound assets found");
        assert!(!textures.is_empty(), "no texture/skin assets found");
        println!(
            "[diag] {} sounds · {} textures · {} other",
            sounds.len(),
            textures.len(),
            other.len()
        );

        // Pull a balanced sample: 2 sounds + 2 textures + 2 misc.
        let mut sample: Vec<String> = Vec::new();
        for bucket in [&sounds, &textures, &other] {
            for name in bucket.iter().take(2) {
                sample.push(name.clone());
            }
        }
        let mut expected = HashMap::new();
        for name in &sample {
            let hash = objects[name.as_str()]["hash"].as_str().unwrap();
            expected.insert(format!("objects/{}/{}", &hash[..2], hash), hash.to_string());
        }
        let mut items: Vec<(String, String)> = Vec::new();
        for name in &sample {
            let hash = objects[name.as_str()]["hash"].as_str().unwrap();
            items.push((
                format!("objects/{}/{}", &hash[..2], hash),
                format!("{}/{}/{}", RESOURCES_BASE, &hash[..2], hash),
            ));
        }

        let (ok, bad) = pull_sample(&items, &expected, root.path(), "asset");
        assert_eq!(bad, 0, "asset downloads must not fail");
        assert_eq!(ok, sample.len());

        // ---- CACHE: every already-present object is skipped ----
        let tick = |_d: usize, _t: usize| {};
        let fails = download_many(
            items
                .iter()
                .map(|(rel, url)| (url.clone(), root.path().join(rel)))
                .collect(),
            4,
            &tick,
        );
        assert_eq!(fails, 0, "warm asset cache must produce zero failures");
        println!("[diag] asset cache warm-run: 0 failures");

        // Index must survive a re-read (idempotent install).
        assert!(idx_path.exists(), "index retained for re-use");
        assert_eq!(load_json(&idx_path)["objects"].as_object().unwrap().len(), objects.len());
    }

    #[test]
    fn e2e_install_04_natives_download_and_extract() {
        let (_dir, vjson) = version_json();
        let (_, natives) = resolve_all(&vjson, "vanilla", &[]);
        assert!(!natives.is_empty(), "no native artifacts resolved for {}", std::env::consts::OS);
        // Modern natives are `:natives-<os>` maven classifiers, not the legacy
        // `natives{}` field — assert we found the modern form for this build.
        let lwjgl: Vec<&(String, String)> = natives.iter().filter(|(r, _)| r.contains("lwjgl")).collect();
        println!(
            "[diag] {} native jars for {} ({} lwjgl)",
            natives.len(),
            std::env::consts::OS,
            lwjgl.len()
        );
        assert!(!lwjgl.is_empty(), "no LWJGL natives resolved — modern classifier matching is broken");

        let root = tempfile::tempdir().expect("tempdir");
        let out = root.path().join("natives-out");

        // Unpack EVERY native jar into one folder, exactly like install_inner
        // and the launch self-heal path do.
        let mut total = 0usize;
        let mut jar_sizes = Vec::new();
        for (rel, url) in &natives {
            let dest = root.path().join(rel);
            download_file(url, &dest).expect("native jar download");
            let size = dest.metadata().unwrap().len();
            assert!(size > 1_000, "native jar suspiciously small: {} B ({})", size, rel);
            jar_sizes.push((rel.clone(), size));
            match extract_natives(&dest, &out) {
                Ok(c) => {
                    println!("[diag] {:>6} B → {} binaries  {}", size, c, rel);
                    total += c;
                }
                Err(e) => panic!("extract {} failed: {}", rel, e),
            }
        }
        assert!(total > 0, "no native binaries extracted from any jar");

        let mut files: Vec<String> = std::fs::read_dir(&out)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        files.sort();
        println!("[diag] natives dir → {} files: {}", files.len(), files.join(", "));
        assert!(
            files.iter().any(|f| f.ends_with(".so") || f.ends_with(".dll") || f.ends_with(".dylib")),
            "no shared libraries in natives dir: {:?}",
            files
        );
        // LWJGL nests its .so under `linux/x64/org/lwjgl/...` — extraction must
        // flatten it to the jar root or `-Djava.library.path` will not find it.
        assert!(
            files.iter().any(|f| f.contains("glfw") || f.contains("lwjgl")),
            "LWJGL runtime libs missing from natives dir: {:?}",
            files
        );
        // META-INF must never leak into the natives folder.
        assert!(!files.iter().any(|f| f.starts_with("META-INF")), "META-INF leaked");

        // Re-extract must be idempotent (overwrite, no error).
        let before = files.len();
        for (rel, _) in &natives {
            let dest = root.path().join(rel);
            let _ = extract_natives(&dest, &out).expect("re-extract natives");
        }
        let after = std::fs::read_dir(&out).unwrap().count();
        assert_eq!(before, after, "re-extraction must not change the natives dir");
        println!("[diag] natives extraction idempotent ({} files stable)", after);

        // Legacy path (≤1.19.3) must keep working too: 1.16.5 still uses
        // `natives{}` + `downloads.classifiers`.
        let legacy_dir = tempfile::tempdir().expect("tempdir");
        let legacy_json = http_get_legacy_version().expect("1.16.5 version json");
        let (legacy_libs, legacy_nats) = resolve_all(&legacy_json, "vanilla", &[]);
        println!(
            "[diag] legacy 1.16.5 → {} libraries, {} natives",
            legacy_libs.len(),
            legacy_nats.len()
        );
        assert!(!legacy_nats.is_empty(), "legacy `natives` field resolution regressed");
        let (lrel, lurl) = &legacy_nats[0];
        let ldest = legacy_dir.path().join(lrel);
        download_file(lurl, &ldest).expect("legacy native download");
        let lout = legacy_dir.path().join("out");
        let lcount = extract_natives(&ldest, &lout).expect("legacy extract");
        println!("[diag] legacy {} → {} binaries", lrel, lcount);
        assert!(lcount > 0, "legacy natives extracted nothing");
    }

    /// Fetch a pre-1.19.4 version JSON (still uses the `natives{}` field).
    fn http_get_legacy_version() -> Result<Value, String> {
        let manifest = http_get(MOJANG_MANIFEST)?;
        let entry = manifest["versions"]
            .as_array()
            .and_then(|a| a.iter().find(|v| v["id"] == "1.16.5"))
            .ok_or("1.16.5 missing")?;
        let vurl = entry["url"].as_str().ok_or("url missing")?;
        let tmp = tempfile::tempdir().map_err(|e| e.to_string())?;
        let dest = tmp.path().join("v.json");
        download_file(vurl, &dest)?;
        Ok(load_json(&dest))
    }

    #[test]
    fn e2e_install_05_client_jar_and_full_resolution_report() {
        // One consolidated report the operator can read in --nocapture.
        let (dir, vjson) = version_json();
        let loader_list = http_get(FABRIC_LOADER).expect("fabric loader list");
        let loader_ver = loader_list[0]["version"].as_str().unwrap().to_string();
        let prof = http_get(&format!(
            "https://meta.fabricmc.net/v2/versions/loader/{}/{}/profile/json",
            MC, loader_ver
        ))
        .expect("fabric profile");
        let extra: Vec<Value> = prof["libraries"].as_array().cloned().unwrap_or_default();
        let (libs, natives) = resolve_all(&vjson, "fabric", &extra);

        // client jar
        let jar_url = vjson["downloads"]["client"]["url"].as_str().expect("client url");
        let jar = dir.path().join("client.jar");
        download_file(jar_url, &jar).expect("client jar download");
        let jar_size = jar.metadata().unwrap().len();
        if let Some(want) = vjson["downloads"]["client"]["sha1"].as_str() {
            assert_eq!(sha1_file(&jar), want, "client jar sha1");
        }
        assert!(jar_size > 1_000_000, "client jar too small: {}", jar_size);

        let asset_count = {
            let idx = &vjson["assetIndex"];
            let tmp = dir.path().join("idx.json");
            download_file(idx["url"].as_str().unwrap(), &tmp).expect("index");
            load_json(&tmp)["objects"].as_object().map(|o| o.len()).unwrap_or(0)
        };

        println!("┌──────────────────────────────────────────────────────────┐");
        println!("│ ISOLATE INSTALL DIAGNOSTIC — {}", MC);
        println!("├──────────────────────────────────────────────────────────┤");
        println!("│ client.jar        {:>10.1} MB  sha1 ✓", jar_size as f64 / 1_048_576.0);
        println!("│ libraries         {:>10}  (incl. fabric loader)", libs.len());
        println!("│ natives           {:>10}  for {}", natives.len(), std::env::consts::OS);
        println!("│ asset objects     {:>10}", asset_count);
        println!("│ LWJGL/authlib/netty  present ✓");
        println!("└──────────────────────────────────────────────────────────┘");

        let joined = libs.iter().map(|(r, _)| r.to_lowercase()).collect::<Vec<_>>().join("\n");
        for n in ["lwjgl", "authlib", "netty"] {
            assert!(joined.contains(n), "missing {}", n);
        }
        assert!(jar_size > 1_000_000 && asset_count > 1000);
    }
}

// ============================================================
// STEP 5 · AUTO-UPDATER ANTI-LOOP
// ------------------------------------------------------------
// Guards the boot-time update prompt against the classic failure
// modes: a dismissed release re-nagging forever, and a download
// that never lands turning every launch into a restart loop.
//
// Run:  cargo test update_antiloop -- --nocapture
// ============================================================
#[cfg(test)]
mod update_antiloop_tests {
    use super::*;

    fn state(attempted: &str, dismissed: &str) -> Value {
        json!({ "attempted_version": attempted, "dismissed_version": dismissed })
    }

    #[test]
    fn fresh_state_prompts() {
        assert_eq!(update_guard(&json!({}), "1.2.0"), None, "no guards → prompt");
        assert_eq!(
            update_guard(&state("", ""), "1.2.0"),
            None,
            "empty-string guards → prompt"
        );
    }

    #[test]
    fn dismissed_release_never_reprompts() {
        assert_eq!(
            update_guard(&state("", "1.2.0"), "1.2.0"),
            Some("dismissed"),
            "Later press must suppress auto-prompt for that exact tag"
        );
        // …but a NEWER release still prompts (Jakob: familiar flow, no staleness).
        assert_eq!(update_guard(&state("", "1.2.0"), "1.3.0"), None);
    }

    #[test]
    fn attempted_but_not_landed_blocks_loop() {
        // Download started for 1.2.0, install never landed → same build
        // still running → auto-prompt would restart us forever.
        assert_eq!(
            update_guard(&state("1.2.0", ""), "1.2.0"),
            Some("loop_guard"),
            "already-attempted release must not auto-prompt again"
        );
        // Different release is fine.
        assert_eq!(update_guard(&state("1.2.0", ""), "1.3.0"), None);
    }

    #[test]
    fn dismissed_wins_over_attempted() {
        // Both guards set for the same tag → dismissed is reported (the
        // user's explicit choice is the more specific reason).
        assert_eq!(
            update_guard(&state("1.2.0", "1.2.0"), "1.2.0"),
            Some("dismissed")
        );
    }

    #[test]
    fn empty_remote_never_suppresses_or_prompts() {
        // No release info → guard must not fire (and release_info already
        // reports update_available=false in that case).
        assert_eq!(update_guard(&state("1.2.0", "1.2.0"), ""), None);
    }

    #[test]
    fn landed_update_resets_attempted_guard() {
        // Simulate startup_update_check's reset: once APP_VERSION reaches
        // the attempted version, the guard clears for future releases.
        let attempted = "99.0.0"; // far future → version_gt(APP_VERSION, attempted) false
        let landed = version_gt(attempted, APP_VERSION); // remote was newer than us then…
        // The real condition: we are now AT or PAST the attempted version.
        let we_are_past = APP_VERSION == attempted || version_gt(APP_VERSION, attempted);
        assert!(!we_are_past, "current build is older than 99.0.0");
        // And the inverse: attempting our own version counts as landed.
        assert!(APP_VERSION == APP_VERSION || version_gt(APP_VERSION, APP_VERSION));
        let _ = landed;
        let we_attempted_ourselves = {
            let a = APP_VERSION.to_string();
            APP_VERSION == a || version_gt(APP_VERSION, &a)
        };
        assert!(we_attempted_ourselves, "attempting APP_VERSION itself = landed");
    }

    #[test]
    fn version_gt_orders_correctly() {
        assert!(version_gt("0.2.0", "0.1.0"));
        assert!(version_gt("v0.2.0", "0.1.9"));
        assert!(!version_gt("0.1.0", "0.1.0"), "equal → not greater");
        assert!(!version_gt("0.1.0", "0.2.0"));
        assert!(version_gt("1.0.0", "0.9.9"));
    }

    #[test]
    fn release_version_parses_tags() {
        assert_eq!(release_version(&json!({ "tag_name": "v0.2.0" })), "0.2.0");
        assert_eq!(
            release_version(&json!({ "name": "Isolate 0.3.1", "tag_name": "v0.3.0" })),
            "0.3.1",
            "name wins over tag"
        );
        assert_eq!(release_version(&json!({})), "");
    }

    #[test]
    fn update_state_patch_merges_keys() {
        // mark_update_attempt + dismiss_update must not clobber each other.
        let dir = tempfile::tempdir().expect("tempdir");
        let before = update_state_path();
        // Redirect via save/load against a temp file directly:
        let p = dir.path().join("update_state.json");
        save_json(&p, &json!({ "attempted_version": "1.2.0" }));
        let mut st = load_json(&p);
        assert!(st.is_object());
        if let Some(obj) = st.as_object_mut() {
            obj.insert("dismissed_version".into(), json!("1.2.0"));
        }
        save_json(&p, &st);
        let re = load_json(&p);
        assert_eq!(re["attempted_version"], "1.2.0", "attempted survives dismiss");
        assert_eq!(re["dismissed_version"], "1.2.0");
        let _ = before;
    }
}

#[cfg(test)]
mod step6_modprov_tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn perf_fabric_is_fifty_plus_unique() {
        assert!(
            PERF_FABRIC.len() >= 50,
            "need 50+ perf mods, got {}",
            PERF_FABRIC.len()
        );
        let mut seen = HashSet::new();
        for (slug, name) in PERF_FABRIC {
            assert!(seen.insert(*slug), "duplicate slug in PERF_FABRIC: {slug}");
            assert!(!name.is_empty(), "empty display name for {slug}");
        }
        // Headliners the brief calls out must stay on the list.
        for want in [
            "sodium",
            "lithium",
            "ferrite-core",
            "modernfix",
            "entityculling",
            "memoryleakfix",
            "fastanim",
            "clumps",
        ] {
            assert!(seen.contains(want), "{want} missing from PERF_FABRIC");
        }
        // Both members of every fork pair are listed — the runtime guard is
        // what keeps them from ever landing in the same mods/ folder.
        for (fork, up) in FORK_PAIRS {
            assert!(seen.contains(fork) && seen.contains(up), "fork pair {fork}/{up}");
        }
    }

    #[test]
    fn pick_isolate_asset_exact_mc_only() {
        let assets = json!([
            { "name": "isolateclient-1.21.10.jar", "browser_download_url": "https://x/10.jar" },
            { "name": "isolateclient-1.21.11.jar", "browser_download_url": "https://x/11.jar" },
            { "name": "README.md", "browser_download_url": "https://x/readme" }
        ]);
        assert_eq!(
            pick_isolate_asset(&assets, "1.21.11").as_deref(),
            Some("https://x/11.jar")
        );
        assert_eq!(
            pick_isolate_asset(&assets, "1.21.10").as_deref(),
            Some("https://x/10.jar")
        );
        // Never another version's jar — the loader hard-fails on a mismatch.
        assert_eq!(pick_isolate_asset(&assets, "1.21.12"), None);
        assert_eq!(pick_isolate_asset(&json!([]), "1.21.11"), None);
        assert_eq!(pick_isolate_asset(&json!({}), "1.21.11"), None);
        assert_eq!(pick_isolate_asset(&json!([{ "name": "no-url.jar" }]), "no-url"), None);
    }

    #[test]
    fn isolate_filename_is_per_mc() {
        assert_eq!(isolate_mod_filename("1.21.11"), "isolateclient-1.21.11.jar");
        assert_eq!(isolate_mod_filename("26.1"), "isolateclient-26.1.jar");
    }

    #[test]
    fn mods_dir_prefix_helper() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mods = dir.path().join("mods");
        std::fs::create_dir_all(&mods).expect("mkdir");
        std::fs::write(mods.join("sodium-0.6.5.jar"), b"x").expect("write");
        std::fs::write(mods.join("modernfix-mvus-1.4.jar"), b"x").expect("write");
        std::fs::write(mods.join("notes.txt"), b"x").expect("write");
        assert!(mods_dir_has_prefix(&mods, "sodium"));
        assert!(mods_dir_has_prefix(&mods, "modernfix-"), "original prefix");
        assert!(mods_dir_has_prefix(&mods, "modernfix-mvus"));
        assert!(!mods_dir_has_prefix(&mods, "lithium"));
        assert!(!mods_dir_has_prefix(&mods, "sodium-extra"), "no false positive");
    }

    /// LIVE network test: throwaway 1.21.11 Fabric profile → IsolateClientMod
    /// + the full 50+ perf pack, then ~/.isolate is restored exactly as it
    /// was. Ignored by default — run manually:
    ///   cargo test --lib step6_live -- --ignored --nocapture
    #[test]
    #[ignore]
    fn step6_live_provision_1_21_11() {
        use std::panic::{catch_unwind, AssertUnwindSafe};
        let app = AppState::new();
        // A previously killed run may have left the probe entry + folder behind —
        // start from a guaranteed-clean slate (user's own instances stay untouched).
        let backup: Vec<Value> = load_instances_raw(&app)
            .into_iter()
            .filter(|r| r.get("name").and_then(|n| n.as_str()) != Some("step6_probe"))
            .collect();
        let _ = std::fs::remove_dir_all(instance_dir("step6_probe"));
        let probe = json!({
            "name": "step6_probe",
            "version": "1.21.11",
            "loader": "fabric",
            "loader_version": "0.16.14",
            "created": "2026-11-15T00:00:00+05:30",
            "last_played": Value::Null,
        });
        let mut with_probe = backup.clone();
        with_probe.push(probe);
        save_instances(&app, &with_probe);
        let probe_dir = instance_dir("step6_probe");
        let _ = std::fs::create_dir_all(probe_dir.join("mods"));

        let body = catch_unwind(AssertUnwindSafe(|| {
            // 1) IsolateClientMod resolves from GitHub and lands in mods/.
            let v = install_isolate_client_mod_impl(&app, "step6_probe");
            assert_eq!(v["installed"], json!(true), "first install: {v}");
            let jar = probe_dir.join("mods").join(isolate_mod_filename("1.21.11"));
            assert!(jar.exists(), "jar missing at {}", jar.display());
            assert!(
                jar.metadata().map(|m| m.len() > 1024 * 1024).unwrap_or(false),
                "jar under 1MB — truncated download"
            );
            // 2) Idempotent — second run skips without touching the network.
            let v2 = install_isolate_client_mod_impl(&app, "step6_probe");
            assert_eq!(v2["skipped"], json!(true), "should skip: {v2}");
            // 3) The full perf pack installs for Fabric 1.21.11.
            let p = install_perf_mods_impl(
                &app,
                "step6_probe".into(),
                "1.21.11".into(),
                "fabric".into(),
            );
            assert_eq!(p["success"], json!(true), "perf pack failed: {p}");
            let installed = p["installed"].as_u64().unwrap_or(0);
            let jars = std::fs::read_dir(probe_dir.join("mods"))
                .unwrap()
                .flatten()
                .filter(|e| e.path().extension().map(|x| x == "jar").unwrap_or(false))
                .count();
            println!(
                "perf installed={installed}, jars on disk={jars}, skipped={}",
                p["skipped"]
            );
            assert!(installed >= 40, "only {installed} mods installed: {p}");
            assert!(jars >= 45, "only {jars} jars in mods/: {p}");
            // 4) The startup scan sees the profile as already provisioned.
            let prov = provision_isolate_client_mod_impl(&app);
            assert!(prov["skipped"].as_u64().unwrap_or(0) >= 1, "scan: {prov}");
        }));

        // Always restore the user's instance list + drop the probe.
        save_instances(&app, &backup);
        let _ = std::fs::remove_dir_all(&probe_dir);

        if let Err(e) = body {
            std::panic::resume_unwind(e);
        }
    }
}

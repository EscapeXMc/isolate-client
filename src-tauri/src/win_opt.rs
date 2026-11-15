// ============================================================
// Isolate Client — Windows Minecraft optimization suite (Phases 1-5).
//
// Spec: audit → HAGS → power plan → launcher/javaw scan → GraalVM/Temurin
// → timestamped backup + JVM flags + options.txt → per-javaw registry
// tweaks → NVIDIA profile → report + rollback.ps1 + BIOS checklist.
//
// Runs in a background thread on every game launch (see launch_game)
// and via the `run_system_optimizer` command. Built from std + serde_json
// only, so it compiles on every OS; the entry points short-circuit on
// non-Windows. All heavy work happens through powershell/reg/powercfg.
// ============================================================
#![allow(dead_code)]

use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const BACKUP_ROOT: &str = r"C:\Minecraft_Optimization_Backup";

/// Generational ZGC — Java 21+ (Minecraft 1.20.5+).
pub const ZGC_FLAGS: [&str; 6] = [
    "-XX:+UseZGC",
    "-XX:+ZGenerational",
    "-XX:+UnlockExperimentalVMOptions",
    "-XX:+AlwaysPreTouch",
    "-XX:+DisableExplicitGC",
    "-XX:+PerfDisableSharedMem",
];

/// NVIDIA profile-inspector settings (best-effort Phase 5).
const NVIDIA_POWER_MGMT_ID: &str = "0x10839d64"; // Power management mode
const NVIDIA_THREADED_ID: &str = "0x10839f70"; // Threaded optimization
const NVIDIA_LOWLATENCY_ID: &str = "0x22563744"; // Low Latency Mode

type LogFn<'a> = &'a dyn Fn(&str);

// ------------------------------------------------------------------
// small helpers
// ------------------------------------------------------------------
fn is_win() -> bool {
    cfg!(windows)
}

fn env_path(key: &str) -> Option<PathBuf> {
    std::env::var_os(key).map(PathBuf::from)
}

fn roaming() -> PathBuf {
    dirs::config_dir().unwrap_or_else(|| PathBuf::from("."))
}

fn local_appdata() -> PathBuf {
    dirs::data_local_dir().unwrap_or_else(|| PathBuf::from("."))
}

/// Run a PowerShell snippet, return stdout (stderr merged on failure).
fn ps(script: &str) -> Result<String, String> {
    let out = Command::new("powershell")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            script,
        ])
        .output()
        .map_err(|e| format!("powershell spawn: {e}"))?;
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    if !out.status.success() && stdout.trim().is_empty() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    Ok(stdout)
}

fn run(program: &str, args: &[&str]) -> Result<String, String> {
    let out = Command::new(program)
        .args(args)
        .output()
        .map_err(|e| format!("{program} spawn: {e}"))?;
    let mut s = String::from_utf8_lossy(&out.stdout).to_string();
    let err = String::from_utf8_lossy(&out.stderr).to_string();
    if !err.trim().is_empty() {
        s.push('\n');
        s.push_str(err.trim());
    }
    if !out.status.success() && s.trim().is_empty() {
        return Err(format!("{program} exited {:?}", out.status.code()));
    }
    Ok(s)
}

/// Parse `"major.minor.patch..."` out of `java -version` output.
fn parse_major_ver(text: &str) -> Option<u32> {
    for line in text.lines() {
        if let Some(i) = line.find('"') {
            let rest = &line[i + 1..];
            if let Some(j) = rest.find('"') {
                let ver = &rest[..j];
                let first = ver.split('.').next()?;
                if let Ok(n) = first.parse::<u32>() {
                    return Some(n);
                }
            }
        }
    }
    None
}

/// Minecraft version compare — snapshots (unparsable) count as "new".
pub fn mc_ge(ver: &str, maj: u32, min: u32, pat: u32) -> bool {
    let mut it = ver.split('.');
    let a: u32 = match it.next().and_then(|s| s.parse().ok()) {
        Some(v) => v,
        None => return true,
    };
    let b: u32 = match it.next().and_then(|s| s.parse().ok()) {
        Some(v) => v,
        None => return true,
    };
    let c: u32 = it.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    (a, b, c) >= (maj, min, pat)
}

/// Choose the flag set for a Minecraft version string.
pub fn gc_flags_for(mc_version: &str) -> &'static [&'static str] {
    // AIKAR_FLAGS lives in the crate root (lib.rs).
    if mc_ge(mc_version, 1, 20, 5) {
        &ZGC_FLAGS
    } else {
        &crate::AIKAR_FLAGS
    }
}

/// Bounded filesystem walk for a file name.
fn walk_find(root: &Path, name: &str, max_depth: usize, out: &mut Vec<PathBuf>, cap: usize) {
    if out.len() >= cap || max_depth == 0 || !root.is_dir() {
        return;
    }
    let entries = match fs::read_dir(root) {
        Ok(e) => e,
        Err(_) => return,
    };
    for e in entries.flatten() {
        if out.len() >= cap {
            return;
        }
        let p = e.path();
        let fname = e.file_name().to_string_lossy().to_lowercase();
        if p.is_dir() {
            // don't descend into huge irrelevant trees
            if !matches!(fname.as_str(), "node_modules" | "$recycle.bin" | "system volume information") {
                walk_find(&p, name, max_depth - 1, out, cap);
            }
        } else if fname == name.to_lowercase() {
            out.push(p);
        }
    }
}

/// Extract `X:\...\javaw.exe` occurrences from a config file's text.
fn javaw_paths_in_text(text: &str) -> Vec<String> {
    let lower = text.to_lowercase();
    let needle = "javaw.exe";
    let mut out = Vec::new();
    let bytes = lower.as_bytes();
    let mut from = 0usize;
    while let Some(pos) = lower[from..].find(needle) {
        let abs = from + pos;
        // backtrack to the nearest ':' right after a drive letter
        let mut start = abs;
        let mut i = abs;
        while i > 0 {
            i -= 1;
            if bytes[i] == b':' {
                if i > 0 && bytes[i - 1].is_ascii_alphabetic() {
                    start = i - 1;
                    break;
                }
            }
            if i == 0 {
                break;
            }
        }
        if start < abs {
            let end = (abs + needle.len()).min(text.len());
            let raw = &text[start..end];
            // trim trailing junk characters that are not part of a path
            let trimmed: String = raw
                .chars()
                .take_while(|c| {
                    c.is_alphanumeric() || matches!(c, ':' | '\\' | '.' | '_' | '-' | ' ' | '(' | ')')
                })
                .collect();
            if trimmed.to_lowercase().ends_with("javaw.exe") {
                out.push(trimmed);
            }
        }
        from = abs + needle.len();
        if from >= lower.len() {
            break;
        }
    }
    out
}

// ------------------------------------------------------------------
// PHASE 1 — hardware & environment audit
// ------------------------------------------------------------------
fn phase1(
    log: LogFn,
) -> (
    Value,
    Vec<String>,
    Vec<String>,
    Vec<PathBuf>,
    Vec<String>,
    Vec<PathBuf>,
) {
    log("[Optimizer] Phase 1: hardware & environment audit");
    let audit_json = ps(
        "$ErrorActionPreference='SilentlyContinue';\
         $cpu=Get-CimInstance Win32_Processor|Select-Object -First 1;\
         $cs=Get-CimInstance Win32_ComputerSystem;\
         $os=Get-CimInstance Win32_OperatingSystem;\
         $gpu=((Get-CimInstance Win32_VideoController).Name) -join '; ';\
         @{cpu=$cpu.Name;cores=$cpu.NumberOfCores;threads=$cpu.NumberOfLogicalProcessors;\
         ram_gb=[math]::Round($cs.TotalPhysicalMemory/1GB,1);gpu=$gpu;\
         os_build=$os.BuildNumber;os_caption=$os.Caption}|ConvertTo-Json -Compress",
    )
    .unwrap_or_default();
    let audit: Value = serde_json::from_str(audit_json.trim()).unwrap_or(Value::Null);
    log(&format!(
        "[Optimizer]   CPU {} · {}C/{}T · {}GB · GPU {} · build {}",
        audit.get("cpu").and_then(|v| v.as_str()).unwrap_or("?"),
        audit.get("cores").and_then(|v| v.as_u64()).unwrap_or(0),
        audit.get("threads").and_then(|v| v.as_u64()).unwrap_or(0),
        audit.get("ram_gb").and_then(|v| v.as_f64()).unwrap_or(0.0),
        audit.get("gpu").and_then(|v| v.as_str()).unwrap_or("?"),
        audit.get("os_build").and_then(|v| v.as_str()).unwrap_or("?"),
    ));

    let mut changes: Vec<String> = Vec::new();
    let mut rollbacks: Vec<String> = Vec::new();

    // ---- HAGS (HwSchMode) ----
    let hags_key = r"HKLM\SYSTEM\CurrentControlSet\Control\GraphicsDrivers";
    let cur = ps(
        "(Get-ItemProperty -Path 'HKLM:\\SYSTEM\\CurrentControlSet\\Control\\GraphicsDrivers' \
         -Name HwSchMode -ErrorAction SilentlyContinue).HwSchMode",
    )
    .unwrap_or_default();
    let cur = cur.trim().to_string();
    if cur == "2" {
        log("[Optimizer]   HAGS: already enabled (HwSchMode=2)");
    } else {
        let old = if cur.is_empty() { None } else { Some(cur.clone()) };
        let body = "Set-ItemProperty -Path 'HKLM:\\SYSTEM\\CurrentControlSet\\Control\\GraphicsDrivers' \
                    -Name HwSchMode -Value 2 -Type DWord -ErrorAction Stop; 'HAGS enabled'";
        match run_elevated(body, log) {
            Ok(_) => {
                log("[Optimizer]   HAGS: enabled (HwSchMode=2)");
                changes.push("HAGS enabled (HKLM HwSchMode=2)".into());
                rollbacks.push(match old {
                    Some(v) => format!("reg add \"{hags_key}\" /v HwSchMode /t REG_DWORD /d {v} /f"),
                    None => format!("reg delete \"{hags_key}\" /v HwSchMode /f"),
                });
            }
            Err(e) => {
                log(&format!("[Optimizer]   HAGS: skipped — {e}"));
                changes.push(format!("HAGS: SKIPPED (needs admin): {e}"));
            }
        }
    }

    // ---- power plan ----
    let before = ps("powercfg -getactivescheme").unwrap_or_default();
    let before_guid = extract_guid(&before);
    let _ = run("powercfg", &["-duplicatescheme", "e9a42b02-d5df-448d-aa00-03f14749eb61"]);
    let list = ps("powercfg -list").unwrap_or_default();
    let mut chosen: Option<(String, String)> = None;
    for line in list.lines() {
        let lname = line.to_lowercase();
        if lname.contains("ultimate performance") {
            if let Some(g) = extract_guid(line) {
                chosen = Some((g, "Ultimate Performance".into()));
                break;
            }
        }
    }
    if chosen.is_none() {
        for line in list.lines() {
            let lname = line.to_lowercase();
            if lname.contains("high performance") {
                if let Some(g) = extract_guid(line) {
                    chosen = Some((g, "High Performance".into()));
                    break;
                }
            }
        }
    }
    match chosen {
        Some((guid, name)) => match run("powercfg", &["-setactive", &guid]) {
            Ok(_) => {
                log(&format!("[Optimizer]   Power plan → {name}"));
                changes.push(format!("Active power plan → {name}"));
                if let Some(oldg) = before_guid {
                    if oldg != guid {
                        rollbacks.push(format!("powercfg -setactive {oldg}"));
                    }
                }
            }
            Err(e) => {
                log(&format!("[Optimizer]   Power plan: failed — {e}"));
                changes.push(format!("Power plan: FAILED ({e})"));
            }
        },
        None => log("[Optimizer]   Power plan: no High/Ultimate scheme found"),
    }

    // ---- installed launchers ----
    let mut launchers: Vec<String> = Vec::new();
    let bases: [(PathBuf, &str); 7] = [
        (roaming().join(".minecraft"), "Official/.minecraft"),
        (roaming().join("PrismLauncher"), "Prism Launcher"),
        (roaming().join("MultiMC"), "MultiMC"),
        (local_appdata().join("ModrinthApp"), "Modrinth App"),
        (local_appdata().join("CurseForge"), "CurseForge"),
        (roaming().join("LunarClient"), "Lunar Client"),
        (local_appdata().join("Badlion Client"), "Badlion Client"),
    ];
    for (p, name) in bases.iter() {
        if p.exists() {
            launchers.push(name.to_string());
        }
    }
    log(&format!(
        "[Optimizer]   Launchers found: {}",
        if launchers.is_empty() {
            "none".to_string()
        } else {
            launchers.join(", ")
        }
    ));

    // ---- javaw.exe scan ----
    let mut javaws: BTreeSet<String> = BTreeSet::new();
    let mut walk_roots: Vec<PathBuf> = Vec::new();
    for k in ["ProgramFiles", "ProgramFiles(x86)"] {
        if let Some(p) = env_path(k) {
            walk_roots.push(p);
        }
    }
    walk_roots.push(local_appdata().join("Programs"));
    let mut found: Vec<PathBuf> = Vec::new();
    for r in &walk_roots {
        let mut sub = Vec::new();
        walk_find(r, "javaw.exe", 4, &mut sub, 40);
        found.extend(sub);
    }
    for f in &found {
        javaws.insert(f.to_string_lossy().to_string());
    }
    let mut cfg_files: Vec<PathBuf> = Vec::new();
    for base in [
        roaming().join(".minecraft"),
        roaming().join("PrismLauncher"),
        roaming().join("MultiMC"),
    ] {
        let mut subs = Vec::new();
        walk_find(&base, "launcher_profiles.json", 3, &mut subs, 5);
        walk_find(&base, "instance.cfg", 4, &mut subs, 40);
        walk_find(&base, "prismlauncher.cfg", 3, &mut subs, 5);
        cfg_files.extend(subs);
    }
    for c in &cfg_files {
        if let Ok(text) = fs::read_to_string(c) {
            for p in javaw_paths_in_text(&text) {
                if Path::new(&p).exists() {
                    javaws.insert(p);
                }
            }
        }
    }
    let javaw_list: Vec<String> = javaws.iter().cloned().collect();
    log(&format!(
        "[Optimizer]   javaw.exe paths: {}",
        if javaw_list.is_empty() {
            "0 (will install Java in Phase 2)".into()
        } else {
            javaw_list.join(" | ")
        }
    ));

    (audit, changes, rollbacks, cfg_files, launchers, found)
}

fn extract_guid(text: &str) -> Option<String> {
    let start = text.find('(')?;
    let end = text[start..].find(")?")? + start;
    let g = &text[start + 1..end];
    if g.len() == 36 && g.chars().filter(|c| *c == '-').count() == 4 {
        Some(g.to_string())
    } else {
        None
    }
}

// ------------------------------------------------------------------
// elevation helper (single UAC prompt for all HKLM work)
// ------------------------------------------------------------------
fn run_elevated(ps_body: &str, log: LogFn) -> Result<String, String> {
    let dir = PathBuf::from(BACKUP_ROOT);
    let _ = fs::create_dir_all(&dir);
    let script = dir.join("_elevate.ps1");
    let outfile = dir.join("_elevate.out");
    let _ = fs::remove_file(&outfile);
    let content = format!(
        "$ErrorActionPreference='Stop'\n\
         try {{\n{}\n  'OK' | Set-Content -LiteralPath '{}' -Encoding UTF8\n\
         }} catch {{\n  $_.Exception.Message | Set-Content -LiteralPath '{}' -Encoding UTF8\n  exit 1\n}}\n",
        ps_body,
        outfile.display(),
        outfile.display()
    );
    fs::write(&script, content).map_err(|e| e.to_string())?;
    let wrapper = format!(
        "Start-Process powershell -Verb RunAs -Wait -ArgumentList \
         '-NoProfile','-ExecutionPolicy','Bypass','-File','{}'",
        script.display()
    );
    ps(&wrapper).map_err(|e| {
        log(&format!("[Optimizer]   elevation failed: {e}"));
        format!("UAC elevation failed or declined: {e}")
    })?;
    let out = fs::read_to_string(&outfile).unwrap_or_default();
    if out.trim() == "OK" {
        Ok(out)
    } else if out.trim().is_empty() {
        Err("elevation produced no output (UAC declined?)".into())
    } else {
        Err(out.trim().to_string())
    }
}

// ------------------------------------------------------------------
// PHASE 2 — GraalVM / Temurin JDK 21
// ------------------------------------------------------------------
fn phase2_java(log: LogFn, javaws: &[String]) -> (Value, Vec<String>, Vec<String>, Option<String>) {
    log("[Optimizer] Phase 2: high-performance Java runtime (GraalVM/Temurin 21)");
    let mut changes: Vec<String> = Vec::new();
    let rollbacks: Vec<String> = Vec::new(); // (nothing to roll back — installs are additive)

    // 1) does any existing javaw already speak Java 21+?
    let mut java21: Option<String> = None;
    for j in javaws {
        let p = Path::new(j);
        if let Some(java) = p.parent().map(|d| d.join("java.exe")) {
            if let Ok(out) = run(&java.to_string_lossy(), &["-version"]) {
                if let Some(maj) = parse_major_ver(&out) {
                    if maj >= 21 {
                        java21 = Some(j.clone());
                        log(&format!("[Optimizer]   Java {maj} found at {j}"));
                        break;
                    }
                }
            }
        }
    }

    if java21.is_none() {
        // 2) winget install — GraalVM first, Temurin as the fallback
        let winget_ok = run("winget", &["--version"]).is_ok();
        if winget_ok {
            let candidates = [
                "GraalVM.GraalVM.JDK.21",
                "Oracle.GraalVM.JDK.21",
                "GraalVM.GraalVM.JDK",
                "EclipseAdoptium.Temurin.21.JDK",
            ];
            for id in candidates {
                log(&format!("[Optimizer]   winget install {id} …"));
                let res = run(
                    "winget",
                    &[
                        "install",
                        "--id",
                        id,
                        "-e",
                        "--accept-source-agreements",
                        "--accept-package-agreements",
                        "--disable-interactivity",
                        "--silent",
                    ],
                );
                match res {
                    Ok(out) => {
                        let low = out.to_lowercase();
                        if low.contains("installed") || low.contains("already installed") {
                            log(&format!("[Optimizer]   {id} installed"));
                            changes.push(format!("Installed {id}"));
                            break;
                        }
                        log(&format!("[Optimizer]   {id}: no install — trying next"));
                    }
                    Err(e) => log(&format!("[Optimizer]   {id}: {e}")),
                }
            }
        } else {
            log("[Optimizer]   winget not available — skipping auto-install");
            changes.push("winget unavailable — install GraalVM/Temurin 21 manually".into());
        }

        // 3) rescan for a Java 21 javaw
        let mut rescan: Vec<PathBuf> = Vec::new();
        let mut roots: Vec<PathBuf> = Vec::new();
        for k in ["ProgramFiles", "ProgramFiles(x86)"] {
            if let Some(p) = env_path(k) {
                roots.push(p);
            }
        }
        for r in &roots {
            let mut sub = Vec::new();
            walk_find(r, "javaw.exe", 4, &mut sub, 60);
            rescan.extend(sub);
        }
        rescan.sort();
        for f in rescan {
            let j = f.to_string_lossy().to_string();
            if let Some(java) = f.parent().map(|d| d.join("java.exe")) {
                if let Ok(out) = run(&java.to_string_lossy(), &["-version"]) {
                    if let Some(maj) = parse_major_ver(&out) {
                        if maj >= 21 {
                            java21 = Some(j);
                            log(&format!("[Optimizer]   Java {maj} verified at {}", java21.clone().unwrap()));
                            break;
                        }
                    }
                }
            }
        }
    } else {
        // verify it runs
        if let Some(j) = &java21 {
            if let Some(java) = Path::new(j).parent().map(|d| d.join("java.exe")) {
                if let Ok(out) = run(&java.to_string_lossy(), &["-version"]) {
                    log(&format!(
                        "[Optimizer]   javaw verified: {}",
                        out.lines().next().unwrap_or("ok").trim()
                    ));
                }
            }
        }
    }

    let report = json!({
        "java21": java21,
        "installed": changes.iter().any(|c| c.starts_with("Installed ")),
    });
    (report, changes, rollbacks, java21)
}

// ------------------------------------------------------------------
// PHASE 3 — backup, RAM, JVM flags, options.txt
// ------------------------------------------------------------------
fn ram_mb_for(total_gb: f64) -> u64 {
    if total_gb >= 16.0 {
        8192
    } else if total_gb >= 12.0 {
        6144
    } else if total_gb >= 8.0 {
        4096
    } else {
        let half = (total_gb / 2.0).floor().max(2.0) as u64;
        half * 1024
    }
}

fn phase3_backup_and_inject(
    log: LogFn,
    audit: &Value,
    cfg_files: &[PathBuf],
    java21: &Option<String>,
    backup_dir: &Path,
    changes: &mut Vec<String>,
    rollbacks: &mut Vec<String>,
) -> Value {
    log("[Optimizer] Phase 3: backup + RAM + JVM flags + options.txt");
    if let Err(e) = fs::create_dir_all(backup_dir) {
        log(&format!("[Optimizer]   backup FAILED: {e}"));
        return json!({ "ok": false, "error": format!("backup: {e}") });
    }
    log(&format!(
        "[Optimizer]   Backup → {}",
        backup_dir.display()
    ));

    // ---- copy launcher configs + options.txt into the backup ----
    let mut to_copy: Vec<PathBuf> = cfg_files.to_vec();
    let mut opt_files: Vec<PathBuf> = Vec::new();
    for base in [
        roaming().join(".minecraft"),
        roaming().join("PrismLauncher"),
        roaming().join("MultiMC"),
        local_appdata().join("ModrinthApp"),
    ] {
        let mut sub = Vec::new();
        walk_find(&base, "options.txt", 5, &mut sub, 40);
        opt_files.extend(sub);
    }
    to_copy.extend(opt_files.clone());
    let mut copied = 0usize;
    for (i, f) in to_copy.iter().enumerate() {
        if let Ok(meta) = fs::metadata(f) {
            if meta.len() > 32 * 1024 * 1024 {
                continue; // don't copy huge files
            }
            let flat = format!("{i:03}_{}", f.file_name().unwrap_or_default().to_string_lossy());
            if fs::copy(f, backup_dir.join(flat)).is_ok() {
                copied += 1;
            }
        }
    }
    log(&format!("[Optimizer]   {copied} config files backed up"));
    changes.push(format!(
        "Timestamped backup of {copied} files → {}",
        backup_dir.display()
    ));
    rollbacks.push(format!(
        "# restore backed-up files from {} (flat names: NNN_original)",
        backup_dir.display()
    ));

    // ---- RAM ----
    let total_gb = audit.get("ram_gb").and_then(|v| v.as_f64()).unwrap_or(8.0);
    let ram_mb = ram_mb_for(total_gb);
    log(&format!(
        "[Optimizer]   RAM: {total_gb}GB total → -Xms{}M = -Xmx{}M",
        ram_mb, ram_mb
    ));
    changes.push(format!("-Xms{ram_mb}M = -Xmx{ram_mb}M (from {total_gb}GB RAM)"));

    // ---- options.txt optimization ----
    let wanted = [
        ("maxFps", "260"),
        ("enableVsync", "false"),
        ("rawMouseInput", "true"),
    ];
    let mut opt_done = 0usize;
    for f in &opt_files {
        let Ok(text) = fs::read_to_string(f) else { continue };
        let mut lines: Vec<String> = text.lines().map(|l| l.to_string()).collect();
        let mut changed = false;
        for (k, v) in wanted.iter() {
            let prefix = format!("{k}:");
            let mut found = false;
            for line in lines.iter_mut() {
                if line.starts_with(&prefix) {
                    let new = format!("{prefix}{v}");
                    if *line != new {
                        *line = new;
                        changed = true;
                    }
                    found = true;
                    break;
                }
            }
            if !found {
                lines.push(format!("{prefix}{v}"));
                changed = true;
            }
        }
        if changed && fs::write(f, lines.join("\n") + "\n").is_ok() {
            opt_done += 1;
        }
    }
    log(&format!(
        "[Optimizer]   options.txt optimized in {opt_done} location(s) (maxFps:260, vsync off, raw mouse)"
    ));
    if opt_done > 0 {
        changes.push(format!("options.txt tuned in {opt_done} file(s)"));
        rollbacks.push("# restore options.txt from the backup folder".into());
    }

    // ---- point launchers at Java 21 + inject GC flags ----
    let mut java_path_used: Option<String> = None;
    if let Some(j) = java21 {
        java_path_used = Some(j.clone());
        let flags: Vec<String> = ZGC_FLAGS.iter().map(|s| s.to_string()).collect();

        // official launcher profiles
        let prof_path = roaming().join(".minecraft").join("launcher_profiles.json");
        if let Ok(text) = fs::read_to_string(&prof_path) {
            if let Ok(mut v) = serde_json::from_str::<Value>(&text) {
                let mut patched = 0usize;
                if let Some(obj) = v.get_mut("profiles").and_then(|p| p.as_object_mut()) {
                    for (_name, prof) in obj.iter_mut() {
                        let Some(p) = prof.as_object_mut() else { continue };
                        let ver = p
                            .get("lastVersionId")
                            .and_then(|x| x.as_str())
                            .unwrap_or("")
                            .to_string();
                        let set: Vec<&str> = if mc_ge(&ver, 1, 20, 5) {
                            ZGC_FLAGS.to_vec()
                        } else {
                            crate::AIKAR_FLAGS.to_vec()
                        };
                        // javaArgs — append missing GC flags
                        let existing = p
                            .get("javaArgs")
                            .and_then(|x| x.as_str())
                            .unwrap_or("")
                            .to_string();
                        let mut args = existing.clone();
                        let mut added = false;
                        for f in &set {
                            if !args.contains(f) {
                                if !args.is_empty() {
                                    args.push(' ');
                                }
                                args.push_str(f);
                                added = true;
                            }
                        }
                        if added || existing.is_empty() {
                            p.insert("javaArgs".into(), json!(args));
                        }
                        // memory — only when the profile already manages it
                        if p.contains_key("minMem") || p.contains_key("maxMem") {
                            p.insert("minMem".into(), json!(ram_mb));
                            p.insert("maxMem".into(), json!(ram_mb));
                        }
                        patched += 1;
                    }
                }
                if patched > 0
                    && fs::write(
                        &prof_path,
                        serde_json::to_string_pretty(&v).unwrap_or_default(),
                    )
                    .is_ok()
                {
                    log(&format!(
                        "[Optimizer]   launcher_profiles.json: {patched} profile(s) patched (GC flags + {ram_mb}MB)"
                    ));
                    changes.push(format!("Official launcher: {patched} profiles → GC flags + {ram_mb}MB"));
                    rollbacks.push(format!(
                        "# launcher_profiles.json original: {}\\launcher_profiles.json",
                        backup_dir.display()
                    ));
                }
            }
        }

        // Prism / MultiMC instances
        let mut inst_cfgs: Vec<PathBuf> = Vec::new();
        for base in [roaming().join("PrismLauncher"), roaming().join("MultiMC")] {
            let mut sub = Vec::new();
            walk_find(&base, "instance.cfg", 4, &mut sub, 60);
            inst_cfgs.extend(sub);
        }
        let mut inst_done = 0usize;
        for cfg in &inst_cfgs {
            let Ok(text) = fs::read_to_string(cfg) else { continue };
            let inst_dir = cfg.parent().map(|p| p.to_path_buf()).unwrap_or_default();
            // version from mmc-pack.json (best effort)
            let mut ver = String::new();
            if let Ok(mp) = fs::read_to_string(inst_dir.join("mmc-pack.json")) {
                if let Ok(v) = serde_json::from_str::<Value>(&mp) {
                    if let Some(comps) = v.get("components").and_then(|c| c.as_array()) {
                        for c in comps {
                            if c.get("uid").and_then(|u| u.as_str()) == Some("net.minecraft") {
                                ver = c
                                    .get("version")
                                    .and_then(|x| x.as_str())
                                    .unwrap_or("")
                                    .to_string();
                                break;
                            }
                        }
                    }
                }
            }
            let set: Vec<&str> = if ver.is_empty() || mc_ge(&ver, 1, 20, 5) {
                ZGC_FLAGS.to_vec()
            } else {
                crate::AIKAR_FLAGS.to_vec()
            };
            let mut out_lines: Vec<String> = Vec::new();
            let mut have_java_args = false;
            let mut have_ms = false;
            let mut have_mx = false;
            let mut have_path = false;
            for line in text.lines() {
                if line.starts_with("JavaArgs=") {
                    have_java_args = true;
                    let mut args = line["JavaArgs=".len()..].to_string();
                    for f in &set {
                        if !args.contains(f) {
                            if !args.is_empty() {
                                args.push(' ');
                            }
                            args.push_str(f);
                        }
                    }
                    out_lines.push(format!("JavaArgs={args}"));
                } else if line.starts_with("JavaMs=") {
                    have_ms = true;
                    out_lines.push(format!("JavaMs={ram_mb}"));
                } else if line.starts_with("JavaMx=") {
                    have_mx = true;
                    out_lines.push(format!("JavaMx={ram_mb}"));
                } else if line.starts_with("JavaPath=") {
                    have_path = true;
                    out_lines.push(format!("JavaPath={}", java21.clone().unwrap_or_default()));
                } else {
                    out_lines.push(line.to_string());
                }
            }
            if !have_java_args {
                out_lines.push(format!("JavaArgs={}", set.join(" ")));
            }
            if !have_ms {
                out_lines.push(format!("JavaMs={ram_mb}"));
            }
            if !have_mx {
                out_lines.push(format!("JavaMx={ram_mb}"));
            }
            if !have_path {
                out_lines.push(format!("JavaPath={}", java21.clone().unwrap_or_default()));
            }
            if fs::write(cfg, out_lines.join("\n") + "\n").is_ok() {
                inst_done += 1;
            }
        }
        if inst_done > 0 {
            log(&format!(
                "[Optimizer]   Prism/MultiMC: {inst_done} instance(s) → Java 21 + GC flags + {ram_mb}MB"
            ));
            changes.push(format!("Prism/MultiMC: {inst_done} instances → Java 21 + GC flags"));
            rollbacks.push(format!(
                "# instance.cfg originals in {}",
                backup_dir.display()
            ));
        }
        let _ = flags;
    } else {
        log("[Optimizer]   No Java 21 found — launcher JVM args left untouched");
    }

    json!({
        "ok": true,
        "backup_dir": backup_dir.display().to_string(),
        "ram_mb": ram_mb,
        "options_tuned": opt_done,
        "java21": java_path_used,
    })
}

// ------------------------------------------------------------------
// PHASE 4 — registry / kernel / GPU routing
// ------------------------------------------------------------------
fn phase4_registry(
    log: LogFn,
    javaws: &[String],
    changes: &mut Vec<String>,
    rollbacks: &mut Vec<String>,
) -> Value {
    log("[Optimizer] Phase 4: registry — CPU priority, GPU routing, DWM, network");
    let mut applied = 0usize;
    let mut skipped = 0usize;

    // ---- 1) per-javaw: IFEO PerfOptions (CpuPriorityClass=3) — HKLM, elevated ----
    let ifeo_key = r"HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Image File Execution Options\javaw.exe\PerfOptions";
    let mut ps_lines: Vec<String> = Vec::new();
    if !javaws.is_empty() {
        // IFEO is keyed by image name — one write covers every javaw.exe
        ps_lines.push(
            "New-Item -Path 'HKLM:\\SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\Image File Execution Options\\javaw.exe\\PerfOptions' -Force | Out-Null; \
             Set-ItemProperty -Path 'HKLM:\\SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\Image File Execution Options\\javaw.exe\\PerfOptions' \
             -Name CpuPriorityClass -Value 3 -Type DWord"
                .to_string(),
        );
    }
    // ---- 4) network & timer (HKLM, elevated) ----
    ps_lines.push(
        "New-Item -Path 'HKLM:\\SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\Multimedia\\SystemProfile' -Force | Out-Null; \
         Set-ItemProperty -Path 'HKLM:\\SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\Multimedia\\SystemProfile' \
         -Name NetworkThrottlingIndex -Value 0xffffffff -Type DWord; \
         try { Set-ItemProperty -Path 'HKLM:\\SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\Multimedia\\SystemProfile' \
         -Name SystemResponsiveness -Value 0 -Type DWord -ErrorAction Stop } catch { \
         Set-ItemProperty -Path 'HKLM:\\SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\Multimedia\\SystemProfile' \
         -Name SystemResponsiveness -Value 10 -Type DWord }"
            .to_string(),
    );
    if !ps_lines.is_empty() {
        match run_elevated(&ps_lines.join("\n"), log) {
            Ok(_) => {
                applied += 1;
                log("[Optimizer]   CpuPriorityClass=3 (IFEO) + NetworkThrottlingIndex/SystemResponsiveness set");
                changes.push("IFEO javaw.exe CpuPriorityClass=3".into());
                changes.push("NetworkThrottlingIndex=0xffffffff, SystemResponsiveness=0".into());
                rollbacks.push(format!("reg delete \"{ifeo_key}\" /f"));
                rollbacks.push(
                    "reg delete \"HKLM\\SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\Multimedia\\SystemProfile\" /v NetworkThrottlingIndex /f".into(),
                );
                rollbacks.push(
                    "reg delete \"HKLM\\SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\Multimedia\\SystemProfile\" /v SystemResponsiveness /f".into(),
                );
            }
            Err(e) => {
                skipped += 1;
                log(&format!("[Optimizer]   HKLM tweaks skipped — {e}"));
                changes.push(format!("HKLM registry tweaks SKIPPED: {e}"));
            }
        }
    }

    // ---- 2+3) per-path HKCU tweaks (no admin needed) ----
    for j in javaws {
        // GPU preference
        let r1 = run(
            "reg",
            &[
                "add",
                r"HKCU\Software\Microsoft\DirectX\UserGpuPreferences",
                "/v",
                j,
                "/t",
                "REG_SZ",
                "/d",
                "GpuPreference=2;",
                "/f",
            ],
        );
        // disable fullscreen optimizations
        let r2 = run(
            "reg",
            &[
                "add",
                r"HKCU\Software\Microsoft\Windows NT\CurrentVersion\AppCompatFlags\Layers",
                "/v",
                j,
                "/t",
                "REG_SZ",
                "/d",
                "~ DISABLEDXMAXIMIZEDWINDOWEDMODE",
                "/f",
            ],
        );
        if r1.is_ok() && r2.is_ok() {
            applied += 1;
        } else {
            skipped += 1;
        }
    }
    if !javaws.is_empty() {
        log(&format!(
            "[Optimizer]   Per-javaw GPU preference + fullscreen-optimizations-off: {} applied, {skipped} skipped",
            javaws.len().saturating_sub(skipped)
        ));
        changes.push(format!(
            "GpuPreference=2 + DISABLEDXMAXIMIZEDWINDOWEDMODE for {} javaw path(s)",
            javaws.len()
        ));
        for j in javaws {
            rollbacks.push(format!(
                "reg delete \"HKCU\\Software\\Microsoft\\DirectX\\UserGpuPreferences\" /v \"{j}\" /f"
            ));
            rollbacks.push(format!(
                "reg delete \"HKCU\\Software\\Microsoft\\Windows NT\\CurrentVersion\\AppCompatFlags\\Layers\" /v \"{j}\" /f"
            ));
        }
    }

    json!({ "applied": applied, "skipped": skipped })
}

// ------------------------------------------------------------------
// PHASE 5 — NVIDIA profile (best-effort) + report + rollback
// ------------------------------------------------------------------
fn phase5_nvidia(log: LogFn, gpu: &str, backup_dir: &Path) -> Value {
    if !gpu.to_lowercase().contains("nvidia") {
        log("[Optimizer] Phase 5: no NVIDIA GPU — driver override skipped");
        return json!({ "skipped": true, "reason": "no NVIDIA GPU" });
    }
    log("[Optimizer] Phase 5: NVIDIA Profile Inspector");

    // locate nvidiaProfileInspector.exe
    let mut tool: Option<PathBuf> = None;
    let mut candidates: Vec<PathBuf> = Vec::new();
    for k in ["ProgramFiles", "ProgramFiles(x86)"] {
        if let Some(p) = env_path(k) {
            candidates.push(p);
        }
    }
    candidates.push(local_appdata().join("Programs"));
    candidates.push(backup_dir.to_path_buf());
    for r in &candidates {
        let mut sub = Vec::new();
        walk_find(r, "nviaprofileinspector.exe", 4, &mut sub, 5);
        walk_find(r, "nvidiaprofileinspector.exe", 4, &mut sub, 5);
        if let Some(f) = sub.into_iter().next() {
            tool = Some(f);
            break;
        }
    }

    // download if missing
    if tool.is_none() {
        log("[Optimizer]   Tool not found — downloading latest release …");
        let zip = backup_dir.join("nvpi.zip");
        let dl = format!(
            "Invoke-WebRequest -Uri 'https://github.com/Orbmu2k/nvidiaProfileInspector/releases/latest/download/NvidiaProfileInspector.zip' \
             -OutFile '{}' -UseBasicParsing",
            zip.display()
        );
        match ps(&dl) {
            Ok(_) => {
                let extract = format!(
                    "Expand-Archive -LiteralPath '{}' -DestinationPath '{}' -Force",
                    zip.display(),
                    backup_dir.display()
                );
                let _ = ps(&extract);
                let mut sub = Vec::new();
                walk_find(backup_dir, "nvidiaprofileinspector.exe", 3, &mut sub, 5);
                walk_find(backup_dir, "nviaprofileinspector.exe", 3, &mut sub, 5);
                tool = sub.into_iter().next();
                if tool.is_none() {
                    log("[Optimizer]   Download/extract failed — manual step needed");
                }
            }
            Err(e) => log(&format!("[Optimizer]   download failed: {e}")),
        }
    }

    // write a .nip profile for javaw.exe (best-effort)
    let nip = backup_dir.join("isolate_javaw.nip");
    let nip_xml = format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<Profiles>
  <Profile>
    <ProfileType>BaseProfile</ProfileType>
    <ProfileName>javaw.exe</ProfileName>
    <Commands>
      <StringListCommand>{id_pow} = 0x00000001</StringListCommand>
      <StringListCommand>{id_thr} = 0x00000001</StringListCommand>
      <StringListCommand>{id_llu} = 0x00000004</StringListCommand>
    </Commands>
  </Profile>
</Profiles>
"#,
        id_pow = NVIDIA_POWER_MGMT_ID,
        id_thr = NVIDIA_THREADED_ID,
        id_llu = NVIDIA_LOWLATENCY_ID
    );
    let _ = fs::write(&nip, nip_xml);

    let mut applied = false;
    if let Some(t) = &tool {
        // try silent import: `-s <file>`, then the bare file argument
        let attempts: [Vec<String>; 2] = [
            vec![
                t.to_string_lossy().to_string(),
                "-s".into(),
                nip.display().to_string(),
            ],
            vec![t.to_string_lossy().to_string(), nip.display().to_string()],
        ];
        for args in attempts {
            let prog = args[0].clone();
            let rest: Vec<&str> = args[1..].iter().map(|s| s.as_str()).collect();
            if run(&prog, &rest).is_ok() {
                applied = true;
                break;
            }
        }
        if applied {
            log("[Optimizer]   .nip profile applied (max perf, threaded on, low-latency ultra)");
        } else {
            log("[Optimizer]   .nip import CLI failed — open the tool manually with the saved profile");
        }
    }

    json!({
        "skipped": !applied,
        "tool": tool.map(|t| t.display().to_string()),
        "nip": nip.display().to_string(),
        "settings": [
            "Power Management Mode = Prefer Maximum Performance",
            "Threaded Optimization = On",
            "Low Latency Mode = Ultra",
        ],
    })
}

fn write_report_and_rollback(
    log: LogFn,
    audit: &Value,
    changes: &[String],
    rollbacks: &[String],
    backup_dir: &Path,
) -> (String, String) {
    let mut report = String::new();
    report.push_str("Isolate Client — Windows Minecraft Optimization Report\r\n");
    report.push_str(&format!(
        "Generated: {}\r\n\r\n",
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
    ));
    report.push_str("== AUDIT ==\r\n");
    report.push_str(&format!(
        "CPU: {}\r\nCores/Threads: {} / {}\r\nRAM: {} GB\r\nGPU: {}\r\nOS: {} (build {})\r\n\r\n",
        audit.get("cpu").and_then(|v| v.as_str()).unwrap_or("?"),
        audit.get("cores").and_then(|v| v.as_u64()).unwrap_or(0),
        audit.get("threads").and_then(|v| v.as_u64()).unwrap_or(0),
        audit.get("ram_gb").and_then(|v| v.as_f64()).unwrap_or(0.0),
        audit.get("gpu").and_then(|v| v.as_str()).unwrap_or("?"),
        audit.get("os_caption").and_then(|v| v.as_str()).unwrap_or("?"),
        audit.get("os_build").and_then(|v| v.as_str()).unwrap_or("?"),
    ));
    report.push_str("== CHANGES ==\r\n");
    for (i, c) in changes.iter().enumerate() {
        report.push_str(&format!("  {:2}. {c}\r\n", i + 1));
    }
    report.push_str(&format!(
        "\r\n== BACKUP ==\r\n  {}\r\n",
        backup_dir.display()
    ));
    report.push_str(
        "\r\n== BIOS CHECKLIST (manual, on reboot) ==\r\n  1. Enable XMP / EXPO memory profile (RAM at rated speed)\r\n  2. Enable Resizable BAR + Above 4G Decoding\r\n",
    );

    let mut rb = String::new();
    rb.push_str("# Isolate Client — optimization rollback\r\n");
    rb.push_str("# Run in an elevated (Administrator) PowerShell.\r\n");
    rb.push_str(&format!(
        "# Backup of originals: {}\r\n\r\n",
        backup_dir.display()
    ));
    for r in rollbacks {
        rb.push_str(r);
        rb.push('\n');
    }

    let report_path = PathBuf::from(BACKUP_ROOT).join("report.txt");
    let rollback_path = PathBuf::from(BACKUP_ROOT).join("rollback.ps1");
    let _ = fs::write(&report_path, &report);
    let _ = fs::write(&rollback_path, &rb);
    log(&format!(
        "[Optimizer]   Report → {} · Rollback → {}",
        report_path.display(),
        rollback_path.display()
    ));
    (report_path.display().to_string(), rollback_path.display().to_string())
}

// ------------------------------------------------------------------
// entry points
// ------------------------------------------------------------------
/// Full suite — Phases 1-5. Called on a background thread at game launch
/// and by the `run_system_optimizer` command. Returns a JSON report.
pub fn run_full(log: LogFn) -> Value {
    if !is_win() {
        return json!({
            "ok": false,
            "skipped": true,
            "platform": std::env::consts::OS,
            "reason": "Windows-only optimization suite",
        });
    }

    let mut changes: Vec<String> = Vec::new();
    let mut rollbacks: Vec<String> = Vec::new();

    // Phase 1
    let (audit, c1, r1, cfg_files, _launchers, javaw_files) = phase1(log);
    changes.extend(c1);
    rollbacks.extend(r1);
    let javaws: Vec<String> = javaw_files
        .iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect();

    // Phase 2
    let (java_report, c2, _r2, java21) = phase2_java(log, &javaws);
    changes.extend(c2);

    // combine javaw lists (phase 1 + newly installed)
    let mut all_javaws: Vec<String> = javaws.clone();
    if let Some(j) = &java21 {
        if !all_javaws.iter().any(|x| x == j) {
            all_javaws.push(j.clone());
        }
    }

    // Phase 3 (creates the timestamped backup first — required before any edits)
    let ts = chrono::Local::now().format("%Y%m%d_%H%M%S");
    let backup_dir = PathBuf::from(BACKUP_ROOT).join(ts.to_string());
    let _ = fs::create_dir_all(&backup_dir);
    let p3 = phase3_backup_and_inject(
        log,
        &audit,
        &cfg_files,
        &java21,
        &backup_dir,
        &mut changes,
        &mut rollbacks,
    );

    // Phase 4
    let p4 = phase4_registry(log, &all_javaws, &mut changes, &mut rollbacks);

    // Phase 5
    let gpu = audit
        .get("gpu")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let p5 = phase5_nvidia(log, &gpu, &backup_dir);

    // report + rollback.ps1
    let (report_path, rollback_path) =
        write_report_and_rollback(log, &audit, &changes, &rollbacks, &backup_dir);

    log(&format!(
        "[Optimizer] Suite complete — {} change(s) recorded.",
        changes.len()
    ));
    for c in &changes {
        log(&format!("[Optimizer]   • {c}"));
    }

    json!({
        "ok": true,
        "summary": format!("{} changes; backup {}", changes.len(), backup_dir.display()),
        "audit": audit,
        "java": java_report,
        "phase3": p3,
        "phase4": p4,
        "phase5": p5,
        "changes": changes,
        "backup_dir": backup_dir.display().to_string(),
        "report_path": report_path,
        "rollback_path": rollback_path,
        "bios_checklist": [
            "Enable XMP / EXPO RAM profile in BIOS",
            "Enable Resizable BAR + Above 4G Decoding in BIOS",
        ],
    })
}

/// Called from launch_game on a worker thread — never blocks the launch.
pub fn run_on_launch(log: LogFn) {
    if !is_win() {
        return;
    }
    log("[Optimizer] Running Windows optimization suite in background…");
    let r = run_full(log);
    let summary = r
        .get("summary")
        .and_then(|v| v.as_str())
        .unwrap_or("finished");
    if r.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) {
        log(&format!("[Optimizer] Done — {summary}"));
    } else {
        log("[Optimizer] Finished with issues — see report.txt");
    }
}

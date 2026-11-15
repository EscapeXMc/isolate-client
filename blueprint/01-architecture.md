# Xryloth — End-to-End Architecture Blueprint

> Source: `/mnt/Data Drive/EscapeX Products/Xryloth/XrylothTauri/` (branch state: v7.0.0.1 marketing / 6.0.0 Cargo).
> Every claim below is read from source. `file:line` refs point into that tree.

---

## 1. What Xryloth actually is

A **desktop Minecraft launcher + ecosystem** built around a Tauri (Rust + React) app:

| Layer | Tech | Size |
|---|---|---|
| Native core / backend | Rust, Tauri 1.6, `src-tauri/src/lib.rs` | **11,944 lines**, 174 Tauri commands |
| Installer binary (second Rust bin) | `src-tauri/src/installer/main.rs` | 959 lines |
| UI | React 18 + TypeScript + Vite 5 + Tailwind 3 + Zustand + Framer Motion, `src/` | **25,996 lines** across 93 files |
| In-game client mod | Fabric mod `minecraft-mods/xryloth-menu` | 3,245 lines Java |
| Server plugins | 8 Bukkit/Paper plugins `minecraft-plugins/` | 5,912 lines Java |
| Website | `xryloth-website-source/` | separate app |
| Second Tauri app | `DriveVault/` | unrelated utility |
| Android wrapper | `insight-wrap-android/` | — |
| Voice/TTS + AI scripts | `backend/engine.py`, `kokoro_*.py`, `indic-tiny-tts-hindi/` | experimental |

The launcher is distributed as **two separate products with separate release trains** (README.md):

- **Launcher** → repo `EscapeXMc/xryloth`
- **Installer** → repo `EscapeXMc/xrylothInstaller` (its updater reads the launcher repo's releases)

---

## 2. High-level component map

```
┌──────────────────────────────────────────────────────────────────────┐
│                         REACT WEBVIEW (src/)                        │
│  App.tsx → MainLayoutShell → Sidebar / TitleBar / routed pages      │
│  zustand stores (stores/index.ts, serverStore.ts)                   │
│  lib/api.ts · lib/tauri.ts · lib/auth.ts · lib/firebase.ts           │
└───────────────┬──────────────────────────────────────────────────────┘
                │  window.__TAURI__.invoke(...)   (withGlobalTauri: true)
┌───────────────▼──────────────────────────────────────────────────────┐
│                     RUST CORE (src-tauri/src/lib.rs)                │
│  AppState{settings, instances, logs, game_output, discord,          │
│           java_cache, server_procs, ai_context, ...}                │
│                                                                      │
│  174 #[tauri::command]s grouped as:                                  │
│   settings · instances · auth · modrinth · launch · java ·          │
│   server_mgr_* (60+) · ai_chat · updater · boost · fs/shell ·      │
│   discord rpc · image cache · notifications                         │
└──┬──────────────┬───────────────┬───────────────┬───────────────────┘
   │              │               │               │
   ▼              ▼               ▼               ▼
 spawn java    HTTP APIs      local disk      child processes
 Minecraft     Mojang/Fabric  ~/.xryloth      • game (java -cp …)
               Modrinth/GitHub  instances.json • local MC servers
               Groq/Gemini     settings.json   • updater scripts
               MS/Ely.by       playtimes.json  • taskkill/reg (boost)
               Firebase        image_cache/
```

### Key architectural decisions

1. **Tauri 1.x with `withGlobalTauri: true`** (`src-tauri/tauri.conf.json:14`) — the frontend calls `window.__TAURI__.invoke` directly; there is no generated TS bindings layer.
2. **Flat JSON files, no database.** Everything lives under `~/.xryloth/` (see §4).
3. **Blocking work is wrapped manually.** Heavy commands use `tokio::task::block_in_place` + `std::thread::spawn` and report progress via `emit_all("install-progress", …)` rather than returning a value — so `install_instance` returns `{"status":"started"}` immediately (lib.rs:1366).
4. **Progress is event-driven, state is polled.** The UI listens to events (`install-progress`, `launch-failed`, `ai-chunk`, `ai-tool`, `update-log`, …) and calls `get_logs` / `get_game_output` to poll buffers.
5. **The whole allowlist is wide open** — `fs.scope: ["**"]`, `protocol.assetScope: ["**"]`, `shell.execute: true`, plus `run_system_command` / `read_file_content` / `write_file_content` commands. This is a trusted-local single-user app, not a sandboxed one. `security.csp: null`.

---

## 3. Runtime flow of the app

### 3.1 Boot sequence (`lib.rs:11736 run()`)

1. **Migrate legacy data**: `~/.eclauncher` → `~/.xryloth` if the new dir doesn't exist (11737-11751).
2. `ensure_dirs()` — creates `~/.xryloth`, `instances/`, `libraries/`, `assets/indexes`, `assets/objects`, `servers/` (285-293).
3. **Single-instance claim** over loopback TCP `127.0.0.1:51543` (11243+). Three outcomes: `Owner(listener)` / `HandedOff` (exit 0) / `Unavailable` (start anyway). Verbs: `xryloth-present`, `xryloth-takeover`; the updater passes `--xryloth-takeover`.
4. `tauri::Builder` → `.manage(AppState::new())` → `.invoke_handler(174 commands)`.
5. `.setup()`:
   - spawn the single-instance TCP server thread
   - `window.set_title("Xryloth")`, `unminimize/show/set_focus`
   - **runtime asset-protocol scope fix**: `tauri.conf.json`'s `assetScope ["**"]` doesn't match absolute `/home/...` paths under Tauri 1.x glob semantics, so `~/.xryloth` is allowed explicitly at runtime (11858-11864)
   - `present_running_window()` — X11 `_NET_ACTIVE_WINDOW` activation (11883), retried once after 1.5 s if still unfocused
   - Discord RPC auto-init after 2 s if `discord_rpc` is on (11898-11911)
   - **auto-start local servers** with `start_on_launch: true` after 4 s (11913-11938)
6. `run(tauri::generate_context!())`

### 3.2 Why `GDK_BACKEND=x11` is forced (`main.rs:9-19`)

Single-instance hand-off, `present_running_window` and startup activation all talk to the window through **Xlib** (WM_CLASS lookups, `_NET_ACTIVE_WINDOW`, unminimize). A native Wayland window is invisible to all of that. Every release up to v6 ran as XWayland; the dynamically-linked build pins `GDK_BACKEND=x11` explicitly **only if the user hasn't set it themselves**.

---

## 4. On-disk data model — `~/.xryloth/`

Everything is a flat file. Helpers: `lib.rs:271-293`, `6228-6232`.

```
~/.xryloth/
├── settings.json            # global key→value settings (HashMap<String,Value>)
├── instances.json           # array of instance records
├── instance_settings.json   # per-instance {ram_mb, jvm_args, java_path} keyed by name
├── playtimes.json           # {instance_name: seconds}
├── mod_profiles.json        # saved mod lists
├── servers.json             # multiplayer server list
├── custom_skin.png          # custom skin override
├── authlib-injector.jar     # for Ely.by skins
├── ai_keys.json             # optional Groq/Gemini keys
├── background.png / background.mp4
├── instances/<name>/        # one folder per profile
│   ├── <mc_version>.json    # Mojang version JSON (verbatim)
│   ├── <mc_version>.jar     # client jar
│   ├── fabric-loader-<v>.jar + fabric-loader-<v>-profile.json
│   ├── natives/  mods/  saves/  resourcepacks/  screenshots/  config/  assets/
│   ├── icon.png, instance.json?
├── libraries/               # shared maven repo (all versions)
├── assets/{indexes,objects} # shared Mojang assets
├── java/                    # auto-downloaded JDKs
├── servers/<name>/          # local MC servers
│   ├── server.json          # type, version, port, ram_mb, motd, start_on_launch…
│   ├── server.properties, logs/, backups/, plugins/, world*/
├── image_cache/             # disk cache for remote images (url→hash)
├── updates/                 # downloaded update packages
└── logs/ …
```

**Migration**: old dir was `~/.eclauncher` (lib.rs:11739).

### Instance record schema (created at `create_instance`, lib.rs:796-801)

```json
{
  "name": "MyPack",
  "version": "1.20.1",
  "loader": "fabric",
  "loader_version": "0.15.11",
  "created": "2026-…",
  "last_played": null,
  "icon": "",
  // added on read by load_instances_raw (578-594):
  "installed": true,
  "version_json_exists": true,
  // added on read by get_instances from instance_settings.json (761-786):
  "ram_mb": 4096, "jvm_args": "-XX:…", "java_path": "/…"
}
```

---

## 5. External network surface

Constants at `lib.rs:38-47`.

| Purpose | Endpoint |
|---|---|
| Version manifest | `https://piston-meta.mojang.com/mc/game/version_manifest_v2.json` |
| Asset objects | `https://resources.download.minecraft.net/{ab}/{hash}/{name}` |
| Fabric versions | `https://meta.fabricmc.net/v2/versions/game` (+ `/loader/{mc}` + `/loader/{mc}/{v}/profile/json`) |
| Quilt | `https://meta.quiltmc.org/v3/versions/loader/…` |
| Forge maven | `https://maven.minecraftforge.net/…/maven-metadata.xml` |
| NeoForge maven | `https://maven.neoforged.net/releases/…` |
| Modrinth | `https://api.modrinth.com/v2` |
| Launcher updates | `https://api.github.com/repos/EscapeXMc/xryloth/releases/latest` |
| MS OAuth (device code) | `login.microsoftonline.com/consumers/oauth2/v2.0/{devicecode,token}` |
| Xbox Live | `user.auth.xboxlive.com/user/authenticate` → `xsts.auth.xboxlive.com/xsts/authorize` |
| Minecraft services | `api.minecraftservices.com/authentication/login_with_xbox` → `/minecraft/profile` |
| Ely.by | `account.ely.by/oauth2/authorize`, `…/api/oauth2/v1/token`, `…/api/account/v1/info` |
| authlib-injector | `api.github.com/repos/yushijinhun/authlib-injector/releases/latest`, `https://authlib-injector.yushi.moe/artifact/latest.json` |
| AI primary | `https://api.groq.com/openai/v1/chat/completions` |
| AI fallback | `https://generativelanguage.googleapis.com/v1beta/models` |
| Online probe | `HEAD https://sessionserver.mojang.com` (2 s timeout, 10 s cache) — `is_connected()` 364-378 |
| Paper / Purpur / Fabric server jars | see `08-servers.md` |
| Java (Adoptium) | see `05-launch-pipeline.md` §Java |

Client UA: `Xryloth/5.0` (lib.rs:103).

---

## 6. Build & packaging

- `package.json` scripts: `dev`, `build` (`tsc && vite build`), `tauri:build:{x64,x86,linux,macos}`.
- Tauri bundles: `nsis`, `dmg`, `appimage`, `deb` (tauri.conf.json `bundle.targets`).
- `build.rs` bakes **API keys at compile time** from env vars or git-ignored `src-tauri/secrets.json` into `OUT_DIR/embedded_keys.rs` and `rustc-env` (build.rs:120-177). It also embeds the **Windows Common-Controls v6 manifest** so `TaskDialogIndirect` resolves (build.rs:12-47, 179-207) — with a linker fallback when cross-compiling via cargo-xwin without `llvm-rc`.
- Linux: `GDK_BACKEND=x11`, deb depends list includes gstreamer + libfuse2 (for AppImage).
- Version triple must be bumped in **three** places: `Cargo.toml` (semver → `APP_VERSION`), `tauri.conf.json` (`package.version`), `package.json`. The **marketing version** is a separate constant `APP_DISPLAY_VERSION = "7.0.0.1"` (lib.rs:37) used by the updater — release tags are 4-part (`v6.0.0.13`) and must be *numerically greater* than it.

See `11-build-release.md` for the full release matrix and regression scripts.

---

## 7. Cross-cutting design patterns (worth copying when rebuilding)

1. **Helper/command split.** Internal fns (`install_instance`, `launch_game`, `install_mod_inner`) do the work; thin `#[tauri::command]` wrappers exist mostly to move them onto `spawn_blocking`/`block_in_place`.
2. **Everything returns `serde_json::Value`**, usually `{"success":true,…}` or `{"error": "…"}` — no typed response structs. Errors are *values*, not `Result::Err` (except `ai_chat`, `uninstall_launcher`).
3. **Ring-buffer logs.** `log_entries` capped at 5000, `game_output` at 500 (lib.rs:313-328).
4. **Self-healing installs.** `extract_natives` re-runs on *every* launch (2010-2017); downloads skip files that already exist and are non-empty.
5. **Graceful degradation everywhere**: library download failures become *warnings* not errors (1354-1359); loader version auto-detect failures skip the loader with a warning (1343-1345).
6. **Version grouping UI**: `build_version_groups` buckets versions by `major.minor` for the picker (536-551); `loader_supports` gates loaders by MC version (524-534).
7. **Windows console suppression** via `CREATE_NO_WINDOW` on every child process (331-337, 2315-2320).

---

## 8. Document index

| File | Contents |
|---|---|
| `01-architecture.md` | this file — system map, data model, endpoints |
| `02-rust-core.md` | lib.rs internals: state, helpers, subsystems |
| `03-commands-reference.md` | all 174 Tauri commands, signatures, behavior |
| `04-frontend.md` | React app: pages, stores, lib layer, design system |
| `05-launch-pipeline.md` | install + launch end-to-end, Java, natives, classpath |
| `06-accounts-auth.md` | Microsoft / Ely.by / offline auth chains |
| `07-instances-mods.md` | instances, templates, Modrinth, modpacks, boost |
| `08-servers.md` | local server manager + self-host |
| `09-fabric-mod.md` | `xryloth-menu` client mod |
| `10-plugins.md` | the 8 Bukkit plugins |
| `11-build-release.md` | build scripts, releases, updater, installer bin |
| `12-ai-chat.md` | Groq/Gemini agent, 100+ tools, image cache |
| `13-peripherals.md` | website, DriveVault, Android, TTS |

# Xryloth → Rebuild Blueprint

An end-to-end, source-verified blueprint of **Xryloth**, a Tauri-based Minecraft launcher, written so someone can rebuild an equivalent client from scratch.

| | |
|---|---|
| **Source repo** | `/mnt/Data Drive/EscapeX Products/Xryloth/XrylothTauri/` |
| **State documented** | marketing `7.0.0.1` / Cargo `6.0.0`, branch HEAD `c78b994` (dirty tree) |
| **Scale** | Rust core **11,944 LOC / 174 Tauri commands** · React UI **25,996 LOC / 93 files** · Fabric client mod **3,245 LOC** · 8 Bukkit plugins **5,912 LOC** |
| **Stack** | Tauri 1.6 · React 18 · TypeScript · Vite 5 · Tailwind 3 · Zustand · Framer Motion · reqwest/tokio · Fabric · Paper |

Every claim in these docs was read from source and cites `file:line`. Nothing is invented; where behavior is a guess it says so.

---

## Reading order

```
Start here
   │
   ▼
01-architecture ──► 02-rust-core ──► 03-commands-reference (all 174)
   │                     │
   │                     ├──────────► 05-launch-pipeline   (install + play)
   │                     ├──────────► 06-accounts-auth     (MS / Ely.by / offline)
   │                     ├──────────► 07-instances-mods    (profiles, Modrinth, boost)
   │                     ├──────────► 08-servers           (server manager)
   │                     └──────────► 12-ai-chat           (Groq/Gemini agent)
   │
   ├──────────────────────► 04-frontend                   (the React app)
   │
   └─ ecosystem ─────────► 09-fabric-mod  (in-game menu mod)
                          10-plugins     (8 Paper plugins)
                          11-build-release (build, release, updater, installer)
                          13-peripherals (website, DriveVault, Android, TTS)
```

---

## Document index

| # | File | Contents | LOC |
|---|---|---|---|
| 01 | `01-architecture.md` | System map, component diagram, boot sequence, `~/.xryloth/` data model, full external endpoint table, build/packaging overview, design patterns to copy | 219 |
| 02 | `02-rust-core.md` | lib.rs internals: section map, constants, HTTP layer + cache, AppState, logging, Discord RPC, system info, updater internals, single-instance protocol, `run()` boot, cross-cutting gotchas | 263 |
| 03 | `03-commands-reference.md` | **All 174 Tauri commands** grouped by area with line, full signature and behavior; notes on the Value/error conventions | 399 |
| 04 | `04-frontend.md` | React app: boot, routing, layout shell, zustand stores, `lib/` layer (api/auth/firebase/chat/updater/…), every page, UI inventory, design tokens, command cross-reference, external endpoints | ~400+ |
| 05 | `05-launch-pipeline.md` | `install_instance` + `launch_game` end-to-end: version JSON, Java ladder + auto-download, library/natives/classpath, loader install (Fabric/Quilt/Forge/NeoForge), JVM args, events | 364 |
| 06 | `06-accounts-auth.md` | Microsoft device-code + XBL/XSTS/MC chain, Ely.by OAuth + authlib-injector, offline accounts, dual storage layers, launch-time auth selection, refresh-token gap | 171 |
| 07 | `07-instances-mods.md` | Instance model, 19 templates, Modrinth install/search, mrpack install/export, mod profiles/updates/dependencies/conflicts, perf-mod lists, Game Boost steps | 194 |
| 08 | `08-servers.md` | Multiplayer list + ping, EcSelfHost, the full `server_mgr_*` manager: create/start/stop/auto-restart, properties, backups, worlds, plugins, events, website mirroring | 205 |
| 09 | `09-fabric-mod.md` | `xryloth-menu` Fabric mod: build, mixins, title screen/glass/panorama/font, boosts screen, launcher↔mod communication, bundled optimization stack | 1233 |
| 10 | `10-plugins.md` | The 8 Paper plugins (HeartVault, SkullHarvest, NexusHub, AegisGuard, AurumEconomy, AccountShield, MinePie, Overencumbered): commands, perms, storage, Discord integrations, Modrinth publishing | ~1500+ |
| 11 | `11-build-release.md` | Repo layout, build scripts line-by-line, release/version-bump checklist, asset-naming + updater scoring contract, regression scripts, OS build matrix, installer binary, secrets, git history | 637 |
| 12 | `12-ai-chat.md` | Groq→Gemini agent: key resolution, system prompt, **216 tools** (how counted), tool selection scoring, execution dispatch, provider calls, event contract, image cache | 193 |
| 13 | `13-peripherals.md` | DriveVault, xryloth website, InsightWrap Android, backend TTS/voice stack, DEV_LOG/SESSION_NOTES history, stray assets | 458 |

---

## Ten facts that define the system

1. **Tauri 1.6 with `withGlobalTauri: true`** — the UI calls `window.__TAURI__.invoke` directly; no generated bindings.
2. **No database** — flat JSON under `~/.xryloth/` (`settings.json`, `instances.json`, `instance_settings.json`, `playtimes.json`, `servers.json`, `mod_profiles.json`), each write rewriting the whole file.
3. **174 commands, all returning `serde_json::Value`** — errors are `{"error": …}` values, not `Result::Err` (two exceptions).
4. **Install is fire-and-forget** — `install_instance` returns `{"status":"started"}` and streams `install-progress` events; the UI must listen, not await.
5. **Three release numbers** — `Cargo.toml` (6.0.0), `tauri.conf.json`, `package.json`, plus the independent marketing `APP_DISPLAY_VERSION = "7.0.0.1"` that release tags must exceed.
6. **Two products** — launcher (`EscapeXMc/xryloth`) and installer (`EscapeXMc/xrylothInstaller`); the installer is the updater.
7. **AI is an agent, not a chatbot** — Groq primary / Gemini fallback, 216 tools that drive instances *and* local servers, keys embedded at compile time.
8. **Server hosting is first-class** — a 60-command manager with auto-restart, backups, Modrinth plugin installs, and `start_on_launch`.
9. **Single-instance over TCP `127.0.0.1:51543`** with a greeting protocol; `GDK_BACKEND=x11` forced for Xlib window activation.
10. **Security posture is "trusted local user"** — `fs.scope: ["**"]`, `shell.execute: true`, `run_system_command`/`delete_path` commands, `security.csp: null`.

---

## Gotchas index (things to fix when rebuilding)

| # | Where | Issue |
|---|---|---|
| 1 | `05` §2.3 | Managed-Java paths hardcode `bin/java.exe` on **all** OSes (lib.rs:5840/5987/6045) — broken on Linux/macOS |
| 2 | `05` §3.2 | Native classifier selection only knows **Windows** names (lib.rs:628-683) |
| 3 | `05` §7 | **No checksum verification** on any download; "exists && non-empty" is the only guard |
| 4 | `05` §7 | Asset/library downloads are **sequential** — slow first install |
| 5 | `03` | `apply_optimizer` **drops map keys** when rewriting `options.txt` (lib.rs:3084) — corrupts the file |
| 6 | `03` | `set_java_for_instance` writes the **global** setting despite its name |
| 7 | `06` §7 | `refresh_token` is stored but **never used** — sessions expire silently |
| 8 | `06` §7 | Offline accounts get the **nil UUID**, not a name-derived offline UUID |
| 9 | `02` §15 | `ELYBY_CLIENT_SECRET` hardcoded in source (lib.rs:46); Groq/Gemini keys extractable from the binary |
| 10 | `02` §3.1 | `AppState.http_cache` is dead code (the static `HTTP_CACHE` is the real one) |
| 11 | `08` §7 | Server jars download with **no integrity check**; MediaFire self-host URL is scraped + hardcoded fallback |
| 12 | `07` §11 | `install_modpack` doesn't sanitize mrpack index `path`s (potential `../` traversal) |
| 13 | `11` | Version triple lives in **three files** + a fourth marketing constant — bump all four |
| 14 | `12` §9 | Default-on AI keys ship inside every binary; rotate and prefer per-user keys |

---

## How this was produced

- Full inventory of 523 non-noise files; every source file read (or read for its section) before writing.
- `lib.rs` outlined (559 signatures), all 174 `#[tauri::command]`s extracted and verified against `generate_handler!` by set-comparison (0 mismatches).
- Parallel deep-dives for frontend, Fabric mod, plugins, and build/peripherals; the Rust core was read directly.
- Citations are `file:line` against the source tree; line numbers were captured from the same reads that produced the prose.

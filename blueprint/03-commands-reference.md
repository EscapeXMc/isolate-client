# Xryloth — Tauri Command Reference (all 174)

Source: `src-tauri/src/lib.rs`. **174** `#[tauri::command]` functions are defined **and** all 174 are registered in `generate_handler![…]` inside `run()` (lib.rs:11771-11842) — verified by set-comparison (0 defined-but-unregistered, 0 registered-but-undefined).

- 41 of them use `#[tauri::command(async)]`; the rest are plain (Tauri runs them on the thread pool).
- **Return convention**: almost every command returns `serde_json::Value`. Success is `{"success": true, …}`; failure is `{"error": "…"}` as a *value* (not `Result::Err`). Exceptions that return `Result<Value, String>`: `ai_chat`, `uninstall_launcher`.
- **Injected params**: `app: State<AppState>` (shared state), `app_handle: tauri::AppHandle` / `handle` (needed to spawn tasks or `emit_all` events). `State` params never cross the IPC boundary.
- Heavy work is wrapped in `tokio::task::block_in_place` and often moved to `std::thread::spawn`, with progress streamed via `emit_all`.

---

## Settings (4)

| Command | Line | Signature | Behavior |
|---|---|---|---|
| `get_settings` | L728 | `app: State<AppState>` | Returns the whole settings map (`~/.xryloth/settings.json` via `load_settings`). |
| `save_settings` | L734 | `app, settings: Value` | Merges the given object into current settings, writes `settings.json`. → `{"success":true}` |
| `get_setting_cmd` | L747 | `app, key: String, default: Option<Value>` | Single key read with fallback (default `Null`). |
| `save_setting_cmd` | L753 | `app, key: String, value: Value` | Single key write to `settings.json`. |

## Instances (4)

| Command | Line | Signature | Behavior |
|---|---|---|---|
| `get_instances` | L760 | `app` | Returns `instances.json` records merged with per-instance `instance_settings.json` (`ram_mb`, `jvm_args`, `java_path`) — see lib.rs:761-786. |
| `create_instance` | L788 | `app, name, mc_version, loader, loader_version` | Appends a record to `instances.json` (`created` timestamp, `last_played: null`, `icon: ""`) and creates the instance folder skeleton (lib.rs:796-801). |
| `delete_instance` | L808 | `app, name` | Removes the record and recursively deletes `instances/<name>/`. |
| `install_instance` | L819 **[async]** | `app_handle, instance_name, mc_version, loader, loader_version` | **Returns `{"status":"started"}` immediately**; the whole install runs on a spawned thread and streams `install-progress` events (lib.rs:827-835, final emit at 1366). Full pipeline: `05-launch-pipeline.md`. |

## Minecraft versions (4)

| Command | Line | Signature | Behavior |
|---|---|---|---|
| `get_minecraft_versions` | L1372 **[async]** | — | Fetches Mojang `version_manifest_v2.json` (cached), returns versions + `latest` release/snapshot. |
| `get_loader_versions` | L1409 **[async]** | `mc_version, loader` | Loader-specific version lists (fabric / quilt / forge / neoforge / optifine) from Fabric meta, Quilt meta, Forge & NeoForge `maven-metadata.xml`. |
| `get_supported_loaders` | L1518 | `mc_version` | Applies `loader_supports()` gates (vanilla any, fabric/quilt ≥1.14, forge ≥1.4, neoforge ≥1.20.2, optifine ≥1.7 — lib.rs:524-534). |
| `get_fabric_versions` | L1527 **[async]** | — | Fabric *game* versions from `https://meta.fabricmc.net/v2/versions/game`. |

## Microsoft auth (3)

| Command | Line | Signature | Behavior |
|---|---|---|---|
| `authenticate_microsoft` | L1549 **[async]** | `app` | Starts the **device-code flow**: POST `login.microsoftonline.com/consumers/oauth2/v2.0/devicecode` with client `00000000402b5328`, scope `XboxLive.signin XboxLive.offline_access MinecraftWin32.app` (lib.rs:1559-1562); clears prior state and stores `user_code`/`verification_uri`/`device_code`. |
| `poll_microsoft_status` | L1638 | `app` | Returns the pending device-code info and/or the completed `ms_auth_result` so the UI can poll. |
| `complete_microsoft_auth_cmd` | L1696 | `app_handle, ms_token, refresh_token: Option<String>` | Runs the full exchange chain (token → Xbox Live user auth → XSTS → `api.minecraftservices.com/authentication/login_with_xbox` → `/minecraft/profile`), then saves the account into `settings.accounts[]`. |

## Ely.by auth (4)

| Command | Line | Signature | Behavior |
|---|---|---|---|
| `authenticate_elyby` | L1707 | `app` | Starts the Ely.by OAuth2 flow (`account.ely.by/oauth2/authorize`), stores state for polling. |
| `poll_elyby_auth` | L1760 **[async]** | `app` | Polls until the user completes the browser auth (`[Auth/Ely.by] Polling for auth code…`). |
| `complete_elyby_auth_cmd` | L1828 **[async]** | `app, code: String` | Exchanges the code at `account.ely.by/api/oauth2/v1/token`, fetches account info, saves an `elyby` account. |
| `create_offline_account` | L1836 | `app, username` | Creates an offline (`type: "offline"`) account with a generated UUID; logs `[Auth/Offline] Creating offline account: …`. |

## Authlib injector (1)

| Command | Line | Signature | Behavior |
|---|---|---|---|
| `ensure_authlib_injector` | L1879 **[async]** | — | Downloads `authlib-injector.jar` (GitHub `yushijinhun/authlib-injector` latest release → `authlib-injector.yushi.moe`) into `~/.xryloth/` if missing; used as `-javaagent:…=ely.by` at launch (lib.rs:2205). |

## Game launch (1)

| Command | Line | Signature | Behavior |
|---|---|---|---|
| `launch_game` | L1895 **[async]** | `app, app_handle, instance_name` | The whole launch pipeline (Java ladder, natives, classpath, auth, spawn, RPC, playtime). Returns `{"success":true,"message":"Game process started"}` after spawning. Documented in `05-launch-pipeline.md` §5. |

## Modrinth (6)

| Command | Line | Signature | Behavior |
|---|---|---|---|
| `search_modrinth` | L2618 **[async]** | `query, project_type?, loader?, mc_version?, limit?` | GET `https://api.modrinth.com/v2/search` with facets (project_type, categories=loader, versions=mc_version). |
| `get_modrinth_project` | L2649 **[async]** | `slug` | GET `/v2/project/{slug}`. |
| `get_modrinth_versions` | L2657 **[async]** | `slug, loader?, mc_version?` | GET `/v2/project/{slug}/versions`, filtered client-side by loader/MC version. |
| `install_mod` | L2745 | `app_handle, instance_name, slug, version_id?` | Resolves version (default: latest compatible), downloads primary file into `instances/<n>/mods/`. |
| `install_resourcepack` | L2757 **[async]** | `app, instance_name, slug, version_id?` | Same but into `resourcepacks/`. |
| `install_modpack` | L2793 **[async]** | `app_handle, instance_name, slug, version_id?` | Downloads the version's primary file (`.mrpack`), parses `modrinth.index.json`, downloads each index file entry to its `path`, and extracts remaining zip entries into the instance (lib.rs:2821-2876). → `{"success":true}` |

## News / status (3)

| Command | Line | Signature | Behavior |
|---|---|---|---|
| `get_minecraft_news` | L2884 **[async]** | — | Reuses the Mojang version manifest to synthesize a "news" list of the first 10 versions (title = `Minecraft <id> (<type>)`). |
| `fetch_partners_remote` | L2922 **[async]** | — | Fetches the remote partners list (used by Partners page; overrides the bundled copy). |
| `get_server_status` | L2950 **[async]** | `host, port: String` | Minecraft server ping (status protocol) with multiple candidate resolutions; returns `{online, host, port, …}`; offline responses cached as fallback (lib.rs:3001-3016). |

## Skin manager (2)

| Command | Line | Signature | Behavior |
|---|---|---|---|
| `get_skin_info` | L3026 | `app` | Reports current skin state (custom skin file presence / active account skin). |
| `set_custom_skin` | L3036 | `file_path` | Copies the chosen image to `~/.xryloth/custom_skin.png`. |

## In-instance "optimizer" (2)

| Command | Line | Behavior |
|---|---|---|
| `get_optimizer_status` | L3046 | Parses `instances/<n>/options.txt` and returns `render_distance` (default 12), `simulation_distance` (12), `fps_limit` (`"261"`), `vsync` (`"true"`), `gamma` (`"1.0"`) (lib.rs:3047-3064). |
| `apply_optimizer` | L3066 | Merges the given keys into the parsed `options.txt` map and rewrites the file. ⚠️ **Bug**: the write loop is `for (k, v) in &existing { content.push_str(&format!(":{}\n", v)); }` — the **key `k` is dropped**, so applying the optimizer writes value-only lines (`:12`) and corrupts `options.txt` (lib.rs:3083-3085). |

## Launcher updates (3)

| Command | Line | Signature | Behavior |
|---|---|---|---|
| `check_updates` | L3226 **[async]** | — | Thin wrapper over `release_info()` (GitHub `EscapeXMc/xryloth/releases/latest`). |
| `check_launcher_update` | L10564 **[async]** | `_app` | Newer update path: scores release assets (`asset_score`, lib.rs:3123) against the current platform, compares against `APP_DISPLAY_VERSION`, emits update events. |
| `download_update` | L10642 **[async]** | `app, url, filename` | Downloads the chosen asset into `~/.xryloth/updates/` with `update-progress`/`update-log` events → `{success, path, size}`. |
| `install_update` | L10676 | `app, path` | Platform-specific apply: Windows runs the setup `.exe` detached (`DETACHED_PROCESS`), Linux/macOS stage-and-replace the running **AppImage** (or binary) and relaunch — see lib.rs:10720-10782. |

## System / logs / playtime (6)

| Command | Line | Signature | Behavior |
|---|---|---|---|
| `get_system_info` | L3235 | — | CPU %, RAM used/total/%, disk. Windows uses `wmic` on a 5 s-timeout background channel (deprecated-but-fast), Linux/macOS use `/proc`/`vm_stat`/`df` (lib.rs:3236-3411). |
| `get_logs` | L3411 | `app` | Returns the launcher log ring buffer (cap 5000, lib.rs:313-328). |
| `clear_logs` | L3417 | `app` | Empties it. |
| `get_game_output` | L3423 | `app` | Returns the game stdout/stderr ring buffer (cap 500). |
| `clear_game_output` | L3429 | `app` | Empties it. |
| `get_play_times` | L3436 | — | Returns `playtimes.json` (`{instance: seconds}`). |
| `get_game_logs` | L10805 | `app, instance_name` | Reads `instances/<n>/logs/latest.log` (and crash reports) from disk. |

## Instance management (15)

| Command | Line | Signature | Behavior |
|---|---|---|---|
| `open_instance_folder` | L3458 | `instance_name` | Opens `instances/<n>` in the OS file manager. |
| `open_instance_path` | L3472 | `instance_name, sub_folder` | Opens a sub-folder (`mods`, `saves`, …). |
| `get_instance_settings` | L3491 | `instance_name` | Reads `instance_settings.json[name]` (`ram_mb`, `jvm_args`, `java_path`). |
| `save_instance_settings` | L3507 | `app, instance_name, ram_mb: i64, jvm_args, java_path, …` | Writes those per-instance settings. |
| `clear_instance_cache` | L3540 | `app, instance_name` | Deletes the instance's cache/natives so the next launch re-extracts. |
| `get_instance_mods` | L3587 | `instance_name` | Lists `mods/*.jar` with enabled state (`.disabled` suffix convention). |
| `toggle_mod` | L3611 | `instance_name, filename` | Renames `x.jar` ⇄ `x.jar.disabled`. |
| `delete_mod` | L3631 | `instance_name, filename` | Deletes the mod file. |
| `rename_instance` | L3643 | `app, old_name, new_name` | Renames folder + all `instances.json` references (incl. settings/playtimes keys). |
| `edit_instance` | L3666 | `app, instance_name, new_name, mc_version, loader, loader_version, …` | Updates version/loader metadata in place (reinstall needed to take effect). |
| `set_instance_icon` | L3715 | `instance_name, icon_data` | Writes base64 icon to `instances/<n>/icon.png`. |
| `get_instance_icon` | L3732 | `instance_name` | Returns the icon path/data. |
| `get_instance_resourcepacks` | L3744 | `instance_name` | Lists `resourcepacks/`. |
| `delete_resourcepack` | L3765 | `instance_name, filename` | Deletes one. |
| `create_instance_from_modpack` | L3777 **[async]** | `app_handle, name, mc_version, loader, loader_version, mrpack_url` | Creates the instance record, downloads `mrpack_url`, reads `modrinth.index.json` for loader/version + file list, extracts zip entries (lib.rs:3807-3855). → `{success, instance}` |

## Instance templates (3)

| Command | Line | Signature | Behavior |
|---|---|---|---|
| `get_instance_templates` | L4147 | — | Returns the built-in `get_builtin_templates()` list (lib.rs:3861-4147): id/name/description/icon/category/mc_version/loader/mods[] — e.g. `fabric-tech-reborn`, `fabric-ad-astra`, `fabric-vault-hunters`, `fabric-skyfactory`, all `1.20.1` + fabric `0.15.11`. |
| `apply_instance_template` | L4170 **[async]** | `app_handle, instance_name, template_id` | Installs each template mod from Modrinth by slug (skip if present), emits `template-progress`, returns `{success, installed, errors, template_name}` (lib.rs:4196-4247). |
| `get_minecraft_news_enhanced` | L4251 **[async]** | — | Parses `https://www.minecraft.net/en-us/articles/rss` (real RSS with media thumbnails) — the actual news feed used by the UI. |

## Version check (1)

| Command | Line | Behavior |
|---|---|---|
| `get_latest_release_tag` | L4335 **[async]** | Returns the newest GitHub release tag (used for "what version am I on" comparisons). |

## Custom background image/video (6)

| Command | Line | Signature | Behavior |
|---|---|---|---|
| `save_background_image` | L4361 | `app, data` | Accepts a `data:` URI, decodes base64, writes `~/.xryloth/backgrounds/custom.{png,jpg,gif,webp}` (ext inferred from MIME). |
| `get_background_image_path` | L4380 | — | Returns the saved path (if any). |
| `delete_background_image` | L4390 | — | Removes it. |
| `save_background_video` | L4401 | `data` | Same for video → `background.mp4`. |
| `get_background_video_path` | L4420 | — | Returns path. |
| `delete_background_video` | L4430 | — | Removes it. |

## Discord RPC (5)

| Command | Line | Signature | Behavior |
|---|---|---|---|
| `init_discord_rpc` | L4441 **[async]** | `app: tauri::AppHandle` | Connects to Discord IPC (unix socket / named pipe, `discord_connect` lib.rs:117), spawns the RPC writer thread, sets the "Browsing Xryloth" activity. |
| `update_discord_rpc` | L4650 **[async]** | `app, details, state, large_image, large_text, small_image, small_text, …` | Sends a SET_ACTIVITY frame (nonce per call). |
| `stop_discord_rpc` | L4679 **[async]** | `app` | Clears activity and drops the handle. |
| `get_discord_status` | L4691 **[async]** | `app` | Whether the RPC connection is live. |
| `open_url` | L4704 | `url` | Opens a URL with the OS handler (webbrowser crate). → `{"success":true}` |

## Performance mods (1)

| Command | Line | Signature | Behavior |
|---|---|---|---|
| `install_performance_mods` | L4895 **[async]** | `app, instance_name, mc_version, loader` | Installs the curated perf-mod set for the instance's loader (sodium/lithium/ferritecore/ImmediatelyFast/entityculling/dynamic-fps/… — lists at lib.rs:4710-4841), skipping ones already present. Used by `game_boost`. |

## Public IP (1)

| Command | Line | Behavior |
|---|---|---|
| `get_public_ip` | L5001 **[async]** | Queries public-IP services (tries several; returns `{success, ip}` or an error string) — used for public server listing. |

## Mod profiles (4)

| Command | Line | Signature | Behavior |
|---|---|---|---|
| `save_mod_profile` | L5039 **[async]** | `app, name, instance_name` | Snapshots the instance's current mod list into `~/.xryloth/mod_profiles.json`. |
| `load_mod_profiles` | L5097 | — | Lists saved profiles. |
| `delete_mod_profile` | L5103 | `name` | Removes one. |
| `apply_mod_profile` | L5113 **[async]** | `app, name, instance_name` | Reinstalls/removes mods so the instance matches the saved profile. |

## Mod maintenance (5)

| Command | Line | Signature | Behavior |
|---|---|---|---|
| `check_mod_updates` | L5189 **[async]** | `app, instance_name` | For each installed mod, queries Modrinth for a newer compatible version (section lib.rs:5188-5283). |
| `analyze_crash_log` | L5285 | `instance_name` | Scans `crash-reports/` and `logs/` for `crash-*` files, picks the newest, and runs pattern analysis (lib.rs:5286-5347). |
| `resolve_mod_dependencies` | L5349 **[async]** | `app, instance_name, slug` | Reads the Modrinth project's dependencies and offers/install missing required mods (lib.rs:5348-5443). |
| `detect_mod_conflicts` | L5610 | `instance_name` | Rule-based conflict scan over installed mods (rules table lib.rs:5611-5712) — duplicate-mod / known-incompatible pairs. |
| `auto_enable_resourcepack` | L5445 | `instance_name, filename` | Inserts the pack into `options.txt` `resourcePacks` and marks it enabled. |

## Export (2)

| Command | Line | Signature | Behavior |
|---|---|---|---|
| `export_instance_mrpack` | L5497 **[async]** | `app, instance_name, name, version_id, output_path` | Builds a `.mrpack` (modrinth.index.json + mods) from the instance (lib.rs:5496-5608). |
| `export_instance_config` | L10495 | `app, instance_name, output_path` | Writes the instance *record* JSON (name/version/loader/…) to a file. |
| `import_instance_config` | L10513 | `app, file_path` | Reads that JSON; requires `name` + `version`, refuses duplicate names, creates the instance. |

## First-launch wizard (2)

| Command | Line | Behavior |
|---|---|---|
| `check_first_launch` | L5713 | `{"first_launch": !~/.xryloth/.wizard_done.exists()}` |
| `complete_first_launch` | L5719 | Writes `~/.xryloth/.wizard_done`. |

## Java (4)

| Command | Line | Signature | Behavior |
|---|---|---|---|
| `check_java_compatibility` | L6078 **[async]** | `app, mc_version` | `{required_java, current_java_version, current_compatible, available_javas, compatible_javas, managed_java_dir}` (details `05-launch-pipeline.md` §2). |
| `download_java` | L6114 **[async]** | `app, major_version: u32` | `auto_download_java()` from Adoptium into `~/.xryloth/java/java-<major>`. ⚠️ validates `bin/java.exe` on all OSes (lib.rs:6045). |
| `get_java_installations` | L6128 | — | `find_all_java_installations()` (30 s cache in `AppState.java_cache`). |
| `set_java_for_instance` | L6133 | `app, java_path` | ⚠️ Despite the name it writes the **global** `settings.java_path`, not `instance_settings.json`. |

## System terminal access — used by Xryloth AI (5)

Wide-open local FS/shell API (paired with the Tauri `shell.execute` allowlist):

| Command | Line | Signature | Behavior |
|---|---|---|---|
| `run_system_command` | L6140 | `command: String` | Runs through the shell hidden (`cmd /C` / `sh -c`), returns stdout+stderr+code. |
| `read_file_content` | L6167 | `path` | Reads a text file. |
| `write_file_content` | L6175 | `path, content` | Writes a text file. |
| `list_directory` | L6186 | `path` | Lists entries with sizes/types. |
| `delete_path` | L6209 | `path` | Deletes a file or directory. |

## Instance cloning (1)

| Command | Line | Signature | Behavior |
|---|---|---|---|
| `clone_instance` | L6234 | `app, instance_name, new_instance_name` | Copies the whole instance folder and appends a new record (section lib.rs:6233-6279). |

## Multiplayer server list (4)

| Command | Line | Signature | Behavior |
|---|---|---|---|
| `get_servers` | L6281 | — | Reads `~/.xryloth/servers.json`. |
| `add_server` | L6287 | `name, address, port: u16` | Appends `{name, address, port}`. |
| `delete_server` | L6303 | `name` | Removes it. |
| `ping_server` | L6352 | `address, port` | Full Minecraft status ping (protocol handshake, player sample, version, MOTD, favicon) — lib.rs:6352-6473. |

## Self-host (3)

| Command | Line | Signature | Behavior |
|---|---|---|---|
| `get_selfhost_status` | L6475 | — | Whether the EcSelfHost jar is downloaded / running. |
| `download_selfhost` | L6484 **[async]** | `app` | Downloads the self-host server jar. |
| `launch_selfhost` | L6593 | `app` | Spawns it. (MediaFire helper `extract_mediafire_download_url` at lib.rs:6534.) |

## Paper versions + legacy server launch (3)

| Command | Line | Signature | Behavior |
|---|---|---|---|
| `get_server_versions` | L6627 **[async]** | — | PaperMC API version list (lib.rs:6617-6654). |
| `get_server_builds` | L6654 **[async]** | `mc_version` | Builds for one MC version. |
| `launch_server` | L6681 | `app, app_handle, instance_name` | Launches a server *jar found inside an instance folder* (`find_server_jar` lib.rs:6778) — the older path, distinct from `server_mgr_*`. |

## Local server manager — core (15)

All operate on `~/.xryloth/servers/<name>/` with `server.json` metadata (see `08-servers.md`).

| Command | Line | Signature | Behavior |
|---|---|---|---|
| `server_mgr_list` | L6975 | `app` | Lists servers + status (running/idle). |
| `server_mgr_create` | L7018 | `app_handle, name, server_type, mc_version, port, motd?, ram_mb?` | Creates the folder, writes `server.json`, accepts EULA optionally later, downloads the server jar. |
| `server_mgr_start` | L7029 | `app, app_handle, name` | Spawns `java -Xmx… -jar <jar> nogui`, wires stdin, records `ServerProc {child, stdin, started_at}` (lib.rs:222-226), streams console lines as events. |
| `server_mgr_stop` | L7154 | `app, app_handle, name` | Graceful: writes `stop` to stdin, waits, then kills. |
| `server_mgr_kill` | L7189 | `app, app_handle, name` | Immediate kill. |
| `server_mgr_command` | L7204 | `app, name, command` | Writes a console command to stdin. |
| `server_mgr_status` | L7218 | `app, name` | `{running, pid, uptime, …}`. |
| `server_mgr_logs` | L7235 | `app, name` | Buffered console output. |
| `server_mgr_delete` | L7243 | `app, app_handle, name` | Stops (if running) and deletes the server folder. |
| `server_mgr_get_properties` | L7266 | `name` | Parses `server.properties` (escape/unescape helpers lib.rs:6798/6822). |
| `server_mgr_set_properties` | L7282 | `name, properties: HashMap<String,String>` | Rewrites `server.properties`. |
| `server_mgr_search_plugins` | L7315 **[async]** | `query, mc_version?, limit?` | Modrinth search filtered to `project_type=plugin`. |
| `server_mgr_install_plugin` | L7365 | `name, slug, version_id?, mc_version?` | Downloads the jar into `plugins/`. |
| `server_mgr_list_plugins` | L7374 | `name` | Lists `plugins/*.jar` (enabled state). |
| `server_mgr_delete_plugin` | L7396 | `name, filename` | Deletes one. |

## Local server manager — features expanded (22)

| Command | Line | Signature | Behavior |
|---|---|---|---|
| `server_mgr_server_types` | L7473 | — | Supported server types (paper/purpur/fabric/…). |
| `server_mgr_list_versions` | L7483 **[async]** | `server_type` | Version list per type. |
| `server_mgr_files_list` | L7524 | `name, subpath?` | Directory listing inside the server folder. |
| `server_mgr_file_read` | L7552 | `name, path` | Read a text file. |
| `server_mgr_file_write` | L7566 | `name, path, content` | Write a text file. |
| `server_mgr_file_delete` | L7578 | `name, path` | Delete. |
| `server_mgr_file_mkdir` | L7588 | `name, path` | mkdir -p. |
| `server_mgr_file_upload` | L7596 | `name, subpath?, filename, data` | Base64 upload. |
| `server_mgr_backup_list` | L7617 | `name` | Lists `backups/`. |
| `server_mgr_backup_create` | L7637 | `name, label?` | Zips world(s) into `backups/`. |
| `server_mgr_backup_restore` | L7679 | `app, name, file` | Stops server if running, restores the zip. |
| `server_mgr_backup_delete` | L7706 | `name, file` | Deletes a backup. |
| `server_mgr_plugin_toggle` | L7715 | `name, filename` | Move jar into/out of `plugins/disabled/` (`.disabled` convention). |
| `server_mgr_list_worlds` | L7735 | `name` | Lists `world*` folders. |
| `server_mgr_delete_world` | L7755 | `app, name, world` | Deletes a world folder (refuses the running one). |
| `server_mgr_set_meta` | L7770 | `name, ram_mb?, motd?, port?` | Updates `server.json` (+ regenerates `server.properties` motd/port). |
| `server_mgr_eula` | L7794 | `name, accept: bool` | Writes `eula.txt`. |
| `server_mgr_clone` | L7803 | `app, source, new_name, new_port?` | Copies the server folder, rewrites metadata/port. |
| `server_mgr_stats` | L7837 | `app, name` | CPU/RAM of the server process + world size. |
| `server_mgr_check_port` | L7855 | `port` | Is the TCP port free? |
| `server_mgr_next_port` | L7863 | `start?` | First free port from a base (default 25565). |
| `server_mgr_clear_logs` | L7874 | `app, name` | Truncates server logs. |

## Local server manager — extended tools (10)

| Command | Line | Signature | Behavior |
|---|---|---|---|
| `server_mgr_update_meta` | L7882 | `name, patch: Value` | Generic JSON-patch of `server.json`. |
| `server_mgr_list_java` | L7909 | — | Java installations usable for servers. |
| `server_mgr_open_folder` | L7914 | `name` | Opens the server folder. |
| `server_mgr_read_icon` | L7923 | `name` | Reads `server-icon.png` (base64). |
| `server_mgr_file_rename` | L7937 | `name, path, new_name` | Rename/move inside the server dir. |
| `server_mgr_file_copy` | L7955 | `name, path` | Copy file. |
| `server_mgr_file_read_b64` | L7988 | `name, path` | Base64 read (binary-safe). |
| `server_mgr_file_download` | L8002 | `name, path` | Download a server file to the user's machine (via dialog path). |
| `server_mgr_reset_world` | L8021 | `app, name, world?` | Deletes the world so it regenerates. |
| `server_mgr_reinstall` | L8087 | `app_handle, name, version` | Re-downloads the server jar for a new version, keeping data. |

## Image disk cache (2)

| Command | Line | Signature | Behavior |
|---|---|---|---|
| `image_cache_get` | L8330 **[async]** | `app, url` | Downloads a remote image once into `~/.xryloth/image_cache/<hash>`; rejects non-http URLs; marks stale entries and re-emits `image-cache` events with the local path (lib.rs:8331-8369). |
| `image_cache_warm` | L8371 **[async]** | `app, urls: Vec<String>` | Prefetches a batch in the background, `{success, queued}`. |

## AI assistant (4)

| Command | Line | Signature | Behavior |
|---|---|---|---|
| `ai_chat` | L10301 | `app, handle, messages: Vec<Value>, context: Option<Value>` | The agent loop: builds the system prompt (`ai_system_prompt` lib.rs:8166), selects tools (`ai_tool_defs` 8433-8839, ~100 tools), calls **Groq** first then **Gemini** fallback (8937-9080), executes tools (`ai_execute_tool_blocking` 9287-10234) and streams `ai-chunk` / `ai-tool` / `ai-done` events. Full doc: `12-ai-chat.md`. |
| `ai_context_set` | L10419 | `kind, name` | Sets the active workspace (`server` / `instance`) injected into the system prompt. |
| `ai_context_get_app` | L10425 | `app` | Returns current context. |
| `ai_context_clear_app` | L10430 | `app` | Clears it. |

## Pins, screenshots, batch (3)

| Command | Line | Signature | Behavior |
|---|---|---|---|
| `pin_instance` / `unpin_instance` | L10437 / L10450 | `app, instance_name` | Toggles the pinned flag (section lib.rs:10436-10463). |
| `get_screenshots` | L10464 | `app, instance_name` | Lists `instances/<n>/screenshots/*.png` with mtime/size. |
| `batch_install_mods` | L10542 **[async]** | `app, instance_name, slugs: Vec<String>` | Installs each slug (latest compatible), returns `{success, installed[], errors[]}` (lib.rs:10542-10562). |

## Game logs & file moves (3)

| Command | Line | Signature | Behavior |
|---|---|---|---|
| `get_game_logs` | L10805 | `app, instance_name` | Reads the instance's `logs/latest.log`. |
| `move_mod_file` | L10819 | `app, instance_name, source_path` | Copies an external file into `mods/` (returns new filename). |
| `move_resourcepack_file` | L10835 | `app, instance_name, source_path` | Same into `resourcepacks/`. |

## Game boost (1)

| Command | Line | Signature | Behavior |
|---|---|---|---|
| `game_boost` | L10853 **[async]** | `app, instance_name` | Multi-step optimizer (lib.rs:10851-11084): optional auto perf-mod install (`perf_auto_mods`, default **on**), RAM/JVM tuning, Windows process priority/power-plan tweaks (`taskkill`/`reg`), etc. Each step gated by a settings flag; returns `{success, steps_completed, steps_total, details[]}`. |

## Notifications & sound (2)

| Command | Line | Signature | Behavior |
|---|---|---|---|
| `show_notification` | L11086 | `title, body` | OS notification: Windows → PowerShell WinRT toast (`ToastText02`, tag `Xryloth`), Linux → `notify-send`, macOS → `osascript` (lib.rs:11087-11123). |
| `play_notification_sound` | L11124 | — | Plays the bundled chime (Windows: PowerShell `[console]::beep`-style / sound player; returns `{"success":true}`). |

## Uninstall (1)

| Command | Line | Signature | Behavior |
|---|---|---|---|
| `uninstall_launcher` | L11156 | `delete_data: bool` → `Result<Value,String>` | Generates and runs a self-deleting script: Windows `.bat` (3 s delay → `del` install dir + Desktop shortcut + optional `~/.xryloth` → `del %~f0`) or POSIX `.sh` (`sleep 2`, `pkill -i -f launcher.appimage`, `rm -rf` + self-delete) — lib.rs:11157-11242. |

---

## Notes for rebuilders

1. **No typed IPC layer.** Every payload is hand-built `json!{}` — the frontend must know the shapes (see `04-frontend.md` for the call sites).
2. **Errors are values.** `invoke()` will *succeed* even when the operation failed; always check `resp.error` / `resp.success`.
3. **Async ≠ non-blocking.** `#[tauri::command(async)]` only moves the fn off the sync handler; the body still typically calls `block_in_place` + `thread::spawn` and returns early (`install_instance`, `ai_chat`, `install_modpack`).
4. **Security posture**: `run_system_command`, `read/write_file_content`, `delete_path` + `fs.scope: ["**"]` + `shell.execute: true` mean the webview has full machine access. If you rebuild, decide consciously whether the AI tool surface stays this open.
5. **Naming traps**: `set_java_for_instance` writes global settings; `get_minecraft_news` is a manifest-derived fake feed (the real one is `get_minecraft_news_enhanced`); `launch_server` (instance-jar path) is separate from `server_mgr_start`.
6. **Known bug**: `apply_optimizer` drops map keys when rewriting `options.txt` (lib.rs:3084).

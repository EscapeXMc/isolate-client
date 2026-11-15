# Xryloth — Servers (multiplayer list, self-host, local server manager)

Source: `src-tauri/src/lib.rs` lines 6280-8094 + related helpers. Line refs exact. Companion: `10-plugins.md` (the plugin ecosystem that runs *on* these servers).

There are **three distinct server subsystems**:

1. **Multiplayer server list** (`servers.json`, ping) — what the player joins from the client.
2. **Self-host (EcSelfHost)** — downloads/runs a separate `EcSelfHost.exe` binary (legacy path).
3. **Local server manager** (`server_mgr_*`, 60+ commands) — full in-launcher Minecraft server hosting. This is the main one.

---

## 1. Multiplayer server list (6280-6473)

| command | line | behavior |
|---|---|---|
| `get_servers` | 6281 | Reads `~/.xryloth/servers.json` (array of `{name, address, port}`). |
| `add_server` | 6287 | Appends `{name, address, port}` (validates name/address). |
| `delete_server` | 6303 | Removes by name. |
| `ping_server` | 6352 | **Full Minecraft status protocol client**: TCP connect → handshake (proto version, state 1) → status request → parses JSON (version, players online/max + sample, motd, favicon `data:image/png;base64,…`, modinfo). Returns a rich object for the server list UI. Handles the "server offline" response shape too. |

`get_server_status(host, port)` (2950) is a lighter sibling used for quick online checks (tries multiple candidate resolutions, caches the last offline response).

---

## 2. Self-host — EcSelfHost (6474-6616)

| command | line | behavior |
|---|---|---|
| `get_selfhost_status` | 6475 | Exists/running state of `~/.xryloth/selfhost/EcSelfHost.exe`. |
| `download_selfhost` | 6484 **[async]** | Scrapes a **MediaFire** page, extracts `downloadUrl` from the HTML (`extract_mediafire_download_url` 6534), falls back to a hardcoded direct link `https://download1.mediafire.com/lr3tqh08syf8tgu/EcSelfHost.exe` (6512), downloads via `download_file_blocking`. |
| `launch_selfhost` | 6593 | Spawns the exe (hidden, Windows only by nature). |

Helpers: `selfhost_dir()` / `selfhost_exe()` (6230-6231).

---

## 3. Paper version catalog (6617-6679)

- `get_server_versions` (6627) — GET `https://fill.papermc.io/v3/projects/paper`, returns MC versions.
- `get_server_builds(mc_version)` (6654) — GET `…/projects/paper/versions/{v}/builds`, returns build list.
- Legacy `launch_server(instance_name)` (6681) — finds a `paper|spigot|paperclip*.jar` **inside an instance folder** (`find_server_jar` 6778) and runs `java -jar <jar> --nogui` on a background thread. This is the *old* "turn a profile into a server" path, separate from `server_mgr_*`.

---

## 4. Local server manager — core

### 4.1 On-disk layout

```
~/.xryloth/servers/<name>/
├── server.json          # launcher metadata (below)
├── server.properties    # vanilla properties (port, motd, …)
├── eula.txt             # written "eula=true" at creation (6940)
├── server.jar           # the downloaded server jar
├── server-icon.png      # optional
├── plugins/             # Bukkit plugins (+ plugins/disabled/)
├── world*/              # worlds
├── logs/
├── xryloth_backups/     # zip backups (7637+)
└── …
```

**`server.json` schema** (written at creation, 6949-6956; extended by setters):

```json
{
  "name": "MyServer", "type": "paper", "version": "1.21.11",
  "port": 25565, "motd": "…", "ram_mb": 4096,
  "created": "RFC3339",
  // optional / added later:
  "xms_mb": 0, "java_path": "", "jvm_args": "", "aikar_flags": false,
  "auto_restart": false, "start_on_launch": false,
  "visibility": "private", "description": "", "tags": "",
  "max_players": 20, "published_at": ""
}
```

`server_mgr_list` (6976-7017) merges this with live process state → `{…meta, icon_exists, path, running}`.

### 4.2 Create — `server_mgr_create` (7018) → `create_server_blocking` (6893)

1. `sanitize_server_name` (6852), reject duplicates, mkdir.
2. Resolve jar URL by `server_type`:

| type | resolver | endpoint |
|---|---|---|
| `paper` (default) | `resolve_paper_download_url` 6874 | `https://fill.papermc.io/v3/projects/paper/versions/{v}/builds/latest` → `downloads["server:default"\|"server:mojang"].url` |
| `vanilla` | `resolve_vanilla_server_url` 6887 | Mojang manifest → version JSON → `downloads.server.url` |
| `purpur` | `resolve_purpur_download_url` 7452 | `https://api.purpurmc.org/v2/purpur/{v}` → `…/{build}/download` |
| `fabric` | `resolve_fabric_download_url` 7460 | Fabric **installer** list (`meta.fabricmc.net/v2/versions/installer`) → installer jar, run to produce the server jar |

3. `download_file_progress` (600 s timeout) with `server-mgr-download` events.
4. Writes `eula.txt` = `eula=true` and a default `server.properties` (6941-6947):

```
server-port={port}  motd={escaped}  online-mode=false  max-players=20
difficulty=easy  gamemode=survival  pvp=true  spawn-protection=0
view-distance=10  simulation-distance=10  allow-flight=true
enable-command-block=true  white-list=false  enforce-whitelist=false
query.port={port}
```

5. Saves `server.json` → `{"success":true,"server":meta}`.

> Note **`online-mode=false` by default** — these are "cracked-friendly" servers by design.

Available types (`server_mgr_server_types` 7473): `paper`, `purpur`, `fabric`, `vanilla`.

### 4.3 Start — `server_mgr_start` (7029) → `launch_local_server` (7036)

- Requires `server.json` (7040).
- **Java**: `server.json.java_path` if set & exists → else global `settings.java_path` → else `find_all_java_installations().first()` → else `"java"` (`resolve_server_java` 6860-6873).
- Command (7070-7086): `hidden_cmd(java)` + `-Xmx{ram_mb}M` (+ `-Xms{xms_mb}M` if set) + optional `jvm_args` + optional Aikar flags (`aikar_flags: true` → the `AIKAR_FLAGS` const, lib.rs:49-74 — **server-side** G1GC tuning) + `-jar server.jar nogui` (Windows) / `--nogui` (else).
- On spawn: inserts `ServerProc{child, stdin, started_at}` into `AppState.server_procs`, clears `server_logs[name]`, emits `server-mgr-status {server, running:true}` (7097-7107).
- **stdout thread** (7108-7138): every line → `forward_server_line` → appends to `server_logs[name]` (cap 2000) + emits `server-mgr-output {server, line}`. When stdout closes (process exit): removes the proc, emits `running:false`, and **if `auto_restart` is on and this wasn't a requested stop** (`server_stopping` set), sleeps 3 s and calls `launch_local_server` again (7121-7137).
- **stderr thread** (7139-7151): same forwarding.

### 4.4 Stop / kill / command

| command | line | behavior |
|---|---|---|
| `server_mgr_stop` | 7154 | Marks `server_stopping`, writes `stop\n` to stdin; **12 s watchdog** thread force-kills if still alive (7168-7177); emits `running:false`. |
| `server_mgr_kill` | 7189 | Immediate `child.kill()` + `wait()`. |
| `server_mgr_command` | 7204 | Writes `<command>\n` to stdin + flush. |
| `server_mgr_status` | 7218 | `{running, pid, uptime_seconds, …}` from `ServerProc.started_at`. |
| `server_mgr_logs` | 7235 | Returns buffered `server_logs[name]`. |
| `server_mgr_clear_logs` | 7874 | Clears the buffer (and/or server log files). |
| `server_mgr_delete` | 7243 | Stop-if-running + `remove_dir_all`. |

### 4.5 Properties, meta, EULA, clone, ports, stats

| command | line | behavior |
|---|---|---|
| `server_mgr_get_properties` | 7266 | Parse `server.properties` (escape/unescape helpers `escape_prop` 6798 / `unescape_prop` 6822 — handles `\:` escapes and Unicode escapes). |
| `server_mgr_set_properties` | 7282 | Rewrite keys in place (preserves unknown keys). |
| `server_mgr_set_meta` | 7770 | Update `ram_mb`/`motd`/`port` in `server.json` (+ mirror motd/port into `server.properties`). |
| `server_mgr_update_meta` | 7882 | Generic JSON patch of `server.json` (used by `server_set_*` AI tools, 9659+). |
| `server_mgr_eula` | 7794 | Write `eula=true/false`. |
| `server_mgr_clone` | 7803 | Copy folder → new name, rewrite `server.json` name/port (new_port optional). |
| `server_mgr_check_port` | 7855 | TCP bind test — is the port free? |
| `server_mgr_next_port` | 7863 | First free port ≥ `start` (default 25565). |
| `server_mgr_stats` | 7837 | Process RSS via `/proc/<pid>/status` `VmRSS:` on Linux (`read_process_rss_kb` 7440), uptime, world size; Windows returns reduced data. |
| `server_mgr_list_java` | 7909 | Javas usable for servers. |
| `server_mgr_open_folder` | 7914 | Open server dir in file manager. |
| `server_mgr_read_icon` | 7923 | `server-icon.png` as base64. |

### 4.6 Files, backups, worlds

| command | line | behavior |
|---|---|---|
| `server_mgr_files_list` | 7524 | Directory listing under the server dir (path-escape guarded). |
| `server_mgr_file_read` / `write` / `delete` / `mkdir` / `upload` | 7552/7566/7578/7588/7596 | Text file ops; upload takes base64 `data`. |
| `server_mgr_file_rename` / `copy` / `read_b64` / `download` | 7937/7955/7988/8002 | Rename/move, copy, binary read, download-out. |
| `server_mgr_backup_list` | 7617 | Lists `xryloth_backups/*.zip`. |
| `server_mgr_backup_create` | 7637 **[async]** | Zips the whole server dir (excluding `xryloth_backups/`) into `xryloth_backups/<label>.zip` (unique name). |
| `server_mgr_backup_restore` | 7679 **[async]** | **Refuses if running** ("Stop the server before restoring a backup."), unzips over the server dir. |
| `server_mgr_backup_delete` | 7706 | Deletes a zip. |
| `server_mgr_list_worlds` | 7735 | Lists `world*` dirs + sizes. |
| `server_mgr_delete_world` | 7755 | Deletes one (guards the active world). |
| `server_mgr_reset_world` | 8021 | Deletes world(s) so they regenerate; `world: null` = all. |
| `server_mgr_reinstall` | 8087 **[async]** | Re-download the server jar for a **new version**, keeping data/plugins/worlds. |

### 4.7 Plugins (Modrinth-backed)

| command | line | behavior |
|---|---|---|
| `server_mgr_search_plugins` | 7315 **[async]** | `search_modrinth(query, project_type="plugin", loader="paper", mc, limit)` → Modrinth `api.modrinth.com/v2/search`. |
| `server_mgr_install_plugin` | 7365 | `install_plugin_blocking` (7325): version resolution ladder — `paper`+MC → any loader+MC → any (7336-7344); downloads jar into `plugins/`. |
| `server_mgr_list_plugins` | 7374 | Lists `plugins/*.jar` (+ disabled). |
| `server_mgr_delete_plugin` | 7396 | Deletes. |
| `server_mgr_plugin_toggle` | 7715 | Moves jar ↔ `plugins/disabled/`. |
| `server_mgr_viaversion_install` (AI-only, via tools) | — | Installs ViaVersion for cross-version play. |

---

## 5. Events consumed by the UI

| event | payload | source |
|---|---|---|
| `server-mgr-status` | `{server, running: bool}` | start/stop/kill/exit (7107, 7134, 7177, …) |
| `server-mgr-output` | `{server, line}` | every console line (`forward_server_line` 6966) |
| `server-mgr-download` | progress during jar/plugin download | create/reinstall |

Boot-time: servers with `start_on_launch: true` are started ~4 s after the launcher opens (see `run()` 11913-11938).

---

## 6. Launcher ↔ website integration

`server_mgr_list`'s comment (7003-7006) states the launcher mirrors `visibility` / `description` / `tags` / `max_players` / `published_at` **to Firebase for the website server list** — i.e. `visibility: "public"` servers are published to the Xryloth website. `get_public_ip` (5001) supplies the address for that listing (tries several IP echo services; returns `{success, ip}`).

---

## 7. Gotchas

1. **Two server paths coexist** — `launch_server` (jar inside a *profile*) and `server_mgr_*` (managed servers folder). They don't share state.
2. **`online-mode=false` by default** and `spawn-protection=0` — convenient for cracked/offline play, unsafe for public exposure.
3. **`eula=true` is written automatically at creation** (6940) — the user never sees the Mojang EULA prompt (there is a `server_mgr_eula` command to revoke it).
4. **Backups zip the entire server dir** including `world/` — big servers = big zips; restore requires a stop, but nothing prevents restoring while the *launcher* thinks it's stopped and another process holds the port.
5. **Auto-restart only triggers when stdout closes without a requested stop** — a crashed JVM whose stdout pipe stays open (rare) won't restart; a manual `kill` does *not* auto-restart (it goes through `server_stopping`).
6. **MediaFire scraping** for EcSelfHost will break whenever MediaFire changes HTML; the hardcoded fallback URL (6512) is equally fragile.
7. **Fabric server type downloads the *installer* jar** and must run it to produce `server.jar` — verify that step actually executes `java -jar installer server` in your rebuild (the resolver at 7460 only fetches the installer list).
8. `server_stats` RSS is **Linux-only** (`/proc`); Windows falls back to `None`.
9. `server.properties` write path uses `escape_prop`/`unescape_prop` — non-ASCII MOTDs are written as `\uXXXX` (6811) to stay vanilla-compatible.

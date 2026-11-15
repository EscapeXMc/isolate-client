# Xryloth — Install & Launch Pipeline (end-to-end)

Everything here is read from `src-tauri/src/lib.rs`. Line refs are exact.

The pipeline has two halves:

1. **Install** — `install_instance()` `lib.rs:820-1369` (async wrapper) → inner thread
2. **Launch** — `launch_game()` `lib.rs:1896-2459`

Both are registered as `#[tauri::command(async)]` and wrapped in `tokio::task::block_in_place`.

---

## 0. Sequence overview

```
USER clicks "Create"
  └─ create_instance (789)            → writes instances.json + folder skeleton
  └─ install_instance (820)           → returns {"status":"started"} immediately
       thread:
        1  GET version_manifest_v2.json            (837-859)   progress 2
        2  GET version JSON for <mc_version>       (861-868)   progress 5
        3  mkdir instances/<name>/{mods,saves,resourcepacks,
              screenshots,natives,assets,config}   (870-874)
           save <mc_version>.json verbatim         (876)
        4  ensure compatible Java (live lookup)    (879-898)   progress 6-7
        5  download client jar (skip if non-empty) (900-930)   progress 8→15
        6  download libraries + natives            (932-1012)  progress 16→46
           + extract_natives() verify              (1010)
        7  asset index + asset objects             (1014-1074) progress 47→75
        8  install loader (fabric/quilt/forge/neoforge) (1076-1346) progress 76→85
        9  verify critical files → warnings        (1348-1364) progress 100
           emit "install-progress" {done:true, success, warnings[]}

USER clicks "Play"
  └─ launch_game (1896)
       1 load instance + version JSON
       2 resolve Java (4-step ladder, may auto-download)
       3 extract_natives (self-healing)
       4 build classpath (MC jar + libs + loader jar + profile libs)
       5 resolve RAM / JVM args (per-instance > global > default)
       6 resolve auth account (active account list > legacy flat keys)
       7 build game args + full command
       8 spawn background thread:
            setup CustomSkinLoader (offline) → pre-launch hook → spawn java
            → stream stdout/stderr → Discord RPC "Playing" → wait exit
            → Discord RPC "Browsing" → post-launch hook → save playtime
```

Progress is pushed as Tauri events:

```js
emit_all("install-progress", { step, message, progress, done, success, warnings })
```
(`lib.rs:830-835`)

---

## 1. Version manifest & version JSON

- Manifest: `MOJANG_VERSION_MANIFEST = https://piston-meta.mojang.com/mc/game/version_manifest_v2.json` (`lib.rs:40`)
- Find the version entry with matching `id`, GET its `url` (848-859).
- The **raw version JSON is stored inside the instance folder** as `instances/<name>/<mc_version>.json` (876) — not shared globally. Library jars and assets *are* shared globally (`~/.xryloth/libraries`, `~/.xryloth/assets`).

Loader version lists (`get_loader_versions` 1410, `get_minecraft_versions` 1373, `get_fabric_versions` 1528, `get_supported_loaders` 1519) also hit Mojang/Fabric/Quilt/Forge/NeoForge metas. Versions are bucketed with `build_version_groups()` (536) into `major.minor` groups for the UI picker, and loader availability is gated by `loader_supports()` (524):

| loader | min MC |
|---|---|
| vanilla | any |
| fabric / quilt | 1.14.0 |
| forge | 1.4.0 |
| neoforge | 1.20.2 |
| optifine | 1.7.0 |

---

## 2. Java: required version, discovery, auto-download

### 2.1 Required version — live, not hardcoded

`get_required_java_for_mc()` (5815-5827):
1. **Primary**: `fetch_java_major_from_web()` (5796) — fetches Mojang's version JSON and reads `javaVersion.majorVersion`, falling back to parsing `javaVersion.component` (`java_21`, `jre-legacy`→8). Results come through `http_get_cached` (10-minute TTL, `lib.rs:83-102`).
2. **Offline fallback** table:

```
>=1.21 → 21 | >=1.20.2 → 17 | >=1.17 → 16 | else → 8
```

### 2.2 Discovery — `find_all_java_installations()` (5829-5968)

Searches, in order:
1. **Managed**: `~/.xryloth/java/<dir>/bin/java.exe` (5840)
2. **System PATH**: `which_java()` (2461) — `where` on Windows, `java -version` probe elsewhere
3. **Windows well-known dirs**: `C:\Program Files\{Java, Eclipse Adoptium, Microsoft, AdoptOpenJDK, Zulu, BellSoft}` (5876-5912)
4. **macOS**: `/Library/Java/JavaVirtualMachines/*/Contents/Home/bin/java` (5914-5939)
5. **Linux**: `/usr/lib/jvm/*/bin/java` (5940-5965)

Each candidate is probed with `get_java_major_version()` (5370-5777):
- **First reads the PE header** (`get_exe_arch_from_pe_header` 5779-5792, machine `0x8664/0xAA64/0x014c`) and **returns `None` if the binary arch ≠ host arch** — never executes a wrong-arch binary (5732-5739).
- Then runs `java -version` with a **5 s kill timeout** (5746-5755) and parses the quoted version string's first component.

`get_java_arch()` (2501) prefers the PE header, else parses `os.arch` from `-XshowSettings:properties`.

Result record: `{path, major_version, name, managed, arch, compatible_arch}`.
Results are **cached for 30 s** in `AppState.java_cache` (6065-6076).

### 2.3 Auto-download — `auto_download_java()` (5981-6063)

- URL: `https://api.adoptium.net/v3/binary/latest/{major}/{os}/{arch}//jdk/hotspot/normal/eclipse` (5970-5979), `os ∈ {windows, mac, linux}`, `arch = get_host_arch()` (`x64`/`aarch64`).
- Downloads to a tempdir, unzips, finds the inner `jdk-*` dir, `rename`s it to `~/.xryloth/java/java-<major>`.
- Re-validates by running `bin/java.exe` and checking the major version; deletes the dir if it can't run ("wrong architecture?").
- On success the path is written back into **global** `java_path` setting (1978, 6119).

> ⚠️ **Gotcha**: the managed-Java path is hard-coded to `bin/java.exe` at 5840, 5987 and 6045 with no `cfg(target_os)`. On Linux/macOS the file is `bin/java`, so **managed JDKs downloaded on Linux are neither found by discovery nor validated after download** (`auto_download_java` would return "java.exe not found"). On Linux, discovery still works through `/usr/lib/jvm` and PATH.

### 2.4 Commands

- `check_java_compatibility(mc_version)` (6078) → `{required_java, current_java_version, current_compatible, available_javas, compatible_javas, managed_java_dir}`
- `download_java(major_version)` (6114)
- `get_java_installations()` (6128), `set_java_for_instance(java_path)` (6133) — note this writes the **global** `java_path` despite the name.

---

## 3. Libraries, natives, classpath

### 3.1 Library download (932-1012)

For every `libraries[]` entry with `downloads.artifact.{url,path}`:
- skip if `~/.xryloth/libraries/<path>` exists and is non-empty
- else `download_file_blocking` (120 s timeout, UA `Xryloth/5.0`)
- failures are collected into `lib_errors` and reported as **warnings**, not fatal (1354)

Natives are then resolved with `get_native_artifact()` (628-683) and also downloaded into `libraries/`.

### 3.2 Native artifact resolution — `get_native_artifact()` (628-683)

Two version-JSON formats are handled:
- **Format B (modern Mojang, 1.19+)** — a library whose `name` contains `:natives-…`; accepts `natives-windows`, `natives-windows-x86_64`, `natives-windows-x86`, `natives-windows-arm64`, matched against `get_host_arch()`.
- **Format A (classic/Forge)** — `downloads.classifiers`, candidate list chosen by arch:
  - aarch64 → `natives-windows-arm64`, `natives-windows`
  - x64 → `natives-windows-x86_64`, `natives-windows-x86`, `natives-windows`
  - else → `natives-windows-x86`, `natives-windows`

> ⚠️ **Gotcha**: only **Windows** classifier names are ever considered — there is no `natives-linux` / `natives-macos` branch anywhere in `lib.rs`. On a Linux host this (a) downloads/extracts Windows `.dll`s when a `natives-windows-x86_64` entry exists, and (b) finds nothing for `natives-linux` entries.

### 3.3 Extraction — `extract_natives()` (689-723)

Opens each native jar as a zip and writes every entry ending in `.dll|.so|.dylib|.jnilib` into `instances/<name>/natives/`. **Missing native jars are downloaded on the fly** (697-700) — this is the *self-healing* path: it runs on **every launch** (2010-2017), so a broken/never-finished install still gets its natives.

Return value is the count written; it's logged as `[Launch] Extracted N native file(s)`.

**Why Linux still works**: the JVM flags point every native-extraction mechanism at `natives/` (2144-2146):

```
-Djava.library.path=<natives>
-Djna.tmpdir=<natives>
-Dorg.lwjgl.system.SharedLibraryExtractPath=<natives>
```

and LWJGL3 platform jars are on the classpath as ordinary libraries, so LWJGL extracts its own `.so` files itself at startup. `extract_natives` is effectively belt-and-braces (and Windows-oriented).

### 3.4 Classpath construction (2020-2127)

Order, de-duplicated by absolute path string (`HashSet`, 2022-2030):

1. `instances/<name>/<mc_version>.jar` (only if present)
2. every `libraries/<downloads.artifact.path>` that exists and passes `is_lib_allowed()`, **excluding** basenames containing `fabric-loader` / `quilt-loader` (2048)
3. **Loader jar**: for Fabric/Quilt, the first `fabric-loader*.jar` / `quilt-loader*.jar` sitting *in the instance folder* (not `*profile*`) — hard error if missing: `"Fabric Loader JAR missing in instance folder!"` (2072-2074)
4. **Loader profile libraries**: the `*<loader>-profile.json` in the instance folder; each library resolved via `downloads.artifact.path` or via `maven_name_to_path(name)` (501-513), skipping entries whose basename contains the loader name (to avoid double-adding the loader jar)

Then `mainClass`:
- default from version JSON (2056), usually `net.minecraft.client.main.Main`
- overridden when a Fabric/Quilt profile parsed successfully (2114-2118):
  - Fabric → `net.fabricmc.loader.impl.launch.knot.KnotClient`
  - Quilt → `org.quiltmc.loader.impl.launch.knot.KnotClient`

Empty classpath → hard error `"Classpath is empty! Installation likely failed."` (2124).

### 3.5 `is_lib_allowed()` — OS rule filtering (601-621)

Implements Mojang's `rules[]` semantics:
- no rules → allowed
- rules with `features` are **ignored** (611)
- otherwise, last matching `os.name == {windows|osx|linux}` decides `allow/disallow`; default allow

### 3.6 `maven_name_to_path()` (501-513)

`group:artifact:version[:classifier]` → `group/as/slashes/artifact/version/artifact-version[-classifier].jar`.

---

## 4. Loader installation (1076-1346)

Skipped entirely when `loader == "vanilla"`.

### 4.1 Version auto-detection (when `loader_version` is empty)

| loader | source |
|---|---|
| fabric | `https://meta.fabricmc.net/v2/versions/loader/{mc}` → first entry with `loader.stable == true` (1082-1107) |
| quilt | `https://meta.quiltmc.org/v3/versions/loader/{mc}` → same (1085) |
| forge | `maven.minecraftforge.net/.../maven-metadata.xml`, first `<version>` starting with `>{mc}-` (1108-1148) |
| neoforge | `maven.neoforged.net/releases/.../maven-metadata.xml`, same scan but filtered to digits/dots (1149-1189) |

Empty result → warning `"Could not determine <loader> version, skipping loader install"` (1343-1345).

### 4.2 Installation

**Fabric (1200-1250)**
1. GET `https://meta.fabricmc.net/v2/versions/loader/{mc}/{v}/profile/json`
2. download every profile library (`url` + `maven_name_to_path(name)`) into `libraries/` if absent
3. copy the `fabric-loader-{v}` jar from `libraries/` → `instances/<name>/fabric-loader-{v}.jar`
4. save the profile as `instances/<name>/fabric-loader-{v}-profile.json`

**Quilt (1251-1301)** — identical against `meta.quiltmc.org/v3`.

**Forge / NeoForge (1302-1340)** — different strategy:
1. download `forge-{v}-installer.jar` / `neoforge-{v}-installer.jar` into the instance folder
2. open it as a zip and extract **only** `META-INF/libraries/**/*.jar` into `~/.xryloth/libraries/` (1318-1327)
3. delete the installer (1332)
4. no `profile.json` is produced — Forge launch therefore relies on the version JSON's own libraries plus whatever the installer unpacked (see `05` §5 note below).

### 4.3 Verification & warnings (1348-1364)

Checked: version JSON exists, client jar exists & non-empty, `lib_errors` empty.
Emits `{"success": true, "warnings": [...]}` or plain `{"success": true}` at `progress: 100, done: true`.

---

## 5. Launch — `launch_game()` (1896-2459)

### 5.1 Inputs & preconditions
- instance looked up by `name` in `instances.json` (1900-1905); missing → `{"error": "Instance X not found"}`
- version JSON must exist in the instance dir (1912-1915) → else `"Version JSON missing. Please reinstall."`

### 5.2 Java resolution ladder (1927-2008)

`required_java = get_required_java_for_mc(mc_version)`; `host_arch = get_host_arch()`.

Per-instance override file `~/.xryloth/instance_settings.json[name]` is read first (1924-1925):

1. **Custom path**: `instance_settings[name].java_path` beats global `settings.java_path` (1929-1931). Accepted only if the file exists **and** major == required **and** arch == host (1936-1943).
2. **Any discovered Java** with matching major **and** `compatible_arch` (1946-1957).
3. **Relax arch**: any Java with matching major (1959-1969).
4. **Auto-download** `auto_download_java(required)`; on failure → hard error naming the version/arch (1973-1984).
5. If a Java *was* found but has the wrong major → auto-download the right one anyway, falling back with a warning if that fails (1986-2002).

Final failure → `"No compatible Java found. Please install Java or check Settings -> Java."`

Logs: `[Launch/Java] Using: <path> (required: Java 21 for MC 1.21.11, arch: x64)`.

### 5.3 JVM arguments (2129-2181)

RAM precedence (2130-2133):

```
instance_settings[name].ram_mb (>=512)  >  settings.ram_mb  >  2048
```
`Xms = min(512, ram)`.

Base flags:

```
-Xmx{ram}M  -Xms{min}M
-XX:+UnlockExperimentalVMOptions -XX:+UseG1GC
-XX:G1NewSizePercent=20 -XX:G1ReservePercent=20
-XX:MaxGCPauseMillis=50 -XX:G1HeapRegionSize=32M
-Djava.library.path=<natives>
-Djna.tmpdir=<natives>
-Dorg.lwjgl.system.SharedLibraryExtractPath=<natives>
-Dminecraft.launcher.brand=Xryloth
-Dminecraft.launcher.version=5.0
```

Then, in order:
- `-javaagent:<authlib-injector.jar>=ely.by` if online **and** Ely.by account present (2151-2167; the jar is fetched on demand via `ensure_authlib_injector()` 1880)
- global `settings.global_jvm` split on whitespace (2170-2172)
- per-instance `instance_settings[name].jvm_args` (2174-2178)

> Note: `AIKAR_FLAGS` (lib.rs:49-74) are **server** flags, not used for the client.

### 5.4 Auth selection (2183-2253)

Two-stage:

**Stage A — active account list.** `settings.active_account_uuid` indexes into `settings.accounts[]` (2185-2189). Each account `{type, username, access_token, uuid}`:

| type | condition | result |
|---|---|---|
| `microsoft` | online && token non-empty & ≠ "0" | `(user, token, uuid, "mojang")` |
| `elyby` | online && token ok | same + inserts authlib-injector agent (2204-2206) |
| `offline` | always | `(user, "0", uuid, "legacy")` |
| anything else | — | fallback to `settings.cracked_username` / `Player` (2213-2217) |

**Stage B — legacy flat keys** if no account matched (2219-2252):
`ely_by_username` + `ely_access_token|mc_token` → Ely.by; else `username`+`mc_token` → Microsoft; else `cracked_username` → offline.

Full chain documented in `06-accounts-auth.md`.

### 5.5 Game arguments (2255-2274)

```
--version <mc> --gameDir <inst_dir> --assetsDir ~/.xryloth/assets
--assetIndex <assetIndex.id|mc> --accessToken <t> --username <u>
--uuid <uuid> --userType <mojang|legacy>
```

Final command:

```
java <jvm_args...> -cp <classpath joined by ";" on Windows else ":">
     <main_class> <game_args...>
```

`classpath_str` join: `cfg!(target_os = "windows") ? ";" : ":"` (2268).

`last_launch_<instance>` RFC3339 timestamp saved to settings (2281).

### 5.6 Process thread (2289-2454)

Runs in `std::thread::spawn`:

1. **CustomSkinLoader setup** if the session looks offline (`cracked_username` present && `mc_token == "0"`) → `setup_custom_skin_loader()` (2530) writes a `config/CustomSkinLoader` profile so offline players still get skins (2291-2296).
2. **Pre-launch hook**: `settings.pre_launch_cmd` run through `run_shell_hidden()` — `cmd /C` on Windows, `sh -c` elsewhere, all stdio nulled (2298-2303, 341-358).
3. `spawn()` with cwd = instance dir, piped stdout/stderr, stdin null, `CREATE_NO_WINDOW` on Windows (2307-2320).
4. On success:
   - logs PID
   - **Discord RPC → "Playing Minecraft (loader)" / "<instance> • <version>"**, connecting first if needed (2330-2359)
   - **two reader threads** drain stdout (prefix `[MC]`, DEBUG lines skipped in the log) and stderr (prefix `[MC/ERR]`) into `game_output` (cap 500) and `log_entries` (cap 5000) (2362-2393)
   - `proc.wait()` → non-zero exit logs `[MC/ERROR] Game exited with code N (ran for …)` (2395-2403)
   - Discord RPC → "Browsing Xryloth / In Launcher" (2405-2419)
5. On spawn failure: logs **the full command line** and emits `launch-failed` `{instance, error, command}` (2421-2434).
6. **Post-launch hook**: `settings.post_launch_cmd` (2437-2442).
7. **Playtime**: `elapsed` seconds added to `playtimes.json[instance]` (2444-2453).

Returns immediately after spawning: `{"success": true, "message": "Game process started"}` (2456).

---

## 6. Events consumed by the UI

| event | payload | emitted from |
|---|---|---|
| `install-progress` | `{step, message, progress, done, success, warnings[]}` | 831 |
| `launch-failed` | `{instance, error, command}` | 2429 |
| `update-log`, `update-progress` | text / `{downloaded,total,speed}` | 10573-10587 |
| `ai-chunk`, `ai-done`, `ai-tool`, `ai-status` | see `12-ai-chat.md` | 10311+ |

Polling companions: `get_logs`, `clear_logs`, `get_game_output`, `clear_game_output` (3411-3434), `get_game_logs(instance)` (10806).

---

## 7. Gotchas to avoid when rebuilding

1. **`install_instance` returns before doing anything** — the UI must listen for `install-progress`, not await a result (1366).
2. **Version JSON is per-instance, libraries/assets are global** — you can delete an instance without losing shared downloads, but two instances of the same MC version each hold their own copy of the version JSON and client jar.
3. **Client jar and every library download is skipped when the file exists and is non-empty** — there is *no* checksum verification (unlike Mojang's SHA1). A truncated-but-non-empty file will never be re-downloaded.
4. **Asset downloads are sequential, one at a time** — no parallelism; progress ticks every 50 files (1063).
5. **Forge/NeoForge install only unpacks `META-INF/libraries/`** from the installer jar — it does not run the installer's actual install task, so `version.json` handling differs from Fabric's clean profile flow (1302-1340).
6. **`set_java_for_instance` writes the global setting**, not `instance_settings.json` (6134-6136).
7. **Managed-Java paths assume `bin/java.exe` on every OS** (5840/5987/6045) — broken on Linux/macOS.
8. **Native classifier selection is Windows-only** (628-683).
9. **No checksum + sequential downloads = slow first install**; the mitigation is only "skip if exists".
10. **Offline sessions still pass `--accessToken 0` and `--userType legacy`** and rely on `CustomSkinLoader` for skins (2209-2212, 2291-2296).

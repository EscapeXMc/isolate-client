# 13 — Peripheral Subprojects & Session Logs (XrylothTauri)

Repo: `/mnt/Data Drive/EscapeX Products/Xryloth/XrylothTauri/`
All citations are `file:line` against that tree. Companion document: `11-build-release.md`.

Every subproject in §A–§D is **git-ignored** (`.gitignore:40-47`) and untracked —
`git status` lists `DriveVault/`, `indic-tiny-tts-hindi/`, `insight-wrap-android/`,
`xryloth-website-source/`, `cache/`, `minecraft-*` as `??`. They are local-only
working material, never part of a release.

---

## A. `DriveVault/` — a second, independent Tauri app

**Purpose:** a dark-themed *Linux* drive manager: lists internal disks, partitions,
removable media, phones (ADB/MTP) and cloud mounts, and can rename / format / open /
eject them behind a "master password" prompt.

**Stack:** Tauri **v1.6** + React 18 + Vite, no other runtime deps
(`DriveVault/package.json:19-29`). It is a *separate npm project* — own
`package.json` (`name: "drivevault"`, `version: "1.0.0"`), own `vite.config.ts`
(dev server pinned to port **1477** with `--strictPort`, `DriveVault/package.json:8`),
own `src-tauri/`.

### Structure

```
DriveVault/
├── package.json            dev on :1477, scripts dev/build/preview/tauri
├── index.html, vite.config.ts, tsconfig.json
├── src/
│   ├── main.tsx, App.tsx            the whole app (298-line single component)
│   ├── styles.css, types.ts (30)
│   ├── lib/api.ts (89), lib/fmt.ts (36)
│   └── components/DriveCard.tsx (101), Icons.tsx (166), Modals.tsx (411)
└── src-tauri/
    ├── Cargo.toml, build.rs, tauri.conf.json, Cargo.lock, icons/
    └── src/ main.rs (115) · scan.rs (335) · ops.rs (409) · phone.rs (293) · cfg.rs (67)
```
Rust total = **1,219 lines**.

### Backend modules

| File | Role |
|---|---|
| `main.rs` | 8 Tauri commands: `list_drives`, `rename_drive`, `format_drive`, `open_drive`, `eject_drive`, `enable_phone_mtp`, `get_settings`, `set_master_password` (`DriveVault/src-tauri/src/main.rs:29-84`). `list_drives` = `scan::scan_local` → `phone::scan_phones` → `phone::scan_cloud` → `phone::merge_phones` (`:29-37`). Helpers shell out to `findmnt -n -o SOURCE /` for the root device and `lsblk -no PKNAME` for the parent (`:11-26`). |
| `scan.rs` | `Drive` struct (23 fields, camelCase serde: `id, name, kind, icon, format, size, used, mountpoint, device, model, removable, system, risky, mounted, can_format, status, uri`) (`scan.rs:5-25`) + `ScanResult`; `run()`/`which()` process helpers. |
| `ops.rs` | privileged operations: `privileged()` runs `sudo -S`, writing the app password to stdin, and falls back to `pkexec` (`ops.rs:6-7`); used for rename/format/eject/open. |
| `phone.rs` | scans `/run/user/<uid>/gvfs` for MTP phones, percent-decodes gvfs paths (`phone.rs:10-25`), plus cloud entries and de-duplication. |
| `cfg.rs` | JSON config (`master_password`, `aliases`), stored under `$XDG_CONFIG_HOME` else `$HOME` (`cfg.rs:19-26`). **Default master password is the literal `"raghuwinder"`** (`cfg.rs:15-17`). |

### Frontend (`src/App.tsx`)

- Modal union type `detail | rename | format | settings | about` (`App.tsx:9-15`),
  toasts with 5.2 s auto-dismiss (`App.tsx:50-54`).
- Reads Tauri through the **global** `window.__TAURI__` API with a v1/v2-compatible
  shim (`getCurrentWindow() ?? appWindow`) (`App.tsx:24-37`).
- Sort order for results: `internal → partition → removable → phone_adb → phone_mtp → cloud`
  then by name (`App.tsx:59`).
- Default settings state ships `{ masterPassword: "raghuwinder" }` (`App.tsx:47`).

### Config differences vs the launcher

| | Xryloth launcher | DriveVault |
|---|---|---|
| `beforeBuildCommand` | `""` (empty, `src-tauri/tauri.conf.json:5`) | `"npm run build"` (`DriveVault/src-tauri/tauri.conf.json:6`) |
| identifier | `com.xryloth.app` | `com.raghuwinder.drivevault` |
| bundle targets | nsis, dmg, appimage, deb | `["deb", "appimage"]` |
| window | 1020×640 frameless | 1120×720 frameless transparent, min 860×560 (`DriveVault/src-tauri/tauri.conf.json:43-55`) |
| deps | winapi, reqwest, tokio, … | only `tauri`, `serde`, `serde_json` (`DriveVault/src-tauri/Cargo.toml:11-14`) |

---

## B. `xryloth-website-source/` — the marketing site (Vercel)

**Purpose:** the *deployed artefact* of a Next.js app whose source lives elsewhere,
plus the Vercel serverless functions. Live at **https://xrylothclient.vercel.app**
(`xryloth-website-source/README.md:1-6`). There is **no `package.json`** — it is a
static export repaired by Python scripts.

### Top level

```
xryloth-website-source/
├── README.md            the full "what was wrong / what was fixed" record
├── patch_site.py        idempotent fixer for the export (entry point)
├── build_assets.py      builds favicons/logo/splash/screenshot from source art
├── build_faq.py         generates public/faq-data.json (~470 → 3000+ answers)
├── serve.py             local server: /api/download, /api/analytics/*, else public/
├── test_downloads.mjs   node end-to-end check of both resolver halves
├── xryloth-downloads.js source of the client resolver (copied into public/)
├── tebex-checkout-stub.js  the Tebex chunk missing from the export
├── vercel.json          rewrites + outputDirectory "public"
├── database.rules.json, database.rules.partnerServers.{json,md}
├── api/                 serverless functions
├── assets/              input art for build_assets.py
├── legacy/              11 superseded one-off fix_hero/fix_logo/fix_download scripts
├── public/              the static export (what actually ships)
└── __pycache__/
```

### Page structure (`public/`)

| Page | `<title>` |
|---|---|
| `index.html` | Xryloth - Free Minecraft Launcher \| Boost FPS & Performance |
| `index-{es,de,fr,nl}.html` | localised variants (meta/title only, body stays English) |
| `download.html` | Download Xryloth — Minecraft Launcher · Releases & Changelog |
| `faq.html` | Xryloth FAQ — 3500+ answers about the launcher, mods and performance |
| `store.html` | Xryloth Store - Minecraft Cosmetics & Items |
| `track.html` | Xryloth Management Portal - Account Dashboard (`noindex`) |
| `contact.html` | Contact Xryloth - Support & Help |
| `privacy.html` / `terms.html` | Privacy Policy / Terms of Service |
| `uninstall.html` | Uninstall Xryloth — Remove the Launcher Safely |
| `preview.html` | Xryloth Preview — See the Launcher in Action |
| `toast-test.html` | tiny test harness |

Supporting: `faq-data.json` (1.2 MB, from `build_faq.py`), `xryloth-downloads.js`,
`theme-boot.js` (single pre-paint theme script), `page-fx.js`, `browserconfig.xml`,
full favicon/PWA icon set, `og-image.{png,svg}`, `banner.jpg`, `bg-wave.jpg`,
`icons.svg`, `fonts/`, `fontawesome/`, `magmad/` (logo/splash/banners),
`store/`, `store-background.webp`, `cursors/`, `_next/static/` (the export's chunks).

`vercel.json:7-24` rewrites `/robots.txt`, `/sitemap.xml`, `/uninstall`, `/preview`,
`/privacy`, `/terms` to their `.html`, and everything else (except `/api/`) to
`index.html`.

**Leftover still visible:** `public/index.html` header links contain
`contact.tmp.html`, `privacy.tmp.html`, `terms.tmp.html` — export-artefact URLs of the
kind `README.md` says were supposed to be fixed (README table row *"hreflang
alternates pointed at export artefacts"*).

### Serverless `api/`

| Function | Role |
|---|---|
| `download.js` (2,986 B) | 302-redirects a `platform` query to the newest matching GitHub asset (the **no-JS** half of the resolver) |
| `create-order.js` / `verify-payment.js` | Razorpay order creation + signature verification (needs `RAZORPAY_KEY_ID`/`RAZORPAY_KEY_SECRET`, README "Deploying") |
| `mc-status.js` (12,384 B) | Minecraft Server List Ping over raw TCP |
| `geo.js` (755 B) | visitor country from the Vercel edge — **nothing calls it** (README "Notes") |

### The download resolver contract

`README.md` download table — platform key → repo → asset pattern:

| key | repo | asset |
|---|---|---|
| `windows` | `EscapeXMc/Xryloth` | `*Windows64.exe` |
| `windows-x86` | `EscapeXMc/Xryloth` | `*windows32.exe` |
| `linux` | `EscapeXMc/Xryloth` | `*.appImage` |
| `windows-installer` | `EscapeXMc/xrylothInstaller` | `*windows_universal.exe` |
| `linux-deb` | `EscapeXMc/xrylothInstaller` | `*.deb` |

Client half caches 15 min in `localStorage` and serves a stale copy if GitHub is
down; server half is the fallback for JS-off. `node test_downloads.mjs` verifies both
against the live API. **There is no macOS entry** — README Notes: "No release has ever
shipped one."

### Notable history from its README (why this folder exists)

The export originally shipped the **FastClient** template: download buttons pointed at
`files.fastclient.net/…/FastClient-Setup.exe`, analytics posted to
`api.fastclient.net`, the theme key was `fastclient-appearance`, the hero was
FastClient's "FASTEST MINECRAFT LAUNCHER" art, and icons were a generic "X". The
README table lists ~20 such fixes, all applied through `patch_site.py`, which is
idempotent and **fails if a patched file stops being valid JS**.

### Data layer

The site contains **no Firebase code**. The Realtime Database (`xryloth-mc`) is
written and read by the *launcher*. Two hard dependencies documented in
`database.rules.partnerServers.md`:
- `/partnerServers` must stay **publicly readable** — `src-tauri/src/lib.rs` fetches it
  with an unauthenticated GET (WebView CORS sandbox blocks the browser).
- `/messages` needs `.indexOn: ["timestamp"]` — chat subscribes with
  `orderByChild("timestamp").limitToLast(100)` and the query is rejected outright
  without the index.

---

## C. `insight-wrap-android/` — Instagram WebView wrapper (Kotlin)

**Purpose:** a standalone Android app that wraps Instagram's mobile web client in a
full-screen WebView with a native draggable floating panel (`insight-wrap-android/README.md:1-4`).

### Files

```
insight-wrap-android/
├── README.md                 feature table + deliberate-scope statement + build steps
├── build.gradle.kts          AGP 8.7.0, Kotlin 2.0.21 (plugins, apply false)
├── settings.gradle.kts       rootProject "InsightWrap", include(":app")
├── gradle.properties, gradle/wrapper/, local.properties
└── app/
    ├── build.gradle.kts
    └── src/main/
        ├── AndroidManifest.xml
        ├── java/com/xryloth/insightwrap/MainActivity.kt   (670 lines)
        ├── assets/injector.js                             (387 lines)
        └── res/ (layout/activity_main.xml, layout/panel_settings.xml,
                  drawable/*, mipmap-anydpi-v26, values/*)
```

### Gradle (`app/build.gradle.kts`)

- `namespace`/`applicationId` `com.xryloth.insightwrap`, `compileSdk = targetSdk = 34`,
  `minSdk = 26`, `versionCode 1`, `versionName "1.0.0"` (`:7-16`).
- Release build: `isMinifyEnabled` + `isShrinkResources`, signed with the **debug**
  keystore ("Sign with your own keystore before distributing") (`:18-28`).
- Java/Kotlin target 17 (`:31-38`); deps are only `core-ktx 1.13.0`,
  `appcompat 1.7.0`, `activity 1.7.0` (`:41-45`).
- ⚠ **README disagrees with the build files**: README says "AGP 8.5.2, Kotlin 1.9.24"
  (`README.md`, Build step 2) while `build.gradle.kts:2-3` declares AGP **8.7.0** /
  Kotlin **2.0.21**.

### `MainActivity.kt` (670 lines)

| Area | Lines | What |
|---|---|---|
| WebView config | `:151-244` | `javaScriptEnabled`/`domStorage`/`database`, Chrome Mobile UA, cookie persistence, `onPageStarted/Finished` (re-injects on every navigation), `shouldOverrideUrlLoading`, progress bar, `onShowFileChooser` for profile uploads |
| Floating FAB | `:246-289` | draggable overlay (touch-slop aware), tap opens the drawer |
| Panel | `:291-372` | bottom drawer; `SeekBar` → WebView `textZoom` 80–200 % (`:320`); `Spinner` for chart palette (`:335`); open/close |
| Back navigation | `:374-390` | `OnBackPressedCallback` — back goes to `webView.history` first |
| Prefs | `:392-422` | `loadPrefs`/`savePrefs`, `buildConfigJson()` → `window.__xrlConfig` |
| `inject()` | `:424-440` | injects `injector.js` + config JSON |
| Downloads | `:442-554` | `withWritePermission`, `fetchAndSave` via `HttpURLConnection` (session cookies), `publishStream` → MediaStore (Android 10+) or public Downloads + scan (8.0–9.0), filename sanitising / extension derivation / MIME mapping |
| `JsBridge` | `:556-644` | `@JavascriptInterface`: `log`, `downloadUrl`, `beginBlobDownload`, `appendBlobChunk`, `commitBlobDownload` (base64 chunking for `blob:` sources) |

`AndroidManifest.xml`: `INTERNET` (`:4`), `WRITE_EXTERNAL_STORAGE` with
`maxSdkVersion="28"` (`:6-8`), one exported launcher activity (`:18-27`),
`usesCleartextTraffic="false"` (`:16`).

### `assets/injector.js` (387 lines)

Header declares the scope (`injector.js:1-16`): 1) content filter
(`markIfAd`/`scanTree`/`unhideAll`, `:80-102`), 2) media download buttons
(`addButtons`, `:189-227`), 3) compact-density class driven by native `textZoom`,
4) SVG chart gradient theming (`restyleSvg`, `:229-297`). Re-injection guard
`window.__xrlInjector` (`:20-24`), SPA navigation hooked through
`history.pushState`/`replaceState` (`:322-332`).

**Deliberate scope:** both README and the injector state that *no* displayed
analytics/insight value is ever rewritten or fabricated — only hiding, buttons,
spacing and chart colours (`README.md` "Deliberate scope"; `injector.js:11-12`).
README also warns that logging into your own account in a third-party WebView is
against Instagram ToS and suggests a throwaway account.

---

## D. Voice / AI stack: `backend/engine.py`, the TTS scripts, `indic-tiny-tts-hindi/`

### D1. `backend/engine.py` (1,567 lines) — legacy, **not wired up**

- JSON-RPC over stdin/stdout, "spawned as Tauri sidecar" (header `engine.py:1-5`),
  `APP_VERSION = "5.0"` (`:29`), `GITHUB_API_URL` for `EscapeXMc/Xryloth` (`:30-32`).
- Mirrors the original Python app's paths exactly: everything under `~/.xryloth/`
  (`:34-42`); Mojang `piston-meta`, Fabric meta, Modrinth API (`:43-45`).
- **Hard-coded Ely.by OAuth secret at `engine.py:48`** (see `11-build-release.md` §8.4).
- 61 top-level `def`s covering: HTTP with a 5-minute TTL cache (`:54-55`, `:95-158`),
  settings/instances/playtimes I/O, version manifests & loader filtering, Microsoft
  device-code auth (`:336-460`), Ely.by auth with a local `127.0.0.1:8080` callback
  server (`:462-567`, uses `http.server` imported at `:27`), offline accounts,
  authlib-injector download (`:579`), instance create/delete/install (`:646-780`),
  game launch + `start_game_process` with a 500-line output buffer (`:781-1106`),
  Java resolution, natives, CustomSkinLoader (`:1107-1202`), Modrinth search/install
  for mods, resource packs and `.mrpack` modpacks (`:1203-1365`), news, skins,
  optimizer, update check, system info (`:1366-1520`).
- Dispatch: a `COMMANDS` dict of ~35 lambdas, then a blocking `main()` read loop
  answering `{id, result}` / `{id, error}`, plus `ping`/`quit`
  (`engine.py:1522-1566`).

**Status: orphaned.** No file in `src/` or `src-tauri/src/` mentions `engine.py`, and
`tauri.conf.json` resources contain only `resources/mods/*` (`tauri.conf.json:84-86`).
It is the architecture described in `DEV_LOG.txt:61-104` (React ⇄ Rust ⇄ Python
JSON-RPC) that was later replaced by a pure-Rust `lib.rs` (11,944 lines). Treat it as
a reference implementation, not a build input.

### D2. TTS scripts at the repo root

| Script | What it does | Output |
|---|---|---|
| `kokoro_tts_ananya.py` (≈110 lines) | Builds a **custom voice "Ananya"** as `0.6·af_bella + 0.4·af_sarah`, renormalised to max-abs 1, saved to `kokoro_voices/Ananya.pt`; then runs `hexgrad/Kokoro-82M` (`kokoro-v0_19.pth`, `config.json`) through `kokoro.KModel.generate(speed=1.05, lang="a")`. Auto-`pip install`s any missing dep from `REQUIRED = [kokoro, onnxruntime, soundfile, torch, huggingface_hub]`. | `ananya_hello.wav` @ 24 kHz |
| `kokoro_ananya.py` | Same voice file, phrase `"Hello, Mera naam AI hain"` | `ananya_hello.wav` |
| `kokoro_ananya_hi.py` | Phrase `"नमस्ते, मेरा नाम AI है"` (pure Hindi) | `ananya_hello_hi.wav` |
| `kokoro_ananya_paragraph.py` | A Hindi paragraph, then auto-plays the result | `ananya_hi_paragraph.wav` |
| `indic_tts_liana.py` (≈115 lines) | Loads `indic-tiny-tts-hindi/` with `TinyTTSOnnx.from_pretrained(MODEL_DIR, threads=2)` (needs only numpy + onnxruntime + huggingface_hub). CLI: `--text/--voice female|male/--both/--speed 0.25-4.0/--seed/--out/--threads/--play`. Default text `"मेरा नाम लियाना है।"`. Prints duration, wall-time and a `sha256:` prefix per file; refuses empty audio with a hint that the model only reads Devanagari. | `liana_*.wav` @ 24 kHz |
| `speak.py` (15 lines) | `kokoro_onnx.KokoroOnnx.from_hf()` (auto-downloads ~300 MB), Hindi sentence, voice profile `hf_alpha` | `hindi_girl_output.wav` — **file not present**, so it never completed here |
| `download_model.py` (10 lines) | `hf_hub_download` of `kokoro-v1.0.onnx` and `voices-v1.0.bin` from `hexgrad/Kokoro-82M` into the repo root | see D4 — both landed as **HTML**, not model data |

### D3. `indic-tiny-tts-hindi/` (local HuggingFace clone)

A complete checkout of `sulabhkatiyar/indic-tiny-tts-hindi`, 89 MB
(`README.md` front-matter: `license: cc-by-4.0`, `pipeline_tag: text-to-speech`,
`language: hi`):

- `model.safetensors` 35.7 MB, `vocoder.safetensors` 54.1 MB, `config.json`
  (`format: "indic-tiny-tts/1"`, `sample_rate: 24000`, voices `female: hi_F`,
  `male: hi_M`, `default_voice: female`), `onnx/`, `samples/`, `assets/`,
  `pyproject.toml`, `MANIFEST.json` (per-file SHA-256), `LICENSE.md`.
- `indic_tiny_tts/` — the Python package (`TinyTTSOnnx`, export/onnx_infer, frontend),
  used by `indic_tts_liana.py` via `sys.path.insert(0, MODEL_DIR)`.
- `README.md` (315 lines) is the upstream model card: **22.5 M parameters**, reads
  Devanagari, 1.64 % CER on FLORES-200 news sentences (vs IndicF5 1.02 %, Indic-TTS
  1.33 %, `hexgrad/Kokoro-82M` 1.56 %, IndicParlerTTS 2.23 %), 38× real-time on one M1
  thread / 7× on x86, no GPU.

### D4. Stray assets at the repo root — what produced them

| File | Size / type | Producer |
|---|---|---|
| `ananya_hello.wav` (101,420 B) | PCM 16-bit mono 24 kHz | `kokoro_ananya.py` (output name declared in the script) |
| `ananya_hello_hi.wav` (121,900 B) | same | `kokoro_ananya_hi.py` |
| `ananya_hi_paragraph.wav` (970,796 B) | same | `kokoro_ananya_paragraph.py` |
| `liana_hello.wav`, `liana_hello_male.wav`, `liana_intro.wav`, `liana_sample_female.wav`, `liana_sample_male.wav` (73–341 KB) | same | `indic_tts_liana.py` (`--out`, `--both`, `--voice male`, custom `--text`) |
| `base.mp3` (34,704 B) | MPEG layer III, 48 kbps, **24 kHz mono** — the same rate as the WAVs | **no producer in the repo.** Timestamp 2026-11-03 21:13 sits between the Ananya samples (21:05–21:07) and is consistent with a manual MP3 transcode of one of them, but nothing references `base.mp3` anywhere (`grep` over `*.py/*.ts/*.tsx/*.rs/*.html/*.sh` = no hits) |
| `kokoro-v1.0.int8.onnx` (117,490 B) | **HTML document**, CRLF | attempted `download_model.py` — the download saved an error/HTML page, not ONNX weights (and the requested name was `kokoro-v1.0.onnx`, not `…int8.onnx`) |
| `voices-v1.0.bin` (117,596 B) | **HTML document**, CRLF | same failed download; also not referenced by any script |
| `kokoro_voices/` | **empty directory** | `kokoro_tts_ananya.py` would write `Ananya.pt`/`af_bella.pt` here; nothing was ever saved, so `kokoro_ananya*.py` would currently fail with *"voice 'Ananya' not found"* |
| `hindi_girl_output.wav` | **absent** | expected output of `speak.py` |
| `r1.png` (1,001,592 B), `r1_boost.png` (862,701 B) | PNG 1280×1024, 2026-11-01 13:08 | **no producer in the repo**; the paired names and identical timestamps read as before/after screenshots (normal vs Game Boost). Unverified — no reference anywhere |
| `index.html.1`, `index.html.2` (530,303 B each) | 569-line saved HTML of **github.com** (`index.html.1:9`, title at `:151`), differing only in `request-id`/nonce | two browser saves of the GitHub homepage, 2026-11-05 14:51 — untracked and unused by the build |
| `discord_friends.html` (13,381 B) | self-contained page, `<title>My Discord Friends & Servers</title>`, Discord colour variables, 1 `<script>`, links to `discord.gg` | hand-written one-off page; no producer script, not referenced by the app |
| `myenv/` | Python **3.12.3** venv (`myenv/pyvenv.cfg`) created with `python3 -m venv` | the TTS sandbox — contains `kokoro_onnx 0.6.1`, `onnxruntime 1.31.1`, `soundfile 0.14.0`, `espeakng_loader 0.2.4`, plus the huggingface_hub/fsspec/filelock/hf_xet stack |
| `cache/mojang_1.21.11.jar` | Minecraft client jar | downloaded during launcher testing |

**No TTS/audio code is referenced by the shipped app** — `grep -i "kokoro\|tts\|speak\|voice"`
over `src-tauri/src/lib.rs` returns nothing. The whole voice stack is an offline
experiment (Nov 3–5, 2026) for an in-app Hindi voice, not a build input.

---

## E. `public/`, `index.html`, `installer.html` — the static shell

### `index.html` (14 lines)

The Vite entry: `<title>Xryloth</title>`, a `#root` div, hard-coded
`html, body { background: #050505 !important }` (`index.html:8`), and
`<script type="module" src="/src/main.tsx">` (`index.html:12`). Vite copies `public/`
verbatim into `dist/`, and Tauri embeds `dist/` at compile time.

### `public/`

```
public/
├── favicon.svg, logo.png (54,063 B)        brand assets, referenced by index.html:5
├── installer.html (14,395 B)               byte-identical copy of src-tauri/installer.html
├── assets/steve-render.png, steve-vector.svg
└── models/steve/
    ├── Minecraft_-_Steve.gltf, SteveEmbedded.gltf, Minecraft_-_Steve.bin
    └── images/steve.png                    three.js model used by the launcher UI
```
`logo.png` is also the icon source for `scripts/package-installer-deb.sh:25,38`.

### `installer.html` (389 lines, identical at `src-tauri/` and `public/`)

Vanilla single-page installer UI using the **global** Tauri API —
`const { invoke } = __TAURI_API__.tauri || __TAURI_API__.core` (`installer.html:195-197`).

DOM: `<h1>Xryloth Installer</h1>` (`:163`), one status line + spinner (`:165-168`),
progress bar (`:170`), three buttons `Install Xryloth` / `Try again` / `Launch Xryloth`
(all `display:none` initially, `:172-174`), an install-path row with a **Change** link
(`:176`), an **Uninstall** link (`:177`), and a custom confirm dialog
(`:183-190`).

Flow comment at `:250` — *"check → (install button | auto-update | straight launch) →
close"*:
- `installer_check_installed` (`:256`) → if installed, `installer_fetch_release`
  (`:260`) and `installer_get_config` (`:280`);
- update available → **automatically** `installer_update` with no button press
  (`:290`, `:305`) — the hands-free flow from commits `a4444d2` / `21c7968`;
- fresh install → `installer_install` with `{config: installConfig()}` (`:320`);
- then `installer_launch_launcher` (`:334`) and `installer_close_app` (`:338`);
- uninstall → `installer_uninstall` (`:374`) then optionally
  `installer_open_uninstall_page` (`:376`).

Backend for all of this: `src-tauri/src/installer/main.rs` (see
`11-build-release.md` §7).

---

## F. Session-log history highlights (`DEV_LOG.txt`, `SESSION_NOTES.md`, `INSTALL_OVERLAY_SESSION.txt`, `CHAT_LOG.txt`)

### F1. `DEV_LOG.txt` — "ECLAUNCHER TAURI MIGRATION" (2026-07-15 → 07-16, 7 sessions, 416 lines)

| Session | What was built / what broke |
|---|---|
| 1 — analysis | Port from a **7,315-line monolithic Python/CustomTkinter `main.py`** to Tauri. Decisions: Tauri over Electron, Python backend via JSON-RPC, React+TS+Framer Motion, glassmorphism theme, Zustand, Tailwind (`DEV_LOG.txt:8-39`). Source map of `main.py`: themes/HTTP `26-262`, animation engine `368-558`, main class `562-7315`. |
| 2 — scaffold | Created `package.json`, `vite.config.ts`, `tailwind.config.js`, tsconfigs, `index.html` (`:43-53`). |
| 3 — codegen (34+ files) | Architecture `[React] ⇄ [Rust/Tauri] ⇄ [Python Engine]` (`:60-61`). `backend/engine.py` ~937 lines with 35+ commands and exact `~/.xryloth/` path parity (`:65-89`); `lib.rs` ~203 lines with a `Mutex<Process>`, **6 candidate engine.py paths**, write+read under one lock, auto-restart on crash (`:91-104`); full React tree of 7 UI components + 10 pages (`:106-140`). |
| 4 — build system | Rust 1.97.1, MSYS2/MinGW-w64, icons, first `.exe`/`.msi` via `npx tauri build --target x86_64-pc-windows-gnu` (`:144-160`). |
| 5 — five critical bugs | **BUG1** `import psutil if False else None` is invalid Python — SyntaxError at module load killed *every* command ("Failed to flush: pipe being closed"); **BUG2** `install_modpack` passed a `project_type` kwarg `get_modrinth_versions()` doesn't accept; **BUG3** engine path resolution never reached the file → 6 candidates + `"resources": ["../backend/engine.py"]`; **BUG4** IPC race (separate lock acquisitions) → single lock; **BUG5** dead process gave cryptic errors → per-operation checks + auto-restart. Plus search-page overhaul (Enter, 350 ms debounce, spinner, error banner, empty states) and a stderr logger (`:164-229`). |
| 6 — backend rewrite | engine.py → ~1000 lines. Ely.by OAuth with a local `127.0.0.1:8080` server and a hard-coded client secret (`:181-189`); full launch path (classpath, G1GC args, Ely.by > Microsoft > Offline precedence, `CREATE_NO_WINDOW`, 500-line output buffer, playtime tracking, CustomSkinLoader for cracked users, pre/post hooks) (`:191-199`); install fixes incl. `install_resourcepack` and full `.mrpack` extraction (`:201-205`); `[Tag]`-prefixed logging (`:207-210`); new `api.ts` bridge methods (`:212-215`). |
| 7 — black screen | `AnimatePresence mode="wait"` in `src/App.tsx` faded the old page out first, producing a black gap → removed page transitions, added per-page `ErrorBoundary`, explicit background (`:218-230`). |
| End | "WHAT REMAINS" list: end-to-end `.exe` test, mod-install progress wiring, resource-pack auto-enable, skin manager UI, Discord RPC, auto-update UI, crash auto-repair (`DEV_LOG.txt:401-416`). |
| Also | `APP SPECIFICATIONS` block records version 5.0, window 1020×640 frameless, 10 pages, 5 loaders, 3 auth methods, and **a Groq API key in plaintext** (`DEV_LOG.txt:140-153`). Theme system: 7 themes × 26 colour fields (`:121-136`). |

### F2. `SESSION_NOTES.md` — version 5.9.0 (76 lines)

- **Previous sessions roll-up** (`:7-24`): Java auto-detection (8/16/17/21), XrylothAI
  4 tabs, offline-auth/CMD-window/"Not Responding" fixes, instance cloning,
  dark/light themes, keyboard shortcuts, multiplayer page, resource-pack preview,
  plugin/server support, grid/list toggle, sorting, pinning, drag & drop, recent games,
  screenshot gallery, batch install, export/import, live log stream, real auto-update,
  13 instance templates, mod config editor, Discord RPC
  (`client_id 1506599015826067476`), Firebase chat (project `xryloth-chatsystem`),
  8 built-in + custom themes with 22 colour fields, QoL set, and **Game Boost**
  (kills 30+ bloatware, clears temp/cache, DNS flush, auto FPS mods).
- **That session's 7 features** (`:26-33`): `AnimatedBackgrounds` (rain/snow/fire/
  galaxy/matrix), `LaunchAnimation` (End Portal), `MinecraftParticles` (canvas),
  `SoundEffects` (Web Audio, no files), `AnimatedLogo`, `MusicPlayer` (14 C418-style
  tracks synthesised with oscillators), `UserStatus` (Discord-style presence).
- **Custom background overhaul** (`:35-41`): new Rust commands
  `save_background_video` / `get_background_video_path` / `delete_background_video`
  writing `~/.xryloth/backgrounds/custom_video.{mp4|webm|mov}`; brightness/dim/fit
  settings; default Pixabay video.
- **HomePage cleanup** (`:43-47`) and **Tech stack / build facts** (`:49-60`): Tauri 1.6
  + React 18, Rust dep list, Java-per-MC-version rule, `GITHUB_API_URL`, build command
  `npx tauri build`, output `…/nsis/Xryloth_5.9.0_x64-setup.exe`, data dir `~/.xryloth/`,
  **Firebase API key in plaintext** (`:55`).
- Key-files list for the session (`:62-76`).

### F3. `INSTALL_OVERLAY_SESSION.txt` (2026-07-13, 59 lines)

Objective: replace the generic spinner with a live terminal-style install log.
Completed: `install_instance` gained `app_handle` + `emit_all("install-progress",
{step, message, progress, done, error})` with real asset-object downloading from
`resources.download.minecraft.net` and a 2 %→100 % phase curve (`:15-20`);
new `src/components/ui/InstallOverlay.tsx` listening via `@tauri-apps/api/event`
(`:22-30`); `InstancesPage.tsx` wired to it with Close/Play-Now actions and
performance-mod auto-install after completion (`:32-38`); `npx tsc --noEmit` clean
(`:40`). Notes the original Windows path `C:\Users\power\Desktop\Xryloth\XrylothTauri`
(`:56`).

### F4. `CHAT_LOG.txt` (2026-07-20, 50 lines, **tracked in git**)

Two problems:
1. **Forge version list 404** — `files.minecraftforge.net/maven/…/json` died when Forge
   migrated infrastructure; fixed by parsing `maven-metadata.xml` from
   `maven.minecraftforge.net` in three places (install auto-detection ~line 749,
   version picker ~line 1087; installer URL needed no change)
   (`CHAT_LOG.txt:4-29`). Key finding: Forge Maven versions look like
   `1.8.9-11.15.1.2318-1.8.9`, so the NeoForge digit-only validator would reject them —
   the Forge path uses a first-match approach because Maven lists newest-first
   (`:31-37`).
2. **Game exit code 1** — MC 1.8.9 needs Java 8 but the launcher used JDK 21
   (`:44-50`).

### F5. Cross-cutting observations for a rebuild

- The logs are **chronologically older than the code**: `DEV_LOG` (July 2026) describes a
  Python-engine architecture that no longer exists; `SESSION_NOTES`/`CHAT_LOG` describe
  5.9.0; git history runs 5.9.1 → 7.0.0.1 (Oct–Nov 2026). Use the logs for *why* a
  decision was made, not for *what the code currently is*.
- `DEV_LOG.txt` is git-ignored (`.gitignore:32`); `CHAT_LOG.txt` is **not** — it is one
  of only 156 tracked files.
- Four separate plaintext credentials live in these logs/code
  (Groq key, Ely.by secret, Firebase key, DriveVault default password) — see
  `11-build-release.md` §8.4. Rotate before any redistribution.

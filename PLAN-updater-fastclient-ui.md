# MASTER PLAN — Auto-Updater + Live Announcements + FastClient-style UI

> Ye file AGLE AI AGENT ke liye single source of truth hai. Token bachane ke liye yahan sab hai:
> codebase structure, architecture plan, file-by-file implementation, Firebase schema,
> GitHub workflow, setup steps, aur FastClient UI replication. **Pehle ye padho, phir code karo.**

---

## 0. PROJECT SNAPSHOT (repo: `/mnt/Data Drive/IsolateClient`)

- **Stack:** Tauri 1.x (Rust backend `src-tauri/src/lib.rs` ~13k LOC) + React 18 + TypeScript + Vite
  (`src/`), Tailwind-ish custom CSS (`src/*.css`). Binary name: `isolate-client`.
- **Build:** `npm run build` (tsc + vite → `dist/`, Tauri `distDir:../dist`, `beforeBuildCommand:npm run build`).
  CI = `.github/workflows/build.yml` (workflow_dispatch; windows job → `windows-bundles` artifact;
  release job **clobbers** assets on tag/release `v0.1.0`).
  Installer: `IsolateClient_0.1.0_windows64-setup.exe` (+ `_x64.msi`, `_windows-x64.zip`).
- **Version constant:** app shows `V0.1.0` (sidebar). Version kahan se aata hai — `package.json` version
  + Tauri config; updater ke liye ek single source chahiye (§3.1).
- **State:** Zustand store `src/stores/app.ts` (`useApp` — page, installing map, notify/toasts, refreshInstances).
- **Events:** Tauri events via `src/lib/tauri.ts` wrapper; `onEvent<T>("install-progress", ...)` in `src/App.tsx:59`.
- **HTTP in Rust:** `http_get`, `download_file`, `download_many(url_list, concurrency, progress_cb)` (lib.rs ~L200-330).
- **Firebase:** already integrated for telemetry (`src/lib/telemetry.ts` — `sendTelemetry({kind:"download"|"launch"})`
  → Firebase). **Isi Firebase project ka re-use karna hai** updater listener ke liye. Config kahan hai:
  grep `firebase` in `src/lib/` (apiKey etc.). RTDB vs Firestore — jo telemetry use kar rahi hai wahi.
- **App data dir:** `~/.isolate/` (Windows: `%USERPROFILE%\.isolate\`) — `instances.json`, `instances/`,
  `libraries/`, `assets/`, `instance_settings.json`, `servers.json`, `playtimes.json`.
  **Updater ko ye sab preserve karne hain.**
- **Current install/launch flow (lib.rs):** `install_instance` (L1990, fire-and-forget thread) →
  `install_inner` → progress events; `launch_game` (checks `installed` = `client.jar` exists, L591).
- **Known bug (pehle se documented):** 1.8.9 install reset/fail — `TODO-KAL.md` dekho. Install logging
  file me add karna pending hai.

### Repo map (kahaan kya hai)

```
/mnt/Data Drive/IsolateClient/
├── package.json, vite.config, tsconfig
├── .github/workflows/build.yml        # CI (workflow_dispatch, windows+linux, release asset clobber)
├── CONTEXT-handoff.md                 # all-context handoff doc
├── TODO-KAL.md                        # kal ka plan (1.8.9 bug + GSC steps)
├── TODO-session-compact.md            # session todos
├── PLAN-updater-fastclient-ui.md      # ← YE FILE
├── site/                              # static marketing site (deployed on Vercel, 184/184 suite)
│   ├── vercel.json (cleanUrls), public/ (index, plans, faq, team, subdomains, account, 404…)
├── src/                               # React frontend
│   ├── App.tsx                        # event listeners, page router, boot()
│   ├── stores/app.ts                  # zustand: page/installing/notify/refreshInstances
│   ├── lib/
│   │   ├── tauri.ts                   # api.* wrappers (invoke) — updater yahan add hoga
│   │   ├── actions.ts                 # installInstance/playInstance/createInstance…
│   │   ├── telemetry.ts               # Firebase sendTelemetry  ← Firebase creds yahan
│   │   └── types.ts                   # Instance, Loader types
│   └── components/
│       ├── Instances.tsx              # PROFILES page (install UI, badges, progress)
│       ├── DiscordChat.tsx (L216→plans), Admin.tsx (L746), Settings.tsx (About→support mail)
│       └── … (sidebar/HOME/MODS/HOST/PARTNERS/CHAT/ACCOUNTS/AI pages)
└── src-tauri/
    ├── tauri.conf.json                # bundle/identifier/window
    └── src/lib.rs                     # SAB backend yahan
        ├── L59 app_dir=~/.isolate, L88 instance_dir, L93 sanitize_name
        ├── L124 log_msg (in-memory only — file logging pending)
        ├── L1966 progress() → emit "install-progress"
        ├── L1990 install_instance / install_blocking / install_inner (manifest→jar→assets→libs)
        ├── L2410 launch_game (not_installed check)
        ├── http_get / download_file / download_many (~L200-330)
        └── commands registry ~L12107 (tauri::generate_handler list — naya command yahan register)
```

**FastClient screenshots (UI reference):** `/home/raghuwinder/Pictures/Screenshots/`
→ pehle `ls` karo, har screenshot dekho (read tool se image), phir match karo:
update modal, announcement banner, icons (SVG style), layout, colors.
**Goal:** same look/UX (SVG icons ka style, banner/modal ka design) but humare palette me:
black `#0a0a0a` + blue `#3b82f6`/`#60a5fa` + lime `#A3FF12`. Credit line
"An isolated client by Shayan & Raghuwinder, built for performance" survive rakhni hai.
Suite `verify_nav.mjs` se **184/184** maintain karna hai (site pages touch mat karo).

---

## 1. ARCHITECTURE PLAN (updater ka flow)

### 1.1 Single source of truth
- `currentVersion` = `package.json` version (build time me Tauri ko embed karo ya `tauri.conf.json`
  version se read). Ek constant `CURRENT_VERSION` (frontend) + `app_version()` (Rust).

### 1.2 Startup check (client start par)
1. Rust ya frontend ek **GET** kare Firebase RTDB endpoint (REST, no SDK needed):
   `https://<db>.firebaseio.com/releases/latest.json` (+ `releases/announcement.json`).
   REST se kaam chalega (RTDB rules public-read for these paths).
2. Semver compare: `latestVersion > currentVersion`? aur `currentVersion < minSupportedVersion` → **forceUpdate**.
3. Cross-verify GitHub: `https://api.github.com/repos/EscapeXMc/isolate-client/releases/latest`
   → asset URL + size + sha256 (release body ya `checksums.txt` asset se).
4. Agar update/announcement hai → UI me banner/modal (§2).

### 1.3 Real-time listener (client khula ho tab)
- Firebase Realtime DB **streaming REST** (`Accept: text/event-stream`) ya firebase SDK — RTDB me
  `releases/latest` pe `on('value')`. Simplest robust: Rust me long-poll/SSE thread → Tauri event
  `release-update` emit → frontend listener (`App.tsx` me `onEvent`).
- Event names: `"release-check"` (startup result), `"release-update"` (live change), `"update-progress"`.

### 1.4 Self-update flow (Update Now button)
```
Download (staging: ~/.isolate/staging/)  →  progress event (pct, speed, status)
  statuses: downloading → verifying → installing → restarting
Verify: SHA-256 (Rust: sha2 crate) vs Firebase sha256; size check; zip/exe sanity
Hand-off: Rust spawns DETACHED updater helper:
  - Option A (chosen): ek chhota Rust bin `isolate-updater` (same repo, second tauri bin ya
    plain std process) ya PowerShell/cmd script written to staging.
  - Flow: helper waits (PID of client) → client exits cleanly (file locks free)
    → helper replaces install dir files (config/profiles `~/.isolate` untouched — woh app dir me hai,
       install dir alag hai) → launches new exe → helper exits.
  - Windows: `CreateProcess` with detached flags / `cmd /c start`; delete-replace ke liye
    rename+retry loop (AV/file-lock → 3 retries × 500ms, phir clear error).
Edge cases:
  - network fail → retry button, download resume (Range header) ya fresh restart of download
  - interrupted → staging partial file delete on next start
  - permission (Program Files me ho to) → fallback install dir `%LOCALAPPDATA%\IsolateClient`
  - rollback: replacement se pehle `old/` me backup; naya launch fail → helper restores backup + relaunch old
  - forceUpdate → "Later" button hidden
```

### 1.5 Kahan kya implement hoga (modular files)

**Rust (`src-tauri/src/`):**
- `updater.rs` (naya module; lib.rs me `mod updater;`)
  - `check_for_update() -> Value` — Firebase REST + GitHub API, semver compare (`semver` crate ya
    hand-rolled parse), returns `{updateAvailable, forceUpdate, latestVersion, downloadUrl, sha256,
    size, releaseNotes, announcement:{id,title,message}, updatedAt}`
  - `start_update_listener(handle)` — background thread, RTDB streaming/SSE → emit `release-update`
  - `download_update(handle, url, sha256)` — staging me download, sha256 verify, emit `update-progress`
  - `apply_update()` — helper script/process spawn + clean exit (`std::process::exit(0)` after spawn)
  - `app_version()` command
- Commands register: `check_for_update`, `start_update_listener`, `download_update`, `apply_update`
  → `tauri::generate_handler![...]` list (lib.rs ~L12107) me add.
- Logging: `log_msg` ka extension — `updater.log` file in `~/.isolate/` (debug ke liye).

**Frontend (`src/`):**
- `src/lib/updater.ts` — API wrappers (`api.checkForUpdate()` etc. tauri.ts me add), semver helper,
  dismissed-announcement persistence (`localStorage.iso_announcement_dismissed = announcementId`),
  dismissed-update persistence (`localStorage.iso_update_later = version`).
- `src/components/UpdateModal.tsx` — modal: version, release notes, size, sha256 (truncated),
  progress bar (downloading/verifying/installing/restarting + MB/s), **Update Now** + **Later**
  (Later hidden if forceUpdate), error state (retry/rollback msg).
- `src/components/AnnouncementBanner.tsx` — top banner: title + message + dismiss (X) —
  announcementId se skip until id change.
- `src/App.tsx` — startup `check_for_update()` + `onEvent("release-update")` listeners;
  Zustand me `updateInfo`/`announcement` slice (`stores/app.ts`).
- **FastClient-style styling:** screenshots se exact layout copy (spacing, icon style, modal shape),
  colors humare palette me, icons = inline SVG (lucide jaisi ya screenshot wale custom SVG).
  CSS: `src/components/UpdateModal.css` ya existing global css pattern follow karo.

**No new runtime deps ideally:** sha256 ke liye Rust crate `sha2`; frontend semver chhota helper
(hand-rolled split on `.`). Firebase REST se SDK ki zaroorat nahi.

---

## 2. FIREBASE SCHEMA (RTDB) + RULES

```json
{
  "releases": {
    "latest": {
      "latestVersion": "0.2.0",
      "minSupportedVersion": "0.1.0",
      "forceUpdate": false,
      "downloadUrl": "https://github.com/EscapeXMc/isolate-client/releases/download/v0.2.0/IsolateClient_0.2.0_windows64-setup.exe",
      "sha256": "61a789c71bdf71664e7f...",
      "size": 2939554,
      "releaseNotes": "## What's fixed\n- Install reliability\n- New update modal",
      "updatedAt": 1763800000000
    },
    "announcement": {
      "announcementId": "2026-11-21-release-0.2",
      "announcementTitle": "0.2.0 is live!",
      "announcementMessage": "Auto-updates now work. Update from the banner.",
      "updatedAt": 1763800000000
    }
  }
}
```

**Rules (public read, admin-only write — service account ya Firebase console se push):**
```json
{
  "rules": {
    "releases": {
      ".read": true,
      ".write": "auth != null && auth.token.admin === true"
    }
  }
}
```
(Write sirf tumhare SA/admin se — client me **sirf read**. Secret key client bundle me mat daalo.)

**Push helpers (repo me `scripts/push-release.mjs` banao):**
`node scripts/push-release.mjs --version 0.2.0 --url <asset-url> --sha256 <hash> --notes-file notes.md`
→ Firebase REST `PUT /releases/latest.json` (service account key env `FIREBASE_KEY` me, repo me mat).

---

## 3. GITHUB RELEASE WORKFLOW

`.github/workflows/release-update.yml` (ya `build.yml` ke release job me step add):
1. `on: push: tags: ['v*']` ya manual dispatch with `version` input.
2. Build (existing windows+linux jobs) → upload assets to release.
3. Compute `sha256sum` of installer → `checksums.txt` asset ke roop me attach.
4. Step: Firebase push (curl REST with secret from GitHub Secrets `FIREBASE_DB_URL` + `FIREBASE_AUTH`
   ya service account) → `releases/latest` update karo.
5. Optional domain endpoint: `https://isolateclient.in/api/latest.json` (Vercel redirect/rewrites se
   GitHub raw/release JSON serve — cross-check ke liye).

**Manual release checklist (docs/RELEASE.md me likhna):**
- [ ] bump version (package.json + tauri.conf.json)
- [ ] `npm run build` + suite 184/184
- [ ] tag `vX.Y.Z` → CI build + release assets
- [ ] sha256 nikalo → `push-release.mjs` se Firebase push (version, url, sha256, notes, forceUpdate flag)
- [ ] GSC/vercel deploy nahi chahiye (site alag hai) — sirf verify live `latest.json`

---

## 4. SETUP STEPS (exact)

1. **Firebase:** RTDB create (jo telemetry wala project hai) → `releases/latest` + `releases/announcement`
   seed karo (upar wala JSON) → rules set karo → DB URL note karo
   (`https://<proj>.firebaseio.com`).
2. **Secrets:** GitHub repo secrets me `FIREBASE_DB_URL`, `FIREBASE_AUTH` (DB secret) — CI push ke liye.
   Local `scripts/push-release.mjs` ke liye env.
3. **Domain (optional cross-check):** `isolateclient.in/api/latest.json` → Vercel rewrite to
   GitHub release JSON ya same Firebase REST (proxy, no secrets).
4. **Client code:** §1.5 modules → `npm run build` → CI dispatch → installer VM me re-test.
5. **Test matrix:** normal update, forceUpdate (Later hidden), announcement dismiss + id change,
   network off (retry), corrupted sha (verify fail + clear error), kill-mid-download (staging cleanup),
   rollback (bad exe case), **fresh install bhi same path se**.

---

## 5. EXECUTION ORDER (agle agent ke liye)

1. `ls /home/raghuwinder/Pictures/Screenshots/` + har image read → FastClient UI notes (layout/icons).
2. Firebase config dhoondho (`src/lib/telemetry.ts` + grep firebase) — RTDB hai ya Firestore.
3. `src-tauri/src/updater.rs` likho (check/listener/download/verify/apply) + commands register.
4. Frontend: `updater.ts`, `stores/app.ts` slice, `UpdateModal.tsx`, `AnnouncementBanner.tsx`,
   `App.tsx` wiring; FastClient screenshots jaisa UI (humare colors).
5. `scripts/push-release.mjs` + `docs/RELEASE.md` + workflow step.
6. `npm run build` → suite 184/184 → CI dispatch → VM test (helpers: `/tmp/opencode/shot.sh`,
   `ktype.sh`, monitor socket; **3-arg `mouse_move dx dy z`** use karna).
7. Commits: `client: ...` prefix, author `EscapeXMc`, push.

## 6. CONSTRAINTS / DO-NOT-TOUCH
- Site (`site/`) frozen except requested changes; suite must stay **184/184**.
- Discord `https://discord.gg/TV2ypJF6TF` untouched. Support: `support@isolateclient.in`.
- Secrets sirf env/Vercel/GitHub Secrets me — bundle me kabhi nahi.
- IsolateClientMod auto-replace feature PAUSED hai (user ne) — start mat karna.
- 1.8.9 install bug (TODO-KAL.md) alag hai — updater usse block mat karna, par install logging
  ka fix dono kaam aayega.
- claude-mem observer auth down since 2026-11-13 (`/login` fix karega; restart se nahi).

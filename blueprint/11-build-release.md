# 11 — Build & Release System (XrylothTauri)

Repo: `/mnt/Data Drive/EscapeX Products/Xryloth/XrylothTauri/`
All citations are `file:line` against that tree. Nothing below is inferred — where the
repo does not say something, it is marked *unknown / not in repo*.

---

## 1. Repo layout map

| Path | What it is | Build-relevant? |
|---|---|---|
| `package.json` | npm project `xryloth-tauri`, version `7.0.0.1`, scripts for tauri targets (`package.json:4`, `package.json:6-18`) | yes |
| `vite.config.ts`, `tailwind.config.js`, `postcss.config.js`, `tsconfig*.json` | frontend build config | yes |
| `index.html` | Vite entry (14 lines: `#root` + `/src/main.tsx`, hard-coded `#050505` bg) (`index.html:1-14`) | yes |
| `src/` | React 18 + TS frontend (`src/App.tsx`, pages, `src/lib/api.ts`, Zustand store) | yes |
| `src-tauri/` | Rust workspace root — one crate, **two binaries** (`src-tauri/Cargo.toml:12-18`) | yes |
| `src-tauri/src/lib.rs` | launcher backend, 11,944 lines — contains the updater | yes |
| `src-tauri/src/main.rs` | 21-line launcher entry; pins `GDK_BACKEND=x11` on Linux (`main.rs:15-18`) | yes |
| `src-tauri/src/installer/main.rs` | second binary `xryloth_installer`, 959 lines | yes |
| `src-tauri/installer.html`, `public/installer.html` | installer UI, byte-identical 14,395 B copies (`diff` clean) | yes |
| `src-tauri/build.rs` | 229 lines: Windows manifest, secret embedding, `tauri_build::try_build` | yes |
| `src-tauri/tauri.conf.json` | bundle targets `nsis, dmg, appimage, deb` (`tauri.conf.json:71-76`), `beforeBuildCommand: ""` (`tauri.conf.json:5`) | yes |
| `src-tauri/.cargo/config.toml` | `lld-link` / `llvm-lib` for both MSVC targets (7 lines) | yes |
| `src-tauri/secrets.json` | git-ignored key store (2 keys, see §8) | yes |
| `build-all.sh`, `build.bat`, `build.ps1`, `build-macos.sh` | platform build drivers (§2) | yes |
| `scripts/*.sh`, `scripts/tools/*` | packaging + regression checks (§2, §5) | yes |
| `README.md`, `RELEASING.md`, `SESSION_NOTES.md`, `INSTALL_OVERLAY_SESSION.txt`, `DEV_LOG.txt`, `CHAT_LOG.txt`, `LICENSE` | docs & session logs (§9, doc 13) | docs |
| `public/` | static assets copied to `dist/`: `logo.png`, `favicon.svg`, `installer.html`, `assets/steve-*.{png,svg}`, `models/steve/*.gltf`+`.bin` | yes |
| `dist/` | Vite output, embedded at **compile time** by Tauri | yes |
| `release/`, `release-v*/` | staged, upload-ready artifacts (12 dirs, `release-v6.0.0.2 … v7.0.0.1`) | output |
| `backend/engine.py` | 1,567-line Python JSON-RPC engine — **orphaned** (see doc 13 §E4) | legacy |
| `DriveVault/` | second Tauri app (doc 13) | peripheral |
| `xryloth-website-source/` | static Vercel site export + serverless `api/` (doc 13) | peripheral, git-ignored |
| `insight-wrap-android/` | Kotlin WebView app (doc 13) | peripheral, git-ignored |
| `indic-tiny-tts-hindi/`, `kokoro_*.py`, `indic_tts_liana.py`, `speak.py`, `download_model.py`, `myenv/` | voice/AI experiments (doc 13 §E4) | peripheral, git-ignored |
| `minecraft-mods/`, `minecraft-plugins/` | local Minecraft server/mod scratch space | peripheral, git-ignored |
| `cache/mojang_1.21.11.jar` | downloaded Minecraft jar | scratch, git-ignored |
| `webkit2gtk-4.0.pc -> webkit2gtk-4.1.pc` | root-owned symlink: makes Tauri v1's `4.0` pkg-config probe find 4.1 | workaround |
| `src-tauri/xryloth_installer_5.9.3_x{86,64}-setup.exe` | leftover NSIS installers sitting inside `src-tauri/` (matched by ignore rule `src-tauri/xryloth_installer_*.exe`, `.gitignore:38`) | stale |
| `index.html.1`, `index.html.2` | 530 KB saved HTML of github.com, two snapshots differing only in request-id/nonce (`index.html.1:9`, `index.html.1:151`) — stray scrape artifacts, untracked | junk |
| `discord_friends.html`, `r1.png`, `r1_boost.png`, `base.mp3`, `*.wav`, `kokoro-v1.0.int8.onnx`, `voices-v1.0.bin` | stray experiment outputs (doc 13 §E4) | junk |
| `.gitignore` | 47 lines — see §8 | yes |

---

## 2. Build scripts — line-by-line behaviour

### 2.1 `build-all.sh` (117 lines) — the Linux-host release driver

| Lines | Behaviour |
|---|---|
| `1-2` | `set -e`: any failing step aborts the whole release. |
| `11-15` | `PROJECT_ROOT` from `$0`; version read dynamically: `node -p "require('./package.json').version"`, fallback `0.0.0` (`build-all.sh:14`). **Only** `package.json` is read here. |
| `21-26` | Prerequisite gate: `cargo`, `npm`, `makensis` (the `nsis` distro package), and `cargo-xwin` (accepts either `command -v cargo-xwin` or `cargo xwin --version`). |
| `29` | `CARGO_BUILD_JOBS` defaults to **2** — comment says "4 cores / low RAM = avoid OOM". |
| `31` | `XWIN_ARCH=x86,x86_64,aarch64` so cargo-xwin downloads the 32-bit MSVC SDK libs too. |
| `37-42` | If `/tmp/opencode/pkgconfig` exists, prepend it to `PKG_CONFIG_PATH` and `/tmp/opencode/lib` to `LIBRARY_PATH` — the Ubuntu 24.04 "webkit2gtk 4.0 → 4.1" compat shim. |
| `47-58` | Writes a `cargo` **runner shim** into a temp dir: if any argument matches `x86_64-pc-windows-msvc` / `i686-pc-windows-msvc` / `aarch64-pc-windows-msvc`, exec `$HOME/.cargo/bin/cargo-xwin "$@"`; otherwise exec plain `cargo`. |
| `61-66` | `[1/4] npm install`, `[2/4] npm run build` (= `tsc && vite build`, `package.json:8`). |
| `69-75` | `[3/4]` `rustup target add` for both MSVC targets, then two `./node_modules/.bin/tauri build --runner "$RUNNER_DIR/cargo" --target <msvc> --bundles nsis`. |
| `78-80` | `[4/4]` `rustup target add x86_64-unknown-linux-gnu` then `npm run tauri:build:linux`. |
| `86-87` | **`rm -rf release`** — the staging folder is rebuilt from scratch every run. |
| `89-106` | Copy step with fallbacks: preferred name `Xryloth_${VERSION}_x64-setup.exe` etc., falling back to a glob (`bundle/nsis/*.exe`, `bundle/appimage/*.AppImage`, `bundle/deb/*.deb`). Also copies the raw binaries as `Xryloth_${VERSION}_x{64,86}-portable.exe` — it looks for `Xryloth.exe` **or** `xryloth.exe` (`build-all.sh:99-100`) because Tauri renames the cargo output to the product name. |
| `108-117` | Prints "Tag this release: v$VERSION", the `ls` of `release/`, the note that the updater downloads `*-setup.exe` / `*.AppImage`, and that macOS is **not** built here. |

Notes:
- It stages into flat `release/`, **not** the `release-v<ver>/launcher/` layout that
  `scripts/verify-launcher-assets.sh` expects (§5) — those two conventions coexist.
- No `cargo tauri build` step for the `xryloth_installer` binary; the installer is
  packaged separately (§2.4).

### 2.2 Windows scripts

`build.bat` (45 lines):
1. `rustup target add` for `i686` + `x86_64` MSVC (`build.bat:7-9`).
2. `cd src-tauri && cargo build --release --target x86_64-pc-windows-msvc`, abort on nonzero errorlevel (`build.bat:12-21`).
3. Same for `i686-pc-windows-msvc` (`build.bat:23-31`).
4. `cargo tauri build --target …` twice to produce NSIS/MSI bundles (`build.bat:33-37`), then `cd ..` and `pause`.
   - It builds **twice per arch** (once `cargo build`, once `cargo tauri build`) — redundant but harmless.

`build.ps1` (62 lines): same logic with a `-Arch both|x64|x86` parameter (`build.ps1:1-3`), `$ErrorActionPreference = "Stop"` (`build.ps1:5`), `Set-Location` into `src-tauri` per arch (`build.ps1:21`, `build.ps1:42`), `exit 1` on failure, `Read-Host` at the end. Note `-Yellow` is passed positionally to `Write-Host` (`build.ps1:16`) — it works only as an unbound value argument; cosmetic only.

### 2.3 `build-macos.sh` (43 lines)

- Refuses to run anywhere but Darwin (`build-macos.sh:7-11`), explicitly "Cross-building .dmg from Linux is not supported."
- `npm install` → `npm run build` → `cargo tauri build --target x86_64-apple-darwin` → `--target aarch64-apple-darwin` (`build-macos.sh:29-36`).
- No collection step: output stays in `src-tauri/target/<triple>/release/bundle/` (`build-macos.sh:41-42`).
- **No `.dmg` has ever been shipped** — the website says "There is no macOS build" (`xryloth-website-source/README.md`, "Notes" §).

### 2.4 `scripts/` packaging

| Script | Lines | Behaviour |
|---|---|---|
| `scripts/finalize-deb.sh` | 74 | Finishes an **electron-builder** `.deb` (the *old* Electron installer, `src-tauri/xryloth-installer/dist/`). Rewrites `Version:` in the control file because Cargo/electron-builder reject 4-part semver while Debian accepts it (`finalize-deb.sh:5-9`, `:50`); repacks with **xz** because Ubuntu 24.04's zstd default is unreadable by 22.04 dpkg (`:10-11`, `:69`); forces a lowercase `.deb` name and *asserts* it (`:34-43`); asserts `Icon=xryloth-installer` and `StartupWMClass=xryloth-installer` in the `.desktop` file and rejects a literal `[object Object]` value (`:57-66`); writes `release-v$VER/installer/xryloth_v${VER}_linux.deb`. |
| `scripts/package-installer-deb.sh` | 93 | Builds a `.deb` around the compiled Rust `xryloth_installer` binary: stages `/opt/XrylothInstaller/xryloth_installer` (`:35`), icon at `usr/share/icons/hicolor/256x256/apps/xryloth-installer.png` (`:38`), `/usr/bin/xryloth-installer` shim (`:41-45`), `.desktop` entry (`:48-59`), control file whose only real dep is `libwebkit2gtk-4.1-0, libfuse2 \| libfuse2t64` (`:67-87` — the comment explains that listing GTK/cairo/etc. directly caused "not installable" errors from t64 vs non-t64 naming), then `dpkg-deb -b -Zxz` (`:90`). Output `release/xryloth_v${VER}_linux.deb`. |
| `scripts/repack-appimage-webkit40.sh` | 68 | Makes a Tauri v1 AppImage run on distros without `webkit2gtk-4.0`. Finds the squashfs superblock by scanning for `hsqs` + version_major 4 at +28 and cuts the runtime off (`:21-40`); `--appimage-extract` (`:43`); deletes the 4.1/6.0 webkit + `libsoup-3.0` to avoid libsoup2/3 clashes (`:45-52`); copies the vendored `webkit2gtk-4.0` helper processes in (`:54-58`, prefix `WEBKIT40_PREFIX` default `~/.local/share/xryloth-compat/webkit40`); appends `LD_LIBRARY_PATH` to `apprun-hooks/linuxdeploy-plugin-gtk.sh` (`:61-62`); repacks with `mksquashfs … -comp gzip -b 128K` and concatenates `runtime.bin + new.sqfs` (`:65-66`). |
| `scripts/verify-launcher-assets.sh` | 115 | See §5. |
| `scripts/verify-launcher-window.sh` | 252 | See §5. |
| `scripts/tools/render-logo.sh` | 33 | Renders `scripts/tools/xryloth-logo.svg` to PNG via headless Chrome: wraps the SVG in a temp HTML page with transparent background (`:15-22`), then per size runs `google-chrome --headless --disable-gpu --no-sandbox --default-background-color=00000000 --window-size=N,N --screenshot=out` (`:29-31`). Arg form `<size>` or `<path>` (path form derives size from the leading digits of the basename, `:26-27`). |

### 2.5 `package.json` scripts (`package.json:6-18`)

```
dev            vite
build          tsc && vite build          ← the frontend step every release must run first
preview        vite preview
tauri          tauri
tauri:build:x64   tauri build --target x86_64-pc-windows-msvc
tauri:build:x86   tauri build --target i686-pc-windows-msvc
tauri:build:all   x64 then x86
tauri:build:linux tauri build --target x86_64-unknown-linux-gnu
tauri:build:macos tauri build --target x86_64-apple-darwin
build:bat      build.bat
build:ps1      powershell -ExecutionPolicy Bypass -File build.ps1
```

### 2.6 `src-tauri/.cargo/config.toml` (7 lines)

```toml
[target.x86_64-pc-windows-msvc]
linker = "lld-link"
ar = "llvm-lib"
[target.i686-pc-windows-msvc]
linker = "lld-link"
ar = "llvm-lib"
```
This is what lets cargo-xwin link MSVC targets from Linux without MSVC's `link.exe`.

### 2.7 `src-tauri/build.rs` (229 lines) — build-time behaviour worth knowing

- Embeds a Windows application manifest declaring Common-Controls **v6** + per-monitor DPI + `asInvoker` (`build.rs:25-54`); without it Windows throws
  *"The procedure entry point TaskDialogIndirect could not be located"* (`build.rs:17-24`, `:203-208`).
- Resolves secrets env-first, then `src-tauri/secrets.json`, then `../secrets.json` (`build.rs:126-150`).
- Writes `$OUT_DIR/embedded_keys.rs` with `DEFAULT_GROQ_KEY`, `DEFAULT_GEMINI_KEY`, `DEFAULT_GITHUB_API_KEY`, `DEFAULT_GROQ_MODEL`, `DEFAULT_GEMINI_MODEL` (`build.rs:173-194`) **and** re-exposes them as `cargo:rustc-env` (`build.rs:197-201`).
- Default models: `openai/gpt-oss-120b` (Groq), `gemini-3.5-flash-lite` (`build.rs:167-171`).
- `rerun-if-changed=secrets.json` + `rerun-if-env-changed` for the five key/model vars (`build.rs:154-160`).
- Calls `tauri_build::try_build` with the manifest (`build.rs:209-214`) but **downgrades failure to a cargo warning**.
- Fallback: when cross-compiling Windows from Linux with no `llvm-rc`/`windres` (`resource_compiler_available`, `build.rs:60-89`), it pushes the manifest through the linker with `/MANIFEST:EMBED` + `/MANIFESTINPUT:` (`build.rs:95-109`, `:220-228`) — so the cross-built .exe has a manifest but **no embedded icon/version resource** unless `llvm-rc` is installed (`build.rs:224-226` warns exactly this).

---

## 3. Release process & version-bump checklist

### 3.1 Version lives in FOUR places, and they currently disagree

| Source | Value | Cite |
|---|---|---|
| `package.json` → `version` | `7.0.0.1` | `package.json:4` |
| `src-tauri/Cargo.toml` → `version` (feeds `APP_VERSION` via `env!("CARGO_PKG_VERSION")`) | `6.0.0` | `Cargo.toml:3`, `lib.rs:35` |
| `src-tauri/tauri.conf.json` → `package.version` (feeds NSIS/AppImage/deb filenames) | `6.0.0` | `tauri.conf.json:12` |
| `APP_DISPLAY_VERSION` in `lib.rs` (what the updater compares against) | `7.0.0.1` | `lib.rs:39` |

Design note: Cargo requires 3-part semver, marketing releases are 4-part, so
`APP_VERSION` deliberately stays 3-part and `APP_DISPLAY_VERSION` carries `7.0.0.1`
(`lib.rs:36-38`). A same-release tag must therefore never look "newer".

### 3.2 Checklist (from `RELEASING.md:77-84`, plus what the scripts enforce)

1. Bump **all** of: `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`, `package.json`
   (`RELEASING.md:79-82`) — plus, in practice, `APP_DISPLAY_VERSION` in `lib.rs:39`,
   which `RELEASING.md` does **not** mention but `verify-launcher-assets.sh` reads
   (`verify-launcher-assets.sh:26`).
2. `npm run build` **before** any `tauri build` — `beforeBuildCommand` is empty
   (`tauri.conf.json:5`), so Tauri embeds whatever `dist/` already contains.
3. Build: `./build-all.sh` (Win x64+x86 NSIS + portables, Linux AppImage+deb), or the
   per-OS scripts (`RELEASING.md:106-110`).
4. Stage into `release-v<ver>/launcher/` (and `release-v<ver>/installer/` for the
   installer repo) — this is the layout every existing `release-v*` dir uses and the
   layout `verify-launcher-assets.sh` looks for (`verify-launcher-assets.sh:33`).
5. Run both regression checks (§5).
6. Publish a **normal** (not draft, not pre-release) GitHub Release; drafts and
   pre-releases are invisible to `/releases/latest` (`RELEASING.md:74-75`).
7. Tag must be numerically greater than `APP_DISPLAY_VERSION`
   (`RELEASING.md:69-73`); the leading `v` is stripped automatically.
8. Release **title** matters: the updater parses the dotted number out of `name`,
   falling back to `tag_name` (`lib.rs:3167-3187`).

### 3.3 Two repositories, two release trains

| What | Repo | Typical assets |
|---|---|---|
| Launcher | `EscapeXMc/xryloth` | `xryloth_v<ver>_linux.appImage`, `xryloth_v<ver>_Windows64.exe`, `xryloth_v<ver>_windows32.exe` (`RELEASING.md:136-143`) |
| Installer | `EscapeXMc/xrylothInstaller` | `xryloth_v<ver>_linux.deb`, `xryloth_v<ver>_windows_universal.exe`, `xryloth_v<ver>_windows32.exe` |

The launcher's updater reads the **launcher** repo (`lib.rs:40`,
`https://api.github.com/repos/EscapeXMc/xryloth/releases/latest`); the standalone
installer binary reads the same launcher repo (`installer/main.rs:30`); the
installer's own repo is only read by the installer tests
(`installer/main.rs:887`) and by the website's `windows-installer`/`linux-deb`
resolver keys (`xryloth-website-source/README.md` download table).
Older docs still say `EscapeXMc/Xryloth` with a capital X (`RELEASING.md:13`,
`installer/main.rs:29-30`) — GitHub repo names are case-insensitive for the API.

---

## 4. Asset naming / updater scoring contract

### 4.1 Launcher-side scoring — `asset_score` (`lib.rs:3123-3148`)

| OS | Condition | Score |
|---|---|---|
| Windows | `.exe` containing `setup` or `install` | `100` (`:3127`) |
| Windows | `.exe` containing `windows64`/`win64` on x86_64, or `windows32`/`win32` on x86 | `100` (`:3131-3133`) |
| Windows | any other `.exe` (portable) | **`-1` = rejected** (`:3134`) |
| Windows | `.msi` | `90` (`:3136`) |
| macOS | `.dmg` / `.pkg` / `.app` | `100` / `90` / `50` (`:3138-3140`) |
| Linux | `.deb` | `100` (`:3144`) |
| Linux | `.flatpak`, `.snap` | `50` (`:3145-3146`) |
| Linux | `.AppImage` | **`-1` = rejected** (`:3141-3148`) |
| anything else | | `-1` (`:3148`) |

Comments state the intent: since v6.x the *installer* owns AppImage replacement, so the
launcher only applies packages it can install itself (`lib.rs:3113-3122`, `:3142-3143`).

`pick_release_asset` walks all assets, skips `score < 0`, keeps the strictly-higher
score, and returns `(name, url, size)` (`lib.rs:3151-3165`).

> ⚠ `RELEASING.md:21-31` documents an **older** table (portable `.exe` = 60,
> `.AppImage` = 100 on Linux, `.deb` = 60). The code above is authoritative.

### 4.2 Version comparison — `release_version` (`lib.rs:3167-3187`)

Takes release `name`, falls back to `tag_name`, strips leading `v`/`V`, then takes the
first digit and everything that is a digit or `.`, trimming trailing dots. So the
release *title* `Xryloth v6.0.0.20` yields `6.0.0.20`. Numeric comparison is
component-wise padding with zeros (`lib.rs:3100-3108`). Only when
`version_gt(remote, APP_DISPLAY_VERSION)` does `release_info()` report
`update_available` (`lib.rs:3199-3214`).

### 4.3 Installer-side asset matching — `find_launcher_asset` (`installer/main.rs:166-186`)

Case-insensitive substring match against the **real** launcher asset names:

| Target | Must contain |
|---|---|
| Windows x64 | `windows64` or `x64` (`:178`) |
| Windows x86 | `windows32`, `windowsx86`, `x86`, or `ia32` (`:180`) |
| Linux | `appimage` **and** (`linux` or `amd64` or `x64`) (`:183`) |

Pinned by test `finds_real_release_asset_names` against
`xryloth_v6.0.0.16_{Windows64,Windows32}.exe` / `xryloth_v6.0.0.16_linux.appImage`
(`installer/main.rs:840-865`).

### 4.4 Website-side resolver keys (`xryloth-website-source/README.md`)

`public/xryloth-downloads.js` + `api/download.js` map
`windows → *Windows64.exe`, `windows-x86 → *windows32.exe`, `linux → *.appImage`,
`windows-installer → *windows_universal.exe`, `linux-deb → *.deb`, each from the right
repo; client result cached in `localStorage` for 15 min, serverless version 302-redirects.

### 4.5 Observed on-disk naming conventions (all under `release-v*/`)

```
release-v<ver>/launcher/xryloth_v<ver>_Windows64.exe     ← 64-bit launcher binary
release-v<ver>/launcher/xryloth_v<ver>_Windows32.exe     ← 32-bit (also seen as _windows32.exe)
release-v<ver>/launcher/xryloth_v<ver>_linux.appImage    ← lowercase "appImage" is required by §4.3
release-v<ver>/launcher/xryloth_v<ver>_amd64.deb
release-v<ver>/launcher/RELEASE_NOTES.md
release-v<ver>/installer/xryloth_v<ver>_{windows_universal.exe,windows32.exe,linux.deb}
release/      ← build-all.sh output: Xryloth_<ver>_x64-setup.exe, _x86-setup.exe,
                _x64-portable.exe, _amd64.AppImage, _amd64.deb   (flat, capital X)
release-v6/   ← oldest layout, mixed capitals incl. a broken `…_linux.Deb`
```
Hard rules enforced by code/scripts:
- `.deb` must be **lowercase** — GNOME Software will not open `.Deb`, and GitHub asset
  names are compared case-insensitively so `.Deb` + `.deb` 422 on upload
  (`finalize-deb.sh:34-43`).
- Windows filename must keep `setup`/`install` **or** the arch token
  `windows64`/`windows32`, otherwise the launcher rejects the asset (`lib.rs:3127-3134`).
- No two assets on one release may share a name; the raw portable output is just
  `xryloth.exe` and **must** be renamed with version + arch (`RELEASING.md:50-51`).
- `download_update` strips any path separator out of the destination filename
  (`lib.rs:10650-10657`).

### 4.6 How an update is actually applied (`lib.rs:10642-10789`)

- `download_update` → `~/.xryloth/updates/<name>` with streamed progress events
  (`lib.rs:10589`, `:10643-10673`).
- `install_update` dispatches per OS (`lib.rs:10677-10692`):
  - **Windows**: spawn the downloaded `.exe` with `DETACHED_PROCESS` (0x8), sleep 900 ms,
    `std::process::exit(0)` so NSIS can replace files (`lib.rs:10695-10709`).
  - **Linux**: if `$APPIMAGE` set → stage `<file>.new`, chmod 755, rename over the
    running AppImage, relaunch with the single-instance takeover flag
    (`lib.rs:10721-10746`); else if `.deb` → `pkexec`/`sudo dpkg -i` then relaunch
    (`lib.rs:10749-10766`); else replace `current_exe()` and relaunch
    (`lib.rs:10768-10789`).
  - **macOS**: `install_update_macos` exists at `lib.rs:10798` (not exercised; no `.dmg`
    has shipped).

---

## 5. Regression verification scripts

### 5.1 `scripts/verify-launcher-assets.sh` (115 lines)

*Why:* Tauri embeds the frontend at compile time (`distDir = "../dist"`) and
`beforeBuildCommand` is empty, so a binary built after a source change but before
`npm run build` silently ships a stale UI (`verify-launcher-assets.sh:5-15`).

Procedure:
1. Version from `$1`, else parsed out of `APP_DISPLAY_VERSION` in `lib.rs`
   (`:24-31`); staged dir must be `release-v$VER/launcher/` (`:33-34`).
2. Freshness fingerprint = newest `dist/assets/index-*.js` basename (`:36`).
3. Required marker = the command string `uninstall_launcher` (the Settings ▸ General
   uninstall button's backend command) — absence ⇒ UI and backend are from different
   builds (`:42-45`).
4. `check_binary`: `strings -a` the binary for `index-<8+chars>.js`, must contain the
   fresh hash; must also contain `uninstall_launcher` (`:50-61`).
5. `check_windows_asset`: if the marker isn't in the `.exe` itself, unpack the NSIS
   payload with `7z` and check the inner `xryloth.exe`/`Xryloth.exe` (`:66-89`) —
   commit `98419dd` added this unpacking step.
6. Linux: `--appimage-extract`, then check the >10 MB file in `squashfs-root/usr/bin`
   (`:97-108`).
7. Exit 1 with "FAILED -- do not upload." on any miss (`:110-115`).
   It also catches shipping the capitalised stale `Xryloth.exe` CLI instead of cargo's
   lowercase `xryloth.exe` (`:13-15`, `:91-92`).

### 5.2 `scripts/verify-launcher-window.sh` (252 lines)

*Why:* the "launcher minimises itself" regression — before 6.0.0.8 a second launch
left the window `Iconic`; GNOME refuses an app's own focus request, so the launcher
also sends a taskbar-style `_NET_ACTIVE_WINDOW` (header `:1-27`, pointing at the
WINDOW ACTIVATION section of `lib.rs`).

Prerequisites, each returning exit 2 (SKIP): file exists (`:30-31`), `python3`
(`:32`), `python3-xlib` (`:33`), `DISPLAY` set (`:34`), screen **not** locked
(GNOME refuses activation while locked — gdbus `org.gnome.ScreenSaver.GetActive` then
`loginctl LockedHint` fallback, `:40-59`), and no launcher already holding
single-instance port **51543** (`:61-78`, matches `SINGLE_INSTANCE_PORT` at
`lib.rs:11260`).

Flow:
- Throwaway `HOME` under `/tmp/opencode/verify-window.*`, with `user-dirs.dirs`
  redirected so the test can never touch the real Desktop (`:80-95`).
- Embedded python `probe.py` finds the window by `WM_CLASS == "xryloth"`, picks the
  mapped one (GTK's group-leader window carries the same class), and reports
  `WM_STATE` + `_NET_WM_STATE` ⇒ `visible`/`minimised`; `iconify` sends an ICCCM
  `WM_CHANGE_STATE` ClientMessage with `IconicState` (`:97-156`).
- Steps: start AppImage, wait ≤40 s for a window, assert not started minimised
  (`:195-207`); sleep 3 s then iconify up to 3 times (`:210-226`); start a second copy
  with the same throwaway env and a 30 s timeout — it must hand over and exit
  (`:229-233`); poll 6× for the window to become visible again (`:236-243`); print
  `LAUNCHER-WINDOW-TEST: PASS/FAIL` (`:246-252`).
- `cleanup` kills only processes whose `/proc/<pid>/environ` contains the throwaway
  `HOME` — never a blunt `pkill` (`:174-192`).

### 5.3 Rust tests (`src-tauri/src/installer/main.rs:805-959`)

- `config_from_ui_json_deserializes_and_normalizes` — UI omits `launcher_path`
  (v6.0.0.16 failed with "missing field `launcher_path`", `:809-825`).
- `config_empty_dir_falls_back_to_home` (`:828-838`).
- `finds_real_release_asset_names` (`:842-865`).
- `download_creates_missing_dir_and_streams_to_disk` — **network test** against the
  live `EscapeXMc/xrylothInstaller` release; asserts byte count, no leftover
  `.part`, executable bit (`:871-931`). Comment at `:878-880` explains why the size is
  read from the API: assets are re-uploaded on every publish.
- `fetch_real_github_release_and_pick_asset` — full network round trip + ranged GET
  (`:935-958`).
  ⇒ `cargo test` needs internet; these are not hermetic.

---

## 6. Windows / Linux / macOS build matrix

| | Windows x64 | Windows x86 | Linux x64 | macOS Intel | macOS ARM |
|---|---|---|---|---|---|
| Target triple | `x86_64-pc-windows-msvc` | `i686-pc-windows-msvc` | `x86_64-unknown-linux-gnu` | `x86_64-apple-darwin` | `aarch64-apple-darwin` |
| Built from | Linux (`cargo-xwin`) **or** Windows (MSVC) | same | Linux | macOS only | macOS only |
| Driver | `build-all.sh:72-73` / `build.ps1:32` | `build-all.sh:74-75` / `build.ps1:53` | `build-all.sh:80` (`npm run tauri:build:linux`) | `build-macos.sh:31` | `build-macos.sh:36` |
| Bundles | NSIS (`--bundles nsis`) | NSIS | AppImage + deb (from `tauri.conf.json:71-76`) | dmg | dmg |
| Extra tools | `makensis`, `cargo-xwin`, `lld-link` (`.cargo/config.toml`) | same | `pkg-config`, webkit2gtk, optionally `mksquashfs` for repack | Xcode | Xcode |
| Never produced in CI | — | — | — | no `.dmg` ever shipped (`xryloth-website-source/README.md` Notes) | — |

**cargo-xwin path in detail**

1. `cargo install cargo-xwin` (`build-all.sh:24-26`), `XWIN_ARCH=x86,x86_64,aarch64` (`:31`).
2. `rustup target add` for the MSVC triples (`:70-71`).
3. `tauri build --runner <tmp>/cargo` — the shim routes any MSVC triple to
   `$HOME/.cargo/bin/cargo-xwin` (`build-all.sh:47-58`).
4. Linking: `lld-link` + `llvm-lib` from `.cargo/config.toml`.
5. Manifest: if no `llvm-rc`, `build.rs` embeds the Common-Controls v6 manifest via the
   linker instead (`build.rs:220-228`) — icon/version resources are then missing.
6. The raw `cargo xwin build --release --target <triple> --bin xryloth` form is also
   documented for one-off exe builds (`RELEASING.md:91-94`).
7. Linux side of a cross build needs the webkit 4.0→4.1 pkg-config alias:
   either the repo-root symlink `webkit2gtk-4.0.pc -> webkit2gtk-4.1.pc`, or the
   `/tmp/opencode/pkgconfig` dir that `build-all.sh:37-42` prepends.

**CI:** a GitHub Actions workflow was added (`580f31c`, `30569cd`) and reverted
(`dea86e8` — "push via GitHub UI instead"); `.github/` does not exist in the working
tree. All releases are built locally.

---

## 7. Installer binary design (`src-tauri/src/installer/main.rs`, 959 lines)

### 7.1 Shape

- Second `[[bin]]` in the same crate: `xryloth_installer` → `src/installer/main.rs`
  (`Cargo.toml:16-18`). It therefore **shares `tauri.conf.json`** with the launcher.
- Purpose (header `main.rs:20-27`): one binary = installer on first run, updater on
  subsequent runs, uninstaller on demand.
- Constants: repo `EscapeXMc/Xryloth` (`:29-30`), install dir name `Xryloth` (`:31`),
  launcher file names `xryloth_launcher` (Linux) / `XrylothLauncher.exe` (Windows)
  (`:32-33`), uninstall URL `https://xrylothclient.vercel.app/uninstall/` (`:34`).
- Includes `$OUT_DIR/embedded_keys.rs` at compile time (`:17-18`) — same secret pipeline
  as the launcher.

### 7.2 State & config

- `InstallConfig` (`:49-61`): `install_dir`, `launcher_path`, three shortcut flags,
  `auto_start`, `version` (release tag recorded at install/update time).
  `launcher_path` is derived, not UI-supplied; `normalize()` recomputes it and falls
  back to `~/Xryloth` (`:63-79`, default impl `:81-100`).
- `InstallerState` (`:102-140`): config, reqwest `Client` (UA `XrylothInstaller/1.0`,
  300 s timeout), `is_update = launcher_path.exists()`, cached latest release, optional
  `AppHandle`.

### 7.3 Flow (per `installer.html:250-338` and the commands)

1. Startup: build a **420×470 non-resizable** window on `installer.html`, then
   **close the auto-created `main` launcher window** — because the shared config would
   otherwise boot the launcher UI (`main.rs:764-786`).
2. Emit `installer-status {installed}` asynchronously (`:788-795`).
3. `installer_check_installed` (`:557-567`) → `installer_get_config`, which prefers the
   saved `installer_config.json` (`:569-582`).
4. `installer_fetch_release` (`:590-606`) → `find_launcher_asset` → `installer_install`
   (`:608-645`): fetch → download → `copy_icons` (`:342-366`) → `create_shortcuts`
   (`:286-296`) → write `installer_config.json` with the tag (`:635-641`).
5. `installer_update` (`:647-698`): compares the **saved** tag to the latest tag —
   comment at `:660-663` records the bug where it compared against the freshly fetched
   tag and always said "Already up to date"; on change it re-downloads and rewrites the
   tag (`:682-692`).
6. `installer_launch_launcher` spawns detached (`:707-724`), then `installer_close_app`
   exits the installer (`:726-731`) — the "hands-free" flow from commit `a4444d2`.
7. `installer_uninstall` removes the launcher binary, `.desktop`/autostart (Linux) or
   `Xryloth.lnk`/`.url` (Windows), and the install dir if empty (`:504-544`).
   `installer_open_uninstall_page` opens the hosted uninstall page instead
   (`:733-744`).

### 7.4 Platform details

- **Download** streams chunk-by-chunk to `.xryloth_launcher.part`, throttles progress
  events to ~5/s, creates the install dir first (fresh-install ENOENT regression
  documented at `:201-213`, `:867-870`), chmod 0755 on Unix, then `rename`s into place
  (`:206-284`).
- **Linux shortcuts**: `~/.local/share/applications/xryloth.desktop` (note: the emitted
  entry literally contains `Categories=Game;` **twice**, `:316-318`), optional copy into
  `~/.config/autostart` (`:330-337`).
- **Windows shortcuts**: raw COM — `CoInitialize` + `IShellLinkW`/`IPersistFile` with
  hand-declared ShellLink GUIDs that winapi 0.3 doesn't expose (`:443-449`), writing
  `Desktop/Xryloth.lnk` and `…/Start Menu/Programs/Xryloth.lnk` (`:414-427`).

### 7.5 How it differs from the main app

| | Launcher (`xryloth`) | Installer (`xryloth_installer`) |
|---|---|---|
| Entry | `src/main.rs` (21 lines) → `xryloth_lib::run()` | own `fn main` (`:750-801`) |
| Window | `tauri.conf.json` "main" window, 1020×640 frameless transparent (`tauri.conf.json:145-159`) | built in `setup()` as window `"installer"`, 420×470 decorated, and the `main` window is closed immediately (`:767-786`) |
| Scope | full launcher: instances, mods, servers, chat, AI, updater | 10 commands only (`:752-763`) |
| HTML | `index.html` → Vite bundle | `installer.html` (389 lines, vanilla JS + global `__TAURI__`, `installer.html:195-197`) |
| Downloads | launcher release, applies `.exe`/`.deb`/binary itself (`lib.rs:10642+`) | launcher release, writes the launcher file into `~/Xryloth` |
| Version state | `APP_DISPLAY_VERSION` compiled in | `installer_config.json` tag on disk |

---

## 8. Secrets handling

### 8.1 `src-tauri/secrets.json` — **keys only, values never reproduced**

Exactly two keys exist:

- `groq_api_key` (string)
- `gemini_api_key` (string)

(`build.rs` also *looks for* `github_api_key` in the same file — `build.rs:165` — but
that key is **absent**, so it resolves to `""` unless the `GITHUB_API_KEY` env var is
set.)

### 8.2 Mechanism

- Resolution order: env var → `src-tauri/secrets.json` → `../secrets.json`
  (`build.rs:126-150`); a dependency-free hand-rolled JSON string extractor
  (`build.rs:111-123`) is used so `build.rs` needs no extra build-deps.
- Output: `$OUT_DIR/embedded_keys.rs` constants + `cargo:rustc-env` re-export
  (`build.rs:173-201`), included by the launcher at `lib.rs:28-33` and by the installer
  at `installer/main.rs:17-18`. Keys are therefore **inside the shipped binaries** —
  that is by design ("keys baked into binaries at build time", commit `0420a29`).
- Rerun triggers: `rerun-if-env-changed` ×5 and `rerun-if-changed=secrets.json`
  (`build.rs:154-160`).

### 8.3 Ignore rules (`.gitignore`)

```
.env, .env.*            (.gitignore:16-17)
secrets.json            (:18)
src-tauri/secrets.json  (:19)
DEV_LOG.txt             (:32)
```
Verified with `git ls-files`: **no secret file is tracked**; `CHAT_LOG.txt` *is*
tracked (`.gitignore` does not cover it), `DEV_LOG.txt` is not.

### 8.4 Secrets that leaked into tracked/untracked docs anyway (rotate these)

- `DEV_LOG.txt:150` prints a Groq API key in plaintext.
- `DEV_LOG.txt:183` prints an Ely.by OAuth client secret.
- `lib.rs:45` and `backend/engine.py:48` hard-code `ELYBY_CLIENT_SECRET`.
- `SESSION_NOTES.md:55` prints the Firebase API key (project `xryloth-chatsystem`).
- `DriveVault/src/App.tsx:47` ships a default master password literal.

### 8.5 `.gitignore` extras worth knowing (`.gitignore:1-47`)

Ignores `node_modules/`, `dist/`, `release/`, `release-v*/`, `src-tauri/target/`,
`src-tauri/gen/`, `*.rs.bk`, `*.pdb`, `*.log`, `.DS_Store`, `Thumbs.db`, `.vscode/`,
`.idea/`, `.xryloth/`, `DEV_LOG.txt`, `xryloth-installer/`,
`src-tauri/xryloth-installer/`, `src-tauri/*_setup.exe`,
`src-tauri/xryloth_installer_*.exe`, and the whole local scratch set: `cache/`,
`indic-tiny-tts-hindi/`, `insight-wrap-android/`, `minecraft-mods/`,
`minecraft-plugins/`, `xryloth-website-source/`, `webkit2gtk-4.0.pc`
(`.gitignore:40-47`).
Only **156 files** are tracked; `git status` shows the entire voice/TTS/DriveVault/
website set as untracked.

---

## 9. Git history summary

`git log --oneline | head -60` on the checked-out branch (`v6.0.0.13-release`) returns
only **9** commits — the branch starts at `ac4134b` (v6.0.0.9). The full picture needs
`--all`: **24 commits** spanning `23c6068` (2026-10-09) → `c78b994` (2026-11-05).

`git log --format="%h %ad %s" --date=short --all` (whole history) with dates:

```
c78b994 2026-11-05 feat: v7.0.0.1 - Glassphorium/Claymorphium UI + minimize fix
21c7968 2026-11-01 feat(installer): v6.0.0.21 — ultra-simple hands-free UI (one status line + progress, up-to-date auto-launch+close, 2-3 buttons only, custom confirm dialog)
a4444d2 2026-10-31 feat(installer): hands-free flow — auto-update on open, then auto-launch the launcher and close itself (async Result commands; sync () command registered no IPC name)
32843ca 2026-10-31 feat: v6.0.0.20 - black-screen fix (41 blocking commands off the UI thread), release/version parsing + Windows updater asset matching, installer test hardened against re-uploads
a9b118a 2026-10-25 feat: v6.0.0.13 - Orange theme installer, topbar navigation, lazy loading, font size settings
e9d3156 2026-10-25 feat: v6.0.0.13 - Orange theme installer, topbar navigation, lazy loading, font size settings
e8ef584 2026-10-24 chore: remove large binary files from git tracking
931efc1 2026-10-24 feat: v6.0.0.13 - Orange theme installer, topbar navigation, lazy loading, font size settings
30569cd 2026-10-24 Add GitHub Actions workflow for release builds
dea86e8 2026-10-24 revert: remove workflow file (push via GitHub UI instead)
580f31c 2026-10-24 ci: add GitHub Actions workflow for multi-platform builds
ac4134b 2026-10-24 feat: v6.0.0.9 - Game restart status, auto mod install, branding fixes
5490f47 2026-10-24 chore: remove large release files from git history
e945a2e 2026-10-24 feat: v6.0.0.9 - Game restart status, auto mod install, branding fixes
7ece51a 2026-10-21 FastClient pass 2: rail hierarchy, profile drawer, quick actions, floating install card
98419dd 2026-10-21 verify-launcher-assets: unpack the NSIS payload before checking it
391b874 2026-10-21 v6.0.0.9: FastClient-style UI pass - ember theme across the whole app
74c58f9 2026-10-20 v6.0.0.9: Xryloth orange theme, new ember mark, window activation fix
a8ac78a 2026-10-17 v6.0.0.6: monochrome UI redesign (white on deep black)
193a0d2 2026-10-16 v6.0.0.1: installer-first updates, Xryloth Installer (scope + dependency checks + Status.txt), launcher self-updater defers to installer
d8f02a2 2026-10-15 chore: remove source from public repo
fb5a723 2026-10-15 v5.9.6: fixed Windows TaskDialogIndirect crash (manifest), public/private server publishing, admin realtime announcements + pings, auto FPS mods
0420a29 2026-10-13 v5.9.5: working Partner Servers (realtime sync + real stats), redesigned sidebar, keys baked into binaries at build time
23c6068 2026-10-09 Xryloth 5.9.1
```

Observed patterns: duplicate/squashed commits (`e9d3156`/`a9b118a`/`931efc1`,
`e945a2e`/`ac4134b`), history rewrites to purge large binaries (`5490f47`, `e8ef584`),
a public/private split (`d8f02a2` "remove source from public repo"), CI added then
removed the same day, and no commit after `c78b994` (2026-11-05, HEAD of
`v6.0.0.13-release`).

Branches (`git branch -a`):

```
  fix/partners-realtime-v5.9.5
  main
* v6.0.0.13-release        ← checked out
  v6.0.0.9-release
  remotes/origin/HEAD -> origin/main
  remotes/origin/main
  remotes/origin/v6.0.0.13-release
  remotes/origin/v6.0.0.9-release
```
Note the checkout is a *release* branch while the working tree already carries the
`7.0.0.1` bump in `package.json`/`lib.rs` — and the tree is dirty (`.gitignore`
modified, `DriveVault/` + all TTS/website files untracked).

---

## 10. Gotchas

1. **Version drift.** `package.json` = 7.0.0.1, `Cargo.toml`/`tauri.conf.json` = 6.0.0,
   `APP_DISPLAY_VERSION` = 7.0.0.1 (§3.1). `build-all.sh` names release files from
   `package.json` (`build-all.sh:14`) while NSIS/AppImage internal metadata comes from
   `tauri.conf.json` — so a mismatched pair produces confusingly-named bundles.
2. **`beforeBuildCommand` is empty** (`tauri.conf.json:5`). Forgetting `npm run build`
   ships a stale UI with no error. That is precisely why
   `scripts/verify-launcher-assets.sh` exists — run it before every upload.
3. **`verify-launcher-assets.sh` wants `release-v<ver>/launcher/`**, but `build-all.sh`
   stages flat `release/` (`build-all.sh:86-106` vs `verify-launcher-assets.sh:33`).
   Copy/rename by hand or the check errors "no staged artifacts".
4. **Two `.deb` namings coexist**: `release-v*/…_linux.deb` (new) and the broken
   historical `release-v6/…_linux.Deb`; GitHub asset names are case-insensitive, so
   re-uploading a capitalised `.Deb` will 422 (`finalize-deb.sh:37-39`).
5. **`asset_score` rejects AppImage on Linux and portable `.exe` on Windows**
   (`lib.rs:3134`, `:3141-3148`) while `RELEASING.md` still documents the opposite
   table. Trust the code.
6. **The launcher's `deb` Depends list contains `libgdk-3-0`, which does not exist on
   Ubuntu 24.04** (`tauri.conf.json:114`) — the very error
   `package-installer-deb.sh:61-66` documents and avoids by depending only on
   `libwebkit2gtk-4.1-0`.
7. **Cross-built Windows exes may lack icon/version resources** when `llvm-rc` is
   absent — `build.rs:220-228` embeds only the manifest and prints a cargo warning
   telling you to `apt install llvm-18-tools`.
8. **`build.rs` swallows `tauri_build::try_build` errors as warnings**
   (`build.rs:212-214`) — a broken allowlist will not fail the build.
9. **`cargo test` for the installer hits the live GitHub API**
   (`installer/main.rs:871-958`); offline or rate-limited runs fail.
10. **`scripts/finalize-deb.sh` targets a subproject that no longer exists**
    (`src-tauri/xryloth-installer/dist`, `finalize-deb.sh:23`) — the Electron installer
    was replaced by the Rust `xryloth_installer`. Only
    `scripts/package-installer-deb.sh` applies to the current code.
11. **webkit2gtk 4.0 vs 4.1** is handled three different ways (repo-root `.pc` symlink,
    `/tmp/opencode/pkgconfig` from `build-all.sh:37-42`, and AppImage repacking via
    `repack-appimage-webkit40.sh`) — pick whichever exists on your machine.
12. **WebKit forces X11**: `main.rs:15-18` pins `GDK_BACKEND=x11` because all
    single-instance / window-activation logic goes through Xlib; Wayland-native would
    break `verify-launcher-window.sh` and the minimise-restore feature.
13. **Single-instance port 51543** (`lib.rs:11260`) — the window regression test (and
    any second launch) is blocked while a launcher runs.
14. **Stray artifacts in the root**: `kokoro-v1.0.int8.onnx` and `voices-v1.0.bin` are
    **HTML documents, not model files** (`file` reports "HTML document … with CRLF") —
    a HuggingFace download that saved an error page (see doc 13 §E4).
15. **`backend/engine.py` is not wired up**: `tauri.conf.json` resources list only
    `resources/mods/*` (`tauri.conf.json:84-86`) and no code references `engine.py`.
    The Rust `lib.rs` replaced it; don't build around it.
16. **The two `index.html.1`/`index.html.2` files are 530 KB GitHub homepage snapshots**,
    untracked and unused — safe to delete, but they are not part of the Vite build.
17. **`DEV_LOG.txt` and `CHAT_LOG.txt` document the whole history but contain live
    credentials** (§8.4); `.gitignore` covers `DEV_LOG.txt` only, `CHAT_LOG.txt` is
    tracked.

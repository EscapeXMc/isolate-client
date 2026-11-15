# Isolate Client

A lean Minecraft launcher. Blue theme, no bloat, built to feel **smooth as butter**.

```
blueprint/     End-to-end documentation of the launcher this was modeled on
src/           React 18 + TypeScript UI
src-tauri/     Rust core (Tauri 1.6)
assets/        Source logo
public/        Static assets served to the UI
```

## Stack

| Layer | Choice | Why |
|---|---|---|
| Shell | **Tauri 1.6** | tiny binary, native webview |
| UI | **React 18 + Vite 5 + TypeScript** | fast HMR, fast builds |
| State | **Zustand** | selector-based re-renders, no provider tree |
| Styling | **Plain CSS** | no Tailwind runtime, no CSS-in-JS |

Deliberately **not** used: Tailwind, Framer Motion, react-router, webfonts, AI,
Discord RPC, server manager, news feed, updater, custom titlebar.

## Performance rules

- No `backdrop-filter` / blur — flat colours only.
- No `requestAnimationFrame` loops and no infinite keyframes (the single
  exception is the loading spinner and the modal fade-in, both one-shot).
- CSS transitions limited to `transform`, `opacity`, `background-color`,
  `border-color`.
- Zustand selectors so a log line doesn't re-render the whole app.
- Solid background colour painted in `index.html` — no white flash on boot.
- Downloads run on a worker pool in Rust; the UI thread never blocks.

## Features

- **Instances** — create / rename / delete, per-instance RAM, JVM args and Java.
- **Loaders** — Vanilla, Fabric, Quilt (all-OS native resolution, correct
  `bin/java` vs `bin/java.exe` per platform).
- **Install pipeline** — version JSON, client jar, assets (8-way parallel),
  libraries, natives; streams `install-progress` events.
- **Launch pipeline** — classpath, rule filtering, argument substitution,
  native extraction, playtime tracking, `game-exit` / `game-output` events.
- **Accounts** — offline with a proper MD5 version-3 UUID, plus Microsoft
  device-code sign-in (XBL → XSTS → Minecraft → profile).
- **Java** — auto-detect system JDKs, or download Temurin from Adoptium.
- **Logs** — launcher log and live game output.

## Data

Everything lives in `~/.isolate/`:

```
settings.json          ram_mb, java_path, offline_username, ms_account
instances.json         instance records
instance_settings.json per-instance overrides
playtimes.json         name → seconds
instances/<name>/      client.jar, <version>.json, natives/, mods/, …
libraries/             shared library + native jars
assets/                shared asset index + objects
java/                  managed Temurin JDKs
```

## Build

```bash
npm install
npm run tauri dev      # develop
npm run tauri build    # package
```

Requirements: Node 20+, Rust 1.77+, and on Linux the usual Tauri system libs
(`libwebkit2gtk-4.1-dev`, `libgtk-3-dev`, `libayatana-appindicator3-dev`).

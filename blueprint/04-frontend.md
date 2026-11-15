# 04 — React Frontend (`XrylothTauri/src/`)

Source root: `/mnt/Data Drive/EscapeX Products/Xryloth/XrylothTauri/src/`
~26,000 LOC across ~93 TS/TSX/CSS files. Stack: React 18 + Vite + TypeScript + Tailwind + Zustand + Framer Motion + Tauri v2 APIs.

> NOTE ON CITATIONS: line numbers below were verified by reading the actual files during documentation. `invoke("cmd")` cross-references in §8 were collected by grepping `src/` for each command string.

---

## 1. Entry & boot sequence

### 1.1 `index.html` (project root, 14 lines)
- Plain HTML shell: `<title>Xryloth</title>`, inline style forcing `html, body { background: #050505 !important; }` (`index.html:8`), body classes `bg-[#050505] text-white overflow-hidden select-none` (`index.html:10`).
- Mounts `#root`, loads `/src/main.tsx` as a module (`index.html:11-12`). Favicon is `/favicon.svg`.
- No CDN scripts, no fonts loaded in HTML (fonts come from CSS).

### 1.2 `vite.config.ts` (23 lines)
- `@vitejs/plugin-react`; alias `@` → `./src` (`vite.config.ts:8-10`).
- Dev server port **1420**, `strictPort: true` (`vite.config.ts:13-16`) — the classic Tauri port.
- `envPrefix: ["VITE_", "TAURI_"]` (`vite.config.ts:17`).
- Build target `chrome105` on Windows else `safari13`; esbuild minify unless `TAURI_DEBUG`; sourcemap only in debug (`vite.config.ts:18-22`).

### 1.3 `src/main.tsx` (71 lines)
- Renders `<React.StrictMode>` root at `main.tsx:67-71`.
- **Browser guard**: if `isTauri()` is false it renders `BrowserNotice` instead of `<App />` (`main.tsx:69`). `BrowserNotice` (`main.tsx:7-65`) is a full-screen styled card telling the user the Vite dev server in a plain browser has no `window.__TAURI_IPC__` bridge so every `invoke()` fails, and to run `npm run tauri dev`.
- Imports `./index.css` (`main.tsx:4`).

### 1.4 `src/lib/tauri.ts` (19 lines)
- Exports `isTauri(): boolean` — returns true when `typeof window.__TAURI_IPC__ === "function"` **or** `typeof window.__TAURI__ !== "undefined"` (`tauri.ts:15-19`). This is the environment guard for every Tauri-only code path.

### 1.5 `src/App.tsx` (278 lines) — root component & boot orchestration

**Imports** (`App.tsx:3-25`): framer-motion, `useLauncherStore`, `MainLayoutShell`, `SplashScreen`, `GameBoostOverlay`, `LaunchAnimation`, `LoadingOverlay`, `LaunchOverlay`, `WizardOverlay`, `MusicPlayer`, `AnnouncementHost`, `ToastProvider`, `DialogHost`, `PartnerNotificationListener`, `api`, Tauri `listen`, `isTauri`, `warmImageCache`, `fetchPartners`, `checkNavGuard`, updater functions (`attachUpdateListeners`, `checkForUpdates`, `startUpdate`), `watchAuthState`.

**`ErrorBoundary`** (`App.tsx:27-59`): class component; on error renders a `glass-card` with "Something went wrong" + error message and a "Reload Page" button that resets state. Uses `useLauncherStore.getState()` (non-reactive) for theme colors (`App.tsx:40`).

**Boot `useEffect`** (`App.tsx:131-158`), runs once:
1. `loadSettings()` → then `loadInstances()` → then `warmStartupImages()` (best-effort) (`App.tsx:135-148`).
2. `api.wizard.checkFirstLaunch()` → if `first_launch` is true, `setShowWizard(true)` → first-run `WizardOverlay` (`App.tsx:145-147`).
3. `fetchLiveVersion()` and `loadSystemInfo()` fire unconditionally (`App.tsx:149-150`).
4. Registers `window` listener for custom event **`ec-launch-game`** whose `detail.instanceName` triggers the boost overlay (`App.tsx:152-157`).

**`warmStartupImages()`** (`App.tsx:103-118`): pre-downloads art to `~/.xryloth/image_cache` via `warmImageCache()`:
- mc-heads avatar `https://mc-heads.net/avatar/8660ba3e76424a96b46f0b738fcc8806?size=64` (`App.tsx:107`)
- every partner `logo`/`banner` http(s) URL and `https://api.mcsrvstat.us/icon/${p.ip}` for each partner IP (`App.tsx:111-113`).

**AI action listener** (`App.tsx:163-178`): when Tauri, listens to Tauri event **`ai-action`**; payload `{type:"open-create-instance"}` → after 450 ms dispatches window CustomEvent `ec-open-create-instance` so the Profiles page (already navigated to by the AI) can open its Create wizard.

**Auth watcher** (`App.tsx:180`): `useEffect(() => watchAuthState(), [])` — attaches a Firebase auth state listener for the app lifetime.

**Updater** (`App.tsx:182-193`): `attachUpdateListeners()` then after a **5 s** timeout: skip if `settings.auto_check_updates === false`; `checkForUpdates()`; if update available and `settings.auto_update !== false`, `startUpdate()` after another 1.8 s.

**Global keyboard shortcuts** (`App.tsx:195-230`):
| Shortcut | Action |
|---|---|
| `Ctrl/Cmd+N` | nav-guard → `setPage("instances")` + dispatch `ec-open-create-instance` |
| `Ctrl/Cmd+M` | nav-guard → `setPage("mods")` |
| `Ctrl/Cmd+,` or `Ctrl/Cmd+.` | `setPage("settings")` |
| `Ctrl/Cmd+T` | nav-guard → `setPage("themes")` |
| `F5` | prevent default, dispatch `ec-refresh-current` |
| `Escape` | dispatch `ec-escape-pressed` |
| Escape while in an input | blur the input (`App.tsx:198-201`) |

**Title** (`App.tsx:232-234`): `document.title = \`Xryloth — ${themeName capitalized}\``.

**Render tree** (`App.tsx:251-277`):
- Before splash completes → `<SplashScreen onComplete=...>` only (`App.tsx:251-253`).
- After: `ToastProvider` → `ErrorBoundary` → **`MainLayoutShell`**, plus `LoadingGate` (subscribes only to `isLoading`, renders `LoadingOverlay`), optional `LaunchOverlay`, always `MusicPlayer`, `AnimatePresence`-wrapped `GameBoostOverlay` → on complete launches the instance via `api.instances.launch(name)` (`App.tsx:240-247`) then runs `LaunchAnimation`, `AnimatePresence`-wrapped `WizardOverlay`, `AnnouncementHost`, `PartnerNotificationListener`, `DialogHost` (`App.tsx:255-276`).
- `LoadingGate` exists specifically so App never subscribes to the whole store (comment at `App.tsx:85-90` explains a prior perf bug where App destructured the WHOLE store).

**Page transition motion** (`App.tsx:61-83`): `pageVariants` (opacity/y/scale, 0.25 s tween ease `[0.4,0,0.2,1]`) and a reduced `potatoPageVariants` (opacity only, 0.08 s linear) used on low-end devices.

### 1.6 Routing — `src/components/LazyPages.tsx` (55 lines)
There is **no react-router**. Navigation is a single string `currentPage` in the Zustand store; `renderPage(currentPage)` maps it to a `React.lazy` component (`LazyPages.tsx:33-46`), defaulting to `HomePage` for unknown keys (`LazyPages.tsx:49`), wrapped in `Suspense` with a spinner `PageLoader` (`LazyPages.tsx:18-31, 48-54`).

Lazy page map (`LazyPages.tsx:5-16, 33-46`):
| key | module |
|---|---|
| `home` | `pages/HomePage` |
| `profiles` | `pages/ProfilesPage` |
| `instances` | `pages/InstancesPage` |
| `settings` | `pages/SettingsPage` |
| `accounts` | `pages/AccountsPage` |
| `chat` | `pages/ChatPage` |
| `xai` | `pages/XrylothAIPage` |
| `partners` | `pages/PartnersPage` |
| `logs` | `pages/LogsPage` |
| `servers` | `pages/ServersPage` |
| `themes` | `pages/ThemesPage` |
| `mods` | `mods/ModBrowserPage` |

Default `currentPage` in the store is **`"play"`** (`stores/index.ts:110`) — which is NOT in the map, so first render falls back to `HomePage` (`LazyPages.tsx:49`).

### 1.7 Splash screen — `src/components/ui/SplashScreen.tsx` (178 lines)
Rendered by App before anything else; calls `onComplete` when done (details in §6).

---

## 2. Layout shell & navigation

### 2.1 `components/layout/MainLayoutShell.tsx` (138 lines) — THE active shell
Imported by `App.tsx:6`. Documented architecture (comment at `MainLayoutShell.tsx:12-34`): a **fixed 3-column layout**:
- **Col 1**: `SidebarRail` (`w-20`, always visible) (`MainLayoutShell.tsx:78-83`)
- **Col 2**: center canvas — `HeaderBar` + page content (`MainLayoutShell.tsx:85-115`)
- **Col 3**: right hub `w-80` containing `ServerWidget`, rendered **only when `currentPage === "play"`** (`MainLayoutShell.tsx:46, 117-135`)

Key behaviors:
- **Ambient ember field**: a `pointer-events-none absolute inset-0 z-0` div with three static radial gradients (amber/orange/white) painted under all columns (`MainLayoutShell.tsx:59-68`). Explicitly "no blur cost" per comment.
- **Window controls pinned to true top-right**: `<div className="absolute top-0 right-0 z-30"><WindowControls /></div>` (`MainLayoutShell.tsx:70-75`). The long comment (`MainLayoutShell.tsx:23-30`) explains this was moved out of the header to fix a bug where the minimize button sat mid-title-area and clicks minimized the window.
- **Full-bleed pages**: `chat`, `xai`, `servers` render without the max-width/scroll wrapper (`MainLayoutShell.tsx:49, 91-97`); all other pages get `<main className="flex-1 overflow-y-auto p-6">` + `max-w-7xl mx-auto` (`MainLayoutShell.tsx:100-113`).
- `play` page renders `HeroSection` + `BottomGrid` directly instead of a lazy page (`MainLayoutShell.tsx:102-106`).
- Right hub top band is a **drag region** (`data-tauri-drag-region`, `MainLayoutShell.tsx:125-128`); `ServerWidget.onJoin` navigates to `partners` (`MainLayoutShell.tsx:129-133`).
- Uses **narrow zustand selectors** so shell re-renders only on route/sidebar changes (`MainLayoutShell.tsx:36-41`).
- Tab label mapping: `play` → `"PLAY"`, otherwise capitalized page name (`MainLayoutShell.tsx:44`).
- Right hub has `pt-14` to clear the corner window controls (`MainLayoutShell.tsx:120`).

### 2.2 `components/layout/SidebarRail.tsx` (84 lines) — left icon rail (active nav)
- `RAIL_ITEMS` (`SidebarRail.tsx:17-27`), each `{id, label, icon, color}`:
  `play`(Gamepad2), `profiles`(Layers), `accounts`(Users), `chat`(MessageSquare), `xai`(Sparkles), `partners`(Zap), `servers`(Server), `mods`(ShoppingBag, label **"STORE"**), `settings`(Settings).
- `<aside className="w-20 bg-[#0A0C0E]/75 backdrop-blur-md border-r ...">` with claymorphic amber logo tile at top (`SidebarRail.tsx:47-52`, class `clay-ember`), nav buttons in the middle, empty reserved bottom div (`SidebarRail.tsx:81`).
- Active button style: `bg-white/[0.09]` + inset highlight shadow; inactive: `text-zinc-500 hover:text-white` (`SidebarRail.tsx:66-70`).
- Click handler calls both `setTab(item.id)` and `setPage(item.id)` then `onClose()` (`SidebarRail.tsx:61-65`).
- Note: `setPage` from store is called *in addition to* the passed `setTab` (duplicate navigation call, harmless).
- Odd effect: `useEffect(() => { if (!isOpen) onClose(); }, [isOpen, onClose])` (`SidebarRail.tsx:41-43`) — calls `onClose` whenever closed.

### 2.3 `components/layout/HeaderBar.tsx` (155 lines) — center header + window controls
Exports three components:
1. **`WindowControls`** (`HeaderBar.tsx:10-49`): the custom min/max/close buttons in a rounded pill (`rounded-bl-2xl`, `data-tauri-drag-region`), each a `GlowButton`:
   - Minimize → `appWindow.minimize()` (`HeaderBar.tsx:19`)
   - Maximize → toggle via `appWindow.isMaximized()` → `unmaximize()`/`maximize()` (`HeaderBar.tsx:28-32`)
   - Close → `appWindow.close()` (`HeaderBar.tsx:41`)
   (All from `@tauri-apps/api/window` `appWindow`, `HeaderBar.tsx:5`.)
2. **`AuthPill`** (internal, `HeaderBar.tsx:51-85`): subscribes to session via `getSession()`/`onSessionChanged()` from `lib/auth`. Signed in → emerald pill with username + optional `ADMIN` badge, click → `setPage("accounts")`; signed out → "Sign In" button → `setPage("accounts")`.
3. **`AccountSelector`** (internal, `HeaderBar.tsx:87-114`): shows first letter avatar + username resolved as `session.username || settings.username || settings.cracked_username || settings.ely_by_username || "No Account"` (`HeaderBar.tsx:92-97`). (ChevronDown shown but **no dropdown implemented**.)
4. **`HeaderBar`** (`HeaderBar.tsx:120-155`): `<header data-tauri-drag-region>` `h-14`, with `WebkitAppRegion: "drag"` inline style (`HeaderBar.tsx:128`); contains a Zap toggle button (calls `toggleSidebar`), "XRYLOTH" brand + current tab subtitle, then `AuthPill` + `AccountSelector`. Comment at `HeaderBar.tsx:151-152` notes window controls live elsewhere (MainLayoutShell).

### 2.4 `components/layout/ServerWidget.tsx` (447 lines) — right hub content
- Renders inside the shell's right `aside` (does not create its own width — comment `ServerWidget.tsx:125-131`).
- **Live partner list**: `subscribePartners()` from `lib/partners` (Firebase realtime via Rust) (`ServerWidget.tsx:143-151`).
- **Status polling**: `fetchStatuses(ip, port)` (mcsrvstat.us via Rust) for every partner, on mount and every **60 s** (`ServerWidget.tsx:153-183`).
- **Helpers**:
  - `joinAddress()` builds copy-able address, default port 25565, avoids double-appending port (`ServerWidget.tsx:31-36`).
  - `safeUrl()` passes through `http(s):|data:|blob:` URLs untouched (data-URL partner logos!) else prefixes `https://` (`ServerWidget.tsx:46-52`).
  - `pingIconUrl()` → `https://api.mcsrvstat.us/icon/<address>` (`ServerWidget.tsx:55-57`).
  - `PartnerLogo` 3-step fallback: partner logo → ping icon → mcsrvstat icon → letter avatar underneath (`ServerWidget.tsx:83-123`).
- **Actions**: `copyIp` via Tauri clipboard `writeText` with `navigator.clipboard` fallback + toast (`ServerWidget.tsx:186-200`); `join` reports to `onJoin` or `setPage("partners")` (`ServerWidget.tsx:202-206`); `openExternal` via `@tauri-apps/api/shell` `open()` with `window.open` fallback (`ServerWidget.tsx:208-219`).
- Discord/Store URLs come from `settings.discord_url` / `settings.webstore_url` (`ServerWidget.tsx:221-222`).
- UI regions: header row with total-online badge + refresh button; scrollable list of partner cards (banner with pixel-grid gradient behind image, logo+name, status ping, player badge, description, address + copy + Join buttons, per-server Discord/Store/Site quick links); bottom Discord + Xryloth Store panels (`ServerWidget.tsx:236-445`).
- Uses `useToast()` for feedback; contains a `sr-only` span "so unused themeColors doesn't break lint" (`ServerWidget.tsx:441-444`).

### 2.5 `components/layout/HeroStage.tsx` (563 lines) — Play page hero + bottom grid
Exports **`HeroSection`** and **`BottomGrid`** (both used by MainLayoutShell for `play`).
- Utils (`HeroStage.tsx:14-43`): `mcId()` normalizes UUID → falls back to default hash `8660ba3e76424a96b46f0b738fcc8806`; `avatarUrl()` → `https://mc-heads.net/avatar/<id>?size=64`; `timeAgo()`, `fmtPlay()`, `prettyLoader()`.
- **HeroSection** (`HeroStage.tsx:46-367`):
  - Selectors: `instances`, `playTimes`, `setPage`; computes `installed` (filter `x.installed`), `recent` (sorted by `last_played` desc), `totalH` (sum playTimes/60) (`HeroStage.tsx:58-77`).
  - **Launch** dispatches window CustomEvent `ec-launch-game` with `{instanceName, version, loader}` (`HeroStage.tsx:81-88`) — App listens and runs boost overlay → `api.instances.launch()`.
  - Time-based greeting (`HeroStage.tsx:90-100`), username from store settings (`HeroStage.tsx:102-106`).
  - **Steve graphic fallback chain**: `/assets/steve-vector.svg` → `/assets/steve-render.png` → live 3D `<SteveModel>` (`HeroStage.tsx:55-56, 154-169`).
  - UI regions: user badge pill with CachedImage avatar + pulsing presence dot (`HeroStage.tsx:176-191`); greeting block (`HeroStage.tsx:195-206`); status chip (`HeroStage.tsx:210-224`); **split LAUNCH GAME button** (main action + chevron profile-picker dropdown, framer-motion AnimatePresence, list of recent instances + "Manage Profiles") (`HeroStage.tsx:226-303`); quick profile selector cards (top 4 + "Add Profile" dashed card) (`HeroStage.tsx:305-361`).
  - Ambient floating orbs are radial gradients, deliberately no CSS blur (`HeroStage.tsx:124-139`).
- **BottomGrid** (`HeroStage.tsx:385-563`):
  - News: `api.news.getEnhanced().catch(() => api.news.get())`, take 4; fallback `NEWS_FALLBACK` hardcodes `https://github.com/EscapeXMc/Xryloth/releases/latest` and `https://discord.gg/` (`HeroStage.tsx:370-404`).
  - Friends presence: `subscribeToOnlineUsers()` from `lib/chatService` (`HeroStage.tsx:406-410`), shows up to 8 online (`HeroStage.tsx:512-553`).
  - News cards: `<a target="_blank">` with image fallback to gradient+pixel grid, category pill, timeAgo, "Read more ↗" (`HeroStage.tsx:431-511`). "All News" button → `setPage("announcements")` (`HeroStage.tsx:424`) — note `announcements` is not in LazyPages map, so it falls back to HomePage.
  - Hint strip pointing to the right hub (`HeroStage.tsx:555-559`).

### 2.6 `components/layout/MainLayout.tsx` (61 lines) — DEAD CODE
An older shell: full-store subscription `useLauncherStore()` (no selectors, `MainLayout.tsx:12-13`), always renders `ServerWidget` (no `showRightHub`), applies `themeColors.bg_main` inline. **Not imported anywhere** (verified by grep; only App.tsx imports `MainLayoutShell`). Same basic shape: SidebarRail + HeaderBar + Hero/BottomGrid on `play` else lazy page.

### 2.7 Other dead layout files (verified: zero importers)
- `components/layout/Sidebar.tsx` (105 lines): slide-in `w-64` drawer nav with `NAV_ITEMS` = play/instances("Profiles")/mods/partners/accounts/settings (`Sidebar.tsx:20-27`), hardcoded version badge **"v6.0.0.15"** (`Sidebar.tsx:92`), `font-lemon` brand. Superseded by SidebarRail.
- `components/layout/TopBar.tsx` (106 lines): `h-[56px]` header with menu toggle, brand, QuickTools (Game Boost → instances, Theme → settings, Notifications badge=3, Refresh → `ec-refresh-current`), dark/light `setThemeMode` toggle (`TopBar.tsx:19-92`). Superseded by HeaderBar.
- `components/layout/HeroSection.tsx` (339 lines): an earlier standalone hero (contains its own picker, `setPage("instances")` calls at lines 188, 236, 270). Superseded by HeroStage's HeroSection.

### 2.8 Nav guard — `lib/navGuard.ts` (18 lines)
Module-level singleton, not React state:
- `setNavGuard(fn)` / `clearNavGuard()` register a callback (`navGuard.ts:4-10`).
- `checkNavGuard(target): boolean` calls the guard unless `target === "settings"` (settings always allowed) (`navGuard.ts:13-18`).
Used by App keyboard shortcuts (`App.tsx:208, 211, 217`) and by pages (e.g. SettingsPage sets a guard when there are unsaved changes — see §5).

### 2.9 Nav-guard & cross-page event bus (window CustomEvents)
Navigation and cross-component signaling use `window.dispatchEvent`/`addEventListener` CustomEvents rather than router params:
| Event | Direction | Purpose |
|---|---|---|
| `ec-launch-game` | HeroStage/HomePage → App | start launch flow (boost overlay → `api.instances.launch`) (`HeroStage.tsx:83`, `App.tsx:152-157`) |
| `ec-open-create-instance` | App (after Tauri `ai-action`) → ProfilesPage | open Create wizard (`App.tsx:171`) |
| `ec-refresh-current` | TopBar/F5 → pages | refresh current page (`TopBar.tsx:65`, `App.tsx:220`) |
| `ec-escape-pressed` | App → pages | global Escape (`App.tsx:224`) |
| `ai-action` (Tauri event) | Rust/AI → App | AI-driven UI actions (`App.tsx:168`) |

---

## 3. State management

Only **two** Zustand stores exist, both in `src/stores/`.

### 3.1 `stores/index.ts` (418 lines) — `useLauncherStore` (the one store)
Created with `create<LauncherStore>()(persist(...))` (`stores/index.ts:105`), storage key **`xryloth-launcher-storage`** (`stores/index.ts:408`), `partialize` persists only `themeName, themeMode, sidebarOpen, settings, instances, playTimes` (`stores/index.ts:409-416`).

**State shape** (interface at `stores/index.ts:45-103`):
| Field | Type / default | Notes |
|---|---|---|
| `settings` | `Settings` = `{}` | backend settings mirror |
| `instances` | `Instance[]` = `[]` | profiles |
| `currentPage` | `string` = **`"play"`** | the "router" |
| `selectedInstance` | `Instance \| null` | |
| `isLoading` / `loadingMessage` | `boolean`/`""` | drives `LoadingOverlay` |
| `themeName` | `string` = `"orange"` | `DEFAULT_THEME`; legacy `"black"` remapped (`stores/index.ts:35-41`) |
| `themeColors` | `ThemeColors` | resolved via `resolveThemeColors(themeName, mode)` (`stores/index.ts:8-13`): custom localStorage theme → `THEMES[themeName]` → `THEME_VARIANTS[mode]` |
| `themeMode` | `"dark"\|"light"` | read from `localStorage["ec-theme-mode"]` (`stores/index.ts:17-23`) |
| `logs` | `string[]` | |
| `playTimes` | `Record<string, number>` | minutes per instance |
| `systemInfo` | `{cpu, ram_pct, ram_used, ram_total, disk_pct, disk_used, disk_total}` zeros | |
| `liveVersion` / `localVersion` | `"5.9"` / `"5.9"` | live = latest GitHub tag, local = installed APP_DISPLAY_VERSION (`stores/index.ts:66-71`) |
| `releaseNotes` | `""` | |
| `sidebarOpen` | `true` | |
| `servers` / `serverLoading` | `ServerStatus[]` / `false` | |

**Actions**:
- `loadSettings()` → `api.settings.get()`, re-resolves theme (`stores/index.ts:135-149`).
- `saveSettings(s)` → `api.settings.save(s)` then merges + re-resolves theme (`:151-166`).
- `setSetting(key, value)` → `api.settings.saveOne`; special-cases `key === "theme"` (`:168-182`).
- `loadInstances()` → `api.instances.get()` (`:184-191`).
- `setPage(page)`, `setSelectedInstance`, `addInstance` (client-side `crypto.randomUUID()` id, `:198-204`), `updateInstance`, `deleteInstance` (client-side only — no backend delete) (`:193-216`).
- `setLoading(loading, message)` (`:218-219`).
- `setTheme(name)` → updates state + `api.settings.saveOne("theme", name)` (`:221-229`).
- `setThemeMode(mode)` → state + `localStorage["ec-theme-mode"]` (`:231-237`).
- `toggleSidebar()` (`:239-240`).
- `loadLogs()` → `api.logs.get()`; `log(msg)` appends timestamped line locally (`:242-252`).
- `loadPlayTimes()` → `api.playTimes.get()` (`:254-262`).
- `loadSystemInfo()` → `api.system.getInfo()` (`:264-269`).
- `fetchLiveVersion()` → `api.version.getLatest()` → sets `liveVersion` (tag), `localVersion` (current_version), `releaseNotes` (`:271-282`).
- **Mod ops**: `installMod(slug, version?)` requires `selectedInstance`, runs `api.mods.install` + refresh via `api.instances.getMods`, wrapped in loading (`:285-310`); `uninstallMod` → `api.instances.deleteMod` + refresh (`:312-328`); `toggleMod` → `api.instances.toggleMod` + optimistic local flip (`:330-352`).
- `refreshServers()` (`:355-405`): `api.servers.get()` then `api.servers.ping` each in parallel — **but the final `set()` maps every result to a hardcoded offline stub** (`stores/index.ts:388-399`: `online:false, version:"", motd:""`), discarding all ping data. This looks like a bug / dead path (serverStore.ts has the correct version of this logic).

**Custom theme colors in localStorage**: key `ec-custom-themes` (`stores/index.ts:15`), read by `loadCustomThemeColors` (`:25-33`).

### 3.2 `stores/serverStore.ts` (101 lines) — `useServerStore`
Plain `create` (no persist) (`serverStore.ts:34`).
- Types: `ServerStatus` (`:4-16`), `ServerEntry` (`:18-22`).
- State: `servers: ServerStatus[]`, `isLoading: boolean`.
- Actions: `setServers`, `setLoading`; `addServer(entry)` → `api.servers.add(name,address,port)` + `refreshServers()` (`:41-48`); `removeServer(address)` → `api.servers.delete(address)` + refresh (`:50-57`); `refreshServers()` (`:59-101`) → `api.servers.get()` + parallel `api.servers.ping`, correctly keeping ping results (contrast with launcher store).
- Used by ServersPage (see §5).

---

## 4. The `lib/` layer

### 4.1 `lib/api.ts` (399 lines) — the single IPC facade
- Wraps `invoke` from `@tauri-apps/api/tauri` in a local `invoke()` that **rejects with a clear error when `isTauri()` is false** (`api.ts:9-16`).
- Exports one object `api` (`api.ts:18`) grouped by domain. Every property is a function returning `invoke<T>("command", args)`. This is the ONLY place (besides `imageCache.ts` and `lib/partners`' dynamic import) that calls Tauri commands — so the full command surface is enumerable from here (see §8 for the complete cross-reference table with call sites).

Domain groups and their commands (all defined in `api.ts`; line ranges noted):
| Group | Lines | Commands |
|---|---|---|
| `settings` | 19-24 | `get_settings`, `save_settings`, `get_setting_cmd`, `save_setting_cmd` |
| `instances` | 25-62 | `get_instances`, `create_instance`, `delete_instance`, `install_instance`, `launch_game`, `open_instance_folder`, `open_instance_path`, `clear_instance_cache`, `get_instance_mods`, `toggle_mod`, `delete_mod`, `rename_instance`, `edit_instance`, `set_instance_icon`, `get_instance_icon`, `get_instance_resourcepacks`, `delete_resourcepack`, `create_instance_from_modpack` |
| `versions` | 63-75 | `get_minecraft_versions`, `get_loader_versions`, `get_fabric_versions`, `get_supported_loaders` |
| `auth` | 76-88 | `authenticate_microsoft`, `poll_microsoft_status`, `complete_microsoft_auth_cmd`, `authenticate_elyby`, `poll_elyby_auth`, `complete_elyby_auth_cmd`, `create_offline_account`, `ensure_authlib_injector` |
| `mods` | 89-101 | `search_modrinth`, `get_modrinth_project`, `get_modrinth_versions`, `install_mod`, `install_resourcepack`, `install_modpack` |
| `news` | 102-106 | `get_minecraft_news`, `get_minecraft_news_enhanced` |
| `partners` | 107-111 | `fetch_partners_remote`, `get_server_status` |
| `skin` | 112-115 | `get_skin_info`, `set_custom_skin` |
| `optimizer` | 116-120 | `get_optimizer_status`, `apply_optimizer` |
| `updates` | 121-128 | `check_updates`, `check_launcher_update`, `download_update`, `install_update` |
| `logs` | 129-134 | `get_logs`, `clear_logs`, `get_game_output`, `clear_game_output` |
| `system` | 135-138 | `get_system_info`, `get_public_ip` |
| `playTimes` | 139-141 | `get_play_times` |
| `version` | 142-144 | `get_latest_release_tag` |
| `background` | 145-152 | `save_background_image`, `get_background_image_path`, `delete_background_image`, `save_background_video`, `get_background_video_path`, `delete_background_video` |
| `discord` | 153-174 | `init_discord_rpc`, `get_discord_status`, `update_discord_rpc` (used by both `update` and `start`), `stop_discord_rpc` |
| `perfMods` | 175-182 | `install_performance_mods`, `install_xryloth_mod` |
| `gameBoost` | 183-188 | `game_boost` |
| `templates` | 189-193 | `get_instance_templates`, `apply_instance_template` |
| `configEditor` | 194-201 | `get_config_files`, `read_config_file`, `write_config_file` |
| top-level `openUrl` | 202 | `open_url` |
| `profiles` | 203-210 | `save_mod_profile`, `load_mod_profiles`, `delete_mod_profile`, `apply_mod_profile` |
| `modUpdates` | 211-213 | `check_mod_updates` |
| `crashLog` | 214-216 | `analyze_crash_log` |
| `dependencies` | 217-220 | `resolve_mod_dependencies` |
| `resourcePack` | 221-230 | `auto_enable_resourcepack`, `toggle_resourcepack`, `enable_resourcepack`, `disable_resourcepack` |
| `export` | 231-234 | `export_instance_mrpack` |
| `conflicts` | 235-237 | `detect_mod_conflicts` |
| `wizard` | 238-241 | `check_first_launch`, `complete_first_launch` |
| `java` | 242-251 | `check_java_compatibility`, `download_java`, `get_java_installations`, `set_java_for_instance` |
| `systemCmd` | 252-263 | `run_system_command`, `read_file_content`, `write_file_content`, `list_directory`, `delete_path` |
| `clone` | 264-267 | `clone_instance` |
| `servers` | 268-275 | `get_servers`, `add_server`, `delete_server`, `ping_server` |
| `selfhost` | 276-280 | `get_selfhost_status`, `download_selfhost`, `launch_selfhost` |
| `serverVersions` | 281-284 | `get_server_versions`, `get_server_builds` |
| `launchServer` | 285-287 | `launch_server` |
| `serverMgr` | 288-345 | **~60 commands** `server_mgr_*`: list/create/start/stop/kill/command/status/logs/delete/get_properties/set_properties/search_plugins/install_plugin/list_plugins/delete_plugin/plugin_toggle/server_types/list_versions/files_list/file_read/file_write/file_delete/file_mkdir/file_upload/backup_list/backup_create/backup_restore/backup_delete/list_worlds/delete_world/set_meta/eula/clone/stats/check_port/next_port/clear_logs/update_meta/list_java/open_folder/read_icon/file_rename/file_copy/file_read_b64/file_download/reset_world/reinstall |
| `ai` | 346-352 | `ai_chat`, `ai_context_get_app`, `ai_context_clear_app` (comment: "agentic assistant powered by Groq with a Gemini fallback") |
| `pin` | 353-356 | `pin_instance`, `unpin_instance` |
| `screenshots` | 357-359 | `get_screenshots` |
| `instanceSettings` | 360-368 | `get_instance_settings`, `save_instance_settings` |
| `exportConfig` | 369-372 | `export_instance_config` |
| `importConfig` | 373-376 | `import_instance_config` |
| `batch` | 377-380 | `batch_install_mods` |
| `update` | 381-384 | `check_launcher_update` (duplicate of `updates.checkLauncher`) |
| `gameLogs` | 385-387 | `get_game_logs` |
| `fileDrop` | 388-393 | `move_mod_file`, `move_resourcepack_file` |
| `uninstall` | 394-398 | `uninstall_launcher` |

Notes:
- `api.auth.microsoftPoll(device_code, interval)` ignores its args and calls `poll_microsoft_status` without them (`api.ts:79-80`) — `device_code` is unused.
- `api.discord.update` default large/small image keys are `chatgpt_image_oct_7_2026_06_05_12_pm` (`api.ts:160-162`).
- `api.updates.checkLauncher` and `api.update.checkLauncher` both map to `check_launcher_update` (`api.ts:123`, `:382-383`).

### 4.2 `lib/tauri.ts` (19 lines)
Exports `isTauri()` only. See §1.4. No commands, no network.

### 4.3 `lib/auth.ts` (190 lines) — Xryloth account system (Firebase email/password)
- Role: optional login; "Login is NEVER compulsory" (`auth.ts:1-2`).
- Exports: `ADMIN_EMAIL = "support@xrylothclient.com"` (`auth.ts:9`), `isAdminEmail()`, `XrylothSession` interface (`:34-42`), `getSession()`, `onSessionChanged(cb)`, `watchAuthState()`, `signup()`, `login()`, `resendVerification()`, `logout()`.
- Session stored in `localStorage["xryloth-account-session"]` (`:44`) and mirrored to a window CustomEvent **`xryloth-account-changed`** on every save (`:58`).
- `watchAuthState()` (`:76-101`) — Firebase `onAuthStateChanged` keeps local session in sync (clears on sign-out, refreshes admin flag, re-derives session for new user). Called once by App (`App.tsx:180`).
- `signup` (`:103-132`): `createUserWithEmailAndPassword` → `updateProfile(displayName)` → `sendEmailVerification` → writes `users/<uid>` = `{email, username, createdAt}` in RTDB; **admin email signs in immediately** (no verification), others get `signOut` + `needsVerification: true`.
- `login` (`:134-162`): sign in → reload → if unverified and not admin: resend verification, sign out, `needsVerification`; reads `users/<uid>.username` from RTDB as fallback username; saves session.
- `resendVerification` (`:164-174`), `logout` (`:176-179`).
- `friendlyAuthError` maps Firebase codes → human messages (`:181-190`).
- No Tauri commands. Long doc comment (`:11-29`) explains removal of a bundled local admin password (security fix).
- Network: Firebase Auth + RTDB (see §9).

### 4.4 `lib/firebase.ts` (27 lines) — Firebase bootstrap + re-exports
- Hardcoded config (`firebase.ts:8-16`):
  - `apiKey: "AIzaSyC7Jb2CuVT-sku8Yw-1AlnxPf0uqr5Hj74"`
  - `authDomain: "xryloth-mc.firebaseapp.com"`
  - `databaseURL: "https://xryloth-mc-default-rtdb.firebaseio.com"`
  - `projectId: "xryloth-mc"`
  - `storageBucket: "xryloth-mc.firebasestorage.app"`
  - `messagingSenderId: "526889634406"`
  - `appId: "1:526889634406:web:a398cc871b48d18c343ba7"`
- Initializes app, `getAuth`, `getDatabase`; re-exports auth + RTDB helpers and `User` type (`:18-27`). No Tauri commands.

### 4.5 `lib/chatService.ts` (437 lines) — Firebase chat/presence layer
- Types: `ChatUser` (`:10-20`), `ChatReaction` (`:22-25`), `ChatMessage` (`:27-45`, incl. `inviteData`, `readBy`, `reactions`, `targetUid`, `channel`), `ChatChannel` (`:47-57`), `PresenceStatus` (`:431`).
- Auth: `registerUser` (checks `usernames/<name>` uniqueness, creates auth + `users/<uid>` + `usernames/<name>` → uid) (`:60-95`), `loginUser` (`:97-111`), `logoutUser` (`:113-121`), `uploadProfileImage` (FileReader → 128×128 canvas JPEG 0.7 → base64 stored in RTDB, "no Storage needed") (`:123-159`).
- Messages: `sendMessage(text, type, inviteData?, mentions?, targetUid?, channel?)` writes to RTDB `messages` with **`serverTimestamp()`** (long comment `:175-183` explains why: limitToLast(100) window + clock skew) (`:162-193`); `extractMentions` (`:195-198`); subscriptions `subscribeToMessages` / `subscribeToMessagesByChannel` / `subscribeToDMs` — all `query(messages, orderByChild("timestamp"), limitToLast(100))` with client-side filtering (`:200-251`); `markMessagesAsRead` (`:253-269`); `addReaction` toggle (`:271-298`).
- Channels: `createChannel`, `joinChannel`, `subscribeToChannels` (RTDB `channels`) (`:300-352`).
- DMs/invites: `sendDM(targetUsername, text)` (`:354-361`), `sendInvite(targetUsername, address, port, mcVersion)` builds a type `"invite"` message (`:400-418`), `searchMessages` pure filter (`:363-366`).
- Presence: `subscribeToOnlineUsers` (RTDB `users`, online-first sort) (`:368-381`), `getUserByUsername` via `usernames/` index (`:383-398`), `setupPresence` (`:421-427`), `setUserStatus("online"|"away"|"dnd")` (`:432-437`).
- No Tauri commands. Pure Firebase RTDB.

### 4.6 `lib/themes.ts` (100 lines) — light/dark variant palettes
- Exports `ThemeVariant = "dark"|"light"`, `ThemeVariantDef`, `THEME_VARIANTS` with full 38-key `ThemeColors` for dark (near-black neutral palette, `themes.ts:12-55`) and light (slate + indigo accent, `:56-99`). Used as fallback by `resolveThemeColors` when no named theme matches.

### 4.7 `lib/updater.ts` (143 lines) — in-app updater state machine (module singleton, not zustand)
- Exports: `UpdateStage` (`"idle"|"available"|"downloading"|"applying"|"error"`), `UpdateState {stage, info, percent, downloaded, total, speed, logs, error}`, `getUpdateState()`, `subscribeUpdate(fn)`, `appendUpdateLog(msg)` (keeps last 400), `attachUpdateListeners()`, `checkForUpdates()`, `startUpdate()`, `dismissUpdate()`.
- `attachUpdateListeners()` (`:62-84`): idempotent; subscribes Tauri events **`update-progress`** (`{downloaded,total,percent,speed}`) and **`update-log`** (string).
- `checkForUpdates()` → `api.updates.check()` = command `check_updates` (`:86-101`).
- `startUpdate()` (`:103-139`): only from stage `available`; `api.updates.download(download_url, download_name)` → `download_update`; wait 800 ms → `api.updates.install(path)` → `install_update` (process exits on success).
- Invoked from App boot (`App.tsx:182-193`).

### 4.8 `lib/announcements.ts` (203 lines) — Firebase announcements + admin ping
- RTDB path **`announcements`** (`:42`); everyone reads, only admin writes (`:1-13` comment).
- Types: `AnnouncementType = "info"|"update"|"event"|"ping"`, `MediaType = "none"|"image"|"video"`, `Announcement` interface (`:16-40`) with `ticker`, `popup`, `targetUid`, `priority`, etc.
- Exports: `isAdmin()` (from session), `myUid()`, `appliesToMe(a)`, `postAnnouncement(a)` (admin-only, `push`+`set` with `createdBy/createdAt`), `deleteAnnouncement(id)`, `fetchAnnouncements()`, `sortAnnouncements` (newest first), `subscribeAnnouncements(cb)` (`onValue` realtime), `markSeen(ids)` / `hasSeen(id)` with `localStorage["xryloth-seen-announcements"]` bounded to 500 ids (`:139-162`), `playPingSound()` — Web Audio four-note chime 784/988/1319/1568 Hz + 130 Hz sub, no asset files (`:164-203`).
- No Tauri commands.

### 4.9 `lib/partners.ts` (262 lines) — partner servers (Firebase + Rust poll)
- RTDB path **`partnerServers`** (`:24`); `PartnerServer` interface (`:6-22`): name, ip, port?, description, version, players, logo/banner (data: or https), website, discord, webstore, tags, featured.
- `fetchPartners()` — direct Firebase read (`:32-41`).
- `fetchPartnersRemote()` — through Rust: `api.partners.fetchRemote()` → **`fetch_partners_remote`** (`:46-55`), "bypassing WebView/CORS network limitations".
- `fetchStatuses(host, port)` (`:69-135`): splits `host:port`, tries Rust `api.partners.status` → **`get_server_status`** when in Tauri; browser fallback hits **`https://api.mcsrvstat.us/3/<host>`** (and `:port` candidate) (`:103-107`).
- `subscribePartners(cb)` (`:187-219`): **desktop** path = shared 20 s poll of `fetch_partners_remote` across all subscribers (comment `:137-141` documents a prior bug: 4 subscribers × 4 s timer = 7300+ fetches/session); **browser** path = Firebase `onValue` realtime. Poll stops when last subscriber unsubscribes.
- Admin CRUD: `addPartner`, `updatePartner`, `removePartner` (RTDB write) (`:221-248`); `submitPartnerApplication` → RTDB **`partnerApplications`** with `status:"pending"` (`:250-262`).
- No direct network except the browser mcsrvstat fallback.

### 4.10 `lib/publicServers.ts` (126 lines) — public server registry for the website
- RTDB path **`publicServers`** (`:45`); `PublicServer` interface (`:10-43`).
- `serverKey(name)` = `<uid>_<slug>` stable key (`:48-52`).
- `publishServer(s)` upserts with owner/createdAt/updatedAt (`:54-76`); `unpublishServer(name)` (`:78-85`); `patchServer(name, patch)` best-effort (`:88-94`); `fetchPublicServers()` sorted by `updatedAt` desc (`:96-107`); `subscribePublicServers(cb)` realtime (`:109-126`).
- Doc comment (`:1-6`): private servers never written; the Xryloth website reads this node.
- No Tauri commands.

### 4.11 `lib/imageCache.ts` (115 lines) — Rust disk-cache client
- Role: every remote image resolved through Rust `~/.xryloth/image_cache` instead of webview fetch (doc `:5-19`).
- Exports: `isRemoteImage(src)` (http/https only), `getCachedImage(src)`, `warmImageCache(urls)`, `subscribeImageCache(cb)`.
- Tauri commands: **`image_cache_get`** (`{url}` → `{success,path}` then `convertFileSrc`) (`:80`), **`image_cache_warm`** (`{urls}` max 64) (`:105`).
- Tauri event: **`image-cache-updated`** (`{url,path}`) → live swap via `convertFileSrc` and fan-out to subscribers (`:44-63`).
- Session memo `resolved` map + `inflight` dedupe (`:33-35`); non-remote srcs pass through.

### 4.12 `lib/deviceTier.ts` (80 lines) — "potato mode"
- Exports `LOW_RAM_GATE_BYTES = 7 * 1024**3` (`:19`), `usePotatoMode()` hook.
- Gate: `systemInfo.ram_total < 7 GiB` (comment `:14-18` explains 8 GB machines report ~7.6-7.9 GiB so gate at 7). Cache key `localStorage["xryl.perf.lowend"]` (`:21`); synchronous hint from `navigator.deviceMemory < 8` (`:41-44`).
- Toggles `document.documentElement.classList "lowend"` (`:70-76`) so `index.css` can drop backdrop-blur; applied at module load before React mounts (`:78-80`).
- No Tauri commands (reads store's `systemInfo` which comes from `get_system_info`).

### 4.13 `lib/navGuard.ts` (18 lines) — see §2.8.

### 4.14 `lib/types.ts` (404 lines) — shared TS types + named themes
- `Settings` (`:4-77`): huge optional bag incl. `mc_token`, `ms_refresh_token`, `username`, `cracked_username`, `ely_*` fields, `theme`, `ram_mb`, `java_path`, `performance_mode`, `discord_rpc`, `auto_update`, `perf_*`, `launcher_bg_*` (type/image/video/opacity/blur/brightness/dimming/fit), `sidebar_*`, `accent_color`, `title_bar_style`, `default_page`, `game_resolution_*`, `[key: string]: any` escape hatch (`:76`).
- `Instance` (`:80-102`): id, name, version, loader, loader_version, created, last_played, icon, installed?, mods?, ram_mb?, jvm_args?, is_favorite?.
- `ModInfo`, `ModDependency`, `ModConflict`, `ModUpdate`, `ModSearchResult`, `ModrinthHit`, `LoaderVersion` (`:105-190`).
- `ServerStatus` (`:193-205`), `ServerEntry` (`:207-211`), `SystemInfo` (`:214-222`).
- `ThemeColors` (`:225-264`): 38 keys.
- **`THEMES`** (`:266-358`): named theme palettes — `orange` (display "Xryloth", accent `#e06b22`, "FastClient pure dark palette"), `ember` ("Ember Mark", `#ff5500`), `black` ("Black (Default)"), `fastclient` ("Fast Client", `#ff0022`), `lunar` ("Lunar", `#00e6a0`). Each a full `ThemeColors`.
- `ModProfile`, `CrashLogInfo`, `MrpackExportResult`, `UpdateInfo` (`:380-392`), `NewsItem` (`:394-400`).
- Ends with no-op `export {}` / `export type {}` (`:402-405`).

---

## 5. Page-by-page

All 9 files in `components/layout/` are documented in §2 (active shell: MainLayoutShell, SidebarRail, HeaderBar, ServerWidget, HeroStage; dead: MainLayout, Sidebar, TopBar, HeroSection) and are not repeated here. This section covers `components/pages/` (15 files), `components/mods/` (2), `components/instances/` (1).

> How this was verified: every file below was read directly (full read for files ≤ ~600 LOC; for the five large files — pages/InstancesPage, SettingsPage, ChatPage, ServersPage, XrylothAIPage — I read the imports/state/`useEffect` scaffolding, all JSX region comments, heading landmarks, and every `api.`/`invoke(` call site with line numbers). Reachability was verified with a full import graph: "NO IMPORTERS" = no file in `src/` imports it, and the route key (if any) is missing from `LazyPages.tsx`.

### 5.1 `components/pages/HomePage.tsx` (671 lines) — full dashboard (route key `home`)
- **Purpose**: the "home" dashboard with hero CTA, news, friends, partners and system stats. Note: the store's default page is `play` (§1.6) and `SidebarRail` never sets `home`, so this page renders only when something explicitly calls `setPage("home")` — the only two callers are the dead `ui/Sidebar.tsx:150` and `ui/TopBar.tsx:76`. In practice it also serves as the **fallback page** for unknown route keys (`LazyPages.tsx:49`).
- **UI regions**: HERO/MAIN ACTION with cave-atmosphere background (`:235-244`), white CTA + version picker (`:269`), 3D player skin preview (`:334`), quick profile slots (`:361`), news feed + utility rail (`:419`), friends presence (`:472`), footer (`:505`), right rail: social cards (`:519`), partnered servers with live status dots (`:535`), players online (`:577`), live system resources (`:604`), developer tools (`:642`).
- **Tauri commands**: `get_minecraft_news_enhanced` (`:134`, falls back to `get_minecraft_news` `:136`), `open_instance_folder` (`:196`).
- **External services**: `https://github.com/EscapeXMc/Xryloth/releases/latest` (`:18`, `RELEASES_URL`), `https://mc-heads.net/avatar/<uuid>?size=128` (`:25`) and `https://mc-heads.net/body/<uuid>/right` (`:29`), partners via `subscribePartners` + `fetchStatuses` (mcsrvstat.us through Rust, §4.9), friends via `subscribeToOnlineUsers` (Firebase, §4.5), session via `getSession`/`onSessionChanged` (§4.3).
- **Notable**: launch dispatches `ec-launch-game` with `{instanceName, version, loader}` (`:194`) → App boost overlay → `launch_game` (`App.tsx:245`). `openFolder` swallows errors (`:196`).

### 5.2 `components/pages/ProfilesPage.tsx` (241 lines) — profile card grid (route key `profiles`)
- **Purpose**: card grid of instances ("Your Profiles") with search, Play/Mods/Packs buttons and a per-card options menu.
- **UI regions**: header + search + "New Profile" button → `setPage("instances")` (`:66-91`); grass-block-styled profile cards (`:94-181`) with loader badge, Active chip, MC version + last-played row, 3-button action bar; empty state (`:182-190`); dashed Create Profile card → `setPage("instances")` (`:192-202`); `ProfileSettingsModal` (`:206-213`); `InstallOverlay` on version/loader change (`:216-238`).
- **Tauri commands**: none directly — all IPC happens inside `ProfileCardOptions` (§6), `ProfileSettingsModal`, `InstallOverlay` and the store's `loadInstances`.
- **Notable**: Play button dispatches `ec-launch-game` (`:148-153`); Mods/Packs buttons write `localStorage["ec-modbrowser-initial-tab"]` then `setPage("mods")` (`:59-62`) — consumed by `ModBrowserPage.tsx:53-58`. `editingProfile` always re-resolves from the store so renamed profiles refresh (`:50-52`). **Dead code**: `MOCK_PROFILES` (`:30-34`) is defined but never referenced.

### 5.3 `components/pages/InstancesPage.tsx` (1566 lines) — THE instance manager (route key `instances`)
- **Purpose**: the largest "profiles/instances" screen: create wizard, instance list, and a per-instance detail pane with tabs `info | mods | packs | modpacks | customize | profiles` (`DetailTab`, `:25`).
- **UI regions**: "New Profile" create panel (`:578`, version picker rendered **as cards** `:591-637` with artwork tiles `:613`, loader/settings under progressive-disclosure disclosure `:639+`, FastClient-style orange check badge `:791`, per-version quick actions `:851`); main "Instances" list heading (`:704`); detail pane; drag-and-drop import handlers (`:765-775`); mod-profiles manager (`:1412-1453`).
- **Tauri commands invoked** (file:line): `get_instances` `:129`, `get_play_times` (via `api.playTimes.get`) `:128`, `get_instance_icon` `:120`, `get_instance_mods` `:115, :479, :495`, `get_instance_resourcepacks` `:116`, `check_java_compatibility` `:134, :190, :1029`, `get_minecraft_versions` (via `api.versions.getMinecraft`) `:127`, `get_supported_loaders` `:172`, `get_settings` `:249`, `create_instance` `:219`, `install_performance_mods` `:255, :413`, `install_xryloth_mod` `:268`, `rename_instance` `:328`, `delete_instance` `:314`, `open_instance_folder` `:337, :876`, `export_config` (raw `invoke`) `:350`, `import_config` (raw `invoke`) `:363`, `toggle_mod` `:374`, `toggle_resourcepack` `:384`, `delete_mod` `:394`, `delete_resourcepack` `:402`, `set_instance_icon` `:434`, `search_modrinth` `:448, :458, :468`, `install_mod` `:478, :490`, `install_resourcepack` `:513`, `auto_enable_resourcepack` `:515`, `create_instance_from_modpack` `:529`, `move_mod_file` (raw `invoke`) `:770`, `move_resourcepack_file` (raw `invoke`) `:772`, `clone_instance` `:952`, `download_java` `:1028`, `check_mod_updates` `:1118`, `detect_mod_conflicts` `:1135`, `export_instance_mrpack` `:1152`, `load_mod_profiles` `:197, :1414, :1453`, `save_mod_profile` `:1412`, `apply_mod_profile` `:1442`, `delete_mod_profile` `:1452`.
- **Notable behaviors**: java compatibility is checked at three points (create `:134`, select `:190`, launch-time `:1029` with `download_java` offered); file drop moves `.jar`/pack files into the instance via Rust instead of webview uploads (`:770-772`); uses `alertDialog`/`promptDialog` from `ui/Dialog`.
- **Broken navigation**: buttons call `setPage("configeditor")` (`:1110`) and `setPage("packs")` (`:1318, :1522`) — **neither key exists in `LazyPages`**, so both silently fall back to HomePage (see §10).

### 5.4 `components/instances/InstancesPage.tsx` (245 lines) — ⚠ DEAD (NO IMPORTERS)
- Earlier, much simpler instance page. Nothing imports it (the `pages/InstancesPage` above is the one LazyPages loads).
- **Tauri commands**: `get_instances` `:28`, `create_instance` `:48`, `delete_instance` `:62`, `create_instance_from_modpack` `:75`.
- **Dead/unreachable UI**: delete-confirmation modal is rendered when `deleteConfirm` is truthy (`:203-214`) but no code ever sets it to an id; Import-Modpack modal gated on `importModalsOpen` (`:226-243`) which is never set `true`; the per-card Modpack/Mods/Packs buttons have no `onClick`; imports `ServerWidget` (`:9`) without using it.

### 5.5 `components/pages/SettingsPage.tsx` (1029 lines) — settings (route key `settings`)
- **Purpose**: 11-tab settings screen. Tabs (`:24-34`): general, appearance, background, java & game, downloads, integrations, directories, updates, tools, recovery, about.
- **UI regions**: title row (`:213`); background editor with URL input (`:357`) and Opacity/Brightness/Blur/Dark-Dimming/Fit-Mode sliders (`:458-535`) plus Reset (`:535`); RAM "recommended values" presets (`:607`); directory path chips with Open/Change (`:818`); Updates tab with Check/Update-now (`:134`, `:870`); Clear-data confirm (`:927`); unsaved-changes leave prompt (`:967`); uninstall confirmation with delete-data checkbox (`:990-1021`).
- **Tauri commands**: `get_java_installations` `:66, :621, :682, :899`, `get_background_image_path` `:71`, `get_background_video_path` `:83`, `save_background_image` `:159`, `save_background_video` `:180`, `uninstall_launcher` (via `api.uninstall.launcher`) `:143`, `delete_background_image` `:378, :393, :416, :563`, `delete_background_video` `:381, :396, :438, :564`, `download_java` `:679`, `run_system_command` `:838` (custom-folder scan), plus store-level `save_settings`/`save_one` on every edit. Updater: `check_updates`/`download_update`/`install_update` via `lib/updater` (`:4`, `:134`, `:870`).
- **Notable**: **debounced auto-save at 700 ms** (`:111-113`) — there is no Save button for most settings; registers a **nav guard** while dirty (`setNavGuard`, `:117-119`, cleared `:979`) so leaving settings prompts; external link `https://xrylothclient.vercel.app` (`:951`). `saveSettings` from the store does the actual IPC (§3.1).
- **Services**: none external besides updater (GitHub releases via Rust `check_updates`).

### 5.6 `components/pages/AccountsPage.tsx` (539 lines) — accounts & auth (route key `accounts`)
- **Purpose**: Minecraft account management (Microsoft device-code, ELY.by, offline/cracked) + optional Xryloth email login.
- **UI regions**: page header + active-account summary (`:228`); offline username input + signed-in/active account cards (`:334`); warning that a MS account without a Minecraft profile cannot launch (`:451`); supported-loaders row with `FabricLogo/ForgeLogo/OptifineLogo/QuiltLogo` (`:482`); monetisation off-ramp copy "works without an account" (`:508`); link to `https://www.minecraft.net/en-us/get-minecraft` (`:527`).
- **Tauri commands**: `authenticate_microsoft` `:104`, `poll_microsoft_status` `:113`, `complete_microsoft_auth_cmd` `:123`, `authenticate_elyby` `:176`, `poll_elyby_auth` `:178`, `create_offline_account` `:200`, `save_setting_cmd` `:222` (persists chosen account into settings).
- **External services**: Microsoft/ELY.by OAuth happen entirely in Rust (the frontend only polls); Xryloth account = Firebase email/password via `lib/auth` (`signup/login/logout/resendVerification`, imported `:21-23`); admin badge keyed to `ADMIN_EMAIL`.
- **Notable**: `skinFile`/`skinInfo` state exists (`:44-45`) but **no `api.skin.*` call site exists anywhere in `src/`** (§8) — skin commands are backend-only/unused by the UI.

### 5.7 `components/pages/ChatPage.tsx` (2217 lines) — community chat (route key `chat`, full-bleed)
- **Purpose**: Discord-style community chat on Firebase RTDB: channels, DMs, reactions, mentions, invites, presence.
- **UI regions**: roles & badges (`:105-141`), Minecraft emote/shortcode map (`:143`), top bar with status switcher Online/Away/DND (`:826-895`), view tabs (`:896`), search (`:930-993`), DM button (`:943`), channel/DM sidebar (`:1023-1126`), message feed + input bar (`:1127-1210`), MC emote quick-insert bar (`:1211-1268`), mention autocomplete dropdown (`:1269`), online-users sidebar (`:1339`), user profile mini-card (`:1392`), ping toast (`:1536`), invite modal (`:1581`), channel-creation modal (`:1715`), DM quick-open modal (`:1830`), `MessageBubble` with hover reaction picker (`:2016-2199`).
- **Tauri commands**: `play_notification_sound` `:325` and `show_notification` `:329` — both fired on **@mentions**, alongside a Web Audio four-tone ping (`playPingSound`, `:49-103`) and a web `Notification` fallback (`:337-344`).
- **External services**: Firebase Auth + RTDB through `lib/chatService` (all imports `:27-44`), `https://mc-heads.net/avatar/Steve/64` default head (`:48`). No other network.
- **Notable**: `ADMIN_EMAIL` badge logic (`:127`); reads/pings only for messages where `mentions` contains the local username (`:315-320`); subscribe functions are `limitToLast(100)` windows (§4.5).

### 5.8 `components/pages/XrylothAIPage.tsx` (989 lines) — AI assistant (route key `xai`, full-bleed)
- **Purpose**: agentic assistant (Groq primary, Gemini fallback per `api.ts:346` comment) with 5 tabs: AI Chat, Terminal, File Manager, Crash Analyzer, AI Settings (tab array `:446-452` region `:495+`).
- **UI regions**: header claiming "208 real tools" (`:461`); workspace selector binding the AI to a server/profile (`:498-561`); live streaming reply + tool-activity feed (`:562-625`); Terminal tab (`:626`); File Manager tab (`:695`); Crash Analyzer tab (`:794`); AI Settings tab with Groq key/model + Gemini key/model fields (`:892-960`); quick-prompt chips (create Paper server, install ViaVersion, whitelist, backup… `:427-436`).
- **Tauri commands**: `ai_chat` `:404`, `get_setting_cmd` `:199-202` (reads existing keys), `save_settings` `:434` (writes `ai_groq_key`, `ai_gemini_key`, `ai_groq_model`, `ai_gemini_model` — comment at `:431-433`: "these exact keys are what the Rust `ai_resolve_key` reads"), `list_directory` `:223`, `read_file_content` `:239`, `write_file_content` `:251`, `delete_path` `:257`, `run_system_command` `:280, :384, :871, :875`, `get_instances` `:193, :311`, `server_mgr_list` `:305`, `analyze_crash_log` `:818`.
- **Tauri events consumed**: `ai-chunk` (streamed tokens, `:325`), `ai-tool` (`:333`), `ai-status` (`:343`), `ai-action` (page navigation/actions, `:347`; App also listens, `App.tsx:168`). StrictMode-safe listener cleanup comment at `:318-320`.
- **External services**: none direct — API keys are stored in launcher settings and used by Rust. Doc links only: `https://console.groq.com/keys` (`:930`), `https://aistudio.google.com/app/apikey` (`:955`).

### 5.9 `components/pages/PartnersPage.tsx` (539 lines) — partner server directory (route key `partners`)
- **Purpose**: browse partnered MC servers; admin CRUD; apply/join links.
- **UI regions**: `StatusStrip` sub-component with status badge + version + latency (`:42`), player-count bar (`:66`), offline fallback (`:86`), MOTD (`:93`); header + admin-only Add Partner button (`:223-245`); guest sign-in prompt (`:246`); non-admin note (`:259`); full inline admin form — name/IP/port, description, version/players/tags, links, logo/banner upload as data-URLs, featured toggle (`:269-379`); filter bar + custom sort (`:395-435`); partner cards with banner, featured badge, tags, live StatusStrip, footer links, admin Edit/Delete (`:436-539`).
- **Tauri commands**: none directly — status pings go through `fetchStatuses` → Rust `get_server_status` (`lib/partners.ts:93`) with browser fallback to `https://api.mcsrvstat.us/3/<host>` (`lib/partners.ts:103-107`).
- **External services**: Firebase RTDB `partnerServers` (via `subscribePartners`, Rust `fetch_partners_remote` on desktop — §4.9), mcsrvstat.us, mc-heads not used here (logos are partner data: `data:` or `https://` URLs). Admin gate: `getSession()` from `lib/auth`.
- **Notable**: `fileToDataUrl` (`:25-31`) inlines logo/banner images as base64 data URLs (RTDB size limit).

### 5.10 `components/pages/ServersPage.tsx` (1944 lines) — Minecraft server manager (route key `servers`, full-bleed)
- **Purpose**: full local-server management UI (create/start/stop, console, players, properties, plugins, files, backups, worlds, publish-to-web).
- **UI regions**: local `LocalServer`/`Stats`/`FileEntry`/`BackupEntry`/`PluginEntry` types (`:29-60`); `PROP_FIELDS` — 34 editable server.properties keys (`:62-74`); 8 tabs `console|players|properties|plugins|files|backups|worlds|settings` (`TABS`, `:76-85`); a large `FEATURES` marketing/TODO-style string array (`:87+`); server list (`:1095`), detail pane (`:1135`) with breadcrumb + actions (`:1346`) and search/sort/view toggle (`:1366`); create modal incl. public/private listing + port availability check (`:1694-1746`); clone modal (`:1828`); change-version modal (`:1855`); image preview (`:1887`); in-app file editor (`:1901`); plugin compatibility warning (`:1921`).
- **Tauri commands** — essentially the whole `server_mgr_*` surface (see §8 for exact lines): list `:382` (and `:305` on AI page), stats `:393`, files_list `:399`, backup_list `:407`, list_worlds `:411`, list_plugins `:415`, get_properties `:419`, list_java `:427`, server_types `:426`, list_versions `:434, :1036`, check_port `:449`, logs `:509`, status `:511`, read_icon `:505, :695`, start `:597, :622`, stop `:605`, kill `:611, :620`, delete `:633`, command `:647`, set_properties `:681, :961`, create `:730`, update_meta `:736, :964`, search_plugins `:766`, install_plugin `:773`, plugin_toggle `:787`, delete_plugin `:793`, file_read_b64 `:804`, file_read `:809`, file_write `:818, :867`, file_delete `:828, :879`, file_rename `:837`, file_copy `:844`, file_download `:851`, file_mkdir `:860`, file_upload `:903, :1009`, backup_create `:920`, backup_restore `:931`, backup_delete `:939`, delete_world `:946`, reset_world `:1027`, reinstall `:1043`, eula `:1051`, clone `:1057`, open_folder `:1019`, clear_logs `:1193`, next_port `:1730`. Also `get_public_ip` `:691`.
- **Tauri events**: `server-mgr-output` (console lines, `:466`), `server-mgr-status` (`:474`), `server-mgr-download` (create progress, `:481`) — StrictMode-safe unlisten comment at `:460`.
- **External services**: Firebase RTDB `publicServers` for publish/unpublish (`publishServer`/`unpublishServer` imported `:26`, §4.10); plugin search hits Modrinth **through Rust** (`search_modrinth` is not called here — plugin search uses `server_mgr_search_plugins`); `isTauri()` guard imported `:6`.

### 5.11 `components/pages/LogsPage.tsx` (233 lines) — log viewer (route key `logs`)
- **Purpose**: three-tab log viewer: Launcher / Game Output / Live Stream.
- **UI regions**: header with Copy/Refresh/Clear (+Pause on live) (`:119-149`); tab bar (`:151-171`); live-instance picker chips (`:173-188`); filter input (`:190-197`); scrollable `GlassCard` with line numbers and severity coloring (`:199-230`, color rules `:108-115`).
- **Tauri commands**: `get_logs` + `get_game_output` polled every 3 s (`:63, :37`), `clear_logs` `:71`, `clear_game_output` `:74`, `get_game_logs` via raw `invoke` every 2 s while live tab is active and not paused (`:45, :41-51`). Clipboard: `@tauri-apps/api/clipboard` `writeText` with `document.execCommand` fallback (`:85-93`).
- **Notable**: auto-selects live instance from `settings.game_session_start` and switches to the live tab (`:26-33`); JetBrains Mono font stack requested inline (`:200`) — no such font is actually loaded (§7/§10).

### 5.12 `components/pages/ThemesPage.tsx` (610 lines) — theme editor (route key `themes`)
- **Purpose**: pick/edit color themes; create custom themes stored in localStorage.
- **UI regions**: live preview (`:166`, `:386`), custom-theme name + quick accent swatches (`:359-385`), color-group tabs (`:422`), individual color fields (`:440`), header (`:487`), active-theme indicator (`:512`), Custom Themes section (`:531`), Built-in Themes section (`:561`).
- **Tauri commands**: none directly — applying a theme goes through `setTheme` → `save_setting_cmd("theme", …)` in the store (§3.1). Custom themes live in `localStorage["ec-custom-themes"]` (`:10-17`), the same key the store reads (`stores/index.ts:15, :25-33`).
- **Notable**: pure client-side hex↔HSL helpers (`lighten/darken/hexToHsl`, `:19-45+`); uses `THEMES` (§4.14) and `THEME_VARIANTS` (§4.6).

### 5.13 `components/mods/ModBrowserPage.tsx` (287 lines) — Modrinth browser (route key `mods`)
- **Purpose**: live-search browser for mods / modpacks / resource packs from Modrinth, with a details modal and install buttons.
- **UI regions**: header (`:169`), 3-tab bar (`:193`), search that fires on every keystroke with debounce (`:214`, debounce `:110` region refs), results grid of `BrowserCard` (`:234`), error state (`:270`), item details modal (`:275` → `ModDetailsModal`).
- **Tauri commands**: `search_modrinth` `:80` (mods tab hardcodes `mcVersion "1.21.11"`, `:79`), `install_modpack` `:131`, `install_resourcepack` `:138`, `install_mod` `:145` — all require an `instanceName` (from `selectedInstance` store or prop) else sets an error (`:85`).
- **External services**: Modrinth only **through Rust** (`search_modrinth`); card art via `CachedImage` (Rust image cache, §4.11).
- **Notable**: reads `localStorage["ec-modbrowser-initial-tab"]` on mount to honor ProfilesPage deep-links (`:53-58`).

### 5.14 `components/pages/ModsBrowserPage.tsx` (55 lines) — ⚠ DEAD (NO IMPORTERS)
- A stub "Mod Browser" page whose 3 tabs all just call `setPage("mods")` (`:17`) with an explanatory empty card (`:45-52`). Not imported and has no `LazyPages` key. Uses `font-lemon` (`:24`), a class that does not exist (§7).

### 5.15 `components/pages/ModpacksBrowserPage.tsx` (452 lines) — ⚠ DEAD (NO IMPORTERS)
- Older standalone modpacks/resource-packs browser (props: `instanceName`). Regions: header (`:126`), category tabs (`:148`), live search (`:166`), results grid (`:186`), error (`:214`), pack details modal (`:219-450`).
- **Tauri commands**: `search_modrinth` `:58`, `install_modpack` `:100`. Superseded by `ModBrowserPage`'s modpacks tab.

### 5.16 `components/mods/ModrinthSearch.tsx` (48 lines) — ⚠ DEAD (NO IMPORTERS)
- Controlled search-input component only (props interface `ModrinthSearchProps`, `:6-11`; live per-keystroke `onSearchChange`, `:21-28`). No fetching, no install; nobody imports it.

### 5.17 `components/pages/AnnouncementsPage.tsx` (374 lines) — ⚠ DEAD (NO IMPORTERS / no route key)
- Admin announcement composer + feed on Firebase RTDB: regions admin composer (`:164`) with media picker (`:218`) and toggles (`:262`), feed (`:286`); `TYPE_META` for ping/update/event/info (`:24-29`); caps media at **3 MB** (`MAX_MEDIA_BYTES`, `:41-42`) to stay under RTDB's 10 MB node limit (comment). Uses `subscribeAnnouncements/postAnnouncement/deleteAnnouncement` (§4.8), `subscribeToOnlineUsers`, `useToast`, `confirmDialog`.
- **Route bug**: two call sites do `setPage("announcements")` (`layout/HeroStage.tsx:424`, `layout/HeroSection.tsx:291`) but `announcements` is **not in `LazyPages`**, so both render HomePage instead of this page (§10).

### 5.18 `components/pages/ConfigEditorPage.tsx` (307 lines) — ⚠ DEAD (NO IMPORTERS / no route key)
- Per-instance config-file editor: file icon/color mapping by extension (`:18-49`), properties syntax highlighting (`:50+`).
- **Tauri commands**: `get_config_files` `:83`, `read_config_file` `:107`, `write_config_file` `:127`.
- Reached-then-lost: `InstancesPage.tsx:1110` calls `setPage("configeditor")`, but with no `LazyPages` key the user lands on HomePage (§10).

---

## 6. UI component inventory (`src/components/ui/*`, 46 files)

One line each; "used by" comes from a full import graph of `src/` (files marked **⚠ dead** have zero importers). Live components:

- **AnnouncementHost** (37): renders the shared `useToast()` list as glass cards pinned bottom-center — used by `App.tsx`. ⚠ duplicates `ToastProvider`'s own top-right list (§10).
- **BrowserCard** (123): Modrinth result card (banner, icon, loaders, downloads) — `ModBrowserPage`, `ModpacksBrowserPage`.
- **CachedImage** (87): `<img>` that resolves through the Rust image cache (`useCachedImageUrl`, §4.11) — `HeroStage`, `ServerWidget`, `HomePage`, `ChatPage`, `AnnouncementsPage`, `pages/InstancesPage`, `ModpacksBrowserPage`, `PartnersPage`, `ServersPage`, `BrowserCard`, `HoverPreview`, `MarkdownRenderer`, `ModDetailsModal`.
- **Dialog** (213): promise-based `confirmDialog`/`promptDialog`/`alertDialog` replacing native dialogs + `DialogHost` renderer (module singleton, `Dialog.tsx:35-77`) — `App.tsx` plus `AnnouncementsPage`, `ChatPage`, `ConfigEditorPage`, `pages/InstancesPage`, `PartnersPage`, `ServersPage`, `SettingsPage`, `XrylothAIPage`, `ProfileCardOptions`, `ProfileSettingsModal`.
- **FabricLogo / ForgeLogo / OptifineLogo / QuiltLogo** (100/118/43/44): inline loader-logo SVGs — `AccountsPage`.
- **FormControls** (144): `GlassInput`, `GlassSelect`, `GlassToggle`, `GlassSlider`, accordion `Section` — `AccountsPage`, `AnnouncementsPage`, `ChatPage`, `pages/InstancesPage`, `LogsPage`, `PartnersPage`, `ServersPage`, `SettingsPage`, `ThemesPage`, `XrylothAIPage`, `ProfileSettingsModal`, `WizardOverlay` (also wraps `Select`).
- **GameBoostOverlay** (23): modal "Game Boost — launching *\<instance\>*" with an OK button that calls `onComplete` (App launches the game only after the user clicks OK) — `App.tsx`.
- **GlassCard** (33): standard blurred card (`hover`/`glow` variants) — used by nearly every page (all live pages except `ThemesPage`).
- **GlowButton** (71): primary/secondary/ghost/danger button with glow styles — used almost everywhere (every live page + HeaderBar/HeroStage/ServerWidget/overlays).
- **InstallOverlay** (240): corner install-progress card; listens to Tauri event **`install-progress`** (`:51`) and invokes `install_instance` (`:74`) — `pages/InstancesPage`, `ProfilesPage`.
- **InstallValidationModal** (437): pre-install version/dependency/conflict validation UI — `ModDetailsModal`.
- **LaunchAnimation** (17): fixed "Launching… *\<instance\>*" spinner dialog, auto-`onComplete` — `App.tsx`.
- **LaunchOverlay** (263): scripted launch ceremony — phases `shake → launch → done` on timers (1.8 s/4.5 s/5 s), 60 star particles + exhaust particles (`LaunchOverlay.tsx:7-30`) — `App.tsx`.
- **LoadingOverlay** (28): fullscreen dim + double spinner + `loadingMessage` from the store — `App.tsx` (via `LoadingGate`).
- **MarkdownRenderer** (160): `react-markdown` + `remark-gfm` + rehype sanitize/raw with `prose` styling — `ModBrowserPage`, `ModpacksBrowserPage`, `ModDetailsModal`.
- **ModDetailsModal** (543): Modrinth project detail: description markdown, versions list, install buttons. Commands: `get_modrinth_project` (`:87`), `get_modrinth_versions` (`:88`), `install_modpack` (`:507`), `install_resourcepack` (`:515`), `install_mod` (`:523`); embeds `InstallValidationModal`; uses `useToast` — `ModBrowserPage`.
- **MusicPlayer** (309): floating player generating C418-style ambient tracks with Web Audio (no audio files; tracks `Minecraft`, `Clark`, `Sweden`, …) — `App.tsx` (always mounted).
- **PartnerNotificationListener** (52): watches `subscribePartners`; after silently recording the first fetch, toasts whenever a new partner ID appears (`PartnerNotificationListener.tsx:16-46`) — `App.tsx`.
- **ProfileCardOptions** (478): the "…" context menu on profile cards (open folder `open_instance_path` `:155` / `open_instance_folder` `:156`, export config `export_instance_config` `:226`, export mrpack `:209`, clear cache `:247`, clone `:180`, delete `:270`, pin/unpin, JVM…); exports helper `toInstallTarget` (`:470`) — `ProfilesPage`, `ProfileSettingsModal`.
- **ProfileSettingsModal** (623): 2-tab (profile/java) instance editor. Commands: `get_minecraft_versions` `:87`, `get_instance_settings` `:88`, `get_java_installations` `:89`, `get_instance_icon` `:90`, `get_supported_loaders` `:110, :129`, `set_instance_icon` `:155`, `edit_instance` `:202`, `save_instance_settings` `:246, :278` — `ProfilesPage`.
- **Select** (168): custom styled dropdown — `AnnouncementsPage`, `ConfigEditorPage`, `pages/InstancesPage`, `ServersPage`, `SettingsPage`, `XrylothAIPage`, `FormControls`, `ProfileSettingsModal`.
- **SoundEffects** (113): Web Audio click/hover SFX (`useSounds`, no asset files) — `MusicPlayer`, plus the dead `ui/Sidebar`/`ui/TopBar`.
- **SplashScreen** (178): boot splash promised in §1.7 — phases `logo → loading → exit`; **potato mode shortens everything** (800→250 ms logo, 2800→900 ms loading, onComplete at 3200→1100 ms; `SplashScreen.tsx:15-24`) and drops the 40 particles (`:31-37`).
- **SteveModel** (215): three.js Steve rendered from a glTF **inlined as a base64 string** to sidestep Tauri's `asset://` sub-resource blocking (comment `SteveModel.tsx:1-7`) — `HeroStage`.
- **Toast** (114): `ToastProvider` context (`toast()` keeps last 5, `:31`) + top-right toast stack with progress bar (`:52-112`) — `App.tsx` + `ServerWidget`, `AnnouncementsPage`, `AnnouncementHost`, `ModDetailsModal`, `PartnerNotificationListener`, `ProfileCardOptions`, `ProfileSettingsModal`.
- **WizardOverlay** (613): first-run wizard, 6 steps `welcome → account → java → ram → theme → done` (`WizardOverlay.tsx:8-15`); commands `create_offline_account` (`:75`), `complete_first_launch` (`:88`) — `App.tsx` (shown when `check_first_launch` says first launch).

⚠ **Dead UI components** (zero importers, verified): **AnimatedBackgrounds** (247, rain/snow/fire/galaxy/matrix canvas backgrounds, `localStorage["ec-animated-bg"]`), **AnimatedLogo** (123, launcher logo mark — only referenced by the dead `ui/Sidebar`/`ui/TopBar`), **AnnouncementPopup** (169, full announcement card + a second `AnnouncementHost` export), **AnnouncementTicker** (89), **Breadcrumb** (62, breadcrumb header — read via exports only), **HoverPreview** (105, hover metadata card), **MinecraftParticles** (169, heart/xp/firework canvas particles + `useParticles`), **Modal** (53, generic framer-motion modal), **Sidebar** (ui, 339, older drawer with `AnimatedLogo`+`SoundEffects`+`Tooltip`), **ThemeSync** (34, writes the `--x-*` CSS variables — see §10, this is load-bearing but never mounted), **TitleBar** (132, older brand+account+window-controls bar), **Tooltip** (71, hover tooltip with shortcut chip — only used by dead `ui/Sidebar`/`ui/TopBar`), **TopBar** (ui, 186, older nav bar), **UpdateOverlay** (13, static "Checking for updates…" modal), **UserStatus** (251, status preset picker: online/gaming/away/dnd/invisible/offline).

---

## 7. Design system

### 7.1 `tailwind.config.js` (project root, 79 lines)
- `content: ["./index.html", "./src/**/*.{js,ts,jsx,tsx}"]` (`:3`); `plugins: []` (`:78`) — **no font plugin, no `tailwindcss-animate`, no `fontFamily` extension** (matters below).
- **Colors** (`:6-46`):
  - `fast.*` — "FastClient pure dark palette": `dark #090B0E`, `canvas #0D0F12`, `card #13161C`, `card2 #161922`, `border rgba(255,255,255,0.08)`, `borderSoft …0.04`, `text #ffffff`, `textSub #a1a1aa`, `textMuted #71717a`, **`accent #e06b22`**, `accent2 #c85108`, `accentText #ffffff`, `danger #ef4444`, `success #22c55e`, `warn #f59e0b` (`:8-24`).
  - `xryloth.*` — legacy brand scale explicitly **remapped onto the FastClient palette** so old `xryloth-*` utilities still render (`:25-40`): `500` = accent `#e06b22`, `800/900/950` = `card/canvas/dark`, `400` = secondary text.
  - `glass.50/100/200` = white alphas `0.03/0.05/0.08` (`:41-45`).
- **Other tokens**: `backdropBlur.xs 2px`, `md 12px` (`:47-50`); `boxShadow` `fast-glow`, `fast-card`, `fast-inset`, `xryloth-glow`, `xryloth-glow-hover` (all amber `rgba(224,107,34,…)`, `:51-57`); `borderRadius.xl 16px`, `2xl 24px` (`:58-61`); animations `fade-in` (0.3 s) and `slide-up` (0.4 s `cubic-bezier(0.16,1,0.3,1)`) with keyframes (`:62-75`).

### 7.2 `src/index.css` (575 lines) — global CSS, keyframe library, and the CSS-variable design system
- **Base** (`:1-17`): Tailwind directives; global reset; `html, body` font stack `"Inter", -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif`, background `#090B0E`, antialiasing. First scrollbar block at 6 px using `var(--x-trace)` (`:14-17`); `.tinycaps` = all-small-caps + lowercase + `0.06em` tracking (`:20-24`).
- **Keyframe library** (`:26-220`, ~50 animation groups): `orbFloat1-3` (`:27+`), `particleDrift`, `shimmer`, `pulseRing`, `glowPulse`, `textGlow`, entrance `fadeIn*/fadeIn/slideIn*` (`:78-89`), `bounce*`, `scale*`, `spin/spinSlow/wiggle/shake/shakeHard`, `gradientShift/gradientRotate/borderGlow`, `float*`, `ripple*`, `blink/typing`, `scanline/scanPulse`, `staggerFadeIn`, icon set `iconBounce/Spin/Pulse/Shake/Pop` (`:153`), button set `buttonGlow/Press/Shine` (`:160`), card set `cardHover/Glow/Border/Shine` (`:168`), sidebar `sidebarPulse/indicatorGlow` (`:177`), progress, `toastSlideIn/Out` (`:197-199`), toggle, `bgPan/Zoom/FadeIn`, Minecraft-specific `creeperPulse/diamondShine/redstoneGlow/goldShimmer/netherFire` (`:210`), `loadingDot1-3` (`:217`).
- **`.animate-*` utility classes** (`:222-270`) — hand-written, **shadowing several Tailwind defaults**: `.animate-bounce`/`.animate-pulse`/`.animate-spin` are redefined to the custom keyframes (`:223, :226, :228`), plus ~45 more (`animate-fade-in-up`, `animate-gradient`, `animate-scan`, `animate-creeper`…). `.stagger-1`…`.stagger-10` add `0.05s`-stepped `animation-delay` (`:272-282`).
- **Theme tokens `:root`** (`:284-316`): `--x-accent #e06b22`, `--x-accent2 #c85108`, `--x-accent-text`, `--x-text`, `--x-text-sub`, `--x-text-muted`, `--x-border`, `--x-border-soft`, `--x-surface`, `--x-surface-hover`, `--x-trace`, `--x-trace-strong`, then the "GLASSPHORIUM × CLAYMORPHIUM" set: `--x-glass`, `--x-glass-hi`, `--x-clay` (dual drop shadow), `--x-clay-hi` (inset rim), `--x-clay-btn`, `--x-clay-press`, `--x-clay-ember` (`:304-316`). The comment says these are "written by ThemeSync in App.tsx" — but `ThemeSync` is never mounted (§10), so every page runs on these hard-coded `:root` defaults.
- **`.glass-card`** (`:320-357`): translucent fill + `backdrop-filter blur(12px) saturate(1.6)`, clay shadow, `::before` 1 px lit top rim (`:331`), `::after` pointer-tracking radial sheen driven by `--mx/--my` shown on hover (`:341-351`), hover lifts `-2px` and borders go amber.
- **Component vocabulary** (`:359-455`): `.panel-dark` (monolithic dark panel, `:366`); clay family `.clay`/`.clay-btn` (press sinks `translateY(1px) scale(0.99)` instead of scaling away, `:380-389`)/`.clay-ember`; `.trace`/`.trace-strong` hairlines (`:391-392`); `.outline-btn` (fills with accent on hover, `:394-412`); `.ghost-btn` (`:414-424`); `.label-muted` (`:426`); `.pill-white` accent pill (`:429-435`); `.glow-border` masked gradient border on hover (`:437-453`); `.text-glow` (`:455`).
- **Controls/utilities**: second scrollbar block at **5 px** (`:457-461`, overrides the earlier 6 px rule), `::selection` (`:463`), custom `input[type=range]` track/thumb (`:465-479`), `.line-clamp-1/2` (`:481-483`), `.gradient-text` (uses `var(--accent)`/`var(--accent2)` which pages set **inline** per element, `:486-491`) and `.gradient-text-animated` (`:492`), `.shine-effect` diagonal sheen loop (`:501-527`), `.glow-hover` (`:531`), hover helpers `.hover-lift/.hover-glow/.hover-scale/.hover-scale-sm/.hover-bright/.hover-saturate` (`:540-551`).
- **Performance rules** (`:553-575`): all heavy Tailwind `backdrop-blur-*` utilities are clamped to `blur(12px) !important` because "webkit2GTK re-runs a backdrop blur every frame" (`:553-564`); `html.lowend *` disables `backdrop-filter` entirely (`:570-574`), applied by `lib/deviceTier.ts` (§4.12).
- **Fonts**: the only families referenced are the unloaded `"Inter"` stack (above) and ad-hoc inline `font-family` requests: JetBrains Mono/Fira Code/Cascadia Code in `LogsPage.tsx:200`, `monospace` in AI code blocks. No `@font-face`, no font CDN in `index.html`, and no font package in `package.json` — so **Inter/JetBrains Mono never load** and system fallbacks are what users see. `font-lemon`, `tracking-tinycaps`, and `animate-in fade-in-slide-up` appear in JSX (e.g. `ModsBrowserPage.tsx:24`, `AnnouncementHost.tsx:19`) but are **not defined anywhere** (no Tailwind `fontFamily`, no `tailwindcss-animate`) → no-ops.

---

## 8. Cross-reference: every Tauri command invoked from the frontend

Collected by scanning all 93 `src/**/*.{ts,tsx}` files for `api.<group>.<method>(` (mapped back to command strings via `lib/api.ts`) **and** for raw `invoke("…")` / `invoke<T>("…")` calls. Result: **147 distinct commands are invoked from the frontend**; `lib/api.ts` defines 172 (§8.1 lists the 35 that are never called).

Path legend: `pages/` = `components/pages/`, `ui/` = `components/ui/`, `layout/` = `components/layout/`, `mods/` = `components/mods/`, `instances/` = `components/instances/`; other paths are relative to `src/`.

### 8.1 Invoked commands

| Command | # | Call sites (`file:line`) |
|---|---|---|
| `server_mgr_backup_create` | 1 | pages/ServersPage.tsx:920 |
| `server_mgr_backup_delete` | 1 | pages/ServersPage.tsx:939 |
| `server_mgr_backup_list` | 1 | pages/ServersPage.tsx:407 |
| `server_mgr_backup_restore` | 1 | pages/ServersPage.tsx:931 |
| `server_mgr_check_port` | 1 | pages/ServersPage.tsx:449 |
| `server_mgr_clear_logs` | 1 | pages/ServersPage.tsx:1193 |
| `server_mgr_clone` | 1 | pages/ServersPage.tsx:1057 |
| `server_mgr_command` | 1 | pages/ServersPage.tsx:647 |
| `server_mgr_create` | 1 | pages/ServersPage.tsx:730 |
| `server_mgr_delete` | 1 | pages/ServersPage.tsx:633 |
| `server_mgr_delete_plugin` | 1 | pages/ServersPage.tsx:793 |
| `server_mgr_delete_world` | 1 | pages/ServersPage.tsx:946 |
| `server_mgr_eula` | 1 | pages/ServersPage.tsx:1051 |
| `server_mgr_file_copy` | 1 | pages/ServersPage.tsx:844 |
| `server_mgr_file_delete` | 2 | pages/ServersPage.tsx:828, :879 |
| `server_mgr_file_download` | 1 | pages/ServersPage.tsx:851 |
| `server_mgr_file_mkdir` | 1 | pages/ServersPage.tsx:860 |
| `server_mgr_file_read` | 1 | pages/ServersPage.tsx:809 |
| `server_mgr_file_read_b64` | 1 | pages/ServersPage.tsx:804 |
| `server_mgr_file_rename` | 1 | pages/ServersPage.tsx:837 |
| `server_mgr_file_upload` | 2 | pages/ServersPage.tsx:903, :1009 |
| `server_mgr_file_write` | 2 | pages/ServersPage.tsx:818, :867 |
| `server_mgr_files_list` | 1 | pages/ServersPage.tsx:399 |
| `server_mgr_get_properties` | 1 | pages/ServersPage.tsx:419 (via `api.serverMgr.getProperties`) |
| `server_mgr_install_plugin` | 1 | pages/ServersPage.tsx:773 |
| `server_mgr_kill` | 2 | pages/ServersPage.tsx:611, :620 |
| `server_mgr_list` | 2 | pages/ServersPage.tsx:382, pages/XrylothAIPage.tsx:305 |
| `server_mgr_list_java` | 1 | pages/ServersPage.tsx:427 |
| `server_mgr_list_plugins` | 1 | pages/ServersPage.tsx:415 |
| `server_mgr_list_versions` | 2 | pages/ServersPage.tsx:434, :1036 |
| `server_mgr_list_worlds` | 1 | pages/ServersPage.tsx:411 |
| `server_mgr_logs` | 1 | pages/ServersPage.tsx:509 |
| `server_mgr_next_port` | 1 | pages/ServersPage.tsx:1730 |
| `server_mgr_open_folder` | 1 | pages/ServersPage.tsx:1019 |
| `server_mgr_plugin_toggle` | 1 | pages/ServersPage.tsx:787 |
| `server_mgr_read_icon` | 2 | pages/ServersPage.tsx:505, :695 |
| `server_mgr_reinstall` | 1 | pages/ServersPage.tsx:1043 |
| `server_mgr_reset_world` | 1 | pages/ServersPage.tsx:1027 |
| `server_mgr_search_plugins` | 1 | pages/ServersPage.tsx:766 |
| `server_mgr_server_types` | 1 | pages/ServersPage.tsx:426 |
| `server_mgr_set_properties` | 2 | pages/ServersPage.tsx:681, :961 |
| `server_mgr_start` | 2 | pages/ServersPage.tsx:597, :622 |
| `server_mgr_stats` | 1 | pages/ServersPage.tsx:393 |
| `server_mgr_status` | 1 | pages/ServersPage.tsx:511 |
| `server_mgr_stop` | 1 | pages/ServersPage.tsx:605 |
| `server_mgr_update_meta` | 2 | pages/ServersPage.tsx:736, :964 |
| `add_server` | 1 | stores/serverStore.ts:43 |
| `ai_chat` | 1 | pages/XrylothAIPage.tsx:404 |
| `analyze_crash_log` | 1 | pages/XrylothAIPage.tsx:818 |
| `apply_mod_profile` | 1 | pages/InstancesPage.tsx:1442 |
| `authenticate_elyby` | 1 | pages/AccountsPage.tsx:176 (via `api.auth.elybyStart`) |
| `authenticate_microsoft` | 1 | pages/AccountsPage.tsx:104 (via `api.auth.microsoftStart`) |
| `auto_enable_resourcepack` | 1 | pages/InstancesPage.tsx:515 |
| `check_first_launch` | 1 | App.tsx:145 |
| `check_java_compatibility` | 3 | pages/InstancesPage.tsx:134, :190, :1029 |
| `check_mod_updates` | 1 | pages/InstancesPage.tsx:1118 |
| `check_updates` | 1 | lib/updater.ts:89 |
| `clear_game_output` | 1 | pages/LogsPage.tsx:74 |
| `clear_instance_cache` | 1 | ui/ProfileCardOptions.tsx:247 |
| `clear_logs` | 1 | pages/LogsPage.tsx:71 |
| `clone_instance` | 2 | pages/InstancesPage.tsx:952, ui/ProfileCardOptions.tsx:180 |
| `complete_first_launch` | 1 | ui/WizardOverlay.tsx:88 |
| `complete_microsoft_auth_cmd` | 1 | pages/AccountsPage.tsx:123 (via `api.auth.microsoftComplete`) |
| `create_instance` | 2 | instances/InstancesPage.tsx:48, pages/InstancesPage.tsx:219 |
| `create_instance_from_modpack` | 2 | instances/InstancesPage.tsx:75, pages/InstancesPage.tsx:529 |
| `create_offline_account` | 2 | pages/AccountsPage.tsx:200, ui/WizardOverlay.tsx:75 |
| `delete_background_image` | 4 | pages/SettingsPage.tsx:378, :393, :416, :563 |
| `delete_background_video` | 4 | pages/SettingsPage.tsx:381, :396, :438, :564 |
| `delete_instance` | 3 | instances/InstancesPage.tsx:62, pages/InstancesPage.tsx:314, ui/ProfileCardOptions.tsx:270 |
| `delete_mod` | 2 | pages/InstancesPage.tsx:394, stores/index.ts:316 |
| `delete_mod_profile` | 1 | pages/InstancesPage.tsx:1452 |
| `delete_path` | 1 | pages/XrylothAIPage.tsx:257 |
| `delete_resourcepack` | 1 | pages/InstancesPage.tsx:402 |
| `delete_server` | 1 | stores/serverStore.ts:52 |
| `detect_mod_conflicts` | 1 | pages/InstancesPage.tsx:1135 |
| `download_java` | 2 | pages/InstancesPage.tsx:1028, pages/SettingsPage.tsx:679 |
| `download_update` | 1 | lib/updater.ts:122 |
| `edit_instance` | 1 | ui/ProfileSettingsModal.tsx:202 |
| `export_config` | 1 | pages/InstancesPage.tsx:350 (raw `invoke`) |
| `export_instance_config` | 1 | ui/ProfileCardOptions.tsx:226 |
| `export_instance_mrpack` | 2 | pages/InstancesPage.tsx:1152, ui/ProfileCardOptions.tsx:209 |
| `fetch_partners_remote` | 1 | lib/partners.ts:49 |
| `get_background_image_path` | 1 | pages/SettingsPage.tsx:71 |
| `get_background_video_path` | 1 | pages/SettingsPage.tsx:83 |
| `get_config_files` | 1 | pages/ConfigEditorPage.tsx:83 |
| `get_game_logs` | 1 | pages/LogsPage.tsx:45 (raw `invoke`) |
| `get_game_output` | 1 | pages/LogsPage.tsx:63 |
| `get_instance_icon` | 2 | pages/InstancesPage.tsx:120, ui/ProfileSettingsModal.tsx:90 |
| `get_instance_mods` | 5 | pages/InstancesPage.tsx:115, :479, :495, stores/index.ts:298, :317 |
| `get_instance_resourcepacks` | 2 | pages/InstancesPage.tsx:116, :517 |
| `get_instance_settings` | 1 | ui/ProfileSettingsModal.tsx:88 |
| `get_instances` | 5 | instances/InstancesPage.tsx:28, pages/InstancesPage.tsx:129, pages/XrylothAIPage.tsx:193, :311, stores/index.ts:186 |
| `get_java_installations` | 5 | pages/SettingsPage.tsx:66, :621, :682, :899, ui/ProfileSettingsModal.tsx:89 |
| `get_latest_release_tag` | 1 | stores/index.ts:273 |
| `get_logs` | 2 | pages/LogsPage.tsx:63, stores/index.ts:244 |
| `get_minecraft_news` | 2 | pages/HomePage.tsx:136, layout/HeroStage.tsx:395 |
| `get_minecraft_news_enhanced` | 2 | pages/HomePage.tsx:134, layout/HeroStage.tsx:395 |
| `get_minecraft_versions` | 2 | pages/InstancesPage.tsx:127, ui/ProfileSettingsModal.tsx:87 |
| `get_modrinth_project` | 1 | ui/ModDetailsModal.tsx:87 |
| `get_modrinth_versions` | 1 | ui/ModDetailsModal.tsx:88 |
| `get_play_times` | 2 | pages/InstancesPage.tsx:128, stores/index.ts:256 |
| `get_public_ip` | 1 | pages/ServersPage.tsx:691 |
| `get_server_status` | 1 | lib/partners.ts:93 |
| `get_servers` | 2 | stores/index.ts:358, stores/serverStore.ts:62 |
| `get_setting_cmd` | 4 | pages/XrylothAIPage.tsx:199-202 |
| `get_settings` | 2 | pages/InstancesPage.tsx:249, stores/index.ts:137 |
| `get_supported_loaders` | 3 | pages/InstancesPage.tsx:172, ui/ProfileSettingsModal.tsx:110, :129 |
| `get_system_info` | 1 | stores/index.ts:266 |
| `image_cache_get` | 1 | lib/imageCache.ts:80 (raw `invoke`) |
| `image_cache_warm` | 1 | lib/imageCache.ts:105 (raw `invoke`) |
| `import_config` | 1 | pages/InstancesPage.tsx:363 (raw `invoke`) |
| `install_instance` | 1 | ui/InstallOverlay.tsx:74 |
| `install_mod` | 5 | pages/InstancesPage.tsx:478, :490, ui/ModDetailsModal.tsx:523, mods/ModBrowserPage.tsx:145, stores/index.ts:291 |
| `install_modpack` | 3 | pages/ModpacksBrowserPage.tsx:100, ui/ModDetailsModal.tsx:507, mods/ModBrowserPage.tsx:131 |
| `install_performance_mods` | 2 | pages/InstancesPage.tsx:255, :413 |
| `install_resourcepack` | 3 | pages/InstancesPage.tsx:513, ui/ModDetailsModal.tsx:515, mods/ModBrowserPage.tsx:138 |
| `install_update` | 1 | lib/updater.ts:128 |
| `install_xryloth_mod` | 1 | pages/InstancesPage.tsx:268 |
| `launch_game` | 1 | App.tsx:245 |
| `list_directory` | 1 | pages/XrylothAIPage.tsx:223 |
| `load_mod_profiles` | 3 | pages/InstancesPage.tsx:197, :1414, :1453 |
| `move_mod_file` | 1 | pages/InstancesPage.tsx:770 (raw `invoke`) |
| `move_resourcepack_file` | 1 | pages/InstancesPage.tsx:772 (raw `invoke`) |
| `open_instance_folder` | 5 | pages/InstancesPage.tsx:337, :876, pages/HomePage.tsx:196, layout/HeroSection.tsx:80, ui/ProfileCardOptions.tsx:156 |
| `open_instance_path` | 1 | ui/ProfileCardOptions.tsx:155 |
| `ping_server` | 2 | stores/index.ts:362, stores/serverStore.ts:66 |
| `play_notification_sound` | 1 | pages/ChatPage.tsx:325 (raw `invoke`) |
| `poll_elyby_auth` | 1 | pages/AccountsPage.tsx:178 (via `api.auth.elybyPoll`) |
| `poll_microsoft_status` | 1 | pages/AccountsPage.tsx:113 (via `api.auth.microsoftPollStatus`) |
| `read_config_file` | 1 | pages/ConfigEditorPage.tsx:107 |
| `read_file_content` | 1 | pages/XrylothAIPage.tsx:239 |
| `rename_instance` | 1 | pages/InstancesPage.tsx:328 |
| `run_system_command` | 5 | pages/XrylothAIPage.tsx:280, :384, :871, :875, pages/SettingsPage.tsx:838 |
| `save_background_image` | 1 | pages/SettingsPage.tsx:159 |
| `save_background_video` | 1 | pages/SettingsPage.tsx:180 |
| `save_instance_settings` | 2 | ui/ProfileSettingsModal.tsx:246, :278 |
| `save_mod_profile` | 1 | pages/InstancesPage.tsx:1412 |
| `save_setting_cmd` | 3 | pages/AccountsPage.tsx:222, stores/index.ts:169, :228 |
| `save_settings` | 2 | pages/XrylothAIPage.tsx:434, stores/index.ts:153 |
| `search_modrinth` | 5 | pages/InstancesPage.tsx:448, :458, :468, pages/ModpacksBrowserPage.tsx:58, mods/ModBrowserPage.tsx:80 |
| `set_instance_icon` | 2 | pages/InstancesPage.tsx:434, ui/ProfileSettingsModal.tsx:155 |
| `show_notification` | 1 | pages/ChatPage.tsx:329 (raw `invoke`) |
| `toggle_mod` | 2 | pages/InstancesPage.tsx:374, stores/index.ts:336 |
| `toggle_resourcepack` | 1 | pages/InstancesPage.tsx:384 |
| `uninstall_launcher` | 1 | pages/SettingsPage.tsx:143 (via `api.uninstall.launcher`) |
| `write_config_file` | 1 | pages/ConfigEditorPage.tsx:127 |
| `write_file_content` | 1 | pages/XrylothAIPage.tsx:251 |

Notes:
- Commands invoked only via raw `invoke()` (never through `api.*`): `export_config`, `import_config`, `move_mod_file`, `move_resourcepack_file`, `play_notification_sound`, `show_notification`, `image_cache_get`, `image_cache_warm`, `get_game_logs` (the last is also exposed as `api.gameLogs.get`, which is unused).
- Some call-site lines above contain several invocations collapsed into one row with `:a, :b` shorthand.

### 8.2 Defined in `lib/api.ts` but NEVER invoked from the frontend (35)

`ai_context_clear_app`, `ai_context_get_app`, `apply_instance_template`, `apply_optimizer`, `batch_install_mods`, `check_launcher_update` (both `api.updates.checkLauncher` and `api.update.checkLauncher`), `complete_elyby_auth_cmd`, `disable_resourcepack`, `download_selfhost`, `enable_resourcepack`, `ensure_authlib_injector`, `game_boost`, `get_discord_status`, `get_fabric_versions`, `get_instance_templates`, `get_loader_versions`, `get_optimizer_status`, `get_screenshots`, `get_selfhost_status`, `get_server_builds`, `get_server_versions`, `get_skin_info`, `import_instance_config`, `init_discord_rpc`, `launch_selfhost`, `launch_server`, `open_url`, `pin_instance`, `resolve_mod_dependencies`, `server_mgr_set_meta`, `set_custom_skin`, `set_java_for_instance`, `stop_discord_rpc`, `unpin_instance`, `update_discord_rpc`.

Notably: the Discord-RPC surface, optimizer, self-host, templates, screenshots, skin, launcher-update, `pin/unpin_instance`, and `game_boost` are wired on the Rust side but the UI never calls them (UI "pin" and "boost" flows don't exist / are done differently).

### 8.3 Tauri events consumed by the frontend

| Event | Listener(s) |
|---|---|
| `update-progress`, `update-log` | lib/updater.ts:66, :78 |
| `image-cache-updated` | lib/imageCache.ts:44 |
| `install-progress` | ui/InstallOverlay.tsx:51 |
| `server-mgr-output`, `server-mgr-status`, `server-mgr-download` | pages/ServersPage.tsx:466, :474, :481 |
| `ai-chunk`, `ai-tool`, `ai-status`, `ai-action` | pages/XrylothAIPage.tsx:325, :333, :343, :347; `ai-action` also at App.tsx:168 |

---

## 9. External network endpoints

Everything the webview itself contacts (full-URL grep across `src/**`); Modrinth, Mojang, Microsoft, GitHub, Groq/Gemini and mcsrvstat traffic normally happens in Rust — the frontend only sees the corresponding commands from §8.

| Endpoint / host | Used for | Where (frontend source) |
|---|---|---|
| Firebase project **`xryloth-mc`**: `firebaseapp.com` (auth), `https://xryloth-mc-default-rtdb.firebaseio.com` (RTDB), `xryloth-mc.firebasestorage.app` (config only), apiKey `AIzaSyC7Jb2CuVT-sku8Yw-1AlnxPf0uqr5Hj74`, appId `1:526889634406:web:a398cc871b48d18c343ba7`, mSenderId `526889634406` | Email/password auth, chat, presence, announcements, partners, public servers | `lib/firebase.ts:8-16` (only Firebase init; the SDK then talks to Google endpoints) |
| `https://xryloth-mc-default-rtdb.firebaseio.com` | direct databaseURL string | `lib/firebase.ts:11` |
| `https://api.mcsrvstat.us/3/<host>[:port]` | partner server status — **browser fallback only** (desktop uses Rust `get_server_status`) | `lib/partners.ts:103, :105, :107` |
| `https://api.mcsrvstat.us/icon/<address>` | server icon images | `App.tsx:113`, `layout/ServerWidget.tsx:56, :86` |
| `https://mc-heads.net/avatar/<uuid-or-hash>?size=64/128` | account/player avatars | `App.tsx:107`, `layout/HeroStage.tsx:19`, `layout/HeroSection.tsx:15`, `pages/HomePage.tsx:25`, `pages/ChatPage.tsx:48` (Steve default), `ui/TitleBar.tsx:7, :11` (dead) |
| `https://mc-heads.net/body/<uuid>/right` | full-body skin render | `pages/HomePage.tsx:29` |
| `https://github.com/EscapeXMc/Xryloth/releases/latest` | release notes / "download" links (actual version check is Rust `check_updates` / `get_latest_release_tag`) | `pages/HomePage.tsx:18`, `layout/HeroStage.tsx:373` |
| `https://console.groq.com/keys`, `https://aistudio.google.com/app/apikey` | documentation links on the AI settings tab (keys are stored in settings, used by Rust) | `pages/XrylothAIPage.tsx:930, :955` |
| `https://xrylothclient.vercel.app` | external link (website) in Settings → About | `pages/SettingsPage.tsx:951` |
| `https://www.minecraft.net/en-us/get-minecraft` | "buy Minecraft" link | `pages/AccountsPage.tsx:527` |
| `settings.discord_url` / `settings.webstore_url` / partner `website`/`discord`/`webstore` fields, `https://discord.gg/` fallback | opened via `@tauri-apps/api/shell` `open()` (or `window.open`) | `layout/ServerWidget.tsx:221-222`, `layout/HeroStage.tsx:379`, `layout/ServerWidget.tsx:208-219` |
| `https://example.com/modpack.mrpack` | placeholder text only | `instances/InstancesPage.tsx:234` |

**Not contacted by the frontend**: `api.modrinth.com`, Mojang/Microsoft auth & session servers, `launchermeta`, Groq/Gemini APIs — all reached only through Rust commands (`search_modrinth`, `authenticate_microsoft`, `ai_chat`, …). All other `https://…` hits in `src/` are SVG namespace URIs (`http://www.w3.org/2000/svg`) or `http://localhost` strings inside the browser-guard messages (`main.tsx:39`, `lib/tauri.ts:7`).

---

## 10. Gotchas, TODOs, duplicated/dead code

Verified during this pass; no `TODO`/`FIXME` markers exist in `src/` — the only suppressions are 3 × `@ts-ignore` (`lib/firebase.ts:1-5`) and 5 × `eslint-disable react-hooks/exhaustive-deps` (`ServerWidget.tsx:182`, `SteveModel.tsx:203`, `ProfileSettingsModal.tsx:122, :135`, plus the `sr-only` lint workaround at `ServerWidget.tsx:441-444`). Tech debt instead lives as dead code and broken navigation:

1. **~4,100 LOC of dead code (≈16% of `src/`)** — 26 files with zero importers: `pages/AnnouncementsPage` (374), `pages/ConfigEditorPage` (307), `pages/ModpacksBrowserPage` (452), `pages/ModsBrowserPage` (55), `instances/InstancesPage` (245), `mods/ModrinthSearch` (48); layout `MainLayout` (61), `Sidebar` (105), `TopBar` (106), `HeroSection` (339); ui `AnimatedBackgrounds` (247), `AnimatedLogo` (123), `AnnouncementPopup` (169), `AnnouncementTicker` (89), `Breadcrumb` (62), `HoverPreview` (105), `MinecraftParticles` (169), `Modal` (53), `Sidebar` (339), `ThemeSync` (34), `TitleBar` (132), `Tooltip` (71), `TopBar` (186), `UpdateOverlay` (13), `UserStatus` (251). All counted from `wc -l`; reachability from a full import graph.
2. **Route keys that don't exist in `LazyPages` silently render HomePage** (§1.6): `setPage("announcements")` (`layout/HeroStage.tsx:424`, `layout/HeroSection.tsx:291`) never reaches the existing AnnouncementsPage; `setPage("configeditor")` (`pages/InstancesPage.tsx:1110`) never reaches ConfigEditorPage; `setPage("packs")` (`pages/InstancesPage.tsx:1318, :1522`) and `setPage("modpacks")` (`ui/Sidebar.tsx:117`, dead file) go nowhere. Users land on the dashboard with no error.
3. **`ThemeSync` is never mounted, so themes don't reach the CSS design system**: `index.css:284-287` says the `--x-*` tokens "are written by ThemeSync in App.tsx", and `ThemeSync.tsx:17-27` is the only code that calls `style.setProperty("--x-…")` — but nothing imports it (§6). Every `.glass-card`, `.panel-dark`, `.outline-btn`, scrollbar, etc. therefore runs on the hard-coded FastClient-orange `:root` defaults forever, even under the Lunar/Ember/black themes.
4. **Every toast renders twice**: `App.tsx` mounts `ToastProvider`, which draws its own top-right toast stack (`Toast.tsx:52-112`), **and** `AnnouncementHost`, which maps the *same* `useToast().toasts` array to a second bottom-center stack (`ui/AnnouncementHost.tsx:6-11`).
5. **35 of the 172 `api.ts` commands are never called** (§8.2): entire subsystems are backend-only — Discord RPC (`init/get/update/stop_discord_rpc`), optimizer (`get/apply_optimizer`), self-host (`*_selfhost`), instance templates, screenshots, skin (`get_skin_info`/`set_custom_skin` — AccountsPage has state but no calls), `pin_instance`/`unpin_instance`, `game_boost`, `launch_server`, `server_mgr_set_meta`, `batch_install_mods`, `import_instance_config`, AI context helpers. Also `api.updates.checkLauncher` and `api.update.checkLauncher` duplicate `check_launcher_update` (§4.1).
6. **Unreachable modals & no-op buttons in `instances/InstancesPage.tsx`**: the delete-confirmation modal (`:203-214`) can never open (nothing sets `deleteConfirm`), the import modal (`:226-243`) can never open (nothing sets `importModalsOpen`), and the per-card Modpack/Mods/Packs buttons have no handlers (`:176-189`). File is dead anyway (§5.4).
7. **`ProfilesPage.tsx:30-34`** defines `MOCK_PROFILES` that nothing uses — leftover from before real instances were wired in.
8. **`stores/index.ts:388-399` `refreshServers()` discards ping data** — it maps every result to a hardcoded offline stub (`online:false, version:"", motd:""`), duplicating the correct logic already in `stores/serverStore.ts:59-101` (§3.1). `deleteInstance` in the store is client-side only — no backend delete (`stores/index.ts:212-216`).
9. **Fonts are requested but never loaded**: `index.css:7` asks for `"Inter"` and `LogsPage.tsx:200` for JetBrains Mono, yet there is no `@font-face`, no CDN link in `index.html`, and no font package in `package.json` — everything falls back to system fonts. Undefined utility classes also appear throughout: `font-lemon`, `tracking-tinycaps`, and `animate-in fade-in-slide-up` (needs `tailwindcss-animate`, not installed) — e.g. `ModsBrowserPage.tsx:24`, `AnnouncementHost.tsx:19`.
10. **Tauri API is v1, not v2**: `package.json` pins `@tauri-apps/api ^1.6.0`; imports use v1 paths (`@tauri-apps/api/tauri`, `appWindow` from `@tauri-apps/api/window`, `window.__TAURI_IPC__` guard). The document header (§1, line 4) says "Tauri v2 APIs" — that claim is wrong for this tree.
11. **`"use client"` directives in 34 files** — Next.js leftovers; harmless in Vite/React but misleading (e.g. every page starts with it).
12. **Hardcoded/stale literals**: dead `layout/Sidebar.tsx:92` shows **"v6.0.0.15"** while the store defaults `liveVersion`/`localVersion` to `"5.9"` (`stores/index.ts:66-71`); `mods/ModBrowserPage.tsx:79` hardcodes MC version `"1.21.11"` for mod search; `ServersPage.tsx:80-250` embeds a ~170-line `FEATURES` marketing array rendered as a checklist in the create modal (`:1679-1681`).
13. **Poll-interval comments drift**: `PartnerNotificationListener.tsx:10` says "Tauri polls every ~4s" but `lib/partners.ts:137-141` documents a shared **20 s** desktop poll; `ServerWidget` polls statuses every 60 s (§2.4). Similarly `App.tsx` warms partner images from first-run data while subscriptions may not have arrived yet.
14. **API facade oddities** (from §4.1): `api.auth.microsoftPoll(device_code, interval)` drops its arguments (`api.ts:79-80`); Discord RPC default asset keys are `chatgpt_image_oct_7_2026_06_05_12_pm` (`api.ts:160-162`) — a timestamped AI-generated image name leaking into config.
15. **Surprising UX flows**: `GameBoostOverlay` doesn't auto-continue — the launch waits until the user clicks **OK** (`ui/GameBoostOverlay.tsx:16`, `App.tsx:240-247`); `LaunchOverlay` is a fixed ~5 s scripted ceremony regardless of actual game state (`LaunchOverlay.tsx:7-14`); chat history is capped at the last 100 messages with client-side search only (§4.5).
16. **StrictMode double-mount handled ad hoc**: `XrylothAIPage.tsx:318-320` and `ServersPage.tsx:460` carry bespoke unlisten guards; other `listen()` sites (`InstallOverlay.tsx:51`, `lib/imageCache.ts:44`) do not.

---

# HANDOFF — 2026-11-23 — FastClient-style client redesign (READ FULLY BEFORE DOING ANYTHING)

> **Future agent:** This is the complete session handoff (supersedes older compacts for the current task).
> Read this fully. Do NOT redo completed work. Ask no questions; plain-language status (Hinglish OK) is fine.
> Pointer also added at the top of `CONTEXT-COMPACT.md`. Older truth-files: `CONTEXT-COMPACT.md`,
> `CONTEXT-handoff.md`, `TODO-KAL.md`, `TODOS-session-compact.md`, `PLAN-updater-fastclient-ui.md`.

---

## 0. FIRST-LINE DISCLOSURE (mandatory every session start)

- **claude-mem observer is DOWN since 2026-11-13T09:39:41Z** — error: `Claude Code could not authenticate the observer`.
  Tell the user this at the very start of your first reply, quote the error, say fix = `/login` or `claude auth login`.
  **Do NOT restart the worker; do NOT suggest restarting.** Restarting won't help.

---

## 1. ACTIVE TASK — client redesign in FastClient's exact way

### User's exact words
> *"bruh asa karo ke tum isss folde ke andne rjao /home/raghuwinder/Pictures/Screenshots/ and har eak iamge ko dhayan se dekho ui and ux ka stucter find akro and hamre liek ko bhi exact that same way me redesign karo!"*

Meaning: live inside `/home/raghuwinder/Pictures/Screenshots/`, study EVERY image carefully, extract the
UI/UX structure, and **redesign our Tauri client in exactly that same structure**.

### Constraints that still apply during this redesign
1. **PALETTE MANDATE (user verbatim, highest priority):** *"IN OUR CLINT AND WEBSITE DONT USE ANY WHERE ANY COLOR only use these 'White' 'Black' only these two colors! and one mint color only 10% AND CONTINUE"*
   → Structure/layout/interaction = FastClient's; **colors = white + black + ONE mint `#34D399` (≤10%)**.
   `#34D399` equals `--ok` in `src/styles.css:24`. `--color-orange-400` in site chunk CSS already remapped
   to `#34d399`. FastClient's orange accent → our mint. NO blue, NO red, NO green, NO purple anywhere.
   (Current `Sidebar.tsx` has `bg-blue-500` active indicator — VIOLATION, fix in redesign.)
2. Site suite must stay **184/184** (`node /tmp/opencode/verify_nav.mjs https://isolateclient.in` after deploy;
   local run without URL). Client edits don't touch `site/` — but never break site pages.
3. Deploy only after explicit user OK (see §5).

### Screenshot folder state (DONE — do not redo)
- 60 images, all **reviewed one-by-one this session** (list captured via python listing).
- **#1–46 (15-51-42 … 16-01-34, plus Gmail login shots) = FastClient v4.0.17 launcher — the reference.**
- **#47–60 (16-43-16 … 20-27-37) = UNRELATED** — Modrinth FluxCord rejection roast, Discord/Veltrion
  hosting purchase, BigRock DNS for isolateclient.in (user personally added `play` CNAME →
  `hustle.veltrionhost.com` and SRV `_minecraft._tcp.play` → `play.isolateclient.in`). IGNORE these.

### FastClient UI/UX structure extracted (the blueprint to replicate)
**Shell / layout**
- Left **icon sidebar (~90px)**: vertical items = icon ABOVE tiny uppercase label: PLAY, PROFILES,
  ACCOUNTS, PARTNERS, STORE, SETTINGS. Active item = filled rounded card. Version string bottom-left (`v4.0.17`).
- Top-right cluster: **two pills** — account pill (e.g. "FASTCLIENT / Sign In" or green-check email) and
  **MINECRAFT account pill** (avatar + name + chevron → dropdown), then native window controls
  (minimize / maximize / close).
- Brand: logo top-left + "FASTCLIENT" wordmark.
- Dark navy-black bg with faint organic/rock texture backdrop; cards = slightly lighter panels, 1px subtle
  borders, ~12px radius.

**Home / PLAY page**
- Full-bleed hero art (Minecraft scene) with **3D character render centered**, nameplate above character.
- Big **LAUNCH GAME** CTA (accent color) with version subtitle + chevron; **profile quick-switch strip**
  below CTA (small cards: icon, name, version, play ▶).
- Right column: Discord "JOIN" card + **Partner Servers list** (icon, name, domain, green online count).
- Below fold: **LATEST NEWS** card grid + **FRIENDS** panel ("View all", empty-state text).

**PROFILES page**
- Title "Your Profiles"; **search bar + sort dropdown ("Version")** in one toolbar row.
- Grid of profile cards: grass-texture header, icon, name, subtitle ("FastClient + Fabric 1.21.11"),
  action row: **Play / Mods / Resource Packs / ⋯**; selected card = accent border + check badge;
  **CREATE PROFILE** dashed card with + .
- Profile settings screen: sections **GENERAL / LAUNCH / ACTIONS** — each row = icon + title + description +
  right control (text input, memory slider "3.0 GB", dropdown, toggle, action buttons); footer **Cancel / Save Changes**.

**Mods browser (inside a profile)**
- Header: profile icon + name + version; **tab pills: Mods · Modpacks · Resource Packs · Shaders · Data Packs**; ✕ close.
- Sub-toolbar: **Browse | Installed** segmented; **Modrinth | CurseForge** source toggle; search box; refresh icon.
- 3-column grid of items: icon, name, author, download count, green "Installed" check badge.
- Click item → **right detail drawer**: icon/name/author, description, tag chips, "View on Modrinth" link,
  bottom **Install** button with progress ("Preparing entityculling 0%").
- Modpacks tab has an info banner: installing converts the profile.

**ACCOUNTS page**
- Toolbar: search + sort ("Date added"). Account card: avatar, name, "Local · Added 10/26/2026",
  warning chip "Account not linked" → opens **Link account modal** (accent CTA), **Skins** button, ⋯ menu
  (Refresh ownership / Clear cache / Release claim / Delete-red). **ADD ACCOUNT** dashed card.
- Add Account screen: left = two big type cards (**Microsoft Account / Local Account**, check badge);
  right panel = contextual form ("Buy Minecraft" upsell + **Sign in with Microsoft** CTA, or
  Random username + input + **Add Local Account**).
- **Skins screen**: left = **3D skin preview, drag-to-rotate** + ← → arrows; right = SKIN SLOTS panel
  (slot card: Upload / Model (Classic·Slim dropdown) / ⋯ (Change name / Export / Delete)), ADD SKIN card,
  bottom **Apply Skin**.
- **App-level account flow (already shipped by FastClient):** email modal → "Continue with email" →
  **verification code modal** (code like `L2H4-CBDG-UNXB-TLK8`, comes via email, 10-min single-use) →
  signed-in pill; account modal shows **2FA (Manage)**, "Manage your FastClient account", **Sign Out**.

**Header account dropdown**
- YOUR ACCOUNTS list (active check), **Manage accounts**, **Add an account**, **Sign Out**,
  then **LANGUAGE grid** with flag buttons: EN ES FR DE PT NL HI RU PL.

**PARTNERS page**
- Toolbar: search + "Become a Partner" button + sort ("Recommended"); 3-col grid of **server banner cards**
  (big thumbnail, small icon, name, "1256 online").

**SETTINGS page** (section pattern)
- **GENERAL**: toggles (Start when I sign in, Automatically Hide Launcher, Hide 3D Character, Discord Rich Presence).
- **PERFORMANCE**: Default Memory Allocation slider + GB readout; collapsible "Advanced Java Options";
  Simultaneous Downloads slider + number.
- **GAME WINDOW**: Fullscreen toggle, Window Width/Height number inputs.
- **DIRECTORIES**: rows (Game Data / Install / Configuration / Java / Logs) with path + **Open/Change** buttons.
- **TOOLS**: Console → Open Console. **RECOVERY**: Send Diagnostic Report, Test Java Launch,
  **Clear Data** (red destructive button).

**STORE**: sidebar tooltip "Coming Soon" + simple modal "Store / Coming Soon / Got it".

**Modals/menus/toasts conventions**: centered card with title + ✕, primary accent button, secondary text
link ("Later"); dropdown menus = dark rounded list w/ icons, destructive item red; system-style toast
top-center "Screenshot captured".

### Our client (IsolateClient) — current architecture
- **Stack:** Tauri 1.x + React 18 + TS + Vite; zustand `src/stores/app.ts`; Tailwind classes +
  `src/styles.css` (2927 lines). Binary `isolate-client`. Dev: `npm run tauri dev` (Vite :1420 → cargo).
- **Layout (`src/App.tsx` L246–317):** `.app` > `.ambient` + `<Sidebar/>` + `.center` (notice banner,
  `<HeaderBar/>`, `.main` page switch) + `<RightHub/>` (home only) + `<WindowControls/>` + `<ResizeHandles/>`
  + install progress bar + toasts + `<AnnouncementBanner/>` + `<UpdateModal/>`.
- **`Sidebar.tsx` (124 L):** `w-24`, brand logo + name, `NAV_ITEMS` = home/instances/mods/servermgr/
  partnerships/discord/accounts/ai/logs/settings (+admin via `fbIsAdmin`), card buttons w/ lucide icons,
  active = white/10 card + **`bg-blue-500` left bar (→ must become mint)**, foot = version/Live dot.
  Exported `NAV_ITEMS` used elsewhere.
- **`HeaderBar.tsx` (82 L):** `ISOLATE` brand + page tab label + spacer + account-pill (dot + username →
  setPage accounts). `WindowControls` exported separately (— □ ✕ buttons w/ tauri window API).
- **`Home.tsx` (313 L):** `.vp` viewport (bg `vp-art.jpg`, grid, vignette), profile badge, status chip,
  greeting (`greet()` + player name + stats), `launch-cta` + caret menu (profile switch, Manage mods,
  Manage profiles), `prof-bar` profile strip, news grid via `api.getNewsEnhanced()` (3 items).
- **Other components + LOC:** Accounts 530, Admin 1041, AiChat 253, AnnouncementBanner 75, DiscordChat 481,
  Instances 490, Logs 82, Mods 416, Partnerships 546, ResizeHandles 129, RightHub 243, ServerManager 993,
  Settings 893, UpdateModal 294, styles.css 2927. **Total ~10.2k lines.**
- **`src/lib/`:** tauri.ts (api wrappers + onEvent), actions.ts (install/play/create), firebase.ts
  (telemetry + fbIsAdmin), types.ts, telemetry.ts.
- **Backend:** `src-tauri/src/lib.rs` (~13k LOC) — all commands; handler registry ~L12107.
  App data `~/.isolate/`. Splash removed earlier; updater plan in `PLAN-updater-fastclient-ui.md`.
- **Pages switch:** `App.tsx` L260–270 maps `page` → component. Store: `useApp` (page, installing,
  notify, booted, banner, ban, config, settings, instances…).

### Screenshot tooling that WORKS (verified this session)
- The Isolate window is **Wayland-native → xdotool/gnome-screenshot can't target it** (xdotool listed only
  Minecraft/X11 windows). **Use Playwright against Vite instead.**
- Vite dev serves the full UI at `http://localhost:1420/` (tauri dev's BeforeDevCommand, or start
  `npm run dev`). Page errors `window.__TAURI_IPC__ is not a function` are **expected/ignorable** in browser.
- Script created + ran green: **`/tmp/opencode/shots_ours.mjs`** → captures
  `/tmp/opencode/ours_{home,instances,mods,accounts,partnerships,settings}.png` at 1280×1024
  (clicks sidebar by `button[title="…"]`). **Screenshots exist but NOT yet viewed/compared — next step.**
- Playwright location: `/home/raghuwinder/claude-animation-skills/kit/node_modules/playwright` (import
  `.../index.mjs`). Force IPv4 (`curl -4`). `rg` unavailable → use `grep`.

### NEXT STEPS (in order)
1. View `/tmp/opencode/ours_*.png` (6 shots) against FastClient reference screenshots.
2. Redesign the shell first: sidebar (icon-over-label items, mint active state, version bottom-left),
   top-right pill cluster (app-account pill + Minecraft account pill + window controls merged into header),
   brand placement — then Home hero (character-centered art + LAUNCH CTA + profile strip), then
   Profiles grid w/ toolbar + card action rows, then Mods browser (tabs + source toggle + detail drawer),
   then Accounts (card + add flow + skins), then Settings section rows.
3. Kill every non-white/black/mint color (blue-500 bar, emerald dots, danger reds → mint/white/gray only;
   ≤10% mint coverage).
4. Verify: `cd "/mnt/Data Drive/IsolateClient" && npm run build` (tsc + vite must be clean) +
   re-run `node /tmp/opencode/shots_ours.mjs` for visual check + local site suite 184/184 if `site/` touched.
5. Commit as `client: …` (see §6) — **deploy/commit only after user OK if site files involved**; client-only
   commits are fine once user approves the redesign visually.

---

## 2. PALETTE / DESIGN DECISIONS (locked)
- Mint accent = **`#34D399`** (== `--ok`, `src/styles.css:24`). ≤10% coverage. Only white/black otherwise.
- Recolor policy (site images): chroma>12 px → per-luminance gray; mint pixels preserved; brand marks mint.
- Shield logo = favicon/hero/cursor/app icons. Cursors: site `cursors/pointer.svg` (white shield),
  `pointer-hand.svg` (mint shield — verify it was remapped from orange); client `public/cursors/` same.
- `data-ready=false` bg bug fixed via `.magmad-background img { opacity: 1 !important; }` in
  `site/public/assets/iso-override.css` (injected by `page-fx.js` after `_next` chunk CSS).
- Hero background image must be a **background layer** behind content (user's explicit demand).
- splash.png fully removed from body + RSC payload in all 5 HTMLs (was causing hydration-mismatch
  intermittent splash reappearances).
- Mobile CTA clip fixed: `@media (max-width: 420px)` in iso-override.css (padding 12px, font 12px, ls .06em).
- 12 Web Design Rules audit on homepage: **all 12 pass** (319 words, mint 5.46%, 6 subtle animations,
  no dead links, sr-only h1 pattern, 360px no h-overflow).

---

## 3. COMPLETED THIS SESSION (do not redo)
- Reviewed all 60 screenshots individually; catalogued FastClient structure (§1).
- Confirmed unrelated shots #47–60 (Modrinth/Discord/BigRock DNS).
- `npm run tauri dev` finally GREEN after two failures: (a) icon RGBA panic — fixed by converting
  `src-tauri/icons/{32x32,128x128,128x128@2x,icon}.png` RGB→RGBA; (b) port 1420 conflict — killed stale
  vite. App launched `[Isolate] Ready.` — background shell **`sh_207919c8b0012EE6VarRhYdZ5b`**, app PID **21117**
  (`src-tauri/target/debug/isolate-client`), Vite :1420 alive (curl 200).
- Created + ran `/tmp/opencode/shots_ours.mjs` → 6 client screenshots in `/tmp/opencode/ours_*.png`.
- Prior session (already done): site 12-rules pass, splash removal (5 HTMLs), iso-override.css bg opacity +
  mobile CTA + dead rule removal, icon RGBA, local suite 184/184, paren/brace delta 0, pageerrors [].

---

## 4. GIT / WORKTREE STATE
- Main at **`4b8d960` pushed**. **UNCOMMITTED (awaiting user deploy OK):** 5 HTMLs
  (`site/public/index.html` + `index-{de,es,fr,nl}.html`), `site/public/assets/iso-override.css`,
  4 `src-tauri/icons/*.png`.
- **Untracked — NEVER commit:** `.agents/`, `skills-lock.json`.
- Commit author (exact): `EscapeXMc <escapex112233@users.noreply.github.com>`; prefixes **`site: …`** /
  **`client: …`**.

---

## 5. DEPLOY PROCEDURE (site; only after explicit user OK)
1. `git add` only intended files → commit with prefix → `git push`.
2. `cd site && npx vercel --prod --yes` (add `--token "$(python3 -c "import json;print(json.load(open('/home/raghuwinder/.local/share/com.vercel.cli/auth.json'))['token'])")"` only if "Not authorized").
   Recent deploys succeeded unauthenticated; authed as `escapex112233-6657`.
3. Live suite: `node /tmp/opencode/verify_nav.mjs https://isolateclient.in` → must be **184/184**.
4. Last good deploy: `isolateclient-a7j4k2gxm` aliased `https://isolateclient.in` (deploy #2 of the batch).

---

## 6. VERIFICATION / SCRIPTS (`/tmp/opencode/`)
- `verify_nav.mjs [baseUrl]` — **the gate**: 184 checks (desktop 79 + mobile 105). Restored post tmp-wipe.
- `shots_ours.mjs` — client UI screenshots via Vite (§1). Others: `shots12.mjs`, `btn_measure.mjs`,
  `final_check.mjs`, `timeline.mjs`, `test_famb.mjs`, `live_team.mjs`, `verify_footer.mjs`, `gsc_submit.py`.
- Client build gate: `npm run build` (tsc + vite). Rust: `npm run tauri dev` / cargo at `/home/raghuwinder/.cargo/bin`.

---

## 7. HOLDS / BLOCKED (respect)
- **Razorpay removal: user said "actually wait do not do it!" — DO NOT START until they say so** (`TODO-KAL.md` item 0).
- **IsolateClientMod auto-replace: PAUSED by user (2026-11-15)** — approved earlier but do NOT build now.
- Deploy of current uncommitted batch: waits for user OK.
- GSC sitemap "Couldn't fetch" → recheck 24h from 21 Nov; API path needs SA Owner on
  `sc-domain:isolateclient.in` then `python3 /tmp/opencode/gsc_submit.py`
  (SA key: `/home/raghuwinder/Downloads/openphone-bfd7c-firebase-adminsdk-fbsvc-12573e7b6b.json`,
  project `openphone-bfd7c` / `824505628703`).
- VM stuck in WinRE (BCD damaged) → blocks installer e2e + 1.8.9 `install.log` root cause.
- `FIREBASE_DB_URL` / `FIREBASE_AUTH` unset; no initial `releases/latest` (Firebase updater pending —
  plan in `PLAN-updater-fastclient-ui.md`).
- No real Adsterra keys.
- Older todos still open: Ely.by full login info in Firebase + e2e; accounts page align + Isolate login
  form; live verify in tauri dev (accounts/admin/discord/partnerships) + HANDOFF/TODOS update.

---

## 8. FROZEN SITE INVARIANTS (site files — do not change)
- Credit: "An isolated client by Shayan & Raghuwinder, built for performance".
- Discord `https://discord.gg/TV2ypJF6TF`. Support mail **`support@isolateclient.in`** exact.
- Nav: HOME · STORE · FEATURES · FAQ · TEAM · PLANSS · DOWNLOAD. Footer-only: Contact/Track/Uninstall/
  Privacy/Terms. Tap targets ≥44px. No h-overflow at 360/1440.
- Benchmark numbers: 1650 / 1180 / 1020 FPS. Video `youtu.be/T-8f3oEy_hE`. Showcase
  `https://i.imgur.com/UyQ4YZQ.png`. IG/YT → @oyeraghuwinder. `/team` mobile hamburger ≤760px.
- MD plan files are read-first, keep them updated: `CONTEXT-COMPACT.md`, `TODO-KAL.md`.

---

## 9. ENVIRONMENT QUIRKS
- Working dir: `/mnt/Data Drive/IsolateClient` (space in path — quote always). Git repo root here.
- Linux/GNOME, Wayland + XWayland mix. `DISPLAY=:0`, `WAYLAND_DISPLAY=wayland-0`.
- Shell tool: `pkill -f "tauri dev"` can kill your own shell if the pattern matches the command string —
  avoid broad pkill patterns. Server restarts kill in-flight tool calls; background shells stream to
  `/home/raghuwinder/.local/share/opencode/shell/<id>/<shell>.out`.
- ctx-mode installed and used for large outputs (ctx_execute/ctx_search). Big Reads get denied — use
  offset/limit or ctx_execute with `file`.
- Today's session date: **2026-11-23**. Screenshots were taken 2026-11-21 15:51–20:27.

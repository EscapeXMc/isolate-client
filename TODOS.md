# Isolate — active todos

## Website (isolateclient.vercel.app) — v2: Xryloth clone in blue
- [x] Clone Xryloth website source → `site/public/` (15 pages, Next static export) + `site/api/`
- [x] Rebrand: Xryloth/xryloth/FastClient → Isolate Client (37 files, 0 residuals)
- [x] Blue theme: orange #f97316/#e98a1e → blue #3b82f6/#2563eb across CSS/JS/HTML/SVG
- [x] Brand images regenerated from app icon: favicons, logos, og-image, banner, splash, title logo, launcher shot
- [x] Download system repointed → `EscapeXMc/isolate-client` (repo created; client `DEFAULT_UPDATE_API` updated too)
- [x] Discord → `https://discord.gg/TV2ypJF6TF` (socials we don't have point there too)
- [x] `/plans/` Razorpay page kept + rewritten in vercel.json (create-order live-tested, 0 redirects)
- [x] Secrets: none in client HTML; Razorpay secret + admin creds only in Vercel env
- [x] Deployed `vercel --prod` — all 20 live URLs HTTP 200, old brand/orange/fastclient = 0 hits
- [ ] Publish first GitHub release on EscapeXMc/isolate-client (buttons show real asset sizes once assets exist)
- [ ] Confirm rank names/features on /plans/ (still placeholders)

### Homepage edits (Nov 12, deployed)
- [x] Removed download-cards section from all 5 index pages (id="downloads" gone, FAQ intact)
- [x] Logo + favicon swapped to user imgur assets (latest: `i.imgur.com/9MH7RRb.png` shield emblem — full icon set
      regenerated: favicon.ico/png/svg/-16..192, apple-touch-icon, magmad/icon, logo-256/512; nav uses favicon.svg)
- [x] Hero splash → user-provided vector wordmark at `magmad/splash.svg` (all index pages ref it; img 458×122)
- [x] "Take A Look Inside" carousel fixed: React never hydrates in this build (original Xryloth chunks also stop
      after RUNTIME_LOADED — their own /__diag beacons prove it), so added `public/showcase.js` vanilla controller
      (arrows + dots + slide title/description parsed from embedded RSC flight payload); wired on all 5 index pages
- [x] Showcase images: 6 placeholders (`isolate-launcher.jpg`) → `magmad/showcase/showcase-1..6.png` (HTML + flight)
- [x] OG tags completed: og:image type/dims fixed (png 1200×630) on 5 index pages; full OG+Twitter sets added to
      /plans/, toast-test, xryloth-client-preview; og:url + twitter:image/title/desc added to /download + /faq
- [x] Live-verified: carousel clicks work on / and /index-de, all assets 200, no secrets
- [x] Showcase replaced with OUR real client screenshots (6 slides, all Isolate-branded, FastClient PNGs deleted):
      1 Home & Launch (shot-home) 2 Partner Servers (shot-partners) 3 Settings & Performance (shot-settings)
      4 Isolate AI (shot-ai3) 5 Sessions & Instances (Pictures 11-11 17-10) 6 Minecraft Main Menu (Pictures
      11-11 18-17, titlebar cropped) → `magmad/showcase/isolate-1..6.jpg`; titles/alts/dots/flight all updated,
      ~7MB FastClient PNGs → ~760KB JPEGs; live-tested (walk all 6, dots, 0 JS errors)

### v1 (superseded by clone, code kept as index-v1-backup.html)

- [x] Fix login endpoint (`accounts:` prefix) — "load failed" resolved, verified in client
- [x] Replace dead Discord invite `discord.gg/isolate` → `https://discord.gg/TV2ypJF6TF` (RightHub.tsx)
- [x] Deal design direction (taste-skill: `type-only-statement`, seed 519082355, brand palette/type kept)
- [x] Capture real product screenshots (home / settings / partners / +more)
- [x] Landing page `site/index.html` — hero, features, showcase, how-it-works, proof, pricing, faq, cta, footer
- [x] Plans page `site/plans/index.html` — 5 Solute tiers (Free + ₹191/₹299/₹599/₹999) + Razorpay checkout
- [x] Serverless `site/api/` — create-order + verify (secret in Vercel env only) → writes rank to Firebase
      Live-tested: real order created, bad signature rejected 400, no secret in client JS
- [x] Gates: lint.mjs 0 FAIL/0 WARN both pages; inspect.mjs 0 FAIL both pages; renders critiqued
- [x] Deploy `vercel --prod` — LIVE: https://isolateclient.vercel.app/ and /plans/ (both HTTP 200)

## Social / Discord system (phases)
- [x] Phase 1a: email/password auth (`fbSignUp`/`fbSignIn`) + session fixes in `firebase.ts`
- [x] Player write probe — players can write `social/users/{uid}` (public read rules OK)
- [ ] Phase 1b: Social page shell + profile (name, picture, bio, badges)
- [ ] Phase 2: friends (requests, list, presence)
- [ ] Phase 3: servers → categories → text channels (create requires owned MC server)
- [ ] Phase 4: DMs + realtime messages (10s poll or SSE)
- [ ] Phase 5: voice chat (rank-gated)
- [ ] Phase 6: ranks read in client (gates voice) ← reads what /plans/ writes
- [ ] Phase 7: local 24/7 Discord bot (token from Settings, auto-start)

## Website deploy notes
- Vercel project `isolateclient` (CLI auth escapex112233-6657), source `site/`
- Secrets in Vercel **production env only**: RAZORPAY_KEY_ID, RAZORPAY_KEY_SECRET, ADMIN_EMAIL, ADMIN_PASSWORD
- `site/.env.local` holds only the OIDC token (gitignored)
- Flow: client POST /api/create-order/ → Razorpay order → checkout → POST /api/verify/ (HMAC check) → admin token → writes `ranks/{email}` in RTDB

## Client app (Tauri) — Nov 13
- [x] Mods page: empty search → most-downloaded default (Rust `index=downloads` when query empty + frontend trending fetch/sort/header)
- [x] Host page redesign: homepage-style hero (`.sm-hero` + `.vp-grid`), server select cards (`.srv-card`), featured plugins banner with logos (`.plug-banner`, auto-load on Plugins tab)
- [x] `npx tsc --noEmit` clean
- [x] Live-verify Host + Mods pages in `tauri dev` window (screenshot + click-through) — Mods shows "Most Downloaded" grid; Host shows hero + server cards + Featured plugins banner with logos
- [x] Feature batch (code): partnership live/fixed count mode + 30s ping loop; Accounts boxes aligned; Isolate login/register (same form = admin); telemetry (`telemetry.ts` + Rust `get_hardware_info`) → `users/{uid}`; Admin panel (Users/Admins/Launcher-config, ban w/ reason+photo/video); remote config polled60s (accent/title/brand/notice); full-screen ban card; Discord chat page (user token → Rust `discord_api` proxy, guilds/channels/DMs/messages); Ely.by + Discord mirrors to Firebase
- [x] `cargo check` clean (window-set-title allowlist + Cargo feature added)
- [x] GPU telemetry fix: `lspci -mm` parse now takes vendor+device quoted fields (was grabbing class name "VGA compatible controller") — `cargo check` clean
- [x] Remote config live-verify: REST write `notice`/`name`/`accent` → client 60s poll applied all three (banner rendered, window title changed, buttons turned red) — then reverted to defaults and verified revert (banner gone, title "Isolate Client", blue accent). `.btn.primary` + gradients now use `var(--accent)`/`var(--accent-2)` (accent-2 derived in App.tsx); `tsc` clean.
- [x] Ban screen e2e: REST-wrote ban fields → client poll rendered full card (reason, until+countdown, by, photo/video links, appeal, re-check); removed fields in Firebase (fresh boot shows normal UI)
- [x] Ely.by sign-in end-to-end test — PASSED (PKCE was the blocker: verifier + `code_challenge=S256` on authorize URL, `code_verifier` on exchange; full login as EscapeXOG, settings + UI verified)
- [x] RTDB sibling-wipe fix: `fbUpsertUser`/`fbSetBan` used PUT on `users/{uid}` (wiped `ely_account`/`banned*`/`pass_hash` on every telemetry write) → PATCH; e2e-verified (boot write events 17→18, `ely_account` survived)
- [x] Live-verify remaining: Accounts login/register, Partnerships Edit toggle, Admins add/remove — Playwright e2e 10/10 PASS (register→badge, admin login→ADMIN badge+Admin panel, sidebar Admin nav after reload, Edit opens/closes with Live fetch/Fixed toggle, admin add→row→remove→gone; `/tmp/opencode/verify_all.mjs`)
- [x] Discord card fix: white/light box → dark blurple card (`.w-discord` gradient + `#5865f2` JOIN); live `17 members · 4 online` from public `GET /invites/TV2ypJF6TF?with_counts=true` (tokenless) + guild widget member list, 60s refresh in RightHub; Rust `discord_invite_info` command. NOTE: guild Widget disabled (403) → avatar-bubble list stays empty until user enables Server Settings → Widget; counts/icon always work.
- [x] Windows optimization suite (Phases 1-5) in `src-tauri/src/win_opt.rs`: 1) audit + HAGS (HKLM HwSchMode=2) + power plan High/Ultimate + launcher/javaw scan; 2) winget GraalVM→Temurin JDK 21; 3) timestamped backup `C:\Minecraft_Optimization_Backup\` + RAM calc + ZGC/Aikar flags into launcher_profiles/Prism instance.cfg + options.txt (maxFps 260, vsync off, raw mouse); 4) per-javaw registry (IFEO CpuPriorityClass=3, GpuPreference=2, DISABLEDXMAXIMIZEDWINDOWEDMODE, NetworkThrottlingIndex) via single UAC elevation; 5) NVIDIA Profile Inspector .nip + report.txt + rollback.ps1 + BIOS checklist. Auto-runs on background thread at `launch_game` + manual `run_system_optimizer` cmd; version-aware GC flags injected into own launches (MC ≥1.20.5 ZGC else Aikar's); runtime `cfg!(windows)` gate, compiles on Linux (`cargo check` clean). HKLM steps need one UAC prompt on first Windows run.
- [x] Admin page v2 (full details + features): per-user **full detail modal** (Identity incl. uid/email/first-seen/last-seen/last-event/events/password-hash · Hardware · Linked accounts Ely/Discord/Microsoft raw JSON · Moderation status/reason/by/at/expires/evidence links · Raw JSON + Copy JSON + 2-step Delete user + Ban/Unblock); stats cards (Total/Active 7d/Banned/Ely/Discord/Admins); search (email/uid/hostname/CPU/GPU/OS) + sort (last/first seen, email, events); JSON+CSV export; **Action log** tab (`admin_log` RTDB node — ban/unban/delete/add_admin/remove_admin/config_save, newest first, last 200) via new `fbDeleteUser`/`fbLogAdminAction`/`fbFetchAdminLog`. Fixed Rules-of-Hooks crash (useMemo after early return). Playwright e2e **30/30 PASS** (`/tmp/opencode/verify_admin.mjs`, screenshots `adm-2-stats.png`/`adm-3-detail.png`) + `tsc` clean.
- [x] Social Phase 6 — ranks read in client (voice gate): `fbFetchRank(email)` reads `ranks/{slug}` (public RTDB read; `rankSlug()` kept identical to `site/api/verify.js`); app store `rank` + `refreshRank()` (boot + 60s, refresh-token fallback via `fbEnsureToken`); DiscordChat voice-channel click now gated — signed out → "Sign in on Accounts"; no rank → err notify + `api.openUrl(plans)`; active rank → "unlocked · audio coming soon" with `rankLabel`; Accounts page shows `★ Solute …` badge next to signed-in (fetch on mount/sign-in, cleared on logout). Labels match plans page (plus ₹191/pro ₹299/elite ₹599/ultra ₹999). `tsc` clean · Playwright 5/5 (`verify_rank.mjs`: badge shows Solute Pro, sign-out clears, slug parity, live read) · Chat page no pageerrors. NOTE: test rank `ranks/admin_isolateclient_in` (solute-pro, payment_id test_e2e_123) left in RTDB for demo — delete when real payments arrive.
- [ ] Splash SVG says "ISOLATED CLIENT" (extra D) — confirm with user before changing

## Production overhaul — Xryloth-clone ecosystem (15 Nov)
- [x] Step 1+2 — Website UI/UX overhaul: pure-black `#0a0a0a` glassmorphism, 9 custom white cursor SVGs, `iso-override.css` cascade-override layer, `page-fx.js` micro-interactions, UX-principles layer (Fitts/Hick/Miller/Doherty/Jakob + squash-stretch/snappy timing) — `verify_ui2.mjs` 22/22 PASS
- [x] Step 3 — E2E Minecraft library/asset diagnostic tests (`e2e_install_*`, 8/8): modern `:natives-<os>` classifier natives download+flatten fix, asset index 4593 objects, client.jar sha1
- [x] Step 4 — Cross-platform build system: `scripts/build-linux.sh` (AppImage/deb, 4-job RAM caps), `scripts/build-windows.ps1` (NSIS/MSI/portable), `.github/workflows/build.yml`, Cargo release profile (`opt-level=s`, thin LTO, strip)
- [x] Step 5 — GitHub auto-updater with anti-loop: `update_guard()` (dismissed/loop_guard), `~/.isolate/update_state.json`, boot check +1.8s, animated `UpdateModal.tsx` — 9/9 unit + `verify_upd.mjs` 15/15
- [x] Step 6 — IsolateClientMod + perf pack auto-provision: strict per-MC Modrinth gate, fork-pair guard (modernfix↔mvus, noisium↔noisiumforked), 4-worker parallel downloads, 10-min API cache, manifest lock; `create_instance` hook (Fabric API + IsolateClientMod) + 4s startup scan — 21/21 unit + **live E2E 374s** (isolateclient-1.21.11.jar + **58 FPS mods**, 59 jars)
- [~] Step 7 — Release automation: `scripts/release.sh` (canonical asset names) + CI `release` job (tag OR dispatch → attach) + site matchers aligned (`windows64`/`appimage`) + first release **v0.1.0 PUBLISHED** (AppImage 197M + deb 3.6M) and live 302 redirects verified on isolateclient.vercel.app — **PENDING: Windows `.exe`** (needs `workflow` scope to push build.yml → CI builds + attaches)
- [ ] One-time GitHub auth: `gh auth refresh -h github.com -s workflow` (browser approve) → push `.github/workflows/build.yml` → `gh workflow run build` → CI release job attaches `IsolateClient_0.1.0_windows64-setup.exe` + `_windows_universal.exe`
## Housekeeping
- [ ] RTDB rules: tighten `partnerships` write to admin-only (currently any signed-in user)
- [ ] Confirm rank names/features (sample content on /plans/ — tiers are Free/₹191/₹299/₹599/₹999, features are placeholders)
- [x] Download URL = GitHub releases (`IsolateClient/isolate-client/releases/latest`) — used in hero + footer CTA

## Done earlier
- [x] Homepage redesign, Settings, Partnerships (login form removed, admin login moved to Accounts)
- [x] Zoom/double-click fix, boot 15s timeout, keys audit, Firebase live (5 partners seeded)

- **Launch gate (15 Nov 2026):** site locked until 2026-11-18 15:46:07 IST; `/18nov/` owner bypass via `localStorage.iso_owner`; gate.js + launch.html + 18nov/ live on prod, tests 13/13 + 22/22 + live 7/7. Step 7 Windows exe also attached to v0.1.0 (run 43708607051) — overhaul Steps 1–7 all ✅.

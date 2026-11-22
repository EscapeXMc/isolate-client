# CONTEXT HANDOFF — Isolate Client site (read fully after compaction)

> ⚠️ **NEWEST: read `CONTEXT-COMPACT.md` FIRST** (2026-11-21) — it supersedes this file for the current active task (20-point site quality pass + GSC submission) and holds fresh audit results. Then read this file for older site context.

## 1. What this project is
- Marketing site for **Isolate Client** (free Minecraft launcher, Windows + Linux).
- Repo root: `/mnt/Data Drive/IsolateClient` (git). Site source: `site/public/` (static HTML/CSS/JS), serverless fns in `site/api/` (create-order, download, geo, mc-status, stats, verify).
- Deploy: `cd site && npx vercel --prod --yes` (CLI authed as `escapex112233-6657`). Production aliases to **https://isolateclient.in** (primary domain, bought 2026-11-20 at BigRock, nameservers → Vercel, project `isolateclient` + www alias). `isolateclient.vercel.app` still serves as mirror. Second domain on account: `xescape.net` (project `escapex`).
- Config: `site/vercel.json` — static rewrites hiding filenames (`/plans`→`/plans/index.html`, `/store`→`/store.html`, `/faq`, `/download`, `/contact`, `/track`, `/team`, `/privacy`, `/terms`, `/preview`, `/uninstall`, `/18nov`). **Now also has `cleanUrls` intent + `/subdomains` rewrite (see §8).**

## 2. Non-negotiable rules (user's standing instructions)
- **Do NOT ask questions.** Make no mistakes. Follow the TODO list. User writes in Hinglish; reply can be Hinglish/English.
- Git author: `EscapeXMc <escapex112233@users.noreply.github.com>`, commit prefix `site: ...`.
- **Credit line must survive every restyle:** "An isolated client by Shayan & Raghuwinder, built for performance" (Raghuwinder → links to `/team`).
- Discord `https://discord.gg/TV2ypJF6TF` never touched. Secrets only in Vercel env. Owner bypass `/18nov/` → `localStorage.iso_owner="1"`.
- Canonical nav: `HOME · STORE · FEATURES · FAQ · TEAM · PLANS · DOWNLOAD` (plans page adds Discord = 8 items). Contact/Track/Uninstall/Privacy/Terms = footer only.
- Nav hamburger works, tap targets ≥44px, no horizontal overflow at 360px and 1440px.
- **Suite gate: `node /tmp/opencode/verify_nav.mjs [baseUrl]` → must be 184/184** (16 pages × desktop+mobile). Run local before deploy, then live after deploy.
- Palette: black `#0a0a0a` ground + Isolate blue (`#3b82f6` / `#60a5fa`) for structure/Discord/focus + Neon Lime `#A3FF12` only for buy/download moments (odd-10%). Text on blue/lime → `#0b1220`.
- Showcase image `https://i.imgur.com/UyQ4YZQ.png` must appear on all 5 index pages; index must keep credit text + a `youtu.be` link (Watch Demo + final CTA → `https://youtu.be/T-8f3oEy_hE`).
- Design skill installed: `.claude/skills/landing-page-design` (from elayadesign/ai-design-skills). Rules: sentence case headings (no caps), no hyphens in copy, benefit-first claims, no gradient backgrounds (only hero heading text gradient `#FFF→#9B9B9B` allowed), motion `cubic-bezier(0.22,1,0.36,1)` transform/opacity only, spacing tokens 2/4/8/12/16/24/32/40/48/64/80/96, dark palette `#000/#181818/#1F1F1F/#272727/#313131/#131209`, borders all-around or none, no invented facts (benchmark numbers 1650/1180/1020 FPS on i7-12700K + RTX 3070 + Minecraft 26.1 are the only citable stats).
- Page facts to preserve: `iso-count.js` live download counter exists; splash logo `/magmad/splash.svg`; `id="download"` anchor + `magmad-download` button id.

## 3. Design system files
- `site/public/assets/iso-override.css` — global design layer (glass panels `#131316`, `--line-2`, `--radius`, section card rules for `#features/#performance/#screenshots/#faq > div`, `.iso-enter` pose reveal animation, buy-neon section rules).
- `site/public/page-fx.js` — injects iso-override.css + IntersectionObserver adds `.iso-enter` to `main section` (auto-reveals children with stagger).
- `site/public/showcase.js` — plain-DOM carousel controller for "Take A Look Inside" (React hydration never completes).
- `site/public/iso-count.js` — live download counter.
- Fonts: **Russo** (headings, `.oswald` class is a misnomer) + **Chakra Petch** (body). Keep both — they are brand fonts.

## 4. Completed work (with commits)
- Plans page redesign (neon lime = buy moment, blue = structure) + Download-for button blue→neon: commit `66dd2fb` (pushed, deployed, 184/184 live). Fixes shipped: `.tbl-scroll{position:relative}` (280px overflow @360), hero image `height:auto`, Pay button lime, price `/ month` own line.
- Domain switch to isolateclient.in: canonicals/og:url/robots/sitemap all retargeted (was vercel.app). Sitemap = 15 URLs, lastmod 2026-11-20, hreflang en/nl/de/fr/es + x-default, excludes noindex pages (track, launch, 18nov, toast-test).
- ai-design-skills installed → `.claude/skills/landing-page-design`, `.claude/skills/figma-designer`.
- GSC submission attempt: script `/tmp/opencode/gsc_submit.py` (PyJWT, IPv4-forced). **Blocked on Google-side (3 stacked blockers):** (1) Search Console API disabled → enable at `https://console.developers.google.com/apis/api/searchconsole.googleapis.com/overview?project=824505628703`; (2) service account lacks `serviceusage.services.enable`; (3) SA not a user on `sc-domain:isolateclient.in`. No Google session exists on machine. Alternative: user pastes `https://isolateclient.in/sitemap.xml` in GSC UI (Sitemaps → Add). Report these links at end — do not ask.
- Credentials found (user-supplied): `/home/raghuwinder/Downloads/openphone-bfd7c-firebase-adminsdk-fbsvc-12573e7b6b.json` (SA `firebase-adminsdk-fbsvc@openphone-bfd7c.iam.gserviceaccount.com`, project 824505628703), `/home/raghuwinder/Downloads/google-services.json`, `/home/raghuwinder/.isolate/settings.json` (Ely.by tokens, unrelated).

## 5. UNCOMMITTED state at time of writing (verify with `git status`)
- Homepage redesign **already applied locally** to `site/public/index.html` (patch: `/tmp/opencode/patch_home_redesign.py`, ran clean; suite passed 184/184 after). Changes: sr-only H1 → "play Minecraft without the frame drops"; hero subheadline `.home-sub` after splash; new **tagline reveal section** `#tagline` (word-by-word `.w` spans, IO +70ms stagger, reduced-motion = visible); new **How it works** 3-step section `#how` between features and performance; feature copy rewritten sentence-case/benefit-first; performance H2 → "More frames than the launchers you know"; page-scoped `<style>` in head; inline reveal script before `</body>`.
- Also applied by same patch: **11 skip links fixed** site-wide (were broken → pointed at `index-es.html#main-content` or `.tmp.html#main-content`; now all `href="#main-content"`), **17 og:image swapped** vercel.app→isolateclient.in.
- `site/public/robots.txt` + `site/public/sitemap.xml` rewritten (isolateclient.in) — uncommitted.
- ~20 HTML files canonical/og:url swaps (isolateclient.in) — uncommitted.
- `.claude/skills/` (untracked), `TODOS-session-compact.md` (untracked), this file (untracked).
- **Not yet done for homepage redesign:** screenshot review (baseline `/tmp/opencode/home_before.png`), deploy, commit.

## 6. TODO list state (claude-mem list `isolate-features`)
- `ely_by_login` = doing — full login info in Firebase + end-to-end test.
- `accounts_page` = doing — accounts page align + Isolate login form integration.
- `live_verify` = todo — tauri dev verify (accounts/admin/discord/partnerships) + HANDOFF/TODOS update.
- `isolateclientmod_autoreplace` = PAUSED by user (2026-11-15): approved but do NOT build. Startup auto-replace of stale `isolateclient-{mc}.jar` vs GitHub latest in `src-tauri/src/lib.rs` (skip-if-exists ~L4735, provision ~L4768, startup thread ~L12064).
- `site_status` = mostly DONE (mobile /team, live counter, footer credit, IG/YT → @oyeraghuwinder; commits `cc0f763` + `03d1af3` pushed) except items now superseded by this file.

## 7. claude-mem status
- Observer auth REFUSED since 2026-11-13 (error: "Claude Code could not authenticate the observer"). Nothing persists until user runs `/login` / `claude auth login`. Do NOT restart worker, do NOT suggest restart. Tell user about outage at start of first reply, quoting the error.

## 8. NEW TASKS (current — user request 2026-11-20, Hinglish)
### 8a. Subdomains page `https://isolateclient.in/subdomains/`
- Create `site/public/subdomains/index.html` + vercel.json rewrite `/subdomains` → `/subdomains/index.html` (and `/subdomains/`).
- Form (copy the user's spec): **Service Type** = fixed "Minecraft Java Edition"; **Server IP Address** (placeholder `e.g. 123.45.67.89`); **Server Port** (default `25565`); **Email Address** (`you@example.com` — "We'll notify you before your subdomain expires"); **Subdomain Name** (e.g. `firemc`) → shows `<name>.elixir.host`.
- Note line: "Subdomains expire after 30 days - create a new one for free anytime".
- **Flow:** fill form → submit → **2-minute wait gate** showing **multiple Adsterra ads** + countdown → then reveal their subdomain (`<name>.elixir.host`) with copy button.
- **Adsterra:** we do NOT have their publisher/ad-unit keys — do NOT invent keys. Build 2–3 ad slot containers driven by a config array at top of the page's script with obvious placeholder keys + comment `// paste your Adsterra ad unit keys here`, so user pastes real keys. Mention this clearly in final report.
- elixir.host is a third-party DNS service — UI-only flow (no real provisioning API known). Success panel shows the subdomain + 30-day expiry note.
- Style: follow landing-page-design skill + site design system (black/blue/lime, Russo/Chakra, sentence case, ≥44px targets, no h-overflow).

### 8b. Clean URLs (hide filenames)
- Add `"cleanUrls": true` to `site/vercel.json` (Vercel then serves `/store` for `store.html` and redirects `.html` URLs to clean ones). Keep existing rewrites. Do NOT mass-edit internal links (suite risk).
- Verify after deploy: `curl -sI https://isolateclient.in/plans` etc.

### 8c. 404 page
- Create `site/public/404.html` (Vercel serves it for unknown routes). Styled like the site: "Page not found" + back-home CTA + nav links. No invented facts.

## 9. Verification & deploy procedure (run in order)
1. `python3 <patch script>` (atomic, asserts counts — model: `/tmp/opencode/patch_plans_redesign.py`, `/tmp/opencode/patch_home_redesign.py`, `/tmp/opencode/patch_canonical_domain.py`).
2. Serve `site/public` over HTTP, then `cd /tmp/opencode && node verify_nav.mjs` → **184/184**.
3. Screenshots: `/tmp/opencode/cap.js` (`node cap.js <path> <out.png>`); Playwright at `/home/raghuwinder/claude-animation-skills/kit/node_modules/playwright`; Node v24 → wrap `require()` + top-level await in async IIFE in `.mjs`.
4. `cd site && npx vercel --prod --yes` → wait READY → `node verify_nav.mjs https://isolateclient.in` → 184/184.
5. Commit as EscapeXMc `site: ...`, push. Update `TODOS-session-compact.md` header commit (keep untracked).

## 10. Environment quirks
- **IPv6 routes are dead** → any Python HTTPS must force IPv4 (`socket.getaddrinfo` monkeypatch) or it hangs on `2001:4860::`. curl unaffected.
- Prefer `/tmp/opencode` for temp files (approved external dir).
- No gcloud ADC, no signed-in Google session anywhere on machine (devai Chrome profile signed out, bot Chrome CDP 9223 `.owpenbot/chrome-bot` signed out).
- context-mode: large outputs → use `ctx_execute` with `file` + print only answers; `ctx_search` to recall.

## 11. User phrasings to preserve (do not paraphrase away)
- "we buyed a domain see the doamins"
- "Continue bro and make a robots.txt sitemap and summit everyhtin to https://search.google.com/search-console?resource_id=sc-domain%3Aisolateclient.in and i have a json fiels for my crediatials find those!"
- "Continue make todos adn follow them and bor the homepage is litterly the copy of fastclient make it better! and use and install https://github.com/elayadesign/ai-design-skills/tree/main"
- "cotninue and dotn ask em anything@" / "make no mistakes"
- "change he DOWNLOAD FOR button from blue to neon" / "REDESIGN THE https://isolateclient.vercel.app/plans UI AND UX"
- New: subdomains page spec (§8a), "file ka name mat show karo in url me", "404 page bhi bna", "ek md file bna ... har ek chij yaad rahe"

## 12. Benchmark facts (cite, never invent)
- 1650 FPS (Isolate) vs 1180 vs 1020 FPS, measured on i7-12700K, RTX 3070, Minecraft 26.1.
- Download counter is live (iso-count.js). Hundreds of free skins/capes claim exists in current copy.

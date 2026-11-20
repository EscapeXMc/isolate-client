# TODOS + Deep Session Compact — Isolate Client Site

> **Purpose:** pre-compaction handoff. Read this file to resume full context instantly.
> **Date:** 2026-11-20 · **Repo:** `/mnt/Data Drive/IsolateClient` · **Site:** https://isolateclient.vercel.app/ · **Git main = `66dd2fb` (pushed)**
> *(prior: `3f664ed` restyle-to-team, `89fb14f`/`3eb19c5`/`20dc3be` earlier tasks)*

---

## 🟢 CURRENT STATE (2026-11-20) — neon Download-for + /plans REDESIGN SHIPPED

**Commit `66dd2fb` (2 files: `iso-override.css`, `plans/index.html`) — deployed + verified 184/184 local AND 184/184 live.**
*(built on `3f664ed` restyle-to-team: CSS-only restyle, unified team-style `footer.foot` everywhere, Russo One + Chakra Petch on every page, `iso-override.css` = global design layer, `mnav.css` = team burger panel.)*

**Architecture facts (from restyle-to-team, still true):** headers keep per-family markup. **Family C** = shadcn `nav[aria-label="Main navigation"]` + `#public-mobile-navigation` panel + trigger `[aria-controls=…]` (11 pages: index×5, store, contact, privacy, terms, uninstall, preview). **Family A** = `nav.bar` + `#menu-btn` + `#nav-links`, brand inside (faq, team, download). **plans** = `nav.bar[aria-label="Primary"]` 8 links, brand outside. **track** = no nav/footer. React NEVER hydrates in this Xryloth build → static burger panel + vanilla `mobile-nav.js`. Restyle bugs fixed: `.magmad` Nunito leak → `font-family: var(--font-body) !important`; `.mark` boxed the plans wordmark → dropped from site.css pill rule; download eyebrow `--muted` bg-as-text → `--muted-foreground`; plans panel links 22px → `nav.bar.open .links a` + `right:0; width:min(300px,…)` (278×44–48).

### A. Neon "Download for" button (user: "change he DOWNLOAD FOR button from blue to neon")
- `iso-override.css`: `.magmad-download` split out of the blue CTA group, literal `#a3ff12` (NO `--neon` token exists; `--accent` is lime globally BUT plans re-declares it blue). Text on lime → `#0b1220`.

### B. Full /plans UI/UX redesign (user: "REDESIGN THE https://isolateclient.vercel.app/plans UI AND UX")
- **Neon lime = the buy moment:** hero "Buy Solute+ ₹191/mo" is now `<button data-plan="solute-plus" data-amount="19100">` (opens checkout directly), hot Solute+ tier (`.rank.hot` lime border/gradient/tag), checkout Pay button (`#co-pay.btn.accent.buy-neon`). **Blue = structure/Discord/focus**; free tier + secondaries stay ghost; Pro/Elite/Ultra CTAs ghost→blue accent.
- **New sections:** `.p-eyebrow` chip, `.pay-trust` pills (UPI/Cards/Netbanking/Razorpay), `.compare` comparison table between rank grid and screenshots (`.tbl-scroll` overflow-x auto + `position:relative` **critical** — contains abspos `.vh` spans else document overflows 280px @360 under `reducedMotion:reduce`), sticky first column, lime ✓/grey —, `.tbl-hint` swipe note, `.plan-sum` summary box, `.pitch` one-liners per card.
- **Checkout UX:** Escape close + focus restore in inline script, err `#f8717a` (deliberate semantic-red — site.css `--danger` is wrongly lime), input focus blue ring, lime panel top border.
- **iso-override section 19** holds `main .rank.hot`(+hover) + `main .btn.buy-neon`/`.checkout .btn.buy-neon` !important overrides (file loads last); plans inline style = layout only.
- **Bugs fixed during redesign:** `height="900"` attr beat `aspect-ratio` on `.hero-shot img` → added `height:auto` (same for `.shot-row img`) so 16:10 applies; `#co-pay` was missing `buy-neon`; `.rank .price small` ("/ month") now `display:block`.
- Patch script model for further HTML edits: `/tmp/opencode/patch_plans_redesign.py` (atomic, asserts).

### C. Tests + deploy (source of truth, `/tmp` wipes daily)
- `/tmp/opencode/verify_nav.mjs` — **single suite, optional base URL**: `node verify_nav.mjs` (own static server on `site/public`) or `node verify_nav.mjs https://isolateclient.vercel.app`. **184/184 both modes.** 16 pages × desktop+mobile: nav labels/order/brand, footer credit + Download, Russo font, no h-overflow @1440 & @360, burger ≥44px, open/Escape/click-outside close, panel tap ≥44, index credit+youtu.be, team lime/no-rose.
- `/tmp/opencode/cap.js` — screenshot tool (`reducedMotion:"reduce"`, `localStorage.iso_owner=1`): `node cap.js <path> <out.png>`.
- Deploy: `cd site && npx vercel --prod --yes` (CLI authed as `escapex112233-6657`). Commit as `EscapeXMc <escapex112233@users.noreply.github.com>`, prefix `site: ...`.
- **Known non-issues:** cap.js `pageerrors: "Connection closed."` benign; `hiddenReveals` informational; hero "ISOLATED CLIENT" on index is a logo `<img>`; download's nav links lack `class="nav"` (visually equivalent, UI frozen).
- **Handoff:** `TODOS-session-compact.md` (this file) stays **untracked**.

### D. Site invariants (never break)
- Credit line "An isolated client by Shayan & Raghuwinder, built for performance" (Raghuwinder → `/team`).
- Discord `https://discord.gg/TV2ypJF6TF` untouched; secrets only in Vercel env; owner bypass `/18nov/` → `localStorage.iso_owner="1"`.
- Canonical nav `HOME · STORE · FEATURES · FAQ · TEAM · PLANS · DOWNLOAD` (plans adds Discord = 8); Contact/Track/Uninstall/Privacy/Terms footer-only.
- Palette: black `#0a0a0a` + Isolate blue (`#3b82f6`/`#60a5fa`) + `#A3FF12` odd-10% accent; button text on blue/lime → `#0b1220`. Showcase image `https://i.imgur.com/UyQ4YZQ.png` on all 5 index pages.

**Resume from here.** Sections below are historical (Tasks 1–3 are all shipped: `89fb14f`, `3eb19c5`, `20dc3be`).

---

## ✅ TASK 1 DONE (2026-11-19, commit `89fb14f`, live 47/47)

### 1. Nav bar buttons for EVERY main page — SHIPPED
- **Canonical set:** `HOME · STORE · FEATURES · FAQ · TEAM · PLANS · DOWNLOAD` (7 links) on ALL navs; Contact/Track/Uninstall/Privacy/Terms stay footer-only; plans page keeps Discord too (8 links).
- **All 3 nav systems patched:** (a) shadcn desktop nav ×11 files + `_next` chunks ×3 (T array gets `["/team","Team"],["/plans","Plans"]`); (b) faq + download adopted team.html hamburger (also fixed pre-existing 360px overflow 517/499 → clean); (c) team.html +plans (7). plans/index.html got full main set (`../` paths) + 44px tap targets.
- **KEY FACT: React NEVER hydrates in this Xryloth build** (pre-existing, upstream same — see TODOS.md L21; diag beacons stop at RUNTIME_LOADED). shadcn burger was dead site-wide → added static panel + vanilla `site/public/mobile-nav.js` + `site/public/assets/mnav.css` (backs off if React ever attaches). Same pattern as showcase.js.
- **Tests:** `/tmp/opencode/verify_nav.mjs` (local47) + `/tmp/opencode/live_nav.mjs` (live 47) — desktop 7-link order, mobile burger ≥44px, 360 no-overflow, Escape/outside close, footer/counter/lime smoke. (`/tmp` wipes daily — recreate from git history or this doc if gone.)
- Verify: `node /tmp/opencode/verify_footer.mjs` (currently 38/38), live check, deploy `cd site && npx vercel --prod --yes`, commit as `EscapeXMc <escapex112233@users.noreply.github.com>`

---

## 🟡 PAUSED TASK (inventory done, replacement NOT applied)

### 2. Site-wide Neon Lime `#A3FF12` as the 10% odd accent (all pages)
**User direction:** "bhai wahi same neon color site ke all pages par use karo as 10%" — black base + Isolate blue = main color + neon lime = the single 10% odd accent, everywhere (mirrors what /team already has).

**Palette rule:** base `#0a0a0a` · main blue `#3b82f6`/`#60a5fa` · **odd 10% = `#A3FF12`** (rgba lime = `163,255,18`). Red/rose/amber/orange/yellow/warm-brown ALL retire to lime. Dark warm text on blue buttons → neutralize to `#0b1220` (NOT lime — text-on-blue stays dark; lime-on-blue only ~3:1).

**Exact inventory (53 files scanned, done — replace these):**

| File | Color | Context | Action |
|---|---|---|---|
| `download.html` L89-93 `.warn` | border `rgba(250,204,21,.22)`, bg `rgba(250,204,21,.06)`, text `#fde68a` | amber warning box (`.show` toggled) | → lime `rgba(163,255,18,α)` + text `#A3FF12` |
| `download.html` L108-112 `.tag` | `color:#1a1008` on `background:var(--brand)` | version tag text on blue | → `#0b1220` (neutral) |
| `download.html` L124 `.md code` | `color:#fbbf24` | code text amber | → `#A3FF12` |
| `download.html` L149 `.error-box` | border `rgba(248,113,113,.3)`, bg `rgba(248,113,113,.07)`, color `#fca5a5` | error box (semantic red) | → lime versions (user wants no red anywhere) |
| `download.html` L66, L113 | `#22c55e` ×2, `#4ade80`, `rgb(74,222,128)` | success greens | → `#A3FF12` |
| `faq.html` L34 | `#4ade80` | green accent | → `#A3FF12` |
| `plans/index.html` L89 | `#4ade80` | green accent | → `#A3FF12` |
| `contact.html`, `privacy.html`, `terms.html` | `#116600` (dark green) | green accent (dark) | → lime-dark equivalent: use `#A3FF12` where on dark bg; check context — may be dark-theme-safe green → `#A3FF12` or keep-if-near-black; inspect line first |
| `index.html`, `index-de/es/fr/nl.html` | `#160d06` | `text-[#160d06]` on `bg-primary` blue buttons (warm-black text) | → `#0b1220` |
| `store.html` | `rgb(233,138,30)` in `shadow-[inset_0_0_56px_rgba(233,138,30,0.15)]` | orange inset glow on product cards + `border-accent-1` | glow → `rgba(163,255,18,0.15)`; check what `border-accent-1` resolves to (accent-1 token) |
| `assets/site.css` | `#f87171` (red-400) | one red usage | → inspect line → likely `#A3FF12` |
| `_next/static/chunks/2xlgh0h7trc28.css` + js chunks | many reds/purples/pinks/ambers/greens (`#ff0000`, `#fb2c36`, `#a685ff`, `#c13584`, etc.) + `--accent:#3b82f6` | **built Next.js export (store/other page assets?)** | these are BUILD ARTIFACTS — check if served pages use them; if the source is elsewhere (a Next project?) find source; if unreachable, patch the chunk CSS carefully or leave (decide after checking which page loads `_next/`) |
| `xryloth-client-preview/app.js`, `styles.css` | `#ff0022`, `#ff3366`, `#ff7a18`, `#b847ff`, `#00e888`, `#e5484d` | **Xryloth preview clone page** (reference design) | decide: it's a preview/clone artifact — probably leave OR lime-ize if page is publicly linked; check `preview.html` |
| `assets/iso-override.css` | `#08060d` (L207 `body svg [fill=...]` fill-remap) | functional icon recolor, NOT accent | **LEAVE** (file is shared site-system; no other warm accents in it) |
| `--accent` tokens | `site.css` `#3b82f6`, iso-override `rgba(59,130,246,0.18)` | blue = MAIN color | **LEAVE blue** (main stays blue; only odd slot goes lime) |

**Green/lime class note:** existing greens (`#4ade80` etc.) unify → `#A3FF12` for one neon language.

**Already done:** `/team` fully lime (commits `fb401dc` iso-override literal-color fix, `306f25e` rose→lime swap). team.html has ZERO red (grep-clean), 0 rose pixels in screenshots.

**Validation after applying:**
- `python3 .../contrast-check.py '<pairs>'` → 13/13 style AA (lime on black = 15.9:1)
- `node /tmp/opencode/verify_footer.mjs` → 38/38 (update if nav-link count changes → task 1)
- `node /tmp/opencode/probe_team_colors.mjs` → 27/27
- `node /tmp/opencode/live_team.mjs` → 42/42 (after deploy)
- grep site for leftovers: `grep -rnoE '#(f87171|fca5a5|fbbf24|fde68a|22c55e|4ade80|160d06|1a1008|116600)\b|rgba\(2(48,113|50,204|33,138)' site/public --include='*.html' --include='*.css'`
- Screenshots per key page (Playwright via `createRequire("/home/raghuwinder/claude-animation-skills/kit/package.json")`, gate bypass `localStorage.iso_owner="1"`)

---

## ✅ COMPLETED THIS SESSION (context for compact)

### /team redesign (octopus-ui-ux-design skill, maximal v9 m8 d6) — SHIPPED
- **Direction:** "Cinematic Brutalist Story" — 3-chapter asymmetric story, Russo One + Chakra Petch, marquee, IO reveals, reduced-motion, focus-visible, JSON-LD, hamburger nav
- **iso-override bug fixed:** shared `assets/iso-override.css` (injected at END of head by `page-fx.js`) forces `!important`: `--muted: rgba(255,255,255,0.07)` (made nav/meta/footer links ~1.1:1 INVISIBLE), `--accent: rgba(59,130,246,.18)` (made rose render blue), `body :is(p,li,.dim,.sub,...){color:#c3ccd9!important}`, `.sub→#96a0b0`, headings `#fff`+blue glow, `.foot` glass
- **Fix strategy:** LITERALS in team.html — `var(--muted)`→`#a1a1aa` (9×), `var(--accent)`→accent hex (8×), `var(--card)`→`#131316` (2×); `body p.sec-label{color:#a1a1aa!important}` (specificity 0,1,2 > iso 0,1,1)
- **Accepted iso overrides:** `p`→`#c3ccd9`, `.sub`→`#96a0b0` (contrast-pass, site-consistent)
- **Accent swap:** rose `#F43F5E` → **neon lime `#A3FF12`** per user 1-main+1-odd plan (class `.rose`→`.neon`); lime on black = 15.9:1 (rose was 5.39:1)
- **Commits (all pushed as EscapeXMc):** `a189cf0` hero-removal era → `cc0f763` counter+footer+team → `03d1af3` mobile /team → **`fb401dc`** redesign+iso-fix → **`306f25e`** rose→lime
- **Tests (all green):** verify_footer 38/38 · color probe 27/27 · contrast 13/13 AA · live 42/42 @1440/390/360 · screenshots 0 rose px · live source 0 red
- **Design lineage persisted:** `~/.claude-octopus/designs/IsolateClient/raghuwinder-main-design-20261118-133441-*.md` (branch main, rev `306f25e`, supersedes rose-era doc) — persist script: `/tmp/opencode/persist_design.sh`, body: `/tmp/opencode/design_body.md`

### Earlier shipped (previous sessions)
- Tauri overhaul steps 1-7, release v0.1.0, CI green
- Launch gate live (14/14), hero text removed + Watch Demo → `https://youtu.be/T-8f3oEy_hE`
- Live Firebase download counter (RTDB `stats/downloads`, baseline 12, `/api/download` 302 no-store, `iso-count.js`)
- Footer credit "An isolated client by Shayan & Raghuwinder, built for performance" + Raghuwinder → `/team`
- Socials: Instagram `https://www.instagram.com/oyeraghuwinder/`, YouTube `https://www.youtube.com/@oyeraghuwinder`, Discord `https://discord.gg/TV2ypJF6TF` (untouched)
- `/team` mobile pass: hamburger, no h-overflow (28/28 local, 10/10 live)

---

## 🛠 Environment / Commands

- **Deploy:** `cd "/mnt/Data Drive/IsolateClient/site" && npx vercel --prod --yes` (CLI auth `escapex112233-6657`; alias auto → production; verify `npx vercel ls isolateclient`)
- **Local test server pattern:** see `/tmp/opencode/verify_footer.mjs` (serves `site/public` on 127.0.0.1:1442 with `/team`→`team.html` rewrite mirror)
- **Scripts:** `/tmp/opencode/verify_footer.mjs` (38) · `probe_team_colors.mjs` (27) · `live_team.mjs` (42) · `shot_team.mjs` (screenshots) · `persist_design.sh` (lineage)
- **Contrast:** `python3 /home/raghuwinder/.claude-octopus/plugin/scripts/helpers/contrast-check.py 'FG:BG ...'` (exit 1=fail)
- **Playwright:** `createRequire("/home/raghuwinder/claude-animation-skills/kit/package.json")` → chromium; owner gate `localStorage.iso_owner="1"`; full-page shots need scroll-through + force `.rv.in` + `scroll-behavior:auto`
- **Design direction doc:** `/tmp/opencode/design-search/direction.md` · critique: `/tmp/opencode/critique-prompt.md`
- **Skill:** `octopus-ui-ux-design` at `/home/raghuwinder/.opencode/claude-octopus/skills/octopus-ui-ux-design/SKILL.md` (Step 8 lineage contract = script inside `persist_design.sh`)

## 📏 Standing constraints (user rules)

1. **Rest of site UI/UX FROZEN** except explicitly-requested changes (nav-bar buttons = now requested; lime accent = now requested)
2. Secrets only in Vercel env (`RAZORPAY_KEY_SECRET`, `ADMIN_EMAIL/PASSWORD`, bot token) — never in HTML/JS
3. Owner bypass: `/18nov/` sets `localStorage.iso_owner="1"`
4. **iso-override.css / page-fx.js = shared system files** — edit only when the task is explicitly site-wide (lime task may touch iso-override only if needed; currently NO warm accents there → leave alone)
5. Git author: `EscapeXMc <escapex112233@users.noreply.github.com>` · commit style: `site: ...` prefix
6. Claude-mem save BROKEN since 2026-11-13 ("Claude Code could not authenticate the observer") — user must `/login`; restart won't fix; work-state lists can't update — already informed
7. `IsolateClientMod` startup auto-replace = **PAUSED by user** (approved, do NOT build)
8. User spellings: "karwyaa"=karwaya, "all ages"=all pages, "bahi/bhai"=brother, "cmapct"=compact

# CONTEXT-COMPACT.md — READ THIS FIRST after compaction

> **⚠️ 2026-11-23: The ACTIVE task handoff now lives in `HANDOFF-2026-11-23-fastclient-redesign.md` — read that file FULLY first (it supersedes older task details below; this file's site/infra facts still apply).**
>
> **Instruction to future agent:** This file is the single source of truth after context compaction. Read it fully before doing anything. Do NOT redo completed work. Session date: 2026-11-21.

---

## 0. ACTIVE TASK (updated 2026-11-21, evening)

**DONE today:**
- 20-point pass: deployed `6276ca1`, live suite 184/184.
- **Homepage v3 deployed `aeb166e`** (pushed + Vercel prod + live suite **184/184**): centered Feather-style hero (badge/chips/glow shot), Feather feature cards (icon tiles), "See It In Action" video section, 6-image screenshot grid (carousel removed), imgur→local fallback, tagline section+JS+CSS fully removed. Local preview server pid 70396 port 8080 cwd `site/public`.
- GSC: API enabled by user; `sites.list`=200; sitemap `https://isolateclient.in/sitemap.xml` submitted manually (status "Couldn't fetch" = day-1 transient, recheck in 24h). robots.txt already live + lists sitemap.

**NEXT (in order):**
1. **PALETTE + LOGO CURSOR OVERHAUL DONE, DEPLOY PENDING (22 Nov):** orange `#FF5500` (CTA/badges) + sky `#38BDF8` (icons/links) + white/black across site AND client, naye shield logos (favicon/splash/icon/OG/Tauri icons), **shield-as-cursor** (site `cursors/pointer.svg` = white shield, `pointer-hand.svg` = orange shield; client `public/cursors/` same). Local suite **184/184 green** (cursor + palette), tsc+vite build clean, images 11/11. **User ne deploy OK diya ("deploy it") — commit (`site:` + `client:`) → push → `cd site && npx vercel --prod --yes` → live suite 184/184.** Note: `/tmp/opencode/verify_nav.mjs` session DB se restore hua (tmp wipe me gaya tha) — ab suite `ff5500` check karta hai. Untracked `.agents/` + `skills-lock.json` = skill installs, commit MAT karna.
2. **USER KA NA KAAM (22 Nov): Razorpay site se remove karna hai — user ne "do not do it" bola, START MAT KARNA jab tak wahi na bole.** Details: `TODO-KAL.md` item0.
3. GSC: 24h baad Sitemaps page refresh — "Success" aana chahiye; 48h baad bhi same → batana. (Optional API path: add SA `firebase-adminsdk-fbsvc@openphone-bfd7c.iam.gserviceaccount.com` as Owner on `sc-domain:isolateclient.in`, phir `python3 /tmp/opencode/gsc_submit.py`.)
4. VM stuck in WinRE (BCD damaged) → blocks installer e2e + 1.8.9 install.log root cause.
5. Firebase updater: `FIREBASE_DB_URL`/`FIREBASE_AUTH` unset; no initial `releases/latest` pushed.

<details><summary>Old active task (completed) — 20-point pass + GSC</summary>

User said: *"DONT DO ANYTHING else just do this first"* — 20-point website quality pass on **isolateclient.in**, THEN submit site to Google Search Console using provided credentials.

### The 20 points (user's exact list)
1. Remove horizontal overflow mobile+desktop
2. Working mobile menu (opens/closes, not clipped)
3. Navbar logo clickable → home
4. Fix broken buttons/CTAs (real actions/flows)
5. Real form validation + error states
6. Accessible success confirmations after submits
7. Replace placeholder/lorem copy with real copy
8. Clear above-the-fold value prop + CTA
9. Standardize colors/fonts/spacing into design system
10. Remove AI clichés (purple gradients, random emojis, repetitive centered layouts)
11. Compress/optimize images (aspect ratios, faster loads)
12. Descriptive alt text; decorative icons marked appropriately
13. Keyboard nav, focus states, text scaling
14. Unique page titles + meta descriptions every route
15. Open Graph / social share tags + share image
16. robots.txt + custom 404 with helpful nav
17. Trust/legal pages (privacy, contact/FAQ, terms)
18. Fix console errors + failed network/API requests
19. Harden prod edge cases (env vars, redirects, auth, payment/error handling)
20. Verify E2E user flows across desktop/tablet/mobile

**THEN:** submit isolateclient.in to GSC (user says "i gave you credential use it!").

</details>

---

## 1. AUDIT ALREADY RUN — findings (DO NOT re-run from scratch)

Script: `/tmp/opencode/audit_site.mjs` (Playwright, local static server on `site/public`).
Full report: `/tmp/opencode/audit_report.txt` (139 lines).

**Result: HIGH 8, MED 171, LOW 1.**

### HIGH (real issues to fix)
- **Mobile menu "didn't open" on 7 pages**: `/faq.html`, `/download.html`, `/team.html`, `/plans/index.html`, `/subdomains/index.html`, `/account/index.html`, `/404.html`
  - ⚠️ **This may be a FALSE POSITIVE**: audit looked for Family A selectors (`#public-mobile-navigation`), but these are **Family B** pages using `nav.bar` + `.links` + `#menu-btn` + `nav.bar.open .links` CSS. Family B has its own inline script: `nav.classList.toggle("open")`. **Verify manually before "fixing"** — script written but NOT yet run: `/tmp/opencode/test_famb.mjs` (run this first).
- **`/contact.html` has no `<form>`** — real issue. Need to inspect contact.html (grep output was swallowed by ctx_search — re-grep directly). Probably needs a real form with validation + success state, or it uses mailto/links only.

### MED highlights (fix after HIGH)
- **No clear header logo link on 14 pages**: index, store, contact, privacy, terms, uninstall, track, preview, plans, launch, index-de/es/fr/nl. Family B pages (team, faq, etc.) use `.brand` class with img — those were NOT flagged. Family A (shadcn) header logo structure needs inspection (`grep -n -iE 'logo|brand' index.html` — output got swallowed, re-run).
- **69 console errors**: mostly `__diag` 404s (local-server artifact of a tracking script — check if real pages call `/__diag`), `api.github.com` ERR_FAILED (blocked by audit route-abort — NOT a real prod issue), `i.imgur.com` 403 (real: imgur blocks hotlinking sometimes), `cloudflare email-decode` ERR_FAILED (audit route blocked it — probably fine live).
- **71 failed reqs**: same categories. Real ones to check: imgur 403, `/api/stats` 404 (local only?), `__diag` 404.
- **Missing og:title + og:image**: `/account/index.html`, `/404.html`.
- **`/track.html` has no h1**.
- **`/subdomains/index.html` has 3 h1s** (LOW).

### What's ALREADY GOOD (don't re-fix)
- ✅ **No horizontal overflow** on any page (desktop 1440 + mobile 360) — point 1 done
- ✅ `robots.txt` exists + points to sitemap; `sitemap.xml` 200; custom `404.html` exists — point 16 mostly done
- ✅ Unique `<title>` + meta description on all 16 tested pages; most have og:title — points 14/15 mostly done
- ✅ Privacy + Terms + FAQ + Contact pages exist — point 17 exists (quality review still needed)
- ✅ No lorem ipsum detected anywhere — point 7 largely fine
- ✅ No dead `#` links / empty buttons found by audit
- ✅ All images tested have `alt` (none flagged)

### Audit caveats
- Audit blocked external fonts/imgur/github via route-abort → some console/net errors are **artifacts**, verify live before "fixing".
- Family A = shadcn pages with `mobile-nav.js` + `#public-mobile-navigation` + `assets/mnav.css` (works: index, store, contact, privacy, terms, uninstall, preview, index-de/es/fr/nl).
- Family B = team-style `nav.bar` with inline script (faq, download, team, plans, subdomains, account, 404, track?, launch?).

---

## 2. GSC SUBMISSION (after 20-point pass)

- Target: `sc-domain:isolateclient.in`
- **Blocked earlier by Google-side steps** (may still be blocked — try anyway, user says credentials given):
  1. Enable API: `https://console.developers.google.com/apis/api/searchconsole.googleapis.com/overview?project=824505628703`
  2. Service account `firebase-adminsdk-fbsvc@openphone-bfd7c.iam.gserviceaccount.com` needs `serviceusage.services.enable` + **Owner** on `sc-domain:isolateclient.in`
  - Key file: `/home/raghuwinder/Downloads/openphone-bfd7c-firebase-adminsdk-fbsvc-12573e7b6b.json`
- Script `/tmp/opencode/gsc_submit.py` was LOST — recreate (force IPv4; env has dead IPv6).
- Fallback: paste `https://isolateclient.in/sitemap.xml` in GSC UI — report links, don't ask.
- Search for existing credentials in project (user says "i gave you credential") — check env vars, `.env*`, Downloads, `/tmp/opencode/`.

---

## 3. OTHER PENDING WORK (after active task — only if user says continue)

### A. VM / Win10 QEMU (currently STUCK in WinRE)
- **Root cause found**: OVMF Boot Manager has **NO "Windows Boot Manager" entry** (only UEFI Misc Device/PXE/HTTP/Shell). Disk boots → WinRE because **BCD is damaged** (missing Windows OS entry → no "Continue" tile). Startup Repair fails. Booting "UEFI Misc Device" also lands in WinRE.
- **Next**: reach WinRE **Command Prompt** → `bcdedit`/`bootrec` rebuild BCD; OR offline fix: stop VM, `qemu-nbd -c /dev/nbd0 disk.qcow2`, mount ESP+Windows partition, repair BCD/boot files from host.
- **QEMU gotchas**: managed by systemd `win10-vm.service` (restart with `systemctl --user restart win10-vm.service` after cleaning pidfile+sockets). QEMU crashed repeatedly this session — sockets at `/mnt/Hard Drive_G:/win10-vm/win10-{monitor,qga}.socket`, symlinks `/tmp/opencode/{mon,qga}.sock`. Monitor works (`info status`). `shot.sh`, `ktype.sh`, `qga.sh` recreated in `/tmp/opencode/`.
- **Typing via sendkey is GARBLED** in WinRE (layout/timing) — arrow/Enter keys work. ktype.sh sleep bumped to 0.12s but still garbled. Mouse tablet: `mouse_set 4`, `mouse_move dx dy z` (3 args); absolute coords attempted `9474 8813` didn't map correctly.
- Boot detect: `bash shot.sh out.png` → mean>60 = screen up. Boot ≈ 15s-8min.

### B. New installer READY (waiting for VM to boot Windows)
- CI run `44903116916` SUCCESS on commit `afc8cec`.
- Installer: `/tmp/opencode/vm/i.exe` (2,948,120 B, sha256 `7acb013e3676c470f4ed309c895e5c8871a18e01019e77f85d8ab071be32db27`)
- HTTP server: `python3 -m http.server 8765` in `/tmp/opencode/vm` (was running pid 38746 — verify), guest fetches `http://10.0.2.2:8765/i.exe`.
- After VM boots: install → e2e-test updater UI (force-update modal, AnnouncementBanner, update-stage events) → read guest `~/.isolate/install.log` → root-cause **1.8.9 install bug**.

### C. Firebase updater backend (code committed, not configured)
- `FIREBASE_DB_URL` + `FIREBASE_AUTH` secrets not set. RTDB: `https://isolateclient-default-rtdb.firebaseio.com`.
- Need: push initial `releases/latest` + `releases/announcement`, verify 15s poll → `release-update` event.
- Release tooling committed: `scripts/push-release.mjs`, `docs/RELEASE.md`, `.github/workflows/release-notify.yml`.

### D. Docs to refresh when tasks complete
- `TODO-KAL.md`, `CONTEXT-handoff.md` — update after active task.
- `TODOS-session-compact.md` L28 = verify_nav.mjs suite spec.

---

## 4. SUITE GATE (must stay green)

- `/tmp/opencode/verify_nav.mjs` — **184/184** (79 desktop + 105 mobile), PAGES=16 (incl `/track.html`, NOT 404.html), NO_NAV/NO_FOOT=`{/track.html}`.
- Run: `node /tmp/opencode/verify_nav.mjs` (local) or `node /tmp/opencode/verify_nav.mjs https://isolateclient.in` (live).
- Playwright: `/home/raghuwinder/claude-animation-skills/kit/node_modules/playwright`.
- **After any site change: run suite + `npm run build` before deploy.**
- ⚠️ verify_nav.mjs lives in `/tmp` (wipes daily) — if lost, recover from session index or TODOS L28.

---

## 5. SITE FACTS

- Root: `/mnt/Data Drive/IsolateClient/site/public/` (16 pages, 11MB).
- Deploy: `cd site && npx vercel --prod --yes` (CLI authed as `escapex112233-6657`).
- Commit as `EscapeXMc <escapex112233@users.noreply.github.com>`, prefix `site: ...`.
- HEAD: `afc8cec` pushed (updater+UI work). Prior: `f9882a1`, `96a8cb0`.
- Palette: black `#0a0a0a` + blue `#3b82f6`/`#60a5fa` + `#A3FF12`. Credit line "An isolated client by Shayan & Raghuwinder, built for performance" must survive every restyle. Discord `https://discord.gg/TV2ypJF6TF` untouched.
- Nav: HOME · STORE · FEATURES · FAQ · TEAM · PLANS · DOWNLOAD (Contact/Track/Uninstall/Privacy/Terms footer-only). ≥44px targets, no h-overflow 360/1440.
- Clean URLs (vercel.json cleanUrls), sitemap clean URLs, support mail `support@isolateclient.in`.
- Owner bypass: `/18nov/` → `localStorage.iso_owner="1"`.
- Big files (optimization targets for point 11): `faq-data.json` 1.2MB, `magmad/whoap.png` 571KB, `magmad/banners/*.png` ~400-477KB, `og-image.png` 177KB, `assets/shots/shot-home.jpg` 175KB.
- FAQ title says "3500+ answers" — faq-data.json is huge; check if real.

---

## 6. CONSTRAINTS / DON'TS

- **IsolateClientMod auto-replace feature PAUSED by user** — do not resume without explicit request.
- No guest QGA typing fix yet; never send `quit` to QEMU monitor via shot.sh.
- claude-mem observer auth DOWN (told user; `/login` fix) — do not restart worker.
- No real Adsterra keys (`AD_KEYS` placeholders).
- Don't ask questions — report and continue (user's standing instruction).
- Frontend-design skill installed at `.claude/skills/frontend-design/SKILL.md`; plan MDs: `PLAN-updater-fastclient-ui.md`, `TODO-KAL.md`.

---

## 7. NEXT ACTIONS (ordered)

1. Run `/tmp/opencode/test_famb.mjs` → confirm/deny Family B mobile menu bug; fix only if real.
2. Inspect `contact.html` (re-grep form/mailto) → add real form + validation + success/error states if missing (points 5,6).
3. Fix Family A logo link (point 3) — inspect `index.html` header first.
4. Fix MED items: og tags on account+404, track h1, subdomains h1s, imgur 403 images, real console errors (points 14,15,18,12).
5. Image compression pass (point 11) on big files listed above.
6. Above-fold/CTA + design-system + copy review (points 8,9,10) — careful, restyle must keep suite 184/184.
7. Keyboard/focus/a11y spot-check (point 13); E2E flows desktop/tablet/mobile (point 20).
8. `npm run build` + `verify_nav.mjs` 184/184 → commit `site: ...` → deploy vercel → live suite re-check.
9. GSC submission (recreate `gsc_submit.py`, IPv4-forced; report links).
10. Update this file + TODO-KAL.md.

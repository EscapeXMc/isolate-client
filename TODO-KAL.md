# TODO — Kal ka Plan (21 Nov 2026)

> Ye file kal ke kaam ke liye hai. Har item ke saath: **problem kya hai**, **fix kaise karna hai**.
> Repo HEAD: `f9882a1` (site + client fixes pushed, tree clean)

---

## 🔴 1. BUG: 1.8.9 instance install complete nahi hota (TOP PRIORITY)

**Problem (VM me live reproduce hui):**
- PROFILES page pe 1.8.9 instance hai → **Install** dabaya to install shuru hui aur progress chala
  (`Downloading assets 450/734`, neeche bottom bar bhi aaya).
- Phir wahi screen wapas **"Not installed"** pe reset ho gaya — koi error toast UI pe dikha hi nahi.
- User ne app **kai baar band karke chalaya** — har baar wahi result, install kabhi finish hi nahi hua.
- Instance card pe `client.jar` exist hi nahi karta (Rust me `installed` flag = `client.jar` exists),
  isliye badge hamesha "Not installed" rehta hai.

**Code path (root cause ki taraf):**
- `src-tauri/src/lib.rs`
  - `install_instance` (~L1990) → `std::thread::spawn` fire-and-forget → `install_blocking` → `install_inner`
  - `install_inner` steps: manifest (5%) → **client.jar (20%)** → assets (35%) → libraries (68%) → natives → done
  - Progress event: `install-progress` (`progress()` fn L1966) → frontend `App.tsx:59` → `setInstalling`
  - On error: `progress(..., 0, "Install failed", true, Some(e))` aur `log_msg` sirf **in-memory** hota hai
    (`log_msg` L124 — stdout + `app.log_entries`, **koi file me save nahi**) → app band karo to error gayab.
- `installed` flag: `load_instances_raw` (L591) = `instance_dir(name)/client.jar` exists.

**Suspects (check in this order):**
1. **App band hote hi install thread mar jaati hai** — thread spawn sirf app process ke andar hai,
   app close = install abort = partial files, `client.jar` ka status uncertain.
   → Fix: install ko resume-able banao (dobara start pe already-downloaded files skip karo) ya
   install complete hone tak app close pe warning do.
2. **Download kahin fail ho raha hai** (404/timeout on Mojang assets/libraries) lekin error sirf
   in-memory me jaata hai — UI pe "Install failed" badge (`setInstalling` error case) dikhna chahiye
   tha, dikh nahi raha → ya error event hi nahi aa raha (thread panic?) ya UI refresh me miss ho raha.
   → Fix: **install log ko file me likho** (`~/.isolate/install.log`) — pehla kaam yahi hai, taaki
   agla run me exact error mile.
3. **Client jar download hi fail** — badge "Not installed" ka matlab `client.jar` exists nahi karta,
   yaani assets 450/734 ke baad bhi jar ka status theek nahi (ya `installed` flag refresh nahi hua).
   → Fix: install ke end me `refreshInstances()` call karo (frontend) + jar existence se check.

**Kal ka plan of attack:**
- [ ] Step 1: `install_inner` ke har step ke end pe `log_msg` + **file append** (`~/.isolate/install.log`) add karo.
- [ ] Step 2: `npm run build` → CI dispatch → new installer → VM me install → Install click → log file padho → exact error milega.
- [ ] Step 3: Error ke hisaab se fix (retry logic / resume / URL / panic catch).
- [ ] Step 4: App close pe install-progress ka warning dialog (agar install pending ho).
- [ ] Step 5: Fix ke baad full e2e: install → badge "Ready" → LAUNCH GAME chalu ho → exit hua.

**VM test helpers (re-use karna, yahi hain):**
- `/tmp/opencode/shot.sh <out.png>` — screendump (monitor socket; `quit` mat bhejna, VM kill ho jayega)
- `/tmp/opencode/ktype.sh 'text'` — sendkey typewriter (Win+R commands ke liye)
- Monitor: `printf 'cmd\n' | socat -t 3 - UNIX-CONNECT:/tmp/opencode/mon.sock`
- **Mouse note:** HMP `mouse_move dx dy z` (3-arg) use karo — 2-arg moves kaam nahi karte the.
  `mouse_set 4` = tablet (absolute), `mouse_set 2` = PS/2. Click: `mouse_button 1` then `mouse_button 0`.
  Har move ke baad screenshot se verify karo, coords screen 1280×800 ke hain.
- Guest me cmd kholne ka tareeqa: `sendkey meta_l-r` → `ktype.sh` → `sendkey ret`
  (app ke sidebar coords: HOME 107, PROFILES 187, MODS 265, HOST 344, PARTNERS 423, CHAT 503, ACCOUNTS 582, AI 668 @ x≈97 — screenshot se har baar re-verify)

---

## 🟠 2. Google Search Console — site add + submit

**Current status (blocked on Google side, script ready):**
- Property: `sc-domain:isolateclient.in`
- `/tmp/opencode/gsc_submit.py` ready hai — bas 3 cheezein Google side pending hain:

**Steps (ye report karna hai, user se poochna nahi):**
1. **Search Console API enable karo:**
   https://console.developers.google.com/apis/api/searchconsole.googleapis.com/overview?project=824505628703
   (abhi "enable-API" state me hai)
2. **Service account ko Owner banao** (Search Console UI → Property Settings → Users → Add → Owner):
   `firebase-adminsdk-fbsvc@openphone-bfd7c.iam.gserviceaccount.com`
   Key file: `/home/raghuwinder/Downloads/openphone-bfd7c-firebase-adminsdk-fbsvc-12573e7b6b.json`
   (SA ke paas `serviceusage.services.enable` permission nahi hai — isliye API console se manually enable)
3. Phir submit script chalao: `python3 /tmp/opencode/gsc_submit.py`
   (IPv4 force karke bana hai kyunki is machine pe IPv6 dead hai)

**Fallback (agar API path fail ho):** GSC UI me manually property add karo aur
`https://isolateclient.in/sitemap.xml` sitemap field me paste karo.

**Also verify karna:**
- [ ] DNS TXT verification record (agar domain property verify pending ho)
- [ ] GSC me `https://isolateclient.in` ki Coverage/Indexing report 24h baad check karo
- [ ] Live site pe `robots.txt` + `sitemap.xml` 200 de rahe hain (pehle check ho chuka hai — re-confirm)

---

## 🟢 3. Kaam jo DONE hai (dobara mat karna)

**Site (deployed + pushed):**
- Domain sweep: 21 files → sab links `isolateclient.in` pe (zero `vercel.app` in src/tauri)
- Support mail: `support@isolateclient.in` (Contact + Account page + Settings About row + CF-email sweep)
- Discord link → `https://isolateclient.in/plans/` (DiscordChat.tsx:216)
- Admin link → `https://isolateclient.in` (Admin.tsx:746)
- `site/vercel.json` cleanUrls + explicit rewrites (catch-all removed)
- `/subdomains/` (form + 2-min wait + adbar fix), `/account/` page, 404 page, clean sitemap
- Deployed via `npx vercel --prod --yes` → **live suite 184/184**
- Commits: `96a8cb0` (site) + `f9882a1` (client) — pushed, HEAD `f9882a1`

**Client:**
- CI run `44749377718` success → release v0.1.0 asset updated
  (`IsolateClient_0.1.0_windows64-setup.exe`, sha256 `61a789c71bdf71664e7f…`)
- Installer VM me install hua aur app launch hua (UI renders: HOME/PROFILES/MODS/HOST/PARTNERS/CHAT/ACCOUNTS/AI)
- GSC script ready (`gsc_submit.py`), nav suite gate ready (`verify_nav.mjs`, live URL accept karta hai)

---

## 🟡 4. Baaki pending (priority order)

- [ ] **1.8.9 install bug fix** (upar detail me) — fix → CI → VM me re-test
- [ ] VM me baki UI flows test: ACCOUNTS (login), Settings About (mailto check), Discord JOIN →
      `isolateclient.in/plans/`, partner link, hamburger/menu
- [ ] GSC enable-API + SA Owner steps → `gsc_submit.py` run → sitemap submit confirm
- [ ] `CONTEXT-handoff.md` / `TODOS-session-compact.md` refresh (session outcome ke saath)
- [ ] Adsterra keys agar mil jayein → subdomains page me inject
      (`AD_KEYS` placeholders `PASTE_ADSTERRA_KEY_1/2/3` — sirf inject karna hai jo `PASTE_` se start na ho)
- [ ] IsolateClientMod auto-replace feature (user ne PAUSE kiya tha — bina pooche start mat karna,
      sirf user bole tabhi resume: `src-tauri/src/lib.rs` startup auto-replace ~L12064)

---

## ⚠️ 5. Known issues / environment notes

- **claude-mem outage:** observer credentials refused since 2026-11-13
  ("Claude Code could not authenticate the observer") — `/login` se fix hoga, restart se nahi.
  Abhi koi memory save nahi ho rahi.
- **QGA dead** guest me (guest-agent service installed nahi) aur sshd bhi nahi chal raha →
  saara guest interaction QEMU monitor HMP se (sendkey + mouse).
- QEMU VM 3G RAM; local `cargo-xwin` build **kabhi VM ke saath concurrently mat chalao**
  (7G total RAM limit). CI hi primary build path hai.
- GSC ke liye IPv6 dead hai → Python HTTPS me IPv4 force zaroori.
- Git author: `EscapeXMc <escapex112233@users.noreply.github.com>`,
  commit prefix: `site: ...` / `client: ...` / `fix(...):`
- Palette: black `#0a0a0a` + blue `#3b82f6`/`#60a5fa` + `#A3FF12`
- Credit line **"An isolated client by Shayan & Raghuwinder, built for performance"** har restyle me survive honi chahiye.
- Suite must stay **184/184** (`verify_nav.mjs`).

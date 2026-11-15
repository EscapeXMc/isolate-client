# 10 — Xryloth Server-Side Plugins

Source tree: `Xryloth/XrylothTauri/minecraft-plugins/`
All `file:line` citations below are relative to that directory unless another base is stated.

---

## 1. Overview

### 1.1 What this is

Eight standalone Paper (Bukkit-family) plugins — each its own Maven module, each
shipping as an independent `.jar` in `minecraft-plugins/dist/` (`README.md:26-41`).
There is no shared runtime library: every plugin is self-contained and depends
only on the Paper API (`README.md:51`: *"No database, no external dependencies —
every plugin uses only the Paper API."*).

| Plugin | One-line summary (`README.md:15-22`) |
|---|---|
| HeartVault | Lifesteal SMP core — steal hearts on PvP kills, eliminate at zero hearts |
| SkullHarvest | Headsteal SMP core — drop player heads on PvP kills, consume for power, killstreak broadcasts |
| NexusHub | Network lobby core — hub protection, spawn/join items, BungeeCord server selector |
| AegisGuard | Network moderation — chat filter, mutes, reports, staff channel, watchlist alerts |
| AurumEconomy | Network economy — balances, payments, wealth leaderboard, GUI shop |
| AccountShield | Account integrity — login fingerprinting, alt linking, ban-evasion blocking, join-flood defence |
| Overencumbered | Survival logistics — per-item weights, slows/blocks/damages over-encumbered players |
| MinePie | Discord bridge — embedded Discord bot (no external libs): status embeds, chat relay, console relay |

### 1.2 Shared build conventions (parent POM)

Parent: `pom.xml` (`packaging: pom`, `groupId com.xryloth`, `artifactId
xryloth-plugins`, `version 1.0.0`, name *"Xryloth Plugin Suite (Paper 1.21.11)"*)
— `pom.xml:7-11`.

- **Modules**: exactly the eight plugins, in the order HeartVault, SkullHarvest,
  NexusHub, AegisGuard, AurumEconomy, AccountShield, Overencumbered, MinePie —
  `pom.xml:14-23`.
- **Java version**: `maven.compiler.release = 21` — `pom.xml:27`. README
  confirms *"Requires JDK 21 and Maven"* (`README.md:57`).
- **API version**: `paper.version = 1.21.11-R0.1-SNAPSHOT` — `pom.xml:28`,
  consumed as `io.papermc.paper:paper-api:${paper.version}` with **`provided`
  scope** — `pom.xml:39-46`. Repository: `https://repo.papermc.io/repository/maven-public/`
  — `pom.xml:31-37`.
- **Encoding**: UTF-8 — `pom.xml:26`.
- **Managed plugin versions**: `maven-compiler-plugin` 3.13.0 and
  `maven-jar-plugin` 3.4.2 under `<pluginManagement>` — `pom.xml:48-62`.
- **Shading**: **there is none.** No module POM declares `maven-shade-plugin`,
  `maven-bundle-plugin`, or relocation rules. Child POMs are ~14 lines and only
  set `<parent>` + `<artifactId>` + `<name>` (e.g.
  `AccountShield/pom.xml:6-14`). This matches the "no external dependencies"
  claim — nothing needs bundling.
- **Runtime descriptor**: each plugin declares `api-version: '1.21'`
  (e.g. `AccountShield/src/main/resources/plugin.yml:4`) and `author: Xryloth`
  (`AccountShield/src/main/resources/plugin.yml:5`) with
  `website: https://github.com/EscapeXMc` (`plugin.yml:7`).

**Build command**: `cd minecraft-plugins && mvn package` (`README.md:59-62`);
JARs land in each module's `target/`, and prebuilt copies are committed under
`dist/*.jar` (`README.md:7`, `README.md:28-38`).

### 1.3 Publishing (Modrinth)

`MODRINTH.md` documents the publishing run:

- **Five** of the eight projects were created and submitted for moderation
  under the Modrinth account **FireXDevelopment**, each with a listed 1.0.0
  version — `MODRINTH.md:3-4`:

  | Plugin | Slug | Project ID | URL |
  |---|---|---|---|
  | HeartVault | `heartvault` | `fIOLbYMI` | https://modrinth.com/plugin/heartvault |
  | SkullHarvest | `skullharvest` | `tCPcgSA5` | https://modrinth.com/plugin/skullharvest |
  | NexusHub | `nexushub` | `OKdLurmB` | https://modrinth.com/plugin/nexushub |
  | AegisGuard | `aegisguard-mc` | `qtuftI4r` | https://modrinth.com/plugin/aegisguard-mc |
  | AurumEconomy | `aurumeconomy` | `DUru4Ggb` | https://modrinth.com/plugin/aurumeconomy |

  (`MODRINTH.md:8-14`). `aegisguard` was taken so the slug is `aegisguard-mc`
  while the display title stays **AegisGuard** (`MODRINTH.md:16-17`).
- **AccountShield, Overencumbered and MinePie have no Modrinth project** — they
  are absent from the `MODRINTH.md` project table (`MODRINTH.md:8-14`), though
  `modrinth-assets/` does contain generated `AccountShield-icon.png`,
  `AccountShield-banner.png`, `Overencumbered-*` and banner art, i.e. assets were
  produced for them.
- Per project the author filled in title/slug, summary, long-form body, 512×512
  icon, 1280×480 featured banner, categories, **MIT license**, environment
  (*client unsupported, server required*), donation link and Discord link, plus
  a 1.0.0 version carrying the compiled jar for loaders
  `paper, spigot, bukkit, purpur, folia` across **12 game versions 1.21 → 1.21.11**
  — `MODRINTH.md:19-33`.
- **Account-wide links** applied to **all 19 projects** on the account
  (`MODRINTH.md:35-48`):
  - Discord: `https://discord.com/invite/YCxYeyEzCE` (`MODRINTH.md:41`)
  - Donation: `https://razorpay.me/@kulwantsingh` (`MODRINTH.md:42`)
  - Razorpay is not one of Modrinth's six accepted donation platforms, so it is
    registered under **"Other"** (`MODRINTH.md:50-53`).
- Assets live in `modrinth-assets/` as `<Name>-icon.png` / `<Name>-banner.png`
  (`MODRINTH.md:55-58`).
- Status: all five submitted (`status: processing`, `queued`); license MIT
  because Modrinth's API has no "All-Rights-Reserved" option
  (`MODRINTH.md:60-67`).

Note: `modrinth-assets/icons/` contains banners for **many more plugins** than
these eight (OmniScape, FluxCord, SnowSteal, EnchantMenu, SkripyTpa, FunnyDeaths,
ISnowiAi, SnowAuth, XryPlug-in, MOTD, FluxCopy, Vault SMP, DynamicMotd,
SnowClear) — i.e. it is a shared asset folder for the whole FireXDevelopment
account, not just this suite (see `MODRINTH.md:44-48` for the full 19-project
list).

---

## 2. AccountShield

> *"A network's account-integrity layer that fingerprints every login, links
> suspicious accounts, blocks ban evaders and stops join floods from a single
> range."* — `docs/AccountShield.md:3-4`

### 2.1 Purpose

Alt-detection / ban-evasion / raid-bot defence. It builds a fingerprint for
every joining account (IP, coarse range, client settings, mod channels, locale)
and scores each new login against every account ever seen; high score ⇒
"same person" (`AccountShield/src/main/resources/config.yml:4-9`,
`AccountShield/src/main/java/com/xryloth/accountshield/AccountShield.java:29-35`).
Storage is plain YAML beside the plugin — no database, nothing leaves the
server (`config.yml:9`, `docs/AccountShield.md:15-16`).

### 2.2 Commands & permissions

Declared in `AccountShield/src/main/resources/plugin.yml:8-24`:

| Command | Description | Permission |
|---|---|---|
| `/accountshield <info\|alts\|ban\|unban\|trust\|untrust\|watch\|list\|reload>` | AccountShield administration | `accountshield.staff` (base, `plugin.yml:13`) |
| `/alts <player>` | Show suspected alternate accounts | `accountshield.staff` (`plugin.yml:17`) |

Aliases on the main command: `ashield`, `as`, `shield` (`plugin.yml:12`).

Permissions: `accountshield.staff` (*"View fingerprints, run alt checks and
manage the watchlist"*, `default: op`) and `accountshield.admin` (*"Manage bans,
trust links and configuration"*, `default: op`) — `plugin.yml:18-24`.

Subcommand handling is a `switch` in `ShieldCommand.onCommand`
(`command/ShieldCommand.java:52-120`); the permission checks actually enforced
in code are `accountshield.admin` for `ban` (`:68`), `unban` (`:79`) and
`reload` (`:112`) — `info`, `alts`, `trust`, `untrust`, `watch`, `list` check
no extra permission in code beyond the `plugin.yml` command-level gate.
`/alts` short-circuits to `runAlts` (`:37-44`).

Subcommand behaviour:
- `info` — full record dump: UUID, first/last seen, brand, locale, IPs, ranges,
  fingerprint/channel counts, watchlist flag, trust links, active ban
  (`:126-142`).
- `alts` — ranked matches with score, `[BANNED]` / `[linked]` tags and reason
  lines (`:144-162`); `[linked]` shown when `score >= linkedThreshold` (`:158`).
- `ban <player> [reason]` — creates a `BanRecord` (reason defaults to
  `"Banned by staff"`, `by` = sender name) and broadcasts an alert (`:164-174`).
- `unban <player>` (`:176-180`).
- `trust <a> <b>` — adds a **mutual** `trustedLinks` entry both ways and saves
  (`:182-189`); `untrust` removes both directions (`:191-202`).
- `watch <player>` — toggles `account.watchlisted` (`:204-210`).
- `list` — total account count plus sorted watchlist names (`:212-220`).
- `reload` — `plugin.reload()` (`:111-118`).

Tab completion: subcommand names at arg 1, player names (store + online) at args
2/3 for `info|alts|ban|unban|trust|untrust|watch`, and players for `/alts`
(`:260-286`).

### 2.3 config.yml keys

`AccountShield/src/main/resources/config.yml`:

| Key | Default | Line | Meaning |
|---|---|---|---|
| `prefix` | `&8[&bShield&8] &r` | `:12` | chat prefix for alerts/commands |
| `alert-threshold` | `50` | `:17` | score at/above which results are reported to staff |
| `linked-threshold` | `60` | `:18` | score treated as "same person"; drives ban evasion |
| `weights.ip` | `45` | `:24` | exact same IP |
| `weights.subnet` | `12` | `:25` | same `/24` (v4) or `/64` (v6) |
| `weights.fingerprint` | `30` | `:26` | identical client settings + brand + locale |
| `weights.channels` | `20` | `:27` | matching mod/plugin channel set |
| `weights.name` | `15` | `:28` | Levenshtein-similar username |
| `weights.recent-ip` | `10` | `:29` | other account last seen from the IP recently |
| `weights.locale` | `3` | `:30` | same client locale |
| `subnet.ipv4-prefix` | `24` | `:34` | IPv4 range prefix |
| `subnet.ipv6-prefix` | `64` | `:35` | IPv6 range prefix |
| `name-similarity` | `0.75` | `:36` | 0..1 name similarity cut-off |
| `channel-similarity` | `0.80` | `:37` | 0..1 Jaccard overlap of mod channels |
| `recent-ip-minutes` | `30` | `:38` | "recently active" window |
| `ban-evasion.enabled` | `true` | `:46` | enable evasion enforcement |
| `ban-evasion.action` | `deny` | `:47` | `deny` \| `kick` \| `alert` |
| `ban-evasion.message` | `&cYou are banned from this network.` | `:48` | kick/deny message |
| `respect-vanilla-bans` | `true` | `:51` | also treat Bukkit `/ban` list as bans |
| `raid-protection.enabled` | `true` | `:59` | join-flood defence |
| `raid-protection.window-seconds` | `60` | `:60` | sliding window length |
| `raid-protection.max-joins-per-subnet` | `5` | `:61` | max joins per range per window |
| `raid-protection.action` | `deny` | `:62` | `deny` \| `alert` |
| `raid-protection.message` | `&cToo many connections from your network. Try again shortly.` | `:63` | |
| `alerts.in-game` | `true` | `:69` | broadcast to `accountshield.staff` holders |
| `alerts.console` | `true` | `:70` | log to console |
| `alerts.webhook-url` | `""` | `:72` | optional Discord webhook (blank = off) |
| `alerts.webhook-username` | `AccountShield` | `:74` | webhook display name; body is `{"content": "<prefix> <text>"}` (`:73`) |
| `autosave-minutes` | `5` | `:77` | flush interval; 0 disables |

`Settings.from(FileConfiguration)` maps every one of these into a POJO with
identical defaults (`config/Settings.java:40-78`) and exposes
`weight(key)` with `getOrDefault(key, 0)` (`Settings.java:80-82`).

### 2.4 Architecture

```
AccountShield (main, JavaPlugin)
 ├─ Settings                 config snapshot            config/Settings.java
 ├─ AccountStore             YAML persistence           storage/AccountStore.java
 ├─ DetectionEngine          scoring                    detection/DetectionEngine.java
 ├─ JoinLimiter              sliding-window counter     util/JoinLimiter.java
 ├─ ConnectionListener       events                     listener/ConnectionListener.java
 └─ ShieldCommand            /accountshield + /alts     command/ShieldCommand.java
```

- **`AccountShield`** (main class, `AccountShield.java:36`): `onEnable` loads
  config, creates the store and engine, registers `ConnectionListener`, wires
  `ShieldCommand` as executor+tab-completer for `accountshield` and executor
  only for `alts`, then starts autosave (`:45-65`). `onDisable` saves
  everything (`:67-72`). `reload()` re-reads config, saves and restarts autosave
  (`:74-79`). Autosave cancels all of this plugin's scheduled tasks first, then
  schedules an async repeating `store.saveAccounts()` at
  `autosaveMinutes * 60 * 20` ticks (`:81-87`).
- **`ConnectionListener`** — three phases:
  1. `AsyncPlayerPreLoginEvent` (`:28-71`): raid-protection gate first (records
     the subnet in `JoinLimiter`, disallows with `KICK_OTHER` if flooded and
     `action == deny`, `:39-51`), then ban-evasion by IP/range via
     `findEvasionBan` (disallows with `KICK_BANNED` when `action == deny`,
     `:54-63`), then records ip/subnet/name/lastSeen on the account (`:66-70`).
  2. `PlayerJoinEvent` at `MONITOR` (`:73-82`): watchlist alert if
     `account.watchlisted`, then **schedules `evaluate(player)` 40 ticks (2 s)
     later** with the comment *"Wait for the client to announce its channels
     before scoring"* (`:80-81`).
  3. `PlayerRegisterChannelEvent` at `MONITOR` lowercases and adds each
     announced channel to `account.channels` (`:84-88`); `PlayerQuitEvent`
     refreshes `lastSeen` and saves (`:90-97`).
- **`AccountShield.evaluate(Player)`** (`:148-187`): re-captures the
  fingerprint, runs `engine.evaluate(subject, store.all())`, alerts staff with a
  summary line + one line per match (`:161-165`), then for every match with
  `score >= linkedThreshold` looks up a ban on the *other* account (own list,
  else vanilla list when `respect-vanilla-bans`) and, if found, alerts and
  **kicks the joining player** unless the action is `alert` (`:167-185`). Note
  the enforcement here is a kick (post-join), not a pre-login disallow.
- **`DetectionEngine.compare`** (`detection/DetectionEngine.java:43-96`)
  accumulates weights:
  - shared IP → `ip` weight (`:54-56`) **else** shared subnet → `subnet` weight
    (`:57-60`) — mutually exclusive;
  - any intersecting fingerprint hash → `fingerprint` (`:62-65`);
  - Jaccard overlap of channels `>= channel-similarity` and both sets
    non-empty → `channels` (`:67-71`);
  - `Levenshtein.similarity >= name-similarity` → `name` (`:73-77`);
  - equal non-blank locale → `locale` (`:79-82`);
  - shared subnet **and** `other.lastSeen` within `recent-ip-minutes` →
    `recent-ip` (`:84-89`).
  Score is clamped to 100 (`:91`) and the match is dropped below
  `alert-threshold` (`:92-94`). Trusted pairs short-circuit to `null`
  (`:44-46`, `isTrusted` at `:98-100`). Results are sorted by score descending
  (`:38`).
  `jaccard` is a plain `|A∩B| / |A∪B|` (`:112-121`); empty set → 0.0.
- **`Fingerprint`** (`detection/Fingerprint.java`): `of(brand, locale, options)`
  builds `brand=…;locale=…;` plus each option key/value sorted by `TreeMap`
  (`:19-27`) and SHA-256 hashes it (hex output, `:41-54`); `ofChannels` hashes
  a sorted `|`-joined channel list (`:29-35`) — *currently unused by capture*,
  channels are stored in the clear instead.
- **`Match`** is a record `(Account other, int score, List<String> reasons)`
  with `reasonLine()` joining reasons by `", "` (`detection/Match.java:8-12`).

### 2.5 Data storage

Two YAML files next to the plugin, written **atomically** (temp file then
`Files.move(..., REPLACE_EXISTING)`) — `storage/AccountStore.java:194-203`:

- **`accounts.yml`** — `accounts.<uuid>.{name, first-seen, last-seen, ips[],
  subnets[], fingerprints[], channels[], locale, brand, watchlisted,
  trusted-links[]}` written by `saveAccounts()` (`:151-175`) and read back in
  `loadAccounts()` (`:205-240`, malformed UUID keys logged and skipped at
  `:236-238`).
- **`bans.yml`** — `bans.<key with ':' replaced by '_'>.{uuid, name, reason,
  since, until, by}` (`:177-192`, `:242-269`). Keys are `uuid:<uuid>` or
  `name:<lowercase>` (`model/BanRecord.java:20-22`); `until == 0` means
  permanent (`BanRecord.java:13`, `active()` at `:16-18`).

In-memory maps are `ConcurrentHashMap` (`AccountStore.java:28-29`) because the
async pre-login thread and the main thread both touch them (class Javadoc
`:19-24`). `Account`'s set fields are `ConcurrentHashMap`-backed sets
(`model/Account.java:16-24`).

`findBan` prefers the UUID key, then the lowercased name key, and only returns
active records (`:89-103`). `seenFrom(ip, subnet)` scans all accounts for a
matching IP **or** subnet (`:132-142`).

### 2.6 External integrations

- **Discord webhook (optional)**: `alerts.webhook-url` (`config.yml:72`). The
  poster is `util/Webhook.java` — JDK `java.net.http.HttpClient` only, static
  client with 10 s connect timeout (`:14-16`), POSTed asynchronously on the
  plugin scheduler with a 15 s request timeout (`:36-42`), body
  `{"content": "…"}` optionally with `{"username": …}` (`:33-35`), failures
  logged as warnings and never thrown into the caller (`:44-49`). JSON string
  escaping is hand-rolled including `\u00xx` for control chars (`:53-75`).
- **Vanilla Bukkit ban list**: `AccountShield.vanillaBan(name)` reads
  `BanList.Type.NAME` and converts a `BanEntry` into a `BanRecord`
  (`AccountShield.java:206-226`), swallowing all throwables (`:223-225`).
- No other HTTP endpoints, no database, no BungeeCord/Velocity messaging.

### 2.7 Implementation details worth noting

- `capture()` reads Paper `ClientOption`s: `LOCALE`, `SKIN_PARTS`,
  `CHAT_COLORS_ENABLED`, `CHAT_VISIBILITY`, `MAIN_HAND`,
  `PARTICLE_VISIBILITY`, `ALLOW_SERVER_LISTINGS` and the client brand
  (`AccountShield.java:128-143`), each guarded by a `try/catch` returning `""`
  on failure (`:249-256`).
- `alert()` fans out to console (colour-stripped), in-game staff with the
  configured prefix, and the webhook (`AccountShield.java:230-245`).
- `startAutosave()` calls `cancelTasks(this)` first, so a `reload()` cannot
  stack duplicate timers (`:81-87`).
- `Subnet.of` masks by prefix bits and renders dotted-quad `/n` for IPv4 and
  hextet `/n` for IPv6, clamping prefixes to 8..32 and 16..128
  (`util/Subnet.java:11-46`).
- `Levenshtein.similarity` = `1 - distance/maxLen` after lowercasing; both
  empty ⇒ 1.0 (`util/Levenshtein.java:32-43`).
- `JoinLimiter.record` prunes expired timestamps then returns
  `deque.size() > max` — i.e. it fires **after** the max-th join inside the
  window (`util/JoinLimiter.java:14-25`). Keys are subnet strings; entries are
  never evicted when idle (only on `clear()`, `:27-29`), so the map grows with
  distinct ranges seen.
- The docs are candid about limits: heuristic detection, VPN/fresh-install
  resets, shared-router false positives, and **per-server data** unless every
  backend points at the same file — *"or an upcoming shared-storage mode"* —
  (`docs/AccountShield.md:114-122`).

---

## 3. AegisGuard

> *"A network moderation suite that filters chat, mutes players, collects
> reports, runs a staff channel, and alerts on watchlisted logins."* —
> `docs/AegisGuard.md:3`

### 3.1 Purpose

Moderation backbone: chat filter, restart-surviving mutes, player reports, a
private staff channel, a watchlist with join alerts, and a quick kick
(`AegisGuard/src/main/java/com/xryloth/aegisguard/AegisGuard.java:33-38`,
`docs/AegisGuard.md:9-15`). Everything routes to players holding `aegis.staff`
(`docs/AegisGuard.md:14-15`).

**Single-class design**: `AegisGuard` itself extends `JavaPlugin` **and**
implements `Listener` and `CommandExecutor` (`AegisGuard.java:39`) — there are
no sub-packages at all (the module contains exactly one `.java` file).

### 3.2 Commands & permissions

From `AegisGuard/src/main/resources/plugin.yml:8-46`:

| Command | Usage | Permission (plugin.yml) | Aliases |
|---|---|---|---|
| `/report` | `/report <player> <reason>` | none declared (`plugin.yml:9-11`) | — |
| `/reports` | `/reports` | none declared in yml; **checked in code** (`AegisGuard.java:222`) | — |
| `/mute` | `/mute <player> <minutes\|perm> [reason]` | `aegis.staff` (`:18`) | — |
| `/unmute` | `/unmute <player>` | `aegis.staff` (`:22`) | — |
| `/kick` | `/kick <player> [reason]` | `aegis.staff` (`:26`) | — |
| `/sc` | `/sc <message>` | `aegis.staff` (`:30`) | `staffchat` (`:31`) |
| `/watch` | `/watch <player>` | `aegis.staff` (`:35`) | — |
| `/aegis` | `/aegis reload` | none declared in yml; `aegis.admin` checked in code (`AegisGuard.java:347`) | `ag` (`:39`) |

Permissions block: `aegis.staff` (*"Access staff tools (mute, kick, reports,
staff chat)"*, `default: op`) and `aegis.admin` (*"Manage AegisGuard
configuration"*, `default: op`) — `plugin.yml:40-46`.

All eight commands share **one** executor wired in a loop
(`AegisGuard.java:63-67`) and dispatch on `command.getName().toLowerCase()`
inside `switch (name)` (`:202-359`); the `default` branch is the `/aegis`
handler (`:344-358`).

Behaviour:
- `report` — requires ≥2 args; prepends a formatted line to an in-memory
  `LinkedList` capped at `max-reports` (oldest trimmed from the tail),
  confirms to the reporter and alerts staff (`:205-220`, cap logic `:214-216`).
- `reports` — `aegis.staff` gate in code; shows the **latest 10** entries
  (`:221-236`, `Math.min(10, reports.size())` at `:231`).
- `mute` — target must be **online** (`:246-250`); duration is minutes or
  `perm`/`permanent` → `PERMANENT = Long.MAX_VALUE` (`:41`, `:252-261`);
  reason defaults to `"No reason given"`; saves `mutes.yml` immediately
  (`:265-267`); notifies both parties (`:268-269`).
- `unmute` — **online-only**; if the player is offline the command refuses
  (`:281-286`), so offline players cannot be unmuted.
- `kick` — online-only; kicks with an Adventure `Component` in red, reason
  default `"Kicked by staff"`, then alerts staff (`:293-313`).
- `sc` — posts `&8[&cStaff&8] &f<name>&7: &f<message>` to `alertStaff`
  (`:314-325`).
- `watch` — case-insensitive toggle of a `HashSet<String>` (`:326-343`).
- `aegis reload` — `reloadConfig()` + `loadConfigValues()` under `aegis.admin`
  (`:346-355`); without `reload` the command prints a help line (`:356`).

### 3.3 config.yml keys

`AegisGuard/src/main/resources/config.yml`:

| Key | Default | Line |
|---|---|---|
| `prefix` | `&8[&cAegis&8] &r` | `:3` |
| `max-reports` | `50` ("How many reports to keep in memory") | `:4` |
| `chat-filter.enabled` | `true` | `:7` |
| `chat-filter.blocked-words` | `["ez noob", "kys", "badword"]` | `:9-12` |

Loaded in `loadConfigValues()` (`AegisGuard.java:76-85`); blocked words are
**lowercased on load** for case-insensitive matching (`:82-84`).

### 3.4 Architecture / event flow

```
AegisGuard (JavaPlugin + Listener + CommandExecutor)
 ├─ loadConfigValues / loadMutes / saveMutes
 ├─ onChat   AsyncPlayerChatEvent @LOW   mute gate → word filter
 ├─ onJoin   PlayerJoinEvent              watchlist alert
 └─ onCommand switch over 8 command names
```

- `onEnable` saves default config, loads config + mutes, registers itself as
  listener and as executor for all 8 commands (`:57-69`).
- `onDisable` writes mutes only (`:71-74`) — reports/watchlist are **not**
  saved.
- **Chat pipeline** (`onChat` at `EventPriority.LOW`, `:166-190`): if muted,
  cancel the event and message the player with the remaining time + reason
  (`:170-176`); else if the filter is enabled, test the lowercased message
  against each blocked word with `contains` (`:181-189`) — cancel, tell the
  player *"That language is not allowed here."* and `alertStaffSync` the
  attempt (`:184-187`).
- `alertStaff` builds a **colour-stripped** `Component.text(...)` and sends it
  to every online player with `aegis.staff`, plus a `[Staff] ` console line
  (`:152-160`); `alertStaffSync` hops to the main thread first (`:162-164`) —
  used from the async chat handler.
- `isMuted(uuid)` treats `PERMANENT` or a future `until` as muted, and lazily
  expires entries (removing them and re-saving `mutes.yml`) when the time
  passes (`:130-142`); `remaining(...)` renders `"permanently"` or
  `"<n> minute(s)"` with a floor of 1 minute (`:144-150`).

### 3.5 Data storage

Only mutes persist: `plugins/AegisGuard/mutes.yml`, created on first run
(`:87-97`) with shape

```yaml
mutes:
  <uuid>:
    until: <epoch-ms or Long.MAX_VALUE>
    reason: <string>
```

Written by `saveMutes()` (clears the `mutes` root then rewrites every entry,
`:113-128`) and read by `loadMutes()` (`:87-111`, malformed UUID keys skipped at
`:107-109`). Saves are triggered on mute/unmute and on disable (`:267`, `:289`,
`:73`).

**In-memory only (lost on restart):** the report list (`:47`) and the watchlist
(`:48`) — neither is read from or written to disk anywhere in the file.

### 3.6 External integrations

**None.** No webhooks, no HTTP, no database, no messaging. Alerts go to
in-game staff + console only (`:152-160`).

### 3.7 Details / gotchas

- Uses the **legacy `AsyncPlayerChatEvent`** (`:17`, `:167`), not Paper's
  `AsyncChatEvent` — this event is deprecated on modern Paper.
- Colour handling is `ChatColor.translateAlternateColorCodes('&', …)` +
  `ChatColor.stripColor` (`:362-368`), unlike AccountShield's Adventure
  `LegacyComponentSerializer`.
- `/mute` and `/unmute` require the target to be **online** (`:246-250`,
  `:281-286`) despite `docs/AegisGuard.md` describing mutes that "persist
  across restarts" (`docs/AegisGuard.md:20-21`).
- `max-reports` only trims when a new report is added (`:214-216`).
- Config comments state blocked-words are "Case-insensitive substrings that
  will block a chat message" (`config.yml:8`) — matching is plain `String.contains`,
  so short words like `kys` will also match inside unrelated words.

---

## 4. AurumEconomy

> *"A lightweight network economy that gives players persistent balances,
> payments, a wealth leaderboard, and a configurable GUI shop."* —
> `docs/AurumEconomy.md:3`

### 4.1 Purpose

Currency for a server/network: starting balance, `/pay`, `/baltop`, admin
`/eco give|take|set`, and a chest GUI shop defined purely in `config.yml`
(`docs/AurumEconomy.md:9-16`). Balances are per-UUID and saved to disk — *"no
database required"* (`docs/AurumEconomy.md:11-12`).

Also a **single-class** plugin: `AurumEconomy` implements `Listener` and
`CommandExecutor` directly (`AurumEconomy/src/main/java/com/xryloth/aurumeconomy/AurumEconomy.java:38`),
one file in the module.

### 4.2 Commands & permissions

`AurumEconomy/src/main/resources/plugin.yml:8-30`:

| Command | Usage | Permission | Aliases |
|---|---|---|---|
| `/balance` | `/balance [player]` | none | `bal`, `money` (`:12`) |
| `/pay` | `/pay <player> <amount>` | none | — |
| `/baltop` | `/baltop` | none | `balancetop`, `moneytop` (`:19`) |
| `/eco` | `/eco <give\|take\|set> <player> <amount>` | `aurum.admin` (`:23`) | — |
| `/shop` | `/shop` | none | — |

Permission `aurum.admin` — *"Manage player balances"*, `default: op`
(`plugin.yml:27-30`).

All five commands share one executor registered in a loop
(`AurumEconomy.java:59-63`) with `switch (command.getName().toLowerCase())`
(`:234-347`); the `default` branch is `/balance` (`:327-345`).

Behaviour:
- `pay` — player-only (`:238-241`), target must be online and not self
  (`:246-254`), amount parsed by `parseAmount` (> 0, else error → `-1`)
  (`:349-361`, guard at `:256-258`), funds checked then moved, both parties
  messaged (`:259-269`).
- `baltop` — sorts a copy of the balance map descending, prints the **top 10**
  with `#n`, display name and formatted money (`:271-282`).
- `eco` — `aurum.admin` gate (`:284-287`); `give` adds, `take` subtracts,
  `set` assigns; unknown action rejected (`:302-310`); `setBalance` clamps to
  `>= 0.0` (`:149-152`).
- `shop` — player-only; refuses if `shopItems` is empty (`:315-326`).
- `balance [player]` — console must name a player (`:336-341`).

### 4.3 config.yml keys

`AurumEconomy/src/main/resources/config.yml`:

| Key | Default | Line |
|---|---|---|
| `starting-balance` | `100.0` | `:3` |
| `currency-symbol` | `$` | `:4` |
| `prefix` | `&6[Aurum] &r` | `:5` |
| `shop.title` | `&6Server Shop` | `:8` |
| `shop.items[]` | 4 sample entries | `:9-33` |

Each `shop.items` map entry supports `slot`, `material`, `name`, `amount`,
`price`, `lore[]` — parsed in `loadShop()` (`:80-108`): unknown material falls
back to `STONE` (`:85-88`), `slot < 0` entries are skipped (`:89-92`), missing
`amount` → 1, missing `price` → 10.0, missing `name` → `material.name()`
(`:100-106`). Defaults are also read in `loadConfigValues()` (`:72-78`).

### 4.4 Architecture

```
AurumEconomy (JavaPlugin + Listener + CommandExecutor)
 ├─ balances   Map<UUID,Double>     ←→ balances.yml
 ├─ shopItems  List<ShopEntry>      ← config.yml shop.items
 ├─ openShop / onClick              GUI (ShopMenu InventoryHolder)
 └─ onCommand  balance|pay|baltop|eco|shop
```

- `onEnable`: config → data → register listener + 5 commands, log loaded
  account count (`:53-65`); `onDisable` → `saveData()` (`:67-70`).
- `onJoin` caches the player's name for `/baltop` and seeds
  `startingBalance` for brand-new UUIDs, saving immediately (`:158-166`).
- **Shop GUI**: `openShop` builds a 27-slot inventory owned by a
  `ShopMenu implements InventoryHolder` marker (`:168-194`, class at
  `:411-427`); each entry gets display name, `&7Price:`, `&7Amount:`, custom
  lore and `&aClick to buy` (`:180-188`). `onClick` identifies the shop by
  `event.getInventory().getHolder() instanceof ShopMenu` (`:206-208`),
  cancels every click, resolves `rawSlot → ShopEntry` (`:213-216`), checks
  funds (`:217-221`), deducts and `addItem`s, dropping any `leftover` at the
  player's feet (`:222-227`), then confirms with the new balance (`:228-229`).
- `balance(id)` lazily materialises `startingBalance` via `computeIfAbsent`
  (`:145-147`); every mutation goes through `setBalance`, which persists
  immediately (`:149-152`).
- Formatting: `DecimalFormat("#,##0.00")` prefixed by `currency-symbol`
  (`:42`, `:154-156`).
- `displayName(UUID)` prefers the online-name cache, else
  `Bukkit.getOfflinePlayer(id).getName()`, else the first 8 chars of the UUID
  (`:363-370`).

### 4.5 Data storage

`plugins/AurumEconomy/balances.yml`, shape written by `saveData()`:

```yaml
balances:
  <uuid>: <double>
```

(`AurumEconomy.java:130-143`, path set at `:136`).

> **Load/save path mismatch (gotcha):** `loadData()` iterates
> `data.getKeys(false)` — the **root** keys — and calls `UUID.fromString(key)`
> on each (`:121-127`). The only root key a saved file has is `balances`
> (`:136`), which is not a valid UUID, so it is swallowed by the
> `IllegalArgumentException` handler at `:124-126`. Nothing in the file reads
> the `balances.` sub-section — contrast AegisGuard, which reads its section
> correctly via `getConfigurationSection("mutes")` (`AegisGuard.java:98`).
> Flagging this for verification against the shipped
> `dist/AurumEconomy-1.0.0.jar`: as written, saved balances appear not to be
> reloaded on restart.

### 4.6 External integrations

**None.** No webhooks, HTTP, database, Vault/Economy API, or chat plugins.

### 4.7 Details / gotchas

- **No Vault integration** — this is a bespoke economy, not a `net.milkbowl.vault`
  provider; nothing in the codebase references Vault.
- `parseAmount` rejects `<= 0` and non-numeric input (`:349-361`), but `/eco`
  `take` can be offset by a negative result only through clamping in
  `setBalance` (`:150`).
- `/pay` and `/baltop` have **no permission** nodes (`plugin.yml:13-19`), so
  they are usable by everyone by default.
- `InventoryClickEvent` handling cancels the event before any other check
  (`:209`), so nothing can be taken out of the shop GUI.
- Shop inventory size is hard-coded to 27 (one row set of 3×9) (`:170`), so
  configured slots must be `0..26` — enforced at `:173-175`.

---

## 5. HeartVault

> *"A lifesteal SMP core that lets players steal hearts on PvP kills and
> eliminates anyone who reaches zero hearts."* — `docs/HeartVault.md:3`

### 5.1 Purpose

Lifesteal: each player has a heart count persisted per UUID; a PvP kill moves
`kill-reward` hearts from victim to killer; reaching zero with `ban-on-zero`
enabled **bans** the player until an admin `/revive`s them
(`HeartVault/src/main/java/com/xryloth/heartvault/HeartVault.java:30-35`,
`docs/HeartVault.md:9-12`). Hearts are backed by the real `MAX_HEALTH`
attribute, so hearts are actual HP rather than a scoreboard trick
(`docs/HeartVault.md:14-15`).

Single-class plugin: `HeartVault` implements `Listener, CommandExecutor`
(`HeartVault.java:36`).

### 5.2 Commands & permissions

`HeartVault/src/main/resources/plugin.yml:8-26`:

| Command | Usage | Permission | Aliases |
|---|---|---|---|
| `/hearts` | `/hearts [player\|set <player> <amount>]` | none for self; `heartvault.admin` for `set` (code `:234`) | `heart`, `hp` (`plugin.yml:12`) |
| `/revive` | `/revive <player>` | `heartvault.admin` (code `:195`) | — |
| `/heartvault` | `/heartvault reload` | `heartvault.admin` (code `:219`) | `hv` (`plugin.yml:19`) |

Permissions: `heartvault.admin` (*"Manage hearts, revive players, reload
config"*, `default: op`) and `heartvault.check.others` (*"Check other players'
hearts"*, `default: true`) — `plugin.yml:20-26`.

Executor registered for `hearts`, `revive`, `heartvault` in a loop
(`HeartVault.java:58-62`); dispatch by command name (`:191-277`).

- `revive <player>` — online target only (`:203-207`); sets hearts to
  `startingHearts`, **pardons the vanilla ban** (`Bukkit.getBanList(...).pardon`,
  `:209`), re-applies health, saves, and messages both parties (`:208-215`).
- `hearts set <player> <amount>` — `heartvault.admin`, online target, integer
  parse with a friendly error (`:233-256`).
- `hearts [player]` — shows a count; console must name a player (`:259-274`).
  Note: **no `heartvault.check.others` check exists in code** even though
  `plugin.yml:24-26` declares it (default `true`, so it is effectively
  decorative).

### 5.3 config.yml keys

`HeartVault/src/main/resources/config.yml:4-12`, read in
`loadConfigValues()` (`HeartVault.java:71-82`):

| Key | Default | Meaning |
|---|---|---|
| `max-hearts` | `10` | hard ceiling (`config.yml:4`) |
| `min-hearts` | `1` | floor when `ban-on-zero` is false (`:5`) |
| `starting-hearts` | `10` | new players (`:6`) |
| `health-per-heart` | `2` | MC HP per heart (`:7`) |
| `kill-reward` | `1` | hearts gained per PvP kill (`:8`) |
| `death-penalty` | `1` | hearts lost per death (`:9`) |
| `ban-on-zero` | `true` | eliminate at 0 (`:10`) |
| `ban-message` | `&cYou have been eliminated from the SMP (0 hearts). Use /revive to return.` | (`:11`) |
| `prefix` | `&6[HeartVault] &r` | (`:12`) |

Code defaults live at `HeartVault.java:73-81` (the `ban-message` code default
omits the `Use /revive to return.` suffix present in `config.yml:11`).

### 5.4 Architecture / event flow

```
HeartVault (JavaPlugin + Listener + CommandExecutor)
 ├─ hearts Map<UUID,Integer> ←→ hearts.yml
 ├─ onPlayerDeath @MONITOR   transfer hearts → maybe ban
 ├─ onJoin / onRespawn       re-apply MAX_HEALTH
 └─ onCommand hearts|revive|heartvault
```

- `onEnable` → config → data → register listener + 3 commands, logs starting
  hearts (`:52-64`); `onDisable` → `saveData()` (`:66-69`).
- **`floor()`** returns `0` when `ban-on-zero` is true, otherwise
  `max(1, minHearts)` (`:118-120`); `setHearts` clamps into
  `[floor(), maxHearts]` (`:126-129`), so a victim can never go below 0 while
  elimination still triggers at `<= 0` (`:158`).
- **`applyHealth`** sets `Attribute.MAX_HEALTH` base value to
  `hearts * healthPerHeart`, force-dies at `max <= 0` and clamps current health
  down when it exceeds the new max (`:131-143`).
- **`onPlayerDeath`** (`MONITOR`, `ignoreCancelled`, `:145-174`): requires a
  distinct killer (`:149-151`); adds `killReward` to the killer and subtracts
  `deathPenalty` from the victim, then re-applies the killer's health
  (`:153-155`). If the victim is now `<= 0` and `ban-on-zero`: adds a vanilla
  NAME ban whose reason is the colour-stripped `ban-message` with source
  `"HeartVault"` (`:160`), stores 0 hearts (`:161`), kicks on the main thread
  in red (`:162-163`) and broadcasts
  `&4&l[HeartVault] &c<name> was eliminated by <killer> (0 hearts).`
  (`:164-165`). Otherwise it re-applies the victim's health and tells them how
  many hearts remain (`:166-170`). The killer is always told their new total
  (`:171-172`), and `saveData()` runs at the end of every such death (`:173`).
- `onJoin` seeds `startingHearts` with `putIfAbsent` and applies health
  (`:176-181`); `onRespawn` re-applies health next tick (`:183-187`).

### 5.5 Data storage

`plugins/HeartVault/hearts.yml` — flat root-level mapping
`<uuid>: <int>`: `saveData()` writes `data.set(uuid.toString(), value)`
(`:104-116`, path at `:109`) and `loadData()` iterates root keys with
`UUID.fromString(key)` (`:84-102`). Save and load use the **same** root level,
so unlike AurumEconomy this round-trips. Files are saved on death, revive,
`hearts set` and disable.

### 5.6 External integrations

- **Vanilla Bukkit ban list only**: `BanList.Type.NAME.addBan(...)` for
  elimination (`:160`) and `.pardon(...)` on revive (`:209`).
- No webhooks, HTTP, database, or cross-server messaging.

### 5.7 Details / gotchas

- Ban entries created by HeartVault carry source `"HeartVault"` (`:160`).
- `/revive` requires the target to be **online** (`:203-207`) while the
  `pardon` call only happens afterwards (`:209`); since an eliminated player is
  vanilla-banned they cannot rejoin, so reviving them via this command only
  works if an admin pardons the ban first (or the ban is otherwise bypassed).
- `Attribute.MAX_HEALTH` is read via `player.getAttribute(Attribute.MAX_HEALTH)`
  and mutated with `setBaseValue` (`:134-136`) — a Paper/Bukkit attribute API
  call rather than potion-effect trickery.
- Docs list `/heartvault reload` with `heartvault.admin`
  (`docs/HeartVault.md:34`) — matches code `:219`.

---

## 6. SkullHarvest

> *"A headsteal SMP core that drops player heads on PvP kills, lets players
> consume them for power, and broadcasts killstreaks."* —
> `docs/SkullHarvest.md:3`

### 6.1 Purpose

Headsteal: every PvP kill drops the victim's head as a `PLAYER_HEAD` tagged
with a `PersistentDataContainer` key so only genuine harvested heads are
consumable (`docs/SkullHarvest.md:9-15`); eating one heals and optionally grants
Absorption and permanently increments a "heads eaten" counter; killstreaks are
tracked live and broadcast at milestones (`docs/SkullHarvest.md:9-12`).

Single-class plugin: `SkullHarvest` implements `Listener, CommandExecutor`
(`SkullHarvest/src/main/java/com/xryloth/skullharvest/SkullHarvest.java:47`).

### 6.2 Commands & permissions

`SkullHarvest/src/main/resources/plugin.yml:8-24`:

| Command | Usage | Permission | Aliases |
|---|---|---|---|
| `/heads` | `/heads [player]` | none | `headcount` (`plugin.yml:12`) |
| `/killstreak` | `/killstreak [player]` | none | `ks`, `streak` (`plugin.yml:16`) |
| `/skullharvest` | `/skullharvest reload` | `skullharvest.admin` (code `:238`) | `sh` (`plugin.yml:20`) |

Permission `skullharvest.admin` — *"Reload config and manage streaks"*,
`default: op` (`plugin.yml:21-24`).

Executor loop registers `heads`, `killstreak`, `skullharvest`
(`:69-73`); dispatch by name (`:211-257`):
- `heads [player]` — target defaults to the sender; `"Player not found."` if
  the lookup fails (`:214-224`).
- `killstreak [player]` — same shape, prints `killstreaks` (`:225-235`).
- `default` (`/skullharvest`) — `reload` subcommand under
  `skullharvest.admin` (`:237-251`). **Hidden extra form**: if invoked as
  `/skullharvest reload reset <player>` (i.e. `args[1] == "reset"` and an
  online `args[2]`), it also zeroes that player's killstreak before reloading
  (`:242-247`). This is the "manage streaks" half of the permission
  description; it is not documented in `docs/SkullHarvest.md:27-33` nor in the
  `usage:` line of `plugin.yml:19`. Otherwise it prints the help line (`:253`).

### 6.3 config.yml keys

`SkullHarvest/src/main/resources/config.yml:3-16`, read in
`loadConfigValues()` (`:82-95`):

| Key | Default | Line |
|---|---|---|
| `drop-head-on-kill` | `true` | `config.yml:3` |
| `consume.heal` | `6.0` | `:6` |
| `consume.absorption` | `true` | `:7` |
| `killstreak-milestones` | `[3, 5, 10, 25]` | `:10-14` |
| `prefix` | `&4[SkullHarvest] &r` | `:16` |

If the milestone list is empty in config it falls back to `List.of(3, 5, 10, 25)`
(`:92-94`).

### 6.4 Architecture / event flow

```
SkullHarvest (JavaPlugin + Listener + CommandExecutor)
 ├─ headKey          NamespacedKey(this, "harvest_head")   :64
 ├─ killstreaks      Map<UUID,Integer>  (in-memory only)
 ├─ headsEaten       Map<UUID,Integer>  ←→ data.yml
 ├─ onPlayerDeath    streak bookkeeping + head drop
 ├─ onInteract       consume tagged head
 └─ onCommand        heads|killstreak|skullharvest
```

- `onEnable`: creates the namespaced key, saves default config, loads config +
  data, registers listener and 3 commands (`:62-75`); `onDisable` saves data
  (`:77-80`).
- **Head crafting** (`createHead`, `:131-148`): `PLAYER_HEAD` with
  `SkullMeta.setOwningPlayer(victim)`, display name `"<name>'s Head"` in red
  with `ITALIC` explicitly disabled, two lore lines
  (`"A trophy torn from battle."` / `"Right-click to consume."`), and the
  victim UUID stored under `headKey` as `PersistentDataType.STRING` (`:143-144`).
  `isHarvestHead` checks type + PDC presence only (`:150-156`), which is what
  makes shop-bought heads inert (`docs/SkullHarvest.md:14-15`).
- **`onPlayerDeath`** (`MONITOR`, `ignoreCancelled`, `:158-175`): when there is
  a distinct killer it resets the victim's streak to 0 and `merge`s +1 onto the
  killer's, broadcasting `"<prefix><killer> &7is on a &c<n> &7killstreak!"` when
  the streak is in `milestones` (`:163-169`). Head drop appends
  `createHead(victim)` to `event.getDrops()` when `dropHead` is on (`:172-174`).
- **`onInteract`** (`:177-207`): only `RIGHT_CLICK_AIR` / `RIGHT_CLICK_BLOCK`
  (`:179-181`) with a tagged head in the **main hand** (`:183-186`); cancels the
  event (`:187`), decrements or removes the stack (`:189-193`), heals to at most
  current max health (`:195-199`), optionally applies
  `PotionEffect(ABSORPTION, 20*60 ticks = 60 s, amplifier 1)` (`:200-202`),
  increments `headsEaten` via `merge`, saves and reports the running total
  (`:204-206`).

### 6.5 Data storage

`plugins/SkullHarvest/data.yml` — flat root mapping `<uuid>: <int>` of heads
eaten: `saveData()` writes `data.set(uuid, n)` (`:117-129`, path `:122`) and
`loadData()` iterates root keys with `UUID.fromString` (`:97-115`). Saved on
each consume and on disable.

**Killstreaks are never persisted** — `killstreaks` (`:50`) is only mutated in
`onPlayerDeath` and the hidden `reset`, and is not written to `data.yml`
(`saveData` only iterates `headsEaten`, `:121-123`), so all streaks reset on
restart.

### 6.6 External integrations

**None.** No webhooks, HTTP, database, or proxy messaging.

### 6.7 Details / gotchas

- `event.setCancelled(true)` on the consume interaction (`:187`) prevents the
  usual "eat food / place block" behaviour of the right-click.
- The stack is consumed **before** the heal is applied (`:189-193` then
  `:195-199`), so the item is gone even if the player is already at full
  health.
- Absorption is fixed at amplifier 1 for 60 seconds — not configurable beyond
  the boolean (`config.yml:7`, `:200-202`).
- Colour helper is `ChatColor.translateAlternateColorCodes` only; there is no
  `stripColors` helper here (`:259-261`).
- `docs/SkullHarvest.md:39` lists the permission as granting *"Reload config and
  reset streaks"* while `plugin.yml:23` says *"Reload config and manage
  streaks"* — both refer to the undocumented `reload reset <player>` path.

---

## 7. NexusHub

> *"A network lobby core that protects the hub, manages spawn and join items,
> and gives players a BungeeCord server selector."* — `docs/NexusHub.md:3`

### 7.1 Purpose

Runs the network's hub world: spawn teleports + `/setspawn`, join items, a
GUI server selector that connects through the proxy using BungeeCord plugin
messaging, void rescue, a welcome title and full lobby protection
(`NexusHub/src/main/java/com/xryloth/nexushub/NexusHub.java:48-53`,
`docs/NexusHub.md:9-26`). Works without a proxy — protection/spawn still work,
the selector just has nothing to connect to (`docs/NexusHub.md:14-15`).

Single-class plugin: `NexusHub implements Listener, CommandExecutor`
(`NexusHub.java:54`).

### 7.2 Commands & permissions

`NexusHub/src/main/resources/plugin.yml:8-30`:

| Command | Usage | Permission | Aliases |
|---|---|---|---|
| `/hub` | `/hub` | none | `lobby` (`plugin.yml:12`) |
| `/spawn` | `/spawn` | none | — |
| `/setspawn` | `/setspawn` | `nexushub.admin` (`plugin.yml:19`, re-checked in code `:364`) | — |
| `/nexushub` | `/nexushub reload` | `nexushub.admin` (code `:380`) | `nh` (`plugin.yml:23`) |

Permissions: `nexushub.admin` (*"Manage hub config and spawn"*, `default: op`)
and `nexushub.bypass` (*"Bypass hub protection"*, `default: op`) —
`plugin.yml:24-30`.

Executor loop registers `hub`, `spawn`, `setspawn`, `nexushub`
(`:84-88`). Command flow (`:359-406`):
- `setspawn` → admin gate, player-only, stores `player.getLocation()` and
  `saveSpawn()` (`:363-376`).
- `nexushub reload` → admin gate, `reloadConfig()` + `loadConfigValues()`
  (`:378-388`); otherwise help text (`:389`).
- **`/hub` and `/spawn` share the same code path** (the trailing generic
  branch, `:393-405`): player-only, refuses if `spawn == null` with
  *"No hub spawn has been set yet."*, teleports to spawn, says
  *"Welcome to the hub!"* and calls `giveJoinItems`.

### 7.3 config.yml keys

`NexusHub/src/main/resources/config.yml`, read in `loadConfigValues()`
(`NexusHub.java:92-146`):

| Key | Default | Line / code |
|---|---|---|
| `hub-worlds` | `[hub]` (falls back to `["world"]` if empty) | `config.yml:4-5`, code `:94-98` |
| `protect` | `true` | `config.yml:7`, code `:99` |
| `teleport-on-join` | `true` | `:8`, `:100` |
| `void-teleport` | `true` | `:9`, `:101` |
| `void-y` | `0.0` | `:10`, `:102` |
| `prefix` | `&b[NexusHub] &r` | `:12`, `:103` |
| `selector.title` | `&8Server Selector` | `:16`, `:107` |
| `selector.slot` | `4` (hotbar slot 0-8) | `:17`, `:108` |
| `selector.material` | `COMPASS` (falls back to `COMPASS` on bad name) | `:18`, `:109-112` |
| `selector.name` | `&b&lServer Selector` | `:19`, `:113` |
| `selector.lore` | `["&7Right-click to choose a server"]` | `:20-21`, `:114` |
| `selector.servers.<key>.slot` | required (`< 0` entries skipped) | `:25`, `:133-136` |
| `… .display` | falls back to the section key | `:138` |
| `… .material` | falls back to `PAPER` | `:139-140` |
| `… .lore` | optional list | `:141` |
| `… .bungee` | `survival` / `skyblock` / `pvp` in the sample | `config.yml:28,35,42` — **never read by the code** (see 7.7) |

The three sample servers are `survival` (slot 11, `GRASS_BLOCK`), `skyblock`
(slot 13, `OAK_SAPLING`) and `pvp` (slot 15, `NETHERITE_SWORD`) —
`config.yml:23-44`.

`spawn` is also stored **inside `config.yml`** as
`spawn.{world,x,y,z,yaw,pitch}` — written by `saveSpawn()` + `saveConfig()`
(`:165-178`) and read by `loadLocation(c, "spawn")` (`:145`, `:148-163`;
returns `null` when the world name is missing or the world is not loaded).

### 7.4 Architecture / event flow

```
NexusHub (JavaPlugin + Listener + CommandExecutor)
 ├─ config-derived: hubWorlds, selector*, spawn
 ├─ onJoin      @+10 ticks: teleport-to-spawn, giveJoinItems, welcome title
 ├─ onInteract  selector item → openSelector (HubMenu GUI)
 ├─ onClick     HubMenu → BungeeCord "Connect" plugin message
 ├─ onMove      void rescue below void-y
 └─ protection: onBreak/onPlace/onDamage/onHunger/onDrop/onWeather/onSpawn
```

- `onEnable`: config → listener → **register outgoing plugin channel
  `BungeeCord` if not already registered** (`:81-83`) → 4 command executors →
  log hub worlds (`:76-90`).
- `onJoin` runs 10 ticks later and only acts if the player is still online
  **and in a hub world**; teleports to spawn when `teleport-on-join` and spawn
  are set, clears the inventory and gives the selector, then shows
  `Title.title("Welcome" aqua, player name gray)` (`:243-258`).
  `giveJoinItems` **wipes the whole inventory** before placing the selector at
  `selectorSlot` (`:201-204`).
- **Selector item detection** (`onInteract`, `:260-275`): right-click only,
  `item.getType() == selectorMaterial`, then a **display-name string equality
  check** `color(selectorName).equals(meta.getDisplayName())` (`:270`) — so only
  an item literally named per config opens the GUI. Click is cancelled and
  `openSelector` runs.
- **Selector GUI** (`openSelector`, `:218-241`): a 27-slot inventory owned by
  `HubMenu implements InventoryHolder` (`:425-443`), whose `targets` map is
  copied from the config-derived `selectorTargets` per menu (`newMenu()`,
  `:446-450`). Each server item shows `display` and either its configured
  `lore` or a generated `&7Click to join &f<id>` (`:231-236`).
- `onClick` cancels all clicks in the menu, resolves `rawSlot → server id`,
  closes the inventory, messages *"Connecting you to <server>..."* and calls
  `sendToServer` (`:277-293`).
- **`sendToServer`** writes the classic BungeeCord plugin-message payload:
  `writeUTF("Connect")` + `writeUTF(server)` on a `ByteArrayOutputStream` and
  `player.sendPluginMessage(this, "BungeeCord", bytes)` (`:206-216`); an
  `IOException` produces *"Could not reach the proxy."* (`:213-215`). Channel
  constant `BUNGEE_CHANNEL = "BungeeCord"` (`:56`).
- **Protection handlers** (all `ignoreCancelled = true`, all gated on
  `protect && isHub(world)`):
  - `BlockBreakEvent` / `BlockPlaceEvent` — cancelled unless `nexushub.bypass`
    (`:309-321`).
  - `EntityDamageEvent` and `FoodLevelChangeEvent` — cancelled for **any entity
    in the hub world**, with no bypass check (`:323-335`).
  - `PlayerDropItemEvent` — cancelled unless bypass (`:337-342`).
  - `WeatherChangeEvent` — cancelled only when `event.toWeatherState()` is
    true, i.e. rain/snow starting (`:344-349`).
  - `CreatureSpawnEvent` — cancelled for every reason except `CUSTOM`
    (`:351-357`).
- **Void rescue** (`onMove`, `:295-307`): needs `void-teleport`, a set `spawn`,
  a hub world, and `event.getTo().getY() < voidY` → `teleport(spawn)`. Note
  this handler runs on every movement packet of every player in a hub world.

### 7.5 Data storage

No separate data files. State lives in `config.yml` itself: the `spawn`
section is written back with `saveConfig()` (`:165-178`), everything else is
read-only config. Nothing else is persisted (no join-item state, no history).

### 7.6 External integrations

- **BungeeCord proxy plugin messaging** — the only outbound network path:
  channel `BungeeCord`, message `Connect <server>`
  (`:56`, `:81-83`, `:206-216`). Per `docs/NexusHub.md:73` this requires a
  BungeeCord/Velocity proxy (Velocity implements the BungeeCord legacy channel).
- No webhooks, HTTP endpoints, or databases.

### 7.7 Details / gotchas

- **`selector.servers.<key>.bungee` is parsed by nothing.** `loadConfigValues`
  reads only `slot`, `display`, `material`, `lore` (`:133-141`) and stores the
  *section key* as the target id (`selectorTargets.put(slot, key)`, `:137`);
  `onClick` then sends that key to the proxy (`:286-292`). So in the sample
  config it works only because key == `bungee` value (`config.yml:24-28`); if
  you rename a section but keep `bungee:` pointing at the real proxy server
  name, the plugin will send the wrong name. `docs/NexusHub.md:63` and
  `config.yml:22` both document `bungee` as *"Name the proxy knows"*.
- The selector item is identified by **material + exact legacy display name**,
  not by PDC (`:266-272`) — a renamed/renamed-by-another-plugin compass will
  not open the menu, and any item with matching material+name will.
- `giveJoinItems` clears the player's entire inventory on join and on
  `/hub`/`/spawn` (`:202`, `:404`).
- `onMove` fires on every movement — no distance/altitude short-circuit besides
  the Y comparison (`:295-307`).
- `/hub` and `/spawn` are behaviourally identical in code (`:393-405`).
- Nothing unregisters the outgoing `BungeeCord` channel on disable, and the
  registration is guarded by `isOutgoingChannelRegistered` (`:81-83`), so
  coexisting with other proxy plugins is safe.

---

## 8. Overencumbered

> *"Every item in the game has weight. Carry too much and you slow down — carry
> much more and you are crushed, unable to sprint or jump, losing health until
> you drop something."* — `docs/Overencumbered.md:3-5`

### 8.1 Purpose

Survival-logistics layer: every material has a weight, the player's total is
recomputed on a timer, divided by capacity to get a ratio, mapped to a tier
(`LIGHT` / `OVERBURDENED` / `CRUSHED`), and that tier's effects are applied
(`docs/Overencumbered.md:18-36`,
`Overencumbered/src/main/java/com/xryloth/overencumbered/Overencumbered.java:97-119`).

This is the **second multi-class plugin** (after AccountShield): seven classes
under `com.xryloth.overencumbered`.

### 8.2 Commands & permissions

`Overencumbered/src/main/resources/plugin.yml:8-23`:

| Command | Usage | Permission |
|---|---|---|
| `/overencumbered` | `/overencumbered <check\|top\|reload>` | `overencumbered.use` at command level (`plugin.yml:13`) |
| ↳ bare / unknown sub | show your own weight | — |
| ↳ `check [player]` | weight of yourself or another | `overencumbered.admin` for others (code `:47`) |
| ↳ `top` | 10 heaviest players online | — |
| ↳ `reload` | reload config | `overencumbered.admin` (code `:60`) |

Aliases: `weight`, `encumbrance`, `oe` (`plugin.yml:12`).

Permissions: `overencumbered.use` (*"View your own weight and the leaderboard"*,
`default: true`), `overencumbered.bypass` (*"Ignore all weight effects
entirely"*, `default: false`), `overencumbered.admin` (*"Reload the
configuration and inspect other players"*, `default: op`) — `plugin.yml:14-23`.

`WeightCommand` implements `CommandExecutor, TabCompleter`
(`command/WeightCommand.java:19`) and is registered as both on
`overencumbered` (`Overencumbered.java:47-50`). Subcommand dispatch defaults to
`"self"` when no args are given (`:29-37`). Checking another player requires
either `overencumbered.admin` **or** typing your own name (`:47-50`). Tab
completion offers `check`, `top`, plus `reload` only for admins, and online
player names for `check` (`:111-128`).

`show()` is notable: it calls `plugin.refresh(target)` first, i.e. **running the
command re-computes and re-applies that player's effects** (`:78-87`).

### 8.3 config.yml keys

`Overencumbered/src/main/resources/config.yml` (143 lines), read by
`WeightSettings.from` (`config/WeightSettings.java:46-96`):

| Key | Default | Line |
|---|---|---|
| `prefix` | `&8[&6Weight&8] &r` | `config.yml:13` |
| `capacity` | `1000.0` | `:16` |
| `capacity-overrides.<permission>` | `overencumbered.capacity.vip: 1500.0` | `:20-21` |
| `unit` | `kg` | `:24` |
| `default-weight` | `0.5` | `:30` |
| `weights.<material>` | 53 exact entries (feather 0.05 … netherite_block 64.0) | `:33-86` |
| `tags.<tag>` | 13 tag entries (logs, planks, wool, leaves, saplings, small_flowers, coals, swords, pickaxes, axes, shovels, hoes) | `:90-102` |
| `weigh.inventory` | `true` | `:108` |
| `weigh.armor` | `true` | `:109` |
| `weigh.offhand` | `true` | `:110` |
| `weigh.ender-chest` | `false` | `:111` |
| `weigh.shulker-contents` | `true` | `:114` |
| `weigh.shulker-contents-factor` | `0.5` | `:115` |
| `weigh.shulker-nesting` | `1` | `:116` |
| `tiers.overburdened` | `0.5` | `:122` |
| `tiers.crushed` | `0.8` | `:123` |
| `effects.overburdened.slowness-amplifier` | `0` | `:127` |
| `effects.overburdened.exhaustion` | `0.02` | `:128` |
| `effects.crushed.slowness-amplifier` | `1` | `:130` |
| `effects.crushed.exhaustion` | `0.05` | `:131` |
| `effects.crushed.block-sprint` | `true` | `:132` |
| `effects.crushed.block-jump` | `true` | `:133` |
| `effects.crushed.damage` | `2.0` | `:134` |
| `effects.crushed.damage-interval-seconds` | `5` | `:135` |
| `hud.action-bar` | `true` | `:141` |
| `hud.update-interval-ticks` | `10` (= twice a second, `:142`) | `:143` |

There is **no config key for the bypass permission** — `bypassPermission` is a
hard-coded field defaulting to `"overencumbered.bypass"`
(`WeightSettings.java:44`) and `WeightSettings.from` never assigns it. The
interval is floored at 1 tick (`:94`).

### 8.4 Architecture

```
Overencumbered (main, JavaPlugin)
 ├─ WeightSettings        config snapshot            config/WeightSettings.java
 ├─ WeightTable           material→weight resolution  weight/WeightTable.java
 ├─ WeightCalculator      sums a player's load        weight/WeightCalculator.java
 ├─ Tier                  LIGHT/OVERBURDENED/CRUSHED  weight/Tier.java
 ├─ WeightListener        sprint/jump/join/quit       listener/WeightListener.java
 └─ WeightCommand         /overencumbered             command/WeightCommand.java
 + util/Text              '&' → Adventure Component   util/Text.java
```

- **Main class** (`Overencumbered.java:28`): holds four concurrent caches —
  `weights`, `tiers`, `slowed`, `lastDamage` (`:35-38`). `onEnable` saves the
  default config, `load()`s settings/table/calculator, registers the listener
  and command, and logs capacity + rule count (`:40-53`). `onDisable` stops the
  task and clears all caches (`:55-62`).
- **Ticker**: `startTask()` schedules `tick()` starting after 20 ticks and
  repeating every `updateIntervalTicks` (`:76-79`); `load()`/`reload()` always
  cancel first (`:76-86`). `tick()` skips players with the bypass permission
  and calls `refresh(player)` for everyone else (`:88-95`).
- **`refresh(player)`** (`:98-119`): computes `calculator.total`, stores it,
  divides by `capacityFor(player)` (`:103-104`), maps to a tier with
  `Tier.of(ratio, overburdenedThreshold, crushedThreshold)`
  (`weight/Tier.java:10-18`: `>= crushed` → CRUSHED, `>= overburdened` →
  OVERBURDENED, else LIGHT). **Creative and Spectator are forced to `LIGHT`**
  (`:107-111`). Then `applyEffects` and, unless dead, an action bar HUD
  (`:113-117`).
- **Effects** (`applyEffects`, `:121-139`):
  - `LIGHT` → removes the Slowness potion effect only if this plugin had added
    it (tracked in `slowed`) (`:124-128`).
  - `OVERBURDENED` → Slowness at the configured amplifier + exhaustion.
  - `CRUSHED` → Slowness + exhaustion + `maybeDamage`.
  Slowness duration is `updateIntervalTicks + 40` ticks with
  `(ambient=true, particles=false, icon=false)` (`slow`, `:141-146`); exhaustion
  is added to `player.setExhaustion`, clamped at 40 (`:148-153`); damage is
  rate-limited per player by `lastDamage` against
  `damage-interval-seconds` and dealt with `player.damage(...)` (`:155-168`).
- **`capacityFor`** starts from `capacity` and takes the **max** over every
  `capacity-overrides` permission the player has (`:176-184`).
- **HUD** (`hud`, `:170-174`): `"<tierColor>Weight &f<w>&7/&f<cap><unit> &8(<color><pct>&8)"`
  — the docs' example is `Weight 742/1000kg (74%)` (`docs/Overencumbered.md:98`).
- **`WeightCalculator`** (`weight/WeightCalculator.java`): sums
  `inventory.getContents()`, `getArmorContents()`, off-hand and (optionally)
  ender chest (`:23-39`); per stack it is `table.weightOf(material) * amount`
  (`:61`), and if `weigh-shulker-contents` is on, the material name ends with
  `SHULKER_BOX` and `depth < shulker-nesting`, the box's `BlockStateMeta`
  inventory is recursively summed and multiplied by
  `shulker-contents-factor` (`:63-77`).
- **`WeightTable`** (`weight/WeightTable.java`): priority **exact material →
  first matching tag → `default-weight`** (class Javadoc `:14-18`,
  `weightOf` `:48-62`). Exact entries go through `Material.matchMaterial`
  (`:31-35`); tags are resolved with
  `Bukkit.getTag("items", NamespacedKey.fromString(key), Material.class)` inside
  a `try/catch` so unknown/invalid tags are silently skipped (`:36-45`) — the
  "typo can never break the plugin" claim in `docs/Overencumbered.md:60-61`.
  `size()` = exact + tag rule counts, logged at startup (`Overencumbered.java:51-52`).
- **`WeightListener`** (`listener/WeightListener.java`): `PlayerToggleSprintEvent`
  and Paper's `com.destroystokyo.paper.event.player.PlayerJumpEvent`, both at
  `HIGH` with `ignoreCancelled`, cancel when the tier is `CRUSHED`, the matching
  `block-*` flag is on and the player lacks bypass (`:23-49`). `PlayerJoinEvent`
  triggers a `refresh` 5 ticks later (`:51-55`); `PlayerQuitEvent` calls
  `plugin.forget(uuid)` to drop all four cache entries (`:57-60`).
- `topWeights(limit)` ranks **online players only** by cached weight descending
  (`Overencumbered.java:202-212`).

### 8.5 Data storage

**None.** No YAML/JSON/database is written by this plugin — `onDisable` only
clears the in-memory maps (`Overencumbered.java:55-62`). `docs/Overencumbered.md:176`
confirms: *"all state lives in `config.yml`"*. Weight and tier are derived
values, recomputed from the live inventory on every tick.

### 8.6 External integrations

**None.** No webhooks, HTTP, database, proxy messaging, or other plugins.

### 8.7 Details / gotchas

- Effects are applied via **vanilla potion effects** plus Paper's sprint/jump
  events, so a minigame plugin managing those will conflict — the docs
  recommend `overencumbered.bypass` for such players
  (`docs/Overencumbered.md:164-166`).
- Crushed damage uses vanilla `damage()` so armour reduces it — documented as
  intentional (`docs/Overencumbered.md:170-171`).
- Weight is recomputed on a timer rather than per inventory packet —
  `docs/Overencumbered.md:167-169`.
- `hud.action-bar` sends an action bar **every tick cycle** for every non-bypass
  player (`Overencumbered.java:115-117`) — there is no separate HUD interval
  from `update-interval-ticks`.
- `slowed` is only cleaned on a LIGHT transition or quit — a player leaving via
  kick/other paths still hits `onQuit`, but a plugin that removes the effect
  itself will be re-applied next tick while over-encumbered (by design).
- `Material.matchMaterial` accepts both `minecraft:stone` and legacy `STONE`
  names, so both config styles work (`WeightTable.java:31`).

---

## 9. MinePie (a.k.a. "MinePie | Status")

> *"A DiscordSRV-style bridge for Paper 1.21.x that embeds a fully working
> Discord bot directly inside the plugin — built on the raw Discord Gateway +
> REST API with zero external dependencies (no JDA, no shading)."* —
> `docs/MinePie.md:3-5`

### 9.1 Purpose

Full Minecraft ↔ Discord bridge: online/offline status embeds, a
self-editing live status embed with a Refresh button, both chat directions,
join/leave/death embeds, console-log relay, allow-listed console command
execution from Discord, and bot presence (`docs/MinePie.md:7-25`,
`MinePie/src/main/java/com/xryloth/minepie/MinePie.java:21-32`).

This is by far the largest plugin: **11 classes / 1,918 lines**. Its module POM
is also the only child POM with extra dependencies (see below).

### 9.2 Build note — the only module with declared dependencies

`MinePie/pom.xml:15-30` adds two **`provided`-scope** dependencies, both
bundled by the Paper server itself (comments at `:16` and `:23`):
- `com.google.code.gson:gson:2.11.0` — JSON for Discord payloads
- `org.apache.logging.log4j:log4j-core:2.24.3` — the console-relay appender

So shading is still not used anywhere in the suite; MinePie simply compiles
against server-provided libraries. `<name>` is `MinePie | Status`
(`pom.xml:13`) — note the pipe in the artifact name. This matches
`docs/MinePie.md:81`: *"No database, no external dependencies — Paper API +
Gson + Log4j only."*

### 9.3 Commands & permissions

`MinePie/src/main/resources/plugin.yml:8-23`:

| Command | Permission |
|---|---|
| `/minepie <help\|reload\|info\|status\|online\|offline\|test\|broadcast\|chat\|reconnect\|toggle\|debug>` | `minepie.use` at command level (`plugin.yml:13`) |

Aliases: `mp`, `minepiestatus` (`plugin.yml:12`); `permission-message: "You
don't have permission to use MinePie."` (`:14`).

Permissions: `minepie.use` (*"Use basic MinePie commands (help, info)"*,
`default: true`) and `minepie.admin` (*"Access all MinePie admin commands"*,
`default: op`, with `children: {minepie.use: true}`) — `plugin.yml:15-23`.

`MinePieCommand implements TabExecutor` (`command/MinePieCommand.java:20`) with
the subcommand list `help, reload, info, status, online, offline, test,
broadcast, chat, reconnect, toggle, debug` (`:22-24`) and feature list
`status, live, chat, join, leave, death, console` (`:25-26`). Everything except
`help` and `info` is wrapped in `admin(...)`, which enforces `minepie.admin`
(`:193-199`).

Per-subcommand behaviour (`:38-121`):
- `help` — prints 10 formatted help lines + `messages.usage` (`:125-142`).
- `info` — version, gateway state, latency, uptime, players, status/chat
  channel ids, console-relay and Discord-commands toggles (`:144-165`).
- `reload` → `plugin.reloadPlugin()` (`:41-44`).
- `reconnect` → `plugin.startGateway()` (`:45-48`).
- `online` / `offline` → status embeds; offline uses reason
  `"Manual test from <sender>"` (`:49-56`).
- `status` → posts the live embed now (`:57-60`).
- `test` → green `✅ MinePie test` embed to the chat channel, falling back to
  the status channel (`:61-75`).
- `broadcast <msg>` → yellow `📢 Server Broadcast` embed to the **status**
  channel (`:76-94`).
- `chat <msg>` → bolded plain message to the MC→Discord chat channel
  (`:95-107`).
- `toggle <feature>` → flips an in-memory runtime toggle (`:108-118`).
- `debug` → gateway state, session id, last sequence, live message id, token
  configured? (`:167-180`).

Tab completion: prefix-filtered subcommands, and `FEATURES` for `toggle`
(`:206-223`).

### 9.4 config.yml keys

`MinePie/src/main/resources/config.yml` is 247 lines and fully commented
(`config.yml:1-43` is a header with setup steps and the placeholder list).
Loaded by `Settings.load()` (`config/Settings.java:101-180`). Grouped:

| Section | Keys (defaults) | `config.yml` lines |
|---|---|---|
| `bot` | `token` (`"PASTE_YOUR_BOT_TOKEN_HERE"`), `intents.guild-messages` `true`, `intents.message-content` `true`, `presence.{enabled,activity,text,update-interval-seconds}` = `true`/`playing`/`"{online}/{max} players online"`/`60` | `45-60` |
| `server` | `name: "My Server"` | `63-64` |
| `status` | `channel-id` `""`, `send-on-enable` `true`, `send-on-disable` `true`, `disable-reason`, `online-embed`, `offline-embed` | `69-105` |
| `status.live` | `enabled` `true`, `update-interval-seconds` `30` (min 10), `reuse-message` `true`, `button.{enabled,label,style}` = `true`/`"🔄 Refresh"`/`secondary`, `embed` | `107-132` |
| `chat.minecraft-to-discord` | `enabled` `true`, `channel-id`, `format` `embed` \| `message`, `message-format`, `ignored-players`, `embed.*` (incl. `author-icon: "https://minotar.net/helm/{player}/128.png"`) | `137-155` |
| `chat.discord-to-minecraft` | `enabled` `true`, `channel-id`, `format` `"&9[Discord] &f{username} &8» &f{message}"`, `relay-bot-webhooks` `false` | `157-162` |
| `console` | `enabled` `false`, `channel-id`, `min-level` `INFO`, `flush-interval-seconds` `2`, `ignored-lines` (2 default regexes) | `167-175` |
| `events.{join,leave,death}` | each `enabled` (`true`/`true`/`false`), `channel-id` (blank ⇒ status channel), `embed.*` | `180-222` |
| `commands.console-execution` | `enabled` `false`, `channel-id`, `prefix` `"!"`, `allowed-user-ids` `[]` | `229-234` |
| `messages` | `prefix`, `no-permission`, `reloaded`, `sent`, `toggled`, `unknown-feature`, `not-configured`, `usage` | `239-247` |

Every embed block supports `title, description, color (hex), timestamp,
fields[] ({name, value, inline}), footer, thumbnail, image, author-name,
author-icon` — parsed uniformly by `EmbedTemplate.load` (`config/EmbedTemplate.java:28-54`).

Notable normalisation in `Settings`:
- Intent bits are computed as `1<<9` (GUILD_MESSAGES) and `1<<15`
  (MESSAGE_CONTENT) (`Settings.java:17-18`, `:105-108`).
- Channel ids accept raw snowflakes **or** Discord `<#…>`/`#…` mentions via
  `Settings.id()` (`:192-200`; returns `0` when unparseable, which every caller
  treats as "not configured").
- Clamps: presence interval `>= 15 s` (`:113`), live interval `>= 10 s`
  (`:125`), console flush `>= 1 s` (`:147`).
- `console.min-level` → Log4j `Level.toLevel` (`:146`); `ignored-lines`
  regexes compiled with a warning on `PatternSyntaxException` (`:149-155`).
- Presence activity default in code is `"watching"` (`:111`) while the shipped
  config says `playing` (`config.yml:58`).
- `disabledRuntime` is explicitly *never persisted* (`:94-95`) — `/minepie
  toggle` state is lost on restart.

### 9.5 Architecture

```
MinePie (main, JavaPlugin)
 ├─ Settings              config snapshot (+ runtime toggles)   config/Settings.java
 ├─ EmbedTemplate         config embed → rendered Embed         config/EmbedTemplate.java
 ├─ Placeholders          {…} resolution                        util/Placeholders.java
 ├─ RestClient            Discord REST v10 (java.net.http)      discord/RestClient.java
 ├─ GatewayClient         Discord Gateway WebSocket             discord/GatewayClient.java
 ├─ DiscordListener       dispatch → MC relay / commands/button discord/DiscordListener.java
 ├─ Embed / DiscordMessages  hand-rolled JSON payload builders  discord/Embed.java, DiscordMessages.java
 ├─ StatusBroadcaster     online/offline/live embeds + data.yml status/StatusBroadcaster.java
 ├─ ChatListener          MC → Discord chat/join/leave/death    listener/ChatListener.java
 └─ ConsoleRelay          Log4j appender → Discord              console/ConsoleRelay.java
     MinePieCommand       /minepie                               command/MinePieCommand.java
```

**Lifecycle** (`MinePie.java`):
- `onEnable` (`:53-100`): sets the static singleton, loads settings, creates a
  **single-threaded daemon executor** named `MinePie-Discord` (`:61-65`),
  `RestClient`, `DiscordListener`, `StatusBroadcaster`, registers
  `ChatListener` and the command, attaches the console relay if enabled
  (`:78`), starts the live-status task (`:79`), schedules the presence task
  (`:81-84`), logs a severe error instead of connecting when no token is set
  (`:86-90`), and fires the "online" status embed 60 ticks after boot (`:93-97`).
  `tokenConfigured()` rejects blank tokens and anything starting with
  `PASTE_YOUR` (`:156-159`).
- `onDisable` (`:102-124`): stops the broadcaster, sends the **offline** embed
  synchronously (`broadcastOfflineSync`, 5-second join, `StatusBroadcaster.java:109-117`),
  detaches the console appender, shuts the gateway down (close frame 1000),
  and shuts the executor down with a 2 s await (`:115-122`).
- `sendToChannel(channelId, payload)` is the single funnel for all outbound
  messages — submitted to `discordExec` (`:168-171`), so no HTTP ever runs on
  the main server thread.
- `reloadPlugin()` reloads config/settings, restarts the live task, re-attaches
  the console relay, rebuilds `RestClient` and restarts the gateway (`:197-206`).
- `attachConsole()` casts `LogManager.getRootLogger()` to
  `org.apache.logging.log4j.core.Logger` and `addAppender`s the relay, plus an
  async flush task every `flush-interval-seconds` (first flush at 40 ticks) —
  all wrapped in `try/catch (Throwable)` so a non-log4j-core server degrades
  gracefully (`:210-226`); `detachConsole()` cancels and removes (`:228-242`).

**GatewayClient** (`discord/GatewayClient.java`) — raw
`java.net.http.WebSocket` implementing `WebSocket.Listener`:
- URL `wss://gateway.discord.gg/?v=10&encoding=json` (`:36`).
- Handles opcode 10 HELLO (choose RESUME vs IDENTIFY, start heartbeats,
  `:217-232`), 11 ACK (records latency, `:195-198`), 1 heartbeat request
  (`:199`), 7 RECONNECT (close with 4000, `:200-203`), 9 INVALID SESSION
  (clear session/seq when `d == false`, `:204-210`) and 0 DISPATCH (`:211`).
- `READY` captures `session_id` + application id and resets the backoff
  counter (`:281-287`).
- Heartbeats run on a dedicated `MinePie-Gateway` daemon scheduler with the
  Discord-specified interval plus random jitter (`:246-251`); a missed ACK
  forces a reconnect (`:253-266`).
- Reconnect backoff: `min(60, 2^min(attempt,6))` seconds (`:161-171`);
  fatal close codes `4004 / 4010 / 4012 / 4014` are **not** retried and are
  explained in the log (`invalid token`, `wrong intents value`, `fatal gateway
  error`, `disallowed intents — enable MESSAGE CONTENT INTENT`) (`:135-145`).
- Outbound: IDENTIFY carries token + intents + `{os: linux, browser:
  MinePie, device: MinePie}` (`:234-244`); presence update is op 3 (`:303-316`);
  fragment reassembly via `frameBuffer` (`:113-125`).

**RestClient** (`discord/RestClient.java`) — base `https://discord.com/api/v10`
(`:19`), `Authorization: Bot <token>` (`:41`), 3 attempts, and **429 handling**
that parses `retry_after`, sleeps `ms + 50` and retries (`:39-60`). Methods:
`sendMessage` (POST `/channels/{id}/messages` → message id, `:78-81`),
`editMessage` (PATCH, `:84-86`) and `respondInteraction`
(POST `/interactions/{id}/{token}/callback`, `:89-91`).

**DiscordListener** (`discord/DiscordListener.java`) handles two dispatches:
- `MESSAGE_CREATE` (`:69-132`): reads channel id, author (bot flag, id,
  `global_name` else `username`), content — and if content is empty it falls
  back to **attachment filenames** (`:83-94`). Then:
  - Discord → MC when channel matches and `runtimeEnabled("chat")`; bots are
    ignored unless `relay-bot-webhooks` is on **and** the message has a
    `webhook_id` (`:98-108`); the `§` character is rewritten to `&` before
    formatting (`:102`), and the broadcast is scheduled onto the main thread
    with `LegacyComponentSerializer.legacySection()` (`:105-106`).
  - Console commands: if the channel matches, the message starts with
    `commands.console-execution.prefix` and (allow-list is empty **or**
    contains the user id) the remainder is dispatched as the console sender on
    the main thread (`:111-118`), then a yellow `⛏ Command executed` embed is
    posted back (`:119-125`). Non-allow-listed attempts are logged as warnings
    (`:126-129`).
- `INTERACTION_CREATE` (`:136-154`): only type 3 (MESSAGE_COMPONENT) with
  `custom_id` starting `minepie:`; `minepie:refresh` replies with interaction
  callback type 7 (UPDATE_MESSAGE) carrying a fresh `livePayload()`.

**ChatListener** (`listener/ChatListener.java`) — all handlers are
`MONITOR`; MC→Discord chat uses `AsyncChatEvent` (the modern Paper event,
`:28-46`) with `PlainTextComponentSerializer`, honouring `ignored-players` and
the runtime `chat` toggle; join/leave/death each fall back to the status
channel when their own `channel-id` is blank (`:52`, `:64`, `:76`) and
re-publish presence on join/leave (`:57`, `:69`).

**StatusBroadcaster** (`status/StatusBroadcaster.java`):
- Loads `data.yml` at construction and reads `live-message-id` (`:31-36`).
- `start()` schedules `tickLive` every `liveInterval * 20` ticks, async
  (`:40-47`); `stop()` cancels and saves (`:49-55`).
- `tickLive` edits the existing message when `reuse-message` is true and the
  edit succeeds, otherwise posts a new one and persists the new id (`:57-71`).
- `livePayload()` renders the live embed with an extra `{interval}` var and
  optionally wraps a `minepie:refresh` button in an action row (`:74-84`).
- `broadcastOfflineSync` submits to the executor and `get(5, SECONDS)` —
  best-effort during shutdown (`:109-117`).

**ConsoleRelay** (`console/ConsoleRelay.java`) extends Log4j's
`AbstractAppender` (`:21`). `append` filters by `consoleEnabled`/channel,
compares `event.getLevel().intLevel() > consoleMinLevel.intLevel()` (`:41`),
drops lines matching any `ignored-lines` regex (`:44-46`), and queues
`"[LEVEL] line"` into a bounded deque of **300** (`:23`, `:47-50`). `flush()`
drains up to **40 lines**, splits them into chunks of at most **1,800 chars**,
sends at most **3** messages per flush as ```` ``` ```` code blocks, and pushes
overflow back to the *front* of the queue (`:53-86`).

**Embed / DiscordMessages** (`discord/Embed.java`, `discord/DiscordMessages.java`)
are hand-rolled Gson builders that clip to Discord's documented limits
(title 256, description 4096, field name 256 / value 1024, footer 2048, author
256, content 2000) (`Embed.java:57-64`, `:66-102`, `:104-107`;
`DiscordMessages.java:31`), with `button()` (type 2, style map primary 1 /
secondary 2 / success 3 / danger 4 / link 5) and `actionRow()` (type 1)
(`Embed.java:109-134`).

### 9.6 Data storage

Only one file: `plugins/MinePie/data.yml` with a single key
`live-message-id`, written by `StatusBroadcaster.save()`
(`status/StatusBroadcaster.java:133-140`) and read at construction (`:35`) —
this is what lets the live embed keep editing the same message across restarts
(`docs/MinePie.md:79-80`). Everything else is `config.yml`. No database.

### 9.7 External integrations (exact endpoints)

| Purpose | Endpoint | Citation |
|---|---|---|
| Discord Gateway | `wss://gateway.discord.gg/?v=10&encoding=json` | `GatewayClient.java:36` |
| Discord REST | `https://discord.com/api/v10` | `RestClient.java:19` |
| Post a message | `POST /channels/{channelId}/messages` | `RestClient.java:79` |
| Edit the live embed | `PATCH /channels/{channelId}/messages/{messageId}` | `RestClient.java:85` |
| Button interaction reply | `POST /interactions/{interactionId}/{interactionToken}/callback` | `RestClient.java:90` |
| Player head/avatar images | `https://minotar.net/helm/{player}/128.png` | `config.yml:155`, `:191`, `:205`, `:219` |
| Bot invite (documented) | `https://discord.com/oauth2/authorize?client_id=YOUR_CLIENT_ID&scope=bot&permissions=67584` | `config.yml:27`, `docs/MinePie.md:35` |
| Bot creation (documented) | `https://discord.com/developers/applications` | `config.yml:23`, `docs/MinePie.md:29` |

Intents required: **MESSAGE CONTENT INTENT** for Discord→Minecraft relay and
console commands (`config.yml:49-50`, `docs/MinePie.md:31-32`); the intent
bits are actually sent in IDENTIFY (`Settings.java:17-18`, `GatewayClient.java:237`).

### 9.8 Details / gotchas

- **`minepie.use` is the command-level permission**, so a non-op *without*
  `minepie.use` (default `true`, so normally everyone) can't run `/minepie` at
  all; `minepie.admin` is only enforced inside `admin(...)` for the 10
  privileged subs (`MinePieCommand.java:193-199`).
- Console command execution is **empty-allow-list = everyone**
  (`DiscordListener.java:114`) — a real foot-gun if someone enables
  `commands.console-execution` without filling `allowed-user-ids`.
- `{uptime}` is **plugin uptime**, not server uptime — measured from
  `enabledAt` (`Placeholders.java:52`, `MinePie.java:56`).
- `{tps-5}`/`{tps-10}`/`{tps-15}` are `Server.getTPS()[0..2]` which are 1m/5m/15m
  averages (`Placeholders.java:30-34`); MSPT is the mean of `getTickTimes()`
  converted from ns (`:35-40`).
- Embed colour parsing supports `#RGB` shorthand expansion and falls back to
  `0x5865F2` (Discord blurple) on bad input (`EmbedTemplate.java:87-99`).
- `bot.presence.activity` supports `playing|streaming|watching|listening|competing`
  mapped to types 0/1/3/2/5 (`MinePie.java:187-193`).
- There is **no rate limiting on the live edit path beyond Discord's 429 retry**
  and no de-duplication if two plugins post to the same channel.
- **In-game `&` formatting vs. `legacySection()`.** Every in-game message in
  MinePie is sent as a `Component` built with
  `LegacyComponentSerializer.legacySection().deserialize(text)`
  (`MinePieCommand.java:139`, `:147-164`, `:201-204`;
  `DiscordListener.java:106`; `ChatListener` renders through
  `Bukkit.broadcast(...)`), but the configured strings use `&` codes
  (`config.yml:240-247`, `config.yml:161`) and `legacySection()` interprets
  `§`, not `&`. Unlike the other plugins in this suite — which run
  `ChatColor.translateAlternateColorCodes('&', …)` first (e.g.
  `AegisGuard.java:362-364`) — nothing translates `&` before deserialization
  here. **Worth verifying in-game**: as read, MinePie's messages may render the
  raw `&8[&6MinePie&8]` prefix. (The `§ → &` rewrite at
  `DiscordListener.java:102` is consistent with that reading: it neutralises
  `§` from Discord users rather than enabling `&`.)
- The plugin registers a static singleton (`MinePie.get()`, `MinePie.java:45-49`)
  but almost everything goes through instance methods instead.

---

## 10. Cross-cutting patterns

### 10.1 Module / descriptor conventions

Every module follows the same shape:

```xml
<parent> com.xryloth:xryloth-plugins:1.0.0, relativePath ../pom.xml </parent>
<artifactId><PluginName></artifactId>
<name><PluginName></name>
```

e.g. `AccountShield/pom.xml:6-14`, `AegisGuard/pom.xml:6-14`,
`HeartVault/pom.xml:6-14`, `SkullHarvest/pom.xml:6-14`,
`NexusHub/pom.xml:6-14`, `Overencumbered/pom.xml:6-14`,
`AurumEconomy/pom.xml:6-14`. Only **MinePie** adds dependencies
(`MinePie/pom.xml:15-30`, both `provided`).

Every `plugin.yml` is uniform:
- `version: 1.0.0`, `api-version: '1.21'`, `author: Xryloth` (line 2/4/5 in all
  eight) — e.g. `AccountShield/src/main/resources/plugin.yml:1-5`.
- `website: https://github.com/EscapeXMc` in **seven** plugins
  (AccountShield, AegisGuard, AurumEconomy, HeartVault, NexusHub,
  Overencumbered, SkullHarvest — all `plugin.yml:7`) but
  `website: https://xryloth.com` in **MinePie**
  (`MinePie/src/main/resources/plugin.yml:7`).
- Package layout is `com.xryloth.<lowercasename>` in all eight.

Two structural styles exist:
- **Single-class plugins** (AegisGuard, AurumEconomy, HeartVault, NexusHub,
  SkullHarvest): one `.java` file that is simultaneously `JavaPlugin`,
  `Listener` and `CommandExecutor`, with a `switch` on
  `command.getName().toLowerCase()` and a `default` branch that doubles as the
  help/`reload` handler.
- **Multi-class plugins** (AccountShield, Overencumbered, MinePie): a main
  class plus `command/`, `config/`/`settings`, `listener/`, `util/` (and
  `detection/`, `storage/`, `weight/`, `discord/`, `status/`, `console/`)
  sub-packages, with a dedicated `Settings` snapshot object.

### 10.2 Config conventions

- `saveDefaultConfig()` on enable, `reloadConfig()` + re-read into a settings
  object on `<plugin> reload` — e.g. `AccountShield.java:47`, `:74-79`;
  `Overencumbered.java:42`, `:64-67`; `MinePie.java:57`, `:197-206`;
  `AegisGuard.java:59`, `:351-352`.
- A **`prefix`** key exists in every plugin's config (or under `messages.` for
  MinePie): `AccountShield/config.yml:12`,
  `AegisGuard/config.yml:3`, `AurumEconomy/config.yml:5`,
  `HeartVault/config.yml:12`, `SkullHarvest/config.yml:16`,
  `NexusHub/config.yml:12`, `Overencumbered/config.yml:13`,
  `MinePie/config.yml:240`.
- Every scalar read is `c.getX(path, default)` with the default duplicated in
  Java (`Settings.from(...)`, `WeightSettings.from(...)`,
  `loadConfigValues()` in the single-class plugins) — config and code defaults
  can drift, and in one case already do (`HeartVault` `ban-message`:
  `config.yml:11` vs `HeartVault.java:80`; MinePie presence activity:
  `config.yml:58` vs `Settings.java:111`).
- YAML comments are a first-class part of the product: both AccountShield and
  Overencumbered ship long explanatory headers
  (`AccountShield/config.yml:1-10`, `Overencumbered/config.yml:1-11`) and
  MinePie ships a 43-line ASCII-art header with setup + placeholder docs
  (`MinePie/config.yml:1-43`).
- Reload is always an **admin-gated subcommand** of the plugin's root command
  (`ShieldCommand.java:111-118`, `AegisGuard.java:346-355`,
  `HeartVault.java:218-227`, `SkullHarvest.java:237-251`,
  `NexusHub.java:379-388`, `WeightCommand.java:59-66`,
  `MinePieCommand.java:41-44`); AurumEconomy has **no reload command at all**.

### 10.3 Message formatting — three coexisting approaches

| Approach | Where |
|---|---|
| `ChatColor.translateAlternateColorCodes('&', s)` (+ `stripColor` for console/webhooks) | AegisGuard `:362-368`, AurumEconomy `:372-378`, HeartVault `:279-285`, NexusHub `:408-414`, SkullHarvest `:259-261` (relative to each file) |
| `util/Text.of(String)` → `LegacyComponentSerializer.legacyAmpersand()` | AccountShield `util/Text.java:9-16`, Overencumbered `util/Text.java:9-16` — **the two copies are byte-identical apart from the Javadoc comment** (`"Tiny text helper:…"` vs `"Turns config '&' strings…"`) |
| `LegacyComponentSerializer.legacySection()` with **no** `&` translation | MinePie (`MinePieCommand.java:139`, `:201-204`, `DiscordListener.java:106`) |

`Text.java` is the clearest case of copy-paste-shared util code in the suite:
there is no common module, so AccountShield and Overencumbered each carry their
own duplicate (`pom.xml` has no shared library module — the parent only
aggregates, `pom.xml:14-23`).

Other repeated idioms:
- **`color(...)` / `strip(...)` private helpers** in every single-class plugin.
- **Atomic-ish YAML writes**: AccountShield writes to a `.tmp` then
  `Files.move(REPLACE_EXISTING)` (`AccountStore.java:194-203`); every other
  plugin uses plain `YamlConfiguration.save(...)`.
- **`ConcurrentHashMap`-backed state** only where an async thread touches it
  (AccountShield `AccountStore.java:28-29`, `Account.java:16-24`;
  Overencumbered `Overencumbered.java:35-38`).
- **Permission naming**: `<plugin>.admin` for reload/config and a separate
  `.staff`/`.use` for ordinary use — `accountshield.{staff,admin}`,
  `aegis.{staff,admin}`, `aurum.admin`, `heartvault.{admin,check.others}`,
  `skullharvest.admin`, `nexushub.{admin,bypass}`,
  `overencumbered.{use,bypass,admin}`, `minepie.{use,admin}`.
- **Permission defaults**: admin/staff nodes are `default: op` everywhere;
  the only `default: true` nodes are `heartvault.check.others`
  (`HeartVault/plugin.yml:24-26`), `overencumbered.use`
  (`Overencumbered/plugin.yml:15-17`) and `minepie.use`
  (`MinePie/plugin.yml:16-18`).
- **`onTabComplete`** exists only in AccountShield (`ShieldCommand.java:260-298`),
  Overencumbered (`WeightCommand.java:111-139`) and MinePie
  (`MinePieCommand.java:206-223`); the single-class plugins ship no tab
  completion.
- **Docs vs. code**: each `docs/<Name>.md` mirrors its `config.yml` snippet and
  command table, and is accurate for commands/permissions in every case I
  checked except where noted (`HeartVault` `check.others` is declared but never
  enforced in code; `SkullHarvest`'s hidden `reload reset`).

---

## 11. Summary table

| Plugin | Purpose | Main class | Commands | Storage |
|---|---|---|---|---|
| **AccountShield** | Alt detection, ban-evasion blocking, join-flood defence | `com.xryloth.accountshield.AccountShield` (`plugin.yml:3`) | `/accountshield <info\|alts\|ban\|unban\|trust\|untrust\|watch\|list\|reload>` (aliases `ashield`,`as`,`shield`), `/alts` | `accounts.yml` + `bans.yml` (atomic YAML) (`AccountStore.java:43-44`) |
| **AegisGuard** | Moderation: chat filter, mutes, reports, staff chat, watchlist, kick | `com.xryloth.aegisguard.AegisGuard` (`plugin.yml:3`) | `/report`, `/reports`, `/mute`, `/unmute`, `/kick`, `/sc` (`staffchat`), `/watch`, `/aegis reload` (`ag`) | `mutes.yml` only — reports & watchlist are memory-only (`AegisGuard.java:88`, `:47-48`) |
| **AurumEconomy** | Balances, `/pay`, `/baltop`, `/eco`, config-driven GUI shop | `com.xryloth.aurumeconomy.AurumEconomy` (`plugin.yml:3`) | `/balance` (`bal`,`money`), `/pay`, `/baltop` (`balancetop`,`moneytop`), `/eco`, `/shop` | `balances.yml` (`AurumEconomy.java:111`) — see load-path gotcha §4.5 |
| **HeartVault** | Lifesteal: steal hearts on PvP kill, ban at zero, `/revive` | `com.xryloth.heartvault.HeartVault` (`plugin.yml:3`) | `/hearts` (`heart`,`hp`) `[player\|set …]`, `/revive`, `/heartvault reload` (`hv`) | `hearts.yml` — flat `<uuid>: <int>` (`HeartVault.java:85`, `:109`) |
| **SkullHarvest** | Headsteal: head drops, consume for power, killstreaks | `com.xryloth.skullharvest.SkullHarvest` (`plugin.yml:3`) | `/heads` (`headcount`), `/killstreak` (`ks`,`streak`), `/skullharvest reload [reset <player>]` (`sh`) | `data.yml` — flat `<uuid>: <int>` heads eaten (`SkullHarvest.java:98`, `:122`); streaks not persisted |
| **NexusHub** | Lobby protection, spawn, join items, BungeeCord server selector | `com.xryloth.nexushub.NexusHub` (`plugin.yml:3`) | `/hub` (`lobby`), `/spawn`, `/setspawn`, `/nexushub reload` (`nh`) | `config.yml` only (spawn written back into it) (`NexusHub.java:165-178`) |
| **Overencumbered** | Per-item weight; slow / block sprint & jump / damage over capacity | `com.xryloth.overencumbered.Overencumbered` (`plugin.yml:3`) | `/overencumbered <check\|top\|reload>` (`weight`,`encumbrance`,`oe`) | **None** — all state derived from the live inventory |
| **MinePie** | Discord bridge: status/live embeds, chat relay, console relay, Discord console commands | `com.xryloth.minepie.MinePie` (`plugin.yml:3`) | `/minepie <help\|reload\|info\|status\|online\|offline\|test\|broadcast\|chat\|reconnect\|toggle\|debug>` (`mp`,`minepiestatus`) | `config.yml` + `data.yml` (`live-message-id` only) (`StatusBroadcaster.java:33-35`) |

Compiled artifacts for all eight are prebuilt in `dist/<Name>-1.0.0.jar`
(`README.md:28-38`).

---

## 12. Relationship to the Xryloth launcher

**There is no direct code-level integration between these plugins and the
launcher.** Evidence, both directions:

**What is *not* there**
- No plugin contains HTTP calls, sockets or file paths pointing at the
  launcher; the only outbound endpoints in the whole suite are Discord's
  gateway/REST (MinePie), Discord webhooks (AccountShield) and the BungeeCord
  plugin channel (NexusHub).
- Grepping the launcher sources (`src/`, `src-tauri/`) for
  `HeartVault|MinePie|AegisGuard|NexusHub|AccountShield|SkullHarvest|Overencumbered|AurumEconomy`
  returns **no** hits in any `.ts`/`.tsx`/`.rs` file — only two cosmetic
  brand mentions (below).
- The launcher repo explicitly **excludes** this tree:
  `XrylothTauri/.gitignore:45` is `minecraft-plugins/` (listed alongside
  `minecraft-mods/` and other local-only working data, `.gitignore:44-46`).
  So the plugins live *beside* the launcher on disk but are not part of the
  launcher build or release.

**Where they *do* touch (loosely)**
1. **Shared Modrinth distribution path.** Five of the eight are published under
   the Modrinth account **FireXDevelopment** (`MODRINTH.md:3-4`, table at
   `:8-14`), and the launcher has a generic Modrinth plugin installer for
   self-hosted Paper servers: `api.serverMgr.searchPlugins` /
   `installPlugin` in `src/lib/api.ts:301-304` → Tauri commands
   `server_mgr_search_plugins` / `server_mgr_install_plugin`
   (`src-tauri/src/lib.rs:7366-7372`), which download into
   `server_dir(<name>)/plugins/` (`src-tauri/src/lib.rs:7324-7326`) and are
   listed/toggled from `server_mgr_list_plugins` (`lib.rs:7375-7394`).
   Search is `search_modrinth(query, "plugin", "paper", …)` (`lib.rs:7319`).
   So a user can install `heartvault`, `skullharvest`, `nexushub`,
   `aegisguard-mc` or `aurumeconomy` **through the launcher** into a server
   the launcher created — but that is a generic Modrinth path, not a
   hard-coded bundle; no launcher code names these slugs.
2. **Branding / product overlap.**
   - `src/components/pages/ChatPage.tsx:1476` renders the static string
     `· Playing on MinePie SMP` in the friends/profile card.
   - `discord_friends.html:219` lists a Discord server
     `{name:"MinePie 1.21+", id:"1521208637689696388", invite:"discord.gg/rRZaNvEHuv", …}`
     next to the `Xryloth Client` server (`:222`).
   - `MinePie/src/main/resources/plugin.yml:7` points its `website` at
     `https://xryloth.com`, the launcher's product site; the other seven point
     at `https://github.com/EscapeXMc` (all `plugin.yml:7`).
   - Both use the same community surfaces: the Modrinth-wide Discord invite
     `https://discord.com/invite/YCxYeyEzCE` and donation link
     `https://razorpay.me/@kulwantsingh` (`MODRINTH.md:41-42`).
3. **Deployment target.** The launcher's server manager creates Paper servers
   (`src-tauri/src/lib.rs:6893`, server types list at `:7476`:
   *"Paper — High performance. Runs Bukkit/Spigot/Paper plugins."*) and the
   README tells admins to drop these jars into `plugins/` and restart
   (`README.md:44-51`) — i.e. the plugins are the *content* the launcher's
   server feature is designed to host, which is the whole practical
   relationship.

**Not evidenced:** any launcher-installed default plugin set, any plugin that
reads launcher config, any shared auth/session, any HTTP API shared between
the two, and any mention of these plugins in the launcher's AI tools (the AI
tool list only exposes generic `server_plugin_search`, `src-tauri/src/lib.rs:8609`).

---

## 13. Gotchas & TODOs (rebuild checklist)

### Correctness issues found in source (verify before shipping)
1. **AurumEconomy never reloads saved balances** — `saveData()` writes
   `balances.<uuid>` (`AurumEconomy.java:136`) but `loadData()` iterates the
   **root** keys and calls `UUID.fromString(key)` (`:121-127`), so the
   `balances` section key is always rejected. Compare HeartVault, which writes
   and reads at the same level (`HeartVault.java:109` vs `:95-97`). Likely
   symptom: every restart resets everyone to `starting-balance`.
2. **NexusHub ignores `selector.servers.<key>.bungee`** — the config documents
   `bungee` as *"Name the proxy knows"* (`NexusHub/config.yml:22-28`,
   `docs/NexusHub.md:63`) but `loadConfigValues` only reads
   `slot/display/material/lore` and stores the **section key** as the target
   (`NexusHub.java:133-141`, `:137`), which is what gets sent in the
   `Connect` message (`:286-292`). Works only while key == bungee value.
3. **MinePie's in-game `&` codes are likely not translated** — everything is
   deserialized with `legacySection()` while all strings use `&`
   (`MinePieCommand.java:201-204`, `MinePie/config.yml:240-247`). See §9.8.
4. **MinePie console-command allow-list is inverted-by-default** — an empty
   `allowed-user-ids` lets **anyone** in that channel run console commands
   (`DiscordListener.java:114`), while the config ships it `enabled: false`
   (`config.yml:231`) — enable carefully.
5. **AegisGuard loses reports and watchlist on restart** — both are plain
   in-memory collections (`AegisGuard.java:47-48`) and `onDisable` only saves
   mutes (`:71-74`). Docs imply the watchlist is a feature
   (`docs/AegisGuard.md:24`) without saying it is volatile.
6. **AegisGuard `/unmute` and HeartVault `/revive` require an online target**
   (`AegisGuard.java:281-286`, `HeartVault.java:203-207`), which is
   self-defeating for the ban/elimination flows those plugins create.
7. **`heartvault.check.others` is declared but never checked**
   (`HeartVault/plugin.yml:24-26` — no reference in `HeartVault.java`).
8. **Overencumbered's bypass permission is not configurable**
   (`WeightSettings.java:44` is never assigned in `from(...)`,
   and `config.yml` has no key for it).
9. **AccountShield `JoinLimiter` never evicts idle ranges** — entries are only
   pruned when a new hit for that key arrives (`util/JoinLimiter.java:14-25`),
   so the map grows monotonically with distinct `/24`s seen.
10. **SkullHarvest killstreaks are memory-only** and its
    `/skullharvest reload reset <player>` form is undocumented in
    `usage:`/docs (`SkullHarvest.java:242-247`, `plugin.yml:19`).
11. **Default configs are not safe out of the box**: AccountShield ships
    `alerts.webhook-url: ""` (fine) but AegisGuard ships sample blocked words
    (`"ez noob"`, `"kys"`, `config.yml:10-12`) that will filter real chat, and
    NexusHub ships `hub-worlds: [hub]` which won't match a default `world` —
    code falls back to `world` only when the list is **empty**
    (`NexusHub.java:96-98`).

### Cross-plugin / suite-level notes
12. **No shared library** — `Text.java`, `color()/strip()` helpers, YAML
    persistence, and the command-dispatch `switch` are duplicated across
    modules; a rebuild should consider a `xryloth-common` module (the parent
    POM would need to become a real aggregator with a shared child).
13. **No shading anywhere** — correct today only because nothing needs it;
    MinePie relies on the server's Gson/Log4j at runtime
    (`MinePie/pom.xml:16-29`), which ties it to Paper (Bukkit-only servers
    without Log4j-core would lose the console relay — the code catches
    `Throwable` and logs `Console relay unavailable`, `MinePie.java:222-225`).
14. **`api-version: '1.21'` + Java 21 + Paper 1.21.11**
    (`pom.xml:27-28`, every `plugin.yml:4`) — rebuilds must keep these in sync.
15. **Legacy APIs in use**: `AsyncPlayerChatEvent` in AegisGuard
    (`AegisGuard.java:17`) and `Bukkit.broadcastMessage` /
    `player.kick(String)`-style calls in HeartVault (`HeartVault.java:164`)
    are deprecated on current Paper; MinePie already uses `AsyncChatEvent`
    (`ChatListener.java:8`).
16. **Data is per-server everywhere** — there is no shared storage backend;
    AccountShield's docs explicitly call out *"an upcoming shared-storage
    mode"* that does not exist in the code (`docs/AccountShield.md:121-122`).
17. **Modrinth coverage is incomplete**: 5 of 8 projects submitted
    (`MODRINTH.md:8-14`); AccountShield, Overencumbered and MinePie have
    assets in `modrinth-assets/` but no project entry in `MODRINTH.md`. All
    five were still `in review` at the time of writing (`MODRINTH.md:60-63`).
18. **No tests, no CI** — there is no `src/test` in any module and no workflow
    file in this tree; `mvn package` is the only documented build step
    (`README.md:59-62`).
19. **`dist/*.jar` are committed build outputs** alongside `target/`
    (`README.md:7`, `README.md:28-38`) — a rebuild should regenerate them
    rather than trusting the checked-in 1.0.0 jars.

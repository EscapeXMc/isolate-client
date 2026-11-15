# 09 — Fabric client mod: `xryloth-menu`

Rebuild reference for the Xryloth client's in-game menu mod.

**Root of everything below:**
`…/XrylothTauri/minecraft-mods/xryloth-menu/` (referred to as *mod root*).

Java sources live under `src/main/java/com/xryloth/menu/` — below they are cited by
simple name plus line, e.g. `XrylothMods.java:265`. Resource files are cited from the
mod root, e.g. `src/main/resources/fabric.mod.json:3`.

| | |
|---|---|
| Mod id | `xrylothmenu` |
| Version | `1.0.0` |
| Minecraft | `1.21.11` (yarn `1.21.11+build.6`) |
| Loader | Fabric `0.19.5` (declared `>=0.16.0`) |
| Java | 21 (bytecode `release 21`; build itself runs on JDK 25) |
| Side | `client` only |
| Dependencies | **none** — no Fabric API, no other mod |
| License | proprietary, `LICENSE:1-5` ("Copyright (c) 2026 Xryloth. All rights reserved.") |

---

## 1. Build setup

### 1.1 Gradle / Loom

`settings.gradle:1-10` only configures `pluginManagement` repositories (Fabric maven,
Maven Central, Gradle Plugin Portal) and `settings.gradle:12` sets
`rootProject.name = 'xryloth-menu'`. There is **no Gradle wrapper committed** (no
`gradlew` in the mod root); README says to use any Gradle 8.11+ / 9.x on `PATH` or run
`gradle wrapper` once (`README.md:58`).

`build.gradle`:

- `build.gradle:1-3` — single plugin: `fabric-loom` **version 1.19.1**.
- `build.gradle:5-10` — `version = mod_version`, `group = maven_group`,
  `archivesName = archives_base_name`.
- `build.gradle:12-16` — the entire dependency set:

  ```groovy
  minecraft "com.mojang:minecraft:${project.minecraft_version}"
  mappings   "net.fabricmc:yarn:${project.yarn_mappings}:v2"
  modImplementation "net.fabricmc:fabric-loader:${project.loader_version}"
  ```

  **There is no `fabric-api` dependency** and no other mod dependency. This is a
  deliberate constraint that shapes the whole design (hand-loaded textures, baked font,
  jar-read panorama — see §5).

- `build.gradle:18-29` — the `client` Loom run uses `runDir "run"` and forces a
  `1280x960` window; the comment explains that at GUI scale 2 this gives a 640×480
  logical menu, i.e. the layout constants in `XrylothMenuScreen` were designed for it.
- `build.gradle:31-36` — `runClient` is pinned to a **JDK 21** toolchain even though
  Loom itself must run on a newer JDK ("Run the game on the JDK the mod targets (21),
  even though Loom itself needs 25").
- `build.gradle:38-43` — `processResources` expands `${version}` inside
  `fabric.mod.json`.
- `build.gradle:45-49` — `sourceCompatibility`/`targetCompatibility` Java 21 and
  `withSourcesJar()`.
- `build.gradle:51-54` — every `JavaCompile` uses UTF-8 and `options.release = 21`.
- `build.gradle:56-60` — the jar embeds `LICENSE`, renamed
  `LICENSE_xryloth-menu`.

`gradle.properties`:

- `gradle.properties:2-5` — tight build env: `-Xmx1600m`, `MaxMetaspaceSize=512m`,
  `workers.max=2`, `parallel=false`, **`daemon=false`**.
- `gradle.properties:8-10` — `minecraft_version=1.21.11`,
  `yarn_mappings=1.21.11+build.6`, `loader_version=0.19.5`.
- `gradle.properties:13-15` — `mod_version=1.0.0`, `maven_group=com.xryloth`,
  `archives_base_name=xryloth-menu`.

### 1.2 How it is built

From `README.md:47-56`:

```bash
cd minecraft-mods/xryloth-menu
JAVA_HOME=/usr/lib/jvm/java-25-openjdk-amd64 gradle build
# -> build/libs/xryloth-menu-1.0.0.jar
```

Loom 1.19.1 requires **JDK 25 to run the build**; `options.release = 21`
(`build.gradle:53`) keeps the shipped bytecode at Java 21, which is what MC 1.21.11
wants. A `runClient` task launches the game in `run/` (see §8 for what `run/` contains
after real sessions).

Built artifacts currently present:

- `build/libs/xryloth-menu-1.0.0.jar` — 2,329,207 bytes (includes baked icon PNGs,
  panorama faces, logo, font atlases, mixins, metadata).
- `build/libs/xryloth-menu-1.0.0-sources.jar` — 2,300,253 bytes.

### 1.3 Pre-build asset tooling (manual, not wired into Gradle)

None of these are Gradle tasks — they are run by hand when assets change:

1. **`scripts/tools/bake-icons.sh`** (56 lines) — bakes the catalogue's Modrinth icons
   into the jar.
   - `bake-icons.sh:22-24` derives the id list by `grep`-ing `new Entry("…"` straight
     out of `XrylothCatalog.java`, so code and icons cannot drift apart.
   - `bake-icons.sh:28-29` fetches `https://api.modrinth.com/v2/project/<id>` and pulls
     `icon_url`.
   - `bake-icons.sh:36-48` — Modrinth only serves **WebP**, which Minecraft cannot
     decode, so each icon is rendered in headless Chrome (`google-chrome --headless …
     --window-size=64,64 --screenshot`) on a transparent background and copied to
     `src/main/resources/assets/xrylothmenu/textures/icons/<id>.png`.
   - `bake-icons.sh:18` uses `mktemp -d /tmp/opencode/icons.XXXXXX` and cleans up via
     `trap`.
2. **`scripts/tools/GenerateFontAtlas.java`** (204 lines, run as
   `java scripts/tools/GenerateFontAtlas.java <font.otf> <out.png> <out.txt>` —
   `README.md:148-153`) — bakes LEMON MILK into a glyph atlas + metrics file:
   - `GenerateFontAtlas.java:31-32` — printable ASCII only, code points 32…126.
   - `GenerateFontAtlas.java:35` — three raster sizes: **12, 24, 48 px**.
   - `GenerateFontAtlas.java:38,41` — 16 glyphs per row, 1 px transparent padding.
   - `GenerateFontAtlas.java:78-111` — per-size cell geometry: one shared cell per
     raster (regular grid so the runtime can index it arithmetically), origin derived
     from the union of visual bounds, **cap height = ink height of `H`**
     (`GenerateFontAtlas.java:105-106`).
   - `GenerateFontAtlas.java:131-194` — the three rasters are stacked vertically into
     one PNG; for each glyph it writes
     `glyph <code> <advance> <inkX> <inkY> <inkW> <inkH>` (`:184-190`) and a
     `size <px> <cellW> <cellH> <columns> <originX> <originY> <0> <gridY> <capHeight>`
     header (`:140-149`).
   - `GenerateFontAtlas.java:171-176` — glyphs are filled **white** so the runtime can
     tint them.
3. **Logo rasterisation** — `README.md:87-91` and `README.md:175-188` say logo.png and
   `icon.png` come from `scripts/tools/xryloth-logo.svg` via `scripts/tools/render-logo.sh`
   (headless Chrome, transparent). **Neither file exists in the repo** — see §9.

### 1.4 Verification

`scripts/verify-jar.sh <jar>` (68 lines) inspects a built jar:

- `verify-jar.sh:9-16` — picks the newest non-`sources` `build/libs/xryloth-menu-*.jar`
  when no argument is given; prints size and SHA-1 (`:18-20`).
- `verify-jar.sh:36-43` — required entries: `fabric.mod.json`,
  `xryloth-menu.mixins.json`, `assets/xrylothmenu/textures/gui/logo.png`,
  `assets/xrylothmenu/icon.png`, and the classes `XrylothMenu`, `gui/XrylothMenuScreen`,
  `XrylothBrand`, `mixin/TitleScreenMixin`.
- `verify-jar.sh:46-55` — **forbidden third-party traces**: entry names matching
  `polyplus|oneconfig|polyfrost|oneclient|compose` must be zero (clean-room proof,
  `README.md:190-195`).
- `verify-jar.sh:58-64` — unzips and `strings`s `XrylothBrand.class`, printing the
  embedded `XRYLOTH|Xryloth|https?://` brand strings.
- `verify-jar.sh:67` — prints `RESULT: PASS|FAIL` and exits non-zero on failure.

---

## 2. Mod metadata

### 2.1 `src/main/resources/fabric.mod.json`

| Field | Value | Line |
|---|---|---|
| `schemaVersion` | `1` | `:2` |
| `id` | `xrylothmenu` | `:3` |
| `version` | `${version}` (expanded from `mod_version` = `1.0.0`) | `:4`, `build.gradle:38-43` |
| `name` | `Xryloth Menu` | `:5` |
| `description` | "Xryloth's dark themed custom main menu." | `:6` |
| `authors` | `["Xryloth"]` | `:7-9` |
| `contact.website` | `https://xrylothclient.vercel.app` | `:10-12` |
| `license` | `All-Rights-Reserved` | `:13` |
| `icon` | `assets/xrylothmenu/icon.png` (128×128 PNG) | `:14` |
| `environment` | `client` | `:15` |
| `entrypoints.client` | `com.xryloth.menu.XrylothMenu` (only entrypoint) | `:16-20` |
| `mixins` | `["xryloth-menu.mixins.json"]` | `:21-23` |
| `depends` | `fabricloader >=0.16.0`, `minecraft ~1.21.11`, `java >=21` | `:24-28` |
| `suggests` | `modmenu: "*"` (soft — reached by reflection, §3.8) | `:29-31` |

Note the mismatch: the build pins loader `0.19.5` (`gradle.properties:10`) while the
metadata only requires `>=0.16.0`, and the README tells players to install
`0.19.5+` (`README.md:64`).

### 2.2 `src/main/resources/xryloth-menu.mixins.json`

```json
"required": true, "minVersion": "0.8", "package": "com.xryloth.menu.mixin",
"compatibilityLevel": "JAVA_21",
"client": ["TitleScreenMixin","DrawContextMixin","ScreenMixin",
           "PressableWidgetMixin","ClickableWidgetMixin"],
"injectors": { "defaultRequire": 1 }
```
(`xryloth-menu.mixins.json:1-16`)

- Only **concrete mixin classes** are listed. The two duck interfaces
  (`api/ClickableWidgetAccessor`, `api/DrawContextExtension`) must *not* be listed —
  an earlier layout that put them in the mixin package and registered them produced
  `Mixin transformation of com.xryloth.menu.mixin.DrawContextExtension failed` /
  `…ClickableWidgetAccessor failed` (see `run/crash-reports/crash-2026-10-20_13.58.02-client.txt`
  and `crash-2026-10-20_16.36.04-client.txt`).
- `defaultRequire: 1` means every injection must apply or the game refuses to start.
- Runtime log note: mixin warns `Compatibility level JAVA_21 … higher than the maximum
  level supported by this version of mixin (JAVA_13)` and then raises it — harmless,
  but it shows up on every boot (`run/logs/debug.log`, FabricLoader/Mixin lines).

---

## 3. Class-by-class walkthrough

Package layout:

```
com.xryloth.menu
├── XrylothMenu            entrypoint + jar texture loader
├── XrylothBrand           all branding strings / palette constants
├── XrylothPanorama        jar-read cube map, scrim overlay
├── XrylothCatalog         hardcoded catalogue of mods & resource packs
├── XrylothMods            Modrinth client, downloader, install state
├── ModMenuBridge          reflective optional link to Mod Menu
├── api/
│   ├── ClickableWidgetAccessor   duck interface (alpha/hovered/focused)
│   └── DrawContextExtension      duck interface (tinted textured quad)
├── font/MenuFont          LEMON MILK atlas renderer
├── gui/
│   ├── Glass              rounded glass primitives (shared)
│   ├── XrylothMenuScreen  the custom title screen
│   └── XrylothBoostsScreen the installer panel
└── mixin/                 5 mixins (see §4)
```

### 3.1 `XrylothMenu` — client entrypoint (`XrylothMenu.java`, 92 lines)

- `XrylothMenu.java:20` — `implements ClientModInitializer`.
- `:22-23` — `MOD_ID = "xrylothmenu"`, logger `"Xryloth/Menu"`.
- `:26-35` — texture `Identifier`s for the logo and the five dock icons
  (`textures/gui/logo.png`, `icon_discord|website|mods|settings|quit.png`) plus the
  jar-relative `LOGO_RESOURCE` path.
- `:37-40` — `onInitializeClient()` does **only** log a line:
  `"Xryloth Menu ready — custom title screen active ({})", XrylothBrand.WEBSITE_URL`.
  Nothing else runs at boot (an earlier version called `ensureLogo()` here and crashed —
  §9.4).
- `:59-61` — `ensureLogo()` → `ensureTexture(LOGO, LOGO_RESOURCE, "xryloth_logo")`.
- `:71-91` — `ensureTexture(id, resource, name)`:
  1. guards against a null client / texture manager (`:72-75`);
  2. opens the file **from this jar** with `XrylothMenu.class.getResourceAsStream`
     (`:77`);
  3. `NativeImage.read` + `textureManager.registerTexture(id, new NativeImageBackedTexture(...))`
     so the texture is uploaded **immediately** (`:82-86`).

  Why by hand (`:42-58` javadoc, `README.md:175-188`): without Fabric API the mod's
  asset namespace is not registered with the resource manager at all, and a
  resource-manager-registered texture is not initialised until a reload, which crashes
  the first draw (`Texture view does not exist`). Being rebuilt on every menu open also
  survives F3+T.

**Interactions:** called by `XrylothMenuScreen.init()` (`XrylothMenuScreen.java:135-146`)
for the logo and the five icons; `XrylothMods.loadIcon()` uses the same
jar-reading pattern for catalogue icons (`XrylothMods.java:653-673`).

### 3.2 `XrylothBrand` — the single branding file (`XrylothBrand.java`, 102 lines)

A constants-only class (private ctor `:11-12`) so all copy/colours can be changed
without touching logic (`README.md:68-85`).

- Copy: `WORDMARK = "XRYLOTH"` (`:17`), `TITLE = "XRYLOTH CLIENT"` (`:20`),
  `TAGLINE = "YOUR MINECRAFT EXPERIENCE"` (`:23`), `VERSION = "1.0.0"` (`:26`).
- Links: `WEBSITE_URL = "https://xrylothclient.vercel.app"` (`:31`),
  `WEBSITE_LABEL = "Website"` (`:34`),
  `DISCORD_URL = "https://discord.com/invite/YCxYeyEzCE"` (`:37`, empty string hides
  the dock button).
- Palette (ARGB ints, "taken from the Xryloth launcher palette (see tailwind.config.js:
  glass surfaces + neon accents)" — `:41-44`):
  - backdrop: `BACKDROP_SCRIM 0x40280C00` (`:47`), `BACKDROP_TOP 0x59000000` (`:50`),
    `BACKDROP_BOTTOM 0x73000000` (`:53`);
  - glass body idle `GLASS_ROW_TOP 0xBD341804` / `GLASS_ROW_BOTTOM 0xB32E1604`
    (`:56,:59`), hover `0x8CF48C06` / `0x8CDC2F02` (`:62,:65`), hover alpha lift `+6`
    (`:68`);
  - rings `BORDER_SOFT 0x61FF963C` (`:71`), `BORDER_HOVER 0xCCFFBA08` (`:74`),
    top `EDGE_LIGHT 0x26FFD9B0` (`:77`);
  - chips `CHIP_BODY 0xC7301608` (`:80`), `CHIP_HOVER 0xD9964410` (`:83`);
  - text `TEXT 0xFFE9EBED` (`:86`), `TEXT_DIM 0x8CFFFFFF` (`:89`),
    `TEXT_BRIGHT 0xFFFFFFFF` (`:92`), `ICON 0xFFFFD9B0` (`:95`);
  - accents `ACCENT 0xFFFFBA08`, `ACCENT_ORANGE 0xFFF48C06`, `ACCENT_RED 0xFFDC2F02`,
    `ACCENT_DEEP 0xFF9D0208` (`:98-101`).

**Interactions:** consumed by `Glass`, `XrylothPanorama.drawOverlay`,
`XrylothMenuScreen`, `PressableWidgetMixin`.

### 3.3 `XrylothPanorama` — the menu's own cube map (`XrylothPanorama.java`, 160 lines)

- `:32` — `PANORAMA = Identifier.of("xrylothmenu", "textures/background/panorama")`.
- `:34-41` — six jar paths `…/panorama_0.png` … `panorama_5.png`.
- `:43-44` — static `RotatingCubeMapRenderer renderer` and `CubemapTexture texture`.
- `:54-61` — `draw(context, w, h)`: `ensure()` then `renderer.render(context, width,
  height, true)`; returns whether anything was drawn (the mixin uses that to decide
  whether to cancel vanilla's background — `ScreenMixin.java:47-53`).
- `:64-69` — `drawOverlay()`: full-screen `BACKDROP_SCRIM` fill, then a top
  `fillGradient` (`BACKDROP_TOP → transparent`) and a bottom one
  (`transparent → BACKDROP_BOTTOM`). *Despite the javadoc saying "scrim and the
  aurora", no aurora is drawn* — see §9.1.
- `:75-104` — `ensure()` (synchronized): lazily builds the renderer; because
  `TextureManager#getTexture` **creates a placeholder** for unknown ids, it tracks its
  own texture reference instead of looking the id up (`:84-88`); if the tracked texture
  is gone (resource reload) it re-stitches and re-registers (`:90-103`), casting to
  `AbstractTexture` because the `ReloadableTexture` overload would go through the
  resource manager, which cannot see this jar (`:99-101`).
- `:117-154` — `loadStrip()`: reads the six faces, requires them all the same size
  (`:129-135`), allocates `NativeImage(width, height * 6)` and copies each face one
  below the other with `copyRect` (`:137-142`), closing every face in `finally`.
- `:157-159` — `metadata()`: `TextureResourceMetadata(false, true, MipmapStrategy.AUTO, 0.0F)`
  — clamped, no mipmaps, "the same as vanilla's".
- `:107-109` — `isOurs(id)` helper (present but unused by current code).

**Interactions:** only `ScreenMixin` calls it (`ScreenMixin.java:47,52`).

### 3.4 `XrylothCatalog` — the install catalogue (`XrylothCatalog.java`, 120 lines)

Pure data, no I/O.

- `:14-21` — `Kind { MOD, RESOURCE_PACK, DIRECT }` (`DIRECT` = not on Modrinth, fetched
  by URL).
- `:23-24` — `record Entry(String id, String name, Kind kind, boolean recommended)`.
- `:30-46` — `PERFORMANCE` (16 entries, all `recommended=true`): sodium, lithium,
  ferrite-core, immediatelyfast, entityculling, moreculling, dynamic-fps, ksyxis,
  zoomify, cloth-config, yacl, placeholder-api, fabric-api, fabric-language-kotlin,
  modmenu, **betterf3 (`Kind.DIRECT`)**.
- `:49-80` — `OPTIONAL` (31 entries, all `recommended=false`): 3dskinlayers, appleskin,
  badoptimizations, balm, boat-item-view, chat-heads, clumps, continuity, controlling,
  crash-assistant, creativecore, debugify, distanthorizons, entity-model-features,
  forge-config-api-port, geckolib, inventory-profiles-next, iris, jade, jei, konkrete,
  malilib, mouse-tweaks, no-chat-reports, reeses-sodium-options, rei, shulkerboxtooltip,
  skyboxify, sodium-extra, status-effect-bars, **yacl (again)**.
- `:83-99` — `RESOURCE_PACKS` (16 entries, all recommended): translations-for-sodium,
  mt-ct-d, spunky-pvp-texture-pack, ctm-overhaul, small-caps-font, new-glowing-ores,
  default-dark-mode, no-fire-2026, no-pumpkin-blur, no-block-break-particles,
  better-mining-bar, fullbright-ub, glowing-glints, enchant-icons-countxd,
  low-shield-pack, even-better-enchants.
- `:102-104` — the single non-Modrinth file:
  `BETTER_F3_URL = "https://www.curseforge.com/api/v1/mods/401648/files/7436341/download"`,
  `BETTER_F3_FILE = "BetterF3-17.0.0-Fabric-1.21.11.jar"` (both hardcoded).
- `:110-114` — `all()` = PERFORMANCE + OPTIONAL + RESOURCE_PACKS flattened.
- `:117-119` — `performance()`.

**Gotcha:** `yacl` appears in both lists (`:41` and `:80`); `XrylothMods.items()`
keyed by id keeps only the **last** one, so YaCL ends up `recommended=false` and is
*not* pre-selected even though it sits in the "performance" set (§9.2).

### 3.5 `XrylothMods` — metadata, download, install state (`XrylothMods.java`, 728 lines)

The biggest class. Four responsibilities: catalogue metadata, downloading, persistent
state, and icon textures.

**Constants / model**

- `:46-48` — logger `"Xryloth/Mods"`, `API = "https://api.modrinth.com/v2"`,
  `USER_AGENT = "XrylothMenu/1.0 (launcher)"`.
- `:51-75` — `class Item` (mutable): wraps an `Entry` plus runtime fields `version`,
  `loader`, `summary`, `fileName`, `fileUrl`, `iconUrl`, `dependencies`, `installed`,
  `failed`, `error`, `selected` ( initialised from `entry.recommended()` — `:69`), and
  `dependency` (true when pulled in by another mod — `:64-65`).
- `:81` — `PINNED` map ("versions that have to be used instead of the newest one") is
  declared but **empty** — the pinning mechanism exists, unused.
- `:83-86` — static `LinkedHashMap ITEMS`, `Set ICONS_LOADED`, `AtomicBoolean BUSY`,
  `ExecutorService pool`.

**State accessors** — `items()` lazily builds the map (`:91-99`); `isBusy()` (`:102-104`);
`isRestartPending(client)` (`:107-109`); `isInstalled(client)` (`:112-114`) — the last
two read the JSON state file (below).

**Metadata refresh (network)**

- `:118-154` — `refresh(client)`: 4-thread pool; for `DIRECT` entries it fakes the
  metadata (`version "17.0.0"`, `loader "fabric"`, a fixed summary, the fixed file name
  — `:123-128`); otherwise it GETs `/project/<id>` for `summary` + `icon_url`, then
  `resolve(item)`; on failure it stores `item.error`.
- `:159-208` — `resolve(item)`: queries
  `/project/<id>/version?game_versions=["<mc>"]&loaders=["fabric"]` (unfiltered for
  `RESOURCE_PACK` — `:165-167`); if empty *and* the entry is a dependency it retries
  without the game-version filter (`:171-177`) so a hard dependency never blocks boot;
  picks `versions.get(0)` (newest) unless `PINNED` matches a `version_number`
  (`:184-196`); then reads `files[0].filename/url` and
  `dependencies = requiredDependencies(chosen)`.
- `:214-235` — `requiredDependencies()`: keeps only `dependency_type == "required"`,
  and resolves Modrinth's short project **hash** to a slug via `projectSlug()`
  (`:241-255`), because dependency lists use ids like `1eAoo2KR`.
- `:721-727` — `mcVersion()` = `SharedConstants.getGameVersion().name()`, falling back
  to `"1.21.11"`.

**Install**

- `:265-374` — `install(client, onDone)`:
  - `BUSY.compareAndSet(false, true)` guard (`:266`) — a second call while busy is
    silently ignored;
  - collects `item.selected` entries (`:270-275`);
  - single-thread daemon executor named `"Xryloth Mod Installer"` (`:282-286`);
  - resolves `mods/` and `resourcepacks/` under `client.runDirectory` and creates them
    (`:290-297`);
  - a work queue with `guard < 200` iterations (`:303`) that, for each item,
    re-`resolve`s it, enqueues its required dependencies as synthetic `Item`s
    (`:307-323`, skipping self-references `:308`), downloads to a `.part` file and
    atomically moves it into place (`:325-337`), records
    `installed[entry.id] = fileName` (`:340`) and, for resource packs, adds the file to
    `enabledPacks` (`:341-343`);
  - per-item failures set `item.failed`/`item.error` and continue (`:344-348`);
  - afterwards enables the packs in `options.txt` (`:351-353`), then writes state:
    `installed=true`, **`restartPending=true`**, `installedAt=now`, `saved` (selected
    ids), `files`, `dependencies` (`:355-368`).
- `:384-410` — `cleanup(client)`: deletes jars whose project id is no longer in the
  catalogue (uses `state.files` because "a mod's file rarely starts with its id" —
  `:380-382`); only `.jar` files are removed (`:399`), so stale resource packs linger.
- `:414-435` — `download()`: `HttpURLConnection`, redirects on, 30 s connect / 120 s
  read timeouts, UA header, `.part` then `Files.move(REPLACE_EXISTING)`. URL is
  `BETTER_F3_URL` for `DIRECT`, else `item.fileUrl`, else a fresh `downloadUrl()`
  (`:438-451`, which string-scrapes `"url"` out of the JSON with `string()/between()`
  — `:705-719`).
- `:454-505` — `enableResourcePacks()`: rewrites the `resourcePacks:[…]` line of
  `runDirectory/options.txt`, preserving existing entries and appending the new pack
  names (`.zip` stripped) — a **hand-rolled options.txt edit** (§7).

**State files**

- `:600-602` — `stateFile` = `<run>/config/xryloth-menu/installs.json`.
- `:509-518` — `State { installed, restartPending, installedAt, saved, files, dependencies }`.
- `:520-560` — `state()` parses it with Gson `JsonParser`.
- `:562-598` — `save()` **serialises JSON by hand** (StringBuilder, no escaping) and
  writes it non-atomically.
- `:604-642` — a *second*, separate status file
  `<run>/config/xryloth-menu/mods.status` holding `first-time` / `Restarted` plus a
  user-facing comment line; `getModStatus()` (`:611-624`) returns `FIRST_TIME` when the
  file is absent, `setModStatus()` (`:627-638`) writes it.

**Textures**

- `:653-673` — `loadIcon(client, item)`: jar-reads
  `/assets/xrylothmenu/textures/icons/<id>.png` (baked by `bake-icons.sh`) and
  registers it once per id (`ICONS_LOADED`).
- `:676-683` — `badgeColor(id)`: stable hash → one of six ember colours, used when an
  icon is missing (fallback badge with the name's initial).

**HTTP helper** — `:687-703` builds a **new `HttpClient` per call** (20 s connect,
40 s request timeout, `Accept: application/json`, non-2xx → `IOException`).

**Interactions:** `XrylothMenuScreen.init` triggers the state machine (`:247-281`),
`XrylothBoostsScreen` drives `refresh()` and `install()`, `XrylothCatalog` supplies the
ids.

### 3.6 `gui/Glass` — shared glass primitives (`Glass.java`, 201 lines)

Static helpers only (`:15-16`), used by both the custom screens and the vanilla-button
mixin so everything looks identical (`Glass.java:10-11`).

- `:19-36` — `fillRounded`: a rounded rect built from **stepped 1-px-tall `fill` calls**
  (one logical pixel per corner row) plus a full-width middle band.
- `:68-72` — `cornerInset(row, radius)` = circular-arc inset:
  `radius - round(sqrt(radius² - (radius-row-0.5)²))`.
- `:42-65` — `fillRoundedGradient`: same shape, each row coloured by
  `lerpColor(colorTop, colorBottom, row/(height-1))`; falls back to `fillGradient` when
  radius is 0 or the colours are equal.
- `:75-110` — `ringRounded`: a **true 1 px ring drawn on top of the body** (the README
  explains why: a translucent ring under a translucent body stacks both alphas —
  `README.md:120-123`); walks the four corner arcs and the four straight edges.
- `:118-141` — `button(...)`: gradient body (idle→hover lerp on `highlight`), ring
  (alpha scaled by `alpha * ringScale`), then a 1 px `EDGE_LIGHT` line across the top
  whose alpha grows with the highlight (`:138-140`). `ringScale` (default 1.0, vanilla
  buttons pass 0.7 — `PressableWidgetMixin.java:79`) softens small outlines.
- `:144-153` — `chip(...)`: the smaller square treatment for the dock and corner chips.
- Colour utilities: `clamp01` (`:160`), `lit()` (adds `GLASS_HOVER_LIFT` to the alpha —
  `:165-168`), `scaleAlpha` (`:171-174`), two `withAlpha` overloads (`:176-182`),
  `lerpColor` per ARGB channel (`:184-196`), `lerp` (`:198-200`).

### 3.7 `font/MenuFont` — baked LEMON MILK renderer (`MenuFont.java`, 322 lines)

Why baked (`:20-31`, `README.md:144-159`): without Fabric API a font cannot be
registered through the resource manager, so each weight is rasterised at build time and
drawn glyph-by-glyph — which also makes every string freely tintable (including the
hover fade).

- Weights: `REGULAR`, `BOLD`, `LIGHT` (`:34-41`).
- `:43-61` — records `Glyph(advance, inkX, inkY, inkW, inkH)` and
  `Raster(size, cellW, cellH, columns, originX, originY, gridX, gridY, capHeight, glyphs)`;
  `Face` holds the texture id `xrylothmenu:font/lemon_<weight>` (`:59`) and the list of
  rasters.
- `:75-106` — `ensureLoaded()` (synchronized): for each weight, `parseMetrics()` from
  `/assets/xrylothmenu/font/lemon_<weight>.txt`, then jar-reads
  `…/font/lemon_<weight>.png` and registers it as a `NativeImageBackedTexture`
  (`:88-95`); called on **every menu open** so F3+T rebuilds it (`:69-73`).
- `:266-321` — `parseMetrics()`: line-based parser; `#` comments; `atlas <w> <h>`;
  `size …` appends a `Raster` (`:289-299`); `glyph <code> <advance> <inkX> <inkY> <inkW> <inkH>`
  fills the *last* raster (`:300-311`). Valid only if atlas size and at least one raster
  exist (`:320`).
- `:113-136` — `width(text, weight, capHeight[, tracking])`: picks the raster, scales
  advances by `capHeight / raster.capHeight`, adds `tracking` between chars.
- `:139-193` — drawing: `draw` (centred), `drawShadowed`, `drawLeft`; the core
  (`:174-193`) computes the pen from `width(...) - tracking` so trailing tracking does
  not shift optical centring (`:184-186`), and for a shadow draws the whole string once
  more at `+1,+1` in `0x50000000` (`:188-191`).
- `:195-228` — `drawGlyphs()`: per char, cell position from
  `index = character - 32`, `columns` and `cellW/cellH` (`:206-208`); destination rect
  from `pen + (inkX - originX) * scale` etc. (`:210-213`); UVs computed **inside the
  cell** so the glyph lands exactly on the pen (`:215-220`); each glyph is one
  `((DrawContextExtension) context).xryloth$drawTexturedQuad(...)` call
  (`:222-223`) with the tint.
- `:234-248` — `pick()`: chooses the raster minimising
  `|log(raster.capHeight / (capHeight * guiScale))|` — i.e. the closest physical pixel
  size, so small text is not a blurry downscale of a big glyph.
- `:250-260` — `guiScale()` from `options.getGuiScale()` (or the window scale factor),
  min 1.

**Shipped data** (`src/main/resources/assets/xrylothmenu/font/`):

| file | atlas | rasters (`size` lines) | glyphs |
|---|---|---|---|
| `lemon_bold.txt` | 928×618 | 12 (cell 16×16, cap 10), 24 (30×30, cap 19), 48 (58×57, cap 38) | 285 = 95×3 |
| `lemon_regular.txt` | 880×618 | 12 (16×16), 24 (29×30), 48 (55×57) | 285 |
| `lemon_light.txt` | 864×618 | 12 (15×16), 24 (28×30), 48 (54×57) | 285 |

Each has matching `lemon_<weight>.png`. The author's notice ships at
`src/main/resources/assets/xrylothmenu/fonts/LEMON_MILK_LICENSE.txt` (donationware,
free for personal/non-profit use, donation requested commercially — note the directory
is `fonts/` (plural) while the atlases are in `font/`).

### 3.8 `ModMenuBridge` — optional Mod Menu link (`ModMenuBridge.java`, 52 lines)

- `:18-19` — `MOD_ID = "modmenu"`, `API_CLASS = "com.terraformersmc.modmenu.api.ModMenuApi"`.
- `:25-31` — `isPresent()` = `FabricLoader.isModLoaded("modmenu")` (any `Throwable` →
  false).
- `:38-51` — `open(parent)`: reflection
  `Class.forName(API_CLASS).getMethod("createModsScreen", Screen.class).invoke(null, parent)`
  then `MinecraftClient.setScreen(...)`; failures are logged and return false.
- Because Mod Menu is not a dependency, the dock shows the Mods icon only when
  `isPresent()` (`XrylothMenuScreen.java:209-231`), and `fabric.mod.json` only
  *suggests* it (`:29-31`).

### 3.9 `gui/XrylothMenuScreen` — the title screen (`XrylothMenuScreen.java`, 577 lines)

`extends Screen` (`:44`) but **adds no vanilla widgets** — everything is drawn and
hit-tested by hand, because "Minecraft 1.21.11 made `PressableWidget#renderWidget`
final and moved button painting behind it, so a custom widget can no longer own its
own look" (`:35-38`, `README.md:117-123`).

**Geometry constants (`:48-90`)** — logo 104 px (min 44), primary buttons 120×22
(min height 18, gap 6 min 4, radius 6), dock icons 24 px (min 20, gap 8), corner chips
26 px at margin 10; vertical rhythm `TOP_MARGIN 46`, `TAGLINE_OFFSET 38`,
`BOTTOM_ROOM 56`; type sizes (cap heights) `TITLE_SIZE 22`, `BUTTON_SIZE 8.5`,
`SMALL_SIZE 7.5`, `BADGE_SIZE 7`; `LABEL_TRACKING 0.8`;
`HIGHLIGHT_SPEED 20.0F` (per-second hover easing); `STEVE` =
vanilla `textures/entity/player/wide/steve.png` (`:86-87`).

**State (`:96-127`)** — `Entry(x, y, label, accent, action)` and
`IconButton(x, y, icon, label, accent, action)` records; `highlight[]` one float per
focusable element (entries then icons); layout fields; `focusedIndex` for keyboard nav;
`lastFrameMs` for delta time.

**`init()` (`:133-282`)**

1. `:135-146` — (re)load the logo, the font atlases and the five dock icons from the
   jar (survives F3+T).
2. `:147-152` — reset lists, focus, mouse tracking.
3. `:154-178` — **adaptive layout**: start at the nominal sizes and shrink logo (−4),
   then icons (−2), then button height (−1) and gap until the whole stack fits
   `height - TOP_MARGIN - BOTTOM_ROOM` (`blockHeight`, `:284-289`).
4. `:180-197` — compute the centred column positions.
5. `:198-204` — the three primary actions: `SINGLEPLAYER` → `SelectWorldScreen`,
   `MULTIPLAYER` → `MultiplayerScreen`, **`BOOSTS` → `XrylothBoostsScreen`**.
6. `:206-238` — the dock, left-to-right: Website (if `WEBSITE_URL` non-empty), Discord
   (if `DISCORD_URL` non-empty), Mods (only if `ModMenuBridge.isPresent()` — dock count
   5 vs 4, `:209-211`), Options → `OptionsScreen`, Quit → `client.scheduleStop()`.
   Each icon carries its own accent used for hover/tooltip tinting.
7. `:240-243` — allocate `highlight[]`, place the profile chip at `(10,10)`.
8. `:245-281` — **installer state machine** (see §6.4): read `mods.status` +
   `installs.json`, run `cleanup()`, auto-install when appropriate, and possibly flip
   the status file.

**`render()` (`:299-342`)**

- `:301` `super.render` → vanilla `renderBackground` → `ScreenMixin` paints the
  panorama (§4.2).
- `:303-314` — frame delta and "mouse moved ⇒ keyboard focus is dropped".
- `:316` — `updateHighlight()` eases each element's highlight toward hovered/focused
  using `1 - exp(-delta * HIGHLIGHT_SPEED)` (`:482-499`).
- `:318-323` — the logo halo (`drawLogoGlow`, `:344-356`: two enlarged tinted copies at
  scale 1.16 (`0x1FFF2A12`) and 1.08 (`0x33FF5A1E`)) then the logo itself via vanilla's
  untinted `context.drawTexturedQuad` (`:320-322`).
- `:325-330` — `TITLE` in `MenuFont.BOLD` at 22 px white with shadow, `TAGLINE` in
  `REGULAR` 7.5 px dimmed.
- `:332-337` — draw entries (`Glass.button` + label, `:373-383`) and dock chips
  (`Glass.chip` + tinted icon quad at 62 % size + hover tooltip pill, `:385-403`).
- `:339-341` — installer status banner, profile chip, footer.

**Other methods**

- `drawProfileChip` (`:405-433`) — chip with the default Steve face drawn from the
  vanilla skin texture (face layer at `64,64` UV, `:419-426`); on hover a pill
  `"Playing as <session username>"` (`:428-432`).
- `drawBoostStatus` (`:435-454`) — the installer banner, centred below the dock:
  blue `INSTALLING PERFORMANCE MODS…` while busy, orange
  `RESTART MINECRAFT TO LOAD THE NEW MODS` when `restartPending`, green
  `PERFORMANCE MODS INSTALLED` when installed, otherwise nothing.
- `drawFooter` (`:456-466`) — bottom-left `Xryloth Client <VERSION> (<mc version>)`,
  bottom-right `Copyright Mojang AB. Do not distribute!`.
- `drawTooltipPill` (`:468-480`) — rounded dark pill (`0xD80A0C11`) + faint white ring,
  used for dock and profile tooltips.
- `activate` (`:513-516`) — plays the vanilla UI click sound then runs the action.
- Input: `mouseClicked(Click, boolean)` (`:520-537`) hit-tests entries then icons;
  `keyPressed(KeyInput)` (`:539-569`) implements ↑/↓/←/→/Tab/Shift-Tab cycling and
  Enter/Space activation across `entries + icons`; `shouldCloseOnEsc()` returns `false`
  (`:571-574`).

### 3.10 `gui/XrylothBoostsScreen` — the installer panel (`XrylothBoostsScreen.java`, 329 lines)

See §6 for the full behavioural description; structurally:

- `:24` `extends Screen`, holds `parent` (`:30-41`), a snapshot `List<Item>` and a
  parallel `List<Identifier>` of icons (`:31-32`), plus `refreshing/finished/scroll`.
- `:43-62` `init()` — copies `XrylothMods.items().values()`, then starts a **daemon
  thread `"Xryloth Mod Catalogue"`** that runs `XrylothMods.refresh(...)` and clears
  `refreshing` on the render thread.
- `:64-157` `render()` — title/subtitle, status line, RAM warning, the scrolling list
  panel, and the action buttons.
- `:159-205` `drawRow()` — checkbox, 24 px icon (or hashed-colour badge with the
  initial), name (green installed / red failed / white), `loader · version` meta line,
  summary (or the error text).
- `:207-215` `drawButton()` — plain rounded button (different, greyer treatment than
  the ember `Glass` look).
- `:217-293` `mouseClicked()` — re-derives button geometry and dispatches; also toggles
  checkboxes in the list.
- `:295-299` `mouseScrolled()` — clamps `scroll` to `maxScroll()` (`:301-306`),
  18 px per notch.
- `:308-315` — ESC returns to `parent`; `:317-320` `shouldCloseOnEsc()` false.

### 3.11 `api/` interfaces (duck types)

- **`ClickableWidgetAccessor`** (`api/ClickableWidgetAccessor.java:9-19`) — exposes
  `xryloth$alpha()`, `xryloth$hovered()`, `xryloth$focused()`; implemented on
  `ClickableWidget` by `ClickableWidgetMixin` so `PressableWidgetMixin` can read the
  widget's private state (`:6-8` javadoc).
- **`DrawContextExtension`** (`api/DrawContextExtension.java:12-30`) — one method
  `xryloth$drawTexturedQuad(texture, x1, y1, x2, y2, u1, u2, v1, v2, color)`; needed
  because vanilla `DrawContext#drawTexturedQuad` cannot tint and the glyph atlas is
  white (`:6-10` javadoc). Implemented by `DrawContextMixin`.

---

## 4. Mixin strategy

Config: `src/main/resources/xryloth-menu.mixins.json` (§2.2), package
`com.xryloth.menu.mixin`, `defaultRequire: 1`.

### 4.1 `TitleScreenMixin` → `net.minecraft.client.gui.screen.TitleScreen`

```java
@Inject(method = "init", at = @At("HEAD"), cancellable = true)
private void xryloth$replaceTitleScreen(CallbackInfo ci) { … client.setScreen(new XrylothMenuScreen()); ci.cancel(); }
```
(`TitleScreenMixin.java:17-28`)

**Why:** cancelling at the *head* of `init` means vanilla never builds its buttons nor
draws its own panorama (`:14-15`). The screen swap is a second `setScreen`, so
`XrylothMenuScreen.init()` runs in place of `TitleScreen.init()`.

### 4.2 `ScreenMixin` → `net.minecraft.client.gui.screen.Screen`

```java
@Inject(method = "renderBackground", at = @At("HEAD"), cancellable = true)
private void xryloth$panorama(DrawContext, int, int, float, CallbackInfo)
```
(`ScreenMixin.java:39-54`)

Behaviour:

1. bail out if there is no client **or a world is loaded** (`:42-45`) — screens opened
   in-game keep the vanilla blurred-game-world backdrop (`:21-23`);
2. `XrylothPanorama.draw(...)`; if it fails, do nothing and let vanilla run (`:47-49`);
3. `this.applyBlur(context)` — vanilla's own blur, driven by the player's
   *Menu Background Blurriness* setting (shadowed method, `:34-37`);
4. `XrylothPanorama.drawOverlay(...)` — scrim + vignette gradients (`:52`);
5. `ci.cancel()` so vanilla's panorama/scrim is not drawn (`:53`).

It shadows `width`/`height` (`:28-32`) because they are `protected` fields of `Screen`.

**Why:** vanilla draws its rotating panorama on *every* menu screen while no world is
loaded (multiplayer, options, world selection), so replacing it here puts the whole
client on one background instead of only the title screen (`:16-19`).

### 4.3 `DrawContextMixin` → `net.minecraft.client.gui.DrawContext`

Implements `DrawContextExtension` (`:27-28`). Shadows the `@Final` fields `client`,
`state` (`GuiRenderState`) and `matrices` (`Matrix3x2fStack`) (`:30-40`), then for
`xryloth$drawTexturedQuad` (`:42-62`):

1. looks the texture up in the texture manager (returns if null, `:45-48`);
2. takes `GpuTextureView` + `GpuSampler` from the `AbstractTexture` (`:50-51`);
3. uses `RenderPipelines.GUI_TEXTURED` (`:52`);
4. pushes `new TexturedQuadGuiElementRenderState(pipeline, TextureSetup.of(view, sampler),
   new Matrix3x2f(matrices), x1,y1,x2,y2, u1,u2,v1,v2, color, null)` into
   `state.addSimpleElement(...)` (`:54-61`).

**Why:** this mirrors vanilla `drawTexturedQuad` but forwards the ARGB colour so the
vertex colour multiplies the texture — the whole tinted-glyph / tinted-icon / logo-halo
system depends on it (`:22-25`).

### 4.4 `PressableWidgetMixin` → `net.minecraft.client.gui.widget.PressableWidget`

Restyles Minecraft's *own* buttons (world select, multiplayer, options…).

- `:46-57` — injects at `RETURN` of `<init>(int,int,int,int,Text)` and **trims the
  widget to 70 %** (`SCALE = 0.7F`, `:30`), re-centring it inside its original box; only
  for buttons at least `TEXT_BUTTON_WIDTH = 60` px wide (`:33`) so icon-only buttons are
  untouched. The trim must happen after construction because the size is set in the
  super constructor (`:42-45`).
- `:59-82` — `drawButton` at `HEAD`, cancellable: reads alpha/hover/focus through the
  `ClickableWidgetAccessor` duck (`:62`), skips invisible widgets (`:70-73`), then draws
  `Glass.button(..., radius = min(6, height/2), highlight = hovered||focused, alpha,
  ringScale 0.7)` and **cancels** the vanilla grey sprite (`:78-81`).
- `:90-94` — `renderWidget` at `HEAD` captures the `DrawContext` into a `@Unique` field
  (because `drawLabel` only receives a text consumer, `:84-89`).
- `:96-126` — `drawLabel` at `HEAD`, cancellable: upper-cases the message, picks a cap
  height of `7.0F` and shrinks it if the text would overflow the widget (`:115-120`),
  then draws it centred in `MenuFont.REGULAR` with `XrylothBrand.TEXT` and cancels
  vanilla (`:122-125`).

**Why:** so vanilla screens look like they belong to Xryloth (`README.md:40-42`), and
because the vanilla button sprite cannot be reused once painting moved behind a `final`
`renderWidget` (`README.md:117-123`).

### 4.5 `ClickableWidgetMixin` → `net.minecraft.client.gui.widget.ClickableWidget`

Implements `ClickableWidgetAccessor` by `@Shadow`ing `protected float alpha`,
`protected boolean hovered`, `private boolean focused` and returning them
(`ClickableWidgetMixin.java:9-35`). No injections.

### 4.6 Mixins deliberately **not** used

- No `MinecraftClientMixin` — an earlier debug screenshot hook
  (`handler$zzb000$xrylothmenu$xryloth$debugScreenshot`, visible in
  `run/crash-reports/crash-2026-10-19_20.09.46-client.txt`) and its `DebugShot` class
  are gone from the source (`README.md:132-135` says the hook was temporary and "not
  part of the shipped jar").
- Interfaces are **not** mixins (see §2.2).

---

## 5. Rendering: how the pieces fit together

### 5.1 The backdrop (panorama + blur + scrim)

```
Screen.render → renderBackground
   └─ ScreenMixin (HEAD, cancelled when world==null)
        ├─ XrylothPanorama.draw  → RotatingCubeMapRenderer.render(ctx, w, h, true)
        │     └─ ensure(): 6 jar PNGs → one vertical strip NativeImage →
        │        CubemapTexture.reload(TextureContents) → registerTexture
        ├─ applyBlur(context)          (vanilla blur, "Menu Background Blurriness")
        └─ XrylothPanorama.drawOverlay (scrim + top/bottom vignette gradients)
```

- Faces are 512×512 in the jar; the 1024×1024 masters live in `panorama-source/`
  (`README.md:172-173`; verified: `panorama_0.png` 512×512, `panorama-source/panorama_0.png`
  1024×1024).
- The stitched strip is `width × (height × 6)` (`XrylothPanorama.java:137`); the game
  then rotates/blurs/darkens it exactly like the vanilla title panorama
  (`README.md:164-170`). A wrong-sized strip historically crashed with
  `Cubemap compatible textures must be square, but size is 16x2`
  (`crash-2026-10-20_16.13.09-client.txt`).
- Because the mod has no Fabric API, the panorama/logo/font/icon PNGs are **never**
  served by the resource manager; all three subsystems (`XrylothPanorama.ensure`,
  `XrylothMenu.ensureTexture`, `MenuFont.ensureLoaded`) read their bytes out of the jar
  with `getResourceAsStream` and register `NativeImageBackedTexture`/`CubemapTexture`
  manually, on demand, and rebuild themselves after F3+T.

### 5.2 Glass panels

Everything is `DrawContext.fill` / `fillGradient` — no textures:

- rounded rectangles = per-row stepped fills with a circular corner inset
  (`Glass.java:19-72`);
- the outline is a separate 1 px ring drawn **over** the body (`:75-110`) so two
  translucencies never stack;
- buttons additionally get a top `EDGE_LIGHT` hairline and a body gradient that lerps
  from the ember idle colours to the orange/red hover colours, driven by a
  `highlight` float eased at `HIGHLIGHT_SPEED = 20/s`
  (`XrylothMenuScreen.java:482-499`);
- chips (dock, corner) use the smaller `CHIP_BODY/CHIP_HOVER` pair (`Glass.java:144-153`).

Vanilla buttons get the identical treatment through `PressableWidgetMixin`, at 70 %
size with a softer ring (`ringScale 0.7`).

### 5.3 The brand block

- Logo halo = two scaled tinted quads via the duck method
  (`XrylothMenuScreen.java:344-371`), logo itself via vanilla's untinted quad (`:320-323`).
- `XRYLOTH CLIENT` headline: `MenuFont.BOLD`, cap height 22, white with a 1 px
  `0x50000000` shadow (`:326-327`); tagline `REGULAR` 7.5 at 88 % alpha (`:328-330`).
- All text goes through `MenuFont`, which picks the raster closest to the *physical*
  pixel size (12/24/48 × GUI scale) and draws white glyph quads tinted per string.

### 5.4 The custom font atlas

Build time (`scripts/tools/GenerateFontAtlas.java`): rasterise ASCII 32-126 at 12/24/48
into one PNG per weight, 16 columns, white ink, write `atlas`/`size`/`glyph` metrics.

Runtime (`MenuFont`): parse metrics → register PNG as a texture → for each glyph compute
destination rect from `advance/capHeight` scaling and UVs inside the grid cell → one
tinted quad per glyph. Shadow = second pass at `+1,+1` in `0x50000000`.

### 5.5 Input & animation on the title screen

- No vanilla widgets: `mouseClicked` hit-tests records (`:520-537`), keyboard focus
  cycles over `entries + icons` (`:539-569`), moving the mouse clears keyboard focus
  (`:310-314`).
- Hover highlight is exponential easing, so the ember bloom is smooth at any FPS
  (`:482-499`), and the vanilla click sound plays on activation (`:513-516`).

---

## 6. The Boosts screen

### 6.1 What a "boost" is

The **Xryloth performance catalogue**: a curated list of Fabric mods (the
"PERFORMANCE" set) plus optional mods and resource packs that the client will download
and install into the running instance for the player. The panel is reached from the
third primary button, `BOOSTS` (`XrylothMenuScreen.java:202-204`), and the header calls
it `XRYLOTH BOOSTS / PERFORMANCE MODS & RESOURCE PACKS`
(`XrylothBoostsScreen.java:72-75`).

It exists because "Minecraft reads its `mods` folder while it boots, so a mod that is
installed while the game is running is only picked up after a restart"
(`XrylothMods.java:39-42`) — hence the panel's constant restart messaging.

### 6.2 Where the data comes from

Two sources, **neither of them the launcher**:

1. **The hardcoded catalogue** — `XrylothCatalog.PERFORMANCE / OPTIONAL / RESOURCE_PACKS`
   (`XrylothCatalog.java:30-99`). `XrylothBoostsScreen.init()` simply copies
   `XrylothMods.items().values()` (`XrylothBoostsScreen.java:47-50`), so the row list,
   names and the default checkbox state (`Item.selected = entry.recommended()`,
   `XrylothMods.java:69`) are fixed at compile time.
2. **The Modrinth API at runtime** — a background thread calls
   `XrylothMods.refresh()` (`XrylothBoostsScreen.java:56-61`), which fills `version`,
   `loader`, `summary`, `iconUrl`, `fileName`, `fileUrl`, `dependencies` per entry
   (`XrylothMods.java:118-208`). Icons are *not* fetched at runtime: they are baked
   into the jar (`bake-icons.sh`) and loaded by `XrylothMods.loadIcon()`
   (`XrylothMods.java:653-673`); when an icon is missing the row shows a hashed-colour
   badge with the initial (`:176-187`, colour from `badgeColor` `:676-683`).

**`run/config/xryloth-menu/installs.json` is not the boosts list** — it is the
installer's own state file, written by the mod itself:

- path: `XrylothMods.java:600-602` → `<runDirectory>/config/xryloth-menu/installs.json`;
- written by `save()` at the end of every install (`XrylothMods.java:355-368`, JSON
  hand-assembled at `:562-598`);
- read by `state()` for `isInstalled()` / `isRestartPending()` (`:520-560`) and for
  `cleanup()` (`:384-410`).

The launcher (`XrylothTauri/src`, `src-tauri/src`) contains **no reference** to
`installs.json`, `mods.status`, `xryloth-menu` or `xrylothmenu` (grep across `src/` and
`src-tauri/src/` returns nothing), so the file is mod-private; the launcher neither
writes nor reads it.

Actual contents after a real run (2026-10-21, `run/config/xryloth-menu/installs.json`):

```json
{
  "installed": true,
  "restartPending": true,
  "installedAt": 1792602578125,
  "saved": ["sodium","lithium","ferrite-core", … ,"even-better-enchants"],   // 31 ids
  "dependencies": ["fabric-api","cloth-config","fabric-language-kotlin","yacl",
                   "placeholder-api","sodium","fusion-connected-textures"],
  "files": {"sodium":"sodium-fabric-0.8.15-beta.1+mc1.21.11.jar", …}          // id → filename
}
```

Note `yacl` appears only under `dependencies` (it was pulled in as Zoomify's
requirement) and `fusion-connected-textures` appears only as a dependency of the CTM
packs — exactly the behaviour described in §3.4/§3.5.

### 6.3 What the screen shows and does

Layout (`XrylothBoostsScreen.java:64-157`):

- header at `top = 26`: title (`:72-73`), subtitle (`:74-75`);
- status line (`:78-90`): blue `DOWNLOADING...` while `isBusy()`, orange
  `RESTART MINECRAFT TO LOAD THE NEW MODS` when `restartPending`, green
  `INSTALLED - THE MODS LOAD ON YOUR NEXT START`, else grey
  `XRYLOTH CAN INSTALL THESE FOR YOU`;
- a permanent explainer line: `MINECRAFT READS ITS MODS FOLDER WHILE IT STARTS, SO A
  RESTART IS NEEDED` (`:92-94`);
- RAM warnings (`:96-113`): if more than 25 items are selected →
  `… GIVE THE GAME AT LEAST 4 GB OF RAM`; else if `Runtime.maxMemory() < 2048 MB` →
  `ONLY <n> MB OF RAM IS AVAILABLE TO MINECRAFT`;
- list panel: `min(540, width-32)` wide, filled `0xCC0B0B0E` with a `0x26FFFFFF` ring
  (`:116-122`), rows of `ROW_HEIGHT = 34` cull-rendered against `scroll`
  (`:124-135`);
- each row (`:159-205`): 14 px checkbox (blue when selected), 24 px icon or badge,
  name (green/red/white), `loader · version`, summary/error truncated to 74 chars;
- buttons (`:141-156`), laid out as up to five slots:
  `INSTALL`, `PERFORMANCE` (reset selection to recommended), `SELECT ALL`, then
  `RESTART` **only when `restartPending`**, then `BACK`.

Click handling (`:217-293`) re-derives the same geometry and maps slot index → action
(`INSTALL` → `XrylothMods.install(...)`; `RESTART` → `client.scheduleStop()`;
`BACK` → `setScreen(parent)`), and a second pass toggles the checkbox under the click
(`:276-291`). Scrolling (`:295-306`) and ESC-to-parent (`:308-315`) round it out.

### 6.4 The install flow and the state machine

On every title-screen open (`XrylothMenuScreen.java:245-281`):

```
modStatus = getModStatus()          // reads config/xryloth-menu/mods.status (absent → FIRST_TIME)
installed = isInstalled()           // reads installs.json
restartPending = isRestartPending() // reads installs.json
cleanup()                           // delete jars of ids that left the catalogue
if FIRST_TIME && !installed && !busy  → install()
if RESTARTED  &&  restartPending      → setModStatus(FIRST_TIME) + maybe install()
if FIRST_TIME && installed && !restartPending → setModStatus(RESTARTED)
```

`install()` (§3.5) downloads everything to `mods/` / `resourcepacks/`, appends the new
packs to `options.txt` (`XrylothMods.java:454-505`), then writes
`installed=true, restartPending=true`. The banner in `drawBoostStatus`
(`XrylothMenuScreen.java:435-454`) reports the three states to the player.

Observed in `run/logs/debug.log`: the background thread `Xryloth Mod Installer` pulled
sodium, lithium, ferritecore, ImmediatelyFast, entityculling, moreculling, dynamic-fps,
Ksyxis, zoomify, cloth-config, placeholder-api, fabric-api, fabric-language-kotlin,
modmenu, BetterF3 and 16 resource packs, then re-encountered already-present files as
dependency re-queues (`"… is already installed"`), and logged
`could not read badoptimizations from Modrinth` (see §9.2).

---

## 7. How the launcher and the mod communicate

**There is no socket, port, or IPC channel.** A grep for `Socket|localhost|127.0.0.1`
over `src/` finds no networking in the mod other than outbound HTTPS. Everything is
**file-based through the shared game/instance directory**, plus the mod's own outbound
HTTP:

### 7.1 Files shared with the launcher / the game

| Path (relative to the instance dir) | Written by | Read by |
|---|---|---|
| `mods/*.jar` | mod installer (`XrylothMods.java:290,327-337`); launcher's own mod manager | Fabric Loader at boot |
| `resourcepacks/*.zip` | mod installer (`:327-329`) | the game |
| `options.txt` → `resourcePacks:[…]` line | **both**: mod (`XrylothMods.java:454-505`) and launcher (`src-tauri/src/lib.rs:5444-5496` `auto_enable_resourcepack`, plus `get_optimizer_status`/`apply_optimizer` at `lib.rs:3048,3068,5448`) | the game |
| `config/xryloth-menu/installs.json` | mod only (`XrylothMods.java:562-602`) | mod only (`:520-560`) |
| `config/xryloth-menu/mods.status` | mod only (`:627-638`) | mod only (`:611-624`) |
| per-mod configs under `run/config/` | each third-party mod | each mod |

The launcher's instance directory (`instances_dir()`) and the mod's
`client.runDirectory` are the same folder when the launcher starts the game, which is
the entire "protocol": the launcher provisions instances and can enable packs itself;
the mod installs the Xryloth catalogue and flips the same `options.txt` key.

Note the two writers use **different formats** for the same key — the launcher appends
`:"<file>"` entries (`lib.rs:5462-5486`), the mod writes `"<name>"` entries with the
`.zip` stripped (`XrylothMods.java:473-487`). The game tolerates both, but they are not
aware of each other (§9.6).

### 7.2 Network the mod performs itself

- `https://api.modrinth.com/v2/...` — project metadata, version lists, dependency slug
  lookups (`XrylothMods.java:47,134,168,175,246,442`), UA `XrylothMenu/1.0 (launcher)`
  (`:48`).
- `https://www.curseforge.com/api/v1/mods/401648/files/7436341/download` — BetterF3
  (`XrylothCatalog.java:102-104`), via `HttpURLConnection` with redirects
  (`XrylothMods.java:414-435`).
- Outbound browser links: `Util.getOperatingSystem().open(url)` for Website/Discord
  (`XrylothMenuScreen.java:291-293`).

Nothing is sent to the launcher: there is no telemetry, no callback URL, no local
server.

---

## 8. Bundled modpack / resourcepack / performance-config inventory (`run/`)

`run/` is the Loom client run directory (`build.gradle:21`), and it is also where real
play sessions happened (logs 2026-10-19 → 2026-10-21, nine crash reports, screenshots).
The state below was produced by the mod's own installer on 2026-10-21 22:38-22:39
(`installs.json` `installedAt`, `run/logs/debug.log`).

### 8.1 Mods (`run/mods/`, 17 jars) — the "performance" set actually installed

| Jar | Version | Role |
|---|---|---|
| `sodium-fabric` | `0.8.15-beta.1+mc1.21.11` | rendering engine replacement |
| `lithium-fabric` | `0.21.4+mc1.21.11` | general game/tick optimisation |
| `ferritecore` | `8.2.0-fabric` | memory dedup (blockstates, quads, models) |
| `ImmediatelyFast-Fabric` | `1.14.3+1.21.11` | immediate-mode rendering batching |
| `entityculling-fabric` | `1.11.3-mc1.21.11` | skip invisible entities/block entities |
| `moreculling-fabric` | `1.21.11-1.6.2` | extra culling (clouds, signs, leaves, item frames…) |
| `dynamic-fps` | `3.11.6+minecraft-1.21.11-fabric` | throttle FPS when unfocused/AFK |
| `Ksyxis` | `1.4.6` | world-loading / chunk loading speedup |
| `zoomify` | `2.15.2+1.21.11` | zoom key (client QoL, listed as performance) |
| `BetterF3-17.0.0-Fabric-1.21.11` | 17.0.0 | readable debug HUD (installed via CurseForge URL) |
| `fabric-api` | `0.141.6+1.21.11` | library (dependency of many mods) |
| `fabric-language-kotlin` | `1.14.2+kotlin.2.4.21` | Kotlin runtime (dependency) |
| `cloth-config-21.11.153-fabric` | 21.11.153 | config library (dependency) |
| `yet_another_config_lib_v3` (YACL) | `3.8.2+1.21.11-fabric` | config library (Zoomify dependency) |
| `placeholder-api-2.8.2+1.21.10` | 2.8.2 | text placeholder library |
| `modmenu-17.0.1` | 17.0.1 | mods screen (the dock's Mods button) |
| `fusion-1.3.16-fabric-mc1.21.11` | 1.3.16 | connected-textures support pulled in by the CTM packs |

The intended stack is therefore: **Sodium + Indigo (Fabric renderer) for rendering,
Lithium for game logic, FerriteCore for heap, ImmediatelyFast for GUI/immediate
batching, Entity Culling + More Culling for overdraw, Dynamic FPS + Ksyxis for
load/idle behaviour**, wrapped by Cloth Config/YACL/Placeholder API/FLK as libraries,
with BetterF3/Mod Menu/Zoomify as visible client QoL. (Also evidenced by the log's
resource-reload line listing `fabric-renderer-indigo`, `immediatelyfast`, `ksyxis`,
`lithium`, `moreculling`, `sodium`, `fusion` …)

### 8.2 Resource packs (`run/resourcepacks/`, 16 zips)

`SodiumTranslations`, `Midnighttigger's Default Connected Textures`, `Spunky Pack 64X(2)`,
`CTM-Overhaul-V_6.0-Pre1`, `SmallCapsFont_v1.4`, `NewGlowingOres-§6[Border]§r`,
`Default-Dark-Mode-26.3-2026.9.0`, `No Fire`, `No Pumpkin Blur 1.4`,
`No Block Break Particles 26.3`, `Better Mining Bar`, `Fullbright-UB-26.3 fub-7.0`,
`Enchantment Outlines`, `enchant-icons-26.2-v1.3`, `Low Shield`,
`EvenBetterEnchants_v3_1.21.5+`.

All 16 are enabled in `run/options.txt:66` (`resourcePacks:["vanilla", …]`, in the
order the mod appended them) — exactly the `saved` resource-pack ids in `installs.json`.
They are PvP/utility packs: FPS-friendly visuals (no fire, no pumpkin blur, no break
particles, fullbright), readability (small caps font, glowing ores, enchant icons,
outlines, better mining bar), and connected textures (CTM Overhaul + Midnighttigger's +
Fusion).

`run/data/fabric_default_resource_packs.json` is `{"values":[]}` (no extra defaults).

### 8.3 Performance / mod configs (`run/config/`)

- **`sodium-options.json`** — quality: `hidden_fluid_culling: true`,
  `pixel_filtering_mode: NEAREST`; performance: `chunk_builder_threads: 0` (auto),
  `chunk_build_defer_mode: ALWAYS`, `animate_only_visible_textures: true`,
  `use_entity_culling: true`, `use_fog_occlusion: true`, `use_block_face_culling: true`,
  `use_no_error_g_l_context: true`, `quad_splitting_mode: SAFE`; advanced:
  `use_advanced_staging_buffers: true`, `cpu_render_ahead_limit: 3`;
  `terrain_sorting_enabled: true`.
- **`sodium-mixins.properties`** — stock/empty (defaults), just the wiki notice.
- **`sodium-fingerprint.json`** — Sodium's integrity fingerprint blob + `t` timestamp.
- **`lithium.properties`** — stock/empty (all Lithium defaults on).
- **`ferritecore.mixin.properties`** — all memory dedup options **on**
  (`replaceNeighborLookup`, `replacePropertyMap`, `cacheMultipartPredicates`,
  `modelResourceLocations`, `multipartDeduplication`, `blockstateCacheDeduplication`,
  `bakedQuadDeduplication`, `modelSides`, `dataComponentPatch`), with the three
  riskier ones left **off** (`useSmallThreadingDetector`, `compactFastMap`,
  `populateNeighborTable`) — a conservative, crash-avoiding profile.
- **`immediatelyfast.json`** — `enhanced_batching: true`, `font_atlas_resizing: true`
  (size 1024), `map_atlas_generation: true` (size 2048),
  `skip_text_translucency_sorting: true`, `fast_text_lookup: true`,
  `avoid_redundant_framebuffer_switching: true`,
  `fix_slow_buffer_upload_on_apple_gpu: true`; all experimental/debug flags off.
- **`moreculling.toml`** — `cloudCulling`, `signTextCulling`, `rainCulling`,
  `useBlockStateCulling`, item-frame culling/LOD (range 11), `paintingCulling`,
  `leavesCullingMode = "DEFAULT"`, `leavesCullingAmount = 2`, `beaconBeamCulling`,
  `useOnModdedBlocksByDefault = true`.
- **`entityculling.json`** (`configVersion 9`) — `safeMode: true`,
  `renderNametagsThroughWalls: true`, `tracingDistance: 128`, `sleepDelay: 10`,
  `hitboxLimit: 50`, `captureRate: 5`, `tickCulling: true`, frustum culling on,
  `solidLeaves: false`. Its whitelists reference **mods not in this pack** (botania,
  create, createbigcannons, alexscaves, cinematiccataclysm, mts, quark, voidscape,
  drg_flares, avm_staff) — clear evidence the config was copied from a larger modpack
  template rather than generated here.
- **`dynamic_fps.json`** — `{}` (pure defaults).
- **`zoomify.json`** — `initialZoom: 4`, `zoomKeyBehaviour: "hold"`,
  `scrollZoom: true` (10 steps, 150 %/step, smoothness 70),
  `affectHandFov: true`, `spyglassBehaviour: "combine"`, secondary zoom 4×.
- **`yacl.json5`** — `showColorPickerIndicator: true`,
  `preloadComplexImageFormats: false`.
- **`modmenu.json`** — `sorting: "ascending"`, `mods_button_style: "classic"`,
  `game_menu_button_style: "replace"`, `mod_count_location: "title_screen"`,
  `modify_title_screen: true`, `modify_game_menu: true`, update checker on.
- **`fabric/indigo-renderer.properties`** — `ambient-occlusion-mode=hybrid` plus the
  default lighting fixes (`fix-smooth-lighting-offset`, `fix-mean-light-calculation`, …).
- **`transition.json`** / **`trender.json`** — leftovers from mods `transition 1.0.26`
  and `trender 1.0.17`, which were loaded in an earlier session (they appear in an older
  log's mod tree) but their jars are **no longer in `run/mods`**. `trender.json` is
  `{"version":1,"style":"VANILLA_MODERN"}`; `transition.json` holds a crash-report
  consent flag. They are not part of the current stack.

### 8.4 Game options that matter (`run/options.txt`)

`guiScale:2` (matches the README's preview conditions), `maxFps:120`,
`renderDistance:16`, `simulationDistance:12`, `graphicsPreset:"fancy"`,
`prioritizeChunkUpdates:1`, `particles:0`, `entityDistanceScaling:1.0`,
`mipmapLevels:4`, `textureFiltering:1`, `inactivityFpsLimit:"afk"`, `gamma:0.5`,
`lang:en_us`. (`options.txt:1-70` region.)

### 8.5 Runtime artifacts (skimmed only)

- `run/logs/debug.log` (+ dated `.gz`) — confirms `Loading Minecraft 1.21.11 with
  Fabric Loader 0.19.5`, the dev run initially loading only `xrylothmenu` (5 entries:
  fabricloader, java, minecraft, mixinextras, xrylothmenu), then the installer
  populating `run/mods`, and the full `Xryloth/Mods` install transcript.
- `run/crash-reports/` — nine crashes, used in §9.4 for causes only.
- `run/resourcepacks/`, `run/screenshots/` — already covered in §8.2 /
  `README.md:128-135`.

---

## 9. Gotchas and TODOs

### 9.1 Documentation drift (README vs code)

1. **"two compact primary actions"** (`README.md:31`) — the code has **three**
   (`SINGLEPLAYER`, `MULTIPLAYER`, `BOOSTS`, `XrylothMenuScreen.java:198-204`,
   `rows = 3` at `:156`).
2. **"a close / quit button (top-right)"** (`README.md:37`) — there is no top-right
   element; only the top-left profile chip exists (`XrylothMenuScreen.java:242-243`,
   `drawProfileChip :405-433`). Quit lives in the dock (`:236-238`).
3. **Footer text** — README says `Xryloth Menu <VERSION> (<game version>)`
   (`README.md:76`); the code draws `Xryloth Client <VERSION> (<game version>)`
   (`XrylothMenuScreen.java:458`).
4. **"a slow blue/purple Xryloth aurora"** (`README.md:26`) — not implemented;
   `drawOverlay` only lays a scrim and two vignette gradients
   (`XrylothPanorama.java:64-69`, whose javadoc still says "scrim and the aurora").
5. **`scripts/tools/xryloth-logo.svg` and `scripts/tools/render-logo.sh`**
   (`README.md:87-91`) do not exist in the repo — only `bake-icons.sh` and
   `GenerateFontAtlas.java` are committed. The logo/icon rasterisation step is lost.

### 9.2 Catalogue / installer bugs

1. **Duplicate id `yacl`** (`XrylothCatalog.java:41` and `:80`). `items()` overwrites by
   id (`XrylothMods.java:94-96`), so the *second* (OPTIONAL, `recommended=false`) wins:
   YetAnotherConfigLib is **not pre-selected** even though it is in the performance set.
   It still gets installed, but only as Zoomify's dependency (see `installs.json`
   `dependencies`, absent from `saved`).
2. **`badoptimizations` is dead** — it is in the OPTIONAL list
   (`XrylothCatalog.java:52`) but has **no baked icon** (checked: 64 icons vs 62 unique
   ids; only `badoptimizations` is missing) and the live log shows
   `could not read badoptimizations from Modrinth`. Its row will show an error/badge
   fallback.
3. **Three orphan icons** ship in the jar with no catalogue entry: `c2me-fabric`,
   `indium`, `nvidium` (leftovers from an earlier catalogue).
4. **`PINNED` is empty** (`XrylothMods.java:81`) — the version-pinning mechanism is
   dead code; nothing prevents a bad "newest" build from being selected.
5. **`restartPending` is never cleared.** It is set `true` on install
   (`XrylothMods.java:357`) and read back (`:527`), but **no code ever writes `false`**
   (grep: only `:357` assigns it). Therefore `RESTART MINECRAFT TO LOAD THE NEW MODS`
   (`XrylothMenuScreen.java:442-444`, `XrylothBoostsScreen.java:81-83`) and the 5-button
   layout persist forever after the first install — and the branch at
   `XrylothMenuScreen.java:278-281` (`installed && !restartPending`) can never run.
6. **`mods.status` is never created in practice.** Because of (5),
   `setModStatus(RESTARTED)` is unreachable, so `getModStatus()` always returns
   `FIRST_TIME` (file absent — confirmed: `run/config/xryloth-menu/` contains only
   `installs.json`). The whole `RESTARTED` state machine
   (`XrylothMenuScreen.java:259-273`) is effectively dormant; only the
   `FIRST_TIME && !installed` auto-install branch fires.
7. **Silent first-launch install.** With the default status, opening the title screen
   auto-starts a network install of ~31 projects with no confirmation
   (`XrylothMenuScreen.java:253-258`), on a background daemon thread.
8. **`install()` silently no-ops when busy** (`:266`) and the `onDone` callbacks passed
   by callers are empty lambdas (`XrylothMenuScreen.java:256-257,269-270`,
   `XrylothBoostsScreen.java:237-238,257-258`) — the UI learns about completion only by
   polling `isBusy()`/`isRestartPending()`.
9. **Hand-rolled JSON with no escaping** (`XrylothMods.java:566-594`): a filename or id
   containing `"` or `\` would corrupt `installs.json`. Writes are not atomic (no
   temp+move) and not locked against concurrent readers.
10. **`enableResourcePacks` rewrites `options.txt` while the game is running**
    (`XrylothMods.java:454-505`); Minecraft rewrites `options.txt` on quit, so the edit
    can be lost (it survives only because the game merges from its in-memory copy only
    for keys it knows — risky). Its parser strips all quotes and splits on `,`, so a
    pack name containing a comma would be mangled.
11. **`cleanup()` only deletes `.jar`** (`:399`) — resource packs removed from the
    catalogue are never deleted from `resourcepacks/`.
12. **Fragile JSON scraping** — `downloadUrl()`/`string()`/`between()` use `indexOf`
    on raw JSON (`XrylothMods.java:438-451,705-719`) instead of Gson; a `"url"` inside
    another string would break it. Also a **new `HttpClient per request** (`:687-698`).
13. **Hardcoded CurseForge file URL** (`XrylothCatalog.java:102-104`) will rot when
    BetterF3 publishes a new build; `refresh()` hardcodes `version = "17.0.0"` for it
    (`XrylothMods.java:124`).
14. **No rate-limit/backoff** on Modrinth (40 s timeouts, 4 concurrent workers, and
    `refresh()` re-fetches project metadata on every Boosts-screen open even when
    `fileName` is already known).

### 9.3 Mixin / API fragility

1. **`PressableWidgetMixin` depends on four private vanilla members**
   (`<init>`, `drawButton`, `renderWidget`, `drawLabel`) — any rename/signature change
   in a MC update breaks boot because `defaultRequire: 1` (`xryloth-menu.mixins.json:14`).
   The README already documents that 1.21.11 made `renderWidget` `final`, which is why
   the custom screens do their own drawing (`README.md:117-123`).
2. **Interfaces must stay out of `mixins.json`** — earlier versions registered
   `DrawContextExtension`/`ClickableWidgetAccessor` as mixins and crashed at bootstrap
   (`run/crash-reports/crash-2026-10-20_13.58.02-client.txt`,
   `crash-2026-10-20_16.36.04-client.txt`).
3. **`TitleScreenMixin` calls `setScreen` from inside `TitleScreen.init`**
   (`TitleScreenMixin.java:26`) — no mixin priority is set, so another mod that also
   swaps the title screen will fight over it (last/winning injection order decides).
4. **`DrawContextMixin` constructs `TexturedQuadGuiElementRenderState` directly**
   (`DrawContextMixin.java:54-61`) — the constructor shape is a 1.21.11 detail and must
   be re-checked on every mapping bump.
5. **`compatibilityLevel: JAVA_21`** triggers a mixin warning on current loader builds
   ("higher than the maximum level supported … JAVA_13"), which is benign but noisy.

### 9.4 Crash history (from `run/crash-reports/`, causes only)

| Symptom | Root cause | Fix now in the tree |
|---|---|---|
| `Could not execute entrypoint stage 'client' … NPE: getTextureManager() is null` | `ensureLogo()` was called from `onInitializeClient()` during bootstrap | entrypoint now only logs; textures load in `XrylothMenuScreen.init()` (`XrylothMenu.java:37-40`, guard `:72-75`) |
| `Texture view does not exist, can't get it before something initializes it` (from `CubeMapRenderer.draw`, while `SplashOverlay` still renders the title screen) | panorama/logo texture registered but not uploaded before first draw | `NativeImageBackedTexture` uploads immediately; `XrylothPanorama.ensure()` registers the cube map before `render` (`XrylothPanorama.java:95-103`) |
| `Cubemap compatible textures must be square, but size is 16x2` | malformed stitched strip fed to `CubemapTexture.load` | `loadStrip()` validates equal face sizes and builds `w × 6h` (`XrylothPanorama.java:129-142`) |
| `Mixin transformation of …DrawContextExtension / …ClickableWidgetAccessor failed` | duck interfaces listed as mixins | interfaces moved to `api/`, only 5 concrete mixins registered |
| `NPE: Cannot load from int array because "center" is null at …DebugShot.tick` (×2) | temporary screenshot/keyboard-test hook | `DebugShot` and the `MinecraftClient` screenshot injection are deleted from the source |

### 9.5 Design constraints to preserve when rebuilding

1. **No Fabric API.** Every "normal" mechanism is therefore replaced: textures are
   jar-read and registered by hand (`XrylothMenu.ensureTexture`), the panorama is
   stitched manually, the font is baked, Mod Menu is reached by reflection. Adding
   Fabric API as a dependency would invalidate most of §5 and the README's provenance
   claims (`README.md:10-11,190-195`).
2. **One branding file** — keep `XrylothBrand` as the only place with copy/colours.
3. **Vanilla buttons restyled globally** via `PressableWidgetMixin`, custom screens
   drawn by hand — both approaches must stay in sync through `Glass` + `MenuFont`.
4. **Everything is rebuilt on menu open** so F3+T resource reloads cannot leave stale or
   uninitialised textures (`XrylothMenu.java:54-55`, `MenuFont.java:69-73`,
   `XrylothPanorama.java:72-74`).
5. **Clean-room rule** — `scripts/verify-jar.sh:46-55` fails the build if any entry
   matches polyplus/oneconfig/polyfrost/oneclient/compose.

### 9.6 Cross-component TODOs (mod ↔ launcher)

- The launcher and the mod both rewrite `options.txt`'s `resourcePacks` line with
  different formats and no locking (§7.1) — a single shared helper (or letting only one
  side own that key) would remove a real corruption/lost-update risk.
- Nothing clears `restartPending`, so the "restart required" state is permanent
  (§9.2.5). A restart-detection signal (e.g. the launcher clearing the flag when it
  relaunches the instance, or the mod comparing a boot-time snapshot of `mods/`) is the
  obvious missing piece — and would be the natural place for a launcher↔mod file
  protocol if one is ever wanted.
- `mods.status` was clearly meant to be written by *something* outside the mod (its
  comment tells the user to flip it by hand — `XrylothMods.java:632-634`) but no
  writer exists; either the launcher should own it or the state machine should be
  simplified away.

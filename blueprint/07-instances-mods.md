# Xryloth — Instances, Mods & Modpacks

Source: `src-tauri/src/lib.rs` (sections: INSTANCES 578, TAURI COMMANDS 725+, INSTANCE TEMPLATES 3859, PERFORMANCE MODS 4710, MOD PROFILES 5036, MOD UPDATES 5188, MOD DEPENDENCY RESOLVER 5348, EXPORT MRPACK 5496, MOD CONFLICTS 5609, BATCH INSTALL 10541, GAME BOOST 10851). Line refs exact. Launch/install mechanics live in `05-launch-pipeline.md`; command signatures in `03-commands-reference.md`.

---

## 1. Instance model

An **instance** (= profile) is a row in `~/.xryloth/instances.json` + a folder `~/.xryloth/instances/<name>/`.

Record created by `create_instance` (lib.rs:788-806):

```json
{
  "name": "MyPack", "version": "1.20.1", "loader": "fabric",
  "loader_version": "0.15.11", "created": "<rfc3339>",
  "last_played": null, "icon": ""
}
```

Enriched on read:
- `load_instances_raw` (578-594) adds `installed` (folder exists) and `version_json_exists`.
- `get_instances` (760-787) merges per-instance `instance_settings.json[name]` → `ram_mb`, `jvm_args`, `java_path`.
- Pins: `pin_instance` / `unpin_instance` (10437/10450) set a pinned flag for the UI.

Folder contents: `<mc_version>.json` (version JSON), `<mc_version>.jar`, loader jars + `*-profile.json`, `mods/ saves/ resourcepacks/ screenshots/ natives/ config/ assets/ crash-reports/ logs/`, `icon.png`, `options.txt`.

**Mutating commands**: `create_instance` 788 · `delete_instance` 808 · `rename_instance` 3643 (renames folder + all references) · `edit_instance` 3666 (changes version/loader metadata; requires reinstall to take effect) · `clone_instance` 6234 (full folder copy) · `set_instance_icon`/`get_instance_icon` 3715/3732.

---

## 2. Templates (`get_builtin_templates` 3861-4147)

19 built-in one-click modpack templates. Schema: `{id, name, description, icon, category, mc_version, loader, loader_version, mods:[{slug,name}]}`.

| id | category | default target |
|---|---|---|
| `fabric-tech-reborn`, `fabric-ad-astra`, `fabric-create-astral`, `fabric-better-end`, `fabric-chromatic-iron`, `fabric-lets-do`, `fabric-vault-hunters`, `fabric-skyfactory` | Fabric | **1.20.1 + fabric 0.15.11** |
| `forge-ftb-revelation`, `forge-rl`, `forge-bettermc` | Forge | Forge (see each entry) |
| `vanilla-optimized`, `vanilla-building`, `vanilla-qol`, `vanilla-shaders` | Vanilla | no loader |
| `server-hardcore`, `server-creative`, `server-skyblock`, `server-survival` | Server | server-side templates |

- `get_instance_templates` (4147) → `{templates:[…]}`.
- `apply_instance_template` (4170 **[async]**): for each mod slug → `get_modrinth_versions(slug, loader, mc)` → download primary file into `mods/` (skip if already present) → log `[Template/OK]` / `[Template/ERR]` → emit `template-progress {success, installed, errors, template_name}` (4236). Unknown template → `{success:false, error:"Template not found"}`.

---

## 3. Installing mods from Modrinth

### 3.1 Search / browse

| command | line | endpoint |
|---|---|---|
| `search_modrinth(query, project_type?, loader?, mc_version?, limit?)` | 2618 **[async]** | `https://api.modrinth.com/v2/search` with facets |
| `get_modrinth_project(slug)` | 2649 | `/v2/project/{slug}` |
| `get_modrinth_versions(slug, loader?, mc_version?)` | 2657 | `/v2/project/{slug}/versions` (client-filtered) |

### 3.2 Single mod — `install_mod` (2745) → `install_mod_inner` (2682)

Version ladder:
1. `get_modrinth_versions(slug, loader, mc)` — matches instance loader+version
2. if empty → `get_modrinth_versions(slug, None, None)` (any) — **2702-2705**
3. pick `version_id` if given, else `versions_arr[0]` (Modrinth returns newest first)
4. primary file (`files[].primary == true`, else first) → `mods/<filename>`
5. skip if destination already exists; failures return `{"success":false,"error":…}`

`install_resourcepack` (2757) — same into `resourcepacks/`.

### 3.3 Batch — `batch_install_mods(instance, slugs[])` (10542)

Loops `install_mod` per slug, collecting `installed[]` and `errors[{slug,error}]` → `{success, installed, errors}`.

---

## 4. Modpacks (`.mrpack`)

### 4.1 Install into an existing instance — `install_modpack` (2793 **[async]**)

1. Resolve version → primary file URL (filename default `modpack.mrpack`).
2. Download to tempdir (`download_file_blocking`).
3. Open zip, read **`modrinth.index.json`** → for each `files[]` entry: `downloads[0]` URL → save to entry's `path` (relative to instance dir) (2840-2855).
4. Extract every remaining zip entry, honoring `overrides/` prefix stripping (`strip_prefix("overrides/")` at 2865).
5. → `{"success":true}`; emits `[Modpack] …` log lines throughout.

### 4.2 Create instance from a modpack URL — `create_instance_from_modpack` (3777 **[async]**)

Same parsing, but first creates the instance record (rejects duplicate names), then downloads `mrpack_url` and applies index + overrides → `{success, instance}`.

### 4.3 Export — `export_instance_mrpack` (5497 **[async]**, section 5496-5608)

Builds a `.mrpack` from an existing instance: writes `modrinth.index.json` (from installed mods where a Modrinth origin is known), packs `mods/` + `overrides/` (config etc.) into the zip at `output_path`.

---

## 5. Mod profiles (5036-5187)

Saved named mod lists in `~/.xryloth/mod_profiles.json`:

| command | line | behavior |
|---|---|---|
| `save_mod_profile(name, instance_name)` | 5039 **[async]** | Snapshot the instance's `mods/` list under `name`. |
| `load_mod_profiles()` | 5097 | List all. |
| `delete_mod_profile(name)` | 5103 | Remove one. |
| `apply_mod_profile(name, instance_name)` | 5113 **[async]** | Make the instance's mods match the snapshot (install missing / disable extra). |

---

## 6. Mod updates — `check_mod_updates` (5189-5283)

For each `mods/*.jar` (name → slug guess: lowercase, `_`/space → `-`, 5210):
1. `GET {MODRINTH_API}/project/{slug}`; if 404, **search** and follow the matched slug (5227-5243).
2. `GET /project/{slug}/version` → newest (`[0]`).
3. If newest ≠ installed version number → push `{slug, name, current_version, latest_version, …}` into `updates`.
Returns `{success, updates, total_checked}`. Downloading an update reuses `install_mod`.

---

## 7. Dependencies & conflicts

### 7.1 `resolve_mod_dependencies(instance, slug)` (5349-5443)

Reads the Modrinth project's `dependencies[]` (`required` / `optional` / `incompatible` / `embedded`) and returns (and can install) missing **required** deps into `mods/` — so installing e.g. a mod that needs Fabric API / Cloth Config doesn't leave a broken profile.

### 7.2 `detect_mod_conflicts(instance)` (5610-5711)

Rule engine over installed jar names (`.disabled` stripped, lowercased, slugified):

```rust
struct ConflictRule { mod1_keywords, mod2_keywords, reason, severity }
```

Rules are keyword-pairs — e.g. `optifine|optifabric|optiforge` × `sodium|embeddium` → *"OptiFine and Sodium/Embeddium are incompatible rendering engines"* (severity `high`), `optifine` × `lithium`, plus more through 5712. Returns matched conflicts with reason+severity for the UI's "conflicts" banner.

### 7.3 Resource pack auto-enable — `auto_enable_resourcepack(instance, filename)` (5445)

Inserts `"<filename>"` into `options.txt`'s `resourcePacks:` list (and `enabled` flags) so a just-installed pack is actually active.

---

## 8. Performance mods (`PERF_MODS_*` 4710-4894)

Curated Modrinth slug lists, installed by `install_performance_mods(instance, mc_version, loader)` (4895) and auto-triggered by `game_boost` when `perf_auto_mods` is on (default **true**, 10864).

**Fabric list** (19 + Fabric API installed first, 4941): `sodium` (fallback `embeddium`), `lithium`, `dynamic-fps`, `modmenu`, `ferrite-core`, `entityculling`, `immediatelyfast`, `moreculling`, `modernfix`, `krypton`, `memoryleakfix`, `starlight`, `badoptimizations` (min 1.19), `threadtweak`, `noisium` (min 1.20), `c2me-fabric`, `enhancedblockentities`, `lazydfu`, `nvidium` (min 1.20).

**Forge list** (19): `embeddium` (fallback `optiforge`), `lithium`, `starlight`, `ferrite-core`, `memoryleakfix`, `entityculling`, `modernfix`, `performant`, `fastchest`, `noisium`, `enhancedblockentities`, `lazydfu`, `krypton-reforged`, `moreculling`, `alternate-current`, `clustered`, `radon`, `booptimize`, `smooth-boot`.

Behavior per mod (`download_mod_from_modrinth` 4714 / `install_perf_mod`):
- `get_modrinth_versions(slug, loader, mc)`; empty → `[PerfMods/WARN] No compatible version` → count as *skipped*.
- `min_mc` not satisfied → try `fallback` slug if present, else skipped (4949-4974).
- destination exists → *skipped* (already installed).
- Result: `{success, installed, skipped, errors, total, message}`.

Returns a summary line: *"Installed N FPS mods (M unsupported for this version were skipped)"*.

---

## 9. Game Boost — `game_boost` (10853-11084)

Multi-step optimizer; every step is **gated by a settings flag** (read via `let s = |k| …unwrap_or(k == "perf_auto_mods")` — only `perf_auto_mods` defaults ON, 10864):

| flag | step | platform |
|---|---|---|
| `boost_kill_processes` | kill listed unnecessary processes (taskkill) | all |
| `boost_clear_temp` | clear system temp + launcher temp (`updates/` etc.), reports MB | all |
| `boost_empty_recycle` | empty recycle bin | Windows |
| `boost_flush_dns` | `ipconfig /flushdns` | Windows |
| `boost_clear_thumbs` | clear thumbnail cache | Windows |
| `perf_auto_mods` | auto-install FPS mods (§8) — skipped for Vanilla | all |
| `boost_optimize_memory` | system memory trim | Windows |

Returns `{success, steps_completed, steps_total, details[]}`; logs `[Boost] Game boost complete: n/m steps`.

---

## 10. Screenshots, config import/export

- `get_screenshots(instance)` (10464) → lists `screenshots/*.png` with mtime/size.
- `export_instance_config(instance, output_path)` (10495) → writes the instance **record JSON** (not the whole folder).
- `import_instance_config(file_path)` (10513) → validates `name` + `version` present, refuses duplicate name, creates instance → `{success, instance}`.
- `move_mod_file` / `move_resourcepack_file` (10819/10835) → copy an arbitrary external file into `mods/` / `resourcepacks/` (used by drag-and-drop from the OS).

---

## 11. Gotchas

1. **`edit_instance` changes metadata only** — the downloaded jar/JSON stay the old version until `install_instance` runs again; the UI must show "reinstall needed".
2. **Mod version matching falls back to "any version"** (2702) — you can silently install a mod built for a different MC version when nothing matches; conflicts then surface via `detect_mod_conflicts`.
3. **Slug guessing from filenames** (`check_mod_updates` 5210) is heuristic — mods with non-Modrinth names need the search fallback (5227) or are skipped.
4. **Template mods are all Modrinth slugs** — a template apply fails soft per-mod (`[Template/ERR]`) and reports counts; partial installs are normal.
5. **`install_modpack` writes index `path`s straight into the instance dir** — a malicious mrpack could path-traverse (`../`) if `path` isn't sanitized; check `overrides/` stripping (2865) and add explicit path validation when rebuilding.
6. **Perf-mod lists are version-gated only coarsely** (`min_mc` major.minor) — e.g. Sodium's API churn across1.20→1.21 is handled only by Modrinth's loader/version facets.
7. **`perf_auto_mods` default-on** means `game_boost` mutates the user's `mods/` folder even if they only wanted a RAM tweak.
8. Templates pin **fabric 0.15.11 / MC 1.20.1** — stale defaults; loader auto-detect at install time may pick newer (see `05-launch-pipeline.md` §4.1).

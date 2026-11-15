# Xryloth — Accounts & Authentication

Source: `src-tauri/src/lib.rs` (Rust side) + `src/components/pages/AccountsPage.tsx` (storage side). Line refs are exact.

---

## 1. Model overview

Three account types, two storage layers:

| type | meaning | token passed to Minecraft |
|---|---|---|
| `microsoft` | Real Mojang account via MS OAuth device-code flow | Mojang session token (`mc_token`) |
| `elyby` | Ely.by account (RU "cracked-premium" service) | Ely.by token + `-javaagent:authlib-injector.jar=ely.by` |
| `offline` | Local/offline username | `0` (i.e. `--accessToken 0`, `--userType legacy`) |

**Storage is split:**

1. **Legacy flat keys in `~/.xryloth/settings.json`** — written by Rust:
   `username`, `cracked_username`, `mc_token`, `mc_uuid`, `account_type`, `ms_refresh_token`, `ely_refresh_token`, `ely_by_username`, …
2. **`settings.accounts[]` array + `settings.active_account_uuid`** — written by the **frontend** (`AccountsPage.tsx:88-95` `saveAccounts()` → `saveSettings({accounts, active_account_uuid})`). Each account record is `{type, username, access_token, uuid, …}` (consumed at lib.rs:2185-2252).

> ⚠️ Rust never writes `accounts[]`; the UI never writes the flat keys directly (it calls the auth commands, which do). Both layers must stay consistent — see §5.

---

## 2. Microsoft (device-code flow)

### 2.1 Start — `authenticate_microsoft` (lib.rs:1549, `#[tauri::command(async)]`)

1. Clears previous `ms_auth_result` / `ms_auth_polling` (1554-1558).
2. POST `https://login.microsoftonline.com/consumers/oauth2/v2.0/devicecode` with (1559-1562):

```json
{"client_id":"00000000402b5328",
 "scope":"XboxLive.signin XboxLive.offline_access MinecraftWin32.app"}
```

(`00000000402b5328` is the well-known **Minecraft Launcher** client id.)

3. Reads `user_code`, `verification_uri` (default `https://microsoft.com/link`), `device_code`, `interval` and returns them so the UI shows "go to <uri> and enter <code>".
4. **Spawns a background polling thread** (1573-1637) so the Tauri event loop stays free:
   - `poll_interval = min(interval, 5).max(1)` seconds
   - POST `https://login.microsoftonline.com/consumers/oauth2/v2.0/token` with
     `client_id=…&grant_type=urn:ietf:params:oauth:grant-type:device_code&device_code=…` (1591)
   - On `access_token` + `refresh_token`: stores `{access_token, refresh_token}` into `app.ms_auth_result` and clears `ms_auth_polling` (1596-1598).

### 2.2 Poll — `poll_microsoft_status` (lib.rs:1638)

```json
{"status":"pending"}                 // while polling
{"status":"done","result":{access_token, refresh_token}}
{"status":"idle"}                    // nothing started / cleared
```

### 2.3 Complete — `complete_microsoft_auth_cmd` (lib.rs:1697) → `complete_ms_auth` (lib.rs:1650)

The classic 4-hop chain, all with `http_post_json` / `http_get` (30 s timeout, UA `Xryloth/5.0`):

| # | Endpoint | Body / header | Extracts |
|---|---|---|---|
| 1 | `https://user.auth.xboxlive.com/user/authenticate` | `{"Properties":{"AuthMethod":"RPS","SiteName":"user.auth.xboxlive.com","RpsTicket":<ms_token>},"RelyingParty":"http://auth.xboxlive.com","TokenType":"JWT"}` | `Token`, `DisplayClaims.xui[0].uhs` |
| 2 | `https://xsts.auth.xboxlive.com/xsts/authorize` | `{"Properties":{"SandboxId":"RETAIL","UserTokens":[<xbl_token>]},"RelyingParty":"rp://api.minecraftservices.com/","TokenType":"JWT"}` | `Token` |
| 3 | `https://api.minecraftservices.com/authentication/login_with_xbox` | `{"identityToken":"XBL3.0 x=<uhs>;<xsts_token>"}` | `access_token` (the **Minecraft** token) |
| 4 | `https://api.minecraftservices.com/minecraft/profile` | `Authorization: Bearer <mc_token>` | `name`, `id` (UUID) |

Then it writes the **flat** settings keys (lib.rs:1681-1687): `mc_token`, `ms_refresh_token`, `username`, `cracked_username`, `mc_uuid`, `account_type="microsoft"`, and returns

```json
{"success":true,"username":…,"uuid":…,"mc_token":…,"refresh_token":…,"type":"microsoft"}
```

The **frontend** then pushes this into `settings.accounts[]` and sets `active_account_uuid`.

> ⚠️ **Refresh tokens are stored but never used.** There is no `grant_type=refresh_token` anywhere in lib.rs — when `mc_token` expires, the user must re-run device-code auth. (`ms_refresh_token` at 1682, `ely_refresh_token` at 1808 are write-only.)

---

## 3. Ely.by (OAuth2 + authlib-injector)

| Command | Line | What it does |
|---|---|---|
| `authenticate_elyby` | L1707 | Starts the OAuth2 browser flow (`account.ely.by/oauth2/authorize`), logs `[Auth/Ely.by] Starting OAuth2 flow...`. |
| `poll_elyby_auth` | L1760 **[async]** | Polls shared state until the user finishes in the browser (`[Auth/Ely.by] Polling for auth code...`). |
| `complete_elyby_auth_cmd` | L1828 **[async]** | POST `grant_type=authorization_code&code=…&redirect_uri=…` to `account.ely.by/api/oauth2/v1/token` (1783), stores `ely_refresh_token` (1808), fetches account info from `account.ely.by/api/account/v1/info`, saves an `elyby` account. |
| `ensure_authlib_injector` | L1879 **[async]** | Downloads `~/.xryloth/authlib-injector.jar` if missing. URL resolution (`fetch_authlib_injector_url` 1847): GitHub `yushijinhun/authlib-injector/releases/latest` first → fallback `https://authlib-injector.yushi.moe/artifact/latest.json` → fallback direct `https://download1.mediafire.com/lr3tqh08syf8tgu/EcSelfHost.exe`-style known URL pattern (1862-1876). |

**At launch** (lib.rs:2202-2207): if the active account is `elyby` and online, the JVM gets

```
-javaagent:<~/.xryloth/authlib-injector.jar>=ely.by
```

prepended to `jvm_args`, and the session is reported to Minecraft as `userType=mojang` with the Ely.by token — authlib-injector rewrites the session servers so skins/profiles resolve via Ely.by.

---

## 4. Offline accounts

`create_offline_account` (lib.rs:1836):

```rust
save_setting_val(&app, "cracked_username", username);
save_setting_val(&app, "username", username);
save_setting_val(&app, "mc_token", "0");
// returns {"success":true, "uuid":"00000000-0000-0000-0000-000000000000", "mc_token":"0", "type":"offline"}
```

- **UUID is the nil UUID**, not the usual `MD5("OfflinePlayer:"+name)` offline UUID — so offline players all share one UUID unless the frontend computes its own (check `accounts[]` records written by the UI).
- Token `"0"` is the marker used everywhere to mean "no session" (e.g. launch condition `acc_token != "0"` at 2198/2202).

**Skins for offline players** — `setup_custom_skin_loader()` (lib.rs:2530-2575), called from the launch thread when the session looks offline (`cracked_username` set and `mc_token == "0"`, 2291-2296):

1. Ensures `mods/CustomSkinLoader_Fabric-14.19.1.jar` exists (deletes and re-downloads if <1000 bytes or not a valid ZIP — the same self-healing pattern as elsewhere).
2. Writes a `config/CustomSkinLoader` profile so the offline player still loads skins from the configured sources.
3. Also relevant: `get_skin_info` (3026) / `set_custom_skin` (3036) copy a user-chosen image to `~/.xryloth/custom_skin.png`.

---

## 5. Auth selection at launch (`launch_game`, lib.rs:2183-2253)

**Stage A — active account list** (preferred):

```rust
let active_uuid = settings["active_account_uuid"];
let accounts    = settings["accounts"];            // array
let active      = accounts.find(|a| a.uuid == active_uuid);
```

| `acc_type` | condition | passed to JVM as |
|---|---|---|
| `microsoft` | online && token non-empty && ≠ `"0"` | `(username, token, uuid, userType="mojang")` |
| `elyby` | online && token ok | same + authlib-injector agent inserted at `jvm_args[0]` (2204-2206) |
| `offline` | always | `(username, "0", uuid, userType="legacy")` |
| anything else | — | fall through to Stage B |

**Stage B — legacy flat keys** (2219-2252) if no active account matched:

1. `ely_by_username` + (`ely_access_token` | `mc_token`) → Ely.by
2. `username` + `mc_token` (≠`"0"`) → Microsoft
3. `cracked_username` → offline

`online` is decided by `is_connected()` (lib.rs:364-378): `HEAD https://sessionserver.mojang.com` with a **2 s** timeout, result cached **10 s** — when offline, Microsoft/Ely.by accounts silently degrade to offline-style launch instead of failing.

Final game args (2255-2274):

```
--username <u> --uuid <uuid> --accessToken <t> --userType <mojang|legacy>
```

---

## 6. Frontend responsibilities (`src/components/pages/AccountsPage.tsx`)

- Reads `settings.accounts` + `settings.active_account_uuid` on mount (84-87).
- `saveAccounts(newAccounts, newActive?)` → `saveSettings({accounts, active_account_uuid})` (88-95).
- Microsoft wizard: `api.auth.microsoftStart()` → shows `user_code`/`verification_uri` → `setInterval` polling `poll_microsoft_status` until `done` → calls `complete_microsoft_auth_cmd` (100-140).
- Offline/Ely.by add flows mirror the same commands.
- Also hosts a separate **Xryloth website account** (email/password `signup`/`login`, admin flag) — unrelated to Minecraft auth (60-80).

---

## 7. Gotchas for rebuilders

1. **Dual storage drift.** Rust writes flat keys, UI writes `accounts[]`. A user who authenticates via an older UI (or edits settings.json) can end up with `accounts[]` pointing at a stale token while `mc_token` is fresh, or vice-versa. Pick ONE store when rebuilding.
2. **No token refresh.** `refresh_token` is saved but never exchanged; sessions expire silently and the game fails auth until re-login.
3. **Offline UUID is all-zeros**, so servers in online mode won't accept them and offline servers may collide multiple users.
4. **`is_connected()` is a 2 s HEAD to sessionserver** with a 10 s cache — offline launches are decided by that single probe, not by the actual auth outcome.
5. **Device-code polling runs on a raw `std::thread`** with `min(interval,5)` s sleeps — the UI only ever sees `pending/done/idle`, so multiple tabs/windows polling simultaneously share one `ms_auth_result` slot (cleared on every new `authenticate_microsoft`, 1554-1558).
6. **authlib-injector fallback URL** is a hardcoded MediaFire direct link (1862) — it will rot; resolve dynamically when rebuilding.
7. Client id `00000000402b5328` + scope `MinecraftWin32.app` is the Minecraft Launcher's public id — required for the device flow to be allowed by Microsoft.

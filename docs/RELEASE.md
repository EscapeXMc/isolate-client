# Release Checklist (Isolate Client)

Har release par ye steps. Updater + live announcements isi flow se chalte hain.

## 1. Version bump
- [ ] `package.json` → `version` (semver: `X.Y.Z`)
- [ ] `src-tauri/Cargo.toml` → `[package] version`
- [ ] `src-tauri/tauri.conf.json` → `version` (agar set hai)
- [ ] `npm run build` passes (tsc + vite)

## 2. Build via CI (primary)
- [ ] Commit + push (`client: …` / `site: …` prefix, author EscapeXMc)
- [ ] GitHub Actions → `build.yml` → **Run workflow**
- [ ] Release job clobbers assets on `v0.1.0` release:
      `IsolateClient_<ver>_windows64-setup.exe`, `_x64.msi`, `_windows-x64.zip`
- [ ] Note asset URL + size from the release page

## 3. Checksum
```bash
curl -4 -L -o /tmp/rel.exe <asset-url>
sha256sum /tmp/rel.exe   # → sha256
```

## 4. Firebase push (live release state + announcement)
```bash
FIREBASE_DB_URL=https://isolateclient-default-rtdb.firebaseio.com \
FIREBASE_AUTH=<RTDB secret — Firebase Console → Realtime Database → … → Database secrets> \
node scripts/push-release.mjs \
  --version X.Y.Z \
  --url <asset-url> --sha256 <hex> --size <bytes> \
  --notes-file /tmp/notes.md \
  --announce-id "YYYY-MM-DD-X.Y.Z" \
  --announce-title "X.Y.Z is live!" \
  --announce-message "<one-liner>"
```
- `--force` → clients ko "Later" nahi dikhega (forceUpdate)
- `--min-version A.B.C` → usse purane clients forced update honge
- Open clients ke liye: **15 second** me `release-update` event chala jayega
  (announcement banner + update modal turant).

Alternative: `release-notify.yml` workflow (release publish par automatically
chalta hai — repo secrets `FIREBASE_DB_URL` + `FIREBASE_AUTH` set hone par).

## 5. Verify
- [ ] `curl -4 "$FIREBASE_DB_URL/releases/latest.json"` → naya version dikh raha
- [ ] App restart → update modal (ya banner) aaya
- [ ] Update Now → Downloading → Verifying (sha256) → Installing → Restart
- [ ] `~/.isolate/install.log` me `[start]/[done]` lines (instance installs)

## Firebase schema (reference)
```json
{
  "releases": {
    "latest": {
      "latestVersion": "X.Y.Z",
      "minSupportedVersion": "0.1.0",
      "forceUpdate": false,
      "downloadUrl": "https://github.com/EscapeXMc/isolate-client/releases/download/vX.Y.Z/…setup.exe",
      "sha256": "<hex>",
      "size": 2939554,
      "releaseNotes": "## What's fixed\n- …",
      "updatedAt": 1763800000000
    },
    "announcement": {
      "announcementId": "YYYY-MM-DD-X.Y.Z",
      "announcementTitle": "X.Y.Z is live!",
      "announcementMessage": "…",
      "updatedAt": 1763800000000
    }
  }
}
```
Rules: public **read**, admin-only **write** (client sirf padhta hai).

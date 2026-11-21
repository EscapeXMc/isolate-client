#!/usr/bin/env node
// ============================================================
// Push a release (+ optional live announcement) to the Firebase
// RTDB `releases/*` nodes that check_release_live / the in-app
// listener read. Secrets come from env — never commit them.
//
//   FIREBASE_DB_URL=https://<proj>-default-rtdb.firebaseio.com \
//   FIREBASE_AUTH=<database-secret> \
//   node scripts/push-release.mjs \
//     --version 0.2.0 \
//     --url https://github.com/EscapeXMc/isolate-client/releases/download/v0.2.0/IsolateClient_0.2.0_windows64-setup.exe \
//     --sha256 <hex> --size 2939554 \
//     [--notes-file notes.md] [--force] [--min-version 0.1.0] \
//     [--announce-id 2026-11-22-0.2.0 --announce-title "0.2.0 is live!" \
//      --announce-message "Auto-updates are here."]
// ============================================================
import { readFileSync } from "node:fs";

const args = process.argv.slice(2);
const opt = (name, dflt = "") => {
  const i = args.indexOf(`--${name}`);
  return i >= 0 && args[i + 1] !== undefined ? args[i + 1] : dflt;
};
const has = (name) => args.includes(`--${name}`);

const DB = process.env.FIREBASE_DB_URL?.replace(/\/$/, "");
const AUTH = process.env.FIREBASE_AUTH || "";

const version = opt("version");
const url = opt("url");
const sha256 = opt("sha256");
if (!DB || !AUTH) {
  console.error("FIREBASE_DB_URL and FIREBASE_AUTH env vars are required.");
  process.exit(1);
}
if (!version || !url) {
  console.error("--version and --url are required.");
  process.exit(1);
}

const latest = {
  latestVersion: version,
  minSupportedVersion: opt("min-version", "0.1.0"),
  forceUpdate: has("force"),
  downloadUrl: url,
  sha256,
  size: Number(opt("size", "0")) || undefined,
  updatedAt: Date.now(),
};
if (has("notes-file")) {
  latest.releaseNotes = readFileSync(opt("notes-file"), "utf8");
}
for (const k of Object.keys(latest)) {
  if (latest[k] === undefined) delete latest[k];
}

async function put(path, body) {
  const res = await fetch(`${DB}/${path}.json?auth=${encodeURIComponent(AUTH)}`, {
    method: "PUT",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(body),
  });
  if (!res.ok) {
    throw new Error(`${path}: HTTP ${res.status} ${await res.text()}`);
  }
  return res.json();
}

const results = [];
results.push(["releases/latest", await put("releases/latest", latest)]);

if (opt("announce-id") || opt("announce-title")) {
  const ann = {
    announcementId: opt("announce-id", `ann-${Date.now()}`),
    announcementTitle: opt("announce-title", ""),
    announcementMessage: opt("announce-message", ""),
    updatedAt: Date.now(),
  };
  results.push(["releases/announcement", await put("releases/announcement", ann)]);
}

for (const [path, val] of results) {
  console.log(`✓ ${path} →`, JSON.stringify(val));
}
console.log("Done. Clients pick this up within 15s (listener poll).");

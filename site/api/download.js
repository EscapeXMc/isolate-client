/**
 * Resolves a platform to the newest matching GitHub release asset and
 * redirects to it (302).
 *
 *   GET /api/download?platform=windows
 *
 * Platform keys:
 *   windows            Windows x64 launcher (.exe)
 *   windows-x86        Windows x86 launcher (.exe)
 *   windows-installer  Windows installer (universal .exe)
 *   linux              Linux launcher (.AppImage)
 *   linux-deb          Linux .deb package
 *
 * The launcher ships from EscapeXMc/isolate-client, the installer — which owns the
 * desktop shortcuts and the update flow — from EscapeXMc/isolate-client.
 * Both repositories are public, so the asset list is read straight from the
 * GitHub API and the visitor is forwarded to the real file.
 *
 * This is the no-JavaScript fallback for the download buttons: the anchors
 * point here by default and are upgraded to direct GitHub URLs by
 * public/isolate-downloads.js once the browser has resolved them.
 */
import { countDownload } from "../lib/fb-count.js";

const LAUNCHER = "EscapeXMc/isolate-client";
const INSTALLER = "EscapeXMc/isolate-client";

// The302 used to be edge-cached (s-maxage=300), which meant repeat clicks
// inside the window never reached this function — and never got counted.
// Counting now runs on every request, so the GitHub lookup is cached in
// memory instead (warm lambdas live for minutes;5 min TTL keeps us far
// below the unauthenticated rate limit).
const GITHUB_TTL_MS = 5 * 60 * 1000;
let releaseCache = { at: 0, data: null };

async function latestRelease(repo) {
  const now = Date.now();
  if (releaseCache.data && now - releaseCache.at < GITHUB_TTL_MS) {
    return releaseCache.data;
  }
  const r = await fetch(
    `https://api.github.com/repos/${repo}/releases/latest`,
    { headers: { Accept: "application/vnd.github+json" } },
  );
  if (!r.ok) throw new Error(`GitHub API ${r.status}`);
  const data = await r.json();
  releaseCache = { at: now, data };
  return data;
}

const PLATFORMS = {
  // Canonical asset names (set by scripts/release.sh + the CI release job):
  //   IsolateClient_<ver>_windows64-setup.exe / _windows_universal.exe /
  //   IsolateClient_<ver>_amd64.AppImage / isolate-client_<ver>_amd64.deb
  // Matchers stay aligned with site/public/isolate-downloads.js so the
  // JS and no-JS paths hand over the same file.
  windows: { repo: INSTALLER, match: /windows64/i },
  "windows-x86": { repo: INSTALLER, match: /windows32/i },
  "windows-installer": { repo: INSTALLER, match: /windows_universal/i },
  linux: { repo: INSTALLER, match: /appimage/i },
  "linux-deb": { repo: INSTALLER, match: /\.deb$/i },
};

export default async function handler(req, res) {
  if (req.method !== "GET" && req.method !== "HEAD") {
    res.setHeader("Allow", "GET, HEAD");
    return res.status(405).json({ error: "Method not allowed" });
  }

  const platform = String(
    (req.query && req.query.platform) || ""
  ).toLowerCase();
  const spec = Object.prototype.hasOwnProperty.call(PLATFORMS, platform)
    ? PLATFORMS[platform]
    : null;

  if (!spec) {
    res.setHeader("Cache-Control", "no-store");
    return res.status(400).json({ error: "Unknown platform" });
  }

  // Count this click toward the hero total. Started now, awaited right before
  // the redirect so the count is never lost, and errors never block the file.
  const counted = countDownload().catch((e) => console.error("download count", e));

  try {
    const data = await latestRelease(spec.repo);
    const assets = Array.isArray(data && data.assets) ? data.assets : [];
    const hit = assets.find((a) => spec.match.test((a && a.name) || ""));
    if (!hit || !hit.browser_download_url) throw new Error("No matching asset");

    await counted;
    // no-store: the browser and the edge must re-run this handler on every
    // click so each one lands in the counter.
    res.setHeader("Cache-Control", "no-store");
    res.setHeader("Location", hit.browser_download_url);
    return res.status(302).end();
  } catch {
    // Never dead-end the visitor: send them to the releases page instead.
    await counted;
    res.setHeader("Cache-Control", "no-store");
    res.setHeader("Location", `https://github.com/${spec.repo}/releases/latest`);
    return res.status(302).end();
  }
}

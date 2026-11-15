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
const LAUNCHER = "EscapeXMc/isolate-client";
const INSTALLER = "EscapeXMc/isolate-client";

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

  try {
    const r = await fetch(
      `https://api.github.com/repos/${spec.repo}/releases/latest`,
      { headers: { Accept: "application/vnd.github+json" } },
    );
    if (!r.ok) throw new Error(`GitHub API ${r.status}`);
    const data = await r.json();
    const assets = Array.isArray(data && data.assets) ? data.assets : [];
    const hit = assets.find((a) => spec.match.test((a && a.name) || ""));
    if (!hit || !hit.browser_download_url) throw new Error("No matching asset");

    res.setHeader(
      "Cache-Control",
      "public, s-maxage=300, stale-while-revalidate=3600",
    );
    res.setHeader("Location", hit.browser_download_url);
    return res.status(302).end();
  } catch {
    // Never dead-end the visitor: send them to the releases page instead.
    res.setHeader("Cache-Control", "no-store");
    res.setHeader("Location", `https://github.com/${spec.repo}/releases/latest`);
    return res.status(302).end();
  }
}

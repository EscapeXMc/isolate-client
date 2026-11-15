/**
 * Live GitHub release resolver for the Isolate Client download buttons.
 *
 * The site is a static export, so nothing knows at build time which release is
 * current. Every download link therefore resolves at runtime:
 *
 *   1. Ask the GitHub API for the latest release of the launcher repo
 *      (EscapeXMc/isolate-client) and of the installer repo
 *      (EscapeXMc/isolate-client).
 *   2. Match the published assets against the platform the visitor asked for.
 *   3. Point the link straight at the real asset and fill in its file size.
 *
 * If the API is unreachable (offline, rate limited, blocked) the links fall
 * back to /api/download?platform=..., a serverless function that performs the
 * same lookup server-side and 302-redirects. That fallback is also what makes
 * the links work with JavaScript disabled.
 *
 * Markup contract
 * ---------------
 *   <a data-download="windows">          href is set to the live asset
 *   <span data-download-size="windows">   text is set to "13 MB"
 *
 * Platform keys: windows, windows-x86, windows-installer, linux, linux-deb.
 */
(function () {
  "use strict";

  var LAUNCHER = "EscapeXMc/isolate-client";
  var INSTALLER = "EscapeXMc/isolate-client";
  var API = "https://api.github.com/repos/";

  /**
   * Platform key -> [repo, asset matcher].
   *
   * The launcher ships from EscapeXMc/isolate-client, the installer — which owns the
   * desktop shortcuts and the update flow — from EscapeXMc/isolate-client.
   * There is no macOS build, so macOS is deliberately absent.
   */
  var PLATFORMS = {
    windows: [LAUNCHER, /windows64/i],
    "windows-x86": [LAUNCHER, /windows32/i],
    "windows-installer": [INSTALLER, /windows_universal/i],
    linux: [LAUNCHER, /appimage/i],
    "linux-deb": [INSTALLER, /\.deb$/i]
  };

  var CACHE_KEY = "isolate-releases-v1";
  var CACHE_TTL = 15 * 60 * 1000; // 15 minutes

  var cache = null; // { at, map }
  var pending = null; // in-flight promise

  /** A working URL for a platform even when the GitHub API is unreachable. */
  function fallback(platform) {
    if (!Object.prototype.hasOwnProperty.call(PLATFORMS, platform)) return null;
    return "/api/download?platform=" + encodeURIComponent(platform);
  }

  function readCache(anyAge) {
    try {
      var raw = localStorage.getItem(CACHE_KEY);
      if (!raw) return null;
      var parsed = JSON.parse(raw);
      if (!parsed || typeof parsed.at !== "number" || !parsed.map) return null;
      if (!anyAge && Date.now() - parsed.at > CACHE_TTL) return null;
      return parsed.map;
    } catch (e) {
      return null;
    }
  }

  function writeCache(map) {
    try {
      localStorage.setItem(
        CACHE_KEY,
        JSON.stringify({ at: Date.now(), map: map })
      );
    } catch (e) {
      /* private mode or quota — the in-memory cache still works */
    }
  }

  function humanSize(bytes) {
    if (typeof bytes !== "number" || bytes <= 0) return null;
    var mb = bytes / 1048576;
    if (mb >= 100) return Math.round(mb) + " MB";
    if (mb >= 10) return mb.toFixed(1) + " MB";
    return mb.toFixed(2) + " MB";
  }

  function latestAssets(repo) {
    return fetch(API + repo + "/releases/latest", {
      headers: { Accept: "application/vnd.github+json" },
      cache: "no-store"
    })
      .then(function (r) {
        if (!r.ok) throw new Error("GitHub API " + r.status);
        return r.json();
      })
      .then(function (data) {
        var assets = (data && data.assets) || [];
        return assets.map(function (a) {
          return {
            name: a.name || "",
            url: a.browser_download_url || "",
            size: typeof a.size === "number" ? a.size : 0
          };
        });
      });
  }

  function buildMap() {
    return Promise.all([latestAssets(LAUNCHER), latestAssets(INSTALLER)]).then(
      function (lists) {
        var byRepo = {};
        lists.forEach(function (list, i) {
          byRepo[i === 0 ? LAUNCHER : INSTALLER] = list;
        });

        var out = {};
        Object.keys(PLATFORMS).forEach(function (platform) {
          var repo = PLATFORMS[platform][0];
          var matcher = PLATFORMS[platform][1];
          var assets = byRepo[repo] || [];
          for (var i = 0; i < assets.length; i++) {
            if (matcher.test(assets[i].name)) {
              out[platform] = {
                url: assets[i].url,
                size: assets[i].size,
                sizeText: humanSize(assets[i].size),
                name: assets[i].name
              };
              break;
            }
          }
        });
        return out;
      }
    );
  }

  function load(force) {
    if (!force && cache && Date.now() - cache.at < CACHE_TTL) {
      return Promise.resolve(cache.map);
    }
    if (pending) return pending;

    var stored = force ? null : readCache(false);
    if (stored) {
      cache = { at: Date.now(), map: stored };
      return Promise.resolve(stored);
    }

    pending = buildMap()
      .then(function (map) {
        cache = { at: Date.now(), map: map };
        writeCache(map);
        pending = null;
        return map;
      })
      .catch(function (err) {
        pending = null;
        // Serve a stale copy rather than nothing at all.
        var stale = readCache(true);
        if (stale) return stale;
        throw err;
      });

    return pending;
  }

  /** Promise<string|null> — the direct GitHub URL for a platform. */
  function resolve(platform) {
    if (!Object.prototype.hasOwnProperty.call(PLATFORMS, platform)) {
      return Promise.resolve(null);
    }
    return load().then(
      function (map) {
        return (map[platform] && map[platform].url) || fallback(platform);
      },
      function () {
        return fallback(platform);
      }
    );
  }

  /** Promise<object> — every platform resolved to a usable URL. */
  function resolveAll() {
    function build(map) {
      var out = {};
      Object.keys(PLATFORMS).forEach(function (p) {
        out[p] = (map && map[p] && map[p].url) || fallback(p);
      });
      return out;
    }
    return load().then(build, function () {
      return build(null);
    });
  }

  /** Promise<object> — every platform resolved to { url, size, sizeText }. */
  function details() {
    function build(map) {
      var out = {};
      Object.keys(PLATFORMS).forEach(function (p) {
        var hit = map && map[p];
        out[p] = hit
          ? hit
          : { url: fallback(p), size: 0, sizeText: null, name: null };
      });
      return out;
    }
    return load().then(build, function () {
      return build(null);
    });
  }

  /**
   * Point every [data-download] link on the page at the live asset and fill
   * in any [data-download-size] labels.
   */
  function applyLinks(root) {
    var scope = root || document;
    var nodes = scope.querySelectorAll("[data-download]");
    var sizes = scope.querySelectorAll("[data-download-size]");
    if (!nodes.length && !sizes.length) return Promise.resolve({});
    return details().then(function (map) {
      var i;
      for (i = 0; i < nodes.length; i++) {
        var node = nodes[i];
        var key = node.getAttribute("data-download");
        var hit = map[key];
        if (hit && hit.url) node.setAttribute("href", hit.url);
      }
      for (i = 0; i < sizes.length; i++) {
        var label = sizes[i];
        var lkey = label.getAttribute("data-download-size");
        var lhit = map[lkey];
        if (lhit && lhit.sizeText) label.textContent = lhit.sizeText;
      }
      try {
        window.dispatchEvent(
          new CustomEvent("isolate:downloads", { detail: map })
        );
      } catch (e) {
        /* CustomEvent unsupported — the hrefs are already set */
      }
      return map;
    });
  }

  window.IsolateDownloads = {
    platforms: Object.keys(PLATFORMS),
    fallback: fallback,
    resolve: resolve,
    resolveAll: resolveAll,
    details: details,
    applyLinks: applyLinks,
    refresh: function () {
      return load(true);
    }
  };

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", function () {
      applyLinks();
    });
  } else {
    applyLinks();
  }
})();

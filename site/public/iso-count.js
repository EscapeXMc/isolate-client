/**
 * Real download counter for the Isolate Client site.
 *
 * Counting — every download-button click adds exactly 1:
 *   - Links still pointing at /api/download are counted server-side. That
 *     route runs on every click (no-store, GitHub lookup cached in memory),
 *     so this script deliberately stays quiet and the click is never
 *     counted twice.
 *   - Links already rewritten by isolate-downloads.js to direct GitHub URLs
 *     (or any releases/download / [download] anchor) never reach our API, so
 *     a navigator.sendBeacon("/api/stats") fires instead — beacons survive
 *     the navigation away from the page.
 *
 * Display — the homepage hero gets an injected line under the platform list:
 *   <p class="iso-dl-count">  ⤓ 1,234 downloads </p>
 * fed from GET /api/stats (GitHub historical baseline + counted clicks),
 * bumped locally on the visitor's own click, refreshed every 60s.
 * Hidden until the first successful fetch — a stale 0 would be a lie.
 */
(function () {
  "use strict";

  var STATS_URL = "/api/stats";
  var numEl = null;
  var current = null;

  function isDownloadLink(a) {
    if (!a || a.tagName !== "A") return false;
    if (a.classList.contains("magmad-download") || a.hasAttribute("data-download")) {
      return true;
    }
    var h = a.getAttribute("href") || "";
    if (h.indexOf("/api/download") === 0) return true;
    if (a.hasAttribute("download")) return true;
    if (/github\.com\/[^/]+\/[^/]+\/releases\/download\//.test(h)) return true;
    return false;
  }

  /** True when the server (not this script) will count the click. */
  function countedServerSide(a) {
    var h = a.getAttribute("href") || "";
    return h.indexOf("/api/download") === 0;
  }

  function format(n) {
    return Number(n).toLocaleString("en-US");
  }

  function render(n) {
    current = n;
    if (numEl) numEl.textContent = format(n);
  }

  function bump() {
    if (typeof current === "number") render(current + 1);
  }

  function beacon() {
    try {
      if (navigator.sendBeacon && navigator.sendBeacon(STATS_URL)) return;
    } catch (e) {
      /* fall through to fetch */
    }
    try {
      fetch(STATS_URL, { method: "POST", keepalive: true }).catch(function () {});
    } catch (e) {
      /* offline — the number simply misses this one click */
    }
  }

  function fetchStats() {
    return fetch(STATS_URL, {
      headers: { Accept: "application/json" },
      cache: "no-store",
    })
      .then(function (r) {
        return r.ok ? r.json() : null;
      })
      .then(function (j) {
        if (j && typeof j.downloads === "number") render(j.downloads);
        return j;
      })
      .catch(function () {
        return null;
      });
  }

  /** Inject the counter line right under the hero's platform list. */
  function injectHero() {
    var anchor =
      document.querySelector("#download .magmad-platforms") ||
      document.querySelector(".magmad-platforms");
    if (!anchor || document.querySelector(".iso-dl-count")) return null;
    var wrap = document.createElement("p");
    wrap.className = "iso-dl-count flex items-center gap-2 pt-1 text-sm text-foreground-2";
    wrap.setAttribute("aria-live", "polite");
    wrap.style.display = "none";
    wrap.innerHTML =
      '<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" ' +
      'fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" ' +
      'stroke-linejoin="round" class="size-4 shrink-0 text-accent-1" aria-hidden="true">' +
      '<path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/>' +
      '<polyline points="7 10 12 15 17 10"/>' +
      '<line x1="12" x2="12" y1="15" y2="3"/></svg>' +
      '<span><strong id="iso-dl-num" class="font-bold text-foreground">0</strong> downloads</span>';
    anchor.insertAdjacentElement("afterend", wrap);
    numEl = wrap.querySelector("#iso-dl-num");
    return wrap;
  }

  function start() {
    var wrap = injectHero();
    fetchStats().then(function (j) {
      if (wrap && j && typeof j.downloads === "number") wrap.style.display = "";
    });
    if (wrap) {
      setInterval(function () {
        if (!document.hidden) fetchStats();
      }, 60000);
    }
  }

  // Capture-phase so the count lands even when the anchor triggers navigation.
  document.addEventListener(
    "click",
    function (e) {
      var a = e.target && e.target.closest ? e.target.closest("a") : null;
      if (!isDownloadLink(a)) return;
      bump();
      if (!countedServerSide(a)) beacon();
    },
    true,
  );

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", start);
  } else {
    start();
  }
})();

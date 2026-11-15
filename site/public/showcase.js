/*
 * Isolate Client — homepage showcase carousel controller
 *
 * The Xryloth/Isolate static export ships the "Take A Look Inside" carousel as
 * server-rendered markup that only becomes interactive after React hydrates.
 * Hydration never completes in this build (see /__diag beacons in the entry
 * chunk), so the arrows and dots were dead. This tiny controller wires the
 * carousel with plain DOM: arrows, dots, slide titles/descriptions.
 *
 * Slide titles + descriptions are recovered from the Next.js flight payload
 * (self.__next_f) that is embedded in every index page, so localized pages
 * keep their translated copy. If parsing fails, images/dots still switch.
 */
(function () {
  "use strict";

  /* Pull the "slides" array out of the embedded RSC flight payload. */
  function parseSlides() {
    try {
      var parts = window.__next_f || [];
      var ft = "";
      for (var i = 0; i < parts.length; i++) {
        if (parts[i] && parts[i][0] === 1) ft += parts[i][1];
      }
      var m = ft.match(/"slides":\[/);
      if (!m) return null;
      var start = m.index + m[0].length - 1; /* position of '[' */
      var depth = 0, inStr = false, esc = false, end = -1;
      for (var j = start; j < ft.length; j++) {
        var c = ft.charAt(j);
        if (inStr) {
          if (esc) { esc = false; }
          else if (c === "\\") { esc = true; }
          else if (c === '"') { inStr = false; }
          continue;
        }
        if (c === '"') { inStr = true; continue; }
        if (c === "[") { depth++; continue; }
        if (c === "]") {
          depth--;
          if (depth === 0) { end = j; break; }
        }
      }
      if (end < 0) return null;
      var slides = JSON.parse(ft.substring(start, end + 1));
      return Array.isArray(slides) && slides.length ? slides : null;
    } catch (e) {
      return null;
    }
  }

  function show(root) {
    if (root.getAttribute("data-showcase-wired") === "1") return;
    root.setAttribute("data-showcase-wired", "1");

    var imgs = Array.prototype.slice.call(
      root.querySelectorAll("div.grid > img")
    );
    if (!imgs.length) return;

    var prevBtn = root.querySelector('button[aria-label="Previous screenshot"]');
    var nextBtn = root.querySelector('button[aria-label="Next screenshot"]');
    var dots = Array.prototype.slice.call(
      root.querySelectorAll('button[aria-label^="Show "]')
    );
    var box = root.querySelector("div.max-w-140");
    var paras = box ? box.querySelectorAll("p") : [];
    var slides = parseSlides();
    var n = imgs.length;
    var cur = 0;

    var ACTIVE_DOT =
      "size-3 rounded-full border transition-colors border-foreground bg-foreground";
    var IDLE_DOT =
      "size-3 rounded-full border transition-colors border-muted-foreground bg-secondary hover:bg-muted-foreground";

    function render(i) {
      cur = ((i % n) + n) % n;
      for (var k = 0; k < n; k++) {
        var active = k === cur;
        var img = imgs[k];
        img.classList.toggle("opacity-100", active);
        img.classList.toggle("opacity-0", !active);
        img.classList.toggle("pointer-events-none", !active);
        img.setAttribute("aria-hidden", active ? "false" : "true");
        if (dots[k]) {
          dots[k].setAttribute("aria-pressed", active ? "true" : "false");
          dots[k].className = active ? ACTIVE_DOT : IDLE_DOT;
        }
      }
      var slide = slides && slides[cur];
      if (paras[0]) {
        paras[0].textContent = slide
          ? slide.title
          : (dots[cur]
              ? dots[cur].getAttribute("aria-label").replace(/^Show\s+/, "")
              : paras[0].textContent);
      }
      if (paras[1] && slide && slide.description) {
        paras[1].textContent = slide.description;
      }
    }

    if (prevBtn) prevBtn.addEventListener("click", function () { render(cur - 1); });
    if (nextBtn) nextBtn.addEventListener("click", function () { render(cur + 1); });
    dots.forEach(function (d, k) {
      d.addEventListener("click", function () { render(k); });
    });

    render(0);
  }

  function boot() {
    var root = document.querySelector("[data-showcase]");
    if (root) show(root);
  }

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", boot);
  } else {
    boot();
  }
})();

/* Isolate Client page transitions — framework-free fade between pages. */
(function () {
  "use strict";
  var html = document.documentElement;
  var reduce = false;
  try { reduce = window.matchMedia("(prefers-reduced-motion: reduce)").matches; } catch (e) {}

  // FastClient-style overhaul: pure-black ground, glass panels, neon accents
  // and the white SVG cursor suite. Appended early (no FOUC), then moved to
  // the END of <head> on DOMContentLoaded so it wins the cascade over each
  // page's own stylesheet.
  var isoCss = document.createElement("link");
  isoCss.rel = "stylesheet";
  isoCss.href = "/assets/iso-override.css";
  document.head.appendChild(isoCss);
  document.addEventListener("DOMContentLoaded", function () {
    if (isoCss.parentNode === document.head) document.head.appendChild(isoCss);
  });

  // Pose-to-pose staging: reveal each <section> once as it scrolls into view
  // (CSS in iso-override.css animates .iso-enter children with staggered delays).
  document.addEventListener("DOMContentLoaded", function () {
    if (reduce || !("IntersectionObserver" in window)) return;
    var io = new IntersectionObserver(
      function (entries) {
        entries.forEach(function (en) {
          if (en.isIntersecting) {
            en.target.classList.add("iso-enter");
            io.unobserve(en.target);
          }
        });
      },
      { rootMargin: "-8% 0px -8% 0px", threshold: 0.12 }
    );
    document.querySelectorAll("main section, #feed > article").forEach(function (s) {
      io.observe(s);
    });
  });

  var style = document.createElement("style");
  style.textContent =
    "html{opacity:1;transition:opacity .22s ease}" +
    "html.xf-hide{opacity:0}" +
    "#xf-toast{position:fixed;left:50%;bottom:28px;transform:translate(-50%,16px);" +
    "opacity:0;pointer-events:none;z-index:9999;display:flex;gap:12px;align-items:center;" +
    "background:#131316;color:#f4f4f5;border:1px solid rgba(52,211,153,.45);border-radius:14px;" +
    "padding:13px 20px;box-shadow:0 18px 50px rgba(0,0,0,.55);max-width:min(92vw,460px);" +
    "transition:opacity .25s ease,transform .25s ease;" +
    "font-family:ui-sans-serif,system-ui,-apple-system,'Segoe UI',Roboto,Arial,sans-serif}" +
    "#xf-toast.show{opacity:1;transform:translate(-50%,0);pointer-events:auto}" +
    "#xf-toast .ic{width:34px;height:34px;border-radius:10px;flex:none;display:grid;place-items:center;" +
    "background:linear-gradient(180deg,#34d399,#34d399);color:#0a0a0a;font-size:16px;font-weight:900}" +
    "#xf-toast h4{margin:0;font-size:13.5px;font-weight:800;letter-spacing:.02em}" +
    "#xf-toast p{margin:2px 0 0;color:#acacac;font-size:11.5px;line-height:1.4}" +
    "@media (prefers-reduced-motion: reduce){html{transition:none}html.xf-hide{opacity:1}" +
    "#xf-toast{transition:none;transform:translate(-50%,0)}}";
  document.head.appendChild(style);

  if (!reduce) {
    // Boot hidden so the first paint comes from black, then fade in.
    html.classList.add("xf-hide");
    document.addEventListener("DOMContentLoaded", function () {
      requestAnimationFrame(function () {
        requestAnimationFrame(function () { html.classList.remove("xf-hide"); });
      });
    });
    // Restores from bfcache must not stay hidden.
    window.addEventListener("pageshow", function (ev) {
      if (ev && ev.persisted) html.classList.remove("xf-hide");
    });
  }

  // --------------------------------------------- "downloading" popup -------
  var toastEl = null, toastTimer = null;

  function dlLabel(a) {
    var href = "";
    try { href = decodeURIComponent(a.getAttribute("href") || ""); } catch (e) { href = a.getAttribute("href") || ""; }
    var platform = (href.match(/platform=([a-z-]+)/i) || [])[1] || "";
    if (/^windows/.test(platform)) return "Windows";
    if (/^linux/.test(platform)) return "Linux";
    if (/\.exe(\?|$)/i.test(href)) return "Windows";
    if (/\.(appimage|deb|rpm)(\?|$)/i.test(href)) return "Linux";
    return (/win/i.test(navigator.userAgent) ? "Windows" : "Linux");
  }

  function isDownloadClick(a) {
    if (a.hasAttribute("download")) return true;
    if (a.hasAttribute("data-download")) return true;
    var href = a.getAttribute("href") || "";
    if (/\/api\/download/.test(href)) return true;
    if (a.classList && a.classList.contains("magmad-download")) return true;
    if (/\.(exe|appimage|deb|rpm|msi|zip|tar\.gz|apk)(\?|#|$)/i.test(href)) return true;
    return false;
  }

  function toast(label) {
    if (!toastEl) {
      toastEl = document.createElement("div");
      toastEl.id = "xf-toast";
      toastEl.setAttribute("role", "status");
      toastEl.setAttribute("aria-live", "polite");
      toastEl.innerHTML =
        '<div class="ic" aria-hidden="true">&#11015;</div>'
        + '<div><h4 id="xf-toast-title"></h4>'
        + '<p>Your download will start automatically — check your browser\u2019s download bar.</p></div>';
      document.body.appendChild(toastEl);
    }
    toastEl.querySelector("#xf-toast-title").textContent =
      "Downloading Isolate Client for " + label + " \u2026";
    toastEl.classList.add("show");
    clearTimeout(toastTimer);
    toastTimer = setTimeout(function () { toastEl.classList.remove("show"); }, 5000);
  }

  // Fade out before leaving for another same-origin page.
  document.addEventListener("click", function (ev) {
    if (reduce) return;
    if (ev.defaultPrevented || ev.button !== 0) return;
    if (ev.metaKey || ev.ctrlKey || ev.shiftKey || ev.altKey) return;
    var a = ev.target && ev.target.closest ? ev.target.closest("a[href]") : null;
    if (!a) return;
    if (isDownloadClick(a)) {
      // Let the download proceed normally (no navigation, no fade) — just
      // tell the visitor it has started.
      toast(dlLabel(a));
      return;
    }
    var href = a.getAttribute("href") || "";
    if (!href || href.charAt(0) === "#") return;
    if (a.target && a.target !== "_self") return;          // new tabs: no fade
    var u;
    try { u = new URL(a.href, location.href); } catch (e) { return; }
    if (u.origin !== location.origin) return;              // external: plain jump
    if (u.href === location.href && u.hash) return;        // in-page anchors
    if (u.href === location.href) return;
    ev.preventDefault();
    html.classList.add("xf-hide");
    setTimeout(function () { location.assign(u.href); }, 170);
  });
})();

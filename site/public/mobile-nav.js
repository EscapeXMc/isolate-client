/* Isolate Client — static mobile nav controller (shadcn pages).
   Works without React hydration; backs off automatically if React ever
   attaches to the burger button (checks __react* fiber keys on click). */
(function () {
  var btn = document.querySelector('button[aria-controls="public-mobile-navigation"]');
  var panel = document.getElementById("public-mobile-navigation");
  if (!btn || !panel) return;

  // Position the panel against the header (skip if already sticky/relative).
  var hdr = btn.closest("header");
  if (hdr && getComputedStyle(hdr).position === "static") {
    hdr.style.position = "relative";
  }

  function reactOwns() {
    try {
      return Object.getOwnPropertyNames(btn).some(function (k) {
        return k.indexOf("__react") === 0;
      });
    } catch (e) {
      return false;
    }
  }

  function set(open) {
    if (reactOwns()) return;
    panel.classList.toggle("open", open);
    btn.setAttribute("aria-expanded", open ? "true" : "false");
    btn.setAttribute("aria-label", open ? "Close navigation menu" : "Open navigation menu");
  }

  btn.addEventListener("click", function (e) {
    if (reactOwns()) return;
    e.stopPropagation();
    set(!panel.classList.contains("open"));
  });
  document.addEventListener("click", function (e) {
    if (reactOwns()) return;
    if (!panel.contains(e.target) && !btn.contains(e.target)) set(false);
  });
  document.addEventListener("keydown", function (e) {
    if (e.key === "Escape") set(false);
  });
  panel.querySelectorAll("a").forEach(function (a) {
    a.addEventListener("click", function () { set(false); });
  });
})();

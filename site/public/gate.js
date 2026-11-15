/* Isolate Client — launch gate
   Locks every page until 2026-11-18 15:46:07 IST (10:16:07 UTC).
   Owner bypass: /18nov/ writes localStorage.iso_owner = "1", then this
   script lets that browser straight through. No flag -> /launch.html. */
(function () {
  var LAUNCH_MS = 1794996967000; // Date.UTC(2026, 10, 18, 10, 16, 7)

  // Never gate the gate itself (launch page / owner page).
  var path = location.pathname;
  if (path === "/launch.html" || path === "/launch" || path.indexOf("/18nov") === 0) return;

  var owner = false;
  try {
    owner = localStorage.getItem("iso_owner") === "1";
  } catch (e) {
    /* private mode -> stay locked */
  }

  if (owner) {
    document.documentElement.classList.add("iso-owner");
    return;
  }

  if (Date.now() < LAUNCH_MS) {
    // Hide before first paint so no content flashes through.
    document.documentElement.style.display = "none";
    location.replace("/launch.html");
  }
})();

/**
 * Pre-paint theme bootstrap.
 *
 * Loaded from <head> so the page never paints in the wrong colour scheme.
 * It must stay in sync with the ThemeProvider's storageKey ("isolate-appearance")
 * in the app bundle — the two used to disagree, which meant the theme the
 * visitor picked was never the one that was restored.
 */
(function () {
  "use strict";

  var KEY = "isolate-appearance";
  // The site is pure-black (#0a0a0a) only — always boot dark so Tailwind
  // `dark:` variants (splash invert, surfaces) resolve correctly regardless
  // of the visitor's OS preference.
  var DEFAULT = "dark";
  var root = document.documentElement;

  function apply(theme) {
    root.classList.remove("light", "dark");
    if (theme === "light" || theme === "dark") root.classList.add(theme);
    root.style.colorScheme = theme === "light" ? "light" : "dark";

    // Some Android WebViews force their own dark mode on top of ours and
    // invert the colours. Pinning the scheme stops that.
    var meta = document.querySelector('meta[name="color-scheme"]');
    if (meta) {
      meta.setAttribute(
        "content",
        theme === "light" ? "only light" : "only dark"
      );
    }
  }

  var stored = null;
  try {
    stored = localStorage.getItem(KEY);
  } catch (e) {
    /* private mode */
  }

  var theme = stored || DEFAULT;
  if (theme === "system") {
    theme =
      window.matchMedia &&
      window.matchMedia("(prefers-color-scheme: dark)").matches
        ? "dark"
        : "light";
  }
  // Pure-black redesign: dark is the only scheme, whatever was stored.
  theme = "dark";

  apply(theme);
})();

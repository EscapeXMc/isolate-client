/**
 * Real profile icons for the instance cards.
 *
 * Two sources, best-first:
 *  1. `<instance>/icon.png` — the backend copies the official Mojang asset
 *     `icons/icon_32x32.png` (from that version's asset index) into the
 *     profile folder, so every installed profile gets the *real* Minecraft
 *     logo instead of the instance-initial letter fallback.
 *  2. `/vico/{version}.png` — icons shipped in the bundle for versions we
 *     want to guarantee offline.
 *
 * Both return null when nothing is available so callers keep their letter
 * underneath and simply hide it via `onError`.
 */
import { convertFileSrc } from "@tauri-apps/api/tauri";

const VERSION_ICONS: Record<string, string> = {
  "1.8.9": "/vico/1.8.9.png",
};

export function pfIconSrc(version: string): string | null {
  // Every 1.8.x release shares the "1.8" asset index (same icon).
  if (version.startsWith("1.8")) return VERSION_ICONS["1.8.9"];
  return VERSION_ICONS[version] ?? null;
}

/** Absolute path → asset:// URL, or null outside the Tauri runtime. */
function assetUrl(path: string): string | null {
  try {
    return convertFileSrc(path) || null;
  } catch {
    // Browser preview has no `window.__TAURI__` — fall through to /vico.
    return null;
  }
}

/** Best available logo for a profile card. */
export function pfSrc(inst: { icon?: string | null; version: string }): string | null {
  if (inst.icon) {
    const url = assetUrl(inst.icon);
    if (url) return url;
  }
  return pfIconSrc(inst.version);
}

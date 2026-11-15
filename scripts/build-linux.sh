#!/usr/bin/env bash
# ============================================================
# Isolate Client — Linux release build (Step 4)
#
# Produces a standalone AppImage + .deb under
#   src-tauri/target/release/bundle/
#
# Low-RAM friendly: caps Cargo parallelism so a 4 GB machine (or a
# small CI runner) does not OOM during LTO/codegen, and reuses the
# existing target dir instead of rebuilding from scratch.
#
# Usage:
#   ./scripts/build-linux.sh            # release AppImage + deb
#   ./scripts/build-linux.sh --quick    # skip LTO (faster, bigger)
#   ./scripts/build-linux.sh --appimage  # AppImage only
# ============================================================
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

BUNDLES="appimage,deb"
QUICK=0
for arg in "$@"; do
  case "$arg" in
    --quick) QUICK=1 ;;
    --appimage) BUNDLES="appimage" ;;
    --deb) BUNDLES="deb" ;;
    -h|--help)
      sed -n '2,18p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
      exit 0 ;;
    *) echo "unknown flag: $arg" >&2; exit 2 ;;
  esac
done

# ---- low-RAM build knobs ------------------------------------
# nproc can be large on dev boxes; cap jobs so LTO doesn't eat all RAM.
CORES="$(getconf _NPROCESSORS_ONLN 2>/dev/null || echo 2)"
# ~1.5 GB of headroom per job keeps us under 4 GB total.
MAX_JOBS=$(( CORES > 4 ? 4 : CORES ))
export CARGO_BUILD_JOBS="$MAX_JOBS"
export CARGO_PROFILE_RELEASE_CODEGEN_UNITS=1
if [ "$QUICK" -eq 1 ]; then
  export CARGO_PROFILE_RELEASE_LTO=false
  echo "[build] quick mode: LTO off"
fi
# Vite: single-threaded esbuild keeps peak RSS low too.
export NODE_OPTIONS="--max-old-space-size=2048"

# ---- preflight ------------------------------------------------
command -v cargo >/dev/null || { echo "cargo not found — install Rust" >&2; exit 1; }
command -v npx   >/dev/null || { echo "npx not found — install Node.js" >&2; exit 1; }

# AppImage needs `linuxdeploy`/`appimagetool` via tauri's bundler; it
# downloads them on demand, so make sure we are online.
echo "[build] rustc $(rustc --version 2>/dev/null || echo '?')"
echo "[build] jobs=$MAX_JOBS bundles=$BUNDLES"

# ---- frontend -------------------------------------------------
echo "[build] 1/3  vite build"
npx tauri build --help >/dev/null 2>&1 || { echo "tauri CLI missing" >&2; exit 1; }
npm run build

# ---- rust bundle ---------------------------------------------
echo "[build] 2/3  cargo bundle ($BUNDLES)"
npx tauri build --bundles "$BUNDLES"

# ---- report ---------------------------------------------------
echo "[build] 3/3  artifacts"
BUNDLE_DIR="$ROOT/src-tauri/target/release/bundle"
found=0
while IFS= read -r f; do
  size="$(du -h "$f" | cut -f1)"
  echo "  ✓ $size  ${f#$ROOT/}"
  found=1
done < <(find "$BUNDLE_DIR" -maxdepth 3 -type f \( -name '*.AppImage' -o -name '*.deb' \) 2>/dev/null)

if [ "$found" -eq 0 ]; then
  echo "[build] no bundle produced — check the cargo output above" >&2
  exit 1
fi
echo "[build] done."

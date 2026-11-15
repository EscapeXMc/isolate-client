#!/usr/bin/env bash
# ============================================================
# Isolate Client — GitHub release upload (Step 7)
#
# Creates (if needed) the GitHub Release for this version and
# uploads whatever platform bundles exist locally, renamed to
# the canonical asset names the website's download resolvers
# match on:
#
#   IsolateClient_<ver>_amd64.AppImage          (or the tauri default,
#                                                which also matches)
#   isolate-client_<ver>_amd64.deb
#   IsolateClient_<ver>_windows64-setup.exe     ← NSIS installer
#   IsolateClient_<ver>_windows_universal.exe   ← same file, installer alias
#   IsolateClient_<ver>_x64.msi
#   IsolateClient_<ver>_windows-x64.zip         ← portable
#
# Windows bundles are normally attached by the CI release job
# (.github/workflows/build.yml) after the tag build finishes —
# this script uploads them too when they happen to exist locally
# (e.g. artifacts copied back from a CI run).
#
# Usage:
#   ./scripts/release.sh              # version from tauri.conf.json
#   ./scripts/release.sh 0.2.0        # explicit version
#   ./scripts/release.sh --dry-run    # show what would upload
#
# Requires: gh CLI authenticated (EscapeXMc), repo pushed.
# ============================================================
set -euo pipefail

REPO="EscapeXMc/isolate-client"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

DRY=0
VER=""
for arg in "$@"; do
  case "$arg" in
    --dry-run) DRY=1 ;;
    -h|--help) sed -n '2,25p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) VER="$arg" ;;
  esac
done

if [ -z "$VER" ]; then
  VER="$(python3 -c "import json;print(json.load(open('src-tauri/tauri.conf.json'))['version'])")"
fi
TAG="v${VER#v}"
VER="${TAG#v}"

if ! command -v gh >/dev/null 2>&1; then
  echo "[release] gh CLI not found — install gh first" >&2
  exit 1
fi
if ! gh auth status >/dev/null 2>&1; then
  echo "[release] gh not authenticated — run: gh auth login" >&2
  exit 1
fi

# ---- collect + rename local bundles -------------------------
STAGE="$ROOT/dist/release"
rm -rf "$STAGE"
mkdir -p "$STAGE"

add() { # add <src> <dest-name>
  [ -f "$1" ] || return 1
  cp "$1" "$STAGE/$2"
  echo "  ✓ $2  ($(du -h "$1" | cut -f1))"
}

found=0

# Linux — tauri's default names already match the site's regexes,
# but we normalise anyway so the release always looks the same.
img="$(find src-tauri/target/release/bundle/appimage -maxdepth 1 -name '*.AppImage' 2>/dev/null | head -1 || true)"
if [ -n "$img" ]; then add "$img" "IsolateClient_${VER}_amd64.AppImage" && found=1; fi

deb="$(find src-tauri/target/release/bundle/deb -maxdepth 1 -name '*.deb' 2>/dev/null | head -1 || true)"
if [ -n "$deb" ]; then add "$deb" "isolate-client_${VER}_amd64.deb" && found=1; fi

# Windows — NSIS setup → windows64 (client matcher) + universal alias.
setup="$(find src-tauri/target/release/bundle/nsis -maxdepth 1 -name '*-setup.exe' 2>/dev/null | head -1 || true)"
if [ -n "$setup" ]; then
  add "$setup" "IsolateClient_${VER}_windows64-setup.exe" && found=1
  add "$setup" "IsolateClient_${VER}_windows_universal.exe" && found=1
fi

msi="$(find src-tauri/target/release/bundle/msi -maxdepth 1 -name '*.msi' 2>/dev/null | head -1 || true)"
if [ -n "$msi" ]; then add "$msi" "IsolateClient_${VER}_x64.msi" && found=1; fi

zipf="$(find dist-portable -maxdepth 1 -name '*.zip' 2>/dev/null | head -1 || true)"
if [ -n "$zipf" ]; then add "$zipf" "IsolateClient_${VER}_windows-x64.zip" && found=1; fi

# CI artifacts copied into dist/ci/ (optional hand-off).
if [ -d dist/ci ]; then
  while IFS= read -r f; do
    base="$(basename "$f")"
    case "$base" in
      *-setup.exe) add "$f" "IsolateClient_${VER}_windows64-setup.exe" && found=1
                   add "$f" "IsolateClient_${VER}_windows_universal.exe" && found=1 ;;
      *.msi)       add "$f" "IsolateClient_${VER}_x64.msi" && found=1 ;;
      *windows-x64.zip) add "$f" "IsolateClient_${VER}_windows-x64.zip" && found=1 ;;
      *)           add "$f" "$base" && found=1 ;;
    esac
  done < <(find dist/ci -maxdepth 2 -type f 2>/dev/null)
fi

if [ "$found" -eq 0 ]; then
  echo "[release] no bundles found — run scripts/build-linux.sh (and/or -ps1) first" >&2
  exit 1
fi

echo "[release] tag=$TAG repo=$REPO"
if [ "$DRY" -eq 1 ]; then
  echo "[release] dry run — files staged in dist/release/:"
  ls -lh "$STAGE"
  exit 0
fi

# ---- create release if missing, then upload -----------------
if gh release view "$TAG" -R "$REPO" >/dev/null 2>&1; then
  echo "[release] release $TAG already exists — updating assets"
else
  echo "[release] creating release $TAG"
  # Prefer generated notes (needs commits on the repo); fall back to a stub.
  gh release create "$TAG" -R "$REPO" \
    --title "Isolate Client $VER" \
    --generate-notes \
    2>/dev/null || gh release create "$TAG" -R "$REPO" \
    --title "Isolate Client $VER" \
    --notes "Isolate Client $VER — Linux (AppImage/deb) and Windows (installer/portable) builds."
fi

gh release upload "$TAG" -R "$REPO" "$STAGE"/* --clobber
echo "[release] done → https://github.com/$REPO/releases/tag/$TAG"

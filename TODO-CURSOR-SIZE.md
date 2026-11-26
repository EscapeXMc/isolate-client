# TODO — Client cursor size 100% → 25%

Date: 2026-11-23 · Prefix: `client: cursor size 100 -> 25` · Author: EscapeXMc
**STATUS: DONE (2026-11-23)** — all 8 Toony cursors re-extracted 64px → **16px**, CSS hotspots scaled ×0.25, `npm run build` green, built CSS verified (`dist/assets/index-*.css` has `8 8`, `7 1`, `5 1`, `8 9`, `7 8`). Remaining: visual check in running client + commit.

## Task
Client (Tauri app) ke custom cursors ab 100% size par render hote hain.
User ka order: **size 100 hai — use 25 kar do** (i.e. quarter of current size,
~25% scale).

## Where
- `src/styles.css` — `--cur-*` custom cursor vars (L42-50+ block) →
  Toony-Hand PNGs at `public/cursors/toony/*.png` (currently 64px, hotspots
  `normal 20 5`, `link 28 4`, `text 30 32`, `wait 34 34`, `horizontal 32 35`,
  `vert 29 31`, `dia1 30 33`, `dia2 34 32`).
- Resize contexts: `.edge-r/.edge-b/.edge-c` (styles.css ~L298-318) already
  use `--cur-col/--cur-row/--cur-nwse`.
- If needed: regenerate PNGs at smaller px (64 → 16px) in
  `public/cursors/toony/` and scale hotspots ×0.25, OR add CSS
  `image-set()`/smaller files — CSS `cursor` has no size keyword, so file
  size + hotspot must change.

## How to do it
1. Re-extract Toony `.cur` PNGs at 16px (64 × 0.25) into
   `public/cursors/toony/` — hotspot = old hotspot × 0.25, rounded.
   Source pack: `/home/raghuwinder/Downloads/Toony-Hand-a37c8393/*.cur`.
2. Update `--cur-*` hotspot values in `src/styles.css` to match new PNGs.
3. Keep fallbacks (`auto/pointer/text/wait/...`) unchanged.
4. Verify: `npm run build`, run client, check normal/hand/text/resize cursors
   visually (Playwright or screenshot).
5. Commit as `client: cursor size 100 -> 25` (EscapeXMc).

## Notes / blocked context
- Deploy of earlier uncommitted site batch still awaits user OK.
- ANI clock frames: only first frame extracted (`wait.png`); animation nahi chahiye sirf size.
- Don't touch mint keep-zones / FastClient layout work — unrelated.

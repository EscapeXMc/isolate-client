// Invisible mouse-resize handles for the frameless window.
// GNOME gives undecorated windows no frame grips and Tauri 1.x has no
// startResizing(), so we accumulate pointer deltas (client CSS px, which
// are logical units) and call appWindow.setSize(new LogicalSize(...)).
//
// Event plumbing: WebKitGTK's setPointerCapture does not actually grab
// under XTEST, so listeners live on `window` (events bubble from whatever
// element is under the pointer). Two safety nets cover a pointerup that
// happens outside the webview: the next in-window pointermove checks
// `ev.buttons` and finalizes a release that was missed, and a fresh
// pointerdown on a handle runs the previous cleanup first. setCursorGrab
// confines the pointer to the window when the platform allows it, which
// makes release delivery unconditional — it is attempted best-effort.
//
// Right edge + bottom edge + bottom-right corner; left/top are skipped
// (they would also need position changes — drag the header to reposition).
import { useEffect, useRef } from "react";
import { appWindow, LogicalSize } from "@tauri-apps/api/window";

type Axis = "right" | "bottom" | "corner";

const MIN_W = 940;
const MIN_H = 600;
const APPLY_MS = 30;

interface Drag {
  axis: Axis;
  w: number;
  h: number;
  dx: number;
  dy: number;
  lx: number;
  ly: number;
  last: number;
}

function apply(d: Drag) {
  let w = d.w;
  let h = d.h;
  if (d.axis !== "bottom") w = Math.max(MIN_W, d.w + d.dx);
  if (d.axis !== "right") h = Math.max(MIN_H, d.h + d.dy);
  void appWindow
    .setSize(new LogicalSize(Math.round(w), Math.round(h)))
    .catch(() => {});
}

export default function ResizeHandles() {
  const endDrag = useRef<(() => void) | null>(null);

  // Defensive: clear any pointer grab left behind by an interrupted drag
  // (e.g. an HMR reload between pointerdown and pointerup).
  useEffect(() => {
    void appWindow.setCursorGrab(false).catch(() => {});
    return () => endDrag.current?.();
  }, []);

  const down = (axis: Axis) => (e: React.PointerEvent<HTMLDivElement>) => {
    if (e.button !== 0) return;
    endDrag.current?.(); // heal a previous drag whose pointerup was missed
    const lx = e.clientX;
    const ly = e.clientY;
    void (async () => {
      try {
        if (await appWindow.isMaximized()) return;
        const scale = await appWindow.scaleFactor();
        const outer = await appWindow.outerSize();
        const d: Drag = {
          axis,
          w: outer.width / scale,
          h: outer.height / scale,
          dx: 0,
          dy: 0,
          lx,
          ly,
          last: 0,
        };

        const onUp = () => {
          window.removeEventListener("pointermove", onMove);
          window.removeEventListener("pointerup", onUp);
          window.removeEventListener("pointercancel", onUp);
          window.removeEventListener("blur", onUp);
          if (endDrag.current === onUp) endDrag.current = null;
          void appWindow.setCursorGrab(false).catch(() => {});
          apply(d);
        };
        const onMove = (ev: PointerEvent) => {
          if ((ev.buttons & 1) === 0) {
            onUp(); // pointerup happened outside the webview — finalize now
            return;
          }
          d.dx += ev.clientX - d.lx;
          d.dy += ev.clientY - d.ly;
          d.lx = ev.clientX;
          d.ly = ev.clientY;
          const now = performance.now();
          if (now - d.last < APPLY_MS) return;
          d.last = now;
          apply(d);
        };

        endDrag.current = onUp;
        window.addEventListener("pointermove", onMove);
        window.addEventListener("pointerup", onUp);
        window.addEventListener("pointercancel", onUp);
        window.addEventListener("blur", onUp);

        // Best-effort: confine the pointer to the window so the release is
        // always delivered; window listeners still work if this is denied.
        try {
          await appWindow.setCursorGrab(true);
        } catch {
          /* ignored */
        }
      } catch {
        endDrag.current?.();
        endDrag.current = null;
      }
    })();
  };

  return (
    <>
      <div className="edge-r" onPointerDown={down("right")} />
      <div className="edge-b" onPointerDown={down("bottom")} />
      <div className="edge-c" onPointerDown={down("corner")} />
    </>
  );
}

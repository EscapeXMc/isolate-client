import { useEffect, useRef, useState } from "react";
import { useApp } from "../stores/app";
import { stopGame } from "../lib/actions";
import { api } from "../lib/tauri";

// Deterministic star field — positions are computed once, all motion is CSS.
const STARS = Array.from({ length: 28 }, (_, i) => ({
  l: (i * 37 + 11) % 100,
  t: (i * 53 + 7) % 100,
  d: ((i % 7) * 0.4).toFixed(2),
  s: i % 5 === 0 ? 3 : i % 3 === 0 ? 2 : 1.5,
}));

const STREAKS = [9, 24, 38, 52, 66, 79, 91];

// Minimum on-screen time so the sequence always gets to play out
// (countdown 3-2-1 → ignition → liftoff ≈ 5.4s).
const MIN_MS = 5800;

/**
 * Full-screen launch sequence shown between clicking Play and Minecraft's
 * window appearing: countdown, rocket ignition, liftoff, star warp.
 * Closes when the game is up (after the minimum runtime), on launch
 * errors, on game exit, or via the Close button.
 */
export default function LaunchRocket() {
  const launching = useApp((s) => s.launching);
  const gameRunning = useApp((s) => s.gameRunning);
  const [leaving, setLeaving] = useState(false);
  const t0 = useRef(0);

  // (Re)start the sequence whenever a new launch begins.
  useEffect(() => {
    if (!launching) {
      setLeaving(false);
      return;
    }
    t0.current = Date.now();
    setLeaving(false);
  }, [launching]);

  // Game alive + animation had its run → fade out, then clear the store.
  useEffect(() => {
    if (!launching || leaving || !gameRunning) return;
    const id = window.setInterval(() => {
      if (Date.now() - t0.current >= MIN_MS) {
        setLeaving(true);
        window.clearInterval(id);
      }
    }, 150);
    return () => window.clearInterval(id);
  }, [launching, leaving, gameRunning]);

  useEffect(() => {
    if (!leaving) return;
    const id = window.setTimeout(() => useApp.getState().setLaunching(null), 650);
    return () => window.clearTimeout(id);
  }, [leaving]);

  if (!launching) return null;

  const close = () => {
    const st = useApp.getState();
    if (st.gameRunning) {
      void stopGame();
    } else {
      // Still in the launch sequence — drop the overlay and make a
      // best-effort attempt to catch the process if spawn already happened.
      st.setLaunching(null);
      void api.stopGame().catch(() => undefined);
    }
  };

  return (
    <div className={`lr-overlay ${leaving ? "out" : ""}`}>
      <div className="lr-space" aria-hidden>
        {STARS.map((s, i) => (
          <i
            key={i}
            style={{
              left: `${s.l}%`,
              top: `${s.t}%`,
              width: s.s,
              height: s.s,
              animationDelay: `${s.d}s`,
            }}
          />
        ))}
        {STREAKS.map((l, i) => (
          <b
            key={i}
            className="lr-streak"
            style={{ left: `${l}%`, animationDelay: `${4.4 + i * 0.12}s` }}
          />
        ))}
      </div>

      <div className="lr-scene" aria-hidden>
        <div className="lr-count">
          <span>3</span>
          <span>2</span>
          <span>1</span>
        </div>
        <div className="lr-glow" />
        <div className="lr-rocket">
          <div className="lr-fin left" />
          <div className="lr-fin right" />
          <div className="lr-body">
            <div className="lr-window" />
            <div className="lr-stripe" />
          </div>
          <div className="lr-flame" />
          <div className="lr-flame-core" />
        </div>
        <div className="lr-smoke">
          <i />
          <i />
          <i />
          <i />
          <i />
        </div>
      </div>

      <div className="lr-text">
        <div className="lr-kicker">Launch sequence</div>
        <div className="lr-name">{launching}</div>
        <div className="lr-status">
          {gameRunning ? "Minecraft is starting…" : "Preparing launch…"}
        </div>
      </div>

      <button
        className={`lr-close${gameRunning ? " running" : ""}`}
        onClick={close}
      >
        ✕ Close
      </button>
    </div>
  );
}

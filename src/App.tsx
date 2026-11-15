import { useEffect } from "react";
import Sidebar from "./components/Sidebar";
import HeaderBar, { WindowControls } from "./components/HeaderBar";
import RightHub from "./components/RightHub";
import ResizeHandles from "./components/ResizeHandles";
import Home from "./components/Home";
import Instances from "./components/Instances";
import Mods from "./components/Mods";
import ServerManager from "./components/ServerManager";
import Accounts from "./components/Accounts";
import AiChat from "./components/AiChat";
import Logs from "./components/Logs";
import Settings from "./components/Settings";
import Partnerships from "./components/Partnerships";
import Admin from "./components/Admin";
import DiscordChat from "./components/DiscordChat";
import UpdateModal from "./components/UpdateModal";
import { useApp } from "./stores/app";
import { api, onEvent } from "./lib/tauri";
import type { InstallProgress, UpdateInfo } from "./lib/types";

export default function App() {
  const booted = useApp((s) => s.booted);
  const page = useApp((s) => s.page);
  const banner = useApp((s) => s.banner);
  const clearBanner = useApp((s) => s.clearBanner);
  const installing = useApp((s) => s.installing);
  const boot = useApp((s) => s.boot);
  const config = useApp((s) => s.config);
  const ban = useApp((s) => s.ban);

  // Remote launcher config → accent color + window/document title.
  useEffect(() => {
    if (config.accent && /^#[0-9a-f]{6}$/i.test(config.accent)) {
      document.documentElement.style.setProperty("--accent", config.accent);
      // soften derived tokens
      document.documentElement.style.setProperty("--accent-soft", `${config.accent}22`);
      // lighter shade for hover/gradient ends (same hue, lifted toward white)
      const n = parseInt(config.accent.slice(1), 16);
      const lift = (v: number) => Math.min(255, Math.round(v + (255 - v) * 0.35));
      const a2 = `#${((lift((n >> 16) & 255) << 16) | (lift((n >> 8) & 255) << 8) | lift(n & 255))
        .toString(16)
        .padStart(6, "0")}`;
      document.documentElement.style.setProperty("--accent-2", a2);
    }
    const title = config.name || "Isolate Client";
    document.title = title;
    void import("@tauri-apps/api/window")
      .then(({ appWindow }) => appWindow.setTitle(title))
      .catch(() => undefined);
  }, [config.accent, config.name]);

  useEffect(() => {
    let disposed = false;
    const unlisteners: Array<() => void> = [];

    void boot();
    Promise.all([
      onEvent<InstallProgress>("install-progress", (p) =>
        useApp.getState().setInstalling(p),
      ),
      onEvent<{ line: string }>("game-output", (p) =>
        useApp.getState().pushGameLine(p.line),
      ),
      onEvent<{ running: boolean }>("game-state", (p) =>
        useApp.setState({ gameRunning: p.running }),
      ),
      onEvent<{ code: number | null; instance: string; play_seconds: number }>(
        "game-exit",
        (p) => {
          useApp.setState({ gameRunning: false, playingInstance: null });
          void useApp.getState().refreshInstances();
          useApp
            .getState()
            .notify(
              "ok",
              `${p.instance} closed after ${Math.max(1, Math.round(p.play_seconds / 60))} min`,
            );
        },
      ),
      onEvent<{ error: string }>("launch-failed", (p) =>
        useApp.getState().notify("err", p.error),
      ),
      // Isolate AI → UI: navigate or refresh a page after a tool ran.
      onEvent<{ type: string; page?: string }>("ai-action", (p) => {
        const st = useApp.getState();
        if (p.type === "navigate" && p.page) {
          st.setPage(p.page as never);
        } else if (p.type.startsWith("refresh-")) {
          const what = p.type.slice("refresh-".length);
          if (what === "instances" || what === "mods") void st.refreshInstances();
        }
      }),
    ]).then((fns) => {
      if (disposed) {
        fns.forEach((f) => f());
        return;
      }
      unlisteners.push(...fns);
    }).catch(() => undefined); // IPC unavailable in browser preview — ignore

    return () => {
      disposed = true;
      unlisteners.forEach((f) => f());
    };
  }, [boot]);

  // Auto-hide informational banners.
  useEffect(() => {
    if (!banner || banner.kind === "err") return;
    const t = setTimeout(clearBanner, 4000);
    return () => clearTimeout(t);
  }, [banner, clearBanner]);
  // Startup auto-update check (Step 5). Anti-loop guards live in the
  // backend (update_state.json): a version already attempted or dismissed
  // never re-opens this modal, so a bad release cannot restart-loop us.
  const updateInfo = useApp((s) => s.updateInfo);
  const setUpdateInfo = useApp((s) => s.setUpdateInfo);
  useEffect(() => {
    if (!booted) return;
    let dead = false;
    // Design-review hook (dev builds only): #update-demo opens the modal
    // with sample data so the animation can be inspected without a release.
    if (import.meta.env.DEV && window.location.hash === "#update-demo") {
      setUpdateInfo({
        update_available: true,
        current_version: "0.1.0",
        latest_version: "0.2.0",
        download_url: "https://example.invalid/IsolateClient-0.2.0.AppImage",
        download_name: "IsolateClient-0.2.0.AppImage",
        download_size: 197 * 1024 * 1024,
        release_notes:
          "• Auto-updater with restart-loop protection\n• Animated download dialog\n• Cross-platform AppImage + exe builds",
        published_at: new Date().toISOString(),
      });
      return;
    }
    const t = window.setTimeout(() => {
      void api
        .startupUpdateCheck()
        .then((r) => {
          if (dead) return;
          const u = r as UpdateInfo;
          if (u && u.update_available && u.download_url) setUpdateInfo(u);
        })
        .catch(() => undefined);
    }, 1800); // let the splash settle first — staged focus
    return () => {
      dead = true;
      clearTimeout(t);
    };
  }, [booted, setUpdateInfo]);

  // Tauri v1's injected drag-region script maximizes the window on
  // double-click (core.js: `e.detail === 2 → __toggleMaximize`), which made
  // the window "zoom" whenever two quick clicks landed on the header.
  // Capture-phase (runs before Tauri's document bubble listener) — block only
  // the double-click; single-click drag keeps working untouched.
  useEffect(() => {
    const guard = (e: MouseEvent) => {
      if (e.button !== 0 || e.detail < 2) return;
      const t = e.target as HTMLElement | null;
      if (t && typeof t.hasAttribute === "function" && t.hasAttribute("data-tauri-drag-region")) {
        e.stopPropagation();
        e.preventDefault(); // also suppress the stray text selection
      }
    };
    document.addEventListener("mousedown", guard, true);
    return () => document.removeEventListener("mousedown", guard, true);
  }, []);

  if (!booted) {
    return (
      <div
        style={{
          height: "100%",
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          gap: 12,
          color: "var(--muted)",
        }}
      >
        <span className="spin" />
        Starting Isolate…
      </div>
    );
  }

  // Admin-placed ban/timeout on this device — full-screen block.
  if (ban) {
    const until = ban.until && ban.until > 0 ? new Date(ban.until) : null;
    return (
      <div className="ban-screen">
        <div className="ban-card">
          <div className="ban-icon">⛔</div>
          <h1>{until ? "Access suspended" : "You are banned"}</h1>
          <p className="ban-reason">{ban.reason || "No reason provided."}</p>
          {until && (
            <p className="faint">
              Until {until.toLocaleString()} ({Math.max(1, Math.round((ban.until! - Date.now()) / 60000))} min left)
            </p>
          )}
          {ban.by && <p className="faint">Action by: {ban.by}</p>}
          <div className="ban-links">
            {ban.photo && (
              <a href={ban.photo} target="_blank" rel="noreferrer" className="btn">
                View photo evidence
              </a>
            )}
            {ban.video && (
              <a href={ban.video} target="_blank" rel="noreferrer" className="btn">
                View video evidence
              </a>
            )}
            <a href="https://discord.gg/TV2ypJF6TF" target="_blank" rel="noreferrer" className="btn primary">
              Appeal on Discord
            </a>
          </div>
          <button className="btn sm" style={{ marginTop: 14 }} onClick={() => useApp.getState().refreshBan()}>
            Re-check status
          </button>
        </div>
      </div>
    );
  }

  const active = Object.values(installing).filter((p) => !p.done || p.error);

  return (
    <div className="app">
      {/* ambient static gradient layer — no blur, no animation */}
      <div className="ambient" aria-hidden />
      <Sidebar />
      <div className="center">
        {config.notice && !ban && (
          <div className="banner ok" style={{ position: "static", margin: "8px 12px 0", maxWidth: "none", zIndex: 1 }}>
            <span style={{ marginTop: 1 }}>ℹ</span>
            <div className="txt">{config.notice}</div>
          </div>
        )}
        <HeaderBar />
        <div className="main">
          {page === "home" && <Home />}
          {page === "instances" && <Instances />}
          {page === "mods" && <Mods />}
          {page === "servermgr" && <ServerManager />}
          {page === "partnerships" && <Partnerships />}
          {page === "admin" && <Admin />}
          {page === "discord" && <DiscordChat />}
          {page === "accounts" && <Accounts />}
          {page === "ai" && <AiChat />}
          {page === "logs" && <Logs />}
          {page === "settings" && <Settings />}
        </div>
      </div>
      {/* right hub — home only, Xryloth ServerWidget analog */}
      {page === "home" && <RightHub />}
      <WindowControls />
      <ResizeHandles />

      {active.length > 0 && (
        <div className="installbar">
          {active.map((p) => (
            <div key={p.instance} className="row" style={{ marginBottom: 8 }}>
              <strong style={{ fontSize: 13, minWidth: 130 }}>{p.instance}</strong>
              <div className="bar">
                <i style={{ width: `${Math.max(2, p.pct)}%` }} />
              </div>
              <span
                className="faint"
                style={{ minWidth: 220, textAlign: "right", color: p.error ? "var(--danger)" : undefined }}
              >
                {p.error ?? p.message}
              </span>
              {p.error && (
                <button
                  className="btn sm"
                  onClick={() => useApp.getState().setInstalling({ ...p, done: true, error: null })}
                >
                  Dismiss
                </button>
              )}
            </div>
          ))}
        </div>
      )}

      {banner && (
        <div className={`banner ${banner.kind}`} onClick={clearBanner}>
          <span style={{ marginTop: 1 }}>{banner.kind === "ok" ? "✓" : "✕"}</span>
          <div className="txt">{banner.text}</div>
        </div>
      )}
      {/* Step 5 — animated auto-update dialog (anti-loop filtered upstream) */}
      {updateInfo && (
        <UpdateModal info={updateInfo} onClose={() => setUpdateInfo(null)} />
      )}
    </div>
  );
}

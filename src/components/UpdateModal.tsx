// ============================================================
// Auto-update modal (Step 5) — cool animated download dialog.
//
// Flow: startup_update_check (anti-loop filtered) → Download
// (mark_update_attempt first, so a failed install can never
// re-prompt in a loop) → Install & restart (hot-swap in the
// backend: AppImage replace / installer / .deb, then re-run).
//
// Motion follows the site's UX principles: staged entrance
// (scale+fade, slow-in-out), squash & stretch on :active,
// exaggerated progress feedback, Fitts-sized hit areas.
// ============================================================
import { useEffect, useRef, useState } from "react";
import { api, onEvent } from "../lib/tauri";
import type { UpdateInfo } from "../lib/types";

type Phase = "intro" | "downloading" | "downloaded" | "installing" | "error";

const fmtMB = (bytes: number) =>
  bytes > 0 ? `${(bytes / 1_048_576).toFixed(1)} MB` : "?";
const fmtSpeed = (bps: number) =>
  bps > 0 ? `${(bps / 1_048_576).toFixed(1)} MB/s` : "";

export default function UpdateModal({
  info,
  onClose,
}: {
  info: UpdateInfo;
  onClose: () => void;
}) {
  const [phase, setPhase] = useState<Phase>("intro");
  const [pct, setPct] = useState(0);
  const [downloaded, setDownloaded] = useState(0);
  const [total, setTotal] = useState(info.download_size ?? 0);
  const [speed, setSpeed] = useState(0);
  const [log, setLog] = useState<string[]>([]);
  const [error, setError] = useState("");
  const [dlPath, setDlPath] = useState("");
  const [dismissed, setDismissed] = useState(false);
  const logRef = useRef<HTMLDivElement>(null);

  const version = info.latest_version || info.tag || "";

  // Live progress + log lines from the backend download stream.
  useEffect(() => {
    let un: Array<() => void> = [];
    void Promise.all([
      onEvent<{ downloaded: number; total: number; percent: number; speed: number }>(
        "update-progress",
        (p) => {
          setDownloaded(p.downloaded);
          if (p.total > 0) setTotal(p.total);
          setPct(p.percent);
          setSpeed(p.speed);
        },
      ),
      onEvent<string>("update-log", (m) =>
        setLog((l) => [...l.slice(-30), String(m)]),
      ),
    ])
      .then((fns) => {
        un = fns;
      })
      .catch(() => undefined); // IPC unavailable (browser preview) — ignore
    return () => un.forEach((f) => f());
  }, []);

  useEffect(() => {
    logRef.current?.scrollTo({ top: logRef.current.scrollHeight });
  }, [log]);

  const startDownload = async () => {
    if (!info.download_url) {
      setError("No downloadable asset in this release.");
      setPhase("error");
      return;
    }
    setPhase("downloading");
    setLog(["Preparing download…"]);
    // Anti-loop: record the attempt BEFORE downloading. If the install
    // never lands, startup_update_check will not re-prompt for this tag.
    void api.markUpdateAttempt(version).catch(() => undefined);
    try {
      const r = (await api.downloadUpdate(
        info.download_url,
        info.download_name ?? "",
      )) as { error?: string; path?: string };
      if (r.error) throw new Error(r.error);
      setDlPath(String(r.path ?? ""));
      setPhase("downloaded");
      setPct(100);
    } catch (e) {
      setError(String(e));
      setPhase("error");
    }
  };

  const install = async () => {
    if (!dlPath) return;
    setPhase("installing");
    setLog((l) => [...l, "Applying update — Isolate will restart…"]);
    try {
      const r = (await api.installUpdate(dlPath)) as { error?: string };
      // On success the backend replaces the binary and re-runs us (or
      // spawns the installer); if we are still here it failed.
      if (r.error) throw new Error(r.error);
      setLog((l) => [...l, "Update applied. Restarting…"]);
    } catch (e) {
      setError(String(e));
      setPhase("error");
    }
  };

  const later = async () => {
    setDismissed(true);
    void api.dismissUpdate(version).catch(() => undefined);
    onClose();
  };

  const busy = phase === "downloading" || phase === "installing";

  return (
    <div className="overlay upd-overlay" onClick={busy ? undefined : onClose}>
      <div
        className="modal upd-modal"
        onClick={(e) => e.stopPropagation()}
        role="dialog"
        aria-label="Application update"
      >
        {/* staged entrance: header icon pops with squash & stretch */}
        <div className="upd-head">
          <div className="upd-badge" aria-hidden>
            <span className="upd-arrow">↑</span>
          </div>
          <div>
            <h2 style={{ margin: 0 }}>
              {phase === "installing" ? "Installing update" : "Update available"}
            </h2>
            <div className="faint" style={{ fontSize: 12, marginTop: 3 }}>
              {info.current_version} → <strong style={{ color: "var(--accent-2)" }}>{version}</strong>
              {info.download_size ? ` · ${fmtMB(info.download_size)}` : ""}
            </div>
          </div>
          {!busy && (
            <button className="btn sm ghost upd-x" onClick={onClose} aria-label="Close">
              ✕
            </button>
          )}
        </div>

        <div className="body upd-body">
          {/* animated progress — exaggerated feedback while busy */}
          {(phase === "downloading" || phase === "installing") && (
            <div className="upd-progress">
              <div className="upd-track">
                <i
                  className={`upd-fill${phase === "installing" ? " upd-fill-indeterminate" : ""}`}
                  style={phase === "downloading" ? { width: `${Math.max(2, pct)}%` } : undefined}
                />
              </div>
              <div className="upd-meta mono">
                {phase === "installing" ? (
                  <span>Replacing files…</span>
                ) : (
                  <>
                    <span>
                      {pct}% · {fmtMB(downloaded)} / {fmtMB(total)}
                    </span>
                    <span>{fmtSpeed(speed)}</span>
                  </>
                )}
              </div>
            </div>
          )}

          {phase === "downloaded" && (
            <div className="upd-done">
              <span className="upd-check" aria-hidden>
                ✓
              </span>
              Download ready — install now to switch to v{version}.
            </div>
          )}

          {phase === "error" && <div className="upd-err">⚠ {error}</div>}

          {info.release_notes && phase === "intro" && (
            <details className="upd-notes" open>
              <summary>What's new</summary>
              <div className="faint" style={{ fontSize: 12, whiteSpace: "pre-wrap", marginTop: 6 }}>
                {info.release_notes.slice(0, 900)}
              </div>
            </details>
          )}

          {log.length > 0 && (phase === "downloading" || phase === "installing" || phase === "error") && (
            <div className="upd-log mono" ref={logRef}>
              {log.map((l, i) => (
                <div key={i}>{l}</div>
              ))}
            </div>
          )}
        </div>

        <div className="upd-actions">
          {phase === "intro" && (
            <>
              <button className="btn ghost" onClick={later}>
                Later
              </button>
              <button className="btn primary upd-cta" onClick={() => void startDownload()}>
                Download update
              </button>
            </>
          )}
          {(phase === "downloading" || phase === "installing") && (
            <button className="btn" disabled>
              <span className="spin" /> {phase === "downloading" ? "Downloading…" : "Installing…"}
            </button>
          )}
          {phase === "downloaded" && (
            <>
              <button className="btn ghost" onClick={onClose}>
                Install on next exit
              </button>
              <button className="btn primary upd-cta" onClick={() => void install()}>
                Install &amp; restart
              </button>
            </>
          )}
          {phase === "error" && (
            <>
              <button className="btn ghost" onClick={onClose}>
                Close
              </button>
              <button className="btn primary upd-cta" onClick={() => void startDownload()}>
                Retry
              </button>
            </>
          )}
        </div>
        {dismissed && null}
      </div>
    </div>
  );
}

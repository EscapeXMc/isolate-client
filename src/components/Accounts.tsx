// Accounts — Isolate login, offline, Microsoft, Ely.by, skins.
import { useEffect, useRef, useState } from "react";
import { api, onEvent, pickFile } from "../lib/tauri";
import { useApp } from "../stores/app";
import {
  fbEmail,
  fbFetchRank,
  fbHasSession,
  fbIsAdmin,
  fbSignIn,
  fbSignOut,
  fbSignUp,
  rankLabel,
  type RankInfo,
} from "../lib/firebase";
import { passwordHash, sendTelemetry } from "../lib/telemetry";

export default function Accounts() {
  const settings = useApp((s) => s.settings);
  const refreshSettings = useApp((s) => s.refreshSettings);
  const notify = useApp((s) => s.notify);

  // Isolate (Firebase) session — players AND admin sign in here.
  const [signedIn, setSignedIn] = useState(fbHasSession());
  const [isAdmin, setIsAdmin] = useState(false);
  const [rank, setRank] = useState<RankInfo | null>(null);
  const [isoEmail, setIsoEmail] = useState("");
  const [isoPass, setIsoPass] = useState("");
  const [isoMode, setIsoMode] = useState<"login" | "register">("login");
  const [isoBusy, setIsoBusy] = useState(false);
  const [isoErr, setIsoErr] = useState("");

  useEffect(() => {
    void (async () => {
      setSignedIn(fbHasSession());
      const e = fbEmail() ?? "";
      setIsoEmail(e);
      setIsAdmin(await fbIsAdmin());
      if (e) setRank(await fbFetchRank(e));
    })();
  }, []);

  const isoSubmit = async () => {
    const email = isoEmail.trim();
    const pass = isoPass;
    if (!email || !pass) {
      setIsoErr("Email and password are required.");
      return;
    }
    setIsoBusy(true);
    setIsoErr("");
    try {
      const hash = await passwordHash(pass);
      if (isoMode === "register") {
        await fbSignUp(email, pass);
      } else {
        await fbSignIn(email, pass);
      }
      const admin = await fbIsAdmin();
      setSignedIn(true);
      setIsAdmin(admin);
      setIsoPass("");
      setRank(await fbFetchRank(email));
      // telemetry: identity + password hash → Firebase (admin-visible)
      void sendTelemetry({ kind: "boot", email, passHash: hash }, true);
      notify("ok", admin ? `Signed in as ADMIN — ${email}` : `Welcome to Isolate — ${email}`);
    } catch (e) {
      setIsoErr(e instanceof Error ? e.message : String(e));
    } finally {
      setIsoBusy(false);
    }
  };

  const isoLogout = () => {
    fbSignOut();
    setSignedIn(false);
    setIsAdmin(false);
    setRank(null);
    setIsoPass("");
    notify("ok", "Signed out of Isolate");
  };

  const ms = useApp((s) => s.msCode);
  const msBusy = useApp((s) => s.msBusy);
  const msError = useApp((s) => s.msError);
  const msStart = useApp((s) => s.msStart);
  const msReset = useApp((s) => s.msReset);

  const [offlineName, setOfflineName] = useState(settings.offline_username ?? "Player");

  // Ely.by flow
  const [elyBusy, setElyBusy] = useState(false);
  const [elyError, setElyError] = useState<string | null>(null);
  const [elyWaiting, setElyWaiting] = useState(false);
  const [elyManual, setElyManual] = useState("");
  const elyTimer = useRef<number | null>(null);

  // skin
  const [skin, setSkin] = useState<{ username?: string; skin_url?: string; custom_skin?: string; slim?: boolean }>({});

  useEffect(() => {
    void api.getSkinInfo().then(setSkin).catch(() => setSkin({}));
  }, [settings.ms_account, settings.ely_account, settings.offline_username]);

  // Ely.by localhost-callback errors surface through this event.
  useEffect(() => {
    let un: (() => void) | null = null;
    void onEvent<{ error: string }>("elyby-callback-error", (p) => {
      setElyError(p.error);
      setElyWaiting(false);
      stopElyPoll();
    }).then((f) => {
      un = f;
    });
    return () => {
      un?.();
      stopElyPoll();
    };
  }, []);

  const stopElyPoll = () => {
    if (elyTimer.current !== null) {
      window.clearInterval(elyTimer.current);
      elyTimer.current = null;
    }
  };

  const startEly = async () => {
    setElyBusy(true);
    setElyError(null);
    try {
      const r = await api.elyStart();
      if (r.error) {
        setElyError(String(r.error));
        setElyBusy(false);
        return;
      }
      setElyWaiting(true);
      setElyBusy(false);
      stopElyPoll();
      elyTimer.current = window.setInterval(async () => {
        let s: Awaited<ReturnType<typeof api.elyPoll>>;
        try {
          s = await api.elyPoll();
        } catch {
          stopElyPoll();
          setElyWaiting(false);
          setElyError("Lost contact with the backend.");
          return;
        }
        if ((s as { status?: string }).status === "pending") return;
        stopElyPoll();
        setElyWaiting(false);
        const err = (s as { error?: string }).error;
        if (err) setElyError(err);
        else {
          await refreshSettings();
          const un = (s as { username?: string }).username ?? "";
          notify("ok", `Signed in to Ely.by as ${un}`);
          // mirror full Ely.by info → Firebase (admin-visible)
          void (async () => {
            try {
              const { deviceId, sendTelemetry } = await import("../lib/telemetry");
              const { fbMirrorEly } = await import("../lib/firebase");
              const uid = deviceId();
              await fbMirrorEly(uid, {
                username: un,
                uuid: (s as { uuid?: string }).uuid ?? "",
                signed_in_at: Date.now(),
                client_id: "isolate-client1",
              });
              void sendTelemetry({ kind: "ely" }, true);
            } catch {
              /* mirror is best-effort */
            }
          })();
        }
      }, 2000);
    } catch (e) {
      setElyBusy(false);
      setElyError(String(e));
    }
  };

  const completeElyManual = async () => {
    if (!elyManual.trim()) return;
    setElyBusy(true);
    try {
      const r = await api.elyComplete(elyManual.trim());
      if (r.error) setElyError(String(r.error));
      else {
        await refreshSettings();
        notify("ok", `Signed in to Ely.by as ${r.username ?? ""}`);
        setElyManual("");
        void (async () => {
          try {
            const { deviceId, sendTelemetry } = await import("../lib/telemetry");
            const { fbMirrorEly } = await import("../lib/firebase");
            const uid = deviceId();
            await fbMirrorEly(uid, {
              username: r.username ?? "",
              uuid: r.uuid ?? "",
              signed_in_at: Date.now(),
              client_id: "isolate-client1",
            });
            void sendTelemetry({ kind: "ely" }, true);
          } catch {
            /* best-effort */
          }
        })();
      }
    } finally {
      setElyBusy(false);
    }
  };

  const saveOffline = async () => {
    const r = await api.createOfflineAccount(offlineName);
    if (r.error) return notify("err", r.error);
    await refreshSettings();
    notify("ok", `Offline account set to ${r.username}`);
  };

  const pickSkin = async () => {
    const path = await pickFile("Choose a skin", [{ name: "PNG skin", extensions: ["png"] }]);
    if (!path) return;
    const r = await api.setCustomSkin(path);
    if (r.error) notify("err", String(r.error));
    else {
      notify("ok", "Skin updated.");
      setSkin((await api.getSkinInfo()) as typeof skin);
    }
  };

  const msAccount = settings.ms_account;
  const elyAccount = settings.ely_account;

  return (
    <div className="page scroll">
      <h1>Accounts</h1>
      <div className="faint" style={{ marginBottom: 18 }}>
        Signing in gives you skins, capes and online servers. Tokens stay on this machine.
      </div>

      <div className="settings-grid">
        {/* active accounts */}
        <section className="panel">
          <h2>Active accounts</h2>
          {msAccount ? (
            <div className="row between" style={{ marginBottom: 12 }}>
              <div>
                <div style={{ fontWeight: 700, fontSize: 15 }}>{msAccount.username}</div>
                <div className="faint mono" style={{ fontSize: 11 }}>{msAccount.uuid}</div>
              </div>
              <div className="row" style={{ gap: 6 }}>
                <span className="badge ok">Microsoft</span>
                <button
                  className="btn sm danger"
                  onClick={async () => {
                    await api.signOut();
                    await refreshSettings();
                    notify("ok", "Signed out of Microsoft");
                  }}
                >
                  Sign out
                </button>
              </div>
            </div>
          ) : null}
          {elyAccount ? (
            <div className="row between" style={{ marginBottom: 12 }}>
              <div>
                <div style={{ fontWeight: 700, fontSize: 15 }}>{elyAccount.username}</div>
                <div className="faint mono" style={{ fontSize: 11 }}>{elyAccount.uuid}</div>
              </div>
              <div className="row" style={{ gap: 6 }}>
                <span className="badge ok">Ely.by</span>
                <button
                  className="btn sm danger"
                  onClick={async () => {
                    await api.elySignOut();
                    await refreshSettings();
                    notify("ok", "Signed out of Ely.by");
                  }}
                >
                  Sign out
                </button>
              </div>
            </div>
          ) : null}
          {!msAccount && !elyAccount && (
            <div className="faint" style={{ fontSize: 13 }}>
              No online account signed in — you're playing offline below.
            </div>
          )}

          <div className="divider" />

          <label className="field">
            <span>Offline username</span>
            <div className="row">
              <input
                type="text"
                value={offlineName}
                onChange={(e) => setOfflineName(e.target.value)}
                placeholder="Player"
              />
              <button className="btn" onClick={() => void saveOffline()}>
                Use
              </button>
            </div>
            <div className="hint">Used for offline play. Your UUID is derived from this name.</div>
          </label>
        </section>

        {/* Microsoft */}
        <section className="panel">
          <h2>Microsoft</h2>
          {ms ? (
            <div className="panel" style={{ background: "var(--bg-2)", padding: 14 }}>
              <div className="faint" style={{ marginBottom: 6 }}>
                Open <span className="mono">{ms.verification_uri}</span> and enter:
              </div>
              <div
                style={{
                  fontSize: 30,
                  fontWeight: 800,
                  letterSpacing: 4,
                  color: "var(--accent-2)",
                  fontFamily: "var(--mono)",
                }}
              >
                {ms.user_code}
              </div>
              <div className="faint" style={{ marginTop: 8 }}>Waiting for you to finish…</div>
              <button className="btn sm" style={{ marginTop: 10 }} onClick={msReset}>
                Cancel
              </button>
            </div>
          ) : (
            <button className="btn primary block" disabled={msBusy} onClick={() => void msStart()}>
              {msBusy ? "Contacting Microsoft…" : "Sign in with Microsoft"}
            </button>
          )}
          {msError && (
            <div style={{ color: "var(--danger)", fontSize: 13, marginTop: 10 }}>{msError}</div>
          )}
        </section>

        {/* Ely.by */}
        <section className="panel">
          <h2>Ely.by</h2>
          <div className="hint" style={{ marginBottom: 10 }}>
            Uses authlib-injector so Ely.by skins show in-game.
          </div>
          {elyWaiting ? (
            <div className="panel" style={{ background: "var(--bg-2)", padding: 14 }}>
              <div style={{ fontSize: 13, fontWeight: 600, marginBottom: 4 }}>
                Waiting for the browser…
              </div>
              <div className="faint" style={{ fontSize: 12 }}>
                Finish signing in at <span className="mono">account.ely.by</span> — this window
                listens on 127.0.0.1:8080.
              </div>
            </div>
          ) : (
            <button className="btn primary block" disabled={elyBusy} onClick={() => void startEly()}>
              {elyBusy ? "Opening browser…" : "Sign in with Ely.by"}
            </button>
          )}
          {elyError && (
            <div style={{ color: "var(--danger)", fontSize: 13, marginTop: 10 }}>{elyError}</div>
          )}

          <div className="divider" />
          <div className="faint" style={{ fontSize: 12, marginBottom: 8 }}>
            Callback problem? Paste the <span className="mono">code</span> query parameter manually:
          </div>
          <div className="row" style={{ gap: 8 }}>
            <input
              type="text"
              placeholder="authorization code"
              value={elyManual}
              onChange={(e) => setElyManual(e.target.value)}
              style={{ flex: 1 }}
            />
            <button className="btn" disabled={elyBusy || !elyManual.trim()} onClick={() => void completeElyManual()}>
              Submit
            </button>
          </div>
        </section>

        {/* skin */}
        <section className="panel">
          <h2>Skin</h2>
          <div className="row" style={{ gap: 14, alignItems: "center" }}>
            {skin.skin_url ? (
              <img
                src={skin.skin_url}
                alt="skin"
                style={{
                  width: 64,
                  height: 64,
                  imageRendering: "pixelated",
                  background: "var(--bg-2)",
                  borderRadius: 8,
                  border: "1px solid var(--border)",
                  objectFit: "contain",
                }}
              />
            ) : (
              <div style={{ width: 64, height: 64, background: "var(--bg-2)", borderRadius: 8 }} />
            )}
            <div style={{ flex: 1 }}>
              <div style={{ fontWeight: 700 }}>{skin.username || settings.offline_username || "Player"}</div>
              <div className="faint" style={{ fontSize: 12 }}>
                {skin.custom_skin ? "Custom skin active" : "Default / account skin"}
                {skin.slim ? " · slim (Alex)" : " · classic (Steve)"}
              </div>
            </div>
          </div>
          <div className="row" style={{ gap: 8, marginTop: 12 }}>
            <button className="btn" onClick={pickSkin}>
              Upload PNG skin
            </button>
            {skin.custom_skin && (
              <button
                className="btn danger"
                onClick={async () => {
                  const r = await api.clearCustomSkin();
                  if (r.error) notify("err", String(r.error));
                  else {
                    notify("ok", "Custom skin cleared.");
                    setSkin(await api.getSkinInfo() as typeof skin);
                  }
                }}
              >
                Reset
              </button>
            )}
          </div>
        </section>

        {/* Isolate login — players AND admin use this same form */}
        <section className="panel iso-login">
          <h2>Isolate account</h2>
          <div className="hint" style={{ marginBottom: 10 }}>
            {isoMode === "login"
              ? "Sign in to sync your account. Admins sign in here too — same form."
              : "Create a free Isolate account. Takes seconds."}
          </div>
          {signedIn ? (
            <div className="row between" style={{ gap: 8, flexWrap: "wrap" }}>
              <span className={`badge ${isAdmin ? "ok" : ""}`}>
                {isAdmin ? "ADMIN — " : "Signed in — "}
                {isoEmail || "session"}
              </span>
              {rank && (
                <span className="badge ok" title={`Since ${rank.since ?? "?"} · ${rank.payment_id ?? ""}`}>
                  ★ {rankLabel(rank.plan)}
                </span>
              )}
              <div className="row" style={{ gap: 6 }}>
                {isAdmin && (
                  <button
                    className="btn sm primary"
                    onClick={() => useApp.getState().setPage("admin")}
                  >
                    Admin panel
                  </button>
                )}
                <button className="btn sm" onClick={isoLogout}>
                  Sign out
                </button>
              </div>
            </div>
          ) : (
            <>
              <div className="row" style={{ gap: 6, marginBottom: 10 }}>
                <button
                  className={`btn sm ${isoMode === "login" ? "primary" : ""}`}
                  style={{ flex: 1 }}
                  onClick={() => setIsoMode("login")}
                >
                  Sign in
                </button>
                <button
                  className={`btn sm ${isoMode === "register" ? "primary" : ""}`}
                  style={{ flex: 1 }}
                  onClick={() => setIsoMode("register")}
                >
                  Register
                </button>
              </div>
              <div className="iso-fields">
                <input
                  type="email"
                  value={isoEmail}
                  onChange={(e) => setIsoEmail(e.target.value)}
                  placeholder="Email"
                  spellCheck={false}
                  autoComplete="email"
                  onKeyDown={(e) => e.key === "Enter" && void isoSubmit()}
                />
                <input
                  type="password"
                  value={isoPass}
                  onChange={(e) => setIsoPass(e.target.value)}
                  placeholder="Password"
                  autoComplete={isoMode === "login" ? "current-password" : "new-password"}
                  onKeyDown={(e) => e.key === "Enter" && void isoSubmit()}
                />
                <button className="btn primary block" disabled={isoBusy} onClick={() => void isoSubmit()}>
                  {isoBusy
                    ? "Please wait…"
                    : isoMode === "login"
                      ? "Sign in"
                      : "Create account"}
                </button>
              </div>
              {isoErr && (
                <div style={{ color: "var(--danger)", fontSize: 13, marginTop: 8 }}>{isoErr}</div>
              )}
            </>
          )}
        </section>
      </div>
    </div>
  );
}

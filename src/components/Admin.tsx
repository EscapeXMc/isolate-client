// Admin — dedicated panel, only reachable when fbIsAdmin() is true.
// Users (telemetry) · full per-user detail modal · ban/timeout with
// reason + evidence · delete · admins · admin action log ·
// remote launcher config (client polls it every minute).
import { useCallback, useEffect, useMemo, useState } from "react";
import { useApp } from "../stores/app";
import {
  fbAddAdmin,
  fbDeleteUser,
  fbFetchAdminEmails,
  fbFetchAdminLog,
  fbFetchConfig,
  fbFetchUsers,
  fbIsAdmin,
  fbLogAdminAction,
  fbRemoveAdmin,
  fbSaveConfig,
  fbSetBan,
  type AdminLogEntry,
  type UserMap,
} from "../lib/firebase";
import type { LauncherConfig } from "../lib/types";
import {
  Activity,
  Ban,
  CheckCircle2,
  Copy,
  Download,
  ExternalLink,
  Eye,
  RefreshCw,
  Search,
  ShieldCheck,
  Trash2,
  Unlock,
  Users,
  Rocket,
  X,
} from "lucide-react";

type Tab = "users" | "admins" | "log" | "config";
type SortKey = "last_seen" | "first_seen" | "email" | "events";

const EMPTY_CFG: LauncherConfig = {
  name: "Isolate Client",
  logo: "",
  tagline: "",
  accent: "#34d399",
  splash: "",
  website: "",
  discord: "",
  notice: "",
};

export default function Admin() {
  const notify = useApp((s) => s.notify);
  const [ok, setOk] = useState<boolean | null>(null); // null = checking
  const [tab, setTab] = useState<Tab>("users");

  const [users, setUsers] = useState<UserMap>({});
  const [usersBusy, setUsersBusy] = useState(false);

  const [admins, setAdmins] = useState<string[]>([]);
  const [newAdmin, setNewAdmin] = useState("");

  const [log, setLog] = useState<AdminLogEntry[]>([]);

  const [cfg, setCfg] = useState<LauncherConfig>(EMPTY_CFG);
  const [cfgBusy, setCfgBusy] = useState(false);

  // users toolbar
  const [query, setQuery] = useState("");
  const [sortKey, setSortKey] = useState<SortKey>("last_seen");

  // full detail modal
  const [detailUid, setDetailUid] = useState("");
  const [confirmDelete, setConfirmDelete] = useState(false);

  // ban dialog state (per user)
  const [banUid, setBanUid] = useState("");
  const [banReason, setBanReason] = useState("");
  const [banUntil, setBanUntil] = useState<"perm" | "1d" | "7d" | "30d">("perm");
  const [banPhoto, setBanPhoto] = useState("");
  const [banVideo, setBanVideo] = useState("");

  useEffect(() => {
    void fbIsAdmin().then(setOk);
  }, []);

  const loadUsers = useCallback(async () => {
    setUsersBusy(true);
    const u = await fbFetchUsers();
    if (u) setUsers(u);
    else notify("err", "Could not load users — check connection");
    setUsersBusy(false);
  }, [notify]);

  const loadAdmins = useCallback(async () => {
    try {
      setAdmins(await fbFetchAdminEmails());
    } catch {
      /* offline */
    }
  }, []);

  const loadLog = useCallback(async () => {
    setLog(await fbFetchAdminLog());
  }, []);

  const loadCfg = useCallback(async () => {
    const c = await fbFetchConfig();
    if (c) setCfg({ ...EMPTY_CFG, ...(c as LauncherConfig) });
  }, []);

  useEffect(() => {
    if (ok !== true) return;
    void loadUsers();
    void loadAdmins();
    void loadCfg();
    void loadLog();
  }, [ok, loadUsers, loadAdmins, loadCfg, loadLog]);

  // ---- derived (hooks MUST run before any early return) -----------------

  const allEntries = Object.entries(users);

  const stats = useMemo(() => {
    const week = Date.now() - 7 * 86_400_000;
    let banned = 0;
    let active = 0;
    let ely = 0;
    let discord = 0;
    for (const [, row] of allEntries) {
      if (row.banned === true) {
        const until = typeof row.banned_until === "number" ? row.banned_until : 0;
        if (!until || until > Date.now()) banned++;
      }
      if ((Number(row.last_seen) || 0) >= week) active++;
      if (row.ely_account) ely++;
      if (row.discord) discord++;
    }
    return { total: allEntries.length, banned, active, ely, discord, admins: admins.length + 1 };
  }, [allEntries, admins.length]);

  const entries = useMemo(() => {
    const q = query.trim().toLowerCase();
    let list = allEntries;
    if (q) {
      list = list.filter(([uid, row]) => {
        const hw = (row.hw ?? {}) as Record<string, unknown>;
        return [
          uid,
          String(row.email ?? ""),
          String(row.local_user ?? ""),
          String(hw.hostname ?? ""),
          String(hw.username ?? ""),
          String(hw.cpu ?? ""),
          String(hw.gpu ?? ""),
          String(hw.os ?? ""),
        ].some((s) => s.toLowerCase().includes(q));
      });
    }
    const num = (v: unknown) => Number(v) || 0;
    return [...list].sort((a, b) => {
      switch (sortKey) {
        case "first_seen":
          return num(b[1].first_seen) - num(a[1].first_seen);
        case "email":
          return String(a[1].email ?? a[0]).localeCompare(String(b[1].email ?? b[0]));
        case "events":
          return num(b[1].events) - num(a[1].events);
        default:
          return num(b[1].last_seen) - num(a[1].last_seen);
      }
    });
  }, [allEntries, query, sortKey]);

  if (ok === null) {
    return (
      <div className="page scroll">
        <div className="faint">Checking permissions…</div>
      </div>
    );
  }
  if (ok === false) {
    return (
      <div className="page scroll">
        <h1>Admin</h1>
        <div className="panel" style={{ marginTop: 14 }}>
          <div style={{ fontWeight: 700, marginBottom: 6 }}>Not authorized</div>
          <div className="faint" style={{ fontSize: 13 }}>
            Sign in with an admin account on the Accounts page to open this panel.
          </div>
          <button
            className="btn"
            style={{ marginTop: 12 }}
            onClick={() => useApp.getState().setPage("accounts")}
          >
            Go to Accounts
          </button>
        </div>
      </div>
    );
  }

  // ---- actions ----------------------------------------------------------

  const me = fbEmailSafe();

  const applyBan = async () => {
    if (!banUid) return;
    if (!banReason.trim()) {
      notify("err", "A reason is required");
      return;
    }
    const until =
      banUntil === "perm" ? null : Date.now() + ({ "1d": 1, "7d": 7, "30d": 30 }[banUntil] * 86_400_000);
    try {
      await fbSetBan(banUid, {
        banned: true,
        banned_reason: banReason.trim(),
        banned_until: until,
        banned_by: me,
        banned_photo: banPhoto.trim() || "",
        banned_video: banVideo.trim() || "",
        banned_at: Date.now(),
      });
      await fbLogAdminAction({
        action: "ban",
        target: banUid,
        by: me,
        detail: `${banUntil === "perm" ? "permanent" : banUntil} — ${banReason.trim()}`,
      });
      notify("ok", `Banned ${displayName(banUid)}`);
      setBanUid("");
      setBanReason("");
      setBanPhoto("");
      setBanVideo("");
      await loadUsers();
      await loadLog();
    } catch (e) {
      notify("err", e instanceof Error ? e.message : String(e));
    }
  };

  const liftBan = async (uid: string) => {
    try {
      await fbSetBan(uid, {
        banned: false,
        banned_reason: "",
        banned_until: null,
        banned_photo: "",
        banned_video: "",
        banned_at: Date.now(),
      });
      await fbLogAdminAction({ action: "unban", target: uid, by: me });
      notify("ok", `Unbanned ${displayName(uid)}`);
      await loadUsers();
      await loadLog();
    } catch (e) {
      notify("err", e instanceof Error ? e.message : String(e));
    }
  };

  const deleteUser = async (uid: string) => {
    try {
      await fbDeleteUser(uid);
      await fbLogAdminAction({ action: "delete_user", target: uid, by: me });
      notify("ok", `Deleted ${displayName(uid)}`);
      setDetailUid("");
      setConfirmDelete(false);
      await loadUsers();
      await loadLog();
    } catch (e) {
      notify("err", e instanceof Error ? e.message : String(e));
    }
  };

  const saveCfg = async () => {
    setCfgBusy(true);
    try {
      await fbSaveConfig(cfg as Record<string, unknown>);
      await fbLogAdminAction({ action: "config_save", target: "config", by: me });
      notify("ok", "Launcher config saved — clients pick it up within a minute");
      void loadLog();
    } catch (e) {
      notify("err", e instanceof Error ? e.message : String(e));
    } finally {
      setCfgBusy(false);
    }
  };

  const addAdmin = async () => {
    const e = newAdmin.trim().toLowerCase();
    if (!e.includes("@")) return notify("err", "Enter a valid email");
    try {
      await fbAddAdmin(e);
      await fbLogAdminAction({ action: "add_admin", target: e, by: me });
      setNewAdmin("");
      await loadAdmins();
      await loadLog();
      notify("ok", `Admin added: ${e}`);
    } catch (err) {
      notify("err", err instanceof Error ? err.message : String(err));
    }
  };

  const removeAdmin = async (e: string) => {
    try {
      await fbRemoveAdmin(e);
      await fbLogAdminAction({ action: "remove_admin", target: e, by: me });
      await loadAdmins();
      await loadLog();
      notify("ok", `Removed ${e}`);
    } catch (err) {
      notify("err", err instanceof Error ? err.message : String(err));
    }
  };

  // ---- render -----------------------------------------------------------

  const detailRow = detailUid ? users[detailUid] : undefined;

  // ---- export -----------------------------------------------------------

  const exportJson = () => {
    downloadFile(
      `isolate-users-${Date.now()}.json`,
      JSON.stringify(Object.fromEntries(entries), null, 2),
      "application/json",
    );
    notify("ok", `Exported ${entries.length} rows as JSON`);
  };

  const exportCsv = () => {
    const cols = [
      "uid", "email", "local_user", "app_version", "first_seen", "last_seen",
      "events", "os", "os_version", "arch", "cpu", "cpu_cores", "ram_mb", "gpu",
      "hostname", "username", "banned", "banned_reason", "banned_until", "has_ely", "has_discord",
    ];
    const esc = (v: unknown) => {
      const s = v === undefined || v === null ? "" : String(v);
      return /[",\n]/.test(s) ? `"${s.replace(/"/g, '""')}"` : s;
    };
    const rows = entries.map(([uid, row]) => {
      const hw = (row.hw ?? {}) as Record<string, unknown>;
      return [
        uid, row.email, row.local_user, row.app_version, row.first_seen, row.last_seen,
        row.events, hw.os, hw.os_version, hw.arch, hw.cpu, hw.cpu_cores, hw.ram_mb, hw.gpu,
        hw.hostname, hw.username, row.banned, row.banned_reason, row.banned_until,
        row.ely_account ? "yes" : "", row.discord ? "yes" : "",
      ].map(esc).join(",");
    });
    downloadFile(
      `isolate-users-${Date.now()}.csv`,
      [cols.join(","), ...rows].join("\n"),
      "text/csv",
    );
    notify("ok", `Exported ${entries.length} rows as CSV`);
  };

  const copyDetail = async () => {
    if (!detailRow) return;
    try {
      await navigator.clipboard.writeText(JSON.stringify({ uid: detailUid, ...detailRow }, null, 2));
      notify("ok", "Copied full JSON to clipboard");
    } catch {
      notify("err", "Clipboard unavailable");
    }
  };

  // ---- render -----------------------------------------------------------

  return (
    <div className="page scroll">
      <div className="ptn-head">
        <h1 style={{ margin: 0 }}>Admin</h1>
        <span className="badge ok">
          <ShieldCheck size={12} style={{ marginRight: 4, verticalAlign: -2 }} />
          Root access
        </span>
        <span className="grow" />
        <button className="btn sm" onClick={() => { void loadUsers(); void loadLog(); }}>
          <RefreshCw size={13} style={{ marginRight: 6, verticalAlign: -2 }} />
          Refresh
        </button>
      </div>
      <div className="faint" style={{ marginBottom: 14 }}>
        Full control over users, admins and the launcher itself.
      </div>

      {/* stats */}
      <div className="adm-stats">
        <Stat label="Total users" value={stats.total} />
        <Stat label="Active (7d)" value={stats.active} />
        <Stat label="Banned" value={stats.banned} danger={stats.banned > 0} />
        <Stat label="Ely.by linked" value={stats.ely} />
        <Stat label="Discord linked" value={stats.discord} />
        <Stat label="Admins" value={stats.admins} />
      </div>

      {/* tabs */}
      <div className="row" style={{ gap: 8, marginBottom: 16 }}>
        <button className={`btn sm ${tab === "users" ? "primary" : ""}`} onClick={() => setTab("users")}>
          <Users size={13} style={{ marginRight: 6, verticalAlign: -2 }} />
          Users ({entries.length})
        </button>
        <button className={`btn sm ${tab === "admins" ? "primary" : ""}`} onClick={() => setTab("admins")}>
          <ShieldCheck size={13} style={{ marginRight: 6, verticalAlign: -2 }} />
          Admins
        </button>
        <button className={`btn sm ${tab === "log" ? "primary" : ""}`} onClick={() => { setTab("log"); void loadLog(); }}>
          <Activity size={13} style={{ marginRight: 6, verticalAlign: -2 }} />
          Action log
        </button>
        <button className={`btn sm ${tab === "config" ? "primary" : ""}`} onClick={() => setTab("config")}>
          <Rocket size={13} style={{ marginRight: 6, verticalAlign: -2 }} />
          Launcher config
        </button>
      </div>

      {/* ---------------- users ---------------- */}
      {tab === "users" && (
        <section className="panel">
          <div className="row between" style={{ marginBottom: 10, gap: 10, flexWrap: "wrap" }}>
            <h2 style={{ margin: 0 }}>Devices & accounts</h2>
            <div className="row" style={{ gap: 8 }}>
              <button className="btn sm" onClick={exportJson}>
                <Download size={13} style={{ marginRight: 6, verticalAlign: -2 }} />
                JSON
              </button>
              <button className="btn sm" onClick={exportCsv}>
                <Download size={13} style={{ marginRight: 6, verticalAlign: -2 }} />
                CSV
              </button>
              {usersBusy && <span className="faint" style={{ fontSize: 12 }}>Loading…</span>}
            </div>
          </div>

          {/* toolbar: search + sort */}
          <div className="row" style={{ gap: 8, marginBottom: 12 }}>
            <div style={{ position: "relative", flex: 1, minWidth: 200 }}>
              <Search
                size={13}
                style={{
                  position: "absolute", left: 10, top: "50%",
                  transform: "translateY(-50%)", opacity: 0.5, pointerEvents: "none",
                }}
              />
              <input
                type="text"
                placeholder="Search email, uid, hostname, CPU, GPU…"
                value={query}
                onChange={(e) => setQuery(e.target.value)}
                style={{ paddingLeft: 30, width: "100%" }}
              />
            </div>
            <select value={sortKey} onChange={(e) => setSortKey(e.target.value as SortKey)}>
              <option value="last_seen">Sort: last seen</option>
              <option value="first_seen">Sort: first seen</option>
              <option value="email">Sort: email</option>
              <option value="events">Sort: events</option>
            </select>
          </div>

          {entries.length === 0 ? (
            <div className="faint" style={{ fontSize: 13 }}>
              {allEntries.length === 0
                ? "No telemetry yet — rows appear as soon as a client launches."
                : "No users match this search."}
            </div>
          ) : (
            <div className="adm-list">
              {entries.map(([uid, row]) => {
                const banned = row.banned === true;
                const until = typeof row.banned_until === "number" ? row.banned_until : 0;
                const activeBan = banned && (!until || until > Date.now());
                const hw = (row.hw ?? {}) as Record<string, string | number>;
                return (
                  <div key={uid} className={`adm-row ${activeBan ? "banned" : ""}`}>
                    <div className="adm-main">
                      <div className="adm-id">
                        <strong>{displayName(uid)}</strong>
                        {row.email ? <span className="faint"> · {String(row.email)}</span> : null}
                        {row.local_user && !row.email ? (
                          <span className="faint"> · local: {String(row.local_user)}</span>
                        ) : null}
                        {activeBan ? (
                          <span className="badge" style={{ marginLeft: 8, borderColor: "var(--danger)", color: "var(--danger)" }}>
                            <Ban size={11} style={{ marginRight: 4, verticalAlign: -2 }} />
                            {until ? `timed out until ${new Date(until).toLocaleString()}` : "BANNED"}
                          </span>
                        ) : null}
                      </div>
                      <div className="adm-meta faint">
                        {String(hw.os ?? "?")} {String(hw.os_version ?? "")} · {String(hw.arch ?? "")} ·{" "}
                        {hw.cpu_cores ? `${hw.cpu_cores} cores · ` : ""}
                        {hw.ram_mb ? `${hw.ram_mb} MB RAM · ` : ""}
                        {hw.cpu ? `${String(hw.cpu)} · ` : ""}
                        {hw.gpu ? `${String(hw.gpu)} · ` : ""}
                        {String(hw.hostname ?? "")} · v{String(row.app_version ?? "?")} ·{" "}
                        {row.last_seen ? new Date(Number(row.last_seen)).toLocaleString() : "?"} ·{" "}
                        {row.first_seen ? `first ${new Date(Number(row.first_seen)).toLocaleDateString()}` : ""}
                        {row.events ? ` · ${row.events} events` : ""}
                        {row.ely_account ? " · Ely.by" : ""}
                        {row.ms_account ? " · Microsoft" : ""}
                        {row.discord ? " · Discord" : ""}
                      </div>
                      {row.banned_reason ? (
                        <div className="adm-reason">
                          <strong>Reason:</strong> {String(row.banned_reason)}
                          {row.banned_photo ? (
                            <a href={String(row.banned_photo)} target="_blank" rel="noreferrer">
                              {" "}photo <ExternalLink size={10} />
                            </a>
                          ) : null}
                          {row.banned_video ? (
                            <a href={String(row.banned_video)} target="_blank" rel="noreferrer">
                              {" "}video <ExternalLink size={10} />
                            </a>
                          ) : null}
                        </div>
                      ) : null}
                    </div>
                    <div className="adm-actions">
                      <button
                        className="btn sm"
                        onClick={() => { setDetailUid(uid); setConfirmDelete(false); }}
                      >
                        <Eye size={13} style={{ marginRight: 6, verticalAlign: -2 }} />
                        Details
                      </button>
                      {activeBan ? (
                        <button className="btn sm" onClick={() => void liftBan(uid)}>
                          <Unlock size={13} style={{ marginRight: 6, verticalAlign: -2 }} />
                          Unblock
                        </button>
                      ) : banUid === uid ? (
                        <div className="adm-banform">
                          <input
                            type="text"
                            placeholder="Reason (required)"
                            value={banReason}
                            onChange={(e) => setBanReason(e.target.value)}
                          />
                          <div className="row" style={{ gap: 6 }}>
                            <select value={banUntil} onChange={(e) => setBanUntil(e.target.value as typeof banUntil)}>
                              <option value="perm">Permanent</option>
                              <option value="1d">1 day</option>
                              <option value="7d">7 days</option>
                              <option value="30d">30 days</option>
                            </select>
                            <input
                              type="text"
                              placeholder="Photo evidence URL"
                              value={banPhoto}
                              onChange={(e) => setBanPhoto(e.target.value)}
                            />
                            <input
                              type="text"
                              placeholder="Video evidence URL"
                              value={banVideo}
                              onChange={(e) => setBanVideo(e.target.value)}
                            />
                          </div>
                          <div className="row" style={{ gap: 6, marginTop: 6 }}>
                            <button className="btn sm danger" onClick={() => void applyBan()}>
                              <Ban size={13} style={{ marginRight: 6, verticalAlign: -2 }} />
                              Confirm ban
                            </button>
                            <button className="btn sm" onClick={() => setBanUid("")}>
                              Cancel
                            </button>
                          </div>
                        </div>
                      ) : (
                        <button className="btn sm danger" onClick={() => { setBanUid(uid); setBanReason(""); setBanPhoto(""); setBanVideo(""); }}>
                          <Ban size={13} style={{ marginRight: 6, verticalAlign: -2 }} />
                          Ban / timeout
                        </button>
                      )}
                    </div>
                  </div>
                );
              })}
            </div>
          )}
        </section>
      )}

      {/* ---------------- admins ---------------- */}
      {tab === "admins" && (
        <section className="panel">
          <h2>Admin accounts</h2>
          <div className="hint" style={{ marginBottom: 12 }}>
            Admins can sign in on the Accounts page with their own email — they get
            this panel plus the Partnerships editor.
          </div>
          <div className="adm-list">
            <div className="adm-row">
              <div className="adm-main">
                <strong>admin@isolateclient.in</strong>
                <span className="badge ok" style={{ marginLeft: 8 }}>
                  <CheckCircle2 size={11} style={{ marginRight: 4, verticalAlign: -2 }} />
                  seed
                </span>
              </div>
            </div>
            {admins.map((e) => (
              <div key={e} className="adm-row">
                <div className="adm-main">
                  <strong>{e}</strong>
                </div>
                <div className="adm-actions">
                  <button className="btn sm danger" onClick={() => void removeAdmin(e)}>
                    <Trash2 size={13} style={{ marginRight: 6, verticalAlign: -2 }} />
                    Remove
                  </button>
                </div>
              </div>
            ))}
          </div>
          <div className="row" style={{ gap: 8, marginTop: 12 }}>
            <input
              type="email"
              placeholder="new-admin@example.com"
              value={newAdmin}
              onChange={(e) => setNewAdmin(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && void addAdmin()}
              style={{ flex: 1 }}
            />
            <button className="btn primary" onClick={() => void addAdmin()}>
              Add admin
            </button>
          </div>
        </section>
      )}

      {/* ---------------- action log ---------------- */}
      {tab === "log" && (
        <section className="panel">
          <div className="row between" style={{ marginBottom: 10 }}>
            <h2 style={{ margin: 0 }}>Admin action log</h2>
            <button className="btn sm" onClick={() => void loadLog()}>
              <RefreshCw size={13} style={{ marginRight: 6, verticalAlign: -2 }} />
              Refresh
            </button>
          </div>
          <div className="hint" style={{ marginBottom: 12 }}>
            Every ban, unban, delete, admin change and config save — newest first (last 200).
          </div>
          {log.length === 0 ? (
            <div className="faint" style={{ fontSize: 13 }}>
              No actions logged yet.
            </div>
          ) : (
            <div className="adm-list">
              {log.map((e) => (
                <div key={`${e.at}-${e.action}-${e.target}`} className="adm-row">
                  <div className="adm-main">
                    <div className="adm-id">
                      <strong>{ACTION_LABELS[e.action] ?? e.action}</strong>
                      <span className="faint"> · {e.target}</span>
                    </div>
                    <div className="adm-meta faint">
                      {new Date(e.at).toLocaleString()} · by {e.by}
                      {e.detail ? ` · ${e.detail}` : ""}
                    </div>
                  </div>
                </div>
              ))}
            </div>
          )}
        </section>
      )}

      {/* ---------------- launcher config ---------------- */}
      {tab === "config" && (
        <section className="panel">
          <h2>Remote launcher config</h2>
          <div className="hint" style={{ marginBottom: 12 }}>
            Every client fetches this from Firebase each minute — name, logo, accent,
            splash and notice update without a release.
          </div>
          <div className="ptn-form">
            <label className="field">
              <span>Launcher name</span>
              <input
                type="text"
                value={cfg.name ?? ""}
                placeholder="Isolate Client"
                onChange={(e) => setCfg({ ...cfg, name: e.target.value })}
              />
            </label>
            <label className="field">
              <span>Logo URL</span>
              <input
                type="text"
                spellCheck={false}
                placeholder="https://…/logo.png"
                value={cfg.logo ?? ""}
                onChange={(e) => setCfg({ ...cfg, logo: e.target.value })}
              />
            </label>
            <label className="field">
              <span>Tagline</span>
              <input
                type="text"
                placeholder="A lean Minecraft launcher"
                value={cfg.tagline ?? ""}
                onChange={(e) => setCfg({ ...cfg, tagline: e.target.value })}
              />
            </label>
            <label className="field">
              <span>Accent color</span>
              <input
                type="color"
                value={cfg.accent || "#34d399"}
                onChange={(e) => setCfg({ ...cfg, accent: e.target.value })}
                style={{ height: 38, padding: 4 }}
              />
            </label>
            <label className="field">
              <span>Splash text (loading screen)</span>
              <input
                type="text"
                placeholder="Starting Isolate…"
                value={cfg.splash ?? ""}
                onChange={(e) => setCfg({ ...cfg, splash: e.target.value })}
              />
            </label>
            <label className="field">
              <span>Notice banner (empty = hidden)</span>
              <input
                type="text"
                placeholder="Scheduled maintenance Saturday"
                value={cfg.notice ?? ""}
                onChange={(e) => setCfg({ ...cfg, notice: e.target.value })}
              />
            </label>
            <label className="field">
              <span>Website URL</span>
              <input
                type="text"
                spellCheck={false}
                placeholder="https://isolateclient.in"
                value={cfg.website ?? ""}
                onChange={(e) => setCfg({ ...cfg, website: e.target.value })}
              />
            </label>
            <label className="field">
              <span>Discord invite</span>
              <input
                type="text"
                spellCheck={false}
                placeholder="https://discord.gg/…"
                value={cfg.discord ?? ""}
                onChange={(e) => setCfg({ ...cfg, discord: e.target.value })}
              />
            </label>
          </div>
          <div className="row" style={{ gap: 8, marginTop: 14 }}>
            <button className="btn primary" disabled={cfgBusy} onClick={() => void saveCfg()}>
              {cfgBusy ? "Saving…" : "Save config"}
            </button>
            <button className="btn" onClick={() => void loadCfg()}>
              Reset
            </button>
          </div>
        </section>
      )}

      {/* ---------------- full detail modal ---------------- */}
      {detailUid && detailRow && (
        <div className="overlay" onClick={() => { setDetailUid(""); setConfirmDelete(false); }}>
          <div
            className="modal wide"
            style={{ maxWidth: 780, cursor: "default" }}
            onClick={(e) => e.stopPropagation()}
          >
            <header>
              <h2>
                <Eye size={15} style={{ marginRight: 8, verticalAlign: -3, opacity: 0.7 }} />
                {displayName(detailUid)} — full details
              </h2>
              <button
                className="btn sm"
                onClick={() => { setDetailUid(""); setConfirmDelete(false); }}
                aria-label="Close details"
              >
                <X size={14} />
              </button>
            </header>
            <div className="body">
              <DetailSection title="Identity">
                <Kv k="UID" v={detailUid} mono copy />
                <Kv k="Email" v={strOrDash(detailRow.email)} />
                <Kv k="Local user" v={strOrDash(detailRow.local_user)} />
                <Kv k="App version" v={strOrDash(detailRow.app_version)} />
                <Kv k="First seen" v={fmtDate(detailRow.first_seen)} />
                <Kv k="Last seen" v={fmtDate(detailRow.last_seen)} />
                <Kv k="Last event" v={strOrDash(detailRow.last_event)} />
                <Kv k="Events" v={String(detailRow.events ?? 0)} />
                <Kv
                  k="Password hash"
                  v={detailRow.pass_hash ? `${String(detailRow.pass_hash).slice(0, 16)}…` : "—"}
                  mono
                />
              </DetailSection>

              <DetailSection title="Hardware">
                {(() => {
                  const hw = (detailRow.hw ?? {}) as Record<string, unknown>;
                  const rows: Array<[string, unknown]> = [
                    ["OS", hw.os],
                    ["OS version", hw.os_version],
                    ["Arch", hw.arch],
                    ["CPU", hw.cpu],
                    ["Cores", hw.cpu_cores],
                    ["RAM", hw.ram_mb ? `${hw.ram_mb} MB` : undefined],
                    ["GPU", hw.gpu],
                    ["Hostname", hw.hostname],
                    ["Username", hw.username],
                  ];
                  const present = rows.filter(([, v]) => v !== undefined && v !== "" && v !== null);
                  return present.length ? (
                    present.map(([k, v]) => <Kv key={k} k={k} v={String(v)} />)
                  ) : (
                    <div className="faint" style={{ fontSize: 12 }}>No hardware data.</div>
                  );
                })()}
              </DetailSection>

              <DetailSection title="Linked accounts">
                <Kv k="Ely.by" v={detailRow.ely_account ? "linked" : "—"} />
                {detailRow.ely_account ? (
                  <pre className="adm-json">{JSON.stringify(detailRow.ely_account, null, 2)}</pre>
                ) : null}
                <Kv k="Discord" v={detailRow.discord ? "linked" : "—"} />
                {detailRow.discord ? (
                  <pre className="adm-json">{JSON.stringify(detailRow.discord, null, 2)}</pre>
                ) : null}
                <Kv k="Microsoft" v={detailRow.ms_account ? "linked" : "—"} />
                {detailRow.ms_account ? (
                  <pre className="adm-json">{JSON.stringify(detailRow.ms_account, null, 2)}</pre>
                ) : null}
              </DetailSection>

              <DetailSection title="Moderation">
                <Kv k="Status" v={detailRow.banned === true ? "BANNED" : "clean"} />
                <Kv k="Reason" v={strOrDash(detailRow.banned_reason)} />
                <Kv k="Banned by" v={strOrDash(detailRow.banned_by)} />
                <Kv k="Banned at" v={fmtDate(detailRow.banned_at)} />
                <Kv
                  k="Expires"
                  v={
                    detailRow.banned_until
                      ? fmtDate(detailRow.banned_until)
                      : detailRow.banned === true
                        ? "never (permanent)"
                        : "—"
                  }
                />
                {detailRow.banned_photo ? (
                  <div style={{ fontSize: 12, marginTop: 4 }}>
                    Photo evidence:{" "}
                    <a href={String(detailRow.banned_photo)} target="_blank" rel="noreferrer">
                      open <ExternalLink size={10} />
                    </a>
                  </div>
                ) : null}
                {detailRow.banned_video ? (
                  <div style={{ fontSize: 12, marginTop: 4 }}>
                    Video evidence:{" "}
                    <a href={String(detailRow.banned_video)} target="_blank" rel="noreferrer">
                      open <ExternalLink size={10} />
                    </a>
                  </div>
                ) : null}
              </DetailSection>

              <DetailSection title="Raw JSON">
                <pre className="adm-json">{JSON.stringify({ uid: detailUid, ...detailRow }, null, 2)}</pre>
              </DetailSection>
            </div>
            <footer>
              <button className="btn sm" onClick={() => void copyDetail()}>
                <Copy size={13} style={{ marginRight: 6, verticalAlign: -2 }} />
                Copy JSON
              </button>
              {detailRow.banned === true ? (
                <button className="btn sm" onClick={() => void liftBan(detailUid)}>
                  <Unlock size={13} style={{ marginRight: 6, verticalAlign: -2 }} />
                  Unblock
                </button>
              ) : (
                <button
                  className="btn sm danger"
                  onClick={() => {
                    setBanUid(detailUid);
                    setBanReason("");
                    setBanPhoto("");
                    setBanVideo("");
                    setDetailUid("");
                    setTab("users");
                  }}
                >
                  <Ban size={13} style={{ marginRight: 6, verticalAlign: -2 }} />
                  Ban
                </button>
              )}
              {confirmDelete ? (
                <button className="btn sm danger" onClick={() => void deleteUser(detailUid)}>
                  <Trash2 size={13} style={{ marginRight: 6, verticalAlign: -2 }} />
                  Really delete — this erases everything
                </button>
              ) : (
                <button className="btn sm danger" onClick={() => setConfirmDelete(true)}>
                  <Trash2 size={13} style={{ marginRight: 6, verticalAlign: -2 }} />
                  Delete user
                </button>
              )}
            </footer>
          </div>
        </div>
      )}
    </div>
  );
}

// ---- small building blocks ---------------------------------------------

const ACTION_LABELS: Record<string, string> = {
  ban: "Banned",
  unban: "Unbanned",
  delete_user: "Deleted user",
  add_admin: "Added admin",
  remove_admin: "Removed admin",
  config_save: "Saved config",
};

function Stat({ label, value, danger }: { label: string; value: number; danger?: boolean }) {
  return (
    <div className="adm-stat">
      <div
        className="adm-stat-v"
        style={danger ? { color: "var(--danger)" } : undefined}
      >
        {value}
      </div>
      <div className="adm-stat-l faint">{label}</div>
    </div>
  );
}

function DetailSection({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div style={{ marginBottom: 16 }}>
      <div
        style={{
          fontSize: 11,
          fontWeight: 700,
          textTransform: "uppercase",
          letterSpacing: "0.08em",
          opacity: 0.55,
          marginBottom: 8,
        }}
      >
        {title}
      </div>
      {children}
    </div>
  );
}

function Kv({ k, v, mono, copy }: { k: string; v: string; mono?: boolean; copy?: boolean }) {
  const doCopy = () => {
    void navigator.clipboard?.writeText(v).catch(() => undefined);
  };
  return (
    <div className="adm-kv">
      <span className="faint" style={{ minWidth: 120, fontSize: 12 }}>{k}</span>
      <span style={{ fontSize: 12.5, wordBreak: "break-all", fontFamily: mono ? "var(--mono)" : undefined }}>
        {v}
        {copy ? (
          <button
            className="btn sm"
            style={{ marginLeft: 8, padding: "2px 7px", fontSize: 10 }}
            onClick={doCopy}
            aria-label={`Copy ${k}`}
          >
            <Copy size={10} />
          </button>
        ) : null}
      </span>
    </div>
  );
}

function strOrDash(v: unknown): string {
  if (v === undefined || v === null || v === "") return "—";
  return String(v);
}

function fmtDate(v: unknown): string {
  const n = Number(v);
  if (!n) return "—";
  return new Date(n).toLocaleString();
}

function downloadFile(name: string, text: string, mime: string): void {
  const blob = new Blob([text], { type: mime });
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = name;
  a.click();
  setTimeout(() => URL.revokeObjectURL(url), 4000);
}

function displayName(uid: string): string {
  return uid.startsWith("d-") ? uid.slice(0, 14) + "…" : uid;
}

function fbEmailSafe(): string {
  try {
    const t = localStorage.getItem("isolate.fb.token");
    if (!t) return "admin";
    const payload = t.split(".")[1] ?? "";
    const b64 = payload.replace(/-/g, "+").replace(/_/g, "/");
    const json = decodeURIComponent(
      atob(b64)
        .split("")
        .map((c) => "%" + c.charCodeAt(0).toString(16).padStart(2, "0"))
        .join(""),
    );
    return (JSON.parse(json) as { email?: string }).email ?? "admin";
  } catch {
    return "admin";
  }
}

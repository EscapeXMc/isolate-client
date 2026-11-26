// Mods — Modrinth browser + installed mods for the selected instance.
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { api } from "../lib/tauri";
import { useApp } from "../stores/app";
import type { ModConflict, ModProfile, ModrinthHit, PackFile } from "../lib/types";
import { RefreshCw, Search, X } from "lucide-react";

type Tab = "browse" | "installed" | "conflicts" | "profiles";

export default function Mods() {
  const instances = useApp((s) => s.instances);
  const notify = useApp((s) => s.notify);

  const [instance, setInstance] = useState("");
  const [tab, setTab] = useState<Tab>("browse");

  // browse state
  const [query, setQuery] = useState("");
  const [hits, setHits] = useState<ModrinthHit[]>([]);
  const [searching, setSearching] = useState(false);
  // true = default "most downloaded" view; false = user searched something
  const [trending, setTrending] = useState(true);
  const [trendReady, setTrendReady] = useState(false);
  const trendingLoaded = useRef(false);
  // FastClient-style detail drawer
  const [detail, setDetail] = useState<ModrinthHit | null>(null);

  // installed state
  const [installed, setInstalled] = useState<PackFile[]>([]);
  const [conflicts, setConflicts] = useState<ModConflict[]>([]);
  const [profiles, setProfiles] = useState<ModProfile[]>([]);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    if (!instance && instances.length > 0) setInstance(instances[0].name);
  }, [instances, instance]);

  const refreshInstalled = useCallback(async () => {
    if (!instance) return;
    try {
      const r = await api.getInstanceMods(instance);
      setInstalled(r.mods ?? []);
    } catch {
      setInstalled([]);
    }
  }, [instance]);

  useEffect(() => {
    void refreshInstalled();
  }, [refreshInstalled]);

  useEffect(() => {
    if (tab === "profiles") {
      void api.loadModProfiles().then((p) => setProfiles(Array.isArray(p) ? p : [])).catch(() => setProfiles([]));
    }
  }, [tab]);

  // Default Browse view: most-downloaded mods (backend sorts empty query by
  // downloads) so the page feels real before the user types anything.
  const loadTrending = useCallback(async () => {
    setTrending(true);
    setTrendReady(false);
    try {
      const r = await api.searchModrinth("", "mod", "", "", 24);
      const list = (r as { hits?: ModrinthHit[] }).hits;
      const err = (r as { error?: string }).error;
      const sorted = Array.isArray(list)
        ? [...list].sort((a, b) => (b.downloads ?? 0) - (a.downloads ?? 0))
        : [];
      setHits(sorted);
      if (err) notify("err", String(err));
    } catch (e) {
      setHits([]);
      notify("err", `Couldn't load trending mods: ${String(e)}`);
    } finally {
      setTrendReady(true);
    }
  }, [notify]);

  useEffect(() => {
    if (tab !== "browse" || trendingLoaded.current) return;
    trendingLoaded.current = true;
    void loadTrending();
  }, [tab, loadTrending]);

  const doSearch = async () => {
    if (!query.trim()) return;
    setSearching(true);
    setTrending(false);
    try {
      const r = await api.searchModrinth(query.trim(), "mod", "", "", 24);
      const list = (r as { hits?: ModrinthHit[] }).hits;
      setHits(Array.isArray(list) ? list : []);
      if (r && (r as { error?: string }).error) notify("err", (r as { error: string }).error);
    } catch (e) {
      notify("err", String(e));
    } finally {
      setSearching(false);
    }
  };

  const install = async (slug: string) => {
    if (!instance) return notify("err", "Select an instance first.");
    setBusy(true);
    try {
      const r = await api.installMod(instance, slug, "");
      if (r.error) notify("err", String(r.error));
      else {
        notify("ok", `Installed ${slug}`);
        await refreshInstalled();
      }
    } catch (e) {
      notify("err", String(e));
    } finally {
      setBusy(false);
    }
  };

  const toggle = async (f: PackFile) => {
    if (!instance) return;
    const enabled = !f.disabled;
    const r = await api.toggleMod(instance, f.filename, enabled);
    if (r.error) notify("err", String(r.error));
    await refreshInstalled();
  };

  const remove = async (f: PackFile) => {
    if (!instance) return;
    const r = await api.deleteMod(instance, f.filename);
    if (r.error) notify("err", String(r.error));
    else notify("ok", `Removed ${f.name}`);
    await refreshInstalled();
  };

  const checkUpdates = async () => {
    if (!instance) return;
    setBusy(true);
    try {
      const r = await api.checkModUpdates(instance);
      const updates = (r as { updates?: unknown[] }).updates ?? [];
      if (r.error) notify("err", String(r.error));
      else if (updates.length === 0) notify("ok", "All mods are up to date.");
      else notify("ok", `${updates.length} mod(s) have updates available.`);
    } finally {
      setBusy(false);
    }
  };

  const detectConflicts = async () => {
    if (!instance) return;
    setBusy(true);
    try {
      const r = await api.detectModConflicts(instance);
      setConflicts(r.conflicts ?? []);
      if (r.error) notify("err", r.error);
      else if ((r.conflicts ?? []).length === 0) notify("ok", "No conflicts found.");
    } finally {
      setBusy(false);
    }
  };

  const installPerf = async () => {
    if (!instance) return;
    const inst = instances.find((i) => i.name === instance);
    if (!inst) return;
    setBusy(true);
    try {
      const r = await api.installPerfMods(instance, inst.version, inst.loader);
      if (r.error) notify("err", String(r.error));
      else {
        notify("ok", "Performance mods installed.");
        await refreshInstalled();
      }
    } finally {
      setBusy(false);
    }
  };

  const saveProfile = async () => {
    if (!instance) return;
    const name = window.prompt("Mod profile name:");
    if (!name) return;
    const r = await api.saveModProfile(name, instance);
    if (r.error) notify("err", String(r.error));
    else notify("ok", `Saved profile "${name}"`);
    setProfiles(await api.loadModProfiles());
  };

  const applyProfile = async (name: string) => {
    if (!instance) return;
    setBusy(true);
    try {
      const r = await api.applyModProfile(name, instance);
      if (r.error) notify("err", String(r.error));
      else {
        notify("ok", `Applied "${name}"`);
        await refreshInstalled();
      }
    } finally {
      setBusy(false);
    }
  };

  const instanceOptions = useMemo(() => instances, [instances]);

  // loose match: is this Modrinth hit already in the instance's mods folder?
  const hitInstalled = useCallback(
    (h: ModrinthHit) => {
      const title = (h.title ?? h.slug ?? "").toLowerCase();
      if (!title) return false;
      return installed.some((f) => f.name.toLowerCase().includes(title));
    },
    [installed],
  );

  return (
    <div className="page scroll">
      <div className="row between" style={{ marginBottom: 14, flexWrap: "wrap", gap: 10 }}>
        <div className="row" style={{ gap: 12 }}>
          <h2 style={{ margin: 0 }}>Mods</h2>
          <span className="tinycaps">Modrinth browser</span>
        </div>
        {/* profile context pill (FastClient shows profile icon + name here) */}
        <div className="md-profile">
          <span className="md-prof-ico" aria-hidden />
          <select value={instance} onChange={(e) => setInstance(e.target.value)}>
            <option value="">Select profile…</option>
            {instanceOptions.map((i) => (
              <option key={i.name} value={i.name}>
                {i.name} ({i.version})
              </option>
            ))}
          </select>
        </div>
      </div>

      <div className="tabs">
        {(["browse", "installed", "conflicts", "profiles"] as Tab[]).map((t) => (
          <button key={t} className={tab === t ? "active" : ""} onClick={() => setTab(t)}>
            {t === "browse" ? "Browse" : t === "installed" ? "Installed" : t === "conflicts" ? "Conflicts" : "Profiles"}
          </button>
        ))}
        <div style={{ flex: 1 }} />
        <button className="btn sm" disabled={busy || !instance} onClick={() => void checkUpdates()}>
          Check updates
        </button>
        <button className="btn sm" disabled={busy || !instance} onClick={() => void installPerf()}>
          FPS mods
        </button>
      </div>

      {tab === "browse" && (
        <>
          {/* sub-toolbar — FastClient: search · refresh · source toggle */}
          <div className="md-toolbar">
            <div className="md-search">
              <Search aria-hidden />
              <input
                type="text"
                placeholder="Search mods… (empty = most downloaded)"
                value={query}
                onChange={(e) => setQuery(e.target.value)}
                onKeyDown={(e) => e.key === "Enter" && void doSearch()}
              />
            </div>
            <button className="btn primary" disabled={searching} onClick={() => void doSearch()}>
              {searching ? "Searching…" : "Search"}
            </button>
            <button
              className="btn sm md-refresh"
              title="Back to most downloaded"
              onClick={() => {
                setQuery("");
                trendingLoaded.current = false;
                setHits([]);
                void loadTrending();
              }}
            >
              <RefreshCw aria-hidden />
            </button>
            <div className="md-source" role="group" aria-label="Source">
              <button className="on" type="button">
                Modrinth
              </button>
              <button type="button" disabled title="Coming soon">
                CurseForge
              </button>
            </div>
          </div>

          <div className="grid-head" style={{ marginBottom: 10 }}>
            <span className="tinycaps">
              {trending ? "Most downloaded mods" : `Results for “${query.trim()}”`}
            </span>
            {hits.length > 0 && (
              <span className="faint" style={{ fontSize: 11 }}>
                {hits.length} mod{hits.length === 1 ? "" : "s"}
                {trending ? " · Modrinth trending" : ""}
              </span>
            )}
          </div>

          {hits.length === 0 ? (
            <div className="empty">
              {!trendReady
                ? "Loading the most downloaded mods…"
                : trending
                  ? "Couldn't load trending mods — check your connection."
                  : "No mods found — try another search."}
            </div>
          ) : (
            <div className="cards">
              {hits.map((h) => (
                <div
                  className="card md-card"
                  key={h.project_id ?? h.slug}
                  role="button"
                  tabIndex={0}
                  onClick={() => setDetail(h)}
                  onKeyDown={(e) => e.key === "Enter" && setDetail(h)}
                >
                  <div className="row between">
                    <div className="row" style={{ gap: 10 }}>
                      {h.icon_url ? (
                        <img src={h.icon_url} alt="" style={{ width: 34, height: 34, borderRadius: 7, objectFit: "cover" }} />
                      ) : (
                        <span className="md-noico" aria-hidden />
                      )}
                      <div className="title">{h.title ?? h.slug}</div>
                    </div>
                    {hitInstalled(h) ? (
                      <span className="badge ok">Installed</span>
                    ) : (
                      <span className="badge grey">{(h.downloads ?? 0).toLocaleString()} ↓</span>
                    )}
                  </div>
                  <div className="faint" style={{ fontSize: 11.5, marginTop: 6 }}>
                    by {h.author ?? "unknown"}
                  </div>
                  <div className="meta">
                    <span className="faint" style={{ fontSize: 12, display: "block", margin: "7px 0" }}>
                      {(h.description ?? "").slice(0, 130)}
                      {(h.description ?? "").length > 130 ? "…" : ""}
                    </span>
                  </div>
                  <div className="actions">
                    <button
                      className="btn primary sm"
                      style={{ flex: 1, justifyContent: "center" }}
                      disabled={busy || !instance}
                      onClick={(e) => {
                        e.stopPropagation();
                        void install(h.slug ?? "");
                      }}
                    >
                      Install
                    </button>
                  </div>
                </div>
              ))}
            </div>
          )}
        </>
      )}

      {tab === "installed" && (
        <>
          <div className="row between" style={{ marginBottom: 10 }}>
            <span className="faint">{installed.length} file(s)</span>
            <button className="btn sm" onClick={() => void refreshInstalled()}>
              Refresh
            </button>
          </div>
          {installed.length === 0 ? (
            <div className="empty">No mods installed in this instance.</div>
          ) : (
            <div className="panel">
              {installed.map((f) => (
                <div key={f.filename} className="row between" style={{ padding: "8px 0", borderBottom: "1px solid var(--border)" }}>
                  <div className="row" style={{ gap: 10 }}>
                    <span className={`badge ${f.disabled ? "grey" : "ok"}`}>{f.disabled ? "Off" : "On"}</span>
                    <span style={{ fontSize: 13 }}>{f.name}</span>
                  </div>
                  <div className="row" style={{ gap: 6 }}>
                    <button className="btn sm" onClick={() => void toggle(f)}>
                      {f.disabled ? "Enable" : "Disable"}
                    </button>
                    <button className="btn sm danger" onClick={() => void remove(f)}>
                      Delete
                    </button>
                  </div>
                </div>
              ))}
            </div>
          )}
        </>
      )}

      {tab === "conflicts" && (
        <>
          <div className="row" style={{ marginBottom: 12 }}>
            <button className="btn primary" disabled={busy || !instance} onClick={() => void detectConflicts()}>
              Scan for conflicts
            </button>
          </div>
          {conflicts.length === 0 ? (
            <div className="empty">Run a scan to detect incompatible mod combinations.</div>
          ) : (
            conflicts.map((c, i) => (
              <div key={i} className="panel" style={{ marginBottom: 10, borderColor: c.severity === "high" ? "var(--danger)" : "var(--border)" }}>
                <div className="row between">
                  <strong style={{ fontSize: 13 }}>
                    {c.mod1} ↔ {c.mod2}
                  </strong>
                  <span className={`badge ${c.severity === "high" ? "warn" : "grey"}`}>{c.severity}</span>
                </div>
                <div className="faint" style={{ fontSize: 12, marginTop: 6 }}>{c.reason}</div>
              </div>
            ))
          )}
        </>
      )}

      {tab === "profiles" && (
        <>
          <div className="row" style={{ marginBottom: 12 }}>
            <button className="btn primary" disabled={!instance} onClick={() => void saveProfile()}>
              Save current mods as profile
            </button>
          </div>
          {profiles.length === 0 ? (
            <div className="empty">No mod profiles saved yet.</div>
          ) : (
            <div className="panel">
              {profiles.map((p) => (
                <div key={p.name} className="row between" style={{ padding: "9px 0", borderBottom: "1px solid var(--border)" }}>
                  <div>
                    <div style={{ fontSize: 13, fontWeight: 600 }}>{p.name}</div>
                    <div className="faint" style={{ fontSize: 11 }}>{(p.mods ?? []).length} mod(s)</div>
                  </div>
                  <div className="row" style={{ gap: 6 }}>
                    <button className="btn sm primary" disabled={busy || !instance} onClick={() => void applyProfile(p.name)}>
                      Apply
                    </button>
                    <button
                      className="btn sm danger"
                      onClick={async () => {
                        await api.deleteModProfile(p.name);
                        setProfiles(await api.loadModProfiles());
                      }}
                    >
                      Delete
                    </button>
                  </div>
                </div>
              ))}
            </div>
          )}
        </>
      )}

      {/* detail drawer — FastClient right panel */}
      {detail && (
        <>
          <div className="md-drawer-back" onClick={() => setDetail(null)} />
          <aside className="md-drawer">
            <div className="md-d-head">
              {detail.icon_url ? (
                <img src={detail.icon_url} alt="" className="md-d-ico" />
              ) : (
                <span className="md-noico big" aria-hidden />
              )}
              <div className="md-d-id">
                <div className="md-d-title">{detail.title ?? detail.slug}</div>
                <div className="md-d-author">by {detail.author ?? "unknown"}</div>
              </div>
              <button className="md-d-x" title="Close" onClick={() => setDetail(null)}>
                <X aria-hidden />
              </button>
            </div>
            <div className="md-d-body">
              <p className="md-d-desc">{detail.description || "No description."}</p>
              {!!detail.categories?.length && (
                <div className="md-d-tags">
                  {detail.categories.map((c) => (
                    <span key={c}>{c}</span>
                  ))}
                </div>
              )}
              <div className="md-d-stats">
                <span>{(detail.downloads ?? 0).toLocaleString()} downloads</span>
                <span>{(detail.follows ?? 0).toLocaleString()} followers</span>
              </div>
              <button
                className="md-d-link"
                onClick={() =>
                  detail.slug && void api.openUrl(`https://modrinth.com/mod/${detail.slug}`)
                }
              >
                View on Modrinth ↗
              </button>
            </div>
            <div className="md-d-foot">
              <button
                className="btn primary"
                disabled={busy || !instance}
                onClick={() => void install(detail.slug ?? "")}
              >
                {busy ? "Installing…" : instance ? `Install into ${instance}` : "Install"}
              </button>
              {!instance && (
                <div className="faint" style={{ fontSize: 11.5, textAlign: "center" }}>
                  Select a profile first.
                </div>
              )}
            </div>
          </aside>
        </>
      )}
    </div>
  );
}

// Self-hosted server manager — create/start/stop, console, properties,
// plugins, files, backups, worlds.
import { useCallback, useEffect, useRef, useState } from "react";
import { api } from "../lib/tauri";
import { useApp } from "../stores/app";
import type { ManagedServer, PackFile, ServerFileEntry } from "../lib/types";

type Tab = "list" | "console" | "properties" | "plugins" | "files" | "backups";

interface ServerStats {
  running: boolean;
  pid?: number;
  uptime_secs?: number;
  memory_kb?: number;
}

export default function ServerManager() {
  const notify = useApp((s) => s.notify);
  const [servers, setServers] = useState<ManagedServer[]>([]);
  const [selected, setSelected] = useState("");
  const [tab, setTab] = useState<Tab>("list");
  const [stats, setStats] = useState<ServerStats | null>(null);
  const [busy, setBusy] = useState(false);

  // create form
  const [creating, setCreating] = useState(false);
  const [form, setForm] = useState({
    name: "",
    type: "paper",
    version: "",
    port: "25565",
    motd: "",
    ram: "4096",
  });
  const [types, setTypes] = useState<{ id: string; name: string; desc: string }[]>([]);
  const [versions, setVersions] = useState<string[]>([]);

  // console
  const [lines, setLines] = useState<string[]>([]);
  const [cmd, setCmd] = useState("");
  const logRef = useRef<HTMLDivElement>(null);

  // properties
  const [props, setProps] = useState<Record<string, string>>({});
  const [propDraft, setPropDraft] = useState<Record<string, string>>({});

  // plugins
  const [plugins, setPlugins] = useState<PackFile[]>([]);
  const [plugQuery, setPlugQuery] = useState("");
  const [plugHits, setPlugHits] = useState<
    { slug?: string; title?: string; icon_url?: string; downloads?: number; description?: string }[]
  >([]);
  const [plugLoading, setPlugLoading] = useState(false);

  // files
  const [subpath, setSubpath] = useState("");
  const [files, setFiles] = useState<ServerFileEntry[]>([]);
  const [editing, setEditing] = useState<{ path: string; content: string } | null>(null);

  // backups / worlds
  const [backups, setBackups] = useState<{ filename: string; size?: number; modified?: number }[]>([]);
  const [worlds, setWorlds] = useState<string[]>([]);

  const refresh = useCallback(async () => {
    try {
      const r = await api.serverList();
      const list = r.servers ?? [];
      setServers(list);
      if (!selected && list.length > 0) setSelected(list[0].name);
    } catch {
      setServers([]);
    }
  }, [selected]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  useEffect(() => {
    if (creating) {
      void api.serverTypes().then((r) => {
        const t = (r as { types?: typeof types }).types ?? [];
        setTypes(t);
      });
    }
  }, [creating]);

  useEffect(() => {
    if (creating && form.type) {
      void api.serverVersions(form.type).then((r) => {
        setVersions(r.versions ?? []);
        if ((r.versions ?? []).length > 0 && !form.version) {
          setForm((f) => ({ ...f, version: r.versions![0] }));
        }
      });
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [creating, form.type]);

  // stats poll (only while a server is selected on list/console tabs)
  useEffect(() => {
    if (!selected) return;
    let stop = false;
    const tick = async () => {
      if (stop) return;
      try {
        const s = await api.serverStats(selected);
        if (!stop) setStats(s as unknown as ServerStats);
      } catch {
        /* ignore */
      }
    };
    void tick();
    const id = window.setInterval(tick, 4000);
    return () => {
      stop = true;
      window.clearInterval(id);
    };
  }, [selected]);

  // console log poll
  useEffect(() => {
    if (tab !== "console" || !selected) return;
    let stop = false;
    const tick = async () => {
      if (stop) return;
      try {
        const r = await api.serverLogs(selected);
        if (!stop && r.lines) setLines(r.lines);
      } catch {
        /* ignore */
      }
    };
    void tick();
    const id = window.setInterval(tick, 1500);
    return () => {
      stop = true;
      window.clearInterval(id);
    };
  }, [tab, selected]);

  useEffect(() => {
    if (logRef.current) logRef.current.scrollTop = logRef.current.scrollHeight;
  }, [lines]);

  // tab data loaders
  useEffect(() => {
    if (!selected) return;
    if (tab === "properties") {
      void api.serverGetProperties(selected).then((r) => {
        const p = r.properties ?? {};
        setProps(p);
        setPropDraft(p);
      });
    } else if (tab === "plugins") {
      void api.serverListPlugins(selected).then((r) => setPlugins(r.plugins ?? []));
      // Featured banner: auto-load most-downloaded plugins when the tab opens
      // with no search yet (empty query → downloads-sorted on the backend).
      if (plugHits.length === 0) {
        const mc = servers.find((s) => s.name === selected)?.version ?? "";
        setPlugLoading(true);
        void api
          .serverSearchPlugins("", mc, 12)
          .then((r) => {
            const h = (r as { hits?: typeof plugHits }).hits;
            if (Array.isArray(h) && h.length > 0) setPlugHits(h);
          })
          .catch(() => { /* keep the empty search state */ })
          .finally(() => setPlugLoading(false));
      }
    } else if (tab === "files") {
      setSubpath("");
      void api.serverFilesList(selected, "").then((r) => setFiles(r.entries ?? []));
    } else if (tab === "backups") {
      void api.serverBackups(selected).then((r) => setBackups(r.backups ?? []));
      void api.serverWorlds(selected).then((r) => setWorlds(r.worlds ?? []));
    }
  }, [tab, selected]);

  const loadFiles = async (sp: string) => {
    if (!selected) return;
    const r = await api.serverFilesList(selected, sp);
    setFiles(r.entries ?? []);
    setSubpath(sp);
  };

  const create = async () => {
    if (!form.name.trim() || !form.version) return notify("err", "Name and version are required.");
    setBusy(true);
    try {
      const r = await api.serverCreate(
        form.name.trim(),
        form.type,
        form.version,
        Number(form.port) || 25565,
        form.motd || undefined,
        Number(form.ram) || undefined,
      );
      if (r.error) notify("err", String(r.error));
      else {
        notify("ok", `Server "${form.name}" created.`);
        setCreating(false);
        setForm({ name: "", type: "paper", version: "", port: "25565", motd: "", ram: "4096" });
        await refresh();
        setSelected(form.name);
      }
    } catch (e) {
      notify("err", String(e));
    } finally {
      setBusy(false);
    }
  };

  const act = async (fn: () => Promise<{ error?: string }>, ok: string) => {
    if (!selected) return;
    setBusy(true);
    try {
      const r = await fn();
      if (r.error) notify("err", String(r.error));
      else notify("ok", ok);
      await refresh();
    } finally {
      setBusy(false);
    }
  };

  const sendCmd = async () => {
    if (!cmd.trim() || !selected) return;
    const r = await api.serverCommand(selected, cmd.trim());
    if (r.error) notify("err", String(r.error));
    setCmd("");
    try {
      const l = await api.serverLogs(selected);
      if (l.lines) setLines(l.lines);
    } catch {
      /* ignore */
    }
  };

  const saveProps = async () => {
    if (!selected) return;
    const changed: Record<string, string> = {};
    for (const k of Object.keys(propDraft)) {
      if (propDraft[k] !== props[k]) changed[k] = propDraft[k];
    }
    if (Object.keys(changed).length === 0) return notify("ok", "No changes.");
    const r = await api.serverSetProperties(selected, changed);
    if (r.error) notify("err", String(r.error));
    else notify("ok", "Properties saved.");
    setProps(propDraft);
  };

  const searchPlugins = async (q?: string) => {
    if (!selected) return;
    const query = (q ?? plugQuery).trim();
    const mc = servers.find((s) => s.name === selected)?.version ?? "";
    setPlugLoading(true);
    try {
      // empty query = featured (backend sorts by downloads)
      const r = await api.serverSearchPlugins(query, mc, 12);
      const hits = (r as { hits?: typeof plugHits }).hits;
      setPlugHits(Array.isArray(hits) ? hits : []);
    } finally {
      setPlugLoading(false);
    }
  };

  const installPlugin = async (slug: string) => {
    if (!selected) return;
    setBusy(true);
    try {
      const r = await api.serverInstallPlugin(selected, slug, "");
      if (r.error) notify("err", String(r.error));
      else {
        notify("ok", `Installed ${slug}`);
        const pl = await api.serverListPlugins(selected);
        setPlugins(pl.plugins ?? []);
      }
    } finally {
      setBusy(false);
    }
  };

  const openFile = async (e: ServerFileEntry) => {
    if (!selected) return;
    const path = subpath ? `${subpath}/${e.name}` : e.name;
    if (e.is_dir) return void loadFiles(path);
    const r = await api.serverFileRead(selected, path);
    if (r.error) notify("err", r.error);
    else setEditing({ path, content: r.content ?? "" });
  };

  const saveFile = async () => {
    if (!selected || !editing) return;
    const r = await api.serverFileWrite(selected, editing.path, editing.content);
    if (r.error) notify("err", String(r.error));
    else {
      notify("ok", "Saved.");
      setEditing(null);
    }
  };

  const selectedServer = servers.find((s) => s.name === selected);

  const fmtSize = (n?: number) =>
    !n ? "0 B" : n > 1e6 ? `${(n / 1e6).toFixed(1)} MB` : n > 1e3 ? `${(n / 1e3).toFixed(1)} KB` : `${n} B`;

  return (
    <div className="page scroll">
      {/* ---------- hero (homepage-style) ---------- */}
      <section className="sm-hero">
        <div className="vp-grid" aria-hidden />
        <div className="sm-hero-body">
          <div className="sm-hero-txt">
            <div className="tinycaps">Self-hosted</div>
            <h2>Server Manager</h2>
            <p className="sm-hero-sub">
              Spin up Paper servers, drop in plugins, watch the live console —
              all from inside Isolate.
            </p>
          </div>
          <div className="sm-hero-actions">
            <span className={`sm-chip ${selected && stats?.running ? "on" : ""}`}>
              <span className={`led ${selected && stats?.running ? "on" : ""}`} />
              {selected
                ? stats?.running
                  ? `${selected} — running`
                  : `${selected} — stopped`
                : servers.length > 0
                  ? "Pick a server below"
                  : "No servers yet"}
            </span>
            <button className="btn primary" onClick={() => setCreating((v) => !v)}>
              {creating ? "Cancel" : servers.length === 0 ? "Create server" : "+ New server"}
            </button>
          </div>
        </div>
      </section>

      {creating && (
        <div className="panel" style={{ marginBottom: 14 }}>
          <strong style={{ fontSize: 13 }}>Create a server</strong>
          <div className="row" style={{ gap: 8, marginTop: 10, flexWrap: "wrap" }}>
            <input
              type="text"
              placeholder="Name"
              value={form.name}
              onChange={(e) => setForm({ ...form, name: e.target.value })}
              style={{ flex: 2, minWidth: 140 }}
            />
            <select
              value={form.type}
              onChange={(e) => setForm({ ...form, type: e.target.value, version: "" })}
              style={{ flex: 1, minWidth: 120 }}
            >
              {(types.length > 0 ? types : [{ id: "paper", name: "Paper", desc: "" }]).map((t) => (
                <option key={t.id} value={t.id}>
                  {t.name}
                </option>
              ))}
            </select>
            <select
              value={form.version}
              onChange={(e) => setForm({ ...form, version: e.target.value })}
              style={{ flex: 1, minWidth: 130 }}
            >
              <option value="">Version…</option>
              {versions.map((v) => (
                <option key={v} value={v}>
                  {v}
                </option>
              ))}
            </select>
            <input
              type="number"
              value={form.port}
              onChange={(e) => setForm({ ...form, port: e.target.value })}
              style={{ width: 100 }}
              title="Port"
            />
            <input
              type="number"
              value={form.ram}
              onChange={(e) => setForm({ ...form, ram: e.target.value })}
              style={{ width: 110 }}
              title="RAM (MB)"
            />
            <input
              type="text"
              placeholder="MOTD (optional)"
              value={form.motd}
              onChange={(e) => setForm({ ...form, motd: e.target.value })}
              style={{ flex: 2, minWidth: 150 }}
            />
            <button className="btn primary" disabled={busy} onClick={() => void create()}>
              {busy ? "Creating…" : "Create"}
            </button>
          </div>
        </div>
      )}

      {/* ---------- server cards (select replacement) ---------- */}
      {servers.length > 0 && (
        <div className="cards" style={{ marginBottom: 14 }}>
          {servers.map((s) => (
            <button
              key={s.name}
              type="button"
              className={`card srv-card${selected === s.name ? " on" : ""}`}
              onClick={() => setSelected(s.name)}
            >
              <div className="row between">
                <div className="row" style={{ gap: 8 }}>
                  <span className={`led ${s.running ? "on" : ""}`} />
                  <span className="title">{s.name}</span>
                </div>
                <span className={`badge ${s.running ? "ok" : "grey"}`}>
                  {s.running ? "Running" : "Stopped"}
                </span>
              </div>
              <div className="meta">
                <span className="badge grey">{(s.type ?? "server").toUpperCase()}</span>
                <span className="badge grey">MC {s.version ?? "—"}</span>
                <span className="badge grey">:{s.port ?? 25565}</span>
                <span className="badge grey">{s.ram_mb ?? 4096} MB</span>
              </div>
              <div className="playtime">
                {selected === s.name ? "Managing — see tabs below" : "Click to manage"}
              </div>
            </button>
          ))}
          <button
            type="button"
            className={`card srv-card new${creating ? " on" : ""}`}
            onClick={() => setCreating((v) => !v)}
          >
            <div className="title" style={{ color: "var(--accent-2)" }}>
              {creating ? "Cancel" : "+ New server"}
            </div>
            <div className="playtime">Paper · Purpur · Fabric · Vanilla…</div>
          </button>
        </div>
      )}

      {!selected ? (
        <div className="empty">
          {servers.length === 0
            ? "No self-hosted servers yet — create one above."
            : "Select a server card above to manage it."}
        </div>
      ) : (
        <>
          <div className="row between" style={{ marginBottom: 12, flexWrap: "wrap", gap: 8 }}>
            <div className="tabs" style={{ marginBottom: 0 }}>
              {(["list", "console", "properties", "plugins", "files", "backups"] as Tab[]).map((t) => (
                <button key={t} className={tab === t ? "active" : ""} onClick={() => setTab(t)}>
                  {t[0].toUpperCase() + t.slice(1)}
                </button>
              ))}
            </div>
            <div className="row" style={{ gap: 6 }}>
              <span className={`badge ${stats?.running ? "ok" : "grey"}`}>
                {stats?.running ? "Running" : "Stopped"}
              </span>
              <button
                className="btn sm primary"
                disabled={busy || stats?.running}
                onClick={() => void act(() => api.serverStart(selected), "Server starting…")}
              >
                Start
              </button>
              <button
                className="btn sm"
                disabled={busy || !stats?.running}
                onClick={() => void act(() => api.serverStop(selected), "Stopping…")}
              >
                Stop
              </button>
              <button
                className="btn sm danger"
                disabled={busy || !stats?.running}
                onClick={() => void act(() => api.serverKill(selected), "Killed.")}
              >
                Kill
              </button>
            </div>
          </div>

          {tab === "list" && (
            <div className="cards">
              <div className="stat">
                <div className="k">Type</div>
                <div className="v">{selectedServer?.type ?? "—"}</div>
              </div>
              <div className="stat">
                <div className="k">Minecraft</div>
                <div className="v">{selectedServer?.version ?? "—"}</div>
              </div>
              <div className="stat">
                <div className="k">Port</div>
                <div className="v">{selectedServer?.port ?? "—"}</div>
              </div>
              <div className="stat">
                <div className="k">RAM</div>
                <div className="v">{selectedServer?.ram_mb ?? 4096} MB</div>
              </div>
              <div className="stat">
                <div className="k">Uptime</div>
                <div className="v">
                  {stats?.running
                    ? `${Math.floor((stats.uptime_secs ?? 0) / 60)}m`
                    : "—"}
                </div>
              </div>
              <div className="stat">
                <div className="k">Memory</div>
                <div className="v">
                  {stats?.running ? `${Math.round((stats.memory_kb ?? 0) / 1024)} MB` : "—"}
                </div>
              </div>

              <div className="card" style={{ gridColumn: "1 / -1" }}>
                <div className="title">MOTD / RAM</div>
                <div className="row" style={{ gap: 8, marginTop: 10, flexWrap: "wrap" }}>
                  <input
                    type="text"
                    defaultValue={selectedServer?.motd ?? ""}
                    placeholder="MOTD"
                    id="motd-input"
                    style={{ flex: 2, minWidth: 180 }}
                    onKeyDown={async (e) => {
                      if (e.key === "Enter") {
                        const v = (e.target as HTMLInputElement).value;
                        await act(() => api.serverSetMeta(selected, null, v, null), "MOTD updated.");
                      }
                    }}
                  />
                  <input
                    type="number"
                    defaultValue={selectedServer?.ram_mb ?? 4096}
                    placeholder="RAM MB"
                    style={{ width: 120 }}
                    onKeyDown={async (e) => {
                      if (e.key === "Enter") {
                        const v = Number((e.target as HTMLInputElement).value);
                        await act(() => api.serverSetMeta(selected, v, null, null), "RAM updated.");
                      }
                    }}
                  />
                  <button
                    className="btn sm"
                    onClick={() => {
                      const m = document.getElementById("motd-input") as HTMLInputElement | null;
                      const v = m?.value ?? "";
                      void act(() => api.serverSetMeta(selected, null, v, null), "MOTD updated.");
                    }}
                  >
                    Save MOTD
                  </button>
                </div>
              </div>

              <div className="card" style={{ gridColumn: "1 / -1" }}>
                <div className="row between">
                  <div>
                    <div className="title">Danger zone</div>
                    <div className="faint" style={{ fontSize: 12, marginTop: 4 }}>
                      Deleting removes the folder, worlds and backups permanently.
                    </div>
                  </div>
                  <div className="row" style={{ gap: 6 }}>
                    <button
                      className="btn sm"
                      disabled={busy}
                      onClick={async () => {
                        const nn = window.prompt("Clone to name:");
                        if (!nn) return;
                        const r = await api.serverClone(selected, nn, null);
                        if (r.error) notify("err", String(r.error));
                        else {
                          notify("ok", `Cloned to ${nn}`);
                          await refresh();
                        }
                      }}
                    >
                      Clone
                    </button>
                    <button
                      className="btn sm danger"
                      disabled={busy || stats?.running}
                      onClick={async () => {
                        if (!window.prompt(`Type the server name "${selected}" to confirm deletion:`)?.trim())
                          return;
                        await act(() => api.serverDelete(selected), "Server deleted.");
                        setSelected("");
                        await refresh();
                      }}
                    >
                      Delete
                    </button>
                  </div>
                </div>
              </div>
            </div>
          )}

          {tab === "console" && (
            <div className="panel" style={{ display: "flex", flexDirection: "column", gap: 10 }}>
              <div
                className="console scroll"
                ref={logRef}
                style={{ height: 340, overflowY: "auto", fontFamily: "var(--mono)", fontSize: 12, whiteSpace: "pre-wrap" }}
              >
                {lines.length === 0 ? (
                  <span className="faint">No output yet — start the server or send a command.</span>
                ) : (
                  lines.map((l, i) => <div key={i}>{l}</div>)
                )}
              </div>
              <div className="row" style={{ gap: 8 }}>
                <input
                  type="text"
                  placeholder="Command (without /)…"
                  value={cmd}
                  onChange={(e) => setCmd(e.target.value)}
                  onKeyDown={(e) => e.key === "Enter" && void sendCmd()}
                  style={{ flex: 1, fontFamily: "var(--mono)" }}
                />
                <button className="btn primary" onClick={() => void sendCmd()}>
                  Send
                </button>
                <button
                  className="btn sm"
                  onClick={async () => {
                    await api.serverClearLogs(selected);
                    setLines([]);
                  }}
                >
                  Clear
                </button>
              </div>
            </div>
          )}

          {tab === "properties" && (
            <>
              <div className="row" style={{ marginBottom: 10 }}>
                <button className="btn primary sm" onClick={() => void saveProps()}>
                  Save changes
                </button>
              </div>
              <div className="panel">
                {Object.keys(propDraft).length === 0 ? (
                  <span className="faint">server.properties not found or empty.</span>
                ) : (
                  Object.keys(propDraft)
                    .sort()
                    .map((k) => (
                      <div key={k} className="row between" style={{ padding: "6px 0", borderBottom: "1px solid var(--border)" }}>
                        <span className="mono" style={{ fontSize: 12, color: "var(--muted)" }}>
                          {k}
                        </span>
                        <input
                          type="text"
                          value={propDraft[k]}
                          onChange={(e) => setPropDraft({ ...propDraft, [k]: e.target.value })}
                          style={{ maxWidth: 320, textAlign: "right" }}
                        />
                      </div>
                    ))
                )}
              </div>
            </>
          )}

          {tab === "plugins" && (
            <>
              {/* featured plugins banner */}
              <div className="plug-banner">
                <div className="plug-banner-txt">
                  <div className="tinycaps" style={{ color: "rgba(255,255,255,0.55)" }}>
                    Modrinth · Paper
                  </div>
                  <strong>{plugQuery.trim() ? "Search results" : "Featured plugins"}</strong>
                  <span className="faint" style={{ fontSize: 12 }}>
                    {plugLoading
                      ? "Loading…"
                      : plugQuery.trim()
                        ? `Matches for “${plugQuery.trim()}”`
                        : "Most-downloaded plugins for your server — one click to install."}
                  </span>
                </div>
                <div className="plug-banner-logos" aria-hidden>
                  {plugHits
                    .filter((h) => h.icon_url)
                    .slice(0, 5)
                    .map((h, i) => (
                      <img key={h.slug ?? i} src={h.icon_url} alt="" />
                    ))}
                </div>
              </div>

              <div className="row" style={{ gap: 8, marginBottom: 12 }}>
                <input
                  type="text"
                  placeholder="Search plugins on Modrinth…"
                  value={plugQuery}
                  onChange={(e) => setPlugQuery(e.target.value)}
                  onKeyDown={(e) => e.key === "Enter" && void searchPlugins()}
                  style={{ flex: 1 }}
                />
                <button className="btn primary" disabled={plugLoading} onClick={() => void searchPlugins()}>
                  {plugLoading ? "Searching…" : "Search"}
                </button>
                {plugQuery.trim() && (
                  <button
                    className="btn"
                    onClick={() => {
                      setPlugQuery("");
                      void searchPlugins("");
                    }}
                  >
                    Featured
                  </button>
                )}
              </div>

              {plugHits.length > 0 && (
                <div className="cards" style={{ marginBottom: 14 }}>
                  {plugHits.map((h) => (
                    <div className="card plug-card" key={h.slug}>
                      <div className="row" style={{ gap: 10 }}>
                        {h.icon_url ? (
                          <img className="plug-logo" src={h.icon_url} alt="" />
                        ) : (
                          <div className="plug-logo plug-logo-fb">
                            {(h.title ?? h.slug ?? "?").slice(0, 1).toUpperCase()}
                          </div>
                        )}
                        <div style={{ minWidth: 0, flex: 1 }}>
                          <div className="title">{h.title ?? h.slug}</div>
                          <div className="faint" style={{ fontSize: 11 }}>
                            {(h.downloads ?? 0).toLocaleString()} downloads
                          </div>
                        </div>
                      </div>
                      {(h.description ?? "") !== "" && (
                        <div className="faint" style={{ fontSize: 12, margin: "8px 0 0", lineHeight: 1.45 }}>
                          {(h.description ?? "").slice(0, 110)}
                          {(h.description ?? "").length > 110 ? "…" : ""}
                        </div>
                      )}
                      <div className="actions">
                        <button
                          className="btn primary sm"
                          style={{ flex: 1, justifyContent: "center" }}
                          disabled={busy}
                          onClick={() => void installPlugin(h.slug ?? "")}
                        >
                          Install
                        </button>
                      </div>
                    </div>
                  ))}
                </div>
              )}

              <div className="row between" style={{ marginBottom: 8 }}>
                <span className="faint">{plugins.length} plugin(s)</span>
                <button className="btn sm" onClick={() => void api.serverListPlugins(selected).then((r) => setPlugins(r.plugins ?? []))}>
                  Refresh
                </button>
              </div>
              <div className="panel">
                {plugins.length === 0 ? (
                  <span className="faint">No plugins installed.</span>
                ) : (
                  plugins.map((p) => (
                    <div key={p.filename} className="row between" style={{ padding: "8px 0", borderBottom: "1px solid var(--border)" }}>
                      <div className="row" style={{ gap: 10 }}>
                        <span className={`badge ${p.enabled ? "ok" : "grey"}`}>{p.enabled ? "On" : "Off"}</span>
                        <span style={{ fontSize: 13 }}>{p.filename.replace(/\.jar(\.disabled)?$/i, "")}</span>
                      </div>
                      <div className="row" style={{ gap: 6 }}>
                        <button
                          className="btn sm"
                          onClick={async () => {
                            await api.serverTogglePlugin(selected, p.filename);
                            const r = await api.serverListPlugins(selected);
                            setPlugins(r.plugins ?? []);
                          }}
                        >
                          {p.enabled ? "Disable" : "Enable"}
                        </button>
                        <button
                          className="btn sm danger"
                          onClick={async () => {
                            await api.serverDeletePlugin(selected, p.filename);
                            const r = await api.serverListPlugins(selected);
                            setPlugins(r.plugins ?? []);
                          }}
                        >
                          Delete
                        </button>
                      </div>
                    </div>
                  ))
                )}
              </div>
            </>
          )}

          {tab === "files" && (
            <>
              <div className="row" style={{ gap: 8, marginBottom: 10 }}>
                <button
                  className="btn sm"
                  disabled={!subpath}
                  onClick={() => {
                    const parts = subpath.split("/").filter(Boolean);
                    parts.pop();
                    void loadFiles(parts.join("/"));
                  }}
                >
                  ← Up
                </button>
                <span className="mono faint" style={{ fontSize: 12, flex: 1 }}>
                  /{subpath}
                </span>
              </div>

              {editing ? (
                <div className="panel">
                  <div className="row between" style={{ marginBottom: 8 }}>
                    <strong className="mono" style={{ fontSize: 13 }}>{editing.path}</strong>
                    <div className="row" style={{ gap: 6 }}>
                      <button className="btn sm primary" onClick={() => void saveFile()}>
                        Save
                      </button>
                      <button className="btn sm" onClick={() => setEditing(null)}>
                        Close
                      </button>
                    </div>
                  </div>
                  <textarea
                    value={editing.content}
                    onChange={(e) => setEditing({ ...editing, content: e.target.value })}
                    rows={18}
                    style={{ width: "100%", fontFamily: "var(--mono)", fontSize: 12 }}
                  />
                </div>
              ) : (
                <div className="panel">
                  {files.length === 0 ? (
                    <span className="faint">Empty folder.</span>
                  ) : (
                    files.map((f) => (
                      <div key={f.name} className="row between" style={{ padding: "7px 0", borderBottom: "1px solid var(--border)" }}>
                        <button
                          className="btn sm"
                          style={{ border: "none", background: "transparent", color: "var(--text)", justifyContent: "flex-start" }}
                          onClick={() => void openFile(f)}
                        >
                          {f.is_dir ? "📁" : "📄"} {f.name}
                        </button>
                        <div className="row" style={{ gap: 8 }}>
                          <span className="faint" style={{ fontSize: 11 }}>{fmtSize(f.size)}</span>
                          {!f.is_dir && (
                            <button
                              className="btn sm danger"
                              onClick={async () => {
                                const p = subpath ? `${subpath}/${f.name}` : f.name;
                                await api.serverFileDelete(selected, p);
                                await loadFiles(subpath);
                              }}
                            >
                              Delete
                            </button>
                          )}
                        </div>
                      </div>
                    ))
                  )}
                </div>
              )}
            </>
          )}

          {tab === "backups" && (
            <div className="cards">
              <div className="card" style={{ gridColumn: "1 / -1" }}>
                <div className="row between">
                  <div className="title">Backups</div>
                  <button
                    className="btn primary sm"
                    disabled={busy}
                    onClick={async () => {
                      setBusy(true);
                      try {
                        const r = await api.serverBackupCreate(selected);
                        if (r.error) notify("err", String(r.error));
                        else notify("ok", "Backup created.");
                        const b = await api.serverBackups(selected);
                        setBackups(b.backups ?? []);
                      } finally {
                        setBusy(false);
                      }
                    }}
                  >
                    Create backup
                  </button>
                </div>
                <div style={{ marginTop: 10 }}>
                  {backups.length === 0 ? (
                    <span className="faint" style={{ fontSize: 12 }}>No backups yet.</span>
                  ) : (
                    backups.map((b) => (
                      <div key={b.filename} className="row between" style={{ padding: "7px 0", borderBottom: "1px solid var(--border)" }}>
                        <div>
                          <div className="mono" style={{ fontSize: 12 }}>{b.filename}</div>
                          <div className="faint" style={{ fontSize: 11 }}>
                            {fmtSize(b.size)}
                            {b.modified ? ` · ${new Date(b.modified * 1000).toLocaleString()}` : ""}
                          </div>
                        </div>
                        <div className="row" style={{ gap: 6 }}>
                          <button
                            className="btn sm"
                            disabled={stats?.running}
                            title={stats?.running ? "Stop the server first" : ""}
                            onClick={async () => {
                              const r = await api.serverBackupRestore(selected, b.filename);
                              if (r.error) notify("err", String(r.error));
                              else notify("ok", "Backup restored.");
                            }}
                          >
                            Restore
                          </button>
                          <button
                            className="btn sm danger"
                            onClick={async () => {
                              await api.serverBackupDelete(selected, b.filename);
                              const r = await api.serverBackups(selected);
                              setBackups(r.backups ?? []);
                            }}
                          >
                            Delete
                          </button>
                        </div>
                      </div>
                    ))
                  )}
                </div>
              </div>

              <div className="card" style={{ gridColumn: "1 / -1" }}>
                <div className="title">Worlds</div>
                <div style={{ marginTop: 10 }}>
                  {worlds.length === 0 ? (
                    <span className="faint" style={{ fontSize: 12 }}>No worlds found (start the server once to generate one).</span>
                  ) : (
                    worlds.map((w) => (
                      <div key={w} className="row between" style={{ padding: "7px 0", borderBottom: "1px solid var(--border)" }}>
                        <span className="mono" style={{ fontSize: 13 }}>{w}</span>
                        <button
                          className="btn sm danger"
                          disabled={stats?.running}
                          title={stats?.running ? "Stop the server first" : ""}
                          onClick={async () => {
                            if (!window.prompt(`Type "${w}" to confirm world deletion:`)?.trim()) return;
                            const r = await api.serverDeleteWorld(selected, w);
                            if (r.error) notify("err", String(r.error));
                            else {
                              notify("ok", `Deleted ${w}`);
                              const wr = await api.serverWorlds(selected);
                              setWorlds(wr.worlds ?? []);
                            }
                          }}
                        >
                          Delete
                        </button>
                      </div>
                    ))
                  )}
                </div>
              </div>
            </div>
          )}
        </>
      )}
    </div>
  );
}

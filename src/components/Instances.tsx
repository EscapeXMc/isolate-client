import { useEffect, useMemo, useState } from "react";
import { useApp } from "../stores/app";
import { api } from "../lib/tauri";
import {
  createInstance,
  formatPlaytime,
  installInstance,
  playInstance,
  stopGame,
} from "../lib/actions";
import type { Instance, Loader, VersionGroup } from "../lib/types";
import { pfSrc } from "../lib/pfIcon";
import {
  ArrowUp,
  FolderOpen,
  MoreHorizontal,
  Play,
  Plus,
  Puzzle,
  Search,
  Settings2,
  Trash2,
  X,
} from "lucide-react";

const LOADERS: { id: Loader; label: string }[] = [
  { id: "vanilla", label: "Vanilla" },
  { id: "fabric", label: "Fabric" },
  { id: "quilt", label: "Quilt" },
];

// Profile-icon tint per loader — grayscale only (white/black palette).
const LOADER_TINT: Record<Loader, string> = {
  vanilla: "#6b6b6b",
  fabric: "#d4d4d4",
  quilt: "#3f3f46",
};

export default function Instances() {
  const instances = useApp((s) => s.instances);
  const installing = useApp((s) => s.installing);
  const gameRunning = useApp((s) => s.gameRunning);
  const playingInstance = useApp((s) => s.playingInstance);
  const launching = useApp((s) => s.launching);
  const refreshInstances = useApp((s) => s.refreshInstances);
  const notify = useApp((s) => s.notify);
  const setPage = useApp((s) => s.setPage);

  const [creating, setCreating] = useState(false);
  const [editing, setEditing] = useState<Instance | null>(null);
  const [confirming, setConfirming] = useState<string | null>(null);
  const [q, setQ] = useState("");
  const [sortAsc, setSortAsc] = useState(true);
  const [menu, setMenu] = useState<string | null>(null);

  useEffect(() => {
    void refreshInstances();
  }, [refreshInstances]);

  // FastClient-style "selected" profile: most recently played (else first ready).
  const selected = useMemo(() => {
    const pool = instances.filter((i) => (i.play_seconds ?? 0) > 0 || i.installed);
    if (!pool.length) return null;
    return [...pool].sort((a, b) => (b.play_seconds ?? 0) - (a.play_seconds ?? 0))[0]
      .name;
  }, [instances]);

  const shown = useMemo(() => {
    const term = q.trim().toLowerCase();
    const list = instances.filter(
      (i) =>
        !term ||
        i.name.toLowerCase().includes(term) ||
        i.version.toLowerCase().includes(term) ||
        i.loader.toLowerCase().includes(term),
    );
    const cmp = (x: string, y: string) =>
      x.localeCompare(y, undefined, { numeric: true });
    return [...list].sort((a, b) =>
      sortAsc ? cmp(a.version, b.version) : cmp(b.version, a.version),
    );
  }, [instances, q, sortAsc]);

  return (
    <div className="page scroll">
      <div className="row between" style={{ marginBottom: 14 }}>
        <div>
          <h1>Your Profiles</h1>
        </div>
      </div>

      {/* toolbar — search + version sort (FastClient) */}
      <div className="pf-toolbar">
        <div className="pf-search">
          <Search aria-hidden />
          <input
            value={q}
            onChange={(e) => setQ(e.target.value)}
            placeholder="Search profiles..."
          />
        </div>
        <button
          className="pf-sort"
          title="Sort by version"
          onClick={() => setSortAsc((v) => !v)}
        >
          <ArrowUp aria-hidden className={sortAsc ? "" : "flip"} />
          Version
        </button>
      </div>

      <div className="pf-grid">
        {shown.map((i) => {
          const prog = installing[i.name];
          const isSel = selected === i.name;
          const loaderLabel =
            LOADERS.find((l) => l.id === i.loader)?.label ?? i.loader;
          return (
            <div key={i.name} className={`pf-card${isSel ? " sel" : ""}`}>
              {isSel && (
                <span className="pf-check" aria-hidden>
                  ✓
                </span>
              )}
              <div className="pf-head">
                <span
                  className="pf-ico"
                  style={{ background: LOADER_TINT[i.loader] ?? "#3f3f46" }}
                  aria-hidden
                >
                  {i.name.slice(0, 1).toUpperCase()}
                  {pfSrc(i) && (
                    <img
                      src={pfSrc(i) as string}
                      alt=""
                      onError={(e) => (e.currentTarget.style.display = "none")}
                    />
                  )}
                </span>
                <div className="pf-meta">
                  <div className="pf-name">{i.name}</div>
                  <div className="pf-sub">
                    Isolate + {loaderLabel} {i.version}
                  </div>
                </div>
              </div>

              <div className="pf-tags">
                <span className="badge">{i.version}</span>
                <span className="badge grey">{loaderLabel}</span>
                <span className="pf-play">{formatPlaytime(i.play_seconds ?? 0)}</span>
              </div>

              {prog && (
                <div className="pf-prog">
                  <div className="bar">
                    <i style={{ width: `${Math.max(2, prog.pct)}%` }} />
                  </div>
                  <div className="faint" style={{ marginTop: 5 }}>
                    {prog.error ?? prog.message}
                  </div>
                </div>
              )}

              <div className="pf-actions">
                {i.installed ? (
                  (() => {
                    // This profile is launching or running → the button turns
                    // into "Close"; any other profile stays locked while a
                    // game is up.
                    const active =
                      launching === i.name ||
                      (gameRunning && playingInstance === i.name);
                    return (
                      <button
                        className={`pf-act primary${active ? " stop" : ""}`}
                        disabled={!active && (gameRunning || !!prog)}
                        onClick={() => {
                          if (!active) {
                            void playInstance(i);
                          } else if (gameRunning) {
                            void stopGame();
                          } else {
                            // still in the launch sequence — dismiss overlay
                            useApp.getState().setLaunching(null);
                            void api.stopGame().catch(() => undefined);
                          }
                        }}
                      >
                        {active ? (
                          <>
                            <X aria-hidden /> Close
                          </>
                        ) : (
                          <>
                            <Play aria-hidden /> Play
                          </>
                        )}
                      </button>
                    );
                  })()
                ) : (
                  <button
                    className="pf-act primary"
                    disabled={!!prog}
                    onClick={() => void installInstance(i.name)}
                  >
                    {prog ? "Installing…" : "Install"}
                  </button>
                )}
                <button
                  className="pf-act"
                  title="Mods"
                  onClick={() => setPage("mods")}
                >
                  <Puzzle aria-hidden /> Mods
                </button>
                <button
                  className="pf-act icon"
                  title="More actions"
                  onClick={() => setMenu(menu === i.name ? null : i.name)}
                >
                  <MoreHorizontal aria-hidden />
                </button>
              </div>

              {menu === i.name && (
                <>
                  <div className="menu-back" onClick={() => setMenu(null)} />
                  <div className="pf-menu">
                    <button
                      onClick={() => {
                        setMenu(null);
                        void api.openInstanceFolder(i.name);
                      }}
                    >
                      <FolderOpen aria-hidden /> Open Folder
                    </button>
                    <button
                      onClick={() => {
                        setMenu(null);
                        setEditing(i);
                      }}
                    >
                      <Settings2 aria-hidden /> Profile Settings
                    </button>
                    <button
                      className="danger"
                      onClick={() => {
                        setMenu(null);
                        setConfirming(i.name);
                      }}
                    >
                      <Trash2 aria-hidden /> Delete
                    </button>
                  </div>
                </>
              )}
            </div>
          );
        })}

        {/* create profile card (FastClient) */}
        <button className="pf-create" onClick={() => setCreating(true)}>
          <Plus aria-hidden />
          <span>Create Profile</span>
        </button>
      </div>

      {instances.length > 0 && shown.length === 0 && (
        <div className="empty">
          <div className="big">No profiles match</div>
          Nothing found for “{q}”.
        </div>
      )}

      {creating && (
        <CreateModal
          close={() => setCreating(false)}
          onCreated={async (name) => {
            setCreating(false);
            await refreshInstances();
            notify("ok", `Created "${name}"`);
          }}
        />
      )}

      {editing && (
        <InstanceSettingsModal
          instance={editing}
          close={() => setEditing(null)}
          onSaved={async () => {
            setEditing(null);
            await refreshInstances();
            notify("ok", "Instance settings saved");
          }}
        />
      )}

      {confirming && (
        <div className="overlay" onClick={() => setConfirming(null)}>
          <div className="modal" style={{ maxWidth: 420 }} onClick={(e) => e.stopPropagation()}>
            <header>
              <h2>Delete instance?</h2>
            </header>
            <div className="body">
              <p style={{ margin: 0 }}>
                <strong>{confirming}</strong> and all of its saves, mods and worlds will be
                permanently removed.
              </p>
            </div>
            <footer>
              <button className="btn ghost" onClick={() => setConfirming(null)}>
                Cancel
              </button>
              <button
                className="btn danger"
                onClick={async () => {
                  const r = await api.deleteInstance(confirming);
                  setConfirming(null);
                  if (r.error) notify("err", r.error);
                  else {
                    await refreshInstances();
                    notify("ok", "Instance deleted");
                  }
                }}
              >
                Delete
              </button>
            </footer>
          </div>
        </div>
      )}
    </div>
  );
}

/* ---------------- create modal ---------------- */

function CreateModal({
  close,
  onCreated,
}: {
  close: () => void;
  onCreated: (name: string) => void | Promise<void>;
}) {
  const notify = useApp((s) => s.notify);

  const [name, setName] = useState("");
  const [loader, setLoader] = useState<Loader>("vanilla");
  const [groups, setGroups] = useState<VersionGroup[]>([]);
  const [group, setGroup] = useState("");
  const [version, setVersion] = useState("");
  const [loaderVersions, setLoaderVersions] = useState<string[]>([]);
  const [loaderVersion, setLoaderVersion] = useState("");
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState<string | null>(null);

  // Load the version catalog once.
  useEffect(() => {
    let alive = true;
    void (async () => {
      const r = await api.getMinecraftVersions(false);
      if (!alive) return;
      setLoading(false);
      if (r.error || !r.versions?.length) {
        setErr(r.error ?? "Could not load the Minecraft version list.");
        return;
      }
      setGroups(r.versions);
      const g = r.versions[0]?.group ?? "";
      setGroup(g);
      const v = r.versions[0]?.versions[0] ?? "";
      setVersion(v);
    })();
    return () => {
      alive = false;
    };
  }, []);

  // Load loader versions when the loader changes.
  useEffect(() => {
    if (loader === "vanilla") {
      setLoaderVersions([]);
      setLoaderVersion("");
      return;
    }
    let alive = true;
    void (async () => {
      const r = await api.getLoaderVersions(loader);
      if (!alive) return;
      const list = r.versions ?? [];
      setLoaderVersions(list);
      setLoaderVersion(list[0] ?? "");
    })();
    return () => {
      alive = false;
    };
  }, [loader]);

  const currentGroup = useMemo(
    () => groups.find((g) => g.group === group),
    [groups, group],
  );

  async function submit() {
    if (!name.trim()) return setErr("Give the instance a name.");
    if (!version) return setErr("Pick a Minecraft version.");
    if (loader !== "vanilla" && !loaderVersion) return setErr("Pick a loader version.");
    setBusy(true);
    setErr(null);
    const e = await createInstance(name.trim(), version, loader, loaderVersion || "");
    setBusy(false);
    if (e) return setErr(e);
    await onCreated(name.trim());
  }

  return (
    <div className="overlay" onClick={close}>
      <div className="modal wide" onClick={(e) => e.stopPropagation()}>
        <header>
          <h2>New instance</h2>
          <button className="iconbtn" onClick={close}>
            ✕
          </button>
        </header>

        <div className="body">
          <label className="field">
            <span>Name</span>
            <input
              type="text"
              value={name}
              placeholder="My Pack"
              autoFocus
              onChange={(e) => setName(e.target.value)}
            />
          </label>

          <label className="field">
            <span>Mod loader</span>
            <select value={loader} onChange={(e) => setLoader(e.target.value as Loader)}>
              {LOADERS.map((l) => (
                <option key={l.id} value={l.id}>
                  {l.label}
                </option>
              ))}
            </select>
          </label>

          <div className="field">
            <span style={{ display: "block", fontSize: 12, fontWeight: 600, color: "var(--muted)", marginBottom: 6 }}>
              Version
            </span>

            {loading ? (
              <div className="picker">
                <div className="col" style={{ display: "flex", alignItems: "center", justifyContent: "center", color: "var(--faint)" }}>
                  Loading versions…
                </div>
              </div>
            ) : groups.length === 0 ? (
              <div className="hint">Version list unavailable — check your connection.</div>
            ) : (
              <div className="picker">
                <div className="col" style={{ maxWidth: 130 }}>
                  {groups.map((g) => (
                    <div
                      key={g.group}
                      className={`item ${g.group === group ? "on" : ""}`}
                      onClick={() => {
                        setGroup(g.group);
                        setVersion(g.versions[0] ?? "");
                      }}
                    >
                      {g.group}
                    </div>
                  ))}
                </div>
                <div className="col">
                  {(currentGroup?.versions ?? []).map((v) => (
                    <div
                      key={v}
                      className={`item ${v === version ? "on" : ""}`}
                      onClick={() => setVersion(v)}
                    >
                      {v}
                    </div>
                  ))}
                </div>
                {loader !== "vanilla" && (
                  <>
                    <div className="col" style={{ maxWidth: 150 }}>
                      <div className="head">{loader} version</div>
                      {loaderVersions.map((v) => (
                        <div
                          key={v}
                          className={`item ${v === loaderVersion ? "on" : ""}`}
                          onClick={() => setLoaderVersion(v)}
                        >
                          {v}
                        </div>
                      ))}
                      {loaderVersions.length === 0 && (
                        <div className="faint" style={{ padding: 9 }}>
                          None available
                        </div>
                      )}
                    </div>
                  </>
                )}
              </div>
            )}

            <div className="hint">
              {version
                ? `Minecraft ${version}${loader !== "vanilla" ? ` · ${loader} ${loaderVersion || "—"}` : ""}`
                : "Select a version"}
            </div>
          </div>

          {err && (
            <div style={{ color: "var(--danger)", fontSize: 13, marginTop: 4 }}>{err}</div>
          )}
        </div>

        <footer>
          <button className="btn ghost" onClick={close}>
            Cancel
          </button>
          <button className="btn primary" disabled={busy} onClick={() => void submit()}>
            {busy ? "Creating…" : "Create"}
          </button>
        </footer>
      </div>
    </div>
  );
}

/* ---------------- per-instance settings ---------------- */

function InstanceSettingsModal({
  instance,
  close,
  onSaved,
}: {
  instance: Instance;
  close: () => void;
  onSaved: () => void | Promise<void>;
}) {
  const java = useApp((s) => s.java);

  const [ram, setRam] = useState(String(instance.ram_mb ?? 4096));
  const [jvm, setJvm] = useState(instance.jvm_args ?? "");
  const [javaPath, setJavaPath] = useState(instance.java_path ?? "");
  const [busy, setBusy] = useState(false);

  async function save() {
    setBusy(true);
    await api.saveInstanceSettings(instance.name, {
      ram_mb: Math.max(512, Number(ram) || 4096),
      jvm_args: jvm,
      java_path: javaPath,
    });
    setBusy(false);
    await onSaved();
  }

  return (
    <div className="overlay" onClick={close}>
      <div className="modal" onClick={(e) => e.stopPropagation()}>
        <header>
          <h2>{instance.name}</h2>
          <button className="iconbtn" onClick={close}>
            ✕
          </button>
        </header>
        <div className="body">
          <div className="meta row wrap" style={{ marginBottom: 16 }}>
            <span className="badge">{instance.version}</span>
            <span className="badge grey">{instance.loader}</span>
          </div>

          <label className="field">
            <span>Memory (MB)</span>
            <input
              type="number"
              min={512}
              step={512}
              value={ram}
              onChange={(e) => setRam(e.target.value)}
            />
            <div className="hint">Leave empty behaviour to default by typing 4096.</div>
          </label>

          <label className="field">
            <span>Extra JVM arguments</span>
            <input
              type="text"
              value={jvm}
              placeholder="-XX:+UseG1GC"
              onChange={(e) => setJvm(e.target.value)}
            />
          </label>

          <label className="field">
            <span>Java</span>
            <select value={javaPath} onChange={(e) => setJavaPath(e.target.value)}>
              <option value="">Auto-detect</option>
              {java.map((j) => (
                <option key={j.path} value={j.path}>
                  Java {j.major} — {j.version}
                </option>
              ))}
            </select>
          </label>
        </div>
        <footer>
          <button className="btn ghost" onClick={close}>
            Cancel
          </button>
          <button className="btn primary" disabled={busy} onClick={() => void save()}>
            {busy ? "Saving…" : "Save"}
          </button>
        </footer>
      </div>
    </div>
  );
}

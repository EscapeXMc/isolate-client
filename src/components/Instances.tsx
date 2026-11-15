import { useEffect, useMemo, useState } from "react";
import { useApp } from "../stores/app";
import { api } from "../lib/tauri";
import {
  createInstance,
  formatPlaytime,
  installInstance,
  playInstance,
} from "../lib/actions";
import type { Instance, Loader, VersionGroup } from "../lib/types";

const LOADERS: { id: Loader; label: string }[] = [
  { id: "vanilla", label: "Vanilla" },
  { id: "fabric", label: "Fabric" },
  { id: "quilt", label: "Quilt" },
];

export default function Instances() {
  const instances = useApp((s) => s.instances);
  const installing = useApp((s) => s.installing);
  const gameRunning = useApp((s) => s.gameRunning);
  const refreshInstances = useApp((s) => s.refreshInstances);
  const notify = useApp((s) => s.notify);

  const [creating, setCreating] = useState(false);
  const [editing, setEditing] = useState<Instance | null>(null);
  const [confirming, setConfirming] = useState<string | null>(null);

  useEffect(() => {
    void refreshInstances();
  }, [refreshInstances]);

  return (
    <div className="page scroll">
      <div className="row between" style={{ marginBottom: 18 }}>
        <div>
          <h1>Instances</h1>
          <div className="faint">{instances.length} total</div>
        </div>
        <button className="btn primary" onClick={() => setCreating(true)}>
          + New instance
        </button>
      </div>

      {instances.length === 0 ? (
        <div className="empty">
          <div className="big">Nothing here yet</div>
          Create an instance, pick a version, then install it.
          <div style={{ marginTop: 16 }}>
            <button className="btn primary" onClick={() => setCreating(true)}>
              + New instance
            </button>
          </div>
        </div>
      ) : (
        <div className="cards">
          {instances.map((i) => {
            const prog = installing[i.name];
            return (
              <div key={i.name} className="card">
                <div className="row between">
                  <div className="title">{i.name}</div>
                  {i.installed ? (
                    <span className="badge ok">Ready</span>
                  ) : (
                    <span className="badge warn">Not installed</span>
                  )}
                </div>

                <div className="meta">
                  <span className="badge">{i.version}</span>
                  <span className="badge grey">{i.loader}</span>
                  {i.loader !== "vanilla" && i.loader_version && (
                    <span className="badge grey">{i.loader_version}</span>
                  )}
                </div>

                {i.ram_mb ? (
                  <div className="faint">{i.ram_mb} MB allocated</div>
                ) : (
                  <div className="faint">Default memory</div>
                )}

                <div className="playtime">{formatPlaytime(i.play_seconds ?? 0)}</div>

                {prog && (
                  <div>
                    <div className="bar">
                      <i style={{ width: `${Math.max(2, prog.pct)}%` }} />
                    </div>
                    <div className="faint" style={{ marginTop: 5 }}>
                      {prog.error ?? prog.message}
                    </div>
                  </div>
                )}

                <div className="actions">
                  {i.installed ? (
                    <button
                      className="btn primary sm"
                      style={{ flex: 1, justifyContent: "center" }}
                      disabled={gameRunning || !!prog}
                      onClick={() => void playInstance(i)}
                    >
                      Play
                    </button>
                  ) : (
                    <button
                      className="btn sm"
                      style={{ flex: 1, justifyContent: "center" }}
                      disabled={!!prog}
                      onClick={() => void installInstance(i.name)}
                    >
                      {prog ? "Installing…" : "Install"}
                    </button>
                  )}
                  <button className="btn sm" title="Open folder" onClick={() => void api.openInstanceFolder(i.name)}>
                    📁
                  </button>
                  <button className="btn sm" title="Settings" onClick={() => setEditing(i)}>
                    ⚙
                  </button>
                  <button
                    className="btn sm danger"
                    title="Delete"
                    onClick={() => setConfirming(i.name)}
                  >
                    ✕
                  </button>
                </div>
              </div>
            );
          })}
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

import { useEffect, useMemo, useState } from "react";
import { marked } from "marked";
import {
  Copy,
  ExternalLink,
  Globe,
  Handshake,
  LogOut,
  Pencil,
  Plus,
  Trash2,
} from "lucide-react";
import { useApp } from "../stores/app";
import { api } from "../lib/tauri";
import {
  FALLBACK_PARTNERS,
  fbDeletePartner,
  fbIsAdmin,
  fbSavePartner,
  fbSignOut,
  slugId,
} from "../lib/firebase";
import type { PartnerServer } from "../lib/types";

const EMPTY_SERVER: PartnerServer = {
  id: "",
  name: "",
  ip: "",
  summary: "",
  description: "",
  tags: [],
  banner: "",
  logo: "",
  website: "",
  discord: "",
  countMode: "live",
  players: 0,
  playersTotal: 0,
  color: "#3b82f6",
  order: 0,
  enabled: true,
};

function Markdown({ source }: { source: string }) {
  const html = useMemo(() => {
    try {
      const r = marked.parse(source || "", { gfm: true, breaks: true });
      return typeof r === "string" ? r : "";
    } catch {
      return "";
    }
  }, [source]);
  if (!html) return null;
  return <div className="md" dangerouslySetInnerHTML={{ __html: html }} />;
}

export default function Partnerships() {
  const partners = useApp((s) => s.partners);
  const partnersLive = useApp((s) => s.partnersLive);
  const refreshPartners = useApp((s) => s.refreshPartners);
  const notify = useApp((s) => s.notify);

  const [admin, setAdmin] = useState(false);

  const [editing, setEditing] = useState<PartnerServer | null>(null);
  const [isNew, setIsNew] = useState(false);
  const [saving, setSaving] = useState(false);
  const [confirmId, setConfirmId] = useState("");

  // Re-check the admin session when the tab regains focus (sign-in may have
  // happened on the Accounts page, or the token may have refreshed).
  useEffect(() => {
    let alive = true;
    const sync = () => void fbIsAdmin().then((a) => alive && setAdmin(a));
    sync();
    window.addEventListener("focus", sync);
    return () => {
      alive = false;
      window.removeEventListener("focus", sync);
    };
  }, []);

  const list = partnersLive ? partners : FALLBACK_PARTNERS;

  // Live player counts — ping every live-mode server every 30s.
  const [pings, setPings] = useState<Record<string, { online?: number; max?: number }>>({});
  useEffect(() => {
    let alive = true;
    const run = async () => {
      const targets = list.filter((p) => (p.countMode ?? "live") === "live" && p.ip);
      const next: Record<string, { online?: number; max?: number }> = {};
      await Promise.all(
        targets.map(async (p) => {
          try {
            const [host, portStr] = p.ip.split(":");
            const r = await api.pingServer(host, Number(portStr) || 25565);
            const j = r as { online?: number; max?: number; success?: boolean };
            if (j.success !== false) next[p.id] = { online: j.online, max: j.max };
          } catch {
            /* offline server — keep old value */
          }
        }),
      );
      if (alive && Object.keys(next).length) setPings((prev) => ({ ...prev, ...next }));
    };
    void run();
    const t = window.setInterval(() => void run(), 30_000);
    return () => {
      alive = false;
      window.clearInterval(t);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [partnersLive, partners.length]);

  const logout = () => {
    fbSignOut();
    setAdmin(false);
    setEditing(null);
    notify("ok", "Signed out");
  };

  const startNew = () => {
    setEditing({ ...EMPTY_SERVER });
    setIsNew(true);
  };

  const startEdit = (p: PartnerServer) => {
    setEditing({ ...p, tags: [...(p.tags ?? [])] });
    setIsNew(false);
  };

  const setField = <K extends keyof PartnerServer>(k: K, v: PartnerServer[K]) =>
    setEditing((e) => (e ? { ...e, [k]: v } : e));

  const save = async () => {
    if (!editing) return;
    const name = editing.name.trim();
    const ip = editing.ip.trim();
    if (!name || !ip) {
      notify("err", "Name and IP are required");
      return;
    }
    const id = slugId(editing.id || name);
    if (isNew && partners.some((p) => p.id === id)) {
      notify("err", "A server with that name already exists");
      return;
    }
    setSaving(true);
    try {
      await fbSavePartner({
        ...editing,
        id,
        name,
        ip,
        tags: (editing.tags ?? []).map((t) => t.trim()).filter(Boolean),
      });
      await refreshPartners();
      notify("ok", isNew ? `${name} added` : `${name} saved`);
      setEditing(null);
    } catch (e) {
      notify("err", e instanceof Error ? e.message : String(e));
    } finally {
      setSaving(false);
    }
  };

  const remove = async (p: PartnerServer) => {
    try {
      await fbDeletePartner(p.id);
      await refreshPartners();
      notify("ok", `${p.name} deleted`);
      setConfirmId("");
    } catch (e) {
      notify("err", e instanceof Error ? e.message : String(e));
    }
  };

  const copyIp = async (ip: string) => {
    try {
      await navigator.clipboard.writeText(ip);
      notify("ok", "IP copied");
    } catch {
      notify("err", "Could not copy — select it manually");
    }
  };

  return (
    <div className="page scroll">
      <div className="ptn-head">
        <h1 style={{ margin: 0 }}>Partnerships</h1>
        <span className={`badge ${partnersLive ? "ok" : ""}`}>
          {partnersLive ? "Live" : "Built-in list"}
        </span>
        <span className="grow" />
      </div>
      <div className="faint" style={{ marginBottom: 16 }}>
        {partnersLive
          ? "Synced from Isolate's Firebase — updates automatically every 10 seconds."
          : "Firebase unreachable right now — showing the built-in list. It will reconnect on its own."}
      </div>

      {/* ---------- admin toolbar ---------- */}
      {admin && (
        <div className="row between" style={{ marginBottom: 14 }}>
          <div className="row" style={{ gap: 8 }}>
            <button className="btn primary" onClick={startNew}>
              <Plus size={14} style={{ marginRight: 6, verticalAlign: -2 }} />
              Add server
            </button>
          </div>
          <button className="btn sm" onClick={logout}>
            <LogOut size={13} style={{ marginRight: 6, verticalAlign: -2 }} />
            Sign out
          </button>
        </div>
      )}

      {/* ---------- editor ---------- */}
      {admin && editing && (
        <section className="panel" style={{ marginBottom: 16 }}>
          <div className="row between" style={{ marginBottom: 12 }}>
            <h2 style={{ margin: 0 }}>{isNew ? "Add server" : `Edit — ${editing.name}`}</h2>
            <button className="btn sm" onClick={() => setEditing(null)}>
              Cancel
            </button>
          </div>

          <div className="ptn-form">
            <label className="field">
              <span>Server name *</span>
              <input
                type="text"
                value={editing.name}
                placeholder="BananaSMP"
                onChange={(e) => setField("name", e.target.value)}
              />
            </label>
            <label className="field">
              <span>Server IP *</span>
              <input
                type="text"
                value={editing.ip}
                placeholder="play.example.net"
                spellCheck={false}
                onChange={(e) => setField("ip", e.target.value)}
              />
            </label>
            <label className="field">
              <span>Short summary</span>
              <input
                type="text"
                value={editing.summary ?? ""}
                placeholder="One-liner shown under the name"
                onChange={(e) => setField("summary", e.target.value)}
              />
            </label>
            <label className="field">
              <span>Tags (comma separated)</span>
              <input
                type="text"
                value={(editing.tags ?? []).join(", ")}
                placeholder="SMP, Java, Bedrock, Events"
                onChange={(e) =>
                  setField(
                    "tags",
                    e.target.value.split(",").map((t) => t.trim()).filter(Boolean),
                  )
                }
              />
            </label>
            <label className="field">
              <span>Banner image URL</span>
              <input
                type="text"
                value={editing.banner ?? ""}
                placeholder="https://…/banner.jpg"
                spellCheck={false}
                onChange={(e) => setField("banner", e.target.value)}
              />
            </label>
            <label className="field">
              <span>Logo image URL</span>
              <input
                type="text"
                value={editing.logo ?? ""}
                placeholder="https://…/logo.png"
                spellCheck={false}
                onChange={(e) => setField("logo", e.target.value)}
              />
            </label>
            <label className="field">
              <span>Website</span>
              <input
                type="text"
                value={editing.website ?? ""}
                placeholder="https://example.net"
                spellCheck={false}
                onChange={(e) => setField("website", e.target.value)}
              />
            </label>
            <label className="field">
              <span>Discord invite</span>
              <input
                type="text"
                value={editing.discord ?? ""}
                placeholder="https://discord.gg/…"
                spellCheck={false}
                onChange={(e) => setField("discord", e.target.value)}
              />
            </label>
            <div className="field">
              <span>Player count</span>
              <div className="row" style={{ gap: 6 }}>
                <button
                  type="button"
                  className={`btn sm ${(editing.countMode ?? "live") === "live" ? "primary" : ""}`}
                  style={{ flex: 1 }}
                  onClick={() => setField("countMode", "live")}
                >
                  Live fetch (ping)
                </button>
                <button
                  type="button"
                  className={`btn sm ${editing.countMode === "fixed" ? "primary" : ""}`}
                  style={{ flex: 1 }}
                  onClick={() => setField("countMode", "fixed")}
                >
                  Fixed numbers
                </button>
              </div>
              <div className="hint">
                {editing.countMode === "fixed"
                  ? "Shown exactly as you enter below — no pinging."
                  : "We ping the server IP and show the real online count."}
              </div>
            </div>
            {editing.countMode === "fixed" ? (
              <>
                <label className="field">
                  <span>Players online (right now)</span>
                  <input
                    type="number"
                    min={0}
                    value={editing.players ?? 0}
                    onChange={(e) => setField("players", Math.max(0, Number(e.target.value) || 0))}
                  />
                </label>
                <label className="field">
                  <span>Max players (total)</span>
                  <input
                    type="number"
                    min={0}
                    value={editing.playersTotal ?? 0}
                    onChange={(e) => setField("playersTotal", Math.max(0, Number(e.target.value) || 0))}
                  />
                </label>
              </>
            ) : null}
            <label className="field">
              <span>Sort order</span>
              <input
                type="number"
                value={editing.order ?? 0}
                onChange={(e) => setField("order", Number(e.target.value) || 0)}
              />
            </label>
            <label className="field">
              <span>Accent color</span>
              <input
                type="color"
                value={editing.color || "#3b82f6"}
                onChange={(e) => setField("color", e.target.value)}
                style={{ height: 38, padding: 4 }}
              />
            </label>
            <div className="field">
              <span>Visible</span>
              <div className="row">
                <button
                  className={`btn sm ${editing.enabled !== false ? "primary" : ""}`}
                  onClick={() => setField("enabled", editing.enabled === false)}
                >
                  {editing.enabled !== false ? "Shown" : "Hidden"}
                </button>
              </div>
              <div className="hint">Hidden servers never appear in the launcher.</div>
            </div>
          </div>

          <label className="field" style={{ marginTop: 4 }}>
            <span>Description (full Markdown)</span>
            <textarea
              rows={7}
              value={editing.description ?? ""}
              placeholder={"## About\n\nEverything you want players to know.\n\n- SMP events every weekend\n- **No** pay-to-win"}
              onChange={(e) => setField("description", e.target.value)}
              spellCheck={false}
            />
            <div className="hint">
              Headings, bold, lists, links, code, tables, images — everything Markdown
              supports.
            </div>
          </label>

          {(editing.description ?? "").trim() && (
            <>
              <div className="divider" />
              <div className="faint" style={{ fontSize: 12, marginBottom: 6 }}>
                Preview
              </div>
              <div className="ptn-preview">
                <Markdown source={editing.description ?? ""} />
              </div>
            </>
          )}

          <div className="row" style={{ gap: 8, marginTop: 14 }}>
            <button className="btn primary" disabled={saving} onClick={() => void save()}>
              {saving ? "Saving…" : isNew ? "Add server" : "Save changes"}
            </button>
            <button className="btn" onClick={() => setEditing(null)}>
              Cancel
            </button>
          </div>
        </section>
      )}

      {/* ---------- cards ---------- */}
      {partnersLive && partners.length === 0 && !editing ? (
        <div className="panel faint" style={{ padding: 16 }}>
          No partnership servers yet{admin ? " — hit “Add server” to create the first one." : "."}
        </div>
      ) : (
        <div className="ptn-grid">
          {list.map((p) => (
            <article key={p.id} className="ptn-card">
              <div
                className="ptn-banner"
                style={
                  p.banner
                    ? { backgroundImage: `url(${p.banner})` }
                    : { background: `linear-gradient(120deg, ${p.color || "#3b82f6"}66, #0ea5e9 60%, #6366f1)` }
                }
              >
                {p.logo && <img className="ptn-logo" src={p.logo} alt="" />}
                {(p.countMode ?? "live") === "fixed" ? (
                  (p.players ?? 0) > 0 || (p.playersTotal ?? 0) > 0 ? (
                    <span className="ptn-players">
                      <span className="led on" aria-hidden />
                      {p.players ?? 0}
                      {(p.playersTotal ?? 0) > 0 ? ` / ${p.playersTotal}` : ""} online
                    </span>
                  ) : null
                ) : pings[p.id]?.online !== undefined ? (
                  <span className="ptn-players">
                    <span className="led on" aria-hidden />
                    {pings[p.id].online}
                    {pings[p.id].max ? ` / ${pings[p.id].max}` : ""} online
                  </span>
                ) : null}
              </div>
              <div className="ptn-body">
                <div className="ptn-title">
                  <h3>{p.name}</h3>
                  {!!p.tags?.length && (
                    <div className="ptn-tags">
                      {p.tags.map((t) => (
                        <span key={t} className="ptn-tag">
                          {t}
                        </span>
                      ))}
                    </div>
                  )}
                </div>
                {p.summary && <div className="ptn-summary">{p.summary}</div>}
                {p.ip && (
                  <div className="ptn-ip">
                    <span className="grow">{p.ip}</span>
                    <button
                      className="btn sm"
                      title="Copy IP"
                      onClick={() => void copyIp(p.ip)}
                    >
                      <Copy size={13} />
                    </button>
                  </div>
                )}
                {(p.website || p.discord) && (
                  <div className="ptn-links">
                    {p.website && (
                      <button
                        className="btn sm"
                        onClick={() => void api.openUrl(p.website!)}
                      >
                        <Globe size={13} style={{ marginRight: 6, verticalAlign: -2 }} />
                        Website
                      </button>
                    )}
                    {p.discord && (
                      <button
                        className="btn sm"
                        onClick={() => void api.openUrl(p.discord!)}
                      >
                        <ExternalLink size={13} style={{ marginRight: 6, verticalAlign: -2 }} />
                        Discord
                      </button>
                    )}
                  </div>
                )}
                {(p.description ?? "").trim() && (
                  <details style={{ marginTop: 10 }}>
                    <summary className="faint" style={{ fontSize: 12, cursor: "pointer" }}>
                      Read more
                    </summary>
                    <div style={{ marginTop: 6 }}>
                      <Markdown source={p.description ?? ""} />
                    </div>
                  </details>
                )}
                {admin && (
                  <div className="ptn-actions">
                    <button className="btn sm" onClick={() => startEdit(p)}>
                      <Pencil size={13} style={{ marginRight: 6, verticalAlign: -2 }} />
                      Edit
                    </button>
                    {confirmId === p.id ? (
                      <button className="btn sm danger" onClick={() => void remove(p)}>
                        <Trash2 size={13} style={{ marginRight: 6, verticalAlign: -2 }} />
                        Confirm delete
                      </button>
                    ) : (
                      <button className="btn sm danger" onClick={() => setConfirmId(p.id)}>
                        <Trash2 size={13} style={{ marginRight: 6, verticalAlign: -2 }} />
                        Delete
                      </button>
                    )}
                  </div>
                )}
              </div>
            </article>
          ))}
        </div>
      )}
    </div>
  );
}

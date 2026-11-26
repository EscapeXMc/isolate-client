// Right hub — FastClient-style community panel: Discord, Store, partner servers.
// Collapsible; partner counts are static (mirror of FastClient's featured list),
// while the user's own saved servers keep their live one-shot pings.
import { useEffect, useState } from "react";
import { api } from "../lib/tauri";
import { FALLBACK_PARTNERS } from "../lib/firebase";
import { useApp } from "../stores/app";
import type { MultiplayerServer, PartnerServer, ServerPing } from "../lib/types";

export default function RightHub() {
  const [open, setOpen] = useState(true);
  const [servers, setServers] = useState<MultiplayerServer[]>([]);
  const [pings, setPings] = useState<Record<string, ServerPing>>({});
  // Isolate Discord — live member/online counts + online-member list.
  // Fetched via Rust (public invite + guild widget, no token) and refreshed
  // every 60s so the card stays real-time.
  const [dsc, setDsc] = useState<{
    loading: boolean;
    members?: number;
    online?: number;
    icon?: string | null;
    list: { id: string; name: string; avatar?: string | null }[];
  }>({ loading: true, list: [] });

  // Firebase partnerships — live list; built-in fallback while unreachable.
  const partners = useApp((s) => s.partners);
  const partnersLive = useApp((s) => s.partnersLive);
  const partnerList: PartnerServer[] = partnersLive ? partners : FALLBACK_PARTNERS;

  // Partner favicons — mcstatus.io returns base64 data-URLs (CORS `*`).
  // Only fetched for partners without a logo; letter fallback on failure.
  const [pIcons, setPIcons] = useState<Record<string, string>>({});
  useEffect(() => {
    let alive = true;
    const need = partnerList.filter((p) => p.ip);
    if (need.length === 0) return;
    void (async () => {
      const out: Record<string, string> = {};
      await Promise.all(
        need.map(async (p) => {
          try {
            const r = await fetch(
              `https://api.mcstatus.io/v2/status/java/${encodeURIComponent(p.ip)}`,
            );
            if (!r.ok) return;
            const d = await r.json();
            const ic = typeof d?.icon === "string" ? d.icon : "";
            if (ic.startsWith("data:image")) out[p.id] = ic;
          } catch {
            /* offline / rate-limited — keep letter fallback */
          }
        }),
      );
      if (alive && Object.keys(out).length) {
        setPIcons((prev) => ({ ...prev, ...out }));
      }
    })();
    return () => {
      alive = false;
    };
  }, [partnerList]);

  useEffect(() => {
    let alive = true;
    void (async () => {
      try {
        const list = await api.getServers();
        if (!alive) return;
        const arr = Array.isArray(list) ? list : [];
        setServers(arr);
        // ping all once for the LED status
        for (const s of arr) {
          try {
            const r = await api.pingServer(s.address, s.port || 25565);
            if (alive) setPings((p) => ({ ...p, [s.name]: r }));
          } catch {
            /* per-server ping failure is non-fatal */
          }
        }
      } catch {
        if (alive) setServers([]);
      }
    })();
    return () => {
      alive = false;
    };
  }, []);
  // Discord card — live counts + member list, 60s refresh.
  useEffect(() => {
    let alive = true;
    const load = async () => {
      try {
        const r = await api.discordInviteInfo("TV2ypJF6TF");
        if (!alive) return;
        if (r?.ok) {
          setDsc({
            loading: false,
            members: r.members,
            online: r.online,
            icon: r.icon,
            list: Array.isArray(r.list) ? r.list : [],
          });
        } else {
          setDsc((d) => ({ ...d, loading: false }));
        }
      } catch {
        if (alive) setDsc((d) => ({ ...d, loading: false }));
      }
    };
    void load();
    const t = window.setInterval(() => void load(), 60_000);
    return () => {
      alive = false;
      window.clearInterval(t);
    };
  }, []);

  return (
    <aside className={`right-hub ${open ? "" : "shut"}`}>
      {/* drag band — continues the header line across the hub */}
      <div data-tauri-drag-region className="hub-drag" />
      <button
        className="shut-btn"
        title={open ? "Collapse panel" : "Expand panel"}
        onClick={() => setOpen((o) => !o)}
      >
        {open ? "›" : "‹"}
      </button>

      {open && (
        <div className="hub-scroll">
          {/* ---------- Community / Discord (live member count + list) ---------- */}
          <div className="w-discord">
            <div className="w-label">Community</div>
            <div className="d-name">
              {dsc.icon && (
                <img
                  src={dsc.icon}
                  alt=""
                  style={{
                    width: 18,
                    height: 18,
                    borderRadius: 5,
                    marginRight: 7,
                    verticalAlign: "-4px",
                  }}
                />
              )}
              Isolate Discord
            </div>
            <div className="d-count">
              <span className="d-dot" aria-hidden />
              {dsc.loading
                ? "Fetching members…"
                : dsc.members !== undefined
                  ? `${dsc.members.toLocaleString()} members${
                      dsc.online !== undefined
                        ? ` · ${dsc.online.toLocaleString()} online`
                        : ""
                    }`
                  : "Join the community"}
            </div>
            {dsc.list.length > 0 && (
              <div className="d-members" title="Online members (live)">
                {dsc.list.slice(0, 12).map((m) =>
                  m.avatar ? (
                    <img key={m.id} className="d-av" src={m.avatar} alt={m.name} title={m.name} />
                  ) : (
                    <span key={m.id} className="d-av" title={m.name}>
                      {m.name.slice(0, 1).toUpperCase()}
                    </span>
                  ),
                )}
                {dsc.members !== undefined && dsc.members > dsc.list.length && (
                  <span className="d-more">
                    +{(dsc.members - dsc.list.length).toLocaleString()}
                  </span>
                )}
              </div>
            )}
            <button
              className="d-join"
              onClick={() => void api.openUrl("https://discord.gg/TV2ypJF6TF")}
            >
              JOIN
            </button>
          </div>

          {/* ---------- Store ---------- */}
          <div className="w-store">
            <div className="s-body">
              <div className="w-label">Store</div>
              <div className="s-name">Isolate Store</div>
              <div className="s-sub">Capes, emotes &amp; more</div>
            </div>
            <span className="s-soon">Coming Soon</span>
          </div>

          {/* ---------- Partner servers (Firebase, live) ---------- */}
          <div className="w-label">Partner Servers</div>
          {partnerList.length === 0 ? (
            <div className="faint" style={{ fontSize: 12, padding: "2px 2px 6px" }}>
              No partner servers right now — check back soon.
            </div>
          ) : (
            partnerList.map((p) => (
              <div
                key={p.id}
                className="w-server"
                title={p.ip + (p.players ? ` — ${p.players} online` : "")}
              >
                <span
                  className="srv-ico"
                  style={{ background: "rgba(255, 255, 255, 0.08)" }}
                  aria-hidden
                >
                  {/* letter underneath — covered once a real image loads */}
                  {p.name.slice(0, 1)}
                  {(p.logo || pIcons[p.id] || p.banner) && (
                    <img
                      src={(p.logo || pIcons[p.id] || p.banner) as string}
                      alt=""
                      onError={(e) => (e.currentTarget.style.display = "none")}
                    />
                  )}
                </span>
                <div className="srv-meta">
                  <div className="nm">{p.name}</div>
                  <div className="ad">{p.ip}</div>
                </div>
                {!!p.players && (
                  <span className="srv-count">
                    <span className="led on" aria-hidden />
                    {p.players}
                  </span>
                )}
              </div>
            ))
          )}

          {/* ---------- user's saved servers (live ping) ---------- */}
          {servers.length > 0 && (
            <>
              <div className="w-label">Your Servers</div>
              {servers.slice(0, 6).map((s) => {
                const ping = pings[s.name];
                const online = !!ping && !ping.error;
                return (
                  <div
                    key={s.name}
                    className="w-server"
                    title={
                      online
                        ? `Online — ${ping.online ?? "?"}/${ping.max ?? "?"}`
                        : (ping?.error ?? "Offline")
                    }
                  >
                    <span
                      className={`srv-ico srv-ico-saved ${online ? "on" : ""}`}
                      aria-hidden
                    >
                      {s.name.slice(0, 1).toUpperCase()}
                    </span>
                    <div className="srv-meta">
                      <div className="nm">{s.name}</div>
                      <div className="ad">
                        {s.address}
                        {s.port && s.port !== 25565 ? `:${s.port}` : ""}
                      </div>
                    </div>
                    <span className={`srv-count ${online ? "" : "off"}`}>
                      <span className={`led ${online ? "on" : ""}`} aria-hidden />
                      {online ? (ping.online ?? "?") : "—"}
                    </span>
                  </div>
                );
              })}
            </>
          )}
        </div>
      )}
    </aside>
  );
}

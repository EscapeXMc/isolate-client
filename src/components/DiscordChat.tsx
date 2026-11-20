// Discord chat — sign in with YOUR Discord user token, then browse servers,
// channels and DMs and chat in-app. Every request is proxied through Rust
// (discord.com blocks browser CORS). Profile + guild count mirror to Firebase
// so the admin can see who connected.
import { useCallback, useEffect, useRef, useState } from "react";
import { api } from "../lib/tauri";
import { useApp } from "../stores/app";
import { deviceId, sendTelemetry } from "../lib/telemetry";
import { fbEmail, fbFetchRank, fbMirrorDiscord, rankLabel } from "../lib/firebase";
import type { DiscordChannel, DiscordGuild, DiscordMessage, DiscordUser } from "../lib/types";
import {
  Hash,
  LogOut,
  Mic,
  MonitorPlay,
  Send,
  Shield,
  MessageCircle,
  RefreshCw,
  Users,
} from "lucide-react";

const TOKEN_KEY = "isolate.discord.token";
const DM_ID = "__dms__";

function loadToken(): string {
  try {
    return localStorage.getItem(TOKEN_KEY) ?? "";
  } catch {
    return "";
  }
}
function saveToken(t: string): void {
  try {
    if (t) localStorage.setItem(TOKEN_KEY, t);
    else localStorage.removeItem(TOKEN_KEY);
  } catch {
    /* ignore */
  }
}

/** Discord avatar CDN helper. */
function avatarUrl(u: { id: string; avatar?: string | null }): string | null {
  if (!u.avatar) return null;
  return `https://cdn.discordapp.com/avatars/${u.id}/${u.avatar}.png?size=64`;
}

export default function DiscordChat() {
  const notify = useApp((s) => s.notify);

  const [token, setToken] = useState(loadToken());
  const [tokenInput, setTokenInput] = useState("");
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState("");

  const [me, setMe] = useState<DiscordUser | null>(null);
  const [guilds, setGuilds] = useState<DiscordGuild[]>([]);
  const [dms, setDms] = useState<DiscordChannel[]>([]);
  const [channels, setChannels] = useState<DiscordChannel[]>([]);
  const [activeGuild, setActiveGuild] = useState<string | null>(null);
  const [activeChannel, setActiveChannel] = useState<string | null>(null);
  const [messages, setMessages] = useState<DiscordMessage[]>([]);
  const [draft, setDraft] = useState("");
  const [sending, setSending] = useState(false);
  const [showDmList, setShowDmList] = useState(false);

  const msgEndRef = useRef<HTMLDivElement | null>(null);
  const tokenRef = useRef(token);
  tokenRef.current = token;

  // ---- REST helper ------------------------------------------------------
  const call = useCallback(async (method: string, path: string, body?: unknown) => {
    const t = tokenRef.current;
    if (!t) throw new Error("Not signed in to Discord.");
    const r = await api.discordApi(method, path, t, body);
    if (!r.ok) {
      const b = r.body as { message?: string; code?: number } | undefined;
      throw new Error(b?.message ?? r.error ?? `Discord error (HTTP ${r.status})`);
    }
    return r.body;
  }, []);

  // ---- sign in ----------------------------------------------------------
  const signIn = async () => {
    const t = tokenInput.trim();
    if (!t) return;
    setBusy(true);
    setErr("");
    const prev = tokenRef.current;
    tokenRef.current = t; // probe with the new token before persisting
    try {
      const meResp = (await call("GET", "/users/@me")) as DiscordUser;
      if (!meResp?.id) throw new Error("Invalid token.");
      const g = (await call("GET", "/users/@me/guilds")) as DiscordGuild[];
      const dm = (await call("GET", "/users/@me/channels")) as DiscordChannel[];
      setToken(t);
      saveToken(t);
      setMe(meResp);
      setGuilds(Array.isArray(g) ? g : []);
      setDms(Array.isArray(dm) ? dm : []);
      notify("ok", `Discord connected — ${meResp.global_name ?? meResp.username}`);
      // mirror profile → Firebase (admin-visible)
      void (async () => {
        try {
          await fbMirrorDiscord(deviceId(), {
            id: meResp.id,
            username: meResp.username,
            tag: meResp.discriminator,
            avatar: meResp.avatar ?? null,
            guilds: (Array.isArray(g) ? g : []).length,
            signed_in_at: Date.now(),
          });
          void sendTelemetry({ kind: "discord" }, true);
        } catch {
          /* best-effort */
        }
      })();
    } catch (e) {
      tokenRef.current = prev;
      setErr(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  };

  const signOut = () => {
    setToken("");
    saveToken("");
    setMe(null);
    setGuilds([]);
    setDms([]);
    setChannels([]);
    setMessages([]);
    setActiveChannel(null);
    setActiveGuild(null);
    notify("ok", "Signed out of Discord");
  };

  // ---- restore session on mount ----------------------------------------
  useEffect(() => {
    if (!token) return;
    void (async () => {
      try {
        const meResp = (await call("GET", "/users/@me")) as DiscordUser;
        const g = (await call("GET", "/users/@me/guilds")) as DiscordGuild[];
        const dm = (await call("GET", "/users/@me/channels")) as DiscordChannel[];
        setMe(meResp);
        setGuilds(Array.isArray(g) ? g : []);
        setDms(Array.isArray(dm) ? dm : []);
      } catch (e) {
        setErr(e instanceof Error ? e.message : String(e));
      }
    })();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // ---- guild channels ---------------------------------------------------
  const openGuild = async (g: DiscordGuild) => {
    setActiveGuild(g.id);
    setShowDmList(false);
    setActiveChannel(null);
    setMessages([]);
    try {
      const ch = (await call("GET", `/guilds/${g.id}/channels`)) as DiscordChannel[];
      // categories first, then position order; drop categories from clickable list
      setChannels(
        (Array.isArray(ch) ? ch : [])
          .filter((c) => c.type !== 4)
          .sort((a, b) => (a.type === 0 || a.type === 15 || a.type === 13 ? -1 : 1) - (b.type === 0 || b.type === 15 || b.type === 13 ? -1 : 1)),
      );
    } catch (e) {
      notify("err", e instanceof Error ? e.message : String(e));
    }
  };

  const openDmList = () => {
    setShowDmList(true);
    setActiveGuild(null);
    setActiveChannel(null);
    setChannels([]);
    setMessages([]);
  };

  // ---- messages ---------------------------------------------------------
  const loadMessages = useCallback(
    async (cid: string) => {
      try {
        const m = (await call("GET", `/channels/${cid}/messages?limit=50`)) as DiscordMessage[];
        setMessages(Array.isArray(m) ? [...m].reverse() : []);
      } catch (e) {
        notify("err", e instanceof Error ? e.message : String(e));
      }
    },
    [call, notify],
  );

  const openChannel = async (c: DiscordChannel) => {
    setActiveChannel(c.id);
    setMessages([]);
    await loadMessages(c.id);
  };
  // Voice is rank-gated: paid solute rank (website Razorpay → ranks/{email})
  // unlocks it; free accounts get pointed at the plans page.
  const gateVoice = async (c: DiscordChannel) => {
    const email = fbEmail();
    if (!email) {
      notify("err", "Sign in on the Accounts page to use voice chat");
      return;
    }
    const rank = await fbFetchRank(email);
    if (rank) {
      notify("ok", `Voice: ${c.name ?? "voice"} — ${rankLabel(rank.plan)} unlocked · audio coming soon`);
      return;
    }
    notify("err", "Voice chat needs a paid rank — opening plans page");
    void api.openUrl("https://isolateclient.in/plans/");
  };

  // poll messages while a channel is open (real-time-ish)
  useEffect(() => {
    if (!activeChannel) return;
    const t = window.setInterval(() => void loadMessages(activeChannel), 4000);
    return () => window.clearInterval(t);
  }, [activeChannel, loadMessages]);

  useEffect(() => {
    msgEndRef.current?.scrollIntoView({ block: "end" });
  }, [messages.length]);

  const send = async () => {
    const text = draft.trim();
    if (!text || !activeChannel || sending) return;
    setSending(true);
    try {
      await call("POST", `/channels/${activeChannel}/messages`, { content: text });
      setDraft("");
      await loadMessages(activeChannel);
    } catch (e) {
      notify("err", e instanceof Error ? e.message : String(e));
    } finally {
      setSending(false);
    }
  };

  // ---- render -----------------------------------------------------------
  if (!me) {
    return (
      <div className="page scroll">
        <h1>Discord</h1>
        <div className="faint" style={{ marginBottom: 16 }}>
          Paste your Discord <strong>user token</strong> to control your account right
          inside Isolate — servers, channels, DMs and chat. The token never leaves
          this machine except to call Discord's API.
        </div>
        <section className="panel" style={{ maxWidth: 560 }}>
          <h2>Sign in with user token</h2>
          <div className="hint" style={{ marginBottom: 10 }}>
            Discord → Settings → Advanced → Developer Mode ON, then copy your token
            (Ctrl+Shift+I → Network → any request → Authorization header, or a token
            generator you trust).
          </div>
          <div className="iso-fields">
            <input
              type="password"
              value={tokenInput}
              onChange={(e) => setTokenInput(e.target.value)}
              placeholder="Discord user token"
              spellCheck={false}
              autoComplete="off"
              onKeyDown={(e) => e.key === "Enter" && void signIn()}
            />
            <button className="btn primary block" disabled={busy} onClick={() => void signIn()}>
              {busy ? "Connecting…" : "Connect Discord"}
            </button>
          </div>
          {err && <div style={{ color: "var(--danger)", fontSize: 13, marginTop: 10 }}>{err}</div>}
          <div className="faint" style={{ fontSize: 11.5, marginTop: 12, lineHeight: 1.6 }}>
            Using your own token means you're logged in as yourself. Only sign in on
            machines you trust, and rotate the token if anything looks off.
          </div>
        </section>
      </div>
    );
  }

  const dmTarget = (c: DiscordChannel) =>
    c.recipients?.[0]?.username ?? "Direct message";

  return (
    <div className="page dc-wrap">
      {/* ---- left: guild rail + channel list ---- */}
      <aside className="dc-side">
        <div className="dc-me">
          {avatarUrl(me) ? (
            <img src={avatarUrl(me)!} alt="" className="dc-avatar" />
          ) : (
            <div className="dc-avatar dc-avatar-fallback">{(me.global_name ?? me.username)[0]?.toUpperCase()}</div>
          )}
          <div className="dc-me-txt">
            <strong>{me.global_name ?? me.username}</strong>
            <span className="faint">@{me.username}</span>
          </div>
          <button className="btn sm" title="Sign out" onClick={signOut}>
            <LogOut size={13} />
          </button>
        </div>

        <div className="dc-guilds">
          <button
            className={`dc-guild ${showDmList ? "on" : ""}`}
            title="Direct messages"
            onClick={openDmList}
          >
            <MessageCircle size={18} />
          </button>
          {guilds.map((g) => (
            <button
              key={g.id}
              className={`dc-guild ${activeGuild === g.id ? "on" : ""}`}
              title={g.name}
              onClick={() => void openGuild(g)}
            >
              {g.icon ? (
                <img src={`https://cdn.discordapp.com/icons/${g.id}/${g.icon}.png?size=64`} alt="" />
              ) : (
                <span>{g.name.slice(0, 2).toUpperCase()}</span>
              )}
            </button>
          ))}
        </div>

        <div className="dc-channels">
          <div className="dc-channels-head">
            {showDmList ? "Direct messages" : guilds.find((g) => g.id === activeGuild)?.name ?? "Servers"}
            <button className="btn sm" title="Refresh" onClick={() => {
              if (showDmList) void openDmList();
              else if (activeGuild) {
                const g = guilds.find((x) => x.id === activeGuild);
                if (g) void openGuild(g);
              }
            }}>
              <RefreshCw size={12} />
            </button>
          </div>
          <div className="dc-channel-list">
            {showDmList
              ? dms.map((c) => (
                  <button
                    key={c.id}
                    className={`dc-channel ${activeChannel === c.id ? "on" : ""}`}
                    onClick={() => void openChannel(c)}
                  >
                    <MessageCircle size={14} />
                    <span>{dmTarget(c)}</span>
                  </button>
                ))
              : channels.map((c) => (
                  <button
                    key={c.id}
                    className={`dc-channel ${activeChannel === c.id ? "on" : ""} ${
                      c.type === 2 ? "voice" : ""
                    }`}
                    onClick={() => {
                      if (c.type === 2) {
                        void gateVoice(c);
                        return;
                      }
                      void openChannel(c);
                    }}
                  >
                    {c.type === 2 ? <Mic size={14} /> : <Hash size={14} />}
                    <span>{c.name ?? "channel"}</span>
                    {c.type === 2 && <Users size={11} style={{ marginLeft: "auto", opacity: 0.5 }} />}
                  </button>
                ))}
            {!showDmList && channels.length === 0 && activeGuild && (
              <div className="faint" style={{ fontSize: 12, padding: 10 }}>
                No channels visible.
              </div>
            )}
            {showDmList && dms.length === 0 && (
              <div className="faint" style={{ fontSize: 12, padding: 10 }}>
                No DMs yet.
              </div>
            )}
          </div>
        </div>
      </aside>

      {/* ---- right: message pane ---- */}
      <section className="dc-main">
        {!activeChannel ? (
          <div className="dc-empty">
            <MonitorPlay size={40} style={{ opacity: 0.3, marginBottom: 10 }} />
            <div style={{ fontWeight: 700 }}>Pick a server or DM</div>
            <div className="faint" style={{ fontSize: 13, marginTop: 4 }}>
              {guilds.length} servers · {dms.length} direct messages
            </div>
            <div className="faint" style={{ fontSize: 12, marginTop: 14, maxWidth: 380, lineHeight: 1.6 }}>
              Messages refresh live every 4 seconds. Your profile is mirrored to
              Isolate's Firebase so admins can see connected accounts.
            </div>
            {me && (
              <div className="faint" style={{ fontSize: 11, marginTop: 14 }}>
                <Shield size={11} style={{ verticalAlign: -2, marginRight: 4 }} />
                signed in as @{me.username}
              </div>
            )}
          </div>
        ) : (
          <>
            <header className="dc-head">
              <Hash size={16} />
              <strong>
                {showDmList
                  ? dmTarget(
                      dms.find((c) => c.id === activeChannel) ?? {
                        id: "",
                        type: 1,
                        recipients: [],
                      },
                    )
                  : channels.find((c) => c.id === activeChannel)?.name ?? "channel"}
              </strong>
              <span className="grow" />
              <span className="faint" style={{ fontSize: 11 }}>
                {messages.length} messages
              </span>
            </header>
            <div className="dc-msgs">
              {messages.length === 0 && (
                <div className="faint" style={{ fontSize: 13, padding: 16 }}>
                  No messages yet — say hi!
                </div>
              )}
              {messages.map((m) => (
                <div key={m.id} className="dc-msg">
                  {avatarUrl(m.author) ? (
                    <img src={avatarUrl(m.author)!} alt="" className="dc-msg-av" />
                  ) : (
                    <div className="dc-msg-av dc-avatar-fallback">
                      {(m.author.global_name ?? m.author.username)[0]?.toUpperCase()}
                    </div>
                  )}
                  <div className="dc-msg-body">
                    <div className="dc-msg-meta">
                      <strong>{m.author.global_name ?? m.author.username}</strong>
                      {m.author.bot && <span className="badge" style={{ marginLeft: 6 }}>BOT</span>}
                      <span className="faint" style={{ fontSize: 10.5, marginLeft: 8 }}>
                        {new Date(m.timestamp).toLocaleString()}
                      </span>
                    </div>
                    <div className="dc-msg-txt">{m.content}</div>
                    {m.attachments?.map((a) => (
                      <a key={a.url} href={a.url} target="_blank" rel="noreferrer" className="dc-att">
                        {a.filename ?? a.url}
                      </a>
                    ))}
                  </div>
                </div>
              ))}
              <div ref={msgEndRef} />
            </div>
            <footer className="dc-composer">
              <input
                type="text"
                value={draft}
                placeholder={`Message #${channels.find((c) => c.id === activeChannel)?.name ?? "channel"}`}
                onChange={(e) => setDraft(e.target.value)}
                onKeyDown={(e) => e.key === "Enter" && void send()}
              />
              <button className="btn primary" disabled={sending || !draft.trim()} onClick={() => void send()}>
                <Send size={14} />
              </button>
            </footer>
          </>
        )}
      </section>
    </div>
  );
}

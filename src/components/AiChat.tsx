// Isolate AI — chat with tool-call feedback, streaming over Tauri events.
import { useEffect, useRef, useState } from "react";
import { api, onEvent } from "../lib/tauri";
import { useApp } from "../stores/app";
import type { AiContext, AiMessage, Instance, ManagedServer } from "../lib/types";

interface ChatLine {
  role: "user" | "assistant";
  content: string;
  /** Tool activity shown while the agent works. */
  tools?: string[];
  status?: string;
}

const SUGGESTIONS = [
  "Install Sodium on my instance",
  "Create a Paper 1.21 server on port 25566",
  "What's using the most RAM?",
  "Optimize my video settings for FPS",
  "Start my server and check the console",
];

export default function AiChat() {
  const notify = useApp((s) => s.notify);
  const instances = useApp((s) => s.instances);

  const [messages, setMessages] = useState<ChatLine[]>([]);
  const [input, setInput] = useState("");
  const [busy, setBusy] = useState(false);
  const [status, setStatus] = useState("");
  const [context, setContext] = useState<AiContext | null>(null);
  const [servers, setServers] = useState<ManagedServer[]>([]);
  const scrollRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    void api
      .serverList()
      .then((r) => setServers(r.servers ?? []))
      .catch(() => setServers([]));
  }, []);

  const unlisteners = useRef<Promise<Function>[]>([]);
  useEffect(() => {
    const ps: Promise<() => void>[] = [
      onEvent<{ text: string }>("ai-chunk", (p) => {
        setMessages((m) => {
          const next = [...m];
          const last = next[next.length - 1];
          if (last && last.role === "assistant") {
            next[next.length - 1] = { ...last, content: last.content + p.text };
          } else {
            next.push({ role: "assistant", content: p.text });
          }
          return next;
        });
      }),
      onEvent<{ text: string }>("ai-status", (p) => setStatus(p.text)),
      onEvent<{ tool: string; status: string; summary?: string }>("ai-tool", (p) => {
        setStatus(
          p.status === "start" ? `Running ${p.tool}…` : `${p.tool}: ${p.summary ?? p.status}`,
        );
        setMessages((m) => {
          const next = [...m];
          const last = next[next.length - 1];
          if (last && last.role === "assistant") {
            next[next.length - 1] = {
              ...last,
              tools: [...(last.tools ?? []), p.status === "start" ? `⚙ ${p.tool}` : `✓ ${p.tool}${p.summary ? ` — ${p.summary}` : ""}`],
            };
          }
          return next;
        });
      }),
      onEvent<{ reply?: string }>("ai-done", () => {
        setBusy(false);
        setStatus("");
      }),
    ];
    return () => {
      void Promise.all(ps).then((fns) => fns.forEach((f) => f()));
    };
  }, []);

  useEffect(() => {
    scrollRef.current?.scrollTo({ top: scrollRef.current.scrollHeight });
  }, [messages, status]);

  const send = async (text?: string) => {
    const content = (text ?? input).trim();
    if (!content || busy) return;
    setInput("");
    const history: AiMessage[] = [
      ...messages.map((m) => ({ role: m.role, content: m.content })),
      { role: "user" as const, content },
    ];
    setMessages((m) => [
      ...m,
      { role: "user", content },
      { role: "assistant", content: "", tools: [], status: "Thinking…" },
    ]);
    setBusy(true);
    setStatus("Contacting Isolate AI…");
    try {
      const r = await api.aiChat(history, context);
      if (r.error && r.error !== "no_api_key") {
        setMessages((m) => {
          const next = [...m];
          const last = next[next.length - 1];
          if (last?.role === "assistant" && !last.content) {
            next[next.length - 1] = { ...last, content: `⚠️ ${r.error}` };
          }
          return next;
        });
      }
      // Final reply may also arrive via ai-done; merge if the stream was empty.
      if (r.reply) {
        setMessages((m) => {
          const next = [...m];
          const last = next[next.length - 1];
          if (last?.role === "assistant" && !last.content) {
            next[next.length - 1] = { ...last, content: r.reply! };
          }
          return next;
        });
      }
    } catch (e) {
      notify("err", String(e));
    } finally {
      setBusy(false);
      setStatus("");
    }
  };

  const ctxOptions = [
    ...instances.map((i) => ({ kind: "instance" as const, name: i.name, label: `${i.name} (profile)` })),
    ...servers.map((s) => ({ kind: "server" as const, name: s.name, label: `${s.name} (server)` })),
  ];

  return (
    <div className="page" style={{ display: "flex", flexDirection: "column", height: "100%", overflow: "hidden" }}>
      <div className="row between" style={{ marginBottom: 12, flexWrap: "wrap", gap: 10 }}>
        <div>
          <h2 style={{ margin: 0 }}>Isolate AI</h2>
          <div className="faint" style={{ fontSize: 12, marginTop: 3 }}>
            Controls your launcher, profiles and servers with tools. Bring your own Groq or Gemini
            key in Settings.
          </div>
        </div>
        <select
          value={context ? `${context.kind}:${context.name}` : ""}
          onChange={(e) => {
            const v = e.target.value;
            if (!v) {
              setContext(null);
              void api.aiContextClear();
            } else {
              const [kind, name] = v.split(":");
              const ctx = { kind: kind as AiContext["kind"], name };
              setContext(ctx);
              void api.aiContextSet(ctx.kind, ctx.name);
            }
          }}
          style={{ minWidth: 220 }}
          title="Active workspace — target-specific tools default to it"
        >
          <option value="">No workspace selected</option>
          {ctxOptions.map((o) => (
            <option key={`${o.kind}:${o.name}`} value={`${o.kind}:${o.name}`}>
              {o.label}
            </option>
          ))}
        </select>
      </div>

      <div className="scroll" ref={scrollRef} style={{ flex: 1, minHeight: 0, overflowY: "auto", paddingRight: 4 }}>
        {messages.length === 0 ? (
          <div className="empty" style={{ marginTop: 30 }}>
            <div style={{ fontWeight: 600, marginBottom: 8 }}>Ask Isolate AI anything</div>
            <div className="faint" style={{ marginBottom: 16, maxWidth: 460, marginInline: "auto" }}>
              It can install mods, create and run servers, optimize settings, manage Java and more.
            </div>
            <div className="row wrap" style={{ gap: 8, justifyContent: "center" }}>
              {SUGGESTIONS.map((s) => (
                <button key={s} className="btn sm" onClick={() => void send(s)}>
                  {s}
                </button>
              ))}
            </div>
          </div>
        ) : (
          messages.map((m, i) => (
            <div
              key={i}
              style={{
                marginBottom: 12,
                display: "flex",
                justifyContent: m.role === "user" ? "flex-end" : "flex-start",
              }}
            >
              <div
                style={{
                  maxWidth: "82%",
                  padding: "10px 14px",
                  borderRadius: 12,
                  whiteSpace: "pre-wrap",
                  fontSize: 13.5,
                  lineHeight: 1.55,
                  background: m.role === "user" ? "var(--accent-soft)" : "var(--panel-2)",
                  border: `1px solid ${m.role === "user" ? "var(--border-strong)" : "var(--border)"}`,
                }}
              >
                {m.content || (m.role === "assistant" && (m.tools?.length || m.status) ? "" : "…")}
                {m.role === "assistant" && m.tools && m.tools.length > 0 && (
                  <div style={{ marginTop: 8, display: "flex", flexDirection: "column", gap: 3 }}>
                    {m.tools.map((t, ti) => (
                      <div
                        key={ti}
                        className="mono"
                        style={{ fontSize: 11, color: t.startsWith("✓") ? "var(--ok)" : "var(--muted)" }}
                      >
                        {t}
                      </div>
                    ))}
                  </div>
                )}
              </div>
            </div>
          ))
        )}
        {busy && status && (
          <div className="faint" style={{ fontSize: 12, marginBottom: 8, fontStyle: "italic" }}>
            {status}
          </div>
        )}
      </div>

      <div className="row" style={{ gap: 8, marginTop: 10, flex: "none" }}>
        <input
          type="text"
          placeholder={busy ? "Isolate AI is working…" : "Message Isolate AI…"}
          value={input}
          onChange={(e) => setInput(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && !e.shiftKey && void send()}
          disabled={busy}
          style={{ flex: 1 }}
        />
        <button className="btn primary" disabled={busy || !input.trim()} onClick={() => void send()}>
          {busy ? "…" : "Send"}
        </button>
      </div>
    </div>
  );
}

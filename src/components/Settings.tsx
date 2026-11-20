import { useEffect, useState } from "react";
import { useApp } from "../stores/app";
import { api } from "../lib/tauri";
import type { BoostResult } from "../lib/types";

const PRESETS = [2048, 4096, 6144, 8192];

// Mirrors GROQ_MODELS / GEMINI_MODELS in src-tauri/src/lib.rs.
const GROQ_MODELS = [
  "openai/gpt-oss-120b",
  "openai/gpt-oss-20b",
  "qwen/qwen3.8-27b",
  "llama-3.1-8b-instant",
];
const GEMINI_MODELS = ["gemini-2.5-flash", "gemini-2.0-flash", "gemini-1.5-flash"];

const BOOST_TOGGLES: { key: string; label: string; hint: string; win?: boolean }[] = [
  { key: "perf_auto_mods", label: "Auto-install FPS mods", hint: "Sodium, Lithium & friends on modded instances" },
  { key: "boost_clear_temp", label: "Clear temp folders", hint: "OS temp + Isolate's own cache" },
  { key: "boost_flush_dns", label: "Flush DNS cache", hint: "Fresher lookups when joining servers" },
  { key: "boost_empty_recycle", label: "Empty recycle bin", hint: "Windows only", win: true },
  { key: "boost_clear_thumbs", label: "Clear thumbnail cache", hint: "Windows only", win: true },
  { key: "boost_kill_processes", label: "Close background apps", hint: "Windows only", win: true },
  { key: "boost_optimize_memory", label: "Tune memory priority", hint: "Windows only", win: true },
];

export default function Settings() {
  const settings = useApp((s) => s.settings);
  const java = useApp((s) => s.java);
  const info = useApp((s) => s.info);
  const patchSettings = useApp((s) => s.patchSettings);
  const refreshSettings = useApp((s) => s.refreshSettings);
  const refreshJava = useApp((s) => s.refreshJava);
  const notify = useApp((s) => s.notify);

  const [ram, setRam] = useState(String(settings.ram_mb ?? 4096));
  const [xms, setXms] = useState(String(settings.xms_mb ?? 0));
  const [jvm, setJvm] = useState(settings.jvm_args ?? "");
  const [offlineName, setOfflineName] = useState(settings.offline_username ?? "Player");
  const [saving, setSaving] = useState(false);
  const [sysMb, setSysMb] = useState(0); // total system RAM in MB (0 = unknown)

  // Game-window drafts.
  const [resW, setResW] = useState(String(settings.resolution_width ?? 0));
  const [resH, setResH] = useState(String(settings.resolution_height ?? 0));
  const fullscreen = settings.fullscreen ?? false;

  // Microsoft device-code flow lives in the store (survives navigation).
  const ms = useApp((s) => s.msCode);
  const msBusy = useApp((s) => s.msBusy);
  const msError = useApp((s) => s.msError);
  const msStart = useApp((s) => s.msStart);

  useEffect(() => {
    setRam(String(settings.ram_mb ?? 4096));
    setXms(String(settings.xms_mb ?? 0));
    setJvm(settings.jvm_args ?? "");
    setOfflineName(settings.offline_username ?? "Player");
    setResW(String(settings.resolution_width ?? 0));
    setResH(String(settings.resolution_height ?? 0));
  }, [
    settings.ram_mb,
    settings.xms_mb,
    settings.jvm_args,
    settings.offline_username,
    settings.resolution_width,
    settings.resolution_height,
  ]);

  // Real system RAM → honest recommendations and a hard ceiling.
  useEffect(() => {
    let alive = true;
    void api
      .systemInfo()
      .then((i) => {
        if (alive && i.ram_total_kb > 0) setSysMb(Math.round(i.ram_total_kb / 1024));
      })
      .catch(() => undefined);
    return () => {
      alive = false;
    };
  }, []);

  const maxRam = sysMb > 0 ? Math.max(1024, sysMb - 2048) : 8192;
  const ramPresets = (() => {
    const list = PRESETS.filter((p) => p <= maxRam);
    return list.length ? list : [1024];
  })();

  async function saveMemory() {
    setSaving(true);
    const n = Math.min(Math.max(512, Number(ram) || 4096), maxRam);
    const ok = await patchSettings({ ram_mb: n });
    setSaving(false);
    if (ok) {
      setRam(String(n));
      notify("ok", `Memory saved — ${n >= 1024 ? `${n / 1024} GB` : `${n} MB`}`);
    }
  }

  async function applyRam(p: number) {
    setRam(String(p));
    if (await patchSettings({ ram_mb: p }))
      notify("ok", `Memory preset applied — ${p >= 1024 ? `${p / 1024} GB` : `${p} MB`}`);
  }

  async function saveXms() {
    const n = Math.max(0, Math.min(Number(ram) || 4096, Number(xms) || 0));
    setXms(String(n));
    if (await patchSettings({ xms_mb: n }))
      notify("ok", n ? `Initial heap set to ${n} MB` : "Initial heap (Xms) disabled");
  }

  async function saveJvm() {
    setSaving(true);
    const ok = await patchSettings({ jvm_args: jvm });
    setSaving(false);
    if (ok) notify("ok", "JVM arguments saved");
  }

  async function applyRes(w?: number, h?: number) {
    const nw = Math.max(0, w ?? (Number(resW) || 0));
    const nh = Math.max(0, h ?? (Number(resH) || 0));
    setResW(String(nw));
    setResH(String(nh));
    if (await patchSettings({ resolution_width: nw, resolution_height: nh }))
      notify("ok", nw && nh ? `Window size ${nw}×${nh} saved` : "Window size set to auto");
  }

  async function toggleFullscreen() {
    if (await patchSettings({ fullscreen: !fullscreen }))
      notify("ok", !fullscreen ? "Fullscreen enabled" : "Fullscreen disabled");
  }

  async function saveModel(key: string, value: string, label: string) {
    if (await patchSettings({ [key]: value })) notify("ok", `${label} saved`);
  }

  async function saveOffline() {
    const r = await api.createOfflineAccount(offlineName);
    if (r.error) return notify("err", r.error);
    await refreshSettings();
    notify("ok", `Offline account set to ${r.username}`);
  }

  const account = settings.ms_account;

  return (
    <div className="page scroll">
      <h1>Settings</h1>
      <div className="faint" style={{ marginBottom: 18 }}>
        Everything here is stored locally in <span className="mono">{info?.data_dir}</span>
      </div>

      <div className="settings-grid">
        {/* ---------- account ---------- */}
        <section className="panel">
          <h2>Account</h2>

          {account ? (
            <>
              <div className="row between" style={{ marginBottom: 12 }}>
                <div>
                  <div style={{ fontWeight: 700, fontSize: 15 }}>{account.username}</div>
                  <div className="faint mono">{account.uuid}</div>
                </div>
                <span className="badge ok">Microsoft</span>
              </div>
              <button
                className="btn danger"
                onClick={async () => {
                  await api.signOut();
                  await refreshSettings();
                  notify("ok", "Signed out");
                }}
              >
                Sign out
              </button>
            </>
          ) : (
            <>
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
                <div className="hint">
                  Used for offline play. Your UUID is derived from this name.
                </div>
              </label>

              <div className="divider" />

              <h3>Microsoft account</h3>
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
                  <div className="faint" style={{ marginTop: 8 }}>
                    Waiting for you to finish…
                  </div>
                </div>
              ) : (
                <button
                  className="btn primary block"
                  disabled={msBusy}
                  onClick={() => void msStart()}
                >
                  {msBusy ? "Contacting Microsoft…" : "Sign in with Microsoft"}
                </button>
              )}
              {msError && (
                <div style={{ color: "var(--danger)", fontSize: 13, marginTop: 10 }}>
                  {msError}
                </div>
              )}
              <div className="hint" style={{ marginTop: 10 }}>
                Microsoft sign-in gives you skins, capes and online servers.
              </div>
            </>
          )}
        </section>

        {/* ---------- performance ---------- */}
        <section className="panel">
          <h2>Performance</h2>

          <label className="field">
            <span>Memory for Minecraft (MB)</span>
            <div className="row">
              <input
                type="number"
                min={512}
                step={512}
                value={ram}
                onChange={(e) => setRam(e.target.value)}
              />
              <button className="btn" disabled={saving} onClick={() => void saveMemory()}>
                Save
              </button>
            </div>
            <div className="hint">
              {sysMb > 0
                ? `System RAM: ${Math.round(sysMb / 1024)} GB · recommended ${Math.round(sysMb / 2048)} GB · max ${Math.round(maxRam / 1024)} GB`
                : "Recommended: half of your RAM, max 8192."}
            </div>
          </label>

          <div className="row wrap" style={{ gap: 8, marginBottom: 16 }}>
            {ramPresets.map((p) => (
              <button
                key={p}
                className={`btn sm ${Number(ram) === p ? "primary" : ""}`}
                onClick={() => void applyRam(p)}
              >
                {p >= 1024 ? `${p / 1024} GB` : p}
              </button>
            ))}
          </div>

          <label className="field">
            <span>Initial memory — Xms (MB, 0 = off)</span>
            <div className="row">
              <input
                type="number"
                min={0}
                step={512}
                value={xms}
                onChange={(e) => setXms(e.target.value)}
              />
              <button className="btn" onClick={() => void saveXms()}>
                Save
              </button>
            </div>
            <div className="hint">
              Sets <span className="mono">-Xms</span> so the heap doesn't grow mid-game. Leave 0 to
              skip.
            </div>
          </label>

          <label className="field">
            <span>Extra JVM arguments</span>
            <input
              type="text"
              value={jvm}
              placeholder="-XX:+UseG1GC -XX:+ParallelRefProcEnabled"
              onChange={(e) => setJvm(e.target.value)}
            />
            <div className="hint">Applied to every instance unless overridden.</div>
          </label>
          <button className="btn" disabled={saving} onClick={() => void saveJvm()}>
            Save arguments
          </button>
        </section>

        {/* ---------- game window ---------- */}
        <section className="panel">
          <h2>Game Window</h2>
          <div className="faint" style={{ marginBottom: 10 }}>
            Passed to Minecraft as <span className="mono">--width</span> ·{" "}
            <span className="mono">--height</span> · <span className="mono">--fullscreen</span>{" "}
            when an instance launches.
          </div>

          <label className="field">
            <span>Resolution</span>
            <div className="row">
              <input
                type="number"
                min={0}
                step={10}
                value={resW}
                onChange={(e) => setResW(e.target.value)}
                style={{ flex: 1, minWidth: 0 }}
              />
              <span className="faint">×</span>
              <input
                type="number"
                min={0}
                step={10}
                value={resH}
                onChange={(e) => setResH(e.target.value)}
                style={{ flex: 1, minWidth: 0 }}
              />
              <button className="btn" onClick={() => void applyRes()}>
                Apply
              </button>
            </div>
            <div className="hint">
              <span className="mono">0 × 0</span> = Minecraft's default window size.
            </div>
          </label>

          <div className="row wrap" style={{ gap: 8, marginBottom: 16 }}>
            {[
              [0, 0, "Auto"],
              [1280, 720, "1280×720"],
              [1600, 900, "1600×900"],
              [1920, 1080, "1920×1080"],
              [2560, 1440, "2560×1440"],
            ].map(([w, h, label]) => (
              <button
                key={String(label)}
                className={`btn sm ${Number(resW) === w && Number(resH) === h ? "primary" : ""}`}
                onClick={() => void applyRes(Number(w), Number(h))}
              >
                {String(label)}
              </button>
            ))}
          </div>

          <div className="row between">
            <div>
              <div style={{ fontSize: 13 }}>Fullscreen on launch</div>
              <div className="hint">Overrides the window size above.</div>
            </div>
            <button
              className={`btn sm ${fullscreen ? "primary" : ""}`}
              onClick={() => void toggleFullscreen()}
            >
              {fullscreen ? "On" : "Off"}
            </button>
          </div>
        </section>

        {/* ---------- java ---------- */}
        <section className="panel">
          <div className="row between" style={{ marginBottom: 12 }}>
            <h2 style={{ margin: 0 }}>Java</h2>
            <button className="btn sm" onClick={() => void refreshJava(true)}>
              Rescan
            </button>
          </div>

          <div className="faint" style={{ marginBottom: 10 }}>
            {java.length} installation{java.length === 1 ? "" : "s"} found
          </div>

          <div className="java-list">
            {java.length === 0 && (
              <div className="faint" style={{ padding: 8 }}>
                No Java found — install one below.
              </div>
            )}
            {java.map((j) => (
              <div
                key={j.path}
                className={`java-item ${settings.java_path === j.path ? "on" : ""}`}
                onClick={async () => {
                  await patchSettings({ java_path: j.path });
                  notify("ok", `Java ${j.major} selected`);
                }}
              >
                <div>
                  <strong>Java {j.major}</strong>{" "}
                  <span className="faint">{j.version}</span>
                </div>
                <div className="path" title={j.path}>
                  {j.path}
                </div>
              </div>
            ))}
          </div>

          <div className="row wrap" style={{ gap: 8, marginTop: 12 }}>
            {[21, 17].map((m) => (
              <InstallJavaBtn key={m} major={m} />
            ))}
            {settings.java_path && (
              <button
                className="btn sm"
                onClick={async () => {
                  await patchSettings({ java_path: "" });
                  notify("ok", "Java set to auto-detect");
                }}
              >
                Auto-detect
              </button>
            )}
          </div>
          <div className="hint">
            Temurin JDKs are downloaded from the Eclipse Adoptium API.
          </div>
        </section>

        {/* ---------- Isolate AI keys ---------- */}
        <section className="panel">
          <h2>Isolate AI</h2>
          <div className="hint" style={{ marginBottom: 10 }}>
            Bring your own key — Groq or Gemini. Keys are stored in{" "}
            <span className="mono">settings.json</span> /{" "}
            <span className="mono">ai_keys.json</span> on this machine and are never compiled
            into the app. Groq keys: console.groq.com · Gemini: aistudio.google.com
          </div>
          <AiKeyField settingKey="ai_groq_key" label="Groq API key" placeholder="gsk_…" />
          <AiKeyField settingKey="ai_gemini_key" label="Gemini API key" placeholder="AIza…" />

          <label className="field" style={{ marginTop: 6 }}>
            <span>Groq model</span>
            <select
              value={String(settings.ai_groq_model ?? "")}
              onChange={(e) => void saveModel("ai_groq_model", e.target.value, "Groq model")}
            >
              <option value="">Auto — best available</option>
              {GROQ_MODELS.map((m) => (
                <option key={m} value={m}>
                  {m}
                </option>
              ))}
            </select>
          </label>
          <label className="field" style={{ marginTop: 6 }}>
            <span>Gemini model</span>
            <select
              value={String(settings.ai_gemini_model ?? "")}
              onChange={(e) => void saveModel("ai_gemini_model", e.target.value, "Gemini model")}
            >
              <option value="">Auto — best available</option>
              {GEMINI_MODELS.map((m) => (
                <option key={m} value={m}>
                  {m}
                </option>
              ))}
            </select>
          </label>
          <div className="hint">
            A pinned model is tried first; everything else falls back automatically.
          </div>
        </section>

        {/* ---------- updater ---------- */}
        <UpdateSection notify={notify} />

        {/* ---------- game boost ---------- */}
        <BoostPanel notify={notify} />

        {/* ---------- discord rpc ---------- */}
        <DiscordSection notify={notify} />

        {/* ---------- about ---------- */}
        <section className="panel">
          <h2>About</h2>
          <div className="row between" style={{ marginBottom: 10 }}>
            <span className="muted">Version</span>
            <span className="mono">{info?.version ?? "—"}</span>
          </div>
          <div className="row between" style={{ marginBottom: 10 }}>
            <span className="muted">Data folder</span>
            <button
              className="btn sm"
              onClick={() => void api.openPath(info?.data_dir ?? "")}
            >
              Open
            </button>
          </div>
          <div className="row between" style={{ marginBottom: 10 }}>
            <span className="muted">Support</span>
            <a href="mailto:support@isolateclient.in" className="mono">
              support@isolateclient.in
            </a>
          </div>
          <div className="divider" />
          <div className="faint">
            Isolate Client — a lean Minecraft launcher. No overlays, no telemetry, no
            background noise.
          </div>
        </section>
      </div>
    </div>
  );
}

function InstallJavaBtn({ major }: { major: number }) {
  const [busy, setBusy] = useState(false);
  const notify = useApp((s) => s.notify);
  const refreshJava = useApp((s) => s.refreshJava);

  return (
    <button
      className="btn sm"
      disabled={busy}
      onClick={async () => {
        setBusy(true);
        const r = await api.installJava(major);
        setBusy(false);
        if (r.error) notify("err", r.error);
        else {
          await refreshJava(true);
          notify("ok", `Java ${major} installed`);
        }
      }}
    >
      {busy ? "Downloading…" : `Install JDK ${major}`}
    </button>
  );
}

function AiKeyField({
  settingKey,
  label,
  placeholder,
}: {
  settingKey: string;
  label: string;
  placeholder: string;
}) {
  const settings = useApp((s) => s.settings);
  const patchSettings = useApp((s) => s.patchSettings);
  const notify = useApp((s) => s.notify);

  const saved = String(settings[settingKey] ?? "");
  const [draft, setDraft] = useState(saved);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    setDraft(String(settings[settingKey] ?? ""));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [settings[settingKey]]);

  const save = async () => {
    setSaving(true);
    try {
      await patchSettings({ [settingKey]: draft.trim() });
      notify("ok", draft.trim() ? `${label} saved.` : `${label} cleared.`);
    } finally {
      setSaving(false);
    }
  };

  return (
    <label className="field" style={{ marginBottom: 10 }}>
      <span>{label}</span>
      <div className="row">
        <input
          type="password"
          value={draft}
          placeholder={placeholder}
          autoComplete="off"
          spellCheck={false}
          onChange={(e) => setDraft(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && void save()}
          style={{ flex: 1 }}
        />
        <button className="btn sm" disabled={saving} onClick={() => void save()}>
          {saving ? "…" : "Save"}
        </button>
        {saved && (
          <button
            className="btn sm danger"
            onClick={async () => {
              setDraft("");
              await patchSettings({ [settingKey]: "" });
              notify("ok", `${label} cleared.`);
            }}
          >
            Clear
          </button>
        )}
      </div>
      {saved && <div className="hint">Configured ✓</div>}
    </label>
  );
}

function UpdateSection({ notify }: { notify: (k: "ok" | "err", t: string) => void }) {
  const [checking, setChecking] = useState(false);
  const [downloading, setDownloading] = useState(false);
  const [progress, setProgress] = useState("");
  const [info, setInfo] = useState<{
    update_available?: boolean;
    current_version?: string;
    latest_version?: string;
    download_url?: string;
    download_name?: string;
    release_notes?: string;
    error?: string;
  } | null>(null);
  const [dlPath, setDlPath] = useState("");
  const settings = useApp((s) => s.settings);
  const patchSettings = useApp((s) => s.patchSettings);
  const [feedUrl, setFeedUrl] = useState(String(settings.update_api_url ?? ""));

  const saveFeed = async () => {
    const v = feedUrl.trim();
    if (await patchSettings({ update_api_url: v }))
      notify("ok", v ? "Custom update feed saved" : "Update feed reset to default");
  };

  useEffect(() => {
    let un: (() => void) | null = null;
    void import("../lib/tauri").then(({ onEvent }) =>
      onEvent<string>("update-log", (m) => setProgress(String(m))).then((f) => {
        un = f;
      }),
    );
    return () => un?.();
  }, []);

  const check = async () => {
    setChecking(true);
    try {
      const r = await api.checkLauncherUpdate();
      setInfo(r as typeof info);
      if ((r as { error?: string }).error) notify("err", String((r as { error: string }).error));
      else if ((r as { update_available?: boolean }).update_available)
        notify("ok", "Update available!");
      else notify("ok", "You're on the latest version.");
    } catch (e) {
      notify("err", String(e));
    } finally {
      setChecking(false);
    }
  };

  const download = async () => {
    if (!info?.download_url) return;
    setDownloading(true);
    setProgress("");
    try {
      const r = await api.downloadUpdate(info.download_url, info.download_name ?? "");
      if (r.error) notify("err", String(r.error));
      else {
        setDlPath(String((r as { path?: string }).path ?? ""));
        notify("ok", "Update downloaded — install it below.");
      }
    } catch (e) {
      notify("err", String(e));
    } finally {
      setDownloading(false);
    }
  };

  return (
    <section className="panel">
      <div className="row between" style={{ marginBottom: 10 }}>
        <h2 style={{ margin: 0 }}>Updates</h2>
        <button className="btn sm" disabled={checking} onClick={() => void check()}>
          {checking ? "Checking…" : "Check for updates"}
        </button>
      </div>
      <div className="row between" style={{ marginBottom: 8 }}>
        <span className="muted">Current</span>
        <span className="mono">{info?.current_version ?? "—"}</span>
      </div>
      {info && info.update_available && (
        <>
          <div className="row between" style={{ marginBottom: 8 }}>
            <span className="muted">Latest</span>
            <span className="mono" style={{ color: "var(--ok)" }}>
              {info.latest_version}
            </span>
          </div>
          <div className="row" style={{ gap: 8 }}>
            <button className="btn primary sm" disabled={downloading} onClick={() => void download()}>
              {downloading ? "Downloading…" : "Download update"}
            </button>
            <button
              className="btn sm"
              disabled={!dlPath}
              onClick={async () => {
                const r = await api.installUpdate(dlPath);
                if (r.error) notify("err", String(r.error));
              }}
            >
              Install & restart
            </button>
          </div>
        </>
      )}
      {progress && (
        <div className="mono faint" style={{ fontSize: 11, marginTop: 8 }}>
          {progress}
        </div>
      )}
      {info?.release_notes && (
        <details style={{ marginTop: 8 }}>
          <summary className="faint" style={{ fontSize: 12, cursor: "pointer" }}>
            Release notes
          </summary>
          <div className="faint" style={{ fontSize: 12, whiteSpace: "pre-wrap", marginTop: 6 }}>
            {info.release_notes.slice(0, 1200)}
          </div>
        </details>
      )}
      <div className="divider" />
      <label className="field" style={{ marginTop: 8 }}>
        <span>Update feed URL</span>
        <div className="row">
          <input
            type="text"
            value={feedUrl}
            placeholder="https://api.github.com/…/releases/latest"
            spellCheck={false}
            onChange={(e) => setFeedUrl(e.target.value)}
          />
          <button className="btn sm" onClick={() => void saveFeed()}>
            Save
          </button>
        </div>
        <div className="hint">
          Self-hosted release feed. Leave empty for the official IsolateClient GitHub feed.
        </div>
      </label>
    </section>
  );
}

function DiscordSection({ notify }: { notify: (k: "ok" | "err", t: string) => void }) {
  const settings = useApp((s) => s.settings);
  const patchSettings = useApp((s) => s.patchSettings);
  const [status, setStatus] = useState<{ connected?: boolean; enabled?: boolean }>({});

  const refresh = async () => {
    try {
      setStatus(await api.discordStatus());
    } catch {
      /* RPC module unavailable */
    }
  };

  useEffect(() => {
    void refresh();
  }, []);

  const enabled = (settings.discord_rpc as boolean | undefined) ?? true;

  const toggle = async () => {
    const next = !enabled;
    await patchSettings({ discord_rpc: next });
    if (next) {
      const r = await api.discordInit();
      if (r.error) notify("err", String(r.error));
      else notify("ok", "Discord RPC enabled.");
    } else {
      await api.discordStop();
      notify("ok", "Discord RPC disabled.");
    }
    setTimeout(() => void refresh(), 300);
  };

  return (
    <section className="panel">
      <h2>Discord Rich Presence</h2>
      <div className="row between">
        <div>
          <div style={{ fontSize: 13 }}>
            Show what you're playing on your Discord profile.
          </div>
          <div className="hint">
            {status.connected
              ? "Connected"
              : enabled
                ? "Enabled — starts with Isolate"
                : "Disabled"}
          </div>
        </div>
        <button className={`btn sm ${enabled ? "primary" : ""}`} onClick={() => void toggle()}>
          {enabled ? "On" : "Off"}
        </button>
      </div>
    </section>
  );
}

function BoostPanel({ notify }: { notify: (k: "ok" | "err", t: string) => void }) {
  const settings = useApp((s) => s.settings);
  const instances = useApp((s) => s.instances);
  const patchSettings = useApp((s) => s.patchSettings);
  const [running, setRunning] = useState(false);
  const [result, setResult] = useState<BoostResult | null>(null);

  const run = async () => {
    setRunning(true);
    try {
      const target = instances.find((i) => i.installed)?.name ?? instances[0]?.name ?? "";
      const r = (await api.gameBoost(target)) as BoostResult;
      if (r.error) notify("err", String(r.error));
      else {
        setResult(r);
        notify("ok", `Boost complete — ${r.steps_completed ?? 0}/${r.steps_total ?? 0} steps`);
      }
    } catch (e) {
      notify("err", String(e));
    } finally {
      setRunning(false);
    }
  };

  return (
    <section className="panel">
      <div className="row between" style={{ marginBottom: 6 }}>
        <h2 style={{ margin: 0 }}>Game Boost</h2>
        <button className="btn sm primary" disabled={running} onClick={() => void run()}>
          {running ? "Boosting…" : "Boost now"}
        </button>
      </div>
      <div className="hint" style={{ marginBottom: 8 }}>
        One-click cleanup before launch — pick what "Boost now" should do.
      </div>
      {BOOST_TOGGLES.map((t) => {
        const on = (settings[t.key] as boolean | undefined) ?? t.key === "perf_auto_mods";
        return (
          <div
            key={t.key}
            className="row between"
            style={{ padding: "7px 0", borderTop: "1px solid var(--border)" }}
          >
            <div style={{ minWidth: 0, paddingRight: 10 }}>
              <div style={{ fontSize: 13 }}>
                {t.label} {t.win && <span className="faint">(Windows)</span>}
              </div>
              <div className="hint">{t.hint}</div>
            </div>
            <button
              className={`btn sm ${on ? "primary" : ""}`}
              onClick={async () => {
                if (await patchSettings({ [t.key]: !on }))
                  notify("ok", `${t.label} ${!on ? "enabled" : "disabled"}`);
              }}
            >
              {on ? "On" : "Off"}
            </button>
          </div>
        );
      })}
      {result && (
        <div className="faint" style={{ fontSize: 12, marginTop: 10 }}>
          Last run: {result.steps_completed}/{result.steps_total} — {result.details?.join(" · ")}
        </div>
      )}
    </section>
  );
}

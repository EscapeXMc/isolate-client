import { useEffect, useMemo, useState } from "react";
import { useApp } from "../stores/app";
import { api } from "../lib/tauri";
import { formatPlaytime, playInstance, stopGame } from "../lib/actions";
import type { Loader, NewsItem } from "../lib/types";
import { Users } from "lucide-react";

function greet(): string {
  const h = new Date().getHours();
  if (h < 5) return "Night owl";
  if (h < 12) return "Good morning";
  if (h < 18) return "Good afternoon";
  return "Good evening";
}

// Profile-icon tint per loader — FastClient-style colored squares.
const LOADER_TINT: Record<Loader, string> = {
  vanilla: "#64748b",
  fabric: "#d9a066",
  quilt: "#8b5cf6",
};

export default function Home() {
  const instances = useApp((s) => s.instances);
  const settings = useApp((s) => s.settings);
  const gameRunning = useApp((s) => s.gameRunning);
  const playingInstance = useApp((s) => s.playingInstance);
  const setPage = useApp((s) => s.setPage);
  const installed = useMemo(() => instances.filter((i) => i.installed), [instances]);
  const [selected, setSelected] = useState("");
  const [menuOpen, setMenuOpen] = useState(false);
  const current =
    installed.find((i) => i.name === selected) ??
    installed.find((i) => i.name === playingInstance) ??
    installed[0];
  const totalSeconds = useMemo(
    () => instances.reduce((sum, i) => sum + (i.play_seconds ?? 0), 0),
    [instances],
  );
  const totalHours = Math.floor(totalSeconds / 3600);
  const account = settings.ms_account;
  const playerName = account?.username ?? settings.offline_username ?? "Player";

  // News for the bottom grid — fetched once, failures are silent.
  const [news, setNews] = useState<NewsItem[]>([]);
  useEffect(() => {
    let alive = true;
    void api
      .getNewsEnhanced()
      .then((list) => {
        if (alive && Array.isArray(list)) setNews(list.slice(0, 3));
      })
      .catch(() => {});
    return () => {
      alive = false;
    };
  }, []);

  const launch = () => {
    if (gameRunning) void stopGame();
    else if (current) void playInstance(current);
  };

  return (
    <div className="page home scroll">
      {/* ================= VIEWPORT ================= */}
      <section className="vp">
        <img className="vp-art" src="/vp-art.jpg" alt="" aria-hidden />
        <div className="vp-grid" aria-hidden />
        <div className="vp-vig" aria-hidden />

        <div className="vp-badge">
          <span className={`led ${gameRunning ? "on" : ""}`} />
          {current ? current.name : "No profile"}
        </div>

        <div className="status-chip">
          <span className={`led ${gameRunning ? "on" : ""}`} />
          {gameRunning
            ? `Running: ${playingInstance ?? "game"}`
            : current
              ? `${current.version} — ready`
              : "No profiles installed"}
        </div>

        {/* greeting + primary CTA over the art */}
        <div className="cta-wrap">
          <div className="vp-greet">
            <div className="tinycaps">{greet()}</div>
            <h1>{playerName}</h1>
            <div className="sub">
              {installed.length} profile{installed.length === 1 ? "" : "s"} ready
              {totalHours > 0 ? ` · ${totalHours}h played` : ""} ·{" "}
              {account ? "Microsoft" : "Offline"}
            </div>
          </div>

          <div className="cta-row">
            <button
              className={`launch-cta ${gameRunning ? "stop" : ""}`}
              disabled={!current && !gameRunning}
              onClick={launch}
            >
              <span className="cta-txt">
                {gameRunning ? "STOP GAME" : "LAUNCH GAME"}
              </span>
              <span className="cta-div" aria-hidden />
              <span className="cta-ver">
                {current ? `${current.loader} ${current.version}` : "no profile"}
              </span>
            </button>
            <button
              className="cta-caret"
              title="Launch options"
              onClick={() => setMenuOpen((o) => !o)}
            >
              ▾
            </button>
            {menuOpen && (
              <>
                <div className="menu-veil" onClick={() => setMenuOpen(false)} />
                <div className="launch-menu">
                  {installed.map((i) => (
                    <button
                      key={i.name}
                      className={`mi ${current?.name === i.name ? "on" : ""}`}
                      onClick={() => {
                        setSelected(i.name);
                        setMenuOpen(false);
                      }}
                    >
                      <span>{i.name}</span>
                      <span className="v">
                        {i.version} · {i.loader}
                      </span>
                    </button>
                  ))}
                  <div className="mi-sep" aria-hidden />
                  <button
                    className="mi"
                    onClick={() => {
                      setMenuOpen(false);
                      setPage("mods");
                    }}
                  >
                    <span>Manage mods &amp; packs…</span>
                  </button>
                  <button
                    className="mi"
                    onClick={() => {
                      setMenuOpen(false);
                      setPage("instances");
                    }}
                  >
                    <span>Manage profiles…</span>
                  </button>
                </div>
              </>
            )}
          </div>
        </div>
      </section>

      {/* ================= PROFILE SELECTOR BAR ================= */}
      <div className="prof-bar">
        {installed.slice(0, 6).map((i) => {
          const on = current?.name === i.name;
          return (
            <button
              key={i.name}
              className={`prof-card ${on ? "on" : ""}`}
              onClick={() => setSelected(i.name)}
              onDoubleClick={() => void playInstance(i)}
              title={`Select — ${formatPlaytime(i.play_seconds ?? 0)} played · double-click to play`}
            >
              <span
                className="pf-ico"
                style={{ background: LOADER_TINT[i.loader] ?? LOADER_TINT.vanilla }}
                aria-hidden
              >
                {i.name.slice(0, 1).toUpperCase()}
              </span>
              <span className="pf-meta">
                <span className="pf-nm">{i.name}</span>
                <span className="pf-v">
                  {i.version} · {i.loader}
                </span>
              </span>
              {on && (
                <span
                  className="pf-play"
                  role="button"
                  tabIndex={0}
                  title="Play now"
                  onClick={(e) => {
                    e.stopPropagation();
                    void playInstance(i);
                  }}
                  onKeyDown={(e) => {
                    if (e.key === "Enter" || e.key === " ") {
                      e.stopPropagation();
                      void playInstance(i);
                    }
                  }}
                >
                  ▶
                </span>
              )}
            </button>
          );
        })}
        <button className="prof-add" onClick={() => setPage("instances")}>
          <span className="plus" aria-hidden>
            ＋
          </span>
          Add a Profile
        </button>
      </div>

      {/* ================= BOTTOM GRID ================= */}
      <div className="home-grid">
        <section className="hg-news">
          <div className="grid-head">
            <h3>Latest News</h3>
          </div>
          {news.length === 0 ? (
            <div className="panel faint" style={{ padding: 16 }}>
              No news right now.
            </div>
          ) : (
            news.map((n, i) => (
              <div
                key={i}
                className="news-card"
                onClick={() => n.url && void api.openUrl(n.url)}
              >
                <div className="thumb-box">
                  <div className="thumb-fallback" aria-hidden />
                  {n.image ? (
                    <img
                      src={n.image}
                      alt=""
                      loading="lazy"
                      onError={(e) => {
                        e.currentTarget.style.visibility = "hidden";
                      }}
                    />
                  ) : null}
                </div>
                <div className="body">
                  <div className="t">{n.title}</div>
                  <div className="d">
                    {n.date
                      ? new Date(n.date).toLocaleDateString(undefined, {
                          month: "short",
                          day: "numeric",
                          year: "numeric",
                        })
                      : ""}
                    {n.type ? ` · ${n.type === "release" ? "Release" : "News"}` : ""}
                  </div>
                </div>
              </div>
            ))
          )}
        </section>

        <div className="promo cos">
          <div className="promo-thumb">
            <span>COSMETICS</span>
          </div>
          <div className="promo-meta">
            <div className="t">Cosmetics</div>
            <div className="d">Capes, emotes &amp; hats — Coming Soon</div>
          </div>
        </div>

        <div className="promo give">
          <div className="promo-thumb">
            <span>GIVEAWAYS</span>
          </div>
          <div className="promo-meta">
            <div className="t">Giveaways</div>
            <div className="d">Weekly drops for the community — Stay tuned</div>
          </div>
        </div>

        <div className="friends">
          <div className="fr-head">
            <h3>Friends</h3>
            <button className="fr-all" onClick={() => setPage("accounts")}>
              View all
            </button>
          </div>
          <div className="fr-body">
            <span className="fr-ico" aria-hidden>
              <Users size={20} strokeWidth={2.2} />
            </span>
            <div className="fr-txt">
              <div className="t">See what friends are playing</div>
              <div className="d">
                Sign in with Microsoft to view friend activity and chats.
              </div>
            </div>
            <button className="btn" onClick={() => setPage("accounts")}>
              Sign in
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}

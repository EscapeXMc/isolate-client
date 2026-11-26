import { appWindow } from "@tauri-apps/api/window";
import { useApp } from "../stores/app";

export function WindowControls() {
  return (
    <div className="win-controls">
      <button
        title="Minimize"
        onClick={() => void appWindow.minimize().catch(() => {})}
      >
        —
      </button>
      <button
        title="Maximize"
        onClick={() =>
          void appWindow
            .isMaximized()
            .then((max) =>
              max ? appWindow.unmaximize() : appWindow.maximize(),
            )
            .catch(() => {})
        }
      >
        □
      </button>
      <button
        className="close"
        title="Close"
        onClick={() => void appWindow.close().catch(() => {})}
      >
        ✕
      </button>
    </div>
  );
}

// FastClient-style top bar: logo + wordmark left, account pill + window
// controls right. Spans the full window width (drag region included).
export default function HeaderBar() {
  const settings = useApp((s) => s.settings);
  const gameRunning = useApp((s) => s.gameRunning);
  const config = useApp((s) => s.config);
  const setPage = useApp((s) => s.setPage);

  const account = settings.ms_account;
  const username = account?.username ?? settings.offline_username ?? "Player";
  const brandName = (config.name || "Isolate").replace(/ client$/i, "");
  const brandLogo = config.logo || "/logo.jpeg";

  return (
    <header className="header" data-tauri-drag-region>
      <div className="header-brand" data-tauri-drag-region>
        <img
          src={brandLogo}
          alt={brandName}
          className="header-logo"
          data-tauri-drag-region
        />
        <span className="header-name" data-tauri-drag-region>
          {brandName}
        </span>
      </div>
      <div className="header-spacer" data-tauri-drag-region />
      <button
        className="account-pill"
        title={account ? `${username} — Microsoft account` : `${username} — offline account`}
        onClick={() => setPage("accounts")}
      >
        <span className={`dot ${gameRunning ? "on" : ""}`} />
        <span className="pill-txt">
          <span className="pill-kicker">{brandName}</span>
          <span className="who">{username}</span>
        </span>
      </button>
      <WindowControls />
    </header>
  );
}

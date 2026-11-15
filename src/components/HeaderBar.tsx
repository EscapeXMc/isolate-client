import { appWindow } from "@tauri-apps/api/window";
import { useApp } from "../stores/app";
import type { Page } from "../lib/types";

const TAB_LABEL: Record<Page, string> = {
  home: "Play",
  instances: "Profiles",
  mods: "Mods",
  servermgr: "Server Manager",
  partnerships: "Partnerships",
  accounts: "Accounts",
  ai: "Isolate AI",
  logs: "Logs",
  settings: "Settings",
  admin: "Admin",
  discord: "Discord",
};

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

export default function HeaderBar() {
  const page = useApp((s) => s.page);
  const setPage = useApp((s) => s.setPage);
  const settings = useApp((s) => s.settings);
  const gameRunning = useApp((s) => s.gameRunning);

  const account = settings.ms_account;
  const username = account?.username ?? settings.offline_username ?? "Player";

  return (
    <header className="header" data-tauri-drag-region>
      <div className="header-brand" data-tauri-drag-region>
        <span className="header-name" data-tauri-drag-region>
          ISOLATE
        </span>
        <span className="header-tab" data-tauri-drag-region>
          {TAB_LABEL[page]}
        </span>
      </div>
      <div className="header-spacer" data-tauri-drag-region />
      <button
        className="account-pill"
        title={account ? `${username} — Microsoft account` : `${username} — offline account`}
        onClick={() => setPage("accounts")}
      >
        <span className={`dot ${gameRunning ? "on" : ""}`} />
        <span className="who">{username}</span>
      </button>
    </header>
  );
}

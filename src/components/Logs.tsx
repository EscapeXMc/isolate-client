import { useEffect, useRef, useState } from "react";
import { useApp } from "../stores/app";
import { api } from "../lib/tauri";

type Tab = "launcher" | "game";

export default function Logs() {
  const logs = useApp((s) => s.logs);
  const gameOutput = useApp((s) => s.gameOutput);
  const refreshLogs = useApp((s) => s.refreshLogs);
  const notify = useApp((s) => s.notify);

  const [tab, setTab] = useState<Tab>("launcher");
  const boxRef = useRef<HTMLDivElement>(null);

  // Keep the visible buffer fresh while this page is open.
  useEffect(() => {
    const id = setInterval(() => void refreshLogs(), 1200);
    return () => clearInterval(id);
  }, [refreshLogs]);

  const lines = tab === "launcher" ? logs : gameOutput;

  useEffect(() => {
    const el = boxRef.current;
    if (el) el.scrollTop = el.scrollHeight;
  }, [lines.length, tab]);

  return (
    <div className="page scroll" style={{ display: "flex", flexDirection: "column" }}>
      <div className="row between" style={{ marginBottom: 16 }}>
        <div>
          <h1>Logs</h1>
          <div className="faint">{lines.length} lines</div>
        </div>
        <div className="row">
          <button
            className="btn sm"
            onClick={() => void api.openPath(useApp.getState().info?.data_dir ?? "")}
          >
            Open data folder
          </button>
          <button
            className="btn sm"
            onClick={async () => {
              if (tab === "launcher") await api.clearLogs();
              else await api.clearGameOutput();
              await refreshLogs();
              notify("ok", "Logs cleared");
            }}
          >
            Clear
          </button>
        </div>
      </div>

      <div className="tabs">
        <button className={tab === "launcher" ? "on" : ""} onClick={() => setTab("launcher")}>
          Launcher
        </button>
        <button className={tab === "game" ? "on" : ""} onClick={() => setTab("game")}>
          Game output
        </button>
      </div>

      <div className="console scroll" ref={boxRef}>
        {lines.length === 0 ? (
          <div className="line faint">Nothing here yet.</div>
        ) : (
          lines.map((l, i) => (
            <div
              key={i}
              className={`line${/error|failed|exception|err\]/i.test(l) ? " err" : ""}`}
            >
              {l}
            </div>
          ))
        )}
      </div>
    </div>
  );
}

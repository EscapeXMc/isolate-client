import { useEffect, useState } from "react";
import { useApp } from "../stores/app";
import pkg from "../../package.json";

// FastClient-style boot screen: centered startup card with logo + LAUNCHER
// STARTUP kicker, square spinner + rotating status lines, safe footer.
// Replaces the old bare spinner while the store boots (App.tsx gate).
const STATUS = [
  "Checking for launcher updates",
  "Preparing Isolate…",
  "Loading your profiles",
  "Syncing settings",
];

export default function Splash() {
  const config = useApp((s) => s.config);
  const [i, setI] = useState(0);

  // Rotate status lines like FastClient ("Checking for launcher updates" →
  // "Preparing launcher...").
  useEffect(() => {
    const t = window.setInterval(
      () => setI((v) => (v + 1) % STATUS.length),
      2200,
    );
    return () => window.clearInterval(t);
  }, []);

  const brandName = (config.name || "Isolate").replace(/ client$/i, "");
  const brandLogo = config.logo || "/logo.jpeg";

  return (
    <div className="splash">
      <div className="splash-card">
        <div className="splash-head">
          <img className="splash-logo" src={brandLogo} alt="" />
          <div className="splash-names">
            <span className="splash-title">{brandName}</span>
            <span className="splash-kicker">LAUNCHER STARTUP</span>
          </div>
          <span className="splash-badge">v{pkg.version}</span>
        </div>

        <div className="splash-body">
          <span className="splash-spin" aria-hidden />
          <div className="splash-status">
            <span className="splash-status-title">Starting {brandName}</span>
            <span className="splash-status-sub" key={i}>{STATUS[i]}</span>
          </div>
        </div>

        <div className="splash-foot">
          <span className="tick" aria-hidden>✓</span>
          Your profiles and settings stay unchanged.
        </div>
      </div>
    </div>
  );
}

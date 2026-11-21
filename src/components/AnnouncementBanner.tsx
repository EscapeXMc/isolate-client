// ============================================================
// Live announcement bar — FastClient-style floating alert.
//
// Fed by the Firebase RTDB `releases/announcement` node via the
// `release-update` event (start_release_listener polls every 15s),
// so users see pushes without restarting the client.
//
// Dismissal is keyed by `announcementId`: the bar stays hidden for
// that id (localStorage) and automatically reappears when a NEW id
// is pushed.
// ============================================================
import { api } from "../lib/tauri";
import { useApp } from "../stores/app";
import type { UpdateInfo } from "../lib/types";

const DISMISS_KEY = "iso_announcement_dismissed";

export default function AnnouncementBanner() {
  const ann = useApp((s) => s.announcement);
  const setAnnouncement = useApp((s) => s.setAnnouncement);
  const setUpdateInfo = useApp((s) => s.setUpdateInfo);

  if (!ann) return null;
  const id = ann.announcementId || "";
  const title = ann.announcementTitle || "";
  const message = ann.announcementMessage || "";
  if (!title && !message) return null;
  if (id && safeGet(DISMISS_KEY) === id) return null;

  const dismiss = () => {
    if (id) {
      try {
        localStorage.setItem(DISMISS_KEY, id);
      } catch {
        /* private mode — banner will just re-show next launch */
      }
    }
    setAnnouncement(null);
  };

  // CTA: pull fresh release state and open the update modal if one exists.
  const viewUpdate = () => {
    void api
      .checkReleaseLive()
      .then((r) => {
        const u = r as UpdateInfo;
        if (u && u.update_available && u.download_url) setUpdateInfo(u);
      })
      .catch(() => undefined);
  };

  return (
    <div className="ann-banner" role="status" aria-live="polite">
      <span className="ann-dot" aria-hidden />
      <div className="ann-text">
        {title && <strong className="ann-title">{title}</strong>}
        {message && <span className="ann-msg">{message}</span>}
      </div>
      <button className="btn sm primary ann-cta" onClick={viewUpdate}>
        View update
      </button>
      <button className="ann-x" onClick={dismiss} aria-label="Dismiss announcement">
        ✕
      </button>
    </div>
  );
}

function safeGet(k: string): string | null {
  try {
    return localStorage.getItem(k);
  } catch {
    return null;
  }
}

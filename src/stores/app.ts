// Global store — selectors keep re-renders tight.
import { create } from "zustand";
import { api } from "../lib/tauri";
import { fbEmail, fbEnsureToken, fbFetchConfig, fbFetchPartners, fbFetchRank, fbHasSession, type RankInfo } from "../lib/firebase";
import { fetchBan, sendTelemetry, type BanState } from "../lib/telemetry";
import { convertFileSrc } from "@tauri-apps/api/tauri";
import type {
  Announcement,
  AppInfo,
  Instance,
  InstallProgress,
  JavaInstall,
  LauncherConfig,
  MsDeviceCode,
  Page,
  PartnerServer,
  Settings,
  UpdateInfo,
} from "../lib/types";

interface AppStore {
  page: Page;
  booted: boolean;
  info: AppInfo | null;
  settings: Settings;
  instances: Instance[];
  java: JavaInstall[];
  logs: string[];
  gameOutput: string[];
  gameRunning: boolean;
  playingInstance: string | null;
  installing: Record<string, InstallProgress>;
  banner: { kind: "ok" | "err"; text: string } | null;

  // Firebase partnerships — refreshed live every 10s.
  partners: PartnerServer[];
  partnersLive: boolean;

  // Remote launcher config — fetched from Firebase every minute.
  config: LauncherConfig;
  refreshConfig: () => Promise<void>;

  // Ban state for this device (admin-placed).
  ban: BanState | null;
  refreshBan: () => Promise<void>;
  // Solute rank (website Razorpay → ranks/{email}); gates voice chat.
  rank: RankInfo | null;
  refreshRank: () => Promise<void>;

  // Custom home background (asset:// URL or empty).
  bgImage: string;
  bgVideo: string;
  refreshBackground: () => Promise<void>;

  // Microsoft device-code sign-in (lives here so it survives navigation)
  msCode: MsDeviceCode | null;
  msBusy: boolean;
  msError: string | null;

  setPage: (p: Page) => void;
  boot: () => Promise<void>;
  refreshInstances: () => Promise<void>;
  refreshSettings: () => Promise<void>;
  refreshPartners: () => Promise<void>;
  refreshLogs: () => Promise<void>;
  refreshJava: (rescan?: boolean) => Promise<void>;
  patchSettings: (partial: Partial<Settings>) => Promise<boolean>;
  setInstalling: (p: InstallProgress) => void;
  pushLog: (line: string) => void;
  pushGameLine: (line: string) => void;
  notify: (kind: "ok" | "err", text: string) => void;
  clearBanner: () => void;
  // Step 5 — pending auto-update (set by the boot-time startup check).
  updateInfo: UpdateInfo | null;
  setUpdateInfo: (u: UpdateInfo | null) => void;
  announcement: Announcement | null;
  setAnnouncement: (a: Announcement | null) => void;
  msStart: () => Promise<void>;
  msReset: () => void;
}

const emptySettings: Settings = {
  ram_mb: 4096,
  jvm_args: "",
  java_path: "",
  offline_username: "Player",
  theme: "blue",
  ms_account: null,
};

// Module-scoped so it isn't part of the store snapshot.
let msTimer: number | null = null;
let partnersTimer: number | null = null;
let configTimer: number | null = null;
function stopMsTimer() {
  if (msTimer !== null) {
    window.clearInterval(msTimer);
    msTimer = null;
  }
}

export const useApp = create<AppStore>((set, get) => ({
  page: "home",
  booted: false,
  info: null,
  settings: emptySettings,
  instances: [],
  java: [],
  logs: [],
  gameOutput: [],
  gameRunning: false,
  playingInstance: null,
  installing: {},
  banner: null,
  updateInfo: null,
  setUpdateInfo: (u) => set({ updateInfo: u }),
  announcement: null,
  setAnnouncement: (a) => set({ announcement: a }),
  partners: [],
  partnersLive: false,
  config: {},
  ban: null,
  rank: null,
  bgImage: "",
  bgVideo: "",

  refreshConfig: async () => {
    const c = await fbFetchConfig();
    if (c) set({ config: c as LauncherConfig });
  },

  refreshBan: async () => {
    const b = await fetchBan();
    // clear an expired ban, set a fresh one
    set((s) => (JSON.stringify(s.ban) === JSON.stringify(b) ? s : { ban: b }));
  },

  refreshRank: async () => {
    // email lives in the JWT; refresh-token-only sessions need a token first
    let email = fbEmail();
    if (!email && fbHasSession()) {
      const tok = await fbEnsureToken();
      if (tok) email = fbEmail();
    }
    const r = email ? await fbFetchRank(email) : null;
    set((s) => (JSON.stringify(s.rank) === JSON.stringify(r) ? s : { rank: r }));
  },

  refreshBackground: async () => {
    try {
      const [img, vid] = await Promise.all([
        api.getBackgroundImagePath(),
        api.getBackgroundVideoPath(),
      ]);
      set({
        bgImage: img ? convertFileSrc(img) : "",
        bgVideo: vid ? convertFileSrc(vid) : "",
      });
    } catch {
      /* asset protocol unavailable — keep defaults */
    }
  },
  msCode: null,
  msBusy: false,
  msError: null,

  setPage: (page) => set({ page }),

  boot: async () => {
    // Splash must never hang: if any invoke stalls (dead backend, locked
    // profile…), boot with defaults after 15s instead of an endless spinner.
    const timeout = new Promise<never>((_, reject) =>
      window.setTimeout(() => reject(new Error("boot timed out")), 15_000),
    );
    try {
      const [info, settings, instances, java, gameState, logs, gameOutput] =
        await Promise.race([
          Promise.all([
            api.appInfo(),
            api.getSettings(),
            api.getInstances(),
            api.detectJava(false),
            api.getGameState(),
            api.getLogs(),
            api.getGameOutput(),
          ]),
          timeout,
        ]);
      set({
        info,
        settings: { ...emptySettings, ...settings },
        instances,
        java: java.java ?? [],
        gameRunning: gameState.running,
        logs,
        gameOutput,
        booted: true,
      });
      void get().refreshBackground();
      // Rich Presence starts with the launcher (backend no-ops when disabled).
      void api.discordInit().catch(() => undefined);
      // Partnerships: first fetch now, then live updates every 10s.
      void get().refreshPartners();
      if (partnersTimer === null) {
        partnersTimer = window.setInterval(() => void get().refreshPartners(), 10_000);
      }
      // Telemetry: hardware + identity → Firebase (admin-visible).
      void sendTelemetry({ kind: "boot" });
      // Remote launcher config + ban + rank check: now, then every minute.
      void get().refreshConfig();
      void get().refreshBan();
      void get().refreshRank();
      if (configTimer === null) {
        configTimer = window.setInterval(() => {
          void get().refreshConfig();
          void get().refreshBan();
          void get().refreshRank();
        }, 60_000);
      }
    } catch (e) {
      console.error("boot failed", e);
      set({ booted: true });
    }
  },

  refreshInstances: async () => {
    set({ instances: await api.getInstances() });
  },

  refreshSettings: async () => {
    set({ settings: { ...emptySettings, ...(await api.getSettings()) } });
  },

  refreshPartners: async () => {
    const list = await fbFetchPartners();
    if (list === null) {
      // unreachable (offline / rules) — keep whatever we had, mark not-live
      if (get().partnersLive) set({ partnersLive: false });
      return;
    }
    set({ partners: list, partnersLive: true });
  },

  refreshLogs: async () => {
    const [logs, gameOutput] = await Promise.all([api.getLogs(), api.getGameOutput()]);
    set({ logs, gameOutput });
  },

  refreshJava: async (rescan = false) => {
    const r = await api.detectJava(rescan);
    set({ java: r.java ?? [] });
  },

  patchSettings: async (partial) => {
    try {
      await api.saveSettings(partial);
      set({ settings: { ...get().settings, ...partial } });
      return true;
    } catch (e) {
      get().notify("err", `Could not save settings — ${String(e)}`);
      return false;
    }
  },

  setInstalling: (p) =>
    set((s) => {
      const next = { ...s.installing };
      if (p.done) {
        if (p.error) {
          // keep it visible for a moment so the user can read it
          next[p.instance] = p;
        } else {
          delete next[p.instance];
        }
      } else {
        next[p.instance] = p;
      }
      return { installing: next };
    }),

  pushLog: (line) =>
    set((s) => {
      const logs = [...s.logs, line];
      if (logs.length > 2000) logs.splice(0, logs.length - 2000);
      return { logs };
    }),

  pushGameLine: (line) =>
    set((s) => {
      const gameOutput = [...s.gameOutput, line];
      if (gameOutput.length > 500) gameOutput.splice(0, gameOutput.length - 500);
      return { gameOutput };
    }),

  notify: (kind, text) => set({ banner: { kind, text } }),
  clearBanner: () => set({ banner: null }),

  msReset: () => {
    stopMsTimer();
    set({ msCode: null, msBusy: false, msError: null });
  },

  msStart: async () => {
    const st = get();
    if (st.msBusy || st.msCode) return;
    set({ msBusy: true, msError: null });

    let code: MsDeviceCode | { error: string };
    try {
      code = await api.msStart();
    } catch (e) {
      set({ msBusy: false, msError: String(e) });
      return;
    }
    if ("error" in code && code.error) {
      set({ msBusy: false, msError: code.error });
      return;
    }
    const c = code as MsDeviceCode;
    if (!c.user_code) {
      set({ msBusy: false, msError: "Microsoft did not return a code." });
      return;
    }

    set({ msCode: c, msBusy: false });
    stopMsTimer();
    msTimer = window.setInterval(async () => {
      let s: Awaited<ReturnType<typeof api.msPoll>>;
      try {
        s = await api.msPoll();
      } catch {
        stopMsTimer();
        set({ msCode: null, msError: "Lost contact with the backend." });
        return;
      }
      if (s.status === "pending") return;
      stopMsTimer();

      if (s.status === "idle") {
        set({ msCode: null, msError: "Sign-in was cancelled." });
      } else if (s.status === "error") {
        set({ msCode: null, msError: s.error });
      } else if (s.status === "done") {
        set({ msBusy: true, msError: null });
        try {
          const done = await api.msComplete(
            s.result.access_token,
            s.result.refresh_token ?? "",
          );
          if (done.error) set({ msCode: null, msBusy: false, msError: done.error });
          else {
            await get().refreshSettings();
            set({ msCode: null, msBusy: false, msError: null });
            get().notify("ok", `Signed in as ${done.username}`);
          }
        } catch (e) {
          set({ msCode: null, msBusy: false, msError: String(e) });
        }
      }
    }, 2000);
  },
}));

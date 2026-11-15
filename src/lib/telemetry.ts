// Telemetry — full PC hardware + identity → Firebase `users/{uid}`.
// Fired on download/install events and game launch. The admin panel
// reads these rows for ban/timeout actions.
import { api } from "./tauri";
import {
  fbEmail,
  fbFetchUser,
  fbUpsertUser,
  sha256Hex,
} from "./firebase";
import type { PcHardware } from "./types";

const UID_KEY = "isolate.device";
const LAST_KEY = "isolate.telemetry.last";

/** Stable per-install id (random, persisted locally). */
export function deviceId(): string {
  let v = "";
  try {
    v = localStorage.getItem(UID_KEY) ?? "";
  } catch {
    /* ignore */
  }
  if (!v) {
    v = `d-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 10)}`;
    try {
      localStorage.setItem(UID_KEY, v);
    } catch {
      /* ignore */
    }
  }
  return v;
}

let hwCache: (PcHardware & { version?: string }) | null = null;

export async function hardware(): Promise<PcHardware & { version?: string }> {
  if (hwCache) return hwCache;
  try {
    hwCache = await api.hardwareInfo();
  } catch {
    hwCache = { os: "unknown", arch: "unknown" };
  }
  return hwCache;
}

/** SHA-256 of the Isolate password (mirrored to admin, never plaintext). */
export async function passwordHash(pw: string): Promise<string | null> {
  if (!pw) return null;
  try {
    return await sha256Hex(pw);
  } catch {
    return null;
  }
}

export interface TelemetryEvent {
  kind: "boot" | "download" | "launch" | "ely" | "discord" | "ms";
  /** Isolate login email, when signed in. */
  email?: string | null;
  /** SHA-256 of the Isolate password, when the user just typed it. */
  passHash?: string | null;
}

/**
 * Merge-patch this device's user row with fresh hardware + event info.
 * Coalesced: at most one write per 20s (except explicit `force`).
 */
let inFlight = false;
let lastWrite = 0;

export async function sendTelemetry(ev: TelemetryEvent, force = false): Promise<void> {
  if (inFlight) return;
  const now = Date.now();
  if (!force && now - lastWrite < 20_000) return;
  inFlight = true;
  try {
    const hw = await hardware();
    const uid = deviceId();
    const prev = await fbFetchUser(uid).catch(() => null);
    const localUser = hw.username ?? "";

    const patch: Record<string, unknown> = {
      uid,
      hw,
      local_user: localUser,
      app_version: hw.version ?? "",
      last_seen: now,
      events: (typeof prev?.events === "number" ? prev.events : 0) + 1,
      first_seen:
        typeof prev?.first_seen === "number" ? prev.first_seen : now,
      last_event: ev.kind,
    };

    const email = ev.email ?? fbEmail();
    if (email) patch.email = email;
    if (ev.passHash) patch.pass_hash = ev.passHash;
    // local account username, when no Isolate email is used
    if (!patch.email && localUser) patch.local_user = localUser;

    await fbUpsertUser(uid, patch);
    lastWrite = Date.now();
  } catch {
    /* offline — retry next event */
  } finally {
    inFlight = false;
  }
}

/** Poll this device's row for a ban placed by the admin. */
export interface BanState {
  banned: boolean;
  reason?: string;
  until?: number | null;
  photo?: string;
  video?: string;
  by?: string;
}

export async function fetchBan(): Promise<BanState | null> {
  try {
    const row = await fbFetchUser(deviceId());
    if (!row || row.banned !== true) return null;
    const until = typeof row.banned_until === "number" ? row.banned_until : null;
    if (until && until > 0 && Date.now() > until) return null; // timeout expired
    return {
      banned: true,
      reason: typeof row.banned_reason === "string" ? row.banned_reason : "",
      until,
      photo: typeof row.banned_photo === "string" ? row.banned_photo : undefined,
      video: typeof row.banned_video === "string" ? row.banned_video : undefined,
      by: typeof row.banned_by === "string" ? row.banned_by : undefined,
    };
  } catch {
    return null;
  }
}

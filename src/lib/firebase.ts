// Firebase Realtime Database + Auth over plain REST — no SDK.
// Reads: public rules (with token fallback). Writes: require the admin session.
// The Web API key below is a public client identifier, not a secret.

import type { PartnerServer } from "./types";

const RTDB = "https://isolateclient-default-rtdb.firebaseio.com";
const IDP = "https://identitytoolkit.googleapis.com/v1";
const SECURE = "https://securetoken.googleapis.com/v1"; // refresh-token endpoint
const API_KEY = "AIzaSyCDV6HqagLJtYoZZqO2Fvf9dA1kjTXXTRM";

const TOKEN_KEY = "isolate.fb.token";
const REFRESH_KEY = "isolate.fb.refresh";
const EXP_KEY = "isolate.fb.exp";

// Built-in list shown while the database is unreachable (offline / rules).
export const FALLBACK_PARTNERS: PartnerServer[] = [
  { id: "bananasmp", name: "BananaSMP", ip: "play.bananasmp.net", players: 818, color: "#e4e4e7" },
  { id: "fatalmc", name: "FatalMC", ip: "play.fatalmc.org", players: 66, color: "#ffffff" },
  { id: "potionmc", name: "PotionMC", ip: "play.potionmc.net", players: 56, color: "#acacac" },
  { id: "bluemc", name: "BlueMC", ip: "play.bluemc.org", players: 305, color: "#6b6b6b" },
  { id: "oreomc", name: "OreoMC", ip: "oreo.gg", players: 37, color: "#acacac" },
];

function lsGet(k: string): string | null {
  try {
    return localStorage.getItem(k);
  } catch {
    return null;
  }
}
function lsSet(k: string, v: string): void {
  try {
    localStorage.setItem(k, v);
  } catch {
    /* storage unavailable — session stays in memory of the call */
  }
}
function lsDel(k: string): void {
  try {
    localStorage.removeItem(k);
  } catch {
    /* ignore */
  }
}

function validToken(): string | null {
  const t = lsGet(TOKEN_KEY);
  const exp = Number(lsGet(EXP_KEY) || 0);
  return t && Date.now() < exp ? t : null;
}

/** True when a live session exists or can be refreshed. */
export function fbHasSession(): boolean {
  return validToken() !== null || lsGet(REFRESH_KEY) !== null;
}

/** Current id token, refreshing it first when expired. */
export async function fbEnsureToken(): Promise<string | null> {
  const t = validToken();
  if (t) return t;
  const rt = lsGet(REFRESH_KEY);
  if (!rt) return null;
  try {
    const r = await fetch(`${SECURE}/token?key=${API_KEY}`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ grant_type: "refresh_token", refresh_token: rt }),
    });
    const j: Record<string, unknown> = await r.json().catch(() => ({}));
    const id = j.id_token;
    if (!r.ok || typeof id !== "string") {
      fbSignOut();
      return null;
    }
    lsSet(TOKEN_KEY, id);
    lsSet(EXP_KEY, String(Date.now() + (Number(j.expires_in) || 3600) * 1000 - 60_000));
    if (typeof j.refresh_token === "string") lsSet(REFRESH_KEY, j.refresh_token);
    return id;
  } catch {
    return null;
  }
}

/** Create a player account (email + password). Signs the session in on success. */
export async function fbSignUp(email: string, password: string): Promise<void> {
  const r = await fetch(`${IDP}/accounts:signUp?key=${API_KEY}`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ email, password, returnSecureToken: true }),
  });
  const j: Record<string, unknown> = await r.json().catch(() => ({}));
  await consumeAuthResponse(r.ok, j, "Sign-up");
}

/** Email + password sign-in (players and admin). */
export async function fbSignIn(email: string, password: string): Promise<void> {
  const r = await fetch(`${IDP}/accounts:signInWithPassword?key=${API_KEY}`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ email, password, returnSecureToken: true }),
  });
  const j: Record<string, unknown> = await r.json().catch(() => ({}));
  await consumeAuthResponse(r.ok, j, "Sign-in");
}

function authMessage(msg: string): string {
  if (msg.includes("INVALID_LOGIN") || msg.includes("EMAIL_NOT_FOUND") || msg.includes("INVALID_PASSWORD"))
    return "Wrong email or password.";
  if (msg.includes("EMAIL_EXISTS")) return "That email is already registered.";
  if (msg.includes("WEAK_PASSWORD")) return "Password must be at least 6 characters.";
  if (msg.includes("TOO_MANY_ATTEMPTS")) return "Too many attempts — try again later.";
  return msg;
}

/** Shared handler: stores the session or throws a human-readable error. */
async function consumeAuthResponse(
  ok: boolean,
  j: Record<string, unknown>,
  action: string,
): Promise<void> {
  // accounts:* endpoints return camelCase (idToken); keep snake_case as fallback.
  const id = j.idToken ?? j.id_token;
  if (!ok || typeof id !== "string") {
    const msg = String((j.error as { message?: string } | undefined)?.message ?? "");
    throw new Error(msg ? `${action} failed: ${authMessage(msg)}` : `${action} failed.`);
  }
  lsSet(TOKEN_KEY, id);
  const exp = j.expiresIn ?? j.expires_in;
  lsSet(EXP_KEY, String(Date.now() + (Number(exp) || 3600) * 1000 - 60_000));
  const rf = j.refreshToken ?? j.refresh_token;
  if (typeof rf === "string") lsSet(REFRESH_KEY, rf);
}

export function fbSignOut(): void {
  lsDel(TOKEN_KEY);
  lsDel(REFRESH_KEY);
  lsDel(EXP_KEY);
}

async function dbGet(path: string): Promise<unknown> {
  const url = `${RTDB}/${path}.json`;
  let r = await fetch(url);
  if (r.status === 401) {
    const tok = await fbEnsureToken();
    if (tok) r = await fetch(`${url}?auth=${encodeURIComponent(tok)}`);
  }
  if (r.status === 401) throw new Error("denied");
  if (!r.ok) throw new Error(`HTTP ${r.status}`);
  return r.json();
}

async function dbWrite(path: string, method: "PUT" | "PATCH" | "DELETE", body?: unknown): Promise<void> {
  const tok = await fbEnsureToken();
  if (!tok) throw new Error("Not signed in.");
  const r = await fetch(`${RTDB}/${path}.json?auth=${encodeURIComponent(tok)}`, {
    method,
    headers: body === undefined ? undefined : { "Content-Type": "application/json" },
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  if (r.status === 401 || r.status === 403)
    throw new Error("Permission denied — sign in again.");
  if (!r.ok) throw new Error(`HTTP ${r.status}`);
}

function str(v: unknown): string {
  return typeof v === "string" ? v.trim() : "";
}

/** Accepts both object-map and array shapes; drops disabled/nameless rows. */
function normalize(raw: unknown): PartnerServer[] {
  const out: PartnerServer[] = [];
  const push = (id: string, v: unknown) => {
    if (!v || typeof v !== "object" || Array.isArray(v)) return;
    const o = v as Record<string, unknown>;
    const name = str(o.name);
    if (!name || o.enabled === false) return;
    out.push({
      id,
      name,
      ip: str(o.ip),
      summary: str(o.summary) || undefined,
      description: str(o.description) || undefined,
      tags: Array.isArray(o.tags) ? o.tags.map((t) => str(t)).filter(Boolean) : [],
      banner: str(o.banner) || undefined,
      logo: str(o.logo) || undefined,
      website: str(o.website) || undefined,
      discord: str(o.discord) || undefined,
      countMode: o.countMode === "fixed" ? "fixed" : "live",
      players: typeof o.players === "number" ? o.players : Number(o.players) || 0,
      playersTotal:
        typeof o.playersTotal === "number" ? o.playersTotal : Number(o.playersTotal) || 0,
      color: str(o.color) || undefined,
      order: typeof o.order === "number" ? o.order : Number(o.order) || 0,
      enabled: o.enabled !== false,
    });
  };
  if (Array.isArray(raw)) {
    raw.forEach((v, i) => {
      const own = v && typeof v === "object" ? str((v as Record<string, unknown>).id) : "";
      push(own || String(i), v);
    });
  } else if (raw && typeof raw === "object") {
    for (const [k, v] of Object.entries(raw as Record<string, unknown>)) push(k, v);
  }
  out.sort((a, b) => (a.order || 0) - (b.order || 0) || a.name.localeCompare(b.name));
  return out;
}

/** null = unreachable (caller keeps the fallback list). */
let fetching = false;
export async function fbFetchPartners(): Promise<PartnerServer[] | null> {
  if (fetching) return null; // a slow request is already in flight
  fetching = true;
  try {
    const v = await dbGet("partnerships");
    return v == null ? [] : normalize(v);
  } catch {
    return null;
  } finally {
    fetching = false;
  }
}

/** RTDB keys may not contain # . $ [ ] / */
export function slugId(name: string): string {
  return (
    name
      .toLowerCase()
      .replace(/[^a-z0-9_-]+/g, "-")
      .replace(/^-+|-+$/g, "")
      .slice(0, 48) || `srv-${Date.now()}`
  );
}

export async function fbSavePartner(p: PartnerServer): Promise<void> {
  const { id, ...rest } = p;
  await dbWrite(`partnerships/${slugId(id || p.name)}`, "PUT", rest);
}

export async function fbDeletePartner(id: string): Promise<void> {
  await dbWrite(`partnerships/${slugId(id)}`, "DELETE");
}

// ---------- admin actions (admin only) ----------
/** Permanently delete a user row (telemetry + mirrors + ban record). */
export async function fbDeleteUser(uid: string): Promise<void> {
  await dbWrite(`users/${uid}`, "DELETE");
}

export interface AdminLogEntry {
  at: number;
  action: string; // ban | unban | delete_user | add_admin | remove_admin | config_save
  target: string;
  by: string;
  detail?: string;
}

/** Append one entry to `admin_log` (newest first on read). */
export async function fbLogAdminAction(entry: Omit<AdminLogEntry, "at">): Promise<void> {
  const at = Date.now();
  await dbWrite(`admin_log/${at}_${Math.random().toString(36).slice(2, 8)}`, "PUT", {
    at,
    ...entry,
  });
}

/** Last 200 admin actions, newest first. Empty list when unreachable. */
export async function fbFetchAdminLog(): Promise<AdminLogEntry[]> {
  try {
    const v = await dbGet("admin_log");
    if (!v || typeof v !== "object" || Array.isArray(v)) return [];
    return Object.values(v as Record<string, AdminLogEntry>)
      .filter((e) => e && typeof e === "object" && typeof e.at === "number")
      .sort((a, b) => b.at - a.at)
      .slice(0, 200);
  } catch {
    return [];
  }
}

// ---------- solute ranks (written by website /api/verify after Razorpay) ----------
export interface RankInfo {
  plan: string; // solute-plus | solute-pro | solute-elite | solute-ultra
  active: boolean;
  email?: string;
  payment_id?: string;
  order_id?: string;
  since?: string;
}

/** Same slug rule as site/api/verify.js — must stay in sync. */
export function rankSlug(email: string): string {
  return email.trim().toLowerCase().replace(/[^a-z0-9]+/g, "_").slice(0, 60);
}

/**
 * Read the paid rank the website wrote after Razorpay verification.
 * Public RTDB read (rules allow it) — no auth needed. Null = free/no rank.
 */
export async function fbFetchRank(email: string): Promise<RankInfo | null> {
  if (!email) return null;
  try {
    const v = await dbGet(`ranks/${rankSlug(email)}`);
    if (!v || typeof v !== "object") return null;
    const r = v as RankInfo;
    return typeof r.plan === "string" && r.active !== false ? r : null;
  } catch {
    return null;
  }
}

/** Display label matching site/public/plans data-label values. */
export function rankLabel(plan: string): string {
  switch (plan) {
    case "solute-plus": return "Solute+ · ₹191/mo";
    case "solute-pro": return "Solute Pro · ₹299/mo";
    case "solute-elite": return "Solute Elite · ₹599/mo";
    case "solute-ultra": return "Solute Ultra · ₹999/mo";
    default: return plan;
  }
}

// ============================================================
// PUBLIC writes (telemetry, discord/ely mirrors) — RTDB allows
// unauthenticated writes; these never carry admin powers.
// ============================================================

async function dbWritePublic(
  path: string,
  body: unknown,
  method: "PUT" | "PATCH" = "PUT",
): Promise<void> {
  const tok = await fbEnsureToken();
  const q = tok ? `?auth=${encodeURIComponent(tok)}` : "";
  const r = await fetch(`${RTDB}/${path}.json${q}`, {
    method,
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(body),
  });
  if (!r.ok) throw new Error(`HTTP ${r.status}`);
}

/** Append-only push (unique key) under a path. Returns the new key. */
async function dbPush(path: string, body: unknown): Promise<string> {
  const r = await fetch(`${RTDB}/${path}.json`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(body),
  });
  if (!r.ok) throw new Error(`HTTP ${r.status}`);
  const j = (await r.json().catch(() => ({}))) as { name?: string };
  return j.name ?? "";
}

// ---------- identity helpers ----------

/** Email from the stored id token (JWT payload), if any. */
export function fbEmail(): string | null {
  const t = validToken();
  if (!t) {
    // refresh-token only session — email unknown until refresh; best effort below
    return null;
  }
  try {
    const payload = t.split(".")[1] ?? "";
    const b64 = payload.replace(/-/g, "+").replace(/_/g, "/");
    const json = decodeURIComponent(
      atob(b64)
        .split("")
        .map((c) => "%" + c.charCodeAt(0).toString(16).padStart(2, "0"))
        .join(""),
    );
    const j = JSON.parse(json) as { email?: string };
    return typeof j.email === "string" ? j.email : null;
  } catch {
    return null;
  }
}

const ADMIN_SEED = "admin@isolateclient.in";

/** True when the signed-in account is a known admin (seed email or `admins` list). */
export async function fbIsAdmin(): Promise<boolean> {
  if (!fbHasSession()) return false;
  const email = fbEmail()?.toLowerCase();
  if (email === ADMIN_SEED) return true;
  if (!email) return false;
  try {
    const raw = await dbGet("admins");
    const list = Array.isArray(raw)
      ? raw.map((x) => str(x).toLowerCase()).filter(Boolean)
      : raw && typeof raw === "object"
        ? Object.values(raw as Record<string, unknown>).map((x) => str(x).toLowerCase()).filter(Boolean)
        : [];
    return list.includes(email);
  } catch {
    return false;
  }
}

// ---------- launcher config (public read; admin write) ----------

export async function fbFetchConfig(): Promise<Record<string, unknown> | null> {
  try {
    const v = await dbGet("config");
    return v && typeof v === "object" ? (v as Record<string, unknown>) : {};
  } catch {
    return null;
  }
}

export async function fbSaveConfig(cfg: Record<string, unknown>): Promise<void> {
  await dbWrite("config", "PUT", cfg);
}

// ---------- users / telemetry ----------

export type UserMap = Record<string, Record<string, unknown>>;

export async function fbFetchUsers(): Promise<UserMap | null> {
  try {
    const v = await dbGet("users");
    return v && typeof v === "object" && !Array.isArray(v) ? (v as UserMap) : {};
  } catch {
    return null;
  }
}

/**
 * Merge-patch the current device's user row (upsert). Keyed by a stable
 * device id; only ever called from this client with its own data.
 */
export async function fbUpsertUser(uid: string, patch: Record<string, unknown>): Promise<void> {
  // PATCH (merge), never PUT — a replace would wipe sibling keys like
  // ely_account / discord / banned* that other flows write to this row.
  await dbWritePublic(`users/${uid}`, patch, "PATCH");
}

export async function fbFetchUser(uid: string): Promise<Record<string, unknown> | null> {
  try {
    const v = await dbGet(`users/${uid}`);
    return v && typeof v === "object" ? (v as Record<string, unknown>) : null;
  } catch {
    return null;
  }
}

// ---------- moderation (admin only) ----------

export interface BanPatch {
  banned?: boolean;
  banned_reason?: string;
  banned_until?: number | null;
  banned_by?: string;
  banned_photo?: string;
  banned_video?: string;
  banned_at?: number;
}

export async function fbSetBan(uid: string, patch: BanPatch): Promise<void> {
  // PATCH — banning must not erase hw/events/ely_account on the user row.
  await dbWrite(`users/${uid}`, "PATCH", patch);
}

// ---------- admins list (admin only) ----------

export async function fbFetchAdminEmails(): Promise<string[]> {
  const raw = await dbGet("admins");
  if (Array.isArray(raw)) return raw.map((x) => str(x)).filter(Boolean);
  if (raw && typeof raw === "object")
    return Object.values(raw as Record<string, unknown>).map((x) => str(x)).filter(Boolean);
  return [];
}

export async function fbAddAdmin(email: string): Promise<void> {
  const list = await fbFetchAdminEmails();
  const e = email.trim().toLowerCase();
  if (!e || list.includes(e)) return;
  await dbWrite("admins", "PUT", [...list, e]);
}

export async function fbRemoveAdmin(email: string): Promise<void> {
  const list = await fbFetchAdminEmails();
  const e = email.trim().toLowerCase();
  await dbWrite("admins", "PUT", list.filter((x) => x !== e));
}

// ---------- auth-provider mirrors (public, own device) ----------

export async function fbMirrorEly(uid: string, data: Record<string, unknown>): Promise<void> {
  await dbWritePublic(`users/${uid}/ely_account`, data);
}

export async function fbMirrorDiscord(uid: string, data: Record<string, unknown>): Promise<void> {
  await dbWritePublic(`users/${uid}/discord`, data);
}

/** SHA-256 hex of a string (WebCrypto). Used for the password hash mirror. */
export async function sha256Hex(input: string): Promise<string> {
  const buf = await crypto.subtle.digest("SHA-256", new TextEncoder().encode(input));
  return Array.from(new Uint8Array(buf))
    .map((b) => b.toString(16).padStart(2, "0"))
    .join("");
}

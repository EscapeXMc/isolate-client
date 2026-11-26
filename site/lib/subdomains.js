/* Subdomain registry + DNS provider for *.isolateclient.in
 * =========================================================
 *
 * Persistence — Firebase Realtime Database (same project the download
 * counter uses), public read / admin write:
 *
 *   subdomains/{name} -> { ip, port, email, createdAt, expiresAt }
 *
 * DNS — a provider abstraction. Cloudflare is implemented and switches on
 * automatically the moment CLOUDFLARE_API_TOKEN + CLOUDFLARE_ZONE_ID are
 * present on the project. Until then registration still succeeds and
 * reports `dns: "pending"`, so the product works end to end the instant
 * credentials land — no code change required.
 *
 * Records created per subdomain:
 *   A    {name}                  -> the player's server IP
 *   SRV  _minecraft._tcp.{name}  -> {name}.isolateclient.in : port
 *        (only when the port is not the Minecraft default 25565, since
 *         the client skips the SRV lookup on the default port)
 */

const FB_KEY = process.env.FB_API_KEY || "AIzaSyCDV6HqagLJtYoZZqO2Fvf9dA1kjTXXTRM";
const RTDB = "https://isolateclient-default-rtdb.firebaseio.com";
const ADMIN_EMAIL = process.env.ADMIN_EMAIL || "";
const ADMIN_PASSWORD = process.env.ADMIN_PASSWORD || "";
const TIMEOUT_MS = 6000;

export const DOMAIN = "isolateclient.in";
export const TTL_DAYS = 30;
export const DEFAULT_MC_PORT = 25565;

/** Names that must never be handed out as subdomains. */
export const RESERVED = new Set([
  "www", "mail", "email", "api", "app", "admin", "root", "ftp", "smtp", "imap",
  "ns1", "ns2", "ns3", "ns4", "cdn", "static", "assets", "blog", "shop", "store",
  "dashboard", "account", "login", "signup", "auth", "oauth", "settings",
  "subdomains", "subdomain", "plans", "download", "team", "faq", "contact",
  "privacy", "terms", "track", "uninstall", "preview", "launch", "feature",
  "isolateclient", "isolate", "minecraft", "server", "status", "dev", "staging",
  "localhost", "test", "demo", "beta", "old", "new", "staging", "mail",
]);

/* ----------------------------- validators ------------------------------ */

export function validIp(v) {
  v = String(v || "").trim();
  if (!v || v.length > 253 || /\s/.test(v)) return false;
  const m = v.match(/^(\d{1,3})\.(\d{1,3})\.(\d{1,3})\.(\d{1,3})$/);
  if (m) return m.slice(1).every((n) => +n <= 255);
  return /^[a-z0-9]([a-z0-9.-]*[a-z0-9])?$/i.test(v);
}

export function validPort(v) {
  return /^\d{1,5}$/.test(String(v || "").trim()) && +v >= 1 && +v <= 65535;
}

export function validEmail(v) {
  return /^[^\s@]+@[^\s@]+\.[^\s@]{2,}$/.test(String(v || "").trim());
}

export function validSub(v) {
  return /^[a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?$/.test(String(v || ""));
}

/* --------------------------- RTDB plumbing ----------------------------- */

function opts(extra) {
  return Object.assign({ signal: AbortSignal.timeout(TIMEOUT_MS) }, extra || {});
}

let tokenCache = { at: 0, token: "" };

/** Short-lived admin idToken (cached ~45 min) for RTDB writes. */
async function adminToken() {
  if (tokenCache.token && Date.now() - tokenCache.at < 45 * 60 * 1000) {
    return tokenCache.token;
  }
  if (!ADMIN_EMAIL || !ADMIN_PASSWORD) throw new Error("admin env missing");
  const r = await fetch(
    `https://identitytoolkit.googleapis.com/v1/accounts:signInWithPassword?key=${FB_KEY}`,
    opts({
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        email: ADMIN_EMAIL,
        password: ADMIN_PASSWORD,
        returnSecureToken: true,
      }),
    }),
  );
  const j = await r.json().catch(() => ({}));
  if (!r.ok || !j.idToken) throw new Error("admin auth failed");
  tokenCache = { at: Date.now(), token: j.idToken };
  return j.idToken;
}

/** Read one record. Public read first, authed as fallback. Never throws. */
export async function getRecord(name) {
  try {
    const r = await fetch(`${RTDB}/subdomains/${encodeURIComponent(name)}.json`, opts());
    if (r.ok) {
      const v = await r.json();
      if (v && typeof v === "object") return v;
      return null;
    }
  } catch {
    /* fall through */
  }
  try {
    const token = await adminToken();
    const r = await fetch(
      `${RTDB}/subdomains/${encodeURIComponent(name)}.json?auth=${encodeURIComponent(token)}`,
      opts(),
    );
    if (!r.ok) return null;
    const v = await r.json();
    return v && typeof v === "object" ? v : null;
  } catch {
    return null;
  }
}

/** Every record keyed by name — used for lazy expiry sweeps. Never throws. */
export async function listRecords() {
  try {
    const r = await fetch(`${RTDB}/subdomains.json`, opts());
    if (!r.ok) return {};
    const v = await r.json();
    return v && typeof v === "object" ? v : {};
  } catch {
    return {};
  }
}

/**
 * Mutate one RTDB path. The database rules currently accept public writes,
 * so the anonymous request is the fast path — and the only one that works
 * when the admin credentials in env are absent/invalid. If the anonymous
 * attempt is rejected (rules tightened later), retry with the admin token.
 */
async function rtdbMutation(path, method, body) {
  const headers = body ? { "Content-Type": "application/json" } : undefined;
  try {
    const r = await fetch(`${RTDB}/${path}.json`, opts({ method, headers, body }));
    if (r.ok || r.status === 404) return;
  } catch {
    /* network hiccup — still try the authenticated route below */
  }
  const token = await adminToken();
  const r = await fetch(
    `${RTDB}/${path}.json?auth=${encodeURIComponent(token)}`,
    opts({ method, headers, body }),
  );
  if (!r.ok && r.status !== 404) throw new Error(`RTDB ${method} ${r.status}`);
}

/** Write one record. Throws on failure. */
export async function putRecord(name, rec) {
  await rtdbMutation(
    `subdomains/${encodeURIComponent(name)}`,
    "PUT",
    JSON.stringify(rec),
  );
}

/** Remove one record. Throws on failure. */
export async function deleteRecord(name) {
  await rtdbMutation(`subdomains/${encodeURIComponent(name)}`, "DELETE");
}

export function isActive(rec) {
  return !!rec && typeof rec === "object" && +rec.expiresAt > Date.now();
}

/* ------------------------------- DNS ----------------------------------- */

/**
 * Cloudflare-backed DNS. Returns null until both env vars are present, which
 * is the signal to the caller that records should be reported as `pending`
 * rather than failed.
 */
function cloudflare() {
  const token = process.env.CLOUDFLARE_API_TOKEN || "";
  const zone = process.env.CLOUDFLARE_ZONE_ID || "";
  return token && zone ? { token, zone } : null;
}

export function dnsConfigured() {
  return !!cloudflare();
}

async function cf(zoneCfg, pathname, init) {
  const r = await fetch(
    `https://api.cloudflare.com/client/v4/zones/${zoneCfg.zone}${pathname}`,
    Object.assign(
      {
        signal: AbortSignal.timeout(TIMEOUT_MS),
        headers: {
          Authorization: `Bearer ${zoneCfg.token}`,
          "Content-Type": "application/json",
        },
      },
      init || {},
    ),
  );
  const j = await r.json().catch(() => ({}));
  if (!r.ok || j.success === false) {
    const msg =
      (Array.isArray(j.errors) && j.errors[0] && j.errors[0].message) ||
      `HTTP ${r.status}`;
    throw new Error(msg);
  }
  return j;
}

/** Find an existing record id by type + name (null when absent). */
async function findRecord(zoneCfg, type, name) {
  const j = await cf(
    zoneCfg,
    `/dns_records?type=${encodeURIComponent(type)}&name=${encodeURIComponent(name)}`,
  );
  const list = Array.isArray(j.result) ? j.result : [];
  return list.length ? list[0] : null;
}

/**
 * Create the DNS records for a subdomain.
 * Returns { configured, ok, records, error }.
 */
export async function ensureDns(name, ip, port) {
  const zoneCfg = cloudflare();
  if (!zoneCfg) return { configured: false, ok: true, records: [] };

  const fqdn = `${name}.${DOMAIN}`;
  const created = [];
  try {
    const aName = fqdn;
    if (!(await findRecord(zoneCfg, "A", aName))) {
      await cf(zoneCfg, "/dns_records", {
        method: "POST",
        body: JSON.stringify({
          type: "A",
          name: aName,
          content: ip,
          ttl: 300,
          proxied: false,
        }),
      });
    }
    created.push({ type: "A", name: aName });

    // Non-default port needs an SRV record, because the Minecraft client only
    // consults SRV when the player did not type an explicit port.
    if (+port !== DEFAULT_MC_PORT) {
      const srvName = `_minecraft._tcp.${fqdn}`;
      if (!(await findRecord(zoneCfg, "SRV", srvName))) {
        await cf(zoneCfg, "/dns_records", {
          method: "POST",
          body: JSON.stringify({
            type: "SRV",
            name: srvName,
            data: {
              priority: 0,
              weight: 5,
              port: +port,
              target: fqdn,
            },
            ttl: 300,
          }),
        });
      }
      created.push({ type: "SRV", name: srvName });
    }

    return { configured: true, ok: true, records: created };
  } catch (e) {
    return {
      configured: true,
      ok: false,
      records: created,
      error: e && e.message ? e.message : "DNS error",
    };
  }
}

/** Remove every record belonging to a subdomain (expiry / release). */
export async function removeDns(name) {
  const zoneCfg = cloudflare();
  if (!zoneCfg) return { configured: false, ok: true };

  const fqdn = `${name}.${DOMAIN}`;
  const targets = [
    { type: "A", name: fqdn },
    { type: "SRV", name: `_minecraft._tcp.${fqdn}` },
  ];
  let ok = true;
  let error = null;

  for (const t of targets) {
    try {
      const rec = await findRecord(zoneCfg, t.type, t.name);
      if (rec) {
        await cf(zoneCfg, `/dns_records/${rec.id}`, { method: "DELETE" });
      }
    } catch (e) {
      ok = false;
      if (!error) error = e && e.message ? e.message : "DNS error";
    }
  }
  return { configured: true, ok, error };
}

/**
 * Lazily release everything that has expired. Runs best-effort alongside a
 * registration so stale records do not accumulate; never blocks the caller.
 */
export async function sweepExpired() {
  const all = await listRecords();
  const now = Date.now();
  const names = Object.keys(all).filter((n) => !isActive(all[n]));
  const released = [];

  for (const name of names.slice(0, 20)) {
    try {
      await removeDns(name);
      await deleteRecord(name);
      released.push(name);
    } catch {
      /* keep the record; the next sweep retries */
    }
  }
  return released;
}

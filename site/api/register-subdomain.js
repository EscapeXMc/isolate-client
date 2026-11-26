/**
 * POST /api/register-subdomain
 *
 * Claims a subdomain of isolateclient.in for a Minecraft Java server.
 *
 *   body: { ip, port, email, sub }
 *   200:  { ok, hostname, ip, port, expiresAt, expiresInDays, dns }
 *   400:  { ok:false, errors:{ field: message } }
 *   409:  { ok:false, errors:{ sub: message } }   name already taken
 *
 * The registration is persisted to Firebase RTDB so it survives lambda
 * recycling, and the matching DNS records are created through the Cloudflare
 * provider when CLOUDFLARE_API_TOKEN + CLOUDFLARE_ZONE_ID are configured
 * (otherwise `dns` comes back as "pending" and the flow still completes).
 */
import {
  DOMAIN,
  TTL_DAYS,
  RESERVED,
  validIp,
  validPort,
  validEmail,
  validSub,
  getRecord,
  putRecord,
  isActive,
  ensureDns,
  sweepExpired,
  dnsConfigured,
} from "../lib/subdomains.js";

export default async function handler(req, res) {
  res.setHeader("Access-Control-Allow-Origin", "*");
  res.setHeader("Access-Control-Allow-Methods", "POST, OPTIONS");
  res.setHeader("Access-Control-Allow-Headers", "Content-Type");

  if (req.method === "OPTIONS") return res.status(204).end();
  if (req.method !== "POST") {
    return res.status(405).json({ ok: false, error: "Method not allowed" });
  }

  let body = req.body;
  if (typeof body === "string") {
    try {
      body = JSON.parse(body);
    } catch {
      body = {};
    }
  }
  body = body || {};

  const ip = String(body.ip || "").trim();
  const port = String(body.port || "25565").trim();
  const email = String(body.email || "").trim().toLowerCase();
  const sub = String(body.sub || "").trim().toLowerCase();

  const errors = {};
  if (!validIp(ip)) errors.ip = "Enter a valid server IP address.";
  if (!validPort(port)) errors.port = "Enter a port between 1 and 65535.";
  if (!validEmail(email)) errors.email = "Enter a valid email address.";
  if (!validSub(sub)) {
    errors.sub = "Use letters, numbers and dashes only (3+ characters).";
  } else if (RESERVED.has(sub)) {
    errors.sub = "That name is reserved. Pick another.";
  }

  if (Object.keys(errors).length) {
    return res.status(400).json({ ok: false, errors });
  }

  // Claim check. An expired record is free to take over.
  const existing = await getRecord(sub);
  if (isActive(existing)) {
    return res.status(409).json({
      ok: false,
      errors: { sub: "That subdomain is taken. Try another name." },
    });
  }

  const now = Date.now();
  const expiresAt = now + TTL_DAYS * 24 * 60 * 60 * 1000;
  const hostname = `${sub}.${DOMAIN}`;

  try {
    await putRecord(sub, { ip, port, email, createdAt: now, expiresAt });
  } catch (e) {
    console.error("register persist", e);
    return res.status(500).json({
      ok: false,
      errors: { sub: "Could not save your subdomain. Please try again." },
    });
  }

  // Best-effort: release other expired names while we are already writing.
  sweepExpired().catch(() => {});

  const dns = await ensureDns(sub, ip, port);

  return res.status(200).json({
    ok: true,
    hostname,
    ip,
    port,
    expiresAt: new Date(expiresAt).toISOString(),
    expiresInDays: TTL_DAYS,
    dns: dns.configured ? (dns.ok ? "active" : "error") : "pending",
    dnsError: dns.ok ? null : dns.error || null,
    dnsRecords: dns.records || [],
  });
}

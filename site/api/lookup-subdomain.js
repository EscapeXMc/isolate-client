/**
 * GET /api/lookup-subdomain?name=firemc
 *
 * Availability check + record read for a subdomain of isolateclient.in.
 *
 *   200 { ok:true, available:true,  hostname, name }
 *   200 { ok:false, available:false, hostname, ip, port, expiresAt }
 *   400 { ok:false, error }
 *
 * `available` is derived from expiry, so a stale name reads as free and can
 * be reclaimed immediately.
 */
import {
  DOMAIN,
  validSub,
  getRecord,
  isActive,
} from "../lib/subdomains.js";

export default async function handler(req, res) {
  res.setHeader("Access-Control-Allow-Origin", "*");
  res.setHeader("Cache-Control", "no-store, max-age=0");

  if (req.method !== "GET") {
    return res.status(405).json({ ok: false, error: "Method not allowed" });
  }

  const name = String(req.query.name || "").trim().toLowerCase();
  if (!validSub(name)) {
    return res.status(400).json({ ok: false, error: "Invalid subdomain name." });
  }

  const hostname = `${name}.${DOMAIN}`;
  const rec = await getRecord(name);

  if (!isActive(rec)) {
    return res.status(200).json({ ok: true, available: true, hostname, name });
  }

  return res.status(200).json({
    ok: true,
    available: false,
    hostname,
    name,
    ip: rec.ip,
    port: rec.port,
    expiresAt: new Date(+rec.expiresAt).toISOString(),
  });
}

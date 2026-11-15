/**
 * Geo lookup for automatic language selection.
 *
 * Vercel injects the visitor's country into the `x-vercel-ip-country` request
 * header, so this is a free, no-API-key, zero-network lookup at the edge. The
 * client resolves the country to a language (see src/lib/i18n.ts); we only
 * return the code here so the mapping lives in exactly one place.
 */
export default function handler(req, res) {
  // Never cache — the country is per-visitor.
  res.setHeader("Cache-Control", "no-store, max-age=0");

  const raw =
    req.headers["x-vercel-ip-country"] ||
    req.headers["x-country"] ||
    req.headers["cf-ipcountry"] ||
    "";

  const country = String(raw).trim().toUpperCase().slice(0, 2) || null;

  res.status(200).json({ country });
}

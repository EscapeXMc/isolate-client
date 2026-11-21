/**
 * GET/POST /__diag -> 204 No Content.
 *
 * The original Next.js static export ships a diagnostics beacon in the entry
 * chunk that POSTs to "/__diag". On a plain static host (and on Vercel without
 * a handler) that 404s and pollutes the browser console on every page.
 * Answer it with an empty 204 so the beacon succeeds and the console stays
 * clean. Nothing is stored — this endpoint intentionally does no work.
 */
export default function handler(req, res) {
  res.setHeader("Cache-Control", "public, s-maxage=86400, stale-while-revalidate=604800");
  res.status(204).end();
}

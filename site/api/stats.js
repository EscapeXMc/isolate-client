/**
 * GET  /api/stats  -> { downloads }  total for the homepage hero (cached ~30s)
 * POST /api/stats  -> { downloads }  +1, called by the iso-count.js click beacon
 *                      (navigator.sendBeacon survives the navigation away)
 *
 * The hero shows stats/downloads_baseline (one-time GitHub historical sum) +
 * stats/downloads (clicks counted since launch) — see lib/fb-count.js.
 */
import { countDownload, getDownloadCount } from "../lib/fb-count.js";

export default async function handler(req, res) {
  if (req.method === "GET") {
    res.setHeader("Cache-Control", "public, s-maxage=30, stale-while-revalidate=60");
    try {
      const downloads = await getDownloadCount();
      return res.status(200).json({ downloads });
    } catch (e) {
      console.error("stats read", e);
      // Never break the hero: an honest 0 beats a 500.
      return res.status(200).json({ downloads: 0 });
    }
  }

  if (req.method === "POST") {
    res.setHeader("Cache-Control", "no-store");
    try {
      const downloads = await countDownload();
      return res.status(200).json({ downloads });
    } catch (e) {
      console.error("stats write", e);
      return res.status(200).json({ ok: false });
    }
  }

  res.setHeader("Allow", "GET, POST");
  return res.status(405).json({ error: "Method not allowed" });
}

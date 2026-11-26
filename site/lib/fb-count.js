// Firebase-backed download counter shared by the API routes.
//
//   stats/downloads          clicks counted since the counter launched (+1 each)
//   stats/downloads_baseline one-time sum of every GitHub release asset
//                            download_count, so the hero starts from the real
//                            historical number instead of 0
//
// The hero shows baseline + clicks. Secrets (admin credentials) come from env
// only — never client code. Every fetch is bounded so a hung Google/GitHub
// call can never stall a download redirect.
const FB_KEY = process.env.FB_API_KEY || "AIzaSyCDV6HqagLJtYoZZqO2Fvf9dA1kjTXXTRM";
const RTDB = "https://isolateclient-default-rtdb.firebaseio.com";
const ADMIN_EMAIL = process.env.ADMIN_EMAIL || "";
const ADMIN_PASSWORD = process.env.ADMIN_PASSWORD || "";
const TIMEOUT_MS = 5000;

let tokenCache = { at: 0, token: "" };

function opts(extra) {
  return Object.assign({ signal: AbortSignal.timeout(TIMEOUT_MS) }, extra || {});
}

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

/** Read a number from RTDB (public read first; authed as fallback). */
async function rtdbNumber(path) {
  try {
    const r = await fetch(`${RTDB}/${path}.json`, opts());
    if (r.ok) {
      const v = await r.json();
      if (typeof v === "number" && isFinite(v)) return v;
      return null; // present but not a number (or null) — no auth retry needed
    }
  } catch {
    /* fall through to authed read */
  }
  try {
    const token = await adminToken();
    const r = await fetch(
      `${RTDB}/${path}.json?auth=${encodeURIComponent(token)}`,
      opts(),
    );
    if (!r.ok) return null;
    const v = await r.json();
    return typeof v === "number" && isFinite(v) ? v : null;
  } catch {
    return null;
  }
}

/** Write a number to RTDB — anonymous first (rules allow it), admin token
 *  as fallback so a future rules tightening does not silently break the
 *  download counter. */
async function rtdbSet(path, value) {
  const headers = { "Content-Type": "application/json" };
  const body = JSON.stringify(value);
  try {
    const r = await fetch(`${RTDB}/${path}.json`, opts({ method: "PUT", headers, body }));
    if (r.ok) return;
  } catch {
    /* fall through to the authenticated attempt */
  }
  const token = await adminToken();
  const r = await fetch(
    `${RTDB}/${path}.json?auth=${encodeURIComponent(token)}`,
    opts({ method: "PUT", headers, body }),
  );
  if (!r.ok) throw new Error(`RTDB write ${r.status}`);
}

/** Sum every release asset's download_count — the real historical total. */
async function githubBaseline() {
  const r = await fetch(
    "https://api.github.com/repos/EscapeXMc/isolate-client/releases?per_page=100",
    opts({ headers: { Accept: "application/vnd.github+json" } }),
  );
  if (!r.ok) throw new Error(`GitHub API ${r.status}`);
  const rel = await r.json();
  if (!Array.isArray(rel)) throw new Error("GitHub bad payload");
  let sum = 0;
  for (const release of rel) {
    for (const asset of release.assets || []) {
      sum += typeof asset.download_count === "number" ? asset.download_count : 0;
    }
  }
  return sum;
}

/** +1 a download click. Returns the new click count, or null on failure. */
export async function countDownload() {
  try {
    const cur = await rtdbNumber("stats/downloads");
    const next = (cur || 0) + 1;
    await rtdbSet("stats/downloads", next);
    return next;
  } catch (e) {
    console.error("countDownload", e);
    return null;
  }
}

/**
 * Total the hero displays: historical GitHub baseline (seeded once) + clicks
 * counted since the counter launched. Never throws — reads degrade to 0.
 */
export async function getDownloadCount() {
  let clicks = await rtdbNumber("stats/downloads");
  if (clicks == null) clicks = 0;

  let baseline = await rtdbNumber("stats/downloads_baseline");
  if (baseline == null) {
    try {
      baseline = await githubBaseline();
      try {
        await rtdbSet("stats/downloads_baseline", baseline);
      } catch (e) {
        console.error("baseline persist", e); // still display it this request
      }
    } catch (e) {
      console.error("baseline seed", e);
      baseline = 0; // next request retries the seed
    }
  }
  return baseline + clicks;
}

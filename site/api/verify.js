// POST /api/verify — verifies the Razorpay signature, then writes the rank to Firebase.
// Secrets (RAZORPAY_KEY_SECRET, admin credentials) come from env only — never client code.
const crypto = require("crypto");

const RAZORPAY_KEY_ID = process.env.RAZORPAY_KEY_ID || "";
const RAZORPAY_KEY_SECRET = process.env.RAZORPAY_KEY_SECRET || "";
const ADMIN_EMAIL = process.env.ADMIN_EMAIL || "";
const ADMIN_PASSWORD = process.env.ADMIN_PASSWORD || "";
const FB_KEY = process.env.FB_API_KEY || "AIzaSyCDV6HqagLJtYoZZqO2Fvf9dA1kjTXXTRM"; // public client id
const RTDB = "https://isolateclient-default-rtdb.firebaseio.com";

const ALLOWED = new Set(["solute-plus", "solute-pro", "solute-elite", "solute-ultra"]);

async function adminToken() {
  const r = await fetch(
    `https://identitytoolkit.googleapis.com/v1/accounts:signInWithPassword?key=${FB_KEY}`,
    {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ email: ADMIN_EMAIL, password: ADMIN_PASSWORD, returnSecureToken: true }),
    }
  );
  const j = await r.json().catch(() => ({}));
  if (!r.ok || !j.idToken) throw new Error("admin auth failed");
  return j.idToken;
}

module.exports = async (req, res) => {
  if (req.method !== "POST") {
    res.status(405).json({ error: "Method not allowed" });
    return;
  }
  if (!RAZORPAY_KEY_SECRET) {
    res.status(500).json({ error: "Server not configured (RAZORPAY_KEY_SECRET)." });
    return;
  }

  let body = req.body;
  if (typeof body === "string") {
    try { body = JSON.parse(body); } catch { body = {}; }
  }
  body = body || {};

  const plan = String(body.plan || "");
  const email = String(body.email || "").trim().toLowerCase();
  const paymentId = String(body.razorpay_payment_id || "");
  const orderId = String(body.razorpay_order_id || "");
  const signature = String(body.razorpay_signature || "");

  if (!ALLOWED.has(plan) || !paymentId || !orderId || !signature) {
    res.status(400).json({ error: "Missing payment fields." });
    return;
  }
  if (!/^[^@\s]+@[^@\s]+\.[^@\s]+$/.test(email)) {
    res.status(400).json({ error: "Invalid email." });
    return;
  }

  // Signature check: HMAC-SHA256(order_id + "|" + payment_id) with the key secret.
  const expected = crypto
    .createHmac("sha256", RAZORPAY_KEY_SECRET)
    .update(`${orderId}|${paymentId}`)
    .digest("hex");
  const a = Buffer.from(expected, "hex");
  const b = Buffer.from(signature, "hex");
  if (a.length !== b.length || !crypto.timingSafeEqual(a, b)) {
    res.status(400).json({ error: "Signature verification failed." });
    return;
  }

  // Persist the rank so the client can gate voice chat on it.
  // Anonymous write first (RTDB rules accept it); admin token only as a
  // fallback, so a broken/absent credential can never lose a paid rank.
  try {
    const slug = email.replace(/[^a-z0-9]+/g, "_").slice(0, 60);
    const payload = {
      plan,
      active: true,
      email,
      payment_id: paymentId,
      order_id: orderId,
      since: new Date().toISOString(),
    };
    const url = `${RTDB}/ranks/${slug}.json`;
    const init = {
      method: "PUT",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(payload),
    };
    let r = await fetch(url, init);
    if (!r.ok) {
      const token = await adminToken();
      r = await fetch(`${url}?auth=${encodeURIComponent(token)}`, init);
    }
    if (!r.ok) console.error("rank write failed", r.status, await r.text());
  } catch (e) {
    // Payment is still valid — log and confirm; rank sync can be retried.
    console.error("rank write error", e);
  }

  res.status(200).json({ ok: true, plan });
};

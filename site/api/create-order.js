// POST /api/create-order — creates a Razorpay order. Secret stays server-side (env).
const RAZORPAY_KEY_ID = process.env.RAZORPAY_KEY_ID || "";
const RAZORPAY_KEY_SECRET = process.env.RAZORPAY_KEY_SECRET || "";

const ALLOWED = new Set(["solute-plus", "solute-pro", "solute-elite", "solute-ultra"]);

module.exports = async (req, res) => {
  if (req.method !== "POST") {
    res.status(405).json({ error: "Method not allowed" });
    return;
  }
  if (!RAZORPAY_KEY_ID || !RAZORPAY_KEY_SECRET) {
    res.status(500).json({ error: "Server not configured (RAZORPAY_KEY_ID / RAZORPAY_KEY_SECRET)." });
    return;
  }

  let body = req.body;
  if (typeof body === "string") {
    try { body = JSON.parse(body); } catch { body = {}; }
  }
  body = body || {};

  const plan = String(body.plan || "");
  const amount = Number(body.amount);
  const email = String(body.email || "").trim().toLowerCase();

  if (!ALLOWED.has(plan)) {
    res.status(400).json({ error: "Unknown plan." });
    return;
  }
  if (!Number.isInteger(amount) || amount < 100 || amount > 1000000) {
    res.status(400).json({ error: "Invalid amount." });
    return;
  }
  if (!/^[^@\s]+@[^@\s]+\.[^@\s]+$/.test(email)) {
    res.status(400).json({ error: "Invalid email." });
    return;
  }

  try {
    const auth = Buffer.from(`${RAZORPAY_KEY_ID}:${RAZORPAY_KEY_SECRET}`).toString("base64");
    const r = await fetch("https://api.razorpay.com/v1/orders", {
      method: "POST",
      headers: {
        Authorization: `Basic ${auth}`,
        "Content-Type": "application/json",
      },
      body: JSON.stringify({
        amount,               // paise
        currency: "INR",
        receipt: `iso_${plan}_${Date.now()}`,
        notes: { plan, email },
      }),
    });
    const j = await r.json().catch(() => ({}));
    if (!r.ok || !j.id) {
      console.error("razorpay order error", r.status, JSON.stringify(j));
      res.status(502).json({ error: "Could not create order. Try again." });
      return;
    }
    // Publishable key only in the response — never the secret.
    res.status(200).json({ orderId: j.id, keyId: RAZORPAY_KEY_ID, amount: j.amount, currency: j.currency });
  } catch (e) {
    console.error("create-order failed", e);
    res.status(502).json({ error: "Network error creating order." });
  }
};

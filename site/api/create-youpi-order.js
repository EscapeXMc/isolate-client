// POST /api/create-youpi-order — creates a YOUPI PG payment session.
// The merchant API key stays server-side (env); the browser only ever sees
// the checkout URL for the session this function just created.
//
// Required env:
//   YOUPI_API_URL     e.g. http://localhost:6100  (payment-api base)
//   YOUPI_MERCHANT_ID e.g. 00000000-0000-0000-0000-000000000001
//   YOUPI_MERCHANT_KEY  mk_live_... (or the merchant webhook secret on local installs)
// Optional env:
//   YOUPI_RETURN_URL  where the portal sends the buyer after checkout
//
// The store sells exactly one product, so the amount is fixed server-side:
// a client-supplied price would let a buyer pay ₹1 for the account.

const YOUPI_API_URL = (process.env.YOUPI_API_URL || "").replace(/\/+$/, "");
const YOUPI_MERCHANT_ID = process.env.YOUPI_MERCHANT_ID || "";
const YOUPI_MERCHANT_KEY = process.env.YOUPI_MERCHANT_KEY || "";
const YOUPI_RETURN_URL = process.env.YOUPI_RETURN_URL || "https://isolateclient.in/store";

const PRODUCT = Object.freeze({
  id: "minecraft-java-account",
  amount: "1250.00",
  currency: "INR",
  name: "Isolate Client Store",
  description: "Minecraft Java Edition Account",
});

module.exports = async (req, res) => {
  if (req.method !== "POST") {
    res.status(405).json({ error: "Method not allowed." });
    return;
  }
  if (!YOUPI_API_URL || !YOUPI_MERCHANT_ID || !YOUPI_MERCHANT_KEY) {
    res.status(500).json({ error: "Server not configured (YOUPI_API_URL / YOUPI_MERCHANT_ID / YOUPI_MERCHANT_KEY)." });
    return;
  }

  let body = req.body;
  if (typeof body === "string") {
    try { body = JSON.parse(body); } catch { body = {}; }
  }
  body = body || {};

  // Single-product store: only the known product may be ordered, whatever the client sent.
  const product = String(body.product || PRODUCT.id);
  if (product !== PRODUCT.id) {
    res.status(400).json({ error: "Unknown product." });
    return;
  }

  const orderId = `ISO-${Date.now()}-${Math.random().toString(36).slice(2, 8).toUpperCase()}`;

  try {
    const r = await fetch(`${YOUPI_API_URL}/api/v1/payments`, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        "X-Merchant-Id": YOUPI_MERCHANT_ID,
        "X-Merchant-Key": YOUPI_MERCHANT_KEY,
        // Same orderId retried within the session window returns the live session
        // instead of minting a duplicate one.
        "Idempotency-Key": orderId,
      },
      body: JSON.stringify({
        orderId,
        amount: PRODUCT.amount,
        currency: PRODUCT.currency,
        returnUrl: YOUPI_RETURN_URL,
        metadata: { product: PRODUCT.id, source: "store" },
      }),
    });

    const j = await r.json().catch(() => ({}));
    if (!r.ok || !j.checkoutUrl) {
      console.error("youpi create error", r.status, JSON.stringify(j));
      const message =
        j.code === "OBSERVERS_UNHEALTHY"
          ? "Payments are temporarily unavailable. Please try again shortly, or open a Discord ticket."
          : "Could not start checkout. Try again, or open a Discord ticket.";
      res.status(502).json({ error: message, code: j.code || "GATEWAY_ERROR" });
      return;
    }

    res.status(200).json({
      orderId: j.orderId || orderId,
      paymentId: j.paymentId,
      checkoutUrl: j.checkoutUrl,
      amount: j.amount || PRODUCT.amount,
      currency: j.currency || PRODUCT.currency,
    });
  } catch (e) {
    console.error("create-youpi-order failed", e);
    res.status(502).json({ error: "Network error starting checkout." });
  }
};

/**
 * Live Minecraft Java server status via the Server List Ping protocol.
 *
 * Browsers can't open raw TCP, so the site asks this endpoint instead. One
 * request can carry many addresses (the cards batch them), results are cached
 * briefly per address, and private/loopback targets are refused so the
 * function can't be used to probe an internal network.
 *
 * GET  /api/mc-status?addr=host:port&addr=host2:port2
 * POST /api/mc-status   { "addresses": ["host:port", ...] }
 */
import net from "node:net";
import dns from "node:dns/promises";

const CONNECT_TIMEOUT = 2500;
const RESPONSE_TIMEOUT = 3000;
const CACHE_TTL = 15_000;
const MAX_ADDRESSES = 24;

const cache = new Map();

/* ------------------------------ varint I/O ------------------------------ */

function encodeVarInt(value) {
  let v = value >>> 0; // -1 becomes 0xFFFFFFFF, which is what status wants
  const bytes = [];
  do {
    let byte = v & 0x7f;
    v >>>= 7;
    if (v !== 0) byte |= 0x80;
    bytes.push(byte);
  } while (v !== 0);
  return Buffer.from(bytes);
}

/** Reads a varint at `offset`; returns null when the buffer is short. */
function readVarInt(buf, offset) {
  let value = 0;
  let shift = 0;
  let i = offset;
  while (i < buf.length) {
    const byte = buf[i++];
    value |= (byte & 0x7f) << shift;
    if ((byte & 0x80) === 0) return { value: value >>> 0, next: i };
    shift += 7;
    if (shift > 35) break;
  }
  return null;
}

function frame(packetId, payload = Buffer.alloc(0)) {
  const body = Buffer.concat([encodeVarInt(packetId), payload]);
  return Buffer.concat([encodeVarInt(body.length), body]);
}

function stringField(value) {
  const bytes = Buffer.from(value, "utf8");
  return Buffer.concat([encodeVarInt(bytes.length), bytes]);
}

/* ------------------------------ address parsing ------------------------------ */

function parseAddress(raw) {
  const input = String(raw ?? "")
    .trim()
    .replace(/\s+/g, "");
  if (!input || input.length > 260) return null;

  let host = input;
  let port = 25565;

  // [::1]:25565 — bracketed IPv6
  const bracketed = input.match(/^\[([0-9a-f:]+)\](?::(\d{1,5}))?$/i);
  if (bracketed) {
    host = bracketed[1];
    port = bracketed[2] ? Number(bracketed[2]) : 25565;
  } else {
    // host:port — but a bare IPv6 has many colons, so only split on the last
    const lastColon = input.lastIndexOf(":");
    if (lastColon > -1 && input.indexOf(":") === lastColon) {
      host = input.slice(0, lastColon);
      port = Number(input.slice(lastColon + 1));
    }
  }

  if (!/^[a-z0-9._:-]+$/i.test(host)) return null;
  if (!Number.isInteger(port) || port < 1 || port > 65535) return null;
  return { host, port, key: `${host.toLowerCase()}:${port}` };
}

const PRIVATE_V4 =
  /^(10\.|127\.|169\.254\.|192\.168\.|0\.|172\.(1[6-9]|2\d|3[01])\.|100\.(6[4-9]|[7-9]\d|1[01]\d|12[0-7])\.)/;

async function isPublicHost(host) {
  const lower = host.toLowerCase();
  if (
    lower === "localhost" ||
    lower.endsWith(".localhost") ||
    lower.endsWith(".local") ||
    lower.endsWith(".internal") ||
    lower === "::1" ||
    lower === "::" ||
    lower.startsWith("fd") ||
    lower.startsWith("fe80")
  ) {
    return "blocked";
  }
  if (PRIVATE_V4.test(lower)) return "blocked";

  if (/^\d{1,3}(\.\d{1,3}){3}$/.test(host)) return true;

  // Resolve first: a public-looking name can still point at 10.x.x.x.
  try {
    const records = await dns.lookup(host, { all: true });
    if (!records.length) return "unresolved";
    return records.every(
      (r) => !PRIVATE_V4.test(r.address) && r.address !== "::1",
    )
      ? true
      : "blocked";
  } catch {
    return "unresolved"; // no such host — say so instead of "blocked"
  }
}

/* ------------------------------ status parsing ------------------------------ */

/** Minecraft MOTDs are full of § colour codes — useless on the web. */
function cleanMotd(text) {
  return text
    .replace(/\u00a7[0-9a-fk-orA-FK-OR]/g, "")
    .replace(/\s+/g, " ")
    .trim()
    .slice(0, 300);
}

function flattenMotd(desc) {
  if (!desc) return "";
  if (typeof desc === "string") return desc;
  if (Array.isArray(desc)) return desc.map(flattenMotd).join("");
  if (typeof desc === "object") {
    let out = "";
    if (typeof desc.text === "string") out += desc.text;
    else if (typeof desc.translate === "string") out += desc.translate;
    if (Array.isArray(desc.extra)) out += desc.extra.map(flattenMotd).join("");
    return out;
  }
  return "";
}

function summariseMods(modinfo) {
  if (!modinfo || typeof modinfo !== "object") return null;
  const list = modinfo.modList;
  if (Array.isArray(list) && list.length) {
    return { kind: modinfo.type || "mods", count: list.length };
  }
  return null;
}

/* --------------------------------- the ping --------------------------------- */

/** Opens a connection, does handshake + status (+ latency ping), then closes. */
export function pingMinecraftServer(host, port) {
  return new Promise((resolve) => {
    const started = Date.now();
    let buffer = Buffer.alloc(0);
    let settled = false;
    let pending = null;
    let pingSentAt = 0;
    let pingTimer = null;

    const finish = (result) => {
      if (settled) return;
      settled = true;
      clearTimeout(connectTimer);
      clearTimeout(responseTimer);
      if (pingTimer) clearTimeout(pingTimer);
      try {
        socket.destroy();
      } catch {
        /* already gone */
      }
      resolve(result);
    };

    const fail = (error) => finish({ ok: false, error });

    const socket = net.connect({ host, port });

    const connectTimer = setTimeout(() => fail("timeout"), CONNECT_TIMEOUT);
    const responseTimer = setTimeout(() => fail("timeout"), RESPONSE_TIMEOUT);

    socket.on("connect", () => {
      const portBytes = Buffer.alloc(2);
      portBytes.writeUInt16BE(port, 0);
      socket.write(
        frame(
          0x00,
          Buffer.concat([
            encodeVarInt(-1), // protocol version (-1 = "just asking for status")
            stringField(host),
            portBytes,
            encodeVarInt(1), // next state: status
          ]),
        ),
      );
      socket.write(frame(0x00)); // status request
    });

    socket.on("data", (chunk) => {
      buffer = Buffer.concat([buffer, chunk]);

      // Consume whole packets; keep whatever is still incomplete in `buffer`.
      for (;;) {
        const len = readVarInt(buffer, 0);
        if (!len || buffer.length < len.next + len.value) return;
        const packet = buffer.subarray(0, len.next + len.value);
        buffer = buffer.subarray(len.next + len.value);

        const id = readVarInt(packet, len.next);
        if (!id) continue;

        // Ping echo — this is the number launchers show as "ping".
        if (id.value === 0x01 && pending) {
          finish({ ...pending, latencyMs: Date.now() - pingSentAt });
          return;
        }

        // Status response: varint-length-prefixed JSON.
        if (id.value === 0x00 && !pending) {
          const jsonLen = readVarInt(packet, id.next);
          if (!jsonLen) continue;
          const start = jsonLen.next;
          const end = start + jsonLen.value;
          if (packet.length < end) continue;

          let parsed;
          try {
            parsed = JSON.parse(packet.subarray(start, end).toString("utf8"));
          } catch {
            fail("bad-response");
            return;
          }

          pending = {
            ok: true,
            online: parsed?.players?.online ?? null,
            max: parsed?.players?.max ?? null,
            version: parsed?.version?.name
              ? cleanMotd(parsed.version.name)
              : null,
            protocol: parsed?.version?.protocol ?? null,
            motd: cleanMotd(flattenMotd(parsed?.description)),
            favicon:
              typeof parsed?.favicon === "string" &&
              parsed.favicon.startsWith("data:image/png;base64,") &&
              parsed.favicon.length < 400_000
                ? parsed.favicon
                : null,
            mods: summariseMods(parsed?.modinfo),
            secureChat:
              typeof parsed?.enforcesSecureChat === "boolean"
                ? parsed.enforcesSecureChat
                : null,
            // Proxies and hosts often hide the player list until asked twice.
            hidesPlayerList: parsed?.players?.sample === undefined,
          };

          // Measure a real ping on the already-open connection. If the server
          // never echoes, the fallback timer below uses the full round trip.
          pingSentAt = Date.now();
          try {
            // 1 byte length + 1 byte id + 8 byte timestamp = 10 bytes.
            const ping = Buffer.alloc(10);
            ping[0] = 9; // packet length (id + payload)
            ping[1] = 1; // ping packet id
            ping.writeBigInt64BE(BigInt(pingSentAt), 2);
            socket.write(ping);
          } catch {
            finish({ ...pending, latencyMs: Date.now() - started });
            return;
          }
          pingTimer = setTimeout(() => {
            finish({ ...pending, latencyMs: Date.now() - started });
          }, 1200);
        }
      }
    });

    socket.on("error", () => fail("unreachable"));
    socket.on("timeout", () => fail("timeout"));
    socket.on("close", () => fail("closed"));
  });
}

/** Cached + guarded single-address ping. `withFavicon` opts into the server
 *  icon — it is a large data URL, so it is only sent when a card actually
 *  needs it as its logo. */
export async function getServerStatus(address, withFavicon = false) {
  const parsed = parseAddress(address);
  if (!parsed) return { address, ok: false, error: "bad-address" };

  const now = Date.now();
  const hit = cache.get(parsed.key);
  if (hit && now - hit.at < CACHE_TTL) {
    // Re-use the cached result, but only ship the icon if this caller wants it.
    if (withFavicon || !hit.value.favicon) return hit.value;
    const { favicon, ...rest } = hit.value;
    return rest;
  }

  const reach = await isPublicHost(parsed.host);
  if (reach !== true) {
    const value = { address: parsed.key, ok: false, error: reach };
    cache.set(parsed.key, { at: now, value });
    return value;
  }

  let value;
  try {
    value = await pingMinecraftServer(parsed.host, parsed.port);
  } catch {
    value = { ok: false, error: "unreachable" };
  }
  const shaped = { address: parsed.key, ...value, checkedAt: now };
  cache.set(parsed.key, { at: now, value: shaped });
  if (!withFavicon && shaped.favicon) {
    const { favicon, ...rest } = shaped;
    return rest;
  }
  return shaped;
}

/* --------------------------------- handler --------------------------------- */

export default async function handler(req, res) {
  const headers = { "Content-Type": "application/json" };
  if (req.method === "OPTIONS") {
    res.setHeader("Allow", "GET, POST");
    return res.status(204).end();
  }

  let addresses = [];
  let withFavicon = false;
  const search = new URL(req.url ?? "/", "http://localhost").searchParams;
  if (req.method === "GET") {
    addresses = search.getAll("addr");
    withFavicon = search.get("favicon") === "1";
  } else if (req.method === "POST") {
    const body = typeof req.body === "string" ? req.body : req.body;
    let parsed = null;
    try {
      parsed = typeof body === "string" ? JSON.parse(body || "{}") : body;
    } catch {
      parsed = null;
    }
    addresses = Array.isArray(parsed?.addresses)
      ? parsed.addresses.slice(0, MAX_ADDRESSES).map(String)
      : [];
    withFavicon = parsed?.favicon === true;
  } else {
    res.setHeader("Allow", "GET, POST");
    return res.status(405).json({ error: "Method not allowed" }, headers);
  }

  addresses = [...new Set(addresses.map((a) => String(a).trim()))].slice(
    0,
    MAX_ADDRESSES,
  );
  if (!addresses.length) {
    return res.status(400).json({ error: "No addresses given" }, headers);
  }

  const results = await Promise.all(
    addresses.map((a) => getServerStatus(a, withFavicon)),
  );
  const status = {};
  for (const r of results) status[r.address] = r;

  // Short shared cache: absorbs the 30s client poll without going stale.
  res.setHeader(
    "Cache-Control",
    "public, s-maxage=15, stale-while-revalidate=30",
  );
  return res.status(200).json({ status, checkedAt: Date.now() }, headers);
}

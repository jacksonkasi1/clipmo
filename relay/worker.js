// Clipmo relay: a dumb mailbox for end-to-end-encrypted clips. It only ever sees
// ciphertext. Deploy to your own Cloudflare account; Clipmo's developer holds nothing.
//
//   POST /v1/:channel        { id, device, blob }      -> 204
//   GET  /v1/:channel?after= -> { items: [{ id, device, blob }] }  (ascending, max 200)
// Auth: `Authorization: Bearer <RELAY_TOKEN>` (a Worker secret you choose).
const RETENTION_MS = 7 * 24 * 3600 * 1000;
const json = (body, status = 200) =>
  new Response(JSON.stringify(body), { status, headers: { "content-type": "application/json" } });

export default {
  async fetch(request, env) {
    const url = new URL(request.url);
    const match = url.pathname.match(/^\/v1\/([0-9a-f]{32})$/);
    if (!match) return json({ error: "not found" }, 404);
    const auth = request.headers.get("authorization") || "";
    if (!env.RELAY_TOKEN || auth !== `Bearer ${env.RELAY_TOKEN}`) return json({ error: "unauthorized" }, 401);
    const channel = match[1];

    if (request.method === "POST") {
      let item;
      try { item = await request.json(); } catch { return json({ error: "bad json" }, 400); }
      const { id, device, blob } = item || {};
      if (!/^\d{16}-[0-9a-f]{8}$/.test(id || "") || typeof device !== "string" || device.length > 100 ||
          typeof blob !== "string" || blob.length > 1_000_000) return json({ error: "invalid item" }, 400);
      await env.DB.prepare("INSERT OR IGNORE INTO mail (channel, id, device, blob) VALUES (?1, ?2, ?3, ?4)")
        .bind(channel, id, device, blob).run();
      const cutoff = String(Date.now() - RETENTION_MS).padStart(16, "0");
      await env.DB.prepare("DELETE FROM mail WHERE channel = ?1 AND id < ?2").bind(channel, cutoff).run();
      return new Response(null, { status: 204 });
    }
    if (request.method === "GET") {
      const after = url.searchParams.get("after") || "";
      const { results } = await env.DB.prepare(
        "SELECT id, device, blob FROM mail WHERE channel = ?1 AND id > ?2 ORDER BY id LIMIT 200",
      ).bind(channel, after).all();
      return json({ items: results });
    }
    return json({ error: "method not allowed" }, 405);
  },
};

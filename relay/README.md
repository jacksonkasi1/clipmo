# Clipmo relay (optional)

Use this when your devices cannot reach each other directly (Wi-Fi client isolation,
firewalls, different networks). It is **your** mailbox: Clipmo's developer never sees
or stores your data, and everything is AES-256-GCM encrypted with a key derived from a
passphrase that never leaves your devices. Only text clips, edits, favorites and
deletes use the relay; images and files stay on the direct LAN path.

Pick one backend, then enter it in Clipmo Settings → Relay (desktop) or Devices → Relay
(Android) on **every** device, with the **same passphrase**.

## Option A: Cloudflare Worker + D1 (this folder)
```bash
cd relay
npx wrangler d1 create clipmo-relay          # paste the id into wrangler.toml
npx wrangler d1 execute clipmo-relay --remote --file schema.sql
npx wrangler secret put RELAY_TOKEN          # any long random string
npx wrangler deploy
```
Backend = "Custom endpoint", URL = your `https://clipmo-relay.<you>.workers.dev`,
Secret = the RELAY_TOKEN. (The `cf` CLI works too.)

## Option B: your own Firebase Realtime Database
Create a project, enable Realtime Database, and set rules so only authenticated access
works. Backend = "Firebase", URL = `https://<project>-default-rtdb.firebaseio.com`,
Secret = a database secret / auth token. Entries live under `/clipmo/<channel>`.
Optionally add a scheduled cleanup (or a Cloud Function) to expire old entries.

## Option C: any SQL database (Turso, Postgres, ...)
Run `schema.sql` and expose the same two routes (`POST /v1/:channel`, `GET
/v1/:channel?after=`) from any server or serverless function, then use "Custom endpoint".
`worker.js` is about 30 lines to port.

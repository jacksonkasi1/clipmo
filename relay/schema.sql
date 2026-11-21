-- Works unchanged on Cloudflare D1, Turso/libSQL, SQLite and (with TEXT keys) Postgres.
CREATE TABLE IF NOT EXISTS mail (
  channel TEXT NOT NULL,
  id      TEXT NOT NULL,
  device  TEXT NOT NULL,
  blob    TEXT NOT NULL,
  PRIMARY KEY (channel, id)
);

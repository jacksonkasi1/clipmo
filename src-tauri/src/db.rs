//! SQLite persistence layer.
//!
//! Design notes:
//! * A single connection guarded by a `parking_lot::Mutex` is used rather than a
//!   pool. Writes happen at human speed (once per copy) and reads are driven by
//!   keystrokes, so contention is nil and we avoid pool overhead entirely.
//! * WAL + `synchronous=NORMAL` keeps the write-per-copy path off the fsync path.
//! * Full-text search uses an FTS5 *external content* table, so the searchable
//!   text is not duplicated on disk. Sync triggers only fire when the indexed
//!   columns actually change, so toggling a star never touches the index.
//! * Image bytes live on disk, never in the database, so listing history rows
//!   stays cheap regardless of how many screenshots were captured.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use parking_lot::Mutex;
use rusqlite::{params, Connection, OptionalExtension, Row};

use crate::error::{Error, Result};
use crate::models::{
    now_ms, BulkFilterAction, ClipItem, CollectionSummary, Counts, DeviceIdentity, FilterScope,
    FilterScopeKind, ImageMeta, ItemKind, ListQuery, NewItem, PlatformKind, Settings, SourceApp,
    StoredFile, SyncStatus,
};

/// Column list shared by every read query so that `row_to_item` stays valid.
const COLUMNS: &str = "id, kind, preview, content, html, rtf, image_path, thumb_path, \
     image_w, image_h, file_paths, size_bytes, app_name, app_exe, app_icon, \
     favorite, copy_count, first_copied_at, last_copied_at, file_assets, \
     device_id, device_name, device_platform, device_color, sync_status, tags";

/// Plain text plus the optional HTML and RTF representations stored for an item.
pub type RichFlavors = (String, Option<String>, Option<String>);

/// Longest preview label we store. Anything longer is truncated for display but
/// the full payload is preserved in `content`.
const PREVIEW_LIMIT: usize = 320;

/// Outcome of a capture, so callers know whether to notify the UI of a brand new
/// row or of a reordered existing one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Upsert {
    Inserted(i64),
    Bumped(i64),
}

impl Upsert {
    pub fn id(self) -> i64 {
        match self {
            Self::Inserted(id) | Self::Bumped(id) => id,
        }
    }

    pub fn is_new(self) -> bool {
        matches!(self, Self::Inserted(_))
    }
}

pub struct Db {
    conn: Mutex<Connection>,
}

impl Db {
    /// Opens (creating if needed) the history database and applies migrations.
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;
        Self::configure(&conn)?;
        Self::migrate(&conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// In-memory database, used by the unit tests.
    #[cfg(test)]
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        Self::configure(&conn)?;
        Self::migrate(&conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    fn configure(conn: &Connection) -> Result<()> {
        // `journal_mode` returns a row, so it must use `query_row` not `execute`.
        conn.query_row("PRAGMA journal_mode = WAL", [], |_| Ok(()))?;
        conn.execute_batch(
            "PRAGMA synchronous = NORMAL;
             PRAGMA foreign_keys = ON;
             PRAGMA temp_store = MEMORY;
             PRAGMA mmap_size = 67108864;",
        )?;
        Ok(())
    }

    fn migrate(conn: &Connection) -> Result<()> {
        conn.execute_batch(
            r#"
CREATE TABLE IF NOT EXISTS items (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    kind            TEXT    NOT NULL,
    preview         TEXT    NOT NULL,
    content         TEXT    NOT NULL DEFAULT '',
    html            TEXT,
    rtf             TEXT,
    image_path      TEXT,
    thumb_path      TEXT,
    image_w         INTEGER,
    image_h         INTEGER,
    file_paths      TEXT,
    size_bytes      INTEGER NOT NULL DEFAULT 0,
    hash            TEXT    NOT NULL,
    app_name        TEXT,
    app_exe         TEXT,
    app_icon        TEXT,
    favorite        INTEGER NOT NULL DEFAULT 0,
    copy_count      INTEGER NOT NULL DEFAULT 1,
    first_copied_at INTEGER NOT NULL,
    last_copied_at  INTEGER NOT NULL
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_items_hash ON items(hash);
CREATE INDEX IF NOT EXISTS idx_items_recent    ON items(last_copied_at DESC);
CREATE INDEX IF NOT EXISTS idx_items_kind      ON items(kind, last_copied_at DESC);
CREATE INDEX IF NOT EXISTS idx_items_favorite  ON items(favorite, last_copied_at DESC);

CREATE VIRTUAL TABLE IF NOT EXISTS items_fts USING fts5(
    preview,
    content,
    content='items',
    content_rowid='id',
    tokenize="unicode61 remove_diacritics 2"
);

CREATE TRIGGER IF NOT EXISTS items_fts_insert AFTER INSERT ON items BEGIN
    INSERT INTO items_fts(rowid, preview, content)
    VALUES (new.id, new.preview, new.content);
END;

CREATE TRIGGER IF NOT EXISTS items_fts_delete AFTER DELETE ON items BEGIN
    INSERT INTO items_fts(items_fts, rowid, preview, content)
    VALUES ('delete', old.id, old.preview, old.content);
END;

-- Scoped to the indexed columns so that bumping copy_count or toggling
-- `favorite` does not rewrite the full-text index.
CREATE TRIGGER IF NOT EXISTS items_fts_update
AFTER UPDATE OF preview, content ON items BEGIN
    INSERT INTO items_fts(items_fts, rowid, preview, content)
    VALUES ('delete', old.id, old.preview, old.content);
    INSERT INTO items_fts(rowid, preview, content)
    VALUES (new.id, new.preview, new.content);
END;

CREATE TABLE IF NOT EXISTS settings (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS collections (
    name TEXT PRIMARY KEY COLLATE NOCASE
);
"#,
        )?;
        if !column_exists(conn, "items", "file_assets")? {
            conn.execute("ALTER TABLE items ADD COLUMN file_assets TEXT", [])?;
        }
        if !column_exists(conn, "items", "tags")? {
            conn.execute(
                "ALTER TABLE items ADD COLUMN tags TEXT NOT NULL DEFAULT '[]'",
                [],
            )?;
        }
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_items_app_name ON items(app_name)",
            [],
        )?;
        for (column, definition) in [
            ("device_id", "TEXT NOT NULL DEFAULT 'local'"),
            ("device_name", "TEXT NOT NULL DEFAULT 'This device'"),
            ("device_platform", "TEXT NOT NULL DEFAULT 'windows'"),
            ("device_color", "TEXT NOT NULL DEFAULT '#28b7e8'"),
            ("sync_status", "TEXT NOT NULL DEFAULT 'local'"),
        ] {
            if !column_exists(conn, "items", column)? {
                conn.execute(
                    &format!("ALTER TABLE items ADD COLUMN {column} {definition}"),
                    [],
                )?;
            }
        }
        // Early builds persisted boolean format flags into the TEXT flavor
        // columns. Normalize those rows so reads cannot fail with an SQLite
        // InvalidColumnType error and hide the entire history list.
        conn.execute_batch(
            "UPDATE items
                SET html = CASE WHEN CAST(html AS INTEGER) = 1 THEN '' ELSE NULL END
              WHERE typeof(html) = 'integer' OR html IN ('0', '1');
             UPDATE items
                SET rtf = CASE WHEN CAST(rtf AS INTEGER) = 1 THEN '' ELSE NULL END
              WHERE typeof(rtf) = 'integer' OR rtf IN ('0', '1');",
        )?;
        Ok(())
    }

    /// Inserts a new entry, or — when the same payload is already present —
    /// moves the existing row to the top and increments its copy counter.
    ///
    /// Dedup is keyed on the content hash, matching the behaviour users expect
    /// from clipboard managers: copying the same snippet twice yields one entry.
    pub fn upsert(&self, item: &NewItem) -> Result<Upsert> {
        let conn = self.conn.lock();
        let now = now_ms();

        let existing: Option<i64> = conn
            .query_row(
                "SELECT id FROM items WHERE hash = ?1",
                params![item.content_hash],
                |r| r.get(0),
            )
            .optional()?;

        if let Some(id) = existing {
            conn.execute(
                "UPDATE items
                    SET copy_count = copy_count + 1,
                        last_copied_at = ?2,
                        app_name = COALESCE(?3, app_name),
                        app_exe  = COALESCE(?4, app_exe),
                        app_icon = COALESCE(?5, app_icon),
                        html = COALESCE(?6, html),
                        rtf = COALESCE(?7, rtf)
                  WHERE id = ?1",
                params![
                    id,
                    now,
                    item.source.as_ref().map(|s| &s.name),
                    item.source.as_ref().map(|s| &s.exe_path),
                    item.source.as_ref().and_then(|s| s.icon_path.as_ref()),
                    item.html,
                    item.rtf,
                ],
            )?;
            return Ok(Upsert::Bumped(id));
        }

        let preview = if item.preview.is_empty() {
            build_preview(item)
        } else {
            truncate_chars(&item.preview, PREVIEW_LIMIT)
        };
        let files_json = if item.files.is_empty() {
            None
        } else {
            Some(serde_json::to_string(&item.files).unwrap_or_default())
        };
        let file_assets_json = if item.file_assets.is_empty() {
            None
        } else {
            Some(serde_json::to_string(&item.file_assets).unwrap_or_default())
        };
        let device = item.device.clone().unwrap_or_else(default_local_device);

        conn.execute(
            "INSERT INTO items (
                kind, preview, content, html, rtf,
                image_path, thumb_path, image_w, image_h,
                file_paths, size_bytes, hash,
                app_name, app_exe, app_icon,
                favorite, copy_count, first_copied_at, last_copied_at,
                file_assets, device_id, device_name, device_platform, device_color, sync_status
             ) VALUES (
                ?1, ?2, ?3, ?4, ?5,
                ?6, ?7, ?8, ?9,
                ?10, ?11, ?12,
                ?13, ?14, ?15,
                0, 1, ?16, ?16,
                ?17, ?18, ?19, ?20, ?21, ?22
             )",
            params![
                item.kind.as_str(),
                preview,
                item.content,
                item.html,
                item.rtf,
                item.image.as_ref().map(|i| &i.path),
                item.image.as_ref().map(|i| &i.thumb_path),
                item.image.as_ref().map(|i| i.width),
                item.image.as_ref().map(|i| i.height),
                files_json,
                item.size_bytes,
                item.content_hash,
                item.source.as_ref().map(|s| &s.name),
                item.source.as_ref().map(|s| &s.exe_path),
                item.source.as_ref().and_then(|s| s.icon_path.as_ref()),
                now,
                file_assets_json,
                device.id,
                device.name,
                device.platform.as_str(),
                device.color,
                item.sync_status.as_str(),
            ],
        )?;

        Ok(Upsert::Inserted(conn.last_insert_rowid()))
    }

    /// Returns history entries newest-first, honouring search and filters.
    pub fn list(&self, query: &ListQuery) -> Result<Vec<ClipItem>> {
        let conn = self.conn.lock();

        let limit = query.limit.unwrap_or(200).min(2_000) as i64;
        let offset = query.offset.unwrap_or(0) as i64;

        let mut sql = format!("SELECT {COLUMNS} FROM items WHERE 1 = 1");
        let mut binds: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        // FTS5 prefix match. An unusable search string (e.g. only punctuation)
        // degrades to "no text filter" rather than returning zero rows.
        let match_expr = query.search.as_deref().and_then(fts_match_expression);
        if let Some(expr) = match_expr {
            let plain = format!(
                "%{}%",
                query
                    .search
                    .as_deref()
                    .unwrap_or_default()
                    .trim()
                    .to_lowercase()
            );
            sql.push_str(" AND (id IN (SELECT rowid FROM items_fts WHERE items_fts MATCH ?) OR lower(COALESCE(app_name, '')) LIKE ? OR lower(COALESCE(app_exe, '')) LIKE ? OR lower(COALESCE(tags, '')) LIKE ? OR lower(COALESCE(device_name, '')) LIKE ?)");
            binds.push(Box::new(expr));
            binds.push(Box::new(plain.clone()));
            binds.push(Box::new(plain.clone()));
            binds.push(Box::new(plain.clone()));
            binds.push(Box::new(plain));
        }

        if !query.kinds.is_empty() {
            let placeholders = std::iter::repeat_n("?", query.kinds.len())
                .collect::<Vec<_>>()
                .join(",");
            sql.push_str(&format!(" AND kind IN ({placeholders})"));
            for kind in &query.kinds {
                binds.push(Box::new(kind.as_str().to_string()));
            }
        }

        if !query.device_ids.is_empty() {
            let placeholders = std::iter::repeat_n("?", query.device_ids.len())
                .collect::<Vec<_>>()
                .join(",");
            sql.push_str(&format!(" AND device_id IN ({placeholders})"));
            for device_id in &query.device_ids {
                binds.push(Box::new(device_id.clone()));
            }
        }

        if !query.tags.is_empty() {
            let placeholders = std::iter::repeat_n("?", query.tags.len())
                .collect::<Vec<_>>()
                .join(",");
            sql.push_str(&format!(
                " AND EXISTS (SELECT 1 FROM json_each(items.tags) AS item_tag WHERE item_tag.value IN ({placeholders}))"
            ));
            for tag in &query.tags {
                binds.push(Box::new(tag.trim().to_lowercase()));
            }
        }

        if !query.source_exes.is_empty() {
            let placeholders = std::iter::repeat_n("?", query.source_exes.len())
                .collect::<Vec<_>>()
                .join(",");
            sql.push_str(&format!(
                " AND lower(COALESCE(app_exe, '')) IN ({placeholders})"
            ));
            for source_exe in &query.source_exes {
                binds.push(Box::new(source_exe.trim().to_lowercase()));
            }
        }

        if query.favorites_only {
            sql.push_str(" AND favorite = 1");
        }

        sql.push_str(" ORDER BY favorite DESC, last_copied_at DESC LIMIT ? OFFSET ?");
        binds.push(Box::new(limit));
        binds.push(Box::new(offset));

        let mut stmt = conn.prepare_cached(&sql)?;
        let refs: Vec<&dyn rusqlite::ToSql> = binds.iter().map(|b| b.as_ref()).collect();
        let rows = stmt.query_map(refs.as_slice(), row_to_item)?;

        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    /// Returns every device represented in persisted history, most recently
    /// active first. Device identity is metadata, never inferred from labels.
    pub fn known_devices(&self) -> Result<Vec<DeviceIdentity>> {
        let conn = self.conn.lock();
        let mut statement = conn.prepare_cached(
            "SELECT device_id, device_name, device_platform, device_color,
                    MAX(last_copied_at) AS most_recent
             FROM items
             GROUP BY device_id, device_name, device_platform, device_color
             ORDER BY most_recent DESC",
        )?;
        let rows = statement.query_map([], |row| {
            Ok(DeviceIdentity {
                id: row.get(0)?,
                name: row.get(1)?,
                platform: PlatformKind::from_db_value(&row.get::<_, String>(2)?),
                color: row.get(3)?,
            })
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Error::from)
    }

    /// Returns normalized tags ordered by most recent use, then frequency.
    pub fn known_tags(&self) -> Result<Vec<String>> {
        let conn = self.conn.lock();
        let mut statement = conn.prepare_cached(
            "SELECT item_tag.value, COUNT(*) AS uses, MAX(items.last_copied_at) AS most_recent
             FROM items, json_each(items.tags) AS item_tag
             WHERE json_valid(items.tags) AND typeof(item_tag.value) = 'text'
             GROUP BY item_tag.value
             ORDER BY most_recent DESC, uses DESC, item_tag.value ASC",
        )?;
        let rows = statement.query_map([], |row| row.get(0))?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Error::from)
    }

    pub fn collections(&self) -> Result<Vec<CollectionSummary>> {
        let conn = self.conn.lock();
        let mut statement = conn.prepare_cached(
            "SELECT collection.name, COUNT(item.id) AS item_count
             FROM (SELECT name FROM collections UNION SELECT DISTINCT item_tag.value FROM items, json_each(items.tags) AS item_tag WHERE json_valid(items.tags) AND typeof(item_tag.value) = 'text') AS collection
             LEFT JOIN items AS item ON EXISTS (SELECT 1 FROM json_each(item.tags) AS item_tag WHERE lower(item_tag.value) = lower(collection.name))
             GROUP BY collection.name ORDER BY lower(collection.name)",
        )?;
        let rows = statement.query_map([], |row| {
            Ok(CollectionSummary {
                name: row.get(0)?,
                item_count: row.get(1)?,
            })
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Error::from)
    }

    pub fn create_collection(&self, name: &str) -> Result<()> {
        let name = normalize_collection_name(name)?;
        self.conn.lock().execute(
            "INSERT OR IGNORE INTO collections (name) VALUES (?1)",
            params![name],
        )?;
        Ok(())
    }

    pub fn delete_collection(&self, name: &str) -> Result<()> {
        let name = normalize_collection_name(name)?;
        let conn = self.conn.lock();
        conn.execute("DELETE FROM collections WHERE name = ?1", params![name])?;
        let mut statement = conn.prepare_cached("SELECT id, tags FROM items WHERE EXISTS (SELECT 1 FROM json_each(items.tags) AS item_tag WHERE lower(item_tag.value) = lower(?1))")?;
        let rows = statement.query_map(params![name], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })?;
        let affected = rows.collect::<std::result::Result<Vec<_>, _>>()?;
        for (id, tags) in affected {
            let tags: Vec<String> = serde_json::from_str(&tags).unwrap_or_default();
            let remaining: Vec<String> = tags
                .into_iter()
                .filter(|tag| !tag.eq_ignore_ascii_case(name.as_str()))
                .collect();
            let remaining_json = serde_json::to_string(&remaining)
                .map_err(|error| Error::Other(error.to_string()))?;
            conn.execute(
                "UPDATE items SET tags = ?2 WHERE id = ?1",
                params![id, remaining_json],
            )?;
        }
        Ok(())
    }

    /// Returns source applications represented in history, newest first.
    pub fn known_sources(&self) -> Result<Vec<SourceApp>> {
        let conn = self.conn.lock();
        let mut statement = conn.prepare_cached(
            "SELECT app_name, app_exe, app_icon
             FROM items
             WHERE app_exe IS NOT NULL AND trim(app_exe) != ''
             GROUP BY lower(app_exe)
             ORDER BY MAX(last_copied_at) DESC",
        )?;
        let rows = statement.query_map([], |row| {
            Ok(SourceApp {
                name: row
                    .get::<_, Option<String>>(0)?
                    .unwrap_or_else(|| "Application".into()),
                exe_path: row.get(1)?,
                icon_path: row.get(2)?,
            })
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Error::from)
    }

    pub fn apply_filter_action(
        &self,
        scope: &FilterScope,
        action: BulkFilterAction,
    ) -> Result<Vec<String>> {
        let conn = self.conn.lock();
        let predicate = match scope.kind {
            FilterScopeKind::Tag => {
                "EXISTS (SELECT 1 FROM json_each(items.tags) AS t WHERE lower(t.value) = lower(?1))"
            }
            FilterScopeKind::Device => "device_id = ?1",
            FilterScopeKind::Source => "lower(COALESCE(app_exe, '')) = lower(?1)",
        };
        if matches!(action, BulkFilterAction::FavoriteAll) {
            conn.execute(
                &format!("UPDATE items SET favorite = 1 WHERE {predicate}"),
                params![scope.value],
            )?;
            return Ok(Vec::new());
        }
        let favorite_guard = if matches!(action, BulkFilterAction::DeleteNonFavorites) {
            " AND favorite = 0"
        } else {
            ""
        };
        let filter = format!("WHERE {predicate}{favorite_guard}");
        let assets = collect_assets(&conn, &filter, params![scope.value])?;
        conn.execute(&format!("DELETE FROM items {filter}"), params![scope.value])?;
        filter_unreferenced_assets(&conn, assets)
    }

    pub fn get(&self, id: i64) -> Result<Option<ClipItem>> {
        let conn = self.conn.lock();
        let mut stmt =
            conn.prepare_cached(&format!("SELECT {COLUMNS} FROM items WHERE id = ?1"))?;
        Ok(stmt.query_row(params![id], row_to_item).optional()?)
    }

    /// Same as `get` but unwraps the `Option` into a `NotFound` error.
    /// Used by command handlers where the caller already has a valid id.
    pub fn get_required(&self, id: i64) -> Result<ClipItem> {
        self.get(id)?.ok_or(Error::NotFound("clipboard item"))
    }

    pub fn get_by_hash(&self, hash: &str) -> Result<Option<ClipItem>> {
        let conn = self.conn.lock();
        let mut stmt =
            conn.prepare_cached(&format!("SELECT {COLUMNS} FROM items WHERE hash = ?1"))?;
        Ok(stmt.query_row(params![hash], row_to_item).optional()?)
    }

    /// Returns the rich flavours for an entry: `(content, html, rtf)`.
    pub fn flavors(&self, id: i64) -> Result<Option<RichFlavors>> {
        let conn = self.conn.lock();
        Ok(conn
            .query_row(
                "SELECT content, html, rtf FROM items WHERE id = ?1",
                params![id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()?)
    }

    pub fn set_favorite(&self, id: i64, favorite: bool) -> Result<()> {
        let changed = self.conn.lock().execute(
            "UPDATE items SET favorite = ?2 WHERE id = ?1",
            params![id, favorite as i32],
        )?;
        require_changed(changed, "clipboard item")
    }

    pub fn set_tags(&self, id: i64, tags: &[String]) -> Result<ClipItem> {
        let normalized: Vec<String> = tags
            .iter()
            .map(|tag| tag.trim().trim_start_matches('#').to_lowercase())
            .filter(|tag| !tag.is_empty())
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .take(20)
            .collect();
        let json =
            serde_json::to_string(&normalized).map_err(|error| Error::Other(error.to_string()))?;
        let conn = self.conn.lock();
        for tag in &normalized {
            conn.execute(
                "INSERT OR IGNORE INTO collections (name) VALUES (?1)",
                params![tag],
            )?;
        }
        let changed = conn.execute(
            "UPDATE items SET tags = ?2 WHERE id = ?1",
            params![id, json],
        )?;
        require_changed(changed, "clipboard item")?;
        self.get_required(id)
    }

    /// Replaces the managed file snapshot state after the background copy
    /// worker completes. Original user paths remain unchanged in `file_paths`.
    pub fn set_file_assets(&self, id: i64, assets: &[StoredFile]) -> Result<Vec<String>> {
        let json =
            serde_json::to_string(assets).map_err(|error| Error::Other(error.to_string()))?;
        let size_bytes: i64 = assets
            .iter()
            .filter(|asset| asset.stored_path.is_some())
            .map(|asset| asset.size_bytes.min(i64::MAX as u64) as i64)
            .sum();
        let conn = self.conn.lock();
        let previous = collect_assets(&conn, "WHERE id = ?1", params![id])?;
        let changed = conn.execute(
            "UPDATE items SET file_assets = ?2, size_bytes = ?3 WHERE id = ?1",
            params![id, json, size_bytes],
        )?;
        require_changed(changed, "clipboard item")?;
        filter_unreferenced_assets(&conn, previous)
    }

    pub fn update_text_content(
        &self,
        id: i64,
        content: &str,
        kind: ItemKind,
        hash: &str,
    ) -> Result<ClipItem> {
        let preview = truncate_chars(
            content
                .lines()
                .map(str::trim)
                .find(|line| !line.is_empty())
                .unwrap_or_else(|| content.trim()),
            PREVIEW_LIMIT,
        );
        let conn = self.conn.lock();
        let conflict: Option<i64> = conn
            .query_row(
                "SELECT id FROM items WHERE hash = ?1 AND id != ?2",
                params![hash, id],
                |row| row.get(0),
            )
            .optional()?;
        if conflict.is_some() {
            return Err(Error::Other(
                "an identical clipboard item already exists".into(),
            ));
        }
        let changed = conn.execute(
            "UPDATE items
                SET kind = ?2, preview = ?3, content = ?4, hash = ?5,
                    html = NULL, rtf = NULL, last_copied_at = ?6
              WHERE id = ?1 AND kind NOT IN ('image', 'files')",
            params![id, kind.as_str(), preview, content, hash, now_ms()],
        )?;
        if changed == 0 {
            return Err(Error::NotFound("editable clipboard item"));
        }
        drop(conn);
        self.get_required(id)
    }

    /// Marks an entry as just-used without incrementing the copy counter, so
    /// pasting from history floats the entry back to the top of the list.
    pub fn touch(&self, id: i64) -> Result<()> {
        let changed = self.conn.lock().execute(
            "UPDATE items SET last_copied_at = ?2 WHERE id = ?1",
            params![id, now_ms()],
        )?;
        require_changed(changed, "clipboard item")
    }

    /// Deletes one entry, returning any on-disk assets that are now orphaned.
    pub fn delete(&self, id: i64) -> Result<Vec<String>> {
        let conn = self.conn.lock();
        let assets = collect_assets(&conn, "WHERE id = ?1", params![id])?;
        let changed = conn.execute("DELETE FROM items WHERE id = ?1", params![id])?;
        require_changed(changed, "clipboard item")?;
        filter_unreferenced_assets(&conn, assets)
    }

    /// Clears history. Starred entries survive unless `include_favorites`.
    pub fn clear(&self, include_favorites: bool) -> Result<Vec<String>> {
        let conn = self.conn.lock();
        let filter = if include_favorites {
            ""
        } else {
            "WHERE favorite = 0"
        };
        let assets = collect_assets(&conn, filter, params![])?;
        conn.execute(&format!("DELETE FROM items {filter}"), params![])?;
        filter_unreferenced_assets(&conn, assets)
    }

    /// Clears one clipboard category while preserving favorites by default.
    pub fn clear_kind(&self, kind: ItemKind, include_favorites: bool) -> Result<Vec<String>> {
        let conn = self.conn.lock();
        let filter = if include_favorites {
            "WHERE kind = ?1"
        } else {
            "WHERE kind = ?1 AND favorite = 0"
        };
        let assets = collect_assets(&conn, filter, params![kind.as_str()])?;
        conn.execute(
            &format!("DELETE FROM items {filter}"),
            params![kind.as_str()],
        )?;
        filter_unreferenced_assets(&conn, assets)
    }

    /// Enforces the retention policy. Starred entries are never pruned.
    ///
    /// Returns the on-disk assets belonging to the removed rows.
    pub fn prune(&self, max_items: u32, retention_days: u32) -> Result<Vec<String>> {
        let mut conn = self.conn.lock();
        let transaction = conn.transaction()?;
        let mut assets = Vec::new();

        if retention_days > 0 {
            let cutoff = now_ms() - (retention_days as i64) * 86_400_000;
            let filter = "WHERE favorite = 0 AND last_copied_at < ?1";
            assets.extend(collect_assets(&transaction, filter, params![cutoff])?);
            transaction.execute(
                "DELETE FROM items WHERE favorite = 0 AND last_copied_at < ?1",
                params![cutoff],
            )?;
        }

        if max_items > 0 {
            // Keep the N most recent non-favorites; drop whatever falls past it.
            let filter = "WHERE favorite = 0 AND id NOT IN (
                              SELECT id FROM items WHERE favorite = 0
                              ORDER BY last_copied_at DESC LIMIT ?1
                          )";
            assets.extend(collect_assets(&transaction, filter, params![max_items])?);
            transaction.execute(&format!("DELETE FROM items {filter}"), params![max_items])?;
        }

        let assets = filter_unreferenced_assets(&transaction, assets)?;
        transaction.commit()?;
        Ok(assets)
    }

    /// Aggregate counts surfaced to the UI for the status line.
    pub fn counts(&self) -> Result<Counts> {
        let conn = self.conn.lock();
        conn.query_row(
            "SELECT
                COUNT(*),
                COALESCE(SUM(favorite), 0),
                COALESCE(SUM(CASE WHEN kind = 'text' THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN kind = 'image' THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN kind = 'files' THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN kind = 'link' THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN kind = 'color' THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN kind = 'email' THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(size_bytes), 0)
             FROM items",
            [],
            |row| {
                let favorites = row.get(1)?;
                Ok(Counts {
                    total: row.get(0)?,
                    favorites,
                    pinned: favorites,
                    text: row.get(2)?,
                    images: row.get(3)?,
                    files: row.get(4)?,
                    links: row.get(5)?,
                    colors: row.get(6)?,
                    emails: row.get(7)?,
                    storage_bytes: row.get(8)?,
                })
            },
        )
        .map_err(Into::into)
    }

    /// The hash of the most recently captured entry. Used to short-circuit
    /// duplicate `WM_CLIPBOARDUPDATE` notifications, which Windows delivers more
    /// than once for a single copy in some applications.
    pub fn newest_hash(&self) -> Result<Option<String>> {
        let conn = self.conn.lock();
        Ok(conn
            .query_row(
                "SELECT hash FROM items ORDER BY last_copied_at DESC LIMIT 1",
                [],
                |r| r.get(0),
            )
            .optional()?)
    }

    pub fn load_settings(&self) -> Result<Settings> {
        let conn = self.conn.lock();
        let raw: Option<String> = conn
            .query_row("SELECT value FROM settings WHERE key = 'app'", [], |r| {
                r.get(0)
            })
            .optional()?;

        // Deserialise leniently: a settings blob written by an older build that
        // lacks newly added fields must not wipe the user's configuration.
        let Some(json) = raw else {
            return Ok(Settings::default());
        };
        let mut value: serde_json::Value = serde_json::from_str(&json).unwrap_or_default();
        let had_file_filter_mode = value.get("fileFilterMode").is_some();
        let stored_version = value
            .get("settingsVersion")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0);

        // Version 3 changed only the default. An explicit All or Include choice
        // must survive the one-time migration; only legacy blobs with no choice
        // inherit the safer Exclude default.
        if stored_version < 3 {
            if !had_file_filter_mode {
                value["fileFilterMode"] = serde_json::json!("exclude");
            }
            value["settingsVersion"] = serde_json::json!(3);
        }

        sanitize_settings_enums(&mut value);
        let defaults = serde_json::to_value(Settings::default())
            .map_err(|error| crate::error::Error::Other(error.to_string()))?;
        merge_valid_settings(&mut value, &defaults);
        serde_json::from_value(value).map_err(|error| crate::error::Error::Other(error.to_string()))
    }

    pub fn save_settings(&self, settings: &Settings) -> Result<()> {
        let json = serde_json::to_string(settings)
            .map_err(|e| crate::error::Error::Other(e.to_string()))?;
        self.conn.lock().execute(
            "INSERT INTO settings (key, value) VALUES ('app', ?1)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![json],
        )?;
        Ok(())
    }

    /// Rewrites only managed asset paths after a verified storage migration.
    pub fn migrate_storage(
        &self,
        old_root: &Path,
        new_root: &Path,
        settings: &Settings,
    ) -> Result<()> {
        let mut conn = self.conn.lock();
        let transaction = conn.transaction()?;
        let rows = {
            let mut statement =
                transaction.prepare("SELECT id, image_path, thumb_path, file_assets FROM items")?;
            let mapped = statement.query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, Option<String>>(3)?,
                ))
            })?;
            mapped.collect::<rusqlite::Result<Vec<_>>>()?
        };

        for (id, image_path, thumb_path, file_assets_json) in rows {
            let image_path = image_path
                .as_deref()
                .map(|path| migrated_path(path, old_root, new_root));
            let thumb_path = thumb_path
                .as_deref()
                .map(|path| migrated_path(path, old_root, new_root));
            let mut assets = file_assets_json
                .as_deref()
                .and_then(|json| serde_json::from_str::<Vec<StoredFile>>(json).ok())
                .unwrap_or_default();
            for asset in &mut assets {
                if let Some(stored_path) = asset.stored_path.as_deref() {
                    asset.stored_path = Some(migrated_path(stored_path, old_root, new_root));
                }
                if let Some(thumb_path) = asset.thumb_path.as_deref() {
                    asset.thumb_path = Some(migrated_path(thumb_path, old_root, new_root));
                }
            }
            let assets_json = if assets.is_empty() {
                None
            } else {
                Some(
                    serde_json::to_string(&assets)
                        .map_err(|error| Error::Other(error.to_string()))?,
                )
            };
            transaction.execute(
                "UPDATE items SET image_path = ?2, thumb_path = ?3, file_assets = ?4 WHERE id = ?1",
                params![id, image_path, thumb_path, assets_json],
            )?;
        }
        let settings_json =
            serde_json::to_string(settings).map_err(|error| Error::Other(error.to_string()))?;
        transaction.execute(
            "INSERT INTO settings (key, value) VALUES ('app', ?1)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![settings_json],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub fn import_synced_text_item(
        &self,
        device: &DeviceIdentity,
        kind: ItemKind,
        content: &str,
        content_hash: &str,
    ) -> Result<Upsert> {
        if !matches!(
            kind,
            ItemKind::Text | ItemKind::Link | ItemKind::Email | ItemKind::Color
        ) {
            return Err(Error::Other(
                "only text-like synced items are supported".into(),
            ));
        }
        self.upsert(&NewItem {
            kind,
            content: content.to_string(),
            size_bytes: content.len().min(i64::MAX as usize) as i64,
            content_hash: content_hash.to_string(),
            device: Some(device.clone()),
            sync_status: SyncStatus::Synced,
            ..Default::default()
        })
    }
}

fn normalize_collection_name(name: &str) -> Result<String> {
    let normalized = name
        .trim()
        .trim_start_matches('#')
        .chars()
        .take(40)
        .collect::<String>();
    if normalized.is_empty() {
        return Err(Error::Other("collection name cannot be empty".into()));
    }
    Ok(normalized.to_lowercase())
}

fn migrated_path(value: &str, old_root: &Path, new_root: &Path) -> String {
    let path = PathBuf::from(value);
    match path.strip_prefix(old_root) {
        Ok(relative) => new_root.join(relative).to_string_lossy().into_owned(),
        Err(_) => value.to_string(),
    }
}

/// Gathers the image/thumbnail paths for rows matching `filter` so the caller can
/// unlink them after the rows are deleted.
fn collect_assets(
    conn: &Connection,
    filter: &str,
    binds: impl rusqlite::Params,
) -> Result<Vec<String>> {
    let sql = format!("SELECT image_path, thumb_path, file_assets FROM items {filter}");
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(binds, |r| {
        Ok((
            r.get::<_, Option<String>>(0)?,
            r.get::<_, Option<String>>(1)?,
            r.get::<_, Option<String>>(2)?,
        ))
    })?;

    let mut assets = Vec::new();
    for row in rows {
        let (image, thumb, file_assets) = row?;
        assets.extend(image);
        assets.extend(thumb);
        if let Some(json) = file_assets {
            let stored = serde_json::from_str::<Vec<StoredFile>>(&json).unwrap_or_default();
            assets.extend(stored.into_iter().filter_map(|asset| asset.stored_path));
        }
    }
    Ok(assets)
}

/// Removes candidates still referenced by any remaining row. This makes
/// cleanup safe even after legacy databases or imports contain shared paths.
fn filter_unreferenced_assets(conn: &Connection, candidates: Vec<String>) -> Result<Vec<String>> {
    if candidates.is_empty() {
        return Ok(Vec::new());
    }

    let mut referenced = HashSet::new();
    let mut statement = conn.prepare("SELECT image_path, thumb_path, file_assets FROM items")?;
    let rows = statement.query_map([], |row| {
        Ok((
            row.get::<_, Option<String>>(0)?,
            row.get::<_, Option<String>>(1)?,
            row.get::<_, Option<String>>(2)?,
        ))
    })?;
    for row in rows {
        let (image, thumb, file_assets) = row?;
        referenced.extend(image);
        referenced.extend(thumb);
        if let Some(json) = file_assets {
            let stored = serde_json::from_str::<Vec<StoredFile>>(&json).unwrap_or_default();
            referenced.extend(stored.into_iter().filter_map(|asset| asset.stored_path));
        }
    }

    let mut unique = HashSet::new();
    Ok(candidates
        .into_iter()
        .filter(|asset| !referenced.contains(asset))
        .filter(|asset| unique.insert(asset.clone()))
        .collect())
}

fn require_changed(changed: usize, resource: &'static str) -> Result<()> {
    if changed == 0 {
        Err(Error::NotFound(resource))
    } else {
        Ok(())
    }
}

fn row_to_item(row: &Row<'_>) -> rusqlite::Result<ClipItem> {
    let image_path: Option<String> = row.get(6)?;
    let thumb_path: Option<String> = row.get(7)?;
    let image = match (image_path, thumb_path) {
        (Some(path), Some(thumb_path)) => Some(ImageMeta {
            path,
            thumb_path,
            width: row.get::<_, Option<u32>>(8)?.unwrap_or_default(),
            height: row.get::<_, Option<u32>>(9)?.unwrap_or_default(),
        }),
        _ => None,
    };

    let files = row
        .get::<_, Option<String>>(10)?
        .and_then(|json| serde_json::from_str::<Vec<String>>(&json).ok())
        .unwrap_or_default();
    let file_assets = row
        .get::<_, Option<String>>(19)?
        .and_then(|json| serde_json::from_str::<Vec<StoredFile>>(&json).ok())
        .unwrap_or_default();

    let app_name: Option<String> = row.get(12)?;
    let app_exe: Option<String> = row.get(13)?;
    let source = match (app_name, app_exe) {
        (Some(name), Some(exe_path)) => Some(SourceApp {
            name,
            exe_path,
            icon_path: row.get(14)?,
        }),
        _ => None,
    };

    Ok(ClipItem {
        id: row.get(0)?,
        kind: ItemKind::from_db_value(&row.get::<_, String>(1)?),
        preview: row.get(2)?,
        content: row.get(3)?,
        has_html: row.get::<_, Option<String>>(4)?.is_some(),
        has_rtf: row.get::<_, Option<String>>(5)?.is_some(),
        image,
        files,
        file_assets,
        size_bytes: row.get(11)?,
        tags: row
            .get::<_, Option<String>>(25)?
            .and_then(|json| serde_json::from_str::<Vec<String>>(&json).ok())
            .unwrap_or_default(),
        source,
        favorite: row.get::<_, i32>(15)? != 0,
        copy_count: row.get(16)?,
        device: DeviceIdentity {
            id: row.get(20)?,
            name: row.get(21)?,
            platform: PlatformKind::from_db_value(&row.get::<_, String>(22)?),
            color: row.get(23)?,
        },
        sync_status: SyncStatus::from_db_value(&row.get::<_, String>(24)?),
        first_copied_at: row.get(17)?,
        last_copied_at: row.get(18)?,
    })
}

fn default_local_device() -> DeviceIdentity {
    DeviceIdentity {
        id: "local".into(),
        name: "This device".into(),
        platform: PlatformKind::current(),
        color: "#28b7e8".into(),
    }
}

fn column_exists(conn: &Connection, table: &str, column: &str) -> Result<bool> {
    let mut statement = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let names = statement.query_map([], |row| row.get::<_, String>(1))?;
    for name in names {
        if name? == column {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Builds the single-line label shown in the list.
fn build_preview(item: &NewItem) -> String {
    if let Some(image) = &item.image {
        return format!("Image ({}×{})", image.width, image.height);
    }

    if !item.files.is_empty() {
        let first = item
            .files
            .first()
            .map(|p| file_name_of(p))
            .unwrap_or_default();
        return if item.files.len() == 1 {
            first
        } else {
            format!("{first} + {} more", item.files.len() - 1)
        };
    }

    let collapsed = item
        .content
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("")
        .to_string();

    let label = if collapsed.is_empty() {
        item.content.trim().to_string()
    } else {
        collapsed
    };

    truncate_chars(&label, PREVIEW_LIMIT)
}

fn file_name_of(path: &str) -> String {
    Path::new(path)
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string())
}

/// Truncates on a character boundary — byte slicing would panic on multi-byte
/// UTF-8, which clipboard content is full of.
fn truncate_chars(value: &str, limit: usize) -> String {
    if value.chars().count() <= limit {
        return value.to_string();
    }
    let mut out: String = value.chars().take(limit).collect();
    out.push('…');
    out
}

/// Turns a user search string into a safe FTS5 prefix-match expression.
///
/// Every token is quoted (so FTS5 operators inside user input are treated as
/// literals) and suffixed with `*` for as-you-type matching. Returns `None` when
/// nothing searchable remains.
fn fts_match_expression(search: &str) -> Option<String> {
    let tokens: Vec<String> = search
        .split_whitespace()
        .map(|token| {
            token
                .chars()
                .filter(|c| c.is_alphanumeric() || matches!(c, '_' | '-' | '.' | '@' | '/' | '#'))
                .collect::<String>()
        })
        .filter(|token| !token.is_empty())
        .map(|token| format!("\"{token}\"*"))
        .collect();

    if tokens.is_empty() {
        None
    } else {
        Some(tokens.join(" "))
    }
}

fn sanitize_settings_enums(value: &mut serde_json::Value) {
    let Some(settings) = value.as_object_mut() else {
        return;
    };
    for (field, allowed) in [
        ("fileFilterMode", &["all", "include", "exclude"][..]),
        ("imageFormat", &["original", "png", "jpeg", "webp"][..]),
        (
            "imageCompression",
            &["none", "normal", "best", "manual"][..],
        ),
        ("backdrop", &["acrylic", "mica", "solid"][..]),
        ("theme", &["system", "light", "dark"][..]),
    ] {
        if settings
            .get(field)
            .and_then(serde_json::Value::as_str)
            .is_some_and(|value| !allowed.contains(&value))
        {
            settings.remove(field);
        }
    }
}

/// Merge persisted settings field-by-field. Missing or type-invalid fields use
/// their current defaults without discarding unrelated valid preferences.
fn merge_valid_settings(value: &mut serde_json::Value, defaults: &serde_json::Value) {
    let (Some(current), Some(defaults)) = (value.as_object_mut(), defaults.as_object()) else {
        *value = defaults.clone();
        return;
    };
    for (key, fallback) in defaults {
        match current.get_mut(key) {
            None => {
                current.insert(key.clone(), fallback.clone());
            }
            Some(existing) if !same_json_shape(existing, fallback) => {
                *existing = fallback.clone();
            }
            Some(_) => {}
        }
    }
}

fn same_json_shape(value: &serde_json::Value, fallback: &serde_json::Value) -> bool {
    use serde_json::Value;
    matches!(
        (value, fallback),
        (Value::Null, Value::Null)
            | (Value::Bool(_), Value::Bool(_))
            | (Value::Number(_), Value::Number(_))
            | (Value::String(_), Value::String(_))
            | (Value::Array(_), Value::Array(_))
            | (Value::Object(_), Value::Object(_))
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text_item(content: &str, hash: &str) -> NewItem {
        NewItem {
            kind: ItemKind::Text,
            content: content.to_string(),
            size_bytes: content.len() as i64,
            content_hash: hash.to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn insert_then_repeat_collapses_into_one_row() {
        let db = Db::open_in_memory().unwrap();
        assert!(db.upsert(&text_item("hello world", "h1")).unwrap().is_new());
        let second = db.upsert(&text_item("hello world", "h1")).unwrap();
        assert!(!second.is_new());

        let items = db.list(&ListQuery::default()).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].copy_count, 2);
    }

    #[test]
    fn full_text_search_matches_prefixes() {
        let db = Db::open_in_memory().unwrap();
        db.upsert(&text_item("the quick brown fox", "h1")).unwrap();
        db.upsert(&text_item("lazy dog sleeping", "h2")).unwrap();

        let hits = db
            .list(&ListQuery {
                search: Some("quic".into()),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].content, "the quick brown fox");
    }

    #[test]
    fn device_category_and_search_filters_compose() {
        let db = Db::open_in_memory().unwrap();
        let mut android_link = text_item("https://github.com/clipmo", "android-link");
        android_link.kind = ItemKind::Link;
        android_link.device = Some(DeviceIdentity {
            id: "phone-1".into(),
            name: "Android phone".into(),
            platform: PlatformKind::Android,
            color: "#00aa00".into(),
        });
        db.upsert(&android_link).unwrap();

        let mut windows_link = text_item("https://github.com/windows", "windows-link");
        windows_link.kind = ItemKind::Link;
        db.upsert(&windows_link).unwrap();

        let hits = db
            .list(&ListQuery {
                search: Some("github".into()),
                kinds: vec![ItemKind::Link],
                device_ids: vec!["phone-1".into()],
                ..Default::default()
            })
            .unwrap();

        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].device.id, "phone-1");

        let device_name_hits = db
            .list(&ListQuery {
                search: Some("android phone".into()),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(device_name_hits.len(), 1);
        assert_eq!(device_name_hits[0].device.id, "phone-1");
        assert_eq!(db.known_devices().unwrap().len(), 2);
    }

    #[test]
    fn punctuation_only_search_does_not_hide_everything() {
        let db = Db::open_in_memory().unwrap();
        db.upsert(&text_item("anything", "h1")).unwrap();

        let hits = db
            .list(&ListQuery {
                search: Some("***".into()),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(
            hits.len(),
            1,
            "unusable search must not filter everything out"
        );
    }

    #[test]
    fn toggling_favorite_keeps_the_row_searchable() {
        let db = Db::open_in_memory().unwrap();
        let id = db.upsert(&text_item("findable text", "h1")).unwrap().id();
        db.set_favorite(id, true).unwrap();

        let hits = db
            .list(&ListQuery {
                search: Some("findable".into()),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(hits.len(), 1);
        assert!(hits[0].favorite);
    }

    #[test]
    fn favorites_are_pinned_above_newer_history() {
        let db = Db::open_in_memory().unwrap();
        let favorite = db.upsert(&text_item("favorite", "favorite")).unwrap().id();
        db.set_favorite(favorite, true).unwrap();
        db.upsert(&text_item("newer", "newer")).unwrap();

        let items = db.list(&ListQuery::default()).unwrap();
        assert_eq!(items[0].id, favorite);
        assert!(items[0].favorite);
    }

    #[test]
    fn category_clear_preserves_favorites() {
        let db = Db::open_in_memory().unwrap();
        let favorite = db.upsert(&text_item("favorite", "favorite")).unwrap().id();
        db.set_favorite(favorite, true).unwrap();
        db.upsert(&text_item("remove", "remove")).unwrap();

        db.clear_kind(ItemKind::Text, false).unwrap();
        let items = db.list(&ListQuery::default()).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id, favorite);
    }

    #[test]
    fn edited_content_updates_kind_and_search_index() {
        let db = Db::open_in_memory().unwrap();
        let id = db.upsert(&text_item("before", "before")).unwrap().id();
        db.update_text_content(id, "person@clipdeck.local", ItemKind::Email, "after")
            .unwrap();

        let item = db.get_required(id).unwrap();
        assert_eq!(item.kind, ItemKind::Email);
        assert_eq!(item.content, "person@clipdeck.local");
        let hits = db
            .list(&ListQuery {
                search: Some("person".into()),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(hits.len(), 1);
    }

    #[test]
    fn tags_are_normalized_and_searchable() {
        let db = Db::open_in_memory().unwrap();
        let id = db
            .upsert(&text_item("quarterly report", "tagged"))
            .unwrap()
            .id();
        let item = db
            .set_tags(id, &[" Work ".into(), "#URGENT".into(), "work".into()])
            .unwrap();
        assert_eq!(item.tags, vec!["urgent", "work"]);
        let hits = db
            .list(&ListQuery {
                search: Some("urgent".into()),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, id);

        let filtered = db
            .list(&ListQuery {
                tags: vec!["work".into()],
                ..Default::default()
            })
            .unwrap();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].id, id);
        assert_eq!(db.known_tags().unwrap(), vec!["urgent", "work"]);
    }

    #[test]
    fn application_name_is_searchable() {
        let db = Db::open_in_memory().unwrap();
        let mut item = text_item("copied value", "app-search");
        item.source = Some(SourceApp {
            name: "Visual Studio Code".into(),
            exe_path: r"C:\Apps\Code.exe".into(),
            icon_path: None,
        });
        let id = db.upsert(&item).unwrap().id();
        let hits = db
            .list(&ListQuery {
                search: Some("studio".into()),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, id);
    }

    #[test]
    fn missing_tag_target_is_reported() {
        let db = Db::open_in_memory().unwrap();
        assert!(matches!(
            db.set_tags(999, &["missing".into()]),
            Err(Error::NotFound("clipboard item"))
        ));
    }

    #[test]
    fn prune_respects_favorites_and_max_items() {
        let db = Db::open_in_memory().unwrap();
        let keep = db.upsert(&text_item("starred", "fav")).unwrap().id();
        db.set_favorite(keep, true).unwrap();
        for i in 0..5 {
            db.upsert(&text_item(&format!("item {i}"), &format!("h{i}")))
                .unwrap();
        }

        db.prune(2, 0).unwrap();
        let items = db.list(&ListQuery::default()).unwrap();
        // 2 most recent non-favorites + the starred entry.
        assert_eq!(items.len(), 3);
        assert!(items.iter().any(|i| i.id == keep));
    }

    #[test]
    fn prune_reports_assets_from_rows_removed_by_the_policy() {
        let db = Db::open_in_memory().unwrap();
        let make_image = |hash: &str, name: &str| NewItem {
            kind: ItemKind::Image,
            image: Some(ImageMeta {
                path: format!("C:/managed/images/{name}.png"),
                thumb_path: format!("C:/managed/thumbs/{name}.png"),
                width: 1,
                height: 1,
            }),
            content_hash: hash.into(),
            ..Default::default()
        };
        let older = db.upsert(&make_image("older-image", "older")).unwrap().id();
        let newer = db.upsert(&make_image("newer-image", "newer")).unwrap().id();
        db.conn
            .lock()
            .execute(
                "UPDATE items SET last_copied_at = CASE id WHEN ?1 THEN 1 WHEN ?2 THEN 2 END",
                params![older, newer],
            )
            .unwrap();

        let orphans = db.prune(1, 0).unwrap();

        assert_eq!(db.counts().unwrap().total, 1);
        assert_eq!(orphans.len(), 2);
        assert!(orphans.iter().any(|path| path.ends_with("older.png")));
    }

    #[test]
    fn storage_migration_rewrites_only_paths_beneath_the_old_root() {
        let db = Db::open_in_memory().unwrap();
        let old_root = PathBuf::from("C:/managed-old");
        let new_root = PathBuf::from("C:/managed-new");
        let external_thumb = "D:/external/thumb.png";
        let external_snapshot = "D:/external/archive.txt";
        let item = NewItem {
            kind: ItemKind::Image,
            image: Some(ImageMeta {
                path: old_root
                    .join("images/captured.png")
                    .to_string_lossy()
                    .into_owned(),
                thumb_path: external_thumb.into(),
                width: 1,
                height: 1,
            }),
            file_assets: vec![StoredFile {
                original_path: "D:/source/archive.txt".into(),
                stored_path: Some(external_snapshot.into()),
                size_bytes: 1,
                is_directory: false,
                status: crate::models::StoredFileStatus::Ready,
                message: None,
                thumb_path: None,
            }],
            content_hash: "migration-paths".into(),
            ..Default::default()
        };
        let id = db.upsert(&item).unwrap().id();

        db.migrate_storage(&old_root, &new_root, &Settings::default())
            .unwrap();

        let migrated = db.get_required(id).unwrap();
        assert_eq!(
            migrated.image.as_ref().unwrap().path,
            new_root.join("images/captured.png").to_string_lossy()
        );
        assert_eq!(migrated.image.as_ref().unwrap().thumb_path, external_thumb);
        assert_eq!(
            migrated.file_assets[0].stored_path.as_deref(),
            Some(external_snapshot)
        );
    }

    #[test]
    fn delete_reports_orphaned_assets() {
        let db = Db::open_in_memory().unwrap();
        let item = NewItem {
            kind: ItemKind::Image,
            image: Some(ImageMeta {
                path: "C:/tmp/a.png".into(),
                thumb_path: "C:/tmp/a.thumb.png".into(),
                width: 10,
                height: 10,
            }),
            content_hash: "img".into(),
            ..Default::default()
        };
        let id = db.upsert(&item).unwrap().id();

        let assets = db.delete(id).unwrap();
        assert_eq!(assets.len(), 2);
        assert!(db.get(id).unwrap().is_none());
    }

    #[test]
    fn multibyte_preview_is_truncated_safely() {
        let db = Db::open_in_memory().unwrap();
        let content = "🎉".repeat(PREVIEW_LIMIT + 50);
        db.upsert(&text_item(&content, "emoji")).unwrap();
        let items = db.list(&ListQuery::default()).unwrap();
        assert!(items[0].preview.ends_with('…'));
    }

    #[test]
    fn settings_round_trip() {
        let db = Db::open_in_memory().unwrap();
        assert_eq!(db.load_settings().unwrap().hotkey, "Ctrl+Shift+V");

        let settings = Settings {
            hotkey: "Ctrl+Alt+C".into(),
            max_items: 42,
            ..Default::default()
        };
        db.save_settings(&settings).unwrap();

        let loaded = db.load_settings().unwrap();
        assert_eq!(loaded.hotkey, "Ctrl+Alt+C");
        assert_eq!(loaded.max_items, 42);
    }

    #[test]
    fn legacy_settings_without_filter_migrate_to_exclude() {
        let db = Db::open_in_memory().unwrap();
        db.conn
            .lock()
            .execute(
                "INSERT INTO settings (key, value) VALUES ('app', ?1)",
                params![r#"{"hotkey":"Alt+V","maxItems":17}"#],
            )
            .unwrap();
        let loaded = db.load_settings().unwrap();
        assert_eq!(
            loaded.file_filter_mode,
            crate::models::FileFilterMode::Exclude
        );
        assert_eq!(loaded.hotkey, "Alt+V");
        assert_eq!(loaded.max_items, 17);
        assert_eq!(loaded.settings_version, 3);
    }

    #[test]
    fn migration_preserves_explicit_all_and_include() {
        for mode in ["all", "include"] {
            let db = Db::open_in_memory().unwrap();
            let raw = format!(r#"{{"settingsVersion":2,"fileFilterMode":"{mode}"}}"#);
            db.conn
                .lock()
                .execute(
                    "INSERT INTO settings (key, value) VALUES ('app', ?1)",
                    params![raw],
                )
                .unwrap();
            assert_eq!(
                serde_json::to_value(db.load_settings().unwrap().file_filter_mode).unwrap(),
                serde_json::json!(mode)
            );
        }
    }

    #[test]
    fn invalid_field_type_uses_default_without_wiping_valid_fields() {
        let db = Db::open_in_memory().unwrap();
        db.conn
            .lock()
            .execute(
                "INSERT INTO settings (key, value) VALUES ('app', ?1)",
                params![r#"{"settingsVersion":3,"hotkey":"Alt+V","maxItems":"bad"}"#],
            )
            .unwrap();
        let loaded = db.load_settings().unwrap();
        assert_eq!(loaded.hotkey, "Alt+V");
        assert_eq!(loaded.max_items, Settings::default().max_items);
    }

    #[test]
    fn invalid_enum_and_null_fields_use_defaults_without_wiping_valid_fields() {
        let db = Db::open_in_memory().unwrap();
        db.conn
            .lock()
            .execute(
                "INSERT INTO settings (key, value) VALUES ('app', ?1)",
                params![r#"{"settingsVersion":3,"hotkey":"Alt+V","fileFilterMode":"unsafe","theme":null}"#],
            )
            .unwrap();
        let loaded = db.load_settings().unwrap();
        assert_eq!(loaded.hotkey, "Alt+V");
        assert_eq!(
            loaded.file_filter_mode,
            crate::models::FileFilterMode::Exclude
        );
        assert_eq!(loaded.theme, Settings::default().theme);
    }

    #[test]
    fn rich_flavors_are_persisted_verbatim() {
        let db = Db::open_in_memory().unwrap();
        let item = NewItem {
            kind: ItemKind::Text,
            content: "formatted".into(),
            html: Some("<strong>formatted</strong>".into()),
            rtf: Some(r"{\rtf1 formatted}".into()),
            content_hash: "rich".into(),
            ..Default::default()
        };
        let id = db.upsert(&item).unwrap().id();

        let (plain, html, rtf) = db.flavors(id).unwrap().unwrap();
        assert_eq!(plain, "formatted");
        assert_eq!(html.as_deref(), Some("<strong>formatted</strong>"));
        assert_eq!(rtf.as_deref(), Some(r"{\rtf1 formatted}"));
        let loaded = db.get_required(id).unwrap();
        assert!(loaded.has_html);
        assert!(loaded.has_rtf);
    }

    #[test]
    fn migration_normalizes_legacy_integer_flavor_flags() {
        let db = Db::open_in_memory().unwrap();
        let id = db.upsert(&text_item("legacy", "legacy")).unwrap().id();
        {
            let conn = db.conn.lock();
            conn.execute(
                "UPDATE items SET html = 1, rtf = 0 WHERE id = ?1",
                params![id],
            )
            .unwrap();
            Db::migrate(&conn).unwrap();
        }

        let item = db.get_required(id).unwrap();
        assert!(item.has_html);
        assert!(!item.has_rtf);
    }

    #[test]
    fn deleting_one_row_does_not_orphan_shared_assets() {
        let db = Db::open_in_memory().unwrap();
        let shared_image = "C:/managed/images/shared.png";
        let shared_thumb = "C:/managed/thumbs/shared.png";
        let make_image = |hash: &str| NewItem {
            kind: ItemKind::Image,
            image: Some(ImageMeta {
                path: shared_image.into(),
                thumb_path: shared_thumb.into(),
                width: 1,
                height: 1,
            }),
            content_hash: hash.into(),
            ..Default::default()
        };
        let first = db.upsert(&make_image("image-a")).unwrap().id();
        let second = db.upsert(&make_image("image-b")).unwrap().id();

        assert!(db.delete(first).unwrap().is_empty());
        let final_assets = db.delete(second).unwrap();
        assert_eq!(final_assets.len(), 2);
    }

    #[test]
    fn missing_rows_fail_mutating_operations() {
        let db = Db::open_in_memory().unwrap();
        assert!(matches!(
            db.set_favorite(999, true),
            Err(Error::NotFound("clipboard item"))
        ));
        assert!(matches!(
            db.touch(999),
            Err(Error::NotFound("clipboard item"))
        ));
        assert!(matches!(
            db.delete(999),
            Err(Error::NotFound("clipboard item"))
        ));
    }
}

//! Local MCP server so Claude Code, Codex, OpenCode and other MCP clients can read and
//! organize clipboard history.
//!
//! Off by default. When enabled in Settings it listens on 127.0.0.1 only and requires
//! a bearer token, so other devices on the network and other websites cannot reach it.
//! Transport: MCP "Streamable HTTP" (JSON-RPC over POST /mcp).
use std::io::Read;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager};

use crate::mcp_tools::{rpc_error, tool_definitions};
use crate::models::{ItemKind, ListQuery, NewItem, SyncStatus};
use crate::AppState;

const FIRST_PORT: u16 = 47_650;
const LAST_PORT: u16 = 47_659;
const MAX_BODY: usize = 2 * 1024 * 1024;
const PROTOCOL_VERSION: &str = "2025-03-26";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct McpFile {
    enabled: bool,
    token: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McpStatus {
    pub enabled: bool,
    pub running: bool,
    pub url: Option<String>,
    pub token: String,
}

#[derive(Default)]
pub struct McpServer {
    server: Mutex<Option<Arc<tiny_http::Server>>>,
    port: Mutex<u16>,
    stop: Arc<AtomicBool>,
}

fn file_path(app: &AppHandle) -> Option<PathBuf> {
    app.path()
        .app_data_dir()
        .ok()
        .map(|dir| dir.join("mcp.json"))
}

fn read_file(app: &AppHandle) -> McpFile {
    let mut file: McpFile = file_path(app)
        .and_then(|path| std::fs::read(path).ok())
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default();
    if file.token.len() < 32 {
        file.token = format!(
            "{}{}",
            uuid::Uuid::new_v4().simple(),
            uuid::Uuid::new_v4().simple()
        );
        write_file(app, &file);
    }
    file
}

fn write_file(app: &AppHandle, file: &McpFile) {
    if let Some(path) = file_path(app) {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(bytes) = serde_json::to_vec_pretty(file) {
            let _ = std::fs::write(&path, bytes);
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
            }
        }
    }
}

impl McpServer {
    pub fn status(&self, app: &AppHandle) -> McpStatus {
        let file = read_file(app);
        let running = self.server.lock().is_some();
        McpStatus {
            enabled: file.enabled,
            running,
            url: running.then(|| format!("http://127.0.0.1:{}/mcp", *self.port.lock())),
            token: file.token,
        }
    }

    pub fn start_if_enabled(&self, app: &AppHandle) {
        if read_file(app).enabled {
            self.start(app);
        }
    }

    pub fn set_enabled(&self, app: &AppHandle, enabled: bool) -> McpStatus {
        let mut file = read_file(app);
        file.enabled = enabled;
        write_file(app, &file);
        if enabled {
            self.start(app);
        } else {
            self.stop();
        }
        self.status(app)
    }

    pub fn rotate_token(&self, app: &AppHandle) -> McpStatus {
        let mut file = read_file(app);
        file.token = format!(
            "{}{}",
            uuid::Uuid::new_v4().simple(),
            uuid::Uuid::new_v4().simple()
        );
        write_file(app, &file);
        self.status(app)
    }

    fn stop(&self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(server) = self.server.lock().take() {
            server.unblock();
        }
    }

    fn start(&self, app: &AppHandle) {
        if self.server.lock().is_some() {
            return;
        }
        let mut bound = None;
        for port in FIRST_PORT..=LAST_PORT {
            if let Ok(server) = tiny_http::Server::http(("127.0.0.1", port)) {
                bound = Some((Arc::new(server), port));
                break;
            }
        }
        let Some((server, port)) = bound else {
            log::warn!("MCP server could not bind a local port");
            return;
        };
        self.stop.store(false, Ordering::SeqCst);
        *self.port.lock() = port;
        *self.server.lock() = Some(server.clone());
        let app = app.clone();
        let stop = self.stop.clone();
        let _ = std::thread::Builder::new()
            .name("mcp-server".into())
            .spawn(move || {
                for request in server.incoming_requests() {
                    if stop.load(Ordering::SeqCst) {
                        break;
                    }
                    let app = app.clone();
                    let _ = std::thread::spawn(move || handle(app, request));
                }
            });
        log::info!("MCP server listening on 127.0.0.1:{port}");
    }
}

fn respond(request: tiny_http::Request, status: u16, body: Option<Value>) {
    let mut response =
        tiny_http::Response::from_string(body.map(|value| value.to_string()).unwrap_or_default())
            .with_status_code(status);
    if let Ok(header) = tiny_http::Header::from_bytes("Content-Type", "application/json") {
        response = response.with_header(header);
    }
    let _ = request.respond(response);
}

fn handle(app: AppHandle, mut request: tiny_http::Request) {
    // Refuse browsers: a web page must never be able to drive this server.
    if request.headers().iter().any(|h| h.field.equiv("Origin")) {
        return respond(request, 403, Some(json!({"error": "forbidden"})));
    }
    let token = read_file(&app).token;
    let authorized = request
        .headers()
        .iter()
        .any(|h| h.field.equiv("Authorization") && h.value.as_str() == format!("Bearer {token}"));
    if !authorized {
        return respond(request, 401, Some(json!({"error": "unauthorized"})));
    }
    if request.url() != "/mcp" {
        return respond(request, 404, Some(json!({"error": "not found"})));
    }
    if *request.method() != tiny_http::Method::Post {
        return respond(request, 405, Some(json!({"error": "use POST"})));
    }
    let mut body = String::new();
    if request
        .as_reader()
        .take(MAX_BODY as u64)
        .read_to_string(&mut body)
        .is_err()
    {
        return respond(request, 400, Some(json!({"error": "bad body"})));
    }
    let Ok(message) = serde_json::from_str::<Value>(&body) else {
        return respond(
            request,
            400,
            Some(rpc_error(Value::Null, -32700, "parse error")),
        );
    };
    // Notifications carry no id and get no JSON-RPC response.
    let Some(id) = message.get("id").cloned() else {
        return respond(request, 202, None);
    };
    let method = message.get("method").and_then(Value::as_str).unwrap_or("");
    let params = message.get("params").cloned().unwrap_or(Value::Null);
    let result = match method {
        "initialize" => Ok(json!({
            "protocolVersion": PROTOCOL_VERSION,
            "capabilities": { "tools": {} },
            "serverInfo": { "name": "clipmo", "version": env!("CARGO_PKG_VERSION") },
            "instructions": "Clipmo clipboard history. Use list_clips/search first; ids are stable per device. Destructive tools need explicit user intent."
        })),
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({ "tools": tool_definitions() })),
        "tools/call" => {
            let name = params.get("name").and_then(Value::as_str).unwrap_or("");
            let args = params.get("arguments").cloned().unwrap_or(json!({}));
            Ok(match call_tool(&app, name, &args) {
                Ok(value) => json!({
                    "content": [{ "type": "text", "text": serde_json::to_string_pretty(&value).unwrap_or_default() }]
                }),
                Err(message) => json!({
                    "isError": true,
                    "content": [{ "type": "text", "text": message }]
                }),
            })
        }
        _ => Err((-32601, "method not found".to_string())),
    };
    let body = match result {
        Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
        Err((code, message)) => rpc_error(id, code, &message),
    };
    respond(request, 200, Some(body));
}

fn summarize(item: &crate::models::ClipItem, full: bool) -> Value {
    let content: String = if full {
        item.content.clone()
    } else {
        item.content.chars().take(300).collect()
    };
    json!({
        "id": item.id,
        "kind": item.kind,
        "content": content,
        "truncated": !full && item.content.chars().count() > 300,
        "favorite": item.favorite,
        "tags": item.tags,
        "device": item.device.name,
        "copyCount": item.copy_count,
        "lastCopiedAt": item.last_copied_at,
        "source": item.source.as_ref().map(|s| s.name.clone()),
    })
}

fn string_list(args: &Value, key: &str) -> Vec<String> {
    args.get(key)
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

fn id_arg(args: &Value, key: &str) -> Result<i64, String> {
    args.get(key)
        .and_then(Value::as_i64)
        .ok_or_else(|| format!("'{key}' must be an integer"))
}

fn call_tool(app: &AppHandle, name: &str, args: &Value) -> Result<Value, String> {
    let state = app.state::<AppState>();
    let e = |error: crate::error::Error| error.to_string();
    let changed = || {
        let _ = app.emit("clip-updated", ());
    };
    match name {
        "list_clips" => {
            let kinds = match args.get("kind").and_then(Value::as_str) {
                Some(kind) => vec![serde_json::from_value::<ItemKind>(json!(kind))
                    .map_err(|_| "unknown kind".to_string())?],
                None => Vec::new(),
            };
            let query = ListQuery {
                search: args
                    .get("search")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                kinds,
                favorites_only: args
                    .get("favorites_only")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
                tags: args
                    .get("tag")
                    .and_then(Value::as_str)
                    .map(|tag| vec![tag.trim().trim_start_matches('#').to_lowercase()])
                    .unwrap_or_default(),
                limit: Some(
                    args.get("limit")
                        .and_then(Value::as_u64)
                        .unwrap_or(20)
                        .clamp(1, 100) as u32,
                ),
                ..ListQuery::default()
            };
            let items = state.db.list(&query).map_err(e)?;
            Ok(json!(items
                .iter()
                .map(|i| summarize(i, false))
                .collect::<Vec<_>>()))
        }
        "get_clip" => {
            let item = state
                .db
                .get(id_arg(args, "id")?)
                .map_err(e)?
                .ok_or("clip not found")?;
            Ok(summarize(&item, true))
        }
        "add_clip" => {
            let text = args
                .get("text")
                .and_then(Value::as_str)
                .ok_or("'text' is required")?;
            if text.trim().is_empty() {
                return Err("text cannot be empty".into());
            }
            let kind = crate::clipboard::classify(text.trim());
            let preview: String = text
                .trim()
                .lines()
                .next()
                .unwrap_or("")
                .chars()
                .take(200)
                .collect();
            let upsert = state
                .db
                .upsert(&NewItem {
                    kind,
                    preview,
                    content: text.to_string(),
                    html: None,
                    rtf: None,
                    image: None,
                    files: vec![],
                    file_assets: vec![],
                    size_bytes: text.len() as i64,
                    content_hash: crate::clipboard::hash_text(text),
                    source: None,
                    device: None,
                    sync_status: SyncStatus::Local,
                })
                .map_err(e)?;
            let id = upsert.id();
            if args
                .get("favorite")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                state.db.set_favorite(id, true).map_err(e)?;
            }
            let tags = string_list(args, "tags");
            if !tags.is_empty() {
                state.db.set_tags(id, &tags).map_err(e)?;
            }
            let item = state.db.get(id).map_err(e)?.ok_or("clip not found")?;
            // Clips captured from the clipboard are handed to sync explicitly; do the same here.
            state.sync.enqueue_item(&item);
            changed();
            Ok(summarize(&item, false))
        }
        "set_favorite" => {
            let id = id_arg(args, "id")?;
            state
                .db
                .set_favorite(
                    id,
                    args.get("favorite")
                        .and_then(Value::as_bool)
                        .ok_or("'favorite' must be a boolean")?,
                )
                .map_err(e)?;
            changed();
            Ok(json!({ "ok": true }))
        }
        "delete_clip" => {
            let _guard = state.storage_operation.read();
            let orphans = state.db.delete(id_arg(args, "id")?).map_err(e)?;
            crate::commands::cleanup_asset_paths_public(&state.storage_root.read(), orphans);
            changed();
            Ok(json!({ "ok": true }))
        }
        "set_tags"
        | "add_tags"
        | "remove_tags"
        | "add_to_collection"
        | "remove_from_collection" => {
            let (ids, tags): (Vec<i64>, Vec<String>) = if name.ends_with("collection") {
                let ids = args
                    .get("ids")
                    .and_then(Value::as_array)
                    .map(|v| v.iter().filter_map(Value::as_i64).collect())
                    .unwrap_or_default();
                (
                    ids,
                    vec![args
                        .get("collection")
                        .and_then(Value::as_str)
                        .ok_or("'collection' is required")?
                        .to_string()],
                )
            } else {
                (vec![id_arg(args, "id")?], string_list(args, "tags"))
            };
            for id in ids {
                let item = state.db.get(id).map_err(e)?.ok_or("clip not found")?;
                let mut next = item.tags.clone();
                match name {
                    "set_tags" => next = tags.clone(),
                    "add_tags" | "add_to_collection" => next.extend(tags.clone()),
                    _ => next.retain(|t| {
                        !tags
                            .iter()
                            .any(|r| r.trim().trim_start_matches('#').eq_ignore_ascii_case(t))
                    }),
                }
                state.db.set_tags(id, &next).map_err(e)?;
            }
            changed();
            Ok(json!({ "ok": true }))
        }
        "list_devices" => {
            let settings = state.settings.read().clone();
            let sync = state.sync.state(&settings);
            Ok(json!({
                "syncEnabled": sync.enabled,
                "thisDevice": sync.device.name,
                "devices": sync.peers.iter().map(|peer| json!({
                    "id": peer.device.id,
                    "name": peer.device.name,
                    "platform": peer.device.platform,
                    "connected": peer.status == SyncStatus::Synced,
                })).collect::<Vec<_>>()
            }))
        }
        "send_to_device" => {
            let ids: Vec<i64> = args
                .get("ids")
                .and_then(Value::as_array)
                .map(|v| v.iter().filter_map(Value::as_i64).collect())
                .unwrap_or_default();
            if ids.is_empty() {
                return Err("'ids' must list at least one clip id".into());
            }
            let target = args
                .get("device_id")
                .and_then(Value::as_str)
                .map(str::to_string);
            let mut sent = 0;
            for id in ids {
                if let Some(item) = state.db.get(id).map_err(e)? {
                    if state.sync.send_item_to(&item, target.clone()) {
                        sent += 1;
                    }
                }
            }
            Ok(json!({ "queued": sent }))
        }
        "list_tags" => Ok(json!(state.db.known_tags().map_err(e)?)),
        "list_collections" => Ok(json!(state.db.collections().map_err(e)?)),
        "create_collection" => {
            state
                .db
                .create_collection(
                    args.get("name")
                        .and_then(Value::as_str)
                        .ok_or("'name' is required")?,
                )
                .map_err(e)?;
            changed();
            Ok(json!({ "ok": true }))
        }
        "delete_collection" => {
            state
                .db
                .delete_collection(
                    args.get("name")
                        .and_then(Value::as_str)
                        .ok_or("'name' is required")?,
                )
                .map_err(e)?;
            changed();
            Ok(json!({ "ok": true }))
        }
        "clear_history" => {
            if args.get("confirm").and_then(Value::as_bool) != Some(true) {
                return Err("clear_history is destructive: pass confirm=true only when the user explicitly asked".into());
            }
            let include_favorites = args
                .get("include_favorites")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let _guard = state.storage_operation.read();
            if args.get("scope").and_then(Value::as_str) == Some("device") {
                let ids = state.db.ids_for_clear(include_favorites, None).map_err(e)?;
                state.sync.detach_deletions(&ids);
            }
            let orphans = state.db.clear(include_favorites).map_err(e)?;
            crate::commands::cleanup_asset_paths_public(&state.storage_root.read(), orphans);
            changed();
            Ok(json!({ "ok": true }))
        }
        _ => Err(format!("unknown tool '{name}'")),
    }
}

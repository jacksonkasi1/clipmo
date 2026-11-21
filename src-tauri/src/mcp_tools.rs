//! Pure MCP tool catalogue and JSON-RPC helpers (no app state), so they can be unit tested.
use serde_json::{json, Value};

pub fn rpc_error(id: Value, code: i32, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

fn tool(name: &str, description: &str, properties: Value, required: &[&str]) -> Value {
    // Hints let MCP clients auto-approve reads and warn before destructive calls.
    let read_only = matches!(name, "list_clips" | "get_clip" | "list_tags" | "list_collections" | "list_devices");
    let destructive = matches!(name, "delete_clip" | "delete_collection" | "clear_history");
    json!({
        "name": name,
        "description": description,
        "inputSchema": { "type": "object", "properties": properties, "required": required },
        "annotations": {
            "readOnlyHint": read_only,
            "destructiveHint": destructive,
            "idempotentHint": read_only || matches!(name, "set_favorite" | "set_tags" | "create_collection"),
            "openWorldHint": false
        }
    })
}

pub fn tool_definitions() -> Vec<Value> {
    let id = json!({ "type": "integer", "description": "Clip id from list_clips" });
    let ids = json!({ "type": "array", "items": { "type": "integer" } });
    let strings = json!({ "type": "array", "items": { "type": "string" } });
    vec![
        tool("list_clips", "List or search clipboard history, newest first. Content is truncated; use get_clip for the full text.",
            json!({
                "search": { "type": "string" },
                "kind": { "type": "string", "enum": ["text","link","email","color","image","files"] },
                "favorites_only": { "type": "boolean" },
                "tag": { "type": "string", "description": "Only clips with this tag" },
                "limit": { "type": "integer", "description": "Default 20, max 100" }
            }), &[]),
        tool("get_clip", "Get one clip with its full text.", json!({ "id": id }), &["id"]),
        tool("add_clip", "Add a text clip to history.",
            json!({ "text": { "type": "string" }, "favorite": { "type": "boolean" }, "tags": strings }), &["text"]),
        tool("set_favorite", "Star or unstar a clip.", json!({ "id": id, "favorite": { "type": "boolean" } }), &["id", "favorite"]),
        tool("delete_clip", "Delete one clip (also removes it from synced devices).", json!({ "id": id }), &["id"]),
        tool("set_tags", "Replace the tags on a clip.", json!({ "id": id, "tags": strings }), &["id", "tags"]),
        tool("add_tags", "Add tags to a clip.", json!({ "id": id, "tags": strings }), &["id", "tags"]),
        tool("remove_tags", "Remove tags from a clip.", json!({ "id": id, "tags": strings }), &["id", "tags"]),
        tool("list_tags", "List all tags.", json!({}), &[]),
        tool("list_collections", "List collections with item counts.", json!({}), &[]),
        tool("create_collection", "Create an empty collection.", json!({ "name": { "type": "string" } }), &["name"]),
        tool("delete_collection", "Delete a collection (clips are kept).", json!({ "name": { "type": "string" } }), &["name"]),
        tool("add_to_collection", "Add clips to a collection.", json!({ "ids": ids, "collection": { "type": "string" } }), &["ids", "collection"]),
        tool("remove_from_collection", "Remove clips from a collection.", json!({ "ids": ids, "collection": { "type": "string" } }), &["ids", "collection"]),
        tool("list_devices", "List devices paired for sync and whether they are connected.", json!({}), &[]),
        tool("send_to_device", "Force-send clips to one paired device (device_id) or all of them (omit device_id), even when automatic sync would not.",
            json!({ "ids": ids, "device_id": { "type": "string", "description": "From list_devices; omit for all devices" } }), &["ids"]),
        tool("clear_history", "DESTRUCTIVE. Delete clips. Requires confirm=true. Favorites are kept unless include_favorites=true. scope 'device' removes only on this device; 'all' (default) also deletes on synced devices.",
            json!({ "confirm": { "type": "boolean" }, "include_favorites": { "type": "boolean" }, "scope": { "type": "string", "enum": ["all","device"] } }), &["confirm"]),
    ]
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_tool_has_a_unique_name_and_a_valid_schema() {
        let tools = tool_definitions();
        let mut names: Vec<&str> = tools.iter().filter_map(|t| t["name"].as_str()).collect();
        let total = names.len();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), total, "duplicate tool names");
        for tool in &tools {
            assert_eq!(tool["inputSchema"]["type"], "object");
            assert!(tool["description"].as_str().is_some_and(|d| d.len() > 10));
            // Every required field must be declared as a property.
            for required in tool["inputSchema"]["required"].as_array().unwrap() {
                assert!(tool["inputSchema"]["properties"].get(required.as_str().unwrap()).is_some());
            }
        }
    }

    #[test]
    fn destructive_tools_are_flagged_and_reads_are_read_only() {
        let tools = tool_definitions();
        let find = |name: &str| tools.iter().find(|t| t["name"] == name).unwrap().clone();
        assert_eq!(find("clear_history")["annotations"]["destructiveHint"], true);
        assert_eq!(find("delete_clip")["annotations"]["destructiveHint"], true);
        assert_eq!(find("list_clips")["annotations"]["readOnlyHint"], true);
        assert_eq!(find("set_favorite")["annotations"]["destructiveHint"], false);
    }

    #[test]
    fn readme_documents_every_tool() {
        let readme = include_str!("../../README.md");
        for tool in tool_definitions() {
            let name = tool["name"].as_str().unwrap();
            assert!(readme.contains(name), "README is missing MCP tool {name}");
        }
    }

    #[test]
    fn rpc_errors_have_the_jsonrpc_shape() {
        let error = rpc_error(json!(7), -32601, "method not found");
        assert_eq!(error["jsonrpc"], "2.0");
        assert_eq!(error["id"], 7);
        assert_eq!(error["error"]["code"], -32601);
    }
}

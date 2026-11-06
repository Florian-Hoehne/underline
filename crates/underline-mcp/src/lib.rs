//! Model Context Protocol (MCP) server for Underline.
//!
//! Exposes a workspace to an AI agent as MCP tools, so the user and the agent
//! read and edit the same notes. Messages are JSON-RPC 2.0, one per line,
//! as in the MCP stdio transport.

use serde_json::{json, Value};
use underline_core::{Note, NoteId, Workspace};

pub const PROTOCOL_VERSION: &str = "2024-11-05";

/// Handles MCP requests against one workspace.
pub struct Server {
    workspace: Workspace,
}

impl Server {
    pub fn new(workspace: Workspace) -> Self {
        Self { workspace }
    }

    /// Handles one JSON-RPC message. Returns `None` for notifications.
    pub fn handle(&self, message: &Value) -> Option<Value> {
        let id = message.get("id")?.clone();
        let method = message.get("method").and_then(Value::as_str).unwrap_or("");
        let params = message.get("params").cloned().unwrap_or(Value::Null);
        let result = match method {
            "initialize" => Ok(json!({
                "protocolVersion": PROTOCOL_VERSION,
                "capabilities": { "tools": {} },
                "serverInfo": { "name": "underline", "version": env!("CARGO_PKG_VERSION") },
            })),
            "ping" => Ok(json!({})),
            "tools/list" => Ok(json!({ "tools": tools() })),
            "tools/call" => Ok(self.call_tool(&params)),
            _ => Err(json!({ "code": -32601, "message": format!("method not found: {method}") })),
        };
        Some(match result {
            Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
            Err(error) => json!({ "jsonrpc": "2.0", "id": id, "error": error }),
        })
    }

    /// Handles one line of the stdio transport.
    pub fn handle_line(&self, line: &str) -> Option<String> {
        let response = match serde_json::from_str::<Value>(line) {
            Ok(message) => self.handle(&message)?,
            Err(err) => json!({
                "jsonrpc": "2.0",
                "id": null,
                "error": { "code": -32700, "message": err.to_string() },
            }),
        };
        Some(response.to_string())
    }

    fn call_tool(&self, params: &Value) -> Value {
        let args = &params["arguments"];
        let page = || args["page"].as_str().map(NoteId::from_title);
        let outcome = match params["name"].as_str() {
            Some("list_notes") => self
                .workspace
                .list()
                .map(|ids| {
                    ids.iter()
                        .map(NoteId::to_string)
                        .collect::<Vec<_>>()
                        .join("\n")
                })
                .map_err(|e| e.to_string()),
            Some("read_note") => match page() {
                Some(id) => self
                    .workspace
                    .read(&id)
                    .map(|n| n.body)
                    .map_err(|e| e.to_string()),
                None => Err("missing argument `page`".to_owned()),
            },
            Some("write_note") => match (page(), args["body"].as_str()) {
                (Some(id), Some(body)) => self
                    .workspace
                    .write(&Note {
                        id: id.clone(),
                        body: body.to_owned(),
                    })
                    .map(|()| format!("saved {id}"))
                    .map_err(|e| e.to_string()),
                _ => Err("missing argument `page` or `body`".to_owned()),
            },
            other => Err(format!("unknown tool: {}", other.unwrap_or(""))),
        };
        let (text, is_error) = match outcome {
            Ok(text) => (text, false),
            Err(text) => (text, true),
        };
        json!({ "content": [{ "type": "text", "text": text }], "isError": is_error })
    }
}

fn tools() -> Value {
    let page = json!({ "type": "string", "description": "Wiki page name, for example `Home`" });
    json!([
        {
            "name": "list_notes",
            "description": "List the pages in the workspace.",
            "inputSchema": { "type": "object", "properties": {} },
        },
        {
            "name": "read_note",
            "description": "Read a page's Markdown.",
            "inputSchema": { "type": "object", "properties": { "page": page }, "required": ["page"] },
        },
        {
            "name": "write_note",
            "description": "Create or replace a page's Markdown.",
            "inputSchema": {
                "type": "object",
                "properties": { "page": page, "body": { "type": "string" } },
                "required": ["page", "body"],
            },
        },
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn server(name: &str) -> Server {
        let dir = std::env::temp_dir().join(format!("underline-mcp-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        Server::new(Workspace::open(dir).unwrap())
    }

    fn call(server: &Server, id: u64, method: &str, params: Value) -> Value {
        server
            .handle(&json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }))
            .unwrap()
    }

    #[test]
    fn agent_writes_then_reads_a_note() {
        let server = server("roundtrip");
        let write = call(
            &server,
            1,
            "tools/call",
            json!({
                "name": "write_note", "arguments": { "page": "Plan", "body": "- ship it" },
            }),
        );
        assert_eq!(write["result"]["isError"], false);

        let list = call(
            &server,
            2,
            "tools/call",
            json!({ "name": "list_notes", "arguments": {} }),
        );
        assert_eq!(list["result"]["content"][0]["text"], "Plan");

        let read = call(
            &server,
            3,
            "tools/call",
            json!({
                "name": "read_note", "arguments": { "page": "Plan" },
            }),
        );
        assert_eq!(read["result"]["content"][0]["text"], "- ship it");
        std::fs::remove_dir_all(server.workspace.root()).unwrap();
    }

    #[test]
    fn initialize_advertises_tools() {
        let server = server("init");
        let response = call(&server, 1, "initialize", json!({}));
        assert_eq!(response["result"]["protocolVersion"], PROTOCOL_VERSION);
        let tools = call(&server, 2, "tools/list", json!({}));
        assert_eq!(tools["result"]["tools"].as_array().unwrap().len(), 3);
    }

    #[test]
    fn notifications_get_no_reply_and_bad_json_gets_parse_error() {
        let server = server("notify");
        assert!(server
            .handle_line(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#)
            .is_none());
        let reply: Value = serde_json::from_str(&server.handle_line("{").unwrap()).unwrap();
        assert_eq!(reply["error"]["code"], -32700);
    }
}

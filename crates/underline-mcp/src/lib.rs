//! Model Context Protocol (MCP) server for Underline.
//!
//! Exposes a workspace to an AI agent as MCP tools, so the user and the agent
//! read and edit the same notes. Messages are JSON-RPC 2.0, one per line,
//! as in the MCP stdio transport.

pub mod install;

use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use serde_json::{json, Value};
use underline_core::{
    remove_presence, stored_agent_access, user_settings_path, write_presence, AgentAccess, Note,
    NoteId, Presence, Workspace, PRESENCE_RENEW,
};

pub const PROTOCOL_VERSION: &str = "2024-11-05";

/// UC-017 BR-001: the reason given while access is off.
pub const REFUSED_OFF: &str = "Agent access to the notes is switched off in Underline. \
     The user can allow it in the status bar of Underline or in its settings (Agent access).";

/// UC-017 A6: the reason given for a change while access is read only.
pub const REFUSED_READ_ONLY: &str = "Underline allows agents only to read the notes of this \
     workspace. The user can allow writing in the status bar of Underline or in its settings \
     (Agent access).";

/// Handles MCP requests against one workspace.
pub struct Server {
    workspace: Workspace,
    /// The user settings file holding agent access (UC-017 BR-002).
    user_settings: Option<PathBuf>,
    /// What the window is told about the connected agent (UC-017 BR-004).
    presence: Arc<Mutex<Presence>>,
}

impl Server {
    /// A server for `workspace` that follows the user settings of this
    /// computer.
    pub fn new(workspace: Workspace) -> Self {
        Self::with_user_settings(workspace, user_settings_path())
    }

    /// A server that reads agent access from the user settings file
    /// `user_settings`.
    pub fn with_user_settings(workspace: Workspace, user_settings: Option<PathBuf>) -> Self {
        Self {
            workspace,
            user_settings,
            presence: Arc::default(),
        }
    }

    /// Serves the stdio transport until `input` ends, keeping a presence file
    /// in the workspace meanwhile (UC-017 BR-004), removed when it ends.
    pub fn serve(&self, input: impl BufRead, mut output: impl Write) -> io::Result<()> {
        let root = self.workspace.root().to_owned();
        let id = std::process::id();
        let done = Arc::new(AtomicBool::new(false));
        let renewer = {
            let (root, done, presence) = (root.clone(), done.clone(), self.presence.clone());
            std::thread::spawn(move || {
                while !done.load(Ordering::Relaxed) {
                    let current = presence.lock().map(|p| p.clone()).unwrap_or_default();
                    let _ = write_presence(&root, id, &current);
                    std::thread::park_timeout(PRESENCE_RENEW);
                }
            })
        };
        let served = (|| {
            for line in input.lines() {
                if let Some(reply) = self.handle_line(&line?) {
                    writeln!(output, "{reply}")?;
                    output.flush()?;
                }
            }
            Ok(())
        })();
        done.store(true, Ordering::Relaxed);
        renewer.thread().unpark();
        let _ = renewer.join();
        remove_presence(&root, id);
        served
    }

    /// The presence the server reports, for tests and the transport.
    pub fn presence(&self) -> Presence {
        self.presence.lock().map(|p| p.clone()).unwrap_or_default()
    }

    /// UC-017 BR-003: the level as stored now, read on every request.
    fn access(&self) -> Result<AgentAccess, String> {
        stored_agent_access(self.workspace.root(), self.user_settings.as_deref())
    }

    /// Handles one JSON-RPC message. Returns `None` for notifications.
    pub fn handle(&self, message: &Value) -> Option<Value> {
        let id = message.get("id")?.clone();
        let method = message.get("method").and_then(Value::as_str).unwrap_or("");
        let params = message.get("params").cloned().unwrap_or(Value::Null);
        let result = match method {
            "initialize" => {
                let name = params["clientInfo"]["name"]
                    .as_str()
                    .map(str::trim)
                    .filter(|n| !n.is_empty())
                    .map(display_name);
                if let Ok(mut presence) = self.presence.lock() {
                    presence.name = name;
                }
                Ok(json!({
                "protocolVersion": PROTOCOL_VERSION,
                "capabilities": { "tools": {} },
                "serverInfo": { "name": "underline", "version": env!("CARGO_PKG_VERSION") },
                }))
            }
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
        let name = params["name"].as_str();
        let outcome = match self.allowed(name) {
            Err(reason) => Err(reason),
            Ok(()) => self.run_tool(name, args, page),
        };
        let (text, is_error) = match outcome {
            Ok(text) => (text, false),
            Err(text) => (text, true),
        };
        json!({ "content": [{ "type": "text", "text": text }], "isError": is_error })
    }

    /// UC-017 BR-001, A6: whether the stored level allows the tool `name`.
    fn allowed(&self, name: Option<&str>) -> Result<(), String> {
        let writes = match name {
            Some("list_notes" | "read_note") => false,
            Some("write_note") => true,
            _ => return Ok(()),
        };
        let access = self
            .access()
            .map_err(|problem| format!("{REFUSED_OFF} (The settings cannot be read: {problem})"))?;
        let refused = !access.reads();
        if let Ok(mut presence) = self.presence.lock() {
            presence.refused = refused;
        }
        if refused {
            Err(REFUSED_OFF.to_owned())
        } else if writes && !access.writes() {
            Err(REFUSED_READ_ONLY.to_owned())
        } else {
            Ok(())
        }
    }

    fn run_tool(
        &self,
        name: Option<&str>,
        args: &Value,
        page: impl Fn() -> Option<NoteId>,
    ) -> Result<String, String> {
        match name {
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
        }
    }
}

/// The name shown for an agent tool's client name (UC-017 BR-004): the
/// identifiers some tools give are shown as their product names.
fn display_name(client: &str) -> String {
    let known = [
        ("claude-code", "Claude Code"),
        ("claude-ai", "Claude Desktop"),
        ("cursor-vscode", "Cursor"),
        ("cursor", "Cursor"),
        ("visual studio code", "VS Code"),
        ("codex-mcp-client", "Codex"),
        ("codex", "Codex"),
        ("gemini-cli-mcp-client", "Gemini CLI"),
        ("gemini-cli", "Gemini CLI"),
    ];
    let lower = client.to_lowercase();
    known
        .iter()
        .find(|(id, _)| lower == *id)
        .map_or_else(|| client.to_owned(), |(_, shown)| (*shown).to_owned())
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

    /// A server for a new workspace whose user settings allow `access`.
    fn server_with(name: &str, access: &str) -> Server {
        let base =
            std::env::temp_dir().join(format!("underline-mcp-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let dir = base.join("notes");
        std::fs::create_dir_all(&dir).unwrap();
        let user = base.join("user.yaml");
        std::fs::write(&user, format!("agent_access: {access}\n")).unwrap();
        Server::with_user_settings(Workspace::open(dir).unwrap(), Some(user))
    }

    fn server(name: &str) -> Server {
        server_with(name, "read and write")
    }

    fn set_access(server: &Server, access: &str) {
        let user = server.user_settings.as_ref().unwrap();
        std::fs::write(user, format!("agent_access: {access}\n")).unwrap();
    }

    fn tool(server: &Server, name: &str, arguments: Value) -> (bool, String) {
        let reply = call(
            server,
            9,
            "tools/call",
            json!({ "name": name, "arguments": arguments }),
        );
        let result = &reply["result"];
        (
            result["isError"] == true,
            result["content"][0]["text"]
                .as_str()
                .unwrap_or("")
                .to_owned(),
        )
    }

    fn remove(server: &Server) {
        let _ = std::fs::remove_dir_all(server.workspace.root().parent().unwrap());
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
        remove(&server);
    }

    #[test]
    fn initialize_advertises_tools() {
        let server = server("init");
        let response = call(&server, 1, "initialize", json!({}));
        assert_eq!(response["result"]["protocolVersion"], PROTOCOL_VERSION);
        let tools = call(&server, 2, "tools/list", json!({}));
        assert_eq!(tools["result"]["tools"].as_array().unwrap().len(), 3);
        remove(&server);
    }

    #[test]
    fn notifications_get_no_reply_and_bad_json_gets_parse_error() {
        let server = server("notify");
        assert!(server
            .handle_line(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#)
            .is_none());
        let reply: Value = serde_json::from_str(&server.handle_line("{").unwrap()).unwrap();
        assert_eq!(reply["error"]["code"], -32700);
        remove(&server);
    }

    /// TC-017-01, TC-017-04, TC-017-06, UC-017 BR-001, BR-003: while access
    /// is off every request about notes is refused with the reason, the tools
    /// are still named, and a change applies from the next request.
    #[test]
    fn uc017_tc04_tc06_off_refuses_notes_and_takes_effect_at_once() {
        let server = server_with("off", "off");
        call(
            &server,
            1,
            "initialize",
            json!({ "clientInfo": { "name": "claude-code", "version": "2" } }),
        );
        assert_eq!(server.presence().shown_name(), "Claude Code");
        let tools = call(&server, 2, "tools/list", json!({}));
        assert_eq!(tools["result"]["tools"].as_array().unwrap().len(), 3);
        for (name, args) in [
            ("list_notes", json!({})),
            ("read_note", json!({ "page": "Plan" })),
            ("write_note", json!({ "page": "Plan", "body": "x" })),
        ] {
            assert_eq!(tool(&server, name, args), (true, REFUSED_OFF.to_owned()));
        }
        assert!(server.presence().refused);
        assert!(!server.workspace.root().join("Plan.md").exists());

        set_access(&server, "read and write");
        assert!(
            !tool(
                &server,
                "write_note",
                json!({ "page": "Plan", "body": "x" })
            )
            .0
        );
        assert_eq!(
            tool(&server, "list_notes", json!({})),
            (false, "Plan".into())
        );
        assert!(!server.presence().refused);

        set_access(&server, "off");
        assert_eq!(tool(&server, "list_notes", json!({})).1, REFUSED_OFF);
        remove(&server);
    }

    /// TC-017-10, UC-017 A6: read only lists and reads, but refuses changes.
    #[test]
    fn uc017_tc10_read_only_refuses_changes() {
        let server = server_with("read-only", "read only");
        std::fs::write(server.workspace.root().join("Plan.md"), "plan").unwrap();
        assert_eq!(
            tool(&server, "list_notes", json!({})),
            (false, "Plan".into())
        );
        assert_eq!(
            tool(&server, "read_note", json!({ "page": "Plan" })),
            (false, "plan".into())
        );
        for page in ["Plan", "New"] {
            let (error, text) = tool(&server, "write_note", json!({ "page": page, "body": "x" }));
            assert!(error);
            assert_eq!(text, REFUSED_READ_ONLY);
        }
        let root = server.workspace.root();
        assert_eq!(
            std::fs::read_to_string(root.join("Plan.md")).unwrap(),
            "plan"
        );
        assert!(!root.join("New.md").exists());
        remove(&server);
    }

    /// TC-017-05: a workspace lowers the user's level for its notes.
    #[test]
    fn uc017_tc05_workspace_lowers_access() {
        let server = server("lowered");
        let root = server.workspace.root();
        std::fs::write(root.join("underline.yaml"), "agent_access: read only\n").unwrap();
        assert!(!tool(&server, "list_notes", json!({})).0);
        let (error, text) = tool(&server, "write_note", json!({ "page": "New", "body": "x" }));
        assert!(error && text == REFUSED_READ_ONLY);
        remove(&server);
    }

    /// TC-017-07, UC-017 BR-004: the transport keeps a presence file while it
    /// runs and removes it when the agent tool ends it.
    #[cfg(unix)]
    #[test]
    fn uc017_tc07_presence_while_serving() {
        let server = server("presence");
        let root = server.workspace.root().to_owned();
        let (to_server, input) = std::os::unix::net::UnixStream::pair().unwrap();
        let mut output = Vec::new();
        std::thread::scope(|scope| {
            let serving = scope.spawn(|| server.serve(io::BufReader::new(input), &mut output));
            let mut to_server = to_server;
            writeln!(
                to_server,
                r#"{{"jsonrpc":"2.0","id":1,"method":"initialize","params":{{"clientInfo":{{"name":"Cursor"}}}}}}"#
            )
            .unwrap();
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
            let names = loop {
                let agents = underline_core::connected_agents(&root, std::time::SystemTime::now());
                if agents.first().and_then(|a| a.name.as_deref()) == Some("Cursor")
                    || std::time::Instant::now() > deadline
                {
                    break agents;
                }
                std::thread::sleep(std::time::Duration::from_millis(50));
            };
            assert_eq!(names.len(), 1);
            assert_eq!(names[0].shown_name(), "Cursor");
            drop(to_server);
            serving.join().unwrap().unwrap();
        });
        assert!(String::from_utf8(output)
            .unwrap()
            .contains("protocolVersion"));
        assert!(underline_core::connected_agents(&root, std::time::SystemTime::now()).is_empty());
        remove(&server);
    }

    #[test]
    fn product_names_of_known_tools() {
        assert_eq!(display_name("claude-code"), "Claude Code");
        assert_eq!(display_name("Some Tool"), "Some Tool");
    }
}

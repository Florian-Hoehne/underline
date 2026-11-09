//! Model Context Protocol (MCP) server for Underline.
//!
//! Exposes a workspace to an AI agent as MCP tools, so the user and the agent
//! read and edit the same notes. Messages are JSON-RPC 2.0, one per line,
//! as in the MCP stdio transport.

pub mod install;

use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};

use serde_json::{json, Value};
use underline_core::{
    apply_to_file, create_todo, delete_todo, find_note, list_for_agent, load_todos, read_for_agent,
    remove_presence, save_todo, search_for_agent, send_request, stored_agent_access, take_answer,
    user_settings_path, window_runs, withdraw_request, write_presence, AgentAccess, AgentAnswer,
    AgentChange, AgentRequest, Day, NoteFormat, NoteId, Presence, Reading, Todo, TodoError,
    Workspace, WriteProblem, PRESENCE_RENEW,
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
    /// ADR-018: counts the requests handed to the window.
    sent: AtomicU64,
}

/// ADR-018: how long the server waits for the window's answer.
const ANSWER_WAIT: Duration = Duration::from_secs(1);

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
            sent: AtomicU64::new(1),
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
        let name = params["name"].as_str();
        let outcome = match self.allowed(name) {
            Err(reason) => Err(reason),
            Ok(()) => self.run_tool(name, args),
        };
        let (texts, is_error) = match outcome {
            Ok(texts) => (texts, false),
            Err(text) => (vec![text], true),
        };
        let content: Vec<Value> = texts
            .into_iter()
            .map(|text| json!({ "type": "text", "text": text }))
            .collect();
        json!({ "content": content, "isError": is_error })
    }

    /// UC-017 BR-001, A6: whether the stored level allows the tool `name`.
    fn allowed(&self, name: Option<&str>) -> Result<(), String> {
        let writes = match name {
            Some("list_notes" | "search_notes" | "read_note" | "list_todos") => false,
            Some("create_todo" | "update_todo" | "delete_todo") => true,
            Some("create_note" | "replace_in_note" | "insert_into_note" | "rewrite_note") => true,
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

    /// UC-019, ADR-018: makes `change` to the note `note`: through the
    /// running window, which knows what the user typed and not saved, or
    /// itself while no window runs.
    fn change(&self, note: &str, change: AgentChange) -> Result<Vec<String>, String> {
        let root = self.workspace.root();
        let answer = if window_runs(root, SystemTime::now()) {
            let id = format!(
                "{}-{}",
                std::process::id(),
                self.sent.fetch_add(1, Ordering::Relaxed)
            );
            let request = AgentRequest {
                agent: self.presence().name,
                note: note.to_owned(),
                change: change.clone(),
            };
            send_request(root, &id, &request).map_err(|e| e.to_string())?;
            match self.await_answer(&id, ANSWER_WAIT) {
                Some(answer) => answer,
                // Not taken: the window is gone, or hangs.
                None if withdraw_request(root, &id) => {
                    if window_runs(root, SystemTime::now()) {
                        return Err(WriteProblem::Busy.to_string());
                    }
                    self.apply_itself(note, &change)
                }
                // Taken: the window answers in a moment.
                None => self
                    .await_answer(&id, ANSWER_WAIT)
                    .ok_or_else(|| WriteProblem::Busy.to_string())?,
            }
        } else {
            self.apply_itself(note, &change)
        };
        if !answer.ok {
            return Err(answer.message);
        }
        if let (Some(id), Some(lines), Ok(mut presence)) =
            (&answer.note, answer.lines, self.presence.lock())
        {
            presence.reading = Some(Reading::now(NoteId::from_relative(id), lines).written());
        }
        Ok(vec![answer.message])
    }

    fn apply_itself(&self, note: &str, change: &AgentChange) -> AgentAnswer {
        let mut workspace = self.workspace.clone();
        apply_to_file(&mut workspace, note, change).0
    }

    fn await_answer(&self, id: &str, wait: Duration) -> Option<AgentAnswer> {
        let until = Instant::now() + wait;
        loop {
            if let Some(answer) = take_answer(self.workspace.root(), id) {
                return Some(answer);
            }
            if Instant::now() >= until {
                return None;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    fn run_tool(&self, name: Option<&str>, args: &Value) -> Result<Vec<String>, String> {
        let text = |key: &str| args[key].as_str().map(str::trim).filter(|t| !t.is_empty());
        let line = |key: &str| -> Result<Option<usize>, String> {
            match &args[key] {
                Value::Null => Ok(None),
                value => value
                    .as_u64()
                    .map(|n| Some(n as usize))
                    .ok_or_else(|| format!("`{key}` must be a line number, counted from 1")),
            }
        };
        match name {
            // UC-018 steps 1–2, A9.
            Some("list_notes") => {
                let listed =
                    list_for_agent(&self.workspace, text("folder")).map_err(|e| e.to_string())?;
                if listed.is_empty() {
                    return Ok(vec!["There are no notes here.".to_owned()]);
                }
                Ok(vec![listed
                    .iter()
                    .map(|l| format!("{} · {}", shown_name(&l.entry.id), l.entry.format.name()))
                    .collect::<Vec<_>>()
                    .join("\n")])
            }
            // UC-018 steps 3–4, A4, A5.
            Some("search_notes") => {
                let words = text("words").ok_or("missing argument `words`")?;
                let listed = list_for_agent(&self.workspace, None).map_err(|e| e.to_string())?;
                Ok(vec![
                    search_for_agent(&self.workspace, &listed, words).to_text(words)
                ])
            }
            // UC-018 steps 5–7, A1–A3, A7, A8.
            Some("read_note") => {
                let wanted = text("note")
                    .or_else(|| text("page"))
                    .ok_or("missing argument `note`")?;
                let entry = find_note(&self.workspace.notes_now().notes, wanted)
                    .map_err(|e| e.to_string())?;
                let read = read_for_agent(
                    &self.workspace,
                    &entry,
                    line("from_line")?,
                    line("to_line")?,
                )
                .map_err(|e| e.to_string())?;
                if let (Some(lines), Ok(mut presence)) = (read.lines, self.presence.lock()) {
                    presence.reading = Some(Reading::now(read.id.clone(), lines));
                }
                Ok(vec![read.text.clone(), read.summary()])
            }
            // UC-019 A1.
            Some("create_note") => {
                let note = text("note").ok_or("missing argument `note`")?;
                let format = match text("format") {
                    None => None,
                    Some(f) if f.eq_ignore_ascii_case("markdown") => Some(NoteFormat::Markdown),
                    Some(f) if f.eq_ignore_ascii_case("asciidoc") => Some(NoteFormat::AsciiDoc),
                    Some(f) => {
                        return Err(format!("`format` is `markdown` or `asciidoc`, not `{f}`"))
                    }
                };
                let body = args["text"].as_str().unwrap_or_default().to_owned();
                self.change(note, AgentChange::Create { format, text: body })
            }
            // UC-019 steps 1–3, A4–A10.
            Some("replace_in_note") => {
                let note = text("note").ok_or("missing argument `note`")?;
                let old = args["old_text"]
                    .as_str()
                    .ok_or("missing argument `old_text`")?;
                let new = args["new_text"]
                    .as_str()
                    .ok_or("missing argument `new_text`")?;
                self.change(
                    note,
                    AgentChange::Replace {
                        old: old.to_owned(),
                        new: new.to_owned(),
                    },
                )
            }
            Some("insert_into_note") => {
                let note = text("note").ok_or("missing argument `note`")?;
                let body = args["text"].as_str().ok_or("missing argument `text`")?;
                self.change(
                    note,
                    AgentChange::Insert {
                        before_line: line("before_line")?,
                        text: body.to_owned(),
                    },
                )
            }
            Some("rewrite_note") => {
                let note = text("note").ok_or("missing argument `note`")?;
                let body = args["text"].as_str().ok_or("missing argument `text`")?;
                self.change(
                    note,
                    AgentChange::Rewrite {
                        text: body.to_owned(),
                    },
                )
            }
            // UC-020 A8.
            Some("list_todos") => {
                let todos = load_todos(self.workspace.root());
                let wanted = text("status").unwrap_or("open").to_lowercase();
                let shown: Vec<String> = todos
                    .todos
                    .iter()
                    .filter(|t| match wanted.as_str() {
                        "done" => t.is_done(),
                        "all" => true,
                        _ => !t.is_done(),
                    })
                    .map(todo_line)
                    .collect();
                Ok(vec![if shown.is_empty() {
                    format!("There are no {wanted} todos.")
                } else {
                    shown.join("\n")
                }])
            }
            Some("create_todo") => {
                let title = args["title"].as_str().unwrap_or_default();
                let mut todo =
                    create_todo(self.workspace.root(), title).map_err(|e| e.to_string())?;
                let changed = self.fill_todo(&mut todo, args);
                if changed.is_err() {
                    let _ = delete_todo(self.workspace.root(), todo.number);
                }
                changed?;
                self.todo_done(format!("created todo {}", todo.name()));
                Ok(vec![format!("created {}", todo_line(&todo))])
            }
            Some("update_todo") => {
                let number = args["number"].as_u64().ok_or("missing argument `number`")?;
                let todos = load_todos(self.workspace.root());
                let mut todo = todos
                    .get(number)
                    .cloned()
                    .ok_or_else(|| TodoError::NoSuchTodo(number).to_string())?;
                if let Some(title) = args["title"].as_str() {
                    todo.set_title(title).map_err(|e| e.to_string())?;
                }
                if let Some(done) = args["done"].as_bool() {
                    todo.set_done(done);
                }
                if let Some(note) = text("unlink") {
                    if !todo.unlink(note) {
                        return Err(format!("todo #{number} does not link to `{note}`"));
                    }
                }
                self.fill_todo(&mut todo, args)?;
                self.todo_done(format!("changed todo {}", todo.name()));
                Ok(vec![format!("changed {}", todo_line(&todo))])
            }
            Some("delete_todo") => {
                let number = args["number"].as_u64().ok_or("missing argument `number`")?;
                let todo = delete_todo(self.workspace.root(), number).map_err(|e| e.to_string())?;
                self.todo_done(format!("deleted todo {}", todo.name()));
                Ok(vec![format!("deleted {}", todo.name())])
            }
            other => Err(format!("unknown tool: {}", other.unwrap_or(""))),
        }
    }

    /// UC-020 A8: the fields an agent gives for a todo, checked, and the
    /// todo stored.
    fn fill_todo(&self, todo: &mut Todo, args: &Value) -> Result<(), String> {
        let day = |key: &str| -> Result<Option<Option<Day>>, String> {
            match args[key].as_str().map(str::trim) {
                None => Ok(None),
                Some("") => Ok(Some(None)),
                Some(text) => text
                    .parse()
                    .map(|d| Some(Some(d)))
                    .map_err(|e: TodoError| e.to_string()),
            }
        };
        if let Some(due) = day("due")? {
            todo.due = due;
        }
        if let Some(scheduled) = day("scheduled")? {
            todo.scheduled = scheduled;
        }
        if let Some(urgency) = args["urgency"].as_str() {
            todo.urgency = urgency.parse()?;
        }
        let mut links: Vec<&str> = args["links"]
            .as_array()
            .map(|a| a.iter().filter_map(Value::as_str).collect())
            .unwrap_or_default();
        if let Some(link) = args["link"].as_str() {
            links.push(link);
        }
        if !links.is_empty() {
            let notes = self.workspace.notes_now().notes;
            for link in links {
                let entry = find_note(&notes, link).map_err(|e| e.to_string())?;
                todo.link(&entry.id);
            }
        }
        save_todo(self.workspace.root(), todo).map_err(|e| e.to_string())
    }

    /// UC-020 A8 step 2: tells the window what the agent did.
    fn todo_done(&self, change: String) {
        let at = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as u64);
        if let Ok(mut presence) = self.presence.lock() {
            presence.todo_change = Some((at, change));
        }
    }
}

/// UC-020 A8: one todo as a line for the agent.
fn todo_line(todo: &Todo) -> String {
    let mut line = format!(
        "#{} {} · {} · {}",
        todo.number,
        todo.title,
        if todo.is_done() { "done" } else { "open" },
        todo.urgency.label().to_lowercase()
    );
    if let Some(due) = todo.due {
        line.push_str(&format!(" · due {due}"));
    }
    if let Some(scheduled) = todo.scheduled {
        line.push_str(&format!(" · scheduled {scheduled}"));
    }
    if !todo.links.is_empty() {
        line.push_str(&format!(" · notes: {}", todo.links.join(", ")));
    }
    line
}

/// UC-018 BR-002: a note named by its folder and title.
fn shown_name(id: &NoteId) -> String {
    match id.folder() {
        Some(folder) => format!("{folder}/{}", id.title()),
        None => id.title(),
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
    let note = json!({
        "type": "string",
        "description": "The note as listed: its folder and title, such as `Projects/Plan`; the title alone while it is unique",
    });
    json!([
        {
            "name": "list_notes",
            "description": "List the notes of the workspace, the most recently changed first, each with its folder, title, and note format (Markdown or AsciiDoc).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "folder": { "type": "string", "description": "Only the notes of this folder and the folders in it" },
                },
            },
        },
        {
            "name": "search_notes",
            "description": "Search the notes for words: every word must occur in the title, folder, or text, in any order and case; words in double quotes are one. Gives the notes found with the lines found and their line numbers.",
            "inputSchema": {
                "type": "object",
                "properties": { "words": { "type": "string" } },
                "required": ["words"],
            },
        },
        {
            "name": "read_note",
            "description": "Read a note as stored, in its note format; all of it or only some lines. A second text gives the note format and the number of lines.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "note": note,
                    "from_line": { "type": "integer", "minimum": 1, "description": "First line to read, counted from 1" },
                    "to_line": { "type": "integer", "minimum": 1, "description": "Last line to read" },
                },
                "required": ["note"],
            },
        },
        {
            "name": "list_todos",
            "description": "List the todos of the workspace: number, title, open or done, urgency, due and scheduled day, and the notes they link.",
            "inputSchema": {
                "type": "object",
                "properties": { "status": { "type": "string", "enum": ["open", "done", "all"] } },
            },
        },
        {
            "name": "create_todo",
            "description": "Create a todo. Days are YYYY-MM-DD; urgency is low, normal, high, or critical; links name notes as listed.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "title": { "type": "string" },
                    "due": { "type": "string" },
                    "scheduled": { "type": "string" },
                    "urgency": { "type": "string", "enum": ["low", "normal", "high", "critical"] },
                    "links": { "type": "array", "items": { "type": "string" } },
                },
                "required": ["title"],
            },
        },
        {
            "name": "update_todo",
            "description": "Change a todo: title, due or scheduled day (an empty text removes it), urgency, done (true or false), link or unlink a note.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "number": { "type": "integer" },
                    "title": { "type": "string" },
                    "due": { "type": "string" },
                    "scheduled": { "type": "string" },
                    "urgency": { "type": "string", "enum": ["low", "normal", "high", "critical"] },
                    "done": { "type": "boolean" },
                    "link": { "type": "string" },
                    "unlink": { "type": "string" },
                },
                "required": ["number"],
            },
        },
        {
            "name": "delete_todo",
            "description": "Delete a todo.",
            "inputSchema": {
                "type": "object",
                "properties": { "number": { "type": "integer" } },
                "required": ["number"],
            },
        },
        {
            "name": "create_note",
            "description": "Create a note with a text, in the workspace's default note format unless `format` names one. Fails when the note exists.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "note": { "type": "string", "description": "Folder and title of the new note, such as `Projects/Plan`" },
                    "text": { "type": "string", "description": "The text, in the note format" },
                    "format": { "type": "string", "enum": ["markdown", "asciidoc"] },
                },
                "required": ["note", "text"],
            },
        },
        {
            "name": "replace_in_note",
            "description": "Replace a passage of a note. `old_text` must occur exactly once in the note as stored, character for character; read the note first.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "note": note,
                    "old_text": { "type": "string" },
                    "new_text": { "type": "string" },
                },
                "required": ["note", "old_text", "new_text"],
            },
        },
        {
            "name": "insert_into_note",
            "description": "Insert text before a line of a note, or at its end without `before_line`.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "note": note,
                    "text": { "type": "string" },
                    "before_line": { "type": "integer", "minimum": 1, "description": "Line counted from 1; after the last line means the end" },
                },
                "required": ["note", "text"],
            },
        },
        {
            "name": "rewrite_note",
            "description": "Replace the whole text of a note. Prefer replace_in_note for a part.",
            "inputSchema": {
                "type": "object",
                "properties": { "note": note, "text": { "type": "string" } },
                "required": ["note", "text"],
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
                "name": "create_note", "arguments": { "note": "Plan", "text": "- ship it" },
            }),
        );
        assert_eq!(write["result"]["isError"], false);

        let list = call(
            &server,
            2,
            "tools/call",
            json!({ "name": "list_notes", "arguments": {} }),
        );
        assert_eq!(list["result"]["content"][0]["text"], "Plan · Markdown");

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
        assert_eq!(tools["result"]["tools"].as_array().unwrap().len(), 11);
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
        assert_eq!(tools["result"]["tools"].as_array().unwrap().len(), 11);
        for (name, args) in [
            ("list_notes", json!({})),
            ("search_notes", json!({ "words": "x" })),
            ("read_note", json!({ "page": "Plan" })),
            ("create_note", json!({ "note": "Plan", "text": "x" })),
        ] {
            assert_eq!(tool(&server, name, args), (true, REFUSED_OFF.to_owned()));
        }
        assert!(server.presence().refused);
        assert!(!server.workspace.root().join("Plan.md").exists());

        set_access(&server, "read and write");
        assert!(
            !tool(
                &server,
                "create_note",
                json!({ "note": "Plan", "text": "x" })
            )
            .0
        );
        assert_eq!(
            tool(&server, "list_notes", json!({})),
            (false, "Plan · Markdown".into())
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
            (false, "Plan · Markdown".into())
        );
        assert_eq!(
            tool(&server, "read_note", json!({ "page": "Plan" })),
            (false, "plan".into())
        );
        for page in ["Plan", "New"] {
            let (error, text) = tool(&server, "create_note", json!({ "note": page, "text": "x" }));
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
        let (error, text) = tool(
            &server,
            "create_note",
            json!({ "note": "New", "text": "x" }),
        );
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

    /// TC-018-01, TC-018-03, TC-018-13: list, search, and read as an agent;
    /// the lines read are told to the window.
    #[test]
    fn uc018_tc01_tc03_look_up_through_the_server() {
        let server = server_with("uc018", "read only");
        let root = server.workspace.root();
        std::fs::write(root.join("Plan.md"), "# Plan\n- ship the release\n").unwrap();
        std::fs::create_dir_all(root.join("Projects")).unwrap();
        std::fs::write(root.join("Projects/My-Ideas.adoc"), "= Ideas\n").unwrap();

        let (error, list) = tool(&server, "list_notes", json!({}));
        assert!(!error);
        assert!(list.contains("Plan · Markdown") && list.contains("Projects/My Ideas · AsciiDoc"));
        let (_, folder) = tool(&server, "list_notes", json!({ "folder": "Projects" }));
        assert_eq!(folder, "Projects/My Ideas · AsciiDoc");
        let (error, nowhere) = tool(&server, "list_notes", json!({ "folder": "Nowhere" }));
        assert!(error && nowhere.contains("no folder"));

        let (_, found) = tool(&server, "search_notes", json!({ "words": "release" }));
        assert_eq!(found, "Plan\n  2: - ship the release\n");

        let reply = call(
            &server,
            5,
            "tools/call",
            json!({ "name": "read_note", "arguments": { "note": "plan", "from_line": 2, "to_line": 9 } }),
        );
        let content = &reply["result"]["content"];
        assert_eq!(content[0]["text"], "- ship the release\n");
        assert_eq!(
            content[1]["text"],
            "Plan · Markdown · 2 lines · lines 2 to 2 given · the note ends at line 2"
        );
        let reading = server.presence().reading.unwrap();
        assert_eq!((reading.note.as_str(), reading.lines), ("Plan", (2, 2)));

        let (_, ideas) = tool(&server, "read_note", json!({ "note": "Projects/My Ideas" }));
        assert_eq!(ideas, "= Ideas\n");
        let (error, missing) = tool(&server, "read_note", json!({ "note": "Plann" }));
        assert!(error && missing.contains("closest are: Plan"));
        let (error, bad) = tool(
            &server,
            "read_note",
            json!({ "note": "Plan", "from_line": "x" }),
        );
        assert!(error && bad.contains("line number"));
        remove(&server);
    }

    /// TC-019-01 to TC-019-03, TC-019-15: without a window the server
    /// writes the files itself; the lines written are told to the window.
    #[test]
    fn uc019_tc15_writes_without_the_window() {
        let server = server("uc019-direct");
        let root = server.workspace.root().to_owned();
        let (error, created) = tool(
            &server,
            "create_note",
            json!({ "note": "Plan", "text": "# Plan\n- draft\n- ship\n" }),
        );
        assert!(!error, "{created}");
        let (error, changed) = tool(
            &server,
            "replace_in_note",
            json!({ "note": "Plan", "old_text": "- draft", "new_text": "- final draft" }),
        );
        assert!(!error && changed.contains("line 2"), "{changed}");
        let wrote = server.presence().reading.unwrap();
        assert!(wrote.wrote && wrote.lines == (2, 2));
        assert!(
            !tool(
                &server,
                "insert_into_note",
                json!({ "note": "Plan", "text": "- done" })
            )
            .0
        );
        assert!(
            !tool(
                &server,
                "insert_into_note",
                json!({ "note": "Plan", "text": "- review", "before_line": 3 })
            )
            .0
        );
        assert_eq!(
            std::fs::read_to_string(root.join("Plan.md")).unwrap(),
            "# Plan\n- final draft\n- review\n- ship\n- done"
        );
        let (error, missing) = tool(
            &server,
            "replace_in_note",
            json!({ "note": "Plan", "old_text": "zebra", "new_text": "x" }),
        );
        assert!(error && missing.contains("does not occur"), "{missing}");
        let (error, format) = tool(
            &server,
            "create_note",
            json!({ "note": "X", "text": "", "format": "word" }),
        );
        assert!(error && format.contains("markdown"));
        assert!(
            !tool(
                &server,
                "rewrite_note",
                json!({ "note": "Plan", "text": "# Plan\n" })
            )
            .0
        );
        assert_eq!(
            std::fs::read_to_string(root.join("Plan.md")).unwrap(),
            "# Plan\n"
        );
        remove(&server);
    }

    /// ADR-018: while a window runs, the change goes through it.
    #[test]
    fn uc019_changes_go_through_the_running_window() {
        let server = server("uc019-window");
        let root = server.workspace.root().to_owned();
        std::fs::write(root.join("Plan.md"), "a\n").unwrap();
        underline_core::renew_window_presence(&root).unwrap();
        let window = {
            let root = root.clone();
            std::thread::spawn(move || {
                for _ in 0..100 {
                    let requests = underline_core::take_requests(&root, SystemTime::now());
                    if let Some((id, request)) = requests.into_iter().next() {
                        assert_eq!(request.note, "Plan");
                        let answer = AgentAnswer {
                            ok: true,
                            message: "changed Plan in the tab".into(),
                            note: Some("Plan".into()),
                            lines: Some((1, 1)),
                        };
                        underline_core::answer_request(&root, &id, &answer).unwrap();
                        return true;
                    }
                    std::thread::sleep(Duration::from_millis(10));
                }
                false
            })
        };
        let (error, text) = tool(
            &server,
            "replace_in_note",
            json!({ "note": "Plan", "old_text": "a", "new_text": "b" }),
        );
        assert!(window.join().unwrap());
        assert_eq!((error, text.as_str()), (false, "changed Plan in the tab"));
        // The file is the window's to write.
        assert_eq!(
            std::fs::read_to_string(root.join("Plan.md")).unwrap(),
            "a\n"
        );

        // A window that does not answer: busy, and nothing written.
        let (error, text) = tool(
            &server,
            "rewrite_note",
            json!({ "note": "Plan", "text": "c" }),
        );
        assert!(error && text == WriteProblem::Busy.to_string(), "{text}");
        assert_eq!(
            std::fs::read_to_string(root.join("Plan.md")).unwrap(),
            "a\n"
        );
        remove(&server);
    }

    /// TC-020-09, UC-020 A8: agents list todos at read only, and change
    /// them at read and write.
    #[test]
    fn uc020_tc09_agent_manages_todos() {
        let server = server_with("uc020", "read only");
        let root = server.workspace.root().to_owned();
        std::fs::write(root.join("Plan.md"), "plan").unwrap();
        create_todo(&root, "Ship").unwrap();
        assert_eq!(
            tool(&server, "list_todos", json!({})),
            (false, "#1 Ship · open · normal".into())
        );
        let (error, text) = tool(&server, "update_todo", json!({ "number": 1, "done": true }));
        assert!(error && text == REFUSED_READ_ONLY);

        set_access(&server, "read and write");
        let (error, text) = tool(
            &server,
            "create_todo",
            json!({ "title": "Review", "links": ["plan"], "due": "2026-11-20", "urgency": "high" }),
        );
        assert!(!error, "{text}");
        assert_eq!(
            text,
            "created #2 Review · open · high · due 2026-11-20 · notes: Plan"
        );
        assert_eq!(
            server.presence().todo_change.unwrap().1,
            "created todo #2 Review"
        );
        assert!(!tool(&server, "update_todo", json!({ "number": 1, "done": true })).0);
        assert_eq!(
            tool(&server, "list_todos", json!({ "status": "done" })).1,
            "#1 Ship · done · normal"
        );
        let (error, text) = tool(
            &server,
            "update_todo",
            json!({ "number": 2, "due": "2026-02-30" }),
        );
        assert!(error && text.contains("not a day"), "{text}");
        let (error, text) = tool(
            &server,
            "create_todo",
            json!({ "title": "X", "links": ["Nowhere"] }),
        );
        assert!(error && text.contains("no note"), "{text}");
        assert_eq!(load_todos(&root).todos.len(), 2);
        assert!(
            !tool(
                &server,
                "update_todo",
                json!({ "number": 2, "due": "", "unlink": "Plan" })
            )
            .0
        );
        assert_eq!(
            tool(&server, "list_todos", json!({})).1,
            "#2 Review · open · high"
        );
        assert!(!tool(&server, "delete_todo", json!({ "number": 2 })).0);
        assert!(tool(&server, "delete_todo", json!({ "number": 2 })).0);

        set_access(&server, "off");
        assert_eq!(
            tool(&server, "list_todos", json!({})),
            (true, REFUSED_OFF.to_owned())
        );
        remove(&server);
    }
}

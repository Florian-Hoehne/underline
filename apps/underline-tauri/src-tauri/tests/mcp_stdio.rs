//! M-01.2, ADR-002, ADR-017, ADR-018 (feature F14): the Tauri binary keeps
//! the `mcp` subcommand. `underline-tauri mcp <dir>` serves the workspace
//! to an agent over MCP on stdin and stdout, end to end with the real
//! binary: it writes notes itself while no window runs (UC-017 A5), and
//! hands its changes to the running window when one does (ADR-018), which
//! here is a session driven as the app's tick drives it.
//!
//! The child gets a scratch home folder (`HOME`, `USERPROFILE`), so it
//! reads the user settings from there and never this computer's.

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::time::{Duration, Instant, SystemTime};

use serde_json::{json, Value};
use underline_core::{NoteId, StateStore, SETTINGS_FILE_NAME};
use underline_session::{Session, SettingsState};

fn scratch(name: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("underline-tauri-mcp-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A scratch home folder whose user settings say `settings`, and a
/// workspace `notes` beside it with `Plan.md`.
fn home_and_workspace(name: &str, settings: &str) -> (PathBuf, PathBuf) {
    let dir = scratch(name);
    let home = dir.join("home");
    let root = dir.join("notes");
    std::fs::create_dir_all(&home).unwrap();
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(home.join(SETTINGS_FILE_NAME), settings).unwrap();
    std::fs::write(root.join("Plan.md"), "# Plan\n\n- one\n- two\n").unwrap();
    (home, root)
}

/// The binary serving `root` over MCP, with `home` as its home folder.
struct Mcp {
    child: Child,
    /// Gone once the input is closed.
    stdin: Option<ChildStdin>,
    stdout: BufReader<ChildStdout>,
    next: u64,
}

impl Mcp {
    fn start(home: &Path, root: &Path) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_underline-tauri"))
            .arg("mcp")
            .arg(root)
            .env("HOME", home)
            .env("USERPROFILE", home)
            .env_remove("XDG_CONFIG_HOME")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .expect("the binary starts");
        let stdin = child.stdin.take();
        let stdout = BufReader::new(child.stdout.take().unwrap());
        let mut mcp = Self {
            child,
            stdin,
            stdout,
            next: 1,
        };
        let hello = mcp.request(
            "initialize",
            json!({ "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": { "name": "Test Agent" } }),
        );
        assert_eq!(
            hello["result"]["serverInfo"]["name"], "underline",
            "{hello}"
        );
        mcp
    }

    fn request(&mut self, method: &str, params: Value) -> Value {
        let id = self.next;
        self.next += 1;
        let message = json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params });
        let stdin = self.stdin.as_mut().expect("the input is open");
        writeln!(stdin, "{message}").unwrap();
        stdin.flush().unwrap();
        let mut line = String::new();
        self.stdout.read_line(&mut line).unwrap();
        let reply: Value = serde_json::from_str(&line).expect("a JSON-RPC reply");
        assert_eq!(reply["id"], id);
        reply
    }

    /// The texts of the tool's answer, and whether it is an error.
    fn call(&mut self, tool: &str, arguments: Value) -> (String, bool) {
        let reply = self.request(
            "tools/call",
            json!({ "name": tool, "arguments": arguments }),
        );
        let result = &reply["result"];
        let text = result["content"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|c| c["text"].as_str())
            .collect::<Vec<_>>()
            .join("\n");
        (text, result["isError"].as_bool().unwrap_or(false))
    }

    /// Closes stdin; the server ends and removes its presence file.
    fn end(mut self) {
        drop(self.stdin.take());
        let deadline = Instant::now() + Duration::from_secs(10);
        while Instant::now() < deadline {
            if self.child.try_wait().unwrap().is_some() {
                return;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        let _ = self.child.kill();
        panic!("the MCP server did not end with its input");
    }
}

impl Drop for Mcp {
    fn drop(&mut self) {
        if let Ok(None) = self.child.try_wait() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

/// The presence files of MCP servers in the workspace.
fn presences(root: &Path) -> usize {
    std::fs::read_dir(root.join(".underline/agents"))
        .map(|entries| {
            entries
                .flatten()
                .filter(|e| e.file_name().to_string_lossy().parse::<u32>().is_ok())
                .count()
        })
        .unwrap_or(0)
}

#[test]
fn m01_2_adr002_uc017_a5_mcp_serves_the_workspace_without_a_window() {
    let (home, root) = home_and_workspace("alone", "agent_access: read and write\n");
    let mut mcp = Mcp::start(&home, &root);

    let tools = mcp.request("tools/list", json!({}));
    assert!(tools["result"]["tools"].as_array().unwrap().len() >= 10);
    let (listed, error) = mcp.call("list_notes", json!({}));
    assert!(!error && listed.contains("Plan"), "{listed}");
    let (read, error) = mcp.call("read_note", json!({ "note": "Plan" }));
    assert!(!error && read.contains("- two"), "{read}");
    let (created, error) = mcp.call(
        "create_note",
        json!({ "note": "Ideas", "text": "# Ideas\n" }),
    );
    assert!(!error, "{created}");
    assert_eq!(
        std::fs::read_to_string(root.join("Ideas.md")).unwrap(),
        "# Ideas\n"
    );
    // UC-017 BR-004: connected while it runs.
    assert_eq!(presences(&root), 1);

    mcp.end();
    assert_eq!(presences(&root), 0, "the presence goes with the server");
}

#[test]
fn uc017_br001_mcp_follows_the_stored_access() {
    let (home, root) = home_and_workspace("off", "");
    let mut mcp = Mcp::start(&home, &root);

    let (refused, error) = mcp.call("list_notes", json!({}));

    assert!(error, "{refused}");
    mcp.end();
}

/// ADR-018, UC-019, NFR-008 (M-17.7): with a window running for the
/// workspace, the server hands a change of an open note to it; the window
/// makes it in the tab, stores it, and answers within a second.
#[test]
fn adr018_uc019_m17_7_the_window_makes_the_agents_change_of_an_open_note() {
    let (home, root) = home_and_workspace("window", "agent_access: read and write\n");
    let mut session = Session::new(
        Some(root.clone()),
        StateStore::in_memory(),
        None,
        SettingsState::with_user_file(Some(home.join(SETTINGS_FILE_NAME)), Instant::now()),
    );
    session.start_workspace();
    let plan = NoteId::from_relative("Plan");
    session.open_note(&plan);
    // The window renews its presence before the agent asks (as the tick
    // does every second).
    tick(&mut session);
    let mut mcp = Mcp::start(&home, &root);

    let asked = Instant::now();
    let agent = std::thread::spawn(move || {
        let answer = mcp.call(
            "replace_in_note",
            json!({ "note": "Plan", "old_text": "- two", "new_text": "- TWO" }),
        );
        mcp.end();
        answer
    });
    while !agent.is_finished() && asked.elapsed() < Duration::from_secs(5) {
        tick(&mut session);
        std::thread::sleep(Duration::from_millis(20));
    }
    let (text, error) = agent.join().unwrap();

    assert!(!error, "{text}");
    assert_eq!(text, "changed Plan; the change is in line 4");
    assert!(
        asked.elapsed() < Duration::from_secs(1),
        "NFR-008: {:?}",
        asked.elapsed()
    );
    assert_eq!(
        std::fs::read_to_string(root.join("Plan.md")).unwrap(),
        "# Plan\n\n- one\n- TWO\n"
    );
    // The window's presence goes when Underline ends (M-17.8).
    session.end().unwrap();
    assert!(!root.join(".underline/agents/window").exists());
}

/// What the app's tick thread does: the agents' files, then the tick.
fn tick(session: &mut Session) {
    let now = Instant::now();
    for _ in 0..4 {
        let Some(job) = session.agents_job(now) else {
            break;
        };
        let done = job.run(SystemTime::now());
        session.agents_done(done, now);
    }
    session.tick(now);
}

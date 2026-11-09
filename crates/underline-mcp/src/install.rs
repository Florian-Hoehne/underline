//! UC-016 Install MCP Server (ADR-017): an entry for a workspace in the
//! user configuration of an agent tool, changed in that tool's own file,
//! leaving everything else in it as it was.

use std::path::{Path, PathBuf};

use serde_json::{json, Map, Value};
use underline_core::write_atomic;

/// The agent tools MCP install knows (FR-060).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tool {
    ClaudeCode,
    ClaudeDesktop,
    Cursor,
    VsCode,
    Codex,
    GeminiCli,
}

impl Tool {
    pub const ALL: [Tool; 6] = [
        Self::ClaudeCode,
        Self::ClaudeDesktop,
        Self::Cursor,
        Self::VsCode,
        Self::Codex,
        Self::GeminiCli,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::ClaudeCode => "Claude Code",
            Self::ClaudeDesktop => "Claude Desktop",
            Self::Cursor => "Cursor",
            Self::VsCode => "VS Code (Copilot)",
            Self::Codex => "Codex",
            Self::GeminiCli => "Gemini CLI",
        }
    }

    /// The program that shows the tool is installed, if it has one.
    fn program(self) -> Option<&'static str> {
        match self {
            Self::ClaudeCode => Some("claude"),
            Self::ClaudeDesktop => None,
            Self::Cursor => Some("cursor"),
            Self::VsCode => Some("code"),
            Self::Codex => Some("codex"),
            Self::GeminiCli => Some("gemini"),
        }
    }

    /// The key that holds the MCP servers in the tool's configuration.
    fn key(self) -> &'static str {
        match self {
            Self::VsCode => "servers",
            Self::Codex => "mcp_servers",
            _ => "mcpServers",
        }
    }

    /// The tool's user configuration file on this computer (ADR-017).
    pub fn config_path(self, places: &Places) -> Option<PathBuf> {
        let home = places.home.as_ref()?;
        Some(match self {
            Self::ClaudeCode => home.join(".claude.json"),
            Self::ClaudeDesktop => places
                .app_config(self)?
                .join("Claude/claude_desktop_config.json"),
            Self::Cursor => home.join(".cursor/mcp.json"),
            Self::VsCode => places.app_config(self)?.join("Code/User/mcp.json"),
            Self::Codex => places
                .codex_home
                .clone()
                .unwrap_or_else(|| home.join(".codex"))
                .join("config.toml"),
            Self::GeminiCli => home.join(".gemini/settings.json"),
        })
    }
}

/// Where things are on this computer; taken from the environment, and set
/// by hand in tests.
#[derive(Debug, Clone, Default)]
pub struct Places {
    pub home: Option<PathBuf>,
    /// `%APPDATA%` on Windows.
    pub appdata: Option<PathBuf>,
    /// `$XDG_CONFIG_HOME` on Linux.
    pub xdg_config: Option<PathBuf>,
    pub codex_home: Option<PathBuf>,
    /// The folders of the `PATH`.
    pub path: Vec<PathBuf>,
    pub os: Os,
}

/// The operating system, for the places of configurations (NFR-013).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Os {
    #[default]
    Linux,
    MacOs,
    Windows,
}

impl Places {
    /// The places of this computer.
    pub fn of_this_computer() -> Self {
        let var = |name: &str| {
            std::env::var_os(name)
                .filter(|v| !v.is_empty())
                .map(PathBuf::from)
        };
        Self {
            home: var("HOME").or_else(|| var("USERPROFILE")),
            appdata: var("APPDATA"),
            xdg_config: var("XDG_CONFIG_HOME"),
            codex_home: var("CODEX_HOME"),
            path: std::env::var_os("PATH")
                .map(|p| std::env::split_paths(&p).collect())
                .unwrap_or_default(),
            os: if cfg!(target_os = "macos") {
                Os::MacOs
            } else if cfg!(windows) {
                Os::Windows
            } else {
                Os::Linux
            },
        }
    }

    /// The folder applications keep their configuration in.
    fn app_config(&self, _tool: Tool) -> Option<PathBuf> {
        match self.os {
            Os::MacOs => Some(self.home.as_ref()?.join("Library/Application Support")),
            Os::Windows => self.appdata.clone(),
            Os::Linux => self
                .xdg_config
                .clone()
                .or_else(|| Some(self.home.as_ref()?.join(".config"))),
        }
    }

    fn on_path(&self, program: &str) -> bool {
        self.path.iter().any(|dir| {
            dir.join(program).is_file()
                || (self.os == Os::Windows
                    && ["exe", "cmd", "bat"]
                        .iter()
                        .any(|e| dir.join(format!("{program}.{e}")).is_file()))
        })
    }
}

/// How a workspace stands in a tool (UC-016 BR-006).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstallState {
    NotFound,
    NotInstalled,
    /// Installed under this entry name.
    Installed(String),
    /// A2: an entry for this workspace starts another Underline.
    ForAnotherPlace(String),
}

/// What an entry starts: an Underline program serving a workspace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Server {
    pub program: PathBuf,
    pub workspace: PathBuf,
}

impl Server {
    fn args(&self) -> Vec<String> {
        vec!["mcp".into(), self.workspace.to_string_lossy().into_owned()]
    }

    /// UC-016 A4, FR-061: the configuration most MCP clients take.
    pub fn snippet(&self) -> String {
        let name = base_name(&self.workspace);
        let value = json!({
            "mcpServers": { name: { "command": self.program.to_string_lossy(), "args": self.args() } }
        });
        serde_json::to_string_pretty(&value).unwrap_or_default()
    }

    /// The command line, for tools that ask for one.
    pub fn command_line(&self) -> String {
        let quote = |s: &str| {
            if s.contains(char::is_whitespace) {
                format!("\"{s}\"")
            } else {
                s.to_owned()
            }
        };
        let mut parts = vec![quote(&self.program.to_string_lossy())];
        parts.extend(self.args().iter().map(|a| quote(a)));
        parts.join(" ")
    }
}

/// Why a configuration could not be changed (UC-016 A6).
#[derive(Debug, thiserror::Error)]
pub enum InstallError {
    #[error("{} cannot be read: {reason}", .path.display())]
    Unreadable { path: PathBuf, reason: String },
    #[error("{} cannot be written: {source}", .path.display())]
    Unwritable {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("the configuration of this tool could not be located")]
    NoPlace,
    /// A2: the entry starts another Underline; replacing it needs a yes.
    #[error("an entry for this workspace starts another Underline")]
    ForAnotherPlace(String),
}

/// UC-016 BR-001: `underline-` and the folder name.
fn base_name(workspace: &Path) -> String {
    let folder = workspace
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "workspace".into());
    let clean: String = folder
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect();
    format!("underline-{clean}")
}

/// The servers of a configuration, as name, program, and arguments.
type Entries = Vec<(String, String, Vec<String>)>;

/// The configuration of one tool as read.
enum Document {
    Json(Value),
    Toml(toml_edit::DocumentMut),
}

impl Document {
    fn read(tool: Tool, path: &Path) -> Result<Self, InstallError> {
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(err) => {
                return Err(InstallError::Unreadable {
                    path: path.to_owned(),
                    reason: err.to_string(),
                })
            }
        };
        let unreadable = |reason: String| InstallError::Unreadable {
            path: path.to_owned(),
            reason,
        };
        if tool == Tool::Codex {
            return text
                .parse::<toml_edit::DocumentMut>()
                .map(Document::Toml)
                .map_err(|e| unreadable(e.to_string()));
        }
        if text.trim().is_empty() {
            return Ok(Document::Json(json!({})));
        }
        match serde_json::from_str::<Value>(&text) {
            Ok(value @ Value::Object(_)) => Ok(Document::Json(value)),
            Ok(_) => Err(unreadable("it does not hold an object".into())),
            Err(err) => Err(unreadable(format!(
                "{err}; it may hold comments, which Underline does not change"
            ))),
        }
    }

    fn entries(&self, tool: Tool) -> Entries {
        match self {
            Document::Json(value) => value
                .get(tool.key())
                .and_then(Value::as_object)
                .map(|servers| {
                    servers
                        .iter()
                        .map(|(name, entry)| {
                            let command = entry["command"].as_str().unwrap_or_default().to_owned();
                            let args = entry["args"]
                                .as_array()
                                .map(|a| {
                                    a.iter()
                                        .filter_map(|v| v.as_str().map(str::to_owned))
                                        .collect()
                                })
                                .unwrap_or_default();
                            (name.clone(), command, args)
                        })
                        .collect()
                })
                .unwrap_or_default(),
            Document::Toml(doc) => doc
                .get(tool.key())
                .and_then(|t| t.as_table_like())
                .map(|servers| {
                    servers
                        .iter()
                        .map(|(name, entry)| {
                            let command = entry
                                .get("command")
                                .and_then(|c| c.as_str())
                                .unwrap_or_default()
                                .to_owned();
                            let args = entry
                                .get("args")
                                .and_then(|a| a.as_array())
                                .map(|a| {
                                    a.iter()
                                        .filter_map(|v| v.as_str().map(str::to_owned))
                                        .collect()
                                })
                                .unwrap_or_default();
                            (name.to_owned(), command, args)
                        })
                        .collect()
                })
                .unwrap_or_default(),
        }
    }

    fn set(&mut self, tool: Tool, name: &str, server: &Server) {
        match self {
            Document::Json(value) => {
                let root = value.as_object_mut().expect("an object");
                let servers = root
                    .entry(tool.key())
                    .or_insert_with(|| Value::Object(Map::new()));
                if !servers.is_object() {
                    *servers = Value::Object(Map::new());
                }
                let mut entry = Map::new();
                if matches!(tool, Tool::VsCode | Tool::ClaudeCode) {
                    entry.insert("type".into(), json!("stdio"));
                }
                entry.insert("command".into(), json!(server.program.to_string_lossy()));
                entry.insert("args".into(), json!(server.args()));
                servers
                    .as_object_mut()
                    .expect("an object")
                    .insert(name.to_owned(), Value::Object(entry));
            }
            Document::Toml(doc) => {
                let servers = doc
                    .entry(tool.key())
                    .or_insert(toml_edit::Item::Table(toml_edit::Table::new()));
                if let Some(servers) = servers.as_table_mut() {
                    servers.set_implicit(true);
                    let mut entry = toml_edit::Table::new();
                    entry["command"] =
                        toml_edit::value(server.program.to_string_lossy().into_owned());
                    let mut args = toml_edit::Array::new();
                    for arg in server.args() {
                        args.push(arg);
                    }
                    entry["args"] = toml_edit::value(args);
                    servers.insert(name, toml_edit::Item::Table(entry));
                }
            }
        }
    }

    fn remove(&mut self, tool: Tool, name: &str) {
        match self {
            Document::Json(value) => {
                if let Some(servers) = value.get_mut(tool.key()).and_then(Value::as_object_mut) {
                    servers.remove(name);
                }
            }
            Document::Toml(doc) => {
                if let Some(servers) = doc.get_mut(tool.key()).and_then(|t| t.as_table_like_mut()) {
                    servers.remove(name);
                }
            }
        }
    }

    fn write(&self, path: &Path) -> Result<(), InstallError> {
        let text = match self {
            Document::Json(value) => {
                let mut text = serde_json::to_string_pretty(value).unwrap_or_default();
                text.push('\n');
                text
            }
            Document::Toml(doc) => doc.to_string(),
        };
        let unwritable = |source| InstallError::Unwritable {
            path: path.to_owned(),
            source,
        };
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(unwritable)?;
        }
        write_atomic(path, text.as_bytes()).map_err(unwritable)
    }
}

/// This workspace's entry: the name of the entry serving its folder, and
/// whether it starts this Underline.
fn own_entry(entries: &Entries, server: &Server) -> Option<(String, bool)> {
    let args = server.args();
    entries
        .iter()
        .find(|(_, _, a)| *a == args)
        .map(|(name, command, _)| (name.clone(), Path::new(command) == server.program))
}

/// UC-016 BR-006: how the workspace stands in `tool`.
pub fn state(tool: Tool, places: &Places, server: &Server) -> Result<InstallState, InstallError> {
    let path = tool.config_path(places).ok_or(InstallError::NoPlace)?;
    // BR-003: its configuration, its folder, or its program is there.
    // Claude Code keeps its file in the home folder, so its own folder counts.
    let folder = match tool {
        Tool::ClaudeCode => places.home.as_ref().map(|h| h.join(".claude")),
        _ => path.parent().map(Path::to_owned),
    };
    let found = path.exists()
        || folder.is_some_and(|f| f.exists())
        || tool.program().is_some_and(|p| places.on_path(p));
    if !found {
        return Ok(InstallState::NotFound);
    }
    let entries = Document::read(tool, &path)?.entries(tool);
    Ok(match own_entry(&entries, server) {
        Some((name, true)) => InstallState::Installed(name),
        Some((name, false)) => InstallState::ForAnotherPlace(name),
        None => InstallState::NotInstalled,
    })
}

/// UC-016 step 4, A2: adds the workspace's entry to `tool`, keeping
/// everything else. An entry starting another Underline is replaced only
/// with `replace`. Returns the entry's name.
pub fn install(
    tool: Tool,
    places: &Places,
    server: &Server,
    replace: bool,
) -> Result<String, InstallError> {
    let path = tool.config_path(places).ok_or(InstallError::NoPlace)?;
    let mut document = Document::read(tool, &path)?;
    let entries = document.entries(tool);
    let name = match own_entry(&entries, server) {
        Some((name, true)) => return Ok(name),
        Some((name, false)) if !replace => return Err(InstallError::ForAnotherPlace(name)),
        Some((name, false)) => name,
        None => {
            let base = base_name(&server.workspace);
            let taken = |n: &str| entries.iter().any(|(e, _, _)| e == n);
            std::iter::once(base.clone())
                .chain((2..).map(|n| format!("{base}-{n}")))
                .find(|n| !taken(n))
                .unwrap_or(base)
        }
    };
    document.set(tool, &name, server);
    document.write(&path)?;
    Ok(name)
}

/// UC-016 A3: removes only the workspace's entry from `tool`.
pub fn remove(tool: Tool, places: &Places, server: &Server) -> Result<(), InstallError> {
    let path = tool.config_path(places).ok_or(InstallError::NoPlace)?;
    let mut document = Document::read(tool, &path)?;
    let Some((name, _)) = own_entry(&document.entries(tool), server) else {
        return Ok(());
    };
    document.remove(tool, &name);
    document.write(&path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    struct Home(PathBuf);

    impl Home {
        fn new(name: &str) -> Self {
            let dir =
                std::env::temp_dir().join(format!("underline-uc016-{name}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&dir);
            fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }

        fn places(&self, os: Os) -> Places {
            Places {
                home: Some(self.0.clone()),
                appdata: Some(self.0.join("AppData/Roaming")),
                xdg_config: None,
                codex_home: None,
                path: vec![self.0.join("bin")],
                os,
            }
        }

        fn read(&self, path: &str) -> String {
            fs::read_to_string(self.0.join(path)).unwrap()
        }
    }

    impl Drop for Home {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn server(workspace: &str) -> Server {
        Server {
            program: PathBuf::from("/opt/underline/underline"),
            workspace: PathBuf::from(workspace),
        }
    }

    /// TC-016-01 / TC-016-02: every tool's place, and an install read back.
    #[test]
    fn uc016_tc01_tc02_every_tool() {
        let home = Home::new("tools");
        let places = home.places(Os::Linux);
        let s = server("/home/ada/notes");
        for tool in Tool::ALL {
            assert_eq!(
                state(tool, &places, &s).unwrap(),
                InstallState::NotFound,
                "{tool:?}"
            );
            let path = tool.config_path(&places).unwrap();
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            if tool == Tool::ClaudeCode {
                fs::create_dir_all(home.0.join(".claude")).unwrap();
            }
            assert_eq!(
                state(tool, &places, &s).unwrap(),
                InstallState::NotInstalled,
                "{tool:?}"
            );
            assert_eq!(
                install(tool, &places, &s, false).unwrap(),
                "underline-notes"
            );
            assert_eq!(
                state(tool, &places, &s).unwrap(),
                InstallState::Installed("underline-notes".into()),
                "{tool:?}"
            );
        }
        let vs = home.read(".config/Code/User/mcp.json");
        assert!(
            vs.contains("\"servers\"") && vs.contains("\"type\": \"stdio\""),
            "{vs}"
        );
        let codex = home.read(".codex/config.toml");
        assert!(codex.contains("[mcp_servers.underline-notes]"), "{codex}");
        assert!(
            codex.contains("args = [\"mcp\", \"/home/ada/notes\"]"),
            "{codex}"
        );
        let mac = home.places(Os::MacOs);
        assert!(Tool::ClaudeDesktop
            .config_path(&mac)
            .unwrap()
            .ends_with("Library/Application Support/Claude/claude_desktop_config.json"));
        let win = home.places(Os::Windows);
        assert!(Tool::VsCode
            .config_path(&win)
            .unwrap()
            .ends_with("AppData/Roaming/Code/User/mcp.json"));
    }

    /// TC-016-03: a second workspace of the same folder name gets a number.
    #[test]
    fn uc016_tc03_one_entry_per_workspace() {
        let home = Home::new("names");
        let places = home.places(Os::Linux);
        fs::create_dir_all(home.0.join(".cursor")).unwrap();
        let a = server("/home/ada/a/notes");
        let b = server("/home/ada/b/notes");
        assert_eq!(
            install(Tool::Cursor, &places, &a, false).unwrap(),
            "underline-notes"
        );
        assert_eq!(
            install(Tool::Cursor, &places, &b, false).unwrap(),
            "underline-notes-2"
        );
        assert_eq!(
            install(Tool::Cursor, &places, &a, false).unwrap(),
            "underline-notes"
        );
    }

    /// TC-016-04 / TC-016-07 / TC-016-12: everything else is kept, also
    /// comments in TOML and entries of other workspaces.
    #[test]
    fn uc016_tc04_tc07_tc12_other_settings_kept() {
        let home = Home::new("kept");
        let places = home.places(Os::Linux);
        fs::create_dir_all(home.0.join(".cursor")).unwrap();
        let cursor = r#"{
  "theme": "dark",
  "mcpServers": {
    "zeta": { "command": "z" },
    "underline-old": { "command": "/opt/underline/underline", "args": ["mcp", "/gone"] },
    "alpha": { "command": "a" }
  }
}
"#;
        fs::write(home.0.join(".cursor/mcp.json"), cursor).unwrap();
        let s = server("/home/ada/notes");
        install(Tool::Cursor, &places, &s, false).unwrap();
        let after: Value = serde_json::from_str(&home.read(".cursor/mcp.json")).unwrap();
        let names: Vec<&String> = after["mcpServers"].as_object().unwrap().keys().collect();
        assert_eq!(names, ["zeta", "underline-old", "alpha", "underline-notes"]);
        assert_eq!(after["theme"], "dark");
        remove(Tool::Cursor, &places, &s).unwrap();
        let before: Value = serde_json::from_str(cursor).unwrap();
        let after: Value = serde_json::from_str(&home.read(".cursor/mcp.json")).unwrap();
        assert_eq!(before, after);

        fs::create_dir_all(home.0.join(".codex")).unwrap();
        let codex =
            "# my settings\nmodel = \"o3\"\n\n[mcp_servers.other] # keep me\ncommand = \"x\"\n";
        fs::write(home.0.join(".codex/config.toml"), codex).unwrap();
        install(Tool::Codex, &places, &s, false).unwrap();
        let text = home.read(".codex/config.toml");
        assert!(text.starts_with("# my settings\nmodel = \"o3\""), "{text}");
        assert!(text.contains("# keep me"), "{text}");
        remove(Tool::Codex, &places, &s).unwrap();
        assert_eq!(home.read(".codex/config.toml"), codex);
    }

    /// TC-016-06: the same workspace with another Underline asks first.
    #[test]
    fn uc016_tc06_for_another_place() {
        let home = Home::new("moved");
        let places = home.places(Os::Linux);
        fs::create_dir_all(home.0.join(".gemini")).unwrap();
        let old = server("/home/ada/notes");
        install(Tool::GeminiCli, &places, &old, false).unwrap();
        let moved = Server {
            program: PathBuf::from("/usr/bin/underline"),
            ..old
        };
        assert_eq!(
            state(Tool::GeminiCli, &places, &moved).unwrap(),
            InstallState::ForAnotherPlace("underline-notes".into())
        );
        assert!(matches!(
            install(Tool::GeminiCli, &places, &moved, false),
            Err(InstallError::ForAnotherPlace(_))
        ));
        install(Tool::GeminiCli, &places, &moved, true).unwrap();
        assert_eq!(
            state(Tool::GeminiCli, &places, &moved).unwrap(),
            InstallState::Installed("underline-notes".into())
        );
    }

    /// TC-016-10: a configuration that cannot be read is left alone.
    #[test]
    fn uc016_tc10_unreadable_left_alone() {
        let home = Home::new("broken");
        let places = home.places(Os::Linux);
        fs::create_dir_all(home.0.join(".config/Code/User")).unwrap();
        let jsonc = "{\n  // my servers\n  \"servers\": {}\n}\n";
        fs::write(home.0.join(".config/Code/User/mcp.json"), jsonc).unwrap();
        let s = server("/home/ada/notes");
        assert!(matches!(
            install(Tool::VsCode, &places, &s, false),
            Err(InstallError::Unreadable { .. })
        ));
        assert_eq!(home.read(".config/Code/User/mcp.json"), jsonc);
    }

    /// TC-016-08 / TC-016-09: the snippet, and a tool found by its program.
    #[test]
    fn uc016_tc08_tc09_snippet_and_found() {
        let s = Server {
            program: PathBuf::from("/opt/my apps/underline"),
            workspace: PathBuf::from("/home/ada/notes"),
        };
        let snippet: Value = serde_json::from_str(&s.snippet()).unwrap();
        assert_eq!(snippet["mcpServers"]["underline-notes"]["args"][0], "mcp");
        assert_eq!(
            s.command_line(),
            "\"/opt/my apps/underline\" mcp /home/ada/notes"
        );
        let home = Home::new("found");
        let places = home.places(Os::Linux);
        assert_eq!(
            state(Tool::Codex, &places, &s).unwrap(),
            InstallState::NotFound
        );
        fs::create_dir_all(home.0.join("bin")).unwrap();
        fs::write(home.0.join("bin/codex"), "").unwrap();
        assert_eq!(
            state(Tool::Codex, &places, &s).unwrap(),
            InstallState::NotInstalled
        );
    }
}

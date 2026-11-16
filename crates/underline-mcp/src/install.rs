//! UC-016 Install MCP Server (ADR-017): an entry for a workspace in the
//! user configuration of an agent tool, changed in that tool's own file,
//! leaving everything else in it as it was.

use std::path::{Path, PathBuf};

use std::time::{Duration, Instant, SystemTime};

use serde_json::value::RawValue;
use serde_json::{json, Map, Value};
use underline_core::write_atomic_private;

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
    /// A6, Q-110: the servers of the configuration are kept in something
    /// else than a table of entries; it is left alone, never replaced.
    #[error(
        "in {}, \"{key}\" holds no list of servers, and Underline does not change it",
        .path.display()
    )]
    NotAServerList { path: PathBuf, key: &'static str },
    /// A6, Q-110: the user may not write the configuration (it was made
    /// read-only); it is not replaced.
    #[error("{} is read-only for you, and Underline does not replace it", .path.display())]
    ReadOnly { path: PathBuf },
    /// Q-110: another install or removal of Underline changes the same
    /// configuration and did not finish within 10 seconds.
    #[error("{} is being changed by another Underline; try again", .path.display())]
    Busy { path: PathBuf },
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

/// The members of a JSON object in their order, each value as written in
/// the file, so that what Underline does not change keeps every character
/// (a number beyond 64 bits keeps its digits, Q-110).
type Members = Vec<(String, Box<RawValue>)>;

/// [`Members`] as read from JSON text.
struct Ordered(Members);

impl<'de> serde::Deserialize<'de> for Ordered {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Visit;
        impl<'de> serde::de::Visitor<'de> for Visit {
            type Value = Ordered;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("an object")
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                mut map: A,
            ) -> Result<Ordered, A::Error> {
                let mut members = Vec::new();
                while let Some(member) = map.next_entry::<String, Box<RawValue>>()? {
                    members.push(member);
                }
                Ok(Ordered(members))
            }
        }
        deserializer.deserialize_map(Visit)
    }
}

/// The members of the JSON text `raw` when it is an object, `None` when it
/// is another value.
fn members(raw: &str) -> Result<Option<Members>, serde_json::Error> {
    if !raw.trim_start().starts_with('{') {
        // Checked as a value, so that broken text is still said to be so.
        serde_json::from_str::<serde::de::IgnoredAny>(raw)?;
        return Ok(None);
    }
    serde_json::from_str::<Ordered>(raw).map(|ordered| Some(ordered.0))
}

/// `members` as an object at nesting `depth`, two spaces a level, each
/// value as it was written.
fn render(members: &Members, depth: usize) -> String {
    if members.is_empty() {
        return "{}".into();
    }
    let inner = "  ".repeat(depth + 1);
    let lines: Vec<String> = members
        .iter()
        .map(|(key, value)| {
            let key = serde_json::to_string(key).unwrap_or_default();
            format!("{inner}{key}: {}", value.get())
        })
        .collect();
    format!("{{\n{}\n{}}}", lines.join(",\n"), "  ".repeat(depth))
}

/// `value` pretty, as a member at nesting `depth`.
fn raw_at(value: &Value, depth: usize) -> Box<RawValue> {
    let text = serde_json::to_string_pretty(value).unwrap_or_default();
    let indent = "  ".repeat(depth);
    let text = text.replace('\n', &format!("\n{indent}"));
    RawValue::from_string(text)
        .unwrap_or_else(|_| RawValue::from_string("{}".into()).expect("{} is JSON"))
}

/// Sets the member `key` to `value`, in its place, or at the end.
fn put(members: &mut Members, key: &str, value: Box<RawValue>) {
    match members.iter_mut().find(|(k, _)| k == key) {
        Some((_, old)) => *old = value,
        None => members.push((key.to_owned(), value)),
    }
}

/// The configuration of one tool as read.
enum Document {
    Json(Members),
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
            return Ok(Document::Json(Vec::new()));
        }
        match members(&text) {
            Ok(Some(members)) => Ok(Document::Json(members)),
            Ok(None) => Err(unreadable("it does not hold an object".into())),
            Err(err) => Err(unreadable(format!(
                "{err}; it may hold comments, which Underline does not change"
            ))),
        }
    }

    /// The servers of a JSON configuration: `Some` with the members of the
    /// object under the tool's key, or none when there is no key; `None`
    /// when the key holds something else.
    fn json_servers(root: &Members, tool: Tool) -> Option<Members> {
        match root.iter().find(|(key, _)| key == tool.key()) {
            None => Some(Vec::new()),
            Some((_, raw)) => members(raw.get()).ok().flatten(),
        }
    }

    fn entries(&self, tool: Tool) -> Entries {
        match self {
            Document::Json(root) => Self::json_servers(root, tool)
                .unwrap_or_default()
                .iter()
                .map(|(name, raw)| {
                    let entry: Value = serde_json::from_str(raw.get()).unwrap_or_default();
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
                .collect(),
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

    /// UC-016 step 4: puts the entry `name` for `server` into the servers.
    /// In TOML the entry is a table of its own (`[mcp_servers.name]`), or
    /// an inline table when the servers are one (`mcp_servers = { … }`).
    /// Returns `false`, changing nothing, when the servers are kept in
    /// something else than a table of entries (A6, Q-110).
    fn set(&mut self, tool: Tool, name: &str, server: &Server) -> bool {
        match self {
            Document::Json(root) => {
                let Some(mut servers) = Self::json_servers(root, tool) else {
                    return false;
                };
                let mut entry = Map::new();
                if matches!(tool, Tool::VsCode | Tool::ClaudeCode) {
                    entry.insert("type".into(), json!("stdio"));
                }
                entry.insert("command".into(), json!(server.program.to_string_lossy()));
                entry.insert("args".into(), json!(server.args()));
                put(&mut servers, name, raw_at(&Value::Object(entry), 2));
                Self::put_servers(root, tool, &servers);
                true
            }
            Document::Toml(doc) => {
                let mut entry = toml_edit::InlineTable::new();
                entry.insert(
                    "command",
                    server.program.to_string_lossy().into_owned().into(),
                );
                entry.insert(
                    "args",
                    server.args().iter().collect::<toml_edit::Array>().into(),
                );
                let servers = doc
                    .entry(tool.key())
                    .or_insert(toml_edit::Item::Table(toml_edit::Table::new()));
                if let Some(servers) = servers.as_table_mut() {
                    servers.set_implicit(true);
                    servers.insert(name, toml_edit::Item::Table(entry.into_table()));
                    true
                } else if let Some(servers) = servers.as_inline_table_mut() {
                    // The space before the closing brace moves behind the
                    // new last entry, not before its comma.
                    if let Some((_, last)) = servers.iter_mut().last() {
                        last.decor_mut().set_suffix("");
                    }
                    servers.insert(name, toml_edit::Value::InlineTable(entry));
                    true
                } else {
                    false
                }
            }
        }
    }

    /// Writes `servers` back under the tool's key of `root`.
    fn put_servers(root: &mut Members, tool: Tool, servers: &Members) {
        if let Ok(raw) = RawValue::from_string(render(servers, 1)) {
            put(root, tool.key(), raw);
        }
    }

    fn remove(&mut self, tool: Tool, name: &str) {
        match self {
            Document::Json(root) => {
                let Some(mut servers) = Self::json_servers(root, tool) else {
                    return;
                };
                let before = servers.len();
                servers.retain(|(key, _)| key != name);
                if servers.len() != before {
                    Self::put_servers(root, tool, &servers);
                }
            }
            Document::Toml(doc) => {
                let Some(servers) = doc.get_mut(tool.key()) else {
                    return;
                };
                if let Some(servers) = servers.as_inline_table_mut() {
                    // The space before the closing brace goes to the entry
                    // that is last now, as [`Document::set`] took it.
                    let last = servers.iter().last().map(|(key, _)| key.to_owned());
                    let removed = servers.remove(name);
                    if last.as_deref() == Some(name) {
                        let suffix = removed
                            .as_ref()
                            .and_then(|v| v.decor().suffix())
                            .and_then(|s| s.as_str())
                            .unwrap_or(" ")
                            .to_owned();
                        if let Some((_, now_last)) = servers.iter_mut().last() {
                            now_last.decor_mut().set_suffix(suffix);
                        }
                    }
                } else if let Some(servers) = servers.as_table_like_mut() {
                    servers.remove(name);
                }
            }
        }
    }

    fn write(&self, path: &Path) -> Result<(), InstallError> {
        let text = match self {
            Document::Json(root) => {
                let mut text = render(root, 0);
                text.push('\n');
                text
            }
            Document::Toml(doc) => doc.to_string(),
        };
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|source| InstallError::Unwritable {
                path: path.to_owned(),
                source,
            })?;
        }
        // Q-110: the file may hold tokens: it keeps who may read it, a new
        // one only the user may read, a link to it stays a link, and one
        // the user may not write is not replaced.
        write_atomic_private(path, text.as_bytes()).map_err(|source| {
            if source.kind() == std::io::ErrorKind::PermissionDenied {
                InstallError::ReadOnly {
                    path: path.to_owned(),
                }
            } else {
                InstallError::Unwritable {
                    path: path.to_owned(),
                    source,
                }
            }
        })
    }
}

/// Q-110: while Underline changes a configuration (read, change, write),
/// a lock file beside it keeps another install or removal of Underline,
/// in this process or another, from writing the file as it read it before,
/// which would lose the first one's entry. It is a file made only when
/// none is there; it goes when the change is done. One left by an
/// Underline that ended in between counts as gone after 30 seconds.
/// The agent tools themselves do not know it.
struct ConfigLock(PathBuf);

impl ConfigLock {
    /// How long to wait for another change, and when a lock counts as left.
    const WAIT: Duration = Duration::from_secs(10);
    const LEFT: Duration = Duration::from_secs(30);

    fn take(path: &Path) -> Result<Self, InstallError> {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let hidden = if name.starts_with('.') { "" } else { "." };
        let lock = path.with_file_name(format!("{hidden}{name}.underline-lock"));
        let start = Instant::now();
        loop {
            match std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&lock)
            {
                Ok(_) => return Ok(Self(lock)),
                Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
                    let left = std::fs::metadata(&lock)
                        .and_then(|meta| meta.modified())
                        .ok()
                        .and_then(|at| SystemTime::now().duration_since(at).ok())
                        .is_some_and(|age| age > Self::LEFT);
                    if left {
                        let _ = std::fs::remove_file(&lock);
                    } else if start.elapsed() > Self::WAIT {
                        return Err(InstallError::Busy {
                            path: path.to_owned(),
                        });
                    } else {
                        std::thread::sleep(Duration::from_millis(10));
                    }
                }
                Err(source) => {
                    return Err(InstallError::Unwritable {
                        path: path.to_owned(),
                        source,
                    })
                }
            }
        }
    }
}

impl Drop for ConfigLock {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
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
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| InstallError::Unwritable {
            path: path.clone(),
            source,
        })?;
    }
    let _lock = ConfigLock::take(&path)?;
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
    // Q-110: "Installed" is said only when the entry is in what is written.
    if !document.set(tool, &name, server) {
        return Err(InstallError::NotAServerList {
            path,
            key: tool.key(),
        });
    }
    document.write(&path)?;
    Ok(name)
}

/// UC-016 A3: removes only the workspace's entry from `tool`.
pub fn remove(tool: Tool, places: &Places, server: &Server) -> Result<(), InstallError> {
    let path = tool.config_path(places).ok_or(InstallError::NoPlace)?;
    if std::fs::symlink_metadata(&path).is_err() {
        // No configuration: nothing to remove.
        return Ok(());
    }
    let _lock = ConfigLock::take(&path)?;
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

    /// UC-016 BR-002, Q-110: a configuration only the user may read stays
    /// so (it may hold tokens), and a new one is created so.
    #[cfg(unix)]
    #[test]
    fn uc016_br002_q110_the_file_mode_is_kept() {
        use std::os::unix::fs::PermissionsExt;
        let home = Home::new("mode");
        let places = home.places(Os::Linux);
        let s = server("/home/ada/notes");
        let mode = |p: &Path| fs::metadata(p).unwrap().permissions().mode() & 0o777;
        let claude = home.0.join(".claude.json");
        fs::write(&claude, "{\"oauthAccount\": {\"token\": \"secret\"}}\n").unwrap();
        fs::set_permissions(&claude, fs::Permissions::from_mode(0o600)).unwrap();

        install(Tool::ClaudeCode, &places, &s, false).unwrap();
        assert_eq!(mode(&claude), 0o600);
        remove(Tool::ClaudeCode, &places, &s).unwrap();
        assert_eq!(mode(&claude), 0o600);

        install(Tool::Cursor, &places, &s, false).unwrap();
        assert_eq!(mode(&home.0.join(".cursor/mcp.json")), 0o600, "a new file");
    }

    /// UC-016 BR-002, Q-110: a configuration that is a symbolic link (as
    /// dotfile managers make them) stays a link; the file it leads to gets
    /// the entry.
    #[cfg(unix)]
    #[test]
    fn uc016_br002_q110_a_link_stays_a_link() {
        let home = Home::new("link");
        let places = home.places(Os::Linux);
        let s = server("/home/ada/notes");
        fs::create_dir_all(home.0.join("dotfiles")).unwrap();
        fs::write(
            home.0.join("dotfiles/claude.json"),
            "{\"theme\": \"dark\"}\n",
        )
        .unwrap();
        let link = home.0.join(".claude.json");
        std::os::unix::fs::symlink("dotfiles/claude.json", &link).unwrap();

        install(Tool::ClaudeCode, &places, &s, false).unwrap();

        assert!(fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink());
        let real: Value = serde_json::from_str(&home.read("dotfiles/claude.json")).unwrap();
        assert_eq!(real["theme"], "dark");
        assert_eq!(real["mcpServers"]["underline-notes"]["args"][0], "mcp");
        assert_eq!(
            state(Tool::ClaudeCode, &places, &s).unwrap(),
            InstallState::Installed("underline-notes".into())
        );
    }

    /// UC-016 step 4, A3, Q-110: Codex servers written as an inline table
    /// get the entry in that table, and lose only it again.
    #[test]
    fn uc016_q110_codex_inline_table() {
        let home = Home::new("inline");
        let places = home.places(Os::Linux);
        let s = server("/home/ada/notes");
        fs::create_dir_all(home.0.join(".codex")).unwrap();
        let codex = "model = \"o3\"\nmcp_servers = { other = { command = \"x\" } }\n";
        fs::write(home.0.join(".codex/config.toml"), codex).unwrap();

        assert_eq!(
            install(Tool::Codex, &places, &s, false).unwrap(),
            "underline-notes"
        );
        let text = home.read(".codex/config.toml");
        assert_eq!(
            text,
            "model = \"o3\"\nmcp_servers = { other = { command = \"x\" }, underline-notes = { command = \"/opt/underline/underline\", args = [\"mcp\", \"/home/ada/notes\"] } }\n"
        );
        let doc: toml_edit::DocumentMut = text.parse().unwrap();
        assert_eq!(doc["mcp_servers"]["other"]["command"].as_str(), Some("x"));
        assert_eq!(
            doc["mcp_servers"]["underline-notes"]["args"]
                .as_array()
                .map(|a| a.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>()),
            Some(vec!["mcp", "/home/ada/notes"])
        );
        assert_eq!(
            state(Tool::Codex, &places, &s).unwrap(),
            InstallState::Installed("underline-notes".into())
        );
        remove(Tool::Codex, &places, &s).unwrap();
        assert_eq!(home.read(".codex/config.toml"), codex, "as it was");
        let after: toml_edit::DocumentMut = home.read(".codex/config.toml").parse().unwrap();
        assert!(after["mcp_servers"].get("underline-notes").is_none());
        assert_eq!(after["mcp_servers"]["other"]["command"].as_str(), Some("x"));
        assert_eq!(after["model"].as_str(), Some("o3"));
    }

    /// UC-016 A6, Q-110: servers kept in something else than a table of
    /// entries are refused with the reason; the file is not touched and
    /// "Installed" is never said.
    #[test]
    fn uc016_a6_q110_servers_that_are_no_object_are_refused() {
        let home = Home::new("not-object");
        let places = home.places(Os::Linux);
        let s = server("/home/ada/notes");
        fs::create_dir_all(home.0.join(".cursor")).unwrap();
        fs::create_dir_all(home.0.join(".config/Code/User")).unwrap();
        fs::create_dir_all(home.0.join(".codex")).unwrap();
        let cases = [
            (
                Tool::Cursor,
                ".cursor/mcp.json",
                "{\"mcpServers\": [\"keep\"]}\n",
            ),
            (
                Tool::VsCode,
                ".config/Code/User/mcp.json",
                "{\"servers\": \"x\"}\n",
            ),
            (Tool::Cursor, ".cursor/mcp.json", "{\"mcpServers\": null}\n"),
            (
                Tool::Codex,
                ".codex/config.toml",
                "mcp_servers = \"none\"\n",
            ),
            (Tool::Codex, ".codex/config.toml", "mcp_servers = [1, 2]\n"),
        ];
        for (tool, file, text) in cases {
            fs::write(home.0.join(file), text).unwrap();
            let err = install(tool, &places, &s, false).unwrap_err();
            assert!(
                matches!(err, InstallError::NotAServerList { .. }),
                "{tool:?} {text}: {err:?}"
            );
            assert!(
                err.to_string().contains("holds no list of servers")
                    && err.to_string().contains(file),
                "{err}"
            );
            assert_eq!(home.read(file), text, "{tool:?} unchanged");
            assert_eq!(
                state(tool, &places, &s).unwrap(),
                InstallState::NotInstalled
            );
            remove(tool, &places, &s).unwrap();
            assert_eq!(home.read(file), text, "{tool:?} unchanged by remove");
        }
    }
    /// UC-016 BR-002, Q-110 (monkey finding): a configuration the user made
    /// read-only (`0444`) is not replaced: the install fails cleanly, the
    /// bytes and the mode stay. The rename over it made it succeed.
    #[cfg(unix)]
    #[test]
    fn uc016_br002_q110_monkey_a_read_only_file_is_not_replaced() {
        use std::os::unix::fs::PermissionsExt;
        let home = Home::new("read-only");
        let places = home.places(Os::Linux);
        let s = server("/home/ada/notes");
        let path = home.0.join(".claude.json");
        fs::write(&path, "{\"a\": 1}").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o444)).unwrap();

        let result = install(Tool::ClaudeCode, &places, &s, false);

        assert!(result.is_err(), "installed into a read-only file");
        assert_eq!(home.read(".claude.json"), "{\"a\": 1}");
    }

    /// UC-016 step 4, Q-110 (monkey finding): installs of two workspaces
    /// at the same time (read, change, write) lose entries: each one
    /// writes the file as it read it. All eight must be there.
    #[test]
    fn uc016_q110_monkey_concurrent_installs_lose_no_entry() {
        let home = Home::new("concurrent");
        let threads: Vec<_> = (0..8)
            .map(|i| {
                let places = home.places(Os::Linux);
                std::thread::spawn(move || {
                    install(
                        Tool::ClaudeCode,
                        &places,
                        &server(&format!("/w/n{i}")),
                        false,
                    )
                })
            })
            .collect();
        for thread in threads {
            let _ = thread.join().unwrap();
        }
        let places = home.places(Os::Linux);
        let missing: Vec<_> = (0..8)
            .filter(|i| {
                !matches!(
                    state(Tool::ClaudeCode, &places, &server(&format!("/w/n{i}"))),
                    Ok(InstallState::Installed(_))
                )
            })
            .collect();
        assert!(missing.is_empty(), "entries lost: {missing:?}");
    }

    /// UC-016 BR-002, Q-110 (monkey finding): everything else in the file
    /// stays as it was: an integer beyond 64 bits (an id) is rewritten as
    /// a float, `18446744073709551617` becoming `1.8446744073709552e+19`.
    #[test]
    fn uc016_br002_q110_monkey_other_values_keep_their_digits() {
        let home = Home::new("digits");
        let places = home.places(Os::Linux);
        let s = server("/home/ada/notes");
        fs::write(
            home.0.join(".claude.json"),
            "{\"id\": 18446744073709551617}\n",
        )
        .unwrap();

        install(Tool::ClaudeCode, &places, &s, false).unwrap();

        assert!(
            home.read(".claude.json").contains("18446744073709551617"),
            "{}",
            home.read(".claude.json")
        );
    }

    /// UC-016 A6, Q-110: a read-only configuration is refused with a
    /// reason that says so; no lock file is left behind.
    #[cfg(unix)]
    #[test]
    fn uc016_a6_q110_read_only_is_said() {
        use std::os::unix::fs::PermissionsExt;
        let home = Home::new("read-only-said");
        let places = home.places(Os::Linux);
        let path = home.0.join(".claude.json");
        fs::write(&path, "{}").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o444)).unwrap();
        if fs::OpenOptions::new().write(true).open(&path).is_ok() {
            return;
        }

        let err =
            install(Tool::ClaudeCode, &places, &server("/home/ada/notes"), false).unwrap_err();

        assert!(matches!(err, InstallError::ReadOnly { .. }), "{err:?}");
        assert!(err
            .to_string()
            .ends_with("is read-only for you, and Underline does not replace it"));
        assert!(!home.0.join(".claude.json.underline-lock").exists());
        assert_eq!(
            fs::read_dir(&home.0).unwrap().count(),
            1,
            "only the configuration"
        );
    }

    /// UC-016 BR-002, Q-110: what Underline does not change keeps every
    /// character, its layout too.
    #[test]
    fn uc016_br002_q110_other_members_keep_their_text() {
        let home = Home::new("verbatim");
        let places = home.places(Os::Linux);
        let s = server("/home/ada/notes");
        fs::create_dir_all(home.0.join(".cursor")).unwrap();
        let before = "{\n  \"big\": 1.000000000000000000001,\n  \"list\": [1,2,  3],\n  \"mcpServers\": {\n    \"z\": {\"command\":\"z\"}\n  }\n}\n";
        fs::write(home.0.join(".cursor/mcp.json"), before).unwrap();

        install(Tool::Cursor, &places, &s, false).unwrap();
        let text = home.read(".cursor/mcp.json");
        assert!(text.contains("\"big\": 1.000000000000000000001,"), "{text}");
        assert!(text.contains("\"list\": [1,2,  3],"), "{text}");
        assert!(text.contains("\"z\": {\"command\":\"z\"},"), "{text}");
        serde_json::from_str::<Value>(&text).unwrap();
        remove(Tool::Cursor, &places, &s).unwrap();
        assert_eq!(home.read(".cursor/mcp.json"), before);
    }
}

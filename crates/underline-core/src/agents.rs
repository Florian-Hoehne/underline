//! UC-017: agent access as stored, and the agents connected to a workspace.
//!
//! The MCP server reads the stored level on every request about notes, so a
//! change takes effect without restarting the agent tool (BR-003), also while
//! the Underline window is closed (BR-007). While it runs, it keeps a small
//! presence file in the workspace's Underline folder (ADR-008), which the
//! window reads to name the connected agents (BR-004).

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use crate::atomic::write_atomic;
use crate::note::NoteId;
use crate::settings::{user_settings_path, AgentAccess, Effective, Settings};
use crate::state::{underline_folder, UNDERLINE_FOLDER};

/// The folder of presence files inside the Underline folder.
pub const AGENTS_FOLDER: &str = "agents";

/// How often a running MCP server renews its presence file.
pub const PRESENCE_RENEW: Duration = Duration::from_secs(1);

/// BR-004: a presence file not renewed for this long no longer counts, so a
/// server that ended without saying so stops counting within five seconds,
/// also when the window looks only once a second.
pub const PRESENCE_STALE: Duration = Duration::from_secs(3);

/// The name of an agent whose tool gives none (BR-004).
pub const UNNAMED_AGENT: &str = "an agent";

/// The agent access in effect for the workspace at `root`, as stored in the
/// user settings at `user_settings` and the workspace settings (UC-017
/// BR-002). A settings file that cannot be read gives off, with the reason,
/// so nothing leaves the computer by mistake (NFR-015).
pub fn stored_agent_access(
    root: &Path,
    user_settings: Option<&Path>,
) -> Result<AgentAccess, String> {
    let user = match user_settings {
        Some(path) => Settings::load_file(path).map_err(|e| e.to_string())?,
        None => Settings::default(),
    };
    let workspace = Settings::load(root).map_err(|e| e.to_string())?;
    Ok(Effective::of(&user, Some(&workspace)).agent_access)
}

/// [`stored_agent_access`] with the user settings of this computer.
pub fn agent_access_now(root: &Path) -> Result<AgentAccess, String> {
    stored_agent_access(root, user_settings_path().as_deref())
}

/// What a running MCP server tells the window about its agent.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Presence {
    /// The name the agent tool gave when it connected, if any.
    pub name: Option<String>,
    /// UC-017 A2: a request about notes was refused because access is off.
    pub refused: bool,
    /// UC-018 BR-006: where the agent read last.
    pub reading: Option<Reading>,
    /// UC-020 A8: what the agent did last with a todo, such as "created
    /// todo #3", and when (milliseconds since the Unix epoch).
    pub todo_change: Option<(u64, String)>,
}

/// UC-018 BR-006: how long the agent cursor marks the lines an agent read.
pub const READING_SHOWN: Duration = Duration::from_secs(10);

/// UC-018 BR-006: the lines of a note an agent read, and when.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reading {
    pub note: NoteId,
    /// First and last line read, counted from 1.
    pub lines: (usize, usize),
    /// Milliseconds since the Unix epoch.
    pub at: u64,
    /// UC-019 BR-006: the agent wrote these lines rather than read them.
    pub wrote: bool,
}

impl Reading {
    /// The lines `lines` of `note`, read now.
    pub fn now(note: NoteId, lines: (usize, usize)) -> Self {
        let at = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as u64);
        Reading {
            note,
            lines,
            at,
            wrote: false,
        }
    }

    /// UC-019 BR-006: the same, for lines the agent wrote.
    pub fn written(self) -> Self {
        Reading {
            wrote: true,
            ..self
        }
    }

    /// Whether the agent cursor still marks it at `now` (BR-006).
    pub fn shown_at(&self, now: SystemTime) -> bool {
        let at = SystemTime::UNIX_EPOCH + Duration::from_millis(self.at);
        now.duration_since(at)
            .map_or(true, |age| age <= READING_SHOWN)
    }
}

impl Presence {
    /// The name shown in the status bar (BR-004).
    pub fn shown_name(&self) -> &str {
        self.name.as_deref().unwrap_or(UNNAMED_AGENT)
    }

    fn to_text(&self) -> String {
        let name = self
            .name
            .as_deref()
            .unwrap_or("")
            .replace(['\n', '\r'], " ");
        let mut text = format!("name: {name}\nrefused: {}\n", self.refused);
        if let Some((at, change)) = &self.todo_change {
            text.push_str(&format!(
                "todo: {at} {}\n",
                change.replace(['\n', '\r'], " ")
            ));
        }
        if let Some(r) = &self.reading {
            text.push_str(&format!(
                "{}: {} {} {} {}\n",
                if r.wrote { "wrote" } else { "reading" },
                r.lines.0,
                r.lines.1,
                r.at,
                r.note.as_str()
            ));
        }
        text
    }

    fn from_text(text: &str) -> Self {
        let mut presence = Presence::default();
        for line in text.lines() {
            if let Some(name) = line.strip_prefix("name: ") {
                let name = name.trim();
                presence.name = (!name.is_empty()).then(|| name.to_owned());
            } else if let Some(change) = line.strip_prefix("todo: ") {
                if let Some((at, text)) = change.split_once(' ') {
                    if let Ok(at) = at.parse() {
                        presence.todo_change = Some((at, text.to_owned()));
                    }
                }
            } else if let Some(refused) = line.strip_prefix("refused: ") {
                presence.refused = refused.trim() == "true";
            } else if let Some((reading, wrote)) = line
                .strip_prefix("reading: ")
                .map(|r| (r, false))
                .or_else(|| line.strip_prefix("wrote: ").map(|r| (r, true)))
            {
                let mut parts = reading.splitn(4, ' ');
                let mut number = || parts.next().and_then(|p| p.parse::<u64>().ok());
                if let (Some(from), Some(to), Some(at)) = (number(), number(), number()) {
                    if let Some(note) = parts.next().filter(|n| !n.is_empty()) {
                        presence.reading = Some(Reading {
                            note: NoteId::from_relative(note),
                            lines: (from as usize, to as usize),
                            at,
                            wrote,
                        });
                    }
                }
            }
        }
        presence
    }
}

/// The presence file of the MCP server process `id` in the workspace `root`.
pub fn presence_path(root: &Path, id: u32) -> PathBuf {
    root.join(UNDERLINE_FOLDER)
        .join(AGENTS_FOLDER)
        .join(id.to_string())
}

/// Writes (or renews) the presence file of the server process `id`.
pub fn write_presence(root: &Path, id: u32, presence: &Presence) -> io::Result<()> {
    let folder = underline_folder(root)?.join(AGENTS_FOLDER);
    fs::create_dir_all(&folder)?;
    write_atomic(&folder.join(id.to_string()), presence.to_text().as_bytes())
}

/// Removes the presence file of the server process `id`, when it ends.
pub fn remove_presence(root: &Path, id: u32) {
    let _ = fs::remove_file(presence_path(root, id));
}

/// BR-004: the agents connected to the workspace `root` at `now`, in the
/// order they connected. Presence files not renewed within
/// [`PRESENCE_STALE`] do not count; those older than a minute are removed.
pub fn connected_agents(root: &Path, now: SystemTime) -> Vec<Presence> {
    let folder = root.join(UNDERLINE_FOLDER).join(AGENTS_FOLDER);
    let Ok(entries) = fs::read_dir(&folder) else {
        return Vec::new();
    };
    let mut found = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let is_id = path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.parse::<u32>().is_ok());
        if !is_id {
            continue;
        }
        let Ok(meta) = entry.metadata() else { continue };
        let Ok(renewed) = meta.modified() else {
            continue;
        };
        let age = now.duration_since(renewed).unwrap_or_default();
        if age > Duration::from_secs(60) {
            let _ = fs::remove_file(&path);
        }
        if age > PRESENCE_STALE {
            continue;
        }
        if let Ok(text) = fs::read_to_string(&path) {
            let created = meta.created().unwrap_or(renewed);
            found.push((created, Presence::from_text(&text)));
        }
    }
    found.sort_by_key(|(created, _)| *created);
    found.into_iter().map(|(_, p)| p).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::{store_setting, SettingKey};

    fn temp(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("underline-agents-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// TC-017-04, TC-017-09: the level is read as stored, each time.
    #[test]
    fn uc017_tc09_stored_level_followed() {
        let dir = temp("stored");
        let user = dir.join("user.yaml");
        let ws = dir.join("notes");
        fs::create_dir_all(&ws).unwrap();
        assert_eq!(stored_agent_access(&ws, Some(&user)), Ok(AgentAccess::Off));
        store_setting(&user, SettingKey::AgentAccess, Some("Read and write")).unwrap();
        assert_eq!(
            stored_agent_access(&ws, Some(&user)),
            Ok(AgentAccess::ReadWrite)
        );
        // A1: the workspace lowers it.
        fs::write(ws.join("underline.yaml"), "agent_access: read only\n").unwrap();
        assert_eq!(
            stored_agent_access(&ws, Some(&user)),
            Ok(AgentAccess::ReadOnly)
        );
        store_setting(&user, SettingKey::AgentAccess, Some("Off")).unwrap();
        assert_eq!(stored_agent_access(&ws, Some(&user)), Ok(AgentAccess::Off));
        // A broken file refuses access, with the reason.
        fs::write(&user, "agent_access: [\n").unwrap();
        assert!(stored_agent_access(&ws, Some(&user)).is_err());
        fs::remove_dir_all(&dir).unwrap();
    }

    /// TC-017-07, TC-017-08: presence files name the connected agents; a
    /// removed or stale one does not count.
    #[test]
    fn uc017_tc07_tc08_connected_agents() {
        let ws = temp("presence");
        let now = SystemTime::now;
        assert!(connected_agents(&ws, now()).is_empty());
        let claude = Presence {
            name: Some("Claude Code".into()),
            ..Presence::default()
        };
        write_presence(&ws, 11, &claude).unwrap();
        write_presence(&ws, 12, &Presence::default()).unwrap();
        let agents = connected_agents(&ws, now());
        let names: Vec<&str> = agents.iter().map(Presence::shown_name).collect();
        assert_eq!(names.len(), 2);
        assert!(names.contains(&"Claude Code") && names.contains(&UNNAMED_AGENT));
        assert!(ws.join(".underline/.gitignore").exists());

        remove_presence(&ws, 12);
        assert_eq!(connected_agents(&ws, now()), vec![claude]);
        // A process killed without removing its file stops counting.
        let later = now() + PRESENCE_STALE + Duration::from_secs(1);
        assert!(connected_agents(&ws, later).is_empty());
        let much_later = now() + Duration::from_secs(120);
        connected_agents(&ws, much_later);
        assert!(!presence_path(&ws, 11).exists());
        fs::remove_dir_all(&ws).unwrap();
    }

    #[test]
    fn presence_round_trip() {
        let presence = Presence {
            name: Some("Cursor".into()),
            refused: true,
            reading: Some(Reading::now(NoteId::from_relative("Work/My Plan"), (3, 9)).written()),
            todo_change: Some((17, "created todo #3".into())),
        };
        let reading = presence.reading.clone().unwrap();
        assert!(reading.shown_at(SystemTime::now()));
        assert!(!reading.shown_at(SystemTime::now() + READING_SHOWN + Duration::from_secs(1)));
        assert_eq!(Presence::from_text(&presence.to_text()), presence);
        assert_eq!(Presence::from_text(""), Presence::default());
    }
}

//! The settings files `underline.yaml` (C-006, ADR-004, UC-010): the user
//! settings file in the home directory and the workspace settings file in a
//! workspace, read into [`Settings`] with every setting optional, combined
//! with the built-in defaults into [`Effective`] settings, and changed one
//! line at a time, so the user's comments and order stay (UC-010 BR-005).

use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use serde::de::{self, Deserializer};
use serde::{Deserialize, Serialize, Serializer};

use crate::atomic::write_atomic;
use crate::format::NoteFormat;

/// File name of the user settings file and of every workspace settings file.
pub const SETTINGS_FILE_NAME: &str = "underline.yaml";

#[derive(Debug, thiserror::Error)]
pub enum SettingsError {
    #[error("cannot read {path}: {source}")]
    Read { path: PathBuf, source: io::Error },
    #[error("cannot write {path}: {source}")]
    Write { path: PathBuf, source: io::Error },
    /// UC-010 A6: the file cannot be understood or holds a value that is not
    /// allowed; `line` counts from 1.
    #[error("{}{}: {message}", .path.display(), .line.map(|l| format!(", line {l}")).unwrap_or_default())]
    Invalid {
        path: PathBuf,
        line: Option<usize>,
        message: String,
    },
}

/// Defines a setting's values: their names in the file and the interface,
/// read without regard to case, spaces, and dashes.
macro_rules! choice {
    ($(#[$meta:meta])* $name:ident { $($variant:ident => $label:expr),+ $(,)? }) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
        pub enum $name {
            #[default]
            $($variant),+
        }

        impl $name {
            pub const ALL: &'static [$name] = &[$($name::$variant),+];

            /// The name shown and written to the file.
            pub fn label(self) -> &'static str {
                match self {
                    $($name::$variant => $label),+
                }
            }
        }

        impl FromStr for $name {
            type Err = String;
            fn from_str(text: &str) -> Result<Self, String> {
                let wanted = simplify(text);
                $name::ALL
                    .iter()
                    .copied()
                    .find(|value| simplify(value.label()) == wanted)
                    .ok_or_else(|| {
                        let names: Vec<&str> = $name::ALL.iter().map(|v| v.label()).collect();
                        format!("{text:?} is none of {}", names.join(", "))
                    })
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(self.label())
            }
        }

        impl Serialize for $name {
            fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.serialize_str(self.label())
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let text = String::deserialize(deserializer)?;
                text.parse().map_err(de::Error::custom)
            }
        }
    };
}

fn simplify(text: &str) -> String {
    text.chars()
        .filter(|c| !matches!(c, ' ' | '-' | '_'))
        .flat_map(char::to_lowercase)
        .collect()
}

choice! {
    /// The themes of Underline (FR-022); light first, the default.
    Theme {
        Light => "Light",
        Dark => "Dark",
        GitHubLight => "GitHub Light",
        GitHubDark => "GitHub Dark",
        SolarizedLight => "Solarized Light",
        SolarizedDark => "Solarized Dark",
        NightOwl => "Night Owl",
        Ubuntu => "Ubuntu",
        Nord => "Nord",
        Dracula => "Dracula",
        Gruvbox => "Gruvbox",
    }
}

choice! {
    /// The set of keys the editor follows (FR-024, FR-025).
    InputMode {
        Normal => "Normal",
        Vim => "Vim",
        Nano => "Nano",
    }
}

choice! {
    /// The side of the source view the line numbers are on (FR-029).
    LineSide {
        Left => "Left",
        Right => "Right",
    }
}

choice! {
    /// What agents may do with the notes through the MCP server (UC-017
    /// BR-002), lowest first; off by default.
    AgentAccess {
        Off => "Off",
        ReadOnly => "Read only",
        ReadWrite => "Read and write",
    }
}

impl AgentAccess {
    /// Whether agents may list and read notes.
    pub fn reads(self) -> bool {
        self >= AgentAccess::ReadOnly
    }

    /// Whether agents may create and change notes.
    pub fn writes(self) -> bool {
        self == AgentAccess::ReadWrite
    }

    /// How the status bar names the level, as in "Agents: read only".
    pub fn status_text(self) -> &'static str {
        match self {
            AgentAccess::Off => "off",
            AgentAccess::ReadOnly => "read only",
            AgentAccess::ReadWrite => "read and write",
        }
    }
}

/// The key under which earlier versions kept the MCP server switch; `true`
/// is read as read and write, `false` as off (UC-017 BR-002).
pub const LEGACY_MCP_KEY: &str = "mcp_server_enabled";

/// One setting of UC-010 BR-001, as named in the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SettingKey {
    DefaultFormat,
    Theme,
    InputMode,
    ShowLineNumbers,
    LineNumberSide,
    ShowMinimap,
    CopyOnSelect,
    GitSyncInterval,
    AgentAccess,
    OverviewWidth,
    ToolPanelWidth,
}

impl SettingKey {
    pub const ALL: [SettingKey; 11] = [
        SettingKey::DefaultFormat,
        SettingKey::Theme,
        SettingKey::InputMode,
        SettingKey::ShowLineNumbers,
        SettingKey::LineNumberSide,
        SettingKey::ShowMinimap,
        SettingKey::CopyOnSelect,
        SettingKey::GitSyncInterval,
        SettingKey::AgentAccess,
        SettingKey::OverviewWidth,
        SettingKey::ToolPanelWidth,
    ];

    /// The key in the settings file.
    pub fn key(self) -> &'static str {
        match self {
            SettingKey::DefaultFormat => "default_format",
            SettingKey::Theme => "theme",
            SettingKey::InputMode => "input_mode",
            SettingKey::ShowLineNumbers => "show_line_numbers",
            SettingKey::LineNumberSide => "line_number_side",
            SettingKey::ShowMinimap => "show_minimap",
            SettingKey::CopyOnSelect => "copy_on_select",
            SettingKey::GitSyncInterval => "git_sync_interval",
            SettingKey::AgentAccess => "agent_access",
            SettingKey::OverviewWidth => "overview_width",
            SettingKey::ToolPanelWidth => "tool_panel_width",
        }
    }
}

/// The settings of one settings file. Every setting is optional: an unset
/// one comes from the user settings or the built-in default. Keys Underline
/// does not know are ignored when reading and kept when writing.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Note format given to new notes of the workspace (UC-001, BR-005);
    /// only in workspace settings.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_format: Option<NoteFormat>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub theme: Option<Theme>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_mode: Option<InputMode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub show_line_numbers: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line_number_side: Option<LineSide>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub show_minimap: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub copy_on_select: Option<bool>,
    /// Minutes between two git syncs, 1 to 60.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub git_sync_interval: Option<u32>,
    /// UC-017: off, read only, or read and write.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_access: Option<AgentAccess>,
    /// The switch of earlier versions, only read: folded into
    /// `agent_access` when the file is read.
    #[serde(skip_serializing, rename = "mcp_server_enabled")]
    pub legacy_mcp_server_enabled: Option<bool>,
    /// Widths of the note overview and the tool panel, 100 to 1,000 pixels.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub overview_width: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_panel_width: Option<u32>,
}

/// The settings of a workspace settings file (UC-001, UC-002).
pub type WorkspaceSettings = Settings;

impl Settings {
    /// Path of the workspace settings file of the workspace at `root`.
    pub fn path(root: &Path) -> PathBuf {
        root.join(SETTINGS_FILE_NAME)
    }

    /// Reads the workspace settings file of `root`; a missing file gives
    /// empty settings.
    pub fn load(root: &Path) -> Result<Self, SettingsError> {
        Self::load_file(&Self::path(root))
    }

    /// Reads the settings file at `path`; a missing or empty file gives
    /// empty settings. UC-010 A6: an error names its line.
    pub fn load_file(path: &Path) -> Result<Self, SettingsError> {
        let text = match fs::read_to_string(path) {
            Ok(text) => text,
            Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(source) => {
                return Err(SettingsError::Read {
                    path: path.to_owned(),
                    source,
                })
            }
        };
        Self::parse(&text, path)
    }

    /// Reads settings from `text`, the content of the file at `path`.
    pub fn parse(text: &str, path: &Path) -> Result<Self, SettingsError> {
        if text.trim().is_empty() {
            return Ok(Self::default());
        }
        let settings: Settings = serde_norway::from_str(text).map_err(|err| {
            let message = err.to_string();
            let line = err
                .location()
                .map(|l| l.line())
                .or_else(|| line_of_message(&message));
            SettingsError::Invalid {
                path: path.to_owned(),
                line,
                message: message
                    .split(" at line ")
                    .next()
                    .unwrap_or(&message)
                    .to_owned(),
            }
        })?;
        let mut settings = settings;
        if settings.agent_access.is_none() {
            settings.agent_access = settings.legacy_mcp_server_enabled.map(|on| {
                if on {
                    AgentAccess::ReadWrite
                } else {
                    AgentAccess::Off
                }
            });
        }
        let ranges = [
            (
                SettingKey::GitSyncInterval,
                settings.git_sync_interval,
                1,
                60,
            ),
            (
                SettingKey::OverviewWidth,
                settings.overview_width,
                100,
                1000,
            ),
            (
                SettingKey::ToolPanelWidth,
                settings.tool_panel_width,
                100,
                1000,
            ),
        ];
        for (key, value, min, max) in ranges {
            if let Some(value) = value.filter(|v| !(min..=max).contains(v)) {
                return Err(SettingsError::Invalid {
                    path: path.to_owned(),
                    line: key_line(text, key.key()).map(|l| l + 1),
                    message: format!("{} is {value}, but must be from {min} to {max}", key.key()),
                });
            }
        }
        Ok(settings)
    }

    /// The value of `key` as written to the file, if this file sets it.
    pub fn value_text(&self, key: SettingKey) -> Option<String> {
        let bool_text = |b: bool| if b { "true" } else { "false" }.to_owned();
        match key {
            SettingKey::DefaultFormat => self.default_format.map(|f| match f {
                NoteFormat::Markdown => "markdown".to_owned(),
                NoteFormat::AsciiDoc => "asciidoc".to_owned(),
            }),
            SettingKey::Theme => self.theme.map(|t| t.label().to_owned()),
            SettingKey::InputMode => self.input_mode.map(|m| m.label().to_owned()),
            SettingKey::ShowLineNumbers => self.show_line_numbers.map(bool_text),
            SettingKey::LineNumberSide => self.line_number_side.map(|s| s.label().to_owned()),
            SettingKey::ShowMinimap => self.show_minimap.map(bool_text),
            SettingKey::CopyOnSelect => self.copy_on_select.map(bool_text),
            SettingKey::GitSyncInterval => self.git_sync_interval.map(|v| v.to_string()),
            SettingKey::AgentAccess => self.agent_access.map(|a| a.label().to_owned()),
            SettingKey::OverviewWidth => self.overview_width.map(|v| v.to_string()),
            SettingKey::ToolPanelWidth => self.tool_panel_width.map(|v| v.to_string()),
        }
    }

    /// Sets `key` to `value` as written in a file, or unsets it for `None`;
    /// a value that is not allowed is refused with the reason.
    pub fn set(&mut self, key: SettingKey, value: Option<&str>) -> Result<(), String> {
        let text = set_key("", key.key(), value);
        let parsed =
            Settings::parse(&text, Path::new(SETTINGS_FILE_NAME)).map_err(|e| e.to_string())?;
        match key {
            SettingKey::DefaultFormat => self.default_format = parsed.default_format,
            SettingKey::Theme => self.theme = parsed.theme,
            SettingKey::InputMode => self.input_mode = parsed.input_mode,
            SettingKey::ShowLineNumbers => self.show_line_numbers = parsed.show_line_numbers,
            SettingKey::LineNumberSide => self.line_number_side = parsed.line_number_side,
            SettingKey::ShowMinimap => self.show_minimap = parsed.show_minimap,
            SettingKey::CopyOnSelect => self.copy_on_select = parsed.copy_on_select,
            SettingKey::GitSyncInterval => self.git_sync_interval = parsed.git_sync_interval,
            SettingKey::AgentAccess => {
                self.agent_access = parsed.agent_access;
                self.legacy_mcp_server_enabled = None;
            }
            SettingKey::OverviewWidth => self.overview_width = parsed.overview_width,
            SettingKey::ToolPanelWidth => self.tool_panel_width = parsed.tool_panel_width,
        }
        Ok(())
    }

    /// Whether this file sets `key`.
    pub fn sets(&self, key: SettingKey) -> bool {
        self.value_text(key).is_some()
    }

    /// Writes a new workspace settings file for `root`, atomically (ADR-005).
    ///
    /// Only for a workspace that has no settings file yet (UC-001): changing an
    /// existing file edits single keys so the user's comments stay (ADR-004).
    pub fn write_new(&self, root: &Path) -> Result<PathBuf, SettingsError> {
        let path = Self::path(root);
        let text = serde_norway::to_string(self).map_err(|err| SettingsError::Invalid {
            path: path.clone(),
            line: None,
            message: err.to_string(),
        })?;
        write_atomic(&path, text.as_bytes()).map_err(|source| SettingsError::Write {
            path: path.clone(),
            source,
        })?;
        Ok(path)
    }
}

/// The line, counted from 1, that a parser message names.
fn line_of_message(message: &str) -> Option<usize> {
    let rest = message.split("line ").nth(1)?;
    rest.split(|c: char| !c.is_ascii_digit())
        .next()?
        .parse()
        .ok()
}

/// The index of the line that sets `key` at the top level of `text`.
fn key_line(text: &str, key: &str) -> Option<usize> {
    text.lines().position(|line| {
        line.strip_prefix(key)
            .is_some_and(|rest| rest.trim_start().starts_with(':'))
    })
}

/// UC-010 BR-005: `text` with `key` set to `value`, or removed for `None`:
/// only that line changes, or one line is added at the end; comments, order,
/// and unknown keys stay. A comment after the old value stays too.
pub fn set_key(text: &str, key: &str, value: Option<&str>) -> String {
    let value = value.map(yaml_scalar);
    let mut lines: Vec<String> = text.lines().map(str::to_owned).collect();
    match (key_line(text, key), value) {
        (Some(index), Some(value)) => {
            let old = &lines[index];
            // The comment after the value, with the spaces before it.
            let comment = old
                .find(" #")
                .map(|at| old[old[..at + 1].trim_end().len()..].to_owned())
                .unwrap_or_default();
            lines[index] = format!("{key}: {value}{comment}");
        }
        (Some(index), None) => {
            lines.remove(index);
        }
        (None, Some(value)) => lines.push(format!("{key}: {value}")),
        (None, None) => {}
    }
    let mut out = lines.join("\n");
    if !out.is_empty() {
        out.push('\n');
    }
    out
}

/// A value as a YAML scalar, quoted only when it needs to be.
fn yaml_scalar(value: &str) -> String {
    let needs_quotes = value.is_empty()
        || value.contains([':', '#', '"', '\''])
        || value.starts_with(['-', '[', '{', '!', '&', '*', '|', '>', '%', '@', '`', ' '])
        || value.ends_with(' ');
    if needs_quotes {
        format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
    } else {
        value.to_owned()
    }
}

/// UC-010 BR-003, BR-005: stores `value` for `key` in the settings file at
/// `path` (`None` removes it), within one atomic write. A file that cannot
/// be read is not written, so the user's text is not lost (A6).
pub fn store_setting(
    path: &Path,
    key: SettingKey,
    value: Option<&str>,
) -> Result<(), SettingsError> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) if err.kind() == io::ErrorKind::NotFound => String::new(),
        Err(source) => {
            return Err(SettingsError::Read {
                path: path.to_owned(),
                source,
            })
        }
    };
    Settings::parse(&text, path)?;
    if value.is_none() && key_line(&text, key.key()).is_none() {
        return Ok(());
    }
    let mut changed = set_key(&text, key.key(), value);
    if key == SettingKey::AgentAccess && key_line(&changed, LEGACY_MCP_KEY).is_some() {
        // UC-017 BR-002: the new key replaces the switch of earlier versions.
        changed = set_key(&changed, LEGACY_MCP_KEY, None);
    }
    if changed == text {
        return Ok(());
    }
    write_atomic(path, changed.as_bytes()).map_err(|source| SettingsError::Write {
        path: path.to_owned(),
        source,
    })
}

/// The settings in effect: the built-in defaults, overlaid by the user
/// settings, overlaid by the workspace settings (UC-010 BR-002).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Effective {
    pub theme: Theme,
    pub input_mode: InputMode,
    pub show_line_numbers: bool,
    pub line_number_side: LineSide,
    pub show_minimap: bool,
    pub copy_on_select: bool,
    pub git_sync_interval: u32,
    /// UC-017 BR-002: the user's level, lowered by the workspace.
    pub agent_access: AgentAccess,
    pub overview_width: u32,
    pub tool_panel_width: u32,
    /// UC-017 A1: the workspace lowers agent access below the user's level.
    pub agent_access_lowered: bool,
    /// UC-010 BR-008: the workspace settings ask for more agent access than
    /// the user settings allow, which is not followed.
    pub agent_access_refused: bool,
}

impl Default for Effective {
    fn default() -> Self {
        Effective::of(&Settings::default(), None)
    }
}

impl Effective {
    /// UC-010 BR-001, BR-002, BR-008: the settings in effect for `user` and,
    /// with a workspace open, `workspace`.
    pub fn of(user: &Settings, workspace: Option<&Settings>) -> Self {
        let empty = Settings::default();
        let ws = workspace.unwrap_or(&empty);
        let user_access = user.agent_access.unwrap_or_default();
        Effective {
            theme: ws.theme.or(user.theme).unwrap_or_default(),
            input_mode: ws.input_mode.or(user.input_mode).unwrap_or_default(),
            show_line_numbers: ws
                .show_line_numbers
                .or(user.show_line_numbers)
                .unwrap_or(true),
            line_number_side: ws
                .line_number_side
                .or(user.line_number_side)
                .unwrap_or_default(),
            show_minimap: ws.show_minimap.or(user.show_minimap).unwrap_or(true),
            copy_on_select: ws.copy_on_select.or(user.copy_on_select).unwrap_or(true),
            git_sync_interval: ws.git_sync_interval.or(user.git_sync_interval).unwrap_or(5),
            // A workspace can only lower agent access.
            agent_access: ws.agent_access.map_or(user_access, |w| w.min(user_access)),
            overview_width: ws.overview_width.or(user.overview_width).unwrap_or(200),
            tool_panel_width: ws.tool_panel_width.or(user.tool_panel_width).unwrap_or(240),
            agent_access_lowered: ws.agent_access.is_some_and(|w| w < user_access),
            agent_access_refused: ws.agent_access.is_some_and(|w| w > user_access),
        }
    }
}

/// The directory that holds the user settings file: the user's home
/// directory (C-006).
pub fn user_settings_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .filter(|home| !home.is_empty())
        .map(PathBuf::from)
}

/// The user settings file, `underline.yaml` in the home directory (C-006).
pub fn user_settings_path() -> Option<PathBuf> {
    user_settings_dir().map(|dir| dir.join(SETTINGS_FILE_NAME))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("underline-settings-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn default_format_round_trips() {
        let dir = temp_dir("roundtrip");
        let settings = WorkspaceSettings {
            default_format: Some(NoteFormat::AsciiDoc),
            ..Settings::default()
        };
        settings.write_new(&dir).unwrap();
        assert_eq!(
            fs::read_to_string(dir.join(SETTINGS_FILE_NAME)).unwrap(),
            "default_format: asciidoc\n"
        );
        assert_eq!(WorkspaceSettings::load(&dir).unwrap(), settings);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn missing_file_and_unknown_keys_are_accepted() {
        let dir = temp_dir("unknown");
        assert_eq!(
            WorkspaceSettings::load(&dir).unwrap(),
            WorkspaceSettings::default()
        );
        fs::write(
            dir.join(SETTINGS_FILE_NAME),
            "# mine\ntheme: Nord\nfuture_key: 3\ndefault_format: markdown\n",
        )
        .unwrap();
        let settings = WorkspaceSettings::load(&dir).unwrap();
        assert_eq!(settings.default_format, Some(NoteFormat::Markdown));
        assert_eq!(settings.theme, Some(Theme::Nord));
        fs::remove_dir_all(&dir).unwrap();
    }

    /// TC-010-02: the built-in defaults of UC-010 BR-001.
    #[test]
    fn uc010_tc02_built_in_defaults() {
        let effective = Effective::default();
        assert_eq!(effective.theme, Theme::Light);
        assert_eq!(effective.input_mode, InputMode::Normal);
        assert!(effective.show_line_numbers && effective.show_minimap && effective.copy_on_select);
        assert_eq!(effective.line_number_side, LineSide::Left);
        assert_eq!(effective.git_sync_interval, 5);
        assert_eq!(effective.agent_access, AgentAccess::Off);
        assert_eq!(
            (effective.overview_width, effective.tool_panel_width),
            (200, 240)
        );
    }

    /// TC-010-04, TC-010-05: a workspace value wins over the user setting.
    #[test]
    fn uc010_tc04_workspace_overrides_user() {
        let user = Settings {
            theme: Some(Theme::Gruvbox),
            input_mode: Some(InputMode::Vim),
            ..Settings::default()
        };
        let workspace = Settings {
            theme: Some(Theme::Dracula),
            ..Settings::default()
        };
        let effective = Effective::of(&user, Some(&workspace));
        assert_eq!(effective.theme, Theme::Dracula);
        assert_eq!(effective.input_mode, InputMode::Vim);
        assert_eq!(Effective::of(&user, None).theme, Theme::Gruvbox);
    }

    /// TC-010-15, UC-010 BR-008, TC-017-05: a workspace lowers agent
    /// access, never raises it.
    #[test]
    fn uc010_tc15_uc017_tc05_agent_access_only_with_the_users_consent() {
        let with = |access| Settings {
            agent_access: Some(access),
            ..Settings::default()
        };
        let none = Settings::default();
        let cloned = Effective::of(&none, Some(&with(AgentAccess::ReadWrite)));
        assert_eq!(cloned.agent_access, AgentAccess::Off);
        assert!(cloned.agent_access_refused && !cloned.agent_access_lowered);
        let read_write = with(AgentAccess::ReadWrite);
        assert_eq!(
            Effective::of(&read_write, Some(&none)).agent_access,
            AgentAccess::ReadWrite
        );
        let lowered = Effective::of(&read_write, Some(&with(AgentAccess::ReadOnly)));
        assert_eq!(lowered.agent_access, AgentAccess::ReadOnly);
        assert!(lowered.agent_access_lowered && !lowered.agent_access_refused);
        let raised = Effective::of(&with(AgentAccess::ReadOnly), Some(&read_write));
        assert_eq!(raised.agent_access, AgentAccess::ReadOnly);
        assert!(raised.agent_access_refused);
    }

    /// TC-017-03: off by default; the switch of earlier versions is read as
    /// read and write or off, and storing the level replaces it.
    #[test]
    fn uc017_tc03_earlier_switch_understood() {
        let path = Path::new("underline.yaml");
        let on = Settings::parse("mcp_server_enabled: true\n", path).unwrap();
        assert_eq!(on.agent_access, Some(AgentAccess::ReadWrite));
        let off = Settings::parse("mcp_server_enabled: false\n", path).unwrap();
        assert_eq!(off.agent_access, Some(AgentAccess::Off));
        let both =
            Settings::parse("mcp_server_enabled: true\nagent_access: read only\n", path).unwrap();
        assert_eq!(both.agent_access, Some(AgentAccess::ReadOnly));
        assert_eq!(Effective::default().agent_access, AgentAccess::Off);

        let dir = std::env::temp_dir().join(format!("underline-uc017-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let file = dir.join("underline.yaml");
        fs::write(&file, "# mine\ntheme: Nord\nmcp_server_enabled: true\n").unwrap();
        store_setting(&file, SettingKey::AgentAccess, Some("Read only")).unwrap();
        let text = fs::read_to_string(&file).unwrap();
        assert!(!text.contains("mcp_server_enabled"), "{text}");
        assert!(
            text.contains("# mine") && text.contains("theme: Nord"),
            "{text}"
        );
        let stored = Settings::load_file(&file).unwrap();
        assert_eq!(stored.agent_access, Some(AgentAccess::ReadOnly));
        fs::remove_dir_all(&dir).unwrap();
    }

    /// TC-010-13: one changed line; comments, order, unknown keys stay.
    #[test]
    fn uc010_tc13_one_line_changes() {
        let text =
            "# Team settings\ndefault_format: markdown\nfuture_key: 3\ntheme: Nord  # our pick\n";
        assert_eq!(
            set_key(text, "theme", Some("Dracula")),
            "# Team settings\ndefault_format: markdown\nfuture_key: 3\ntheme: Dracula  # our pick\n"
        );
        assert_eq!(
            set_key(text, "theme", None),
            "# Team settings\ndefault_format: markdown\nfuture_key: 3\n"
        );
        assert_eq!(
            set_key(text, "show_minimap", Some("false")),
            format!("{text}show_minimap: false\n")
        );
        assert_eq!(
            set_key("", "theme", Some("GitHub Light")),
            "theme: GitHub Light\n"
        );
        // A nested key of the same name is not taken for it.
        assert_eq!(
            set_key("other:\n  theme: x\n", "theme", Some("Nord")),
            "other:\n  theme: x\ntheme: Nord\n"
        );
    }

    /// TC-010-09: an error names its line; a file with an error is not
    /// written.
    #[test]
    fn uc010_tc09_errors_name_their_line() {
        let dir = temp_dir("errors");
        let path = dir.join(SETTINGS_FILE_NAME);
        fs::write(&path, "# mine\ntheme: Purple\n").unwrap();
        match Settings::load_file(&path) {
            Err(SettingsError::Invalid { line, message, .. }) => {
                assert_eq!(line, Some(2), "{message}");
                assert!(message.contains("Purple"), "{message}");
            }
            other => panic!("{other:?}"),
        }
        assert!(store_setting(&path, SettingKey::ShowMinimap, Some("false")).is_err());
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            "# mine\ntheme: Purple\n"
        );
        fs::write(&path, "theme: nord\ngit_sync_interval: 90\n").unwrap();
        match Settings::load_file(&path) {
            Err(SettingsError::Invalid { line, .. }) => assert_eq!(line, Some(2)),
            other => panic!("{other:?}"),
        }
        // Names are read without regard to case, spaces, and dashes.
        fs::write(&path, "theme: github-dark\ninput_mode: VIM\n").unwrap();
        let settings = Settings::load_file(&path).unwrap();
        assert_eq!(settings.theme, Some(Theme::GitHubDark));
        assert_eq!(settings.input_mode, Some(InputMode::Vim));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn set_in_memory() {
        let mut settings = Settings::default();
        settings.set(SettingKey::Theme, Some("Nord")).unwrap();
        settings
            .set(SettingKey::OverviewWidth, Some("300"))
            .unwrap();
        assert_eq!(settings.theme, Some(Theme::Nord));
        assert_eq!(
            settings.value_text(SettingKey::OverviewWidth).as_deref(),
            Some("300")
        );
        assert!(settings.set(SettingKey::OverviewWidth, Some("5")).is_err());
        settings.set(SettingKey::Theme, None).unwrap();
        assert_eq!(settings.theme, None);
    }

    /// TC-010-01, BR-005: a change is stored in its file, which appears on
    /// the first change.
    #[test]
    fn uc010_tc01_store_a_setting() {
        let dir = temp_dir("store");
        let path = dir.join(SETTINGS_FILE_NAME);
        store_setting(&path, SettingKey::Theme, Some("Nord")).unwrap();
        store_setting(&path, SettingKey::ShowMinimap, Some("false")).unwrap();
        let settings = Settings::load_file(&path).unwrap();
        assert_eq!(settings.theme, Some(Theme::Nord));
        assert_eq!(settings.show_minimap, Some(false));
        store_setting(&path, SettingKey::Theme, None).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "show_minimap: false\n");
        fs::remove_dir_all(&dir).unwrap();
    }
}

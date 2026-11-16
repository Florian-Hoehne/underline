//! The settings files `underline.yaml` (C-006, ADR-004, UC-010): the user
//! settings file in the home directory and the workspace settings file in a
//! workspace, read into [`Settings`] with every setting optional, combined
//! with the built-in defaults into [`Effective`] settings, and changed one
//! line at a time, so the user's comments and order stay (UC-010 BR-005).

use std::fmt;
use std::fs;
use std::io;
use std::ops::RangeInclusive;
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

/// FR-100, UC-010 BR-001, ADR-030: the language of the spell check, or
/// off. The settings file names it by its code (`en-US`), the settings
/// window by its name ("English (US)"); either is read, without regard to
/// case, spaces, dashes and underscores. Only languages whose dictionary
/// comes with Underline are offered (OQ-206, OQ-298).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum SpellCheckLanguage {
    /// No word is marked.
    #[default]
    Off,
    /// English as written in the United States.
    EnglishUs,
    /// English as written in the United Kingdom.
    EnglishUk,
    /// German as written in Germany (with `ß`).
    GermanDe,
    /// German as written in Switzerland (`ss` for `ß`).
    GermanCh,
    /// French.
    French,
    /// Italian.
    Italian,
    /// Spanish.
    Spanish,
}

impl SpellCheckLanguage {
    /// Every value, off first, then by name, in the order the settings
    /// window offers them.
    pub const ALL: &'static [SpellCheckLanguage] = &[
        SpellCheckLanguage::Off,
        SpellCheckLanguage::EnglishUk,
        SpellCheckLanguage::EnglishUs,
        SpellCheckLanguage::French,
        SpellCheckLanguage::GermanDe,
        SpellCheckLanguage::GermanCh,
        SpellCheckLanguage::Italian,
        SpellCheckLanguage::Spanish,
    ];

    /// The value as the settings file holds it: `off` or a language code.
    #[must_use]
    pub fn code(self) -> &'static str {
        match self {
            SpellCheckLanguage::Off => "off",
            SpellCheckLanguage::EnglishUs => "en-US",
            SpellCheckLanguage::EnglishUk => "en-GB",
            SpellCheckLanguage::GermanDe => "de-DE",
            SpellCheckLanguage::GermanCh => "de-CH",
            SpellCheckLanguage::French => "fr-FR",
            SpellCheckLanguage::Italian => "it-IT",
            SpellCheckLanguage::Spanish => "es-ES",
        }
    }

    /// The name the settings window shows.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            SpellCheckLanguage::Off => "Off",
            SpellCheckLanguage::EnglishUs => "English (US)",
            SpellCheckLanguage::EnglishUk => "English (UK)",
            SpellCheckLanguage::GermanDe => "German (Germany)",
            SpellCheckLanguage::GermanCh => "German (Switzerland)",
            SpellCheckLanguage::French => "French (France)",
            SpellCheckLanguage::Italian => "Italian (Italy)",
            SpellCheckLanguage::Spanish => "Spanish (Spain)",
        }
    }

    /// The code of the language's dictionary, the name of its folder
    /// (ADR-030); `None` when the spell check is off.
    #[must_use]
    pub fn dictionary(self) -> Option<&'static str> {
        (self != SpellCheckLanguage::Off).then(|| self.code())
    }

    /// OQ-206, OQ-298, OQ-299: the offered language a locale of the system
    /// names, such as `en_GB.UTF-8`, `de-CH` or `fr`; `None` for any other.
    /// The locale is read as [`SpellCheckLanguage::of_tag`] reads a tag,
    /// without its character set (`.UTF-8`) and modifier (`@euro`).
    #[must_use]
    pub fn of_locale(locale: &str) -> Option<Self> {
        let tag = locale.split(['.', '@']).next().unwrap_or_default();
        Self::of_tag(tag)
    }

    /// OQ-207, OQ-298: the offered language a language tag names, such as
    /// a note's attribute `lang`: `de-CH`, `de_CH`, `de-ch`, `de-CH-1996`;
    /// `None` for any other, `off` included. English without a region
    /// counts as English (US), of another region than the US and the UK as
    /// none. German of Switzerland and Liechtenstein, and Swiss German
    /// (`gsw`), count as German (Switzerland), German of any other region or
    /// without one as German (Germany); French, Italian and Spanish of any
    /// region as French, Italian and Spanish.
    #[must_use]
    pub fn of_tag(tag: &str) -> Option<Self> {
        let mut parts = tag.trim().split(['-', '_']);
        let language = parts.next().unwrap_or_default();
        let region =
            parts.find(|part| part.len() == 2 && part.chars().all(|c| c.is_ascii_alphabetic()));
        let is = |code: &str| language.eq_ignore_ascii_case(code);
        let region_is = |code: &str| region.is_some_and(|r| r.eq_ignore_ascii_case(code));
        if is("en") {
            if region.is_none() || region_is("US") {
                Some(SpellCheckLanguage::EnglishUs)
            } else if region_is("GB") {
                Some(SpellCheckLanguage::EnglishUk)
            } else {
                None
            }
        } else if is("gsw") || (is("de") && (region_is("CH") || region_is("LI"))) {
            Some(SpellCheckLanguage::GermanCh)
        } else if is("de") {
            Some(SpellCheckLanguage::GermanDe)
        } else if is("fr") {
            Some(SpellCheckLanguage::French)
        } else if is("it") {
            Some(SpellCheckLanguage::Italian)
        } else if is("es") {
            Some(SpellCheckLanguage::Spanish)
        } else {
            None
        }
    }
}

impl FromStr for SpellCheckLanguage {
    type Err = String;
    fn from_str(text: &str) -> Result<Self, String> {
        let wanted = simplify(text);
        SpellCheckLanguage::ALL
            .iter()
            .copied()
            .find(|value| simplify(value.code()) == wanted || simplify(value.label()) == wanted)
            .ok_or_else(|| {
                let codes: Vec<&str> = SpellCheckLanguage::ALL.iter().map(|v| v.code()).collect();
                format!("{text:?} is none of {}", codes.join(", "))
            })
    }
}

impl fmt::Display for SpellCheckLanguage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

impl Serialize for SpellCheckLanguage {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.code())
    }
}

impl<'de> Deserialize<'de> for SpellCheckLanguage {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        text.parse().map_err(de::Error::custom)
    }
}

/// OQ-206: the spell check language of the system's locale, when it is one
/// of the offered languages; read once per process.
#[must_use]
pub fn system_spell_language() -> Option<SpellCheckLanguage> {
    static SYSTEM: std::sync::OnceLock<Option<SpellCheckLanguage>> = std::sync::OnceLock::new();
    *SYSTEM.get_or_init(|| {
        sys_locale::get_locale()
            .as_deref()
            .and_then(SpellCheckLanguage::of_locale)
    })
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
    /// UC-005 BR-019: close pairs while typing.
    ClosePairs,
    /// FR-100, UC-005 BR-023: the language of the spell check, or off.
    SpellCheckLanguage,
    /// UC-010 BR-009: the size of the text of the notes, in percent.
    EditorZoom,
    /// UC-010 BR-011: long lines wrap at the edge of the editor.
    SoftWrap,
    /// UC-010 BR-010: the overview, the tool panel and the toolbars hidden.
    FocusMode,
    GitSyncInterval,
    AgentAccess,
    OverviewWidth,
    ToolPanelWidth,
    /// The menu bar at the top of the window, switched by its burger
    /// button.
    ShowMenuBar,
    /// UC-008 BR-008: search hits blink in the minimap.
    BlinkFoundInMinimap,
    /// UC-003 BR-008: the template folder; only for a workspace.
    TemplateFolder,
    /// UC-025 BR-002: the daily note folder; only for a workspace.
    DailyNoteFolder,
    /// UC-025 BR-003: the daily note template; only for a workspace.
    DailyNoteTemplate,
}

impl SettingKey {
    pub const ALL: [SettingKey; 21] = [
        SettingKey::DefaultFormat,
        SettingKey::Theme,
        SettingKey::InputMode,
        SettingKey::ShowLineNumbers,
        SettingKey::LineNumberSide,
        SettingKey::ShowMinimap,
        SettingKey::CopyOnSelect,
        SettingKey::ClosePairs,
        SettingKey::SpellCheckLanguage,
        SettingKey::EditorZoom,
        SettingKey::SoftWrap,
        SettingKey::FocusMode,
        SettingKey::GitSyncInterval,
        SettingKey::AgentAccess,
        SettingKey::OverviewWidth,
        SettingKey::ToolPanelWidth,
        SettingKey::ShowMenuBar,
        SettingKey::BlinkFoundInMinimap,
        SettingKey::TemplateFolder,
        SettingKey::DailyNoteFolder,
        SettingKey::DailyNoteTemplate,
    ];

    /// UC-010 BR-001, BR-002: the settings that exist only for a workspace:
    /// the default format, the template folder, the daily note folder and
    /// the daily note template; the user settings never give them.
    pub fn workspace_only(self) -> bool {
        matches!(
            self,
            SettingKey::DefaultFormat
                | SettingKey::TemplateFolder
                | SettingKey::DailyNoteFolder
                | SettingKey::DailyNoteTemplate
        )
    }

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
            SettingKey::ClosePairs => "close_pairs",
            SettingKey::SpellCheckLanguage => "spell_check_language",
            SettingKey::EditorZoom => "editor_zoom",
            SettingKey::SoftWrap => "soft_wrap",
            SettingKey::FocusMode => "focus_mode",
            SettingKey::GitSyncInterval => "git_sync_interval",
            SettingKey::AgentAccess => "agent_access",
            SettingKey::OverviewWidth => "overview_width",
            SettingKey::ToolPanelWidth => "tool_panel_width",
            SettingKey::ShowMenuBar => "show_menu_bar",
            SettingKey::BlinkFoundInMinimap => "blink_found_in_minimap",
            SettingKey::TemplateFolder => "template_folder",
            SettingKey::DailyNoteFolder => "daily_note_folder",
            SettingKey::DailyNoteTemplate => "daily_note_template",
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
    /// UC-005 BR-019: close pairs while typing.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub close_pairs: Option<bool>,
    /// FR-100, UC-005 BR-023: the language of the spell check, or off.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spell_check_language: Option<SpellCheckLanguage>,
    /// UC-010 BR-009: the editor zoom in percent, 50 to 300, kept as the
    /// file says it: a value between two steps stays in the file and is
    /// shown on the nearest step ([`Effective::editor_zoom`], [`zoom_step`]).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub editor_zoom: Option<u32>,
    /// UC-010 BR-011: soft wrap.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub soft_wrap: Option<bool>,
    /// UC-010 BR-010: the focus mode.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub focus_mode: Option<bool>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub show_menu_bar: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blink_found_in_minimap: Option<bool>,
    /// UC-003 BR-008: the folder of the workspace that holds its templates;
    /// only in workspace settings, `templates` when not given.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub template_folder: Option<String>,
    /// UC-025 BR-002: the folder of the workspace that holds its daily
    /// notes; only in workspace settings, `daily` when not given.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub daily_note_folder: Option<String>,
    /// UC-025 BR-003: the template a new daily note starts from, by its
    /// place in the workspace (`templates/Day.md`); only in workspace
    /// settings, the template titled Daily when not given.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub daily_note_template: Option<String>,
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
                SettingKey::EditorZoom,
                settings.editor_zoom,
                *EDITOR_ZOOM.start(),
                *EDITOR_ZOOM.end(),
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
            SettingKey::ClosePairs => self.close_pairs.map(bool_text),
            SettingKey::SpellCheckLanguage => {
                self.spell_check_language.map(|l| l.code().to_owned())
            }
            SettingKey::EditorZoom => self.editor_zoom.map(|v| v.to_string()),
            SettingKey::SoftWrap => self.soft_wrap.map(bool_text),
            SettingKey::FocusMode => self.focus_mode.map(bool_text),
            SettingKey::GitSyncInterval => self.git_sync_interval.map(|v| v.to_string()),
            SettingKey::AgentAccess => self.agent_access.map(|a| a.label().to_owned()),
            SettingKey::OverviewWidth => self.overview_width.map(|v| v.to_string()),
            SettingKey::ToolPanelWidth => self.tool_panel_width.map(|v| v.to_string()),
            SettingKey::ShowMenuBar => self.show_menu_bar.map(bool_text),
            SettingKey::BlinkFoundInMinimap => self.blink_found_in_minimap.map(bool_text),
            SettingKey::TemplateFolder => self.template_folder.clone(),
            SettingKey::DailyNoteFolder => self.daily_note_folder.clone(),
            SettingKey::DailyNoteTemplate => self.daily_note_template.clone(),
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
            SettingKey::ClosePairs => self.close_pairs = parsed.close_pairs,
            SettingKey::SpellCheckLanguage => {
                self.spell_check_language = parsed.spell_check_language;
            }
            SettingKey::EditorZoom => self.editor_zoom = parsed.editor_zoom,
            SettingKey::SoftWrap => self.soft_wrap = parsed.soft_wrap,
            SettingKey::FocusMode => self.focus_mode = parsed.focus_mode,
            SettingKey::GitSyncInterval => self.git_sync_interval = parsed.git_sync_interval,
            SettingKey::AgentAccess => {
                self.agent_access = parsed.agent_access;
                self.legacy_mcp_server_enabled = None;
            }
            SettingKey::OverviewWidth => self.overview_width = parsed.overview_width,
            SettingKey::ToolPanelWidth => self.tool_panel_width = parsed.tool_panel_width,
            SettingKey::ShowMenuBar => self.show_menu_bar = parsed.show_menu_bar,
            SettingKey::BlinkFoundInMinimap => {
                self.blink_found_in_minimap = parsed.blink_found_in_minimap;
            }
            SettingKey::TemplateFolder => self.template_folder = parsed.template_folder,
            SettingKey::DailyNoteFolder => self.daily_note_folder = parsed.daily_note_folder,
            SettingKey::DailyNoteTemplate => {
                self.daily_note_template = parsed.daily_note_template;
            }
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

/// Plain scalars YAML reads as null, which a text such as a folder named
/// `null` must not become (UC-010 BR-001).
const YAML_NULLS: [&str; 4] = ["~", "null", "Null", "NULL"];

/// A value as a YAML scalar, quoted only when it needs to be.
fn yaml_scalar(value: &str) -> String {
    let needs_quotes = value.is_empty()
        || YAML_NULLS.contains(&value)
        || value.contains([':', '#', '"', '\''])
        || value.starts_with([
            '-', '[', ']', '{', '}', ',', '?', '!', '&', '*', '|', '>', '%', '@', '`', ' ', '\t',
        ])
        || value.ends_with([' ', '\t']);
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
    /// UC-005 BR-019: close pairs while typing; on by default.
    pub close_pairs: bool,
    /// FR-100, UC-005 BR-023: the language of the spell check, or off; by
    /// default the system's language when it is offered, else off
    /// (OQ-206).
    pub spell_check_language: SpellCheckLanguage,
    /// UC-010 BR-009: the editor zoom in percent, a step of
    /// [`EDITOR_ZOOM_STEP`] within [`EDITOR_ZOOM`]; [`EDITOR_ZOOM_DEFAULT`]
    /// by default. A file's value between two steps shows as the nearest.
    pub editor_zoom: u32,
    /// UC-010 BR-011: soft wrap; on by default.
    pub soft_wrap: bool,
    /// UC-010 BR-010: the focus mode; off by default.
    pub focus_mode: bool,
    pub git_sync_interval: u32,
    /// UC-017 BR-002: the user's level, lowered by the workspace.
    pub agent_access: AgentAccess,
    pub overview_width: u32,
    pub tool_panel_width: u32,
    /// Whether the menu bar is shown; hidden, the window shows the name of
    /// the workspace in its place.
    pub show_menu_bar: bool,
    /// UC-008 BR-008: whether search hits blink in the minimap.
    pub blink_found_in_minimap: bool,
    /// UC-017 A1: the workspace lowers agent access below the user's level.
    pub agent_access_lowered: bool,
    /// UC-010 BR-008: the workspace settings ask for more agent access than
    /// the user settings allow, which is not followed.
    pub agent_access_refused: bool,
}

impl Default for Effective {
    /// The built-in defaults on a system whose language is not offered: the
    /// spell check is off.
    fn default() -> Self {
        Effective::of_with_system(&Settings::default(), None, None)
    }
}

impl Effective {
    /// UC-010 BR-001, BR-002, BR-008: the settings in effect for `user` and,
    /// with a workspace open, `workspace`; the spell check language
    /// defaults to the system's ([`system_spell_language`]).
    pub fn of(user: &Settings, workspace: Option<&Settings>) -> Self {
        Self::of_with_system(user, workspace, system_spell_language())
    }

    /// [`Effective::of`] on a system whose language is `system` (OQ-206):
    /// the spell check language neither settings file gives is `system`,
    /// else off.
    pub fn of_with_system(
        user: &Settings,
        workspace: Option<&Settings>,
        system: Option<SpellCheckLanguage>,
    ) -> Self {
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
            close_pairs: ws.close_pairs.or(user.close_pairs).unwrap_or(true),
            spell_check_language: ws
                .spell_check_language
                .or(user.spell_check_language)
                .or(system)
                .unwrap_or_default(),
            editor_zoom: zoom_step(
                ws.editor_zoom
                    .or(user.editor_zoom)
                    .unwrap_or(EDITOR_ZOOM_DEFAULT),
            ),
            soft_wrap: ws.soft_wrap.or(user.soft_wrap).unwrap_or(true),
            focus_mode: ws.focus_mode.or(user.focus_mode).unwrap_or(false),
            git_sync_interval: ws.git_sync_interval.or(user.git_sync_interval).unwrap_or(5),
            // A workspace can only lower agent access.
            agent_access: ws.agent_access.map_or(user_access, |w| w.min(user_access)),
            overview_width: ws.overview_width.or(user.overview_width).unwrap_or(200),
            tool_panel_width: ws.tool_panel_width.or(user.tool_panel_width).unwrap_or(240),
            show_menu_bar: ws.show_menu_bar.or(user.show_menu_bar).unwrap_or(false),
            blink_found_in_minimap: ws
                .blink_found_in_minimap
                .or(user.blink_found_in_minimap)
                .unwrap_or(true),
            agent_access_lowered: ws.agent_access.is_some_and(|w| w < user_access),
            agent_access_refused: ws.agent_access.is_some_and(|w| w > user_access),
        }
    }
}

/// UC-010 BR-009: the editor zoom's smallest and largest value, in percent.
pub const EDITOR_ZOOM: RangeInclusive<u32> = 50..=300;

/// UC-010 BR-001: the editor zoom's built-in default, in percent.
pub const EDITOR_ZOOM_DEFAULT: u32 = 100;

/// UC-010 BR-009: the editor zoom changes in steps of this many percent.
pub const EDITOR_ZOOM_STEP: u32 = 10;

/// UC-010 BR-009: `percent` on the nearest step of the editor zoom, within
/// its range; a file may hold a value between two steps.
#[must_use]
pub fn zoom_step(percent: u32) -> u32 {
    let stepped =
        percent.saturating_add(EDITOR_ZOOM_STEP / 2) / EDITOR_ZOOM_STEP * EDITOR_ZOOM_STEP;
    stepped.clamp(*EDITOR_ZOOM.start(), *EDITOR_ZOOM.end())
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

    /// TC-010-22 step 3: the template folder, the daily note folder and the
    /// daily note template are written and read as text, a number or a
    /// path with a colon too; they exist only for a workspace.
    #[test]
    fn uc010_tc22_template_and_daily_note_settings() {
        let mut settings = Settings::default();
        for (key, value) in [
            (SettingKey::TemplateFolder, "tpl"),
            (SettingKey::DailyNoteFolder, "2026"),
            (SettingKey::DailyNoteTemplate, "templates/Day: one.md"),
        ] {
            settings.set(key, Some(value)).unwrap();
            assert_eq!(settings.value_text(key).as_deref(), Some(value));
            assert!(key.workspace_only());
            let text = set_key("", key.key(), Some(value));
            let read = Settings::parse(&text, Path::new(SETTINGS_FILE_NAME)).unwrap();
            assert_eq!(read.value_text(key).as_deref(), Some(value), "{text}");
        }
        assert_eq!(settings.template_folder.as_deref(), Some("tpl"));
        settings.set(SettingKey::TemplateFolder, None).unwrap();
        assert_eq!(settings.template_folder, None);
        assert!(!SettingKey::Theme.workspace_only());
        assert!(SettingKey::DefaultFormat.workspace_only());
    }

    /// UC-010 BR-001, UC-025 BR-002 (monkey): a folder named `~` or `null` is
    /// a legal folder name; written to the file and read back it must stay
    /// that value, not turn into YAML null (which silently restores the
    /// default folder).
    #[test]
    fn uc010_br001_folder_named_like_yaml_null_round_trips() {
        for value in ["~", "null", "Null", "NULL"] {
            for key in [
                SettingKey::TemplateFolder,
                SettingKey::DailyNoteFolder,
                SettingKey::DailyNoteTemplate,
            ] {
                let text = set_key("", key.key(), Some(value));
                let read = Settings::parse(&text, Path::new(SETTINGS_FILE_NAME)).unwrap();
                assert_eq!(read.value_text(key).as_deref(), Some(value), "{text:?}");
            }
        }
    }

    /// UC-010 BR-001 (monkey): `,x` is a legal folder name but `set` refuses
    /// it with a YAML error, because it is written unquoted.
    #[test]
    fn uc010_br001_folder_starting_with_comma_can_be_set() {
        let mut settings = Settings::default();
        settings
            .set(SettingKey::DailyNoteFolder, Some(",x"))
            .unwrap();
        assert_eq!(settings.daily_note_folder.as_deref(), Some(",x"));
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
        assert!(!effective.show_menu_bar, "the menu bar starts hidden");
        assert!(effective.blink_found_in_minimap);
    }

    /// TC-010-18, TC-010-19, TC-010-20, UC-010 BR-001: editor zoom 100
    /// percent, soft wrap on, focus mode off at first.
    #[test]
    fn uc010_br001_zoom_soft_wrap_and_focus_mode_defaults() {
        let effective = Effective::default();
        assert_eq!(effective.editor_zoom, 100);
        assert!(effective.soft_wrap);
        assert!(!effective.focus_mode);
    }

    /// TC-010-18 step 2, UC-010 BR-009: the editor zoom is stored and read
    /// back; a value between two steps is shown on the nearest step, and one
    /// beyond 50 to 300 percent is an error that names its line.
    #[test]
    fn tc010_18_editor_zoom_is_stored_within_its_range() {
        let dir = temp_dir("zoom");
        let path = dir.join(SETTINGS_FILE_NAME);
        store_setting(&path, SettingKey::EditorZoom, Some("120")).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "editor_zoom: 120\n");
        let settings = Settings::load_file(&path).unwrap();
        assert_eq!(Effective::of(&settings, None).editor_zoom, 120);

        let odd = Settings::parse("editor_zoom: 124\n", &path).unwrap();
        assert_eq!(Effective::of(&odd, None).editor_zoom, 120);
        for wrong in ["editor_zoom: 40\n", "theme: Nord\neditor_zoom: 310\n"] {
            match Settings::parse(wrong, &path) {
                Err(SettingsError::Invalid { line, message, .. }) => {
                    assert_eq!(line, Some(wrong.lines().count()), "{message}");
                    assert!(message.contains("from 50 to 300"), "{message}");
                }
                other => panic!("{wrong:?} gave {other:?}"),
            }
        }
        fs::remove_dir_all(&dir).unwrap();
    }

    /// UC-010 BR-009: the steps of the editor zoom.
    #[test]
    fn uc010_br009_zoom_steps() {
        assert_eq!(zoom_step(100), 100);
        assert_eq!(zoom_step(104), 100);
        assert_eq!(zoom_step(105), 110);
        assert_eq!(zoom_step(0), 50);
        assert_eq!(zoom_step(u32::MAX), 300);
    }

    /// TC-010-19 step 3, TC-010-20 step 2, UC-010 BR-010, BR-011: the focus
    /// mode and soft wrap are settings like any other; a workspace value
    /// wins.
    #[test]
    fn uc010_br010_br011_focus_mode_and_soft_wrap_are_layered() {
        let user = Settings {
            focus_mode: Some(true),
            soft_wrap: Some(false),
            editor_zoom: Some(150),
            ..Settings::default()
        };
        let workspace = Settings {
            soft_wrap: Some(true),
            editor_zoom: Some(80),
            ..Settings::default()
        };
        let effective = Effective::of(&user, Some(&workspace));
        assert!(effective.focus_mode);
        assert!(effective.soft_wrap);
        assert_eq!(effective.editor_zoom, 80);
        let mut set = Settings::default();
        set.set(SettingKey::FocusMode, Some("true")).unwrap();
        set.set(SettingKey::SoftWrap, Some("false")).unwrap();
        assert_eq!(
            set.value_text(SettingKey::FocusMode).as_deref(),
            Some("true")
        );
        assert_eq!(
            set.value_text(SettingKey::SoftWrap).as_deref(),
            Some("false")
        );
        assert!(set.set(SettingKey::EditorZoom, Some("20")).is_err());
    }

    /// The burger button's switch of the menu bar is stored like any
    /// setting and read back.
    #[test]
    fn menu_bar_switch_is_stored() {
        let dir = temp_dir("menu-bar");
        let path = dir.join(SETTINGS_FILE_NAME);
        store_setting(&path, SettingKey::ShowMenuBar, Some("true")).unwrap();
        let settings = Settings::load_file(&path).unwrap();
        assert_eq!(settings.show_menu_bar, Some(true));
        assert_eq!(
            settings.value_text(SettingKey::ShowMenuBar).as_deref(),
            Some("true")
        );
        assert!(Effective::of(&settings, None).show_menu_bar);
        fs::remove_dir_all(&dir).unwrap();
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

    /// FR-100, UC-010 BR-001: the spell check language is written by its
    /// code and read by its code or its name.
    #[test]
    fn fr100_the_spell_check_language_by_code_or_name() {
        let parsed = Settings::parse("spell_check_language: en-GB\n", Path::new("u.yaml")).unwrap();
        assert_eq!(
            parsed.spell_check_language,
            Some(SpellCheckLanguage::EnglishUk)
        );
        let named =
            Settings::parse("spell_check_language: English (US)\n", Path::new("u.yaml")).unwrap();
        assert_eq!(
            named.spell_check_language,
            Some(SpellCheckLanguage::EnglishUs)
        );
        let off = Settings::parse("spell_check_language: off\n", Path::new("u.yaml")).unwrap();
        assert_eq!(off.spell_check_language, Some(SpellCheckLanguage::Off));
        let swiss = Settings::parse("spell_check_language: de-CH\n", Path::new("u.yaml")).unwrap();
        assert_eq!(
            swiss.spell_check_language,
            Some(SpellCheckLanguage::GermanCh),
            "OQ-299: German comes with Underline"
        );
        let italian = Settings::parse(
            "spell_check_language: Italian (Italy)\n",
            Path::new("u.yaml"),
        )
        .unwrap();
        assert_eq!(
            italian.spell_check_language,
            Some(SpellCheckLanguage::Italian)
        );
        let error = Settings::parse("spell_check_language: de-AT\n", Path::new("u.yaml"))
            .expect_err("German (Austria) is not offered");
        assert!(error.to_string().contains("en-US"), "{error}");
        let mut settings = Settings::default();
        settings
            .set(SettingKey::SpellCheckLanguage, Some("fr-FR"))
            .unwrap();
        assert_eq!(
            settings
                .value_text(SettingKey::SpellCheckLanguage)
                .as_deref(),
            Some("fr-FR")
        );
        assert_eq!(SpellCheckLanguage::French.dictionary(), Some("fr-FR"));
        assert_eq!(SpellCheckLanguage::Off.dictionary(), None);
    }

    /// OQ-206, OQ-298, TC-010-21: the default is the system's language when
    /// it is offered, else off; the workspace's value wins over the user's.
    #[test]
    fn oq206_the_spell_check_language_defaults_to_the_systems() {
        let of = SpellCheckLanguage::of_locale;
        assert_eq!(of("en_GB.UTF-8"), Some(SpellCheckLanguage::EnglishUk));
        assert_eq!(of("en-US"), Some(SpellCheckLanguage::EnglishUs));
        assert_eq!(of("en"), Some(SpellCheckLanguage::EnglishUs));
        assert_eq!(of("fr-CA"), Some(SpellCheckLanguage::French));
        assert_eq!(of("es_MX@euro"), Some(SpellCheckLanguage::Spanish));
        assert_eq!(of("de-CH"), Some(SpellCheckLanguage::GermanCh));
        assert_eq!(of("de_CH.UTF-8"), Some(SpellCheckLanguage::GermanCh));
        assert_eq!(of("de_LI"), Some(SpellCheckLanguage::GermanCh));
        assert_eq!(of("de_DE.UTF-8"), Some(SpellCheckLanguage::GermanDe));
        assert_eq!(of("de_AT"), Some(SpellCheckLanguage::GermanDe));
        assert_eq!(of("de"), Some(SpellCheckLanguage::GermanDe));
        assert_eq!(of("it_IT.UTF-8"), Some(SpellCheckLanguage::Italian));
        assert_eq!(of("it-CH"), Some(SpellCheckLanguage::Italian));
        assert_eq!(of("gsw_CH.UTF-8"), Some(SpellCheckLanguage::GermanCh));
        assert_eq!(of("en-AU"), None);
        assert_eq!(of("C"), None);

        let user = Settings::default();
        let uk = Some(SpellCheckLanguage::EnglishUk);
        assert_eq!(
            Effective::of_with_system(&user, None, uk).spell_check_language,
            SpellCheckLanguage::EnglishUk
        );
        assert_eq!(
            Effective::of_with_system(&user, None, None).spell_check_language,
            SpellCheckLanguage::Off
        );
        let me = Settings {
            spell_check_language: Some(SpellCheckLanguage::Off),
            ..Settings::default()
        };
        let workspace = Settings {
            spell_check_language: Some(SpellCheckLanguage::EnglishUk),
            ..Settings::default()
        };
        assert_eq!(
            Effective::of_with_system(&me, None, uk).spell_check_language,
            SpellCheckLanguage::Off
        );
        assert_eq!(
            Effective::of_with_system(&me, Some(&workspace), None).spell_check_language,
            SpellCheckLanguage::EnglishUk
        );
    }

    /// OQ-207: a note's attribute `lang` names an offered language by its
    /// code, written with `-` or `_` and in any case, with a region or a
    /// variant, or by its language alone.
    #[test]
    fn oq207_a_note_names_its_language_by_tag() {
        let of = SpellCheckLanguage::of_tag;
        assert_eq!(of("de-CH"), Some(SpellCheckLanguage::GermanCh));
        assert_eq!(of("de_CH"), Some(SpellCheckLanguage::GermanCh));
        assert_eq!(of("de-ch"), Some(SpellCheckLanguage::GermanCh));
        assert_eq!(of("de-CH-1996"), Some(SpellCheckLanguage::GermanCh));
        assert_eq!(of("gsw"), Some(SpellCheckLanguage::GermanCh));
        assert_eq!(of("de"), Some(SpellCheckLanguage::GermanDe));
        assert_eq!(of("it"), Some(SpellCheckLanguage::Italian));
        assert_eq!(of(" it-IT "), Some(SpellCheckLanguage::Italian));
        assert_eq!(of("en_us"), Some(SpellCheckLanguage::EnglishUs));
        assert_eq!(of("en-AU"), None);
        assert_eq!(of("German (Switzerland)"), None);
        assert_eq!(of("off"), None);
        assert_eq!(of("xx-YY"), None);
        assert_eq!(of(""), None);
    }
}

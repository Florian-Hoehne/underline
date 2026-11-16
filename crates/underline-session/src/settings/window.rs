//! UC-010 steps 2 to 4, A1, A2, A8, A9 (M-20.2 to M-20.5): what the
//! settings window shows of "For me" and "This workspace", and a change made
//! in it.
//!
//! Ported from `show_window` and `value_widget` of the egui app
//! (`apps/underline-desktop/src/settings.rs`): the same rows, labels,
//! choices and notes in the third column, word for word. The window only
//! lays them out and sends the value the user chose back.

use serde::Serialize;
use underline_core::{
    folder_setting_problem, zoom_step, AgentAccess, Effective, InputMode, LineSide, NoteFormat,
    SettingKey, Settings, SpellCheckLanguage, Template, Theme, DAILY_NOTE_FOLDER,
    DAILY_TEMPLATE_TITLE, EDITOR_ZOOM, EDITOR_ZOOM_STEP, TEMPLATE_FOLDER,
};

use super::{agent_access_refused_message, Scope, SettingName, SettingsState};

/// M-20.3: the git sync interval is 1 to 60 minutes (UC-010 BR-001).
const SYNC_MINUTES: (u32, u32) = (1, 60);

/// M-20.3: a panel is 100 to 1000 pixels wide (UC-010 BR-001).
const PANEL_PIXELS: (u32, u32) = (100, 1000);

/// "As for me": the button that removes a workspace's own value (A2).
const AS_FOR_ME: &str = "As for me";

/// The hover text of "As for me".
const AS_FOR_ME_HINT: &str = "Use the setting from \"For me\" again";

/// "Follow the notes": the default format without a value of its own (A8).
const FOLLOW_THE_NOTES: &str = "Follow the notes";

/// UC-025 BR-003: the daily note template without a value of its own.
fn titled_daily() -> String {
    format!("The template titled {DAILY_TEMPLATE_TITLE}")
}

/// What the settings window shows (UC-010 step 2, M-20.2 to M-20.4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct SettingsWindowView {
    /// Grows with every change of a settings file the session knows of; the
    /// window keeps the newest view and drops an older one that arrives
    /// late (an answer after an event, or the other way round).
    pub revision: u32,
    /// A workspace is open, so "This workspace" is offered (A9).
    pub workspace: bool,
    /// BR-007: the workspace settings file is in a git repository that does
    /// not ignore it; "This workspace" says that its values are shared.
    pub shared: bool,
    /// The rows of "For me", in the order of `SettingKey::ALL`, without the
    /// settings that exist only for a workspace (the default format, the
    /// template folder, the daily note folder and template).
    pub me: Vec<SettingRowView>,
    /// The rows of "This workspace"; empty without a workspace.
    pub this_workspace: Vec<SettingRowView>,
}

/// The answer to a change in the settings window (M-20.5): what it shows
/// now, and what went wrong, if anything, in the words of the status bar
/// (UC-010 A6, A7, a refused value), so the window can say it too.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct SettingsAnswer {
    /// What the settings window shows now.
    pub view: SettingsWindowView,
    /// The message of the change, `None` when it applied and was stored.
    pub problem: Option<String>,
}

/// One row of the settings window: the name, the control with the value in
/// effect, and the note in the third column (M-20.3, M-20.4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct SettingRowView {
    /// The setting, sent back with a change.
    pub key: SettingName,
    /// Its name, such as "Git sync every (minutes)".
    pub label: String,
    /// The control and the value it shows.
    pub control: SettingControlView,
    /// The third column, if it says anything.
    pub note: Option<SettingNoteView>,
    /// TC-010-22: what the setting does, shown under its control; `None`
    /// for the settings whose name says it.
    pub help: Option<String>,
    /// TC-010-22: the heading of the group the row starts, such as
    /// "Templates and daily note"; `None` for a row inside a group.
    pub section: Option<String>,
}

/// The control of one setting, as egui draws it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum SettingControlView {
    /// A choice of values (egui's combo box). `shown` is the text of the
    /// closed box; it is one of the choices' labels except for the default
    /// format, which shows "{fmt} (from the notes)" while it follows the
    /// notes.
    Choice {
        /// The text of the closed box.
        shown: String,
        /// The values to choose from, in their order.
        choices: Vec<SettingChoiceView>,
    },
    /// A check box with its text, such as "Shown".
    Check {
        /// Whether it is checked.
        on: bool,
        /// The text beside it.
        text: String,
    },
    /// A whole number within `min` and `max`.
    Number {
        /// The value in effect.
        value: u32,
        /// The smallest value allowed.
        min: u32,
        /// The largest value allowed.
        max: u32,
        /// How far one step of the arrow keys goes (UC-010 BR-009: the
        /// editor zoom in steps of 10).
        step: u32,
    },
    /// UC-010 BR-001: a folder of the workspace, typed; an empty field
    /// removes the workspace's value, so the built-in default applies.
    Text {
        /// The value in effect: the workspace's own, else the default.
        value: String,
        /// The built-in default, such as `templates`.
        default: String,
    },
}

/// One value of a choice.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct SettingChoiceView {
    /// The name shown, such as "GitHub Light".
    pub label: String,
    /// What is sent back to choose it; `None` removes the value (only
    /// "Follow the notes" of the default format, A8).
    pub value: Option<String>,
    /// It is the value chosen now.
    pub selected: bool,
}

/// The third column of a row (M-20.4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum SettingNoteView {
    /// A2: the button "As for me", which sends the change to `None`.
    AsForMe {
        /// "As for me".
        label: String,
        /// "Use the setting from "For me" again".
        hint: String,
    },
    /// A weak text, such as "as for me" or "this workspace uses its own
    /// value".
    Text {
        /// The text.
        text: String,
    },
}

impl SettingsState {
    /// Grows whenever what a settings file says may have changed; the
    /// settings window is told again only then.
    #[must_use]
    pub fn revision(&self) -> u32 {
        self.revision
    }

    /// UC-010 step 2 (M-20.2 to M-20.4): what the settings window shows;
    /// `derived` is the note format the workspace's notes give (A8, UC-002
    /// BR-002), `templates` the templates of the workspace, which the daily
    /// note template is chosen from (UC-025 BR-003).
    #[must_use]
    pub fn window_view(
        &self,
        derived: Option<NoteFormat>,
        templates: &[Template],
    ) -> SettingsWindowView {
        let me = Effective::of_with_system(&self.user, None, self.system_language());
        let workspace = self.workspace.as_ref().map(|(_, settings, _)| settings);
        let rows = |scope: Scope, shown: &Effective| -> Vec<SettingRowView> {
            SettingKey::ALL
                .into_iter()
                .filter(|key| scope == Scope::Workspace || !key.workspace_only())
                .map(|key| SettingRowView {
                    key: name_of(key),
                    label: label(key).to_owned(),
                    control: self.control(key, scope, shown, derived, templates),
                    note: self.note(key, scope),
                    help: help(key).map(str::to_owned),
                    section: (key == SettingKey::TemplateFolder)
                        .then(|| TEMPLATES_SECTION.to_owned()),
                })
                .collect()
        };
        SettingsWindowView {
            revision: self.revision,
            workspace: workspace.is_some(),
            shared: self
                .workspace
                .as_ref()
                .is_some_and(|(_, _, shared)| *shared),
            me: rows(Scope::Me, &me),
            this_workspace: if workspace.is_some() {
                rows(Scope::Workspace, &self.effective)
            } else {
                Vec::new()
            },
        }
    }

    /// UC-010 steps 3 and 4, A1, A2, A8 (M-20.5): `key` changed to `value`
    /// in the part `scope` of the settings window, `None` for "As for me"
    /// and "Follow the notes". In effect at once and stored at once; the
    /// failures are said in the status bar as for every change.
    ///
    /// As in egui, a value equal to the one shown is no change, so choosing
    /// the theme "This workspace" already uses gives it no value of its own;
    /// a number is kept within its range, and a changed panel width sets
    /// the panels to it (Q-03). Without a workspace, "This workspace"
    /// changes nothing.
    pub fn window_change(&mut self, scope: Scope, key: SettingKey, value: Option<String>) {
        if scope == Scope::Workspace && self.workspace.is_none() {
            return;
        }
        // UC-010 BR-002: the user settings never give a workspace's own
        // settings.
        if scope == Scope::Me && key.workspace_only() {
            return;
        }
        // A value as the window sends it never holds spaces or line breaks
        // around it; one that does must not reach the file as a new line.
        let value = value.map(|value| within_range(key, value.trim().to_owned()));
        // UC-010 BR-001: an emptied folder or template is no value of its
        // own (entity SETTINGS: an empty value means it is not given).
        let value = value.filter(|value| !(text_setting(key) && value.is_empty()));
        // OQ-274 (b): a folder that cannot take notes is refused at once,
        // and the field keeps what was typed.
        if let Some(value) = value.as_deref().filter(|_| text_setting(key)) {
            let top_allowed = key == SettingKey::DailyNoteFolder;
            if let Some(problem) = folder_setting_problem(value, top_allowed) {
                self.messages
                    .push(format!("{} not changed: {problem}", label(key)));
                return;
            }
        }
        // UC-010 BR-008: a workspace can only lower agent access; the window
        // offers no more, and more is refused here too.
        if scope == Scope::Workspace && key == SettingKey::AgentAccess {
            let raised = value
                .as_deref()
                .and_then(|value| value.parse::<AgentAccess>().ok())
                .is_some_and(|level| level > self.user_agent_access());
            if raised {
                self.messages.push(agent_access_refused_message());
                return;
            }
        }
        if !key.workspace_only() {
            if let Some(value) = &value {
                let shown = match scope {
                    Scope::Me => {
                        Effective::of_with_system(&self.user, None, self.system_language())
                    }
                    Scope::Workspace => self.effective.clone(),
                };
                if is_shown(&shown, key, value) {
                    return;
                }
            }
        }
        let applied = self.change(scope, key, value);
        if applied && matches!(key, SettingKey::OverviewWidth | SettingKey::ToolPanelWidth) {
            self.width_generation = self.width_generation.wrapping_add(1);
        }
    }

    fn control(
        &self,
        key: SettingKey,
        scope: Scope,
        shown: &Effective,
        derived: Option<NoteFormat>,
        templates: &[Template],
    ) -> SettingControlView {
        let check = |on: bool, text: &str| SettingControlView::Check {
            on,
            text: text.to_owned(),
        };
        let number = |value: u32, (min, max): (u32, u32)| SettingControlView::Number {
            value,
            min,
            max,
            step: 1,
        };
        match key {
            SettingKey::DefaultFormat => self.default_format(derived),
            SettingKey::Theme => choose(shown.theme, Theme::ALL),
            SettingKey::InputMode => choose(shown.input_mode, InputMode::ALL),
            SettingKey::LineNumberSide => choose(shown.line_number_side, LineSide::ALL),
            SettingKey::ShowLineNumbers => {
                check(shown.show_line_numbers, "Shown, in the source view")
            }
            SettingKey::ShowMinimap => check(shown.show_minimap, "Shown"),
            SettingKey::CopyOnSelect => check(shown.copy_on_select, "On"),
            SettingKey::ClosePairs => check(shown.close_pairs, "On"),
            SettingKey::SpellCheckLanguage => spell_languages(shown.spell_check_language),
            SettingKey::EditorZoom => SettingControlView::Number {
                value: shown.editor_zoom,
                min: *EDITOR_ZOOM.start(),
                max: *EDITOR_ZOOM.end(),
                step: EDITOR_ZOOM_STEP,
            },
            SettingKey::SoftWrap => check(shown.soft_wrap, "On"),
            SettingKey::FocusMode => check(shown.focus_mode, "On"),
            SettingKey::AgentAccess => match scope {
                Scope::Me => choose(shown.agent_access, AgentAccess::ALL),
                // UC-010 BR-008, UC-017 BR-002: a workspace can only lower
                // it, so only the levels up to the user's are offered.
                Scope::Workspace => {
                    let user = self.user_agent_access();
                    let lower: Vec<AgentAccess> = AgentAccess::ALL
                        .iter()
                        .copied()
                        .filter(|level| *level <= user)
                        .collect();
                    choose(shown.agent_access, &lower)
                }
            },
            SettingKey::GitSyncInterval => number(shown.git_sync_interval, SYNC_MINUTES),
            SettingKey::OverviewWidth => number(shown.overview_width, PANEL_PIXELS),
            SettingKey::ToolPanelWidth => number(shown.tool_panel_width, PANEL_PIXELS),
            SettingKey::ShowMenuBar => check(shown.show_menu_bar, "Shown"),
            SettingKey::BlinkFoundInMinimap => check(shown.blink_found_in_minimap, "Blinking"),
            SettingKey::TemplateFolder => self.folder(key, TEMPLATE_FOLDER),
            SettingKey::DailyNoteFolder => self.folder(key, DAILY_NOTE_FOLDER),
            SettingKey::DailyNoteTemplate => self.daily_template(templates),
        }
    }

    /// UC-010 BR-001: the template or daily note folder: the workspace's
    /// own, else `default`.
    fn folder(&self, key: SettingKey, default: &str) -> SettingControlView {
        SettingControlView::Text {
            value: self
                .workspace_text(key)
                .unwrap_or_else(|| default.to_owned()),
            default: default.to_owned(),
        }
    }

    /// UC-025 BR-003, TC-010-22: the daily note template, chosen from the
    /// templates of the workspace, or "The template titled Daily"; a value
    /// that names no template is offered as it is, marked "not found".
    fn daily_template(&self, templates: &[Template]) -> SettingControlView {
        let own = self.workspace_text(SettingKey::DailyNoteTemplate);
        let mut choices = vec![SettingChoiceView {
            label: titled_daily(),
            value: None,
            selected: own.is_none(),
        }];
        let mut found = false;
        for template in templates {
            let file = template.file_name();
            let selected = own
                .as_deref()
                .is_some_and(|own| own == file || own == template.id.as_str());
            found |= selected;
            choices.push(SettingChoiceView {
                label: file.clone(),
                value: Some(file),
                selected,
            });
        }
        if let (Some(own), false) = (&own, found) {
            choices.push(SettingChoiceView {
                label: format!("{own} (not found)"),
                value: Some(own.clone()),
                selected: true,
            });
        }
        let shown = choices
            .iter()
            .find(|choice| choice.selected)
            .map_or_else(titled_daily, |choice| choice.label.clone());
        SettingControlView::Choice { shown, choices }
    }

    /// A8: the workspace's own format, else the one its notes give, else
    /// "Follow the notes".
    fn default_format(&self, derived: Option<NoteFormat>) -> SettingControlView {
        let own = self
            .workspace
            .as_ref()
            .and_then(|(_, settings, _)| settings.default_format);
        let shown = match (own, derived) {
            (Some(format), _) => format.name().to_owned(),
            (None, Some(format)) => format!("{} (from the notes)", format.name()),
            (None, None) => FOLLOW_THE_NOTES.to_owned(),
        };
        let mut choices: Vec<SettingChoiceView> = NoteFormat::ALL
            .iter()
            .map(|format| SettingChoiceView {
                label: format.name().to_owned(),
                value: Some(format.name().to_lowercase()),
                selected: own == Some(*format),
            })
            .collect();
        choices.push(SettingChoiceView {
            label: FOLLOW_THE_NOTES.to_owned(),
            value: None,
            selected: own.is_none(),
        });
        SettingControlView::Choice { shown, choices }
    }

    /// M-20.4: the third column of `key` in `scope`.
    fn note(&self, key: SettingKey, scope: Scope) -> Option<SettingNoteView> {
        let (_, workspace, _) = self.workspace.as_ref()?;
        let text = |text: &str| {
            Some(SettingNoteView::Text {
                text: text.to_owned(),
            })
        };
        match scope {
            Scope::Workspace if workspace.sets(key) => {
                if key.workspace_only() {
                    text("own")
                } else {
                    Some(SettingNoteView::AsForMe {
                        label: AS_FOR_ME.to_owned(),
                        hint: AS_FOR_ME_HINT.to_owned(),
                    })
                }
            }
            // UC-010 BR-001: the built-in default of a workspace's own
            // setting, not the user's.
            Scope::Workspace if key.workspace_only() && key != SettingKey::DefaultFormat => {
                text("default")
            }
            Scope::Workspace => text("as for me"),
            Scope::Me if key == SettingKey::AgentAccess && self.effective.agent_access_refused => {
                text("this workspace asks for more; not followed")
            }
            Scope::Me if workspace.sets(key) => text("this workspace uses its own value"),
            Scope::Me => None,
        }
    }
}

/// The name of a setting in the settings window, as in egui.
fn label(key: SettingKey) -> &'static str {
    match key {
        SettingKey::DefaultFormat => "Default format",
        SettingKey::Theme => "Theme",
        SettingKey::InputMode => "Input mode",
        SettingKey::ShowLineNumbers => "Line numbers",
        SettingKey::LineNumberSide => "Line number side",
        SettingKey::ShowMinimap => "Minimap",
        SettingKey::CopyOnSelect => "Copy on select",
        SettingKey::ClosePairs => "Close pairs",
        SettingKey::SpellCheckLanguage => "Spell check language",
        SettingKey::EditorZoom => "Editor zoom (percent)",
        SettingKey::SoftWrap => "Soft wrap",
        SettingKey::FocusMode => "Focus mode",
        SettingKey::GitSyncInterval => "Git sync every (minutes)",
        SettingKey::AgentAccess => "Agent access",
        SettingKey::OverviewWidth => "Note overview width",
        SettingKey::ToolPanelWidth => "Tool panel width",
        SettingKey::ShowMenuBar => "Menu bar",
        SettingKey::BlinkFoundInMinimap => "Search hits in the minimap",
        SettingKey::TemplateFolder => "Template folder",
        SettingKey::DailyNoteFolder => "Daily note folder",
        SettingKey::DailyNoteTemplate => "Daily note template",
    }
}

/// The window's name of `key`.
fn name_of(key: SettingKey) -> SettingName {
    match key {
        SettingKey::DefaultFormat => SettingName::DefaultFormat,
        SettingKey::Theme => SettingName::Theme,
        SettingKey::InputMode => SettingName::InputMode,
        SettingKey::ShowLineNumbers => SettingName::ShowLineNumbers,
        SettingKey::LineNumberSide => SettingName::LineNumberSide,
        SettingKey::ShowMinimap => SettingName::ShowMinimap,
        SettingKey::CopyOnSelect => SettingName::CopyOnSelect,
        SettingKey::ClosePairs => SettingName::ClosePairs,
        SettingKey::SpellCheckLanguage => SettingName::SpellCheckLanguage,
        SettingKey::EditorZoom => SettingName::EditorZoom,
        SettingKey::SoftWrap => SettingName::SoftWrap,
        SettingKey::FocusMode => SettingName::FocusMode,
        SettingKey::GitSyncInterval => SettingName::GitSyncInterval,
        SettingKey::AgentAccess => SettingName::AgentAccess,
        SettingKey::OverviewWidth => SettingName::OverviewWidth,
        SettingKey::ToolPanelWidth => SettingName::ToolPanelWidth,
        SettingKey::ShowMenuBar => SettingName::ShowMenuBar,
        SettingKey::BlinkFoundInMinimap => SettingName::BlinkFoundInMinimap,
        SettingKey::TemplateFolder => SettingName::TemplateFolder,
        SettingKey::DailyNoteFolder => SettingName::DailyNoteFolder,
        SettingKey::DailyNoteTemplate => SettingName::DailyNoteTemplate,
    }
}

/// A combo box over `all`, showing `current` (egui's `choose`).
fn choose<T: Copy + PartialEq + std::fmt::Display>(current: T, all: &[T]) -> SettingControlView {
    SettingControlView::Choice {
        shown: current.to_string(),
        choices: all
            .iter()
            .map(|value| SettingChoiceView {
                label: value.to_string(),
                value: Some(value.to_string()),
                selected: *value == current,
            })
            .collect(),
    }
}

/// FR-100, UC-010 BR-001: the spell check languages by their names; the
/// value sent back is the code the settings file holds (`en-US`).
fn spell_languages(current: SpellCheckLanguage) -> SettingControlView {
    SettingControlView::Choice {
        shown: current.label().to_owned(),
        choices: SpellCheckLanguage::ALL
            .iter()
            .map(|language| SettingChoiceView {
                label: language.label().to_owned(),
                value: Some(language.code().to_owned()),
                selected: *language == current,
            })
            .collect(),
    }
}

/// TC-010-22: the heading of the settings of templates and daily notes.
const TEMPLATES_SECTION: &str = "Templates and daily note";

/// TC-010-22: what a setting does, under its control.
fn help(key: SettingKey) -> Option<&'static str> {
    match key {
        SettingKey::TemplateFolder => {
            Some("New notes can start from the notes in this folder. Empty: the folder templates.")
        }
        SettingKey::DailyNoteFolder => {
            Some("Daily notes are kept in this folder. Empty: the folder daily.")
        }
        SettingKey::DailyNoteTemplate => Some("A new daily note starts from this template."),
        SettingKey::SpellCheckLanguage => Some(SPELL_CHECK_HELP),
        _ => None,
    }
}

/// FR-099, FR-100: how the spell check's corrections are reached, and its
/// default.
const SPELL_CHECK_HELP: &str = "Right-click a marked word, or press Shift+F10 on it, for corrections. Default: your system language, if offered.";

/// UC-010 BR-001: the settings typed as text, the folders.
fn text_setting(key: SettingKey) -> bool {
    matches!(
        key,
        SettingKey::TemplateFolder | SettingKey::DailyNoteFolder
    )
}

/// egui's drag values keep a number within its range; a value that is no
/// number stays as it is and is refused when it is set.
fn within_range(key: SettingKey, value: String) -> String {
    if key == SettingKey::EditorZoom {
        // UC-010 BR-009: on the nearest step of 10 percent.
        return match value.trim().parse::<u32>() {
            Ok(number) => zoom_step(number).to_string(),
            Err(_) => value,
        };
    }
    let range = match key {
        SettingKey::GitSyncInterval => SYNC_MINUTES,
        SettingKey::OverviewWidth | SettingKey::ToolPanelWidth => PANEL_PIXELS,
        _ => return value,
    };
    match value.trim().parse::<u32>() {
        Ok(number) => number.clamp(range.0, range.1).to_string(),
        Err(_) => value,
    }
}

/// Whether `value`, as written to a file, is what `shown` holds for `key`.
fn is_shown(shown: &Effective, key: SettingKey, value: &str) -> bool {
    let mut wanted = Settings::default();
    if wanted.set(key, Some(value)).is_err() {
        return false;
    }
    let mut current = Settings::default();
    let bool_text = |on: bool| on.to_string();
    let text = match key {
        SettingKey::DefaultFormat
        | SettingKey::TemplateFolder
        | SettingKey::DailyNoteFolder
        | SettingKey::DailyNoteTemplate => return false,
        SettingKey::Theme => shown.theme.label().to_owned(),
        SettingKey::InputMode => shown.input_mode.label().to_owned(),
        SettingKey::ShowLineNumbers => bool_text(shown.show_line_numbers),
        SettingKey::LineNumberSide => shown.line_number_side.label().to_owned(),
        SettingKey::ShowMinimap => bool_text(shown.show_minimap),
        SettingKey::CopyOnSelect => bool_text(shown.copy_on_select),
        SettingKey::ClosePairs => bool_text(shown.close_pairs),
        SettingKey::SpellCheckLanguage => shown.spell_check_language.code().to_owned(),
        SettingKey::EditorZoom => shown.editor_zoom.to_string(),
        SettingKey::SoftWrap => bool_text(shown.soft_wrap),
        SettingKey::FocusMode => bool_text(shown.focus_mode),
        SettingKey::GitSyncInterval => shown.git_sync_interval.to_string(),
        SettingKey::AgentAccess => shown.agent_access.label().to_owned(),
        SettingKey::OverviewWidth => shown.overview_width.to_string(),
        SettingKey::ToolPanelWidth => shown.tool_panel_width.to_string(),
        SettingKey::ShowMenuBar => bool_text(shown.show_menu_bar),
        SettingKey::BlinkFoundInMinimap => bool_text(shown.blink_found_in_minimap),
    };
    current.set(key, Some(&text)).is_ok() && current.value_text(key) == wanted.value_text(key)
}

#[cfg(test)]
mod tests;

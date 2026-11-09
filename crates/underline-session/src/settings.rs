//! UC-010 Adjust Settings, without a window: the user settings and the
//! settings of the open workspace, the settings in effect, where a change is
//! stored, and watching the user settings file.
//!
//! Ported from `apps/underline-desktop/src/settings.rs` (`SettingsState`);
//! the settings window itself comes with feature F10 (M-20.1 to M-20.4).

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

use serde::{Deserialize, Serialize};
use underline_core::{
    store_setting, user_settings_path, AgentAccess, Effective, Git, SettingKey, Settings,
    SettingsError, Theme, WorkspaceSettings, SETTINGS_FILE_NAME,
};

use crate::palette::PaletteView;

/// How often the user settings file is looked at (M-20.6, UC-010 A5).
pub const USER_SETTINGS_POLL: Duration = Duration::from_millis(500);

/// The two parts of the settings (UC-010 step 2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    /// "For me": the user settings file.
    Me,
    /// "This workspace": the workspace settings file.
    Workspace,
}

/// The settings of the window: what the files say, what is in effect, and
/// the messages for the status bar.
#[derive(Debug)]
pub struct SettingsState {
    user: Settings,
    user_path: Option<PathBuf>,
    /// When the user settings file was modified as last read, and when it
    /// was looked at.
    user_seen: Option<SystemTime>,
    user_checked: Instant,
    /// The workspace's root and settings, and whether its settings file is
    /// shared through git (UC-010 BR-007).
    workspace: Option<(PathBuf, Settings, bool)>,
    /// Files that cannot be read now, which are not written (UC-010 A6).
    broken: Vec<PathBuf>,
    effective: Effective,
    /// Messages for the status bar.
    messages: Vec<String>,
    /// Counts the times a settings file was read again (Q-03): each time,
    /// the panels go back to their stored widths, as in the egui app.
    width_generation: u32,
}

impl SettingsState {
    /// Reads the user settings file in the home directory at start; one with
    /// an error leaves the built-in defaults in effect (UC-010 A6).
    #[must_use]
    pub fn load(now: Instant) -> Self {
        Self::with_user_file(user_settings_path(), now)
    }

    /// Like [`SettingsState::load`], with the user settings file at
    /// `user_path`; `None` stands for a system without a home directory.
    #[must_use]
    pub fn with_user_file(user_path: Option<PathBuf>, now: Instant) -> Self {
        let mut state = Self {
            user: Settings::default(),
            user_path,
            user_seen: None,
            user_checked: now,
            workspace: None,
            broken: Vec::new(),
            effective: Effective::default(),
            messages: Vec::new(),
            width_generation: 0,
        };
        state.read_user();
        state.recompute();
        state
    }

    /// The settings in effect (UC-010 BR-002).
    #[must_use]
    pub fn effective(&self) -> &Effective {
        &self.effective
    }

    /// Q-03, M-05.1: grows each time a settings file is read again after it
    /// changed outside; the window then sets the panels back to the widths
    /// in effect, as egui does with its `width_generation`. The settings
    /// window (feature F10) bumps it too when it changes a width.
    #[must_use]
    pub fn width_generation(&self) -> u32 {
        self.width_generation
    }

    /// The directory of the user settings file, which is never a workspace
    /// (UC-002 BR-007).
    #[must_use]
    pub fn user_settings_dir(&self) -> Option<PathBuf> {
        self.user_path
            .as_deref()
            .and_then(Path::parent)
            .map(Path::to_owned)
    }

    /// Messages for the status bar since the last call.
    pub fn take_messages(&mut self) -> Vec<String> {
        std::mem::take(&mut self.messages)
    }

    fn read_user(&mut self) {
        let Some(path) = self.user_path.clone() else {
            return;
        };
        self.user_seen = modified(&path);
        match Settings::load_file(&path) {
            Ok(settings) => {
                self.user = settings;
                self.broken.retain(|p| p != &path);
            }
            Err(err) => self.broken_file(&path, &err),
        }
    }

    fn broken_file(&mut self, path: &Path, err: &SettingsError) {
        if !self.broken.iter().any(|p| p == path) {
            self.broken.push(path.to_owned());
        }
        self.messages
            .push(format!("The settings keep their last values: {err}"));
    }

    /// The settings of the workspace at `root`, opened now (UC-002).
    pub fn open_workspace(&mut self, root: &Path, git: &Git) {
        let path = WorkspaceSettings::path(root);
        let settings = match Settings::load_file(&path) {
            Ok(settings) => {
                self.broken.retain(|p| p != &path);
                settings
            }
            Err(err) => {
                self.broken_file(&path, &err);
                Settings::default()
            }
        };
        let shared = root.join(".git").exists() && !git.is_ignored(root, SETTINGS_FILE_NAME);
        self.workspace = Some((root.to_owned(), settings, shared));
        // A workspace opened anew is reported even when the last one asked
        // for more too.
        self.effective.agent_access_refused = false;
        self.recompute();
    }

    /// UC-010 A5: the workspace settings file changed outside.
    pub fn reload_workspace(&mut self) {
        let Some((root, _, shared)) = self.workspace.clone() else {
            return;
        };
        let path = WorkspaceSettings::path(&root);
        match Settings::load_file(&path) {
            Ok(settings) => {
                self.broken.retain(|p| p != &path);
                self.workspace = Some((root, settings, shared));
                self.width_generation = self.width_generation.wrapping_add(1);
                self.recompute();
            }
            Err(err) => self.broken_file(&path, &err),
        }
    }

    /// UC-010 A5, M-20.6: looks at the user settings file every
    /// [`USER_SETTINGS_POLL`] and reads it again when it changed.
    pub fn poll(&mut self, now: Instant) {
        if now.saturating_duration_since(self.user_checked) < USER_SETTINGS_POLL {
            return;
        }
        self.user_checked = now;
        let Some(path) = &self.user_path else {
            return;
        };
        if modified(path) != self.user_seen {
            self.read_user();
            self.width_generation = self.width_generation.wrapping_add(1);
            self.recompute();
        }
    }

    fn recompute(&mut self) {
        let workspace = self.workspace.as_ref().map(|(_, s, _)| s);
        let refused = self.effective.agent_access_refused;
        self.effective = Effective::of(&self.user, workspace);
        // UC-017 A1: said when the workspace starts asking for more.
        if self.effective.agent_access_refused && !refused {
            self.messages.push(agent_access_refused_message());
        }
    }

    fn path(&self, scope: Scope) -> Option<PathBuf> {
        match scope {
            Scope::Me => self.user_path.clone(),
            Scope::Workspace => self
                .workspace
                .as_ref()
                .map(|(root, _, _)| WorkspaceSettings::path(root)),
        }
    }

    fn settings_mut(&mut self, scope: Scope) -> Option<&mut Settings> {
        match scope {
            Scope::Me => Some(&mut self.user),
            Scope::Workspace => self.workspace.as_mut().map(|(_, s, _)| s),
        }
    }

    /// UC-010 BR-003: where a change made outside the settings window goes:
    /// the workspace when it has a value of its own, otherwise the user.
    #[must_use]
    pub fn scope_of(&self, key: SettingKey) -> Scope {
        match &self.workspace {
            Some((_, settings, _)) if settings.sets(key) => Scope::Workspace,
            _ => Scope::Me,
        }
    }

    /// Changes `key` in `scope` (UC-010 A1 to A4): in effect at once, stored
    /// within 1 second (BR-004, BR-005); a file with an error or one that
    /// cannot be written keeps the change for this session and says so (A6,
    /// A7).
    pub fn change(&mut self, scope: Scope, key: SettingKey, value: Option<String>) {
        let Some(settings) = self.settings_mut(scope) else {
            return;
        };
        if let Err(reason) = settings.set(key, value.as_deref()) {
            self.messages.push(reason);
            return;
        }
        self.recompute();
        let Some(path) = self.path(scope) else {
            self.messages
                .push("There is no home directory to store the setting in".into());
            return;
        };
        if self.broken.contains(&path) {
            self.messages.push(format!(
                "{} has an error; the setting applies until Underline is closed and is stored once the file is corrected",
                path.display()
            ));
            return;
        }
        match store_setting(&path, key, value.as_deref()) {
            Ok(()) => {
                if scope == Scope::Me {
                    self.user_seen = modified(&path);
                }
            }
            Err(err) => self.messages.push(format!(
                "The setting could not be stored and applies until Underline is closed: {err}"
            )),
        }
    }

    /// UC-010 A3, A4, M-20.5: changes a setting from outside the settings
    /// window, in the file BR-003 names.
    pub fn quick_change(&mut self, key: SettingKey, value: String) {
        let scope = self.scope_of(key);
        self.change(scope, key, Some(value));
    }

    /// UC-017 BR-002: the agent access the user settings allow.
    #[must_use]
    pub fn user_agent_access(&self) -> AgentAccess {
        self.user.agent_access.unwrap_or_default()
    }

    /// UC-017 A1: the level the workspace settings lower agent access to.
    #[must_use]
    pub fn workspace_agent_access(&self) -> Option<AgentAccess> {
        self.workspace.as_ref().and_then(|(_, s, _)| s.agent_access)
    }

    /// What the window needs of the settings in effect, with the palette of
    /// the theme (M-01.6).
    #[must_use]
    pub fn view(&self) -> SettingsView {
        SettingsView::of(&self.effective, self.width_generation)
    }
}

/// UC-010 BR-008, UC-017 A1: the workspace asks for more agent access than
/// the user allows.
#[must_use]
pub fn agent_access_refused_message() -> String {
    "This workspace's settings ask for more agent access than your own settings allow; that is not followed (only your own settings can raise it)".into()
}

fn modified(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

/// One setting of UC-010 BR-001, named as in the settings file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
#[allow(missing_docs)]
pub enum SettingName {
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
    ShowMenuBar,
    BlinkFoundInMinimap,
}

impl From<SettingName> for SettingKey {
    fn from(name: SettingName) -> Self {
        match name {
            SettingName::DefaultFormat => SettingKey::DefaultFormat,
            SettingName::Theme => SettingKey::Theme,
            SettingName::InputMode => SettingKey::InputMode,
            SettingName::ShowLineNumbers => SettingKey::ShowLineNumbers,
            SettingName::LineNumberSide => SettingKey::LineNumberSide,
            SettingName::ShowMinimap => SettingKey::ShowMinimap,
            SettingName::CopyOnSelect => SettingKey::CopyOnSelect,
            SettingName::GitSyncInterval => SettingKey::GitSyncInterval,
            SettingName::AgentAccess => SettingKey::AgentAccess,
            SettingName::OverviewWidth => SettingKey::OverviewWidth,
            SettingName::ToolPanelWidth => SettingKey::ToolPanelWidth,
            SettingName::ShowMenuBar => SettingKey::ShowMenuBar,
            SettingName::BlinkFoundInMinimap => SettingKey::BlinkFoundInMinimap,
        }
    }
}

/// The settings in effect as the window shows them (UC-010 BR-002), with
/// the palette of the theme in effect (M-01.6). Choices are given by the
/// names the settings file and the window use, such as "GitHub Light".
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct SettingsView {
    /// The theme in effect (FR-022), by its name.
    pub theme: String,
    /// The colours of the theme in effect.
    pub palette: PaletteView,
    /// The keys the editor follows (FR-024, FR-025): "Normal", "Vim", "Nano".
    pub input_mode: String,
    /// Line numbers in the source view (FR-029).
    pub show_line_numbers: bool,
    /// "Left" or "Right".
    pub line_number_side: String,
    /// The minimap beside the editor.
    pub show_minimap: bool,
    /// Copying the selection as it is made.
    pub copy_on_select: bool,
    /// Minutes between two git syncs.
    pub git_sync_interval: u32,
    /// What agents may do (UC-017 BR-002): "Off", "Read only",
    /// "Read and write".
    pub agent_access: String,
    /// The workspace lowers agent access below the user's level (UC-017 A1).
    pub agent_access_lowered: bool,
    /// The workspace asks for more agent access, which is not followed
    /// (UC-010 BR-008).
    pub agent_access_refused: bool,
    /// Width of the note overview in pixels.
    pub overview_width: u32,
    /// Width of the tool panel in pixels.
    pub tool_panel_width: u32,
    /// Whether the menu bar is shown (M-03.1).
    pub show_menu_bar: bool,
    /// Whether search hits blink in the minimap (UC-008 BR-008).
    pub blink_found_in_minimap: bool,
    /// Q-03: grows each time a settings file was read again; the window
    /// then sets the panels back to `overview_width` and `tool_panel_width`.
    pub width_generation: u32,
}

impl SettingsView {
    /// The view of `effective`, with the panel widths' generation (Q-03).
    #[must_use]
    pub fn of(effective: &Effective, width_generation: u32) -> Self {
        Self {
            theme: effective.theme.label().to_owned(),
            palette: PaletteView::of(underline_facade::themes::palette(effective.theme)),
            input_mode: effective.input_mode.label().to_owned(),
            show_line_numbers: effective.show_line_numbers,
            line_number_side: effective.line_number_side.label().to_owned(),
            show_minimap: effective.show_minimap,
            copy_on_select: effective.copy_on_select,
            git_sync_interval: effective.git_sync_interval,
            agent_access: effective.agent_access.label().to_owned(),
            agent_access_lowered: effective.agent_access_lowered,
            agent_access_refused: effective.agent_access_refused,
            overview_width: effective.overview_width,
            tool_panel_width: effective.tool_panel_width,
            show_menu_bar: effective.show_menu_bar,
            blink_found_in_minimap: effective.blink_found_in_minimap,
            width_generation,
        }
    }
}

/// The names of all themes, in the order of the Theme menu (M-03.6).
#[must_use]
pub fn theme_names() -> Vec<String> {
    Theme::ALL.iter().map(|t| t.label().to_owned()).collect()
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    /// A fresh, empty folder for one test.
    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "underline-session-settings-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn user_file(dir: &Path) -> PathBuf {
        dir.join("home").join(SETTINGS_FILE_NAME)
    }

    fn state_in(dir: &Path) -> SettingsState {
        fs::create_dir_all(dir.join("home")).unwrap();
        SettingsState::with_user_file(Some(user_file(dir)), Instant::now())
    }

    #[test]
    fn uc010_load_without_a_file_gives_the_defaults() {
        let dir = temp_dir("defaults");

        let mut state = state_in(&dir);

        assert_eq!(state.effective(), &Effective::default());
        assert!(state.take_messages().is_empty());
    }

    #[test]
    fn uc010_load_reads_the_user_file() {
        let dir = temp_dir("reads");
        fs::create_dir_all(dir.join("home")).unwrap();
        fs::write(user_file(&dir), "theme: Nord\nshow_menu_bar: true\n").unwrap();

        let state = state_in(&dir);

        assert_eq!(state.effective().theme, Theme::Nord);
        assert!(state.effective().show_menu_bar);
    }

    #[test]
    fn uc010_a6_a_broken_user_file_keeps_the_defaults_and_says_so() {
        let dir = temp_dir("broken");
        fs::create_dir_all(dir.join("home")).unwrap();
        fs::write(user_file(&dir), "theme: Purple\n").unwrap();

        let mut state = state_in(&dir);

        assert_eq!(state.effective().theme, Theme::Light);
        let messages = state.take_messages();
        assert_eq!(messages.len(), 1);
        assert!(
            messages[0].starts_with("The settings keep their last values: "),
            "{messages:?}"
        );
    }

    #[test]
    fn uc010_m20_5_quick_change_applies_and_goes_to_the_user_file() {
        let dir = temp_dir("quick-user");
        let mut state = state_in(&dir);

        state.quick_change(SettingKey::Theme, "Dracula".into());

        assert_eq!(state.effective().theme, Theme::Dracula);
        assert_eq!(
            fs::read_to_string(user_file(&dir)).unwrap(),
            "theme: Dracula\n"
        );
        assert!(state.take_messages().is_empty());
    }

    #[test]
    fn uc010_m20_5_quick_change_goes_to_the_workspace_file_when_it_sets_the_key() {
        let dir = temp_dir("quick-workspace");
        let root = dir.join("ws");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join(SETTINGS_FILE_NAME), "show_menu_bar: false\n").unwrap();
        let mut state = state_in(&dir);
        state.open_workspace(&root, &Git::new());

        state.quick_change(SettingKey::ShowMenuBar, "true".into());
        state.quick_change(SettingKey::Theme, "Nord".into());

        assert!(state.effective().show_menu_bar);
        assert_eq!(
            fs::read_to_string(root.join(SETTINGS_FILE_NAME)).unwrap(),
            "show_menu_bar: true\n"
        );
        assert_eq!(
            fs::read_to_string(user_file(&dir)).unwrap(),
            "theme: Nord\n"
        );
    }

    #[test]
    fn uc010_m20_5_a_value_that_is_not_allowed_is_reported() {
        let dir = temp_dir("not-allowed");
        let mut state = state_in(&dir);

        state.quick_change(SettingKey::Theme, "Purple".into());

        assert_eq!(state.effective().theme, Theme::Light);
        assert_eq!(state.take_messages().len(), 1);
        assert!(!user_file(&dir).exists());
    }

    #[test]
    fn uc010_a7_without_a_home_directory_the_change_applies_and_says_so() {
        let mut state = SettingsState::with_user_file(None, Instant::now());

        state.quick_change(SettingKey::Theme, "Nord".into());

        assert_eq!(state.effective().theme, Theme::Nord);
        assert_eq!(
            state.take_messages(),
            ["There is no home directory to store the setting in"]
        );
    }

    #[test]
    fn uc010_a6_a_broken_file_is_not_written() {
        let dir = temp_dir("broken-not-written");
        fs::create_dir_all(dir.join("home")).unwrap();
        fs::write(user_file(&dir), "theme: Purple\n").unwrap();
        let mut state = state_in(&dir);
        state.take_messages();

        state.quick_change(SettingKey::ShowMenuBar, "true".into());

        assert!(state.effective().show_menu_bar);
        assert_eq!(
            state.take_messages(),
            [format!(
                "{} has an error; the setting applies until Underline is closed and is stored once the file is corrected",
                user_file(&dir).display()
            )]
        );
        assert_eq!(
            fs::read_to_string(user_file(&dir)).unwrap(),
            "theme: Purple\n"
        );
    }

    #[test]
    fn uc010_a7_a_file_that_cannot_be_written_keeps_the_change_for_the_session() {
        let dir = temp_dir("unwritable");
        // The file's folder is missing: reading finds no file, writing fails.
        let path = dir.join("missing").join(SETTINGS_FILE_NAME);
        let mut state = SettingsState::with_user_file(Some(path), Instant::now());
        state.take_messages();

        state.quick_change(SettingKey::Theme, "Nord".into());

        assert_eq!(state.effective().theme, Theme::Nord);
        let messages = state.take_messages();
        assert_eq!(messages.len(), 1);
        assert!(
            messages[0].starts_with(
                "The setting could not be stored and applies until Underline is closed: "
            ),
            "{messages:?}"
        );
    }

    #[test]
    fn uc010_a5_m20_6_the_user_file_is_read_again_every_500_ms() {
        let dir = temp_dir("poll");
        let start = Instant::now();
        fs::create_dir_all(dir.join("home")).unwrap();
        let mut state = SettingsState::with_user_file(Some(user_file(&dir)), start);
        fs::write(user_file(&dir), "theme: Gruvbox\n").unwrap();

        state.poll(start + Duration::from_millis(499));
        assert_eq!(state.effective().theme, Theme::Light);

        state.poll(start + USER_SETTINGS_POLL);
        assert_eq!(state.effective().theme, Theme::Gruvbox);
    }

    #[test]
    fn q03_m05_1_reading_the_user_file_again_resets_the_panel_widths() {
        let dir = temp_dir("width-user");
        let start = Instant::now();
        fs::create_dir_all(dir.join("home")).unwrap();
        let mut state = SettingsState::with_user_file(Some(user_file(&dir)), start);
        assert_eq!(state.width_generation(), 0);

        state.poll(start + USER_SETTINGS_POLL);
        assert_eq!(state.width_generation(), 0, "an unchanged file is not read");

        fs::write(user_file(&dir), "overview_width: 300\n").unwrap();
        state.poll(start + USER_SETTINGS_POLL * 2);

        assert_eq!(state.width_generation(), 1);
        assert_eq!(state.view().width_generation, 1);
        assert_eq!(state.view().overview_width, 300);
    }

    #[test]
    fn q03_m05_1_a_quick_change_keeps_the_panel_widths() {
        let dir = temp_dir("width-quick");
        let mut state = state_in(&dir);

        state.quick_change(SettingKey::OverviewWidth, "320".into());

        assert_eq!(state.effective().overview_width, 320);
        assert_eq!(state.width_generation(), 0);
    }

    #[test]
    fn q03_reading_the_workspace_file_again_resets_the_panel_widths() {
        let dir = temp_dir("width-workspace");
        let root = dir.join("ws");
        fs::create_dir_all(&root).unwrap();
        let mut state = state_in(&dir);
        state.open_workspace(&root, &Git::new());
        assert_eq!(state.width_generation(), 0, "opening is no reading again");
        fs::write(root.join(SETTINGS_FILE_NAME), "overview_width: 260\n").unwrap();

        state.reload_workspace();

        assert_eq!(state.width_generation(), 1);
        assert_eq!(state.effective().overview_width, 260);
    }

    #[test]
    fn uc010_m20_6_an_own_change_is_not_read_again() {
        let dir = temp_dir("own-change");
        let start = Instant::now();
        let mut state = state_in(&dir);
        state.quick_change(SettingKey::Theme, "Nord".into());

        state.poll(start + Duration::from_secs(1));

        assert_eq!(state.effective().theme, Theme::Nord);
        assert!(state.take_messages().is_empty());
    }

    #[test]
    fn uc010_br008_a_workspace_asking_for_more_agent_access_is_reported() {
        let dir = temp_dir("refused");
        let root = dir.join("ws");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join(SETTINGS_FILE_NAME), "agent_access: read only\n").unwrap();
        let mut state = state_in(&dir);

        state.open_workspace(&root, &Git::new());

        assert!(state.effective().agent_access_refused);
        assert_eq!(state.take_messages(), [agent_access_refused_message()]);
        assert_eq!(state.user_agent_access(), AgentAccess::Off);
        assert_eq!(state.workspace_agent_access(), Some(AgentAccess::ReadOnly));
    }

    #[test]
    fn uc010_a5_reload_workspace_reads_the_changed_file() {
        let dir = temp_dir("reload");
        let root = dir.join("ws");
        fs::create_dir_all(&root).unwrap();
        let mut state = state_in(&dir);
        state.open_workspace(&root, &Git::new());
        fs::write(root.join(SETTINGS_FILE_NAME), "theme: Ubuntu\n").unwrap();

        state.reload_workspace();

        assert_eq!(state.effective().theme, Theme::Ubuntu);
        assert_eq!(state.scope_of(SettingKey::Theme), Scope::Workspace);
        assert_eq!(state.scope_of(SettingKey::ShowMenuBar), Scope::Me);
    }

    #[test]
    fn uc010_a6_a_broken_workspace_file_keeps_the_user_values_and_says_so() {
        let dir = temp_dir("broken-workspace");
        let root = dir.join("ws");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join(SETTINGS_FILE_NAME), "theme: Purple\n").unwrap();
        fs::create_dir_all(dir.join("home")).unwrap();
        fs::write(user_file(&dir), "theme: Nord\n").unwrap();
        let mut state = state_in(&dir);

        state.open_workspace(&root, &Git::new());

        assert_eq!(state.effective().theme, Theme::Nord);
        let messages = state.take_messages();
        assert_eq!(messages.len(), 1);
        assert!(
            messages[0].starts_with("The settings keep their last values: "),
            "{messages:?}"
        );
    }

    #[test]
    fn uc010_a5_a6_a_workspace_file_broken_outside_keeps_its_last_values() {
        let dir = temp_dir("reload-broken");
        let root = dir.join("ws");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join(SETTINGS_FILE_NAME), "theme: Ubuntu\n").unwrap();
        let mut state = state_in(&dir);
        state.open_workspace(&root, &Git::new());
        fs::write(root.join(SETTINGS_FILE_NAME), "theme: [\n").unwrap();

        state.reload_workspace();

        assert_eq!(state.effective().theme, Theme::Ubuntu);
        let messages = state.take_messages();
        assert_eq!(messages.len(), 1);
        assert!(
            messages[0].starts_with("The settings keep their last values: "),
            "{messages:?}"
        );
        // A1/A6: a change now applies but leaves the broken file alone.
        state.quick_change(SettingKey::Theme, "Nord".into());
        assert_eq!(state.effective().theme, Theme::Nord);
        assert_eq!(
            fs::read_to_string(root.join(SETTINGS_FILE_NAME)).unwrap(),
            "theme: [\n"
        );
    }

    #[test]
    fn m01_6_the_view_carries_the_theme_and_its_palette() {
        let effective = Effective {
            theme: Theme::GitHubDark,
            show_menu_bar: true,
            ..Effective::default()
        };

        let view = SettingsView::of(&effective, 0);

        assert_eq!(view.theme, "GitHub Dark");
        assert!(view.show_menu_bar);
        assert!(view.palette.dark);
        assert_eq!(view.palette.background, "#0d1117");
    }

    #[test]
    fn m03_6_theme_names_follow_the_order_of_the_themes() {
        let names = theme_names();

        assert_eq!(names.len(), 11);
        assert_eq!(names[0], "Light");
        assert_eq!(names[2], "GitHub Light");
    }

    #[test]
    fn uc010_setting_names_serialize_as_the_file_keys() {
        let names = [
            SettingName::DefaultFormat,
            SettingName::Theme,
            SettingName::InputMode,
            SettingName::ShowLineNumbers,
            SettingName::LineNumberSide,
            SettingName::ShowMinimap,
            SettingName::CopyOnSelect,
            SettingName::GitSyncInterval,
            SettingName::AgentAccess,
            SettingName::OverviewWidth,
            SettingName::ToolPanelWidth,
            SettingName::ShowMenuBar,
            SettingName::BlinkFoundInMinimap,
        ];
        let keys: Vec<SettingKey> = names.iter().map(|&n| SettingKey::from(n)).collect();

        assert_eq!(keys, SettingKey::ALL);
        for name in names {
            assert_eq!(
                serde_json::to_value(name).unwrap(),
                serde_json::json!(SettingKey::from(name).key())
            );
        }
    }
}

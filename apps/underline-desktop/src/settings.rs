//! UC-010 Adjust Settings in the window: the user settings and the settings
//! of the open workspace, the settings in effect, where a change is stored,
//! watching the user settings file, and the settings window.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

use eframe::egui;
use underline_core::{
    store_setting, user_settings_path, AgentAccess, Effective, Git, InputMode, LineSide,
    NoteFormat, SettingKey, Settings, SettingsError, Theme, WorkspaceSettings, SETTINGS_FILE_NAME,
};
use underline_facade::themes::Palette;

/// The two parts of the settings (UC-010 step 2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    /// "For me": the user settings file.
    Me,
    /// "This workspace": the workspace settings file.
    Workspace,
}

/// The settings of the window.
pub struct SettingsState {
    user: Settings,
    user_path: Option<PathBuf>,
    /// When the user settings file was read last, and when it was looked at.
    user_seen: Option<SystemTime>,
    user_checked: Instant,
    /// The workspace's root and settings, and whether its settings file is
    /// shared through git (BR-007).
    workspace: Option<(PathBuf, Settings, bool)>,
    /// Files that cannot be read now, which are not written (A6).
    broken: Vec<PathBuf>,
    pub effective: Effective,
    /// Whether the settings window is open, and its part.
    pub open: bool,
    scope: Scope,
    /// Messages for the status bar.
    messages: Vec<String>,
    /// Counts the widths set from the settings, so the panels take them.
    pub width_generation: u64,
}

impl SettingsState {
    /// Reads the user settings file at start; one with an error leaves the
    /// built-in defaults in effect (A6).
    pub fn load() -> Self {
        let mut state = Self {
            user: Settings::default(),
            user_path: user_settings_path(),
            user_seen: None,
            user_checked: Instant::now(),
            workspace: None,
            broken: Vec::new(),
            effective: Effective::default(),
            open: false,
            scope: Scope::Me,
            messages: Vec::new(),
            width_generation: 0,
        };
        state.read_user();
        state.recompute();
        state
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
            Err(err) => self.broken_file(&path, err),
        }
    }

    fn broken_file(&mut self, path: &Path, err: SettingsError) {
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
                self.broken_file(&path, err);
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
                self.width_generation += 1;
                self.recompute();
            }
            Err(err) => self.broken_file(&path, err),
        }
    }

    /// UC-010 A5: looks at the user settings file about once a second and
    /// reads it again when it changed.
    pub fn poll(&mut self) {
        if self.user_checked.elapsed() < Duration::from_millis(500) {
            return;
        }
        self.user_checked = Instant::now();
        let Some(path) = &self.user_path else {
            return;
        };
        if modified(path) != self.user_seen {
            self.read_user();
            self.width_generation += 1;
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
    pub fn scope_of(&self, key: SettingKey) -> Scope {
        match &self.workspace {
            Some((_, settings, _)) if settings.sets(key) => Scope::Workspace,
            _ => Scope::Me,
        }
    }

    /// Changes `key` in `scope` (A1 to A4): in effect at once, stored within
    /// 1 second (BR-004, BR-005); a file with an error or one that cannot be
    /// written keeps the change for this session and says so (A6, A7).
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

    /// UC-017 BR-002: the agent access the user settings allow.
    pub fn user_agent_access(&self) -> AgentAccess {
        self.user.agent_access.unwrap_or_default()
    }

    /// UC-017 A1: the level the workspace settings lower agent access to.
    pub fn workspace_agent_access(&self) -> Option<AgentAccess> {
        self.workspace.as_ref().and_then(|(_, s, _)| s.agent_access)
    }

    /// UC-017 A1: opens the settings window at `scope`.
    pub fn open_at(&mut self, scope: Scope) {
        if scope == Scope::Me || self.workspace.is_some() {
            self.scope = scope;
        }
        self.open = true;
    }

    /// Changes a setting from outside the settings window (A3, A4).
    pub fn quick_change(&mut self, key: SettingKey, value: String) {
        let scope = self.scope_of(key);
        self.change(scope, key, Some(value));
    }

    /// The settings window (UC-010 step 2): "For me" and "This workspace".
    /// `derived` is the note format the workspace's notes give.
    /// `extra` adds the section "Agents" (UC-016) at the end.
    pub fn show_window(
        &mut self,
        ctx: &egui::Context,
        derived: Option<NoteFormat>,
        extra: impl FnOnce(&mut egui::Ui),
    ) {
        if !self.open {
            return;
        }
        let mut open = true;
        let mut changes = Vec::new();
        egui::Window::new("Settings")
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_width(460.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.scope, Scope::Me, "For me");
                    if self.workspace.is_some() {
                        ui.selectable_value(&mut self.scope, Scope::Workspace, "This workspace");
                    } else {
                        self.scope = Scope::Me;
                    }
                });
                ui.separator();
                let workspace = self.workspace.as_ref().map(|(_, s, shared)| (s.clone(), *shared));
                match self.scope {
                    Scope::Me => {
                        ui.weak("These settings apply in every workspace.");
                    }
                    Scope::Workspace => {
                        ui.weak("These settings apply in this workspace only.");
                        if workspace.as_ref().is_some_and(|(_, shared)| *shared) {
                            ui.colored_label(
                                ui.visuals().warn_fg_color,
                                "These values are in underline.yaml of the repository and apply to everyone who uses it.",
                            );
                        }
                    }
                }
                ui.add_space(6.0);
                let shown = match self.scope {
                    Scope::Me => Effective::of(&self.user, None),
                    Scope::Workspace => self.effective.clone(),
                };
                egui::Grid::new("settings-grid")
                    .num_columns(3)
                    .spacing([12.0, 8.0])
                    .show(ui, |ui| {
                        for key in SettingKey::ALL {
                            if key == SettingKey::DefaultFormat && self.scope == Scope::Me {
                                continue;
                            }
                            ui.label(label(key));
                            let change = value_widget(
                                ui,
                                key,
                                &shown,
                                self.scope,
                                derived,
                                &workspace,
                                self.user_agent_access(),
                            );
                            if let Some(change) = change {
                                changes.push(change);
                            }
                            match (&self.scope, &workspace) {
                                (Scope::Workspace, Some((settings, _))) => {
                                    if settings.sets(key) {
                                        if key == SettingKey::DefaultFormat {
                                            ui.weak("own");
                                        } else if ui
                                            .button("As for me")
                                            .on_hover_text("Use the setting from \"For me\" again")
                                            .clicked()
                                        {
                                            changes.push((key, None));
                                        }
                                    } else {
                                        ui.weak("as for me");
                                    }
                                }
                                (Scope::Me, Some(_))
                                    if key == SettingKey::AgentAccess
                                        && self.effective.agent_access_refused =>
                                {
                                    ui.weak("this workspace asks for more; not followed");
                                }
                                (Scope::Me, Some((settings, _))) if settings.sets(key) => {
                                    ui.weak("this workspace uses its own value");
                                }
                                _ => {
                                    ui.label("");
                                }
                            }
                            ui.end_row();
                        }
                    });
                ui.add_space(8.0);
                egui::CollapsingHeader::new("Agents")
                    .id_salt("agents")
                    .default_open(true)
                    .show(ui, extra);
            });
        for (key, value) in changes {
            self.change(self.scope, key, value);
            if matches!(key, SettingKey::OverviewWidth | SettingKey::ToolPanelWidth) {
                self.width_generation += 1;
            }
        }
        self.open = open;
    }

    /// The palette of the theme in effect.
    pub fn palette(&self) -> &'static Palette {
        underline_facade::themes::palette(self.effective.theme)
    }
}

/// UC-010 BR-008, UC-017 A1: the workspace asks for more agent access than
/// the user allows.
pub fn agent_access_refused_message() -> String {
    "This workspace's settings ask for more agent access than your own settings allow; that is not followed (only your own settings can raise it)".into()
}

fn modified(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

fn label(key: SettingKey) -> &'static str {
    match key {
        SettingKey::DefaultFormat => "Default format",
        SettingKey::Theme => "Theme",
        SettingKey::InputMode => "Input mode",
        SettingKey::ShowLineNumbers => "Line numbers",
        SettingKey::LineNumberSide => "Line number side",
        SettingKey::ShowMinimap => "Minimap",
        SettingKey::CopyOnSelect => "Copy on select",
        SettingKey::GitSyncInterval => "Git sync every (minutes)",
        SettingKey::AgentAccess => "Agent access",
        SettingKey::OverviewWidth => "Note overview width",
        SettingKey::ToolPanelWidth => "Tool panel width",
        SettingKey::ShowMenuBar => "Menu bar",
        SettingKey::BlinkFoundInMinimap => "Search hits in the minimap",
    }
}

type Change = (SettingKey, Option<String>);

/// The control of one setting, showing the value in effect; gives the new
/// value when the user changes it.
fn value_widget(
    ui: &mut egui::Ui,
    key: SettingKey,
    shown: &Effective,
    scope: Scope,
    derived: Option<NoteFormat>,
    workspace: &Option<(Settings, bool)>,
    user_access: AgentAccess,
) -> Option<Change> {
    let id = ui.make_persistent_id(("setting", key.key(), scope == Scope::Me));
    let some = |text: &str| Some((key, Some(text.to_owned())));
    match key {
        SettingKey::DefaultFormat => {
            let own = workspace.as_ref().and_then(|(s, _)| s.default_format);
            let name = |f: NoteFormat| f.name().to_owned();
            let text = match (own, derived) {
                (Some(format), _) => name(format),
                (None, Some(format)) => format!("{} (from the notes)", name(format)),
                (None, None) => "Follow the notes".to_owned(),
            };
            let mut change = None;
            egui::ComboBox::from_id_salt(id)
                .selected_text(text)
                .show_ui(ui, |ui| {
                    for format in NoteFormat::ALL {
                        if ui
                            .selectable_label(own == Some(format), name(format))
                            .clicked()
                        {
                            change = some(&format.name().to_lowercase());
                        }
                    }
                    if ui
                        .selectable_label(own.is_none(), "Follow the notes")
                        .clicked()
                    {
                        change = Some((key, None));
                    }
                });
            change
        }
        SettingKey::Theme => choose(ui, id, key, shown.theme, Theme::ALL),
        SettingKey::InputMode => choose(ui, id, key, shown.input_mode, InputMode::ALL),
        SettingKey::LineNumberSide => choose(ui, id, key, shown.line_number_side, LineSide::ALL),
        SettingKey::ShowLineNumbers => check(
            ui,
            key,
            shown.show_line_numbers,
            "Shown, in the source view",
        ),
        SettingKey::ShowMinimap => check(ui, key, shown.show_minimap, "Shown"),
        SettingKey::CopyOnSelect => check(ui, key, shown.copy_on_select, "On"),
        SettingKey::AgentAccess => match scope {
            Scope::Me => choose(ui, id, key, shown.agent_access, AgentAccess::ALL),
            // UC-010 BR-008, UC-017 BR-002: a workspace can only lower it, so
            // only the levels up to the user's are offered.
            Scope::Workspace => {
                let lower: Vec<AgentAccess> = AgentAccess::ALL
                    .iter()
                    .copied()
                    .filter(|a| *a <= user_access)
                    .collect();
                choose(ui, id, key, shown.agent_access, &lower)
            }
        },
        SettingKey::GitSyncInterval => number(ui, key, shown.git_sync_interval, 1..=60),
        SettingKey::OverviewWidth => number(ui, key, shown.overview_width, 100..=1000),
        SettingKey::ToolPanelWidth => number(ui, key, shown.tool_panel_width, 100..=1000),
        SettingKey::ShowMenuBar => check(ui, key, shown.show_menu_bar, "Shown"),
        SettingKey::BlinkFoundInMinimap => check(ui, key, shown.blink_found_in_minimap, "Blinking"),
    }
}

fn choose<T: Copy + PartialEq + std::fmt::Display>(
    ui: &mut egui::Ui,
    id: egui::Id,
    key: SettingKey,
    current: T,
    all: &[T],
) -> Option<Change> {
    let mut change = None;
    egui::ComboBox::from_id_salt(id)
        .selected_text(current.to_string())
        .show_ui(ui, |ui| {
            for value in all {
                if ui
                    .selectable_label(*value == current, value.to_string())
                    .clicked()
                    && *value != current
                {
                    change = Some((key, Some(value.to_string())));
                }
            }
        });
    change
}

fn check(ui: &mut egui::Ui, key: SettingKey, current: bool, text: &str) -> Option<Change> {
    let mut value = current;
    ui.checkbox(&mut value, text)
        .changed()
        .then(|| (key, Some(value.to_string())))
}

fn number(
    ui: &mut egui::Ui,
    key: SettingKey,
    current: u32,
    range: std::ops::RangeInclusive<u32>,
) -> Option<Change> {
    let mut value = current;
    ui.add(egui::DragValue::new(&mut value).range(range))
        .changed()
        .then(|| (key, Some(value.to_string())))
}

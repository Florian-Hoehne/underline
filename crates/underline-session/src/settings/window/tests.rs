//! Tests of the settings window's rows and changes (UC-010, inventory M-20.2
//! to M-20.5), against `show_window` and `value_widget` of the egui app.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use underline_core::{AgentAccess, Git, NoteFormat, SettingKey, Theme, SETTINGS_FILE_NAME};

use super::{SettingChoiceView, SettingControlView, SettingNoteView, SettingRowView};
use crate::settings::{Scope, SettingName, SettingsState};

/// A fresh, empty folder for one test.
fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "underline-session-settings-window-{name}-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("home")).unwrap();
    dir
}

fn user_file(dir: &Path) -> PathBuf {
    dir.join("home").join(SETTINGS_FILE_NAME)
}

/// The settings with the user file `user` (if any) and the workspace
/// `dir/ws` with the file `workspace` (if any).
fn state(dir: &Path, user: Option<&str>, workspace: Option<&str>) -> (SettingsState, PathBuf) {
    if let Some(text) = user {
        fs::write(user_file(dir), text).unwrap();
    }
    let root = dir.join("ws");
    fs::create_dir_all(&root).unwrap();
    if let Some(text) = workspace {
        fs::write(root.join(SETTINGS_FILE_NAME), text).unwrap();
    }
    let mut state = SettingsState::with_user_file(Some(user_file(dir)), Instant::now());
    state.open_workspace(&root, &Git::new());
    state.take_messages();
    (state, root)
}

fn row(rows: &[SettingRowView], key: SettingName) -> &SettingRowView {
    rows.iter().find(|row| row.key == key).expect("the row")
}

fn shown(row: &SettingRowView) -> String {
    match &row.control {
        SettingControlView::Choice { shown, .. } => shown.clone(),
        SettingControlView::Check { on, .. } => on.to_string(),
        SettingControlView::Number { value, .. } => value.to_string(),
    }
}

fn note(row: &SettingRowView) -> Option<&str> {
    match &row.note {
        Some(SettingNoteView::Text { text }) => Some(text),
        Some(SettingNoteView::AsForMe { label, .. }) => Some(label),
        None => None,
    }
}

fn choices(row: &SettingRowView) -> Vec<&SettingChoiceView> {
    match &row.control {
        SettingControlView::Choice { choices, .. } => choices.iter().collect(),
        _ => Vec::new(),
    }
}

#[test]
fn uc010_a9_m20_2_without_a_workspace_only_for_me() {
    let dir = temp_dir("no-workspace");
    let state = SettingsState::with_user_file(Some(user_file(&dir)), Instant::now());

    let view = state.window_view(None);

    assert!(!view.workspace);
    assert!(!view.shared);
    assert!(view.this_workspace.is_empty());
    assert_eq!(view.me.len(), 12, "every setting but the default format");
    assert!(view.me.iter().all(|row| row.note.is_none()));
}

#[test]
fn uc010_m20_3_the_rows_and_their_names_as_in_egui() {
    let dir = temp_dir("labels");
    let (state, _) = state(&dir, None, None);

    let view = state.window_view(Some(NoteFormat::Markdown));

    let labels: Vec<&str> = view
        .this_workspace
        .iter()
        .map(|row| row.label.as_str())
        .collect();
    assert_eq!(
        labels,
        [
            "Default format",
            "Theme",
            "Input mode",
            "Line numbers",
            "Line number side",
            "Minimap",
            "Copy on select",
            "Git sync every (minutes)",
            "Agent access",
            "Note overview width",
            "Tool panel width",
            "Menu bar",
            "Search hits in the minimap",
        ]
    );
    assert_eq!(view.me.len(), 12);
    assert_eq!(view.me[0].label, "Theme");
    let texts: Vec<(SettingName, String)> = view
        .me
        .iter()
        .filter_map(|row| match &row.control {
            SettingControlView::Check { text, .. } => Some((row.key, text.clone())),
            _ => None,
        })
        .collect();
    assert_eq!(
        texts,
        [
            (
                SettingName::ShowLineNumbers,
                "Shown, in the source view".to_owned()
            ),
            (SettingName::ShowMinimap, "Shown".to_owned()),
            (SettingName::CopyOnSelect, "On".to_owned()),
            (SettingName::ShowMenuBar, "Shown".to_owned()),
            (SettingName::BlinkFoundInMinimap, "Blinking".to_owned()),
        ]
    );
}

#[test]
fn tc010_02_m20_3_every_setting_shows_its_built_in_default() {
    let dir = temp_dir("defaults");
    let (state, _) = state(&dir, None, None);

    let view = state.window_view(Some(NoteFormat::Markdown));

    let values: Vec<String> = view.me.iter().map(shown).collect();
    assert_eq!(
        values,
        [
            "Light", "Normal", "true", "Left", "true", "true", "5", "Off", "200", "240", "false",
            "true"
        ]
    );
    assert!(
        !user_file(&dir).exists(),
        "TC-010-02 step 3: nothing written"
    );
    let themes = choices(row(&view.me, SettingName::Theme));
    assert_eq!(themes.len(), 11);
    assert!(themes[0].selected && themes[0].label == "Light");
    assert_eq!(themes[2].value.as_deref(), Some("GitHub Light"));
    assert!(matches!(
        row(&view.me, SettingName::GitSyncInterval).control,
        SettingControlView::Number {
            min: 1,
            max: 60,
            ..
        }
    ));
    assert!(matches!(
        row(&view.me, SettingName::OverviewWidth).control,
        SettingControlView::Number {
            min: 100,
            max: 1000,
            ..
        }
    ));
}

#[test]
fn tc010_04_m20_4_the_third_column_tells_whose_value_applies() {
    let dir = temp_dir("notes");
    let (state, _) = state(&dir, Some("theme: Gruvbox\n"), Some("theme: Dracula\n"));

    let view = state.window_view(None);

    let theme = row(&view.this_workspace, SettingName::Theme);
    assert_eq!(shown(theme), "Dracula");
    assert_eq!(
        theme.note,
        Some(SettingNoteView::AsForMe {
            label: "As for me".into(),
            hint: "Use the setting from \"For me\" again".into(),
        })
    );
    assert_eq!(
        note(row(&view.this_workspace, SettingName::InputMode)),
        Some("as for me")
    );
    let mine = row(&view.me, SettingName::Theme);
    assert_eq!(shown(mine), "Gruvbox", "For me shows the user's own value");
    assert_eq!(note(mine), Some("this workspace uses its own value"));
    assert_eq!(note(row(&view.me, SettingName::InputMode)), None);
}

#[test]
fn uc010_a8_m20_4_the_default_format_shows_where_it_comes_from() {
    let dir = temp_dir("format");
    let (mut state, root) = state(&dir, None, None);

    let derived = state.window_view(Some(NoteFormat::Markdown));
    let format = row(&derived.this_workspace, SettingName::DefaultFormat);
    assert_eq!(shown(format), "Markdown (from the notes)");
    assert_eq!(note(format), Some("as for me"));
    let labels: Vec<(&str, Option<&str>, bool)> = choices(format)
        .iter()
        .map(|c| (c.label.as_str(), c.value.as_deref(), c.selected))
        .collect();
    assert_eq!(
        labels,
        [
            ("Markdown", Some("markdown"), false),
            ("AsciiDoc", Some("asciidoc"), false),
            ("Follow the notes", None, true),
        ]
    );
    assert_eq!(
        shown(row(
            &state.window_view(None).this_workspace,
            SettingName::DefaultFormat
        )),
        "Follow the notes"
    );

    state.window_change(
        Scope::Workspace,
        SettingKey::DefaultFormat,
        Some("asciidoc".into()),
    );

    let own = state.window_view(Some(NoteFormat::Markdown));
    let format = row(&own.this_workspace, SettingName::DefaultFormat);
    assert_eq!(shown(format), "AsciiDoc");
    assert_eq!(note(format), Some("own"), "no button for the format");
    assert_eq!(
        fs::read_to_string(root.join(SETTINGS_FILE_NAME)).unwrap(),
        "default_format: asciidoc\n"
    );

    // A8 step 2: "Follow the notes" removes it again.
    state.window_change(Scope::Workspace, SettingKey::DefaultFormat, None);

    assert_eq!(
        fs::read_to_string(root.join(SETTINGS_FILE_NAME)).unwrap(),
        ""
    );
}

#[test]
fn tc010_04_uc010_a1_a_change_for_this_workspace_goes_to_its_file() {
    let dir = temp_dir("a1");
    let (mut state, root) = state(&dir, None, None);

    state.window_change(Scope::Workspace, SettingKey::Theme, Some("Dracula".into()));

    assert_eq!(state.effective().theme, Theme::Dracula);
    assert_eq!(
        fs::read_to_string(root.join(SETTINGS_FILE_NAME)).unwrap(),
        "theme: Dracula\n"
    );
    assert!(!user_file(&dir).exists(), "the user file is unchanged");
    let view = state.window_view(None);
    assert_eq!(
        note(row(&view.me, SettingName::Theme)),
        Some("this workspace uses its own value")
    );

    // TC-010-04 step 5: For me changes, the workspace keeps its own.
    state.window_change(Scope::Me, SettingKey::Theme, Some("Gruvbox".into()));

    assert_eq!(state.effective().theme, Theme::Dracula);
    assert_eq!(
        fs::read_to_string(user_file(&dir)).unwrap(),
        "theme: Gruvbox\n"
    );
}

#[test]
fn tc010_05_uc010_a2_as_for_me_removes_the_workspace_value() {
    let dir = temp_dir("a2");
    let (mut state, root) = state(
        &dir,
        Some("theme: Gruvbox\n"),
        Some("# shared\ntheme: Dracula\nshow_minimap: false\n"),
    );

    state.window_change(Scope::Workspace, SettingKey::Theme, None);

    assert_eq!(state.effective().theme, Theme::Gruvbox);
    assert_eq!(
        fs::read_to_string(root.join(SETTINGS_FILE_NAME)).unwrap(),
        "# shared\nshow_minimap: false\n"
    );
    assert_eq!(
        note(row(
            &state.window_view(None).this_workspace,
            SettingName::Theme
        )),
        Some("as for me")
    );
}

#[test]
fn m20_5_choosing_the_value_shown_is_no_change_as_in_egui() {
    let dir = temp_dir("same");
    let (mut state, root) = state(&dir, Some("theme: Nord\n"), None);
    let revision = state.revision();

    state.window_change(Scope::Workspace, SettingKey::Theme, Some("Nord".into()));
    state.window_change(Scope::Me, SettingKey::ShowMinimap, Some("true".into()));

    assert!(!root.join(SETTINGS_FILE_NAME).exists());
    assert_eq!(
        fs::read_to_string(user_file(&dir)).unwrap(),
        "theme: Nord\n"
    );
    assert_eq!(state.revision(), revision);
}

#[test]
fn m20_3_numbers_stay_within_their_range() {
    let dir = temp_dir("range");
    let (mut state, _) = state(&dir, None, None);

    state.window_change(Scope::Me, SettingKey::GitSyncInterval, Some("0".into()));
    state.window_change(Scope::Me, SettingKey::OverviewWidth, Some("5000".into()));

    assert_eq!(state.effective().git_sync_interval, 1);
    assert_eq!(state.effective().overview_width, 1000);
    assert_eq!(
        fs::read_to_string(user_file(&dir)).unwrap(),
        "git_sync_interval: 1\noverview_width: 1000\n"
    );
}

#[test]
fn m20_5_a_value_that_is_no_number_is_refused_and_said() {
    let dir = temp_dir("no-number");
    let (mut state, _) = state(&dir, None, None);

    state.window_change(Scope::Me, SettingKey::ToolPanelWidth, Some("wide".into()));

    assert_eq!(state.effective().tool_panel_width, 240);
    assert_eq!(state.take_messages().len(), 1);
    assert!(!user_file(&dir).exists());
}

#[test]
fn q03_m20_5_a_width_changed_in_the_window_sets_the_panels() {
    let dir = temp_dir("width");
    let (mut state, _) = state(&dir, None, None);
    let before = state.width_generation();

    state.window_change(Scope::Me, SettingKey::ToolPanelWidth, Some("300".into()));

    assert_eq!(state.width_generation(), before + 1);
    assert_eq!(state.view().tool_panel_width, 300);
    state.window_change(Scope::Me, SettingKey::Theme, Some("Dark".into()));
    assert_eq!(state.width_generation(), before + 1, "only widths");
}

#[test]
fn tc010_15_uc010_br008_a_workspace_only_lowers_agent_access() {
    let dir = temp_dir("agents");
    let (state, _) = state(
        &dir,
        Some("agent_access: read only\n"),
        Some("agent_access: read and write\n"),
    );

    let view = state.window_view(None);

    let workspace = row(&view.this_workspace, SettingName::AgentAccess);
    let offered: Vec<&str> = choices(workspace)
        .iter()
        .map(|c| c.label.as_str())
        .collect();
    assert_eq!(offered, ["Off", "Read only"]);
    assert_eq!(shown(workspace), "Read only");
    let me = row(&view.me, SettingName::AgentAccess);
    assert_eq!(choices(me).len(), 3);
    assert_eq!(note(me), Some("this workspace asks for more; not followed"));
    assert_eq!(state.user_agent_access(), AgentAccess::ReadOnly);
}

#[test]
fn tc010_14_uc010_br007_a_repository_shares_the_workspace_settings() {
    let dir = temp_dir("shared");
    let (_, root) = state(&dir, None, None);
    let git = Git::new();
    let init = std::process::Command::new("git")
        .args(["init", "-q"])
        .current_dir(&root)
        .status();
    if !init.is_ok_and(|status| status.success()) {
        return;
    }
    let mut state = SettingsState::with_user_file(Some(user_file(&dir)), Instant::now());

    state.open_workspace(&root, &git);
    assert!(state.window_view(None).shared);

    fs::write(root.join(".gitignore"), "underline.yaml\n").unwrap();
    state.open_workspace(&root, &git);
    assert!(!state.window_view(None).shared, "ignored, so not shared");
}

#[test]
fn tc010_14_a_workspace_without_git_is_not_shared() {
    let dir = temp_dir("private");
    let (state, _) = state(&dir, None, None);

    assert!(!state.window_view(None).shared);
}

#[test]
fn uc010_a9_this_workspace_changes_nothing_without_a_workspace() {
    let dir = temp_dir("no-workspace-change");
    let mut state = SettingsState::with_user_file(Some(user_file(&dir)), Instant::now());

    state.window_change(Scope::Workspace, SettingKey::Theme, Some("Nord".into()));

    assert_eq!(state.effective().theme, Theme::Light);
    assert!(state.take_messages().is_empty());
    assert!(!user_file(&dir).exists());
}

#[test]
fn uc010_a6_m20_5_a_change_in_the_window_leaves_a_broken_file_alone() {
    let dir = temp_dir("broken");
    let (mut state, _) = state(&dir, Some("theme: Purple\n# mine\n"), None);

    state.window_change(Scope::Me, SettingKey::Theme, Some("Dracula".into()));

    assert_eq!(state.effective().theme, Theme::Dracula);
    assert_eq!(
        state.take_messages(),
        [format!(
            "{} has an error; the setting applies until Underline is closed and is stored once the file is corrected",
            user_file(&dir).display()
        )]
    );
    assert_eq!(
        fs::read_to_string(user_file(&dir)).unwrap(),
        "theme: Purple\n# mine\n"
    );
}

#[test]
fn uc010_scope_names_as_the_window_sends_them() {
    assert_eq!(serde_json::to_value(Scope::Me).unwrap(), "me");
    assert_eq!(serde_json::to_value(Scope::Workspace).unwrap(), "workspace");
    let view = SettingControlView::Number {
        value: 5,
        min: 1,
        max: 60,
    };
    assert_eq!(
        serde_json::to_value(view).unwrap(),
        serde_json::json!({ "kind": "number", "value": 5, "min": 1, "max": 60 })
    );
}

#[test]
fn tc010_15_br008_a_workspace_value_above_the_user_level_is_refused() {
    let dir = temp_dir("agents-raise");
    let (mut state, root) = state(&dir, Some("agent_access: read only\n"), None);

    state.window_change(
        Scope::Workspace,
        SettingKey::AgentAccess,
        Some("Read and write".into()),
    );

    assert_eq!(state.effective().agent_access, AgentAccess::ReadOnly);
    assert!(!root.join(SETTINGS_FILE_NAME).exists());
    assert_eq!(
        state.take_messages(),
        [crate::settings::agent_access_refused_message()]
    );

    // Lowering is allowed.
    state.window_change(
        Scope::Workspace,
        SettingKey::AgentAccess,
        Some("Off".into()),
    );
    assert_eq!(state.effective().agent_access, AgentAccess::Off);
}

#[test]
fn m20_5_a_value_with_a_line_break_writes_no_blank_line() {
    let dir = temp_dir("trim");
    let (mut state, _) = state(&dir, None, None);

    state.window_change(Scope::Me, SettingKey::ShowMinimap, Some("false\n".into()));
    state.window_change(Scope::Me, SettingKey::Theme, Some(" Nord ".into()));

    assert_eq!(
        fs::read_to_string(user_file(&dir)).unwrap(),
        "show_minimap: false\ntheme: Nord\n"
    );
}

#[test]
fn q03_a_refused_width_does_not_set_the_panels() {
    let dir = temp_dir("width-refused");
    let (mut state, _) = state(&dir, None, None);
    let before = state.width_generation();

    state.window_change(Scope::Me, SettingKey::OverviewWidth, Some("wide".into()));

    assert_eq!(state.width_generation(), before);
}

#[test]
fn m20_2_the_revision_grows_with_every_change() {
    let dir = temp_dir("revision");
    let (mut state, _) = state(&dir, None, None);
    let before = state.window_view(None).revision;

    state.window_change(Scope::Me, SettingKey::Theme, Some("Nord".into()));

    assert!(state.window_view(None).revision > before);
}

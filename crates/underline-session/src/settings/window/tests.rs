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
        SettingControlView::Text { value, .. } => value.clone(),
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

    let view = state.window_view(None, &[]);

    assert!(!view.workspace);
    assert!(!view.shared);
    assert!(view.this_workspace.is_empty());
    assert_eq!(view.me.len(), 17, "every setting but the default format");
    assert!(view.me.iter().all(|row| row.note.is_none()));
}

#[test]
fn uc010_m20_3_the_rows_and_their_names_as_in_egui() {
    let dir = temp_dir("labels");
    let (state, _) = state(&dir, None, None);

    let view = state.window_view(Some(NoteFormat::Markdown), &[]);

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
            "Close pairs",
            "Spell check language",
            "Editor zoom (percent)",
            "Soft wrap",
            "Focus mode",
            "Git sync every (minutes)",
            "Agent access",
            "Note overview width",
            "Tool panel width",
            "Menu bar",
            "Search hits in the minimap",
            // TC-010-22: only for a workspace.
            "Template folder",
            "Daily note folder",
            "Daily note template",
        ]
    );
    assert_eq!(view.me.len(), 17);
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
            (SettingName::ClosePairs, "On".to_owned()),
            (SettingName::SoftWrap, "On".to_owned()),
            (SettingName::FocusMode, "On".to_owned()),
            (SettingName::ShowMenuBar, "Shown".to_owned()),
            (SettingName::BlinkFoundInMinimap, "Blinking".to_owned()),
        ]
    );
}

#[test]
fn tc010_02_m20_3_every_setting_shows_its_built_in_default() {
    let dir = temp_dir("defaults");
    let (state, _) = state(&dir, None, None);

    let view = state.window_view(Some(NoteFormat::Markdown), &[]);

    let values: Vec<String> = view.me.iter().map(shown).collect();
    assert_eq!(
        values,
        [
            "Light", "Normal", "true", "Left", "true", "true", "true", "Off", "100", "true",
            "false", "5", "Off", "200", "240", "false", "true"
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

    let view = state.window_view(None, &[]);

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

    let derived = state.window_view(Some(NoteFormat::Markdown), &[]);
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
            &state.window_view(None, &[]).this_workspace,
            SettingName::DefaultFormat
        )),
        "Follow the notes"
    );

    state.window_change(
        Scope::Workspace,
        SettingKey::DefaultFormat,
        Some("asciidoc".into()),
    );

    let own = state.window_view(Some(NoteFormat::Markdown), &[]);
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
    let view = state.window_view(None, &[]);
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
            &state.window_view(None, &[]).this_workspace,
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

    let view = state.window_view(None, &[]);

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
    assert!(state.window_view(None, &[]).shared);

    fs::write(root.join(".gitignore"), "underline.yaml\n").unwrap();
    state.open_workspace(&root, &git);
    assert!(
        !state.window_view(None, &[]).shared,
        "ignored, so not shared"
    );
}

#[test]
fn tc010_14_a_workspace_without_git_is_not_shared() {
    let dir = temp_dir("private");
    let (state, _) = state(&dir, None, None);

    assert!(!state.window_view(None, &[]).shared);
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
        step: 1,
    };
    assert_eq!(
        serde_json::to_value(view).unwrap(),
        serde_json::json!({ "kind": "number", "value": 5, "min": 1, "max": 60, "step": 1 })
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
    let before = state.window_view(None, &[]).revision;

    state.window_change(Scope::Me, SettingKey::Theme, Some("Nord".into()));

    assert!(state.window_view(None, &[]).revision > before);
}

/// TC-010-22 step 1, UC-010 BR-001: "For me" shows close pairs on; turned
/// off it is stored in the user settings file and in effect.
#[test]
fn tc010_22_close_pairs_is_on_at_first_and_can_be_turned_off() {
    let dir = temp_dir("close-pairs");
    let (mut state, _) = state(&dir, None, None);
    let view = state.window_view(Some(NoteFormat::Markdown), &[]);
    assert_eq!(shown(row(&view.me, SettingName::ClosePairs)), "true");
    assert!(state.effective.close_pairs);

    state.window_change(Scope::Me, SettingKey::ClosePairs, Some("false".into()));

    assert!(!state.effective.close_pairs);
    let written = std::fs::read_to_string(user_file(&dir)).expect("the user file");
    assert!(written.contains("close_pairs: false"), "{written}");
}

/// TC-010-18, UC-010 BR-009: the editor zoom is a number from 50 to 300 in
/// steps of 10; a value between two steps goes to the nearest one, and one
/// beyond the range to its end.
#[test]
fn tc010_18_editor_zoom_in_steps_of_ten() {
    let dir = temp_dir("zoom-window");
    let (mut state, _) = state(&dir, None, None);
    let view = state.window_view(None, &[]);
    assert_eq!(
        row(&view.me, SettingName::EditorZoom).control,
        SettingControlView::Number {
            value: 100,
            min: 50,
            max: 300,
            step: 10,
        }
    );

    state.window_change(Scope::Me, SettingKey::EditorZoom, Some("124".into()));
    assert_eq!(state.effective.editor_zoom, 120);
    state.window_change(Scope::Me, SettingKey::EditorZoom, Some("900".into()));
    assert_eq!(state.effective.editor_zoom, 300);

    let written = std::fs::read_to_string(user_file(&dir)).expect("the user file");
    assert_eq!(written, "editor_zoom: 300\n");
}

/// TC-010-19 step 3, TC-010-20 step 2, UC-010 A3, BR-010, BR-011: the
/// focus mode and soft wrap switched outside the window are stored in the
/// user settings file, or in the workspace's when it has its own value.
#[test]
fn tc010_19_20_focus_mode_and_soft_wrap_are_stored_as_quick_switches() {
    let dir = temp_dir("focus-wrap");
    let (mut state, root) = state(&dir, None, Some("soft_wrap: true\n"));

    state.quick_change(SettingKey::FocusMode, "true".into());
    state.quick_change(SettingKey::SoftWrap, "false".into());

    assert!(state.effective.focus_mode);
    assert!(!state.effective.soft_wrap);
    let user = std::fs::read_to_string(user_file(&dir)).expect("the user file");
    assert_eq!(user, "focus_mode: true\n");
    let workspace =
        std::fs::read_to_string(root.join(underline_core::SETTINGS_FILE_NAME)).expect("workspace");
    assert_eq!(workspace, "soft_wrap: false\n");
}

/// TC-010-22: "This workspace" shows the template folder, the daily note
/// folder and the daily note template with their built-in defaults; "For
/// me" offers none of them; a change goes to the workspace settings file,
/// an emptied folder removes it, and a template that does not exist is
/// shown as such.
#[test]
fn uc010_tc22_template_and_daily_note_settings() {
    use underline_core::{NoteId, Template};

    let dir = temp_dir("tc22");
    let (mut state, root) = state(&dir, None, None);
    let templates = [Template {
        id: NoteId::from_relative("tpl/Day"),
        format: NoteFormat::Markdown,
    }];

    let view = state.window_view(None, &templates);
    for name in [
        SettingName::TemplateFolder,
        SettingName::DailyNoteFolder,
        SettingName::DailyNoteTemplate,
    ] {
        assert!(view.me.iter().all(|row| row.key != name), "{name:?}");
    }
    let folder = row(&view.this_workspace, SettingName::TemplateFolder);
    assert_eq!(folder.label, "Template folder");
    assert_eq!(shown(folder), "templates");
    assert_eq!(note(folder), Some("default"));
    assert_eq!(folder.section.as_deref(), Some("Templates and daily note"));
    assert!(folder
        .help
        .as_deref()
        .unwrap()
        .contains("Empty: the folder templates."));
    assert_eq!(
        shown(row(&view.this_workspace, SettingName::DailyNoteFolder)),
        "daily"
    );
    let template = row(&view.this_workspace, SettingName::DailyNoteTemplate);
    assert_eq!(shown(template), "The template titled Daily");
    let offered: Vec<(&str, Option<&str>)> = choices(template)
        .iter()
        .map(|c| (c.label.as_str(), c.value.as_deref()))
        .collect();
    assert_eq!(
        offered,
        [
            ("The template titled Daily", None),
            ("tpl/Day.md", Some("tpl/Day.md")),
        ]
    );

    // "For me" never takes them.
    state.window_change(Scope::Me, SettingKey::TemplateFolder, Some("x".into()));
    assert!(!user_file(&dir).exists());

    state.window_change(
        Scope::Workspace,
        SettingKey::TemplateFolder,
        Some(" tpl ".into()),
    );
    state.window_change(
        Scope::Workspace,
        SettingKey::DailyNoteFolder,
        Some("journal".into()),
    );
    state.window_change(
        Scope::Workspace,
        SettingKey::DailyNoteTemplate,
        Some("tpl/Day.md".into()),
    );
    assert_eq!(
        fs::read_to_string(root.join(SETTINGS_FILE_NAME)).unwrap(),
        "template_folder: tpl\ndaily_note_folder: journal\ndaily_note_template: tpl/Day.md\n"
    );
    let view = state.window_view(None, &templates);
    let folder = row(&view.this_workspace, SettingName::TemplateFolder);
    assert_eq!(shown(folder), "tpl");
    assert_eq!(note(folder), Some("own"));
    assert_eq!(
        shown(row(&view.this_workspace, SettingName::DailyNoteTemplate)),
        "tpl/Day.md"
    );

    // OQ-274 (b): a folder that cannot take notes is refused at once.
    for (key, value) in [
        (SettingKey::DailyNoteFolder, ".days"),
        (SettingKey::DailyNoteFolder, "../out"),
        (SettingKey::TemplateFolder, "."),
    ] {
        state.window_change(Scope::Workspace, key, Some(value.into()));
        let said = state.take_messages();
        assert_eq!(said.len(), 1, "{value}");
        assert!(said[0].contains(" not changed: "), "{said:?}");
    }
    assert_eq!(
        fs::read_to_string(root.join(SETTINGS_FILE_NAME)).unwrap(),
        "template_folder: tpl\ndaily_note_folder: journal\ndaily_note_template: tpl/Day.md\n"
    );

    // An emptied field gives the default again.
    state.window_change(
        Scope::Workspace,
        SettingKey::DailyNoteFolder,
        Some("  ".into()),
    );
    // TC-025-05: a template that does not exist.
    state.window_change(
        Scope::Workspace,
        SettingKey::DailyNoteTemplate,
        Some("templates/Journal-Day.md".into()),
    );
    assert_eq!(
        fs::read_to_string(root.join(SETTINGS_FILE_NAME)).unwrap(),
        "template_folder: tpl\ndaily_note_template: templates/Journal-Day.md\n"
    );
    let view = state.window_view(None, &templates);
    assert_eq!(
        shown(row(&view.this_workspace, SettingName::DailyNoteTemplate)),
        "templates/Journal-Day.md (not found)"
    );
    assert_eq!(
        shown(row(&view.this_workspace, SettingName::DailyNoteFolder)),
        "daily"
    );
}

/// FR-100, TC-010-21 step 2: the spell check language is chosen by its
/// name; the file holds its code, and "This workspace" may give its own.
#[test]
fn tc010_21_the_spell_check_language_by_name_stored_by_code() {
    let dir = temp_dir("spell-language");
    let (mut state, root) = state(&dir, Some("spell_check_language: off\n"), Some(""));
    let view = state.window_view(Some(NoteFormat::Markdown), &[]);
    let me = row(&view.me, SettingName::SpellCheckLanguage);
    assert_eq!(me.label, "Spell check language");
    assert_eq!(shown(me), "Off");
    let SettingControlView::Choice { choices, .. } = &me.control else {
        panic!("a choice");
    };
    let names: Vec<(&str, Option<&str>)> = choices
        .iter()
        .map(|c| (c.label.as_str(), c.value.as_deref()))
        .collect();
    assert_eq!(
        names,
        [
            ("Off", Some("off")),
            ("English (UK)", Some("en-GB")),
            ("English (US)", Some("en-US")),
            ("French (France)", Some("fr-FR")),
            ("German (Germany)", Some("de-DE")),
            ("German (Switzerland)", Some("de-CH")),
            ("Italian (Italy)", Some("it-IT")),
            ("Spanish (Spain)", Some("es-ES")),
        ]
    );

    state.window_change(
        Scope::Workspace,
        SettingKey::SpellCheckLanguage,
        Some("en-GB".into()),
    );
    assert_eq!(
        state.effective.spell_check_language,
        underline_core::SpellCheckLanguage::EnglishUk
    );
    let written = std::fs::read_to_string(root.join(SETTINGS_FILE_NAME)).expect("the file");
    assert!(written.contains("spell_check_language: en-GB"), "{written}");
}

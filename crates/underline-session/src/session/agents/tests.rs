//! Tests of feature F14 (UC-016 to UC-019, M-04.4, M-04.9, M-17): the
//! agents entry, the presence files, the agents' requests through the
//! window, the agent cursor and the section "Agents".
//!
//! Everything lives in temporary folders: the workspaces, the user
//! settings file and, for MCP install, the home folder of the agent tools'
//! configurations (`Places` of a scratch folder, never this computer's).

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

use underline_core::{
    send_request, take_answer, write_presence, AgentAnswer, AgentChange, AgentRequest, NoteId,
    Presence, Reading, StateStore, SETTINGS_FILE_NAME,
};
use underline_mcp::install::{Os, Places};

use super::FLUSH_AT_MOST;
use crate::agents_view::{AgentAction, AgentLevelName, AgentToolName};
use crate::sync_view::SyncBatch;
use crate::{Event, Session, SettingsState};

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "underline-session-agents-{name}-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// A session on `dir/notes` with `Home.md`, `A.md` (two lines) and
/// `B.md`, the user settings in `dir/home` holding `user`; `A` is open.
fn session_with(dir: &Path, user: &str) -> (Session, PathBuf) {
    let root = dir.join("notes");
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("Home.md"), "# Home\n").unwrap();
    fs::write(root.join("A.md"), "one\ntwo\n").unwrap();
    fs::write(root.join("B.md"), "bee\n").unwrap();
    fs::create_dir_all(dir.join("home")).unwrap();
    fs::write(dir.join("home").join(SETTINGS_FILE_NAME), user).unwrap();
    let mut session = Session::new(
        Some(root.clone()),
        StateStore::in_memory(),
        None,
        SettingsState::with_user_file(
            Some(dir.join("home").join(SETTINGS_FILE_NAME)),
            Instant::now(),
        ),
    );
    session.start_workspace();
    session.open_note(&a());
    session.take_events();
    (session, root)
}

fn a() -> NoteId {
    NoteId::from_relative("A")
}

/// What the tick's thread does with the agents' files at `now`.
fn cycle(session: &mut Session, now: Instant) {
    for _ in 0..4 {
        let Some(job) = session.agents_job(now) else {
            break;
        };
        let done = job.run(SystemTime::now());
        session.agents_done(done, now);
    }
}

/// An MCP server of the agent `name`, process `id`, connected.
fn connect(root: &Path, id: u32, name: &str, reading: Option<Reading>) {
    let presence = Presence {
        name: Some(name.to_owned()),
        reading,
        ..Presence::default()
    };
    write_presence(root, id, &presence).unwrap();
}

/// The agent `agent` asks the window for `change` of `note` (ADR-018).
fn ask(root: &Path, id: &str, agent: &str, note: &str, change: AgentChange) {
    let request = AgentRequest {
        agent: Some(agent.to_owned()),
        note: note.to_owned(),
        change,
    };
    send_request(root, id, &request).unwrap();
}

fn replace(old: &str, new: &str) -> AgentChange {
    AgentChange::Replace {
        old: old.into(),
        new: new.into(),
    }
}

/// The answer to `id`, once the window gave it.
fn answer(root: &Path, id: &str) -> AgentAnswer {
    take_answer(root, id).unwrap_or_else(|| panic!("no answer to {id}"))
}

/// Runs the tick and the agents' files until the saves ended.
fn settle(session: &mut Session, now: Instant) {
    session.settle_saves();
    session.tick(now);
    cycle(session, now);
}

fn menu_text(session: &Session) -> String {
    session
        .agents_view()
        .menu
        .expect("a workspace is open")
        .text
}

fn agent_marks(events: &[Event]) -> Option<Vec<crate::AgentMarksView>> {
    events.iter().rev().find_map(|e| match e {
        Event::AgentMarks(marks) => Some(marks.clone()),
        _ => None,
    })
}

#[test]
fn uc017_m17_1_the_agents_entry_says_the_access_and_names_the_connected() {
    let dir = temp_dir("entry");
    let (mut session, root) = session_with(&dir, "agent_access: read only\n");
    assert_eq!(menu_text(&session), "Agents: read only");

    connect(&root, 101, "Claude Code", None);
    // BR-004: in the order they connected.
    std::thread::sleep(Duration::from_millis(20));
    connect(&root, 102, "Cursor", None);
    cycle(&mut session, Instant::now());

    assert_eq!(
        menu_text(&session),
        "Agents: read only · Agents connected: Claude Code, Cursor"
    );
    let events = session.take_events();
    assert!(
        events.iter().any(|e| matches!(e, Event::Agents(_))),
        "{events:?}"
    );
    let view = session.agents_view();
    let menu = view.menu.unwrap();
    assert_eq!(
        menu.hover,
        "What agents may do with the notes through Underline's MCP server"
    );
    assert_eq!(menu.workspace_settings, "Settings of this workspace…");
    let levels: Vec<(&str, bool)> = menu
        .levels
        .iter()
        .map(|l| (l.label.as_str(), l.chosen))
        .collect();
    assert_eq!(
        levels,
        [
            ("Off", false),
            ("Read only", true),
            ("Read and write", false)
        ]
    );
    assert_eq!(
        view.head,
        [
            format!(
                "Let an agent tool reach the notes of {} through Underline's MCP server.",
                root.display()
            ),
            "Agent access is read only.".to_owned()
        ]
    );
}

#[test]
fn uc017_a1_the_workspace_caps_the_menu() {
    let dir = temp_dir("capped");
    let root = dir.join("notes");
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join(SETTINGS_FILE_NAME), "agent_access: read only\n").unwrap();
    let (session, _) = session_with(&dir, "agent_access: read and write\n");

    let menu = session.agents_view().menu.unwrap();

    assert_eq!(menu.text, "Agents: read only in this workspace");
    let labels: Vec<&str> = menu.levels.iter().map(|l| l.label.as_str()).collect();
    assert_eq!(labels, ["Off", "Read only"]);
}

#[test]
fn uc017_a2_an_agent_asked_while_access_is_off() {
    let dir = temp_dir("asked");
    let (mut session, root) = session_with(&dir, "");
    let presence = Presence {
        name: Some("Claude".into()),
        refused: true,
        ..Presence::default()
    };
    write_presence(&root, 7, &presence).unwrap();

    cycle(&mut session, Instant::now());

    let menu = session.agents_view().menu.unwrap();
    assert_eq!(menu.text, "Agents: off");
    assert_eq!(menu.asked.as_deref(), Some("Claude asked for access"));
    assert_eq!(
        session.agents_view().head[1],
        "Agent access is off; agents reach the notes only while it is on."
    );
}

#[test]
fn uc017_br002_m17_1_the_menu_changes_the_user_setting() {
    let dir = temp_dir("choose");
    let (mut session, _) = session_with(&dir, "");

    session.set_agent_access(AgentLevelName::ReadWrite);

    assert_eq!(
        fs::read_to_string(dir.join("home").join(SETTINGS_FILE_NAME)).unwrap(),
        "agent_access: Read and write\n"
    );
    assert_eq!(menu_text(&session), "Agents: read and write");
}

#[test]
fn m17_head_without_a_workspace() {
    let dir = temp_dir("no-workspace");
    let session = Session::new(
        None,
        StateStore::in_memory(),
        None,
        SettingsState::with_user_file(Some(dir.join(SETTINGS_FILE_NAME)), Instant::now()),
    );

    let view = session.agents_view();

    assert_eq!(view.menu, None);
    assert_eq!(view.head, ["Open a workspace to connect agents to it."]);
    assert!(!view.workspace);
    assert_eq!(session.start_info().agents, view);
}

#[test]
fn adr018_m17_7_m17_8_the_window_presence_is_renewed_and_removed_at_the_end() {
    let dir = temp_dir("presence");
    let (mut session, root) = session_with(&dir, "");
    let window = root.join(".underline/agents/window");

    cycle(&mut session, Instant::now());
    assert!(window.exists());

    session.end().unwrap();
    assert!(!window.exists());
}

#[test]
fn d05_a162_switching_the_workspace_removes_its_window_presence() {
    let dir = temp_dir("d05");
    let (mut session, root) = session_with(&dir, "");
    let other = dir.join("other");
    fs::create_dir_all(&other).unwrap();
    fs::write(other.join("Home.md"), "# Other\n").unwrap();
    cycle(&mut session, Instant::now());
    assert!(root.join(".underline/agents/window").exists());

    session.open_chosen(&other, false);
    cycle(&mut session, Instant::now());

    assert!(!root.join(".underline/agents/window").exists());
    assert!(other.join(".underline/agents/window").exists());
}

#[test]
fn uc019_m17_7_without_writing_access_a_change_is_refused() {
    let dir = temp_dir("read-only");
    let (mut session, root) = session_with(&dir, "agent_access: read only\n");
    connect(&root, 9, "Claude", None);
    ask(&root, "9-0", "Claude", "B", replace("bee", "BEE"));

    cycle(&mut session, Instant::now());

    let answer = answer(&root, "9-0");
    assert!(!answer.ok);
    assert_eq!(
        answer.message,
        "the note `B` cannot be written: Underline allows agents only to read the notes of this workspace"
    );
    assert_eq!(fs::read_to_string(root.join("B.md")).unwrap(), "bee\n");
}

#[test]
fn uc019_br010_m04_9_notes_not_open_change_in_their_files_and_are_reported() {
    let dir = temp_dir("not-open");
    let (mut session, root) = session_with(&dir, "agent_access: read and write\n");
    connect(&root, 9, "Claude", None);
    ask(&root, "9-0", "Claude", "B", replace("bee", "BEE"));

    cycle(&mut session, Instant::now());

    assert!(answer(&root, "9-0").ok);
    assert_eq!(fs::read_to_string(root.join("B.md")).unwrap(), "BEE\n");
    assert_eq!(session.status(), "Claude changed B");
    let opens = session.agent_opens();
    assert_eq!(opens.len(), 1);
    assert_eq!(
        (opens[0].note.as_str(), opens[0].label.as_str()),
        ("B", "Open")
    );

    // A second change of the same agent joins the report.
    let create = AgentChange::Create {
        format: None,
        text: "# C\n".into(),
    };
    ask(&root, "9-1", "Claude", "C", create);
    cycle(&mut session, Instant::now() + Duration::from_millis(200));

    assert!(answer(&root, "9-1").ok);
    assert!(root.join("C.md").exists());
    assert_eq!(session.status(), "Claude created C and changed B");
    let labels: Vec<String> = session.agent_opens().into_iter().map(|o| o.label).collect();
    assert_eq!(labels, ["Open C", "Open B"], "the newest first");
    let events = session.take_events();
    let status = events.iter().find_map(|e| match e {
        Event::Status(status) => Some(status.clone()),
        _ => None,
    });
    assert_eq!(status.unwrap().agents.len(), 2);
    // D-04 (fixed, A-163): a background save does not end the report.
    assert!(session.status_kept());
}

#[test]
fn uc019_a4_a_note_not_found_is_refused() {
    let dir = temp_dir("not-found");
    let (mut session, root) = session_with(&dir, "agent_access: read and write\n");
    connect(&root, 9, "Claude", None);
    ask(&root, "9-0", "Claude", "Nowhere", replace("a", "b"));
    let before = session.status().to_owned();

    cycle(&mut session, Instant::now());

    let answer = answer(&root, "9-0");
    assert!(!answer.ok, "{answer:?}");
    assert_eq!(session.status(), before);
}

/// ADR-025 section 3.6, UC-019 steps 2–4, BR-007: a change of an open note
/// asks the window for its edits, waits for them, and is made on the
/// mirror as an undo step of its own, stored at once; the agent cursor
/// marks the lines.
#[test]
fn uc019_adr025_an_open_note_waits_for_the_windows_edits() {
    let dir = temp_dir("open");
    let (mut session, root) = session_with(&dir, "agent_access: read and write\n");
    // The window holds the text and typed in the first line.
    session.window_type(&a(), 3, "!");
    connect(&root, 9, "Claude", None);
    ask(&root, "9-0", "Claude", "A", replace("two", "TWO"));
    let start = Instant::now();
    let before = session.status().to_owned();

    cycle(&mut session, start);
    let events = session.tick(start);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::Flush(f) if f.note.as_deref() == Some("A"))),
        "the window is asked for its edits: {events:?}"
    );
    assert_eq!(session.window_text(&a()), "one!\ntwo\n", "not yet");
    assert!(take_answer(&root, "9-0").is_none());

    // The window sends what it has (here nothing more).
    let rev = session.mirror_rev(&a());
    let flush = SyncBatch {
        seq: 0,
        base: rev,
        edits: Vec::new(),
    };
    session.editor_sync(&a(), &flush).unwrap();
    let events = session.tick(start + Duration::from_millis(10));

    assert_eq!(session.window_text(&a()), "one!\nTWO\n");
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::Changed(c) if c.note == "A")),
        "{events:?}"
    );
    let marks = agent_marks(&events).expect("the agent cursor");
    assert_eq!(marks[0].note, "A");
    assert_eq!(
        (
            marks[0].marks[0].from,
            marks[0].marks[0].to,
            marks[0].marks[0].name.as_str()
        ),
        (1, 2, "Claude")
    );

    settle(&mut session, start + Duration::from_millis(20));
    let answer = answer(&root, "9-0");
    assert!(answer.ok, "{answer:?}");
    assert_eq!(answer.message, "changed A; the change is in line 2");
    assert_eq!(
        fs::read_to_string(root.join("A.md")).unwrap(),
        "one!\nTWO\n",
        "stored at once, with the user's edit"
    );
    // As egui's `save_now`: no "Saved A".
    assert_eq!(session.status(), before);
}

#[test]
fn adr025_without_the_windows_answer_the_change_is_made_after_300_ms() {
    let dir = temp_dir("timeout");
    let (mut session, root) = session_with(&dir, "agent_access: read and write\n");
    session.window_type(&a(), 0, ">");
    connect(&root, 9, "Claude", None);
    ask(&root, "9-0", "Claude", "A", replace("two", "TWO"));
    let start = Instant::now();
    cycle(&mut session, start);

    session.tick(start + Duration::from_millis(100));
    assert_eq!(session.window_text(&a()), ">one\ntwo\n");
    session.tick(start + FLUSH_AT_MOST);

    assert_eq!(session.window_text(&a()), ">one\nTWO\n");
    settle(&mut session, start + FLUSH_AT_MOST);
    assert!(answer(&root, "9-0").ok);
}

#[test]
fn uc019_a5_a_line_the_user_changed_is_refused() {
    let dir = temp_dir("user-line");
    let (mut session, root) = session_with(&dir, "agent_access: read and write\n");
    // The user typed in "two", not stored yet.
    session.window_type(&a(), 7, "!");
    connect(&root, 9, "Claude", None);
    ask(&root, "9-0", "Claude", "A", replace("two", "TWO"));
    let start = Instant::now();

    cycle(&mut session, start);
    session.tick(start + FLUSH_AT_MOST);
    cycle(&mut session, start + FLUSH_AT_MOST);

    let answer = answer(&root, "9-0");
    assert!(!answer.ok);
    assert!(
        answer.message.contains("is editing the note `A` at line 2"),
        "{}",
        answer.message
    );
    assert_eq!(session.window_text(&a()), "one\ntwo!\n");
}

#[test]
fn adr018_a_note_being_stored_answers_busy() {
    let dir = temp_dir("busy");
    let (mut session, root) = session_with(&dir, "agent_access: read and write\n");
    connect(&root, 9, "Claude", None);
    ask(&root, "9-0", "Claude", "A", replace("two", "TWO"));
    let start = Instant::now();
    cycle(&mut session, start);
    {
        let open = session.open.as_mut().unwrap();
        let index = open.tabs.find(&a()).unwrap();
        open.tabs.get_mut(index).saving = true;
    }

    // No window synced: made at once, here refused as busy.
    session.agents_tick(start);
    cycle(&mut session, start);

    let answer = answer(&root, "9-0");
    assert_eq!(answer.message, "Underline is busy; try again");
    assert_eq!(session.window_text(&a()), "one\ntwo\n");
}

#[test]
fn uc018_br006_m17_6_the_agent_cursor_shows_for_10_seconds_as_access_allows() {
    let dir = temp_dir("cursor");
    let (mut session, root) = session_with(&dir, "agent_access: read only\n");
    let read = Reading::now(a(), (2, 2));
    connect(&root, 9, "Claude", Some(read.clone()));
    let start = Instant::now();
    cycle(&mut session, start);

    let marks = agent_marks(&session.tick(start)).expect("marks");
    assert_eq!(marks.len(), 1);
    assert_eq!(
        (marks[0].marks[0].from, marks[0].marks[0].to),
        (1, 2),
        "line 2, counted from 0, the end not included"
    );

    // Written lines show only with writing access (UC-017 BR-005).
    let wrote = Reading {
        lines: (1, 1),
        at: read.at + 1,
        ..read.clone()
    };
    connect(&root, 9, "Claude", Some(wrote.written()));
    cycle(&mut session, start + Duration::from_secs(1));
    let marks = agent_marks(&session.tick(start + Duration::from_secs(1))).expect("changed");
    assert!(marks.is_empty(), "{marks:?}");

    // Gone 10 seconds after.
    session.set_agent_access(AgentLevelName::ReadWrite);
    let shown = agent_marks(&session.tick(start + Duration::from_secs(2))).expect("shown");
    assert_eq!(shown.len(), 1);
    let gone = agent_marks(&session.tick(start + Duration::from_secs(12))).expect("gone");
    assert!(gone.is_empty());
}

#[test]
fn uc020_a8_a_todo_an_agent_changed_is_the_status() {
    let dir = temp_dir("todo");
    let (mut session, root) = session_with(&dir, "");
    let presence = Presence {
        name: Some("Claude".into()),
        todo_change: Some((1_700_000_000_000, "created todo #3".into())),
        ..Presence::default()
    };
    write_presence(&root, 9, &presence).unwrap();

    cycle(&mut session, Instant::now());

    assert_eq!(session.status(), "Claude created todo #3");
}

// M-17.2 to M-17.5: the section "Agents", in a scratch home folder.

fn scratch_places(home: &Path) -> Places {
    Places {
        home: Some(home.to_owned()),
        os: Os::Linux,
        ..Places::default()
    }
}

fn act(
    session: &mut Session,
    places: &Places,
    action: Option<AgentAction>,
) -> crate::AgentsSectionView {
    let job = session.agents_section_start(action);
    let done = job.run(places);
    session.agents_section_done(done)
}

fn row(view: &crate::AgentsSectionView, tool: AgentToolName) -> crate::AgentToolRowView {
    view.tools.iter().find(|r| r.tool == tool).unwrap().clone()
}

#[test]
fn uc016_m17_2_the_tools_and_their_states() {
    let dir = temp_dir("section");
    let home = dir.join("tools-home");
    fs::create_dir_all(home.join(".claude")).unwrap();
    let places = scratch_places(&home);
    let (mut session, _) = session_with(&dir, "");
    session
        .agents
        .set_program(PathBuf::from("/opt/underline/underline"));

    let view = act(&mut session, &places, None);

    assert_eq!(view.tools.len(), 6);
    let claude = row(&view, AgentToolName::ClaudeCode);
    assert_eq!(
        (claude.name.as_str(), claude.state.as_str()),
        ("Claude Code", "not installed")
    );
    assert_eq!(claude.button.unwrap().label, "Install");
    let cursor = row(&view, AgentToolName::Cursor);
    assert_eq!(cursor.state, "not found");
    assert!(cursor.weak);
    assert!(!cursor.button.unwrap().enabled);
    assert_eq!(view.copy.unwrap().label, "Copy configuration");
    assert_eq!(view.copy_hover, "For any other agent tool");
}

#[test]
fn uc016_a1_m17_3_install_with_access_off_asks_first() {
    let dir = temp_dir("switch-on");
    let home = dir.join("tools-home");
    fs::create_dir_all(home.join(".claude")).unwrap();
    let places = scratch_places(&home);
    let (mut session, root) = session_with(&dir, "");
    session
        .agents
        .set_program(PathBuf::from("/opt/underline/underline"));
    act(&mut session, &places, None);
    let install = AgentAction::Install {
        tool: AgentToolName::ClaudeCode,
        replace: false,
    };

    let view = act(&mut session, &places, Some(install));

    let question = view.question.expect("asked");
    assert_eq!(
        question.text.as_deref(),
        Some("Agents reach the notes only while agent access is on. Allow agents to read, or to read and write, for you?")
    );
    let labels: Vec<&str> = question.buttons.iter().map(|b| b.label.as_str()).collect();
    assert_eq!(
        labels,
        [
            "Allow reading and install",
            "Allow reading and writing and install",
            "Install only",
            "Cancel"
        ]
    );
    assert!(!home.join(".claude.json").exists(), "nothing written yet");

    let view = act(&mut session, &places, Some(question.buttons[1].action));

    assert_eq!(
        fs::read_to_string(dir.join("home").join(SETTINGS_FILE_NAME)).unwrap(),
        "agent_access: Read and write\n"
    );
    assert_eq!(
        view.report.as_deref(),
        Some("Installed notes in Claude Code as underline-notes; restart Claude Code if it does not show it yet.")
    );
    assert_eq!(
        row(&view, AgentToolName::ClaudeCode).state,
        "installed as underline-notes"
    );
    let config = fs::read_to_string(home.join(".claude.json")).unwrap();
    assert!(config.contains("/opt/underline/underline"), "{config}");
    assert!(config.contains(&root.display().to_string()), "{config}");

    // A3: "Remove".
    let remove = row(&view, AgentToolName::ClaudeCode).button.unwrap();
    assert_eq!(remove.label, "Remove");
    let view = act(&mut session, &places, Some(remove.action));
    assert_eq!(
        view.report.as_deref(),
        Some("Removed notes from Claude Code.")
    );
    assert_eq!(row(&view, AgentToolName::ClaudeCode).state, "not installed");
}

#[test]
fn uc016_a1_install_only_says_access_is_off() {
    let dir = temp_dir("install-only");
    let home = dir.join("tools-home");
    fs::create_dir_all(home.join(".cursor")).unwrap();
    let places = scratch_places(&home);
    let (mut session, _) = session_with(&dir, "");
    session
        .agents
        .set_program(PathBuf::from("/opt/underline/underline"));

    let view = act(
        &mut session,
        &places,
        Some(AgentAction::InstallOnly {
            tool: AgentToolName::Cursor,
        }),
    );

    assert_eq!(
        view.report.as_deref(),
        Some("Installed notes in Cursor as underline-notes; restart Cursor if it does not show it yet. Agent access is off, so the agent cannot reach the notes yet.")
    );
    assert_eq!(
        fs::read_to_string(dir.join("home").join(SETTINGS_FILE_NAME)).unwrap(),
        "",
        "access stays off"
    );
}

#[test]
fn uc016_a2_m17_3_an_entry_for_another_place_is_replaced_after_asking() {
    let dir = temp_dir("replace");
    let home = dir.join("tools-home");
    fs::create_dir_all(home.join(".cursor")).unwrap();
    let places = scratch_places(&home);
    let (mut session, _) = session_with(&dir, "agent_access: read only\n");
    session.agents.set_program(PathBuf::from("/old/underline"));
    act(
        &mut session,
        &places,
        Some(AgentAction::InstallOnly {
            tool: AgentToolName::Cursor,
        }),
    );
    session.agents.set_program(PathBuf::from("/new/underline"));

    let view = act(&mut session, &places, None);
    let cursor = row(&view, AgentToolName::Cursor);
    assert_eq!(cursor.state, "underline-notes: installed for another place");
    assert!(cursor.warn);
    let view = act(&mut session, &places, Some(cursor.button.unwrap().action));

    let question = view.question.expect("Replace or Cancel");
    assert_eq!(
        question.text.as_deref(),
        Some("An entry for this workspace starts another Underline. Replace it?")
    );
    assert_eq!(view.report, None, "the question says it");
    let labels: Vec<&str> = question.buttons.iter().map(|b| b.label.as_str()).collect();
    assert_eq!(labels, ["Replace", "Cancel"]);

    // "Cancel" takes the question away with its text.
    let cancelled = act(&mut session, &places, Some(question.buttons[1].action));
    assert!(cancelled.question.is_none() && cancelled.report.is_none());
    let install = row(&cancelled, AgentToolName::Cursor)
        .button
        .unwrap()
        .action;
    let view = act(&mut session, &places, Some(install));
    let question = view.question.expect("asked again");

    let view = act(&mut session, &places, Some(question.buttons[0].action));
    assert_eq!(
        row(&view, AgentToolName::Cursor).state,
        "installed as underline-notes"
    );
    assert!(fs::read_to_string(home.join(".cursor/mcp.json"))
        .unwrap()
        .contains("/new/underline"));
}

#[test]
fn uc016_a6_m17_4_an_unreadable_configuration() {
    let dir = temp_dir("unreadable");
    let home = dir.join("tools-home");
    fs::create_dir_all(home.join(".cursor")).unwrap();
    fs::write(home.join(".cursor/mcp.json"), "{ not json").unwrap();
    let places = scratch_places(&home);
    let (mut session, _) = session_with(&dir, "agent_access: read only\n");

    let view = act(&mut session, &places, None);
    let cursor = row(&view, AgentToolName::Cursor);
    assert_eq!(cursor.state, "cannot be read");
    assert!(cursor.warn && cursor.button.is_none() && cursor.hover.is_some());

    let view = act(
        &mut session,
        &places,
        Some(AgentAction::Install {
            tool: AgentToolName::Cursor,
            replace: false,
        }),
    );
    assert!(
        view.report
            .as_deref()
            .is_some_and(|r| r.starts_with("Cursor was not changed: ")),
        "{:?}",
        view.report
    );
    assert_eq!(
        fs::read_to_string(home.join(".cursor/mcp.json")).unwrap(),
        "{ not json"
    );
}

#[test]
fn uc016_a4_m17_5_copy_configuration() {
    let dir = temp_dir("copy");
    let places = scratch_places(&dir.join("tools-home"));
    let (mut session, root) = session_with(&dir, "");
    session
        .agents
        .set_program(PathBuf::from("/opt/my apps/underline"));

    let job = session.agents_section_start(Some(AgentAction::Copy));
    let snippet = job.copied().expect("a configuration").to_owned();
    let view = session.agents_section_done(job.run(&places));

    assert!(snippet.contains("\"mcpServers\""), "{snippet}");
    assert_eq!(
        view.report,
        Some(format!(
            "Copied the configuration of notes. Its command: \"/opt/my apps/underline\" mcp {}",
            root.display()
        ))
    );
}

#[test]
fn m17_2_the_section_is_empty_without_a_workspace() {
    let dir = temp_dir("section-none");
    let mut session = Session::new(
        None,
        StateStore::in_memory(),
        None,
        SettingsState::with_user_file(Some(dir.join(SETTINGS_FILE_NAME)), Instant::now()),
    );
    let places = scratch_places(&dir.join("tools-home"));

    let view = act(&mut session, &places, None);

    assert!(view.tools.is_empty());
    assert_eq!(view.copy, None);
}

/// Review F14: an agent's change being stored is answered when the save
/// ends, also when its tab went meanwhile (deleted, another branch …).
#[test]
fn uc019_the_answer_comes_also_when_the_tab_went() {
    let dir = temp_dir("tab-gone");
    let (mut session, root) = session_with(&dir, "agent_access: read and write\n");
    connect(&root, 9, "Claude", None);
    ask(&root, "9-0", "Claude", "A", replace("two", "TWO"));
    let start = Instant::now();
    cycle(&mut session, start);
    session.agents_tick(start);
    {
        let open = session.open.as_mut().unwrap();
        let index = open.tabs.find(&a()).unwrap();
        open.tabs.close(index);
    }

    settle(&mut session, start);

    let answer = answer(&root, "9-0");
    assert!(answer.ok, "{answer:?}");
    assert_eq!(fs::read_to_string(root.join("A.md")).unwrap(), "one\nTWO\n");
}

/// Review F14: switching the workspace answers the requests still waiting
/// for the window busy, so their servers ask again.
#[test]
fn uc019_leaving_the_workspace_answers_waiting_requests() {
    let dir = temp_dir("leave-waiting");
    let (mut session, root) = session_with(&dir, "agent_access: read and write\n");
    session.window_type(&a(), 0, ">");
    connect(&root, 9, "Claude", None);
    ask(&root, "9-0", "Claude", "A", replace("two", "TWO"));
    cycle(&mut session, Instant::now());
    let other = dir.join("other");
    fs::create_dir_all(&other).unwrap();
    fs::write(other.join("Home.md"), "# Other\n").unwrap();

    session.open_chosen(&other, false);
    cycle(&mut session, Instant::now());

    assert_eq!(answer(&root, "9-0").message, "Underline is busy; try again");
}

/// Review F14: Underline ends: a request waiting for the window is
/// answered before the presence goes.
#[test]
fn m17_8_ending_answers_waiting_requests() {
    let dir = temp_dir("end-waiting");
    let (mut session, root) = session_with(&dir, "agent_access: read and write\n");
    session.window_type(&a(), 0, ">");
    connect(&root, 9, "Claude", None);
    ask(&root, "9-0", "Claude", "A", replace("two", "TWO"));
    cycle(&mut session, Instant::now());

    session.end().unwrap();

    assert_eq!(answer(&root, "9-0").message, "Underline is busy; try again");
    assert!(!root.join(".underline/agents/window").exists());
}

/// Review F14, UC-017 BR-003: access switched off between taking a request
/// and changing a note's file refuses it.
#[test]
fn uc017_br003_access_switched_off_before_the_file_is_changed() {
    let dir = temp_dir("off-between");
    let (mut session, root) = session_with(&dir, "agent_access: read and write\n");
    connect(&root, 9, "Claude", None);
    ask(&root, "9-0", "Claude", "B", replace("bee", "BEE"));
    let now = Instant::now();
    // Look and take the request; it waits to be found among the notes.
    let job = session.agents_job(now).unwrap();
    let done = job.run(SystemTime::now());
    session.agents_done(done, now);
    let job = session.agents_job(now).expect("the request to find");
    session.set_agent_access(AgentLevelName::ReadOnly);
    let done = job.run(SystemTime::now());
    session.agents_done(done, now);

    let refused = answer(&root, "9-0");
    assert!(refused
        .message
        .ends_with("Underline allows agents only to read the notes of this workspace"));
    assert_eq!(fs::read_to_string(root.join("B.md")).unwrap(), "bee\n");
}

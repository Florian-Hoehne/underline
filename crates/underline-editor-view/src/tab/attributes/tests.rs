use underline_markup::NoteFormat;

use super::*;
use crate::editor_view::{EditorInput, KeyName};

fn set(name: &str, text: &str) -> AttributeChangeView {
    AttributeChangeView::Set {
        name: name.into(),
        value: AttributeValueView::Text { text: text.into() },
    }
}

fn undo(tab: &mut EditorTab) {
    tab.source_input(
        EditorInput::Key {
            key: KeyName::Z,
            ctrl: true,
            alt: false,
            shift: false,
            meta: false,
            page: 10,
        },
        false,
    );
}

const PLAN: &str = "---\nstatus: draft\ndue: 2026-12-01\n---\n# Plan\n\nSee #ferry.\n";

/// TC-024-01 step 1, TC-024-11: the attributes, their lines and offsets,
/// and the tags of the note.
#[test]
fn tc024_01_the_head_of_the_note() {
    let mut tab = EditorTab::new(PLAN, NoteFormat::Markdown);
    let head = tab.note_head();
    let names: Vec<(&str, AttributeKindName, &str)> = head
        .attributes
        .iter()
        .map(|a| (a.name.as_str(), a.kind, a.value.as_str()))
        .collect();
    assert_eq!(
        names,
        [
            ("status", AttributeKindName::Text, "draft"),
            ("due", AttributeKindName::Date, "2026-12-01")
        ]
    );
    assert_eq!(
        (head.attributes[1].line, head.attributes[1].offset),
        (3, 18)
    );
    assert_eq!(head.tags, ["ferry"]);
    assert!(!head.asciidoc);
}

/// TC-024-12, BR-006: a change in the source view is one undo step; the
/// selection stays on its text; the head follows at once.
#[test]
fn tc024_12_a_change_is_one_undo_step() {
    let mut tab = EditorTab::new(PLAN, NoteFormat::Markdown);
    let at = PLAN.find("Plan\n").unwrap();
    tab.editor.set_selection(at, at);
    let edited = tab.source_attribute(set("status", "final"));
    assert_eq!(edited.problem, None);
    let patch = edited.patch.unwrap();
    assert_eq!(patch.changes.len(), 1);
    assert_eq!(tab.editor.text(), PLAN.replace("draft", "final"));
    assert_eq!(tab.editor.cursor(), at, "the cursor stays on # Plan");
    assert_eq!(tab.note_head().attributes[0].value, "final");
    undo(&mut tab);
    assert_eq!(tab.editor.text(), PLAN);
    assert_eq!(tab.note_head().attributes[0].value, "draft");
}

/// TC-024-06, TC-024-07: a refused change writes nothing and says why.
#[test]
fn tc024_06_07_refused_changes() {
    let mut tab = EditorTab::new(PLAN, NoteFormat::Markdown);
    let edited = tab.source_attribute(AttributeChangeView::Add {
        name: "my status".into(),
        value: AttributeValueView::Text { text: "x".into() },
    });
    assert_eq!(edited.patch, None);
    assert_eq!(
        edited.problem.as_deref(),
        Some("A name cannot hold a space.")
    );
    let edited = tab.source_attribute(AttributeChangeView::Set {
        name: "due".into(),
        value: AttributeValueView::Date {
            text: "2026-13-01".into(),
        },
    });
    assert!(edited.problem.unwrap().starts_with("A date is expected"));
    assert_eq!(tab.editor.text(), PLAN);
}

/// TC-024-08: the line of a front matter that cannot be read, and its
/// offset for the source view.
#[test]
fn tc024_08_problem_with_its_line() {
    let broken = "---\nstatus: draft\njust text\n---\nBody\n";
    let mut tab = EditorTab::new(broken, NoteFormat::Markdown);
    let head = tab.note_head();
    let problem = head.problem.unwrap();
    assert_eq!((problem.line, problem.offset), (3, 18));
    assert!(head.attributes.is_empty());
}

/// Web offsets count UTF-16 units and a `\r\n` once.
#[test]
fn uc024_offsets_are_web_offsets() {
    let note = "= Ü 🙂\r\n:a: b\r\n";
    let mut tab = EditorTab::new(note, NoteFormat::AsciiDoc);
    let head = tab.note_head();
    assert!(head.asciidoc);
    // "= Ü 🙂" is 6 units, the line break one.
    assert_eq!(head.attributes[0].offset, 7);
}

//! Tests of link completion in the source view (FR-080, UC-005 A23, A24,
//! BR-018, TC-005-33, TC-005-34); the facade's are in `facade_tests.rs`.

use std::time::Instant;

use underline_markup::NoteFormat;

use crate::editor_view::{revision, EditorInput, LinkChoice};
use crate::EditorTab;

/// A tab of `format` holding `text`, the cursor at its end, as after typing
/// it.
fn typed(text: &str, format: NoteFormat) -> EditorTab {
    let mut tab = EditorTab::new("", format);
    tab.source_input(
        EditorInput::Text {
            text: text.to_owned(),
        },
        false,
    );
    tab
}

fn choice(
    tab: &EditorTab,
    note: Option<&str>,
    format: &str,
    anchor: Option<&str>,
    label: &str,
) -> LinkChoice {
    LinkChoice {
        rev: revision(tab.editor.revision()),
        note: note.map(str::to_owned),
        format: note.map(|_| format.to_owned()),
        anchor: anchor.map(str::to_owned),
        label: label.to_owned(),
    }
}

/// TC-005-33 steps 1 and 2: `[list](pack` asks for "pack"; choosing
/// `travel/Packing` writes the link as the toolbar does, with the text
/// "list", the cursor after it.
#[test]
fn tc_005_33_steps_1_2_markdown_place_completed() {
    let mut tab = typed("Intro [list](pack", NoteFormat::Markdown);

    let context = tab.link_context("Ideas", false, false).unwrap();
    assert_eq!(context.typed, "pack");
    assert_eq!(context.heading, None);
    assert_eq!(context.from, 6, "where `[` is, in web units");
    assert_eq!(context.named.as_deref(), Some("pack"));
    assert_eq!(context.rev, revision(tab.editor.revision()));

    let chosen = choice(&tab, Some("travel/Packing"), "Markdown", None, "Packing");
    let patch = tab.source_complete("Ideas", &chosen).unwrap().patch;

    assert_eq!(tab.editor.text(), "Intro [list](travel/Packing.md)");
    assert_eq!(patch.head as usize, "Intro [list](travel/Packing.md)".len());
    assert!(
        tab.link_context("Ideas", false, false).is_none(),
        "the place is closed"
    );
    // One change: one undo takes the link back to what was typed.
    tab.source_input(
        EditorInput::Key {
            key: crate::editor_view::KeyName::Z,
            ctrl: true,
            alt: false,
            shift: false,
            meta: false,
            page: 10,
        },
        false,
    );
    assert_eq!(tab.editor.text(), "Intro [list](pack");
}

/// TC-005-33 step 3: after `#` the heading is written as GitHub names it,
/// with its text when the link has none.
#[test]
fn tc_005_33_step_3_heading_of_another_note() {
    let mut tab = typed("[](travel/Packing.md#pa", NoteFormat::Markdown);

    let context = tab.link_context("Ideas", false, false).unwrap();
    assert_eq!(context.typed, "travel/Packing.md");
    assert_eq!(context.heading.as_deref(), Some("pa"));
    assert_eq!(context.named.as_deref(), Some("travel/Packing"));

    let chosen = choice(
        &tab,
        Some("travel/Packing"),
        "Markdown",
        Some("papers"),
        "Papers",
    );
    tab.source_complete("Ideas", &chosen).unwrap();

    assert_eq!(tab.editor.text(), "[Papers](travel/Packing.md#papers)");
}

/// UC-005 A23 step 1: a `#` with no note typed lists the headings of the
/// note itself; the link names only the heading.
#[test]
fn uc_005_a23_heading_of_the_note_itself() {
    let mut tab = typed("See [](#go", NoteFormat::Markdown);
    let context = tab.link_context("Ideas", false, false).unwrap();
    assert_eq!(context.named, None);
    assert_eq!(context.heading.as_deref(), Some("go"));

    let chosen = choice(&tab, None, "", Some("goals"), "Goals");
    tab.source_complete("Ideas", &chosen).unwrap();

    assert_eq!(tab.editor.text(), "See [Goals](#goals)");
}

/// TC-005-33 step 5: `xref:pla` in an AsciiDoc note, completed relative to
/// a note in a folder.
#[test]
fn tc_005_33_step_5_asciidoc_xref() {
    let mut tab = typed("See xref:pla", NoteFormat::AsciiDoc);
    assert_eq!(
        tab.link_context("guides/Guide", false, false)
            .unwrap()
            .typed,
        "pla"
    );

    let chosen = choice(&tab, Some("Plan"), "Markdown", None, "Plan");
    tab.source_complete("guides/Guide", &chosen).unwrap();

    assert_eq!(tab.editor.text(), "See xref:../Plan.md[Plan]");
}

/// UC-005 A24, TC-005-34 step 2: no place of a link, no completion; a
/// choice for another revision writes nothing.
#[test]
fn tc_005_34_step_2_nothing_typed_or_text_changed() {
    let tab = typed("[x](zzzz yes", NoteFormat::Markdown);
    assert!(tab.link_context("Ideas", false, false).is_none());

    let mut tab = typed("[x](pl", NoteFormat::Markdown);
    let mut chosen = choice(&tab, Some("Plan"), "Markdown", None, "Plan");
    chosen.rev = chosen.rev.wrapping_add(1);
    assert!(tab.source_complete("Ideas", &chosen).is_none());
    assert_eq!(tab.editor.text(), "[x](pl");

    // In a code block no place of a link is typed.
    let tab = typed("```\n[x](pl", NoteFormat::Markdown);
    assert!(tab.link_context("Ideas", false, false).is_none());
}

/// NFR-018: asking for the place of a link reads only the cursor's line,
/// also in a long note.
#[test]
fn nfr_018_asking_is_fast_in_a_long_note() {
    let body = "A line of text with a [link](Plan.md) in it.\n".repeat(20_000);
    let tab = typed(&format!("{body}[x](pl"), NoteFormat::Markdown);
    let started = Instant::now();
    for _ in 0..20 {
        assert!(tab.link_context("Ideas", false, false).is_some());
    }
    let each = started.elapsed() / 20;
    eprintln!("NFR-018: the place of a link in 20,000 lines: {each:?} per key");
    assert!(each.as_millis() < 200, "{each:?}");
}

fn written(text: &str, format: NoteFormat, label: &str) -> String {
    let mut tab = typed(text, format);
    let chosen = choice(&tab, Some("Plan"), "Markdown", None, label);
    tab.source_complete("Ideas", &chosen).unwrap();
    tab.editor.text()
}

/// UC-005 A23 (chaos): the place of a link is not typed after the closing
/// fence of a block that was closed with a longer fence, nor inside an
/// indented fence of a list item.
#[test]
fn uc_005_a23_chaos_not_in_a_fence_of_a_list_item() {
    let tab = typed("- item\n\n    ```\n    code [a](pl", NoteFormat::Markdown);
    assert!(
        tab.link_context("Ideas", false, false).is_none(),
        "inside an indented code fence of a list item"
    );
}

/// UC-005 A23 (chaos): inside a code span that opened on an earlier line
/// of the same paragraph no place is typed.
#[test]
fn uc_005_a23_chaos_code_span_over_two_lines() {
    let tab = typed("see `code\n[a](pl", NoteFormat::Markdown);
    assert!(tab.link_context("Ideas", false, false).is_none());
}

/// UC-005 A23 (chaos): the cursor at the very start of the text, and a
/// text of multibyte characters before the link.
#[test]
fn uc_005_a23_chaos_start_and_multibyte() {
    let tab = EditorTab::new("", NoteFormat::Markdown);
    assert!(tab.link_context("Ideas", false, false).is_none());
    assert!(tab.link_context("Ideas", true, true).is_some());
    let mut tab = typed("日本語😀é [x](日", NoteFormat::Markdown);
    let chosen = choice(&tab, Some("日本"), "Markdown", None, "日本");
    tab.source_complete("Ideas", &chosen).unwrap();
    assert_eq!(tab.editor.text(), "日本語😀é [x](日本.md)");
}

/// UC-005 A23, BR-018, UC-004 BR-005 (chaos): a note named `C#` (no space,
/// so OQ-248 does not apply) is written so the link leads to the note: the
/// `#` of its place would start a heading.
#[test]
fn uc_005_a23_chaos_place_with_hash() {
    let mut tab = typed("[](pl", NoteFormat::Markdown);
    let chosen = choice(&tab, Some("C#"), "Markdown", None, "C#");
    tab.source_complete("Ideas", &chosen).unwrap();
    let text = tab.editor.text();
    assert!(
        !text.contains("](C#"),
        "{text:?}: the `#` starts a heading, the link leads to no note"
    );
}

/// UC-005 BR-018, OQ-248: a title with brackets is escaped in the link's
/// text, in both note formats.
#[test]
fn uc_005_br_018_title_with_brackets() {
    assert_eq!(
        written("[](pl", NoteFormat::Markdown, "Plan [v2]"),
        "[Plan \\[v2\\]](Plan.md)"
    );
    assert_eq!(
        written("xref:pl", NoteFormat::AsciiDoc, "Plan [v2]"),
        "xref:Plan.md[Plan [v2\\]]"
    );
}

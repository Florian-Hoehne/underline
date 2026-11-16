use super::*;

const MD: NoteFormat = NoteFormat::Markdown;
const ADOC: NoteFormat = NoteFormat::AsciiDoc;

/// `source` after `change`; panics when it is refused.
fn changed(source: &str, format: NoteFormat, change: &AttributeChange) -> String {
    let edit = head_edit(source, format, change).unwrap();
    let mut text = source.to_owned();
    text.replace_range(edit.range, &edit.text);
    text
}

fn set(name: &str, value: AttributeValue) -> AttributeChange {
    AttributeChange::Set {
        name: name.into(),
        value,
    }
}

fn add(name: &str, value: AttributeValue) -> AttributeChange {
    AttributeChange::Add {
        name: name.into(),
        value,
    }
}

fn remove(name: &str) -> AttributeChange {
    AttributeChange::Remove { name: name.into() }
}

fn text(value: &str) -> AttributeValue {
    AttributeValue::Text(value.into())
}

fn list(items: &[&str]) -> AttributeValue {
    AttributeValue::List(items.iter().map(|&s| s.to_owned()).collect())
}

/// What the head shows: names, kinds and values.
fn shown(source: &str, format: NoteFormat) -> Vec<(String, AttributeKind, String)> {
    note_head(source, format)
        .attributes
        .into_iter()
        .map(|a| (a.name, a.kind, a.value))
        .collect()
}

const PLAN: &str = "---\nstatus: draft\ndue: 2026-12-01\n---\n# Plan\n";

/// TC-024-01: the front matter of a Markdown note as attributes; a change
/// of one value writes only it.
#[test]
fn tc024_01_markdown_attributes() {
    assert_eq!(
        shown(PLAN, MD),
        [
            ("status".into(), AttributeKind::Text, "draft".into()),
            ("due".into(), AttributeKind::Date, "2026-12-01".into()),
        ]
    );
    let head = note_head(PLAN, MD);
    assert_eq!(head.attributes[0].line, 2);
    assert_eq!(head.front_matter, Some(0..38));
    assert_eq!(
        changed(PLAN, MD, &set("status", text("final"))),
        "---\nstatus: final\ndue: 2026-12-01\n---\n# Plan\n"
    );
}

/// TC-024-02: an AsciiDoc attribute is added below the others, in lower
/// case.
#[test]
fn tc024_02_asciidoc_attributes() {
    let guide = "= Guide\n:author: Ann\n\nText\n";
    assert_eq!(
        shown(guide, ADOC),
        [("author".into(), AttributeKind::Text, "Ann".into())]
    );
    let added = changed(guide, ADOC, &add("Edition", text("2")));
    assert_eq!(added, "= Guide\n:author: Ann\n:edition: 2\n\nText\n");
    assert_eq!(
        shown(&added, ADOC)[1],
        ("edition".into(), AttributeKind::Text, "2".into())
    );
    assert_eq!(
        head_edit(&added, ADOC, &add("EDITION", text("3"))),
        Err(AttributeError::Taken("edition".into()))
    );
    assert_eq!(
        changed(&added, ADOC, &set("Edition", text("3"))),
        "= Guide\n:author: Ann\n:edition: 3\n\nText\n"
    );
}

/// TC-024-03, BR-003: a new attribute follows the last one; comments,
/// blank lines and spaces stay byte for byte.
#[test]
fn tc024_03_only_the_attribute_changes() {
    let mixed = "---\n# a comment\ntags: [a, b]\n\ntitle:  \"Mixed\"\n---\nBody\n";
    assert_eq!(
        changed(mixed, MD, &add("owner", text("Ann"))),
        "---\n# a comment\ntags: [a, b]\n\ntitle:  \"Mixed\"\nowner: Ann\n---\nBody\n"
    );
    assert_eq!(
        changed(mixed, MD, &set("title", text("Other"))),
        "---\n# a comment\ntags: [a, b]\n\ntitle:  Other\n---\nBody\n"
    );
    assert_eq!(
        changed(mixed, MD, &remove("tags")),
        "---\n# a comment\n\ntitle:  \"Mixed\"\n---\nBody\n"
    );
}

/// TC-024-04, BR-002: the kinds of values, and a switch written back.
#[test]
fn tc024_04_kinds_of_values() {
    let kinds = "---\ntitle: Kinds\ntags: [a, b]\nweight: 2.5\ndue: 2026-12-01\ndone: false\n---\n";
    let kinds_shown: Vec<AttributeKind> = shown(kinds, MD).into_iter().map(|a| a.1).collect();
    assert_eq!(
        kinds_shown,
        [
            AttributeKind::Text,
            AttributeKind::List,
            AttributeKind::Number,
            AttributeKind::Date,
            AttributeKind::YesNo
        ]
    );
    assert_eq!(note_head(kinds, MD).attributes[1].items, ["a", "b"]);
    assert_eq!(
        changed(kinds, MD, &set("done", AttributeValue::YesNo(true))),
        kinds.replace("done: false", "done: true")
    );
    assert_eq!(
        changed(
            kinds,
            MD,
            &set("weight", AttributeValue::Number("-3.25".into()))
        ),
        kinds.replace("2.5", "-3.25")
    );
}

/// TC-024-05, A1, A2: the first attribute makes the head, removing the
/// last one removes it, in both formats.
#[test]
fn tc024_05_first_and_last_attribute() {
    let plain = "# Plain\n";
    let with = changed(plain, MD, &add("status", text("draft")));
    assert_eq!(with, "---\nstatus: draft\n---\n# Plain\n");
    assert_eq!(changed(&with, MD, &remove("status")), plain);

    for plain in ["= Plain\n", "= Plain"] {
        let with = changed(plain, ADOC, &add("status", text("draft")));
        assert_eq!(with.trim_end(), "= Plain\n:status: draft");
        assert_eq!(changed(&with, ADOC, &remove("status")), plain);
    }
    // Without a title the attribute goes first.
    let untitled = "Some text\n";
    let with = changed(untitled, ADOC, &add("a", text("b")));
    assert_eq!(with, ":a: b\nSome text\n");
    assert_eq!(changed(&with, ADOC, &remove("a")), untitled);
}

/// A2: a front matter that keeps a comment is not removed.
#[test]
fn uc024_a2_a_comment_keeps_the_front_matter() {
    let note = "---\n# keep me\nstatus: draft\n---\nText\n";
    assert_eq!(
        changed(note, MD, &remove("status")),
        "---\n# keep me\n---\nText\n"
    );
}

/// TC-024-06, A3, BR-004: names not allowed or taken.
#[test]
fn tc024_06_names() {
    let refused = |name: &str| head_edit(PLAN, MD, &add(name, text("x"))).unwrap_err();
    assert_eq!(refused("my status"), AttributeError::Space);
    assert_eq!(refused("status"), AttributeError::Taken("status".into()));
    assert_eq!(refused(""), AttributeError::NoName);
    assert_eq!(refused("   "), AttributeError::NoName);
    assert_eq!(refused("1st"), AttributeError::Start);
    assert_eq!(refused("a:b"), AttributeError::Character(':'));
    assert_eq!(refused(&"a".repeat(101)), AttributeError::TooLong);
    assert!(head_edit(PLAN, MD, &add(&"ä".repeat(100), text("x"))).is_ok());
    assert_eq!(
        AttributeError::Space.to_string(),
        "A name cannot hold a space."
    );
}

/// TC-024-07, A4: a value that does not fit its kind writes nothing.
#[test]
fn tc024_07_values_that_do_not_fit() {
    let kinds = "---\nweight: 2.5\ndue: 2026-12-01\n---\n";
    assert_eq!(
        head_edit(
            kinds,
            MD,
            &set("weight", AttributeValue::Number("soon".into()))
        ),
        Err(AttributeError::NotANumber)
    );
    assert_eq!(
        head_edit(
            kinds,
            MD,
            &set("due", AttributeValue::Date("2026-13-01".into()))
        ),
        Err(AttributeError::NotADate)
    );
    assert_eq!(
        head_edit(
            kinds,
            MD,
            &set("due", AttributeValue::Date("2025-02-29".into()))
        ),
        Err(AttributeError::NotADate)
    );
    assert!(head_edit(
        kinds,
        MD,
        &set("due", AttributeValue::Date("2024-02-29".into()))
    )
    .is_ok());
    assert_eq!(
        head_edit(kinds, MD, &set("weight", text("a\nb"))),
        Err(AttributeError::LineBreak)
    );
    assert_eq!(
        head_edit("= T\n", ADOC, &add("n", AttributeValue::Number("2".into()))),
        Err(AttributeError::Kind)
    );
}

/// TC-024-08, A5: a front matter with a line without a name is told with
/// its line and never changed.
#[test]
fn tc024_08_front_matter_that_cannot_be_read() {
    let broken = "---\nstatus: draft\njust text\n---\nBody\n";
    let head = note_head(broken, MD);
    assert!(head.attributes.is_empty());
    let problem = head.problem.unwrap();
    assert_eq!(problem.line, 3, "{problem:?}");
    assert!(!problem.reason.is_empty(), "{problem:?}");
    assert_eq!(problem.markup, "---\nstatus: draft\njust text\n---");
    for change in [
        set("status", text("x")),
        add("a", text("b")),
        remove("status"),
    ] {
        assert_eq!(
            head_edit(broken, MD, &change),
            Err(AttributeError::Unreadable)
        );
    }
    assert_eq!(readable_front_matter(broken), None);
    // Twice the same name is no YAML GitHub reads either.
    assert!(note_head("---\na: 1\na: 2\n---\n", MD).problem.is_some());
    // A list instead of names and values.
    assert!(note_head("---\n- a\n---\n", MD).problem.is_some());
    // An unclosed quote.
    assert!(note_head("---\na: \"x\n---\n", MD).problem.is_some());
}

/// TC-024-09, A6, OQ-215: a nested value is its markup, kept when another
/// attribute changes, and refused itself.
#[test]
fn tc024_09_nested_values() {
    let nested = "---\nauthor:\n  name: Ann\n  mail: ann@example.org\nstatus: draft\n---\n";
    let head = note_head(nested, MD);
    assert_eq!(head.attributes[0].kind, AttributeKind::Nested);
    assert_eq!(
        head.attributes[0].value,
        "author:\n  name: Ann\n  mail: ann@example.org"
    );
    assert_eq!(
        changed(nested, MD, &set("status", text("final"))),
        nested.replace("draft", "final")
    );
    assert_eq!(
        head_edit(nested, MD, &set("author", text("x"))),
        Err(AttributeError::Nested("author".into()))
    );
    // Removing a nested attribute removes its lines.
    assert_eq!(
        changed(nested, MD, &remove("author")),
        "---\nstatus: draft\n---\n"
    );
    for other in [
        "---\nx: {a: 1}\n---\n",
        "---\nx: |\n  two\n  lines\n---\n",
        "---\nx: &anchor 1\n---\n",
        "---\nx:\n  - a: 1\n---\n",
        "---\nx: [a, [b]]\n---\n",
    ] {
        assert_eq!(
            note_head(other, MD).attributes[0].kind,
            AttributeKind::Nested,
            "{other}"
        );
    }
}

/// TC-024-11, BR-005: an item added to the list `tags`, written as the
/// list is written.
#[test]
fn tc024_11_tags_list() {
    let trip = "---\ntags: [travel]\n---\nsee #greece\n";
    assert_eq!(
        changed(trip, MD, &set("tags", list(&["travel", "ferry"]))),
        "---\ntags: [travel, ferry]\n---\nsee #greece\n"
    );
    let block = "---\ntags:\n  - travel # first\nnext: 1\n---\n";
    assert_eq!(note_head(block, MD).attributes[0].items, ["travel"]);
    assert_eq!(
        changed(block, MD, &set("tags", list(&["travel", "ferry"]))),
        "---\ntags:\n  - travel\n  - ferry\nnext: 1\n---\n"
    );
    let adoc = "= Trip\n:tags: travel\n";
    assert_eq!(
        changed(adoc, ADOC, &set("tags", list(&["travel", " ferry ", ""]))),
        "= Trip\n:tags: travel, ferry\n"
    );
}

/// BR-003, YAML: quotes, comments after a value, and texts that would read
/// as another kind are written so they read back the same.
#[test]
fn uc024_br003_yaml_quoting_and_comments() {
    let note = "---\na: 'it''s'   # c\nb: \"x\\\"y\"\nc:\n---\n";
    assert_eq!(
        shown(note, MD),
        [
            ("a".into(), AttributeKind::Text, "it's".into()),
            ("b".into(), AttributeKind::Text, "x\"y".into()),
            ("c".into(), AttributeKind::Text, String::new()),
        ]
    );
    assert_eq!(
        changed(note, MD, &set("a", text("plain"))),
        "---\na: plain   # c\nb: \"x\\\"y\"\nc:\n---\n"
    );
    assert_eq!(
        changed(note, MD, &set("c", text("now"))),
        "---\na: 'it''s'   # c\nb: \"x\\\"y\"\nc: now\n---\n"
    );
    for (value, written) in [
        ("true", "\"true\""),
        ("12", "\"12\""),
        ("2026-01-01", "\"2026-01-01\""),
        ("a: b", "\"a: b\""),
        ("#x", "\"#x\""),
        ("say \"hi\"", "say \"hi\""),
        ("", "\"\""),
        ("- x", "\"- x\""),
        ("null", "\"null\""),
    ] {
        let after = changed(note, MD, &set("a", text(value)));
        assert!(
            after.contains(&format!("a: {written}   # c")),
            "{value}: {after}"
        );
        let head = note_head(&after, MD);
        assert!(head.problem.is_none(), "{value}: {after}");
        assert_eq!(head.attributes[0].value, value, "{after}");
        assert_eq!(head.attributes[0].kind, AttributeKind::Text, "{after}");
    }
    let listed = changed(note, MD, &set("a", list(&["x, y", "[z]", "w"])));
    let head = note_head(&listed, MD);
    assert!(head.problem.is_none(), "{listed}");
    assert_eq!(head.attributes[0].items, ["x, y", "[z]", "w"]);
}

/// A value after a name with a comment only.
#[test]
fn uc024_set_after_a_comment() {
    let note = "---\na: # later\n---\n";
    assert_eq!(
        changed(note, MD, &set("a", text("b"))),
        "---\na: b # later\n---\n"
    );
}

/// CRLF: added lines take the note's line breaks; others stay.
#[test]
fn uc024_crlf_notes() {
    let note = "---\r\nstatus: draft\r\n---\r\nText\r\n";
    assert_eq!(
        shown(note, MD),
        [("status".into(), AttributeKind::Text, "draft".into())]
    );
    assert_eq!(
        changed(
            note,
            MD,
            &add("due", AttributeValue::Date("2026-12-01".into()))
        ),
        "---\r\nstatus: draft\r\ndue: 2026-12-01\r\n---\r\nText\r\n"
    );
    assert_eq!(
        changed(note, MD, &set("status", text("final"))),
        "---\r\nstatus: final\r\n---\r\nText\r\n"
    );
    assert_eq!(changed(note, MD, &remove("status")), "Text\r\n");
    let plain = "# T\r\n";
    assert_eq!(
        changed(plain, MD, &add("a", text("b"))),
        "---\r\na: b\r\n---\r\n# T\r\n"
    );
    let adoc = "= T\r\n:a: b\r\n\r\nText\r\n";
    assert_eq!(
        changed(adoc, ADOC, &add("c", text("d"))),
        "= T\r\n:a: b\r\n:c: d\r\n\r\nText\r\n"
    );
    assert_eq!(
        changed(adoc, ADOC, &set("a", text("x"))),
        "= T\r\n:a: x\r\n\r\nText\r\n"
    );
    assert_eq!(changed(adoc, ADOC, &remove("a")), "= T\r\n\r\nText\r\n");
}

/// Multibyte names and values: the places are bytes, the rest stays.
#[test]
fn uc024_multibyte() {
    let note = "\u{feff}---\ntitel: Grüße 🙂\nstraße: 1\n---\nÜber\n";
    let head = note_head(note, MD);
    assert_eq!(head.attributes[0].value, "Grüße 🙂");
    assert_eq!(&note[head.attributes[0].value_span.clone()], "Grüße 🙂");
    assert_eq!(head.attributes[1].name, "straße");
    assert_eq!(
        changed(note, MD, &set("titel", text("日本語"))),
        "\u{feff}---\ntitel: 日本語\nstraße: 1\n---\nÜber\n"
    );
    let bom = "\u{feff}Text\n";
    assert_eq!(
        changed(bom, MD, &add("ä", text("ö"))),
        "\u{feff}---\nä: ö\n---\nText\n"
    );
}

/// AsciiDoc: the header ends at a blank line; an entry below it is no
/// attribute; author lines, comments and unset entries are kept.
#[test]
fn uc024_br001_asciidoc_header() {
    let note = "// note\n= Doc\nAnn Author <ann@example.org>\n:Status: draft\n// why\n:toc!:\n:tags: a, b\n\n:later: no\n";
    assert_eq!(
        shown(note, ADOC),
        [
            ("status".into(), AttributeKind::Text, "draft".into()),
            ("tags".into(), AttributeKind::List, "a, b".into()),
        ]
    );
    assert_eq!(
        changed(note, ADOC, &add("x", text("y"))),
        note.replace(":tags: a, b\n", ":tags: a, b\n:x: y\n")
    );
    assert_eq!(
        changed(note, ADOC, &set("status", text(""))),
        note.replace(":Status: draft", ":Status:")
    );
    let authored = "= Doc\nAnn Author\n\nText\n";
    assert_eq!(
        changed(authored, ADOC, &add("a", text("b"))),
        "= Doc\nAnn Author\n:a: b\n\nText\n"
    );
    // A value spread over lines is changed only in the source view.
    let spread = "= Doc\n:long: one \\\ntwo\n";
    assert_eq!(
        note_head(spread, ADOC).attributes[0].kind,
        AttributeKind::Nested
    );
}

/// No front matter, no attribute; a change of one that is not there is
/// refused (A7: the note changed meanwhile).
#[test]
fn uc024_a7_missing_attribute() {
    assert_eq!(note_head("Text\n", MD), NoteHead::default());
    assert_eq!(
        head_edit("Text\n", MD, &set("a", text("b"))),
        Err(AttributeError::Missing("a".into()))
    );
    assert_eq!(
        head_edit(PLAN, MD, &remove("nope")),
        Err(AttributeError::Missing("nope".into()))
    );
    // `---` that is not closed is no front matter.
    assert!(note_head("---\na: b\n", MD).attributes.is_empty());
    // An empty front matter holds no attributes and can take one.
    let empty = "---\n---\nText\n";
    assert!(note_head(empty, MD).problem.is_none());
    assert_eq!(
        changed(empty, MD, &add("a", text("b"))),
        "---\na: b\n---\nText\n"
    );
}

/// Round trip: every edit leaves the bytes outside its range unchanged and
/// the head readable.
#[test]
fn uc024_round_trips_keep_the_rest() {
    let notes = [
        "---\n# c\na: 1 # one\n\nb: [x, 'y']\nc:\n  - d\n  - e\nn:\n  k: v\n---\nText\n",
        "---\r\na: \"q\"\r\n---\r\n",
    ];
    for note in notes {
        let head = note_head(note, MD);
        assert!(head.problem.is_none(), "{note}");
        for attribute in &head.attributes {
            if attribute.kind == AttributeKind::Nested {
                continue;
            }
            let value = match attribute.kind {
                AttributeKind::List => list(&["p", "q r"]),
                _ => text("new value"),
            };
            let edit = head_edit(note, MD, &set(&attribute.name, value)).unwrap();
            let mut whole = note.to_owned();
            whole.replace_range(edit.range.clone(), &edit.text);
            assert_eq!(&whole[..edit.range.start], &note[..edit.range.start]);
            assert_eq!(
                &whole[edit.range.start + edit.text.len()..],
                &note[edit.range.end..]
            );
            let again = note_head(&whole, MD);
            assert!(again.problem.is_none(), "{whole}");
            assert_eq!(again.attributes.len(), head.attributes.len(), "{whole}");
        }
    }
}

// ---------------------------------------------------------------------------
// Test monkey, package 6a: each of these fails on the code as it stands.
// ---------------------------------------------------------------------------

/// UC-024 BR-003, A5: after a change the front matter is still YAML GitHub
/// reads and the value reads back as written. Characters YAML does not
/// allow in a plain or quoted scalar (the line separators U+2028 and
/// U+2029, which YAML 1.1 reads as line breaks, and the non-characters
/// U+FFFE and U+FFFF) must be escaped, not written as they are.
#[test]
fn uc024_br003_text_with_yaml_breaking_characters_stays_readable() {
    for value in [
        "a\u{2028}b",
        "a\u{2029}b",
        "a\u{fffe}b",
        "a\u{ffff}b",
        "\u{2028}",
    ] {
        let out = changed(PLAN, MD, &add("note", text(value)));
        let head = note_head(&out, MD);
        assert!(
            head.problem.is_none(),
            "{value:?} made the front matter unreadable: {:?} in {out:?}",
            head.problem
        );
        assert_eq!(head.attributes.len(), 3, "{value:?}: {out:?}");
        assert_eq!(head.attributes[2].value, value, "{out:?}");
        // The other lines stay as they were.
        assert!(out.starts_with("---\nstatus: draft\ndue: 2026-12-01\n"));
    }
}

/// UC-024 BR-001, BR-002 (OQ-264): an AsciiDoc value that ends in ` \` goes
/// on in the next line, so writing one would swallow the line after the
/// new attribute: the note's header or body. The text is refused or
/// written so that it reads back as that text on one line.
#[test]
fn uc024_br002_asciidoc_text_ending_in_a_continuation_mark() {
    let note = "= T\n:a: 1\n\nbody\n";
    let Ok(out) = head_edit(note, ADOC, &add("k", text("a \\"))).map(|edit| {
        let mut out = note.to_owned();
        out.replace_range(edit.range, &edit.text);
        out
    }) else {
        return;
    };
    let head = note_head(&out, ADOC);
    let k = head.attribute("k", ADOC).expect("the attribute is read");
    assert_eq!(k.kind, AttributeKind::Text, "{out:?}");
    assert_eq!(k.value, "a \\", "{out:?}");
    assert_eq!(
        &out[k.span.clone()],
        ":k: a \\\n",
        "the attribute spans its own line only: {out:?}"
    );
}

/// UC-024 BR-001 (C-004), UC-001 NFR-007: a byte order mark at the start of
/// an AsciiDoc note is not part of the first line; the title is found and
/// a new attribute follows it, and one written is read back.
#[test]
fn uc024_br001_asciidoc_note_with_a_byte_order_mark() {
    let note = "\u{feff}= T\n";
    let out = changed(note, ADOC, &add("a", text("1")));
    assert_eq!(out, "\u{feff}= T\n:a: 1\n", "after the title");
    let head = note_head(&out, ADOC);
    assert_eq!(shown(&out, ADOC).len(), 1, "{head:?}");
    // A second one with the same name is refused, as without the mark.
    assert_eq!(
        head_edit(&out, ADOC, &add("a", text("2"))),
        Err(AttributeError::Taken("a".into()))
    );
    // Remove what was added: the original text again.
    assert_eq!(changed(&out, ADOC, &remove("a")), note);
}

/// UC-024 BR-002: an item of an AsciiDoc list holds no comma, as the items
/// are separated by commas; an item that would be read back as two is
/// refused, or the list reads back as written.
#[test]
fn uc024_br002_asciidoc_list_item_with_a_comma() {
    let note = "= T\n";
    let Ok(edit) = head_edit(note, ADOC, &add("tags", list(&["a,b", "c"]))) else {
        return;
    };
    let mut out = note.to_owned();
    out.replace_range(edit.range, &edit.text);
    let head = note_head(&out, ADOC);
    assert_eq!(head.attributes[0].items, ["a,b", "c"], "{out:?}");
}

/// UC-024 BR-003, A5: a front matter whose mapping is indented as a whole
/// is YAML GitHub reads; a new attribute written at the start of a line
/// would make it two kinds of indentation, so it is refused or indented
/// the same. The front matter stays readable.
#[test]
fn uc024_br003_add_to_an_indented_front_matter_keeps_it_readable() {
    let note = "---\n  status: draft\n  due: 2026-12-01\n---\nBody\n";
    assert!(note_head(note, MD).problem.is_none());
    let Ok(edit) = head_edit(note, MD, &add("note", text("x"))) else {
        return;
    };
    let mut out = note.to_owned();
    out.replace_range(edit.range, &edit.text);
    let head = note_head(&out, MD);
    assert!(head.problem.is_none(), "{:?} in {out:?}", head.problem);
}

/// UC-024 BR-004: the name an AsciiDoc note writes (in lower case) is one
/// the header reads back: `İ` in lower case is `i` and a combining dot,
/// which an attribute name does not hold.
#[test]
fn uc024_br004_asciidoc_name_in_lower_case_reads_back() {
    let note = "= T\n";
    let Ok(edit) = head_edit(note, ADOC, &add("İx", text("v"))) else {
        return;
    };
    let mut out = note.to_owned();
    out.replace_range(edit.range, &edit.text);
    let written = checked_name("İx", ADOC).unwrap();
    assert!(
        note_head(&out, ADOC).attribute(&written, ADOC).is_some(),
        "the attribute {written:?} written in {out:?} is not read back"
    );
}

/// UC-024 A6: a key written in double quotes with an escaped quote is a key
/// of the mapping; it does not turn the entry before it into a nested
/// value, nor does it disappear.
#[test]
fn uc024_a6_quoted_key_with_an_escaped_quote() {
    let note = "---\na: 1\n\"x\\\"y\": 2\nb: 3\n---\n";
    assert!(note_head(note, MD).problem.is_none());
    let kinds: Vec<_> = note_head(note, MD)
        .attributes
        .into_iter()
        .map(|a| (a.name, a.kind))
        .collect();
    assert_eq!(
        kinds,
        [
            ("a".to_owned(), AttributeKind::Number),
            ("x\"y".to_owned(), AttributeKind::Number),
            ("b".to_owned(), AttributeKind::Number)
        ]
    );
}

// ---------------------------------------------------------------------------
// Review fixes, package 6a.
// ---------------------------------------------------------------------------

/// UC-024 BR-003 (review): an indented mapping is read, and a new name takes
/// its indentation.
#[test]
fn uc024_br003_indented_front_matter_is_read_and_extended() {
    let note = "---\n  status: draft\n  tags:\n    - a\n---\n";
    assert_eq!(
        shown(note, MD),
        [
            ("status".into(), AttributeKind::Text, "draft".into()),
            ("tags".into(), AttributeKind::List, "a".into()),
        ]
    );
    assert_eq!(
        changed(note, MD, &add("x", text("z"))),
        "---\n  status: draft\n  tags:\n    - a\n  x: z\n---\n"
    );
}

/// UC-024 BR-003 (review): an AsciiDoc entry without a value, whose name is
/// written with a letter whose lower case is shorter (the Kelvin sign), takes
/// a value right after its own name.
#[test]
fn uc024_br003_asciidoc_value_after_a_name_in_another_case() {
    let note = "= T\n:\u{212a}ey: v\n";
    assert_eq!(
        changed(note, ADOC, &set("key", text(""))),
        "= T\n:\u{212a}ey:\n"
    );
}

/// UC-024 A2, A6 (review): removing a block scalar removes its lines, an
/// indented `#` line among them; a comment at the mapping's indentation
/// after it stays.
#[test]
fn uc024_a6_remove_a_block_scalar_with_a_comment_line() {
    let note = "---\ntext: |\n  one\n  # c\n# kept\nnext: 1\n---\n";
    assert_eq!(
        changed(note, MD, &remove("text")),
        "---\n# kept\nnext: 1\n---\n"
    );
}

/// UC-024 BR-003 (review): texts GitHub's YAML (1.1) reads as numbers or
/// times are quoted; `1e5` is no number to offer, as YAML 1.1 and 1.2 read
/// it differently.
#[test]
fn uc024_br003_yaml_1_1_readings_are_quoted() {
    for value in [
        "1:30",
        "1_000",
        "0b101",
        "2026-11-11 10:00",
        "1e5",
        "0x1F",
        "y",
        "<<",
    ] {
        let out = changed(PLAN, MD, &set("status", text(value)));
        assert!(
            out.contains(&format!("status: \"{value}\"")),
            "{value}: {out}"
        );
        assert_eq!(note_head(&out, MD).attributes[0].value, value);
    }
    assert_eq!(
        head_edit(
            PLAN,
            MD,
            &set("status", AttributeValue::Number("1e5".into()))
        ),
        Err(AttributeError::NotANumber)
    );
    assert_eq!(plain_kind("1e5"), AttributeKind::Text);
}

/// NFR-007 (review): a front matter of 20,000 names is read and changed in
/// linear time.
#[test]
fn uc024_many_names_in_linear_time() {
    let mut note = String::from("---\n");
    for n in 0..20_000 {
        note.push_str(&format!("k{n}: {n}\n"));
    }
    note.push_str("---\n");
    let start = std::time::Instant::now();
    assert_eq!(note_head(&note, MD).attributes.len(), 20_000);
    assert!(head_edit(&note, MD, &set("k19999", text("x"))).is_ok());
    assert!(start.elapsed().as_secs() < 5, "{:?}", start.elapsed());
}

/// UC-024 BR-005, OQ-270 (app test): an item of `tags` that is no tag is
/// refused, about the value.
#[test]
fn oq270_tags_items_must_be_tags() {
    let trip = "---\ntags: [travel]\n---\n";
    let refused = head_edit(trip, MD, &set("tags", list(&["travel", "two words"]))).unwrap_err();
    assert_eq!(refused, AttributeError::NotATag("two words".into()));
    assert_eq!(refused.about(), AttributeField::Value);
    assert!(head_edit(trip, MD, &set("tags", list(&["#travel", "a/b/"]))).is_ok());
    assert_eq!(
        head_edit("= T\n", ADOC, &add("Tags", text("2026"))),
        Err(AttributeError::NotATag("2026".into()))
    );
    assert_eq!(AttributeError::Space.about(), AttributeField::Name);
}

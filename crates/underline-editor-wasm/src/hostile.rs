//! ADR-025, step B3c: no call of the webview panics, whatever its
//! arguments: offsets past the end or inside a character, a range backwards,
//! a revision that is not the text's, huge lines and pages, JSON that is not
//! the DTO. A panic in WASM would stop the tab for good.

use std::panic::{catch_unwind, AssertUnwindSafe};

use serde_json::json;

use crate::calls::Core;

/// Texts with surrogate pairs (emoji), CRLF, and blocks of every kind.
const TEXTS: [(&str, &str); 3] = [
    (
        "# A 😀\r\n\nx😀y *b* `c`\n\n- [ ] l\n\n```mermaid\ngraph TD\n```\n\n| a |\n|---|\n| 1 |\n",
        "Markdown",
    ),
    (
        "= A\n\nNOTE: 😀\n\n.T\nimage::i.png[I]\n\n----\nx\n----\n",
        "AsciiDoc",
    ),
    ("", "Markdown"),
];

/// Offsets around the text's ends and inside its emoji.
const OFFSETS: [u32; 9] = [0, 1, 5, 6, 7, 12, 13, 10_000, u32::MAX];

/// Revisions: the first ones, and ones the text never had.
const REVS: [u32; 4] = [0, 1, 999, u32::MAX];

fn keys() -> Vec<serde_json::Value> {
    let names = [
        "Backspace",
        "Delete",
        "Enter",
        "Tab",
        "ArrowLeft",
        "ArrowRight",
        "ArrowUp",
        "ArrowDown",
        "Home",
        "End",
        "PageUp",
        "PageDown",
        "Escape",
        "BracketLeft",
        "Z",
        "Y",
        "K",
        "U",
        "W",
    ];
    let mut keys = Vec::new();
    for name in names {
        for (ctrl, shift) in [(false, false), (true, false), (true, true), (false, true)] {
            keys.push(
                json!({ "kind": "key", "key": name, "ctrl": ctrl, "alt": false,
                "shift": shift, "meta": false, "page": u32::MAX }),
            );
        }
    }
    keys
}

/// Runs `work` on a fresh tab of each text in each mode; the panics, with
/// what was called.
fn panics(what: &str, work: impl Fn(&mut Core)) -> Vec<String> {
    let mut found = Vec::new();
    for (text, format) in TEXTS {
        for mode in ["Normal", "Vim", "Nano"] {
            let mut core = Core::open("Home", text, format, false).expect("a format");
            core.set_mode(mode).expect("a mode");
            if catch_unwind(AssertUnwindSafe(|| work(&mut core))).is_err() {
                found.push(format!("{what} on {format} {text:?} in {mode}"));
            }
        }
    }
    found
}

/// ADR-025: no call panics with offsets, ranges, revisions, lines or pages
/// that do not fit the text.
#[test]
fn adr025_no_call_panics_with_arguments_that_do_not_fit() {
    let mut found = Vec::new();
    found.extend(panics("select", |core| {
        for rev in REVS {
            for a in OFFSETS {
                for h in OFFSETS {
                    let _ = core.select(rev, a, h);
                    let _ = core.clipboard("\"cut\"", "null");
                    let _ = core.copy_on_select(true);
                    let _ = core.insert(
                        &json!({ "kind": "dropped", "place": "😀.png", "image": true }).to_string(),
                    );
                    let _ = core.select(rev, h, a);
                    let _ = core.input(&json!({ "kind": "text", "text": "😀\r\n" }).to_string());
                }
            }
        }
    }));
    found.extend(panics("fold", |core| {
        for line in [0, 1, 5, 1000, u32::MAX] {
            let _ = core.fold(line);
            let _ = core.views(f64::from(line));
        }
    }));
    // UC-008, M-13: the searches and the find field over every place.
    found.extend(panics("find", |core| {
        for text in ["😀", "\r", "\"", "a b", "", "(", "^", "$0", "\\n", ".*"] {
            for regex in [false, true] {
                let spec = json!({
                    "text": text,
                    "switches": { "matchCase": regex, "wholeWord": true, "regex": regex },
                    "oneText": regex,
                    "shownOnly": !regex,
                })
                .to_string();
                let _ = core.set_search(&spec);
                let _ = core.set_search("null");
                let _ = core.set_search(&spec);
                let _ = core.views(2.0);
                for a in OFFSETS {
                    let _ = core.select(0, a, a);
                    for step in ["\"first\"", "\"next\"", "\"previous\""] {
                        let _ = core.find(&spec, step);
                    }
                    let _ = core.find_status("null");
                    let _ = core.find_status("\"$1\"");
                    let _ = core.replace(&spec, "$0x", false);
                    let _ = core.facade_replace(&spec, "😀", a % 2 == 0);
                    let _ = core.replace(&spec, "", true);
                    let _ = core.input(&json!({ "kind": "text", "text": "😀\r\n" }).to_string());
                }
            }
        }
    }));
    found.extend(panics("apply", |core| {
        for base in REVS {
            for from in OFFSETS {
                for to in OFFSETS {
                    let changes =
                        json!([{ "from": from, "to": to, "insert": "😀", "newStep": true }]);
                    let _ = core.apply(base, &changes.to_string(), to, from, true);
                }
            }
        }
    }));
    found.extend(panics("input", |core| {
        for key in keys() {
            let _ = core.input(&key.to_string());
            let _ = core.views(1.0);
        }
        for tool in [
            "heading", "bold", "italic", "code", "list", "quote", "link", "image", "table",
        ] {
            let _ = core.input(&json!({ "kind": "tool", "tool": tool }).to_string());
        }
    }));
    found.extend(panics("facade", |core| {
        for rev in REVS {
            for a in OFFSETS {
                for h in [0, 7, u32::MAX] {
                    let at = json!({ "rev": rev, "anchor": a, "head": h }).to_string();
                    let rows = json!({ "at": a, "up": h, "down": a, "start": h, "end": a })
                        .to_string();
                    for input in [
                        json!({ "kind": "text", "text": "*😀*" }),
                        json!({ "kind": "key", "key": "Enter", "ctrl": false, "alt": false, "shift": false, "meta": false }),
                        json!({ "kind": "key", "key": "Backspace", "ctrl": false, "alt": false, "shift": false, "meta": false }),
                        json!({ "kind": "key", "key": "ArrowUp", "ctrl": false, "alt": false, "shift": true, "meta": false }),
                        json!({ "kind": "tool", "tool": "bold" }),
                    ] {
                        let _ = core.facade_input(&at, &input.to_string(), &rows, Some("😀"));
                    }
                    let _ = core.facade_clipboard(&at, "\"cut\"", false, "null");
                    let pasted = json!({ "text": "a\r\n😀", "markup": "# a\r\n\r\n😀", "image": false });
                    let _ = core.facade_clipboard(&at, "\"paste\"", false, &pasted.to_string());
                    let _ = core.facade_insert(&at, &json!({ "kind": "link", "address": "a b", "label": null }).to_string());
                    let _ = core.facade_insert(&at, &json!({ "kind": "text", "text": "\r\n" }).to_string());
                    let _ = core.copy_on_select(true);
                }
            }
        }
        let _ = core.facade_state();
        let _ = core.facade_head();
        let _ = core.views(f64::NAN);
    }));
    // FR-080: link completion at every place, with choices that fit or not.
    found.extend(panics("complete", |core| {
        for typed in ["[x](😀", "\r\n[[p#h", "xref:a", "<<b,", "[]("] {
            let _ = core.input(&json!({ "kind": "text", "text": typed }).to_string());
            for rev in REVS {
                for a in OFFSETS {
                    let _ = core.select(rev, a, a);
                    let _ = core.link_context(a % 2 == 0);
                    let at = json!({ "rev": rev, "anchor": a, "head": a }).to_string();
                    let _ = core.facade_link_context(&at, true);
                    for (note, format) in [
                        (json!("😀/../x"), json!("Markdown")),
                        (json!(null), json!(null)),
                        (json!("a"), json!("Rtf")),
                        (json!("a"), json!(null)),
                    ] {
                        let choice = json!({ "rev": rev, "note": note, "format": format,
                            "anchor": "😀", "label": "]\r\n" })
                        .to_string();
                        let _ = core.complete_link(&choice);
                        let _ = core.facade_complete_link(&at, &choice);
                    }
                }
            }
        }
    }));
    found.extend(comfort_panics());
    assert!(found.is_empty(), "panics:\n{}", found.join("\n"));
}

/// FR-081 to FR-086 (package 4): close pairs, moving, duplicating and
/// indenting, and going to a line, at every place, in both views, with
/// close pairs on and off. Each place gets a fresh tab, so copies made by
/// "Duplicate line" do not pile up.
fn comfort_panics() -> Vec<String> {
    let key = |key: &str, ctrl: bool, alt: bool, shift: bool| {
        json!({ "kind": "key", "key": key, "ctrl": ctrl, "alt": alt, "shift": shift,
            "meta": false, "page": 3 })
    };
    let mut inputs: Vec<serde_json::Value> = ["(", "[", "\"", "`", "*", "~", "_", "#", ")", "]"]
        .iter()
        .map(|text| json!({ "kind": "text", "text": text }))
        .collect();
    inputs.extend([
        key("Tab", false, false, false),
        key("Tab", false, false, true),
        key("ArrowUp", false, true, false),
        key("ArrowDown", false, true, false),
        key("D", true, false, true),
        key("Backspace", false, false, false),
    ]);
    for action in [
        "moveLineUp",
        "moveLineDown",
        "duplicateLine",
        "indent",
        "outdent",
    ] {
        inputs.push(json!({ "kind": "edit", "action": action }));
    }
    let targets = [
        json!({ "line": 0, "column": null }),
        json!({ "line": 1, "column": 0 }),
        json!({ "line": 3, "column": u32::MAX }),
        json!({ "line": u32::MAX, "column": 7 }),
    ];
    let mut found = Vec::new();
    for pairs in [true, false] {
        for a in OFFSETS {
            for h in [0, 6, u32::MAX] {
                found.extend(panics(&format!("comfort at {a}, {h}"), |core| {
                    core.set_close_pairs(pairs);
                    let rev = core_rev(core);
                    let _ = core.select(rev, a, h);
                    for input in &inputs {
                        let _ = core.input(&input.to_string());
                        let rev = core_rev(core);
                        let at = json!({ "rev": rev, "anchor": a, "head": h }).to_string();
                        let mut facade = input.clone();
                        facade.as_object_mut().map(|o| o.remove("page"));
                        let _ = core.facade_input(&at, &facade.to_string(), "null", None);
                    }
                    for target in &targets {
                        let _ = core.line_note(&target.to_string());
                        let _ = core.go_to_line(&target.to_string());
                        let at = json!({ "rev": 7, "anchor": a, "head": h }).to_string();
                        let _ = core.facade_go_to_line(&at, &target.to_string());
                    }
                    let _ = core.line_count();
                    let _ = core.text_statistics();
                    let _ = core.views(3.0);
                }));
            }
        }
    }
    found
}

/// BR-020 (review): an AsciiDoc `+` right before an item gives the facade a
/// block of no text; moving and copying around it never panics.
#[test]
fn br020_moving_around_an_asciidoc_continuation_never_panics() {
    let mut found = Vec::new();
    for text in [
        "* a\n+\n* b\n",
        "* a\n+\ncont\n* b\n",
        "+\n* a\n+\n",
        "* a\n+\n+\n* b",
    ] {
        for at in 0..12_u32 {
            for action in [
                "moveLineUp",
                "moveLineDown",
                "duplicateLine",
                "indent",
                "outdent",
            ] {
                let run = catch_unwind(AssertUnwindSafe(|| {
                    let mut core = Core::open("Home", text, "AsciiDoc", false).expect("a format");
                    let rev = core_rev(&mut core);
                    let place = json!({ "rev": rev, "anchor": at, "head": at }).to_string();
                    let input = json!({ "kind": "edit", "action": action }).to_string();
                    let _ = core.facade_input(&place, &input, "null", None);
                    let _ = core.input(&input);
                }));
                if run.is_err() {
                    found.push(format!("{action} at {at} in {text:?}"));
                }
            }
        }
    }
    assert!(found.is_empty(), "panics:\n{}", found.join("\n"));
}

/// The revision of the tab's text.
fn core_rev(core: &mut Core) -> u32 {
    serde_json::from_str::<serde_json::Value>(&core.state().unwrap_or_default())
        .ok()
        .and_then(|state| state["rev"].as_u64())
        .and_then(|rev| u32::try_from(rev).ok())
        .unwrap_or(0)
}

/// UC-004 A17: what is no line number is said so, and a line number is
/// read with its column.
#[test]
fn tc_004_33_the_line_target_is_read() {
    let answer: serde_json::Value =
        serde_json::from_str(&crate::calls::line_target("120:8").expect("JSON")).expect("JSON");
    assert_eq!(
        answer,
        json!({ "target": { "line": 120, "column": 8 }, "problem": null })
    );
    let wrong: serde_json::Value =
        serde_json::from_str(&crate::calls::line_target("abc").expect("JSON")).expect("JSON");
    assert_eq!(
        wrong,
        json!({ "target": null, "problem": "A line number is expected, such as 120 or 120:8." })
    );
}

/// ADR-025: JSON that is not the DTO a call takes is an error, not a panic.
#[test]
fn adr025_wrong_json_is_an_error() {
    let mut core = Core::open("Home", "a\n", "Markdown", false).expect("a format");
    let at = json!({ "rev": 0, "anchor": 0, "head": 0 }).to_string();
    let input = json!({ "kind": "text", "text": "x" }).to_string();
    for wrong in [
        "",
        "{",
        "null",
        "42",
        "{\"kind\":\"nothing\"}",
        "[1,2]",
        "\"copy\"",
    ] {
        assert!(core.input(wrong).is_err(), "input {wrong}");
        assert!(core.apply(0, wrong, 0, 0, false).is_err(), "apply {wrong}");
        assert!(
            core.facade_input(wrong, &input, "null", None).is_err(),
            "at {wrong}"
        );
        assert!(
            core.facade_input(&at, wrong, "null", None).is_err(),
            "facade input {wrong}"
        );
        if wrong != "null" {
            assert!(core.keys_status(wrong).is_err(), "vim {wrong}");
            assert!(
                core.facade_input(&at, &input, wrong, None).is_err(),
                "rows {wrong}"
            );
        }
        let switches = json!({ "matchCase": false, "wholeWord": false, "regex": false });
        let spec =
            json!({ "text": "a", "switches": switches, "oneText": false, "shownOnly": false });
        let spec = spec.to_string();
        assert!(core.find(&spec, wrong).is_err(), "find step {wrong}");
        assert!(core.find(wrong, "\"next\"").is_err(), "find spec {wrong}");
        assert!(core.replace(wrong, "x", true).is_err(), "replace {wrong}");
        assert!(
            core.facade_replace(wrong, "x", false).is_err(),
            "facade replace {wrong}"
        );
        if wrong != "null" {
            assert!(core.set_search(wrong).is_err(), "search {wrong}");
        }
        if wrong != "null" && !wrong.starts_with('"') {
            assert!(core.find_status(wrong).is_err(), "replacement {wrong}");
        }
        if wrong != "\"copy\"" {
            assert!(core.clipboard(wrong, "null").is_err(), "action {wrong}");
            assert!(
                core.facade_clipboard(&at, wrong, false, "null").is_err(),
                "facade action {wrong}"
            );
        }
        if wrong != "null" {
            assert!(
                core.clipboard("\"paste\"", wrong).is_err(),
                "content {wrong}"
            );
            assert!(core.insert(wrong).is_err(), "insertion {wrong}");
            assert!(
                core.facade_insert(&at, wrong).is_err(),
                "facade insertion {wrong}"
            );
            assert!(core.complete_link(wrong).is_err(), "link {wrong}");
            assert!(
                core.facade_complete_link(&at, wrong).is_err(),
                "facade link {wrong}"
            );
        }
        assert!(
            core.facade_link_context(wrong, true).is_err(),
            "link at {wrong}"
        );
        // serde reads a struct from an array of its fields too: `[1,2]` is
        // line 1, column 2.
        if wrong != "[1,2]" {
            assert!(core.go_to_line(wrong).is_err(), "line {wrong}");
            assert!(
                core.facade_go_to_line(&at, wrong).is_err(),
                "facade line {wrong}"
            );
        }
        assert!(
            core.facade_go_to_line(wrong, "{\"line\":1,\"column\":null}")
                .is_err(),
            "facade line at {wrong}"
        );
    }
    assert!(Core::open("Home", "", "Rtf", false).is_err());
    assert!(core.set_mode("Emacs").is_err());
}

/// A small xorshift generator, so the random calls are the same each run.
struct Random(u64);

impl Random {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, n: u64) -> u32 {
        u32::try_from(self.next() % n).unwrap_or(0)
    }
}

/// ADR-025: random runs of calls with random offsets, revisions and inputs
/// in both views never panic (seeds in the message when one does).
#[test]
fn adr025_random_calls_never_panic() {
    let inputs = [
        json!({ "kind": "text", "text": "😀" }),
        json!({ "kind": "text", "text": "\r\n" }),
        json!({ "kind": "text", "text": "*" }),
        json!({ "kind": "key", "key": "Enter", "ctrl": false, "alt": false, "shift": false, "meta": false, "page": 3 }),
        json!({ "kind": "key", "key": "Backspace", "ctrl": false, "alt": false, "shift": false, "meta": false, "page": 3 }),
        json!({ "kind": "key", "key": "Delete", "ctrl": true, "alt": false, "shift": false, "meta": false, "page": 3 }),
        json!({ "kind": "key", "key": "Z", "ctrl": true, "alt": false, "shift": false, "meta": false, "page": 3 }),
        json!({ "kind": "key", "key": "ArrowDown", "ctrl": false, "alt": false, "shift": true, "meta": false, "page": 3 }),
        json!({ "kind": "text", "text": "(" }),
        json!({ "kind": "key", "key": "ArrowUp", "ctrl": false, "alt": true, "shift": false, "meta": false, "page": 3 }),
        json!({ "kind": "key", "key": "Tab", "ctrl": false, "alt": false, "shift": false, "meta": false, "page": 3 }),
        json!({ "kind": "edit", "action": "duplicateLine" }),
        json!({ "kind": "edit", "action": "outdent" }),
    ];
    let mut failed = Vec::new();
    for seed in 1..=60_u64 {
        let (text, format) = TEXTS[usize::try_from(seed % 3).unwrap_or(0)];
        let mode = ["Normal", "Vim", "Nano"][usize::try_from(seed % 3).unwrap_or(0)];
        let run = catch_unwind(AssertUnwindSafe(|| {
            let mut random = Random(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
            let mut core = Core::open("Home", text, format, false).expect("a format");
            core.set_mode(mode).expect("a mode");
            for _ in 0..150 {
                let rev = random.below(40);
                let (a, h) = (random.below(80), random.below(80));
                let input = &inputs[random.below(inputs.len() as u64) as usize];
                match random.below(6) {
                    0 => {
                        let _ = core.select(rev, a, h);
                    }
                    1 => {
                        let _ = core.input(&input.to_string());
                    }
                    2 => {
                        let changes =
                            json!([{ "from": a, "to": h, "insert": "😀\r", "newStep": false }]);
                        let _ = core.apply(rev, &changes.to_string(), h, a, false);
                    }
                    3 => {
                        let at = json!({ "rev": rev, "anchor": a, "head": h }).to_string();
                        let mut facade = input.clone();
                        facade.as_object_mut().map(|o| o.remove("page"));
                        let _ = core.facade_input(&at, &facade.to_string(), "null", None);
                    }
                    4 => {
                        let at = json!({ "rev": rev, "anchor": a, "head": h }).to_string();
                        let _ = core.facade_clipboard(&at, "\"cut\"", false, "null");
                    }
                    _ => {
                        let _ = core.fold(random.below(20));
                        let _ = core.views(f64::from(random.below(5000)));
                    }
                }
            }
        }));
        if run.is_err() {
            failed.push(seed);
        }
    }
    assert!(failed.is_empty(), "seeds that panicked: {failed:?}");
}

/// FR-097, UC-005 BR-016 (package 7a): random runs over several views of
/// one note (ids 0 and `u32::MAX` too, views that were never opened or are
/// closed), with and without the sync, `sourceSince` from any revision,
/// never panic (seeds in the message), and every
/// answer of `sourceSince` is JSON.
#[test]
fn fr097_random_calls_over_several_views_never_panic() {
    let inputs = [
        json!({ "kind": "text", "text": "😀" }),
        json!({ "kind": "text", "text": "\r\n" }),
        json!({ "kind": "text", "text": "x" }),
        json!({ "kind": "key", "key": "Enter", "ctrl": false, "alt": false, "shift": false, "meta": false, "page": 3 }),
        json!({ "kind": "key", "key": "Backspace", "ctrl": false, "alt": false, "shift": false, "meta": false, "page": 3 }),
        json!({ "kind": "key", "key": "Z", "ctrl": true, "alt": false, "shift": false, "meta": false, "page": 3 }),
        json!({ "kind": "key", "key": "Y", "ctrl": true, "alt": false, "shift": false, "meta": false, "page": 3 }),
        json!({ "kind": "edit", "action": "duplicateLine" }),
    ];
    let ids = [0_u32, 1, 2, 3, u32::MAX];
    let mut failed = Vec::new();
    for seed in 1..=120_u64 {
        let (text, format) = TEXTS[usize::try_from(seed % 3).unwrap_or(0)];
        let mode = ["Normal", "Vim", "Nano"][usize::try_from(seed % 3).unwrap_or(0)];
        let run = catch_unwind(AssertUnwindSafe(|| {
            let mut random = Random(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
            let mut core = Core::open("Home", text, format, false).expect("a format");
            core.set_mode(mode).expect("a mode");
            if seed % 2 == 0 {
                core.start_sync(0);
            }
            for _ in 0..200 {
                let id = ids[random.below(ids.len() as u64) as usize];
                let (rev, a, h) = (random.below(300), random.below(80), random.below(80));
                let input = &inputs[random.below(inputs.len() as u64) as usize];
                match random.below(12) {
                    0 => core.view_open(id),
                    1 => core.view_close(id),
                    2 | 3 => core.use_view(id, random.below(2) == 0),
                    4 => {
                        let answer = core.source_since(rev).expect("an answer");
                        serde_json::from_str::<serde_json::Value>(&answer).expect("JSON");
                    }
                    5 => {
                        let answer = core
                            .source_since(u32::MAX - random.below(3))
                            .expect("an answer");
                        serde_json::from_str::<serde_json::Value>(&answer).expect("JSON");
                    }
                    6 => {
                        let _ = core.input(&input.to_string());
                    }
                    7 => {
                        let _ = core.select(rev, a, h);
                    }
                    8 => core.set_view(random.below(2) == 0),
                    9 => {
                        let at = json!({ "rev": rev, "anchor": a, "head": h }).to_string();
                        let mut facade = input.clone();
                        facade.as_object_mut().map(|o| o.remove("page"));
                        let _ = core.facade_input(&at, &facade.to_string(), "null", None);
                        let _ = core.facade_head();
                    }
                    10 => {
                        let _ = core.text();
                    }
                    _ => {
                        let _ = core.fold(random.below(10));
                        let _ = core.state();
                    }
                }
            }
        }));
        if run.is_err() {
            failed.push(seed);
        }
    }
    assert!(failed.is_empty(), "seeds that panicked: {failed:?}");
}

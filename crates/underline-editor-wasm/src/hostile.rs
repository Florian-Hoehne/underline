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
                    let _ = core.clipboard("\"cut\"", None);
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
                    let _ = core.facade_clipboard(&at, "\"cut\"", false, None);
                    let _ = core.facade_clipboard(&at, "\"paste\"", false, Some("a\r\n😀"));
                }
            }
        }
        let _ = core.facade_state();
        let _ = core.views(f64::NAN);
    }));
    assert!(found.is_empty(), "panics:\n{}", found.join("\n"));
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
        if wrong != "\"copy\"" {
            assert!(core.clipboard(wrong, None).is_err(), "action {wrong}");
            assert!(
                core.facade_clipboard(&at, wrong, false, None).is_err(),
                "facade action {wrong}"
            );
        }
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
                        let _ = core.facade_clipboard(&at, "\"cut\"", false, None);
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

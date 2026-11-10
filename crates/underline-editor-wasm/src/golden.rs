//! ADR-025, step B3c: the golden calls. The same calls run natively here and
//! in the webview's WASM (`ui/src/lib/editor/core.test.ts`); both must give
//! the answers in `golden/calls.json`. Run with `UNDERLINE_UPDATE_GOLDEN=1`
//! to write the file anew after a change of the editor.

use serde_json::{json, Value};

use crate::calls::{CallError, Core};

/// Where the golden calls live.
const GOLDEN: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/golden/calls.json");

/// The arguments of `call` that cross as JSON text; the others are numbers,
/// flags or plain strings (`core.ts` stringifies the same ones).
fn json_args(call: &str) -> &'static [usize] {
    match call {
        "input" | "keysStatus" | "insert" => &[0],
        "apply" | "clipboard" => &[0, 1],
        "facadeInput" => &[0, 1, 2],
        "facadeInsert" => &[0, 1],
        "facadeClipboard" => &[0, 1, 3],
        _ => &[],
    }
}

/// Argument `index` of `args` as the JSON text `core.ts` sends.
fn text_arg(args: &[Value], index: usize) -> String {
    serde_json::to_string(&args[index]).expect("a JSON value")
}

/// Argument `index` of `args` as a plain string, or `None` for `null`.
fn str_arg(args: &[Value], index: usize) -> Option<String> {
    args[index].as_str().map(str::to_owned)
}

fn num(args: &[Value], index: usize) -> u32 {
    u32::try_from(args[index].as_u64().expect("a number")).expect("a u32")
}

/// Calls `call` with `args` on `core`, as `WasmTab` does; the answer as
/// JSON (`null` for none), or `{"error": …}` with the message the window
/// gets as its `Error`.
fn dispatch(core: &mut Core, call: &str, args: &[Value]) -> Value {
    let arg = |i: usize| {
        if json_args(call).contains(&i) {
            text_arg(args, i)
        } else {
            str_arg(args, i).unwrap_or_default()
        }
    };
    let flag = |i: usize| args[i].as_bool().expect("a flag");
    let answer: Result<Option<String>, CallError> = match call {
        "setMode" => core.set_mode(&arg(0)).map(|()| None),
        "setView" => {
            core.set_view(flag(0));
            Ok(None)
        }
        "setLooks" => {
            core.set_looks(&arg(0), flag(1));
            Ok(None)
        }
        "setNow" => {
            core.set_now(args[0].as_f64().expect("a time"));
            Ok(None)
        }
        "state" => core.state().map(Some),
        "input" => core.input(&arg(0)).map(Some),
        "apply" => core
            .apply(num(args, 0), &arg(1), num(args, 2), num(args, 3), flag(4))
            .map(Some),
        "select" => core
            .select(num(args, 0), num(args, 1), num(args, 2))
            .map(Some),
        "fold" => core.fold(num(args, 0)).map(Some),
        "keysStatus" => core.keys_status(&arg(0)).map(Some),
        "clipboard" => core.clipboard(&arg(0), &arg(1)).map(Some),
        "copyOnSelect" => core.copy_on_select(flag(0)).map(Some),
        "overCopied" => Ok(Some(
            serde_json::to_string(&core.over_copied()).expect("a flag"),
        )),
        "insert" => core.insert(&arg(0)).map(Some),
        "facadeInsert" => core.facade_insert(&arg(0), &arg(1)).map(Some),
        "facadeState" => core.facade_state().map(Some),
        "facadeInput" => core
            .facade_input(&arg(0), &arg(1), &arg(2), str_arg(args, 3).as_deref())
            .map(Some),
        "facadeClipboard" => core
            .facade_clipboard(&arg(0), &arg(1), flag(2), &arg(3))
            .map(Some),
        "facadeKeys" => core.facade_keys().map(Some),
        "views" => core.views(args[0].as_f64().expect("a time")).map(Some),
        "text" => Ok(Some(serde_json::to_string(&core.text()).expect("a string"))),
        "hash" => Ok(Some(serde_json::to_string(&core.hash()).expect("a string"))),
        _ => panic!("no call {call}"),
    };
    match answer {
        Ok(Some(answer)) => serde_json::from_str(&answer).expect("JSON from the core"),
        Ok(None) => Value::Null,
        Err(error) => json!({ "error": error.to_string() }),
    }
}

/// A run of calls on one tab, recording each with its answer.
struct Run {
    core: Core,
    steps: Vec<Value>,
}

impl Run {
    fn call(&mut self, call: &str, args: Value) -> Value {
        let args = args.as_array().cloned().expect("an array of arguments");
        let out = dispatch(&mut self.core, call, &args);
        self.steps
            .push(json!({ "call": call, "args": args, "out": out.clone() }));
        out
    }
}

/// The scenario `name`: the tab of `text` in `format`, driven by `drive`.
fn scenario(name: &str, text: &str, format: &str, drive: impl FnOnce(&mut Run)) -> Value {
    let mut run = Run {
        core: Core::open("Home", text, format, false).expect("a format"),
        steps: Vec::new(),
    };
    drive(&mut run);
    json!({
        "name": name,
        "open": { "note": "Home", "text": text, "format": format, "mac": false },
        "steps": run.steps,
    })
}

fn key(key: &str, ctrl: bool, shift: bool) -> Value {
    json!({ "kind": "key", "key": key, "ctrl": ctrl, "alt": false, "shift": shift, "meta": false, "page": 1 })
}

fn facade_key(key: &str, ctrl: bool, shift: bool) -> Value {
    json!({ "kind": "key", "key": key, "ctrl": ctrl, "alt": false, "shift": shift, "meta": false })
}

fn rev(out: &Value) -> Value {
    out["rev"].clone()
}

/// The source view: typing (emoji, CRLF), Enter, Backspace, the toolbar,
/// undo, folding, the sections, the clipboard, the Vim changes and the
/// status of the keys.
fn source_md() -> Value {
    let text = "# Title\n\nSome text 😀\r\nmore.\n\n## Sub\nbody\n\n```rust\nfn main() {}\n```\n";
    scenario("source, Markdown", text, "Markdown", |run| {
        run.call("setMode", json!(["Emacs"]));
        run.call("setMode", json!(["Normal"]));
        run.call("setNow", json!([1000.0]));
        run.call("input", json!([{ "kind": "nothing" }]));
        run.call("keysStatus", json!([{ "mode": "sleepy" }]));
        run.call("clipboard", json!(["shred", null]));
        let state = run.call("state", json!([]));
        run.call("select", json!([rev(&state), 19, 19]));
        run.call("input", json!([{ "kind": "text", "text": "x😀" }]));
        run.call("input", json!([key("Enter", false, false)]));
        run.call("input", json!([key("Backspace", false, false)]));
        let out = run.call("input", json!([key("B", true, false)]));
        run.call("select", json!([rev(&out), 9, 13]));
        run.call("input", json!([{ "kind": "tool", "tool": "italic" }]));
        run.call("input", json!([key("Z", true, false)]));
        run.call("views", json!([1000.0]));
        run.call("fold", json!([5]));
        run.call("views", json!([1400.0]));
        run.call("input", json!([key("ArrowDown", false, false)]));
        run.call("input", json!([key("Home", false, true)]));
        run.call("copyOnSelect", json!([true]));
        run.call("overCopied", json!([]));
        run.call("copyOnSelect", json!([true]));
        run.call("clipboard", json!(["copy", null]));
        run.call("clipboard", json!(["cut", null]));
        run.call("copyOnSelect", json!([true]));
        run.call(
            "clipboard",
            json!(["paste", { "text": null, "markup": null, "image": true }]),
        );
        let pasted = run.call(
            "clipboard",
            json!(["paste", { "text": "pasted\nline", "markup": "**pasted**", "image": false }]),
        );
        run.call(
            "insert",
            json!([{ "kind": "dropped", "place": "a b.png", "image": true }]),
        );
        run.call("insert", json!([{ "kind": "text", "text": " " }]));
        run.call(
            "insert",
            json!([{ "kind": "link", "address": "Plan.md", "label": "Plan" }]),
        );
        run.call(
            "apply",
            json!([rev(&pasted["patch"]), [{ "from": 0, "to": 0, "insert": "V", "newStep": true }], 1, 1, false]),
        );
        run.call(
            "apply",
            json!([0, [{ "from": 0, "to": 0, "insert": "W", "newStep": true }], 1, 1, false]),
        );
        run.call("setMode", json!(["Vim"]));
        run.call(
            "keysStatus",
            json!([{ "mode": "insert", "pending": "", "marked": false }]),
        );
        run.call("setMode", json!(["Nano"]));
        run.call("keysStatus", json!([null]));
        run.call("input", json!([key("W", true, false)]));
        run.call("input", json!([{ "kind": "text", "text": "body" }]));
        run.call("views", json!([2000.0]));
        run.call("input", json!([key("Enter", false, false)]));
        run.call("views", json!([2100.0]));
        run.call("text", json!([]));
        run.call("hash", json!([]));
    })
}

/// The facade: typing with escapes, Enter and Backspace per kind of block,
/// arrows with rows, copy and paste of the own markup, Vim, the keys.
fn facade(name: &str, text: &str, format: &str, at: u32) -> Value {
    scenario(name, text, format, |run| {
        run.call("setMode", json!(["Normal"]));
        run.call("setView", json!([true]));
        run.call("setLooks", json!(["Light", false]));
        let state = run.call("facadeState", json!([]));
        run.call(
            "facadeInput",
            json!([{ "rev": "zero" }, { "kind": "text", "text": "x" }, null, null]),
        );
        run.call(
            "facadeInput",
            json!([{ "rev": 0, "anchor": 0, "head": 0 }, { "kind": "text", "text": "x" }, { "at": -1 }, null]),
        );
        run.call(
            "facadeClipboard",
            json!([{ "rev": 0, "anchor": 0, "head": 0 }, "shred", false, null]),
        );
        run.call(
            "facadeInsert",
            json!([{ "rev": 0, "anchor": 0, "head": 0 }, { "kind": "x" }]),
        );
        let at_of = |out: &Value, anchor: u32, head: u32| json!({ "rev": rev(out), "anchor": anchor, "head": head });
        let out = run.call(
            "facadeInput",
            json!([at_of(&state, at, at), { "kind": "text", "text": "*x*" }, null, null]),
        );
        let here = out["head"].as_u64().unwrap();
        let here = u32::try_from(here).unwrap();
        let out = run.call(
            "facadeInput",
            json!([
                at_of(&out, here, here),
                facade_key("Enter", false, false),
                null,
                null
            ]),
        );
        let here = u32::try_from(out["head"].as_u64().unwrap()).unwrap();
        let out = run.call(
            "facadeInput",
            json!([
                at_of(&out, here, here),
                facade_key("Backspace", false, false),
                null,
                null
            ]),
        );
        let here = u32::try_from(out["head"].as_u64().unwrap()).unwrap();
        let rows = json!({ "at": here, "up": null, "down": null, "start": 0, "end": here });
        let out = run.call(
            "facadeInput",
            json!([
                at_of(&out, here, here),
                facade_key("ArrowLeft", false, true),
                rows,
                null
            ]),
        );
        let (anchor, head) = (
            u32::try_from(out["anchor"].as_u64().unwrap()).unwrap(),
            u32::try_from(out["head"].as_u64().unwrap()).unwrap(),
        );
        let copied = run.call(
            "facadeClipboard",
            json!([at_of(&out, anchor, head), "copy", false, null]),
        );
        let markup = copied["copied"].clone();
        let out = run.call(
            "facadeClipboard",
            json!([at_of(&copied["patch"], head, head), "paste", false, { "text": markup, "markup": null, "image": false }]),
        )["patch"]
            .clone();
        let here = u32::try_from(out["head"].as_u64().unwrap()).unwrap();
        let out = run.call(
            "facadeClipboard",
            json!([at_of(&out, here, here), "paste", true, { "text": "*plain*", "markup": "**html**", "image": false }]),
        )["patch"]
            .clone();
        let here = u32::try_from(out["head"].as_u64().unwrap()).unwrap();
        let out = run.call(
            "facadeClipboard",
            json!([at_of(&out, here, here), "paste", false, { "text": "html", "markup": "## From\n\n*html*", "image": false }]),
        )["patch"]
            .clone();
        let here = u32::try_from(out["head"].as_u64().unwrap()).unwrap();
        let out = run.call(
            "facadeInsert",
            json!([at_of(&out, here, here), { "kind": "image", "place": "Plan-2026-11-10-0930.png" }]),
        );
        run.call("copyOnSelect", json!([true]));
        run.call(
            "facadeInput",
            json!([at_of(&out, 0, 0), { "kind": "tool", "tool": "bold" }, null, null]),
        );
        run.call("setMode", json!(["Vim"]));
        run.call("facadeKeys", json!([]));
        let state = run.call("facadeState", json!([]));
        run.call(
            "facadeInput",
            json!([at_of(&state, 0, 0), { "kind": "text", "text": "j" }, null, null]),
        );
        run.call("views", json!([5000.0]));
        run.call("text", json!([]));
        run.call("hash", json!([]));
    })
}

/// All scenarios, as `golden/calls.json` holds them.
fn golden() -> Value {
    let markdown = "# Facade\n\nA paragraph with **bold** and 😀 here.\r\n\n- item one\n- item two\n\n> a quote\n\n```rust\nlet x = 1;\n```\n\n| a | b |\n|---|---|\n| 1 | 2 |\n";
    let asciidoc = "= Facade\n\nA paragraph with *bold* here.\n\n* item one\n* item two\n\nNOTE: An admonition.\n\n[source,python]\n----\nprint(1)\n----\n";
    json!([
        source_md(),
        facade("facade, Markdown", markdown, "Markdown", 14),
        facade("facade, AsciiDoc", asciidoc, "AsciiDoc", 14),
    ])
}

/// Where `kept` and `fresh` first differ: the scenario and the step.
fn first_difference(kept: &Value, fresh: &Value) -> Option<String> {
    let (Some(kept), Some(fresh)) = (kept.as_array(), fresh.as_array()) else {
        return Some("the top, which is no list of scenarios".into());
    };
    if kept.len() != fresh.len() {
        return Some(format!(
            "the number of scenarios ({} for {})",
            kept.len(),
            fresh.len()
        ));
    }
    for (old, new) in kept.iter().zip(fresh) {
        let name = new["name"].as_str().unwrap_or("?");
        if old["name"] != new["name"] || old["open"] != new["open"] {
            return Some(format!("scenario {name:?}: its opening"));
        }
        let (Some(old_steps), Some(new_steps)) = (old["steps"].as_array(), new["steps"].as_array())
        else {
            return Some(format!("scenario {name:?}: its steps"));
        };
        for (index, (a, b)) in old_steps.iter().zip(new_steps).enumerate() {
            if a != b {
                return Some(format!("scenario {name:?}, step {index} ({})", b["call"]));
            }
        }
        if old_steps.len() != new_steps.len() {
            return Some(format!("scenario {name:?}: the number of steps"));
        }
    }
    None
}

/// `value` with the keys of every object in order, so the file is the same
/// whether serde_json keeps the order of insertion (its `preserve_order`
/// feature, which another crate of the workspace may switch on) or not.
fn sorted(value: Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut entries: Vec<(String, Value)> = map.into_iter().collect();
            entries.sort_by(|a, b| a.0.cmp(&b.0));
            Value::Object(entries.into_iter().map(|(k, v)| (k, sorted(v))).collect())
        }
        Value::Array(items) => Value::Array(items.into_iter().map(sorted).collect()),
        other => other,
    }
}

/// ADR-025, M-08, M-09, M-10, M-11 (step B3c): the calls of the webview
/// give the golden answers natively; `core.test.ts` checks the WASM against
/// the same file.
#[test]
fn adr025_the_calls_give_the_golden_answers() {
    let fresh = sorted(golden());
    if std::env::var_os("UNDERLINE_UPDATE_GOLDEN").is_some() {
        let text = serde_json::to_string_pretty(&fresh).expect("JSON") + "\n";
        std::fs::write(GOLDEN, text).expect("the golden file is written");
    }
    let text = std::fs::read_to_string(GOLDEN).unwrap_or_else(|err| {
        panic!("golden/calls.json is missing ({err}); write it with UNDERLINE_UPDATE_GOLDEN=1")
    });
    let kept: Value = serde_json::from_str(&text).unwrap_or_else(|err| {
        panic!(
            "golden/calls.json is no valid JSON ({err}); write it with UNDERLINE_UPDATE_GOLDEN=1"
        )
    });
    // Compared as JSON values: the order of an object's keys does not count.
    if let Some(difference) = first_difference(&kept, &fresh) {
        panic!("golden/calls.json is outdated at {difference}; run with UNDERLINE_UPDATE_GOLDEN=1");
    }
}

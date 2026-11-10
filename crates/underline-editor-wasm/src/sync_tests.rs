//! ADR-025, step B4c: the window's sync calls against the backend's mirror,
//! as the webview and the mock backend make them: edits go out after the
//! input, the backend's changes come back as one patch for the source view.

use serde_json::{json, Value};

use crate::calls::Core;
use crate::mirror::Mirror;

fn key(name: &str) -> String {
    json!({ "kind": "key", "key": name, "ctrl": false, "alt": false, "shift": false, "meta": false, "page": 1 }).to_string()
}

fn text(text: &str) -> String {
    json!({ "kind": "text", "text": text }).to_string()
}

/// Sends what the core has to `mirror`; the `RemoteApplied`.
fn sync(core: &mut Core, mirror: &mut Mirror) -> Value {
    let batch = core.take_outbox().unwrap();
    assert_ne!(batch, "null", "something to send");
    let received: Value = serde_json::from_str(&mirror.receive(&batch).unwrap()).unwrap();
    let answer = received["answer"].to_string();
    serde_json::from_str(&core.acknowledge(&answer).unwrap()).unwrap()
}

/// ADR-025: typing goes to the backend; an agent's change comes back as a
/// patch for the view, with the cursor kept; both texts are alike.
#[test]
fn adr025_typing_goes_out_and_the_backends_change_comes_back() {
    let start = "one\r\ntwo 😀\n";
    let mut mirror = Mirror::new(start);
    let mut core = Core::open("A", start, "Markdown", false).unwrap();
    core.start_sync(mirror.rev());
    core.state().unwrap();
    core.select(0, 3, 3).unwrap();
    core.input(&text("!")).unwrap();
    // A batch lost on its way is sent again.
    let lost = core.take_outbox().unwrap();
    assert_eq!(core.take_outbox().unwrap(), "null", "one batch on its way");
    core.failed().unwrap();
    assert_eq!(core.take_outbox().unwrap(), lost);
    core.failed().unwrap();

    let applied = sync(&mut core, &mut mirror);
    assert_eq!(
        applied,
        json!({ "patch": null, "desync": false, "reason": null })
    );
    assert_eq!(mirror.text(), "one!\r\ntwo 😀\n");
    // The failures asked to fetch too.
    sync(&mut core, &mut mirror);
    assert!(core.idle().unwrap());

    mirror.backend_edit(0, 0, "# ", true).unwrap();
    core.ring().unwrap();
    let applied = sync(&mut core, &mut mirror);
    let patch = &applied["patch"];
    assert_eq!(
        patch["changes"],
        json!([{ "from": 0, "to": 0, "insert": "# " }])
    );
    assert_eq!(
        (patch["anchor"].clone(), patch["head"].clone()),
        (json!(6), json!(6)),
        "the cursor stays after the '!'"
    );
    assert_eq!(patch["scroll"], json!(false));
    assert_eq!(core.text(), mirror.text());
    assert!(core.in_step(mirror.rev()).unwrap());

    // The agent's change is an undo step of its own.
    core.input(&json!({ "kind": "key", "key": "Z", "ctrl": true, "alt": false, "shift": false, "meta": false, "page": 1 }).to_string()).unwrap();
    assert_eq!(core.text(), "one!\r\ntwo 😀\n");
    sync(&mut core, &mut mirror);
    assert_eq!(mirror.text(), core.text());
}

/// ADR-025, UC-004 A7: a reload from outside comes as a patch replacing the
/// whole text.
#[test]
fn uc004_a7_a_reload_comes_as_one_patch() {
    let mut mirror = Mirror::new("old\n");
    let mut core = Core::open("A", "old\n", "Markdown", false).unwrap();
    core.start_sync(mirror.rev());
    core.state().unwrap();
    mirror.reload("new text\n");
    core.ring().unwrap();

    let applied = sync(&mut core, &mut mirror);

    assert_eq!(
        applied["patch"]["changes"],
        json!([{ "from": 0, "to": 4, "insert": "new text\n" }])
    );
    assert_eq!(core.text(), "new text\n");
    // The core confirms the reload.
    assert!(!core.idle().unwrap());
}

/// ADR-025 section 3.10: a desync is said; after `ed_resync` the core is in
/// step again; calls before the sync started are an error.
#[test]
fn adr025_a_desync_is_said_and_mended() {
    let mut mirror = Mirror::new("abc");
    let mut core = Core::open("A", "abc", "Markdown", false).unwrap();
    assert!(core.take_outbox().is_err(), "not syncing yet");
    core.start_sync(mirror.rev());
    core.input(&key("End")).unwrap();
    core.input(&text("d")).unwrap();
    let batch = core.take_outbox().unwrap();
    let received: Value = serde_json::from_str(&mirror.receive(&batch).unwrap()).unwrap();
    let mut wrong = received["answer"].clone();
    wrong["hash"] = json!("0000000000000000");

    let applied: Value =
        serde_json::from_str(&core.acknowledge(&wrong.to_string()).unwrap()).unwrap();

    assert_eq!(applied["desync"], json!(true));
    assert!(applied["reason"]
        .as_str()
        .is_some_and(|r| r.contains("Hash")));
    let sent = core.text();
    let answer: Value = serde_json::from_str(&mirror.resync(&sent).unwrap()).unwrap();
    core.reset(
        &sent,
        u32::try_from(answer["rev"].as_u64().unwrap()).unwrap(),
    )
    .unwrap();
    assert!(core.in_step(mirror.rev()).unwrap());
    assert_eq!(mirror.text(), "abcd");
}

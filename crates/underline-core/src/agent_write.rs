//! UC-019: an agent creates a note or changes part of one. The rules are the
//! same whether the MCP server writes the file itself or the window applies
//! the change to an open tab (ADR-018).

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};
use similar::{DiffTag, TextDiff};

use crate::agents::{AGENTS_FOLDER, PRESENCE_STALE};
use crate::atomic::write_atomic;
use crate::conflict::{conflict_places, Joining};
use crate::format::NoteFormat;
use crate::lookup::{find_note, LookupError};
use crate::new_note::NewNote;
use crate::note::NoteId;
use crate::state::{underline_folder, UNDERLINE_FOLDER};
use crate::workspace::{NoteEntry, Workspace};

/// What an agent asks to do with a note (UC-019 step 1, A1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AgentChange {
    /// A1: a new note with this text.
    Create {
        format: Option<NoteFormat>,
        text: String,
    },
    /// Replace a passage that occurs exactly once (BR-003).
    Replace { old: String, new: String },
    /// Insert before a line counted from 1; at the end without a line, or
    /// for a line after the last.
    Insert {
        before_line: Option<usize>,
        text: String,
    },
    /// Replace the whole text.
    Rewrite { text: String },
}

/// Why an agent's change is not made.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum WriteProblem {
    #[error(transparent)]
    Lookup(#[from] LookupError),
    /// A4.
    #[error("the passage to replace does not occur in the note `{0}`; read the note again and quote it exactly")]
    NotFound(String),
    /// A4.
    #[error("the passage to replace occurs {count} times in the note `{note}`; quote more of it so it occurs once")]
    NotUnique { note: String, count: usize },
    #[error("the passage to replace is empty")]
    EmptyPassage,
    #[error("lines are counted from 1")]
    BadLine,
    /// A5.
    #[error("the user is editing the note `{note}` at line {line} and has not saved it yet; read the note again in a moment")]
    UserEditing { note: String, line: usize },
    /// A7, BR-005.
    #[error("the note `{0}` holds places in conflict from a git sync; the user has to resolve them in Underline first")]
    Conflict(String),
    /// A2, A3.
    #[error("the note cannot be created: {0}")]
    Create(String),
    /// A9.
    #[error("the note `{note}` cannot be written: {reason}")]
    Write { note: String, reason: String },
    /// ADR-018: the window did not answer in time.
    #[error("Underline is busy; try again")]
    Busy,
}

/// A change as lines of the stored text: lines `from` to `to` (counted
/// from 0, `to` not included) are replaced by `lines`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Splice {
    pub from: usize,
    pub to: usize,
    pub lines: String,
}

/// The lines of `text` with their line endings.
fn lines_of(text: &str) -> Vec<&str> {
    text.split_inclusive('\n').collect()
}

/// BR-001: the agent's text with the line breaks the note already has.
fn with_breaks_of(note: &str, text: &str) -> String {
    let plain = text.replace("\r\n", "\n");
    if note.contains("\r\n") {
        plain.replace('\n', "\r\n")
    } else {
        plain
    }
}

fn line_break_of(note: &str) -> &'static str {
    if note.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    }
}

/// UC-019 steps 1–2, A4, A7, BR-001, BR-003: the change `change` to the
/// stored text `base` of the note `note`, as lines.
pub fn plan_splice(note: &str, base: &str, change: &AgentChange) -> Result<Splice, WriteProblem> {
    if !conflict_places(base, Joining::Merge).is_empty() {
        return Err(WriteProblem::Conflict(note.to_owned()));
    }
    let lines = lines_of(base);
    let total = lines.len();
    let starts: Vec<usize> = lines
        .iter()
        .scan(0, |at, line| {
            let start = *at;
            *at += line.len();
            Some(start)
        })
        .collect();
    let line_at = |byte: usize| starts.partition_point(|s| *s <= byte).saturating_sub(1);
    match change {
        AgentChange::Create { .. } => Err(WriteProblem::Create(format!(
            "the note `{note}` exists; change it instead"
        ))),
        AgentChange::Rewrite { text } => Ok(Splice {
            from: 0,
            to: total,
            lines: with_breaks_of(base, text),
        }),
        AgentChange::Replace { old, new } => {
            let old = with_breaks_of(base, old);
            if old.is_empty() {
                return Err(WriteProblem::EmptyPassage);
            }
            let count = base.matches(old.as_str()).count();
            let at = match count {
                0 => return Err(WriteProblem::NotFound(note.to_owned())),
                1 => base.find(old.as_str()).unwrap_or_default(),
                count => {
                    return Err(WriteProblem::NotUnique {
                        note: note.to_owned(),
                        count,
                    })
                }
            };
            let end = at + old.len();
            let (from, last) = (line_at(at), line_at(end - 1));
            let line_end = starts[last] + lines[last].len();
            Ok(Splice {
                from,
                to: last + 1,
                lines: format!(
                    "{}{}{}",
                    &base[starts[from]..at],
                    with_breaks_of(base, new),
                    &base[end..line_end]
                ),
            })
        }
        AgentChange::Insert { before_line, text } => {
            let before = before_line.unwrap_or(total + 1);
            if before == 0 {
                return Err(WriteProblem::BadLine);
            }
            let mut text = with_breaks_of(base, text);
            if before <= total {
                if !text.ends_with('\n') {
                    text.push_str(line_break_of(base));
                }
                return Ok(Splice {
                    from: before - 1,
                    to: before - 1,
                    lines: text,
                });
            }
            // At the end: a last line without a line break gets one first.
            match lines.last() {
                Some(last) if !last.ends_with('\n') => Ok(Splice {
                    from: total - 1,
                    to: total,
                    lines: format!("{last}{}{text}", line_break_of(base)),
                }),
                _ => Ok(Splice {
                    from: total,
                    to: total,
                    lines: text,
                }),
            }
        }
    }
}

/// The lines (counted from 1) that hold `splice` once applied at line
/// `at` (counted from 0).
fn changed_lines(at: usize, splice: &Splice) -> (usize, usize) {
    let count = lines_of(&splice.lines).len();
    (at + 1, at + count.max(1))
}

/// `splice` applied to `base`, and the lines that now hold it.
pub fn apply_splice(base: &str, splice: &Splice) -> (String, (usize, usize)) {
    let lines = lines_of(base);
    let text = format!(
        "{}{}{}",
        lines[..splice.from].concat(),
        splice.lines,
        lines[splice.to..].concat()
    );
    (text, changed_lines(splice.from, splice))
}

/// `splice` of the stored text, put into the text of an open tab.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Merged {
    /// The bytes of the tab's text that are replaced, and by what.
    pub range: std::ops::Range<usize>,
    pub replacement: String,
    /// The tab's text afterwards.
    pub text: String,
    /// The lines of the tab's text that now hold the change.
    pub lines: (usize, usize),
}

/// UC-019 A5, A6, BR-004: puts `splice`, planned on the stored text `base`,
/// into the tab's text `current`, which may hold changes not yet saved. A
/// change that replaces or removes a line the user changed, or inserts right
/// before or after it, is refused.
pub fn merge_splice(
    note: &str,
    base: &str,
    current: &str,
    splice: &Splice,
) -> Result<Merged, WriteProblem> {
    let diff = TextDiff::from_lines(base, current);
    let ops = diff.ops();
    let (a, b) = (splice.from, splice.to);
    for op in ops {
        if op.tag() == DiffTag::Equal {
            continue;
        }
        let (c, d) = (op.old_range().start, op.old_range().end);
        let touches = if a == b {
            c <= a && a <= d
        } else if c == d {
            a <= c && c <= b
        } else {
            a < d && c < b
        };
        if touches {
            return Err(WriteProblem::UserEditing {
                note: note.to_owned(),
                line: op.new_range().start + 1,
            });
        }
    }
    let current_lines = lines_of(current);
    let mapped = ops
        .iter()
        .filter(|op| op.tag() == DiffTag::Equal)
        .find(|op| op.old_range().contains(&a))
        .map(|op| op.new_range().start + (a - op.old_range().start))
        .unwrap_or_else(|| {
            // Only at the end: after everything of the tab.
            if a >= lines_of(base).len() {
                current_lines.len()
            } else {
                a
            }
        });
    let start: usize = current_lines[..mapped].iter().map(|l| l.len()).sum();
    let end = start
        + current_lines[mapped..mapped + (b - a)]
            .iter()
            .map(|l| l.len())
            .sum::<usize>();
    let text = format!("{}{}{}", &current[..start], splice.lines, &current[end..]);
    Ok(Merged {
        range: start..end,
        replacement: splice.lines.clone(),
        text,
        lines: changed_lines(mapped, splice),
    })
}

/// What an agent is told after a change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentAnswer {
    pub ok: bool,
    pub message: String,
    /// The note changed or created, as listed.
    pub note: Option<String>,
    /// The lines that now hold the change, counted from 1.
    pub lines: Option<(usize, usize)>,
}

impl AgentAnswer {
    pub fn refused(problem: &WriteProblem) -> Self {
        AgentAnswer {
            ok: false,
            message: problem.to_string(),
            note: None,
            lines: None,
        }
    }

    fn changed(id: &NoteId, lines: (usize, usize)) -> Self {
        let place = if lines.0 >= lines.1 {
            format!("line {}", lines.0)
        } else {
            format!("lines {} to {}", lines.0, lines.1)
        };
        AgentAnswer {
            ok: true,
            message: format!("changed {}; the change is in {place}", id.as_str()),
            note: Some(id.as_str().to_owned()),
            lines: Some(lines),
        }
    }
}

/// UC-019 A1–A3: creates the note `name` (folder and title) with `text`.
pub fn create_for_agent(
    workspace: &mut Workspace,
    name: &str,
    format: Option<NoteFormat>,
    text: &str,
) -> Result<NoteId, WriteProblem> {
    let name = name.trim().trim_matches('/');
    let (folder, title) = name.rsplit_once('/').unwrap_or(("", name));
    let new = NewNote {
        folder: folder.to_owned(),
        title: title.to_owned(),
        format: format.unwrap_or_else(|| workspace.default_format()),
    };
    workspace
        .create_note(&new, text)
        .map_err(|problem| WriteProblem::Create(problem.to_string()))
}

/// UC-019 steps 2–3, A1–A4, A7, A9, A10: applies `change` to the note
/// `name` as stored, as the MCP server does while no window runs, and the
/// window does for a note not open. Returns the answer and, for a change
/// made, the note.
pub fn apply_to_file(
    workspace: &mut Workspace,
    name: &str,
    change: &AgentChange,
) -> (AgentAnswer, Option<NoteId>) {
    match apply_to_file_inner(workspace, name, change) {
        Ok((id, _)) if matches!(change, AgentChange::Create { .. }) => {
            let answer = AgentAnswer {
                ok: true,
                message: format!("created {}", id.as_str()),
                note: Some(id.as_str().to_owned()),
                lines: None,
            };
            (answer, Some(id))
        }
        Ok((id, lines)) => (AgentAnswer::changed(&id, lines), Some(id)),
        Err(problem) => (AgentAnswer::refused(&problem), None),
    }
}

fn apply_to_file_inner(
    workspace: &mut Workspace,
    name: &str,
    change: &AgentChange,
) -> Result<(NoteId, (usize, usize)), WriteProblem> {
    if let AgentChange::Create { format, text } = change {
        let id = create_for_agent(workspace, name, *format, text)?;
        let lines = lines_of(text).len();
        return Ok((id, (1, lines.max(1))));
    }
    let entry = find_note(&workspace.notes_now().notes, name)?;
    let path = workspace.file_of(&entry);
    let write = |reason: String| WriteProblem::Write {
        note: entry.id.as_str().to_owned(),
        reason,
    };
    let base = fs::read_to_string(&path).map_err(|e| write(e.to_string()))?;
    let splice = plan_splice(entry.id.as_str(), &base, change)?;
    let (text, lines) = apply_splice(&base, &splice);
    write_atomic(&path, text.as_bytes()).map_err(|e| write(e.to_string()))?;
    Ok((entry.id, lines))
}

/// UC-019 for an open tab: plans `change` on the stored text `base` and puts
/// it into the tab's text `current`.
pub fn plan_for_tab(
    entry: &NoteEntry,
    base: &str,
    current: &str,
    change: &AgentChange,
) -> Result<Merged, WriteProblem> {
    let splice = plan_splice(entry.id.as_str(), base, change)?;
    merge_splice(entry.id.as_str(), base, current, &splice)
}

/// The answer for a change made in a tab.
pub fn answer_for_tab(id: &NoteId, merged: &Merged) -> AgentAnswer {
    AgentAnswer::changed(id, merged.lines)
}

// ADR-018: requests from the MCP server to the running window.

/// A change an MCP server hands to the running window.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentRequest {
    /// The name the agent tool gave, if any.
    pub agent: Option<String>,
    /// The note as the agent named it.
    pub note: String,
    pub change: AgentChange,
}

const WINDOW_FILE: &str = "window";
const REQUESTS: &str = "requests";
const ANSWERS: &str = "answers";

fn agents_dir(root: &Path) -> PathBuf {
    root.join(UNDERLINE_FOLDER).join(AGENTS_FOLDER)
}

/// ADR-018: the running window renews this about once a second.
pub fn renew_window_presence(root: &Path) -> io::Result<()> {
    let folder = underline_folder(root)?.join(AGENTS_FOLDER);
    fs::create_dir_all(&folder)?;
    write_atomic(
        &folder.join(WINDOW_FILE),
        std::process::id().to_string().as_bytes(),
    )
}

/// ADR-018: removes the window's file, when it closes the workspace.
pub fn remove_window_presence(root: &Path) {
    let _ = fs::remove_file(agents_dir(root).join(WINDOW_FILE));
}

/// ADR-018: whether a window runs for the workspace at `now`.
pub fn window_runs(root: &Path, now: SystemTime) -> bool {
    fs::metadata(agents_dir(root).join(WINDOW_FILE))
        .and_then(|m| m.modified())
        .is_ok_and(|renewed| now.duration_since(renewed).unwrap_or_default() <= PRESENCE_STALE)
}

fn yaml_error(err: serde_norway::Error) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, err.to_string())
}

/// ADR-018: hands `request` to the window under the name `id`.
pub fn send_request(root: &Path, id: &str, request: &AgentRequest) -> io::Result<()> {
    let folder = underline_folder(root)?.join(AGENTS_FOLDER).join(REQUESTS);
    fs::create_dir_all(&folder)?;
    let text = serde_norway::to_string(request).map_err(yaml_error)?;
    write_atomic(&folder.join(format!("{id}.yaml")), text.as_bytes())
}

/// ADR-018: the answer to the request `id`, once the window gave it; it is
/// removed when taken.
pub fn take_answer(root: &Path, id: &str) -> Option<AgentAnswer> {
    let path = agents_dir(root).join(ANSWERS).join(format!("{id}.yaml"));
    let text = fs::read_to_string(&path).ok()?;
    let answer = serde_norway::from_str(&text).ok()?;
    let _ = fs::remove_file(path);
    Some(answer)
}

/// ADR-018: withdraws the request `id` that was not answered in time.
/// Returns whether it was still waiting, so the window had not taken it.
pub fn withdraw_request(root: &Path, id: &str) -> bool {
    fs::remove_file(agents_dir(root).join(REQUESTS).join(format!("{id}.yaml"))).is_ok()
}

/// ADR-018: the requests waiting for the window, oldest first. A request it
/// cannot read is answered as refused; requests and answers older than a
/// minute are removed.
pub fn take_requests(root: &Path, now: SystemTime) -> Vec<(String, AgentRequest)> {
    let dir = agents_dir(root);
    if let Ok(entries) = fs::read_dir(dir.join(ANSWERS)) {
        for entry in entries.flatten() {
            if age(&entry.path(), now) > Duration::from_secs(60) {
                let _ = fs::remove_file(entry.path());
            }
        }
    }
    let Ok(entries) = fs::read_dir(dir.join(REQUESTS)) else {
        return Vec::new();
    };
    let mut found = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(id) = path
            .file_name()
            .and_then(|n| n.to_str())
            .and_then(|n| n.strip_suffix(".yaml"))
            .map(str::to_owned)
        else {
            continue;
        };
        if age(&path, now) > Duration::from_secs(60) {
            let _ = fs::remove_file(&path);
            continue;
        }
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        // Taken: the server sees the request is gone and waits for the answer.
        if fs::remove_file(&path).is_err() {
            continue;
        }
        let renewed = entry.metadata().and_then(|m| m.modified()).unwrap_or(now);
        match serde_norway::from_str::<AgentRequest>(&text) {
            Ok(request) => found.push((renewed, id, request)),
            Err(err) => {
                let _ = answer_request(
                    root,
                    &id,
                    &AgentAnswer {
                        ok: false,
                        message: format!("the request could not be read: {err}"),
                        note: None,
                        lines: None,
                    },
                );
            }
        }
    }
    found.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
    found.into_iter().map(|(_, id, r)| (id, r)).collect()
}

/// ADR-018: the window's answer to the request `id`.
pub fn answer_request(root: &Path, id: &str, answer: &AgentAnswer) -> io::Result<()> {
    let folder = underline_folder(root)?.join(AGENTS_FOLDER).join(ANSWERS);
    fs::create_dir_all(&folder)?;
    let text = serde_norway::to_string(answer).map_err(yaml_error)?;
    write_atomic(&folder.join(format!("{id}.yaml")), text.as_bytes())
}

fn age(path: &Path, now: SystemTime) -> Duration {
    fs::metadata(path)
        .and_then(|m| m.modified())
        .map(|t| now.duration_since(t).unwrap_or_default())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn replace(old: &str, new: &str) -> AgentChange {
        AgentChange::Replace {
            old: old.into(),
            new: new.into(),
        }
    }

    fn insert(before_line: Option<usize>, text: &str) -> AgentChange {
        AgentChange::Insert {
            before_line,
            text: text.into(),
        }
    }

    fn applied(base: &str, change: &AgentChange) -> Result<(String, (usize, usize)), WriteProblem> {
        plan_splice("Plan", base, change).map(|s| apply_splice(base, &s))
    }

    const PLAN: &str = "# Plan\n- draft\n- ship\n";

    /// TC-019-01: a passage replaced; the lines that hold it.
    #[test]
    fn uc019_tc01_replace_a_passage() {
        let (text, lines) = applied(PLAN, &replace("- draft", "- final draft")).unwrap();
        assert_eq!(text, "# Plan\n- final draft\n- ship\n");
        assert_eq!(lines, (2, 2));
        let (text, lines) = applied(PLAN, &replace("draft\n- ship", "a\n- b\n- c")).unwrap();
        assert_eq!(text, "# Plan\n- a\n- b\n- c\n");
        assert_eq!(lines, (2, 4));
    }

    /// TC-019-02: inserted before a line, at the end, and the whole text.
    #[test]
    fn uc019_tc02_insert_and_rewrite() {
        let (text, lines) = applied(PLAN, &insert(Some(3), "- review")).unwrap();
        assert_eq!(text, "# Plan\n- draft\n- review\n- ship\n");
        assert_eq!(lines, (3, 3));
        let (text, lines) = applied(PLAN, &insert(Some(99), "- done\n")).unwrap();
        assert_eq!(text, "# Plan\n- draft\n- ship\n- done\n");
        assert_eq!(lines, (4, 4));
        let (text, _) = applied("a", &insert(None, "b")).unwrap();
        assert_eq!(text, "a\nb");
        let (text, _) = applied("", &insert(None, "first")).unwrap();
        assert_eq!(text, "first");
        let (text, _) = applied(
            PLAN,
            &AgentChange::Rewrite {
                text: "# Plan\n".into(),
            },
        )
        .unwrap();
        assert_eq!(text, "# Plan\n");
        assert_eq!(
            applied(PLAN, &insert(Some(0), "x")),
            Err(WriteProblem::BadLine)
        );
    }

    /// TC-019-05: a passage not found, not unique, or of another case.
    #[test]
    fn uc019_tc05_passage_exact_and_unique() {
        let base = "- todo\n- todo\n- ship\n";
        assert!(matches!(
            applied(base, &replace("- zebra", "x")),
            Err(WriteProblem::NotFound(_))
        ));
        assert!(matches!(
            applied(base, &replace("- todo", "x")),
            Err(WriteProblem::NotUnique { count: 2, .. })
        ));
        assert!(matches!(
            applied(base, &replace("- SHIP", "x")),
            Err(WriteProblem::NotFound(_))
        ));
        assert_eq!(
            applied(base, &replace("", "x")),
            Err(WriteProblem::EmptyPassage)
        );
    }

    /// TC-019-08, BR-005: no change to a note in conflict.
    #[test]
    fn uc019_tc08_conflict_refused() {
        let base = "a\n<<<<<<< HEAD\nmine\n=======\ntheirs\n>>>>>>> origin/main\n";
        assert!(matches!(
            applied(base, &insert(None, "x")),
            Err(WriteProblem::Conflict(_))
        ));
    }

    /// TC-019-12, BR-001: the agent's text takes the note's line breaks.
    #[test]
    fn uc019_tc12_line_breaks_follow_the_note() {
        let base = "= Old\r\n* one\r\n";
        let (text, _) = applied(base, &insert(None, "* item\none more")).unwrap();
        assert_eq!(text, "= Old\r\n* one\r\n* item\r\none more");
        let (text, _) = applied(base, &replace("* one\n", "* two\n")).unwrap();
        assert_eq!(text, "= Old\r\n* two\r\n");
    }

    fn merged(current: &str, change: &AgentChange) -> Result<Merged, WriteProblem> {
        let base: String = (1..=10).map(|n| format!("line {n}\n")).collect();
        let splice = plan_splice("Plan", &base, change)?;
        merge_splice("Plan", &base, current, &splice)
    }

    fn typed_in_line_2() -> String {
        (1..=10)
            .map(|n| {
                if n == 2 {
                    "line 2 and more\n".to_owned()
                } else {
                    format!("line {n}\n")
                }
            })
            .collect()
    }

    /// TC-019-06, A5, BR-004: a line the user changed is not touched.
    #[test]
    fn uc019_tc06_same_place_refused() {
        let current = typed_in_line_2();
        for change in [
            replace("line 2\n", "x\n"),
            insert(Some(3), "x"),
            insert(Some(2), "x"),
            AgentChange::Rewrite { text: "x".into() },
        ] {
            assert_eq!(
                merged(&current, &change),
                Err(WriteProblem::UserEditing {
                    note: "Plan".into(),
                    line: 2
                }),
                "{change:?}"
            );
        }
        // A whole line typed by the user between lines 4 and 5.
        let mut inserted = typed_in_line_2().replace("line 2 and more", "line 2");
        inserted = inserted.replace("line 5\n", "new\nline 5\n");
        assert!(merged(&inserted, &insert(Some(5), "x")).is_err());
        assert!(merged(&inserted, &replace("line 4\nline 5", "x\ny")).is_err());
    }

    /// TC-019-07, A6: a change elsewhere is put into the tab's text, which
    /// keeps what the user typed.
    #[test]
    fn uc019_tc07_merged_elsewhere() {
        let current = typed_in_line_2();
        let merge = merged(&current, &replace("line 8", "line eight")).unwrap();
        assert!(merge.text.contains("line 2 and more\n") && merge.text.contains("line eight\n"));
        assert_eq!(merge.lines, (8, 8));
        assert_eq!(&current[merge.range.clone()], "line 8\n");
        // Replacing line 3, next to the changed line 2, is allowed.
        assert!(merged(&current, &replace("line 3", "three")).is_ok());
        // Lines the user removed shift the place.
        let fewer = current.replace("line 4\n", "");
        let merge = merged(&fewer, &insert(Some(9), "x")).unwrap();
        assert_eq!(merge.lines, (8, 8));
        assert_eq!(merge.text.lines().nth(7), Some("x"));
        let end = merged(&current, &insert(None, "end\n")).unwrap();
        assert!(end.text.ends_with("line 10\nend\n"));
    }

    fn workspace(name: &str, files: &[(&str, &str)]) -> (PathBuf, Workspace) {
        let dir =
            std::env::temp_dir().join(format!("underline-write-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        for (path, text) in files {
            let path = dir.join(path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, text).unwrap();
        }
        (dir.clone(), Workspace::open(dir).unwrap())
    }

    /// TC-019-03, TC-019-04, TC-019-10, TC-019-15: written to the files.
    #[test]
    fn uc019_tc03_tc04_tc10_files() {
        let (dir, mut ws) = workspace(
            "files",
            &[
                ("Plan.md", PLAN),
                ("Work/Todo.md", "w"),
                ("Home/Todo.md", "h"),
            ],
        );
        let create = |text: &str| AgentChange::Create {
            format: None,
            text: text.into(),
        };
        let (answer, id) = apply_to_file(&mut ws, "Projects/Ideas", &create("# Ideas\n"));
        assert!(answer.ok, "{answer:?}");
        assert_eq!(answer.message, "created Projects/Ideas");
        assert_eq!(id.unwrap().as_str(), "Projects/Ideas");
        assert_eq!(
            fs::read_to_string(dir.join("Projects/Ideas.md")).unwrap(),
            "# Ideas\n"
        );
        let adoc = AgentChange::Create {
            format: Some(NoteFormat::AsciiDoc),
            text: String::new(),
        };
        assert!(apply_to_file(&mut ws, "Notes", &adoc).0.ok);
        assert!(dir.join("Notes.adoc").is_file());

        let (exists, _) = apply_to_file(&mut ws, "Plan", &create("x"));
        assert!(
            !exists.ok && exists.message.contains("exists"),
            "{}",
            exists.message
        );
        assert_eq!(fs::read_to_string(dir.join("Plan.md")).unwrap(), PLAN);
        for bad in ["A:B", ".hidden/X"] {
            let (answer, _) = apply_to_file(&mut ws, bad, &create("x"));
            assert!(!answer.ok, "{bad}");
        }

        let (answer, _) = apply_to_file(&mut ws, "plan", &replace("- draft", "- final draft"));
        assert!(
            answer.ok && answer.message.contains("line 2"),
            "{}",
            answer.message
        );
        assert!(fs::read_to_string(dir.join("Plan.md"))
            .unwrap()
            .contains("final draft"));
        let (answer, _) = apply_to_file(&mut ws, "Plann", &replace("a", "b"));
        assert!(
            answer.message.contains("closest are: Plan"),
            "{}",
            answer.message
        );
        let (answer, _) = apply_to_file(&mut ws, "Todo", &replace("w", "x"));
        assert!(
            answer.message.contains("several notes"),
            "{}",
            answer.message
        );
        fs::remove_dir_all(dir).unwrap();
    }

    /// ADR-018: requests and answers between the server and the window.
    #[test]
    fn requests_and_answers() {
        let (dir, _) = workspace("requests", &[]);
        let now = SystemTime::now;
        assert!(!window_runs(&dir, now()));
        renew_window_presence(&dir).unwrap();
        assert!(window_runs(&dir, now()));
        assert!(!window_runs(
            &dir,
            now() + PRESENCE_STALE + Duration::from_secs(1)
        ));

        let request = AgentRequest {
            agent: Some("Claude Code".into()),
            note: "Plan".into(),
            change: insert(Some(2), "x"),
        };
        send_request(&dir, "7-1", &request).unwrap();
        assert_eq!(
            take_requests(&dir, now()),
            vec![("7-1".to_owned(), request)]
        );
        assert!(take_requests(&dir, now()).is_empty());
        assert!(!withdraw_request(&dir, "7-1"));
        assert_eq!(take_answer(&dir, "7-1"), None);
        let answer = AgentAnswer {
            ok: true,
            message: "changed Plan".into(),
            note: Some("Plan".into()),
            lines: Some((2, 2)),
        };
        answer_request(&dir, "7-1", &answer).unwrap();
        assert_eq!(take_answer(&dir, "7-1"), Some(answer));
        assert_eq!(take_answer(&dir, "7-1"), None);

        send_request(
            &dir,
            "7-2",
            &AgentRequest {
                agent: None,
                note: "Plan".into(),
                change: AgentChange::Rewrite { text: "x".into() },
            },
        )
        .unwrap();
        assert!(withdraw_request(&dir, "7-2"));
        remove_window_presence(&dir);
        assert!(!window_runs(&dir, now()));
        fs::remove_dir_all(dir).unwrap();
    }
}

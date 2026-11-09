//! UC-018: what an agent looks up through the MCP server: the notes, a
//! search, and the text of a note, always as stored (BR-003).

use std::fs;
use std::io;
use std::sync::Arc;
use std::time::SystemTime;

use crate::conflict::{conflict_places, Joining};
use crate::format::NoteFormat;
use crate::note::NoteId;
use crate::search::{search_notes, SearchRequest};
use crate::workspace::{NoteEntry, Workspace};

/// A5: at most this many notes in one answer of the search.
pub const AGENT_FOUND_NOTES: usize = 50;
/// A5: at most this many lines of each note found.
pub const AGENT_FOUND_LINES: usize = 3;
/// A2: at most this many notes named as closest.
pub const CLOSEST_NOTES: usize = 5;

/// Why a lookup gives no answer.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LookupError {
    /// A2: no note of that name; the closest names.
    #[error("{}", no_such_note(.name, .closest))]
    NoSuchNote { name: String, closest: Vec<NoteId> },
    /// A3: several notes of that title.
    #[error("{}", several_notes(.name, .notes))]
    SeveralNotes { name: String, notes: Vec<NoteId> },
    /// A9: no such folder.
    #[error("there is no folder `{0}` in the workspace")]
    NoSuchFolder(String),
    /// A7: the note's file cannot be read.
    #[error("the note `{name}` cannot be read: {reason}")]
    Unreadable { name: String, reason: String },
    #[error("missing argument `{0}`")]
    Missing(&'static str),
    #[error("{0}")]
    BadLines(String),
}

fn no_such_note(name: &str, closest: &[NoteId]) -> String {
    let mut text = format!("there is no note `{name}` in the workspace");
    if !closest.is_empty() {
        let names: Vec<&str> = closest.iter().map(NoteId::as_str).collect();
        text.push_str(&format!("; the closest are: {}", names.join(", ")));
    }
    text
}

fn several_notes(name: &str, notes: &[NoteId]) -> String {
    let names: Vec<&str> = notes.iter().map(NoteId::as_str).collect();
    format!(
        "there are several notes titled `{name}`: {}; name one with its folder",
        names.join(", ")
    )
}

/// A note as listed to an agent (step 2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListedNote {
    pub entry: NoteEntry,
    pub changed: Option<SystemTime>,
}

/// How names are compared: case and spaces against dashes do not matter
/// (BR-002).
fn normal(name: &str) -> String {
    name.trim()
        .trim_matches('/')
        .split_whitespace()
        .collect::<Vec<_>>()
        .join("-")
        .to_lowercase()
}

/// Steps 1–2, A9, BR-001, BR-009: the notes of the workspace, or of
/// `folder` and the folders in it, the most recently changed first.
pub fn list_for_agent(
    workspace: &Workspace,
    folder: Option<&str>,
) -> Result<Vec<ListedNote>, LookupError> {
    let notes = workspace.notes_now().notes;
    let notes: Vec<NoteEntry> = match folder.map(normal).filter(|f| !f.is_empty()) {
        None => notes,
        Some(wanted) => {
            let inside: Vec<NoteEntry> = notes
                .into_iter()
                .filter(|e| {
                    e.id.folder()
                        .map(normal)
                        .is_some_and(|f| f == wanted || f.starts_with(&format!("{wanted}/")))
                })
                .collect();
            let exists = !inside.is_empty() || workspace.root().join(folder.unwrap_or("")).is_dir();
            if !exists {
                return Err(LookupError::NoSuchFolder(
                    folder.unwrap_or_default().trim().to_owned(),
                ));
            }
            inside
        }
    };
    let mut listed: Vec<ListedNote> = notes
        .into_iter()
        .map(|entry| ListedNote {
            changed: fs::metadata(workspace.file_of(&entry))
                .and_then(|m| m.modified())
                .ok(),
            entry,
        })
        .collect();
    listed.sort_by(|a, b| b.changed.cmp(&a.changed).then(a.entry.id.cmp(&b.entry.id)));
    Ok(listed)
}

/// BR-002, A2, A3: the note `name` names, by folder and title, or by the
/// title alone while it is unique.
pub fn find_note(notes: &[NoteEntry], name: &str) -> Result<NoteEntry, LookupError> {
    let wanted = normal(name);
    if wanted.is_empty() {
        return Err(LookupError::Missing("note"));
    }
    if let Some(entry) = notes.iter().find(|e| normal(e.id.as_str()) == wanted) {
        return Ok(entry.clone());
    }
    if !wanted.contains('/') {
        let titled: Vec<&NoteEntry> = notes
            .iter()
            .filter(|e| normal(&e.id.title()) == wanted)
            .collect();
        match titled.as_slice() {
            [one] => return Ok((*one).clone()),
            [] => {}
            several => {
                return Err(LookupError::SeveralNotes {
                    name: name.trim().to_owned(),
                    notes: several.iter().map(|e| e.id.clone()).collect(),
                })
            }
        }
    }
    Err(LookupError::NoSuchNote {
        name: name.trim().to_owned(),
        closest: closest(notes, &wanted),
    })
}

/// A2: up to [`CLOSEST_NOTES`] notes whose titles come closest to `wanted`.
fn closest(notes: &[NoteEntry], wanted: &str) -> Vec<NoteId> {
    let wanted_title = wanted.rsplit('/').next().unwrap_or(wanted);
    let mut near: Vec<(usize, &NoteId)> = notes
        .iter()
        .filter_map(|e| {
            let title = normal(&e.id.title());
            let distance = edit_distance(&title, wanted_title);
            let similar = distance <= (wanted_title.chars().count() / 3).max(2)
                || (wanted_title.chars().count() >= 3
                    && (title.contains(wanted_title) || wanted_title.contains(&title)));
            similar.then_some((distance, &e.id))
        })
        .collect();
    near.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(b.1)));
    near.into_iter()
        .take(CLOSEST_NOTES)
        .map(|(_, id)| id.clone())
        .collect()
}

fn edit_distance(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut previous = row[0];
        row[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let above = row[j + 1];
            row[j + 1] = (above + 1)
                .min(row[j] + 1)
                .min(previous + usize::from(ca != *cb));
            previous = above;
        }
    }
    row[b.len()]
}

/// What an agent gets when it reads a note (step 6, A1, A8).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoteText {
    pub id: NoteId,
    pub format: NoteFormat,
    /// The lines given, as stored, each ending as in the file.
    pub text: String,
    /// The first and last line given, counted from 1 (BR-009); `None` for
    /// an empty note or a range after its end.
    pub lines: Option<(usize, usize)>,
    /// How many lines the note has.
    pub total: usize,
    /// A1: the range asked for goes beyond the end of the note.
    pub beyond_end: bool,
    /// A8: the note holds places in conflict.
    pub conflicts: usize,
}

impl NoteText {
    /// The line that tells the agent what it got.
    pub fn summary(&self) -> String {
        let lines = |n: usize| {
            if n == 1 {
                "1 line".to_owned()
            } else {
                format!("{n} lines")
            }
        };
        let mut text = format!(
            "{} · {} · {}",
            self.id.as_str(),
            self.format.name(),
            lines(self.total)
        );
        match self.lines {
            Some((from, to)) if (from, to) != (1, self.total) => {
                text.push_str(&format!(" · lines {from} to {to} given"));
            }
            None if self.beyond_end => text.push_str(" · no lines given"),
            _ => {}
        }
        if self.beyond_end {
            text.push_str(&format!(" · the note ends at line {}", self.total));
        }
        if self.conflicts > 0 {
            text.push_str(&format!(
                " · it holds {} the user has not resolved yet; each shows both versions",
                if self.conflicts == 1 {
                    "1 place in conflict".to_owned()
                } else {
                    format!("{} places in conflict", self.conflicts)
                }
            ));
        }
        text
    }
}

/// Step 5–6, A1, A7, A8: reads `entry` as stored, all of it or lines
/// `from` to `to` (counted from 1, both included).
pub fn read_for_agent(
    workspace: &Workspace,
    entry: &NoteEntry,
    from: Option<usize>,
    to: Option<usize>,
) -> Result<NoteText, LookupError> {
    let unreadable = |reason: String| LookupError::Unreadable {
        name: entry.id.as_str().to_owned(),
        reason,
    };
    let text = match fs::read(workspace.file_of(entry)) {
        Ok(bytes) => String::from_utf8(bytes)
            .map_err(|_| unreadable("it is not stored as UTF-8 text".into()))?,
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
            return Err(LookupError::NoSuchNote {
                name: entry.id.as_str().to_owned(),
                closest: Vec::new(),
            })
        }
        Err(err) => return Err(unreadable(err.to_string())),
    };
    let lines: Vec<&str> = text.split_inclusive('\n').collect();
    let total = lines.len();
    let from = from.unwrap_or(1);
    if from == 0 || to == Some(0) {
        return Err(LookupError::BadLines("lines are counted from 1".into()));
    }
    let wanted_to = to.unwrap_or(total.max(from));
    if wanted_to < from {
        return Err(LookupError::BadLines(format!(
            "the last line {wanted_to} comes before the first line {from}"
        )));
    }
    let beyond_end = to.is_some_and(|to| to > total) || from > total.max(1);
    let last = wanted_to.min(total);
    let (given, range) = if from <= last {
        (lines[from - 1..last].concat(), Some((from, last)))
    } else {
        (String::new(), None)
    };
    Ok(NoteText {
        id: entry.id.clone(),
        format: entry.format,
        text: given,
        lines: range,
        total,
        beyond_end,
        conflicts: conflict_places(&text, Joining::Merge).len(),
    })
}

/// One note found by an agent's search, with the lines given (A5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentFound {
    pub id: NoteId,
    /// The line number and the line, shortened around the words when long.
    pub lines: Vec<(usize, String)>,
    /// How many lines hold a word.
    pub places: usize,
}

/// The answer to an agent's search (steps 3–4, A4, A5).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AgentSearch {
    pub found: Vec<AgentFound>,
    /// How many notes match beyond those given.
    pub more_notes: usize,
    pub unreadable: usize,
}

impl AgentSearch {
    /// The answer as text for the agent.
    pub fn to_text(&self, words: &str) -> String {
        if self.found.is_empty() {
            return format!("No note matches {words}.");
        }
        let mut text = String::new();
        let mut more_lines = 0;
        for note in &self.found {
            text.push_str(note.id.as_str());
            text.push('\n');
            for (number, line) in &note.lines {
                text.push_str(&format!("  {number}: {line}\n"));
            }
            more_lines += note.places - note.lines.len();
        }
        if self.more_notes > 0 || more_lines > 0 {
            text.push_str(&format!(
                "{} more notes and {} more lines match; search more narrowly or read a note.\n",
                self.more_notes, more_lines
            ));
        }
        if self.unreadable > 0 {
            text.push_str(&format!("{} notes could not be read.\n", self.unreadable));
        }
        text
    }
}

/// Steps 3–4, BR-004: searches the notes `listed` as stored, by the rules
/// of UC-008, in their order.
pub fn search_for_agent(workspace: &Workspace, listed: &[ListedNote], words: &str) -> AgentSearch {
    let request = SearchRequest {
        text: words.to_owned(),
        notes: listed.iter().map(|l| l.entry.clone()).collect(),
        open: Default::default(),
    };
    let mut texts = std::collections::HashMap::new();
    let result = search_notes(
        &request,
        |entry| {
            let text = fs::read_to_string(workspace.file_of(entry)).ok()?;
            let lowered: Arc<str> = Arc::from(text.to_lowercase());
            let text: Arc<str> = Arc::from(text);
            texts.insert(entry.id.clone(), Arc::clone(&text));
            Some((text, lowered))
        },
        || false,
    )
    .unwrap_or_default();
    let mut answer = AgentSearch {
        unreadable: result.unreadable,
        more_notes: result.found.len().saturating_sub(AGENT_FOUND_NOTES),
        ..AgentSearch::default()
    };
    for note in result.found.into_iter().take(AGENT_FOUND_NOTES) {
        let text = texts
            .get(&note.id)
            .cloned()
            .unwrap_or_else(|| Arc::from(""));
        let lines = note
            .lines
            .iter()
            .take(AGENT_FOUND_LINES)
            .map(|found| {
                let number = text[..found.line.start.min(text.len())]
                    .matches('\n')
                    .count()
                    + 1;
                (number, found.shown.trim_end().to_owned())
            })
            .collect();
        answer.found.push(AgentFound {
            id: note.id,
            lines,
            places: note.places,
        });
    }
    answer
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};
    use std::time::Duration;

    fn workspace(name: &str, files: &[(&str, &str)]) -> (PathBuf, Workspace) {
        let dir =
            std::env::temp_dir().join(format!("underline-lookup-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        for (path, text) in files {
            let path = dir.join(path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, text).unwrap();
        }
        fs::create_dir_all(&dir).unwrap();
        (dir.clone(), Workspace::open(dir).unwrap())
    }

    fn touch(path: &Path, ago: u64) {
        let time = SystemTime::now() - Duration::from_secs(ago);
        fs::File::options()
            .write(true)
            .open(path)
            .unwrap()
            .set_modified(time)
            .unwrap();
    }

    fn names(listed: &[ListedNote]) -> Vec<&str> {
        listed.iter().map(|l| l.entry.id.as_str()).collect()
    }

    /// TC-018-01: list, search, read.
    #[test]
    fn uc018_tc01_list_search_read() {
        let (dir, ws) = workspace(
            "main",
            &[
                ("Plan.md", "# Plan\n- ship the release\n"),
                ("Projects/Ideas.adoc", "= Ideas\n"),
            ],
        );
        touch(&dir.join("Projects/Ideas.adoc"), 100);
        let listed = list_for_agent(&ws, None).unwrap();
        assert_eq!(names(&listed), ["Plan", "Projects/Ideas"]);
        assert_eq!(listed[1].entry.format, NoteFormat::AsciiDoc);

        let found = search_for_agent(&ws, &listed, "release");
        assert_eq!(found.found.len(), 1);
        assert_eq!(found.found[0].lines, [(2, "- ship the release".to_owned())]);

        let plan = find_note(&ws.notes_now().notes, "Plan").unwrap();
        let read = read_for_agent(&ws, &plan, None, None).unwrap();
        assert_eq!(read.text, "# Plan\n- ship the release\n");
        assert_eq!((read.total, read.format), (2, NoteFormat::Markdown));
        assert_eq!(read.summary(), "Plan · Markdown · 2 lines");
        fs::remove_dir_all(dir).unwrap();
    }

    /// TC-018-02: which notes, the most recently changed first.
    #[test]
    fn uc018_tc02_which_notes() {
        let (dir, ws) = workspace(
            "which",
            &[
                ("A.md", "a"),
                ("B.md", "b"),
                (".underline/deleted/1/Old.md", "old"),
                (".hidden/Secret.md", "Secret"),
                ("picture.png", "png"),
                ("Other/underline.yaml", ""),
                ("Other/Inner.md", "Secret"),
            ],
        );
        touch(&dir.join("A.md"), 86_400);
        let listed = list_for_agent(&ws, None).unwrap();
        assert_eq!(names(&listed), ["B", "A"]);
        assert!(search_for_agent(&ws, &listed, "Secret").found.is_empty());
        fs::remove_dir_all(dir).unwrap();
    }

    /// TC-018-03: some lines, and beyond the end.
    #[test]
    fn uc018_tc03_some_lines() {
        let long: String = (1..=500).map(|n| format!("line {n}\n")).collect();
        let (dir, ws) = workspace("lines", &[("Long.md", &long)]);
        let entry = find_note(&ws.notes_now().notes, "Long").unwrap();
        let part = read_for_agent(&ws, &entry, Some(10), Some(12)).unwrap();
        assert_eq!(part.text, "line 10\nline 11\nline 12\n");
        assert_eq!((part.lines, part.total), (Some((10, 12)), 500));
        assert_eq!(
            part.summary(),
            "Long · Markdown · 500 lines · lines 10 to 12 given"
        );
        let end = read_for_agent(&ws, &entry, Some(498), Some(510)).unwrap();
        assert_eq!(end.text, "line 498\nline 499\nline 500\n");
        assert!(end.beyond_end && end.summary().contains("ends at line 500"));
        let after = read_for_agent(&ws, &entry, Some(600), None).unwrap();
        assert!(after.text.is_empty() && after.beyond_end);
        assert!(read_for_agent(&ws, &entry, Some(0), None).is_err());
        assert!(read_for_agent(&ws, &entry, Some(5), Some(3)).is_err());
        fs::remove_dir_all(dir).unwrap();
    }

    /// TC-018-04, TC-018-05: names, closest names, and the same title in
    /// several folders.
    #[test]
    fn uc018_tc04_tc05_naming() {
        let notes: Vec<NoteEntry> = [
            "Plan",
            "Planning",
            "Release",
            "Work/Todo",
            "Home/Todo",
            "My-Notes",
        ]
        .iter()
        .map(|id| NoteEntry {
            id: NoteId::from_relative(id),
            format: NoteFormat::Markdown,
        })
        .collect();
        let id = |name: &str| find_note(&notes, name).map(|e| e.id.as_str().to_owned());
        assert_eq!(id("plan").unwrap(), "Plan");
        assert_eq!(id("My Notes").unwrap(), "My-Notes");
        assert_eq!(id("work/todo").unwrap(), "Work/Todo");
        match find_note(&notes, "Plann") {
            Err(LookupError::NoSuchNote { closest, .. }) => {
                let closest: Vec<&str> = closest.iter().map(NoteId::as_str).collect();
                assert_eq!(closest, ["Plan", "Planning"]);
            }
            other => panic!("{other:?}"),
        }
        match find_note(&notes, "Todo") {
            Err(LookupError::SeveralNotes { notes, .. }) => assert_eq!(notes.len(), 2),
            other => panic!("{other:?}"),
        }
        assert!(find_note(&notes, "Nothing at all here").is_err());
    }

    /// TC-018-06: nothing found, many found, quoted words.
    #[test]
    fn uc018_tc06_search_limits() {
        let mut files: Vec<(String, String)> = (0..80)
            .map(|n| (format!("N{n:02}.md"), "todo\n".to_owned()))
            .collect();
        files.push(("Many.md".into(), "todo\n".repeat(10)));
        files.push(("Ship.md".into(), "ship the release\nthe ship\n".into()));
        let refs: Vec<(&str, &str)> = files
            .iter()
            .map(|(a, b)| (a.as_str(), b.as_str()))
            .collect();
        let (dir, ws) = workspace("limits", &refs);
        let listed = list_for_agent(&ws, None).unwrap();
        let none = search_for_agent(&ws, &listed, "zebra");
        assert!(none.found.is_empty() && none.to_text("zebra").starts_with("No note"));
        let many = search_for_agent(&ws, &listed, "todo");
        assert_eq!((many.found.len(), many.more_notes), (50, 31));
        assert!(many.found.iter().all(|f| f.lines.len() <= 3));
        assert!(many.to_text("todo").contains("31 more notes"));
        let quoted = search_for_agent(&ws, &listed, "\"ship the\"");
        assert_eq!(quoted.found[0].lines, [(1, "ship the release".to_owned())]);
        assert_eq!(search_for_agent(&ws, &listed, "SHIP").found[0].places, 2);
        fs::remove_dir_all(dir).unwrap();
    }

    /// TC-018-08, TC-018-09: deleted in between; places in conflict.
    #[test]
    fn uc018_tc08_tc09_unreadable_and_conflicts() {
        let (dir, ws) = workspace(
            "conflict",
            &[
                ("Ideas.md", "x"),
                (
                    "Plan.md",
                    "a\n<<<<<<< HEAD\nmine\n=======\ntheirs\n>>>>>>> origin/main\n",
                ),
            ],
        );
        let notes = ws.notes_now().notes;
        let ideas = find_note(&notes, "Ideas").unwrap();
        fs::remove_file(dir.join("Ideas.md")).unwrap();
        assert!(matches!(
            read_for_agent(&ws, &ideas, None, None),
            Err(LookupError::NoSuchNote { .. })
        ));
        let plan = find_note(&notes, "Plan").unwrap();
        let read = read_for_agent(&ws, &plan, None, None).unwrap();
        assert_eq!(read.conflicts, 1);
        assert!(read.text.contains("mine") && read.text.contains("theirs"));
        assert!(read.summary().contains("1 place in conflict"));
        fs::remove_dir_all(dir).unwrap();
    }

    /// TC-018-10: one folder only.
    #[test]
    fn uc018_tc10_one_folder() {
        let (dir, ws) = workspace(
            "folder",
            &[
                ("Plan.md", "p"),
                ("Projects/Ideas.md", "i"),
                ("Projects/2026/Q1.md", "q"),
                ("Empty/.keep", ""),
            ],
        );
        let mut inside = names(&list_for_agent(&ws, Some("projects")).unwrap())
            .into_iter()
            .map(str::to_owned)
            .collect::<Vec<_>>();
        inside.sort();
        assert_eq!(inside, ["Projects/2026/Q1", "Projects/Ideas"]);
        assert!(list_for_agent(&ws, Some("Empty")).unwrap().is_empty());
        assert_eq!(
            list_for_agent(&ws, Some("Nowhere")),
            Err(LookupError::NoSuchFolder("Nowhere".into()))
        );
        fs::remove_dir_all(dir).unwrap();
    }

    /// TC-018-12: looking up changes nothing.
    #[test]
    fn uc018_tc12_only_reading() {
        let (dir, ws) = workspace("reading", &[("Plan.md", "p"), ("Ideas.md", "i")]);
        let before: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().metadata().unwrap().modified().unwrap())
            .collect();
        let listed = list_for_agent(&ws, None).unwrap();
        search_for_agent(&ws, &listed, "i");
        let ideas = find_note(&ws.notes_now().notes, "Ideas").unwrap();
        read_for_agent(&ws, &ideas, None, None).unwrap();
        let after: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().metadata().unwrap().modified().unwrap())
            .collect();
        assert_eq!(before, after);
        assert!(!dir.join(".underline").exists());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn edit_distances() {
        assert_eq!(edit_distance("plan", "plann"), 1);
        assert_eq!(edit_distance("", "abc"), 3);
        assert_eq!(edit_distance("kitten", "sitting"), 3);
    }
}

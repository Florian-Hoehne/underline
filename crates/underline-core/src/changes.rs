//! UC-011 Review Changes: the branch of the workspace, the notes and files
//! changed since the last commit, the diff of a note, and discarding the
//! changes of a note. git is asked through the installed program (ADR-006);
//! the diff itself is worked out here, so the unsaved text of a tab counts
//! too (BR-001).

use std::fs;
use std::ops::Range;
use std::path::Path;

use similar::{ChangeTag, DiffTag, TextDiff};

use crate::atomic::write_atomic;
use crate::format::NoteFormat;
use crate::git::{Git, GitError};
use crate::note::NoteId;

/// Where the workspace stands (UC-011 step 1, A7, A8).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BranchState {
    Branch(String),
    /// A7: the branch the first commit will make.
    NoCommit(String),
    /// A8: not on a branch, on this commit (short).
    Detached(String),
}

/// How a file changed since the last commit (BR-003).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChangeKind {
    New,
    Changed,
    Deleted,
    /// Renamed from this place, maybe changed too.
    Renamed {
        from: String,
    },
}

impl ChangeKind {
    /// The letter of the mark (BR-003).
    pub fn letter(&self) -> char {
        match self {
            Self::New => 'N',
            Self::Changed => 'M',
            Self::Deleted => 'D',
            Self::Renamed { .. } => 'R',
        }
    }
}

/// A file changed since the last commit, its place relative to the
/// workspace with `/`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileChange {
    pub path: String,
    pub kind: ChangeKind,
}

impl FileChange {
    /// The note it is, if it is a note.
    pub fn note(&self) -> Option<(NoteId, NoteFormat)> {
        let (stem, extension) = self.path.rsplit_once('.')?;
        let format = NoteFormat::from_extension(extension)?;
        Some((NoteId::from_relative(stem), format))
    }
}

/// The state of a workspace against its last commit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Changes {
    pub branch: BranchState,
    /// Notes first, then other files, each by place (BR-002).
    pub files: Vec<FileChange>,
    /// UC-012 BR-009: a merge waits for its conflicts to be resolved.
    pub merging: bool,
    /// UC-012 BR-006: the message of the last commit, when it may still be
    /// amended because the remote does not hold it.
    pub amendable: Option<String>,
}

impl Changes {
    /// The change of the note `id`, if it changed.
    pub fn of_note(&self, id: &NoteId) -> Option<&FileChange> {
        self.files
            .iter()
            .find(|c| c.note().is_some_and(|(n, _)| &n == id))
    }
}

/// How much of the lines a deleted and a new note must share to count as
/// one note renamed, as git judges renames.
const RENAME_SIMILARITY: f32 = 0.5;

impl Git {
    /// UC-011 steps 1 and 4: the branch and the changed files of the
    /// workspace at `dir`, which may be a folder inside its repository.
    pub fn changes(&self, dir: &Path) -> Result<Changes, GitError> {
        let prefix = self
            .run(dir, &["rev-parse", "--show-prefix"])?
            .trim()
            .to_owned();
        let out = self.run(
            dir,
            &[
                "status",
                "--porcelain=v2",
                "--branch",
                "-z",
                "--untracked-files=all",
                "--",
                ".",
            ],
        )?;
        let mut head = None;
        let mut oid = None;
        let mut files = Vec::new();
        let mut entries = out.split('\0').filter(|e| !e.is_empty());
        while let Some(entry) = entries.next() {
            if let Some(header) = entry.strip_prefix("# ") {
                if let Some(name) = header.strip_prefix("branch.head ") {
                    head = Some(name.to_owned());
                } else if let Some(id) = header.strip_prefix("branch.oid ") {
                    oid = Some(id.to_owned());
                }
                continue;
            }
            let fields: Vec<&str> = entry.splitn(9, ' ').collect();
            let (kind, path) = match fields.first().copied() {
                Some("?") => (ChangeKind::New, entry[2..].to_owned()),
                Some("1") if fields.len() == 9 => {
                    let xy = fields[1];
                    let kind = if xy.contains('D') {
                        ChangeKind::Deleted
                    } else if xy.starts_with('A') {
                        ChangeKind::New
                    } else {
                        ChangeKind::Changed
                    };
                    (kind, fields[8].to_owned())
                }
                Some("2") => {
                    // A rename git already knows: the next entry is its origin.
                    let fields: Vec<&str> = entry.splitn(10, ' ').collect();
                    let from = entries.next().unwrap_or_default().to_owned();
                    let Some(path) = fields.get(9) else { continue };
                    (
                        ChangeKind::Renamed {
                            from: strip(&prefix, &from),
                        },
                        (*path).to_owned(),
                    )
                }
                Some("u") => {
                    let fields: Vec<&str> = entry.splitn(11, ' ').collect();
                    let Some(path) = fields.get(10) else { continue };
                    (ChangeKind::Changed, (*path).to_owned())
                }
                _ => continue,
            };
            files.push(FileChange {
                path: strip(&prefix, &path),
                kind,
            });
        }
        let branch = match (head.as_deref(), oid.as_deref()) {
            (Some("(detached)"), Some(oid)) => BranchState::Detached(oid.chars().take(7).collect()),
            (Some(name), Some("(initial)")) => BranchState::NoCommit(name.to_owned()),
            (Some(name), _) => BranchState::Branch(name.to_owned()),
            _ => BranchState::Detached(String::new()),
        };
        self.pair_renames(dir, &mut files);
        files.sort_by(|a, b| {
            (a.note().is_none(), a.path.to_lowercase())
                .cmp(&(b.note().is_none(), b.path.to_lowercase()))
        });
        let merging = self
            .run(dir, &["rev-parse", "-q", "--verify", "MERGE_HEAD"])
            .is_ok();
        let amendable = match branch {
            BranchState::Branch(_) | BranchState::Detached(_) => self.amendable(dir),
            BranchState::NoCommit(_) => None,
        };
        Ok(Changes {
            branch,
            files,
            merging,
            amendable,
        })
    }

    /// The message of the last commit while the remote branch does not hold
    /// it (UC-012 BR-006).
    fn amendable(&self, dir: &Path) -> Option<String> {
        if let Ok(upstream) = self.run(dir, &["rev-parse", "--abbrev-ref", "@{upstream}"]) {
            let upstream = upstream.trim().to_owned();
            if self
                .run(dir, &["merge-base", "--is-ancestor", "HEAD", &upstream])
                .is_ok()
            {
                return None;
            }
        }
        self.run(dir, &["log", "-1", "--format=%B"])
            .ok()
            .map(|m| m.trim_end().to_owned())
    }

    /// UC-012 step 6, A6: records `changes` with `message` as a new commit,
    /// or as the last commit amended; returns the commit's short name. Only
    /// the paths of `changes` are committed; whatever else is staged stays
    /// staged (BR-001).
    pub fn commit_changes(
        &self,
        dir: &Path,
        changes: &[FileChange],
        message: &str,
        amend: bool,
    ) -> Result<String, GitError> {
        let mut paths: Vec<&str> = Vec::new();
        let mut added: Vec<&str> = Vec::new();
        for change in changes {
            paths.push(&change.path);
            match &change.kind {
                ChangeKind::Deleted => {}
                ChangeKind::Renamed { from } => {
                    paths.push(from);
                    added.push(&change.path);
                }
                _ => added.push(&change.path),
            }
        }
        if !added.is_empty() {
            let mut add = vec!["add", "--"];
            add.extend(added);
            self.run(dir, &add)?;
        }
        let mut commit = vec!["commit", "--quiet", "--message", message];
        if amend {
            commit.push("--amend");
        }
        if paths.is_empty() {
            commit.push("--only");
        } else {
            commit.push("--");
            commit.extend(paths);
        }
        self.run(dir, &commit)?;
        self.run(dir, &["rev-parse", "--short", "HEAD"])
            .map(|id| id.trim().to_owned())
    }

    /// UC-012 A3, BR-005: keeps the user's name and email in their own git
    /// settings, for every repository.
    pub fn set_identity(&self, dir: &Path, name: &str, email: &str) -> Result<(), GitError> {
        self.run(dir, &["config", "--global", "user.name", name])?;
        self.run(dir, &["config", "--global", "user.email", email])
            .map(drop)
    }
}

/// UC-012 BR-002: a message naming what changed by the titles of the notes.
pub fn proposed_message(changes: &[&FileChange]) -> String {
    let mut updated = Vec::new();
    let mut added = Vec::new();
    let mut deleted = Vec::new();
    let mut renamed = Vec::new();
    let mut others = 0;
    for change in changes {
        let Some((id, _)) = change.note() else {
            others += 1;
            continue;
        };
        match &change.kind {
            ChangeKind::Changed => updated.push(id.title()),
            ChangeKind::New => added.push(id.title()),
            ChangeKind::Deleted => deleted.push(id.title()),
            ChangeKind::Renamed { from } => {
                let old = from.rsplit_once('.').map_or(from.as_str(), |(s, _)| s);
                renamed.push(format!(
                    "{} to {}",
                    NoteId::from_relative(old).title(),
                    id.title()
                ));
            }
        }
    }
    let name = |verb: &str, titles: &[String]| -> Option<String> {
        match titles.len() {
            0 => None,
            1..=3 => Some(format!("{verb} {}", join_titles(titles))),
            n => Some(format!(
                "{verb} {} and {} more notes",
                titles[..3].join(", "),
                n - 3
            )),
        }
    };
    let mut parts: Vec<String> = [
        name("Update", &updated),
        name("Add", &added),
        name("Delete", &deleted),
        name("Rename", &renamed),
    ]
    .into_iter()
    .flatten()
    .collect();
    let files = match others {
        0 => None,
        1 => Some("1 other file".to_owned()),
        n => Some(format!("{n} other files")),
    };
    match (parts.last_mut(), files) {
        (Some(last), Some(files)) => {
            last.push_str(" and ");
            last.push_str(&files);
        }
        (None, Some(files)) => parts.push(format!("Update {files}")),
        _ => {}
    }
    parts.join("; ")
}

/// `A`, `A, B`, `A, B, C`.
fn join_titles(titles: &[String]) -> String {
    titles.join(", ")
}

impl Git {
    /// A deleted note and a new note of the same format that share most of
    /// their lines are one note renamed (UC-006), as git would find it once
    /// both were recorded.
    fn pair_renames(&self, dir: &Path, files: &mut Vec<FileChange>) {
        let deleted: Vec<usize> = (0..files.len())
            .filter(|&i| files[i].kind == ChangeKind::Deleted && files[i].note().is_some())
            .collect();
        if deleted.is_empty() {
            return;
        }
        let mut gone = Vec::new();
        for i in deleted {
            let Some(old) = self.committed(dir, &files[i].path).ok().flatten() else {
                continue;
            };
            let format = files[i].note().map(|(_, f)| f);
            let best = (0..files.len())
                .filter(|&j| {
                    files[j].kind == ChangeKind::New && files[j].note().map(|(_, f)| f) == format
                })
                .filter_map(|j| {
                    let new = fs::read_to_string(dir.join(&files[j].path)).ok()?;
                    let ratio = TextDiff::from_lines(&old, &new).ratio();
                    (ratio >= RENAME_SIMILARITY).then_some((j, ratio))
                })
                .max_by(|a, b| a.1.total_cmp(&b.1));
            if let Some((j, _)) = best {
                files[j].kind = ChangeKind::Renamed {
                    from: files[i].path.clone(),
                };
                gone.push(i);
            }
        }
        gone.sort_unstable();
        for i in gone.into_iter().rev() {
            files.remove(i);
        }
    }

    /// The content `path` (relative to the workspace at `dir`) had at the
    /// last commit, or `None` when it was not in it.
    pub fn committed(&self, dir: &Path, path: &str) -> Result<Option<String>, GitError> {
        match self.run(dir, &["show", &format!("HEAD:./{path}")]) {
            Ok(text) => Ok(Some(text)),
            Err(GitError::Failed { .. }) => Ok(None),
            Err(err) => Err(err),
        }
    }
}

/// `path` from the root of the repository, made relative to the workspace.
fn strip(prefix: &str, path: &str) -> String {
    path.strip_prefix(prefix).unwrap_or(path).to_owned()
}

/// What a line of a diff is (BR-004).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineKind {
    Same,
    Removed,
    Added,
}

/// Why a changed line differs only a little (BR-004).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OnlyDiffers {
    Spaces,
    LineEnding,
}

/// A line of a diff.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffLine {
    pub kind: LineKind,
    /// Its numbers in the last commit and now, from 1.
    pub old: Option<usize>,
    pub new: Option<usize>,
    /// The line without its line ending.
    pub text: String,
    /// Byte ranges of `text` removed or added within the line.
    pub words: Vec<Range<usize>>,
    pub only: Option<OnlyDiffers>,
}

/// A changed part of a note with the lines around it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hunk {
    pub lines: Vec<DiffLine>,
}

/// UC-011 step 6, BR-004: the changed parts of `new` against `old`, with
/// `context` unchanged lines around each, parts joined when they come close.
pub fn diff(old: &str, new: &str, context: usize) -> Vec<Hunk> {
    let diff = TextDiff::from_lines(old, new);
    let mut hunks = Vec::new();
    for group in diff.grouped_ops(context) {
        let mut lines = Vec::new();
        for op in &group {
            let replace = op.tag() == DiffTag::Replace;
            let first_line = lines.len();
            for change in diff.iter_inline_changes(op) {
                let kind = match change.tag() {
                    ChangeTag::Equal => LineKind::Same,
                    ChangeTag::Delete => LineKind::Removed,
                    ChangeTag::Insert => LineKind::Added,
                };
                let mut text = String::new();
                let mut words = Vec::new();
                for (emphasized, part) in change.iter_strings_lossy() {
                    let start = text.len();
                    text.push_str(&part);
                    if emphasized && replace {
                        words.push(start..text.len());
                    }
                }
                let ending = text.len() - text.trim_end_matches(['\n', '\r']).len();
                let full = text.clone();
                text.truncate(text.len() - ending);
                let end = text.len();
                words.retain_mut(|w| {
                    w.end = w.end.min(end);
                    w.start < w.end
                });
                lines.push((
                    DiffLine {
                        kind,
                        old: change.old_index().map(|i| i + 1),
                        new: change.new_index().map(|i| i + 1),
                        text,
                        words,
                        only: None,
                    },
                    full,
                ));
            }
            if replace {
                mark_small_differences(&mut lines[first_line..]);
            }
        }
        hunks.push(Hunk {
            lines: lines.into_iter().map(|(line, _)| line).collect(),
        });
    }
    hunks
}

/// Pairs removed and added lines of one replaced part and notes those that
/// differ only in spaces or their line ending.
fn mark_small_differences(lines: &mut [(DiffLine, String)]) {
    let removed: Vec<usize> = (0..lines.len())
        .filter(|&i| lines[i].0.kind == LineKind::Removed)
        .collect();
    let added: Vec<usize> = (0..lines.len())
        .filter(|&i| lines[i].0.kind == LineKind::Added)
        .collect();
    for (&r, &a) in removed.iter().zip(&added) {
        let (old, new) = (&lines[r].1, &lines[a].1);
        let only = if old.trim_end_matches(['\n', '\r']) == new.trim_end_matches(['\n', '\r']) {
            Some(OnlyDiffers::LineEnding)
        } else if old.split_whitespace().eq(new.split_whitespace()) {
            Some(OnlyDiffers::Spaces)
        } else {
            None
        };
        if only.is_some() {
            lines[r].0.only = only;
            lines[a].0.only = only;
        }
    }
}

/// How many lines a diff removes or adds (UC-011 A6 step 1).
pub fn changed_lines(hunks: &[Hunk]) -> usize {
    hunks
        .iter()
        .flat_map(|h| &h.lines)
        .filter(|l| l.kind != LineKind::Same)
        .count()
}

/// UC-011 A6 step 2 for a changed or deleted note, and for the old place of
/// a renamed one: writes its committed content back at `path` (relative to
/// the workspace at `dir`).
pub fn restore_committed(git: &Git, dir: &Path, path: &str) -> Result<(), DiscardError> {
    let content = git
        .committed(dir, path)?
        .ok_or_else(|| DiscardError::NotCommitted(path.to_owned()))?;
    let target = dir.join(path);
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(DiscardError::Io)?;
    }
    write_atomic(&target, content.as_bytes()).map_err(DiscardError::Io)
}

/// Why changes could not be discarded.
#[derive(Debug, thiserror::Error)]
pub enum DiscardError {
    #[error(transparent)]
    Git(#[from] GitError),
    #[error("{0} is not in the last commit")]
    NotCommitted(String),
    #[error("cannot write the note: {0}")]
    Io(std::io::Error),
}

/// Asks git for the changes in the background (UC-011 BR-006, A3), so
/// typing never waits for git; requests that pile up are answered once.
pub struct ChangeTracker {
    requests: std::sync::mpsc::Sender<()>,
    results: std::sync::mpsc::Receiver<Result<Changes, String>>,
}

impl ChangeTracker {
    /// Starts the tracker of the workspace at `dir`; `wake` is called when
    /// an answer is ready. It answers once at the start.
    pub fn start(git: Git, dir: std::path::PathBuf, wake: impl Fn() + Send + 'static) -> Self {
        let (requests, inbox) = std::sync::mpsc::channel::<()>();
        let (outbox, results) = std::sync::mpsc::channel();
        std::thread::Builder::new()
            .name("underline-changes".into())
            .spawn(move || {
                while inbox.recv().is_ok() {
                    while inbox.try_recv().is_ok() {}
                    let answer = git.changes(&dir).map_err(|e| e.to_string());
                    if outbox.send(answer).is_err() {
                        return;
                    }
                    wake();
                }
            })
            .expect("the changes thread starts");
        let tracker = Self { requests, results };
        tracker.refresh();
        tracker
    }

    /// Asks again.
    pub fn refresh(&self) {
        let _ = self.requests.send(());
    }

    /// The newest answer that arrived, if any.
    pub fn poll(&self) -> Option<Result<Changes, String>> {
        self.results.try_iter().last()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::tests::isolated_git;

    struct Repo {
        dir: std::path::PathBuf,
        git: Git,
    }

    impl Repo {
        fn new(name: &str) -> Self {
            let base =
                std::env::temp_dir().join(format!("underline-uc011-{name}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&base);
            let git = isolated_git(
                &base.join("home"),
                "[user]\n\tname = T\n\temail = t@example.com\n[init]\n\tdefaultBranch = main\n",
            );
            let dir = base.join("ws");
            fs::create_dir_all(&dir).unwrap();
            git.init(&dir, "main").unwrap();
            Self { dir, git }
        }

        fn write(&self, path: &str, text: &str) {
            let path = self.dir.join(path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, text).unwrap();
        }

        fn commit(&self) {
            self.git.run(&self.dir, &["add", "-A"]).unwrap();
            self.git
                .run(&self.dir, &["commit", "--quiet", "-m", "c"])
                .unwrap();
        }

        fn kinds(&self) -> Vec<(String, ChangeKind)> {
            self.git
                .changes(&self.dir)
                .unwrap()
                .files
                .into_iter()
                .map(|c| (c.path, c.kind))
                .collect()
        }
    }

    impl Drop for Repo {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(self.dir.parent().unwrap());
        }
    }

    /// TC-011-01 / TC-011-02 / TC-011-11 / TC-011-14: the branch, new,
    /// changed, deleted, renamed notes, and other files after them.
    #[test]
    fn uc011_tc01_tc02_tc11_tc14_changes() {
        let repo = Repo::new("changes");
        repo.write("Home.md", "# Home\n");
        let changes = repo.git.changes(&repo.dir).unwrap();
        assert_eq!(changes.branch, BranchState::NoCommit("main".into()));
        assert_eq!(repo.kinds(), [("Home.md".into(), ChangeKind::New)]);
        let draft: String = (0..20).map(|n| format!("line {n}\n")).collect();
        repo.write("Plan.md", "one\ntwo\n");
        repo.write("Old.md", "old\n");
        repo.write("Draft.md", &draft);
        repo.commit();
        let changes = repo.git.changes(&repo.dir).unwrap();
        assert_eq!(changes.branch, BranchState::Branch("main".into()));
        assert!(changes.files.is_empty());
        repo.write("Plan.md", "one\nTWO\n");
        repo.write("New.md", "new\n");
        fs::remove_file(repo.dir.join("Old.md")).unwrap();
        fs::remove_file(repo.dir.join("Draft.md")).unwrap();
        repo.write("Final.md", &format!("{draft}one more\n"));
        repo.write("img/shot.png", "png");
        assert_eq!(
            repo.kinds(),
            [
                (
                    "Final.md".into(),
                    ChangeKind::Renamed {
                        from: "Draft.md".into()
                    }
                ),
                ("New.md".into(), ChangeKind::New),
                ("Old.md".into(), ChangeKind::Deleted),
                ("Plan.md".into(), ChangeKind::Changed),
                ("img/shot.png".into(), ChangeKind::New),
            ]
        );
        assert_eq!(
            repo.git.committed(&repo.dir, "Plan.md").unwrap().as_deref(),
            Some("one\ntwo\n")
        );
        assert_eq!(repo.git.committed(&repo.dir, "New.md").unwrap(), None);
    }

    /// TC-011-06 / TC-011-15: ignored files are not shown; a workspace not
    /// on a branch shows its commit.
    #[test]
    fn uc011_tc06_tc15_ignored_and_detached() {
        let repo = Repo::new("ignored");
        repo.write(".gitignore", "build/\n");
        repo.write("A.md", "a\n");
        repo.commit();
        repo.write("build/out.md", "x\n");
        assert!(repo.kinds().is_empty());
        repo.git
            .run(&repo.dir, &["checkout", "--quiet", "--detach"])
            .unwrap();
        let changes = repo.git.changes(&repo.dir).unwrap();
        assert!(matches!(changes.branch, BranchState::Detached(ref c) if c.len() == 7));
    }

    /// TC-012-01 / TC-012-02 / TC-012-05 / TC-012-12: commit chosen
    /// changes, renamed and deleted notes, and amend the last commit.
    #[test]
    fn uc012_tc01_tc02_tc05_tc12_commit() {
        let repo = Repo::new("commit");
        repo.write("Plan.md", "plan\n");
        repo.write("Ideas.md", "ideas\n");
        let draft: String = (0..20).map(|n| format!("line {n}\n")).collect();
        repo.write("Draft.md", &draft);
        repo.write("Old.md", "old\n");
        repo.commit();
        repo.write("Plan.md", "plan 2\n");
        repo.write("Ideas.md", "ideas 2\n");
        fs::remove_file(repo.dir.join("Old.md")).unwrap();
        fs::remove_file(repo.dir.join("Draft.md")).unwrap();
        repo.write("Final.md", &draft);
        repo.write("img/shot.png", "png");
        let changes = repo.git.changes(&repo.dir).unwrap();
        assert!(changes.amendable.is_some());
        let chosen: Vec<FileChange> = changes
            .files
            .iter()
            .filter(|c| c.path != "Ideas.md")
            .cloned()
            .collect();
        let id = repo
            .git
            .commit_changes(&repo.dir, &chosen, "Plan the trip", false)
            .unwrap();
        assert_eq!(id.len(), 7);
        assert_eq!(repo.kinds(), [("Ideas.md".into(), ChangeKind::Changed)]);
        let stat = repo
            .git
            .run(&repo.dir, &["show", "--stat", "--format=%s", "-M", "HEAD"])
            .unwrap();
        assert!(stat.starts_with("Plan the trip"), "{stat}");
        assert!(stat.contains("Draft.md => Final.md"), "{stat}");
        assert!(stat.contains("Old.md"), "{stat}");
        // Amend with the remaining note and a new message.
        let rest = repo.git.changes(&repo.dir).unwrap();
        assert_eq!(rest.amendable.as_deref(), Some("Plan the trip"));
        repo.git
            .commit_changes(&repo.dir, &rest.files, "Plan the trip, with ideas", true)
            .unwrap();
        assert!(repo.kinds().is_empty());
        let log = repo.git.run(&repo.dir, &["log", "--format=%s"]).unwrap();
        assert_eq!(log, "Plan the trip, with ideas\nc\n");
        // The message alone can be amended too.
        repo.git
            .commit_changes(&repo.dir, &[], "Planned", true)
            .unwrap();
        let log = repo
            .git
            .run(&repo.dir, &["log", "-1", "--format=%s"])
            .unwrap();
        assert_eq!(log.trim(), "Planned");
    }

    /// TC-012-03: the proposed message.
    #[test]
    fn uc012_tc03_proposed_message() {
        let change = |path: &str, kind: ChangeKind| FileChange {
            path: path.into(),
            kind,
        };
        let files = [
            change("A.md", ChangeKind::Changed),
            change("B.md", ChangeKind::Changed),
            change("C.md", ChangeKind::Changed),
            change("D.md", ChangeKind::Changed),
            change("E.md", ChangeKind::Changed),
            change("New.md", ChangeKind::New),
            change("Old.md", ChangeKind::Deleted),
            change(
                "Final.md",
                ChangeKind::Renamed {
                    from: "Draft.md".into(),
                },
            ),
            change("img/a.png", ChangeKind::Changed),
            change("img/b.png", ChangeKind::New),
        ];
        let all: Vec<&FileChange> = files.iter().collect();
        assert_eq!(
            proposed_message(&all),
            "Update A, B, C and 2 more notes; Add New; Delete Old; Rename Draft to Final and 2 other files"
        );
        assert_eq!(proposed_message(&all[..2]), "Update A, B");
        assert_eq!(proposed_message(&all[8..9]), "Update 1 other file");
        assert_eq!(proposed_message(&[]), "");
    }

    /// TC-011-01 / TC-011-04: context, joined parts, and changed words.
    #[test]
    fn uc011_tc04_diff() {
        let old: String = (1..=60).map(|n| format!("line {n} word\n")).collect();
        let new = old
            .replace("line 5 word", "line 5 WORD")
            .replace("line 9 word", "line 9 WORD")
            .replace("line 40 word", "line 40 WORD");
        let hunks = diff(&old, &new, 3);
        assert_eq!(hunks.len(), 2);
        let numbers = |h: &Hunk| (h.lines.first().unwrap().old, h.lines.last().unwrap().old);
        assert_eq!(numbers(&hunks[0]), (Some(2), Some(12)));
        assert_eq!(numbers(&hunks[1]), (Some(37), Some(43)));
        let added = hunks[0]
            .lines
            .iter()
            .find(|l| l.kind == LineKind::Added)
            .unwrap();
        assert_eq!(added.new, Some(5));
        assert_eq!(
            added
                .words
                .iter()
                .map(|w| &added.text[w.clone()])
                .collect::<Vec<_>>(),
            ["WORD"]
        );
        assert_eq!(changed_lines(&hunks), 6);
    }

    /// TC-011-05: spaces and line endings.
    #[test]
    fn uc011_tc05_spaces_and_line_endings() {
        let hunks = diff("a b\nc\n", "a b  \nc\r\n", 1);
        let only: Vec<_> = hunks[0].lines.iter().filter_map(|l| l.only).collect();
        assert!(only.contains(&OnlyDiffers::Spaces));
        assert!(only.contains(&OnlyDiffers::LineEnding));
    }

    /// TC-011-12: discarding writes the committed content back.
    #[test]
    fn uc011_tc12_restore_committed() {
        let repo = Repo::new("discard");
        repo.write("Plan.md", "committed\n");
        repo.commit();
        repo.write("Plan.md", "changed\n");
        restore_committed(&repo.git, &repo.dir, "Plan.md").unwrap();
        assert_eq!(
            fs::read_to_string(repo.dir.join("Plan.md")).unwrap(),
            "committed\n"
        );
        fs::remove_file(repo.dir.join("Plan.md")).unwrap();
        restore_committed(&repo.git, &repo.dir, "Plan.md").unwrap();
        assert!(repo.kinds().is_empty());
        assert!(matches!(
            restore_committed(&repo.git, &repo.dir, "New.md"),
            Err(DiscardError::NotCommitted(_))
        ));
    }
}

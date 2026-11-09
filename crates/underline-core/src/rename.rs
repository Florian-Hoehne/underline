//! UC-006 Rename Note: the new title and folder of a note, the move of its
//! file, and the places its links and the links to it name afterwards.
//!
//! Finding the links in the markup of a note is the facade's part; this
//! module says, for one link as written, what it has to say after the rename.

use std::fs;
use std::io;
use std::path::Path;

use crate::atomic::{write_atomic, write_new_atomic};
use crate::embed::relative;
use crate::format::NoteFormat;
use crate::links::{normalize, percent_decode};
use crate::new_note::{
    folder_levels, heading_title, page_name, topmost_missing, NewNote, NewNoteProblem,
};
use crate::note::NoteId;
use crate::workspace::Workspace;

/// What the user chose in the rename dialog (UC-006 steps 2–3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rename {
    /// Folder inside the workspace with `/` between levels; empty for the top
    /// folder.
    pub folder: String,
    pub title: String,
}

/// A planned rename: the note `from` becomes `to`, in its note format
/// (UC-006 BR-002).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoteMove {
    pub from: NoteId,
    pub to: NoteId,
    pub format: NoteFormat,
    /// The new title as the note shows it in its heading and in link texts.
    pub title: String,
}

impl NoteMove {
    /// The file of the note before the rename, relative to the workspace.
    pub fn from_file(&self) -> String {
        self.from.file_name(self.format)
    }

    /// The file of the note after the rename, relative to the workspace.
    pub fn to_file(&self) -> String {
        self.to.file_name(self.format)
    }

    /// Whether the note goes to another folder (UC-006 BR-005).
    pub fn changes_folder(&self) -> bool {
        self.from.folder() != self.to.folder()
    }

    /// UC-006 BR-003, BR-005: what the link `link`, written in the note at
    /// `note` (its file relative to the workspace, before the rename), has to
    /// name after the rename; `None` when it stays as it is.
    ///
    /// A link to the renamed note leads to its new place, with the heading it
    /// names and with or without the extension as before. In the renamed note
    /// itself, after a move, every relative link is changed so it leads to the
    /// same note or file from the new folder.
    pub fn relink(&self, note: &str, link: &str) -> Option<String> {
        let Resolved {
            path,
            heading,
            resolved,
            extension,
            to_renamed,
        } = self.resolve(note, link)?;
        let note_dir = folder_of(note);
        let in_renamed = note == self.from_file();
        if !to_renamed && !(in_renamed && self.changes_folder()) {
            return None;
        }
        let target = if to_renamed {
            match extension {
                Some(_) => self.to_file(),
                None => self.to.as_str().to_owned(),
            }
        } else {
            resolved.clone()
        };
        let dir_after = if in_renamed {
            folder_of(&self.to_file()).to_owned()
        } else {
            note_dir.to_owned()
        };
        let mut place = relative(Path::new(&dir_after), Path::new(&target));
        if place.is_empty() {
            return None;
        }
        if path.starts_with("./") && !place.starts_with("..") {
            place.insert_str(0, "./");
        }
        if path.contains('%') {
            place = place.replace('%', "%25").replace(' ', "%20");
        }
        if let Some(heading) = heading {
            place.push('#');
            place.push_str(heading);
        }
        (place != link).then_some(place)
    }

    /// Whether the link `link`, written in the note at `note` (before the
    /// rename), leads to the renamed note.
    pub fn links_to(&self, note: &str, link: &str) -> bool {
        self.resolve(note, link).is_some_and(|r| r.to_renamed)
    }

    /// Where a relative link leads, relative to the workspace; `None` for
    /// web addresses, full places, links to headings only, and links that
    /// leave the workspace.
    fn resolve<'l>(&self, note: &str, link: &'l str) -> Option<Resolved<'l>> {
        if link.contains("://") || link.starts_with("mailto:") {
            return None;
        }
        let (path, heading) = match link.split_once('#') {
            Some((path, heading)) => (path, Some(heading)),
            None => (link, None),
        };
        if path.is_empty() || path.starts_with('/') || path.starts_with('\\') {
            return None;
        }
        let decoded = percent_decode(path);
        if Path::new(&decoded).is_absolute() {
            return None;
        }
        let resolved = normalize(&Path::new(folder_of(note)).join(&decoded))?
            .components()
            .map(|c| c.as_os_str().to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join("/");
        let extension = resolved
            .rsplit_once('.')
            .map(|(_, extension)| extension)
            .filter(|extension| !extension.contains('/'));
        let note_format = extension.and_then(NoteFormat::from_extension);
        let stem = match note_format {
            Some(_) => resolved
                .rsplit_once('.')
                .map_or(resolved.as_str(), |(s, _)| s),
            None => resolved.as_str(),
        };
        let to_renamed = stem == self.from.as_str()
            && match note_format {
                Some(format) => format == self.format,
                None => extension.is_none(),
            };
        Some(Resolved {
            path,
            heading,
            extension: note_format,
            to_renamed,
            resolved,
        })
    }

    /// UC-006 BR-003, BR-004: the new title for a link text or a first
    /// heading that shows exactly the old title; `None` for any other text.
    pub fn retitle(&self, text: &str) -> Option<String> {
        let shows = |title: &str| {
            title
                .replace('-', " ")
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
        };
        (shows(text) == self.from.title() && text.trim() != self.title).then(|| self.title.clone())
    }
}

/// A relative link, split and resolved.
struct Resolved<'l> {
    /// The place as written, and the heading it names.
    path: &'l str,
    heading: Option<&'l str>,
    /// The place relative to the workspace, and its note format if it names
    /// the file of a note.
    resolved: String,
    extension: Option<NoteFormat>,
    to_renamed: bool,
}

/// The folder of a file relative to the workspace, empty at the top.
fn folder_of(file: &str) -> &str {
    file.rsplit_once('/').map_or("", |(folder, _)| folder)
}

impl Workspace {
    /// The note format of the note `id`, when its file exists.
    pub fn format_of(&self, id: &NoteId) -> Option<NoteFormat> {
        NoteFormat::ALL
            .into_iter()
            .find(|format| self.root().join(id.file_name(*format)).is_file())
    }

    /// UC-006 step 4: checks the new title and folder of the note `from` as
    /// for a new note (BR-001), where the note itself does not count as its
    /// namesake. `None` when nothing would change (A4).
    pub fn plan_rename(
        &self,
        from: &NoteId,
        rename: &Rename,
    ) -> Result<Option<NoteMove>, NewNoteProblem> {
        let format = self.format_of(from).ok_or_else(|| NewNoteProblem::Io {
            action: "find the note",
            source: io::Error::from(io::ErrorKind::NotFound),
        })?;
        let new = NewNote {
            folder: rename.folder.clone(),
            title: rename.title.clone(),
            format,
        };
        let to = match self.plan_note(&new) {
            Ok(id) => id,
            Err(NewNoteProblem::Exists(existing)) if existing == *from => {
                let mut parts = folder_levels(&rename.folder)?;
                parts.push(page_name(&rename.title));
                NoteId::from_relative(&parts.join("/"))
            }
            Err(problem) => return Err(problem),
        };
        Ok((to != *from).then(|| NoteMove {
            from: from.clone(),
            to,
            format,
            title: heading_title(&rename.title),
        }))
    }

    /// UC-006 step 5: moves the note to its new place with `content`, which
    /// is its content with the changes of the rename. The new file is written
    /// whole before the old one disappears, a file that is there is never
    /// replaced, and on failure nothing is left changed (A5, BR-008).
    pub fn move_note(&mut self, planned: &NoteMove, content: &str) -> Result<(), NewNoteProblem> {
        let old = self.root().join(planned.from_file());
        let new = self.root().join(planned.to_file());
        let folder = new.parent().unwrap_or(self.root()).to_owned();
        let created = topmost_missing(self.root(), &folder);
        let io = |action| move |source| NewNoteProblem::Io { action, source };
        let result = fs::create_dir_all(&folder)
            .map_err(io("create the folder"))
            .and_then(|()| match write_new_atomic(&new, content.as_bytes()) {
                Ok(()) => fs::remove_file(&old).map_err(|source| {
                    let _ = fs::remove_file(&new);
                    NewNoteProblem::Io {
                        action: "remove the old file",
                        source,
                    }
                }),
                // On a file system that ignores case, a change of case only
                // finds the note itself there.
                Err(err)
                    if err.kind() == io::ErrorKind::AlreadyExists
                        && same_file_ignoring_case(&old, &new) =>
                {
                    write_atomic(&old, content.as_bytes())
                        .and_then(|()| fs::rename(&old, &new))
                        .map_err(io("rename the note"))
                }
                Err(err) if err.kind() == io::ErrorKind::AlreadyExists => {
                    Err(NewNoteProblem::Exists(planned.to.clone()))
                }
                Err(source) => Err(NewNoteProblem::Io {
                    action: "write the note",
                    source,
                }),
            });
        if result.is_err() {
            if let Some(created) = created {
                let _ = fs::remove_dir_all(created);
            }
            return result;
        }
        self.rescan();
        // UC-006 BR-006, UC-020 BR-005: todo links follow the note. A todo
        // that cannot be stored keeps the old link, shown as missing.
        let _ = crate::todos::relink_todos(self.root(), &planned.from, &planned.to);
        Ok(())
    }
}

/// Whether `a` and `b` differ only in case and name the same file.
fn same_file_ignoring_case(a: &Path, b: &Path) -> bool {
    a.to_string_lossy().to_lowercase() == b.to_string_lossy().to_lowercase()
        && fs::metadata(a).is_ok()
        && fs::metadata(b).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn planned(from: &str, to: &str, title: &str) -> NoteMove {
        NoteMove {
            from: NoteId::from_relative(from),
            to: NoteId::from_relative(to),
            format: NoteFormat::Markdown,
            title: title.to_owned(),
        }
    }

    /// TC-006-01: links to the renamed note, from anywhere in the workspace.
    #[test]
    fn uc006_tc01_links_to_the_renamed_note() {
        let m = planned("Plan", "Road-Map", "Road Map");
        assert_eq!(m.relink("Ideas.md", "Plan.md"), Some("Road-Map.md".into()));
        assert_eq!(
            m.relink("Week.md", "Plan#goals"),
            Some("Road-Map#goals".into())
        );
        assert_eq!(
            m.relink("team/Review.md", "../Plan.md"),
            Some("../Road-Map.md".into())
        );
        assert_eq!(
            m.relink("Ideas.md", "./Plan.md"),
            Some("./Road-Map.md".into())
        );
        for other in [
            "Planning.md",
            "Plan.adoc",
            "team/Plan.md",
            "#plan",
            "https://x.org/Plan.md",
            "img/Plan.png",
        ] {
            assert_eq!(m.relink("Ideas.md", other), None, "{other}");
        }
        assert_eq!(m.relink("team/Review.md", "Plan.md"), None);
    }

    /// TC-006-02: a moved note keeps leading to the same notes and files.
    #[test]
    fn uc006_tc02_links_of_a_moved_note() {
        let m = planned("Plan", "projects/2027/Plan", "Plan");
        let own = |link| m.relink("Plan.md", link);
        assert_eq!(own("Ideas.md"), Some("../../Ideas.md".into()));
        assert_eq!(own("img/chart.png"), Some("../../img/chart.png".into()));
        assert_eq!(own("#goals"), None);
        assert_eq!(own("Plan.md#goals"), None);
        assert_eq!(own("https://example.org"), None);
        assert_eq!(own("/home/me/talk.mp4"), None);
        assert_eq!(own("projects/2027/Other.md"), Some("Other.md".into()));
        assert_eq!(
            m.relink("Ideas.md", "Plan.md"),
            Some("projects/2027/Plan.md".into())
        );
        assert_eq!(
            m.relink("projects/Index.md", "../Plan.md"),
            Some("2027/Plan.md".into())
        );
        let back = planned("projects/2027/Plan", "Plan", "Plan");
        assert_eq!(
            back.relink("projects/2027/Plan.md", "../../Ideas.md"),
            Some("Ideas.md".into())
        );
        assert_eq!(
            back.relink("projects/2027/Plan.md", "../../../outside.md"),
            None
        );
    }

    /// TC-006-03 / TC-006-04: link texts and headings that show the old title.
    #[test]
    fn uc006_tc03_tc04_texts_that_show_the_old_title() {
        let m = planned("Getting-Started", "First-Steps", "First Steps");
        assert_eq!(m.retitle("Getting Started"), Some("First Steps".into()));
        assert_eq!(m.retitle("Getting-Started"), Some("First Steps".into()));
        assert_eq!(m.retitle("start here"), None);
        assert_eq!(m.retitle("Getting Started now"), None);
    }

    #[test]
    fn uc006_percent_encoded_links() {
        let m = planned("my notes/Plan", "my notes/Road-Map", "Road Map");
        assert_eq!(
            m.relink("Index.md", "my%20notes/Plan.md"),
            Some("my%20notes/Road-Map.md".into())
        );
    }

    struct Sandbox(std::path::PathBuf);

    impl Sandbox {
        fn new(name: &str, files: &[(&str, &str)]) -> Self {
            let root =
                std::env::temp_dir().join(format!("underline-uc006-{name}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&root);
            fs::create_dir_all(&root).unwrap();
            for (path, content) in files {
                let path = root.join(path);
                fs::create_dir_all(path.parent().unwrap()).unwrap();
                fs::write(path, content).unwrap();
            }
            Self(root)
        }

        fn names(&self) -> Vec<String> {
            let mut names: Vec<_> = fs::read_dir(&self.0)
                .unwrap()
                .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
                .collect();
            names.sort();
            names
        }
    }

    impl Drop for Sandbox {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn rename(folder: &str, title: &str) -> Rename {
        Rename {
            folder: folder.into(),
            title: title.into(),
        }
    }

    /// TC-006-01 / TC-006-02: the note moves whole, into new folders too.
    #[test]
    fn uc006_tc01_tc02_moves_the_note() {
        let sandbox = Sandbox::new("move", &[("Plan.md", "# Plan\n")]);
        let mut workspace = Workspace::open(&sandbox.0).unwrap();
        let id = NoteId::from_relative("Plan");
        let planned = workspace
            .plan_rename(&id, &rename("projects/2027", "Road Map"))
            .unwrap()
            .unwrap();
        assert_eq!(planned.to_file(), "projects/2027/Road-Map.md");
        assert_eq!(planned.title, "Road Map");
        workspace.move_note(&planned, "# Road Map\n").unwrap();
        assert_eq!(
            fs::read_to_string(sandbox.0.join("projects/2027/Road-Map.md")).unwrap(),
            "# Road Map\n"
        );
        assert_eq!(sandbox.names(), ["projects"]);
        assert!(workspace.listing().notes.iter().any(|n| n.id == planned.to));
    }

    /// TC-006-05: an AsciiDoc note stays AsciiDoc.
    #[test]
    fn uc006_tc05_format_stays() {
        let sandbox = Sandbox::new("adoc", &[("Setup.adoc", "= Setup\n")]);
        let workspace = Workspace::open(&sandbox.0).unwrap();
        let planned = workspace
            .plan_rename(&NoteId::from_relative("Setup"), &rename("", "Installation"))
            .unwrap()
            .unwrap();
        assert_eq!(planned.to_file(), "Installation.adoc");
    }

    /// TC-006-06: a change of case only is allowed.
    #[test]
    fn uc006_tc06_change_of_case() {
        let sandbox = Sandbox::new("case", &[("road-map.md", "x")]);
        let mut workspace = Workspace::open(&sandbox.0).unwrap();
        let planned = workspace
            .plan_rename(&NoteId::from_relative("road-map"), &rename("", "Road Map"))
            .unwrap()
            .unwrap();
        workspace.move_note(&planned, "x").unwrap();
        assert_eq!(sandbox.names(), ["Road-Map.md"]);
    }

    /// TC-006-07 to TC-006-10: titles, namesakes, and folders that are not
    /// allowed, and nothing to change.
    #[test]
    fn uc006_tc07_to_tc10_checks() {
        let sandbox = Sandbox::new(
            "checks",
            &[
                ("Plan.md", ""),
                ("Ideas.md", ""),
                ("Notes.adoc", ""),
                ("other/underline.yaml", ""),
            ],
        );
        let workspace = Workspace::open(&sandbox.0).unwrap();
        let id = NoteId::from_relative("Plan");
        let plan = |folder, title| workspace.plan_rename(&id, &rename(folder, title));
        assert!(matches!(
            plan("", "Plan: Q1"),
            Err(NewNoteProblem::Title(_))
        ));
        assert!(matches!(plan("", "CON"), Err(NewNoteProblem::Title(_))));
        assert!(matches!(plan("", "ideas"), Err(NewNoteProblem::Exists(_))));
        assert!(matches!(plan("", "Notes"), Err(NewNoteProblem::Exists(_))));
        assert!(matches!(
            plan(".drafts", "Plan"),
            Err(NewNoteProblem::Hidden(_))
        ));
        assert!(matches!(
            plan("other", "Plan"),
            Err(NewNoteProblem::OwnWorkspace(_))
        ));
        assert!(matches!(
            plan("../outside", "Plan"),
            Err(NewNoteProblem::Outside)
        ));
        assert_eq!(plan("", "Plan").unwrap(), None);
        assert_eq!(plan("", " Plan ").unwrap(), None);
        assert!(plan("", "Plans").unwrap().is_some());
    }

    /// TC-006-11: a note that cannot be moved stays where it was, and the
    /// folders made for it are removed.
    #[test]
    fn uc006_tc11_nothing_changed_on_failure() {
        let sandbox = Sandbox::new("fail", &[("Plan.md", "keep"), ("full", "a file")]);
        let mut workspace = Workspace::open(&sandbox.0).unwrap();
        let planned = NoteMove {
            from: NoteId::from_relative("Plan"),
            to: NoteId::from_relative("new/full/x/Plan"),
            format: NoteFormat::Markdown,
            title: "Plan".into(),
        };
        let blocked = NoteMove {
            to: NoteId::from_relative("full/x/Plan"),
            ..planned
        };
        assert!(matches!(
            workspace.move_note(&blocked, "changed"),
            Err(NewNoteProblem::Io { .. })
        ));
        assert_eq!(
            fs::read_to_string(sandbox.0.join("Plan.md")).unwrap(),
            "keep"
        );
        assert_eq!(sandbox.names(), ["Plan.md", "full"]);
        let taken = NoteMove {
            to: NoteId::from_relative("Taken"),
            ..blocked
        };
        fs::write(sandbox.0.join("Taken.md"), "other").unwrap();
        assert!(matches!(
            workspace.move_note(&taken, "changed"),
            Err(NewNoteProblem::Exists(_))
        ));
        assert_eq!(
            fs::read_to_string(sandbox.0.join("Taken.md")).unwrap(),
            "other"
        );
        assert_eq!(
            fs::read_to_string(sandbox.0.join("Plan.md")).unwrap(),
            "keep"
        );
    }

    /// UC-006 BR-006, TC-020-07: todo links follow a moved note.
    #[test]
    fn uc006_br006_todo_links_follow() {
        let root =
            std::env::temp_dir().join(format!("underline-uc006-todo-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("Plan.md"), "plan").unwrap();
        let mut todo = crate::todos::create_todo(&root, "Ship").unwrap();
        todo.link(&NoteId::from_relative("Plan"));
        crate::todos::save_todo(&root, &todo).unwrap();
        let mut workspace = Workspace::open(&root).unwrap();
        let m = planned("Plan", "Work/Project-Plan", "Project Plan");
        workspace.move_note(&m, "plan").unwrap();
        let todos = crate::todos::load_todos(&root);
        assert_eq!(todos.todos[0].links, ["Work/Project-Plan"]);
        fs::remove_dir_all(&root).unwrap();
    }
}

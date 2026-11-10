//! Feature F6 of the migration: putting files and pasted images into a note
//! (UC-005 A9 to A13, A18, A19, M-10.4 to M-10.6) and the links the link
//! dialog makes to a note of the workspace (M-10.3).
//!
//! Ported from `answer_tool`, `drop_items` and `paste_image` of
//! `apps/underline-desktop/src/app.rs`. What goes into the text is the
//! window's editor core's (ADR-025): the session hands it an
//! [`Insertion`]. Copying a file and storing an image touch the disk, so
//! they run on an [`EmbedPlan`] without the session (A-22); the session
//! takes the outcome after ([`Session::finish_embed`]).
//!
//! Checks added here, before `underline_core::embed_file` (shared with the
//! egui app and unchanged): the file and the workspace are compared as
//! canonical paths, so a path through `..` or a symbolic link counts by
//! where it really is (A-68); only regular files and folders are taken, so
//! reading never waits on a pipe or a device; a browser's link file is read
//! only when it is small.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use underline_core::{
    dropped_link, embed_file, relative, store_pasted_image, EmbedError, NoteFormat, NoteId,
    Workspace,
};
use underline_facade::images::encode_png;

use super::editing::not_open as not_open_error;
use super::Session;
use crate::editor_view::{DragName, EmbedAnswer, Insertion};

/// UC-005 A9: the hint while files are dragged over the window.
const DROP_HINT: &str = "Drop to insert at the place under the pointer";
use crate::UiError;

/// UC-005 A10: the status after a copy that got another name.
fn renamed_status(place: &str) -> String {
    format!("Copied as {place}, since the name was taken")
}

/// UC-005 A19: the question about a file larger than 10 MB, as egui words it.
fn large_question(path: &Path, bytes: u64) -> String {
    format!(
        "{} has {} MB. Copy it into the workspace, or only link to where it is (it is then not part of the workspace and not pushed with git)?",
        path.display(),
        bytes / (1024 * 1024)
    )
}

/// A link file of a browser (`.url`, `.webloc`) larger than this is no
/// link file; it is put into the note as a file.
const LINK_FILE_AT_MOST: u64 = 64 * 1024;

/// Review F6: a pipe, a device or a socket is refused before anything reads
/// it.
fn not_regular(path: &Path) -> String {
    format!(
        "{} is not a regular file; only files can be put into a note",
        path.display()
    )
}

/// UC-005 A12: the link a dropped item stands for, reading a browser's link
/// file only when it is a small regular file.
fn link_of(item: &Path) -> Option<(String, Option<String>)> {
    let link_file = item
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("url") || e.eq_ignore_ascii_case("webloc"));
    if link_file {
        let meta = fs::metadata(item).ok()?;
        if !meta.is_file() || meta.len() > LINK_FILE_AT_MOST {
            return None;
        }
    }
    dropped_link(item)
}

/// Whether `path` is a note by its extension, so that copying it changes
/// the overview.
fn is_note(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| NoteFormat::from_extension(e).is_some())
}

/// Where files go for one tab: the workspace and the note's file, as given
/// and as canonical paths, and a copy of the workspace to list its notes
/// anew after a note was copied in. Made under the session, used without it.
#[derive(Debug, Clone)]
pub struct EmbedPlan {
    root: PathBuf,
    note: PathBuf,
    workspace: Workspace,
}

/// What putting files into a note did, for the session to take in.
#[derive(Debug, Default)]
pub struct EmbedDone {
    /// What the window does, one per item.
    pub answers: Vec<EmbedAnswer>,
    /// The status message, if any (the last item's wins, as in egui).
    status: Option<String>,
    /// A note was copied in.
    note_copied: bool,
    /// Then: the workspace with its notes listed anew, outside the session.
    rescanned: Option<Workspace>,
}

impl EmbedPlan {
    /// UC-005 A9 to A11, A19, M-10.4: puts `file` into the note: copied
    /// next to it from outside the workspace, referred to where it is
    /// inside; a large file is asked about unless `allow_large` (the user
    /// chose "Copy"). The answer of the image dialog (`answer_tool` with
    /// `ToolAnswer::File` of egui).
    #[must_use]
    pub fn embed(&self, file: &str, allow_large: bool) -> EmbedDone {
        let mut done = EmbedDone::default();
        let answer = self.embed_one(Path::new(file), allow_large, false, &mut done);
        done.answers.push(answer);
        self.rescan_if_needed(&mut done);
        done
    }

    /// UC-005 A9 to A13, M-10.5: what is dropped goes in at the cursor, the
    /// items one after the other, a space between them: a web address or a
    /// browser's link file as a link, a file as [`EmbedPlan::embed`] puts it
    /// (in a code block only its place, which the editor core decides).
    /// Nothing to drop: "This cannot be dropped into a note" (`drop_items`
    /// of egui).
    #[must_use]
    pub fn drop_items(&self, paths: &[String]) -> EmbedDone {
        let mut done = EmbedDone::default();
        if paths.is_empty() {
            done.status = Some("This cannot be dropped into a note".to_owned());
            return done;
        }
        for (n, path) in paths.iter().enumerate() {
            if n > 0 {
                done.answers.push(EmbedAnswer::Insert {
                    insertion: Insertion::Text { text: " ".into() },
                });
            }
            let path = Path::new(path);
            let answer = match link_of(path) {
                Some((address, label)) => EmbedAnswer::Insert {
                    insertion: Insertion::Link { address, label },
                },
                None => self.embed_one(path, false, true, &mut done),
            };
            done.answers.push(answer);
        }
        self.rescan_if_needed(&mut done);
        done
    }

    /// A note was copied in: its notes are listed anew here, outside the
    /// session; the session takes the listing over (`finish_embed`).
    fn rescan_if_needed(&self, done: &mut EmbedDone) {
        if done.note_copied {
            let mut workspace = self.workspace.clone();
            workspace.rescan();
            done.rescanned = Some(workspace);
        }
    }

    fn embed_one(
        &self,
        file: &Path,
        allow_large: bool,
        dropped: bool,
        done: &mut EmbedDone,
    ) -> EmbedAnswer {
        let status = &mut done.status;
        // Compared where it really is: a path through `..` or a symbolic
        // link into or out of the workspace counts by its target (A-68).
        // A path that does not resolve is left to `embed_file`, which says
        // it cannot be read.
        let (file, root, note) = match fs::canonicalize(file) {
            Ok(real) => {
                match fs::metadata(&real) {
                    Ok(meta) if !meta.is_file() && !meta.is_dir() => {
                        *status = Some(not_regular(file));
                        return EmbedAnswer::Refused;
                    }
                    _ => {}
                }
                let root = fs::canonicalize(&self.root).unwrap_or_else(|_| self.root.clone());
                let note = self
                    .note
                    .parent()
                    .and_then(|dir| fs::canonicalize(dir).ok())
                    .zip(self.note.file_name())
                    .map_or_else(|| self.note.clone(), |(dir, name)| dir.join(name));
                (real, root, note)
            }
            Err(_) => (file.to_owned(), self.root.clone(), self.note.clone()),
        };
        let copied = !file.starts_with(&root);
        match embed_file(&root, &note, &file, allow_large) {
            Ok(embedded) => {
                done.note_copied |= copied && is_note(&file);
                if embedded.renamed {
                    *status = Some(renamed_status(&embedded.place));
                }
                let (place, image) = (embedded.place, embedded.image);
                let insertion = if dropped {
                    Insertion::Dropped { place, image }
                } else {
                    Insertion::File { place, image }
                };
                EmbedAnswer::Insert { insertion }
            }
            Err(EmbedError::Large { path, bytes }) => EmbedAnswer::Large {
                question: large_question(&path, bytes),
                path: path.display().to_string(),
            },
            Err(err) => {
                *status = Some(err.to_string());
                EmbedAnswer::Refused
            }
        }
    }

    /// UC-005 A18, BR-014, M-10.6: stores a pasted image (RGBA, not
    /// premultiplied) as a PNG next to the note, named after the note and
    /// the time `now`; the name, the place relative to the note, or why
    /// not.
    ///
    /// # Errors
    ///
    /// The image could not be encoded or written.
    pub fn store_image(
        &self,
        width: u32,
        height: u32,
        rgba: &[u8],
        now: SystemTime,
    ) -> Result<String, String> {
        // `encode_png` asserts the length; the clipboard always matches it.
        let expected = u64::from(width) * u64::from(height) * 4;
        if u64::try_from(rgba.len()).ok() != Some(expected) {
            return Err(format!("{} bytes for a {width}x{height} image", rgba.len()));
        }
        let png = encode_png(width, height, rgba).map_err(|e| e.to_string())?;
        store_pasted_image(&self.note, &png, now).map_err(|e| e.to_string())
    }
}

impl Session {
    /// Where files go for the tab of `note` (F6).
    ///
    /// # Errors
    ///
    /// The note has no open tab.
    pub fn embed_plan(&self, note: &NoteId) -> Result<EmbedPlan, UiError> {
        let open = self.open.as_ref().ok_or_else(|| not_open_error(note))?;
        let index = open.tabs.find(note).ok_or_else(|| not_open_error(note))?;
        Ok(EmbedPlan {
            root: open.workspace.root().to_owned(),
            note: open.tabs.get(index).path.clone(),
            workspace: open.workspace.clone(),
        })
    }

    /// Takes in what [`EmbedPlan::embed`] or [`EmbedPlan::drop_items`] did:
    /// its status, and, when a note was copied in, the notes listed anew
    /// outside the session (`answer_tool` of egui rescans the workspace
    /// after every answer; here only a copied note changes the overview).
    pub fn finish_embed(&mut self, done: EmbedDone) -> Vec<EmbedAnswer> {
        if let Some(status) = done.status {
            self.status = status;
        }
        if let (Some(rescanned), Some(open)) = (done.rescanned, &mut self.open) {
            // Only when the workspace is still the one of the plan.
            if rescanned.root() == open.workspace.root() {
                open.workspace = rescanned;
                self.refresh_overview();
                self.views_dirty = true;
            }
        }
        done.answers
    }

    /// UC-005 A18, M-10.6: takes in a stored image: "Pasted image stored as
    /// {name}", or "The pasted image could not be stored: {err}"; the name
    /// to insert, if stored.
    pub fn finish_pasted_image(&mut self, stored: Result<String, String>) -> Option<String> {
        match stored {
            Ok(name) => {
                self.status = format!("Pasted image stored as {name}");
                Some(name)
            }
            Err(err) => {
                self.status = format!("The pasted image could not be stored: {err}");
                None
            }
        }
    }

    /// M-10.3, UC-005 A2: the link to the note `target` from the tab of
    /// `note`: its place relative to the note's folder, labelled with its
    /// title (`ToolAnswer::Note` of egui).
    ///
    /// # Errors
    ///
    /// The note has no open tab, or `target` is no note of the workspace.
    pub fn note_link(&self, note: &NoteId, target: &NoteId) -> Result<Insertion, UiError> {
        let plan = self.embed_plan(note)?;
        let open = self.open.as_ref().ok_or_else(|| not_open_error(note))?;
        let target_path = open
            .workspace
            .note_path(target)
            .map_err(|err| UiError::new(crate::UiErrorKind::Invalid, err.to_string()))?;
        let dir = plan.note.parent().unwrap_or(&plan.root).to_owned();
        let dir = std::path::absolute(&dir).unwrap_or(dir);
        let target_path = std::path::absolute(&target_path).unwrap_or(target_path);
        Ok(Insertion::Link {
            address: relative(&dir, &target_path),
            label: Some(target.title()),
        })
    }

    /// UC-005 A9, M-10.5: files dragged over the window. While they are
    /// over it: "Drop to insert at the place under the pointer" (egui); when
    /// they leave, that hint goes (review F6, A-65); dropped while no note
    /// is shown: "Open a note to drop files into it" (A-69; egui says
    /// nothing).
    pub fn drag(&mut self, state: DragName) {
        match state {
            DragName::Over => self.status = DROP_HINT.to_owned(),
            DragName::Left => {
                if self.status == DROP_HINT {
                    self.status.clear();
                }
            }
            DragName::NoNote => self.status = "Open a note to drop files into it".to_owned(),
        }
    }
}

#[cfg(test)]
mod tests;

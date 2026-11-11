//! UC-020 BR-006, A7, A9 (review F15): writing the todo files without the
//! session, never over a change made elsewhere.
//!
//! Every write of the window goes through [`Disk`], one after the other.
//! Before a todo's file is written over, it is read again and compared, by
//! its content, with the version the change was made from (its base) and
//! with the version this window wrote last; when it is neither (changed or
//! deleted by an agent, a sync or another program), or cannot be read, the
//! file stays as it is and the change is kept as not stored with the
//! reason. An older change of a todo never replaces a newer one.
//!
//! The comparison is of the todos as parsed: every key counts, also keys a
//! later version or an agent added (kept in `Todo::other`); only layout,
//! key order and comments are not seen. Between reading the file again and
//! writing it, another program can still write it (a few milliseconds,
//! A-188); within Underline the writes go one after the other.

use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use underline_core::{
    create_todo, delete_todo, save_todo, todos_folder, write_new_atomic, Todo, TodoError,
};

/// What [`Disk::save`] did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Saved {
    /// The todo was written.
    Written,
    /// A newer change of the todo was written already; this one is left.
    Skipped,
    /// The file changed elsewhere, went, or cannot be read; it was not
    /// written over, for this reason.
    Refused(String),
}

/// What the file of a todo holds now.
#[derive(Debug)]
enum OnDisk {
    Missing,
    Unreadable(String),
    Todo(Box<Todo>),
}

#[derive(Debug, Default)]
struct Written {
    /// By todo, the number of the newest change written.
    seqs: HashMap<u64, u64>,
    /// By todo, the version this window wrote last.
    ours: HashMap<u64, Todo>,
}

/// The todo files of one workspace, written one after the other.
#[derive(Clone, Default)]
pub(super) struct Disk(Arc<Mutex<Written>>);

impl std::fmt::Debug for Disk {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Disk")
    }
}

fn todo_path(root: &Path, number: u64) -> PathBuf {
    todos_folder(root).join(format!("{number}.yaml"))
}

/// The file of todo `number`, read again.
fn on_disk(root: &Path, number: u64) -> OnDisk {
    match fs::read_to_string(todo_path(root, number)) {
        Err(err) if err.kind() == io::ErrorKind::NotFound => OnDisk::Missing,
        Err(err) => OnDisk::Unreadable(err.to_string()),
        Ok(text) => match serde_norway::from_str::<Todo>(&text) {
            Ok(mut todo) => {
                todo.number = number;
                OnDisk::Todo(Box::new(todo))
            }
            Err(err) => OnDisk::Unreadable(err.to_string()),
        },
    }
}

/// Review F15 (new text, Q-117): the todo changed elsewhere since it was
/// read.
pub(super) fn changed_elsewhere(number: u64) -> String {
    format!(
        "todo #{number} was changed elsewhere since it was read; yours is kept here, change it again to store it"
    )
}

/// Review F15 (new text, Q-117): the todo was deleted elsewhere.
pub(super) fn deleted_elsewhere(number: u64) -> String {
    format!("todo #{number} was deleted elsewhere; yours is kept here, change it again to store it")
}

impl Disk {
    fn lock(&self) -> MutexGuard<'_, Written> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Stores `todo` as change `seq`, made from `base` (the version read,
    /// `None` when it had no file): not when a newer change was stored, nor
    /// over a file that holds neither `base` nor what this window wrote.
    pub(super) fn save(
        &self,
        root: &Path,
        todo: &Todo,
        base: Option<&Todo>,
        seq: u64,
    ) -> Result<Saved, TodoError> {
        self.save_over(root, todo, base, seq, false)
    }

    /// [`Disk::save`], but with `strict` only over a file that holds `base`
    /// itself, not what this window wrote since (the undo of a mark, review
    /// F15).
    pub(super) fn save_over(
        &self,
        root: &Path,
        todo: &Todo,
        base: Option<&Todo>,
        seq: u64,
        strict: bool,
    ) -> Result<Saved, TodoError> {
        let mut written = self.lock();
        let number = todo.number;
        if written
            .seqs
            .get(&number)
            .is_some_and(|newest| *newest > seq)
        {
            return Ok(Saved::Skipped);
        }
        let ours = written.ours.get(&number);
        let allowed = match on_disk(root, number) {
            OnDisk::Todo(now) => base == Some(&*now) || (!strict && ours == Some(&*now)),
            OnDisk::Missing if base.is_none() => true,
            OnDisk::Missing => return Ok(Saved::Refused(deleted_elsewhere(number))),
            OnDisk::Unreadable(reason) => {
                return Ok(Saved::Refused(format!(
                    "todo #{number} cannot be read: {reason}"
                )));
            }
        };
        if !allowed {
            return Ok(Saved::Refused(changed_elsewhere(number)));
        }
        save_todo(root, todo)?;
        written.seqs.insert(number, seq);
        written.ours.insert(number, todo.clone());
        Ok(Saved::Written)
    }

    /// UC-020 steps 3–4: a new todo with the next free number.
    pub(super) fn create(&self, root: &Path, title: &str, seq: u64) -> Result<Todo, TodoError> {
        let mut written = self.lock();
        let todo = create_todo(root, title)?;
        written.seqs.insert(todo.number, seq);
        written.ours.insert(todo.number, todo.clone());
        Ok(todo)
    }

    /// A5: deletes todo `number` as change `seq`; no older change of it
    /// is written after it (review F15).
    pub(super) fn delete(&self, root: &Path, number: u64, seq: u64) -> Result<Todo, TodoError> {
        let mut written = self.lock();
        let todo = delete_todo(root, number)?;
        written.seqs.insert(number, seq);
        written.ours.remove(&number);
        Ok(todo)
    }

    /// Review F15: nothing older than change `seq` is written to todo
    /// `number` any more (a todo kept only here was deleted, or the
    /// changes of a workspace left were given up).
    pub(super) fn forget(&self, number: u64, seq: u64) {
        let mut written = self.lock();
        let newest = written.seqs.entry(number).or_insert(seq);
        *newest = (*newest).max(seq);
        written.ours.remove(&number);
    }

    /// A5 step 2: the deleted todo comes back, but never over a file of the
    /// same number made since.
    pub(super) fn restore(&self, root: &Path, todo: &Todo, seq: u64) -> Result<Saved, TodoError> {
        let mut written = self.lock();
        let text = serde_norway::to_string(todo).map_err(|e| TodoError::Write(e.to_string()))?;
        fs::create_dir_all(todos_folder(root)).map_err(|e| TodoError::Write(e.to_string()))?;
        match write_new_atomic(&todo_path(root, todo.number), text.as_bytes()) {
            Ok(()) => {
                written.seqs.insert(todo.number, seq);
                written.ours.insert(todo.number, todo.clone());
                Ok(Saved::Written)
            }
            Err(err) if err.kind() == io::ErrorKind::AlreadyExists => Ok(Saved::Refused(format!(
                "there is a todo #{} again",
                todo.number
            ))),
            Err(err) => Err(TodoError::Write(err.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "underline-session-todo-disk-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn uc020_br006_a_file_changed_elsewhere_is_not_written_over() {
        let root = root("changed");
        let disk = Disk::default();
        let base = disk.create(&root, "Base", 1).unwrap();
        // Another program writes over it; the window's change was made from the base.
        let mut other = base.clone();
        other.title = "Theirs".into();
        save_todo(&root, &other).unwrap();
        let mut mine = base.clone();
        mine.title = "Mine".into();

        let saved = disk.save(&root, &mine, Some(&base), 2).unwrap();

        assert_eq!(saved, Saved::Refused(changed_elsewhere(1)));
        assert!(matches!(on_disk(&root, 1), OnDisk::Todo(t) if t.title == "Theirs"));
    }

    #[test]
    fn uc020_a5_a_restore_never_replaces_a_new_file() {
        let root = root("restore");
        let disk = Disk::default();
        let todo = disk.create(&root, "One", 1).unwrap();
        let deleted = disk.delete(&root, 1, 2).unwrap();
        save_todo(&root, &todo).unwrap();

        let saved = disk.restore(&root, &deleted, 3).unwrap();

        assert_eq!(saved, Saved::Refused("there is a todo #1 again".into()));
    }

    #[test]
    fn review_f15_a_strict_save_refuses_what_this_window_wrote_since() {
        let root = root("strict");
        let disk = Disk::default();
        let marked = disk.create(&root, "Marked", 1).unwrap();
        let mut later = marked.clone();
        later.title = "Changed later".into();
        assert_eq!(
            disk.save(&root, &later, Some(&marked), 2).unwrap(),
            Saved::Written
        );
        let mut before = marked.clone();
        before.title = "Before".into();

        let saved = disk
            .save_over(&root, &before, Some(&marked), 3, true)
            .unwrap();

        assert_eq!(saved, Saved::Refused(changed_elsewhere(1)));
        assert!(matches!(on_disk(&root, 1), OnDisk::Todo(t) if t.title == "Changed later"));
    }

    #[test]
    fn review_f15_nothing_older_is_written_after_a_todo_was_forgotten() {
        let root = root("forget");
        let disk = Disk::default();
        let todo = disk.create(&root, "Gone", 1).unwrap();
        disk.forget(1, 5);

        let saved = disk.save(&root, &todo, Some(&todo), 4).unwrap();

        assert_eq!(saved, Saved::Skipped);
    }

    #[test]
    fn uc020_br002_create_never_writes_over_a_file() {
        let root = root("create");
        let disk = Disk::default();
        disk.create(&root, "One", 1).unwrap();
        // Another computer's todo #2 arrived after the window read.
        fs::write(todo_path(&root, 2), "title: Theirs\n").unwrap();

        let made = disk.create(&root, "Mine", 2).unwrap();

        assert_eq!(made.number, 3);
        assert_eq!(
            fs::read_to_string(todo_path(&root, 2)).unwrap(),
            "title: Theirs\n"
        );
    }

    #[test]
    fn review_f15_an_extra_key_of_an_agent_counts_as_a_change() {
        let root = root("extra-key");
        let disk = Disk::default();
        let base = disk.create(&root, "Base", 1).unwrap();
        let text = fs::read_to_string(todo_path(&root, 1)).unwrap();
        fs::write(todo_path(&root, 1), format!("{text}estimate: 3\n")).unwrap();
        let mut mine = base.clone();
        mine.title = "Mine".into();

        let saved = disk.save(&root, &mine, Some(&base), 2).unwrap();

        assert_eq!(saved, Saved::Refused(changed_elsewhere(1)));
    }
}

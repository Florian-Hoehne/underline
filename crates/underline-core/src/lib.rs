//! Core domain of Underline: notes and the notebook that stores them.
//!
//! A notebook is a plain directory of Markdown files, which is exactly the
//! layout of a GitHub or GitLab wiki repository. That keeps the local store,
//! wiki sync and MCP access working on the same files.

mod note;
mod notebook;

pub use note::{Note, NoteId};
pub use notebook::{Notebook, NotebookError};

//! Core domain of Underline: workspaces and the notes they hold.
//!
//! A workspace is a plain directory of Markdown and AsciiDoc files, which is
//! also the layout of a GitHub or GitLab wiki repository. That keeps the local
//! store, git sync, and MCP access working on the same files.

mod atomic;
mod create;
mod format;
mod git;
mod note;
mod settings;
mod workspace;

pub use atomic::write_atomic;
pub use create::{
    check_directory, create_workspace, CreateError, CreateProblem, DirectoryProblem,
    GitRepositoryOption, NewWorkspace, Surroundings, FALLBACK_BRANCH, FIRST_COMMIT_MESSAGE,
    HOME_TITLE, REMOTE_NAME,
};
pub use format::NoteFormat;
pub use git::{Git, GitError};
pub use note::{Note, NoteId};
pub use settings::{user_settings_dir, SettingsError, WorkspaceSettings, SETTINGS_FILE_NAME};
pub use workspace::{Workspace, WorkspaceError};

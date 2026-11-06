//! Core domain of Underline: workspaces and the notes they hold.
//!
//! A workspace is a plain directory of Markdown and AsciiDoc files, which is
//! also the layout of a GitHub or GitLab wiki repository. That keeps the local
//! store, git sync, and MCP access working on the same files.

mod atomic;
mod create;
mod format;
mod git;
mod new_note;
mod note;
mod open;
mod settings;
mod state;
mod workspace;

pub use atomic::{write_atomic, write_new_atomic};
pub use create::{
    check_directory, create_workspace, CreateError, CreateProblem, DirectoryProblem,
    GitRepositoryOption, NewWorkspace, Surroundings, FALLBACK_BRANCH, FIRST_COMMIT_MESSAGE,
    HOME_TITLE, REMOTE_NAME,
};
pub use format::NoteFormat;
pub use git::{Git, GitError};
pub use new_note::{
    check_name, heading_title, page_name, NameProblem, NewNote, NewNoteProblem,
    FORBIDDEN_CHARACTERS, MAX_PATH_CHARS, MAX_TITLE_CHARS,
};
pub use note::{Note, NoteId};
pub use open::{check_open_directory, check_readable_directory, OpenProblem};
pub use settings::{user_settings_dir, SettingsError, WorkspaceSettings, SETTINGS_FILE_NAME};
pub use state::{
    now, AppState, EditingView, NoteState, StateError, StateNotice, StateStore, TabState,
    Timestamp, WorkspaceState, STATE_FILE_NAME, STATE_WRITE_DELAY,
};
pub use workspace::{
    is_workspace_directory, note_overview, Listing, NoteEntry, OverviewEntry, Workspace,
    WorkspaceError,
};

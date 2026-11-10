//! Core domain of Underline: workspaces and the notes they hold.
//!
//! A workspace is a plain directory of Markdown and AsciiDoc files, which is
//! also the layout of a GitHub or GitLab wiki repository. That keeps the local
//! store, git sync, and MCP access working on the same files.

mod agent_write;
mod agents;
mod atomic;
mod autosave;
mod branches;
mod changes;
mod conflict;
mod create;
mod deleted;
mod embed;
mod format;
mod git;
mod links;
mod lookup;
mod new_note;
mod note;
mod open;
mod properties;
mod rename;
mod saver;
mod search;
mod settings;
mod state;
mod sync;
mod tabs;
mod todos;
mod watcher;
mod workspace;

pub use agent_write::{
    answer_for_tab, answer_request, apply_splice, apply_to_file, create_for_agent, merge_splice,
    plan_for_tab, plan_splice, remove_window_presence, renew_window_presence, send_request,
    take_answer, take_requests, window_runs, withdraw_request, AgentAnswer, AgentChange,
    AgentRequest, Merged, Splice, WriteProblem,
};
pub use agents::{
    agent_access_now, connected_agents, presence_path, remove_presence, stored_agent_access,
    write_presence, Presence, Reading, AGENTS_FOLDER, PRESENCE_RENEW, PRESENCE_STALE,
    READING_SHOWN, UNNAMED_AGENT,
};
pub use atomic::{write_atomic, write_new_atomic};
pub use autosave::{Autosave, AUTOSAVE_LATEST, AUTOSAVE_QUIET, AUTOSAVE_RETRY};
pub use branches::{branch_name, BranchNameProblem, Branches, SwitchError};
pub use changes::{
    changed_lines, diff, proposed_message, restore_committed, BranchState, ChangeKind,
    ChangeTracker, Changes, DiffLine, DiscardError, FileChange, Hunk, LineKind, OnlyDiffers,
};
pub use conflict::{
    conflict_places, copy_name, resolve_place, Choice, CompleteError, ConflictPlace, Joining, Side,
};
pub use create::{
    check_directory, create_workspace, CreateError, CreateProblem, DirectoryProblem,
    GitRepositoryOption, NewWorkspace, Surroundings, FALLBACK_BRANCH, FIRST_COMMIT_MESSAGE,
    HOME_TITLE, REMOTE_NAME,
};
pub use deleted::{DeleteProblem, DeletedNote, DELETED_FOLDER, KEEP_DELETED_MS};
pub use embed::{
    dropped_link, embed_file, is_image, relative, store_pasted_image, EmbedError, Embedded,
    IMAGE_EXTENSIONS, LARGE_FILE,
};
pub use format::NoteFormat;
pub use git::{Git, GitError};
pub use links::{asciidoc_anchor, heading_anchor, resolve_link, LinkTarget};
pub use lookup::{
    find_note, list_for_agent, read_for_agent, search_for_agent, AgentFound, AgentSearch,
    ListedNote, LookupError, NoteText, AGENT_FOUND_LINES, AGENT_FOUND_NOTES, CLOSEST_NOTES,
};
pub use new_note::{
    check_name, heading_title, page_name, NameProblem, NewNote, NewNoteProblem,
    FORBIDDEN_CHARACTERS, MAX_PATH_CHARS, MAX_TITLE_CHARS,
};
pub use note::{Note, NoteId};
pub use open::{check_open_directory, check_readable_directory, OpenProblem};
pub use properties::{format_local_time, format_size, NoteProperties};
pub use rename::{NoteMove, Rename};
pub use saver::{NoteSaver, NoteWriter, OwnWrites, SaveJob, SaveResult};
pub use search::{
    found_lines, lower, search_notes, FoundLine, FoundNote, Query, SearchRequest, SearchResult,
    Searcher, Texts, LINE_CONTEXT, SHOWN_PLACES,
};
pub use settings::{
    set_key, store_setting, user_settings_dir, user_settings_path, AgentAccess, Effective,
    InputMode, LineSide, SettingKey, Settings, SettingsError, Theme, WorkspaceSettings,
    LEGACY_MCP_KEY, SETTINGS_FILE_NAME,
};
pub use state::{
    now, AppState, EditingView, NoteState, StateError, StateNotice, StateStore, TabState,
    Timestamp, WorkspaceState, STATE_FILE_NAME, STATE_WRITE_DELAY,
};
pub use sync::{Distance, SyncMode, SyncOutcome, SyncRequest, Syncer, AUTO_PREFIX};
pub use tabs::{Opened, TabList, MAX_TABS};
pub use todos::{
    create_todo, delete_todo, links_summary, load_todos, overdue_count, overdue_days, relink_todos,
    restore_todo, save_todo, todo_list, todo_number_of, todo_relative_path, todos_fingerprint,
    todos_folder, Day, Todo, TodoConflict, TodoError, TodoFilter, TodoListState, TodoSort,
    TodoStatus, Todos, Urgency, MAX_TODO_TITLE, TODOS_FOLDER,
};
pub use watcher::{
    outside_copy_path, WorkspaceEvent, WorkspaceWatcher, BULK_CHANGES, WATCH_COLLECT,
};
pub use workspace::{
    is_workspace_directory, note_overview, Listing, NoteEntry, OverviewEntry, Workspace,
    WorkspaceError,
};

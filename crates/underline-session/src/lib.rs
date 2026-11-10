//! Underline's workspace session, without a user interface.
//!
//! The egui app keeps its orchestration in its window code: the open
//! workspace, the tabs with their documents, the save pipeline, changes from
//! outside, agent requests (ADR-018), the git flows, the sync rules, the todos
//! poll, the settings state machine, exporting, following links and the status
//! texts. This crate is where that logic is ported to, feature by feature, so
//! that the Tauri app (`apps/underline-tauri`) is only a thin layer of commands
//! and events over it and the logic is unit-tested without a window.
//!
//! The egui frame loop becomes a backend tick (ADR-022): a thread calls
//! [`Session::tick`] every 100 ms, the tick keeps today's intervals (agent
//! requests 100 ms, settings 500 ms, presence and todos 1 s, change tracker
//! 3 s, sync interval, autosave deadlines) and returns the [`Event`]s the
//! window needs. The wake callbacks of the background services trigger the
//! tick at once instead of repainting a frame.
//!
//! Ported so far (feature F1, the shell): the error every command returns
//! ([`UiError`]), what the window learns on start ([`StartInfo`], M-01), the
//! settings state machine ([`SettingsState`], UC-010), the theme palettes
//! ([`PaletteView`], M-01.6), the app state (M-01.3, M-22.1) and the status
//! message (M-04.7). Feature F2 adds the workspace: the Open and Create
//! dialogs (UC-001, UC-002, M-21), the note overview (M-05), the tabs
//! (M-06) and what is remembered of them (M-22.2). Feature F3 adds the text
//! of the open notes (ADR-024): editing in the source view (UC-005, M-08,
//! M-10.1), the autosave (M-10.8), storing before a close (M-06.3, M-21.4),
//! the changes from outside (M-07.5) and the properties (M-16.2). Feature
//! F4 adds the Nano keys (M-11.3), the status of the input modes for the
//! status line (M-11.4), the formatting toolbar (M-10.2) and folding
//! (M-08.8); the window's Vim keys send their edits as changes (A-01).
//! Feature F5a adds the facade's blocks (M-09), its diagrams as SVG
//! (ADR-009) and what the app's own protocol `ul-asset` serves (ADR-023,
//! [`assets`]). Since ADR-025 (variant B) the window's editor core holds the
//! text and answers every input; the session keeps the mirror it syncs with
//! (`ed_sync`), stores it, and sends what only it can make: the colours of
//! the source view (`ed-highlight`) and the facade's colours and figures
//! (`ed-enrich`).
//!
//! The data types crossing to the window derive `specta::Type` behind the
//! `specta` feature, so TypeScript bindings can be generated from them.

pub mod assets;
mod clip;
mod editor_view;
mod error;
mod event;
mod facade_view;
mod palette;
mod session;
mod settings;
mod start;
mod sync_view;
mod workspace_view;

pub use clip::clipboard_view;
pub use editor_view::{
    ClipAction, ClipContentView, Clipped, CloseQuestion, CopyOnSelectView, DragName,
    EditorCommandName, EditorInput, EditorPatch, EditorState, EmbedAnswer, FoundView,
    HighlightView, Insertion, KeyHintView, KeyName, KeysStatusView, NotePropertiesView,
    SectionView, SectionsView, SelectionCopy, SpanStyle, TabPropertiesView, TabViews, TextChange,
    ToolName, ToolView, VimChange, VimModeName, VimState,
};
pub use error::{UiError, UiErrorKind};
pub use event::{Event, StatusView};
pub use facade_view::{
    BlockKindView, BlockView, BlocksView, CellAlignName, CodeColors, FacadeAt, FacadeInput,
    FacadePatch, FacadeState, FigureView, GapView, RowsView, RunView, TableView,
};
pub use palette::PaletteView;
pub use session::{
    CreatePlan, EmbedDone, EmbedPlan, KeepCopy, OpenPlan, PreparedCreate, PreparedOpen, Session,
    Synced,
};
pub use settings::{
    agent_access_refused_message, theme_names, Scope, SettingName, SettingsState, SettingsView,
    USER_SETTINGS_POLL,
};
pub use start::{Platform, StartInfo};
pub use sync_view::{
    ChangedView, EnrichView, FlushView, MirrorView, RemoteApplied, RemoteChange, RemoteOp,
    SyncAnswer, SyncBatch, SyncEdit,
};
pub use workspace_view::{
    CreateAnswer, EditingViewName, FormatName, NewWorkspaceRequest, OpenAnswer, OpenOffer,
    OverviewItem, StartOpened, TabView, TabsView, TodoTabView, WorkspaceSnapshot, WorkspaceView,
};

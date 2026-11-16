// A stand-in for the Rust backend, so the window runs in a plain browser
// (Playwright against http://localhost:1420, plan section 10). Each handler
// returns what the command returns on success; the types come from the
// generated bindings, so a changed command breaks the mock at compile time.
// Like the backend, the mock sends `status`, `settings`, `workspace`,
// `overview` and `tabs` events after a command changed them.
//
// The workspaces are a small fake file system (`FAKE_FILES`), so the Open
// and Create dialogs, the note overview and the tabs can be tried in a
// browser. Its rules are rough stand-ins for underline-core; the real rules
// are tested in Rust. The text of an open note is the mirror's: the autosave
// marks and clears the tab a second after a change, and notes listed in
// `MockStart.failingSaves` cannot be stored, to try the failure paths.
//
// ADR-025: the source view's text lives in the window's editor core; the
// mock keeps the backend's mirror of each note with the real one
// (`MirrorCore`, the Rust `Server` in the WASM), so `ed_sync` answers with
// the same revisions, transforms and hashes. `window.__ulMock` lets a test
// change a note as the backend would: `backendEdit` (as an agent) and
// `outsideChange` (as another program).
//
// Feature F6: the system clipboard is a fake one with text, HTML and an
// image (`setClipboard`, `clipboardContent`); its HTML becomes markup by a
// rough stand-in for `html_to_markup`. Files to put into a note come from a
// small fake world outside the workspace (`FAKE_OUTSIDE`), and
// `dragEnter`/`drop` deliver dragged files as Tauri would.
//
// Package 2 (go to note and the command palette): `/home/user/Plans` holds
// the notes of TC-004-25 to TC-004-27 (`Plan`, `Release-Plan`,
// `travel/Plane-Tickets`, `travel/Packing`, `Explanation`, `Ideas`,
// `Über-uns`), and `/home/user/Large` 10,000 notes in 13 folders for
// NFR-018 (TC-004-28, TC-023-10). Both overlays rank in the editor's WASM,
// so the mock answers nothing new for them.
//
// Feature F8: notes are created, renamed (the links to them in the other
// notes follow, roughly), deleted with the undo of the status bar, and
// restored or removed from the deleted notes; `/home/user/Notes` starts
// with three deleted notes, one of which ("Ideas") finds its place taken.
// `cannotKeep(true)` makes the deleted notes unable to keep a note (UC-007
// A8); a note in the folder `locked` cannot be written (UC-003 A5).
//
// Feature F7: following a link (`ed_follow_link`) finds it at the place
// clicked in the mirror's text and resolves it roughly as `resolve_link`
// does, over the fake notes; what the system would open is
// only recorded (`openedWithSystem`). The tool panel's outline, code and
// figures come as `indexes` events, from a rough stand-in for the facade's
// blocks. The note `Link-Tour` of `/home/user/Work` holds a link of every
// kind.
//
// Package 3 (FR-078 to FR-080, UC-004 A14, UC-005 A23, A24): the backlinks
// and outgoing links of the active note come as the event `links`, made by
// a rough stand-in for the backend's link index (`mockLinks`: Markdown
// links and `<address>` outside code, resolved as `followLink` does, over
// the fake notes and the open texts); `links_open` opens a linking note at
// its link, `links_headings` gives the headings link completion lists.
// `/home/user/Links` holds the notes of TC-004-29 to TC-004-31 and
// TC-005-33 (`Guide` is an AsciiDoc note, for its step 5) (`Project-Plan` with "Goals", `Brainstorm` linking to it on
// lines 4 and 9, `travel/Itinerary` on line 2, `Snippets` only in code,
// `Hub` with a link of every kind and two broken ones, `travel/Checklist`
// with "Clothes" and "Papers"); `img/logo.png` is a file there. Completing
// a link is the window's editor core's, so the mock answers nothing more.
// Package 12 (OQ-244, OQ-199, UC-004 A22): pictures (`![…](…)`, `image::`)
// and included files (`include::`) count as links, marked as such
// (`Itinerary` shows a missing map, `Guide` includes `Project-Plan`); the
// unlinked mentions of the active note's title come with the lists
// (`mockMentions`; `Diary` mentions "project plan" twice), and
// `links_link_mention` links one as the backend does.
//
// Feature F9: the search of the overlay (`search_set`) runs over the fake
// notes and the open texts with a rough stand-in for `search_notes` (words,
// quotes, case, title and folder first), answers as the event `search`
// with the first 100 found notes, gives more with `search_page`, and
// `search_open` opens a found note at its word. It searches again when a
// text or the overview changes (UC-008 A3). `Trip` and `Log` of
// `/home/user/Notes` hold "ferry" and "deploy" in many places.
//
// Package 1 (FR-073 to FR-075, UC-022): the search switches of the overlay
// are matched roughly with JavaScript's regular expressions; the replace of
// the workspace (`replace_set`, `replace_page`, `replace_leave_out`,
// `replace_confirm`, `replace_undo`, `replace_close`) finds its places with
// the same rough rules, answers as the event `replace`, changes an open
// note through its mirror (one undo step) and a closed one in the fake
// files, refuses notes listed in `failingSaves` (A5), and offers the undo
// in the status. Replacing in the find field of a note is the real one, in
// the window's editor core. The rules themselves are tested in Rust.
//
// Package 6b (FR-094, FR-095, UC-003 A7, A8, UC-025): `/home/user/Templates`
// holds the templates of TC-003-17 (`templates/Meeting`, `templates/Spec` in
// AsciiDoc) and TC-025-01 (`templates/Daily` in AsciiDoc) and the daily note
// `daily/2026-11-05` (TC-025-04). New notes are made from a template with
// its placeholders filled in (a rough stand-in for `fill_template`), and the
// daily note is opened or created in the daily note folder the workspace
// settings name (the settings window has the template folder, the daily
// note folder and the daily note template). The clock is `MockStart.now` or
// the hook `setNow` (`2026-11-11T08:15`), else the browser's.
//
// Feature F10: the settings are a user file and one file per workspace
// (`mock-settings.ts`); the settings window shows and changes them
// (`settings_window`, `settings_set`), quick switches go where the setting
// comes from (BR-003), and the hooks change a file from outside (A5), break
// it (A6) or make the files unwritable (A7). `/home/user/Work` shares its
// settings through git (BR-007). The notes of every fake workspace give
// Markdown as their format.
//
// Feature F11: exporting a note (UC-009). The system's save dialog is a fake
// one (`exportDialog`): it chooses a file (by default the proposed one),
// is cancelled, or cannot show, which asks for the path in a field. The
// export "writes" into a fake set of files (`exportedFiles`) after a short
// wait (longer for a PDF); `/home/user/out/Plan.html` exists from the start,
// to try replacing. Rough stand-ins: a table counts as an approximated block
// of the other format, and an image whose place holds "missing" as missing
// in HTML and PDF; `/home/user/locked` cannot be written. As in Rust, one
// dialog at a time ("Choosing where to save {t}…"), reports of exports that
// end together are kept, a failure is marked as an error, the status is
// busy while one runs, and "Saved {t}" does not replace an export's status.
//
// Feature F12: `/home/user/Work` is a git repository (UC-011, UC-012) with
// a fake last commit "Add the meeting notes": README is changed, the note
// `Projects/Roadmap` is new, `Old-Minutes` is deleted and the other file
// `assets/logo.png` (no text) is new. The changes, the overview's letters
// and the diff come as the events `changes` and `diff`, from rough
// stand-ins of git's diff and the proposed message (`mock-git.ts`).
// Committing records the chosen changes in the fake commit; git does not
// know the user at first, so the first commit asks "Your name for commits"
// (`gitIdentity(true)` skips that; `gitIdentityFails(reason)` makes keeping
// the name fail). `gitCommitFails(reason)` makes git
// refuse commits (A4), `gitMerging(true)` stands for an unfinished merge
// (BR-009), and `gitLog()` lists the commit messages, newest first.
//
// Feature F13: `/home/user/Team` is a git repository with a remote
// (UC-013 to UC-015), everything committed; `/home/user/Work` has no
// remote. Team's branches are `main` and `drafts` (which holds the note
// `Sketch`), and the remote has `origin/review`. It syncs when it opens and
// with "Sync now", "Pull" and "Push": a fake sync commits the changes as
// `Auto: …` after a short wait, and ends as `syncOutcome(kind)` says (by
// default `synced`; `offline`, `failed`, `signIn`, `pushRejected`,
// `inTheWay` or `conflict`). A conflict marks a place in `Meetings`, the
// file `assets/team.png` (Keep here/Keep remote), the note `Plan` deleted
// on the remote (Keep/Delete) and todo #3 (Keep mine/Take remote); resolving
// all completes and syncs again, "Abort the sync" puts Meetings back.
// `gitBranchInTheWay(files)` makes the next switch meet changes in the way,
// `gitProblem(reason)` makes git unable to answer (UC-011 A9), and
// `syncs()` counts the fake syncs started.
//
// Feature F14 (`mock-agents.ts`): agents connect and leave (`agentConnect`,
// `agentDisconnect`), read lines (`agentRead`, the agent cursor for 10 s),
// change or create notes (`agentChange`, `agentCreate`: an open note takes
// the change as the backend's edit, a note not open is reported in the
// status with "Open"), and change todos (`agentTodo`). The agents entry of
// the status bar changes the user's access; the section "Agents" installs
// into fake tools (`agentTool` sets how one stands), never into files.
//
// Feature F15 (`mock-todos.ts`): the todos of each fake workspace, the
// section "Todos", the tab "Todos" with its sort and filter, the overdue
// todos and the undo of the status bar. `/home/user/Notes` starts with an
// overdue todo, a critical one with four notes, one linked to a note that
// no longer exists and a done one; `/home/user/Work` with none. The hooks
// change a todo from outside (`todoOutside`, shown after 300 ms, as the
// backend's look of the second), as an agent (`agentTodo` with a todo),
// make the todo files unwritable (`todoWriteFails`; cleared, what was not
// stored is stored half a second later) or a file unreadable
// (`todoUnreadable`), and list the todos (`todos`).
//
// Feature F16: `/home/user/Many` holds one note more than tabs may be open
// (M-06.6); at the limit the tab opened longest ago that can be stored
// closes, else the note does not open (D-07, fixed). `closeWindow` closes
// the window as the system would (M-21.4: the close handshake, "Changes not
// stored", the last sync of `/home/user/Team`), `windowClosed` says whether
// Underline ended, and `failSaves` makes the saves of notes fail.

import { mockIPC } from '@tauri-apps/api/mocks';
import { MockAgents, levelOf, type MockToolState } from './mock-agents';
import { MockTodos, mockCalendar, type MockTodo } from './mock-todos';
import { emit } from '@tauri-apps/api/event';
import type {
  ChosenView,
  DiffLineKind,
  HunkView,
  PaneName,
  ReplaceNoteView,
  SearchSwitches,
  SnapshotsSpaceView,
} from './bindings';
import type {
  commands,
  AgentAction,
  AgentLevelName,
  AgentOpenView,
  AgentToolName,
  BranchEntryView,
  BranchTarget,
  ChoiceName,
  SideName,
  SyncModeName,
  ChangesView,
  ChangeView,
  ConflictBarView,
  GitBarView,
  TodoConflictView,
  TodoAnswer,
  TodoChange,
  TodoFilterName,
  TodoSortName,
  CommitAnswer,
  CreateNoteAnswer,
  DailyNoteAnswer,
  DeletedNoteView,
  DiffView,
  ExportAnswer,
  ExportChoice,
  ExportKindName,
  ExportOfferView,
  ExportPlaceAnswer,
  TypedExportPath,
  NewNoteRequest,
  RenameAnswer,
  RestoreAnswer,
  StatusOffer,
  CreateAnswer,
  DragName,
  EditingViewName,
  EmbedAnswer,
  FormatName,
  Insertion,
  NewWorkspaceRequest,
  OpenAnswer,
  OverviewItem,
  PaletteView,
  Scope,
  SettingName,
  SettingsView,
  StartInfo,
  FigureView,
  FormulaView,
  IndexEntryView,
  IndexesView,
  LinkAnswer,
  TabPropertiesView,
  TabsView,
  ToolView,
  UiError,
  WorkspaceSnapshot,
  SyncBatch,
  FoundLineView,
  FoundNoteView,
  FoundPlace,
  BacklinkNoteView,
  HeadingView,
  LinkFormName,
  LinkPlace,
  LinksView,
  MentionNoteView,
  MentionsView,
  OutgoingLinkView,
} from './bindings';
import { loadEditorCore, MirrorCore } from './editor/core';
import { mockDrop, useMockDrops } from './editor/drop';
import { fakeBlocks, TOUR_NOTE, TOUR_TEXT } from './facade/fakeFacade';
import { MockSettings, userFileOf, type MockSettingsPlace } from './mock-settings';
import {
  MOCK_SPELL_LANGUAGE,
  MockSpeller,
  noteLanguage,
  offeredLanguage,
  SPELL,
  SPELL_TEXTS,
  spellLanguageName,
} from './mock-spell';
import {
  mockChangedLines,
  mockDiff,
  mockProposedMessage,
  type MockProposalChange,
} from './mock-git';
import {
  MOCK_TODO_CONFLICT,
  mockBranchName,
  mockConflictPlaces,
  mockCopyName,
  mockResolvePlace,
  mockSyncBar,
  type MockLastSync,
  type MockSyncOutcome,
} from './mock-remote';
import palettes from './palettes.generated.json';
import { mockTitle } from './mock-title';

/** The success value of a command in the generated bindings. */
type Ok<C extends (...args: never[]) => Promise<{ status: string }>> = Extract<
  Awaited<ReturnType<C>>,
  { status: 'ok' }
>['data'];

/** One handler per backend command, keyed by the command's Rust name. */
export type MockHandlers = {
  app_start: () => Ok<typeof commands.appStart>;
  settings_quick: (args: {
    key: SettingName;
    value: string;
  }) => Ok<typeof commands.settingsQuick> | Promise<Ok<typeof commands.settingsQuick>>;
  settings_window: () =>
    Ok<typeof commands.settingsWindow> | Promise<Ok<typeof commands.settingsWindow>>;
  settings_set: (args: {
    scope: Scope;
    key: SettingName;
    value: string | null;
  }) => Ok<typeof commands.settingsSet> | Promise<Ok<typeof commands.settingsSet>>;
  wiki_check: (args: {
    url: string;
  }) => Ok<typeof commands.wikiCheck> | Promise<Ok<typeof commands.wikiCheck>>;
  ws_start: () => Ok<typeof commands.wsStart> | Promise<Ok<typeof commands.wsStart>>;
  ws_recent: () => Ok<typeof commands.wsRecent>;
  ws_open: (args: {
    path: string;
    anyway: boolean;
  }) => Ok<typeof commands.wsOpen> | Promise<Ok<typeof commands.wsOpen>>;
  ws_check_new: (args: { path: string }) => Ok<typeof commands.wsCheckNew>;
  ws_create: (args: {
    request: NewWorkspaceRequest;
  }) => Ok<typeof commands.wsCreate> | Promise<Ok<typeof commands.wsCreate>>;
  tab_open: (args: { note: string }) => Promise<Ok<typeof commands.tabOpen>>;
  tab_open_side: (args: { note: string }) => Promise<Ok<typeof commands.tabOpenSide>>;
  tab_activate: (args: {
    note: string;
    pane?: PaneName | null;
  }) => Promise<Ok<typeof commands.tabActivate>>;
  pane_focus: (args: { pane: PaneName }) => Promise<Ok<typeof commands.paneFocus>>;
  split_end: () => Promise<Ok<typeof commands.splitEnd>>;
  tab_close: (args: {
    note: string;
    force: boolean;
    pane?: PaneName | null;
  }) => Promise<Ok<typeof commands.tabClose>>;
  tab_store: (args: { note: string }) => Promise<Ok<typeof commands.tabStore>>;
  tab_set_view: (args: {
    note: string;
    view: EditingViewName;
    pane?: PaneName | null;
  }) => Promise<Ok<typeof commands.tabSetView>>;
  ed_viewport: (args: {
    note: string;
    from: number;
    to: number;
  }) => Promise<Ok<typeof commands.edViewport>>;
  clip_read_text: () => Ok<typeof commands.clipReadText>;
  clip_write_text: (args: { text: string }) => Ok<typeof commands.clipWriteText>;
  clip_read: (args: {
    note: string;
    rich: FormatName | null;
    overCopied: boolean;
  }) => Ok<typeof commands.clipRead>;
  clip_write: (args: {
    note: string;
    text: string;
    html: string | null;
    keepBefore: boolean;
  }) => Ok<typeof commands.clipWrite>;
  clip_store_image: (args: {
    note: string;
    overCopied: boolean;
  }) => Promise<Ok<typeof commands.clipStoreImage>>;
  ed_embed: (args: {
    note: string;
    path: string;
    allowLarge: boolean;
  }) => Promise<Ok<typeof commands.edEmbed>>;
  ed_drop: (args: { note: string; paths: string[] }) => Promise<Ok<typeof commands.edDrop>>;
  ed_drag: (args: { state: DragName }) => Promise<Ok<typeof commands.edDrag>>;
  clip_forget: (args: { note: string }) => Ok<typeof commands.clipForget>;
  ed_note_link: (args: { note: string; target: string }) => Ok<typeof commands.edNoteLink>;
  app_close_anyway: () => Ok<typeof commands.appCloseAnyway>;
  ed_mirror: (args: { note: string }) => Promise<Ok<typeof commands.edMirror>>;
  ed_sync: (args: { note: string; batch: SyncBatch }) => Promise<Ok<typeof commands.edSync>>;
  ed_resync: (args: { note: string; text: string }) => Promise<Ok<typeof commands.edResync>>;
  app_request_close: (args: { sent: boolean }) => Promise<Ok<typeof commands.appRequestClose>>;
  ed_follow_link: (args: {
    note: string;
    at: number;
    rev: number;
    side?: boolean;
  }) => Promise<Ok<typeof commands.edFollowLink>>;
  link_answer: (args: { id: number; open: boolean }) => Promise<Ok<typeof commands.linkAnswer>>;
  note_new_start: () => Ok<typeof commands.noteNewStart>;
  note_plan: (args: { request: NewNoteRequest }) => Ok<typeof commands.notePlan>;
  note_create: (args: { request: NewNoteRequest }) => Promise<Ok<typeof commands.noteCreate>>;
  note_daily_open: () => Promise<Ok<typeof commands.noteDailyOpen>>;
  note_daily_step: (args: { later: boolean }) => Promise<Ok<typeof commands.noteDailyStep>>;
  note_rename_start: (args: { note: string }) => Ok<typeof commands.noteRenameStart>;
  note_plan_rename: (args: {
    note: string;
    title: string;
    folder: string;
  }) => Ok<typeof commands.notePlanRename>;
  note_rename: (args: {
    note: string;
    title: string;
    folder: string;
  }) => Promise<Ok<typeof commands.noteRename>>;
  note_delete_start: (args: { note: string }) => Ok<typeof commands.noteDeleteStart>;
  note_delete: (args: {
    note: string;
    forGood: boolean;
  }) => Promise<Ok<typeof commands.noteDelete>>;
  note_undo_delete: () => Promise<Ok<typeof commands.noteUndoDelete>>;
  ws_restore: (args: { key: string }) => Promise<Ok<typeof commands.wsRestore>>;
  ws_restore_as: (args: {
    title: string;
    folder: string;
  }) => Promise<Ok<typeof commands.wsRestoreAs>>;
  ws_purge: (args: { key: string | null }) => Promise<Ok<typeof commands.wsPurge>>;
  search_set: (args: {
    text: string;
    switches: SearchSwitches;
  }) => Promise<Ok<typeof commands.searchSet>>;
  replace_set: (args: {
    text: string;
    replacement: string;
    switches: SearchSwitches;
  }) => Promise<Ok<typeof commands.replaceSet>>;
  replace_page: (args: { serial: number; from: number }) => Ok<typeof commands.replacePage>;
  replace_leave_out: (args: {
    serial: number;
    note: string;
    at: number | null;
    out: boolean;
  }) => Ok<typeof commands.replaceLeaveOut>;
  replace_confirm: (args: {
    serial: number;
    note: string | null;
  }) => Promise<Ok<typeof commands.replaceConfirm>>;
  replace_undo: () => Promise<Ok<typeof commands.replaceUndo>>;
  replace_close: () => Ok<typeof commands.replaceClose>;
  snapshots_list: (args: { note: string }) => Ok<typeof commands.snapshotsList>;
  snapshot_diff: (args: { note: string; key: string }) => Ok<typeof commands.snapshotDiff>;
  snapshot_whole: (args: { note: string; key: string }) => Ok<typeof commands.snapshotWhole>;
  snapshot_restore: (args: {
    note: string;
    key: string;
  }) => Promise<Ok<typeof commands.snapshotRestore>>;
  snapshot_undo_restore: () => Promise<Ok<typeof commands.snapshotUndoRestore>>;
  snapshots_close: () => Ok<typeof commands.snapshotsClose>;
  snapshots_space: () => Ok<typeof commands.snapshotsSpace>;
  snapshots_remove_all: () => Promise<Ok<typeof commands.snapshotsRemoveAll>>;
  palette_ran: (args: { id: string }) => Ok<typeof commands.paletteRan>;
  split_share: (args: { share: number }) => Promise<Ok<typeof commands.splitShare>>;
  spell_suggest: (args: {
    word: string;
    note: string;
  }) => Promise<Ok<typeof commands.spellSuggest>>;
  spell_add_word: (args: { word: string }) => Promise<Ok<typeof commands.spellAddWord>>;
  spell_add_workspace_word: (args: {
    word: string;
  }) => Promise<Ok<typeof commands.spellAddWorkspaceWord>>;
  search_page: (args: { serial: number; from: number }) => Ok<typeof commands.searchPage>;
  search_open: (args: {
    note: string;
    place: FoundPlace | null;
  }) => Promise<Ok<typeof commands.searchOpen>>;
  links_open: (args: { note: string; place: LinkPlace }) => Promise<Ok<typeof commands.linksOpen>>;
  links_headings: (args: { note: string }) => Ok<typeof commands.linksHeadings>;
  links_follow: (args: {
    note: string;
    at: number;
    rev: number;
  }) => Promise<Ok<typeof commands.linksFollow>>;
  links_link_mention: (args: {
    active: string;
    note: string;
    place: LinkPlace;
  }) => Promise<Ok<typeof commands.linksLinkMention>>;
  links_more_mentions: (args: { note: string }) => Ok<typeof commands.linksMoreMentions>;
  export_start: (args: { note: string }) => Promise<Ok<typeof commands.exportStart>>;
  export_choose: (args: {
    note: string;
    kind: ExportKindName;
  }) => Promise<Ok<typeof commands.exportChoose>>;
  export_run: (args: {
    note: string;
    kind: ExportKindName;
    typed: TypedExportPath | null;
  }) => Promise<Ok<typeof commands.exportRun>>;
  export_open: (args: { index: number }) => Promise<Ok<typeof commands.exportOpen>>;
  export_approximated: (args: { index: number }) => Ok<typeof commands.exportApproximated>;
  git_choose: (args: { path: string }) => Promise<Ok<typeof commands.gitChoose>>;
  git_commit_choice: (args: {
    path: string | null;
    chosen: boolean;
  }) => Promise<Ok<typeof commands.gitCommitChoice>>;
  git_commit: (args: { message: string; amend: boolean }) => Promise<Ok<typeof commands.gitCommit>>;
  git_set_identity: (args: {
    name: string;
    email: string;
    message: string;
    amend: boolean;
  }) => Promise<Ok<typeof commands.gitSetIdentity>>;
  git_discard_start: (args: { path: string }) => Promise<Ok<typeof commands.gitDiscardStart>>;
  git_discard: (args: { path: string }) => Promise<Ok<typeof commands.gitDiscard>>;
  git_branches: () => Promise<Ok<typeof commands.gitBranches>>;
  git_branch_name: (args: { typed: string }) => Ok<typeof commands.gitBranchName>;
  git_switch: (args: { target: BranchTarget }) => Promise<Ok<typeof commands.gitSwitch>>;
  sync_request: (args: { mode: SyncModeName }) => Promise<Ok<typeof commands.syncRequest>>;
  conflict_choose: (args: {
    note: string;
    rev: number;
    place: number | null;
    choice: ChoiceName;
  }) => Promise<Ok<typeof commands.conflictChoose>>;
  conflict_take_side: (args: {
    path: string;
    side: SideName;
  }) => Promise<Ok<typeof commands.conflictTakeSide>>;
  conflict_keep: (args: {
    path: string;
    keep: boolean;
  }) => Promise<Ok<typeof commands.conflictKeep>>;
  conflict_abort: () => Promise<Ok<typeof commands.conflictAbort>>;
  todo_conflict_resolve: (args: {
    number: number;
    mine: boolean;
  }) => Promise<Ok<typeof commands.todoConflictResolve>>;
  agents_set_access: (args: {
    level: AgentLevelName;
  }) => Promise<Ok<typeof commands.agentsSetAccess>>;
  agents_section: (args: {
    action: AgentAction | null;
  }) => Promise<Ok<typeof commands.agentsSection>>;
  todo_create: (args: { title: string }) => Promise<Ok<typeof commands.todoCreate>>;
  todo_change: (args: {
    number: number;
    change: TodoChange;
  }) => Promise<Ok<typeof commands.todoChange>>;
  todo_mark: (args: { number: number; done: boolean }) => Promise<Ok<typeof commands.todoMark>>;
  todo_delete: (args: { number: number }) => Promise<Ok<typeof commands.todoDelete>>;
  todo_undo: () => Promise<Ok<typeof commands.todoUndo>>;
  todo_open_link: (args: { link: string }) => Promise<Ok<typeof commands.todoOpenLink>>;
  todo_candidates: (args: {
    number: number;
    query: string;
  }) => Promise<Ok<typeof commands.todoCandidates>>;
  todo_month: (args: { day: string | null; step: number }) => Ok<typeof commands.todoMonth>;
  todo_list_show: (args: {
    filter: TodoFilterName | null;
  }) => Promise<Ok<typeof commands.todoListShow>>;
  todo_list_close: () => Promise<Ok<typeof commands.todoListClose>>;
  ws_switch_anyway: () => Promise<Ok<typeof commands.wsSwitchAnyway>>;
  todo_list_set: (args: {
    sort: TodoSortName;
    filter: TodoFilterName;
  }) => Promise<Ok<typeof commands.todoListSet>>;
  /** ADR-025: what a test may do as the backend would (`window.__ulMock`). */
  __hooks?: MockHooks;
};

/** F11: what the fake save dialog does (UC-009 step 2). */
export type MockExportDialog =
  { answer: 'choose'; path?: string } | { answer: 'cancel' } | { answer: 'unavailable' };

/** F11: the kinds of export of a Markdown and an AsciiDoc note (`ExportKind::label`). */
export const FAKE_EXPORT_KINDS: { markdown: ExportChoice[]; asciiDoc: ExportChoice[] } = {
  markdown: [
    { kind: 'otherFormat', label: 'AsciiDoc' },
    { kind: 'html', label: 'HTML' },
    { kind: 'pdf', label: 'PDF' },
  ],
  asciiDoc: [
    { kind: 'otherFormat', label: 'Markdown' },
    { kind: 'html', label: 'HTML' },
    { kind: 'pdf', label: 'PDF' },
  ],
};

/** F7: a note with a link of every kind, headings, code and figures. */
export const LINK_TOUR_NOTE = 'Link-Tour';

/** The text of the link tour (M-07.3, M-16.5). */
export const LINK_TOUR_TEXT = `# Link tour

Hold Ctrl and click a link.

- [Down to the details](#details)
- [A heading that is not there](#nowhere)
- [The roadmap](Projects/Roadmap.md#roadmap)
- [A note to write](Ideas/Someday.md)
- [The web](https://example.com/underline)
- [A report](report.pdf)
- [Outside](../../etc/hosts)
- [A server](ssh://server.example/home)
- [A script](tools/deploy.sh)
- [A full place](file:///home/user/Documents/report.pdf)

## Details

\`\`\`sh
ls -l
\`\`\`

\`\`\`mermaid
graph TD
A-->B
\`\`\`

![The plan](img/plan.svg)

### Deeper

The end.
`;

/** The user's home directory in the fake file system. */
export const FAKE_HOME = '/home/user';

/** F13: the fake git workspace with a remote (UC-013 to UC-015). */
export const TEAM = '/home/user/Team';

/** M-06.6: at most this many tabs are open (`MAX_TABS` of underline-core). */
export const MAX_TABS = 100;

/** F16, M-06.6: a workspace with `MAX_TABS + 1` notes, to try the tab limit. */
export const MANY = '/home/user/Many';

/**
 * Package 2, TC-004-25 to TC-004-27: a workspace for go to note, with the
 * notes of the test cases' preconditions.
 */
export const PLANS = '/home/user/Plans';

/** Package 2, NFR-018, TC-004-28, TC-023-10: a workspace of 10,000 notes. */
export const LARGE = '/home/user/Large';

/** Package 3, TC-004-29 to TC-004-31, TC-005-33: notes that link to each other. */
export const LINKS = '/home/user/Links';

/** The texts of the notes of `LINKS`, by note. */
export const LINKS_TEXTS: Readonly<Record<string, string>> = {
  'Project-Plan': '# Project Plan\n\n## Goals\n\nShip it. See [the goals](#goals).\n',
  Brainstorm:
    '# Brainstorm\n\nOne idea\n[the plan](Project-Plan.md) first\n\nTwo\n\nThree\n[goals](Project-Plan.md#goals)\n',
  'travel/Itinerary': '# Itinerary\n[Plan](../Project-Plan.md) for the trip\n![Map](map.png)\n',
  Snippets: '# Snippets\n\n`[x](Project-Plan.md)`\n\n```\n[y](Project-Plan.md)\n```\n',
  Hub:
    '# Hub\n\n[Project Plan](Project-Plan.md)\n[Gone](Gone.md)\n[Goals](Project-Plan.md#nothing)\n' +
    '[Logo](img/logo.png)\n<https://example.org>\n',
  Diary:
    '# Diary\n\nNothing linked yet.\nThe project plan is late; ask about the Project Plan.\n' +
    '`project plan` stays.\n',
  'travel/Checklist': '# Checklist\n\n## Clothes\n\n## Papers\n',
  Guide: '= Guide\n\nType xref: here to link a note.\n\ninclude::Project-Plan.md[]\n',
};

/**
 * Package 4 (FR-081 to FR-086): the notes of TC-005-35 to TC-005-38 and
 * TC-004-33, to try close pairs, moving lines, indenting list items, the
 * current line and the matching bracket, and going to a line.
 */
export const COMFORT = '/home/user/Comfort';

/** TC-004-33: `Long` has 300 lines; line 120 is in a paragraph from line 118, in "Results". */
function longText(): string {
  const lines = Array.from({ length: 300 }, (_, at) => `Line ${at + 1} of the long note.`);
  lines[0] = '# Long';
  lines[1] = '';
  lines[98] = '';
  lines[99] = '## Results';
  lines[100] = '';
  lines[116] = '';
  lines[200] = '';
  lines[201] = '## Later';
  lines[202] = '';
  return lines.join('\n');
}

/** The texts of the notes of `COMFORT`, by note. */
export const COMFORT_TEXTS: Readonly<Record<string, string>> = {
  Pairs: '# Pairs\n\nType a word here\n',
  Lines: 'one\ntwo\nthree\n\nfour\nfive\n\n- a\n  - a1\n- b\n',
  List: '- a\n- b\n- c\n',
  Bullets: '* a\n* b\n',
  Code: 'f(a[1], {b})\ng((x)\n',
  Long: longText(),
};

/**
 * Package 5 (FR-087 to FR-090): the notes of TC-004-39 (text statistics)
 * and TC-010-18 to TC-010-20 (editor zoom, focus mode, soft wrap).
 */
export const READING = '/home/user/Reading';

/** TC-010-20: a line and a paragraph of 400 characters, a wide table and a code line of 300. */
function wideText(): string {
  const words = (count: number) =>
    Array.from({ length: count }, (_, at) => `word${String(at).padStart(3, '0')}`)
      .join(' ')
      .slice(0, count);
  const table = `| ${Array.from({ length: 12 }, (_, at) => `Column ${at + 1} heading`).join(' | ')} |`;
  return [
    '# Wide',
    '',
    words(400),
    '',
    words(400),
    '',
    table,
    `|${Array.from({ length: 12 }, () => '---').join('|')}|`,
    `| ${Array.from({ length: 12 }, (_, at) => `cell ${at + 1}`).join(' | ')} |`,
    '',
    '```text',
    'x'.repeat(300),
    '```',
    '',
  ].join('\n');
}

/**
 * FR-098 (plan section 10): the mock's stand-in for a typeset formula, a
 * placeholder picture 0.6 em wide per character of its source (the mirror
 * widens it), as the backend's RaTeX picture comes through `ul-asset`.
 */
const MOCK_FORMULA: FormulaView = {
  source: '',
  display: false,
  picture: `data:image/svg+xml,${encodeURIComponent(
    '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 60 120" preserveAspectRatio="none">' +
      '<rect x="2" y="10" width="56" height="100" rx="8" fill="#8a8f98" fill-opacity="0.35"/>' +
      '<path d="M14 90 L30 30 L46 90" stroke="#8a8f98" stroke-width="6" fill="none"/></svg>',
  )}`,
  width: 600,
  height: 1200,
  depth: 300,
  error: null,
};

/** The texts of the notes of `READING`, by note. */
export const READING_TEXTS: Readonly<Record<string, string>> = {
  Count:
    "# Title\n\nIt's a well-known [fact](https://example.org/a-b-c).\n\n```rust\nlet x = 1;\n```\n",
  'Count-AsciiDoc':
    "= Title\n:toc:\n\nIt's a well-known https://example.org/a-b-c[fact].\n\n[source,rust]\n----\nlet x = 1;\n----\n",
  Wide: wideText(),
  // FR-098: TC-004-38 (`Math.md`, `Math.adoc`), TC-005-42 and TC-009-18.
  Math: 'Energy $E=mc^2$ and price 5$ or 6$\n\n$$\\sum_{i=1}^n i$$\n\n```math\n\\sqrt{2}\n```\n\n$$\\frac{1}{$$\n',
  'Math-AsciiDoc': '= Math\n:stem: latexmath\n\nSee stem:[a^2] and latexmath:[b^2].\n',
  // OQ-222: TC-004-38 step 4, AsciiMath (`:stem:` without a value).
  'Math-AsciiMath':
    '= Math\n:stem:\n\nSee stem:[x^2/2] and asciimath:[sqrt a].\n\n[asciimath]\n++++\nsum_(i=1)^n i\n++++\n',
  'Math-Edit': '$$a^2$$\n\nThe inline formula $b$ here.\n',
  'Math-Export': 'Euler $e^{i\\pi}+1=0$.\n\n$$\\int_0^1 x\\,dx$$\n\n$$\\frac{1}{$$\n',
};

/**
 * Package 6a (FR-091 to FR-093): the notes of TC-024 (note attributes),
 * TC-004-32 and TC-008-21 (tags and search by tag). The TCs' `Plan.md`,
 * `Trip.md` and `Guide.adoc` are `Quarter-Plan`, `Ferry-Trip` and
 * `Travel-Guide` here, as the mock keeps one text per note name; `Plain.adoc`
 * is `Plain-AsciiDoc`.
 */
export const ATTRIBUTES = '/home/user/Attributes';

/** The texts of the notes of `ATTRIBUTES`, by note. */
export const ATTRIBUTE_TEXTS: Readonly<Record<string, string>> = {
  'Quarter-Plan': '---\nstatus: draft\ndue: 2026-12-01\n---\n# Plan\n\nThe plan of the quarter.\n',
  Handbook: '= Guide\n:author: Ann\n\nThe handbook.\n',
  Mixed: '---\n# Kept as written\ntags: [a, b]\n\ntitle:  "Mixed"\n---\n# Mixed\n',
  Kinds:
    '---\ntitle: Kinds\ntags: [a, b]\nweight: 2.5\ndue: 2026-12-01\ndone: false\n---\n# Kinds\n',
  Plain: '# Plain\n',
  'Plain-AsciiDoc': '= Plain\n',
  Broken: '---\nstatus: draft\njust text\n---\n# Broken\n',
  Nested: '---\nauthor:\n  name: Ann\n  mail: ann@example.org\nstatus: draft\n---\n# Nested\n',
  'Ferry-Trip':
    '---\ntags: [Travel]\n---\nFerry to #hydra and #Project/alpha, issue #12, C#, `#code`, [link](#ferry)\n\nsee #greece\n',
  'Travel-Guide': '= Guide\n:tags: travel, docs\n\na #highlight# word\n',
  Hydra: '---\ntags: [travel/greece]\n---\n# Hydra\n\nThe island.\n',
  Price: '# Price\n\ncosts #42\n\ntravel\n',
};

/** Package 6b: the workspace of the templates and the daily notes (TC-003-17, TC-025). */
export const TEMPLATES = '/home/user/Templates';

/** The texts of the notes of `TEMPLATES`, by note. */
export const TEMPLATE_TEXTS: Readonly<Record<string, string>> = {
  'templates/Meeting': '# {{title}}\nDate: {{date}} {{time}}\n{{cursor}}\nOwner: {{owner}}\n',
  'templates/Spec': '= {{title}}\n\n{{cursor}}\n',
  'templates/Daily': '= {{title}}\n\n{{cursor}}\n== Done\n',
  'daily/2026-11-05': '# 2026-11-05\n\nAn earlier day.\n',
};

/** The notes of the mock written in AsciiDoc (TC-005-33 step 5, TC-005-37); all others are Markdown. */
const ASCIIDOC_NOTES: ReadonlySet<string> = new Set([
  'templates/Spec',
  'templates/Daily',
  'Guide',
  'Bullets',
  'Count-AsciiDoc',
  'Handbook',
  'Plain-AsciiDoc',
  'Travel-Guide',
  'Math-AsciiDoc',
  'Math-AsciiMath',
]);

/** The note format of a fake note. */
export function mockFormat(note: string): FormatName {
  return ASCIIDOC_NOTES.has(note) ? 'AsciiDoc' : 'Markdown';
}

/** The files of `LINKS` that are no notes. */
const LINKS_FILES: readonly string[] = ['img/logo.png'];

/** The notes of `LARGE`: 10,000 in 13 folders. */
const LARGE_NOTES: readonly string[] = Array.from(
  { length: 10_000 },
  (_, n) => `area-${n % 13}/Meeting-${String(n).padStart(5, '0')}-project-${n % 97}`,
);

/**
 * The fake file system: workspaces by their root with their notes' ids, and
 * `/home/user/Letters`, a directory with files that is no workspace yet.
 */
export const FAKE_FILES: Readonly<Record<string, readonly string[]>> = {
  '/home/user/Notes': [
    'Home',
    'Ideas',
    'Trip',
    'Log',
    'Reading-List',
    'guide/Getting-Started',
    'guide/Home',
    'Journal/2026-11-09',
  ],
  '/home/user/Work': ['README', 'Meetings', 'Projects/Roadmap', TOUR_NOTE, LINK_TOUR_NOTE],
  '/home/user/Letters': ['Draft'],
  '/home/user/Team': ['Home', 'Plan', 'Meetings'],
  [PLANS]: [
    'Plan',
    'Release-Plan',
    'travel/Plane-Tickets',
    'travel/Packing',
    'Explanation',
    'Ideas',
    'Über-uns',
  ],
  [LARGE]: LARGE_NOTES,
  [LINKS]: Object.keys(LINKS_TEXTS),
  [COMFORT]: Object.keys(COMFORT_TEXTS),
  [READING]: Object.keys(READING_TEXTS),
  [ATTRIBUTES]: Object.keys(ATTRIBUTE_TEXTS),
  [TEMPLATES]: ['Home', ...Object.keys(TEMPLATE_TEXTS)],
  // Package 8b: TC-005-39, TC-005-40.
  [SPELL]: Object.keys(SPELL_TEXTS),
  // F16, M-06.6: one note more than tabs may be open.
  [MANY]: [
    'Home',
    ...Array.from({ length: MAX_TABS }, (_, i) => `N${String(i + 1).padStart(3, '0')}`),
  ],
};

/**
 * F6: files outside the workspaces by path, with their size in bytes; a
 * size of -1 is a folder. Over 10 MB a file is asked about (UC-005 A19).
 */
export const FAKE_OUTSIDE: Readonly<Record<string, number>> = {
  '/home/user/Pictures': -1,
  '/home/user/Pictures/photo.png': 48_000,
  '/home/user/Pictures/poster.png': 12 * 1024 * 1024,
  '/home/user/Pictures/banner.png': 11 * 1024 * 1024,
  '/home/user/Documents/report.pdf': 120_000,
};

/**
 * Q-61: files of the workspaces that are no notes, by their place in the
 * workspace; a link to one opens it (or asks first when it may run).
 */
export const FAKE_WORKSPACE_FILES: readonly string[] = ['tools/deploy.sh', 'img/plan.svg'];

/** Decodes `%20` and the like; a `%` that starts no escape stays (as `percent_decode`). */
function safeDecode(text: string): string {
  try {
    return decodeURIComponent(text);
  } catch {
    return text;
  }
}

/** UC-004 BR-007, Q-62: the scheme of `address` in lower case, as `link_scheme`. */
export function mockLinkScheme(address: string): string | null {
  const match = /^([A-Za-z][A-Za-z0-9+.-]*):/.exec(address.trim());
  return match ? match[1].toLowerCase() : null;
}

/** UC-004 BR-007, Q-62: only http, https and mailto open without asking. */
export function mockOpensWithoutAsking(address: string): boolean {
  return ['http', 'https', 'mailto'].includes(mockLinkScheme(address) ?? '');
}

/** Q-61: the kinds of programs by their ending, as `EXECUTABLE_ENDINGS` (deliberately not complete). */
const MOCK_EXECUTABLE_ENDINGS: Readonly<Record<string, string>> = {
  desktop: 'a desktop launcher',
  sh: 'a shell script',
  command: 'a shell script',
  run: 'a program',
  bin: 'a program',
  appimage: 'a program',
  py: 'a Python script',
  pl: 'a Perl script',
  rb: 'a Ruby script',
  bat: 'a batch file',
  cmd: 'a batch file',
  lnk: 'a Windows shortcut',
  url: 'an internet shortcut',
  webloc: 'an internet shortcut',
  inetloc: 'an internet shortcut',
  exe: 'a Windows program',
  com: 'a Windows program',
  pif: 'a Windows program',
  scf: 'a Windows Explorer command',
  msi: 'a Windows installer',
  msp: 'a Windows installer',
  inf: 'a Windows setup file',
  application: 'a Windows program',
  'appref-ms': 'a Windows program',
  gadget: 'a Windows gadget',
  scr: 'a screen saver program',
  ps1: 'a PowerShell script',
  js: 'a script',
  jse: 'a Windows script',
  vbs: 'a Windows script',
  vbe: 'a Windows script',
  wsf: 'a Windows script',
  wsh: 'a Windows script',
  hta: 'an HTML application',
  chm: 'a Windows help file',
  msc: 'a Windows console',
  cpl: 'a Control Panel item',
  reg: 'a registry file',
  'library-ms': 'a Windows library',
  'search-ms': 'a Windows search',
  'settingcontent-ms': 'a Windows settings shortcut',
  jar: 'a Java program',
  app: 'a macOS app',
  terminal: 'a Terminal setting',
  workflow: 'an Automator workflow',
  scpt: 'an AppleScript',
};

/**
 * UC-004 BR-007, Q-61: the kind of program a file is by its name, with
 * its article, as `executable_by_name`: dots and spaces at the end and a
 * `:stream` are left out first.
 */
export function mockExecutableKind(path: string): string | null {
  const name = (path.split(/[/\\]/).at(-1) ?? path).split(':')[0].replace(/[. ]+$/, '');
  const ending = /\.([^.]+)$/.exec(name)?.[1]?.toLowerCase();
  return ending === undefined ? null : (MOCK_EXECUTABLE_ENDINGS[ending] ?? null);
}

/** Characters written as `\u{…}` in a question, as `shown_text`. */
const MOCK_HIDDEN =
  /[\p{Cc}\u00ad\u061c\u180e\u200b-\u200f\u202a-\u202e\u2060-\u2064\u2066-\u2069\ufeff]/gu;

/** Q-61: `text` with invisible and direction characters written out, as `shown_text`. */
export function mockShownText(text: string): string {
  return text.replace(MOCK_HIDDEN, (c) => `\\u{${c.codePointAt(0)!.toString(16)}}`);
}

/** Q-62: the scheme as the question names it, as `shown_scheme`. */
function mockShownScheme(address: string): string {
  const scheme = mockLinkScheme(address);
  if (scheme !== null) {
    return scheme;
  }
  const raw = address.split(':')[0];
  const shown = [...raw]
    .map((c) => (/^[!-~]$/.test(c) ? c : `\\u{${c.codePointAt(0)!.toString(16)}}`))
    .join('');
  return shown === '' ? '?' : shown;
}

/** Q-61: a name of at most 60 characters, the middle left out, as `shortened`. */
function mockShortened(name: string): string {
  const chars = [...name];
  return chars.length <= 60 ? name : `${chars.slice(0, 28).join('')}…${chars.slice(-28).join('')}`;
}

/** The question "Open …" of the session, without its id (`LinkAnswer` `askOpen`). */
export type MockOpenQuestion = {
  about: 'link' | 'program' | 'file';
  title: string;
  question: string;
  warning: string | null;
  target: string;
};

/**
 * UC-004 A4, Q-61, Q-62: the question the session asks before opening
 * `target`: of the scheme of an address, of a program, of a `file://`
 * target, or of a place on another computer (`//server/share`), never
 * looked at; `null` when it opens directly.
 */
export function mockOpenQuestion(
  target: string,
  how: 'address' | 'file' | 'fileUrl',
): MockOpenQuestion | null {
  const shown = mockShownText(target);
  if (how === 'address') {
    if (mockOpensWithoutAsking(target)) {
      return null;
    }
    const scheme = mockShownScheme(target);
    return {
      about: 'link',
      title: `Open ${scheme}: link`,
      question: `This link uses the ${scheme}: scheme and opens another program on this computer.`,
      warning: null,
      target: shown,
    };
  }
  const name = mockShortened(mockShownText(target.split(/[/\\]/).at(-1) ?? target));
  const program = mockExecutableKind(target);
  if (program !== null) {
    return {
      about: 'program',
      title: 'Open program',
      question: `${name} is ${program}.`,
      warning: 'Opening it may run it on this computer.',
      target: shown,
    };
  }
  if (/^[/\\]{2}/.test(target)) {
    return {
      about: 'file',
      title: 'Open file',
      question: `This link points to a file on another computer or a device: ${name}.`,
      warning: null,
      target: shown,
    };
  }
  if (how === 'fileUrl') {
    const ending = /\.([^./]+)$/.exec(name)?.[1];
    const kind =
      FAKE_OUTSIDE[target] === -1 ? 'folder' : ending ? `${ending.toUpperCase()} file` : 'file';
    return {
      about: 'file',
      title: 'Open file',
      question: `This link points to a file on this computer: ${name}, ${kind}.`,
      warning: null,
      target: shown,
    };
  }
  return null;
}

/** F8: a note among the deleted notes of the mock (UC-007 A5). */
export type MockDeleted = { key: string; note: string; text: string; at: number };

/**
 * F8, M-05.6: the deleted notes each workspace starts with. `Ideas` is in
 * the workspace again, so restoring it asks for another place (UC-007 A6).
 */
export const FAKE_DELETED: Readonly<Record<string, readonly MockDeleted[]>> = {
  '/home/user/Notes': [
    {
      key: '1794000000000:0:Old-Plan',
      note: 'Old-Plan',
      text: '# Old Plan\n',
      at: 1794000000000,
    },
    {
      key: '1793900000000:0:guide/Draft',
      note: 'guide/Draft',
      text: '# Draft\n',
      at: 1793900000000,
    },
    { key: '1793800000000:0:Ideas', note: 'Ideas', text: '# Ideas, older\n', at: 1793800000000 },
  ],
};

/** `ms` since 1970 as local time to the minute, as `format_local_time` gives it. */
function localTime(ms: number): string {
  const d = new Date(ms);
  const two = (n: number) => String(n).padStart(2, '0');
  return `${d.getFullYear()}-${two(d.getMonth() + 1)}-${two(d.getDate())} ${two(d.getHours())}:${two(d.getMinutes())}`;
}

/** What the fake system clipboard holds (F6). */
export type MockClipboard = { text: string | null; html: string | null; image: boolean };

/**
 * A rough stand-in for `html_to_markup` (Markdown only): headings, bold,
 * italic, list items and paragraphs; every other tag goes. The real rules
 * are tested in Rust.
 */
export function mockHtmlToMarkup(html: string): string {
  return html
    .replace(
      /<h([1-6])[^>]*>(.*?)<\/h\1>/gis,
      (_, level: string, text: string) => `${'#'.repeat(Number(level))} ${text}\n\n`,
    )
    .replace(/<(b|strong)[^>]*>(.*?)<\/\1>/gis, '**$2**')
    .replace(/<(i|em)[^>]*>(.*?)<\/\1>/gis, '*$2*')
    .replace(/<li[^>]*>(.*?)<\/li>/gis, '- $1\n')
    .replace(/<\/p>/gi, '\n\n')
    .replace(/<[^>]+>/g, '')
    .replace(/\n{3,}/g, '\n\n')
    .trim();
}

/** The palettes of all themes, as the backend exports them. */
export const PALETTES: readonly PaletteView[] = palettes;

/** The palette of the theme named `name`. */
export function paletteOf(name: string): PaletteView | undefined {
  return PALETTES.find((palette) => palette.theme === name);
}

/** The built-in settings (UC-010 BR-001): light theme, menu bar hidden. */
export function defaultSettings(): SettingsView {
  return new MockSettings({}, PALETTES).effective();
}

/** What the mock starts with; every part can be replaced. */
export type MockStart = Partial<Omit<StartInfo, 'settings'>> & {
  settings?: Partial<SettingsView>;
  /** The recently opened workspaces, the most recent first. */
  recent?: string[];
  /** Notes (ids) whose saves fail, to try M-10.8 and the close question. */
  failingSaves?: string[];
  /** M-07.2: the view a newly opened tab shows; the backend's is the facade. */
  view?: EditingViewName;
  /** Package 6b: the local clock, `2026-11-11T08:15`; the browser's when not given. */
  now?: string;
};

/** M-10.2: the toolbar's buttons, as the backend gives them (`Tool::label`, `Tool::hint`). */
export const FAKE_TOOLS: readonly ToolView[] = [
  {
    tool: 'heading',
    label: 'H',
    hint: 'Heading (Ctrl+Shift+H)',
    name: 'Heading',
    key: 'H',
    shift: true,
  },
  {
    tool: 'bold',
    label: 'B',
    hint: 'Bold (Ctrl+B)',
    name: 'Bold',
    key: 'B',
    shift: false,
  },
  {
    tool: 'italic',
    label: 'I',
    hint: 'Italic (Ctrl+I)',
    name: 'Italic',
    key: 'I',
    shift: false,
  },
  {
    tool: 'code',
    label: '</>',
    hint: 'Code (Ctrl+E)',
    name: 'Code',
    key: 'E',
    shift: false,
  },
  {
    tool: 'list',
    label: 'List',
    hint: 'List (Ctrl+Shift+L)',
    name: 'List',
    key: 'L',
    shift: true,
  },
  {
    tool: 'quote',
    label: 'Quote',
    hint: 'Quote (Ctrl+Shift+Q)',
    name: 'Quote',
    key: 'Q',
    shift: true,
  },
  {
    tool: 'link',
    label: 'Link',
    hint: 'Link (Ctrl+K)',
    name: 'Link',
    key: 'K',
    shift: false,
  },
  {
    tool: 'image',
    label: 'Image',
    hint: 'Image (Ctrl+Shift+I)',
    name: 'Image',
    key: 'I',
    shift: true,
  },
  {
    tool: 'table',
    label: 'Table',
    hint: 'Table (Ctrl+Shift+T)',
    name: 'Table',
    key: 'T',
    shift: true,
  },
];

/** F9, UC-008: notes with places to find (TC-008-01, TC-008-11). */
const SEARCH_TEXTS: Readonly<Record<string, string>> = {
  Trip: `# Trip

Book the ferry to Hydra.
Pack light; the FERRY leaves at nine.

## Islands

Hydra has no cars; the Ferryman knows every harbour.
`,
  Log: `# Log

${Array.from({ length: 7 }, (_, n) => `- ${n + 1}: deploy the site`).join('\n')}
- ${'A very long line that goes on and on about nothing in particular, '.repeat(5)}and only then does it deploy, at its end.
`,
};

/** The reason a failing save gives in the mock. */
export const MOCK_SAVE_ERROR = 'Permission denied (os error 13)';

/** The text of a fake note. */
export function fakeNoteText(note: string): string {
  if (note in SEARCH_TEXTS) {
    return SEARCH_TEXTS[note];
  }
  if (note in LINKS_TEXTS) {
    return LINKS_TEXTS[note];
  }
  if (note in COMFORT_TEXTS) {
    return COMFORT_TEXTS[note];
  }
  if (note in READING_TEXTS) {
    return READING_TEXTS[note];
  }
  if (note in ATTRIBUTE_TEXTS) {
    return ATTRIBUTE_TEXTS[note];
  }
  if (note in TEMPLATE_TEXTS) {
    return TEMPLATE_TEXTS[note];
  }
  if (note in SPELL_TEXTS) {
    return SPELL_TEXTS[note];
  }
  if (note === TOUR_NOTE) {
    return TOUR_TEXT;
  }
  if (note === LINK_TOUR_NOTE) {
    return LINK_TOUR_TEXT;
  }
  const title = mockTitle(note);
  return `# ${title}\n\nThis note lives in the mock backend.\nType here to try the source view.\n`;
}

/** A label of the tool panel, at most 60 characters (`index.rs`). */
function shorten(text: string): string {
  const trimmed = text.trim();
  const chars = [...trimmed];
  return chars.length <= 60 ? trimmed : `${chars.slice(0, 59).join('')}…`;
}

/**
 * M-07.3: a rough stand-in for `link_at_web` over Markdown: the address of
 * the `[label](address)` whose label holds `at`, from its first character
 * to just after its last.
 */
export function mockLinkAt(text: string, at: number): string | null {
  for (const found of text.matchAll(/\[([^\]]*)\]\(([^)\s]*)\)/g)) {
    const start = (found.index ?? 0) + 1;
    if (at >= start && at <= start + found[1].length) {
      return found[2];
    }
  }
  return null;
}

/** A link of a note as the mock finds it: its place, text and where it is. */
type MockLink = {
  at: number;
  end: number;
  line: number;
  place: string;
  text: string;
  form: LinkFormName;
};

/**
 * FR-078, FR-079, OQ-244: a rough stand-in for `found_links`:
 * `[text](place)`, pictures `![alt](place)`, `<address>`, and AsciiDoc's
 * `include::place[]` and `image::place[]`, not in fenced code or a code
 * span. `at` and `end` are in UTF-16 units; a picture's text is its place,
 * as the backend lists it.
 */
export function mockFoundLinks(text: string): MockLink[] {
  const found: MockLink[] = [];
  let fenced = false;
  let start = 0;
  text.split('\n').forEach((line, index) => {
    if (line.startsWith('```')) {
      fenced = !fenced;
    } else if (!fenced) {
      for (const match of line.matchAll(
        /(!?)\[([^\]]*)\]\(([^)\s]*)\)|<((?:https?:\/\/|mailto:)[^>\s]+)>|(include::|image::?)([^[\s]+)\[[^\]]*\]/g,
      )) {
        const at = match.index ?? 0;
        const ticks = (line.slice(0, at).match(/`/g) ?? []).length;
        if (ticks % 2 === 1) {
          continue;
        }
        const form: LinkFormName =
          match[1] === '!' || match[5]?.startsWith('image')
            ? 'picture'
            : match[5] === 'include::'
              ? 'include'
              : 'link';
        const place = match[3] ?? match[4] ?? match[6];
        found.push({
          at: start + at,
          end: start + at + match[0].length,
          line: index + 1,
          place,
          text: form !== 'link' ? place : match[4] === undefined ? match[2] : match[4],
          form,
        });
      }
    }
    start += line.length + 1;
  });
  return found;
}

/**
 * OQ-248, UC-005 A2: the place of the note file `to` as a link in the note
 * file `from` names it, as `link_place`: relative to its folder, with `%`,
 * `#`, spaces, brackets and angle brackets percent-encoded.
 */
export function mockLinkPlace(from: string, to: string): string {
  const folder = from.includes('/') ? from.slice(0, from.lastIndexOf('/')).split('/') : [];
  const parts = to.split('/');
  let common = 0;
  while (common < folder.length && common < parts.length && folder[common] === parts[common]) {
    common += 1;
  }
  const encode = (part: string) =>
    part.replace(
      /[%# ()[\]<>\t:`]/g,
      (c) => `%${c.charCodeAt(0).toString(16).toUpperCase().padStart(2, '0')}`,
    );
  return [...folder.slice(common).map(() => '..'), ...parts.slice(common).map(encode)].join('/');
}

/** OQ-248: a title as the text of a link, its brackets escaped, as `link_text`. */
export function mockLinkText(label: string, markdown: boolean): string {
  return markdown ? label.replace(/[[\]\\`]/g, '\\$&') : label.replace(/]/g, '\\]');
}

/** BR-021: titles with fewer characters than this have no unlinked mentions. */
const MENTION_LEAST_CHARS = 3;

/** BR-021: how many unlinked mentions are listed at first, and more each time. */
export const MOCK_MENTIONS_SHOWN = 200;

/**
 * FR-103, UC-004 BR-021: a rough stand-in for `unlinked_mentions`: the
 * title of `note` as whole words, upper and lower case alike, outside
 * fenced code, code spans and links, in the other notes of `order`; at
 * most `limit` listed. Places are bytes, as the backend counts them.
 */
export function mockMentions(
  note: string,
  order: readonly string[],
  textOf: (note: string) => string,
  limit = MOCK_MENTIONS_SHOWN,
): MentionsView {
  const title = mockTitle(note);
  const words = title.split(/\s+/).filter((word) => word !== '');
  if (words.join('').length < MENTION_LEAST_CHARS) {
    return { notes: [], count: 0, more: 0, tooShort: true };
  }
  const escaped = words.map((word) => word.replace(/[.*+?^${}()|[\]\\]/g, '\\$&'));
  const pattern = new RegExp(
    `(?<![\\p{L}\\p{N}#])${escaped.join('[ \\t]+')}(?![\\p{L}\\p{N}])`,
    'giu',
  );
  const notes: MentionNoteView[] = [];
  let count = 0;
  let listed = 0;
  for (const other of order) {
    if (other === note) {
      continue;
    }
    const text = textOf(other);
    const links = mockFoundLinks(text);
    const mentions: MentionNoteView['mentions'] = [];
    let fenced = false;
    let start = 0;
    text.split('\n').forEach((line, index) => {
      if (line.startsWith('```')) {
        fenced = !fenced;
      } else if (!fenced) {
        for (const match of line.matchAll(pattern)) {
          const at = start + (match.index ?? 0);
          const end = at + match[0].length;
          const ticks = (line.slice(0, match.index).match(/`/g) ?? []).length;
          if (ticks % 2 === 1 || links.some((link) => link.at < end && at < link.end)) {
            continue;
          }
          count += 1;
          if (listed + mentions.length >= limit) {
            continue;
          }
          mentions.push({
            line: index + 1,
            before: line.slice(0, match.index).trimStart(),
            mention: match[0],
            after: line.slice((match.index ?? 0) + match[0].length).trimEnd(),
            place: {
              from: bytesBefore(text, at),
              to: bytesBefore(text, end),
              check: mockLinkCheck(match[0]),
            },
          });
        }
      }
      start += line.length + 1;
    });
    if (mentions.length > 0) {
      listed += mentions.length;
      notes.push({
        note: other,
        title: mockTitle(other),
        folder: other.includes('/') ? other.slice(0, other.lastIndexOf('/')) : null,
        mentions,
      });
    }
  }
  return { notes, count, more: count - listed, tooShort: false };
}

/** The anchors of the ATX headings of `text`, with their labels and levels. */
export function mockHeadings(text: string): HeadingView[] {
  const headings: HeadingView[] = [];
  let fenced = false;
  for (const line of text.split('\n')) {
    if (line.startsWith('```')) {
      fenced = !fenced;
      continue;
    }
    const heading = fenced ? null : /^(#{1,6})\s+(.*?)\s*$/.exec(line);
    if (heading) {
      headings.push({
        label: heading[2],
        anchor: mockAnchor(heading[2]),
        level: heading[1].length,
      });
    }
  }
  return headings;
}

/** Where a link leads, roughly as `resolve_link_with`. */
type MockTarget =
  | { kind: 'web'; address: string }
  | { kind: 'heading'; anchor: string }
  | { kind: 'note'; id: string; anchor: string | null; exists: boolean }
  | { kind: 'file'; path: string; full?: boolean }
  | { kind: 'inline' }
  | { kind: 'outside' };

function mockTarget(note: string, written: string, notes: ReadonlySet<string>): MockTarget {
  // OQ-244: a picture held in the note.
  if (/^\s*data:/i.test(written)) {
    return { kind: 'inline' };
  }
  if ((written.includes('://') && !/^file:\/\//i.test(written)) || /^mailto:/i.test(written)) {
    return { kind: 'web', address: written };
  }
  // Q-63: a full `file://` place is a file of this computer; a relative
  // one is relative to the note, as any place.
  const place = written.replace(/^file:\/\//i, '');
  if (place !== written && place.startsWith('/')) {
    return { kind: 'file', path: place, full: true };
  }
  const hash = place.indexOf('#');
  const path = hash < 0 ? place : place.slice(0, hash);
  const anchor = hash < 0 ? null : place.slice(hash + 1);
  if (path === '') {
    return { kind: 'heading', anchor: anchor ?? '' };
  }
  const parts = note.includes('/') ? note.slice(0, note.lastIndexOf('/')).split('/') : [];
  for (const part of decodeURIComponent(path).split('/')) {
    if (part === '..') {
      if (parts.pop() === undefined) {
        return { kind: 'outside' };
      }
    } else if (part !== '.' && part !== '') {
      parts.push(part);
    }
  }
  const relative = parts.join('/');
  const extension = /\.([^./]+)$/.exec(relative)?.[1];
  const isNote = extension === 'md' || extension === 'adoc';
  const id = isNote ? relative.replace(/\.[^./]+$/, '') : relative;
  if (notes.has(id)) {
    return { kind: 'note', id, anchor, exists: true };
  }
  if (isNote || extension === undefined) {
    return { kind: 'note', id, anchor, exists: false };
  }
  return { kind: 'file', path: relative };
}

/** The check of a link's markup, FNV-1a over its bytes, as `LinkPlace::check_of`. */
export function mockLinkCheck(link: string): number {
  let hash = 0x811c9dc5;
  for (const byte of new TextEncoder().encode(link)) {
    hash = Math.imul(hash ^ byte, 0x01000193) >>> 0;
  }
  return hash;
}

/** The bytes of `text` before UTF-16 unit `at` (the backend counts bytes). */
function bytesBefore(text: string, at: number): number {
  return new TextEncoder().encode(text.slice(0, at)).length;
}

/**
 * FR-078, FR-079, UC-004 BR-015: a rough stand-in for the backend's lists
 * of `note` (its text of revision `rev`): the other notes of `order` (the
 * overview's) linking to it, and its own links with the broken ones
 * marked; `files` are the other files of the workspace, `all` its notes.
 */
export function mockLinks(
  note: string,
  rev: number,
  order: readonly string[],
  textOf: (note: string) => string,
  files: readonly string[],
  all: readonly string[] = order,
  mentionLimit = MOCK_MENTIONS_SHOWN,
): LinksView {
  const notes = new Set(all);
  const backlinks: BacklinkNoteView[] = [];
  for (const other of order) {
    if (other === note) {
      continue;
    }
    const text = textOf(other);
    if (!text.includes('](') && !text.includes('::')) {
      continue;
    }
    const lines = text.split('\n');
    const links = mockFoundLinks(text)
      .filter((link) => {
        const target = mockTarget(other, link.place, notes);
        return target.kind === 'note' && target.id === note;
      })
      .map((link) => {
        const lineStart = text.lastIndexOf('\n', link.at - 1) + 1;
        const line = lines[link.line - 1] ?? '';
        return {
          line: link.line,
          before: line.slice(0, link.at - lineStart).trimStart(),
          link: text.slice(link.at, link.end),
          after: line.slice(link.end - lineStart).trimEnd(),
          form: link.form,
          place: {
            from: bytesBefore(text, link.at),
            to: bytesBefore(text, link.end),
            check: mockLinkCheck(text.slice(link.at, link.end)),
          },
        };
      });
    if (links.length > 0) {
      backlinks.push({
        note: other,
        title: mockTitle(other),
        folder: other.includes('/') ? other.slice(0, other.lastIndexOf('/')) : null,
        links,
      });
    }
  }
  const own = textOf(note);
  const anchors = (id: string) =>
    new Set(mockHeadings(id === note ? own : textOf(id)).map((h) => h.anchor));
  const outgoing: OutgoingLinkView[] = mockFoundLinks(own).map((link) => {
    const target = mockTarget(note, link.place, notes);
    const base = {
      text: link.text || link.place,
      line: link.line,
      offset: link.at,
      form: link.form,
    };
    switch (target.kind) {
      case 'web':
        return { ...base, target: target.address, kind: 'web', broken: false };
      case 'heading':
        return {
          ...base,
          target: `#${target.anchor}`,
          kind: 'note',
          broken: !anchors(note).has(target.anchor),
        };
      case 'note':
        return {
          ...base,
          target: target.exists
            ? `${target.id}${target.anchor === null ? '' : `#${target.anchor}`}`
            : link.place,
          kind: 'note',
          broken:
            !target.exists || (target.anchor !== null && !anchors(target.id).has(target.anchor)),
        };
      case 'file':
        return {
          ...base,
          target: target.path,
          kind: 'file',
          broken: !target.full && !files.includes(target.path),
        };
      case 'inline':
        return { ...base, target: link.place, kind: 'inline', broken: false };
      case 'outside':
        return { ...base, target: link.place, kind: 'file', broken: false };
    }
  });
  return {
    note,
    rev,
    backlinks,
    backlinkCount: backlinks.reduce((sum, source) => sum + source.links.length, 0),
    outgoing,
    mentions: mockMentions(note, order, textOf, mentionLimit),
  };
}

/** GitHub's anchor of a heading (`heading_anchor`). */
export function mockAnchor(heading: string): string {
  return [...heading.trim().toLowerCase()]
    .map((c) => (c === ' ' ? '-' : /[\p{L}\p{N}_-]/u.test(c) ? c : ''))
    .join('');
}

/**
 * M-16.5: a rough stand-in for `indexes` over Markdown: ATX headings,
 * fenced code (a Mermaid or PlantUML block is a figure) and images alone
 * on their line. The real rules are tested in Rust.
 */
export function mockIndexes(note: string, rev: number, text: string): IndexesView {
  const outline: (IndexEntryView & { level: number })[] = [];
  const code: IndexEntryView[] = [];
  const figures: IndexEntryView[] = [];
  const lines = text.split('\n');
  let at = 0;
  let fence: { offset: number; language: string; lines: string[] } | null = null;
  for (const line of lines) {
    if (fence) {
      if (line.startsWith('```')) {
        const diagram = /^(mermaid|plantuml|puml)$/i.test(fence.language);
        const first =
          fence.lines
            .map((l) => l.trim())
            .find((l) => l !== '' && !(diagram && (l.startsWith('@') || l.startsWith('%%')))) ??
          '(empty)';
        if (diagram) {
          const name = /mermaid/i.test(fence.language) ? 'Mermaid' : 'PlantUML';
          figures.push({
            label: `${name} diagram: ${shorten(first)}`,
            depth: 0,
            offset: fence.offset,
          });
        } else {
          const label = fence.language ? `${fence.language}: ${shorten(first)}` : shorten(first);
          code.push({ label, depth: 0, offset: fence.offset });
        }
        fence = null;
      } else {
        fence.lines.push(line);
      }
    } else if (line.startsWith('```')) {
      fence = { offset: at, language: line.slice(3).trim(), lines: [] };
    } else {
      const heading = /^(#{1,6})\s+(.*)$/.exec(line);
      const image = /^!\[([^\]]*)\]\(([^)]*)\)\s*$/.exec(line);
      if (heading) {
        outline.push({
          label: shorten(heading[2]),
          depth: 0,
          offset: at,
          level: heading[1].length,
        });
      } else if (image) {
        figures.push({ label: shorten(image[1] || image[2]), depth: 0, offset: at });
      }
    }
    at += line.length + 1;
  }
  const top = Math.min(...outline.map((h) => h.level));
  return {
    note,
    rev,
    outline: outline.map(({ level, ...entry }) => ({ ...entry, depth: level - top })),
    code,
    figures,
  };
}

/**
 * Handlers answering like a window started without a workspace (M-02),
 * with their own settings and status that the commands change.
 */
/** UC-022: a place as the mock finds it. */
type MockPlace = {
  at: number;
  line: number;
  before: string;
  old: string;
  new: string;
  after: string;
};

/** FR-075: every search switch off. */
const NO_SWITCHES: SearchSwitches = { matchCase: false, wholeWord: false, regex: false };

/**
 * FR-075, UC-022 BR-002: a search text as a JavaScript expression, roughly
 * as `Query::new`: one text (`oneText`) or one word, a regular expression
 * with the switch, whole words by letters and digits; what is wrong as the
 * Rust side says it, roughly.
 */
export function mockPattern(
  text: string,
  switches: SearchSwitches,
): { regex: RegExp; groups: number } | { problem: string } {
  const escaped = switches.regex ? text : text.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  const bounded = switches.wholeWord
    ? `(?<![\\p{L}\\p{N}])(?:${escaped})(?![\\p{L}\\p{N}])`
    : escaped;
  try {
    const regex = new RegExp(bounded, `gmu${switches.matchCase ? '' : 'i'}`);
    const groups = (new RegExp(`${escaped}|`, 'u').exec('')?.length ?? 1) - 1;
    return { regex, groups };
  } catch (error) {
    const why = error instanceof Error ? error.message.replace(/^.*: /, '') : String(error);
    return { problem: `Not a valid regular expression: ${why.toLowerCase()}.` };
  }
}

/** UC-022 BR-003: the replacement as JavaScript writes it; what is wrong, roughly. */
export function mockReplacement(
  typed: string,
  switches: SearchSwitches,
  groups: number,
): { with: string } | { problem: string } {
  if (!switches.regex) {
    return { with: typed.replaceAll('$', '$$$$') };
  }
  for (const match of typed.matchAll(/\$(\$|\d)/g)) {
    const n = Number(match[1]);
    if (match[1] !== '$' && n > groups) {
      return { problem: `The replacement names $${n}, but the expression has no group ${n}.` };
    }
  }
  return { with: typed.replace(/\$0/g, '$$&') };
}

/**
 * FR-075, UC-008 BR-004: a note found with a search switch, roughly as
 * `found_lines`: the lines holding a place, the first five shown with the
 * places marked.
 */
function mockFoundByPattern(
  note: string,
  title: string,
  folder: string | null,
  text: string,
  regex: RegExp,
): FoundNoteView | null {
  const lines: FoundLineView[] = [];
  let places = 0;
  let start = 0;
  for (const line of text.split('\n')) {
    const hits = [...line.matchAll(regex)].filter((m) => m[0] !== '');
    if (hits.length > 0) {
      places += 1;
      if (lines.length < 5) {
        const parts: FoundLineView['parts'] = [];
        let last = 0;
        for (const hit of hits) {
          if (hit.index > last) parts.push({ text: line.slice(last, hit.index), hit: false });
          parts.push({ text: hit[0], hit: true });
          last = hit.index + hit[0].length;
        }
        if (last < line.length) parts.push({ text: line.slice(last), hit: false });
        const from = bytes(text.slice(0, start + hits[0].index));
        lines.push({ parts, place: { from, to: from + bytes(hits[0][0]) } });
      }
    }
    start += line.length + 1;
  }
  if (places === 0) {
    return null;
  }
  return {
    note,
    title,
    folder,
    places: places === 1 ? '1 place' : `${places} places`,
    lines,
    more: Math.max(0, places - lines.length),
    place: lines[0]?.place ?? null,
  };
}

/** F9, UC-008 BR-002: the words of a search text, roughly as `Query::parse`. */
export function mockQuery(text: string): string[] {
  const words: string[] = [];
  for (const match of text.matchAll(/"([^"]*)"?|(\S+)/g)) {
    const word = (match[1] ?? match[2] ?? '').toLowerCase();
    if (word !== '' && !words.includes(word)) {
      words.push(word);
    }
  }
  return words;
}

/**
 * UC-008 BR-012, roughly: the tags of `text` in lower case: those of a line
 * `tags:` or `:tags:` and the `#tag` of the text; the real rules (code,
 * links, front matter) are tested in Rust.
 */
export function mockTags(text: string): string[] {
  const tags = new Set<string>();
  for (const line of text.matchAll(/^:?tags:[ \t]*(.*)$/gm)) {
    for (const item of line[1].replace(/^\[|\]$/g, '').split(',')) {
      const tag = item.trim().replace(/^["'#]+|["']+$/g, '');
      if (/^[\p{L}\p{N}_\-/]+$/u.test(tag) && /[^\d/]/.test(tag)) {
        tags.add(tag.toLowerCase());
      }
    }
  }
  for (const match of text.matchAll(/(?:^|[^\p{L}\p{N}&#])#([\p{L}\p{N}_\-/]+)/gmu)) {
    const tag = match[1].replace(/\/+$/, '');
    if (/[^\d/]/.test(tag)) {
      tags.add(tag.toLowerCase());
    }
  }
  return [...tags];
}

/** UC-008 BR-012: whether a note with `tags` carries `wanted` or a tag below it. */
function carries(tags: readonly string[], wanted: string): boolean {
  return tags.some((tag) => tag === wanted || tag.startsWith(`${wanted}/`));
}

/** The UTF-8 length of `text`. */
function bytes(text: string): number {
  return new TextEncoder().encode(text).length;
}

/** The web offset (UTF-16) of the UTF-8 offset `byte` of `text`, clamped. */
export function webAtByte(text: string, byte: number): number {
  let at = 0;
  let seen = 0;
  for (const char of text) {
    if (seen >= byte) {
      break;
    }
    seen += bytes(char);
    at += char.length;
  }
  return at;
}

/** F9, UC-008 BR-004, A5: a found note, roughly as `found_lines`. */
function mockFoundNote(
  note: string,
  title: string,
  folder: string | null,
  text: string,
  words: readonly string[],
): FoundNoteView {
  const lines: FoundLineView[] = [];
  let places = 0;
  let start = 0;
  for (const line of text.split('\n')) {
    const low = line.toLowerCase();
    const at = words
      .map((w) => [low.indexOf(w), w.length] as const)
      .filter(([i]) => i >= 0)
      .sort((a, b) => a[0] - b[0]);
    if (at.length > 0) {
      places += 1;
      if (lines.length < 5) {
        const [first, length] = at[0];
        const from = Math.max(0, first - 40);
        const to = Math.min(line.length, from + 120);
        const shown = `${from > 0 ? '…' : ''}${line.slice(from, to)}${to < line.length ? '…' : ''}`;
        const parts: FoundLineView['parts'] = [];
        const lower = shown.toLowerCase();
        let last = 0;
        for (let i = 0; i < shown.length;) {
          const word = words.find((w) => lower.startsWith(w, i));
          if (word) {
            if (i > last) parts.push({ text: shown.slice(last, i), hit: false });
            parts.push({ text: shown.slice(i, i + word.length), hit: true });
            i += word.length;
            last = i;
          } else {
            i += 1;
          }
        }
        if (last < shown.length) parts.push({ text: shown.slice(last), hit: false });
        const byte = start + bytes(line.slice(0, first));
        lines.push({
          parts,
          place: { from: byte, to: byte + bytes(line.slice(first, first + length)) },
        });
      }
    }
    start += bytes(line) + 1;
  }
  return {
    note,
    title,
    folder,
    places: places === 0 ? 'in the title or folder' : places === 1 ? '1 place' : `${places} places`,
    lines,
    more: Math.max(places - lines.length, 0),
    place: lines[0]?.place ?? null,
  };
}

/** UC-026 A1: as the backend says it (`session/snapshots.rs`, `NO_SNAPSHOT`). */
export const MOCK_NO_SNAPSHOT = 'This note has no snapshot yet. Underline takes one:';

/** UC-026 A1, BR-001: as the backend says it (`SNAPSHOTS_TAKEN`). */
export const MOCK_SNAPSHOTS_TAKEN = [
  'before you change the note, at most once every 10 minutes',
  'before another program changes it while it is open',
  'before an agent changes it',
  'before a replace in the workspace changes it',
  'before git changes it: a pull, a switch of branch, a resolved conflict',
  'before a snapshot of it is restored',
] as const;

/** UC-026 BR-002: as the backend says it (`SNAPSHOTS_KEPT`). */
export const MOCK_SNAPSHOTS_KEPT = 'Kept on this computer for 30 days';

/** UC-026 BR-001: why a snapshot was taken (entity SNAPSHOT.cause). */
export type MockCause =
  'Editing' | 'OutsideChange' | 'AgentChange' | 'ReplaceInWorkspace' | 'Git' | 'Restore';

/**
 * A cause by its key (`OutsideChange`), its name in the specification
 * (`Outside change`) or its label (`Before an outside change`); an unknown
 * one is refused.
 */
export function mockCause(cause: string): MockCause {
  const plain = (text: string) => text.toLowerCase().replace(/[^a-z]/g, '');
  const keys = Object.keys(MOCK_CAUSE_LABELS) as MockCause[];
  const found = keys.find(
    (key) =>
      plain(key) === plain(cause) ||
      plain(MOCK_CAUSE_LABELS[key]) === plain(cause) ||
      (key === 'ReplaceInWorkspace' && plain(cause) === 'replaceinworkspace'),
  );
  if (found === undefined) {
    throw new Error(`No snapshot cause ${cause}; one of ${keys.join(', ')}`);
  }
  return found;
}

/** UC-026 step 2: the causes as the list names them (`SnapshotCause::label`). */
export const MOCK_CAUSE_LABELS: Record<MockCause, string> = {
  Editing: 'Before editing',
  OutsideChange: 'Before an outside change',
  AgentChange: "Before an agent's change",
  ReplaceInWorkspace: 'Before a replace in the workspace',
  Git: 'Before a git change',
  Restore: 'Before a restore',
};

/** The lines of `old` and `new` and which of them are the same, by the longest common lines. */
function mockLineOps(
  old: string,
  now: string,
): { kind: DiffLineKind; text: string; a: number | null; b: number | null }[] {
  const a = old.split('\n');
  const b = now.split('\n');
  if (a.at(-1) === '') a.pop();
  if (b.at(-1) === '') b.pop();
  const common: number[][] = Array.from({ length: a.length + 1 }, () =>
    new Array<number>(b.length + 1).fill(0),
  );
  for (let i = a.length - 1; i >= 0; i--) {
    for (let j = b.length - 1; j >= 0; j--) {
      common[i][j] =
        a[i] === b[j] ? common[i + 1][j + 1] + 1 : Math.max(common[i + 1][j], common[i][j + 1]);
    }
  }
  const ops: { kind: DiffLineKind; text: string; a: number | null; b: number | null }[] = [];
  let i = 0;
  let j = 0;
  while (i < a.length || j < b.length) {
    if (i < a.length && j < b.length && a[i] === b[j]) {
      ops.push({ kind: 'same', text: a[i], a: ++i, b: ++j });
    } else if (j < b.length && (i >= a.length || common[i][j + 1] >= common[i + 1][j])) {
      ops.push({ kind: 'added', text: b[j], a: null, b: ++j });
    } else {
      ops.push({ kind: 'removed', text: a[i], a: ++i, b: null });
    }
  }
  return ops;
}

/** UC-026 step 2: how many lines differ, a changed line once (`lines_differing`). */
export function mockLinesDiffering(old: string, now: string): number {
  let removed = 0;
  let added = 0;
  let total = 0;
  for (const op of mockLineOps(old, now)) {
    if (op.kind === 'removed') {
      removed += 1;
    } else if (op.kind === 'added') {
      added += 1;
    } else {
      total += Math.max(removed, added);
      removed = 0;
      added = 0;
    }
  }
  return total + Math.max(removed, added);
}

/** UC-026 A7: "12.4 MB in 1,203 snapshots", roughly as `format_size` and `thousands` write it. */
function mockSnapshotsAmount(all: readonly { text: string }[]): string {
  const bytes = all.reduce((sum, s) => sum + new TextEncoder().encode(s.text).length, 0);
  const size =
    bytes < 1024
      ? `${bytes} bytes`
      : bytes < 1024 * 1024
        ? `${Math.round(bytes / 1024)} KB`
        : `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  const count = all.length === 1 ? '1 snapshot' : `${all.length.toLocaleString('en-US')} snapshots`;
  return `${size} in ${count}`;
}

/** UC-026 A7, OQ-294: what the settings window shows of the snapshots `all`. */
function mockSnapshotsSpace(all: readonly { text: string }[]): SnapshotsSpaceView {
  const amount = mockSnapshotsAmount(all);
  return {
    summary: all.length === 0 ? 'None' : amount,
    kept: 'Kept on this computer for 30 days',
    any: all.length > 0,
    question:
      `Remove all snapshots of this workspace (${amount})? Earlier versions of its notes ` +
      'can no longer be restored. This cannot be undone.',
    problem: null,
  };
}

/** UC-026 step 4: the differences as one part, with every line. */
function mockHunks(old: string, now: string): HunkView[] {
  const number = (n: number | null) => (n === null ? '    ' : String(n).padStart(4));
  const sign: Record<DiffLineKind, string> = { same: ' ', removed: '-', added: '+' };
  return [
    {
      lines: mockLineOps(old, now).map((op) => ({
        kind: op.kind,
        numbers: `${number(op.a)} ${number(op.b)} ${sign[op.kind]} `,
        parts: [{ text: op.text, strong: false }],
        only: null,
        offset: null,
      })),
    },
  ];
}

export function mockHandlers(start: MockStart = {}): MockHandlers {
  // F10: the settings files of the mock, the user's seeded from `start.settings`.
  const model = new MockSettings(userFileOf(start.settings ?? {}), PALETTES);
  let status = start.status ?? '';
  // F11: the exports' offers, error mark and busy state of the status.
  const extra: MockStatusExtra = { exports: [], error: false, busy: false, agents: [] };
  // F14: the agents of the open workspace and the section "Agents".
  const agents = new MockAgents();
  const setStatus = async (
    text: string,
    offer: StatusOffer | null = null,
    report = false,
    opens: AgentOpenView[] = [],
  ) => {
    status = text;
    if (!report) {
      extra.exports = [];
      extra.error = false;
    }
    extra.agents = opens;
    await emit('status', { text, offer, ...extra });
  };
  /** The window is told the settings in effect and what the settings window shows. */
  const sendSettings = async () => {
    await emit('settings', model.effective());
    await emit('settings-window', model.windowView('Markdown'));
    await emit('agents', agentsView(agents, model));
  };
  return {
    app_start: () => {
      const { recent, ...info } = start;
      return {
        givenDir: null,
        lastWorkspace: null,
        recentWorkspaces: recent ?? [],
        platform: 'linux',
        themes: PALETTES.map((palette) => palette.theme),
        tools: [...FAKE_TOOLS],
        inputModes: ['Normal', 'Vim', 'Nano'],
        exportKinds: FAKE_EXPORT_KINDS,
        recentCommands: [],
        ...info,
        settings: model.effective(),
        status,
        agents: agentsView(agents, model),
      };
    },
    settings_quick: async ({ key, value }) => {
      const said = model.quick(key, value);
      if (said !== null) {
        await setStatus(said);
      }
      await sendSettings();
      return null;
    },
    settings_window: () => model.windowView('Markdown'),
    settings_set: async ({ scope, key, value }) => {
      const said = model.windowChange(scope, key, value);
      if (said !== null) {
        await setStatus(said);
      }
      await emit('settings', model.effective());
      // F14: the agents entry follows the access (UC-017 A1); not awaited,
      // so the answer comes as soon as before.
      void emit('agents', agentsView(agents, model));
      return { view: model.windowView('Markdown'), problem: said };
    },
    wiki_check: async ({ url }) => {
      await setStatus(wikiStatus(url));
      return null;
    },
    ...workspaceHandlers(start, setStatus, model, sendSettings, () => status, extra, agents),
  };
}

/** F11: what the status says besides its text about exports. */
type MockStatusExtra = {
  exports: ExportOfferView[];
  error: boolean;
  busy: boolean;
  agents: AgentOpenView[];
};

/** F14: what the agents entry and the section's head show (`Session::agents_view`). */
function agentsView(agents: MockAgents, model: MockSettings) {
  const effective = model.effective();
  return agents.view(model.root, {
    user: model.user.agent_access ?? 'Off',
    effective: effective.agentAccess,
    cap: model.root === null ? null : (model.workspaces[model.root]?.agent_access ?? null),
    lowered: effective.agentAccessLowered,
  });
}

/** One note tab of the fake session. */
type FakeTab = { note: string; view: EditingViewName };

/** The workspace commands over the fake file system. */
function workspaceHandlers(
  start: MockStart,
  setStatus: (
    text: string,
    offer?: StatusOffer | null,
    report?: boolean,
    opens?: AgentOpenView[],
  ) => Promise<void>,
  model: MockSettings,
  sendSettings: () => Promise<void>,
  status: () => string = () => '',
  extra: MockStatusExtra = { exports: [], error: false, busy: false, agents: [] },
  agents: MockAgents = new MockAgents(),
): Pick<
  MockHandlers,
  | 'ws_start'
  | 'ws_recent'
  | 'ws_open'
  | 'ws_check_new'
  | 'ws_create'
  | 'tab_open'
  | 'tab_open_side'
  | 'tab_activate'
  | 'pane_focus'
  | 'split_end'
  | 'tab_close'
  | 'tab_store'
  | 'tab_set_view'
  | 'ed_viewport'
  | 'clip_read_text'
  | 'clip_write_text'
  | 'clip_read'
  | 'clip_write'
  | 'clip_store_image'
  | 'ed_embed'
  | 'ed_drop'
  | 'ed_drag'
  | 'clip_forget'
  | 'ed_note_link'
  | 'app_close_anyway'
  | 'ed_mirror'
  | 'ed_sync'
  | 'ed_resync'
  | 'app_request_close'
  | 'ed_follow_link'
  | 'link_answer'
  | 'note_new_start'
  | 'note_plan'
  | 'note_create'
  | 'note_daily_open'
  | 'note_daily_step'
  | 'note_rename_start'
  | 'note_plan_rename'
  | 'note_rename'
  | 'note_delete_start'
  | 'note_delete'
  | 'note_undo_delete'
  | 'ws_restore'
  | 'ws_restore_as'
  | 'ws_purge'
  | 'search_set'
  | 'search_page'
  | 'search_open'
  | 'links_open'
  | 'links_headings'
  | 'links_follow'
  | 'links_link_mention'
  | 'links_more_mentions'
  | 'replace_set'
  | 'replace_page'
  | 'replace_leave_out'
  | 'replace_confirm'
  | 'replace_undo'
  | 'replace_close'
  | 'snapshots_list'
  | 'snapshot_diff'
  | 'snapshot_whole'
  | 'snapshot_restore'
  | 'snapshot_undo_restore'
  | 'snapshots_close'
  | 'snapshots_space'
  | 'snapshots_remove_all'
  | 'palette_ran'
  | 'split_share'
  | 'spell_suggest'
  | 'spell_add_word'
  | 'spell_add_workspace_word'
  | 'export_start'
  | 'export_choose'
  | 'export_run'
  | 'export_open'
  | 'export_approximated'
  | 'git_choose'
  | 'git_commit_choice'
  | 'git_commit'
  | 'git_set_identity'
  | 'git_discard_start'
  | 'git_discard'
  | 'git_branches'
  | 'git_branch_name'
  | 'git_switch'
  | 'sync_request'
  | 'conflict_choose'
  | 'conflict_take_side'
  | 'conflict_keep'
  | 'conflict_abort'
  | 'todo_conflict_resolve'
  | 'agents_set_access'
  | 'agents_section'
  | 'todo_create'
  | 'todo_change'
  | 'todo_mark'
  | 'todo_delete'
  | 'todo_undo'
  | 'todo_open_link'
  | 'todo_candidates'
  | 'todo_month'
  | 'todo_list_show'
  | 'todo_list_close'
  | 'todo_list_set'
  | 'ws_switch_anyway'
  | '__hooks'
> {
  const files: Record<string, string[]> = Object.fromEntries(
    Object.entries(FAKE_FILES).map(([root, notes]) => [root, [...notes]]),
  );
  /** Package 6b: the formats of the notes created here, which may be AsciiDoc. */
  const formats = new Map<string, FormatName>();
  const formatOf = (note: string): FormatName => formats.get(note) ?? mockFormat(note);
  /** The file of `note` in the workspace, with its extension. */
  const fileOf = (note: string) => `${note}.${formatOf(note) === 'AsciiDoc' ? 'adoc' : 'md'}`;
  /** Package 6b: the local clock, `2026-11-11T08:15`; `null` for the browser's. */
  let mockNow: string | null = start.now ?? null;
  let recent = [...(start.recent ?? start.recentWorkspaces ?? [])];
  let root: string | null = null;
  /**
   * The tabs of the active pane and its active tab (UC-004 BR-018); the
   * other pane of the split view is `other` while it is shown.
   */
  let tabs: FakeTab[] = [];
  let active: string | null = null;
  /** FR-096: the active pane, and the other pane of the split view. */
  let pane: PaneName = 'left';
  let other: { tabs: FakeTab[]; active: string | null } | null = null;
  /** OQ-220: the remembered share of the left pane, in tenths of a percent, by workspace. */
  const splitShares: Record<string, number> = {};
  /** OQ-198: the commands run from the palette, the most recent first (`recentCommands`). */
  let ranCommands = [...(start.recentCommands ?? [])];
  let clock = 0;
  /** What each workspace remembers: its tabs and when notes were opened. */
  const remembered: Record<
    string,
    {
      tabs: FakeTab[];
      active: string | null;
      pane?: PaneName;
      other?: { tabs: FakeTab[]; active: string | null } | null;
    }
  > = {};
  /** Whether `note` has a tab, in either pane. */
  const isOpen = (note: string) =>
    tabs.some((tab) => tab.note === note) || !!other?.tabs.some((tab) => tab.note === note);
  /** UC-004 BR-018: the pane `to` becomes the active pane. */
  const focusPane = (to: PaneName) => {
    if (other === null || to === pane) {
      return;
    }
    const was = { tabs, active };
    ({ tabs, active } = other);
    other = was;
    pane = to;
  };
  /** UC-004 A20: the split view ends; the right tabs join the left ones. */
  const endSplit = () => {
    if (other === null) {
      return;
    }
    const keep = active;
    focusPane('left');
    for (const tab of other?.tabs ?? []) {
      if (!tabs.some((t) => t.note === tab.note)) {
        tabs.push(tab);
      }
    }
    other = null;
    pane = 'left';
    active = keep !== null && tabs.some((t) => t.note === keep) ? keep : (tabs[0]?.note ?? null);
    // OQ-279: the tab "Todos" stays, in the one pane left.
    todos.list(root).pane = 'left';
  };
  /** A pane left without tabs ends the split view (A20). */
  const settlePanes = () => {
    if (other === null) {
      return;
    }
    if (tabs.length === 0) {
      ({ tabs, active } = other);
      other = null;
      pane = 'left';
    } else if (other.tabs.length === 0) {
      other = null;
      pane = 'left';
    }
    if (other === null) {
      // OQ-279: the tab "Todos" stays, in the one pane left.
      todos.list(root).pane = 'left';
    }
  };
  /** The tab of `note` goes from the other pane too (the note is gone). */
  const forgetOther = (note: string) => {
    if (other) {
      const at = other.tabs.findIndex((tab) => tab.note === note);
      if (at >= 0) {
        other.tabs.splice(at, 1);
        if (other.active === note) {
          other.active = other.tabs[Math.min(at, other.tabs.length - 1)]?.note ?? null;
        }
      }
    }
    settlePanes();
  };
  const lastOpened: Record<string, Record<string, number>> = {};
  let started = false;

  const normal = (path: string) => (path.length > 1 ? path.replace(/\/+$/, '') : path);
  /** UC-003 BR-001; OQ-272: a page name that is exactly a date keeps its dashes. */
  const title = mockTitle;
  const folder = (note: string) =>
    note.includes('/') ? note.slice(0, note.lastIndexOf('/')) : null;
  const outerOf = (path: string) =>
    [...recent, ...(root ? [root] : []), ...Object.keys(files)]
      .filter((known) => known !== '/home/user/Letters')
      .find((known) => path.startsWith(`${known}/`));

  /** UC-002 BR-008: recently opened first, then by title and folder. */
  const byOverview =
    (opened: Record<string, number>) =>
    (a: string, b: string): number =>
      (opened[b] ?? -1) - (opened[a] ?? -1) ||
      title(a).toLowerCase().localeCompare(title(b).toLowerCase()) ||
      (folder(a) ?? '').localeCompare(folder(b) ?? '');

  const overview = (): OverviewItem[] => {
    if (root === null) {
      return [];
    }
    const opened = lastOpened[root] ?? {};
    const notes = files[root] ?? [];
    // Counted once, so a workspace of 10,000 notes (LARGE) lists quickly.
    const counts = new Map<string, number>();
    for (const n of notes) {
      const t = title(n).toLowerCase();
      counts.set(t, (counts.get(t) ?? 0) + 1);
    }
    const count = (t: string) => counts.get(t) ?? 0;
    return [...notes].sort(byOverview(opened)).map((note) => ({
      note,
      format: formatOf(note),
      label:
        count(title(note).toLowerCase()) > 1
          ? `${title(note)}  (${folder(note) ?? 'top folder'})`
          : title(note),
      title: title(note),
      folder: folder(note),
      mark: gitMark(note),
    }));
  };
  /** The texts of the open notes, and their save state. */
  const texts = new Map<string, string>();
  const unstored = new Set<string>();
  const saveErrors = new Map<string, string>();
  const saveTimers = new Map<string, ReturnType<typeof setTimeout>>();
  /** The test that made the mock ended (`dispose`). */
  let disposed = false;
  /** F15: the todos of the fake workspaces and the tab "Todos". */
  const todos = new MockTodos();
  const todoTimers = new Set<ReturnType<typeof setTimeout>>();
  /** F15: runs `work` after `ms`, unless the mock was taken down. */
  const later = (ms: number, work: () => Promise<void> | void) => {
    const timer = setTimeout(() => {
      todoTimers.delete(timer);
      if (!disposed) {
        void work();
      }
    }, ms);
    todoTimers.add(timer);
  };
  let clipboard: MockClipboard = { text: null, html: null, image: false };
  /** BR-011: per note, what the clipboard held before copy on select. */
  const before = new Map<string, MockClipboard>();
  /** How many images were pasted, for their names. */
  let pastedImages = 0;
  /** F7, UC-004 A4: what the system's program opened, and what the question asks about. */
  const opened: string[] = [];
  let openAsked: { id: number; target: string } | null = null;
  let openAsks = 0;
  /** The folder of `note` in the workspace, with a slash, or nothing. */
  const folderOf = (note: string) =>
    note.includes('/') ? note.slice(0, note.lastIndexOf('/') + 1) : '';
  /** The place of the workspace file `place` relative to the note `note`. */
  const relativeTo = (note: string, place: string) => {
    const from = folderOf(note).split('/').filter(Boolean);
    const to = place.split('/');
    let common = 0;
    while (common < from.length && common < to.length - 1 && from[common] === to[common]) {
      common += 1;
    }
    return [...from.slice(common).map(() => '..'), ...to.slice(common)].join('/');
  };
  const IMAGE = /\.(png|jpe?g|gif|svg|webp)$/i;
  /** UC-005 A9 to A13, A19: `embed_file` over the fake world. */
  const embed = async (note: string, path: string, allowLarge: boolean): Promise<EmbedAnswer> => {
    const image = IMAGE.test(path);
    if (root && path.startsWith(`${root}/`)) {
      const place = relativeTo(note, path.slice(root.length + 1));
      return { kind: 'insert', insertion: { kind: 'file', place, image } };
    }
    const size = FAKE_OUTSIDE[path];
    if (size === undefined) {
      await setStatus(`${path} cannot be read: No such file or directory (os error 2)`);
      return { kind: 'refused' };
    }
    if (size < 0) {
      await setStatus(`${path} is a folder; folders cannot be put into a note`);
      return { kind: 'refused' };
    }
    if (size > 10 * 1024 * 1024 && !allowLarge) {
      return {
        kind: 'large',
        path,
        question: `${path} has ${Math.floor(size / (1024 * 1024))} MB. Copy it into the workspace, or only link to where it is (it is then not part of the workspace and not pushed with git)?`,
      };
    }
    const place = path.split('/').at(-1) ?? path;
    return { kind: 'insert', insertion: { kind: 'file', place, image } };
  };
  const textOf = (note: string): string => texts.get(note) ?? fakeNoteText(note);
  /** UC-026: the fake snapshots of each note, newest first. */
  type MockSnapshot = { key: string; taken: number; cause: MockCause; text: string };
  const snapshotsOf = new Map<string, MockSnapshot[]>();
  let snapshotSerial = 0;
  /** UC-026 A3: the undo of the last restore, while its report shows. */
  let restoreUndo: { status: string; note: string; key: string } | null = null;
  /** UC-026 A4: the note whose snapshots are shown. */
  let snapshotsShown: string | null = null;
  /** UC-026 BR-001: a snapshot of `text`, unless the newest holds it. */
  const takeSnapshot = (
    note: string,
    text: string,
    cause: MockCause,
    taken = Date.now(),
  ): MockSnapshot => {
    const kept = snapshotsOf.get(note) ?? [];
    if (kept[0]?.text === text) {
      return kept[0];
    }
    snapshotSerial += 1;
    const made = { key: `${taken}-${snapshotSerial}.snap`, taken, cause, text };
    snapshotsOf.set(
      note,
      [made, ...kept].sort((a, b) => b.taken - a.taken),
    );
    if (snapshotsShown === note) {
      void emit('snapshots-changed', note);
    }
    return made;
  };
  const snapshotTime = (taken: number): string => {
    const at = new Date(taken);
    const two = (n: number) => String(n).padStart(2, '0');
    return `${at.getFullYear()}-${two(at.getMonth() + 1)}-${two(at.getDate())} ${two(at.getHours())}:${two(at.getMinutes())}:${two(at.getSeconds())}`;
  };
  const snapshotOf = (note: string, key: string): MockSnapshot => {
    const found = snapshotsOf.get(note)?.find((snapshot) => snapshot.key === key);
    if (!found) {
      throw {
        kind: 'invalid',
        message: `${title(note)} could not be restored: the snapshot is gone`,
      };
    }
    return found;
  };
  const noteNow = (note: string): string => mirrors.get(note)?.text() ?? textOf(note);
  /**
   * UC-026 A4: the note shown with its snapshots changed, as the backend
   * sees it on its own (its text or its file): the window asks again.
   */
  const noteTouched = (note: string) => {
    if (snapshotsShown === note) {
      void emit('snapshots-changed', note);
    }
  };
  /** UC-026 BR-003: the snapshots of deleted notes, by the deletion's key. */
  const snapshotsDeleted = new Map<string, MockSnapshot[]>();
  /** F8, UC-007: the deleted notes of each workspace, most recently deleted first. */
  const deletedNotes: Record<string, MockDeleted[]> = Object.fromEntries(
    Object.entries(FAKE_DELETED).map(([dir, notes]) => [dir, notes.map((d) => ({ ...d }))]),
  );
  /** UC-007 A4: the undo of the last deletion, while its report shows. */
  let undo: { status: string; deleted: MockDeleted; reopen: boolean } | null = null;
  /** UC-007 A6: the deleted note "Restore note" asks a place for. */
  let restoring: { deleted: MockDeleted; reopen: boolean } | null = null;
  /** UC-007 A8: the deleted notes cannot keep a note (`cannotKeep`). */
  let keepFails = false;
  /** UC-006 A7: open notes whose links a rename changed; their save keeps its report. */
  const quiet = new Set<string>();
  const deletedView = (): DeletedNoteView[] =>
    (root ? (deletedNotes[root] ?? []) : []).map((d) => ({
      key: d.key,
      title: title(d.note),
      folder: folder(d.note) ?? 'top folder',
      hover: `${folder(d.note) ?? 'top folder'} · deleted ${localTime(d.at)}`,
      deleted: localTime(d.at),
    }));
  const extension = (format: FormatName) => (format === 'AsciiDoc' ? 'adoc' : 'md');
  /** UC-003 BR-001, BR-002, roughly: the note's id, or why it cannot be. */
  const planNote = (
    noteTitle: string,
    noteFolder: string,
    except: string | null = null,
  ): { id: string } | { problem: string; existing?: string } => {
    const name = noteTitle.trim();
    if (name === '') {
      return { problem: 'The title is empty.' };
    }
    const bad = [...name].find((c) => '/\\:*?"<>|'.includes(c));
    if (bad !== undefined) {
      return { problem: `The title cannot hold the character \`${bad}\`.` };
    }
    const levels = noteFolder.split('/').filter((level) => level.trim() !== '');
    const hidden = levels.find((level) => level.startsWith('.'));
    if (hidden !== undefined) {
      return { problem: `The folder \`${hidden}\` is hidden.` };
    }
    const id = [...levels, name.split(/\s+/).join('-')].join('/');
    const existing = (root ? (files[root] ?? []) : []).find(
      (n) => n.toLowerCase() === id.toLowerCase() && n !== except,
    );
    return existing === undefined ? { id } : { problem: '', existing };
  };
  /** The note text the backend's format module writes for a new note. */
  const newNoteText = (noteTitle: string, format: FormatName = 'Markdown') =>
    `${format === 'AsciiDoc' ? '=' : '#'} ${noteTitle.trim().split(/\s+/).join(' ')}\n`;
  /** UC-025 BR-001: the date and time of the mock's clock, `2026-11-11` and `08:15`. */
  const localClock = (): { date: string; time: string } => {
    if (mockNow !== null) {
      const [date, time = '00:00'] = mockNow.split('T');
      return { date, time: time.slice(0, 5) };
    }
    const now = new Date();
    const two = (n: number) => String(n).padStart(2, '0');
    return {
      date: `${now.getFullYear()}-${two(now.getMonth() + 1)}-${two(now.getDate())}`,
      time: `${two(now.getHours())}:${two(now.getMinutes())}`,
    };
  };
  /**
   * A folder the workspace settings give `key`, as levels joined by `/`, or
   * why it cannot take a note (UC-003 BR-006, roughly); `fallback` when not given.
   */
  const settingFolder = (
    key: SettingName,
    fallback: string,
  ): { folder: string } | { problem: string } => {
    const value = (model.workspaceValue(key) ?? '').trim() || fallback;
    if (value.startsWith('/') || value.startsWith('\\')) {
      return { problem: 'the folder lies outside the workspace' };
    }
    const levels = value.split('/').map((level) => level.trim());
    const kept = levels.filter((level) => level !== '' && level !== '.');
    if (kept.includes('..')) {
      return { problem: 'the folder lies outside the workspace' };
    }
    const hidden = kept.find((level) => level.startsWith('.'));
    return hidden === undefined
      ? { folder: kept.join('/') }
      : { problem: `the folder \`${hidden}\` is hidden` };
  };
  /** UC-003 BR-008: the templates of the open workspace, by their place. */
  const templateNotes = (): string[] => {
    const place = settingFolder('template_folder', 'templates');
    if (root === null || !('folder' in place) || place.folder === '') {
      return [];
    }
    return (files[root] ?? []).filter((note) => note.startsWith(`${place.folder}/`)).sort();
  };
  model.templatesOf = () => templateNotes().map((note) => `${note}.${extension(formatOf(note))}`);
  /** UC-003 BR-008, a rough stand-in for `fill_template`: the text and the cursor's place. */
  const fillTemplate = (
    text: string,
    noteTitle: string,
    date: string,
    time: string,
  ): { text: string; cursor: number | null } => {
    let cursor: number | null = null;
    let out = '';
    // OQ-275: a line of only {{cursor}} after an empty line replaces it.
    const lines: string[] = [];
    for (const line of text.match(/[^\n]*\n|[^\n]+$/g) ?? []) {
      const before = lines.at(-1);
      if (line.replace(/\r?\n$/, '') === '{{cursor}}' && before?.replace(/\r?\n$/, '') === '') {
        lines.pop();
      }
      lines.push(line);
    }
    let rest = lines.join('');
    const values: Record<string, string> = {
      '{{title}}': noteTitle.trim().split(/\s+/).join(' '),
      '{{date}}': date,
      '{{time}}': time,
      '{{cursor}}': '',
    };
    while (rest !== '') {
      const placeholder = Object.keys(values).find((key) => rest.startsWith(key));
      if (placeholder === undefined) {
        out += rest[0];
        rest = rest.slice(1);
        continue;
      }
      if (placeholder === '{{cursor}}') {
        cursor ??= out.length;
      }
      out += values[placeholder];
      rest = rest.slice(placeholder.length);
    }
    return { text: out, cursor };
  };
  /** A new note `id` holding `text` in `format`, opened in a tab; its mirror's revision. */
  const addNote = async (
    id: string,
    text: string,
    format: FormatName,
    side = false,
  ): Promise<number> => {
    if (root !== null) {
      files[root] = [...(files[root] ?? []), id];
    }
    formats.set(id, format);
    texts.set(id, text);
    await loadEditorCore();
    const mirror = new MirrorCore(text);
    mirrors.set(id, mirror);
    if (side) {
      // UC-004 A18: offered by a link followed to the side.
      openToSide(id);
    } else {
      openTab(id);
    }
    await sendAll();
    return mirror.rev;
  };
  /** UC-006 BR-003, roughly: the links to `from` in `text`, as written from `note`. */
  const linkTo = (note: string, target: string) => `](${relativeTo(note, `${target}.md`)}`;
  const countLinks = (text: string, needle: string) => text.split(needle).length - 1;
  /**
   * UC-006 BR-003, OQ-244, roughly: the markup of each link of `other` to
   * `target` (a link, an include, a picture), and as it reads for `to`.
   */
  const linkForms = (other: string, target: string, to: string): [string, string][] => {
    const ext = formatOf(target) === 'AsciiDoc' ? 'adoc' : 'md';
    const place = (id: string) => relativeTo(other, `${id}.${ext}`);
    return [
      [linkTo(other, target), linkTo(other, to)],
      ...['include::', 'image::', 'image:'].map((macro): [string, string] => [
        `${macro}${place(target)}[`,
        `${macro}${place(to)}[`,
      ]),
    ];
  };
  const countAll = (text: string, forms: [string, string][]) =>
    forms.reduce((sum, [needle]) => sum + countLinks(text, needle), 0);
  /** M-22.2: the remembered tabs and "last opened" follow a renamed note. */
  const renameEverywhere = (from: string, to: string) => {
    if (!root) {
      return;
    }
    files[root] = (files[root] ?? []).map((n) => (n === from ? to : n));
    tabs = tabs.map((tab) => (tab.note === from ? { ...tab, note: to } : tab));
    if (active === from) {
      active = to;
    }
    if (other) {
      other.tabs = other.tabs.map((tab) => (tab.note === from ? { ...tab, note: to } : tab));
      if (other.active === from) {
        other.active = to;
      }
    }
    const opened = lastOpened[root] ?? {};
    if (from in opened) {
      opened[to] = opened[from];
      delete opened[from];
    }
    for (const map of [texts, mirrors, saveErrors] as Map<string, unknown>[]) {
      if (map.has(from)) {
        map.set(to, map.get(from));
        map.delete(from);
      }
    }
    for (const set of [unstored, quiet]) {
      if (set.delete(from)) {
        set.add(to);
      }
    }
    remember();
  };
  /** UC-007 A4, A5: puts `deleted` back, or asks for another place (A6). */
  const restore = async (
    deleted: MockDeleted,
    reopen: boolean,
    place: { title: string; folder: string } | null,
  ): Promise<RestoreAnswer> => {
    if (!root) {
      return { kind: 'done' };
    }
    const wanted = place ?? { title: title(deleted.note), folder: folder(deleted.note) ?? '' };
    const planned = planNote(wanted.title, wanted.folder);
    if (!('id' in planned)) {
      if (planned.existing === undefined && place === null) {
        await setStatus(`The note could not be restored: ${planned.problem}`);
        return { kind: 'done' };
      }
      restoring = { deleted, reopen };
      return {
        kind: 'ask',
        title: wanted.title,
        folder: wanted.folder,
        problem:
          planned.existing === undefined
            ? planned.problem
            : `A note named ${title(planned.existing)} exists in that folder. Choose another title or folder.`,
      };
    }
    files[root] = [...(files[root] ?? []), planned.id];
    texts.set(planned.id, deleted.text);
    // UC-026 BR-003: its snapshots come back with it.
    const kept = snapshotsDeleted.get(deleted.key);
    if (kept) {
      snapshotsOf.set(planned.id, kept);
      snapshotsDeleted.delete(deleted.key);
    }
    deletedNotes[root] = (deletedNotes[root] ?? []).filter((d) => d !== deleted);
    restoring = null;
    undo = null;
    if (reopen) {
      openTab(planned.id);
    }
    await sendAll();
    await setStatus(`Restored ${title(planned.id)}`);
    return { kind: 'restored', note: planned.id };
  };
  /** ADR-025: the backend's mirror of each note a window's core holds. */
  const mirrors = new Map<string, MirrorCore>();
  /**
   * FR-099, ADR-030: the spell check of the mock: English (US) only; its
   * dictionary knows the words of the mock's notes but the misspelled ones
   * of the spell workspace.
   */
  const speller = new MockSpeller(Object.values(SPELL_TEXTS));
  /** The languages whose missing dictionary was said. */
  const spellSaid = new Set<string>();
  /** What the status bar would show of the spell check while each note is active. */
  const spellStates = new Map<string, { problem: string | null; detail: string | null }>();
  /** OQ-207: the status bar shows the spell state of the active note, told only when it changes. */
  const showSpellState = async () => {
    const state = (active !== null ? spellStates.get(active) : undefined) ?? spellShown;
    if (state.problem !== spellShown.problem || state.detail !== spellShown.detail) {
      spellShown = state;
      await emit('spell-state', state);
    }
  };
  /** What the status bar was told of the spell check last. */
  let spellShown: { problem: string | null; detail: string | null } = {
    problem: null,
    detail: null,
  };
  /** The checks waiting for the text to rest (`SPELL_DEBOUNCE`), by note. */
  const spellTimers = new Map<string, ReturnType<typeof setTimeout>>();
  /** UC-005 BR-023: the marks of `note` for its mirror's revision (`spell-marks`). */
  const sendSpell = async (note: string) => {
    const mirror = mirrors.get(note);
    if (!mirror || disposed) {
      return;
    }
    const asciidoc = mockFormat(note) === 'AsciiDoc';
    const setting = model.spellLanguage();
    // OQ-207: the note's attribute `lang` overrides the setting, unless off.
    const named = setting === 'off' ? null : noteLanguage(mirror.text(), asciidoc);
    const language = named === null ? setting : (offeredLanguage(named) ?? named);
    // FR-100, OQ-207: what the status bar shows, as `shown_spell_state` of
    // the session: the note's own problem while it names its language, else
    // that of the setting; told again whenever it changes, `null` too.
    const missing = (code: string) => code !== 'off' && code !== MOCK_SPELL_LANGUAGE;
    let state: { problem: string | null; detail: string | null } = { problem: null, detail: null };
    // A note naming the setting's own language counts as the setting's.
    if (named !== null && language !== setting) {
      if (missing(language)) {
        const name = spellLanguageName(language);
        state = {
          problem: `${title(note)}: no dictionary for ${name}`,
          detail: `No dictionary for ${name}, the language this note names in its attribute \`lang\`. Change or remove the attribute.`,
        };
      }
    } else if (missing(setting)) {
      const name = spellLanguageName(setting);
      const detail = `No dictionary for ${name}. Choose another language in the settings.`;
      state = { problem: `Spell check: ${name}, no dictionary`, detail };
      if (!spellSaid.has(setting)) {
        spellSaid.add(setting);
        await setStatus(detail);
      }
    }
    spellStates.set(note, state);
    await showSpellState();
    const marks = language === MOCK_SPELL_LANGUAGE ? speller.marks(mirror.text(), asciidoc) : [];
    await emit('spell-marks', { note, rev: mirror.rev, marks });
  };
  /** ADR-030: a note is checked once its text rested 250 ms. */
  const spellLater = (note: string) => {
    clearTimeout(spellTimers.get(note));
    spellTimers.set(
      note,
      setTimeout(() => {
        spellTimers.delete(note);
        void sendSpell(note);
      }, 250),
    );
  };
  /** Every note a core holds is checked again (another language, a word added). */
  const spellAll = () => {
    for (const note of mirrors.keys()) {
      void sendSpell(note);
    }
  };
  model.onChange = (key) => {
    if (key === 'spell_check_language') {
      spellSaid.clear();
      spellStates.clear();
      spellShown = { problem: null, detail: null };
      void emit('spell-state', spellShown);
      setTimeout(spellAll, 0);
    }
  };
  /** The mirror's text is the note's now. */
  const followMirror = (note: string, mirror: MirrorCore) => {
    texts.set(note, mirror.text());
    noteTouched(note);
  };
  /**
   * ADR-025: the backend's colours and figures of the facade's blocks
   * (`ed-enrich`), from the fake blocks' figures; no code colours.
   */
  const sendEnrich = async (note: string) => {
    const mirror = mirrors.get(note);
    if (!mirror || disposed) {
      return;
    }
    const images: Record<string, FigureView> = {};
    let diagram: FigureView | null = null;
    for (const block of fakeBlocks(mirror.text())) {
      if (block.kind.type === 'image' && block.figure) {
        images[block.kind.target] = { ...block.figure, caption: null };
      } else if (block.kind.type === 'code' && block.figure) {
        diagram = block.figure;
      }
    }
    const looks = model.effective();
    const sent = mirror.enrichment(
      note,
      mockFormat(note) === 'Markdown',
      looks.theme,
      looks.palette.dark,
      images,
      diagram ?? {
        picture: null,
        message: null,
        failed: false,
        sourceShown: false,
        loading: null,
        unreadable: null,
        caption: null,
      },
      MOCK_FORMULA,
    );
    try {
      await emit('ed-enrich', { ...sent, clear: true });
    } catch {
      // The mock was taken down meanwhile (a test ended).
    }
  };
  /** F12: the fake git of each git workspace (UC-011, UC-012). */
  type MockOther = { path: string; kind: 'new' | 'changed' | 'deleted'; text: string | null };
  type MockGit = {
    committed: Map<string, string>;
    others: MockOther[];
    messages: string[];
  };
  const gits: Record<string, MockGit> = {};
  let gitIdentity = false;
  let gitIdentityFails: string | null = null;
  let gitCommitFails: string | null = null;
  let gitMerging = false;
  const gitLeftOut = new Set<string>();
  let gitChosen: string | null = null;
  let gitDiscarding: string | null = null;
  let gitTimer: ReturnType<typeof setTimeout> | null = null;
  /** The fake repository of the open workspace, if it is one. */
  const gitOf = (): MockGit | null => {
    if (root === TEAM) {
      // F13: everything committed.
      gits[root] ??= {
        committed: new Map((FAKE_FILES[TEAM] ?? []).map((note) => [note, fakeNoteText(note)])),
        others: [],
        messages: ['Plan the quarter'],
      };
      return gits[root];
    }
    if (root !== '/home/user/Work') {
      return null;
    }
    gits[root] ??= {
      committed: new Map([
        ...(FAKE_FILES['/home/user/Work'] ?? [])
          .filter((note) => note !== 'Projects/Roadmap')
          .map((note): [string, string] => [
            note,
            note === 'README'
              ? fakeNoteText(note).replace('Type here to try', 'Type to try')
              : fakeNoteText(note),
          ]),
        ['Old-Minutes', '# Old Minutes\n\nThe minutes of last year.\n'],
      ]),
      others: [{ path: 'assets/logo.png', kind: 'new', text: null }],
      messages: ['Add the meeting notes'],
    };
    return gits[root];
  };
  type MockChange = {
    path: string;
    kind: 'new' | 'changed' | 'deleted' | 'renamed';
    note: string | null;
    from: string | null;
    text: string | null;
  };
  /** UC-011 steps 1, 4: the changes since the fake commit, notes first (BR-002). */
  const gitChanges = (): MockChange[] => {
    const git = gitOf();
    if (!git || !root) {
      return [];
    }
    const notes = files[root] ?? [];
    const changes: MockChange[] = [];
    for (const note of notes) {
      const now = mirrors.get(note)?.text() ?? textOf(note);
      const was = git.committed.get(note);
      if (was === undefined || was !== now) {
        changes.push({
          path: `${note}.md`,
          kind: was === undefined ? 'new' : 'changed',
          note,
          from: null,
          text: now,
        });
      }
    }
    for (const [note, was] of git.committed) {
      if (notes.includes(note)) {
        continue;
      }
      // A note deleted and one new with its text are one note renamed.
      const renamed = changes.find((c) => c.kind === 'new' && c.text === was);
      if (renamed) {
        renamed.kind = 'renamed';
        renamed.from = note;
      } else {
        changes.push({ path: `${note}.md`, kind: 'deleted', note, from: null, text: null });
      }
    }
    changes.sort((a, b) => a.path.toLowerCase().localeCompare(b.path.toLowerCase()));
    return [...changes, ...git.others.map((o) => ({ ...o, note: null, from: null }))];
  };
  const LETTERS = { new: 'N', changed: 'M', deleted: 'D', renamed: 'R' } as const;
  /** M-05.3: the letter of `note` in the overview. */
  const gitMark = (note: string): string | null => {
    if (!gitOf()) {
      return null;
    }
    const change = gitChanges().find((c) => c.note === note && c.kind !== 'deleted');
    return change ? LETTERS[change.kind] : unstored.has(note) ? 'M' : null;
  };
  const changeName = (c: MockChange) =>
    c.note === null
      ? c.path
      : c.kind === 'renamed'
        ? `${title(c.note)} ← ${title(c.from ?? '')}`
        : folder(c.note) !== null
          ? `${title(c.note)} · ${folder(c.note)}`
          : title(c.note);
  const changesView = (): ChangesView | null => {
    const git = gitOf();
    if (!git) {
      return null;
    }
    const changes = gitChanges();
    for (const left of [...gitLeftOut]) {
      if (!changes.some((c) => c.path === left)) {
        gitLeftOut.delete(left);
      }
    }
    const conflicted = new Set(remoteOf()?.conflicts.map((c) => c.path) ?? []);
    // UC-015 BR-005: the conflicts first.
    const conflictRows: ChangeView[] = (remoteOf()?.conflicts ?? []).map((c) => ({
      path: c.path,
      kind: 'conflict',
      letter: 'C',
      name:
        c.note === null ? c.path : changeName({ ...c, kind: 'changed', from: null, text: null }),
      note: c.note !== null,
      chosen: true,
      deletedOn: c.deletedOn,
    }));
    const rows: ChangeView[] = [
      ...conflictRows,
      ...changes
        .filter((c) => !conflicted.has(c.path))
        .map((c) => ({
          path: c.path,
          kind: c.kind,
          letter: LETTERS[c.kind],
          name: changeName(c),
          note: c.note !== null,
          chosen: !gitLeftOut.has(c.path),
          deletedOn: null,
        })),
    ];
    const proposal: MockProposalChange[] = changes
      .filter((c) => !gitLeftOut.has(c.path))
      .map((c) => ({
        kind: c.kind,
        title: c.note === null ? null : title(c.note),
        from: c.from === null ? null : title(c.from),
      }));
    const problem = remoteOf()?.problem ?? null;
    if (problem !== null) {
      return {
        problem,
        branch: null,
        files: [],
        merging: false,
        amendable: null,
        proposed: '',
      };
    }
    return {
      problem: null,
      branch: { label: remoteOf()?.current ?? 'main', detached: false },
      files: rows,
      merging: gitMerging || conflictRows.length > 0,
      amendable: git.messages[0] ?? null,
      proposed: mockProposedMessage(proposal),
    };
  };
  /** M-19.3: the diff of the chosen file, else of the active note. */
  const diffView = (): DiffView | null => {
    const git = gitOf();
    if (!git) {
      return null;
    }
    const changes = gitChanges();
    const chosen = changes.find((c) => c.path === gitChosen);
    if (chosen && chosen.note !== active) {
      if (chosen.note === null && chosen.text === null) {
        const kind = chosen.kind === 'renamed' ? 'changed' : chosen.kind;
        return {
          path: chosen.path,
          note: null,
          rev: null,
          body: { kind: 'notText', text: `This file is not text; it is ${kind}.` },
        };
      }
      const was = chosen.note === null ? '' : (git.committed.get(chosen.note) ?? '');
      const hunks = mockDiff(was, chosen.text ?? '');
      return {
        path: chosen.path,
        note: null,
        rev: null,
        body: hunks.length === 0 ? { kind: 'unchanged' } : { kind: 'hunks', hunks },
      };
    }
    if (active === null) {
      return null;
    }
    const change = changes.find((c) => c.note === active);
    const was = git.committed.get(change?.from ?? active) ?? '';
    const mirror = mirrors.get(active);
    const hunks = mockDiff(was, mirror?.text() ?? textOf(active), 3, true);
    return {
      path: `${active}.md`,
      note: active,
      rev: mirror?.rev ?? 1,
      body: hunks.length === 0 ? { kind: 'unchanged' } : { kind: 'hunks', hunks },
    };
  };
  /** The window is told the changes and the diff, as the backend's tick does. */
  const sendGit = async () => {
    if (disposed) {
      return;
    }
    try {
      await emit('changes', changesView());
      await emit('diff', diffView());
      await emit('git-bar', gitBarView());
      await emit('conflict', conflictBarView());
      await emit('todo-conflicts', remoteOf()?.todos ?? []);
    } catch {
      // The mock was taken down meanwhile (a test ended).
    }
  };
  /** F15: the todos, as the backend's tick sends them. */
  const sendTodos = async () => {
    if (disposed || root === null) {
      return;
    }
    const list = todos.list(root);
    try {
      await emit(
        'todos',
        todos.view(
          root,
          files[root] ?? [],
          list.open && list.active && list.pane === pane ? null : active,
        ),
      );
    } catch {
      // The mock was taken down meanwhile (a test ended).
    }
  };
  /** F15: what a todo command did: the status with its offer, and the todos. */
  const todoDone = async (done: {
    answer: TodoAnswer;
    status?: string;
    offer?: 'delete' | 'mark';
  }): Promise<TodoAnswer> => {
    if (done.status !== undefined) {
      const offer =
        done.offer === 'delete' ? 'undoTodoDelete' : done.offer === 'mark' ? 'undoTodoMark' : null;
      await setStatus(done.status, offer);
    }
    await sendTodos();
    return done.answer;
  };
  /** UC-011 A3: a text changed; the changes and the diff follow soon. */
  const gitAgain = () => {
    if (gitTimer !== null || !gitOf()) {
      return;
    }
    gitTimer = setTimeout(() => {
      gitTimer = null;
      void checkResolved();
      void sendGit();
      void emit('overview', overview()).catch(() => undefined);
    }, 50);
  };
  /** UC-012 steps 5–7, A1 to A6: the fake commit. */
  const gitCommit = async (message: string, amend: boolean): Promise<CommitAnswer> => {
    const git = gitOf();
    if (!git || !root || gitMerging) {
      return { kind: 'done' };
    }
    const text = message.trim();
    if (text === '') {
      return { kind: 'problem', problem: 'A commit needs a message.' };
    }
    const chosen = gitChanges().filter((c) => !gitLeftOut.has(c.path));
    const amending = amend && git.messages.length > 0;
    if (chosen.length === 0 && !amending) {
      return { kind: 'problem', problem: 'Nothing is chosen.' };
    }
    // Step 5, A5: the open notes are stored first.
    const failures = tabs.map((tab) => store(tab.note)).filter((f): f is string => f !== null);
    if (failures.length > 0) {
      await sendAll();
      await setStatus(`Nothing was committed, because ${failures.join('; ')}`);
      return { kind: 'done' };
    }
    if (gitCommitFails !== null) {
      await setStatus(`The commit failed: \`git commit\` failed: ${gitCommitFails}`);
      return { kind: 'done' };
    }
    if (!gitIdentity) {
      return { kind: 'identity' };
    }
    for (const change of chosen) {
      if (change.note === null) {
        git.others = git.others.filter((o) => o.path !== change.path);
      } else if (change.kind === 'deleted') {
        git.committed.delete(change.note);
      } else {
        git.committed.set(change.note, change.text ?? '');
        if (change.from !== null) {
          git.committed.delete(change.from);
        }
      }
    }
    if (amending) {
      git.messages[0] = text;
    } else {
      git.messages.unshift(text);
    }
    const id = (0xa1b2c00 + git.messages.length * 17 + (amending ? 1 : 0)).toString(16).slice(0, 7);
    const notes = chosen.filter((c) => c.note !== null).length;
    const others = chosen.length - notes;
    const held = [
      notes > 0 ? (notes === 1 ? '1 note' : `${notes} notes`) : null,
      others > 0 ? (others === 1 ? '1 file' : `${others} files`) : null,
    ].filter((part) => part !== null);
    const holding = held.length > 0 ? ` holding ${held.join(' and ')}` : '';
    gitLeftOut.clear();
    await sendAll();
    await setStatus(
      amending ? `Amended the last commit, now ${id}${holding}` : `Committed ${id}${holding}`,
    );
    // F13, UC-014 BR-001: a commit is synced right after.
    void runSync('full', false);
    return { kind: 'committed' };
  };
  /** UC-011 A6 steps 2–3: the fake discard. */
  const gitDiscard = async (path: string) => {
    const git = gitOf();
    const change = gitChanges().find((c) => c.path === path);
    if (!git || !root || !change || change.note === null) {
      return;
    }
    const note = change.note;
    const closeTabOf = (gone: string) => {
      const at = tabs.findIndex((tab) => tab.note === gone);
      if (at >= 0) {
        clearTimeout(saveTimers.get(gone));
        tabs.splice(at, 1);
        if (active === gone) {
          active = tabs[Math.min(at, tabs.length - 1)]?.note ?? null;
        }
      }
      forgetOther(gone);
      for (const map of [texts, mirrors, saveErrors] as Map<string, unknown>[]) {
        map.delete(gone);
      }
      unstored.delete(gone);
      remember();
    };
    if (change.kind === 'new') {
      // A new note goes to the deleted notes.
      clock += 1;
      deletedNotes[root] = [
        { key: `${Date.now()}:${clock}:${note}`, note, text: change.text ?? '', at: Date.now() },
        ...(deletedNotes[root] ?? []),
      ];
      files[root] = (files[root] ?? []).filter((n) => n !== note);
      closeTabOf(note);
    } else if (change.kind === 'renamed' && change.from !== null) {
      files[root] = (files[root] ?? []).map((n) => (n === note ? change.from! : n));
      texts.set(change.from, git.committed.get(change.from) ?? '');
      closeTabOf(note);
    } else {
      const was = git.committed.get(note) ?? '';
      if (change.kind === 'deleted') {
        files[root] = [...(files[root] ?? []), note];
      }
      texts.set(note, was);
      clearTimeout(saveTimers.get(note));
      unstored.delete(note);
      saveErrors.delete(note);
      const mirror = mirrors.get(note);
      if (mirror) {
        const rev = mirror.reload(was);
        followMirror(note, mirror);
        await emit('ed-changed', { note, rev });
      }
    }
    gitChosen = null;
    await sendAll();
    await setStatus(`Discarded the changes of ${title(note)}`);
  };

  // F13: branches, syncs and conflicts of the git workspaces (UC-013 to
  // UC-015); only Team has a remote.
  type MockConflict = { path: string; note: string | null; deletedOn: SideName | null };
  type MockRemote = {
    hasRemote: boolean;
    local: string[];
    current: string;
    remoteOnly: string[];
    running: boolean;
    last: MockLastSync | null;
    succeeded: number | null;
    ahead: number;
    behind: number;
    conflicts: MockConflict[];
    todos: TodoConflictView[];
    /** Meetings before the sync joined, for "Abort the sync". */
    hereText: string | null;
    problem: string | null;
  };
  const remotes: Record<string, MockRemote> = {};
  /** F13: the git report "Saved {t}" does not replace. */
  let gitReport: string | null = null;
  const setReport = async (text: string) => {
    gitReport = text;
    await setStatus(text);
  };
  /** UC-015 BR-007: a note in conflict waits; the status says so. */
  const conflictRefused = (note: string, verb: string): boolean => {
    if (!remoteOf()?.conflicts.some((c) => c.note === note)) {
      return false;
    }
    void setStatus(`${title(note)} is in conflict; resolve it before ${verb} it`);
    return true;
  };
  let syncNext: MockSyncOutcome = 'synced';
  let branchInTheWay: string[] | null = null;
  let syncCount = 0;
  let syncTimer: ReturnType<typeof setTimeout> | null = null;
  /** The branches and syncs of the open git workspace. */
  const remoteOf = (): MockRemote | null => {
    if (root === null || !gitOf()) {
      return null;
    }
    remotes[root] ??= {
      hasRemote: root === TEAM,
      local: root === TEAM ? ['drafts', 'main'] : ['main'],
      current: 'main',
      remoteOnly: root === TEAM ? ['origin/review'] : [],
      running: false,
      last: null,
      succeeded: null,
      ahead: 0,
      behind: 0,
      conflicts: [],
      todos: [],
      hereText: null,
      problem: null,
    };
    return remotes[root];
  };
  const byName = (a: string, b: string) => a.toLowerCase().localeCompare(b.toLowerCase());
  /** M-04.1 to M-04.3: the git part of the status bar. */
  const gitBarView = (): GitBarView => {
    const state = remoteOf();
    if (!state) {
      return { branch: null, problem: null, conflicts: null, sync: null };
    }
    const count = state.conflicts.length;
    const changes = changesView()?.files.length ?? 0;
    return {
      branch:
        state.problem !== null
          ? null
          : changes === 0
            ? `Branch ${state.current}`
            : `Branch ${state.current} · ${changes === 1 ? '1 change' : `${changes} changes`}`,
      problem: state.problem === null ? null : `Changes cannot be shown: ${state.problem}`,
      conflicts: count === 0 ? null : count === 1 ? '1 conflict' : `${count} conflicts`,
      sync: state.hasRemote ? mockSyncBar({ ...state, conflicts: count, now: Date.now() }) : null,
    };
  };
  /** M-19.9: the conflict bar of the active note while it holds places. */
  const conflictBarView = (): ConflictBarView | null => {
    const state = remoteOf();
    const note = active;
    if (!state || note === null) {
      return null;
    }
    if (!state.conflicts.some((c) => c.note === note && c.deletedOn === null)) {
      return null;
    }
    const mirror = mirrors.get(note);
    const places = mockConflictPlaces(mirror?.text() ?? textOf(note));
    if (places.length === 0) {
      return null;
    }
    return {
      note,
      rev: mirror?.rev ?? 1,
      places: places.map((p) => ({
        line: p.line,
        offset: p.start,
        here: p.here.trimEnd(),
        remote: p.remote.trimEnd(),
      })),
    };
  };
  /** The note's text is `text` now, also in its open tab. */
  const replaceText = async (note: string, text: string) => {
    texts.set(note, text);
    const mirror = mirrors.get(note);
    if (mirror) {
      const rev = mirror.reload(text);
      followMirror(note, mirror);
      await emit('ed-changed', { note, rev });
    }
  };
  /** The tab of `note` closes. */
  const closeTabOf = (note: string) => {
    const at = tabs.findIndex((tab) => tab.note === note);
    if (at >= 0) {
      clearTimeout(saveTimers.get(note));
      tabs.splice(at, 1);
      if (active === note) {
        active = tabs[Math.min(at, tabs.length - 1)]?.note ?? null;
      }
    }
    forgetOther(note);
    for (const map of [texts, mirrors, saveErrors] as Map<string, unknown>[]) {
      map.delete(note);
    }
    unstored.delete(note);
    remember();
  };
  /** UC-014 A3: the fake sync met conflicts. */
  const makeConflict = async (state: MockRemote): Promise<string[]> => {
    const here = textOf('Meetings');
    state.hereText = here;
    const lines = here.split('\n');
    const at = Math.min(2, Math.max(0, lines.length - 2));
    const marked = [
      ...lines.slice(0, at),
      '<<<<<<< HEAD',
      lines[at],
      '=======',
      'The remote changed this line.',
      '>>>>>>> 1a2b3c4',
      ...lines.slice(at + 1),
    ].join('\n');
    await replaceText('Meetings', marked);
    state.conflicts = [
      { path: '.todos/3.yaml', note: null, deletedOn: null },
      { path: 'Meetings.md', note: 'Meetings', deletedOn: null },
      { path: 'Plan.md', note: 'Plan', deletedOn: 'remote' },
      { path: 'assets/team.png', note: null, deletedOn: null },
    ];
    state.todos = [MOCK_TODO_CONFLICT];
    return state.conflicts.map((c) => c.path);
  };
  /**
   * UC-014 step 2: the changes not left out are committed as `Auto: …`;
   * with `message`, under that message (the joining's commit, UC-015 BR-004).
   */
  const autoCommit = (state: MockRemote, message: string | null = null) => {
    const git = gitOf();
    const chosen = gitChanges().filter((c) => !gitLeftOut.has(c.path));
    if (!git || chosen.length === 0) {
      return;
    }
    for (const change of chosen) {
      if (change.note === null) {
        git.others = git.others.filter((o) => o.path !== change.path);
      } else if (change.kind === 'deleted') {
        git.committed.delete(change.note);
      } else {
        git.committed.set(change.note, change.text ?? '');
        if (change.from !== null) {
          git.committed.delete(change.from);
        }
      }
    }
    git.messages.unshift(
      message ??
        `Auto: ${mockProposedMessage(
          chosen.map((c) => ({
            kind: c.kind,
            title: c.note === null ? null : title(c.note),
            from: c.from === null ? null : title(c.from),
          })),
        )}`,
    );
    state.ahead += 1;
  };
  /** How a fake sync ends (UC-014 steps 3–7, A1 to A7, A11). */
  const endSync = async (
    at: string,
    state: MockRemote,
    mode: SyncModeName,
    outcome: MockSyncOutcome,
  ) => {
    state.running = false;
    if (disposed || root !== at) {
      return;
    }
    autoCommit(state);
    let said: string | null = null;
    let files: string[] = [];
    switch (outcome) {
      case 'synced':
        state.succeeded = Date.now();
        if (mode !== 'pull') {
          state.ahead = 0;
        }
        if (mode !== 'push') {
          state.behind = 0;
        }
        break;
      case 'signIn':
        said = `Signing in to the remote failed (fatal: could not read Username for 'https://example.com': terminal prompts disabled). Set up a sign-in for git, such as a saved token or an SSH key.`;
        break;
      case 'pushRejected':
        state.behind = 1;
        said = 'The remote has commits this branch lacks; sync to join them first';
        break;
      case 'inTheWay':
        files = ['README.md'];
        said = `The sync stopped: changes you left out of commits are in the way of the remote's: ${files.join(', ')}. Commit or discard them.`;
        break;
      case 'conflict':
        files = await makeConflict(state);
        said = `The sync stopped: ${files.join(', ')} changed here and on the remote in the same lines`;
        break;
      case 'offline':
      case 'failed':
        break;
    }
    state.last = { outcome, files };
    await sendAll();
    if (said !== null) {
      await setReport(said);
    }
  };
  /** UC-014 step 1, A1, A8, A10: a fake sync after the open notes are stored. */
  const runSync = async (mode: SyncModeName, asked: boolean) => {
    const state = remoteOf();
    if (!state?.hasRemote) {
      if (asked) {
        await setStatus('This workspace has no remote to sync with');
      }
      return;
    }
    if (state.conflicts.length > 0 || gitMerging) {
      if (asked) {
        await setStatus('No sync, because notes are in conflict; resolve them first');
      }
      return;
    }
    if (state.running) {
      return;
    }
    const failures = tabs.map((tab) => store(tab.note)).filter((f): f is string => f !== null);
    if (failures.length > 0) {
      // D-08 (fixed): no sync starts.
      await sendAll();
      await setStatus(`Not everything could be saved before the sync: ${failures.join('; ')}`);
      return;
    }
    syncCount += 1;
    state.running = true;
    await sendGit();
    const at = root ?? '';
    const outcome = syncNext;
    syncTimer = setTimeout(() => {
      syncTimer = null;
      void endSync(at, state, mode, outcome);
    }, 300);
  };
  /** F16, M-21.4: Underline ended (the window would be gone). */
  let windowClosed = false;
  /** UC-014 A9: the last sync before closing was asked for. */
  let closingSync = false;
  /**
   * M-21.4, UC-005 BR-001: Underline is to close: every change is stored,
   * else "Changes not stored" asks (A14 step 3); with a remote, a last
   * sync runs first, ten seconds at most (UC-014 A9). A second close
   * meanwhile goes through, as in the backend.
   */
  const requestClose = async () => {
    const failures = tabs.map((tab) => store(tab.note)).filter((f): f is string => f !== null);
    if (root !== null) {
      failures.push(...todos.settle(root));
    }
    await sendAll();
    if (failures.length > 0) {
      await emit('close-question', {
        tab: null,
        what: 'Underline',
        error: failures.join('; '),
        switching: false,
      });
      return;
    }
    const state = remoteOf();
    if (!closingSync && state?.hasRemote) {
      closingSync = true;
      await runSync('full', false);
      if (state.running) {
        await setStatus('Syncing before closing…');
        const deadline = Date.now() + 10_000;
        const wait = () =>
          later(100, () => {
            if (!state.running || Date.now() >= deadline) {
              windowClosed = true;
            } else {
              wait();
            }
          });
        wait();
        return;
      }
    }
    windowClosed = true;
  };
  /** UC-015 step 7: nothing in conflict any more: the joining completes and the sync goes on. */
  const afterResolving = async (state: MockRemote) => {
    if (state.conflicts.length > 0 || state.hereText === null) {
      return;
    }
    state.hereText = null;
    autoCommit(state, `Merge remote changes into ${state.current}`);
    await sendAll();
    await setReport('The conflicts are resolved; syncing');
    await runSync('full', false);
  };
  /** UC-015 step 6: a note in conflict without places is resolved. */
  const checkResolved = async () => {
    const state = remoteOf();
    if (!state || state.conflicts.length === 0) {
      return;
    }
    const before = state.conflicts.length;
    state.conflicts = state.conflicts.filter(
      (c) =>
        c.note === null ||
        c.deletedOn !== null ||
        mockConflictPlaces(mirrors.get(c.note)?.text() ?? textOf(c.note)).length > 0,
    );
    if (state.conflicts.length !== before) {
      for (const tab of [...tabs, ...(other?.tabs ?? [])]) {
        store(tab.note);
      }
      await afterResolving(state);
    }
  };
  const label = (note: string) =>
    title(note) +
    (saveErrors.has(note) ? ' ⚠' : unstored.has(note) && !quiet.has(note) ? ' •' : '');
  const tabViews = (list: FakeTab[]) =>
    list.map((tab) => ({
      note: tab.note,
      title: title(tab.note),
      label: label(tab.note),
      path: `${root}/${tab.note}.md`,
      view: tab.view,
      saveError: saveErrors.get(tab.note) ?? null,
    }));
  const tabsView = (): TabsView => {
    const left = pane === 'left' || other === null ? { tabs, active } : other;
    const right = other === null ? null : pane === 'right' ? { tabs, active } : other;
    return {
      tabs: tabViews(left.tabs),
      active,
      todoList: {
        open: todos.list(root).open,
        active: todos.list(root).active,
        pane: todos.list(root).pane,
      },
      split:
        right === null
          ? null
          : {
              right: tabViews(right.tabs),
              leftActive: left.active,
              rightActive: right.active,
              pane,
              leftShare: (root !== null ? splitShares[root] : undefined) ?? 500,
            },
    };
  };
  const properties = (): TabPropertiesView | null =>
    active === null
      ? null
      : {
          note: active,
          properties: {
            place: `${active}.${formatOf(active) === 'AsciiDoc' ? 'adoc' : 'md'}`,
            format: formatOf(active),
            size: `${new TextEncoder().encode(textOf(active)).length} bytes`,
            lastChange: '2026-11-09 10:00',
          },
        };
  /** Notes whose saves fail (`MockStart.failingSaves`, the hook `failSaves`). */
  const failing = new Set(start.failingSaves ?? []);
  /** Stores `note` now; the failure names the note, as the backend's does. */
  const store = (note: string): string | null => {
    if (!unstored.has(note) && !saveErrors.has(note)) {
      return null;
    }
    if (failing.has(note)) {
      saveErrors.set(note, MOCK_SAVE_ERROR);
      return `${title(note)}: ${MOCK_SAVE_ERROR}`;
    }
    unstored.delete(note);
    saveErrors.delete(note);
    return null;
  };
  /** M-10.8: stored a second after the last change. */
  const changed = async (note: string) => {
    const first = !unstored.has(note);
    unstored.add(note);
    clearTimeout(saveTimers.get(note));
    saveTimers.set(
      note,
      setTimeout(() => {
        saveTimers.delete(note);
        if (disposed) {
          return;
        }
        const failure = store(note);
        // UC-006 A7: a save of a rename's links keeps its report; D-04
        // (fixed): "Saved" does not end an offer.
        const quietly =
          quiet.delete(note) ||
          (undo !== null && status() === undo.status) ||
          // F15: so does a todo's report with its "Undo".
          todos.offer(root, status()) !== null ||
          exportKept() ||
          // F13: a git report stays (like A-113).
          (gitReport !== null && status() === gitReport);
        if (!failure && quietly) {
          void sendAll();
          return;
        }
        void setStatus(
          failure
            ? `${title(note)} could not be stored: ${MOCK_SAVE_ERROR}`
            : `Saved ${title(note)}`,
        ).then(sendAll);
      }, 1000),
    );
    if (first) {
      await emit('tabs', tabsView());
    }
  };
  const snapshot = (): WorkspaceSnapshot => ({
    workspace: root ? { root, name: root.split('/').at(-1) ?? root } : null,
    overview: overview(),
    tabs: tabsView(),
  });
  /** M-16.5: the lists of the active note, as the backend's tick sends them. */
  const sendIndexes = async () => {
    if (active === null || disposed) {
      return;
    }
    const mirror = mirrors.get(active);
    try {
      await emit(
        'indexes',
        mockIndexes(active, mirror?.rev ?? 1, mirror?.text() ?? textOf(active)),
      );
    } catch {
      // The mock was taken down meanwhile (a test ended).
    }
    linksSoon();
  };
  /**
   * FR-078, FR-079: the backlinks and outgoing links of the active note,
   * a moment after the change, as the backend's thread finds them; the
   * open notes count with their text.
   */
  let linksTimer: ReturnType<typeof setTimeout> | null = null;
  /** BR-021: how many unlinked mentions are listed, and of which note. */
  const mentionLimit: { note: string | null; limit: number } = {
    note: null,
    limit: MOCK_MENTIONS_SHOWN,
  };
  const linksSoon = () => {
    if (linksTimer !== null) {
      return;
    }
    linksTimer = setTimeout(() => {
      linksTimer = null;
      void sendLinks();
    }, 50);
  };
  const sendLinks = async () => {
    if (active === null || disposed || root === null) {
      return;
    }
    const note = active;
    const text = (id: string) => mirrors.get(id)?.text() ?? textOf(id);
    // Only the notes that may hold a link or a mention, in the overview's
    // order: the 10,000 notes of LARGE hold none, and sorting them all
    // would hold up the window's keys (NFR-018 is measured there).
    const notes = files[root] ?? [];
    const named = title(note).toLowerCase();
    const order = [
      note,
      ...notes.filter((other) => {
        if (other === note) {
          return false;
        }
        const t = text(other);
        return t.includes('](') || t.includes('::') || t.toLowerCase().includes(named);
      }),
    ].sort(byOverview(lastOpened[root] ?? {}));
    // The files of the workspace that are no notes, as the backend asks
    // the disk (package 12 review: existing pictures and scripts are not
    // broken).
    const others = [...(root === LINKS ? LINKS_FILES : []), ...FAKE_WORKSPACE_FILES];
    // BR-021: another note lists the first 200 mentions again.
    if (mentionLimit.note !== note) {
      mentionLimit.note = note;
      mentionLimit.limit = MOCK_MENTIONS_SHOWN;
    }
    try {
      await emit(
        'links',
        mockLinks(
          note,
          mirrors.get(note)?.rev ?? 1,
          order,
          text,
          others,
          notes,
          mentionLimit.limit,
        ),
      );
    } catch {
      // The mock was taken down meanwhile (a test ended).
    }
  };
  /** F9: the search of the overlay, as the mock runs it. */
  let searchText = '';
  let searchSwitches: SearchSwitches = NO_SWITCHES;
  let searchSerial = 0;
  let searchFound: FoundNoteView[] = [];
  let searchTimer: ReturnType<typeof setTimeout> | null = null;
  const SEARCH_PAGE = 100;
  /** UC-008, roughly as `search_notes` (BR-001 to BR-004). */
  const runSearch = async () => {
    if (disposed || root === null || searchText === '') {
      return;
    }
    const switched = searchSwitches.matchCase || searchSwitches.wholeWord || searchSwitches.regex;
    if (searchSwitches.regex) {
      // UC-008 A9: a search again keeps saying what is wrong (no race with
      // `search_set`).
      const pattern = mockPattern(searchText, searchSwitches);
      if ('problem' in pattern) {
        try {
          await emit('search', {
            text: searchText,
            switches: searchSwitches,
            problem: pattern.problem,
            result: null,
          });
        } catch {
          // The mock was taken down meanwhile (a test ended).
        }
        return;
      }
    }
    const typed = mockQuery(searchText);
    if (typed.length === 0) {
      return;
    }
    // UC-008 BR-012: words `#tag` search for a tag (not with the switch
    // regular expression); the places marked are the tag's words.
    const isTag = (word: string) =>
      !searchSwitches.regex && /^#[\p{L}\p{N}_\-/]+$/u.test(word) && /[^\d/#]/.test(word);
    const tagWords = typed.filter(isTag).map((word) => word.slice(1));
    const words = tagWords.length > 0 ? [...typed.filter((w) => !isTag(w)), ...tagWords] : typed;
    const byName: FoundNoteView[] = [];
    const byText: FoundNoteView[] = [];
    for (const { note } of overview()) {
      const text = mirrors.get(note)?.text() ?? textOf(note);
      if (tagWords.length > 0) {
        const tags = mockTags(text);
        const rest = typed.filter((w) => !isTag(w));
        const low = text.toLowerCase();
        if (tagWords.every((tag) => carries(tags, tag)) && rest.every((w) => low.includes(w))) {
          byText.push(mockFoundNote(note, title(note), folder(note), text, words));
        }
        continue;
      }
      if (switched) {
        // FR-075, roughly: one pattern, places in the text only.
        const pattern = mockPattern(searchSwitches.regex ? searchText : words[0], searchSwitches);
        if ('problem' in pattern) {
          continue;
        }
        const found = mockFoundByPattern(note, title(note), folder(note), text, pattern.regex);
        if (found !== null) {
          byText.push(found);
        }
        continue;
      }
      const name = [title(note).toLowerCase(), (folder(note) ?? '').toLowerCase()];
      const low = text.toLowerCase();
      if (!words.every((w) => name.some((n) => n.includes(w)) || low.includes(w))) {
        continue;
      }
      const found = mockFoundNote(note, title(note), folder(note), text, words);
      (words.every((w) => name.some((n) => n.includes(w))) ? byName : byText).push(found);
    }
    searchFound = [...byName, ...byText];
    searchSerial += 1;
    try {
      await emit('search', {
        text: searchText,
        switches: searchSwitches,
        problem: null,
        result: {
          serial: searchSerial,
          text: searchText,
          switches: searchSwitches,
          total: searchFound.length,
          notes: searchFound.slice(0, SEARCH_PAGE),
          unreadable: null,
        },
      });
    } catch {
      // The mock was taken down meanwhile (a test ended).
    }
  };
  /** UC-022: the replace of the workspace, as the mock runs it. */
  const replacing = {
    open: false,
    text: '',
    replacement: '',
    switches: NO_SWITCHES,
    asked: 0,
    serial: 0,
    problem: null as string | null,
    notes: [] as ReplaceNoteView[],
    /** Every place of each note, also those not shown (A7). */
    all: new Map<string, MockPlace[]>(),
    /** The texts the places were found in, by note. */
    seen: new Map<string, string>(),
    leftNotes: new Set<string>(),
    /** `note`, the line and the column of a place left out. */
    leftPlaces: new Set<string>(),
    undo: null as {
      status: string;
      notes: { note: string; before: string; after: string }[];
    } | null,
    timer: null as ReturnType<typeof setTimeout> | null,
  };
  const REPLACE_PAGE = 1000;
  const REPLACE_SHOWN = 10_000;
  /** A place left out is known by its line and where in the line it starts (A4). */
  const placeKey = (note: string, text: string, at: number) => {
    const start = text.lastIndexOf('\n', at - 1) + 1;
    const end = text.indexOf('\n', at);
    return `${note}\u0000${text.slice(start, end < 0 ? text.length : end)}\u0000${at - start}`;
  };
  /** UC-022 step 2: every place of `text`, roughly as `note_places`. */
  const mockPlaces = (text: string, regex: RegExp, replacement: string): MockPlace[] =>
    [...text.matchAll(regex)]
      .filter((m) => m[0] !== '' && (!m[0].includes('\n') || replacing.switches.regex))
      .map((m) => {
        const at = m.index;
        const start = text.lastIndexOf('\n', at - 1) + 1;
        const end = text.indexOf('\n', at + m[0].length);
        const lineEnd = end < 0 ? text.length : end;
        const one = new RegExp(regex.source, regex.flags.replace('g', ''));
        return {
          at,
          line: text.slice(0, at).split('\n').length,
          before: text.slice(Math.max(start, at - 40), at),
          old: m[0],
          new: m[0].replace(one, replacement),
          after: text.slice(at + m[0].length, Math.min(lineEnd, at + m[0].length + 80)),
        };
      });
  /** "1 place", "2 places", "10,000 places", as the backend counts. */
  const placesOf = (n: number, thing = 'place') =>
    n === 1 ? `1 ${thing}` : `${n.toLocaleString('en-US')} ${thing}s`;
  /** Step 3: what is chosen now, roughly as `chosen_view`. */
  const chosenView = (): ChosenView => {
    let places = 0;
    let notes = 0;
    let total = 0;
    for (const found of replacing.notes) {
      const all = replacing.all.get(found.note) ?? [];
      total += all.length;
      if (replacing.leftNotes.has(found.note)) {
        continue;
      }
      const text = replacing.seen.get(found.note) ?? '';
      const left = all.filter((p) =>
        replacing.leftPlaces.has(placeKey(found.note, text, p.at)),
      ).length;
      if (all.length - left > 0) {
        places += all.length - left;
        notes += 1;
      }
    }
    return {
      places,
      notes,
      summary: `${places.toLocaleString('en-US')} of ${placesOf(total)} chosen`,
      action:
        places === 0 ? 'Replace' : `Replace ${placesOf(places)} in ${placesOf(notes, 'note')}`,
    };
  };
  /** UC-022 step 2, A1, A2, A4, A7: the places of every note, told as `replace`. */
  const runReplace = async () => {
    if (disposed || root === null || !replacing.open) {
      return;
    }
    replacing.notes = [];
    replacing.all.clear();
    replacing.seen.clear();
    replacing.problem = null;
    let found = 0;
    let shown = 0;
    if (replacing.text !== '') {
      const pattern = mockPattern(replacing.text, replacing.switches);
      const writes =
        'problem' in pattern
          ? pattern
          : mockReplacement(replacing.replacement, replacing.switches, pattern.groups);
      if ('problem' in writes) {
        replacing.problem = writes.problem;
      } else if (!('problem' in pattern)) {
        for (const { note } of overview()) {
          const text = mirrors.get(note)?.text() ?? textOf(note);
          const places = mockPlaces(text, pattern.regex, writes.with);
          if (places.length === 0) {
            continue;
          }
          replacing.seen.set(note, text);
          replacing.all.set(note, places);
          found += places.length;
          // A7: no more than 10,000 places are shown.
          const room = Math.max(0, REPLACE_SHOWN - shown);
          shown += Math.min(room, places.length);
          replacing.notes.push({
            note,
            title: title(note),
            folder: folder(note),
            count: placesOf(places.length),
            leftOut: replacing.leftNotes.has(note),
            places: places.slice(0, room).map((place) => ({
              ...place,
              leftOut: replacing.leftPlaces.has(placeKey(note, text, place.at)),
            })),
          });
        }
      }
    }
    replacing.serial += 1;
    const notes = replacing.notes.length;
    try {
      await emit('replace', {
        asked: replacing.asked,
        problem: replacing.problem,
        result:
          replacing.text === '' || replacing.problem !== null
            ? null
            : {
                serial: replacing.serial,
                asked: replacing.asked,
                places: found,
                notes,
                summary: `${placesOf(found)} in ${placesOf(notes, 'note')}`,
                more:
                  found > shown
                    ? `The first ${REPLACE_SHOWN.toLocaleString('en-US')} places are shown; ${(found - shown).toLocaleString('en-US')} more are not shown and are replaced too.`
                    : null,
                list: replacePage(0),
                listed: notes,
                unreadable: null,
                chosen: chosenView(),
              },
      });
    } catch {
      // The mock was taken down meanwhile (a test ended).
    }
  };
  /** A page of the notes from the `from`th on, about 1,000 places, as the backend pages. */
  const replacePage = (from: number): ReplaceNoteView[] => {
    const page: ReplaceNoteView[] = [];
    let places = 0;
    for (const found of replacing.notes.slice(from)) {
      if (places >= REPLACE_PAGE) {
        break;
      }
      places += Math.max(found.places.length, 1);
      page.push(found);
    }
    return page;
  };
  /** UC-022 A4: a text or the overview changed: the places are found again soon. */
  const replaceAgain = () => {
    if (!replacing.open || replacing.timer !== null) {
      return;
    }
    replacing.timer = setTimeout(() => {
      replacing.timer = null;
      void runReplace();
    }, 30);
  };
  /**
   * UC-022 step 5, BR-005: `note` gets `text`; open, through its mirror as
   * one undo step, else in the fake files. Why not, when it cannot be stored.
   */
  const replaceWorkspaceNote = async (
    note: string,
    from: string,
    to: string,
  ): Promise<string | null> => {
    if (failing.has(note)) {
      return MOCK_SAVE_ERROR;
    }
    const mirror = mirrors.get(note);
    if (!mirror) {
      texts.set(note, to);
      noteTouched(note);
      return null;
    }
    const a = Array.from(from);
    const b = Array.from(to);
    let start = 0;
    while (start < a.length && start < b.length && a[start] === b[start]) start++;
    let same = 0;
    while (
      same < a.length - start &&
      same < b.length - start &&
      a[a.length - 1 - same] === b[b.length - 1 - same]
    ) {
      same++;
    }
    const insert = b.slice(start, b.length - same).join('');
    const rev = mirror.backendEdit(start, a.length - same - start, insert, true);
    if (rev !== undefined) {
      followMirror(note, mirror);
      await emit('ed-changed', { note, rev });
    }
    return null;
  };

  /** UC-008 A3: a text or the overview changed: the search runs again soon. */
  const searchAgain = () => {
    replaceAgain();
    if (searchText === '' || searchTimer !== null) {
      return;
    }
    searchTimer = setTimeout(() => {
      searchTimer = null;
      void runSearch();
    }, 30);
  };
  const sendAll = async () => {
    if (disposed) {
      return;
    }
    searchAgain();
    const now = snapshot();
    try {
      await emit('workspace', now.workspace);
      await emit('overview', now.overview);
      await emit('deleted', deletedView());
      await emit('tabs', now.tabs);
      await emit('properties', properties());
      await sendIndexes();
      await sendGit();
      await sendTodos();
      await showSpellState();
      if (settingsSaid !== undefined) {
        const said = settingsSaid;
        settingsSaid = undefined;
        await sendSettings();
        if (said !== null) {
          await setStatus(said);
        }
      }
    } catch {
      // The mock was taken down meanwhile (a test ended).
    }
  };
  /** UC-004 A2: the jump to the heading `anchor` of `note`, or why there is none. */
  const jumpToHeading = async (note: string, anchor: string): Promise<LinkAnswer> => {
    const mirror = mirrors.get(note);
    const text = mirror?.text() ?? textOf(note);
    let at = 0;
    for (const line of text.split('\n')) {
      const heading = /^(#{1,6})(\s+)(.*)$/.exec(line);
      if (heading && mockAnchor(heading[3]) === anchor) {
        return {
          kind: 'jump',
          note,
          rev: mirror?.rev ?? 1,
          offset: at + heading[1].length + heading[2].length,
        };
      }
      at += line.length + 1;
    }
    await setStatus(`The heading ${anchor} was not found`);
    return { kind: 'done' };
  };
  /** UC-004 BR-007: the system's program opens `target` (only recorded). */
  const openWithSystem = (target: string) => {
    opened.push(target);
  };
  /** F11: what the fake save dialog does next. */
  let exportDialog: MockExportDialog = { answer: 'choose' };
  /** F11: the files exports wrote, by path, with what they hold. */
  const exported = new Map<string, string>([['/home/user/out/Plan.html', '<p>old</p>']]);
  /** UC-009 BR-007: the folder of the last export, for this session. */
  let lastExportFolder: string | null = null;
  /** UC-009 step 3: the file chosen in the dialog, until the export runs. */
  let exportChosen: { note: string; kind: ExportKindName; path: string } | null = null;
  /** M-04.8: the reports the status shows, with the status they belong to (A-120). */
  let exportReports: {
    title: string;
    target: string | null;
    open: boolean;
    approximated: number[];
  }[] = [];
  let reportsStatus = '';
  /** The running exports' "Exporting {t}…", and the open dialog's "Choosing where to save {t}…". */
  const exportsRunning: string[] = [];
  let choosing: { status: string; before: string } | null = null;
  /** D-04, A-113: an export's own status, which "Saved {t}" does not replace. */
  const exportKept = () =>
    choosing?.status === status() ||
    exportsRunning.includes(status()) ||
    (exportReports.length > 0 && reportsStatus === status());
  const exportLabel = (kind: ExportKindName) =>
    FAKE_EXPORT_KINDS.markdown.find((choice) => choice.kind === kind)?.label ?? kind;
  const exportExtension = (kind: ExportKindName) =>
    kind === 'otherFormat' ? 'adoc' : kind === 'html' ? 'html' : 'pdf';
  const exportName = (note: string, kind: ExportKindName) =>
    `${note.split('/').at(-1) ?? note}.${exportExtension(kind)}`;
  const exportFolder = () => lastExportFolder ?? FAKE_HOME;
  /** The fake folders a typed path may name (review F11). */
  const isFolder = (path: string) =>
    FAKE_OUTSIDE[path] === -1 ||
    [FAKE_HOME, '/home/user/out', '/home/user/locked', ...Object.keys(files)].includes(
      path.replace(/\/+$/, ''),
    );
  /** Whether `path` holds a file in the fake world. */
  const fileExists = (path: string) =>
    exported.has(path) ||
    path in FAKE_OUTSIDE ||
    (root !== null && (files[root] ?? []).some((note) => path === `${root}/${note}.md`));
  /** A5, A-116: a note's file, `underline.yaml`, or anything under a dot folder of the workspace. */
  const isWorkspaceFile = (path: string) => {
    if (root === null || !path.startsWith(`${root}/`)) {
      return false;
    }
    const inside = path.slice(root.length + 1);
    return (
      inside === 'underline.yaml' ||
      inside.split('/').some((part) => part.startsWith('.')) ||
      (files[root] ?? []).some((note) => inside === `${note}.md`)
    );
  };
  /** UC-004 A2 to A4: a rough stand-in for `follow_link` over the fake notes. */
  const followLink = async (note: string, raw: string, side = false): Promise<LinkAnswer> => {
    const link = raw.trim();
    // Q-61: a question not answered does not outlive the next link.
    openAsked = null;
    const fileUrl = /^file:\/\//i.test(link);
    // Q-61, Q-62: opens `target` or asks first, every time.
    const openOrAsk = (target: string, how: 'address' | 'file' | 'fileUrl'): LinkAnswer => {
      const question = mockOpenQuestion(target, how);
      if (question === null) {
        openWithSystem(target);
        return { kind: 'done' };
      }
      openAsks += 1;
      openAsked = { id: openAsks, target };
      return { kind: 'askOpen', id: openAsks, ...question };
    };
    // Q-62: `://` makes an address only after something that can be a scheme.
    const beforeAddress = link.split('#')[0].split('://');
    const addressed = beforeAddress.length > 1 && /^[^/\\?]+$/.test(beforeAddress[0]);
    if ((addressed && !fileUrl) || /^mailto:/i.test(link)) {
      return openOrAsk(link, 'address');
    }
    // Q-63: a `file://` place without query, fragment and `localhost`, decoded.
    const [path, heading] = fileUrl
      ? [
          safeDecode(
            link
              .slice('file://'.length)
              .split(/[#?]/)[0]
              .replace(/^localhost\//i, '/'),
          ),
          undefined,
        ]
      : (link.split(/#(.*)/s) as [string, string | undefined]);
    if (path === '' && !fileUrl) {
      return jumpToHeading(note, heading ?? '');
    }
    // Q-61: a place on another computer is asked about from its text alone.
    if (/^[/\\]{2}/.test(path)) {
      return openOrAsk(path, 'file');
    }
    if (path.startsWith('/')) {
      if (path in FAKE_OUTSIDE) {
        return openOrAsk(path, fileUrl ? 'fileUrl' : 'file');
      }
      await setStatus(`${path} does not exist`);
      return { kind: 'done' };
    }
    const parts = folderOf(note).split('/').filter(Boolean);
    for (const part of (fileUrl ? path : safeDecode(path)).split('/')) {
      if (part === '..') {
        if (parts.pop() === undefined) {
          await setStatus(`${link} leads outside the workspace and is not followed`);
          return { kind: 'done' };
        }
      } else if (part !== '.' && part !== '') {
        parts.push(part);
      }
    }
    const relative = parts.join('/');
    if (FAKE_WORKSPACE_FILES.includes(relative)) {
      return openOrAsk(`${root}/${relative}`, fileUrl ? 'fileUrl' : 'file');
    }
    if (fileUrl) {
      await setStatus(`${root}/${relative} does not exist`);
      return { kind: 'done' };
    }
    const extension = /\.([^./]+)$/.exec(relative)?.[1];
    const isNote = extension === 'md' || extension === 'adoc';
    const id = isNote ? relative.replace(/\.[^./]+$/, '') : relative;
    if (root && files[root]?.includes(id)) {
      if (side) {
        openToSide(id);
      } else {
        openTab(id);
      }
      await sendAll();
      return heading === undefined ? { kind: 'done' } : jumpToHeading(id, heading);
    }
    if (isNote || extension === undefined) {
      const at = id.lastIndexOf('/');
      const noteTitle = title(id);
      return {
        kind: 'newNote',
        folder: at < 0 ? '' : id.slice(0, at),
        title: noteTitle,
        format: 'Markdown',
        problem: `${noteTitle} does not exist yet. Create it?`,
      };
    }
    await setStatus(`${root}/${relative} does not exist`);
    return { kind: 'done' };
  };
  const remember = () => {
    if (root) {
      remembered[root] = {
        tabs: [...tabs],
        active,
        pane,
        other: other && { tabs: [...other.tabs], active: other.active },
      };
    }
  };
  const viewed = (note: string) => {
    if (root) {
      lastOpened[root] = { ...lastOpened[root], [note]: ++clock };
      // F15, UC-021 step 6: the note shows; the tab "Todos" stays open.
      // OQ-279: only in the tab's pane; the other pane keeps the todos.
      const list = todos.list(root);
      if (list.pane === pane) {
        list.active = false;
      }
    }
  };
  /**
   * Takes the tab of `note` out, with its marks. A stored text stays the
   * note's, as on disk; one not stored ("Close anyway") is lost, and the
   * note reads as the fake file again (roughly: the mock keeps no older
   * stored text).
   */
  const dropTab = (note: string, stored: boolean) => {
    const at = tabs.findIndex((tab) => tab.note === note);
    if (at < 0) {
      return;
    }
    clearTimeout(saveTimers.get(note));
    saveTimers.delete(note);
    if (!stored) {
      texts.delete(note);
    }
    mirrors.delete(note);
    unstored.delete(note);
    saveErrors.delete(note);
    tabs.splice(at, 1);
    if (active === note) {
      active = tabs[Math.min(at, tabs.length - 1)]?.note ?? null;
    }
    forgetOther(note);
  };
  /**
   * UC-004 A6, M-06.6: at the limit, the tab used longest ago that can be
   * stored closes (D-07, fixed); `false` when none can, and the note does
   * not open. Roughly the backend's order: by when a note was opened last.
   */
  const makeRoomFor = (note: string): boolean => {
    // UC-004 BR-008: both panes count.
    const count = tabs.length + (other?.tabs.length ?? 0);
    if (count < MAX_TABS || tabs.some((tab) => tab.note === note)) {
      return true;
    }
    const opened = (root && lastOpened[root]) || {};
    // Review F16: never the active tab of a pane.
    const order = [
      ...tabs.filter((tab) => tab.note !== active).map((tab) => ({ tab, here: true })),
      ...(other?.tabs ?? [])
        .filter((tab) => tab.note !== other?.active)
        .map((tab) => ({ tab, here: false })),
    ].sort((a, b) => (opened[a.tab.note] ?? 0) - (opened[b.tab.note] ?? 0));
    const both = (n: string) =>
      tabs.some((t) => t.note === n) && !!other?.tabs.some((t) => t.note === n);
    const closing = order.find(({ tab }) => both(tab.note) || store(tab.note) === null);
    if (!closing) {
      void setStatus(
        `${title(note)} was not opened: ${MAX_TABS} tabs are open, and none can close without losing changes`,
      );
      return false;
    }
    const gone = closing.tab.note;
    if (both(gone)) {
      // Its other tab keeps the note open.
      const list = closing.here ? tabs : other!.tabs;
      list.splice(
        list.findIndex((t) => t.note === gone),
        1,
      );
    } else if (closing.here) {
      dropTab(gone, true);
    } else {
      forgetOther(gone);
      clearTimeout(saveTimers.get(gone));
      mirrors.delete(gone);
    }
    void setStatus(`Closed the tab ${title(gone)}, used longest ago`);
    return true;
  };
  const openTab = (note: string, view: EditingViewName = start.view ?? 'facade'): boolean => {
    if (!makeRoomFor(note)) {
      return false;
    }
    if (!tabs.some((tab) => tab.note === note)) {
      const at = active === null ? tabs.length : tabs.findIndex((t) => t.note === active) + 1;
      tabs.splice(at, 0, { note, view });
    }
    active = note;
    viewed(note);
    remember();
    return true;
  };
  /**
   * UC-004 A18, A19: opens `note` to the side, in the other pane, which the
   * split view brings when it is not shown; the note of the active tab
   * shows its other editing view there.
   */
  const openToSide = (note: string): boolean => {
    if (tabs.length === 0 && (other?.tabs.length ?? 0) === 0) {
      // A18, A20: with no tab there is no side; the note opens in the one pane.
      other = null;
      pane = 'left';
      return openTab(note);
    }
    const shown = active === note ? tabs.find((tab) => tab.note === note)?.view : undefined;
    if (other === null) {
      other = { tabs: [], active: null };
    }
    focusPane(pane === 'left' ? 'right' : 'left');
    const view: EditingViewName =
      shown === 'facade' ? 'source' : shown === 'source' ? 'facade' : (start.view ?? 'facade');
    const opened = openTab(note, view);
    settlePanes();
    return opened;
  };
  /** F10: the settings of the workspace opened last, still to be sent. */
  let settingsSaid: string | null | undefined;
  const switchTo = (dir: string) => {
    root = dir;
    // F14, D-05 (fixed): the agents of the workspace left are forgotten.
    agents.leave();
    void sendAgentMarks();
    gitChosen = null;
    gitDiscarding = null;
    // F13, UC-014 BR-001: a workspace with a remote syncs when it opens.
    if (dir === TEAM) {
      setTimeout(() => void runSync('full', false), 0);
    }
    settingsSaid = model.openWorkspace(dir);
    recent = [dir, ...recent.filter((r) => r !== dir)];
    const back = remembered[dir];
    tabs = back ? [...back.tabs] : [];
    active = back?.active ?? null;
    pane = back?.pane ?? 'left';
    other = back?.other ? { tabs: [...back.other.tabs], active: back.other.active } : null;
    // BR-018: the left pane is the active one when the workspace opens again.
    focusPane('left');
    if (!back) {
      const first = ['Home', 'README'].find((n) => files[dir]?.includes(n));
      if (first) {
        openTab(first);
      }
    }
  };
  const problem = (
    path: string,
    text: string,
    offer: Extract<OpenAnswer, { kind: 'problem' }>['offer'],
  ): OpenAnswer => ({ kind: 'problem', path, problem: text, offer, recent: [...recent] });

  const open = async (path: string, anyway: boolean): Promise<OpenAnswer> => {
    const dir = normal(path);
    if (dir === root) {
      return { kind: 'done' };
    }
    if (!(dir in files) && !outerOf(dir) && dir !== FAKE_HOME) {
      recent = recent.filter((r) => r !== dir);
      return problem(
        path,
        `${dir} does not exist. Choose another directory or create a new workspace there.`,
        { kind: 'create', directory: dir },
      );
    }
    if (dir === FAKE_HOME) {
      return problem(
        path,
        `the home directory ${dir} cannot be a workspace, because it holds the user settings file and all of your files. Choose another directory.`,
        null,
      );
    }
    const outer = outerOf(dir);
    if (outer && !anyway) {
      return problem(path, `${dir} lies inside the workspace ${outer}.`, {
        kind: 'outer',
        outer,
        inner: dir,
      });
    }
    if (!(dir in files)) {
      const prefix = `${dir.slice(outer!.length + 1)}/`;
      files[dir] = (files[outer!] ?? [])
        .filter((n) => n.startsWith(prefix))
        .map((n) => n.slice(prefix.length));
    }
    // F15 review: todos that cannot be stored are asked about first.
    if (await askBeforeSwitch({ dir, created: false })) {
      return { kind: 'done' };
    }
    switchTo(dir);
    await sendAll();
    await setStatus(`Opened ${dir}`);
    return { kind: 'done' };
  };
  /** F15 review: the workspace that opens after "Close anyway". */
  let switchWaiting: { dir: string; created: boolean } | null = null;
  /** Asks "Changes not stored" when todos of the open workspace cannot be stored. */
  const askBeforeSwitch = async (waiting: { dir: string; created: boolean }) => {
    const failures = root === null ? [] : todos.settle(root);
    if (failures.length === 0) {
      return false;
    }
    switchWaiting = waiting;
    await emit('close-question', {
      tab: null,
      what: `the workspace ${root?.split('/').at(-1) ?? ''}`,
      error: failures.join('; '),
      switching: true,
    });
    return true;
  };

  const checkNew = (path: string): CreateAnswer => {
    const dir = normal(path.trim());
    const outer = outerOf(dir);
    if (outer) {
      return {
        stage: 'directory',
        path,
        problem: `${dir} lies inside the workspace ${outer}. Choose another directory.`,
        offer: null,
      };
    }
    if (dir in files || dir === FAKE_HOME) {
      return {
        stage: 'directory',
        path,
        problem: `${dir} already holds files or folders. Choose another directory or open it as a workspace.`,
        offer: dir,
      };
    }
    return { stage: 'options', directory: dir };
  };

  // F14: the agents (UC-017 to UC-019).
  const LEVEL_LABELS: Record<AgentLevelName, string> = {
    off: 'Off',
    readOnly: 'Read only',
    readWrite: 'Read and write',
  };
  const setAgentLevel = async (level: AgentLevelName) => {
    const label = LEVEL_LABELS[level];
    if ((model.user.agent_access ?? 'Off') === label) {
      return;
    }
    const said = model.change('me', 'agent_access', label);
    await sendSettings();
    await sendAgentMarks();
    if (said !== null) {
      await setStatus(said);
    }
  };
  const sendAgents = async () => {
    await emit('agents', agentsView(agents, model));
  };
  let marksTimer: ReturnType<typeof setTimeout> | null = null;
  /** M-17.6: the cursors in the open notes, again when the next one ends. */
  const sendAgentMarks = async () => {
    if (disposed) {
      return;
    }
    const now = Date.now();
    const shown = agents.marks(
      [...tabs, ...(other?.tabs ?? [])].map((tab) => tab.note),
      model.effective().agentAccess,
      now,
    );
    if (marksTimer !== null) {
      clearTimeout(marksTimer);
      marksTimer = null;
    }
    const next = Math.min(...[...agents.cursors.values()].map((c) => c.until));
    if (Number.isFinite(next)) {
      marksTimer = setTimeout(() => void sendAgentMarks(), Math.max(0, next - now) + 20);
    }
    await emit('agent-marks', shown);
  };
  /** UC-019: the agent `name` may write, else why not. */
  const agentRefusal = (note: string): string | null =>
    levelOf(model.effective().agentAccess) >= 2
      ? null
      : `the note \`${note}\` cannot be written: Underline allows agents only to read the notes of this workspace`;
  /** UC-019 BR-010: a note not open changed; the status reports it with "Open". */
  const reportAgentChange = async (name: string, verb: 'created' | 'changed', note: string) => {
    const text = agents.reportChange(name, verb, note, status());
    await setStatus(text, null, false, agents.opens(text));
  };
  /** UC-026 steps 5–6: `note` gets the snapshot `key`'s text; the status. */
  const restoreSnapshot = async (note: string, key: string, undoing: boolean): Promise<string> => {
    const snapshot = snapshotOf(note, key);
    const now = noteNow(note);
    if (now === snapshot.text) {
      restoreUndo = null;
      const text = `${title(note)} already holds the snapshot of ${snapshotTime(snapshot.taken)}`;
      await setStatus(text);
      return text;
    }
    const before = takeSnapshot(note, now, 'Restore');
    const failed = await replaceWorkspaceNote(note, now, snapshot.text);
    if (failed !== null) {
      restoreUndo = null;
      const text = `${title(note)} could not be stored: ${failed}`;
      await setStatus(text);
      return text;
    }
    if (undoing) {
      restoreUndo = null;
      const text = `Took back the restore of ${title(note)}`;
      await setStatus(text);
      return text;
    }
    const text = `Restored ${title(note)} to the snapshot of ${snapshotTime(snapshot.taken)}`;
    restoreUndo = { status: text, note, key: before.key };
    await setStatus(text, 'undoRestore');
    searchAgain();
    return text;
  };
  const handlers: ReturnType<typeof workspaceHandlers> = {
    ws_start: async () => {
      if (!started) {
        started = true;
        const dir = start.givenDir ?? start.lastWorkspace ?? null;
        if (dir !== null) {
          const answer = await open(dir, false);
          return { answer, snapshot: snapshot() };
        }
      }
      return { answer: { kind: 'done' }, snapshot: snapshot() };
    },
    ws_recent: () => [...recent],
    ws_open: ({ path, anyway }) => open(path, anyway),
    ws_check_new: ({ path }) => checkNew(path),
    ws_create: async ({ request }) => {
      const checked = checkNew(request.directory);
      if (checked.stage !== 'options') {
        return checked;
      }
      const dir = checked.directory;
      files[dir] = ['Home'];
      if (await askBeforeSwitch({ dir, created: true })) {
        return { stage: 'done' };
      }
      switchTo(dir);
      openTab('Home');
      await sendAll();
      await setStatus(`Workspace created in ${dir}`);
      return { stage: 'done' };
    },
    search_set: async ({ text, switches }) => {
      searchText = text.trim();
      searchSwitches = switches ?? NO_SWITCHES;
      const problem =
        searchSwitches.regex && searchText !== ''
          ? (() => {
              const pattern = mockPattern(searchText, searchSwitches);
              return 'problem' in pattern ? pattern.problem : null;
            })()
          : null;
      if (problem !== null) {
        searchFound = [];
        await emit('search', { text: searchText, switches: searchSwitches, problem, result: null });
      } else if (searchText === '') {
        searchFound = [];
        await emit('search', { text: '', switches: searchSwitches, problem: null, result: null });
      } else {
        // As the searcher's thread: the answer comes a moment later.
        setTimeout(() => void runSearch(), 20);
      }
      return null;
    },
    search_page: ({ serial, from }) =>
      serial === searchSerial ? searchFound.slice(from, from + SEARCH_PAGE) : null,
    replace_set: async ({ text, replacement, switches }) => {
      if (
        text !== replacing.text ||
        JSON.stringify(switches) !== JSON.stringify(replacing.switches)
      ) {
        replacing.leftNotes.clear();
        replacing.leftPlaces.clear();
      }
      Object.assign(replacing, { open: true, text, replacement, switches });
      replacing.asked += 1;
      // As the searcher's thread: the answer comes a moment later.
      setTimeout(() => void runReplace(), 20);
      return replacing.asked;
    },
    replace_page: ({ serial, from }) => (serial === replacing.serial ? replacePage(from) : null),
    replace_leave_out: ({ serial, note, at, out }) => {
      const text = replacing.seen.get(note);
      if (serial !== replacing.serial || text === undefined) {
        return null;
      }
      const set = at === null ? replacing.leftNotes : replacing.leftPlaces;
      const key = at === null ? note : placeKey(note, text, at);
      if (out) {
        set.add(key);
      } else {
        set.delete(key);
      }
      return chosenView();
    },
    replace_confirm: async ({ serial, note }) => {
      if (serial !== replacing.serial) {
        const text =
          'The places changed while you looked at them; look at them again before you replace';
        await setStatus(text);
        return text;
      }
      const done: { note: string; before: string; after: string }[] = [];
      const failed: string[] = [];
      const changedMeanwhile: string[] = [];
      let places = 0;
      for (const found of replacing.notes) {
        if ((note !== null && found.note !== note) || replacing.leftNotes.has(found.note)) {
          continue;
        }
        const before = replacing.seen.get(found.note) ?? '';
        // BR-004, as the backend: only a note that still holds its text.
        const now = mirrors.get(found.note)?.text() ?? textOf(found.note);
        if (now !== before) {
          changedMeanwhile.push(found.title);
          continue;
        }
        const chosen = (replacing.all.get(found.note) ?? []).filter(
          (place) => !replacing.leftPlaces.has(placeKey(found.note, before, place.at)),
        );
        if (chosen.length === 0) {
          continue;
        }
        let after = before;
        for (const place of [...chosen].reverse()) {
          after = after.slice(0, place.at) + place.new + after.slice(place.at + place.old.length);
        }
        // UC-022 BR-005, UC-026 BR-001: the note before the replacement.
        takeSnapshot(found.note, before, 'ReplaceInWorkspace');
        const why = await replaceWorkspaceNote(found.note, before, after);
        if (why !== null) {
          failed.push(`${found.title} could not be changed: ${why}`);
          continue;
        }
        places += chosen.length;
        done.push({ note: found.note, before, after });
      }
      let text =
        done.length === 0 && failed.length === 0 && changedMeanwhile.length === 0
          ? 'Nothing was replaced: every place is left out'
          : done.length === 0
            ? 'Nothing was replaced'
            : `Replaced ${placesOf(places)} in ${placesOf(done.length, 'note')}`;
      if (changedMeanwhile.length > 0) {
        text += `; ${changedMeanwhile.join(', ')} ${changedMeanwhile.length === 1 ? 'was changed meanwhile and is left as it is' : 'were changed meanwhile and are left as they are'}`;
      }
      text = [text, ...failed].join('; ');
      replacing.undo = done.length > 0 ? { status: text, notes: done } : null;
      await setStatus(text, done.length > 0 ? 'undoReplace' : null);
      await sendAll();
      void runReplace();
      return text;
    },
    replace_undo: async () => {
      const undone = replacing.undo;
      if (undone === null || undone.status !== status()) {
        return status();
      }
      replacing.undo = null;
      const changedSince: string[] = [];
      let back = 0;
      for (const each of undone.notes) {
        const now = mirrors.get(each.note)?.text() ?? textOf(each.note);
        if (now !== each.after) {
          changedSince.push(title(each.note));
          continue;
        }
        takeSnapshot(each.note, each.after, 'Restore');
        await replaceWorkspaceNote(each.note, each.after, each.before);
        back += 1;
      }
      let text = `Took back the replacement in ${placesOf(back, 'note')}`;
      if (changedSince.length > 0) {
        text += `; ${changedSince.join(', ')} ${changedSince.length === 1 ? 'was changed since and is left as it is' : 'were changed since and are left as they are'}`;
      }
      await setStatus(text);
      await sendAll();
      return text;
    },
    replace_close: () => {
      replacing.open = false;
      replacing.notes = [];
      return null;
    },
    snapshots_list: ({ note }) => {
      if (!root || !files[root]?.includes(note)) {
        throw { kind: 'invalid', message: `The note ${title(note)} is gone` };
      }
      snapshotsShown = note;
      const now = noteNow(note);
      const items = (snapshotsOf.get(note) ?? []).map((snapshot) => ({
        key: snapshot.key,
        taken: snapshotTime(snapshot.taken),
        cause: MOCK_CAUSE_LABELS[snapshot.cause],
        differs: mockLinesDiffering(snapshot.text, now),
        problem: null,
      }));
      return {
        note,
        title: title(note),
        items,
        none:
          items.length === 0 ? { lead: MOCK_NO_SNAPSHOT, when: [...MOCK_SNAPSHOTS_TAKEN] } : null,
        kept: MOCK_SNAPSHOTS_KEPT,
        problem: null,
      };
    },
    snapshot_diff: ({ note, key }) => {
      const found = snapshotsOf.get(note)?.find((snapshot) => snapshot.key === key);
      if (!found) {
        return { note, key, body: { kind: 'problem', text: 'The snapshot is gone' } };
      }
      const now = noteNow(note);
      if (found.text === now) {
        return { note, key, body: { kind: 'same' } };
      }
      return { note, key, body: { kind: 'hunks', hunks: mockHunks(found.text, now) } };
    },
    snapshot_whole: ({ note, key }) => {
      const found = snapshotsOf.get(note)?.find((snapshot) => snapshot.key === key);
      if (!found) {
        return { note, key, blocks: [], problem: 'The snapshot is gone' };
      }
      return { note, key, blocks: fakeBlocks(found.text), problem: null };
    },
    snapshot_restore: async ({ note, key }) => restoreSnapshot(note, key, false),
    snapshot_undo_restore: async () => {
      const undo = restoreUndo;
      if (undo === null || undo.status !== status()) {
        return status();
      }
      restoreUndo = null;
      return restoreSnapshot(undo.note, undo.key, true);
    },
    snapshots_close: () => {
      snapshotsShown = null;
      return null;
    },
    // UC-026 A7, OQ-294: a rough stand-in for `SnapshotStore::usage`, with
    // the texts of `session/snapshots.rs`.
    snapshots_space: () => mockSnapshotsSpace([...snapshotsOf.values()].flat()),
    snapshots_remove_all: async () => {
      const all = [...snapshotsOf.values()].flat();
      snapshotsOf.clear();
      restoreUndo = null;
      const report =
        all.length === 0
          ? 'There were no snapshots to remove'
          : `Removed all snapshots of this workspace: ${mockSnapshotsAmount(all)}`;
      await setStatus(report);
      if (snapshotsShown !== null) {
        void emit('snapshots-changed', snapshotsShown);
      }
      return report;
    },
    // UC-023 BR-004, OQ-198: the user's recently run commands.
    palette_ran: ({ id }) => {
      ranCommands = [id, ...ranCommands.filter((known) => known !== id)].slice(0, 20);
      return null;
    },
    // OQ-220: the width of the split view, within 15 percent of each pane.
    split_share: async ({ share }) => {
      if (root !== null) {
        splitShares[root] = Math.min(850, Math.max(150, Math.round(share)));
        await sendAll();
      }
      return null;
    },
    // UC-005 A28: the corrections come a moment later, as from the backend.
    spell_suggest: async ({ word, note }) => {
      await new Promise((resolve) => setTimeout(resolve, 30));
      const setting = model.spellLanguage();
      const text = mirrors.get(note)?.text() ?? '';
      const named = setting === 'off' ? null : noteLanguage(text, mockFormat(note) === 'AsciiDoc');
      const language = named === null ? setting : (offeredLanguage(named) ?? named);
      if (language === 'off') {
        return [];
      }
      if (language !== MOCK_SPELL_LANGUAGE) {
        throw { kind: 'internal', message: `No dictionary for ${language}` };
      }
      return speller.suggest(word);
    },
    spell_add_word: async ({ word }) => {
      const refused = speller.add(word);
      if (refused !== null) {
        throw { kind: 'invalid', message: refused };
      }
      await setStatus(`Added "${word.trim()}" to your dictionary`);
      spellAll();
      return null;
    },
    // OQ-208: the workspace dictionary, `underline-words.txt` in the workspace.
    spell_add_workspace_word: async ({ word }) => {
      const refused = speller.add(word, 'workspace');
      if (refused !== null) {
        throw { kind: 'invalid', message: refused };
      }
      await setStatus(`Added "${word.trim()}" to the workspace dictionary`);
      spellAll();
      return null;
    },
    search_open: async ({ note, place }) => {
      if (!root || !files[root]?.includes(note)) {
        await setStatus(`${title(note)} cannot be read: note \`${note}\` not found`);
        return null;
      }
      openTab(note);
      await sendAll();
      if (place === null) {
        return null;
      }
      await loadEditorCore();
      let mirror = mirrors.get(note);
      if (!mirror) {
        mirror = new MirrorCore(textOf(note));
        mirrors.set(note, mirror);
      }
      const text = mirror.text();
      return {
        note,
        rev: mirror.rev,
        anchor: webAtByte(text, place.from),
        head: webAtByte(text, place.to),
      };
    },
    links_open: async ({ note, place }) => {
      if (!root || !files[root]?.includes(note)) {
        await setStatus(`${title(note)} cannot be read: note \`${note}\` not found`);
        return null;
      }
      openTab(note);
      await sendAll();
      await loadEditorCore();
      let mirror = mirrors.get(note);
      if (!mirror) {
        mirror = new MirrorCore(textOf(note));
        mirrors.set(note, mirror);
      }
      const at = webAtByte(mirror.text(), place.from);
      return { note, rev: mirror.rev, anchor: at, head: at };
    },
    links_follow: async ({ note, at, rev }) => {
      const mirror = mirrors.get(note);
      if (!mirror || !isOpen(note)) {
        throw notOpen(note);
      }
      // As the backend: the link that starts there, in that revision.
      const link =
        mirror.rev === rev
          ? (mockFoundLinks(mirror.text()).find((found) => found.at === at)?.place ?? null)
          : null;
      return link === null ? { kind: 'done' } : followLink(note, link);
    },
    // UC-004 A22: the mention, found again at its place, becomes a link.
    links_link_mention: async ({ active, note, place }) => {
      const text = mirrors.get(note)?.text() ?? textOf(note);
      const found = mockMentions(active, [note], () => text, Infinity).notes[0]?.mentions.find(
        (mention) =>
          mention.place.from === place.from &&
          mention.place.to === place.to &&
          mention.place.check === place.check,
      );
      if (!root || !files[root]?.includes(active) || !files[root]?.includes(note)) {
        throw { kind: 'invalid', message: `${note} is no note of the workspace` };
      }
      if (!found) {
        const moved = `${title(note)} changed since the mentions were listed; the mention is no longer at its place`;
        await setStatus(moved);
        linksSoon();
        return moved;
      }
      const from = webAtByte(text, place.from);
      const to = webAtByte(text, place.to);
      const markdown = formatOf(note) === 'Markdown';
      const address = mockLinkPlace(fileOf(note), fileOf(active));
      const label = mockLinkText(found.mention, markdown);
      let link = markdown ? `[${label}](${address})` : `link:${address}[${label}]`;
      let start = from;
      // OQ-305: `!` would make a picture, an odd `\` would escape the `[`.
      const slashes = (/\\*$/.exec(text.slice(0, from))?.[0].length ?? 0) % 2 === 1;
      if (markdown && slashes) {
        link = `\\${link}`;
      } else if (markdown && text[from - 1] === '!') {
        start = from - 1;
        link = `\\!${link}`;
      }
      const open = mirrors.has(note);
      if (!open) {
        takeSnapshot(note, text, 'Editing');
      }
      const failed = await replaceWorkspaceNote(
        note,
        text,
        text.slice(0, start) + link + text.slice(to),
      );
      const linked = `Linked the mention of ${title(active)} in ${title(note)}.`;
      const status =
        failed !== null
          ? `${title(note)} could not be changed: ${failed}`
          : open
            ? `${linked} Undo with Ctrl+Z in ${title(note)}.`
            : `${linked} The earlier version is in its snapshots.`;
      await setStatus(status);
      linksSoon();
      return status;
    },
    links_more_mentions: ({ note }) => {
      if (mentionLimit.note === note) {
        mentionLimit.limit += MOCK_MENTIONS_SHOWN;
        linksSoon();
      }
      return null;
    },
    links_headings: ({ note }) =>
      root && files[root]?.includes(note)
        ? mockHeadings(mirrors.get(note)?.text() ?? textOf(note))
        : [],
    tab_open: async ({ note }) => {
      if (root && files[root]?.includes(note)) {
        openTab(note);
        await sendAll();
      } else {
        await setStatus(`${title(note)} cannot be read: note \`${note}\` not found`);
      }
      return null;
    },
    tab_open_side: async ({ note }) => {
      if (root && files[root]?.includes(note)) {
        openToSide(note);
        await sendAll();
      } else {
        await setStatus(`${title(note)} cannot be read: note \`${note}\` not found`);
      }
      return null;
    },
    tab_activate: async ({ note, pane: side }) => {
      if (side && other !== null && side !== pane) {
        focusPane(side);
      } else if (!side && !tabs.some((tab) => tab.note === note) && other) {
        focusPane(pane === 'left' ? 'right' : 'left');
      }
      if (tabs.some((tab) => tab.note === note)) {
        active = note;
        viewed(note);
        remember();
        await sendAll();
      }
      return null;
    },
    pane_focus: async ({ pane: side }) => {
      if (other !== null && side !== pane) {
        focusPane(side);
        remember();
        await sendAll();
      }
      return null;
    },
    split_end: async () => {
      endSplit();
      remember();
      await sendAll();
      return null;
    },
    tab_close: async ({ note, force, pane: side }) => {
      const back = pane;
      if (side && side !== pane) {
        focusPane(side);
      } else if (!side && !tabs.some((tab) => tab.note === note) && other) {
        focusPane(pane === 'left' ? 'right' : 'left');
      }
      const at = tabs.findIndex((tab) => tab.note === note);
      if (at < 0) {
        focusPane(back);
        return null;
      }
      if (other?.tabs.some((tab) => tab.note === note)) {
        // UC-005 BR-016: the note stays open in the other pane.
        tabs.splice(at, 1);
        if (active === note) {
          active = tabs[Math.min(at, tabs.length - 1)]?.note ?? null;
        }
        settlePanes();
        focusPane(back);
        remember();
        await sendAll();
        return null;
      }
      const failure = store(note);
      if (failure && !force) {
        focusPane(back);
        await sendAll();
        return { tab: note, what: `the tab ${title(note)}`, error: failure, switching: false };
      }
      dropTab(note, failure === null);
      settlePanes();
      focusPane(back);
      remember();
      await sendAll();
      return null;
    },
    tab_store: async ({ note }) => {
      unstored.add(note);
      const failure = store(note);
      await setStatus(
        failure
          ? `${title(note)} could not be stored: ${MOCK_SAVE_ERROR}`
          : `Stored ${title(note)}`,
      );
      await sendAll();
      return null;
    },
    tab_set_view: async ({ note, view, pane: side }) => {
      if (side && side !== pane && other) {
        other.tabs = other.tabs.map((tab) => (tab.note === note ? { ...tab, view } : tab));
      } else {
        tabs = tabs.map((tab) => (tab.note === note ? { ...tab, view } : tab));
      }
      remember();
      await emit('tabs', tabsView());
      return null;
    },
    clip_read_text: () => clipboard.text,
    clip_write_text: ({ text }) => {
      clipboard = { text, html: null, image: false };
      return null;
    },
    clip_read: ({ note, rich, overCopied }) => {
      const content = (overCopied ? before.get(note) : undefined) ?? clipboard;
      return {
        text: content.text || null,
        markup: rich !== null && content.html !== null ? mockHtmlToMarkup(content.html) : null,
        image: content.image && content.text === null && content.html === null,
      };
    },
    clip_write: ({ note, text, html, keepBefore }) => {
      if (keepBefore) {
        before.set(note, clipboard);
      }
      clipboard = { text, html, image: false };
      return null;
    },
    clip_store_image: async ({ note, overCopied }) => {
      const content = (overCopied ? before.get(note) : undefined) ?? clipboard;
      if (!content.image) {
        return null;
      }
      pastedImages += 1;
      const name = `${title(note).replace(/ /g, '-')}-2026-11-10-1200${pastedImages > 1 ? `-${pastedImages}` : ''}.png`;
      await setStatus(`Pasted image stored as ${name}`);
      return name;
    },
    ed_embed: async ({ note, path, allowLarge }) => embed(note, path, allowLarge),
    ed_drop: async ({ note, paths }) => {
      if (paths.length === 0) {
        await setStatus('This cannot be dropped into a note');
        return [];
      }
      const answers: EmbedAnswer[] = [];
      for (const [n, path] of paths.entries()) {
        if (n > 0) {
          answers.push({ kind: 'insert', insertion: { kind: 'text', text: ' ' } });
        }
        if (/^(https?:\/\/|mailto:)/.test(path)) {
          answers.push({ kind: 'insert', insertion: { kind: 'link', address: path, label: null } });
          continue;
        }
        const answer = await embed(note, path, false);
        if (answer.kind === 'insert' && answer.insertion.kind === 'file') {
          const { place, image } = answer.insertion;
          const dropped: Insertion = { kind: 'dropped', place, image };
          answers.push({ kind: 'insert', insertion: dropped });
        } else {
          answers.push(answer);
        }
      }
      return answers;
    },
    ed_drag: async ({ state }) => {
      const hint = 'Drop to insert at the place under the pointer';
      if (state === 'over') {
        await setStatus(hint);
      } else if (state === 'noNote') {
        await setStatus('Open a note to drop files into it');
      } else if (status() === hint) {
        await setStatus('');
      }
      return null;
    },
    clip_forget: ({ note }) => {
      before.delete(note);
      return null;
    },
    // OQ-248: as link completion writes the place and the text.
    ed_note_link: ({ note, target }) => ({
      kind: 'link',
      address: mockLinkPlace(fileOf(note), fileOf(target)),
      label: mockLinkText(title(target), formatOf(note) === 'Markdown'),
    }),
    ed_viewport: async ({ note, from, to }) => {
      const mirror = mirrors.get(note);
      const lines = (mirror?.text() ?? textOf(note)).split('\n').slice(from, to);
      await emit('ed-highlight', {
        note,
        // ADR-025: the colours carry the mirror's revision.
        rev: mirror?.rev ?? 0,
        from,
        lines: lines.map((line) =>
          line.startsWith('#') ? [[0, line.length, 0] as [number, number, number]] : [],
        ),
        styles: [{ color: model.effective().palette.heading, bold: true, italic: false }],
      });
      return null;
    },
    app_close_anyway: () => {
      windowClosed = true;
      return null;
    },
    ed_mirror: async ({ note }) => {
      if (!isOpen(note)) {
        throw notOpen(note);
      }
      await loadEditorCore();
      let mirror = mirrors.get(note);
      if (!mirror) {
        mirror = new MirrorCore(textOf(note));
        mirrors.set(note, mirror);
      }
      // The colours and figures follow, as the backend's tick sends them.
      setTimeout(() => void sendEnrich(note), 20);
      // FR-099: and the marks of the spell check.
      setTimeout(() => void sendSpell(note), 30);
      setTimeout(() => void sendIndexes(), 20);
      return {
        note,
        rev: mirror.rev,
        text: mirror.text(),
        format: formatOf(note),
        anchor: 0,
        head: 0,
      };
    },
    ed_sync: async ({ note, batch }) => {
      const mirror = mirrors.get(note);
      if (!mirror || !isOpen(note)) {
        throw notOpen(note);
      }
      const before = mirror.text();
      const { answer, reverted } = mirror.receive(batch);
      if (mirror.text() !== before) {
        // UC-026 BR-001: a snapshot of the text before, every 10 minutes.
        const newest = snapshotsOf.get(note)?.[0];
        if (newest === undefined || Date.now() - newest.taken > 10 * 60 * 1000) {
          takeSnapshot(note, before, 'Editing');
        }
        searchAgain();
        gitAgain();
        if (quiet.delete(note)) {
          // The user's own change: the tab shows its mark now.
          unstored.delete(note);
        }
        followMirror(note, mirror);
        await changed(note);
        setTimeout(() => void sendEnrich(note), 0);
        setTimeout(() => void sendIndexes(), 0);
        spellLater(note);
      }
      if (reverted !== null) {
        // UC-004 A9, as the backend says it (`session/sync.rs`).
        await setStatus(
          `${title(note)} was changed outside while you were editing; your text stays, the other version is kept as ${title(note)} (changed outside).md`,
        );
      }
      return answer;
    },
    ed_resync: async ({ note, text }) => {
      const mirror = mirrors.get(note);
      if (!mirror) {
        throw notOpen(note);
      }
      const answer = mirror.resync(text);
      followMirror(note, mirror);
      await changed(note);
      return answer;
    },
    app_request_close: async ({ sent }) => {
      // ADR-025 section 3.9: without `sent`, the window asks itself.
      if (sent) {
        await requestClose();
      }
      return null;
    },
    ed_follow_link: async ({ note, at, rev, side }) => {
      const mirror = mirrors.get(note);
      if (!mirror || !isOpen(note)) {
        throw notOpen(note);
      }
      // As the backend: a place of another revision follows nothing.
      const link = mirror.rev === rev ? mockLinkAt(mirror.text(), at) : null;
      return link === null ? { kind: 'done' } : followLink(note, link, side ?? false);
    },
    note_new_start: () => {
      if (root === null) {
        return null;
      }
      const place = settingFolder('template_folder', 'templates');
      return {
        folder: (active && folder(active)) ?? '',
        format: model.defaultFormat('Markdown') as FormatName,
        templates: templateNotes().map((note) => {
          const below = 'folder' in place ? note.slice(place.folder.length + 1) : note;
          const at = below.lastIndexOf('/');
          return {
            note,
            label: at < 0 ? title(note) : `${title(note)} (${below.slice(0, at)})`,
            format: formatOf(note),
          };
        }),
        templateFolder: 'folder' in place ? place.folder : 'templates',
      };
    },
    note_plan: ({ request }) => {
      const planned = planNote(request.title, request.folder);
      return 'id' in planned ? `${planned.id}.${extension(request.format)}` : null;
    },
    note_create: async ({ request }): Promise<CreateNoteAnswer> => {
      const planned = planNote(request.title, request.folder);
      if (!('id' in planned)) {
        return planned.existing === undefined
          ? { kind: 'problem', problem: planned.problem, existing: null }
          : {
              kind: 'problem',
              problem: `The note ${title(planned.existing)} exists. Change the title or the folder, or open it.`,
              existing: planned.existing,
            };
      }
      if (!root || request.folder.split('/').includes('locked')) {
        await setStatus(`The note could not be created: cannot write the note: ${MOCK_SAVE_ERROR}`);
        return { kind: 'done' };
      }
      const id = planned.id;
      // UC-003 A7, A8: from the template chosen, if it is one still.
      const chosen = request.template ?? null;
      const template = chosen !== null && templateNotes().includes(chosen) ? chosen : null;
      const { date, time } = localClock();
      const format = template === null ? request.format : formatOf(template);
      const filled =
        template === null
          ? { text: newNoteText(request.title, format), cursor: null }
          : fillTemplate(textOf(template), request.title, date, time);
      const rev = await addNote(id, filled.text, format, request.side ?? false);
      await setStatus(
        chosen !== null && template === null
          ? `Created the note ${title(id)} with only its title: the template ${chosen}.md could not be read (it is no template of the workspace now)`
          : `Created the note ${title(id)}`,
      );
      return {
        kind: 'created',
        note: id,
        rev,
        offset: filled.cursor ?? filled.text.length,
      };
    },
    // UC-025 steps 1 to 4, A1, A3 to A5, roughly as `open_daily_note`.
    note_daily_open: async (): Promise<DailyNoteAnswer> => {
      if (root === null) {
        return { kind: 'done' };
      }
      const { date, time } = localClock();
      const place = settingFolder('daily_note_folder', 'daily');
      if (!('folder' in place)) {
        await setStatus(
          `The daily note cannot be created in the daily note folder: ${place.problem}`,
          'workspaceSettings',
        );
        return { kind: 'done' };
      }
      const id = place.folder === '' ? date : `${place.folder}/${date}`;
      const existing = (files[root] ?? []).find((note) => note.toLowerCase() === id.toLowerCase());
      if (existing !== undefined) {
        const wasOpen = isOpen(existing);
        openTab(existing);
        await sendAll();
        await setStatus('');
        if (wasOpen) {
          return { kind: 'shown', note: existing };
        }
        const mirror = mirrors.get(existing);
        const text = mirror?.text() ?? textOf(existing);
        return { kind: 'opened', note: existing, rev: mirror?.rev ?? 1, offset: text.length };
      }
      if (place.folder.split('/').includes('locked')) {
        await setStatus(
          `The daily note could not be created: cannot create the folder: ${MOCK_SAVE_ERROR}`,
        );
        return { kind: 'done' };
      }
      // BR-003: the template the settings name, else the one titled Daily.
      const named = model.workspaceValue('daily_note_template')?.trim() || null;
      const templates = templateNotes();
      const template =
        named === null
          ? (templates.find((note) => note.split('/').at(-1)?.toLowerCase() === 'daily') ?? null)
          : ((files[root] ?? []).find(
              (note) =>
                note === named.replace(/\.(md|adoc)$/, '') &&
                (!/\.(md|adoc)$/.test(named) || named.endsWith(`.${extension(formatOf(note))}`)),
            ) ?? null);
      const format: FormatName =
        template === null ? (model.defaultFormat('Markdown') as FormatName) : formatOf(template);
      const filled =
        template === null
          ? { text: newNoteText(date, format), cursor: null }
          : fillTemplate(textOf(template), date, date, time);
      const rev = await addNote(id, filled.text, format);
      await setStatus(
        named !== null && template === null
          ? `Created the daily note ${date} with only its title: the daily note template ${named} was not found`
          : `Created the daily note ${date}`,
      );
      return { kind: 'opened', note: id, rev, offset: filled.cursor ?? filled.text.length };
    },
    // UC-025 A2: the nearest earlier or later daily note of the folder.
    note_daily_step: async ({ later }): Promise<DailyNoteAnswer> => {
      const place = settingFolder('daily_note_folder', 'daily');
      const dateOf = (note: string): string | null => {
        const at = note.lastIndexOf('/');
        const inFolder = 'folder' in place && (at < 0 ? '' : note.slice(0, at)) === place.folder;
        const page = note.slice(at + 1);
        return inFolder && /^\d{4}-\d{2}-\d{2}$/.test(page) ? page : null;
      };
      const current = active === null ? null : dateOf(active);
      if (root === null || current === null) {
        return { kind: 'notDaily' };
      }
      const days = (files[root] ?? [])
        .map((note) => ({ note, date: dateOf(note) }))
        .filter((day): day is { note: string; date: string } => day.date !== null)
        .sort((a, b) => a.date.localeCompare(b.date));
      const next = later
        ? days.find((day) => day.date > current)
        : days.filter((day) => day.date < current).at(-1);
      if (next === undefined) {
        await setStatus(`There is no ${later ? 'later' : 'earlier'} daily note`);
        return { kind: 'done' };
      }
      openTab(next.note);
      await sendAll();
      await setStatus('');
      return { kind: 'shown', note: next.note };
    },
    note_rename_start: ({ note }) =>
      conflictRefused(note, 'renaming')
        ? null
        : {
            note,
            title: title(note),
            folder: folder(note) ?? '',
            home: note === 'Home',
          },
    note_plan_rename: ({ note, title: newTitle, folder: newFolder }) => {
      const planned = planNote(newTitle, newFolder, note);
      return 'id' in planned && planned.id !== note ? `${planned.id}.md` : null;
    },
    note_rename: async ({ note, title: newTitle, folder: newFolder }): Promise<RenameAnswer> => {
      const planned = planNote(newTitle, newFolder, note);
      if (!('id' in planned)) {
        return {
          kind: 'problem',
          problem:
            planned.existing === undefined
              ? planned.problem
              : `A note named ${title(planned.existing)} exists in that folder. Change the title or the folder.`,
        };
      }
      const to = planned.id;
      if (to === note || !root) {
        return { kind: 'done' };
      }
      // Step 5: the tab is stored first; its pending autosave goes.
      clearTimeout(saveTimers.get(note));
      saveTimers.delete(note);
      const failure = store(note);
      if (failure) {
        await setStatus(`The note could not be renamed, because ${failure}`);
        return { kind: 'done' };
      }
      renameEverywhere(note, to);
      // UC-026 BR-003: the snapshots follow the note.
      const followed = snapshotsOf.get(note);
      if (followed) {
        snapshotsOf.set(
          to,
          [...followed, ...(snapshotsOf.get(to) ?? [])].sort((a, b) => b.taken - a.taken),
        );
        snapshotsOf.delete(note);
      }
      let links = 0;
      let notes = 0;
      for (const other of files[root] ?? []) {
        if (other === to) {
          continue;
        }
        const forms = linkForms(other, note, to);
        const mirror = mirrors.get(other);
        const text = mirror?.text() ?? textOf(other);
        const found = countAll(text, forms);
        if (found === 0) {
          continue;
        }
        links += found;
        notes += 1;
        if (!mirror) {
          texts.set(
            other,
            forms.reduce((now, [needle, by]) => now.split(needle).join(by), text),
          );
          continue;
        }
        // UC-006 A7: the open note changes in its text, saved quietly.
        let rev = mirror.rev;
        for (const [needle, by] of forms) {
          const now = mirror.text();
          let at = now.lastIndexOf(needle);
          while (at >= 0) {
            rev = mirror.backendEdit(at, needle.length, by, false) ?? rev;
            at = at === 0 ? -1 : now.lastIndexOf(needle, at - 1);
          }
        }
        followMirror(other, mirror);
        // M-14.2: no mark for the rename's links of a tab without changes.
        if (!unstored.has(other)) {
          quiet.add(other);
        }
        await changed(other);
        await emit('ed-changed', { note: other, rev });
      }
      await sendAll();
      await setStatus(
        `Renamed ${title(note)} to ${title(to)}; updated ${links === 1 ? '1 link' : `${links} links`} in ${notes === 1 ? '1 note' : `${notes} notes`}`,
      );
      return { kind: 'renamed', from: note, to };
    },
    note_delete_start: ({ note }) => {
      if (conflictRefused(note, 'deleting')) {
        return null;
      }
      let links = 0;
      let notes = 0;
      for (const other of root ? (files[root] ?? []) : []) {
        const found =
          other === note
            ? 0
            : countAll(mirrors.get(other)?.text() ?? textOf(other), linkForms(other, note, note));
        links += found;
        notes += found > 0 ? 1 : 0;
      }
      return {
        note,
        title: title(note),
        links:
          links === 0
            ? 'No links in other notes lead to it.'
            : `${links === 1 ? '1 link' : `${links} links`} in ${notes === 1 ? '1 note' : `${notes} notes`} lead to it and will lead nowhere.`,
        home: note === 'Home',
      };
    },
    note_delete: async ({ note, forGood }) => {
      if (!root) {
        return { kind: 'done' };
      }
      if (keepFails && !forGood) {
        return {
          kind: 'cannotKeep',
          question: `The note cannot be kept among the deleted notes (${MOCK_SAVE_ERROR}), so it cannot be restored. Delete it for good?`,
        };
      }
      const text = mirrors.get(note)?.text() ?? textOf(note);
      const reopen = isOpen(note);
      files[root] = (files[root] ?? []).filter((n) => n !== note);
      const at = tabs.findIndex((tab) => tab.note === note);
      if (at >= 0) {
        clearTimeout(saveTimers.get(note));
        tabs.splice(at, 1);
        if (active === note) {
          active = tabs[Math.min(at, tabs.length - 1)]?.note ?? null;
        }
      }
      // UC-026 BR-003: the snapshots go with the note (kept with it below).
      const keptSnapshots = snapshotsOf.get(note);
      forgetOther(note);
      for (const map of [texts, mirrors, saveErrors, snapshotsOf] as Map<string, unknown>[]) {
        map.delete(note);
      }
      unstored.delete(note);
      delete lastOpened[root]?.[note];
      remember();
      const report = `Deleted ${title(note)}`;
      undo = null;
      if (!forGood) {
        clock += 1;
        const deleted: MockDeleted = {
          key: `${Date.now()}:${clock}:${note}`,
          note,
          text,
          at: Date.now(),
        };
        deletedNotes[root] = [deleted, ...(deletedNotes[root] ?? [])];
        // UC-026 BR-003: the snapshots are kept with the deleted note.
        if (keptSnapshots) {
          snapshotsDeleted.set(deleted.key, keptSnapshots);
        }
        undo = { status: report, deleted, reopen };
      }
      await sendAll();
      await setStatus(report, forGood ? null : 'undoDelete');
      return { kind: 'done' };
    },
    note_undo_delete: async () => {
      const offer = undo;
      undo = null;
      if (offer === null || status() !== offer.status) {
        return { kind: 'done' };
      }
      return restore(offer.deleted, offer.reopen, null);
    },
    ws_restore: async ({ key }) => {
      const deleted = (root ? (deletedNotes[root] ?? []) : []).find((d) => d.key === key);
      return deleted ? restore(deleted, false, null) : { kind: 'done' };
    },
    ws_restore_as: async ({ title: newTitle, folder: newFolder }) =>
      restoring
        ? restore(restoring.deleted, restoring.reopen, { title: newTitle, folder: newFolder })
        : { kind: 'done' },
    ws_purge: async ({ key }) => {
      if (!root) {
        return null;
      }
      const all = deletedNotes[root] ?? [];
      const targets = key === null ? all : all.filter((d) => d.key === key);
      deletedNotes[root] = all.filter((d) => !targets.includes(d));
      if (undo && targets.includes(undo.deleted)) {
        undo = null;
      }
      await sendAll();
      await setStatus(
        `Removed ${targets.length === 1 ? '1 deleted note' : `${targets.length} deleted notes`} for good`,
      );
      return null;
    },
    export_start: async ({ note }) => {
      if (!root || conflictRefused(note, 'exporting')) {
        return null;
      }
      return { note, title: title(note), choices: FAKE_EXPORT_KINDS.markdown };
    },
    export_choose: async ({ note, kind }): Promise<ExportPlaceAnswer> => {
      // One dialog at a time (review F11); not a note in conflict (UC-015 BR-007).
      if (!root || choosing !== null || conflictRefused(note, 'exporting')) {
        return { kind: 'done' };
      }
      const asking = `Choosing where to save ${title(note)}…`;
      choosing = { status: asking, before: status() };
      // App-tester F11: the status before comes back whole after a cancel.
      const beforeExtra = { exports: extra.exports, error: extra.error };
      await setStatus(asking);
      const dialog = exportDialog;
      // The user takes a moment in the dialog.
      await new Promise((resolve) => setTimeout(resolve, 20));
      const before = choosing.before;
      choosing = null;
      if (dialog.answer !== 'choose' && status() === asking) {
        extra.exports = beforeExtra.exports;
        extra.error = beforeExtra.error;
        await setStatus(
          before,
          undo !== null && undo.status === before ? 'undoDelete' : null,
          true,
        );
      }
      if (dialog.answer === 'cancel') {
        return { kind: 'done' };
      }
      if (dialog.answer === 'unavailable') {
        return {
          kind: 'typed',
          heading: `Export ${title(note)} as ${exportLabel(kind)} to:`,
          path: `${exportFolder()}/${exportName(note, kind)}`,
        };
      }
      const path = dialog.path ?? `${exportFolder()}/${exportName(note, kind)}`;
      exportChosen = { note, kind, path };
      return { kind: 'chosen', path };
    },
    export_run: async ({ note, kind, typed }): Promise<ExportAnswer> => {
      if (conflictRefused(note, 'exporting')) {
        return { kind: 'done' };
      }
      let path: string;
      if (typed !== null) {
        path = typed.path.trim();
        if (path === '') {
          return { kind: 'problem', problem: 'Enter the file to export to.' };
        }
        if (isFolder(path)) {
          return { kind: 'problem', problem: `${path} is a folder. Enter a file name in it.` };
        }
      } else {
        const chosen = exportChosen;
        exportChosen = null;
        if (chosen === null || chosen.note !== note || chosen.kind !== kind) {
          return { kind: 'done' };
        }
        path = chosen.path;
      }
      if (!root) {
        return { kind: 'done' };
      }
      if (isWorkspaceFile(path) || tabs.some((tab) => path === `${root}/${tab.note}.md`)) {
        const refused = 'A note cannot be replaced by an export; choose another place';
        await setStatus(refused);
        return typed === null ? { kind: 'askAgain' } : { kind: 'problem', problem: refused };
      }
      // Only a file that may be replaced is asked about (app-tester F11).
      if (typed !== null && fileExists(path) && typed.replace?.trim() !== path) {
        return {
          kind: 'exists',
          problem: 'The file exists. Export again to replace it.',
          path,
        };
      }
      // BR-001: the tab's text with its changes not yet saved.
      const text = mirrors.get(note)?.text() ?? textOf(note);
      lastExportFolder = path.slice(0, Math.max(path.lastIndexOf('/'), 0)) || '/';
      const running = `Exporting ${title(note)}…`;
      exportsRunning.push(running);
      extra.busy = true;
      await setStatus(running);
      await new Promise((resolve) => setTimeout(resolve, kind === 'pdf' ? 800 : 100));
      exportsRunning.splice(exportsRunning.indexOf(running), 1);
      extra.busy = exportsRunning.length > 0;
      let line: string;
      let failure: string | null = null;
      let report: (typeof exportReports)[number];
      if (path.startsWith('/home/user/locked/')) {
        line = `${title(note)} could not be exported: Permission denied (os error 13)`;
        failure = line;
        report = { title: title(note), target: null, open: false, approximated: [] };
      } else {
        const lines = text.split('\n');
        const approximated =
          kind === 'otherFormat'
            ? lines.flatMap((l, n) =>
                l.startsWith('|') && !(lines[n - 1] ?? '').startsWith('|') ? [n + 1] : [],
              )
            : [];
        const missing =
          kind === 'otherFormat'
            ? []
            : [...text.matchAll(/!\[[^\]]*\]\(([^)]*missing[^)]*)\)/g)].map(
                (m) => `the image ${m[1]} (No such file or directory (os error 2))`,
              );
        exported.set(path, text);
        line = `Exported ${title(note)} to ${path}`;
        if (approximated.length > 0) {
          line += `; ${approximated.length === 1 ? '1 block' : `${approximated.length} blocks`} only approximated`;
        }
        if (missing.length > 0) {
          line += `; missing: ${missing.join(', ')}`;
        }
        const extensions =
          kind === 'otherFormat' ? ['md', 'adoc'] : kind === 'html' ? ['html', 'htm'] : ['pdf'];
        const open = extensions.includes(path.split('.').at(-1)?.toLowerCase() ?? '');
        report = { title: title(note), target: path, open, approximated };
        // A6: an export into the workspace is a new note.
        if (kind === 'otherFormat' && path.startsWith(`${root}/`) && path.endsWith('.adoc')) {
          const added = path.slice(root.length + 1, -'.adoc'.length);
          if (!(files[root] ?? []).includes(added)) {
            files[root] = [...(files[root] ?? []), added];
            await sendAll();
          }
        }
      }
      // A-120: reports of exports ending while earlier ones show are kept.
      const shown = exportReports.length > 0 && reportsStatus === status();
      if (!shown) {
        exportReports = [];
        extra.error = false;
      }
      exportReports.push(report);
      reportsStatus = shown ? `${reportsStatus}\n${line}` : line;
      if (exportReports.length > 3) {
        exportReports.shift();
        reportsStatus = reportsStatus.slice(reportsStatus.indexOf('\n') + 1);
      }
      extra.error ||= failure !== null;
      extra.exports = exportReports.map((r) => ({
        title: r.title,
        open: r.open,
        show: r.approximated.length > 0,
      }));
      await setStatus(reportsStatus, null, true);
      return failure !== null && typed !== null
        ? { kind: 'problem', problem: failure }
        : { kind: 'done' };
    },
    export_open: async ({ index }) => {
      const report = reportsStatus === status() ? exportReports[index] : undefined;
      if (report?.open && report.target !== null) {
        openWithSystem(report.target);
      }
      return null;
    },
    export_approximated: ({ index }) => {
      const report = reportsStatus === status() ? exportReports[index] : undefined;
      return report ? { title: report.title, lines: report.approximated } : null;
    },
    git_choose: async ({ path }) => {
      const change = gitChanges().find((c) => c.path === path);
      if (!change) {
        return null;
      }
      if (change.kind === 'deleted' || change.note === null) {
        gitChosen = path;
      } else {
        gitChosen = null;
        openTab(change.note);
      }
      await sendAll();
      return null;
    },
    git_commit_choice: async ({ path, chosen }) => {
      if (path !== null) {
        if (chosen) {
          gitLeftOut.delete(path);
        } else {
          gitLeftOut.add(path);
        }
      } else if (chosen) {
        gitLeftOut.clear();
      } else {
        for (const change of gitChanges()) {
          gitLeftOut.add(change.path);
        }
      }
      await sendGit();
      return null;
    },
    git_commit: async ({ message, amend }) => gitCommit(message, amend),
    git_set_identity: async ({ name, email, message, amend }) => {
      if (name.trim() === '' || !email.includes('@')) {
        return { kind: 'done' };
      }
      if (gitIdentityFails !== null) {
        const problem = `Your name could not be kept: ${gitIdentityFails}`;
        await setStatus(problem);
        return { kind: 'nameNotKept', problem };
      }
      gitIdentity = true;
      return gitCommit(message, amend);
    },
    git_discard_start: async ({ path }) => {
      const git = gitOf();
      const change = gitChanges().find((c) => c.path === path);
      if (!git || !change || change.note === null) {
        return null;
      }
      gitDiscarding = path;
      const consequence =
        change.kind === 'new'
          ? 'The note is new; it goes to the deleted notes.'
          : change.kind === 'deleted'
            ? 'The note comes back as it was committed.'
            : (() => {
                const lines = mockChangedLines(
                  git.committed.get(change.from ?? change.note) ?? '',
                  change.text ?? '',
                );
                return `${lines === 1 ? '1 changed line' : `${lines} changed lines`} will be lost and cannot be restored.`;
              })();
      return {
        path,
        question: `Discard the changes of ${title(change.note)} since the last commit?`,
        consequence,
      };
    },
    git_discard: async ({ path }) => {
      if (gitDiscarding !== path) {
        return null;
      }
      gitDiscarding = null;
      await gitDiscard(path);
      return null;
    },
    git_branches: async () => {
      const state = remoteOf();
      if (!state || state.problem !== null) {
        return null;
      }
      const entries: BranchEntryView[] = [
        ...state.local.map((name) => ({
          name,
          label: name === state.current ? `${name}  (current)` : name,
          current: name === state.current,
          remote: false,
        })),
        ...state.remoteOnly.map((name) => ({
          name,
          label: `${name}  (on the remote)`,
          current: false,
          remote: true,
        })),
      ];
      return {
        branches: entries,
        problem: null,
        merging: state.conflicts.length > 0 || gitMerging,
      };
    },
    git_branch_name: ({ typed }) => mockBranchName(typed),
    git_switch: async ({ target }) => {
      const state = remoteOf();
      if (!state || state.conflicts.length > 0 || gitMerging) {
        return { kind: 'done' };
      }
      let name = target.name;
      if (target.kind === 'local' && name === state.current) {
        return { kind: 'done' };
      }
      if (target.kind === 'new') {
        const checked = mockBranchName(name);
        if (!checked.ok) {
          return { kind: 'invalid', problem: checked.text };
        }
        name = checked.text.slice('Branch: '.length);
        if (state.local.includes(name)) {
          return {
            kind: 'exists',
            problem: `The branch ${name} exists; choose it in the list to switch to it.`,
            name,
          };
        }
      }
      if (target.kind === 'remote') {
        name = name.slice(name.indexOf('/') + 1);
      }
      const failures = tabs.map((tab) => store(tab.note)).filter((f): f is string => f !== null);
      if (failures.length > 0) {
        await sendAll();
        await setStatus(`Did not switch, because ${failures.join('; ')}`);
        return { kind: 'done' };
      }
      if (branchInTheWay !== null) {
        const files = branchInTheWay;
        branchInTheWay = null;
        return {
          kind: 'inTheWay',
          text: `Changes not committed would be overwritten: ${files.join(', ')}. Commit them first, or stay.`,
        };
      }
      const carried = changesView()?.files.length ?? 0;
      if (target.kind !== 'local') {
        state.local = [...state.local, name].sort(byName);
        state.remoteOnly = state.remoteOnly.filter((r) => r !== target.name);
      }
      state.current = name;
      const closed: string[] = [];
      if (root === TEAM) {
        // The branch `drafts` holds the note Sketch.
        const notes = files[root] ?? [];
        const committed = gitOf()?.committed;
        if (name === 'drafts' && !notes.includes('Sketch')) {
          files[root] = [...notes, 'Sketch'];
          committed?.set('Sketch', textOf('Sketch'));
        } else if (name !== 'drafts' && notes.includes('Sketch')) {
          files[root] = notes.filter((n) => n !== 'Sketch');
          committed?.delete('Sketch');
          if (tabs.some((tab) => tab.note === 'Sketch')) {
            closeTabOf('Sketch');
            closed.push(title('Sketch'));
          }
        }
      }
      let said = `Switched to ${name}`;
      if (carried > 0) {
        said += `; carried along ${carried === 1 ? '1 change' : `${carried} changes`}`;
      }
      if (closed.length > 0) {
        said += `; closed ${closed.join(', ')}, not on this branch`;
      }
      await sendAll();
      await setReport(said);
      return { kind: 'done' };
    },
    sync_request: async ({ mode }) => {
      await runSync(mode, true);
      return null;
    },
    conflict_choose: async ({ note, rev, place, choice }) => {
      const state = remoteOf();
      const mirror = mirrors.get(note);
      // S3 (review F13): a choice of an older text does nothing.
      if (
        !state ||
        !mirror ||
        mirror.rev !== rev ||
        !state.conflicts.some((c) => c.note === note)
      ) {
        return null;
      }
      const places = mockConflictPlaces(mirror.text());
      const chosen = place === null ? places : places.slice(place, place + 1);
      if (chosen.length === 0) {
        return null;
      }
      // From the end, so the places before keep their offsets.
      for (const p of [...chosen].reverse()) {
        mirror.backendEdit(p.start, p.end - p.start, mockResolvePlace(p, choice), false);
      }
      followMirror(note, mirror);
      await changed(note);
      await emit('ed-changed', { note, rev: mirror.rev });
      await checkResolved();
      await sendGit();
      return null;
    },
    conflict_take_side: async ({ path, side }) => {
      const state = remoteOf();
      if (!state?.conflicts.some((c) => c.path === path && c.note === null)) {
        return null;
      }
      state.conflicts = state.conflicts.filter((c) => c.path !== path);
      if (path === '.todos/3.yaml') {
        state.todos = [];
      }
      await sendAll();
      await setReport(
        `Kept one version of ${path}; the other is ${mockCopyName(path, side === 'here' ? 'remote' : 'here')}`,
      );
      await afterResolving(state);
      return null;
    },
    conflict_keep: async ({ path, keep }) => {
      const state = remoteOf();
      const conflict = state?.conflicts.find((c) => c.path === path && c.deletedOn !== null);
      if (!state || !conflict) {
        return null;
      }
      if (!keep && conflict.note !== null && root !== null) {
        const note = conflict.note;
        clock += 1;
        deletedNotes[root] = [
          { key: `${Date.now()}:${clock}:${note}`, note, text: textOf(note), at: Date.now() },
          ...(deletedNotes[root] ?? []),
        ];
        files[root] = (files[root] ?? []).filter((n) => n !== note);
        closeTabOf(note);
      }
      state.conflicts = state.conflicts.filter((c) => c.path !== path);
      await sendAll();
      await afterResolving(state);
      return null;
    },
    conflict_abort: async () => {
      const state = remoteOf();
      if (!state || state.conflicts.length === 0) {
        return null;
      }
      state.conflicts = [];
      state.todos = [];
      if (state.hereText !== null) {
        await replaceText('Meetings', state.hereText);
        state.hereText = null;
      }
      // App-tester F13: no stale "Sync stopped: conflict".
      if (state.last?.outcome === 'conflict') {
        state.last = null;
      }
      await sendAll();
      await setReport('The sync was aborted; the workspace is as before it');
      return null;
    },
    ws_switch_anyway: async () => {
      const waiting = switchWaiting;
      switchWaiting = null;
      if (waiting === null) {
        return null;
      }
      if (root !== null) {
        todos.giveUp(root);
      }
      switchTo(waiting.dir);
      if (waiting.created) {
        openTab('Home');
      }
      await sendAll();
      await setStatus(
        waiting.created ? `Workspace created in ${waiting.dir}` : `Opened ${waiting.dir}`,
      );
      return null;
    },
    todo_create: async ({ title }) =>
      root === null ? { kind: 'failed' } : todoDone(todos.create(root, title)),
    todo_change: async ({ number, change }) =>
      root === null
        ? { kind: 'done' }
        : todoDone(todos.change(root, number, change, files[root] ?? [])),
    todo_mark: async ({ number, done }) =>
      root === null ? { kind: 'done' } : todoDone(todos.mark(root, number, done)),
    todo_delete: async ({ number }) =>
      root === null ? { kind: 'failed' } : todoDone(todos.delete(root, number)),
    todo_undo: async () => {
      const done = root === null ? null : todos.undo(root, status());
      return done === null ? { kind: 'done' } : todoDone(done);
    },
    todo_open_link: async ({ link }) => {
      const note = MockTodos.find(root ? (files[root] ?? []) : [], link);
      if (note === undefined) {
        await setStatus(`The note ${link} no longer exists; remove the link with ×`);
      } else {
        openTab(note);
        await sendAll();
      }
      return null;
    },
    todo_candidates: async ({ number, query }) =>
      root === null
        ? { notes: [], none: null }
        : todos.candidates(root, files[root] ?? [], number, query),
    todo_month: ({ day, step }) => mockCalendar(day, step),
    todo_list_show: async ({ filter }) => {
      // OQ-279: the tab "Todos" opens in the active pane (it moves there).
      const list = todos.list(root);
      list.open = true;
      list.active = true;
      list.pane = other === null ? 'left' : pane;
      if (filter !== null) {
        list.filter = filter;
      }
      await sendAll();
      return null;
    },
    todo_list_close: async () => {
      const list = todos.list(root);
      list.open = false;
      list.active = false;
      await sendAll();
      return null;
    },
    todo_list_set: async ({ sort, filter }) => {
      const list = todos.list(root);
      list.sort = sort;
      list.filter = filter;
      await sendTodos();
      return null;
    },
    todo_conflict_resolve: async ({ number, mine }) => {
      const state = remoteOf();
      if (!state?.todos.some((t) => t.number === number)) {
        return null;
      }
      if (root !== null) {
        todos.resolve(root, number, mine);
      }
      state.todos = state.todos.filter((t) => t.number !== number);
      state.conflicts = state.conflicts.filter((c) => c.path !== `.todos/${number}.yaml`);
      await sendAll();
      await afterResolving(state);
      return null;
    },
    // Q-61, Q-62: "Open" opens what was asked about; nothing is remembered.
    link_answer: async ({ id, open: answer }) => {
      if (openAsked === null || openAsked.id !== id) {
        return null;
      }
      const { target } = openAsked;
      openAsked = null;
      if (answer) {
        openWithSystem(target);
      }
      return null;
    },
    // F14, M-17.1: the agents menu changes the user's access (UC-017 BR-002).
    agents_set_access: async ({ level }) => {
      await setAgentLevel(level);
      return null;
    },
    // F14, M-17.2 to M-17.5: the section "Agents" over the fake tools.
    agents_section: async ({ action }) => {
      if (action !== null && root !== null) {
        const reads = () => levelOf(model.effective().agentAccess) >= 1;
        const done = agents.act(action, root, model.user.agent_access ?? 'Off', reads);
        if (done.level !== null && action.kind === 'allowAndInstall') {
          await setAgentLevel(done.level);
          agents.installAfterAllowing(action.tool, root, reads);
        }
        if (done.copied !== null) {
          clipboard = { text: done.copied, html: null, image: false };
        }
      }
      return agents.section(root);
    },
  };
  handlers.__hooks = {
    recentCommands: () => [...ranCommands],
    splitShare: () => (root === null ? null : (splitShares[root] ?? null)),
    addSnapshot: (note, text, cause = 'Editing', taken = Date.now()) =>
      takeSnapshot(note, text, mockCause(cause), taken).key,
    agentWrite: async (on) => {
      model.quick('agent_access', on ? 'Read and write' : 'Read only');
      await sendSettings();
    },
    snapshots: (note) =>
      (snapshotsOf.get(note) ?? []).map((snapshot) => ({
        cause: snapshot.cause,
        text: snapshot.text,
      })),
    closeWindow: async () => {
      await emit('ed-flush', { note: null, closing: true });
    },
    windowClosed: () => windowClosed,
    failSaves: (notes) => {
      failing.clear();
      for (const note of notes) {
        failing.add(note);
      }
    },
    backendEdit: async (note, at, remove, insert, sealed = true) => {
      const mirror = mirrors.get(note);
      const rev = mirror?.backendEdit(at, remove, insert, sealed);
      if (mirror && rev !== undefined) {
        followMirror(note, mirror);
        await changed(note);
        await emit('ed-changed', { note, rev });
        await sendIndexes();
      }
      return rev ?? null;
    },
    outsideChange: async (note, text) => {
      const mirror = mirrors.get(note);
      // F9, UC-008 A3: a change from outside searches again.
      searchAgain();
      if (!mirror) {
        // A note not open: its file holds the new text.
        if (root !== null && files[root]?.includes(note)) {
          texts.set(note, text);
          noteTouched(note);
          // FR-078: the backlinks follow (NFR-019).
          linksSoon();
        }
        return null;
      }
      // UC-026 BR-001: the text the tab showed.
      takeSnapshot(note, mirror.text(), 'OutsideChange');
      const rev = mirror.reload(text);
      followMirror(note, mirror);
      await emit('ed-changed', { note, rev });
      await setStatus(`${title(note)} was changed outside`);
      await sendIndexes();
      return rev;
    },
    mirrorText: (note) => mirrors.get(note)?.text() ?? null,
    noteText: (note) => textOf(note),
    agentConnect: async (name, refused = false) => {
      agents.connected = [...agents.connected.filter((a) => a.name !== name), { name, refused }];
      await sendAgents();
    },
    agentDisconnect: async (name) => {
      agents.connected = agents.connected.filter((a) => a.name !== name);
      await sendAgents();
      await sendAgentMarks();
    },
    agentRead: async (name, note, from, to, wrote = false) => {
      agents.cursors.set(name, { note, lines: [from, to], wrote, until: Date.now() + 10_000 });
      await sendAgentMarks();
    },
    agentChange: async (name, note, oldText, newText) => {
      const refused = agentRefusal(note);
      if (refused !== null) {
        return refused;
      }
      const mirror = mirrors.get(note);
      const text = mirror ? mirror.text() : textOf(note);
      const at = text.indexOf(oldText);
      if (oldText === '' || at < 0) {
        return `the passage to replace does not occur in the note \`${note}\`; read the note again and quote it exactly`;
      }
      const line = text.slice(0, at).split('\n').length;
      const lines = Math.max(1, newText.split('\n').length - (newText.endsWith('\n') ? 1 : 0));
      // UC-026 BR-001: the note before the agent's change.
      takeSnapshot(note, text, 'AgentChange');
      if (mirror) {
        // ADR-025: as the backend's change of an open note, an undo step of its own.
        const rev = mirror.backendEdit(
          Array.from(text.slice(0, at)).length,
          Array.from(oldText).length,
          newText,
          true,
        );
        followMirror(note, mirror);
        await changed(note);
        await emit('ed-changed', { note, rev });
        await sendIndexes();
        agents.cursors.set(name, {
          note,
          lines: [line, line + lines - 1],
          wrote: true,
          until: Date.now() + 10_000,
        });
        await sendAgentMarks();
      } else {
        texts.set(note, text.slice(0, at) + newText + text.slice(at + oldText.length));
        noteTouched(note);
        await reportAgentChange(name, 'changed', note);
      }
      const place = lines > 1 ? `lines ${line} to ${line + lines - 1}` : `line ${line}`;
      return `changed ${note}; the change is in ${place}`;
    },
    agentCreate: async (name, note, text) => {
      const refused = agentRefusal(note);
      if (refused !== null || root === null) {
        return refused ?? 'Underline is busy; try again';
      }
      if (files[root]?.includes(note)) {
        return `the note cannot be created: the note ${note} exists`;
      }
      files[root] = [...(files[root] ?? []), note];
      texts.set(note, text);
      await sendAll();
      await reportAgentChange(name, 'created', note);
      return `created ${note}`;
    },
    agentTodo: async (name, change, todo) => {
      // UC-020 A8: the agent's change shows within a second, the status names it.
      if (todo !== undefined && root !== null) {
        todos.outside(root, todo);
        later(300, sendTodos);
      }
      await setStatus(`${name} ${change}`);
    },
    todoOutside: (todo, at) => {
      const place = at ?? root;
      if (place !== null) {
        todos.outside(place, todo);
        later(300, sendTodos);
      }
    },
    todoWriteFails: (reason) => {
      todos.writeFails = reason;
      if (reason === null) {
        // M-18.9: the look of the second stores what was not stored.
        later(500, async () => {
          if (todos.retry(root)) {
            await sendTodos();
          }
        });
      }
    },
    todoUnreadable: (lines) => {
      todos.unreadable = [...lines];
      later(300, sendTodos);
    },
    todos: () => (root === null ? [] : todos.of(root).map((t) => ({ ...t }))),
    agentTool: (tool, state) => {
      agents.tools[tool] = state;
    },
    settingsOutside: async (place, key, value) => {
      const said = model.outside(place, key, value);
      await sendSettings();
      if (said !== null) {
        await setStatus(said);
      }
    },
    settingsBroken: async (place) => {
      model.broken.add(place);
      await setStatus(
        `The settings keep their last values: ${place === 'user' ? '/home/user' : root}/underline.yaml, line 1: theme: "Purple" is none of the themes`,
      );
    },
    settingsUnwritable: (on) => {
      model.unwritable = on;
    },
    settingsFile: (place) => ({
      ...(place === 'user' ? model.user : (model.workspaces[root ?? ''] ?? {})),
    }),
    cannotKeep: (on) => {
      keepFails = on;
    },
    openedWithSystem: () => [...opened],
    gitIdentity: (known) => {
      gitIdentity = known;
    },
    gitIdentityFails: (reason) => {
      gitIdentityFails = reason;
    },
    gitCommitFails: (reason) => {
      gitCommitFails = reason;
    },
    gitMerging: async (on) => {
      gitMerging = on;
      await sendGit();
    },
    gitLog: () => [...(gitOf()?.messages ?? [])],
    setNow: (now) => {
      mockNow = now;
    },
    syncOutcome: (outcome) => {
      syncNext = outcome;
    },
    syncs: () => syncCount,
    gitBranchInTheWay: (files) => {
      branchInTheWay = files;
    },
    gitProblem: async (reason) => {
      const state = remoteOf();
      if (state) {
        state.problem = reason;
        await sendGit();
      }
    },
    exportDialog: (dialog) => {
      exportDialog = dialog;
    },
    exportedFiles: () => Object.fromEntries(exported),
    setClipboard: (content) => {
      clipboard = { text: null, html: null, image: false, ...content };
    },
    clipboardContent: () => ({ ...clipboard }),
    dragEnter: () => mockDrop({ type: 'enter' }),
    dragLeave: () => mockDrop({ type: 'leave' }),
    drop: (paths, x, y) => mockDrop({ type: 'drop', paths, x, y }),
    dispose: () => {
      for (const timer of saveTimers.values()) {
        clearTimeout(timer);
      }
      saveTimers.clear();
      if (searchTimer !== null) {
        clearTimeout(searchTimer);
        searchTimer = null;
      }
      if (gitTimer !== null) {
        clearTimeout(gitTimer);
        gitTimer = null;
      }
      if (syncTimer !== null) {
        clearTimeout(syncTimer);
        syncTimer = null;
      }
      if (marksTimer !== null) {
        clearTimeout(marksTimer);
        marksTimer = null;
      }
      for (const timer of todoTimers) {
        clearTimeout(timer);
      }
      todoTimers.clear();
      if (linksTimer !== null) {
        clearTimeout(linksTimer);
        linksTimer = null;
      }
      disposed = true;
    },
  };
  return handlers;
}

/** ADR-025: what a test (or Playwright) may do as the backend would. */
export type MockHooks = {
  /** OQ-198: the commands the palette said it ran, the most recent first. */
  recentCommands(): string[];
  /** OQ-220: the share of the split view remembered for the open workspace, in tenths of a percent. */
  splitShare(): number | null;
  /** As an agent: `remove` chars at char `at` replaced by `insert`. */
  backendEdit(
    note: string,
    at: number,
    remove: number,
    insert: string,
    sealed?: boolean,
  ): Promise<number | null>;
  /** As another program: the note's file holds `text` now. */
  outsideChange(note: string, text: string): Promise<number | null>;
  /** The backend's text of `note`, if a window's core holds it. */
  mirrorText(note: string): string | null;
  /** F16: the text of `note` as the backend has it: the open tab's, else the file's. */
  noteText(note: string): string;
  /** F14, UC-017 BR-004: the agent `name` connects (`refused`: it asked while access is off, A2). */
  agentConnect(name: string, refused?: boolean): Promise<void>;
  /** F14, UC-017 A3: the agent `name` disconnects. */
  agentDisconnect(name: string): Promise<void>;
  /** F14, M-17.6: the agent `name` read (or `wrote`) lines `from` to `to` (from 1) of `note`. */
  agentRead(name: string, note: string, from: number, to: number, wrote?: boolean): Promise<void>;
  /**
   * F14, UC-019: the agent `name` replaces `oldText` by `newText` in `note`:
   * in the open tab as the backend's edit, else in the note's file with a
   * report in the status. The agent's answer.
   */
  agentChange(name: string, note: string, oldText: string, newText: string): Promise<string>;
  /** F14, UC-019 A1: the agent `name` creates `note` with `text`; the agent's answer. */
  agentCreate(name: string, note: string, text: string): Promise<string>;
  /**
   * F14, UC-020 A8: the agent `name` changed a todo (`change`, such as
   * "created todo #3"); F15: with `todo`, that todo is created or changed so.
   */
  agentTodo(
    name: string,
    change: string,
    todo?: Partial<MockTodo> & { number: number },
  ): Promise<void>;
  /**
   * F15, UC-020 A7: another program or a sync creates or changes a todo of
   * the open workspace (or of `root`); the window shows it 300 ms later.
   */
  todoOutside(todo: Partial<MockTodo> & { number: number }, root?: string): void;
  /** F15, UC-020 A9: the todo files cannot be written (`reason`), or can again (`null`). */
  todoWriteFails(reason: string | null): void;
  /** F15, M-18.1: lines for todo files that cannot be read (empty: none). */
  todoUnreadable(lines: string[]): void;
  /** F15: the todos of the open workspace as stored. */
  todos(): MockTodo[];
  /** F14, M-17.2: how the fake agent tool `tool` stands from now on. */
  agentTool(tool: AgentToolName, state: MockToolState): void;
  /** F10, UC-010 A5: another program sets `key` in a settings file (`null` removes it). */
  settingsOutside(place: MockSettingsPlace, key: SettingName, value: string | null): Promise<void>;
  /** F10, UC-010 A6: the settings file cannot be read now; changes are not stored. */
  settingsBroken(place: MockSettingsPlace): Promise<void>;
  /** F10, UC-010 A7: the settings files cannot be written (`true`) or can again. */
  settingsUnwritable(on: boolean): void;
  /** F10: what a settings file holds now, by key. */
  settingsFile(place: MockSettingsPlace): Partial<Record<SettingName, string>>;
  /** F8, UC-007 A8: the deleted notes cannot keep a note (`true`) or can again. */
  cannotKeep(on: boolean): void;
  /** F7, UC-004 BR-007: what the system's program would have opened, in order. */
  openedWithSystem(): string[];
  /** F12, UC-012 A3: git knows the user's name and email (`true`) or not (the start). */
  gitIdentity(known: boolean): void;
  /** F12, UC-012 A3: git cannot keep the name and email, with `reason` (`null`: it can). */
  gitIdentityFails(reason: string | null): void;
  /** F12, UC-012 A4: git refuses every commit with `reason` (`null`: no more). */
  gitCommitFails(reason: string | null): void;
  /** F12, UC-012 BR-009: a merge waits for its conflicts (`true`) or not. */
  gitMerging(on: boolean): Promise<void>;
  /** F12: the messages of the fake commits of the open workspace, newest first. */
  gitLog(): string[];
  /** Package 6b: the mock's local clock, `2026-11-11T08:15`; `null` for the browser's. */
  setNow(now: string | null): void;
  /** F13, UC-014: how the fake syncs end from now on (`synced` at first). */
  syncOutcome(outcome: MockSyncOutcome): void;
  /** F13: how many fake syncs started. */
  syncs(): number;
  /** F13, UC-013 A3: the next switch meets these changes in the way (`null`: none). */
  gitBranchInTheWay(files: string[] | null): void;
  /** F13, UC-011 A9: git cannot answer, with `reason` (`null`: it can again). */
  gitProblem(reason: string | null): Promise<void>;
  /** F11, UC-009 step 2: what the fake save dialog does from now on. */
  exportDialog(dialog: MockExportDialog): void;
  /** F11: the files the exports wrote, by path, with the text exported. */
  exportedFiles(): Record<string, string>;
  /** F6: the system clipboard holds `content` now (unset parts are empty). */
  setClipboard(content: Partial<MockClipboard>): void;
  /** F6: what the system clipboard holds. */
  clipboardContent(): MockClipboard;
  /** M-10.5: files are dragged into the window. */
  dragEnter(): void;
  /** M-10.5: they leave it without a drop. */
  dragLeave(): void;
  /** M-10.5: `paths` are dropped at `x`, `y` (CSS pixels of the window). */
  drop(paths: string[], x: number, y: number): void;
  /**
   * F16, M-21.4: the user closes the window: the window is asked to send
   * its edits (`ed-flush` with `closing`) and answers `app_request_close`.
   */
  closeWindow(): Promise<void>;
  /** F16, M-21.4: whether Underline ended (closed, or "Close anyway"). */
  windowClosed(): boolean;
  /** F16, M-06.6, M-21.4: the saves of these notes fail from now on (empty: none). */
  failSaves(notes: string[]): void;
  /**
   * UC-026: `note` gets a snapshot of `text` taken at `taken` (milliseconds,
   * now by default) for `cause` ("Editing" by default); its key.
   */
  addSnapshot(note: string, text: string, cause?: MockCause | string, taken?: number): string;
  /** UC-017, UC-019: agents may write the notes of the workspace (`true`) or only read them. */
  agentWrite(on: boolean): Promise<void>;
  /** UC-026: the snapshots of `note`, newest first: cause and text. */
  snapshots(note: string): { cause: MockCause; text: string }[];
  /** The test ended: no timer of the mock outlives it. */
  dispose(): void;
};

/** The hooks of the mock installed last (`window.__ulMock`). */
export function mockHooks(): MockHooks {
  const hooks = (globalThis as unknown as { __ulMock?: MockHooks }).__ulMock;
  if (!hooks) {
    throw new Error('No mock backend was made');
  }
  return hooks;
}

function notOpen(note: string): UiError {
  return { kind: 'invalid', message: `The tab ${note} is not open` };
}

/**
 * A rough stand-in for `underline_wiki::WikiRemote` (the mock only needs
 * plausible texts; the real rules are tested in Rust).
 */
function wikiStatus(url: string): string {
  const match = /^https?:\/\/([^/]+)\/(.+?)\/?$/.exec(url);
  const project = match?.[2].replace(/\.git$/, '').replace(/\.wiki$/, '');
  const segments = project?.split('/').filter(Boolean).length ?? 0;
  const github = match?.[1] === 'github.com';
  if (!match || !project || (github ? segments !== 2 : segments < 2)) {
    return `\`${url}\` is not a GitHub or GitLab project URL`;
  }
  return `Wiki repository: https://${match[1]}/${project}.wiki.git`;
}

/** Whether the page runs inside the Tauri window. */
export function insideTauri(): boolean {
  return '__TAURI_INTERNALS__' in window;
}

/**
 * Answers every command from `handlers` and delivers events to listeners.
 * A handler reports a failure by throwing a `UiError`, which reaches the
 * caller as `{ status: "error" }`.
 */
export function installMockBackend(handlers: Partial<MockHandlers> = mockHandlers()): void {
  (globalThis as unknown as { __ulMock?: MockHooks }).__ulMock = handlers.__hooks;
  useMockDrops();
  mockIPC(
    (cmd, args) => {
      if (cmd in handlers) {
        const handler = handlers[cmd as keyof MockHandlers] as (args: unknown) => unknown;
        return handler(args);
      }
      const error: UiError = {
        kind: 'internal',
        message: `The mock backend has no command ${cmd}`,
      };
      throw error;
    },
    { shouldMockEvents: true },
  );
}

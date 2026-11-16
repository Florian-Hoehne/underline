// FR-077, UC-023 BR-001: every command of Underline, in the registry of
// `commands.ts`: those of the menu bar (File, Theme, Settings), the top bar,
// the context menus of a note and of a tab (for the active note), the
// formatting toolbar, the switch of the editing view, the status line and
// the git and todo parts of the tool panel and the status bar, and the keys
// of the window. Each runs the same method of the shell its menu entry,
// button or key runs (BR-005); the palette only finds and starts it. The keys
// of the Vim and Nano input modes are none of these (UC-005 BR-009, BR-010).
//
// OQ-243: a key may run its command in the text of a note only (the
// toolbar's) or outside it only (Replace in notes); the registry's test
// checks that no two commands claim one key for the same place.
//
// Package 4 (FR-081 to FR-086): the editing comfort's keys are the
// editor's (in the text of a note): Alt+Up, Alt+Down, Ctrl+Shift+D, Tab and
// Shift+Tab; "Go to line…" is the window's Ctrl+G with the input mode
// Normal (UC-004 BR-017, as the find field); close pairs is switched here
// as in the settings (UC-010 BR-006).
//
// Package 5 (FR-087 to FR-089, UC-010 A3, A11, A12, BR-006): the editor
// zoom with Ctrl+Plus, Ctrl+Minus and Ctrl+0 (Cmd on macOS), the focus mode
// with Ctrl+Alt+F (Cmd+Shift+. on macOS, where Cmd+Ctrl+F is the system's
// full screen and Cmd+Alt+F replaces in the note; OQ-262) and soft wrap with
// Alt+Z are the window's keys, taken wherever the keyboard is, but not over a
// dialog or the palette: the enlarged picture keeps its own zoom keys. No input mode gives them a meaning (the
// Vim keys of `codemirror-vim` and the Nano keys of `editor/keys.ts` leave
// them).
//
// Package 6a (FR-091 to FR-093, UC-024, UC-004 A15, UC-008 A8): "Add note
// attribute" opens the form of a new attribute in the note properties with
// the keyboard in its name; "Search by tag…" asks for a tag of the active
// note and searches the workspace for it. Neither has a key.
//
// Package 6b (FR-094, FR-095, UC-025, UC-003 A7): "Daily note" with
// Ctrl+Shift+J, "Previous daily note" and "Next daily note" with
// Ctrl+Shift+PageUp and Ctrl+Shift+PageDown (Cmd on macOS, UC-025 BR-007,
// OQ-219) are the window's keys, taken wherever the keyboard is but not over
// a dialog or the palette. The spec's Ctrl+Alt+D and Ctrl+Alt+arrows were
// left (review): the desktops of Linux take them (show desktop, switch
// workspace), Cmd+Alt+D hides the Dock on macOS, and Ctrl+Alt is AltGr on
// Windows. No input mode and no key of the editors gives the new keys a
// meaning (CodeMirror selects a page with Shift+PageUp, not with Ctrl; Nano
// and Vim have no Ctrl+Shift+J). "New note from template…" opens "New note"
// with the keyboard on "Template:".
//
// Package 7a (FR-096, FR-097, UC-004 A18 to A20, BR-018, OQ-277): "Open to
// the side" with Ctrl+\ (Cmd+\ on macOS, the spec's key; by its place on
// keyboards where it needs AltGr) and Ctrl+Shift+S opens the active note to
// the side, in its other editing view (A19); "Focus the other pane" with
// Ctrl+F6 (F6 alone goes between the regions of a pane), "Focus the left
// pane" and "Focus the right pane" with Ctrl+1 and Ctrl+2 move the keyboard
// between the panes; "End the split view" with Ctrl+Shift+M. Cmd stands for
// Ctrl on macOS. The window takes the keys wherever the keyboard is, but
// not over a dialog or the palette; no input mode and no key of the editors
// gives them a meaning, none needs AltGr but Ctrl+\ by its sign, and no
// desktop of Linux, Windows or macOS takes them.
//
// Left out, as OQ-237 lists: the field "Wiki project URL" of the Workspace
// menu (it asks for a text, not a choice), the buttons that act on one
// changed file or one todo, the status bar's "Undo" and "Open" of a report,
// and the keys that move the keyboard (Tab, F6, Shift+F10, Escape).

import type { EditActionName, ExportChoice, LineTargetView, ToolName } from './bindings';
import { tick } from 'svelte';
import { keyboardInText, type Command, type CommandChoice } from './commands';
import { focusFallback } from './focus';
import { headOf } from './editor/attributes';
import { SPELL_LANGUAGES } from './spellLanguages';
import type { Shell } from './shell.svelte';

/** UC-023 A2: why a command cannot run now. */
export const NO_WORKSPACE = 'No workspace is open.';
export const NO_NOTE = 'No note is open.';
export const NO_GIT = 'The workspace is no git repository.';
export const NO_REMOTE = 'The workspace has no remote to sync with.';
export const NO_SYNC_CONFLICT = 'No sync waits for its conflicts.';
export const FIND_CLOSED = 'The find field is not open.';
export const NOTHING_CHOSEN = 'Nothing is chosen.';
export const MERGING = 'A synchronization left notes in conflict; resolve them before committing.';
export const NO_AGENTS = 'Open a workspace to connect agents to it.';
export const OPEN_A_NOTE = 'Open a note first.';
export const NO_SPLIT = 'The split view is not shown.';
export const NO_TAGS = 'The note has no tags.';

/** The ids of the commands other parts of the window run or show. */
export const COMMAND = {
  newNote: 'file.newNote',
  openWorkspace: 'file.openWorkspace',
  newWorkspace: 'file.newWorkspace',
  theme: 'theme.choose',
  settings: 'settings.open',
  goToNote: 'note.goTo',
  palette: 'palette.open',
  dailyNote: 'note.daily',
  split: 'view.split',
} as const;

/**
 * What the note area does for the commands of the note shown (NoteArea.svelte
 * registers it while it shows a note): the find field, the toolbar and the
 * switch of the editing view act in the view shown.
 */
export type NoteActions = {
  /** UC-008 A6, UC-005 A20: opens the find field, for replacing with `replacing`. */
  find(replacing: boolean): void;
  /** M-13.4: the next or the previous hit while the find field is open. */
  findStep(back: boolean): void;
  /** M-10.2: a function of the formatting toolbar in the view shown. */
  tool(tool: ToolName): void;
  /** M-07.2: the switch "Facade" / "Source". */
  view(view: 'facade' | 'source'): void;
  /**
   * FR-080, UC-005 A23: link completion at the cursor (`complete`); UC-004
   * A2, Q-64: follows the link at the cursor (`follow`).
   */
  link(action: 'complete' | 'follow' | 'followSide'): void;
  /** UC-005 A26, A27: moves, duplicates or indents in the view shown, as its key does. */
  edit(action: EditActionName): void;
  /** UC-004 A16 step 1: how many lines the active note has, and whether the facade shows it. */
  lineInfo(): { lines: number; facade: boolean } | null;
  /** UC-004 A16 step 3: the cursor of the view shown to `target`. */
  goToLine(target: LineTargetView): void;
  /**
   * UC-005 A28: the cursor to the next or the previous marked word, or the
   * spell menu of the word at the cursor, in the view shown.
   */
  spell(step: 'next' | 'previous' | 'menu'): void;
};

/** The input mode in effect: "Normal", "Vim" or "Nano". */
function inputMode(shell: Shell): string {
  return shell.settings?.inputMode ?? 'Normal';
}

/**
 * UX review: a dialog that waits for an answer (Rename, Delete, Export, New
 * note, the settings…) is open; go to note and the palette do not open over
 * it, as Ctrl+Shift+E does not. The palette itself does not count.
 */
function dialogOpen(): boolean {
  return Array.from(document.querySelectorAll('[role="dialog"]')).some(
    (dialog) => !dialog.classList.contains('palette'),
  );
}

/**
 * UC-023 BR-001, OQ-237: the names of the toolbar's functions in the
 * palette, which says what each does, as its buttons have no menu entry.
 */
const TOOL_TITLES: Readonly<Record<ToolName, string>> = {
  heading: 'Format as heading',
  bold: 'Format as bold',
  italic: 'Format as italic',
  code: 'Format as code',
  list: 'Format as list',
  quote: 'Format as quote',
  link: 'Insert link…',
  image: 'Insert image…',
  table: 'Insert table',
};

/** The note of the active note tab, `null` without one or while the tab "Todos" shows. */
function activeNote(shell: Shell): string | null {
  return shell.todoListShown ? null : shell.tabs.active;
}

/** FR-091: the tags of `note`, as its editor core reads them; none while it is not open. */
function tagsOf(note: string): string[] {
  return headOf(note)?.tags ?? [];
}

/** UC-009 step 1: the kinds of export of the active note, by its format. */
function exportKinds(shell: Shell): ExportChoice[] {
  const kinds = shell.start?.exportKinds;
  const note = activeNote(shell);
  const format = shell.overview.find((item) => item.note === note)?.format ?? 'Markdown';
  return (format === 'AsciiDoc' ? kinds?.asciiDoc : kinds?.markdown) ?? [];
}

/** Every command of the window, in no particular order (the palette sorts). */
export function commandsOf(shell: Shell): Command[] {
  const noWorkspace = () => (shell.workspace === null ? NO_WORKSPACE : null);
  const needsNote = () => noWorkspace() ?? (activeNote(shell) === null ? NO_NOTE : null);
  const needsView = () => needsNote() ?? (shell.noteActions === null ? NO_NOTE : null);
  const needsGit = () =>
    noWorkspace() ?? (shell.changes === null ? NO_GIT : (shell.changes.problem ?? null));
  const withNote = (work: (note: string) => void) => () => {
    const note = activeNote(shell);
    if (note !== null) {
      work(note);
    }
  };
  const normalKeys = () => inputMode(shell) === 'Normal';
  /** Package 5: the view keys wait while a dialog or the palette has the keyboard. */
  const nothingOver = () => !dialogOpen() && !shell.palette.open;

  const commands: Command[] = [
    // M-03.4: the File menu.
    {
      id: COMMAND.newNote,
      title: 'New note…',
      menu: 'File',
      unavailable: noWorkspace,
      run: () => void shell.startNewNote(),
    },
    // UC-003 A7: the same dialog, with the keyboard on its templates.
    {
      id: 'file.newNoteFromTemplate',
      title: 'New note from template…',
      menu: 'File',
      unavailable: noWorkspace,
      run: () => void shell.startNewNote('', true),
    },
    {
      id: COMMAND.openWorkspace,
      title: 'Open workspace…',
      menu: 'File',
      unavailable: () => null,
      run: () => void shell.startOpen(),
    },
    {
      id: COMMAND.newWorkspace,
      title: 'New workspace…',
      menu: 'File',
      unavailable: () => null,
      run: () => shell.startCreate(),
    },
    // M-03.6: the Theme menu, as one command that asks for the theme (A3).
    {
      id: COMMAND.theme,
      title: 'Theme…',
      menu: 'Theme',
      unavailable: () => null,
      choices: () =>
        shell.themes.map((name) => ({
          value: name,
          label: name,
          current: name === shell.settings?.theme,
        })),
      run: (choice) => {
        if (choice !== undefined) {
          void shell.chooseTheme(choice);
        }
      },
    },
    // M-03.7, M-20.1: the Settings menu and Ctrl+, (UC-010 BR-006).
    {
      id: COMMAND.settings,
      title: 'Settings…',
      menu: 'Settings',
      key: { key: ',', command: true },
      global: true,
      takesKey: () => true,
      unavailable: () => null,
      run: () => void shell.toggleSettings(),
    },
    // UC-010 A11, BR-009 (FR-087): the editor zoom; Ctrl+= is Ctrl+Plus
    // on keyboards whose plus needs Shift.
    {
      id: 'view.zoomIn',
      title: 'Zoom in',
      menu: 'View',
      key: { key: '+', command: true },
      alsoKeys: [
        { key: '+', command: true, shift: true },
        { key: '=', command: true },
      ],
      global: true,
      takesKey: nothingOver,
      unavailable: () => null,
      run: () => void shell.zoomEditor(1),
    },
    {
      id: 'view.zoomOut',
      title: 'Zoom out',
      menu: 'View',
      key: { key: '-', command: true },
      global: true,
      takesKey: nothingOver,
      unavailable: () => null,
      run: () => void shell.zoomEditor(-1),
    },
    {
      id: 'view.zoomReset',
      title: 'Reset zoom',
      menu: 'View',
      key: { key: '0', command: true },
      global: true,
      takesKey: nothingOver,
      unavailable: () => null,
      run: () => void shell.zoomEditor(0),
    },
    // UC-010 A12, BR-010 (FR-088): the focus mode.
    {
      id: 'view.focusMode',
      title: shell.focusMode ? 'Turn off focus mode' : 'Turn on focus mode',
      menu: 'View',
      key: {
        mac: { key: '.', command: true, shift: true },
        other: { key: 'F', command: true, alt: true },
      },
      global: true,
      takesKey: nothingOver,
      unavailable: () => null,
      run: () => void shell.toggleFocusMode(),
    },
    // FR-096, FR-097, UC-004 A18, A19: the active note to the side.
    {
      id: COMMAND.split,
      title: 'Open to the side',
      menu: 'View',
      key: { key: '\\', command: true },
      alsoKeys: [{ key: 'S', command: true, shift: true }],
      global: true,
      takesKey: () => needsNote() === null && nothingOver(),
      unavailable: needsNote,
      run: withNote((note) => void shell.openToSide(note)),
    },
    // UC-004 A20.
    {
      id: 'view.endSplit',
      title: 'End the split view',
      menu: 'View',
      key: { key: 'M', command: true, shift: true },
      global: true,
      takesKey: () => shell.split && nothingOver(),
      unavailable: () => noWorkspace() ?? (shell.split ? null : NO_SPLIT),
      run: () => void shell.endSplit(),
    },
    {
      id: 'view.otherPane',
      title: 'Focus the other pane',
      menu: 'View',
      key: { key: 'F6', command: true },
      global: true,
      takesKey: () => shell.split && nothingOver(),
      unavailable: () => noWorkspace() ?? (shell.split ? null : NO_SPLIT),
      run: () => void shell.focusPaneView(shell.activePane === 'left' ? 'right' : 'left'),
    },
    // UC-004 BR-018: the keyboard to the other pane, which becomes the active one.
    ...(
      [
        ['left', 'Focus the left pane', '1'],
        ['right', 'Focus the right pane', '2'],
      ] as const
    ).map(([pane, title, key]): Command => ({
      id: `view.focus${pane === 'left' ? 'Left' : 'Right'}Pane`,
      title,
      menu: 'View',
      key: { key, command: true },
      global: true,
      takesKey: () => shell.split && nothingOver(),
      unavailable: () => noWorkspace() ?? (shell.split ? null : NO_SPLIT),
      run: () => void shell.focusPaneView(pane),
    })),
    // M-03.1, M-03.2: the top bar.
    {
      id: 'view.menuBar',
      title: shell.settings?.showMenuBar ? 'Hide the menu' : 'Show the menu',
      menu: 'View',
      unavailable: () => null,
      run: () => void shell.toggleMenuBar(),
    },
    {
      id: 'search.notes',
      title: 'Search notes',
      menu: 'Search',
      key: { key: 'F', command: true, shift: true },
      global: true,
      takesKey: () => true,
      unavailable: () => null,
      run: () => {
        shell.palette.close();
        shell.search.show();
      },
    },
    {
      id: 'search.replace',
      title: 'Replace in notes',
      menu: 'Search',
      key: { key: 'H', command: true, shift: true },
      // OQ-243: in the text of a note the key is the toolbar's heading.
      keyPlace: 'outside',
      global: true,
      unavailable: noWorkspace,
      run: () => {
        shell.palette.close();
        if (shell.search.open && shell.search.replacing) {
          shell.search.stopReplacing();
        } else {
          shell.search.showReplace();
        }
      },
    },
    // FR-076, UC-004 A12, BR-014: Ctrl+P where the input mode gives it no
    // meaning of its own; the palette reaches it in every input mode.
    {
      id: COMMAND.goToNote,
      title: 'Go to note…',
      menu: 'Note',
      key: { key: 'P', command: true },
      global: true,
      takesKey: () => !dialogOpen() && (normalKeys() || !keyboardInText()),
      unavailable: () => null,
      run: () => shell.palette.show('notes'),
    },
    // FR-095, UC-025 BR-007: the daily note of today, and the daily notes
    // before and after the one in the active tab (A2), from anywhere.
    {
      id: COMMAND.dailyNote,
      // TC-025-11 step 2 runs "Daily note" from the palette.
      title: 'Daily note',
      menu: 'Note',
      key: { key: 'J', command: true, shift: true },
      global: true,
      takesKey: () => noWorkspace() === null && nothingOver(),
      unavailable: noWorkspace,
      run: () => void shell.openDailyNote(),
    },
    {
      id: 'note.dailyPrevious',
      title: 'Previous daily note',
      menu: 'Note',
      key: { key: 'PageUp', command: true, shift: true },
      global: true,
      takesKey: () => needsNote() === null && nothingOver(),
      unavailable: needsNote,
      run: () => void shell.stepDailyNote(false),
    },
    {
      id: 'note.dailyNext',
      title: 'Next daily note',
      menu: 'Note',
      key: { key: 'PageDown', command: true, shift: true },
      global: true,
      takesKey: () => needsNote() === null && nothingOver(),
      unavailable: needsNote,
      run: () => void shell.stepDailyNote(true),
    },
    // FR-077, UC-023 BR-007: Ctrl+Shift+P and F1 from anywhere.
    {
      id: COMMAND.palette,
      title: 'Command palette',
      menu: 'View',
      key: { key: 'P', command: true, shift: true },
      alsoKeys: [{ key: 'F1' }],
      global: true,
      takesKey: () => !dialogOpen(),
      unavailable: () => null,
      run: () => shell.palette.show('commands'),
    },
    // M-14, M-05.4, M-06.4: the context menus of a note and of its tab, for
    // the active note; F2 renames it (UC-006 step 1).
    {
      id: 'note.rename',
      title: 'Rename…',
      menu: 'Note',
      key: { key: 'F2' },
      global: true,
      takesKey: () => needsNote() === null && shell.notes.rename === null,
      unavailable: needsNote,
      run: withNote((note) => void shell.startRename(note)),
    },
    {
      id: 'note.delete',
      title: 'Delete…',
      menu: 'Note',
      unavailable: needsNote,
      run: withNote((note) => void shell.startDelete(note)),
    },
    // UC-026 step 1 (FR-102): the snapshots of the active note.
    {
      id: 'note.snapshots',
      title: 'Snapshots…',
      menu: 'Note',
      unavailable: needsNote,
      run: withNote((note) => void shell.showSnapshots(note)),
    },
    // M-15.1, UC-009 BR-008: asks for the kind in "Export note"; not while
    // another dialog has the keyboard (review F11).
    {
      id: 'note.export',
      title: 'Export…',
      menu: 'Note',
      key: { key: 'E', command: true, shift: true },
      global: true,
      takesKey: () =>
        document.querySelector('[role="dialog"]') === null || shell.exporting !== null,
      unavailable: needsNote,
      run: withNote((note) => void shell.startExport(note)),
    },
    ...exportKinds(shell).map((choice): Command => ({
      id: `note.export.${choice.kind}`,
      title: `${choice.label}…`,
      menu: 'Note › Export',
      unavailable: needsNote,
      run: withNote((note) => void shell.startExport(note, choice.kind)),
    })),
    {
      id: 'note.close',
      title: 'Close tab',
      menu: 'Note',
      unavailable: needsNote,
      // App-tester: the keyboard goes to the note overview when the text it
      // was in is gone, as after the tab's close button, never to nowhere.
      run: withNote((note) => {
        void shell
          .closeTab(note)
          .then(() => tick())
          .then(() => {
            const active = document.activeElement;
            if (active === null || active === document.body) {
              focusFallback();
            }
          });
      }),
    },
    // M-13.4, UC-008 A6, BR-009: the find field, with the input mode Normal
    // by its keys; UC-005 BR-017, OQ-195: Ctrl+H, Cmd+Alt+F on macOS.
    {
      id: 'note.find',
      title: 'Find in note',
      menu: 'Note',
      key: { key: 'F', command: true },
      global: true,
      takesKey: () => normalKeys() && needsView() === null,
      unavailable: needsView,
      run: () => shell.noteActions?.find(false),
    },
    {
      id: 'note.replace',
      title: 'Replace in note',
      menu: 'Note',
      key: { mac: { key: 'F', command: true, alt: true }, other: { key: 'H', command: true } },
      global: true,
      takesKey: () => normalKeys() && needsView() === null,
      unavailable: needsView,
      run: () => shell.noteActions?.find(true),
    },
    {
      id: 'note.findNext',
      title: 'Find next',
      menu: 'Note',
      key: { key: 'F3' },
      global: true,
      unavailable: () => needsView() ?? (shell.search.find === null ? FIND_CLOSED : null),
      run: () => shell.noteActions?.findStep(false),
    },
    {
      id: 'note.findPrevious',
      title: 'Find previous',
      menu: 'Note',
      key: { key: 'F3', shift: true },
      global: true,
      unavailable: () => needsView() ?? (shell.search.find === null ? FIND_CLOSED : null),
      run: () => shell.noteActions?.findStep(true),
    },
    // FR-080, UC-005 A23, OQ-247: link completion; Ctrl+Space is the
    // editor's key (in the text of a note), Ctrl also on macOS.
    {
      id: 'note.completeLink',
      title: 'Complete link',
      menu: 'Note',
      key: { key: 'Space', ctrl: true },
      keyPlace: 'text',
      unavailable: needsView,
      run: () => shell.noteActions?.link('complete'),
    },
    // UC-004 A2 to A4, Q-64, OQ-250: following the link at the cursor by
    // keyboard; Ctrl+Enter is "Enlarge picture" in the facade (A-45).
    {
      id: 'note.followLink',
      title: 'Follow link',
      menu: 'Note',
      unavailable: needsView,
      run: () => shell.noteActions?.link('follow'),
    },
    // UC-004 A18 (FR-096): the link at the cursor, followed into the other pane.
    {
      id: 'note.followLinkSide',
      title: 'Open to the side: link at the cursor',
      menu: 'Note',
      unavailable: needsView,
      run: () => shell.noteActions?.link('followSide'),
    },
    // UC-024 step 3: the form of a new note attribute, by keyboard.
    {
      id: 'note.addAttribute',
      title: 'Add note attribute',
      menu: 'Note',
      unavailable: () => (needsView() === null ? null : OPEN_A_NOTE),
      run: () => shell.showAddAttribute(),
    },
    // UC-004 A15, UC-008 A8: a tag of the active note, searched for.
    {
      id: 'note.searchTag',
      title: 'Search by tag…',
      menu: 'Note',
      unavailable: () => {
        const missing = needsView();
        if (missing !== null) {
          return OPEN_A_NOTE;
        }
        const note = activeNote(shell);
        return note !== null && tagsOf(note).length === 0 ? NO_TAGS : null;
      },
      choices: () => {
        const note = activeNote(shell);
        return (note === null ? [] : tagsOf(note)).map((tag) => ({
          value: tag,
          label: `#${tag}`,
          current: false,
        }));
      },
      run: (choice) => {
        if (choice !== undefined) {
          shell.searchTag(choice);
        }
      },
    },
    // M-07.2: the switch of the editing view.
    {
      id: 'view.facade',
      title: 'Show facade view',
      menu: 'Editing view',
      unavailable: needsView,
      run: () => shell.noteActions?.view('facade'),
    },
    {
      id: 'view.source',
      title: 'Show source view',
      menu: 'Editing view',
      unavailable: needsView,
      run: () => shell.noteActions?.view('source'),
    },
    // FR-086, UC-004 A16, BR-017: Ctrl+G (Cmd+G) with the input mode
    // Normal, as the find field; the palette reaches it in every mode.
    {
      id: 'note.goToLine',
      title: 'Go to line…',
      menu: 'Note',
      key: { key: 'G', command: true },
      global: true,
      // Review: without a note the key says so in the status bar (App.svelte).
      takesKey: () => !dialogOpen() && normalKeys(),
      unavailable: () => (needsView() === null ? null : OPEN_A_NOTE),
      run: () => shell.palette.showLine(),
    },
    // FR-082, FR-083, UC-005 A26, A27: the editor's keys, in the text of a
    // note only; in the Vim keys' normal mode they are Vim's (BR-009).
    ...(
      [
        ['moveLineUp', 'Move line up', { key: 'ArrowUp', alt: true }],
        ['moveLineDown', 'Move line down', { key: 'ArrowDown', alt: true }],
        ['duplicateLine', 'Duplicate line', { key: 'D', command: true, shift: true }],
        ['indent', 'Indent list item', { key: 'Tab' }],
        ['outdent', 'Outdent list item', { key: 'Tab', shift: true }],
      ] as const
    ).map(([action, title, key]): Command => ({
      id: `edit.${action}`,
      title,
      menu: 'Note',
      key,
      keyPlace: 'text',
      unavailable: needsView,
      run: () => shell.noteActions?.edit(action),
    })),
    // M-10.2: the formatting toolbar; its keys are the editor's (keys.ts).
    ...(shell.start?.tools ?? []).map((tool): Command => ({
      id: `format.${tool.tool}`,
      title: TOOL_TITLES[tool.tool] ?? tool.name,
      menu: 'Formatting',
      key: { key: tool.key, command: true, shift: tool.shift },
      // The editor's keys (keys.ts), in the text of a note only (OQ-243).
      keyPlace: 'text',
      unavailable: needsView,
      run: () => shell.noteActions?.tool(tool.tool),
    })),
    // M-11.4: the switches and the choice of the status line (UC-010 A3).
    {
      id: 'status.minimap',
      title: shell.settings?.showMinimap ? 'Hide the minimap' : 'Show the minimap',
      menu: 'Status line',
      unavailable: () => null,
      run: () => void shell.quickSetting('show_minimap', String(!shell.settings?.showMinimap)),
    },
    {
      id: 'status.copyOnSelect',
      title:
        (shell.settings?.copyOnSelect ?? true)
          ? 'Turn off copy on select'
          : 'Turn on copy on select',
      menu: 'Status line',
      unavailable: () => null,
      run: () =>
        void shell.quickSetting('copy_on_select', String(!(shell.settings?.copyOnSelect ?? true))),
    },
    // UC-010 A3, BR-011 (FR-089): soft wrap, a switch of the status line.
    {
      id: 'status.softWrap',
      title: (shell.settings?.softWrap ?? true) ? 'Turn off soft wrap' : 'Turn on soft wrap',
      menu: 'Status line',
      key: { key: 'Z', alt: true },
      global: true,
      takesKey: nothingOver,
      unavailable: () => null,
      run: () => void shell.toggleSoftWrap(),
    },
    // UC-010 BR-001, BR-006, UC-005 BR-019: close pairs, as in the settings.
    {
      id: 'settings.closePairs',
      title: (shell.settings?.closePairs ?? true) ? 'Turn off close pairs' : 'Turn on close pairs',
      menu: 'Settings',
      unavailable: () => null,
      run: () =>
        void shell.quickSetting('close_pairs', String(!(shell.settings?.closePairs ?? true))),
    },
    {
      id: 'status.keys',
      title: 'Choose the input mode…',
      menu: 'Status line',
      unavailable: () => null,
      choices: () =>
        (shell.start?.inputModes ?? ['Normal', 'Vim', 'Nano']).map((mode) => ({
          value: mode,
          label: mode,
          current: mode === inputMode(shell),
        })),
      run: (choice) => {
        if (choice !== undefined) {
          void shell.quickSetting('input_mode', choice);
        }
      },
    },
    // UC-005 A28: the marked words by the keyboard.
    {
      id: 'spell.next',
      title: 'Next misspelled word',
      menu: 'Note',
      key: { key: 'F7' },
      global: true,
      takesKey: nothingOver,
      unavailable: needsView,
      run: () => shell.noteActions?.spell('next'),
    },
    {
      id: 'spell.previous',
      title: 'Previous misspelled word',
      menu: 'Note',
      key: { key: 'F7', shift: true },
      global: true,
      takesKey: nothingOver,
      unavailable: needsView,
      run: () => shell.noteActions?.spell('previous'),
    },
    {
      id: 'spell.menu',
      title: 'Show corrections for the word at the cursor',
      menu: 'Note',
      unavailable: needsView,
      run: () => shell.noteActions?.spell('menu'),
    },
    // UC-010 BR-001, BR-006, FR-100: the spell check language, as in the settings.
    {
      id: 'settings.spellCheckLanguage',
      title: 'Choose the spell check language…',
      menu: 'Settings',
      unavailable: () => null,
      choices: () =>
        SPELL_LANGUAGES.map(([code, name]) => ({
          value: code,
          label: name,
          current: code === (shell.settings?.spellCheckLanguage ?? 'off'),
        })),
      run: (choice) => {
        if (choice !== undefined) {
          void shell.quickSetting('spell_check_language', choice);
        }
      },
    },
    // M-19 (UC-011 to UC-015): git, in the tool panel and the status bar.
    {
      id: 'git.changes',
      title: 'Show changes',
      menu: 'Git',
      key: { key: 'G', command: true, shift: true },
      global: true,
      takesKey: () => true,
      unavailable: needsGit,
      run: () => shell.showChanges(),
    },
    {
      id: 'git.branches',
      title: 'Branches',
      menu: 'Git',
      key: { key: 'B', command: true, shift: true },
      global: true,
      unavailable: needsGit,
      run: () => void shell.openBranchMenu(),
    },
    {
      id: 'git.commit',
      title: shell.commit.amend ? 'Amend' : 'Commit',
      menu: 'Git',
      unavailable: () => {
        const git = needsGit();
        if (git !== null) {
          return git;
        }
        const changes = shell.changes;
        if (changes?.merging) {
          return MERGING;
        }
        return changes !== null &&
          changes.files.every((change) => !change.chosen) &&
          !shell.commit.amend
          ? NOTHING_CHOSEN
          : null;
      },
      run: () => {
        shell.showChanges();
        void shell.commitChanges();
      },
    },
    {
      id: 'git.sync',
      title: 'Sync now',
      menu: 'Git',
      unavailable: () => {
        const git = needsGit();
        const sync = shell.gitBar.sync;
        if (git !== null) {
          return git;
        }
        if (sync === null) {
          return NO_REMOTE;
        }
        if (sync.running) {
          return 'A sync is running.';
        }
        return sync.canSync ? null : 'Resolve the conflicts first.';
      },
      run: () => void shell.sync('full'),
    },
    {
      id: 'git.abort',
      title: 'Abort the sync',
      menu: 'Git',
      unavailable: () => needsGit() ?? (shell.gitBar.conflicts === null ? NO_SYNC_CONFLICT : null),
      run: () => shell.askAbort(),
    },
    // M-06.5, M-04.5 (UC-021): the tab "Todos".
    {
      id: 'todos.open',
      title: 'Open the todo list',
      menu: 'Todos',
      unavailable: noWorkspace,
      run: () => void shell.showTodoList(),
    },
    // M-17.1 (UC-017): the agents menu of the status bar.
    {
      id: 'agents.access',
      title: 'Agent access…',
      menu: 'Agents',
      unavailable: () => (shell.agents.menu === null ? NO_AGENTS : null),
      choices: (): CommandChoice[] =>
        (shell.agents.menu?.levels ?? []).map((level) => ({
          value: level.level,
          label: level.label,
          current: level.chosen,
        })),
      run: (choice) => {
        const level = shell.agents.menu?.levels.find((each) => each.level === choice);
        if (level && !level.chosen) {
          void shell.setAgentAccess(level.level);
        }
      },
    },
    {
      id: 'agents.settings',
      title: shell.agents.menu?.workspaceSettings ?? 'Settings of this workspace…',
      menu: 'Agents',
      unavailable: () => (shell.agents.menu === null ? NO_AGENTS : null),
      run: () => void shell.openSettings('workspace'),
    },
  ];
  return commands;
}

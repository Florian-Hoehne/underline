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
// Feature F9: the search of the overlay (`search_set`) runs over the fake
// notes and the open texts with a rough stand-in for `search_notes` (words,
// quotes, case, title and folder first), answers as the event `search`
// with the first 100 found notes, gives more with `search_page`, and
// `search_open` opens a found note at its word. It searches again when a
// text or the overview changes (UC-008 A3). `Trip` and `Log` of
// `/home/user/Notes` hold "ferry" and "deploy" in many places.
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

import { mockIPC } from '@tauri-apps/api/mocks';
import { emit } from '@tauri-apps/api/event';
import type {
  commands,
  ChangesView,
  ChangeView,
  CommitAnswer,
  CreateNoteAnswer,
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
} from './bindings';
import { loadEditorCore, MirrorCore } from './editor/core';
import { mockDrop, useMockDrops } from './editor/drop';
import { fakeBlocks, TOUR_NOTE, TOUR_TEXT } from './facade/fakeFacade';
import { MockSettings, userFileOf, type MockSettingsPlace } from './mock-settings';
import {
  mockChangedLines,
  mockDiff,
  mockProposedMessage,
  type MockProposalChange,
} from './mock-git';
import palettes from './palettes.generated.json';

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
  tab_activate: (args: { note: string }) => Promise<Ok<typeof commands.tabActivate>>;
  tab_close: (args: { note: string; force: boolean }) => Promise<Ok<typeof commands.tabClose>>;
  tab_store: (args: { note: string }) => Promise<Ok<typeof commands.tabStore>>;
  tab_set_view: (args: {
    note: string;
    view: EditingViewName;
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
  app_request_close: (args: { sent: boolean }) => Ok<typeof commands.appRequestClose>;
  ed_follow_link: (args: {
    note: string;
    at: number;
    rev: number;
  }) => Promise<Ok<typeof commands.edFollowLink>>;
  link_open_web: (args: { open: boolean }) => Promise<Ok<typeof commands.linkOpenWeb>>;
  note_new_start: () => Ok<typeof commands.noteNewStart>;
  note_plan: (args: { request: NewNoteRequest }) => Ok<typeof commands.notePlan>;
  note_create: (args: { request: NewNoteRequest }) => Promise<Ok<typeof commands.noteCreate>>;
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
  search_set: (args: { text: string }) => Promise<Ok<typeof commands.searchSet>>;
  search_page: (args: { serial: number; from: number }) => Ok<typeof commands.searchPage>;
  search_open: (args: {
    note: string;
    place: FoundPlace | null;
  }) => Promise<Ok<typeof commands.searchOpen>>;
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
};

/** M-10.2: the toolbar's buttons, as the backend gives them (`Tool::label`, `Tool::hint`). */
export const FAKE_TOOLS: readonly ToolView[] = [
  { tool: 'heading', label: 'H', hint: 'Heading (Ctrl+Shift+H)' },
  { tool: 'bold', label: 'B', hint: 'Bold (Ctrl+B)' },
  { tool: 'italic', label: 'I', hint: 'Italic (Ctrl+I)' },
  { tool: 'code', label: '</>', hint: 'Code (Ctrl+E)' },
  { tool: 'list', label: 'List', hint: 'List (Ctrl+Shift+L)' },
  { tool: 'quote', label: 'Quote', hint: 'Quote (Ctrl+Shift+Q)' },
  { tool: 'link', label: 'Link', hint: 'Link (Ctrl+K)' },
  { tool: 'image', label: 'Image', hint: 'Image (Ctrl+Shift+I)' },
  { tool: 'table', label: 'Table', hint: 'Table (Ctrl+Shift+T)' },
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
  if (note === TOUR_NOTE) {
    return TOUR_TEXT;
  }
  if (note === LINK_TOUR_NOTE) {
    return LINK_TOUR_TEXT;
  }
  const title = (note.split('/').at(-1) ?? note).replace(/-/g, ' ');
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

export function mockHandlers(start: MockStart = {}): MockHandlers {
  // F10: the settings files of the mock, the user's seeded from `start.settings`.
  const model = new MockSettings(userFileOf(start.settings ?? {}), PALETTES);
  let status = start.status ?? '';
  // F11: the exports' offers, error mark and busy state of the status.
  const extra: MockStatusExtra = { exports: [], error: false, busy: false };
  const setStatus = async (text: string, offer: StatusOffer | null = null, report = false) => {
    status = text;
    if (!report) {
      extra.exports = [];
      extra.error = false;
    }
    await emit('status', { text, offer, ...extra });
  };
  /** The window is told the settings in effect and what the settings window shows. */
  const sendSettings = async () => {
    await emit('settings', model.effective());
    await emit('settings-window', model.windowView('Markdown'));
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
        ...info,
        settings: model.effective(),
        status,
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
      return { view: model.windowView('Markdown'), problem: said };
    },
    wiki_check: async ({ url }) => {
      await setStatus(wikiStatus(url));
      return null;
    },
    ...workspaceHandlers(start, setStatus, model, sendSettings, () => status, extra),
  };
}

/** F11: what the status says besides its text about exports. */
type MockStatusExtra = { exports: ExportOfferView[]; error: boolean; busy: boolean };

/** One note tab of the fake session. */
type FakeTab = { note: string; view: EditingViewName };

/** The workspace commands over the fake file system. */
function workspaceHandlers(
  start: MockStart,
  setStatus: (text: string, offer?: StatusOffer | null, report?: boolean) => Promise<void>,
  model: MockSettings,
  sendSettings: () => Promise<void>,
  status: () => string = () => '',
  extra: MockStatusExtra = { exports: [], error: false, busy: false },
): Pick<
  MockHandlers,
  | 'ws_start'
  | 'ws_recent'
  | 'ws_open'
  | 'ws_check_new'
  | 'ws_create'
  | 'tab_open'
  | 'tab_activate'
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
  | 'link_open_web'
  | 'note_new_start'
  | 'note_plan'
  | 'note_create'
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
  | '__hooks'
> {
  const files: Record<string, string[]> = Object.fromEntries(
    Object.entries(FAKE_FILES).map(([root, notes]) => [root, [...notes]]),
  );
  let recent = [...(start.recent ?? start.recentWorkspaces ?? [])];
  let root: string | null = null;
  let tabs: FakeTab[] = [];
  let active: string | null = null;
  let clock = 0;
  /** What each workspace remembers: its tabs and when notes were opened. */
  const remembered: Record<string, { tabs: FakeTab[]; active: string | null }> = {};
  const lastOpened: Record<string, Record<string, number>> = {};
  let started = false;

  const normal = (path: string) => (path.length > 1 ? path.replace(/\/+$/, '') : path);
  const title = (note: string) => (note.split('/').at(-1) ?? note).replace(/-/g, ' ');
  const folder = (note: string) =>
    note.includes('/') ? note.slice(0, note.lastIndexOf('/')) : null;
  const outerOf = (path: string) =>
    [...recent, ...(root ? [root] : []), ...Object.keys(files)]
      .filter((known) => known !== '/home/user/Letters')
      .find((known) => path.startsWith(`${known}/`));

  const overview = (): OverviewItem[] => {
    if (root === null) {
      return [];
    }
    const opened = lastOpened[root] ?? {};
    const notes = files[root] ?? [];
    const count = (t: string) => notes.filter((n) => title(n).toLowerCase() === t).length;
    return [...notes]
      .sort(
        (a, b) =>
          (opened[b] ?? -1) - (opened[a] ?? -1) ||
          title(a).toLowerCase().localeCompare(title(b).toLowerCase()) ||
          (folder(a) ?? '').localeCompare(folder(b) ?? ''),
      )
      .map((note) => ({
        note,
        format: 'Markdown' as const,
        label:
          count(title(note).toLowerCase()) > 1
            ? `${title(note)}  (${folder(note) ?? 'top folder'})`
            : title(note),
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
  let clipboard: MockClipboard = { text: null, html: null, image: false };
  /** BR-011: per note, what the clipboard held before copy on select. */
  const before = new Map<string, MockClipboard>();
  /** How many images were pasted, for their names. */
  let pastedImages = 0;
  /** F7, UC-004 A4: what the system's program opened, and the web question. */
  const opened: string[] = [];
  let webAllowed = false;
  let webAsked: string | null = null;
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
  const newNoteText = (noteTitle: string) => `# ${noteTitle.trim().split(/\s+/).join(' ')}\n`;
  /** UC-006 BR-003, roughly: the links to `from` in `text`, as written from `note`. */
  const linkTo = (note: string, target: string) => `](${relativeTo(note, `${target}.md`)}`;
  const countLinks = (text: string, needle: string) => text.split(needle).length - 1;
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
  /** The mirror's text is the note's now. */
  const followMirror = (note: string, mirror: MirrorCore) => {
    texts.set(note, mirror.text());
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
      true,
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
    const rows: ChangeView[] = changes.map((c) => ({
      path: c.path,
      kind: c.kind,
      letter: LETTERS[c.kind],
      name: changeName(c),
      note: c.note !== null,
      chosen: !gitLeftOut.has(c.path),
    }));
    const proposal: MockProposalChange[] = changes
      .filter((c) => !gitLeftOut.has(c.path))
      .map((c) => ({
        kind: c.kind,
        title: c.note === null ? null : title(c.note),
        from: c.from === null ? null : title(c.from),
      }));
    return {
      problem: null,
      branch: { label: 'main', detached: false },
      files: rows,
      merging: gitMerging,
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
    } catch {
      // The mock was taken down meanwhile (a test ended).
    }
  };
  /** UC-011 A3: a text changed; the changes and the diff follow soon. */
  const gitAgain = () => {
    if (gitTimer !== null || !gitOf()) {
      return;
    }
    gitTimer = setTimeout(() => {
      gitTimer = null;
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
  const label = (note: string) =>
    title(note) +
    (saveErrors.has(note) ? ' ⚠' : unstored.has(note) && !quiet.has(note) ? ' •' : '');
  const tabsView = (): TabsView => ({
    tabs: tabs.map((tab) => ({
      note: tab.note,
      title: title(tab.note),
      label: label(tab.note),
      path: `${root}/${tab.note}.md`,
      view: tab.view,
      saveError: saveErrors.get(tab.note) ?? null,
    })),
    active,
    todoList: { open: false, active: false },
  });
  const properties = (): TabPropertiesView | null =>
    active === null
      ? null
      : {
          note: active,
          properties: {
            place: `${active}.md`,
            format: 'Markdown',
            size: `${new TextEncoder().encode(textOf(active)).length} bytes`,
            lastChange: '2026-11-09 10:00',
          },
        };
  /** Stores `note` now; the failure names the note, as the backend's does. */
  const store = (note: string): string | null => {
    if (!unstored.has(note) && !saveErrors.has(note)) {
      return null;
    }
    if (start.failingSaves?.includes(note)) {
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
          quiet.delete(note) || (undo !== null && status() === undo.status) || exportKept();
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
  };
  /** F9: the search of the overlay, as the mock runs it. */
  let searchText = '';
  let searchSerial = 0;
  let searchFound: FoundNoteView[] = [];
  let searchTimer: ReturnType<typeof setTimeout> | null = null;
  const SEARCH_PAGE = 100;
  /** UC-008, roughly as `search_notes` (BR-001 to BR-004). */
  const runSearch = async () => {
    if (disposed || root === null || searchText === '') {
      return;
    }
    const words = mockQuery(searchText);
    if (words.length === 0) {
      return;
    }
    const byName: FoundNoteView[] = [];
    const byText: FoundNoteView[] = [];
    for (const { note } of overview()) {
      const text = mirrors.get(note)?.text() ?? textOf(note);
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
        result: {
          serial: searchSerial,
          text: searchText,
          total: searchFound.length,
          notes: searchFound.slice(0, SEARCH_PAGE),
          unreadable: null,
        },
      });
    } catch {
      // The mock was taken down meanwhile (a test ended).
    }
  };
  /** UC-008 A3: a text or the overview changed: the search runs again soon. */
  const searchAgain = () => {
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
  const followLink = async (note: string, raw: string): Promise<LinkAnswer> => {
    const link = raw.trim();
    if ((link.includes('://') && !link.startsWith('file://')) || link.startsWith('mailto:')) {
      if (webAllowed) {
        openWithSystem(link);
        return { kind: 'done' };
      }
      webAsked = link;
      return { kind: 'askWeb', address: link };
    }
    const [path, heading] = link.startsWith('file://')
      ? [link.slice('file://'.length), undefined]
      : (link.split(/#(.*)/s) as [string, string | undefined]);
    if (path === '') {
      return jumpToHeading(note, heading ?? '');
    }
    if (path.startsWith('/')) {
      if (path in FAKE_OUTSIDE) {
        openWithSystem(path);
      } else {
        await setStatus(`${path} does not exist`);
      }
      return { kind: 'done' };
    }
    const parts = folderOf(note).split('/').filter(Boolean);
    for (const part of decodeURIComponent(path).split('/')) {
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
    const extension = /\.([^./]+)$/.exec(relative)?.[1];
    const isNote = extension === 'md' || extension === 'adoc';
    const id = isNote ? relative.replace(/\.[^./]+$/, '') : relative;
    if (root && files[root]?.includes(id)) {
      openTab(id);
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
      remembered[root] = { tabs: [...tabs], active };
    }
  };
  const viewed = (note: string) => {
    if (root) {
      lastOpened[root] = { ...lastOpened[root], [note]: ++clock };
    }
  };
  const openTab = (note: string) => {
    if (!tabs.some((tab) => tab.note === note)) {
      const at = active === null ? tabs.length : tabs.findIndex((t) => t.note === active) + 1;
      tabs.splice(at, 0, { note, view: start.view ?? 'facade' });
    }
    active = note;
    viewed(note);
    remember();
  };
  /** F10: the settings of the workspace opened last, still to be sent. */
  let settingsSaid: string | null | undefined;
  const switchTo = (dir: string) => {
    root = dir;
    gitChosen = null;
    gitDiscarding = null;
    settingsSaid = model.openWorkspace(dir);
    recent = [dir, ...recent.filter((r) => r !== dir)];
    const back = remembered[dir];
    tabs = back ? [...back.tabs] : [];
    active = back?.active ?? null;
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
    switchTo(dir);
    await sendAll();
    await setStatus(`Opened ${dir}`);
    return { kind: 'done' };
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
      switchTo(dir);
      openTab('Home');
      await sendAll();
      await setStatus(`Workspace created in ${dir}`);
      return { stage: 'done' };
    },
    search_set: async ({ text }) => {
      searchText = text.trim();
      if (searchText === '') {
        searchFound = [];
        await emit('search', { text: '', result: null });
      } else {
        // As the searcher's thread: the answer comes a moment later.
        setTimeout(() => void runSearch(), 20);
      }
      return null;
    },
    search_page: ({ serial, from }) =>
      serial === searchSerial ? searchFound.slice(from, from + SEARCH_PAGE) : null,
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
    tab_open: async ({ note }) => {
      if (root && files[root]?.includes(note)) {
        openTab(note);
        await sendAll();
      } else {
        await setStatus(`${title(note)} cannot be read: note \`${note}\` not found`);
      }
      return null;
    },
    tab_activate: async ({ note }) => {
      if (tabs.some((tab) => tab.note === note)) {
        active = note;
        viewed(note);
        remember();
        await sendAll();
      }
      return null;
    },
    tab_close: async ({ note, force }) => {
      const at = tabs.findIndex((tab) => tab.note === note);
      if (at < 0) {
        return null;
      }
      const failure = store(note);
      if (failure && !force) {
        await sendAll();
        return { tab: note, what: `the tab ${title(note)}`, error: failure };
      }
      clearTimeout(saveTimers.get(note));
      texts.delete(note);
      mirrors.delete(note);
      unstored.delete(note);
      saveErrors.delete(note);
      tabs.splice(at, 1);
      if (active === note) {
        active = tabs[Math.min(at, tabs.length - 1)]?.note ?? null;
      }
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
    tab_set_view: async ({ note, view }) => {
      tabs = tabs.map((tab) => (tab.note === note ? { ...tab, view } : tab));
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
    ed_note_link: ({ note, target }) => ({
      kind: 'link',
      address: relativeTo(note, `${target}.md`),
      label: title(target),
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
    app_close_anyway: () => null,
    ed_mirror: async ({ note }) => {
      if (!tabs.some((tab) => tab.note === note)) {
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
      setTimeout(() => void sendIndexes(), 20);
      return {
        note,
        rev: mirror.rev,
        text: mirror.text(),
        format: 'Markdown',
        anchor: 0,
        head: 0,
      };
    },
    ed_sync: async ({ note, batch }) => {
      const mirror = mirrors.get(note);
      if (!mirror || !tabs.some((tab) => tab.note === note)) {
        throw notOpen(note);
      }
      const before = mirror.text();
      const { answer, reverted } = mirror.receive(batch);
      if (mirror.text() !== before) {
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
    app_request_close: () => null,
    ed_follow_link: async ({ note, at, rev }) => {
      const mirror = mirrors.get(note);
      if (!mirror || !tabs.some((tab) => tab.note === note)) {
        throw notOpen(note);
      }
      // As the backend: a place of another revision follows nothing.
      const link = mirror.rev === rev ? mockLinkAt(mirror.text(), at) : null;
      return link === null ? { kind: 'done' } : followLink(note, link);
    },
    note_new_start: () =>
      root === null
        ? null
        : {
            folder: (active && folder(active)) ?? '',
            format: model.defaultFormat('Markdown') as FormatName,
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
      const text = newNoteText(request.title);
      files[root] = [...(files[root] ?? []), id];
      texts.set(id, text);
      await loadEditorCore();
      const mirror = new MirrorCore(text);
      mirrors.set(id, mirror);
      openTab(id);
      await sendAll();
      await setStatus(`Created the note ${title(id)}`);
      return { kind: 'created', note: id, rev: mirror.rev, offset: text.length };
    },
    note_rename_start: ({ note }) => ({
      note,
      title: title(note),
      folder: folder(note) ?? '',
      home: note === 'Home',
    }),
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
      let links = 0;
      let notes = 0;
      for (const other of files[root] ?? []) {
        if (other === to) {
          continue;
        }
        const needle = linkTo(other, note);
        const mirror = mirrors.get(other);
        const text = mirror?.text() ?? textOf(other);
        const found = countLinks(text, needle);
        if (found === 0) {
          continue;
        }
        links += found;
        notes += 1;
        if (!mirror) {
          texts.set(other, text.split(needle).join(linkTo(other, to)));
          continue;
        }
        // UC-006 A7: the open note changes in its text, saved quietly.
        let at = text.lastIndexOf(needle);
        let rev = mirror.rev;
        while (at >= 0) {
          rev = mirror.backendEdit(at, needle.length, linkTo(other, to), false) ?? rev;
          at = at === 0 ? -1 : text.lastIndexOf(needle, at - 1);
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
      let links = 0;
      let notes = 0;
      for (const other of root ? (files[root] ?? []) : []) {
        const found =
          other === note
            ? 0
            : countLinks(mirrors.get(other)?.text() ?? textOf(other), linkTo(other, note));
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
      const reopen = tabs.some((tab) => tab.note === note);
      files[root] = (files[root] ?? []).filter((n) => n !== note);
      const at = tabs.findIndex((tab) => tab.note === note);
      if (at >= 0) {
        clearTimeout(saveTimers.get(note));
        tabs.splice(at, 1);
        if (active === note) {
          active = tabs[Math.min(at, tabs.length - 1)]?.note ?? null;
        }
      }
      for (const map of [texts, mirrors, saveErrors] as Map<string, unknown>[]) {
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
      if (!root) {
        return null;
      }
      return { note, title: title(note), choices: FAKE_EXPORT_KINDS.markdown };
    },
    export_choose: async ({ note, kind }): Promise<ExportPlaceAnswer> => {
      // One dialog at a time (review F11).
      if (!root || choosing !== null) {
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
    link_open_web: async ({ open: answer }) => {
      const address = webAsked;
      webAsked = null;
      if (answer && address !== null) {
        webAllowed = true;
        openWithSystem(address);
      }
      return null;
    },
  };
  handlers.__hooks = {
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
        }
        return null;
      }
      const rev = mirror.reload(text);
      followMirror(note, mirror);
      await emit('ed-changed', { note, rev });
      await setStatus(`${title(note)} was changed outside`);
      await sendIndexes();
      return rev;
    },
    mirrorText: (note) => mirrors.get(note)?.text() ?? null,
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
      disposed = true;
    },
  };
  return handlers;
}

/** ADR-025: what a test (or Playwright) may do as the backend would. */
export type MockHooks = {
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

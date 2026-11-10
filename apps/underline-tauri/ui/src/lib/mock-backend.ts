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

import { mockIPC } from '@tauri-apps/api/mocks';
import { emit } from '@tauri-apps/api/event';
import type {
  commands,
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
  SettingName,
  SettingsView,
  StartInfo,
  FigureView,
  TabPropertiesView,
  TabsView,
  ToolView,
  UiError,
  WorkspaceSnapshot,
  SyncBatch,
} from './bindings';
import { loadEditorCore, MirrorCore } from './editor/core';
import { mockDrop, useMockDrops } from './editor/drop';
import { fakeBlocks, TOUR_NOTE, TOUR_TEXT } from './facade/fakeFacade';
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
  /** ADR-025: what a test may do as the backend would (`window.__ulMock`). */
  __hooks?: MockHooks;
};

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
    'Reading-List',
    'guide/Getting-Started',
    'guide/Home',
    'Journal/2026-11-09',
  ],
  '/home/user/Work': ['README', 'Meetings', 'Projects/Roadmap', TOUR_NOTE],
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
  return {
    theme: 'Light',
    palette: PALETTES[0],
    inputMode: 'Normal',
    showLineNumbers: true,
    lineNumberSide: 'Left',
    showMinimap: true,
    copyOnSelect: true,
    gitSyncInterval: 5,
    agentAccess: 'Off',
    agentAccessLowered: false,
    agentAccessRefused: false,
    overviewWidth: 200,
    toolPanelWidth: 240,
    showMenuBar: false,
    blinkFoundInMinimap: true,
    widthGeneration: 0,
  };
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

/** The reason a failing save gives in the mock. */
export const MOCK_SAVE_ERROR = 'Permission denied (os error 13)';

/** The text of a fake note. */
export function fakeNoteText(note: string): string {
  if (note === TOUR_NOTE) {
    return TOUR_TEXT;
  }
  const title = (note.split('/').at(-1) ?? note).replace(/-/g, ' ');
  return `# ${title}\n\nThis note lives in the mock backend.\nType here to try the source view.\n`;
}

/**
 * Handlers answering like a window started without a workspace (M-02),
 * with their own settings and status that the commands change.
 */
export function mockHandlers(start: MockStart = {}): MockHandlers {
  let settings: SettingsView = { ...defaultSettings(), ...start.settings };
  let status = start.status ?? '';
  const setStatus = async (text: string) => {
    status = text;
    await emit('status', { text });
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
        ...info,
        settings,
        status,
      };
    },
    settings_quick: async ({ key, value }) => {
      if (key === 'theme') {
        const palette = paletteOf(value);
        if (!palette) {
          const names = PALETTES.map((p) => p.theme).join(', ');
          await setStatus(`${JSON.stringify(value)} is none of ${names}`);
          return null;
        }
        settings = { ...settings, theme: value, palette };
      } else if (key === 'show_menu_bar') {
        settings = { ...settings, showMenuBar: value === 'true' };
      } else if (key === 'overview_width') {
        settings = { ...settings, overviewWidth: Number(value) };
      } else if (key === 'input_mode') {
        settings = { ...settings, inputMode: value };
      } else if (key === 'show_minimap') {
        settings = { ...settings, showMinimap: value === 'true' };
      } else if (key === 'copy_on_select') {
        settings = { ...settings, copyOnSelect: value === 'true' };
      } else if (key === 'show_line_numbers') {
        settings = { ...settings, showLineNumbers: value === 'true' };
      } else if (key === 'line_number_side') {
        settings = { ...settings, lineNumberSide: value };
      } else if (key === 'blink_found_in_minimap') {
        settings = { ...settings, blinkFoundInMinimap: value === 'true' };
      } else {
        return null;
      }
      await emit('settings', settings);
      return null;
    },
    wiki_check: async ({ url }) => {
      await setStatus(wikiStatus(url));
      return null;
    },
    ...workspaceHandlers(
      start,
      setStatus,
      () => settings,
      () => status,
    ),
  };
}

/** One note tab of the fake session. */
type FakeTab = { note: string; view: EditingViewName };

/** The workspace commands over the fake file system. */
function workspaceHandlers(
  start: MockStart,
  setStatus: (text: string) => Promise<void>,
  settings: () => SettingsView,
  status: () => string = () => '',
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
        label:
          count(title(note).toLowerCase()) > 1
            ? `${title(note)}  (${folder(note) ?? 'top folder'})`
            : title(note),
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
    const looks = settings();
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
  const label = (note: string) =>
    title(note) + (saveErrors.has(note) ? ' ⚠' : unstored.has(note) ? ' •' : '');
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
  const sendAll = async () => {
    const now = snapshot();
    await emit('workspace', now.workspace);
    await emit('overview', now.overview);
    await emit('tabs', now.tabs);
    await emit('properties', properties());
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
  const switchTo = (dir: string) => {
    root = dir;
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
        styles: [{ color: settings().palette.heading, bold: true, italic: false }],
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
        followMirror(note, mirror);
        await changed(note);
        setTimeout(() => void sendEnrich(note), 0);
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
  };
  handlers.__hooks = {
    backendEdit: async (note, at, remove, insert, sealed = true) => {
      const mirror = mirrors.get(note);
      const rev = mirror?.backendEdit(at, remove, insert, sealed);
      if (mirror && rev !== undefined) {
        followMirror(note, mirror);
        await changed(note);
        await emit('ed-changed', { note, rev });
      }
      return rev ?? null;
    },
    outsideChange: async (note, text) => {
      const mirror = mirrors.get(note);
      if (!mirror) {
        return null;
      }
      const rev = mirror.reload(text);
      followMirror(note, mirror);
      await emit('ed-changed', { note, rev });
      await setStatus(`${title(note)} was changed outside`);
      return rev;
    },
    mirrorText: (note) => mirrors.get(note)?.text() ?? null,
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

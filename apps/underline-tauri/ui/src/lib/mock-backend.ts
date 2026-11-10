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
// are tested in Rust. The open notes are fake documents
// (`editor/fakeEditor.ts`): the source view can be typed in, the autosave
// marks and clears the tab after a second, and notes listed in
// `MockStart.failingSaves` cannot be stored, to try the failure paths.

import { mockIPC } from '@tauri-apps/api/mocks';
import { emit } from '@tauri-apps/api/event';
import type {
  ClipAction,
  commands,
  CreateAnswer,
  EditorInput,
  EditingViewName,
  NewWorkspaceRequest,
  OpenAnswer,
  OverviewItem,
  PaletteView,
  SettingName,
  SettingsView,
  StartInfo,
  EditorPatch,
  TabPropertiesView,
  TabsView,
  UiError,
  WorkspaceSnapshot,
} from './bindings';
import { FakeEditor } from './editor/fakeEditor';
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
  ed_open: (args: { note: string }) => Ok<typeof commands.edOpen>;
  ed_input: (args: {
    note: string;
    input: EditorInput;
  }) => Ok<typeof commands.edInput> | Promise<Ok<typeof commands.edInput>>;
  ed_select: (args: {
    note: string;
    rev: number;
    anchor: number;
    head: number;
  }) => Ok<typeof commands.edSelect>;
  ed_viewport: (args: {
    note: string;
    from: number;
    to: number;
  }) => Promise<Ok<typeof commands.edViewport>>;
  ed_clipboard: (args: {
    note: string;
    action: ClipAction;
  }) => Promise<Ok<typeof commands.edClipboard>>;
  app_close_anyway: () => Ok<typeof commands.appCloseAnyway>;
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
  '/home/user/Work': ['README', 'Meetings', 'Projects/Roadmap'],
  '/home/user/Letters': ['Draft'],
};

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
};

/** The reason a failing save gives in the mock. */
export const MOCK_SAVE_ERROR = 'Permission denied (os error 13)';

/** The text of a fake note. */
export function fakeNoteText(note: string): string {
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
    ...workspaceHandlers(start, setStatus, () => settings),
  };
}

/** One note tab of the fake session. */
type FakeTab = { note: string; view: EditingViewName };

/** The workspace commands over the fake file system. */
function workspaceHandlers(
  start: MockStart,
  setStatus: (text: string) => Promise<void>,
  settings: () => SettingsView,
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
  | 'ed_open'
  | 'ed_input'
  | 'ed_select'
  | 'ed_viewport'
  | 'ed_clipboard'
  | 'app_close_anyway'
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
  /** The fake documents of the open notes, and their save state. */
  const docs = new Map<string, FakeEditor>();
  const unstored = new Set<string>();
  const saveErrors = new Map<string, string>();
  const saveTimers = new Map<string, ReturnType<typeof setTimeout>>();
  let clipboard: string | null = null;
  const docOf = (note: string): FakeEditor => {
    let doc = docs.get(note);
    if (!doc) {
      doc = new FakeEditor(note, fakeNoteText(note));
      docs.set(note, doc);
    }
    return doc;
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
            size: `${new TextEncoder().encode(docOf(active).text).length} bytes`,
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
  const marked = (patch: EditorPatch, note: string): EditorPatch => ({
    ...patch,
    unstored: unstored.has(note),
  });
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
      tabs.splice(at, 0, { note, view: 'facade' });
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

  return {
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
      docs.delete(note);
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
    ed_open: ({ note }) => {
      if (!tabs.some((tab) => tab.note === note)) {
        throw notOpen(note);
      }
      return docOf(note).state();
    },
    ed_input: async ({ note, input }) => {
      const doc = docOf(note);
      const patch = doc.input(input);
      if (patch.changes.length > 0) {
        await changed(note);
      }
      return marked(patch, note);
    },
    ed_select: ({ note, rev, anchor, head }) => {
      docOf(note).select(rev, anchor, head);
      return null;
    },
    ed_viewport: async ({ note, from, to }) => {
      const doc = docOf(note);
      const lines = doc.text.split('\n').slice(from, to);
      await emit('ed-highlight', {
        note,
        rev: doc.rev,
        from,
        lines: lines.map((line) =>
          line.startsWith('#') ? [[0, line.length, 0] as [number, number, number]] : [],
        ),
        styles: [{ color: settings().palette.heading, bold: true, italic: false }],
      });
      return null;
    },
    ed_clipboard: async ({ note, action }) => {
      const doc = docOf(note);
      let patch: EditorPatch;
      if (action === 'copy') {
        clipboard = doc.selected() || clipboard;
        patch = doc.unchanged();
      } else if (action === 'cut') {
        const cut = doc.cut();
        clipboard = cut.text || clipboard;
        patch = cut.patch;
      } else {
        patch = clipboard === null ? doc.unchanged() : doc.paste(clipboard);
      }
      if (patch.changes.length > 0) {
        await changed(note);
      }
      return marked(patch, note);
    },
    app_close_anyway: () => null,
  };
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

// Feature F10: the settings window (UC-010, inventory M-20.1 to M-20.5),
// against the mock backend's settings (`mock-settings.ts`).

import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { clearMocks } from '@tauri-apps/api/mocks';
import { afterEach, describe, expect, it, vi } from 'vitest';
import App from '../App.svelte';
import { connectedShell } from '../test/shell';
import { mockHandlers, mockHooks, type MockStart } from './mock-backend';

afterEach(() => {
  mockHooks().dispose();
  cleanup();
  clearMocks();
  vi.restoreAllMocks();
});

async function app(start: MockStart = {}, handlers = mockHandlers(start)) {
  const shell = await connectedShell(start, handlers);
  render(App, { props: { shell } });
  await shell.startWorkspace();
  return shell;
}

const NOTES: MockStart = { givenDir: '/home/user/Notes' };
const WORK: MockStart = { givenDir: '/home/user/Work' };

const settings = () => screen.getByRole('dialog', { name: 'Settings' });
const inSettings = () => within(settings());

function ctrlComma(target: Element = document.body) {
  return fireEvent.keyDown(target, { key: ',', ctrlKey: true });
}

async function openSettings() {
  await ctrlComma();
  return waitFor(() => inSettings().getByLabelText('Theme'));
}

/** Chooses the option `label` of the select named `name`. */
async function choose(name: string, label: string) {
  const select = inSettings().getByLabelText<HTMLSelectElement>(name);
  const option = within(select).getByRole('option', { name: label }) as HTMLOptionElement;
  select.value = option.value;
  await fireEvent.change(select);
  return select;
}

/** The text the closed select named `name` shows. */
function shown(name: string): string {
  const select = inSettings().getByLabelText<HTMLSelectElement>(name);
  return select.selectedOptions[0]?.textContent ?? '';
}

/** The third column of the row of `control`. */
function noteOf(control: HTMLElement): string {
  // The row's cells are siblings in the grid: name, control, note.
  let cell: Element | null = control.closest('.grid > *');
  while (cell && !cell.classList.contains('note')) {
    cell = cell.nextElementSibling;
  }
  return cell?.textContent?.trim() ?? '';
}

describe('M-20.1 the window "Settings"', () => {
  it('Ctrl+, opens it and closes it again, wherever the keyboard is', async () => {
    await app(NOTES);

    await openSettings();
    expect(settings()).toHaveStyle({ '--dialog-width': '460px' });
    expect(settings()).toHaveClass('resizable');
    await ctrlComma(settings());

    expect(screen.queryByRole('dialog', { name: 'Settings' })).not.toBeInTheDocument();
  });

  it('Escape and "×" close it; the changes stay (UC-010 step 5)', async () => {
    const shell = await app(NOTES);
    await openSettings();
    await choose('Theme', 'Nord');
    await waitFor(() => expect(shell.settings?.theme).toBe('Nord'));

    await fireEvent.keyDown(inSettings().getByRole('tab', { name: 'For me' }), { key: 'Escape' });
    expect(screen.queryByRole('dialog', { name: 'Settings' })).not.toBeInTheDocument();
    await openSettings();
    await fireEvent.click(inSettings().getByRole('button', { name: 'Close' }));

    expect(screen.queryByRole('dialog', { name: 'Settings' })).not.toBeInTheDocument();
    expect(shell.settings?.theme).toBe('Nord');
  });

  it('M-03.7 "Settings…" of the menu opens it, with the keyboard on its part', async () => {
    await app({ ...NOTES, settings: { showMenuBar: true } });
    await fireEvent.click(screen.getByRole('menuitem', { name: 'Settings' }));
    await fireEvent.click(screen.getByRole('menuitem', { name: 'Settings…' }));

    const forMe = await waitFor(() => inSettings().getByRole('tab', { name: 'For me' }));
    await waitFor(() => expect(forMe).toHaveFocus());
  });
});

describe('M-20.2 "For me" and "This workspace"', () => {
  it('UC-010 A9: without a workspace only "For me", without parts or the default format', async () => {
    await app();
    await openSettings();

    expect(inSettings().queryByRole('tablist')).not.toBeInTheDocument();
    expect(inSettings().getByRole('group', { name: 'For me' })).toBeInTheDocument();
    expect(inSettings().getByText('These settings apply in every workspace.')).toBeInTheDocument();
    expect(inSettings().queryByLabelText('Default format')).not.toBeInTheDocument();
    expect(shown('Theme')).toBe('Light');
    await waitFor(() => expect(inSettings().getByLabelText('Theme')).toHaveFocus());
  });

  it('with a workspace both, the part chosen with the arrow keys', async () => {
    const shell = await app(NOTES);
    await openSettings();
    const forMe = inSettings().getByRole('tab', { name: 'For me' });

    await fireEvent.keyDown(forMe, { key: 'ArrowRight' });

    const workspace = inSettings().getByRole('tab', { name: 'This workspace' });
    expect(workspace).toHaveAttribute('aria-selected', 'true');
    await waitFor(() => expect(workspace).toHaveFocus());
    expect(shell.settingsScope).toBe('workspace');
    expect(
      inSettings().getByText('These settings apply in this workspace only.'),
    ).toBeInTheDocument();
    expect(shown('Default format')).toBe('Markdown (from the notes)');
    expect(inSettings().queryByText(/apply to everyone who uses it/)).not.toBeInTheDocument();
  });

  it('TC-010-14 BR-007: a repository that shares the file says so', async () => {
    await app(WORK);
    await openSettings();
    await fireEvent.click(inSettings().getByRole('tab', { name: 'This workspace' }));

    expect(
      inSettings().getByText(
        'These values are in underline.yaml of the repository and apply to everyone who uses it.',
      ),
    ).toBeInTheDocument();
  });
});

describe('M-20.3 to M-20.5 the settings and their changes', () => {
  it("TC-010-02: every setting with its built-in default, in egui's order", async () => {
    await app(NOTES);
    await openSettings();

    const names = Array.from(settings().querySelectorAll('.grid .name')).map((n) => n.textContent);
    expect(names).toEqual([
      'Theme',
      'Input mode',
      'Line numbers',
      'Line number side',
      'Minimap',
      'Copy on select',
      'Git sync every (minutes)',
      'Agent access',
      'Note overview width',
      'Tool panel width',
      'Menu bar',
      'Search hits in the minimap',
    ]);
    expect(shown('Input mode')).toBe('Normal');
    expect(
      inSettings().getByRole('checkbox', { name: 'Line numbers Shown, in the source view' }),
    ).toBeChecked();
    expect(inSettings().getByRole('checkbox', { name: 'Menu bar Shown' })).not.toBeChecked();
    expect(inSettings().getByLabelText('Git sync every (minutes)')).toHaveValue(5);
    expect(inSettings().getByLabelText('Note overview width')).toHaveValue(200);
    expect(mockHooks().settingsFile('user')).toEqual({});
  });

  it('TC-010-01: a theme for me applies at once and is stored', async () => {
    const handlers = mockHandlers(NOTES);
    const set = vi.fn(handlers.settings_set);
    const shell = await app(NOTES, { ...handlers, settings_set: set });
    await openSettings();

    await choose('Theme', 'Nord');

    expect(set).toHaveBeenCalledWith({ scope: 'me', key: 'theme', value: 'Nord' });
    await waitFor(() => expect(shell.settings?.theme).toBe('Nord'));
    expect(document.documentElement.style.getPropertyValue('--palette-background')).not.toBe('');
    expect(mockHooks().settingsFile('user')).toEqual({ theme: 'Nord' });
    expect(shown('Theme')).toBe('Nord');
  });

  it('TC-010-04/05 A1, A2: a value for this workspace, and "As for me"', async () => {
    const shell = await app(NOTES);
    await openSettings();
    await fireEvent.click(inSettings().getByRole('tab', { name: 'This workspace' }));
    expect(noteOf(inSettings().getByLabelText('Theme'))).toBe('as for me');

    await choose('Theme', 'Dracula');

    await waitFor(() => expect(shell.settings?.theme).toBe('Dracula'));
    expect(mockHooks().settingsFile('workspace')).toEqual({ theme: 'Dracula' });
    expect(mockHooks().settingsFile('user')).toEqual({});
    const asForMe = inSettings().getByRole('button', { name: 'As for me: Theme' });
    expect(asForMe).toHaveAttribute('title', 'Use the setting from "For me" again');
    await fireEvent.click(inSettings().getByRole('tab', { name: 'For me' }));
    expect(noteOf(inSettings().getByLabelText('Theme'))).toBe('this workspace uses its own value');
    expect(shown('Theme')).toBe('Light');

    await fireEvent.click(inSettings().getByRole('tab', { name: 'This workspace' }));
    await fireEvent.click(inSettings().getByRole('button', { name: 'As for me: Theme' }));

    await waitFor(() => expect(noteOf(inSettings().getByLabelText('Theme'))).toBe('as for me'));
    expect(shell.settings?.theme).toBe('Light');
    expect(mockHooks().settingsFile('workspace')).toEqual({});
  });

  it('M-20.5: choosing the value shown gives the workspace no value of its own (egui)', async () => {
    await app(NOTES);
    await openSettings();
    await fireEvent.click(inSettings().getByRole('tab', { name: 'This workspace' }));

    await choose('Input mode', 'Normal');

    expect(mockHooks().settingsFile('workspace')).toEqual({});
  });

  it('TC-010-11 A8: the default format of the workspace, and "Follow the notes"', async () => {
    await app(NOTES);
    await openSettings();
    await fireEvent.click(inSettings().getByRole('tab', { name: 'This workspace' }));

    await choose('Default format', 'AsciiDoc');

    await waitFor(() => expect(shown('Default format')).toBe('AsciiDoc'));
    expect(noteOf(inSettings().getByLabelText('Default format'))).toBe('own');
    expect(mockHooks().settingsFile('workspace')).toEqual({ default_format: 'asciidoc' });

    await choose('Default format', 'Follow the notes');

    await waitFor(() => expect(shown('Default format')).toBe('Markdown (from the notes)'));
    expect(mockHooks().settingsFile('workspace')).toEqual({});
  });

  it('TC-010-03: check boxes switch line numbers and the minimap at once', async () => {
    const shell = await app(NOTES);
    await openSettings();

    await fireEvent.click(
      inSettings().getByRole('checkbox', { name: 'Line numbers Shown, in the source view' }),
    );
    await choose('Line number side', 'Right');
    await fireEvent.click(inSettings().getByRole('checkbox', { name: 'Minimap Shown' }));

    await waitFor(() => expect(shell.settings?.showMinimap).toBe(false));
    expect(shell.settings?.showLineNumbers).toBe(false);
    expect(shell.settings?.lineNumberSide).toBe('Right');
    expect(inSettings().getByRole('checkbox', { name: 'Minimap Shown' })).not.toBeChecked();
    expect(mockHooks().settingsFile('user')).toEqual({
      show_line_numbers: 'false',
      line_number_side: 'Right',
      show_minimap: 'false',
    });
  });

  it('M-20.5 BR-003: a quick switch goes to the workspace file when it sets the key', async () => {
    const shell = await app(NOTES);
    await openSettings();
    await fireEvent.click(inSettings().getByRole('tab', { name: 'This workspace' }));
    await fireEvent.click(inSettings().getByRole('checkbox', { name: 'Copy on select On' }));
    await waitFor(() => expect(shell.settings?.copyOnSelect).toBe(false));

    await shell.quickSetting('copy_on_select', 'true');
    await shell.quickSetting('show_minimap', 'false');

    expect(mockHooks().settingsFile('workspace')).toEqual({ copy_on_select: 'true' });
    expect(mockHooks().settingsFile('user')).toEqual({ show_minimap: 'false' });
    await waitFor(() =>
      expect(inSettings().getByRole('checkbox', { name: 'Copy on select On' })).toBeChecked(),
    );
  });

  it('M-20.3: numbers stay in their range; arrow keys step and store', async () => {
    const shell = await app(NOTES);
    await openSettings();
    const width = inSettings().getByLabelText<HTMLInputElement>('Note overview width');
    const sync = inSettings().getByLabelText<HTMLInputElement>('Git sync every (minutes)');

    await fireEvent.change(width, { target: { value: '5000' } });
    await fireEvent.keyDown(sync, { key: 'ArrowUp' });

    await waitFor(() => expect(width).toHaveValue(1000));
    expect(sync).toHaveValue(6);
    await waitFor(() =>
      expect(mockHooks().settingsFile('user')).toEqual({
        overview_width: '1000',
        git_sync_interval: '6',
      }),
    );
    expect(shell.settings?.overviewWidth).toBe(1000);
    // Q-03: a width changed in the window sets the panel to it.
    await waitFor(() => expect(shell.overviewWidth).toBe(1000));
  });

  it('M-20.3: a number field left empty keeps the value in effect', async () => {
    await app(NOTES);
    await openSettings();
    const width = inSettings().getByLabelText<HTMLInputElement>('Tool panel width');

    await fireEvent.change(width, { target: { value: '' } });

    expect(width).toHaveValue(240);
    expect(mockHooks().settingsFile('user')).toEqual({});
  });

  it('TC-010-15 BR-008: a workspace only lowers agent access', async () => {
    await app({ ...NOTES, settings: { agentAccess: 'Read only' } });
    await openSettings();
    await fireEvent.click(inSettings().getByRole('tab', { name: 'This workspace' }));

    const access = inSettings().getByLabelText<HTMLSelectElement>('Agent access');
    const offered = within(access)
      .getAllByRole('option')
      .map((o) => o.textContent);
    expect(offered).toEqual(['Off', 'Read only']);
  });
});

describe('UC-010 A5 to A7: files changed, broken or not writable', () => {
  it('A5: a file changed outside shows in the open window', async () => {
    const shell = await app(NOTES);
    await openSettings();

    await mockHooks().settingsOutside('user', 'theme', 'Solarized Dark');
    await mockHooks().settingsOutside('workspace', 'input_mode', 'Vim');

    await waitFor(() => expect(shown('Theme')).toBe('Solarized Dark'));
    expect(shell.settings?.inputMode).toBe('Vim');
    expect(noteOf(inSettings().getByLabelText('Input mode'))).toBe(
      'this workspace uses its own value',
    );
  });

  it('A7: a change that cannot be stored applies and says why', async () => {
    const shell = await app(NOTES);
    mockHooks().settingsUnwritable(true);
    await openSettings();

    await choose('Theme', 'Ubuntu');

    await waitFor(() => expect(shell.settings?.theme).toBe('Ubuntu'));
    expect(shell.status).toBe(
      'The setting could not be stored and applies until Underline is closed: Permission denied (os error 13)',
    );
  });

  it('the window says why it could not be filled', async () => {
    // Without a workspace no view was sent before.
    await app(
      {},
      {
        ...mockHandlers(),
        settings_window: () => {
          throw { kind: 'internal', message: 'The settings could not be read' };
        },
      },
    );
    vi.spyOn(console, 'error').mockImplementation(() => {});

    await ctrlComma();

    await waitFor(() =>
      expect(inSettings().getByRole('alert')).toHaveTextContent('The settings could not be read'),
    );
  });
});

/** Types `text` into the number field `input` as the keyboard would, without leaving it. */
function typeInto(input: HTMLInputElement, text: string) {
  input.focus();
  input.value = text;
  input.dispatchEvent(new Event('input', { bubbles: true }));
}

/** A key as the keyboard sends it, bubbling from `target`. */
function press(target: Element, key: string, init: KeyboardEventInit = {}) {
  target.dispatchEvent(
    new KeyboardEvent('keydown', { key, bubbles: true, cancelable: true, ...init }),
  );
}

describe('review F10', () => {
  it('app-tester: Enter in a number field sends it, without leaving the field', async () => {
    const shell = await app(NOTES);
    await openSettings();
    const width = inSettings().getByLabelText<HTMLInputElement>('Tool panel width');

    typeInto(width, '300');
    press(width, 'Enter');

    await waitFor(() => expect(shell.settings?.toolPanelWidth).toBe(300));
    expect(mockHooks().settingsFile('user')).toEqual({ tool_panel_width: '300' });
    expect(width).toHaveFocus();
  });

  it.each([
    ['Escape', (target: Element) => press(target, 'Escape')],
    ['Ctrl+,', (target: Element) => press(target, ',', { ctrlKey: true })],
  ])('ux 3: a number typed and not sent is stored when %s closes the window', async (_, close) => {
    const shell = await app(NOTES);
    await openSettings();
    const sync = inSettings().getByLabelText<HTMLInputElement>('Git sync every (minutes)');

    typeInto(sync, '12');
    close(sync);

    await waitFor(() => expect(shell.settingsOpen).toBe(false));
    await waitFor(() =>
      expect(mockHooks().settingsFile('user')).toEqual({ git_sync_interval: '12' }),
    );
  });

  it('ux 4: quick steps write once, the last value, and no answer undoes a step', async () => {
    const handlers = mockHandlers(NOTES);
    const set = vi.fn(handlers.settings_set);
    await app(NOTES, { ...handlers, settings_set: set });
    await openSettings();
    const sync = inSettings().getByLabelText<HTMLInputElement>('Git sync every (minutes)');

    for (const key of ['ArrowUp', 'ArrowUp', 'PageUp', 'ArrowDown']) {
      press(sync, key);
    }

    expect(sync).toHaveValue(16);
    await waitFor(() => expect(set).toHaveBeenCalledTimes(1));
    expect(set).toHaveBeenCalledWith({ scope: 'me', key: 'git_sync_interval', value: '16' });
    expect(sync).toHaveValue(16);
  });

  it('ux 1: after "As for me" the keyboard stays in the row', async () => {
    await app(NOTES);
    await openSettings();
    await fireEvent.click(inSettings().getByRole('tab', { name: 'This workspace' }));
    await choose('Theme', 'Dracula');
    const asForMe = await waitFor(() =>
      inSettings().getByRole('button', { name: 'As for me: Theme' }),
    );
    asForMe.focus();

    await fireEvent.click(asForMe);

    await waitFor(() => expect(inSettings().getByLabelText('Theme')).toHaveFocus());
  });

  it('ux 5: a control is described by its note, "As for me" by its hint', async () => {
    await app(NOTES);
    await openSettings();
    await fireEvent.click(inSettings().getByRole('tab', { name: 'This workspace' }));
    await choose('Theme', 'Dracula');

    const asForMe = await waitFor(() =>
      inSettings().getByRole('button', { name: 'As for me: Theme' }),
    );
    expect(asForMe).toHaveAccessibleDescription('Use the setting from "For me" again');
    expect(inSettings().getByLabelText('Input mode')).toHaveAccessibleDescription('as for me');
  });

  it('ux 2: what went wrong stays in the window until a change is stored', async () => {
    await app(NOTES);
    mockHooks().settingsUnwritable(true);
    await openSettings();

    await choose('Theme', 'Ubuntu');

    const alert = await waitFor(() => inSettings().getByRole('alert'));
    expect(alert).toHaveTextContent(
      'The setting could not be stored and applies until Underline is closed: Permission denied (os error 13)',
    );
    mockHooks().settingsUnwritable(false);
    await choose('Theme', 'Nord');
    await waitFor(() => expect(inSettings().queryByRole('alert')).not.toBeInTheDocument());
  });

  it('app-tester: an open menu closes when Ctrl+, opens the window', async () => {
    await app({ ...NOTES, settings: { showMenuBar: true } });
    await fireEvent.click(screen.getByRole('menuitem', { name: 'Theme' }));
    expect(screen.getByRole('menu')).toBeInTheDocument();

    await openSettings();

    await waitFor(() => expect(screen.queryByRole('menu')).not.toBeInTheDocument());
  });

  it('app-tester: after a list opened and closed by itself, Escape closes the window', async () => {
    await app(NOTES);
    await openSettings();
    const theme = inSettings().getByLabelText('Theme');

    // The native list opens with the pointer and closes with its own Escape,
    // which the page never sees.
    await fireEvent.pointerDown(theme);
    await fireEvent.mouseDown(theme);
    press(theme, 'Escape');

    await waitFor(() =>
      expect(screen.queryByRole('dialog', { name: 'Settings' })).not.toBeInTheDocument(),
    );
  });

  it('app-tester: the window keeps its width when the app window narrows', async () => {
    await app(NOTES);
    await openSettings();
    // jsdom lays nothing out: the sizes of a window 500 px wide.
    const frame = settings();
    const parent = frame.parentElement!;
    Object.defineProperty(frame, 'offsetParent', { configurable: true, get: () => parent });
    Object.defineProperty(frame, 'offsetWidth', { configurable: true, get: () => 460 });
    Object.defineProperty(parent, 'clientWidth', { configurable: true, get: () => 500 });

    window.dispatchEvent(new Event('resize'));

    expect(settings().style.getPropertyValue('--dialog-left')).toBe('20px');
  });

  it('"Settings…" names its keys for screen readers', async () => {
    await app({ ...NOTES, settings: { showMenuBar: true } });
    await fireEvent.click(screen.getByRole('menuitem', { name: 'Settings' }));

    expect(screen.getByRole('menuitem', { name: 'Settings…' })).toHaveAttribute(
      'aria-keyshortcuts',
      'Control+,',
    );
  });
});

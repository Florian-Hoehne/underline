// Feature F6 (UC-005 A2, A9 to A13, A17 to A19): the clipboard with HTML
// and images, copy on select, the Link and Image dialogs and dropped files,
// against the mock backend (M-09.14, M-10.3 to M-10.7).

import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { clearMocks } from '@tauri-apps/api/mocks';
import { afterEach, describe, expect, it, vi } from 'vitest';
import App from '../App.svelte';
import { connectedShell } from '../test/shell';
import { activeSourceView } from './editor/active';
import { mockHandlers, mockHooks, type MockHandlers, type MockStart } from './mock-backend';

afterEach(() => {
  cleanup();
  clearMocks();
  vi.restoreAllMocks();
});

async function workbench(start: MockStart = {}, handlers: MockHandlers = mockHandlers(start)) {
  const shell = await connectedShell(start, handlers);
  render(App, { props: { shell } });
  await shell.startWorkspace();
  return shell;
}

const SOURCE = { givenDir: '/home/user/Notes', view: 'source' as const };
const FACADE = { givenDir: '/home/user/Notes' };

async function textField(): Promise<HTMLElement> {
  await waitFor(() => expect(activeSourceView()).not.toBeNull());
  return document.querySelector<HTMLElement>('.cm-content')!;
}

const text = () => activeSourceView()!.view.state.doc.toString();
const statusBar = () => screen.getByRole('status');

function press(field: HTMLElement, key: string, init: KeyboardEventInit = {}): KeyboardEvent {
  const event = new KeyboardEvent('keydown', { key, bubbles: true, cancelable: true, ...init });
  field.dispatchEvent(event);
  return event;
}

function paste(field: HTMLElement): Event {
  const event = new Event('paste', { bubbles: true, cancelable: true });
  field.dispatchEvent(event);
  return event;
}

/** Puts the source view's cursor at `at`, as a click would. */
async function placeCursor(at: number) {
  const view = activeSourceView()!;
  view.view.dispatch({ selection: { anchor: at }, userEvent: 'select.pointer' });
  await view.settled();
}

describe('M-10.3 the dialog "Insert link" (UC-005 A2)', () => {
  it('Link asks for a web address; Enter inserts it at the cursor, and the dialog closes', async () => {
    await workbench(SOURCE);
    const field = await textField();
    await placeCursor(0);

    await fireEvent.click(screen.getByRole('button', { name: 'Link (Ctrl+K)' }));
    const dialog = await screen.findByRole('dialog', { name: 'Insert link' });
    const address = within(dialog).getByLabelText('Web address:');
    await waitFor(() => expect(address).toHaveFocus());
    expect(within(dialog).getByRole('button', { name: 'Insert' })).toBeDisabled();
    await fireEvent.input(address, { target: { value: ' https://x.org ' } });
    await fireEvent.submit(address.closest('form')!);

    await waitFor(() => expect(text()).toMatch(/^\[https:\/\/x\.org\]\(https:\/\/x\.org\)# Home/));
    expect(screen.queryByRole('dialog', { name: 'Insert link' })).toBeNull();
    await waitFor(() => expect(field).toHaveFocus());
  });

  it('a note of the workspace, found by its name, goes in as a relative link with its title', async () => {
    await workbench(SOURCE);
    await textField();
    await placeCursor(0);

    press(document.querySelector('.cm-content')!, 'k', { ctrlKey: true });
    const dialog = await screen.findByRole('dialog', { name: 'Insert link' });
    const filter = within(dialog).getByLabelText('Or a note of the workspace:');
    await fireEvent.input(filter, { target: { value: 'START' } });
    const notes = within(dialog).getByRole('list', { name: 'Notes of the workspace' });
    expect(
      within(notes)
        .getAllByRole('button')
        .map((b) => b.textContent),
    ).toEqual(['Getting Started']);
    await fireEvent.click(within(notes).getByRole('button', { name: 'Getting Started' }));

    await waitFor(() =>
      expect(text()).toMatch(/^\[Getting Started\]\(guide\/Getting-Started\.md\)# Home/),
    );
    expect(screen.queryByRole('dialog')).toBeNull();
  });

  it('Escape closes it and nothing is inserted', async () => {
    await workbench(SOURCE);
    await textField();
    const before = text();

    await fireEvent.click(screen.getByRole('button', { name: 'Link (Ctrl+K)' }));
    const dialog = await screen.findByRole('dialog', { name: 'Insert link' });
    await fireEvent.keyDown(within(dialog).getByLabelText('Web address:'), { key: 'Escape' });

    expect(screen.queryByRole('dialog')).toBeNull();
    expect(text()).toBe(before);
  });
});

describe('M-10.4 the dialog "Insert image" (UC-005 A2, A19)', () => {
  it('a typed image file goes in as a picture; a file that is no image as a link', async () => {
    await workbench(SOURCE);
    await textField();
    await placeCursor(0);

    await fireEvent.click(screen.getByRole('button', { name: 'Image (Ctrl+Shift+I)' }));
    let dialog = await screen.findByRole('dialog', { name: 'Insert image' });
    const path = within(dialog).getByLabelText('Image file:');
    await waitFor(() => expect(path).toHaveFocus());
    await fireEvent.input(path, { target: { value: '/home/user/Pictures/photo.png' } });
    await fireEvent.click(within(dialog).getByRole('button', { name: 'Insert' }));
    await waitFor(() => expect(text()).toMatch(/^!\[\]\(photo\.png\)\n\n# Home/));
    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());

    await placeCursor(0);
    await fireEvent.click(screen.getByRole('button', { name: 'Image (Ctrl+Shift+I)' }));
    dialog = await screen.findByRole('dialog', { name: 'Insert image' });
    await fireEvent.input(within(dialog).getByLabelText('Image file:'), {
      target: { value: '/home/user/Documents/report.pdf' },
    });
    await fireEvent.click(within(dialog).getByRole('button', { name: 'Insert' }));
    await waitFor(() => expect(text()).toMatch(/^\[report\.pdf\]\(report\.pdf\)!\[\]/));
  });

  it('a file over 10 MB is asked about; "Link to its place" links to it where it is', async () => {
    await workbench(SOURCE);
    await textField();
    await placeCursor(0);

    await fireEvent.click(screen.getByRole('button', { name: 'Image (Ctrl+Shift+I)' }));
    let dialog = await screen.findByRole('dialog', { name: 'Insert image' });
    await fireEvent.input(within(dialog).getByLabelText('Image file:'), {
      target: { value: '/home/user/Pictures/poster.png' },
    });
    await fireEvent.click(within(dialog).getByRole('button', { name: 'Insert' }));

    // The dialog is made anew for the question.
    await waitFor(() =>
      expect(screen.getByRole('dialog', { name: 'Insert image' })).toHaveTextContent(
        '/home/user/Pictures/poster.png has 12 MB. Copy it into the workspace, or only link to where it is (it is then not part of the workspace and not pushed with git)?',
      ),
    );
    dialog = screen.getByRole('dialog', { name: 'Insert image' });
    expect(within(dialog).getByRole('button', { name: 'Copy' })).toBeInTheDocument();
    await fireEvent.click(within(dialog).getByRole('button', { name: 'Link to its place' }));

    await waitFor(() =>
      expect(text()).toMatch(
        /^\[\/home\/user\/Pictures\/poster\.png\]\(\/home\/user\/Pictures\/poster\.png\)# Home/,
      ),
    );
  });

  it('a folder is refused: the dialog closes and the status bar says why', async () => {
    await workbench(SOURCE);
    await textField();
    const before = text();

    await fireEvent.click(screen.getByRole('button', { name: 'Image (Ctrl+Shift+I)' }));
    const dialog = await screen.findByRole('dialog', { name: 'Insert image' });
    await fireEvent.input(within(dialog).getByLabelText('Image file:'), {
      target: { value: '/home/user/Pictures' },
    });
    await fireEvent.click(within(dialog).getByRole('button', { name: 'Insert' }));

    await waitFor(() =>
      expect(statusBar()).toHaveTextContent(
        '/home/user/Pictures is a folder; folders cannot be put into a note',
      ),
    );
    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
    expect(text()).toBe(before);
  });
});

describe('M-09.14, M-10.6 pasting with formats (UC-005 A17, A18)', () => {
  it('the source view pastes the plain text, never the HTML', async () => {
    await workbench(SOURCE);
    const field = await textField();
    await placeCursor(0);
    mockHooks().setClipboard({ text: 'plain ', html: '<b>rich</b>' });

    expect(paste(field).defaultPrevented).toBe(true);

    await waitFor(() => expect(text()).toMatch(/^plain # Home/));
  });

  it('an image alone is stored next to the note and inserted; the status says so', async () => {
    await workbench(SOURCE);
    const field = await textField();
    await placeCursor(0);
    mockHooks().setClipboard({ image: true });

    paste(field);

    await waitFor(() => expect(text()).toMatch(/^!\[\]\(Home-2026-11-10-1200\.png\)\n\n# Home/));
    expect(statusBar()).toHaveTextContent('Pasted image stored as Home-2026-11-10-1200.png');
  });

  it('the facade pastes HTML of other programs as markup; Ctrl+Shift+V the plain text', async () => {
    await workbench(FACADE);
    const facade = await waitFor(() => {
      const element = document.querySelector<HTMLElement>('.facade-text');
      expect(element).not.toBeNull();
      expect(element!.querySelector('[data-b="0"]')).not.toBeNull();
      return element!;
    });
    mockHooks().setClipboard({ text: 'Install bold', html: '<h2>Install</h2><p><b>bold</b></p>' });

    paste(facade);

    await waitFor(() =>
      expect(mockHooks().mirrorText('Home')).toMatch(/^## Install\n\n\*\*bold\*\*\n\n# Home/),
    );

    press(facade, 'V', { ctrlKey: true, shiftKey: true });
    await waitFor(() => expect(mockHooks().mirrorText('Home')).toContain('Install bold'));
  });

  it('copying in the facade puts the markup with its HTML on the clipboard', async () => {
    await workbench(FACADE);
    const facade = await waitFor(() => {
      const element = document.querySelector<HTMLElement>('.facade-text');
      expect(element?.querySelector('[data-b="0"]')).toBeTruthy();
      return element!;
    });
    // Ctrl+A selects the note in the core.
    press(facade, 'a', { ctrlKey: true });
    await waitFor(() => expect(mockHooks().clipboardContent().text).toMatch(/^# Home/));

    facade.dispatchEvent(new Event('copy', { bubbles: true, cancelable: true }));

    await waitFor(() => expect(mockHooks().clipboardContent().html).toMatch(/^<h1>Home<\/h1>/));
  });
});

describe('M-10.7 copy on select (FR-031, FR-032, BR-011)', () => {
  it('a selection goes to the clipboard at once; pasting over it brings the former content', async () => {
    await workbench(SOURCE);
    const field = await textField();
    mockHooks().setClipboard({ text: 'former' });

    activeSourceView()!.view.dispatch({
      selection: { anchor: 0, head: 6 },
      userEvent: 'select.pointer',
    });
    await waitFor(() => expect(mockHooks().clipboardContent().text).toBe('# Home'));

    paste(field);

    await waitFor(() => expect(text()).toMatch(/^former\n\nThis note/));
  });

  it('switched off, a selection leaves the clipboard alone', async () => {
    await workbench({ ...SOURCE, settings: { copyOnSelect: false } });
    await textField();
    mockHooks().setClipboard({ text: 'kept' });

    activeSourceView()!.view.dispatch({
      selection: { anchor: 0, head: 6 },
      userEvent: 'select.pointer',
    });
    await activeSourceView()!.settled();
    await new Promise((resolve) => setTimeout(resolve, 20));

    expect(mockHooks().clipboardContent().text).toBe('kept');
  });
});

describe('M-10.5 dropping files (UC-005 A9 to A13)', () => {
  it('dragging says where it goes; the items go in at the cursor with spaces between', async () => {
    await workbench(SOURCE);
    await textField();
    await placeCursor(0);

    mockHooks().dragEnter();
    await waitFor(() =>
      expect(statusBar()).toHaveTextContent('Drop to insert at the place under the pointer'),
    );
    // Off the text: the cursor stays where it is.
    mockHooks().drop(['https://x.org', '/home/user/Pictures/photo.png'], -10, -10);

    // The picture stands on a paragraph of its own (`apply_format`).
    await waitFor(() =>
      expect(text()).toMatch(/^\[https:\/\/x\.org\]\(https:\/\/x\.org\)\s*\n\n!\[\]\(photo\.png\)/),
    );
  });

  it('nothing to drop is said in the status bar', async () => {
    await workbench(SOURCE);
    await textField();

    mockHooks().drop([], -10, -10);

    await waitFor(() =>
      expect(statusBar()).toHaveTextContent('This cannot be dropped into a note'),
    );
  });
});

describe('review F6: dropping with large files, leaving, and no note', () => {
  it('a drop with two large files asks for each in turn and keeps the order', async () => {
    await workbench(SOURCE);
    await textField();
    await placeCursor(0);

    mockHooks().drop(
      [
        '/home/user/Pictures/photo.png',
        '/home/user/Pictures/poster.png',
        '/home/user/Documents/report.pdf',
        '/home/user/Pictures/banner.png',
      ],
      -10,
      -10,
    );
    await waitFor(() =>
      expect(screen.getByRole('dialog', { name: 'Insert image' })).toHaveTextContent(
        '/home/user/Pictures/poster.png has 12 MB.',
      ),
    );
    // The question is the dialog's description for screen readers.
    const question = screen.getByText(/poster\.png has 12 MB/);
    expect(screen.getByRole('dialog', { name: 'Insert image' })).toHaveAttribute(
      'aria-describedby',
      question.id,
    );
    await fireEvent.click(screen.getByRole('button', { name: 'Link to its place' }));
    await waitFor(() =>
      expect(screen.getByRole('dialog', { name: 'Insert image' })).toHaveTextContent(
        '/home/user/Pictures/banner.png has 11 MB.',
      ),
    );
    await fireEvent.click(screen.getByRole('button', { name: 'Cancel' }));

    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
    const now = text();
    const at = (needle: string) => now.indexOf(needle);
    expect(at('![](photo.png)')).toBeGreaterThanOrEqual(0);
    expect(at('(/home/user/Pictures/poster.png)')).toBeGreaterThan(at('![](photo.png)'));
    expect(at('[report.pdf](report.pdf)')).toBeGreaterThan(at('(/home/user/Pictures/poster.png)'));
    expect(now).not.toContain('banner');
  });

  it('the hint goes when the files leave; a drop with no note shown is said', async () => {
    await workbench({ givenDir: '/home/user/Letters' });
    await waitFor(() => expect(screen.getByText('Select or create a note.')).toBeInTheDocument());

    mockHooks().dragEnter();
    await waitFor(() =>
      expect(statusBar()).toHaveTextContent('Drop to insert at the place under the pointer'),
    );
    mockHooks().dragLeave();
    await waitFor(() => expect(statusBar()).not.toHaveTextContent('Drop to insert'));

    mockHooks().drop(['/home/user/Pictures/photo.png'], 10, 10);
    await waitFor(() => expect(statusBar()).toHaveTextContent('Open a note to drop files into it'));
  });

  it('the link dialog says when no note is found', async () => {
    await workbench(SOURCE);
    await textField();

    await fireEvent.click(screen.getByRole('button', { name: 'Link (Ctrl+K)' }));
    const dialog = await screen.findByRole('dialog', { name: 'Insert link' });
    await fireEvent.input(within(dialog).getByLabelText('Or a note of the workspace:'), {
      target: { value: 'zzz' },
    });

    expect(within(dialog).getByRole('status')).toHaveTextContent('No notes found');
  });
});

describe('app-tester F6: a put of "+ in Vim visual mode', () => {
  const VIM = { ...SOURCE, settings: { inputMode: 'Vim' } };

  it('replaces the visual selection and the Vim keys go on working', async () => {
    const errors = vi.spyOn(console, 'error');
    await workbench({ ...VIM, settings: { inputMode: 'Vim', copyOnSelect: false } });
    const field = await textField();
    mockHooks().setClipboard({ text: 'TXT' });

    for (const key of ['v', 'l', '"', '+', 'p']) {
      press(field, key);
    }
    await waitFor(() => expect(text()).toMatch(/^TXTHome/));
    press(field, 'x');

    await waitFor(() => expect(text()).toMatch(/^TXHome/));
    expect(errors.mock.calls.flat().join(' ')).not.toMatch(/crashed|Invalid position/);
  });

  it('a visual block (Ctrl+V) put leaves the Vim keys working', async () => {
    const errors = vi.spyOn(console, 'error');
    await workbench({ ...VIM, settings: { inputMode: 'Vim', copyOnSelect: false } });
    const field = await textField();
    mockHooks().setClipboard({ text: 'TXT' });

    press(field, 'v', { ctrlKey: true });
    for (const key of ['j', 'l', '"', '+', 'p']) {
      press(field, key);
    }
    await waitFor(() => expect(text()).toContain('TXT'));
    const before = text();
    press(field, 'x');

    await waitFor(() => expect(text().length).toBe(before.length - 1));
    expect(errors.mock.calls.flat().join(' ')).not.toMatch(/crashed|Invalid position/);
  });

  it('a paste event outside insert mode brings no text, as egui; in insert mode it does', async () => {
    await workbench(VIM);
    const field = await textField();
    const before = text();
    mockHooks().setClipboard({ text: 'TXT' });

    expect(paste(field).defaultPrevented).toBe(true);
    await activeSourceView()!.settled();
    expect(text()).toBe(before);

    // In insert mode the text comes.
    press(field, 'i');
    paste(field);
    await waitFor(() => expect(text()).toBe(`TXT${before}`));
  });
});

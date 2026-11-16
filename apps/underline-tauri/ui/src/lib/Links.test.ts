// Feature F7: following links (M-07.3, UC-004 A2 to A4) and the tool
// panel's outline, code and figures (M-16.5), against the mock backend.

import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { clearMocks } from '@tauri-apps/api/mocks';
import { afterEach, describe, expect, it, vi } from 'vitest';
import App from '../App.svelte';
import { connectedShell } from '../test/shell';
import { activeSourceView } from './editor/active';
import { jumpTo } from './editor/jump';
import { activeFacade } from './facade/active';
import {
  LINK_TOUR_NOTE,
  LINK_TOUR_TEXT,
  mockHandlers,
  mockHooks,
  mockIndexes,
  mockOpenQuestion,
  type MockHandlers,
  type MockStart,
} from './mock-backend';

afterEach(() => {
  mockHooks().dispose();
  cleanup();
  clearMocks();
  vi.restoreAllMocks();
});

const WORK = { givenDir: '/home/user/Work' };

async function workbench(start: MockStart = WORK, change: Partial<MockHandlers> = {}) {
  const shell = await connectedShell(start, change);
  render(App, { props: { shell } });
  await shell.startWorkspace();
  return shell;
}

/** Workbench whose `ed_follow_link` is watched. */
async function watched(start: MockStart = WORK) {
  const handlers = mockHandlers(start);
  const follow = vi.fn(handlers.ed_follow_link);
  const shell = await workbench(start, { ...handlers, ed_follow_link: follow });
  return { shell, follow };
}

/** The facade's text once its first block is drawn. */
async function facadeText(): Promise<HTMLElement> {
  return waitFor(() => {
    const found = document.querySelector<HTMLElement>('.facade-text');
    expect(found?.querySelector('[data-b="0"]')).toBeTruthy();
    return found!;
  });
}

/** The run of the facade that links to `address`. */
async function linkRun(address: string): Promise<HTMLElement> {
  const text = await facadeText();
  return waitFor(() => {
    const run = text.querySelector<HTMLElement>(`[data-link="${CSS.escape(address)}"]`);
    expect(run).not.toBeNull();
    return run!;
  });
}

/** Where the heading text `heading` starts in the link tour. */
function headingAt(heading: string): number {
  return LINK_TOUR_TEXT.indexOf(heading);
}

const tools = () => screen.getByRole('complementary', { name: 'Tools' });

describe('M-07.3 following links (UC-004 A2 to A4)', () => {
  it('UC-004 BR-006 a Ctrl+click on a link in the facade jumps to the heading it names', async () => {
    const { shell, follow } = await watched();
    await shell.openNote(LINK_TOUR_NOTE);
    const run = await linkRun('#details');
    const jump = vi.spyOn(activeFacade()!, 'jump');

    await fireEvent.click(run, { ctrlKey: true });

    await waitFor(() => expect(jump).toHaveBeenCalledWith(headingAt('Details'), 1));
    // The place of the link's first character, not its address.
    expect(follow).toHaveBeenCalledWith({
      note: LINK_TOUR_NOTE,
      at: LINK_TOUR_TEXT.indexOf('Down to the details'),
      rev: 1,
      side: false,
    });
  });

  it('UC-004 BR-006 a plain click, or Cmd on Linux, does not follow', async () => {
    const { shell, follow } = await watched();
    await shell.openNote(LINK_TOUR_NOTE);
    const run = await linkRun('#details');

    await fireEvent.click(run);
    await fireEvent.click(run, { metaKey: true });
    await new Promise((resolve) => setTimeout(resolve, 20));

    expect(follow).not.toHaveBeenCalled();
  });

  it('UC-004 BR-006 Cmd+click follows on macOS', async () => {
    const start = { ...WORK, platform: 'macos' as const };
    const { shell, follow } = await watched(start);
    await shell.openNote(LINK_TOUR_NOTE);

    await fireEvent.click(await linkRun('#details'), { ctrlKey: true });
    await new Promise((resolve) => setTimeout(resolve, 20));
    expect(follow).not.toHaveBeenCalled();
    await fireEvent.click(await linkRun('#details'), { metaKey: true });

    await waitFor(() => expect(follow).toHaveBeenCalled());
  });

  it('UC-004 A2 a heading that is not there is said in the status bar', async () => {
    const shell = await workbench();
    await shell.openNote(LINK_TOUR_NOTE);

    await fireEvent.click(await linkRun('#nowhere'), { ctrlKey: true });

    await waitFor(() => expect(shell.status).toBe('The heading nowhere was not found'));
  });

  it('UC-004 A2 a link to another note opens it at its heading', async () => {
    const shell = await workbench();
    await shell.openNote(LINK_TOUR_NOTE);

    await fireEvent.click(await linkRun('Projects/Roadmap.md#roadmap'), { ctrlKey: true });

    await waitFor(() => expect(shell.tabs.active).toBe('Projects/Roadmap'));
    // The cursor stands at the heading's text, after "# ".
    await waitFor(() => expect(document.querySelector('.place')).toHaveTextContent('Ln 1, Col 3'));
  });

  it('UC-004 A3 M-14.1 a note that does not exist is offered in "New note"', async () => {
    const shell = await workbench();
    await shell.openNote(LINK_TOUR_NOTE);

    await fireEvent.click(await linkRun('Ideas/Someday.md'), { ctrlKey: true });

    const dialog = await screen.findByRole('dialog', { name: 'New note' });
    expect(within(dialog).getByLabelText('Title:')).toHaveValue('Someday');
    expect(
      within(dialog).getByLabelText('Folder (empty for the top folder, new folders are created):'),
    ).toHaveValue('Ideas');
    expect(within(dialog).getByRole('alert')).toHaveTextContent(
      'Someday does not exist yet. Create it?',
    );
    await waitFor(() => expect(within(dialog).getByText('File: Ideas/Someday.md')).toBeTruthy());
    expect(shell.tabs.active).toBe(LINK_TOUR_NOTE);

    await fireEvent.click(within(dialog).getByRole('button', { name: 'Create' }));

    await waitFor(() => expect(shell.tabs.active).toBe('Ideas/Someday'));
    await waitFor(() => expect(shell.status).toBe('Created the note Someday'));
    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'New note' })).toBeNull());
  });

  it('UC-004 BR-005 a link leading outside the workspace is not followed', async () => {
    const shell = await workbench();
    await shell.openNote(LINK_TOUR_NOTE);

    await fireEvent.click(await linkRun('../../etc/hosts'), { ctrlKey: true });

    await waitFor(() =>
      expect(shell.status).toBe('../../etc/hosts leads outside the workspace and is not followed'),
    );
  });

  it('UC-004 A4 a file that does not exist is said', async () => {
    const shell = await workbench();
    await shell.openNote(LINK_TOUR_NOTE);

    await fireEvent.click(await linkRun('report.pdf'), { ctrlKey: true });

    await waitFor(() => expect(shell.status).toBe('/home/user/Work/report.pdf does not exist'));
  });

  it('UC-004 BR-007 Q-62 a web address opens directly, every time', async () => {
    const shell = await workbench();
    await shell.openNote(LINK_TOUR_NOTE);
    const address = 'https://example.com/underline';

    await fireEvent.click(await linkRun(address), { ctrlKey: true });
    await waitFor(() => expect(mockHooks().openedWithSystem()).toEqual([address]));
    await fireEvent.click(await linkRun(address), { ctrlKey: true });
    await waitFor(() => expect(mockHooks().openedWithSystem()).toEqual([address, address]));
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  });

  it('UC-004 BR-007 Q-62 another scheme is asked about every time, naming it', async () => {
    const shell = await workbench();
    await shell.openNote(LINK_TOUR_NOTE);
    const address = 'ssh://server.example/home';

    await fireEvent.click(await linkRun(address), { ctrlKey: true });
    const dialog = await screen.findByRole('alertdialog', { name: 'Open ssh: link' });
    expect(dialog).toHaveAccessibleDescription(
      `This link uses the ssh: scheme and opens another program on this computer. Open it? Address: ${address}`,
    );
    // The keyboard starts on "Cancel"; Escape cancels too (D-02).
    expect(within(dialog).getByRole('button', { name: 'Cancel' })).toHaveFocus();
    await fireEvent.keyDown(dialog, { key: 'Escape' });
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
    expect(mockHooks().openedWithSystem()).toEqual([]);

    // Asked again; "Open" opens it.
    await fireEvent.click(await linkRun(address), { ctrlKey: true });
    const again = await screen.findByRole('alertdialog', { name: 'Open ssh: link' });
    await fireEvent.click(within(again).getByRole('button', { name: 'Open link' }));
    await waitFor(() => expect(mockHooks().openedWithSystem()).toEqual([address]));
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();

    // "Open" lets nothing through without asking.
    await fireEvent.click(await linkRun(address), { ctrlKey: true });
    const third = await screen.findByRole('alertdialog', { name: 'Open ssh: link' });
    await fireEvent.click(within(third).getByRole('button', { name: 'Cancel' }));
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
    expect(mockHooks().openedWithSystem()).toEqual([address]);
  });

  it('UC-004 BR-007 Q-61 a link to a program asks first, naming the file and its kind', async () => {
    const shell = await workbench();
    await shell.openNote(LINK_TOUR_NOTE);

    await fireEvent.click(await linkRun('tools/deploy.sh'), { ctrlKey: true });
    const dialog = await screen.findByRole('alertdialog', { name: 'Open program' });
    expect(dialog).toHaveTextContent(
      'deploy.sh is a shell script. ⚠ Warning: Opening it may run it on this computer. Open it?',
    );
    expect(within(dialog).getByText('Opening it may run it on this computer.').tagName).toBe(
      'STRONG',
    );
    expect(dialog).toHaveTextContent('/home/user/Work/tools/deploy.sh');
    expect(within(dialog).getByRole('button', { name: 'Cancel' })).toHaveFocus();
    await fireEvent.click(within(dialog).getByRole('button', { name: 'Cancel' }));
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
    expect(mockHooks().openedWithSystem()).toEqual([]);

    await fireEvent.click(await linkRun('tools/deploy.sh'), { ctrlKey: true });
    const again = await screen.findByRole('alertdialog', { name: 'Open program' });
    await fireEvent.click(within(again).getByRole('button', { name: 'Open program' }));
    await waitFor(() =>
      expect(mockHooks().openedWithSystem()).toEqual(['/home/user/Work/tools/deploy.sh']),
    );
  });

  it('UC-004 A4 Q-61 a file:// target asks first, naming the file and its kind', async () => {
    const shell = await workbench();
    await shell.openNote(LINK_TOUR_NOTE);
    const link = 'file:///home/user/Documents/report.pdf';

    await fireEvent.click(await linkRun(link), { ctrlKey: true });
    const dialog = await screen.findByRole('alertdialog', { name: 'Open file' });
    expect(dialog).toHaveTextContent(
      'This link points to a file on this computer: report.pdf, PDF file. Open it?',
    );
    expect(dialog).toHaveTextContent('Address: /home/user/Documents/report.pdf');
    // "Copy" puts the address on the clipboard.
    await fireEvent.click(within(dialog).getByRole('button', { name: 'Copy' }));
    await waitFor(() =>
      expect(within(dialog).getByRole('button', { name: 'Copied' })).toBeInTheDocument(),
    );
    await fireEvent.click(within(dialog).getByRole('button', { name: 'Open file' }));
    await waitFor(() =>
      expect(mockHooks().openedWithSystem()).toEqual(['/home/user/Documents/report.pdf']),
    );
  });

  it('ADR-025 the edits reach the backend before the link is followed', async () => {
    const handlers = mockHandlers(WORK);
    const calls: string[] = [];
    const shell = await workbench(WORK, {
      ...handlers,
      ed_sync: async (args) => {
        calls.push('sync');
        return handlers.ed_sync(args);
      },
      ed_follow_link: async (args) => {
        calls.push('follow');
        return handlers.ed_follow_link(args);
      },
    });
    await shell.openNote(LINK_TOUR_NOTE);
    const text = await facadeText();
    text.dispatchEvent(
      new InputEvent('beforeinput', {
        inputType: 'insertText',
        data: 'x',
        bubbles: true,
        cancelable: true,
      }),
    );

    await fireEvent.click(await linkRun('#details'), { ctrlKey: true });

    await waitFor(() => expect(calls).toContain('follow'));
    expect(calls.indexOf('sync')).toBeLessThan(calls.indexOf('follow'));
  });

  it('UC-004 BR-006 a Ctrl+click in the source view follows the link under it', async () => {
    const start = { ...WORK, view: 'source' as const };
    const { shell, follow } = await watched(start);
    await shell.openNote(LINK_TOUR_NOTE);
    const view = await waitFor(() => {
      const shown = activeSourceView();
      expect(shown?.note).toBe(LINK_TOUR_NOTE);
      return shown!;
    });
    // jsdom lays nothing out: the place under the pointer is given.
    const label = LINK_TOUR_TEXT.indexOf('Down to the details') + 2;
    vi.spyOn(view.view, 'posAtCoords').mockReturnValue(label);
    const jump = vi.spyOn(view, 'jump');

    await fireEvent.click(view.view.contentDOM, { ctrlKey: true, clientX: 5, clientY: 5 });

    await waitFor(() =>
      expect(follow).toHaveBeenCalledWith({ note: LINK_TOUR_NOTE, at: label, rev: 1, side: false }),
    );
    await waitFor(() => expect(jump).toHaveBeenCalledWith(headingAt('Details'), 1));
    await waitFor(() => expect(view.view.state.selection.main.head).toBe(headingAt('Details')));
  });

  it('UC-004 BR-006 a Ctrl+click in the source view beside a link follows nothing', async () => {
    const start = { ...WORK, view: 'source' as const };
    const { shell, follow } = await watched(start);
    await shell.openNote(LINK_TOUR_NOTE);
    const view = await waitFor(() => {
      const shown = activeSourceView();
      expect(shown?.note).toBe(LINK_TOUR_NOTE);
      return shown!;
    });
    vi.spyOn(view.view, 'posAtCoords').mockReturnValue(3);

    await fireEvent.click(view.view.contentDOM, { ctrlKey: true, clientX: 5, clientY: 5 });
    await view.settled();

    expect(follow).not.toHaveBeenCalled();
  });
});

describe('M-16.5 outline, code and figures (FR-041 to FR-043)', () => {
  it('the tool panel lists the headings, the code and the figures, each with its count', async () => {
    const shell = await workbench();
    await shell.openNote(LINK_TOUR_NOTE);

    const outline = await within(tools()).findByRole('list', { name: 'Outline' });
    expect(within(tools()).getByText('Outline (3)')).toBeInTheDocument();
    expect(within(tools()).getByText('Code (1)')).toBeInTheDocument();
    expect(within(tools()).getByText('Figures (2)')).toBeInTheDocument();
    const headings = within(outline).getAllByRole('button');
    expect(headings.map((button) => button.textContent)).toEqual([
      'Link tour',
      'Details',
      'Deeper',
    ]);
    // 14 px per level below the highest.
    expect(headings.map((button) => button.style.paddingLeft)).toEqual([
      'calc(var(--space-1) + 0px)',
      'calc(var(--space-1) + 14px)',
      'calc(var(--space-1) + 28px)',
    ]);
    const figures = within(tools()).getByRole('list', { name: 'Figures' });
    expect(
      within(figures)
        .getAllByRole('button')
        .map((b) => b.textContent),
    ).toEqual(['Mermaid diagram: graph TD', 'The plan']);
    expect(
      within(within(tools()).getByRole('list', { name: 'Code' })).getByRole('button'),
    ).toHaveTextContent('sh: ls -l');
  });

  it('an empty list says "None"', async () => {
    const shell = await workbench();
    await shell.openNote('Meetings');

    await waitFor(() => expect(within(tools()).getByText('Code (0)')).toBeInTheDocument());
    expect(within(tools()).getByText('Outline (1)')).toBeInTheDocument();
    expect(within(tools()).getAllByText('None', { selector: 'p' })).toHaveLength(2);
  });

  it('lists of another note are not shown', async () => {
    const shell = await workbench();
    await shell.openNote(LINK_TOUR_NOTE);
    await within(tools()).findByRole('list', { name: 'Outline' });

    shell.indexes = mockIndexes('README', 1, '# Elsewhere\n');

    await waitFor(() =>
      expect(within(tools()).queryByRole('list', { name: 'Outline' })).not.toBeInTheDocument(),
    );
    expect(within(tools()).getByText('Properties')).toBeInTheDocument();
  });

  it('the lists follow the text', async () => {
    const shell = await workbench();
    await shell.openNote(LINK_TOUR_NOTE);
    await within(tools()).findByText('Outline (3)');
    await waitFor(() => expect(mockHooks().mirrorText(LINK_TOUR_NOTE)).not.toBeNull());

    await mockHooks().backendEdit(LINK_TOUR_NOTE, 0, 0, '# New top\n\n');

    await within(tools()).findByText('Outline (4)');
  });

  it('a click in the facade scrolls the block to the top and leaves the cursor', async () => {
    const shell = await workbench();
    await shell.openNote(LINK_TOUR_NOTE);
    await facadeText();
    const reveal = vi.spyOn(activeFacade()!, 'reveal');
    const jump = vi.spyOn(activeFacade()!, 'jump');
    const outline = await within(tools()).findByRole('list', { name: 'Outline' });

    await fireEvent.click(within(outline).getByRole('button', { name: 'Deeper, level 3' }));

    expect(reveal).toHaveBeenCalledWith(LINK_TOUR_TEXT.indexOf('### Deeper'));
    expect(jump).not.toHaveBeenCalled();
  });

  it('a click in the source view puts the cursor there and the keyboard into the text', async () => {
    const start = { ...WORK, view: 'source' as const };
    const shell = await workbench(start);
    await shell.openNote(LINK_TOUR_NOTE);
    const view = await waitFor(() => {
      const shown = activeSourceView();
      expect(shown?.note).toBe(LINK_TOUR_NOTE);
      return shown!;
    });
    const code = await within(tools()).findByRole('list', { name: 'Code' });

    await fireEvent.click(within(code).getByRole('button', { name: 'sh: ls -l' }));

    const at = LINK_TOUR_TEXT.indexOf('```sh');
    await waitFor(() => expect(view.view.state.selection.main.head).toBe(at));
    expect(view.view.hasFocus).toBe(true);
  });

  it('ux review F7: headings deeper than 4 levels are indented no further', async () => {
    const shell = await workbench();
    await shell.openNote(LINK_TOUR_NOTE);
    await within(tools()).findByRole('list', { name: 'Outline' });

    shell.indexes = {
      ...mockIndexes(LINK_TOUR_NOTE, 1, '# A\n'),
      outline: [0, 4, 6].map((depth) => ({ label: `H${depth}`, depth, offset: depth })),
    };

    await waitFor(() =>
      expect(within(tools()).getByRole('button', { name: 'H6, level 7' })).toBeInTheDocument(),
    );
    expect(within(tools()).getByRole('button', { name: 'H4, level 5' }).style.paddingLeft).toBe(
      'calc(var(--space-1) + 56px)',
    );
    expect(within(tools()).getByRole('button', { name: 'H6, level 7' }).style.paddingLeft).toBe(
      'calc(var(--space-1) + 56px)',
    );
  });
});

describe('M-07.3 a jump carries the revision of its place', () => {
  it('jumps while the text is that revision, else nothing moves', async () => {
    const start = { ...WORK, view: 'source' as const };
    const shell = await workbench(start);
    await shell.openNote(LINK_TOUR_NOTE);
    const view = await waitFor(() => {
      const shown = activeSourceView();
      expect(shown?.note).toBe(LINK_TOUR_NOTE);
      return shown!;
    });
    await view.settled();
    const before = view.view.state.selection.main.head;

    jumpTo(LINK_TOUR_NOTE, 99, headingAt('Details'));
    await view.settled();
    expect(view.view.state.selection.main.head).toBe(before);

    jumpTo(LINK_TOUR_NOTE, 1, headingAt('Details'));
    await waitFor(() => expect(view.view.state.selection.main.head).toBe(headingAt('Details')));
  });
});

describe('the mock asks as the session does (Q-61, Q-62)', () => {
  it('opens http, https and mailto directly, and asks about every other scheme', () => {
    expect(mockOpenQuestion('HTTPS://example.org', 'address')).toBeNull();
    expect(mockOpenQuestion('mailto:me@example.org', 'address')).toBeNull();
    expect(mockOpenQuestion('ms-msdt://id', 'address')?.title).toBe('Open ms-msdt: link');
    expect(mockOpenQuestion('smb://server/share', 'address')?.target).toBe('smb://server/share');
    // An unreadable scheme is named as written, never "Open : link".
    expect(mockOpenQuestion('\u200bhttp://x', 'address')?.title).toBe('Open \\u{200b}http: link');
  });

  it('names a program and its kind with one article, as the session does', () => {
    expect(mockOpenQuestion('/w/run.BAT', 'file')).toMatchObject({
      about: 'program',
      question: 'run.BAT is a batch file.',
      warning: 'Opening it may run it on this computer.',
    });
    expect(mockOpenQuestion('/w/x.hta', 'file')?.question).toBe('x.hta is an HTML application.');
    expect(mockOpenQuestion('/w/a.exe::$DATA', 'file')?.about).toBe('program');
    expect(mockOpenQuestion('/w/report.pdf', 'file')).toBeNull();
    expect(mockOpenQuestion('/home/user/Pictures', 'fileUrl')?.question).toBe(
      'This link points to a file on this computer: Pictures, folder.',
    );
    expect(mockOpenQuestion('//server/share/a.pdf', 'file')?.question).toBe(
      'This link points to a file on another computer or a device: a.pdf.',
    );
    expect(mockOpenQuestion('/w/evil\u202efdp.exe', 'file')?.question).toBe(
      'evil\\u{202e}fdp.exe is a Windows program.',
    );
  });
});

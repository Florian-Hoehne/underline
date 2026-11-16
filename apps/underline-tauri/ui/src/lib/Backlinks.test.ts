// Package 3 (FR-078, FR-079, UC-004 step 4, BR-015, A14): the backlinks
// and outgoing links of the tool panel, against the mock backend's
// workspace `/home/user/Links` (TC-004-29 to TC-004-31). The rules of the
// lists are the backend's, tested in Rust; here what the window shows and
// does with them.

import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { clearMocks } from '@tauri-apps/api/mocks';
import { afterEach, describe, expect, it, vi } from 'vitest';
import App from '../App.svelte';
import { connectedShell } from '../test/shell';
import { activeSourceView } from './editor/active';
import {
  LINKS,
  LINKS_TEXTS,
  mockFoundLinks,
  mockHandlers,
  mockHooks,
  mockLinkCheck,
  mockLinks,
  type MockHandlers,
  type MockStart,
} from './mock-backend';

afterEach(() => {
  mockHooks().dispose();
  cleanup();
  clearMocks();
  vi.restoreAllMocks();
});

const START: MockStart = { givenDir: LINKS, view: 'source' };

async function workbench(start: MockStart = START, change: Partial<MockHandlers> = {}) {
  const shell = await connectedShell(start, change);
  render(App, { props: { shell } });
  await shell.startWorkspace();
  return shell;
}

const tools = () => screen.getByRole('complementary', { name: 'Tools' });

/** The section of the tool panel whose summary starts with `name`. */
function section(name: string): HTMLElement {
  const summary = within(tools())
    .getAllByText((_, element) => element?.tagName === 'SUMMARY')
    .find((element) => element.textContent?.startsWith(name));
  expect(summary, name).toBeTruthy();
  return summary!.parentElement!;
}

/** The text of the source view of `note`, once it shows. */
async function sourceOf(note: string) {
  return waitFor(() => {
    const view = activeSourceView();
    expect(view?.note).toBe(note);
    return view!;
  });
}

describe('FR-078 backlinks (UC-004 BR-015, A14)', () => {
  it('TC-004-29 the notes linking to the note, with their lines and the link marked', async () => {
    const shell = await workbench();
    await shell.openNote('Project-Plan');

    // "Hub" (TC-004-30) links to it too.
    await waitFor(() => expect(section('Backlinks')).toHaveTextContent('Backlinks (5)'));
    const list = within(section('Backlinks')).getByRole('list', { name: 'Backlinks' });
    const links = within(list).getAllByRole('button');
    expect(links.map((link) => link.getAttribute('aria-label'))).toEqual([
      '4 [the plan](Project-Plan.md) first, in Brainstorm',
      '9 [goals](Project-Plan.md#goals), in Brainstorm',
      '3 [Project Plan](Project-Plan.md), in Hub',
      '5 [Goals](Project-Plan.md#nothing), in Hub',
      '2 [Plan](../Project-Plan.md) for the trip, in Itinerary',
    ]);
    expect(list).toHaveTextContent('travel');
    // The link is marked in its line; code and the note itself are no backlinks.
    expect(links[0].querySelector('mark')).toHaveTextContent('[the plan](Project-Plan.md)');
    const sources = [...list.querySelectorAll('.source-name')].map((name) => name.textContent);
    expect(sources.map((name) => name?.trim())).toEqual(['Brainstorm', 'Hub', 'Itinerary travel']);
  });

  it('TC-004-29 step 2, A14 choosing a backlink opens its note with the cursor on the link', async () => {
    const handlers = mockHandlers(START);
    const open = vi.fn(handlers.links_open);
    const shell = await workbench(START, { ...handlers, links_open: open });
    await shell.openNote('Project-Plan');
    const list = await waitFor(() =>
      within(section('Backlinks')).getByRole('list', { name: 'Backlinks' }),
    );

    await fireEvent.click(within(list).getByRole('button', { name: /^9 .*in Brainstorm$/ }));

    const at = LINKS_TEXTS.Brainstorm.indexOf('[goals]');
    await waitFor(() =>
      expect(open).toHaveBeenCalledWith({
        note: 'Brainstorm',
        place: { from: at, to: at + 30, check: mockLinkCheck('[goals](Project-Plan.md#goals)') },
      }),
    );
    const view = await sourceOf('Brainstorm');
    await waitFor(() => expect(view.view.state.selection.main.head).toBe(at));
  });

  it('a note nobody links to says "None"; before the lists come, "Looking for links…"', async () => {
    const handlers = mockHandlers(START);
    const shell = await workbench(START, handlers);
    await shell.openNote('Diary');
    expect(section('Backlinks')).toHaveTextContent('Looking for links…');

    await waitFor(() => expect(section('Backlinks')).toHaveTextContent('Backlinks (0)'));
    expect(section('Backlinks')).toHaveTextContent('None');
    expect(section('Outgoing links')).toHaveTextContent('None');
  });

  it('TC-004-31 a link added by another program shows; an unsaved change of an open note counts', async () => {
    const shell = await workbench();
    await shell.openNote('Project-Plan');
    await waitFor(() => expect(section('Backlinks')).toHaveTextContent('Backlinks (5)'));

    await mockHooks().outsideChange('Diary', '# Diary\n\n[plan](Project-Plan.md)\n');
    await waitFor(() => expect(section('Backlinks')).toHaveTextContent('Backlinks (6)'));
    expect(section('Backlinks')).toHaveTextContent('Diary');

    // Step 2: "Brainstorm" in another tab loses its link of line 4.
    await shell.openNote('Brainstorm');
    await sourceOf('Brainstorm');
    const text = LINKS_TEXTS.Brainstorm;
    const at = text.indexOf('[the plan]');
    await mockHooks().backendEdit('Brainstorm', at, '[the plan](Project-Plan.md) '.length, '');
    await shell.openNote('Project-Plan');
    await waitFor(() => expect(section('Backlinks')).toHaveTextContent('Backlinks (5)'));
    const list = within(section('Backlinks')).getByRole('list', { name: 'Backlinks' });
    expect(within(list).queryByRole('button', { name: /^4 .*in Brainstorm$/ })).toBeNull();
    expect(within(list).getByRole('button', { name: /^9 .*in Brainstorm$/ })).toBeTruthy();
  });
});

describe('FR-079 outgoing links (UC-004 BR-015, A14)', () => {
  it('TC-004-30 the links in the order of the note, the broken ones marked in words', async () => {
    const shell = await workbench();
    await shell.openNote('Hub');

    await waitFor(() => expect(section('Outgoing links')).toHaveTextContent('Outgoing links (5)'));
    const list = within(section('Outgoing links')).getByRole('list', { name: 'Outgoing links' });
    const links = within(list).getAllByRole('button');
    expect(links.map((link) => link.getAttribute('aria-label'))).toEqual([
      'Project Plan, note Project-Plan, line 3',
      'Gone, note Gone.md, broken link, line 4',
      'Goals, note Project-Plan#nothing, broken link, line 5',
      'Logo, file img/logo.png, line 6',
      'https://example.org, web address https://example.org, line 7',
    ]);
    expect(links.map((link) => link.textContent?.includes('broken'))).toEqual([
      false,
      true,
      true,
      false,
      false,
    ]);
  });

  it('TC-004-30 step 2 choosing a link puts the cursor on it; with Ctrl it is followed', async () => {
    const handlers = mockHandlers(START);
    const follow = vi.fn(handlers.links_follow);
    const shell = await workbench(START, { ...handlers, links_follow: follow });
    await shell.openNote('Hub');
    const view = await sourceOf('Hub');
    const list = await waitFor(() =>
      within(section('Outgoing links')).getByRole('list', { name: 'Outgoing links' }),
    );
    const plan = within(list).getByRole('button', { name: /^Project Plan/ });

    await fireEvent.click(plan);
    const at = LINKS_TEXTS.Hub.indexOf('[Project Plan]');
    await waitFor(() => expect(view.view.state.selection.main.head).toBe(at));
    expect(follow).not.toHaveBeenCalled();

    // Step 3: with Ctrl held, "Project Plan" opens (A2).
    await fireEvent.click(plan, { ctrlKey: true });
    await waitFor(() => expect(follow).toHaveBeenCalledWith({ note: 'Hub', at, rev: 1 }));
    await sourceOf('Project-Plan');
  });

  it('Q-64: Ctrl+Enter on a link of the list follows it; the hint says so', async () => {
    const handlers = mockHandlers(START);
    const follow = vi.fn(handlers.links_follow);
    const shell = await workbench(START, { ...handlers, links_follow: follow });
    await shell.openNote('Hub');
    await sourceOf('Hub');
    const list = await waitFor(() =>
      within(section('Outgoing links')).getByRole('list', { name: 'Outgoing links' }),
    );
    const gone = within(list).getByRole('button', { name: /^Gone/ });
    expect(gone).toHaveAccessibleDescription('Ctrl+click or Ctrl+Enter follows a link.');

    await fireEvent.keyDown(gone, { key: 'Enter' });
    await new Promise((resolve) => setTimeout(resolve, 20));
    expect(follow).not.toHaveBeenCalled();
    await fireEvent.keyDown(gone, { key: 'Enter', ctrlKey: true });

    const at = LINKS_TEXTS.Hub.indexOf('[Gone]');
    await waitFor(() => expect(follow).toHaveBeenCalledWith({ note: 'Hub', at, rev: 1 }));
    // A3: the missing note is offered for creation.
    await waitFor(() => expect(shell.notes.newNote?.title).toBe('Gone'));
    // Its file is planned a moment later; not after the test.
    shell.closeNoteDialog('newNote');
  });
});

describe('the mock stands in for the link index', () => {
  it('checks a link as the backend does (LinkPlace::check_of)', () => {
    expect(mockLinkCheck('[goals](Plan.md#goals) é')).toBe(436_401_621);
  });

  it('finds links outside code and pictures, and resolves them over the notes', () => {
    expect(mockFoundLinks(LINKS_TEXTS.Snippets)).toEqual([]);
    expect(mockFoundLinks('![a](b.png) [c](d.md) <https://e.org>').map((l) => l.place)).toEqual([
      'd.md',
      'https://e.org',
    ]);
    const view = mockLinks(
      'Project-Plan',
      1,
      Object.keys(LINKS_TEXTS),
      (note) => LINKS_TEXTS[note],
      ['img/logo.png'],
    );
    expect(view.backlinkCount).toBe(5);
    expect(view.outgoing).toEqual([
      {
        text: 'the goals',
        target: '#goals',
        kind: 'note',
        broken: false,
        line: 5,
        offset: LINKS_TEXTS['Project-Plan'].indexOf('[the goals]'),
      },
    ]);
  });
});

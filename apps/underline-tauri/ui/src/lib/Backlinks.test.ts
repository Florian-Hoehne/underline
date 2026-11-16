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
  mockLinkPlace,
  mockLinks,
  mockLinkText,
  mockMentions,
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

    // "Hub" (TC-004-30) links to it too, and "Guide" includes it (OQ-244).
    await waitFor(() => expect(section('Backlinks')).toHaveTextContent('Backlinks (6)'));
    const list = within(section('Backlinks')).getByRole('list', { name: 'Backlinks' });
    const links = within(list).getAllByRole('button');
    expect(links.map((link) => link.getAttribute('aria-label'))).toEqual([
      '4 [the plan](Project-Plan.md) first, in Brainstorm',
      '9 [goals](Project-Plan.md#goals), in Brainstorm',
      '5 include::Project-Plan.md[], include, in Guide',
      '3 [Project Plan](Project-Plan.md), in Hub',
      '5 [Goals](Project-Plan.md#nothing), in Hub',
      '2 [Plan](../Project-Plan.md) for the trip, in Itinerary',
    ]);
    expect(list).toHaveTextContent('travel');
    // The link is marked in its line; code and the note itself are no backlinks.
    expect(links[0].querySelector('mark')).toHaveTextContent('[the plan](Project-Plan.md)');
    const sources = [...list.querySelectorAll('.source-name')].map((name) => name.textContent);
    expect(sources.map((name) => name?.trim())).toEqual([
      'Brainstorm',
      'Guide',
      'Hub',
      'Itinerary travel',
    ]);
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
    await waitFor(() => expect(section('Backlinks')).toHaveTextContent('Backlinks (6)'));

    await mockHooks().outsideChange('Diary', '# Diary\n\n[plan](Project-Plan.md)\n');
    await waitFor(() => expect(section('Backlinks')).toHaveTextContent('Backlinks (7)'));
    expect(section('Backlinks')).toHaveTextContent('Diary');

    // Step 2: "Brainstorm" in another tab loses its link of line 4.
    await shell.openNote('Brainstorm');
    await sourceOf('Brainstorm');
    const text = LINKS_TEXTS.Brainstorm;
    const at = text.indexOf('[the plan]');
    await mockHooks().backendEdit('Brainstorm', at, '[the plan](Project-Plan.md) '.length, '');
    await shell.openNote('Project-Plan');
    await waitFor(() => expect(section('Backlinks')).toHaveTextContent('Backlinks (6)'));
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

  it('finds links, pictures and includes outside code, and resolves them over the notes', () => {
    expect(mockFoundLinks(LINKS_TEXTS.Snippets)).toEqual([]);
    expect(
      mockFoundLinks('![a](b.png) [c](d.md) <https://e.org> include::f.adoc[] image::g.png[]').map(
        (l) => [l.form, l.place],
      ),
    ).toEqual([
      ['picture', 'b.png'],
      ['link', 'd.md'],
      ['link', 'https://e.org'],
      ['include', 'f.adoc'],
      ['picture', 'g.png'],
    ]);
    const view = mockLinks(
      'Project-Plan',
      1,
      Object.keys(LINKS_TEXTS),
      (note) => LINKS_TEXTS[note],
      ['img/logo.png'],
    );
    expect(view.backlinkCount).toBe(6);
    expect(view.outgoing).toEqual([
      {
        text: 'the goals',
        target: '#goals',
        kind: 'note',
        form: 'link',
        broken: false,
        line: 5,
        offset: LINKS_TEXTS['Project-Plan'].indexOf('[the goals]'),
      },
    ]);
  });
});

describe('OQ-244 pictures and included files count as links (UC-004 BR-015)', () => {
  it('TC-004-41 a picture is an outgoing link, marked "picture", and broken when missing', async () => {
    const shell = await workbench();
    await shell.openNote('travel/Itinerary');

    await waitFor(() => expect(section('Outgoing links')).toHaveTextContent('Outgoing links (2)'));
    const list = within(section('Outgoing links')).getByRole('list', { name: 'Outgoing links' });
    const links = within(list).getAllByRole('button');
    expect(links.map((link) => link.getAttribute('aria-label'))).toEqual([
      'Plan, note Project-Plan, line 2',
      'map.png, picture, file travel/map.png, broken link, line 3',
    ]);
    // Said in words, not by colour or an icon alone.
    expect(links[0].querySelector('.form')).toBeNull();
    expect(links[1].querySelector('.form')).toHaveTextContent('picture');
    expect(links[1]).toHaveTextContent('broken');
  });

  it('TC-004-41 an included note lists the including note as a backlink, marked "include"', async () => {
    const shell = await workbench();
    await shell.openNote('Project-Plan');
    const list = await waitFor(() =>
      within(section('Backlinks')).getByRole('list', { name: 'Backlinks' }),
    );
    const include = await waitFor(() =>
      within(list).getByRole('button', { name: /include, in Guide$/ }),
    );
    expect(include.querySelector('.form')).toHaveTextContent('include');
    expect(include.querySelector('mark')).toHaveTextContent('include::Project-Plan.md[]');
  });
});

describe('FR-103 unlinked mentions (OQ-199, UC-004 BR-021, A22)', () => {
  it('TC-004-42 step 1 lists the mentions of the title, not those in code or links', async () => {
    const shell = await workbench();
    await shell.openNote('Project-Plan');

    await waitFor(() =>
      expect(section('Unlinked mentions')).toHaveTextContent('Unlinked mentions (2)'),
    );
    const list = within(section('Unlinked mentions')).getByRole('list', {
      name: 'Mentions in Diary',
    });
    const mentions = within(list)
      .getAllByRole('button')
      .filter((button) => !button.classList.contains('link-mention'));
    expect(mentions.map((button) => button.getAttribute('aria-label'))).toEqual([
      'line 4, unlinked mention “project plan”: The project plan is late; ask about the Project Plan.',
      'line 4, unlinked mention “Project Plan”: The project plan is late; ask about the Project Plan.',
    ]);
    const marks = [...list.querySelectorAll('mark.unlinked')].map((mark) => mark.textContent);
    expect(marks).toEqual(['project plan', 'Project Plan']);
    // Two on one line are told apart.
    expect(
      within(list)
        .getAllByRole('button', { name: /^Link “/ })
        .map((button) => button.getAttribute('aria-label')),
    ).toEqual([
      'Link “project plan” in Diary, line 4, mention 1 of 2',
      'Link “Project Plan” in Diary, line 4, mention 2 of 2',
    ]);
    expect(section('Unlinked mentions')).toHaveTextContent(
      'Link turns the underlined text into a link to this note.',
    );
    expect(section('Unlinked mentions').querySelector('[aria-live="polite"]')).toHaveTextContent(
      '2 unlinked mentions',
    );
  });

  it('TC-004-42 step 3 "Link" turns a mention into a link to the note', async () => {
    const handlers = mockHandlers(START);
    const link = vi.fn(handlers.links_link_mention);
    const shell = await workbench(START, { ...handlers, links_link_mention: link });
    await shell.openNote('Project-Plan');
    const button = await waitFor(() =>
      within(section('Unlinked mentions')).getByRole('button', {
        name: 'Link “project plan” in Diary, line 4, mention 1 of 2',
      }),
    );

    await fireEvent.click(button);

    const at = new TextEncoder().encode(
      LINKS_TEXTS.Diary.slice(0, LINKS_TEXTS.Diary.indexOf('project plan')),
    ).length;
    await waitFor(() =>
      expect(link).toHaveBeenCalledWith({
        active: 'Project-Plan',
        note: 'Diary',
        place: { from: at, to: at + 12, check: mockLinkCheck('project plan') },
      }),
    );
    await waitFor(() =>
      expect(section('Unlinked mentions')).toHaveTextContent('Unlinked mentions (1)'),
    );
    await waitFor(() => expect(section('Backlinks')).toHaveTextContent('Backlinks (7)'));
    expect(shell.status).toBe(
      'Linked the mention of Project Plan in Diary. The earlier version is in its snapshots.',
    );
  });

  it('A22 a mention that moved meanwhile is not linked; the status says so', async () => {
    const handlers = mockHandlers(START);
    const shell = await workbench(START, handlers);
    await shell.openNote('Project-Plan');
    const button = await waitFor(() =>
      within(section('Unlinked mentions')).getByRole('button', {
        name: 'Link “Project Plan” in Diary, line 4, mention 2 of 2',
      }),
    );
    // Another program moves the text before the click arrives.
    await mockHooks().outsideChange('Diary', '# Diary\n\nMoved. ' + LINKS_TEXTS.Diary.slice(9));

    await fireEvent.click(button);

    await waitFor(() =>
      expect(shell.status).toBe(
        'Diary changed since the mentions were listed; the mention is no longer at its place',
      ),
    );
  });

  it('TC-004-42 step 4 at most 200 are listed; "Show more" lists more', async () => {
    const handlers = mockHandlers(START);
    const more = vi.fn(handlers.links_more_mentions);
    const shell = await workbench(START, { ...handlers, links_more_mentions: more });
    await shell.openNote('Project-Plan');
    await waitFor(() =>
      expect(section('Unlinked mentions')).toHaveTextContent('Unlinked mentions (2)'),
    );
    await mockHooks().outsideChange('Diary', 'Project plan, '.repeat(250));
    const button = await waitFor(() =>
      within(section('Unlinked mentions')).getByRole('button', {
        name: 'Show more (50 not listed)',
      }),
    );
    expect(section('Unlinked mentions')).toHaveTextContent('Unlinked mentions (250)');
    expect(section('Unlinked mentions').querySelectorAll('mark.unlinked')).toHaveLength(200);

    await fireEvent.click(button);

    await waitFor(() => expect(more).toHaveBeenCalledWith({ note: 'Project-Plan' }));
    await waitFor(() =>
      expect(section('Unlinked mentions').querySelectorAll('mark.unlinked')).toHaveLength(250),
    );
    expect(
      within(section('Unlinked mentions')).queryByRole('button', { name: /^Show more/ }),
    ).toBeNull();
  });

  it('BR-021 none, or a short title, folds the section shut with (0)', async () => {
    const shell = await workbench();
    await shell.openNote('Hub');
    await waitFor(() =>
      expect(section('Unlinked mentions')).toHaveTextContent('Unlinked mentions (0)'),
    );
    expect(section('Unlinked mentions')).toHaveTextContent('None');
    expect((section('Unlinked mentions') as HTMLDetailsElement).open).toBe(false);

    shell.links = {
      ...shell.links!,
      mentions: { notes: [], count: 0, more: 0, tooShort: true },
    };
    await waitFor(() =>
      expect(section('Unlinked mentions')).toHaveTextContent(
        "This note's title is shorter than 3 characters, so mentions are not searched.",
      ),
    );
    expect(section('Unlinked mentions').querySelector('summary')).toHaveTextContent(
      /^Unlinked mentions \(0\)$/,
    );
    expect((section('Unlinked mentions') as HTMLDetailsElement).open).toBe(false);

    // A note with mentions opens it again.
    await shell.openNote('Project-Plan');
    await waitFor(() =>
      expect((section('Unlinked mentions') as HTMLDetailsElement).open).toBe(true),
    );
  });

  it('A22, review 15 after "Link" the keyboard goes to the next "Link", then to the title', async () => {
    const shell = await workbench();
    await shell.openNote('Project-Plan');
    const first = await waitFor(() =>
      within(section('Unlinked mentions')).getByRole('button', {
        name: 'Link “project plan” in Diary, line 4, mention 1 of 2',
      }),
    );
    first.focus();
    await fireEvent.click(first);
    await waitFor(() =>
      expect(document.activeElement?.getAttribute('aria-label')).toBe(
        'Link “Project Plan” in Diary, line 4, mention 1 of 1',
      ),
    );
    await fireEvent.click(document.activeElement as HTMLElement);
    await waitFor(() =>
      expect(document.activeElement).toBe(section('Unlinked mentions').querySelector('summary')),
    );
  });

  it('review 16 the mentions of a note are one Tab stop; Up, Down, Home and End move', async () => {
    const shell = await workbench();
    await shell.openNote('Project-Plan');
    const list = await waitFor(() =>
      within(section('Unlinked mentions')).getByRole('list', { name: 'Mentions in Diary' }),
    );
    const entries = () => [
      ...list.querySelectorAll<HTMLButtonElement>('button[data-kind="mention"]'),
    ];
    expect(entries().map((entry) => entry.tabIndex)).toEqual([0, -1]);
    entries()[0].focus();

    await fireEvent.keyDown(entries()[0], { key: 'ArrowDown' });
    await waitFor(() => expect(document.activeElement).toBe(entries()[1]));
    expect(entries().map((entry) => entry.tabIndex)).toEqual([-1, 0]);
    await fireEvent.keyDown(entries()[1], { key: 'Home' });
    await waitFor(() => expect(document.activeElement).toBe(entries()[0]));
    // The "Link" buttons move along their column.
    const links = [...list.querySelectorAll<HTMLButtonElement>('button[data-kind="link"]')];
    links[0].focus();
    await fireEvent.keyDown(links[0], { key: 'End' });
    await waitFor(() => expect(document.activeElement).toBe(links[1]));
  });

  it('review 16 "Show more" comes before the list', async () => {
    const shell = await workbench();
    await shell.openNote('Project-Plan');
    await mockHooks().outsideChange('Diary', 'Project plan, '.repeat(250));
    const more = await waitFor(() =>
      within(section('Unlinked mentions')).getByRole('button', { name: /^Show more/ }),
    );
    const list = within(section('Unlinked mentions')).getByRole('list', {
      name: 'Unlinked mentions',
    });
    expect(more.compareDocumentPosition(list) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  });

  it('the mock finds mentions as the backend: whole words, case, not in code or links', () => {
    const texts: Record<string, string> = {
      'Road-map': '# Road map\n',
      Notes: 'road map, Road  Map, roadmap, `road map`, [road map](Road-map.md), #road map\n',
      Go: 'go go\n',
    };
    const view = mockMentions('Road-map', Object.keys(texts), (note) => texts[note]);
    expect(view.notes[0].mentions.map((mention) => mention.mention)).toEqual([
      'road map',
      'Road  Map',
    ]);
    expect(mockMentions('Go', Object.keys(texts), (note) => texts[note]).tooShort).toBe(true);
  });
});

describe('package 12 review: the mock stands in for the backend', () => {
  it('a picture held in the note is listed, never broken; existing files are not broken', () => {
    const texts: Record<string, string> = {
      Hub: '![x](data:image/png;base64,AAAA) [d](tools/deploy.sh) [f](file://tools/deploy.sh) [g](gone.sh)\n',
    };
    const view = mockLinks('Hub', 1, ['Hub'], (note) => texts[note], ['tools/deploy.sh']);
    expect(view.outgoing.map((link) => [link.kind, link.broken])).toEqual([
      ['inline', false],
      ['file', false],
      ['file', false],
      ['file', true],
    ]);
  });

  it('OQ-244 a rename rewrites includes and pictures too', async () => {
    const handlers = mockHandlers(START);
    const shell = await workbench(START, handlers);
    await shell.openNote('Hub');
    await handlers.note_rename({ note: 'Project-Plan', title: 'Road Map', folder: '' });
    expect(mockHooks().noteText('Guide')).toContain('include::Road-Map.md[]');
  });

  it('A22 the mock writes `:` and backticks as the backend does', () => {
    expect(mockLinkPlace('a/N.md', 'Tel:-Alice.md')).toBe('../Tel%3A-Alice.md');
    expect(mockLinkText('Plan`s [v2]', true)).toBe('Plan\\`s \\[v2\\]');
  });
});

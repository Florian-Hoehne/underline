// Package 3 (FR-080, UC-005 A23, A24, BR-018, TC-005-33, TC-005-34): link
// completion in both views, against the mock backend's `/home/user/Links`.
// Whether a link is typed and how it is written is the editor core's (in
// the WASM, tested in Rust); here the list, its keys and what lands in the
// text.

import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { clearMocks } from '@tauri-apps/api/mocks';
import { afterEach, describe, expect, it, vi } from 'vitest';
import App from '../App.svelte';
import { connectedShell } from '../test/shell';
import { activeSourceView } from './editor/active';
import { LinkCompletion } from './editor/linkCompletion.svelte';
import { loadEditorCore } from './editor/core';
import type { LinkChoice, LinkContextView, OverviewItem } from './bindings';
import { LINKS, mockHooks, type MockStart } from './mock-backend';

afterEach(() => {
  mockHooks().dispose();
  cleanup();
  clearMocks();
  vi.restoreAllMocks();
});

const SOURCE: MockStart = { givenDir: LINKS, view: 'source' };
const FACADE: MockStart = { givenDir: LINKS };

async function workbench(start: MockStart) {
  const shell = await connectedShell(start);
  render(App, { props: { shell } });
  await shell.startWorkspace();
  return shell;
}

/** The text field of the source view of `note`, once its text is in. */
async function sourceField(note: string): Promise<HTMLElement> {
  await waitFor(() => expect(activeSourceView()?.note).toBe(note));
  return document.querySelector<HTMLElement>('.cm-content')!;
}

/** The facade's text, once its first block is drawn. */
async function facadeField(): Promise<HTMLElement> {
  return waitFor(() => {
    const found = document.querySelector<HTMLElement>('.facade-text');
    expect(found?.querySelector('[data-b="0"]')).toBeTruthy();
    return found!;
  });
}

function type(field: HTMLElement, text: string) {
  field.dispatchEvent(
    new InputEvent('beforeinput', {
      inputType: 'insertText',
      data: text,
      bubbles: true,
      cancelable: true,
    }),
  );
}

const suggestions = () => screen.queryByRole('listbox', { name: /to link to/ });

async function list(): Promise<HTMLElement> {
  return waitFor(() => {
    const shown = suggestions();
    expect(shown).not.toBeNull();
    return shown!;
  });
}

const options = (shown: HTMLElement) =>
  within(shown)
    .getAllByRole('option')
    .map((option) => option.textContent?.replace(/\s+/g, ' ').trim());

/** The mirror's text of `note` once it holds `part`. */
async function holds(note: string, part: string): Promise<string> {
  return waitFor(() => {
    const text = mockHooks().mirrorText(note) ?? '';
    expect(text).toContain(part);
    return text;
  });
}

describe('FR-080 link completion in the source view (UC-005 A23)', () => {
  it('TC-005-33 steps 1 and 2: `[list](check` lists the note with its folder; Enter writes the link', async () => {
    const shell = await workbench(SOURCE);
    await shell.openNote('Diary');
    const field = await sourceField('Diary');

    type(field, '[list](check');

    const shown = await list();
    expect(options(shown)).toEqual(['Checklist travel']);
    expect(within(shown).getByRole('option')).toHaveAttribute('aria-selected', 'true');
    await fireEvent.keyDown(field, { key: 'Enter' });

    const text = await holds('Diary', '[list](travel/Checklist.md)');
    expect(text.startsWith('[list](travel/Checklist.md)')).toBe(true);
    await waitFor(() => expect(suggestions()).toBeNull());
    // The cursor is after the link.
    expect(activeSourceView()!.view.state.selection.main.head).toBe(
      '[list](travel/Checklist.md)'.length,
    );
  });

  it('TC-005-33 step 3: after `#` the headings of the note named; the link names the heading', async () => {
    const shell = await workbench(SOURCE);
    await shell.openNote('Diary');
    const field = await sourceField('Diary');

    type(field, '[](travel/Checklist.md#pa');

    const shown = await list();
    expect(shown).toHaveAccessibleName('Headings to link to');
    expect(options(shown)).toEqual(['Papers , level 2']);
    expect(screen.getByText('Headings of Checklist')).toBeTruthy();
    await fireEvent.keyDown(field, { key: 'Tab' });

    await holds('Diary', '[Papers](travel/Checklist.md#papers)');
  });

  it('the arrow keys move in the list, a click chooses', async () => {
    const shell = await workbench(SOURCE);
    await shell.openNote('Diary');
    const field = await sourceField('Diary');

    type(field, '[](');
    const shown = await list();
    const all = options(shown);
    expect(all.length).toBeGreaterThan(3);
    await fireEvent.keyDown(field, { key: 'ArrowDown' });
    await waitFor(() =>
      expect(within(shown).getAllByRole('option')[1]).toHaveAttribute('aria-selected', 'true'),
    );
    await fireEvent.keyDown(field, { key: 'ArrowUp' });
    await fireEvent.keyDown(field, { key: 'ArrowUp' });
    await waitFor(() =>
      expect(within(shown).getAllByRole('option').at(-1)).toHaveAttribute('aria-selected', 'true'),
    );

    await fireEvent.click(within(shown).getByRole('option', { name: /^Hub/ }));
    await holds('Diary', '[Hub](Hub.md)');
  });

  it('TC-005-34 step 2, A24: no note matches, no list; Escape closes it and the text stays', async () => {
    const shell = await workbench(SOURCE);
    await shell.openNote('Diary');
    const field = await sourceField('Diary');
    field.focus();

    type(field, '[x](zzzz');
    await holds('Diary', '[x](zzzz');
    await new Promise((resolve) => setTimeout(resolve, 30));
    expect(suggestions()).toBeNull();

    type(field, ' [y](pro');
    await list();
    await fireEvent.keyDown(field, { key: 'Escape' });
    await waitFor(() => expect(suggestions()).toBeNull());
    // Typing on in the same link keeps it closed; the text is as typed.
    type(field, 'j');
    await holds('Diary', '[x](zzzz [y](proj');
    await new Promise((resolve) => setTimeout(resolve, 30));
    expect(suggestions()).toBeNull();
    // Escape closed only the list: the keyboard stays in the text.
    expect(document.activeElement).toBe(field);
  });

  it('UC-005 A23: Ctrl+Space asks in the place of a written link', async () => {
    const shell = await workbench(SOURCE);
    await shell.openNote('Diary');
    const field = await sourceField('Diary');
    type(field, '[the hub](Hu)');
    await holds('Diary', '[the hub](Hu)');
    const view = activeSourceView()!;
    view.view.dispatch({ selection: { anchor: '[the hub](Hu'.length } });
    await waitFor(() => expect(suggestions()).toBeNull());

    await fireEvent.keyDown(field, { key: ' ', ctrlKey: true });
    const shown = await list();
    expect(options(shown)[0]).toBe('Hub');
    await fireEvent.keyDown(field, { key: 'Enter' });

    const text = await holds('Diary', '[the hub](Hub.md)');
    expect(text).not.toContain('Hu)');
  });
});

describe('FR-080 link completion: more of the source view', () => {
  it('TC-005-33 step 5: `xref:gui` in an AsciiDoc note writes an AsciiDoc link', async () => {
    const shell = await workbench(SOURCE);
    await shell.openNote('Guide');
    const field = await sourceField('Guide');

    type(field, 'xref:pro');
    const shown = await list();
    expect(options(shown)[0]).toBe('Project Plan');
    await fireEvent.keyDown(field, { key: 'Enter' });

    await holds('Guide', 'xref:Project-Plan.md[Project Plan]');
  });

  it('the list is named by the text for screen readers while it shows; "No suggestions" is said', async () => {
    const shell = await workbench(SOURCE);
    await shell.openNote('Diary');
    const field = await sourceField('Diary');
    expect(field).toHaveAttribute('aria-haspopup', 'listbox');
    expect(field).not.toHaveAttribute('aria-controls');

    type(field, '[](hub');
    const shown = await list();
    await waitFor(() => expect(field).toHaveAttribute('aria-controls', shown.id));
    const chosen = within(shown).getByRole('option', { selected: true });
    expect(field).toHaveAttribute('aria-activedescendant', chosen.id);
    expect(screen.getByText('1 suggestion')).toBeTruthy();

    type(field, 'zzz');
    await waitFor(() => expect(suggestions()).toBeNull());
    expect(screen.getByText('No suggestions')).toBeTruthy();
    await waitFor(() => expect(field).not.toHaveAttribute('aria-controls'));
  });

  it('OQ-251: the place of a note matches too; Ctrl+Space opens the list again after Escape', async () => {
    const shell = await workbench(SOURCE);
    await shell.openNote('Diary');
    const field = await sourceField('Diary');
    field.focus();

    type(field, '[](travel/Check');
    const shown = await list();
    expect(options(shown)).toEqual(['Checklist travel']);
    await fireEvent.keyDown(field, { key: 'Escape' });
    await waitFor(() => expect(suggestions()).toBeNull());

    await fireEvent.keyDown(field, { key: ' ', ctrlKey: true });
    await list();
  });

  it('UC-023: "Complete link" of the command palette opens the list at the cursor', async () => {
    const shell = await workbench(FACADE);
    await shell.openNote('Diary');
    await facadeField();
    await holds('Diary', '# Diary');

    const command = shell.commandById('note.completeLink')!;
    expect(command.unavailable()).toBeNull();
    command.run();

    await list();
    expect(shell.commandById('note.followLink')).toBeTruthy();
  });
});

describe('FR-080 link completion in the facade (UC-005 A23, BR-018)', () => {
  it('TC-005-33 step 4: `[[proj` and Enter write a normal link; `[[` is not in the note', async () => {
    const shell = await workbench(FACADE);
    await shell.openNote('Diary');
    const field = await facadeField();
    await holds('Diary', '# Diary');

    type(field, '[[proj');
    const shown = await list();
    expect(options(shown)[0]).toBe('Project Plan');
    await fireEvent.keyDown(field, { key: 'Enter' });

    const text = await holds('Diary', '[Project Plan](Project-Plan.md)');
    expect(text).not.toContain('[[');
    expect(text).not.toContain('\\[');
  });

  it('TC-005-34 step 1, A24: `[[` and Escape keep `[[` as typed text', async () => {
    const shell = await workbench(FACADE);
    await shell.openNote('Diary');
    const field = await facadeField();
    await holds('Diary', '# Diary');

    type(field, '[[');
    await list();
    await fireEvent.keyDown(field, { key: 'Escape' });

    await waitFor(() => expect(suggestions()).toBeNull());
    await holds('Diary', '\\[\\[');
  });
});

describe('the list on its own (LinkCompletion)', () => {
  const overview: OverviewItem[] = [
    { note: 'Plan', label: 'Plan', title: 'Plan', folder: null, format: 'Markdown', mark: null },
    {
      note: 'travel/Packing',
      label: 'Packing',
      title: 'Packing',
      folder: 'travel',
      format: 'AsciiDoc',
      mark: null,
    },
  ];
  const context = (typed: string, heading: string | null = null): LinkContextView => ({
    rev: 3,
    from: 0,
    typed,
    heading,
    named: null,
  });

  it('NFR-018: ranks as go to note does, the folder matches too; a choice names the format', async () => {
    await loadEditorCore();
    const completion = new LinkCompletion();
    completion.attach({ overview: () => overview, headings: async () => [] });
    const chosen: LinkChoice[] = [];
    const target = { note: 'Ideas', complete: (choice: LinkChoice) => chosen.push(choice) };

    completion.offer(target, context('travel'), null);
    expect(completion.list.entries.map((entry) => entry.kind === 'note' && entry.title)).toEqual([
      'Packing',
    ]);
    completion.choose();

    expect(chosen).toEqual([
      { rev: 3, note: 'travel/Packing', format: 'AsciiDoc', anchor: null, label: 'Packing' },
    ]);
    expect(completion.context).toBeNull();
  });

  it('a `#` without a note lists the headings of the note itself', async () => {
    await loadEditorCore();
    const completion = new LinkCompletion();
    const headings = vi.fn(async () => [{ label: 'Goals', anchor: 'goals', level: 2 }]);
    completion.attach({ overview: () => overview, headings });
    const chosen: LinkChoice[] = [];
    const target = { note: 'Ideas', complete: (choice: LinkChoice) => chosen.push(choice) };

    completion.offer(target, context('', 'go'), null);
    expect(completion.list.loading).toBe(true);
    await waitFor(() => expect(completion.list.entries).toHaveLength(1));
    expect(headings).toHaveBeenCalledWith('Ideas');
    expect(completion.list.headingsOf).toEqual({ title: null });
    completion.choose();

    expect(chosen).toEqual([{ rev: 3, note: null, format: null, anchor: 'goals', label: 'Goals' }]);
  });
});

describe('NFR-018 link completion within 200 ms in 10,000 notes', () => {
  it('TC-005-34 step 3: the list follows 20 link places within 200 ms each', async () => {
    await loadEditorCore();
    const overview: OverviewItem[] = Array.from({ length: 10_000 }, (_, n) => {
      const folder = `area-${n % 13}`;
      const title = `Meeting ${String(n).padStart(5, '0')} project ${n % 97}`;
      return {
        note: `${folder}/${title.replace(/ /g, '-')}`,
        label: title,
        title,
        folder,
        format: 'Markdown',
        mark: null,
      };
    });
    const completion = new LinkCompletion();
    completion.attach({ overview: () => overview, headings: async () => [] });
    const target = { note: 'Home', complete: () => {} };
    const texts = [
      'm',
      'me',
      'mee',
      'meet',
      'meeting',
      'meeting-0012',
      'project',
      'area-3',
      'area-12/Meet',
      'xyz',
      '9999',
      'project-96',
      'MEETING',
      'proj',
      'g-01',
      '',
      'a',
      't',
      'ing-0',
      '07',
    ];
    const times: number[] = [];
    for (const typed of texts) {
      const started = performance.now();
      completion.offer(
        target,
        { rev: 1, from: 0, typed, heading: null, named: null },
        { left: 0, top: 0, bottom: 0 },
      );
      const shown = completion.list.entries.length;
      times.push(performance.now() - started);
      expect(shown).toBeLessThanOrEqual(50);
    }
    console.info(
      `NFR-018 link completion, 10,000 notes, ms per key: ${times.map((t) => t.toFixed(1)).join(' ')}`,
    );
    expect(Math.max(...times)).toBeLessThan(200);
  });
});

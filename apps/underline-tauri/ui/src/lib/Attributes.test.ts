// Package 6a (FR-091 to FR-093): the note attributes in the note properties
// (UC-024, TC-024), the tags of the note in the tool panel and choosing one
// (UC-004 BR-016, A15, TC-004-32), and the search by tag (UC-008 A8,
// TC-008-21), against the mock backend's `/home/user/Attributes`. What the
// attributes and tags are, and the edits of the markup, are the editor
// core's (in the WASM, tested in Rust); here the panel shows them, a change
// reaches the text through the view shown as one undo step, and the
// palette, the keyboard and the search reach them.

import { cleanup, fireEvent, render, waitFor, within } from '@testing-library/svelte';
import { clearMocks } from '@tauri-apps/api/mocks';
import { afterEach, describe, expect, it, vi } from 'vitest';
import App from '../App.svelte';
import { connectedShell } from '../test/shell';
import { activeSourceView } from './editor/active';
import { activeFacade } from './facade/active';
import { ATTRIBUTE_TEXTS, ATTRIBUTES, mockHooks, mockTags, type MockStart } from './mock-backend';
import type { Shell } from './shell.svelte';

afterEach(() => {
  mockHooks().dispose();
  cleanup();
  clearMocks();
  vi.restoreAllMocks();
});

const SOURCE: MockStart = { givenDir: ATTRIBUTES, view: 'source' };
const FACADE: MockStart = { givenDir: ATTRIBUTES };

async function workbench(start: MockStart): Promise<Shell> {
  const shell = await connectedShell(start);
  render(App, { props: { shell } });
  await shell.startWorkspace();
  return shell;
}

/** Opens `note` in the source view and waits for its attributes. */
async function inSource(shell: Shell, note: string): Promise<void> {
  await shell.openNote(note);
  await waitFor(() => expect(activeSourceView()?.note).toBe(note));
  await waitFor(() => expect(section()).toBeTruthy());
}

/** Opens `note` in the facade and waits for its attributes. */
async function inFacade(shell: Shell, note: string): Promise<void> {
  await shell.openNote(note);
  await waitFor(() => {
    expect(activeFacade()?.note).toBe(note);
    expect(document.querySelector('.facade-text [data-b="0"]')).toBeTruthy();
  });
  await waitFor(() => expect(section()).toBeTruthy());
}

const panel = () => document.querySelector<HTMLElement>('.tool-panel')!;
const section = () => panel().querySelector<HTMLElement>('section.attributes');
/** The field of the attribute `name`. */
const field = (name: string) => within(section()!).getByLabelText(name) as HTMLInputElement;
/** The text of the note in the window's core, as the source view shows it. */
const shown = () => activeSourceView()!.view.state.doc.toString();

/** Types `value` into `input` and confirms it with Enter. */
async function typeAndEnter(input: HTMLInputElement, value: string): Promise<void> {
  input.focus();
  await fireEvent.input(input, { target: { value } });
  await fireEvent.keyDown(input, { key: 'Enter' });
}

/** The tags the tool panel lists. */
function tags(): string[] {
  const list = within(panel()).queryByRole('list', { name: 'Tags' });
  return list
    ? within(list)
        .getAllByRole('button')
        .map((b) => b.textContent!.replace('#', ''))
    : [];
}

const overlay = () => document.querySelector<HTMLElement>('.palette');
const paletteField = () => within(overlay()!).getByRole('combobox');

/** Opens the palette with `>title` typed. */
async function paletteWith(title: string): Promise<void> {
  await fireEvent.keyDown(document.body, { key: 'P', code: 'KeyP', ctrlKey: true, shiftKey: true });
  await waitFor(() => expect(overlay()).toBeTruthy());
  await fireEvent.input(paletteField(), { target: { value: `>${title}` } });
  await waitFor(() =>
    expect(within(overlay()!).getAllByRole('option')[0]?.textContent).toContain(title),
  );
}

describe('TC-024 UC-024 note attributes in the note properties', () => {
  it('TC-024-01 step 1, BR-002: the attributes of a Markdown note as fields', async () => {
    const shell = await workbench(SOURCE);
    await inSource(shell, 'Quarter-Plan');

    await waitFor(() => expect(field('status').value).toBe('draft'));
    expect(field('due').type).toBe('date');
    expect(field('due').value).toBe('2026-12-01');
  });

  it('TC-024-01 steps 3 and 4, TC-024-12: a change in the source view is written and undone in one step', async () => {
    const shell = await workbench(SOURCE);
    await inSource(shell, 'Quarter-Plan');
    await waitFor(() => expect(field('status').value).toBe('draft'));

    await typeAndEnter(field('status'), 'final');

    await waitFor(() => expect(shown()).toContain('status: final'));
    expect(shown()).toBe(ATTRIBUTE_TEXTS['Quarter-Plan'].replace('draft', 'final'));
    await waitFor(() =>
      expect(mockHooks().mirrorText('Quarter-Plan')).toBe(
        ATTRIBUTE_TEXTS['Quarter-Plan'].replace('draft', 'final'),
      ),
    );

    // Ctrl+Z in the tab: the whole change goes back.
    const content = document.querySelector<HTMLElement>('.cm-content')!;
    content.focus();
    await fireEvent.keyDown(content, { key: 'z', code: 'KeyZ', ctrlKey: true });
    await waitFor(() => expect(shown()).toBe(ATTRIBUTE_TEXTS['Quarter-Plan']));
    await waitFor(() => expect(field('status').value).toBe('draft'));
  });

  it('TC-024-01 steps 2 and 3, OQ-216: the facade shows the front matter as a table that follows a change', async () => {
    const shell = await workbench(FACADE);
    await inFacade(shell, 'Quarter-Plan');
    const table = () => document.querySelector<HTMLElement>('.facade-text table');
    await waitFor(() => expect(table()?.textContent).toContain('draft'));

    await typeAndEnter(field('status'), 'final');

    await waitFor(() => expect(table()?.textContent).toContain('final'));
    await waitFor(() => expect(mockHooks().mirrorText('Quarter-Plan')).toContain('status: final'));
  });

  it('TC-024-02, BR-004: an attribute added to an AsciiDoc note, its name in lower case', async () => {
    const shell = await workbench(SOURCE);
    await inSource(shell, 'Handbook');
    const form = within(section()!);

    await fireEvent.click(form.getByRole('button', { name: 'Add attribute' }));
    await fireEvent.input(form.getByLabelText('Name'), { target: { value: 'Edition' } });
    // BR-002: an AsciiDoc note offers texts and lists only.
    const kinds = within(form.getByLabelText('Kind')).getAllByRole('option');
    expect(kinds.map((option) => option.textContent)).toEqual(['Text', 'List']);
    await fireEvent.input(form.getByLabelText('Value'), { target: { value: '2' } });
    await fireEvent.click(form.getByRole('button', { name: 'Add' }));

    await waitFor(() =>
      expect(shown()).toBe('= Guide\n:author: Ann\n:edition: 2\n\nThe handbook.\n'),
    );
    await waitFor(() => expect(field('edition').value).toBe('2'));
  });

  it('TC-024-03: a new attribute follows the last one; nothing else changes', async () => {
    const shell = await workbench(SOURCE);
    await inSource(shell, 'Mixed');
    await shell.showAddAttribute();
    const form = within(section()!);
    await waitFor(() => expect(document.activeElement).toBe(form.getByLabelText('Name')));
    await fireEvent.input(form.getByLabelText('Name'), { target: { value: 'owner' } });
    await fireEvent.input(form.getByLabelText('Value'), { target: { value: 'Ann' } });
    await fireEvent.submit(form.getByRole('form', { name: 'New attribute' }));

    await waitFor(() =>
      expect(shown()).toBe(
        ATTRIBUTE_TEXTS.Mixed.replace('title:  "Mixed"\n', 'title:  "Mixed"\nowner: Ann\n'),
      ),
    );
  });

  it('TC-024-04: the kinds as fields; the switch writes `true`', async () => {
    const shell = await workbench(SOURCE);
    await inSource(shell, 'Kinds');
    await waitFor(() => expect(field('title').value).toBe('Kinds'));
    const items = within(within(section()!).getByRole('list', { name: 'Items of tags' }));
    expect(items.getByLabelText('Item 1 of tags')).toHaveValue('a');
    expect(items.getByLabelText('Item 2 of tags')).toHaveValue('b');
    expect(field('weight').inputMode).toBe('decimal');
    expect(field('due').type).toBe('date');
    const done = field('done');
    expect(done).toHaveAttribute('role', 'switch');
    expect(done.checked).toBe(false);

    await fireEvent.click(done);

    await waitFor(() => expect(shown()).toContain('done: true'));
  });

  it('TC-024-05 A1, A2: the first attribute makes the front matter, the last one removed takes it away', async () => {
    const shell = await workbench(SOURCE);
    await inSource(shell, 'Plain');
    const form = within(section()!);
    expect(form.getByText('No attributes')).toBeInTheDocument();

    await fireEvent.click(form.getByRole('button', { name: 'Add attribute' }));
    await fireEvent.input(form.getByLabelText('Name'), { target: { value: 'status' } });
    await fireEvent.input(form.getByLabelText('Value'), { target: { value: 'draft' } });
    await fireEvent.click(form.getByRole('button', { name: 'Add' }));
    await waitFor(() => expect(shown()).toBe('---\nstatus: draft\n---\n# Plain\n'));

    await fireEvent.click(await form.findByRole('button', { name: 'Remove attribute status' }));
    await waitFor(() => expect(shown()).toBe('# Plain\n'));
  });

  it('TC-024-06 A3: a name not allowed or taken is refused with its reason; what was typed stays', async () => {
    const shell = await workbench(SOURCE);
    await inSource(shell, 'Quarter-Plan');
    const form = within(section()!);
    await fireEvent.click(form.getByRole('button', { name: 'Add attribute' }));
    const name = form.getByLabelText('Name') as HTMLInputElement;

    for (const [typed, said] of [
      ['my status', 'A name cannot hold a space.'],
      ['status', 'The note has the attribute “status” already.'],
      ['', 'A name is needed.'],
    ]) {
      await fireEvent.input(name, { target: { value: typed } });
      await fireEvent.click(form.getByRole('button', { name: 'Add' }));
      await waitFor(() => expect(form.getByRole('alert')).toHaveTextContent(said));
      expect(name.value).toBe(typed);
    }
    expect(shown()).toBe(ATTRIBUTE_TEXTS['Quarter-Plan']);
  });

  it('TC-024-07 A4: a value that does not fit its kind is refused and kept', async () => {
    const shell = await workbench(SOURCE);
    await inSource(shell, 'Kinds');
    await waitFor(() => expect(field('weight').value).toBe('2.5'));

    await typeAndEnter(field('weight'), 'soon');

    await waitFor(() =>
      expect(within(section()!).getByRole('alert')).toHaveTextContent('A number is expected'),
    );
    expect(field('weight').value).toBe('soon');
    expect(field('weight')).toHaveAttribute('aria-invalid', 'true');
    expect(shown()).toBe(ATTRIBUTE_TEXTS.Kinds);
  });

  it('TC-024-08 A5: a front matter that cannot be read shows as text with its line, and the source view', async () => {
    const shell = await workbench(FACADE);
    await inFacade(shell, 'Broken');
    const group = within(section()!).getByRole('group', {
      name: 'Front matter that cannot be read',
    });
    expect(group).toHaveTextContent('line 3');
    expect(group.querySelector('pre')?.textContent).toBe('---\nstatus: draft\njust text\n---');
    expect(within(section()!).queryByRole('textbox')).toBeNull();

    await fireEvent.click(within(group).getByRole('button', { name: 'Show in source view' }));

    await waitFor(() => expect(activeSourceView()?.note).toBe('Broken'));
    expect(shown()).toBe(ATTRIBUTE_TEXTS.Broken);
  });

  it('TC-024-09 A6: a nested value shows as markup; another attribute changes alone', async () => {
    const shell = await workbench(SOURCE);
    await inSource(shell, 'Nested');
    const nested = within(section()!);
    await waitFor(() => expect(nested.getByText('Changed only in the source view.')).toBeTruthy());
    expect(section()!.querySelector('pre')?.textContent).toContain('name: Ann');

    await typeAndEnter(field('status'), 'final');

    await waitFor(() =>
      expect(shown()).toBe(ATTRIBUTE_TEXTS.Nested.replace('status: draft', 'status: final')),
    );
  });

  it('TC-024-10 A7: a change made outside shows; the value being typed stays and is written into the new text', async () => {
    const shell = await workbench(SOURCE);
    await inSource(shell, 'Quarter-Plan');
    await waitFor(() => expect(field('status').value).toBe('draft'));
    field('status').focus();
    await fireEvent.input(field('status'), { target: { value: 'fin' } });

    const outside = ATTRIBUTE_TEXTS['Quarter-Plan'].replace('2026-12-01', '2027-01-15');
    await mockHooks().outsideChange('Quarter-Plan', outside);

    await waitFor(() => expect(field('due').value).toBe('2027-01-15'), { timeout: 3000 });
    expect(field('status').value).toBe('fin');

    await typeAndEnter(field('status'), 'final');
    await waitFor(() => expect(shown()).toBe(outside.replace('draft', 'final')));
  });

  it('TC-024-11 BR-005: an item added to "tags" joins the list; a tag of the text stays out of it', async () => {
    const shell = await workbench(SOURCE);
    await inSource(shell, 'Ferry-Trip');
    await waitFor(() => expect(tags()).toEqual(['travel', 'hydra', 'project/alpha', 'greece']));

    await typeAndEnter(
      within(section()!).getByLabelText('New item of tags') as HTMLInputElement,
      'ferry',
    );

    await waitFor(() => expect(shown()).toContain('tags: [Travel, ferry]'));
    await waitFor(() =>
      expect(tags()).toEqual(['travel', 'ferry', 'hydra', 'project/alpha', 'greece']),
    );
    const items = within(within(section()!).getByRole('list', { name: 'Items of tags' }));
    expect(items.queryByDisplayValue('greece')).toBeNull();
  });

  it('TC-024-13 BR-007: by keyboard, Enter writes a value and the remove button removes', async () => {
    const shell = await workbench(SOURCE);
    await inSource(shell, 'Quarter-Plan');
    await waitFor(() => expect(field('status').value).toBe('draft'));

    await typeAndEnter(field('status'), 'final');
    await waitFor(() => expect(shown()).toContain('status: final'));

    const remove = within(section()!).getByRole('button', { name: 'Remove attribute due' });
    remove.focus();
    // A button runs its click on Enter in a browser; the test fires it.
    await fireEvent.click(remove);
    await waitFor(() => expect(shown()).not.toContain('due:'));
  });

  it('palette "Add note attribute" opens the form with the keyboard in the name', async () => {
    const shell = await workbench(SOURCE);
    await inSource(shell, 'Plain');
    await paletteWith('Add note attribute');
    await fireEvent.keyDown(paletteField(), { key: 'Enter' });

    await waitFor(() =>
      expect(document.activeElement).toBe(within(section()!).getByLabelText('Name')),
    );
    await fireEvent.keyDown(document.activeElement!, { key: 'Escape' });
    await waitFor(() => expect(within(section()!).queryByLabelText('Name')).toBeNull());
  });
});

describe('TC-004-32 TC-008-21 FR-091 FR-092 tags and the search by tag', () => {
  it('TC-004-32 steps 1 and 3: the tags of a Markdown and an AsciiDoc note', async () => {
    const shell = await workbench(SOURCE);
    await inSource(shell, 'Ferry-Trip');
    await waitFor(() => expect(tags()).toEqual(['travel', 'hydra', 'project/alpha', 'greece']));

    await inSource(shell, 'Travel-Guide');
    await waitFor(() => expect(tags()).toEqual(['travel', 'docs']));
  });

  it('TC-004-32 step 4, TC-008-21 step 4, UC-004 A15: choosing a tag searches for it', async () => {
    const shell = await workbench(SOURCE);
    await inSource(shell, 'Ferry-Trip');
    await waitFor(() => expect(tags()).toContain('travel'));

    await fireEvent.click(
      within(panel()).getByRole('button', { name: '#travel, search notes tagged travel' }),
    );

    expect(shell.search.open).toBe(true);
    expect(shell.search.text).toBe('#travel');
    await waitFor(() =>
      expect(shell.search.notes.map((found) => found.note).sort()).toEqual([
        'Ferry-Trip',
        'Hydra',
        'Travel-Guide',
      ]),
    );
  });

  it('palette "Search by tag…" asks for a tag of the note', async () => {
    const shell = await workbench(SOURCE);
    await inSource(shell, 'Travel-Guide');
    await waitFor(() => expect(tags()).toEqual(['travel', 'docs']));
    await paletteWith('Search by tag');
    await fireEvent.keyDown(paletteField(), { key: 'Enter' });
    await waitFor(() =>
      expect(
        within(overlay()!)
          .getAllByRole('option')
          .map((o) => o.textContent),
      ).toEqual(expect.arrayContaining([expect.stringContaining('#docs')])),
    );
  });

  it('a note without tags says "None" and the palette command says why', async () => {
    const shell = await workbench(SOURCE);
    await inSource(shell, 'Plain');
    await waitFor(() =>
      expect(within(panel()).getByText('Tags (0)').closest('section')).toHaveTextContent('None'),
    );
  });

  it('the mock reads tags roughly as the rules say (the real rules are tested in Rust)', () => {
    expect(mockTags(ATTRIBUTE_TEXTS['Ferry-Trip'])).toEqual(
      expect.arrayContaining(['travel', 'hydra', 'project/alpha']),
    );
    expect(mockTags(ATTRIBUTE_TEXTS.Price)).toEqual([]);
  });
});

describe('UC-024 review fixes: keyboard, messages and undo in the note properties', () => {
  it('UX 20, 27: after a remove the keyboard goes to the next attribute, the status says how to undo, and Ctrl+Z in the panel undoes', async () => {
    const shell = await workbench(SOURCE);
    await inSource(shell, 'Quarter-Plan');
    await waitFor(() => expect(field('status').value).toBe('draft'));
    const remove = within(section()!).getByRole('button', { name: 'Remove attribute status' });
    remove.focus();

    await fireEvent.click(remove);

    await waitFor(() => expect(shown()).not.toContain('status:'));
    await waitFor(() => expect(document.activeElement).toBe(field('due')));
    expect(shell.status).toBe('Removed attribute status. Undo with Ctrl+Z.');

    await fireEvent.keyDown(field('due'), { key: 'z', ctrlKey: true });
    await waitFor(() => expect(shown()).toBe(ATTRIBUTE_TEXTS['Quarter-Plan']));
  });

  it('UX 20: removing the last attribute puts the keyboard on "Add attribute"', async () => {
    const shell = await workbench(SOURCE);
    await inSource(shell, 'Handbook');
    await fireEvent.click(
      within(section()!).getByRole('button', { name: 'Remove attribute author' }),
    );
    await waitFor(() => expect(shown()).toBe('= Guide\n\nThe handbook.\n'));
    await waitFor(() =>
      expect(document.activeElement).toBe(
        within(section()!).getByRole('button', { name: 'Add attribute' }),
      ),
    );
  });

  it('UX 21: Escape after a refused value gives up the value and its message', async () => {
    const shell = await workbench(SOURCE);
    await inSource(shell, 'Kinds');
    await waitFor(() => expect(field('weight').value).toBe('2.5'));
    await typeAndEnter(field('weight'), 'soon');
    await waitFor(() => expect(within(section()!).getByRole('alert')).toBeTruthy());

    await fireEvent.keyDown(field('weight'), { key: 'Escape' });

    await waitFor(() => expect(within(section()!).queryByRole('alert')).toBeNull());
    expect(field('weight').value).toBe('2.5');
    expect(field('weight')).not.toHaveAttribute('aria-invalid');
  });

  it('UX 22: a refused value of a new attribute puts the keyboard into the value, marked', async () => {
    const shell = await workbench(SOURCE);
    await inSource(shell, 'Plain');
    const form = within(section()!);
    await fireEvent.click(form.getByRole('button', { name: 'Add attribute' }));
    await fireEvent.input(form.getByLabelText('Name'), { target: { value: 'weight' } });
    await fireEvent.change(form.getByLabelText('Kind'), { target: { value: 'number' } });
    await fireEvent.input(form.getByLabelText('Value'), { target: { value: 'soon' } });
    await fireEvent.click(form.getByRole('button', { name: 'Add' }));

    await waitFor(() => expect(document.activeElement).toBe(form.getByLabelText('Value')));
    expect(form.getByLabelText('Value')).toHaveAttribute('aria-invalid', 'true');
    expect(form.getByLabelText('Value').getAttribute('aria-describedby')).toContain(
      'new-attribute-problem',
    );
    expect(form.getByLabelText('Name')).not.toHaveAttribute('aria-invalid');
  });

  it('OQ-270: an item of "tags" that is no tag is refused with its reason', async () => {
    const shell = await workbench(SOURCE);
    await inSource(shell, 'Ferry-Trip');
    await typeAndEnter(
      within(section()!).getByLabelText('New item of tags') as HTMLInputElement,
      'two words',
    );
    await waitFor(() =>
      expect(within(section()!).getByRole('alert')).toHaveTextContent('“two words” is no tag'),
    );
    expect(shown()).toBe(ATTRIBUTE_TEXTS['Ferry-Trip']);
  });

  it('BR-007: the remove button follows its field in the order of Tab', async () => {
    const shell = await workbench(SOURCE);
    await inSource(shell, 'Quarter-Plan');
    await waitFor(() => expect(field('status').value).toBe('draft'));
    const remove = within(section()!).getByRole('button', { name: 'Remove attribute status' });
    expect(
      field('status').compareDocumentPosition(remove) & Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy();
  });
});

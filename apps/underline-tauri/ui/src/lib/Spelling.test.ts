// Package 8b (FR-099, FR-100): the spell check in both editing views
// (UC-005 BR-023, A28; TC-005-39, TC-005-40), against the mock backend's
// `/home/user/Spell`, whose checker knows English (US) only (mock-spell.ts).
// The checker, its skip rules and the user's words file are tested in Rust
// (`underline-spell`, `session/spell/tests.rs`).

import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { clearMocks } from '@tauri-apps/api/mocks';
import { afterEach, describe, expect, it, vi } from 'vitest';
import App from '../App.svelte';
import { connectedShell } from '../test/shell';
import { activeSourceView } from './editor/active';
import { activeFacade } from './facade/active';
import { mockHooks, type MockStart } from './mock-backend';
import { SPELL, SPELL_TEXTS } from './mock-spell';
import type { Shell } from './shell.svelte';

afterEach(() => {
  mockHooks().dispose();
  cleanup();
  clearMocks();
  vi.restoreAllMocks();
});

const FACADE: MockStart = { givenDir: SPELL };
const SOURCE: MockStart = { givenDir: SPELL, view: 'source' };

async function workbench(start: MockStart): Promise<Shell> {
  const shell = await connectedShell(start);
  render(App, { props: { shell } });
  await shell.startWorkspace();
  return shell;
}

/** The facade's text of `note`, once its first block is drawn. */
async function facadeField(shell: Shell, note: string): Promise<HTMLElement> {
  await shell.openNote(note);
  return waitFor(() => {
    const found = document.querySelector<HTMLElement>('.facade-text');
    expect(activeFacade()?.note).toBe(note);
    expect(found?.querySelector('[data-b="0"]')).toBeTruthy();
    return found!;
  });
}

/** The text field of the source view of `note`, once its text is in, with the keyboard. */
async function sourceField(shell: Shell, note: string): Promise<HTMLElement> {
  await shell.openNote(note);
  await waitFor(() => expect(activeSourceView()?.note).toBe(note));
  const field = document.querySelector<HTMLElement>('.cm-content')!;
  field.focus();
  return field;
}

async function becomes(note: string, text: string): Promise<void> {
  await waitFor(() => expect(mockHooks().mirrorText(note)).toBe(text));
}

/** The words marked in `field`, as shown. */
function marked(field: HTMLElement): string[] {
  return Array.from(field.querySelectorAll('[data-spell]')).map((mark) => mark.textContent ?? '');
}

const menu = () => screen.getByRole('menu', { name: /^Spelling of / });

describe('TC-005-39 a word is corrected and added to the dictionary (UC-005 A28)', () => {
  it('steps 1 to 3 in the facade: "ferri" is marked; the menu opens with "Add to dictionary", "ferry" fills in; Ctrl+Z brings "ferri" back', async () => {
    const shell = await workbench(FACADE);
    const field = await facadeField(shell, 'Spell');
    await waitFor(() => expect(marked(field)).toEqual(['ferri']));

    activeFacade()!.jump(SPELL_TEXTS.Spell.indexOf('ferri') + 2);
    await activeFacade()!.mark();
    await fireEvent.keyDown(field, { key: 'F10', shiftKey: true });

    const opened = await waitFor(menu);
    expect(opened).toHaveAccessibleName('Spelling of ferri, misspelled');
    // OQ-302: the corrections came within 100 ms: the first has the
    // keyboard, "Add …" stands last.
    const ferry = within(opened).getByRole('menuitem', { name: 'ferry' });
    await waitFor(() => expect(ferry).toHaveFocus());
    const items = within(opened).getAllByRole('menuitem');
    expect(items.at(-1)).toHaveAccessibleName('Add "ferri" to dictionary');
    expect(within(opened).getByText('ferri')).toBeInTheDocument();
    expect(within(opened).queryByText('Looking for corrections…')).toBeNull();

    await fireEvent.click(ferry);
    await becomes('Spell', SPELL_TEXTS.Spell.replace('ferri', 'ferry'));
    expect(screen.queryByRole('menu')).toBeNull();
    await waitFor(() => expect(marked(field)).toEqual([]));
    await waitFor(() => expect(shell.spellAnnouncement).toBe('Corrected to ferry'));

    await fireEvent.keyDown(field, { key: 'z', code: 'KeyZ', ctrlKey: true });
    await becomes('Spell', SPELL_TEXTS.Spell);
  });

  it('steps 2 and 3 in the source view: the menu key opens the menu on the word at the cursor', async () => {
    const shell = await workbench(SOURCE);
    const field = await sourceField(shell, 'Spell');
    await waitFor(() => expect(marked(field)).toEqual(['ferri']));
    const view = activeSourceView()!.view;
    view.dispatch({ selection: { anchor: SPELL_TEXTS.Spell.indexOf('ferri') + 1 } });

    await fireEvent.keyDown(field, { key: 'ContextMenu' });
    const opened = await waitFor(menu);
    await fireEvent.click(
      await waitFor(() => within(opened).getByRole('menuitem', { name: 'ferry' })),
    );
    await becomes('Spell', SPELL_TEXTS.Spell.replace('ferri', 'ferry'));

    await fireEvent.keyDown(field, { key: 'z', code: 'KeyZ', ctrlKey: true });
    await becomes('Spell', SPELL_TEXTS.Spell);
  });

  it('Escape closes the menu and gives the keyboard back; no menu off a marked word', async () => {
    const shell = await workbench(SOURCE);
    const field = await sourceField(shell, 'Spell');
    await waitFor(() => expect(marked(field)).toEqual(['ferri']));
    const view = activeSourceView()!.view;

    view.dispatch({ selection: { anchor: 0 } });
    await fireEvent.keyDown(field, { key: 'F10', shiftKey: true });
    expect(screen.queryByRole('menu')).toBeNull();

    view.dispatch({ selection: { anchor: SPELL_TEXTS.Spell.indexOf('ferri') } });
    await fireEvent.keyDown(field, { key: 'F10', shiftKey: true });
    const opened = await waitFor(menu);
    await fireEvent.keyDown(opened, { key: 'Escape' });
    expect(screen.queryByRole('menu')).toBeNull();
    await waitFor(() => expect(field).toHaveFocus());
  });

  it('step 4: "Add to dictionary" removes both marks of "Kubernetes"', async () => {
    const shell = await workbench(SOURCE);
    const field = await sourceField(shell, 'Other');
    await waitFor(() => expect(marked(field)).toEqual(['Kubernetes', 'Kubernetes']));
    activeSourceView()!.view.dispatch({
      selection: { anchor: SPELL_TEXTS.Other.indexOf('Kubernetes') + 3 },
    });

    await fireEvent.keyDown(field, { key: 'F10', shiftKey: true });
    const opened = await waitFor(menu);
    // No word of the dictionary is one edit away: it says so.
    expect(within(opened).getByText('No corrections found')).toBeInTheDocument();
    const add = within(opened).getByRole('menuitem', { name: 'Add "Kubernetes" to dictionary' });
    await waitFor(() => expect(add).toHaveFocus());
    await fireEvent.click(add);

    await waitFor(() => expect(marked(field)).toEqual([]));
    await waitFor(() => expect(shell.status).toBe('Added "Kubernetes" to the dictionary'));
  });
});

describe('TC-005-40 what the spell check leaves out (BR-023)', () => {
  it('step 1: code, the address, the tag, HTTPX, abc123, the formula and the name are not marked', async () => {
    const shell = await workbench(SOURCE);
    const field = await sourceField(shell, 'Mixed-Spell');
    await waitFor(() => expect(marked(field)).toEqual(['offen', 'Strasse', 'fehlr']));
  });

  it('step 3: with the language off no word is marked', async () => {
    const shell = await workbench(FACADE);
    const field = await facadeField(shell, 'Spell');
    await waitFor(() => expect(marked(field)).toEqual(['ferri']));

    await shell.quickSetting('spell_check_language', 'off');
    await waitFor(() => expect(marked(field)).toEqual([]));
  });

  it('a language without a dictionary says so and marks nothing', async () => {
    const shell = await workbench(FACADE);
    const field = await facadeField(shell, 'Spell');
    await waitFor(() => expect(marked(field)).toEqual(['ferri']));

    await shell.quickSetting('spell_check_language', 'fr-FR');
    await waitFor(() => expect(marked(field)).toEqual([]));
    await waitFor(() =>
      expect(shell.status).toBe(
        'No dictionary for French (France). Choose another language in the settings.',
      ),
    );
    expect(
      await screen.findByRole('button', { name: /^Spell check: French \(France\), no dictionary/ }),
    ).toBeInTheDocument();
  });
});

describe('UC-010 BR-006 the spell check language in the command palette (FR-100)', () => {
  it('offers the languages by name, the one in effect marked; choosing Off removes the marks', async () => {
    const shell = await workbench(FACADE);
    const field = await facadeField(shell, 'Spell');
    await waitFor(() => expect(marked(field)).toEqual(['ferri']));

    shell.palette.show('commands');
    const overlay = await waitFor(() => {
      const found = document.querySelector<HTMLElement>('.palette');
      expect(found).not.toBeNull();
      return found!;
    });
    const input = within(overlay).getByRole('combobox');
    await fireEvent.input(input, { target: { value: '>spell check language' } });
    await fireEvent.keyDown(input, { key: 'Enter' });

    const names = () =>
      within(overlay)
        .queryAllByRole('option')
        .map((option) => option.querySelector('.name')?.textContent);
    await waitFor(() =>
      expect(names()).toEqual([
        'Off',
        'English (US)',
        'English (UK)',
        'French (France)',
        'Spanish (Spain)',
      ]),
    );
    await fireEvent.input(input, { target: { value: 'off' } });
    await fireEvent.keyDown(input, { key: 'Enter' });

    await waitFor(() => expect(shell.settings?.spellCheckLanguage).toBe('off'));
    await waitFor(() => expect(marked(field)).toEqual([]));
  });
});

describe('UC-005 A28 the marked words by the keyboard and for screen readers', () => {
  it('F7 goes to the next misspelled word, the palette shows its corrections; marks say misspelled', async () => {
    const shell = await workbench(SOURCE);
    const field = await sourceField(shell, 'Spell');
    await waitFor(() => expect(marked(field)).toEqual(['ferri']));
    expect(field.querySelector('[data-spell]')).toHaveAttribute('aria-invalid', 'spelling');
    const view = activeSourceView()!.view;
    view.dispatch({ selection: { anchor: 0 } });

    await fireEvent.keyDown(field, { key: 'F7' });
    expect(view.state.selection.main.head).toBe(SPELL_TEXTS.Spell.indexOf('ferri'));
    shell.noteActions?.spell('menu');
    await waitFor(menu);
  });

  it('ADR-030: a correction chosen after the text changed is not made', async () => {
    const shell = await workbench(SOURCE);
    const field = await sourceField(shell, 'Spell');
    await waitFor(() => expect(marked(field)).toEqual(['ferri']));
    const source = activeSourceView()!;
    source.view.dispatch({ selection: { anchor: SPELL_TEXTS.Spell.indexOf('ferri') + 1 } });
    await fireEvent.keyDown(field, { key: 'F10', shiftKey: true });
    const opened = await waitFor(menu);
    const ferry = await waitFor(() => within(opened).getByRole('menuitem', { name: 'ferry' }));

    source.view.dispatch({ selection: { anchor: 0 } });
    await source.insert({ kind: 'text', text: 'Now ' });
    await fireEvent.click(ferry);

    await waitFor(() =>
      expect(shell.status).toBe(
        'The text changed since the word was marked; the correction was not made',
      ),
    );
    expect(mockHooks().mirrorText('Spell')).toContain('ferri');
  });
});

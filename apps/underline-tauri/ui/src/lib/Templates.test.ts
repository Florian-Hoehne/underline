// Package 6b (FR-094, FR-095): templates in "New note" (UC-003 A7, A8,
// TC-003-17, TC-003-18), the daily note (UC-025, TC-025-01 to TC-025-11)
// and their settings (TC-010-22), wired to the mock backend. The rules
// themselves are tested in Rust (`templates/tests.rs`, `session/daily`).

import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { clearMocks } from '@tauri-apps/api/mocks';
import { afterEach, describe, expect, it, vi } from 'vitest';
import App from '../App.svelte';
import { connectedShell } from '../test/shell';
import { keyText } from './commands';
import { COMMAND } from './commandList';
import { mockHooks, TEMPLATES, type MockStart } from './mock-backend';

afterEach(() => {
  mockHooks().dispose();
  cleanup();
  clearMocks();
  vi.restoreAllMocks();
});

const AT_0815: MockStart = { givenDir: TEMPLATES, now: '2026-11-11T08:15' };

async function workbench(start: MockStart = AT_0815) {
  const shell = await connectedShell(start);
  render(App, { props: { shell } });
  await shell.startWorkspace();
  return shell;
}

const dialog = (name: string) => screen.findByRole('dialog', { name });
const statusBar = () => document.querySelector<HTMLElement>('.status-bar')!;

/** Ctrl+Shift+`key` (a letter or PageUp/PageDown) where the keyboard is. */
function ctrlShift(key: string, code: string = key.length === 1 ? `Key${key}` : key) {
  const target = document.activeElement ?? document.body;
  return fireEvent.keyDown(target, { key, code, ctrlKey: true, shiftKey: true });
}

describe('UC-003 A7 New note from a template (TC-003-17)', () => {
  it('offers the templates, locks the format to the template and fills it in', async () => {
    const shell = await workbench({ ...AT_0815, now: '2026-11-11T09:00' });
    shell.commandById('file.newNote')!.run();

    const found = await dialog('New note');
    const template = await within(found).findByLabelText<HTMLSelectElement>('Template:');
    const options = within(template)
      .getAllByRole('option')
      .map((option) => option.textContent);
    expect(options).toEqual(['No template', 'Daily', 'Meeting', 'Spec']);
    expect(template.value).toBe('');

    // Step 5: "Spec" sets AsciiDoc and does not let it change.
    template.value = 'templates/Spec';
    await fireEvent.change(template);
    await waitFor(() =>
      expect(within(found).getByRole('radio', { name: 'AsciiDoc' })).toBeChecked(),
    );
    expect(within(found).getByRole('group', { name: 'Note format:' })).toBeDisabled();
    expect(within(found).getByText('The template sets the note format.')).toBeTruthy();

    // "No template" lets it change again.
    template.value = '';
    await fireEvent.change(template);
    await waitFor(() =>
      expect(within(found).getByRole('group', { name: 'Note format:' })).not.toBeDisabled(),
    );

    template.value = 'templates/Meeting';
    await fireEvent.change(template);
    const title = within(found).getByLabelText('Title:');
    await fireEvent.input(title, { target: { value: 'Standup' } });
    await fireEvent.submit(title);

    await waitFor(() => expect(shell.tabs.active).toBe('Standup'));
    expect(mockHooks().noteText('Standup')).toBe(
      '# Standup\nDate: 2026-11-11 09:00\n\nOwner: {{owner}}\n',
    );
    expect(mockHooks().noteText('templates/Meeting')).toContain('{{title}}');
    await waitFor(() => expect(shell.status).toBe('Created the note Standup'));
  });

  it('"New note from template…" puts the keyboard on the templates', async () => {
    const shell = await workbench();
    shell.commandById('file.newNoteFromTemplate')!.run();

    const found = await dialog('New note');
    await waitFor(() => expect(within(found).getByLabelText('Template:')).toHaveFocus());
    shell.closeNoteDialog('newNote');
  });

  it('without templates the command says where they are kept, and the dialog offers none', async () => {
    const shell = await workbench({ givenDir: '/home/user/Notes' });
    shell.commandById('file.newNoteFromTemplate')!.run();

    await waitFor(() =>
      expect(shell.status).toBe(
        'There are no templates: the notes in the folder templates are the templates of this workspace',
      ),
    );
    expect(screen.queryByRole('dialog', { name: 'New note' })).toBeNull();

    // UX review: "Template:" is shown, disabled, and says where templates are kept.
    shell.commandById('file.newNote')!.run();
    const found = await dialog('New note');
    expect(within(found).getByLabelText('Template:')).toBeDisabled();
    expect(
      within(found).getByText('No templates: notes in the folder templates are offered here.'),
    ).toBeTruthy();
    shell.closeNoteDialog('newNote');
  });
});

describe('UC-025 Open daily note', () => {
  it('TC-025-01, TC-025-11: Ctrl+Shift+J creates it from the template, listed first', async () => {
    const shell = await workbench();

    await ctrlShift('J');

    await waitFor(() => expect(shell.tabs.active).toBe('daily/2026-11-11'));
    // OQ-275: one empty line above "Done"; OQ-272: the title keeps its dashes.
    expect(mockHooks().noteText('daily/2026-11-11')).toBe('= 2026-11-11\n\n== Done\n');
    expect(shell.overview[0]?.title).toBe('2026-11-11');
    await waitFor(() => expect(shell.status).toBe('Created the daily note 2026-11-11'));
    expect(shell.overview[0]?.note).toBe('daily/2026-11-11');
    expect(shell.overview[0]?.format).toBe('AsciiDoc');

    // TC-025-03 step 2: asked again, nothing is created, the report goes.
    await ctrlShift('J');
    await waitFor(() => expect(shell.status).toBe(''));
    await waitFor(() => expect(shell.tabs.active).toBe('daily/2026-11-11'));
    expect(shell.overview.filter((item) => item.note === 'daily/2026-11-11')).toHaveLength(1);
  });

  it('TC-025-04: Ctrl+Shift+PageUp and PageDown go to the earlier and later daily notes', async () => {
    const shell = await workbench();
    await shell.openDailyNote();
    await waitFor(() => expect(shell.status).toBe('Created the daily note 2026-11-11'));

    await ctrlShift('PageUp');
    await waitFor(() => expect(shell.tabs.active).toBe('daily/2026-11-05'));
    await ctrlShift('PageUp');
    await waitFor(() => expect(shell.status).toBe('There is no earlier daily note'));
    await ctrlShift('PageDown');
    await waitFor(() => expect(shell.tabs.active).toBe('daily/2026-11-11'));
    await ctrlShift('PageDown');
    await waitFor(() => expect(shell.status).toBe('There is no later daily note'));
  });

  it("OQ-271: outside a daily note, the status names the key of today's", async () => {
    const shell = await workbench();
    await shell.openNote('Home');

    await ctrlShift('PageUp');

    await waitFor(() =>
      expect(shell.status).toBe(
        "This note is not a daily note. Press Ctrl+Shift+J to open today's.",
      ),
    );
    expect(shell.tabs.active).toBe('Home');
  });

  it('TC-025-10: the day changes with the clock', async () => {
    const shell = await workbench({ ...AT_0815, now: '2026-11-11T23:59' });
    await shell.openDailyNote();
    await waitFor(() => expect(shell.tabs.active).toBe('daily/2026-11-11'));
    mockHooks().setNow('2026-11-12T00:01');

    await shell.openDailyNote();

    await waitFor(() => expect(shell.tabs.active).toBe('daily/2026-11-12'));
    await waitFor(() => expect(shell.status).toBe('Created the daily note 2026-11-12'));
  });

  it('BR-007: the keys are in the registry and the top bar', async () => {
    const shell = await workbench();
    const daily = shell.commandById(COMMAND.dailyNote)!;
    expect(daily.title).toBe('Daily note');
    expect(keyText(daily.key, false)).toBe('Ctrl+Shift+J');
    expect(keyText(daily.key, true)).toBe('Cmd+Shift+J');
    expect(keyText(shell.commandById('note.dailyPrevious')!.key, false)).toBe('Ctrl+Shift+PageUp');
    expect(keyText(shell.commandById('note.dailyNext')!.key, true)).toBe('Cmd+Shift+PageDown');
    // No key of the window uses Ctrl+Alt (AltGr on Windows) for them.
    for (const id of [COMMAND.dailyNote, 'note.dailyPrevious', 'note.dailyNext']) {
      expect(keyText(shell.commandById(id)!.key, false)).not.toContain('Alt');
    }

    // UX review: the same form of hint as the palette button's.
    const button = screen.getByRole('button', { name: 'Daily note' });
    expect(button).toHaveAttribute('title', 'Daily note  Ctrl+Shift+J');
    await fireEvent.click(button);
    await waitFor(() => expect(shell.tabs.active).toBe('daily/2026-11-11'));
    await waitFor(() => expect(shell.status).toBe('Created the daily note 2026-11-11'));
  });

  it('TC-025-06: a hidden folder creates nothing and offers the settings of the workspace', async () => {
    const shell = await workbench();
    // Written into the settings file by another program (the window refuses it).
    await mockHooks().settingsOutside('workspace', 'daily_note_folder', '.days');

    await shell.openDailyNote();

    await waitFor(() =>
      expect(shell.status).toBe(
        'The daily note cannot be created in the daily note folder: the folder `.days` is hidden',
      ),
    );
    const offer = within(statusBar()).getByRole('button', { name: 'Open settings' });
    expect(shell.overview.some((item) => item.note.startsWith('.days'))).toBe(false);
    await fireEvent.click(offer);
    await waitFor(() => expect(screen.getByLabelText('Daily note folder')).toBeTruthy());
  });
});

describe('TC-010-22 the settings of templates and daily notes', () => {
  it('"This workspace" shows them with their defaults; "For me" does not', async () => {
    const shell = await workbench();
    await shell.openSettings('workspace');

    expect(await screen.findByLabelText<HTMLInputElement>('Template folder')).toHaveValue(
      'templates',
    );
    const template = screen.getByLabelText<HTMLSelectElement>('Daily note template');
    expect(template.selectedOptions[0]?.textContent).toBe('The template titled Daily');
    expect(within(template).getByRole('option', { name: 'templates/Daily.adoc' })).toBeTruthy();

    await shell.openSettings('me');
    await waitFor(() => expect(screen.queryByLabelText('Template folder')).toBeNull());
    expect(shell.settingsWindow?.me.some((row) => row.key === 'daily_note_folder')).toBe(false);
  });

  it('an emptied folder gives the default again; Enter sends it', async () => {
    const shell = await workbench();
    await shell.openSettings('workspace');
    const folder = await screen.findByLabelText<HTMLInputElement>('Template folder');

    folder.value = 'tpl';
    await fireEvent.input(folder);
    await fireEvent.keyDown(folder, { key: 'Enter' });
    await waitFor(() =>
      expect(
        shell.settingsWindow?.thisWorkspace.find((row) => row.key === 'template_folder')?.note,
      ).toEqual({ kind: 'text', text: 'own' }),
    );
    expect(screen.getByText('Templates and daily note')).toBeTruthy();
    expect(folder).toHaveAccessibleDescription(/Empty: the folder templates\./);

    folder.value = '  ';
    await fireEvent.input(folder);
    await fireEvent.change(folder);
    await waitFor(() => expect(folder).toHaveValue('templates'));
  });

  it('OQ-274 (b): a folder that cannot take notes is said under the field, which keeps it', async () => {
    const shell = await workbench();
    await shell.openSettings('workspace');
    const folder = await screen.findByLabelText<HTMLInputElement>('Daily note folder');

    folder.value = '.days';
    await fireEvent.input(folder);
    await fireEvent.change(folder);

    await waitFor(() =>
      expect(folder).toHaveAccessibleDescription(
        /Daily note folder not changed: the folder `\.days` is hidden/,
      ),
    );
    expect(folder).toHaveValue('.days');
    expect(folder).toHaveAttribute('aria-invalid', 'true');
    expect(mockHooks().settingsFile('workspace').daily_note_folder).toBeUndefined();
  });
});

// Package 5 (FR-087 to FR-090): the editor zoom (UC-010 A11, BR-009,
// TC-010-18), the focus mode (A12, BR-010, TC-010-19), soft wrap (A3,
// BR-011, TC-010-20) and the text statistics of the note properties (UC-004
// BR-020, TC-004-39), against the mock backend's `/home/user/Reading`. What
// the statistics count is the editor core's (in the WASM, tested in Rust);
// here the keys, the palette, the status line and the tool panel reach it,
// and what lands in the settings file.

import { cleanup, fireEvent, render, waitFor, within } from '@testing-library/svelte';
import { clearMocks } from '@tauri-apps/api/mocks';
import { StateEffect } from '@codemirror/state';
import { afterEach, describe, expect, it, vi } from 'vitest';
import App from '../App.svelte';
import { connectedShell } from '../test/shell';
import { activeSourceView } from './editor/active';
import { WheelZoom } from './editor/zoom';
import { statisticsTexts } from './editor/statistics';
import { activeFacade } from './facade/active';
import { keyText } from './commands';
import { mockHooks, READING, READING_TEXTS, type MockStart } from './mock-backend';
import type { Shell } from './shell.svelte';

afterEach(() => {
  mockHooks().dispose();
  cleanup();
  clearMocks();
  vi.restoreAllMocks();
});

const SOURCE: MockStart = { givenDir: READING, view: 'source' };
const FACADE: MockStart = { givenDir: READING };

async function workbench(start: MockStart): Promise<Shell> {
  const shell = await connectedShell(start);
  render(App, { props: { shell } });
  await shell.startWorkspace();
  return shell;
}

/** The text field of the source view of `note`, once its text is in, with the keyboard. */
async function sourceField(shell: Shell, note: string): Promise<HTMLElement> {
  await shell.openNote(note);
  await waitFor(() => expect(activeSourceView()?.note).toBe(note));
  const field = document.querySelector<HTMLElement>('.cm-content')!;
  field.focus();
  return field;
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

/** The value of the row `name` of the note properties. */
function property(name: string): string | null {
  const panel = document.querySelector<HTMLElement>('.tool-panel');
  const term = Array.from(panel?.querySelectorAll('dt') ?? []).find(
    (dt) => dt.textContent === name,
  );
  return term?.nextElementSibling?.textContent ?? null;
}

const overlay = () => document.querySelector<HTMLElement>('.palette');
const paletteField = () => within(overlay()!).getByRole('combobox');

/** Runs the palette's command named `title`. */
async function runFromPalette(title: string): Promise<void> {
  await fireEvent.keyDown(document.body, { key: 'P', code: 'KeyP', ctrlKey: true, shiftKey: true });
  await waitFor(() => expect(overlay()).toBeTruthy());
  await fireEvent.input(paletteField(), { target: { value: `>${title}` } });
  await waitFor(() =>
    expect(within(overlay()!).getAllByRole('option')[0]?.textContent).toContain(title),
  );
  await fireEvent.keyDown(paletteField(), { key: 'Enter' });
}

describe('TC-004-39 FR-090 UC-004 BR-020 the text statistics in the note properties', () => {
  it('TC-004-39 step 1: 8 words, the characters shown and 1 minute', async () => {
    const shell = await workbench(SOURCE);
    await sourceField(shell, 'Count');

    await waitFor(() => expect(property('Words')).toBe('8'));
    expect(property('Characters')).toBe('38');
    expect(property('Reading time')).toBe('1 minute');
    expect(property('Selected words')).toBeNull();
  });

  it('TC-004-39 step 2: the selection "well-known fact" is 2 words and 15 characters', async () => {
    const shell = await workbench(SOURCE);
    await sourceField(shell, 'Count');
    await waitFor(() => expect(property('Words')).toBe('8'));

    const text = READING_TEXTS.Count;
    const view = activeSourceView()!.view;
    view.dispatch({
      selection: { anchor: text.indexOf('well-known'), head: text.indexOf('](') },
    });

    await waitFor(() => expect(property('Selected words')).toBe('2'), { timeout: 2000 });
    expect(property('Selected characters')).toBe('15');
    expect(property('Words')).toBe('8');
  });

  it('TC-004-39 step 3, BR-020: the statistics follow a change within a second', async () => {
    const shell = await workbench(SOURCE);
    const field = await sourceField(shell, 'Count');
    await waitFor(() => expect(property('Words')).toBe('8'));

    const view = activeSourceView()!.view;
    view.dispatch({ selection: { anchor: view.state.doc.length } });
    for (const c of 'more words') {
      field.dispatchEvent(
        new InputEvent('beforeinput', {
          inputType: 'insertText',
          data: c,
          bubbles: true,
          cancelable: true,
        }),
      );
    }

    await waitFor(() => expect(property('Words')).toBe('10'), { timeout: 1000 });
  });

  it('BR-020: the facade and an AsciiDoc note count the same text', async () => {
    const shell = await workbench(FACADE);
    await facadeField(shell, 'Count-AsciiDoc');

    await waitFor(() => expect(property('Words')).toBe('8'));
    expect(property('Characters')).toBe('38');
  });

  it('BR-020: the texts of the note properties', () => {
    expect(
      statisticsTexts({ words: 1234, characters: 1, readingMinutes: 7, selection: null }),
    ).toEqual({ words: '1,234', characters: '1', reading: '7 minutes', selection: null });
    expect(
      statisticsTexts({
        words: 0,
        characters: 0,
        readingMinutes: 0,
        selection: { words: 1, characters: 1 },
      }),
    ).toMatchObject({ reading: '0 minutes', selection: { words: '1', characters: '1' } });
  });
});

describe('TC-010-18 FR-087 UC-010 A11 BR-009 the editor zoom', () => {
  const zoomOf = (selector: string) =>
    document.querySelector<HTMLElement>(selector)?.style.getPropertyValue('--editor-zoom');

  it('TC-010-18 steps 1 to 4: Ctrl+Plus, Ctrl+0 and Ctrl+Minus, within 50 and 300 percent', async () => {
    const shell = await workbench(SOURCE);
    const field = await sourceField(shell, 'Count');

    await fireEvent.keyDown(field, { key: '+', ctrlKey: true });
    await fireEvent.keyDown(field, { key: '=', ctrlKey: true });

    expect(shell.status).toBe('Editor zoom: 120%');
    await waitFor(() => expect(mockHooks().settingsFile('user').editor_zoom).toBe('120'));
    await waitFor(() => expect(zoomOf('.cm-editor')).toBe('1.2'));
    expect(mockHooks().mirrorText('Count')).toBe(READING_TEXTS.Count);

    await fireEvent.keyDown(field, { key: '0', ctrlKey: true });
    await waitFor(() => expect(shell.settings?.editorZoom).toBe(100));
    expect(shell.status).toBe('Editor zoom: 100%');

    for (let press = 0; press < 6; press++) {
      await fireEvent.keyDown(field, { key: '-', ctrlKey: true });
    }
    await waitFor(() => expect(shell.settings?.editorZoom).toBe(50));
    expect(shell.status).toBe('Editor zoom: 50% (smallest)');
    expect(mockHooks().settingsFile('user').editor_zoom).toBe('50');
    // Only the text of the note: the panels keep their size.
    expect(zoomOf('.tool-panel')).toBe('');
  });

  it('TC-010-18 step 5: Ctrl and the mouse wheel over the facade zoom it by 10 percent per notch', async () => {
    const shell = await workbench(FACADE);
    const text = await facadeField(shell, 'Count');

    await fireEvent.wheel(text, { deltaY: -100, ctrlKey: true });
    await waitFor(() => expect(shell.settings?.editorZoom).toBe(110));
    await waitFor(() => expect(zoomOf('.facade')).toBe('1.1'));
    // Without Ctrl the wheel scrolls.
    await fireEvent.wheel(text, { deltaY: -100 });
    expect(shell.editorZoom).toBe(110);
  });

  it('A11, OQ-263: a step beyond 300 percent does nothing but say so', async () => {
    const shell = await workbench({ ...SOURCE, settings: { editorZoom: 300 } });
    await sourceField(shell, 'Count');
    shell.status = '';
    await shell.zoomEditor(1);
    expect(shell.status).toBe('Editor zoom: 300% (largest)');
    expect(shell.editorZoom).toBe(300);
    expect(mockHooks().settingsFile('user').editor_zoom).toBe('300');
  });

  it('review: Ctrl+0 is told by its key on AZERTY, where it types à', async () => {
    const shell = await workbench({ ...SOURCE, settings: { editorZoom: 150 } });
    const field = await sourceField(shell, 'Count');
    await fireEvent.keyDown(field, { key: 'à', code: 'Digit0', ctrlKey: true });
    await waitFor(() => expect(shell.settings?.editorZoom).toBe(100));
  });

  it('review: the zoom keys wait while the palette is open', async () => {
    const shell = await workbench(SOURCE);
    await sourceField(shell, 'Count');
    shell.palette.show('commands');
    const field = await waitFor(paletteField);
    await fireEvent.keyDown(field, { key: '+', ctrlKey: true });
    expect(shell.editorZoom).toBe(100);
  });

  it('UX M1: after a zoom the line numbers are made anew and measured', async () => {
    const shell = await workbench(SOURCE);
    const field = await sourceField(shell, 'Count');
    const view = activeSourceView()!.view;
    const measure = vi.spyOn(view, 'requestMeasure');
    const dispatch = vi.spyOn(view, 'dispatch');
    await fireEvent.keyDown(field, { key: '+', ctrlKey: true });
    await waitFor(() => expect(view.dom.style.getPropertyValue('--editor-zoom')).toBe('1.1'));
    // A compartment's reconfigure carries the compartment it is for.
    const reconfigured = () =>
      dispatch.mock.calls.some(([spec]) => {
        const effects = (spec as { effects?: unknown }).effects;
        return (Array.isArray(effects) ? effects : [effects]).some(
          (effect) =>
            effect instanceof StateEffect &&
            typeof effect.value === 'object' &&
            effect.value !== null &&
            'compartment' in effect.value,
        );
      });
    await waitFor(() => expect(reconfigured()).toBe(true));
    expect(measure.mock.calls.length).toBeGreaterThanOrEqual(2);
  });

  it('review: a zoom changed in the settings window updates the status', async () => {
    const shell = await workbench(SOURCE);
    await sourceField(shell, 'Count');
    await shell.zoomEditor(1);
    expect(shell.status).toBe('Editor zoom: 110%');
    await shell.changeSetting('me', 'editor_zoom', '200');
    await waitFor(() => expect(shell.settings?.editorZoom).toBe(200));
    await waitFor(() => expect(shell.status).toBe('Editor zoom: 200%'));
  });

  it('A11: the turns of a touchpad add up to a notch', () => {
    const wheel = new WheelZoom();
    const turn = (deltaY: number, deltaMode = 0) =>
      wheel.step({
        deltaY,
        deltaMode,
        ctrlKey: true,
        altKey: false,
        shiftKey: false,
        metaKey: false,
      });
    expect(turn(-20)).toBe(0);
    expect(turn(-20)).toBe(0);
    expect(turn(-20)).toBe(1);
    expect(turn(30)).toBe(0);
    expect(turn(-30)).toBe(0);
    expect(turn(4, 1)).toBe(-1);
  });

  it('BR-006: the keys as the palette names them', async () => {
    const shell = await workbench(SOURCE);
    const key = (id: string, mac: boolean) => keyText(shell.commandById(id)?.key, mac);
    expect(key('view.zoomIn', false)).toBe('Ctrl+Plus');
    expect(key('view.zoomOut', false)).toBe('Ctrl+Minus');
    expect(key('view.zoomReset', false)).toBe('Ctrl+0');
    expect(key('view.zoomIn', true)).toBe('Cmd+Plus');
    expect(key('view.focusMode', false)).toBe('Ctrl+Alt+F');
    // Review: Cmd+Ctrl+F is the system's full screen on macOS.
    expect(key('view.focusMode', true)).toBe('Cmd+Shift+.');
    expect(key('status.softWrap', false)).toBe('Alt+Z');
  });
});

describe('TC-010-19 FR-088 UC-010 A12 BR-010 the focus mode', () => {
  it('TC-010-19 steps 1, 2 and 4: hides the overview, the tool panel and the toolbars', async () => {
    const shell = await workbench(SOURCE);
    const field = await sourceField(shell, 'Count');
    expect(document.querySelector('.top-bar')).toBeTruthy();
    expect(document.querySelector('.tool-panel')).toBeTruthy();

    await fireEvent.keyDown(field, { key: 'f', code: 'KeyF', ctrlKey: true, altKey: true });

    await waitFor(() => expect(document.querySelector('.top-bar')).toBeNull());
    expect(document.querySelector('.tool-panel')).toBeNull();
    expect(document.querySelector('.views')).toBeNull();
    expect(document.querySelector('aside.overview')).toBeNull();
    // The tabs, the note and the status bar stay.
    expect(document.querySelector('[role="tablist"]')).toBeTruthy();
    expect(document.querySelector('.cm-content')).toBeTruthy();
    expect(document.querySelector('.status-bar')).toBeTruthy();
    await waitFor(() => expect(mockHooks().settingsFile('user').focus_mode).toBe('true'));
    // UX review M2: the status bar says how to leave, the keyboard stays in the note.
    expect(shell.status).toBe('Focus mode on. Press Ctrl+Alt+F to leave.');
    expect(document.activeElement).toBe(document.querySelector('.cm-content'));

    // Step 2: the search and the palette open as usual.
    await fireEvent.keyDown(document.body, {
      key: 'F',
      code: 'KeyF',
      ctrlKey: true,
      shiftKey: true,
    });
    await waitFor(() => expect(shell.search.open).toBe(true));
    shell.search.close();

    // Step 4: the palette's command shows everything again.
    await runFromPalette('Turn off focus mode');
    await waitFor(() => expect(document.querySelector('.top-bar')).toBeTruthy());
    expect(document.querySelector('.tool-panel')).toBeTruthy();
    expect(document.querySelector('.views')).toBeTruthy();
    await waitFor(() => expect(mockHooks().settingsFile('user').focus_mode).toBe('false'));
  });

  it('OQ-262: Ctrl+Shift+G ends the focus mode, as the changes show in the tool panel', async () => {
    const shell = await workbench({ givenDir: '/home/user/Work', settings: { focusMode: true } });
    await waitFor(() => expect(shell.changes).not.toBeNull());
    expect(document.querySelector('.tool-panel')).toBeNull();

    await fireEvent.keyDown(document.body, {
      key: 'G',
      code: 'KeyG',
      ctrlKey: true,
      shiftKey: true,
    });

    const message = await waitFor(() =>
      within(document.querySelector<HTMLElement>('.tool-panel')!).getByRole('textbox', {
        name: 'Commit message',
      }),
    );
    await waitFor(() => expect(message).toHaveFocus());
    expect(shell.focusMode).toBe(false);
  });

  it('TC-010-19 step 3: on after a restart, as the settings say, with the hint once', async () => {
    const shell = await workbench({ ...SOURCE, settings: { focusMode: true } });
    expect(document.querySelector('.top-bar')).toBeNull();
    expect(shell.status).toBe('Focus mode on. Press Ctrl+Alt+F to leave.');
  });

  it('UX M2: the status line switches the focus mode and names its key', async () => {
    const shell = await workbench(SOURCE);
    await sourceField(shell, 'Count');
    const status = () => within(document.querySelector<HTMLElement>('.editor-status')!);
    const focus = status().getByRole('button', { name: /focus/i });
    expect(focus).toHaveAttribute('aria-pressed', 'false');
    expect(focus).toHaveAttribute('aria-keyshortcuts', 'Control+Alt+F');
    expect(focus.getAttribute('title')).toBe(
      'Hide the note overview, the tool panel and the toolbars (Ctrl+Alt+F)',
    );
    expect(status().getByRole('button', { name: /wrap/i }).getAttribute('title')).toBe(
      'Wrap long lines at the edge of the editor (Alt+Z)',
    );

    await fireEvent.click(focus);
    await waitFor(() => expect(document.querySelector('.tool-panel')).toBeNull());
    const shown = status().getByRole('button', { name: /focus/i });
    expect(shown).toHaveAttribute('aria-pressed', 'true');
    await fireEvent.click(shown);
    await waitFor(() => expect(document.querySelector('.tool-panel')).toBeTruthy());
    expect(shell.focusMode).toBe(false);
  });

  it('UX M2: the keyboard in a part the focus mode hides goes into the note', async () => {
    const shell = await workbench(SOURCE);
    await sourceField(shell, 'Count');
    document.querySelector<HTMLElement>('.views button')!.focus();
    await shell.toggleFocusMode();
    await waitFor(() => expect(document.activeElement).toBe(document.querySelector('.cm-content')));
  });

  it("review: on macOS Cmd+Shift+. switches it, told by the key's place", async () => {
    const shell = await workbench({ ...SOURCE, platform: 'macos' });
    const field = await sourceField(shell, 'Count');
    await fireEvent.keyDown(field, { key: '>', code: 'Period', metaKey: true, shiftKey: true });
    await waitFor(() => expect(shell.focusMode).toBe(true));
    expect(shell.status).toBe('Focus mode on. Press Cmd+Shift+. to leave.');
  });
});

describe('TC-010-20 FR-089 UC-010 A3 BR-011 soft wrap', () => {
  it('TC-010-20 steps 1 to 4: Alt+Z, the status line and the facade', async () => {
    const shell = await workbench(SOURCE);
    const field = await sourceField(shell, 'Wide');
    expect(field.classList.contains('cm-lineWrapping')).toBe(true);
    const wrap = () =>
      within(document.querySelector('.editor-status')!).getByRole('button', { name: /wrap/i });
    expect(wrap()).toHaveAttribute('aria-pressed', 'true');

    await fireEvent.keyDown(field, { key: 'z', code: 'KeyZ', altKey: true });

    await waitFor(() => expect(field.classList.contains('cm-lineWrapping')).toBe(false));
    await waitFor(() => expect(shell.settings?.softWrap).toBe(false));
    expect(wrap()).toHaveAttribute('aria-pressed', 'false');
    await waitFor(() => expect(mockHooks().settingsFile('user').soft_wrap).toBe('false'));
    // BR-011: the line numbers still count the note's lines.
    expect(activeSourceView()!.lineCount()).toBe(READING_TEXTS.Wide.split('\n').length);

    // Step 3: the facade keeps its paragraphs wrapped; tables and code scroll.
    shell.noteActions?.view('facade');
    await waitFor(() => expect(activeFacade()?.note).toBe('Wide'));
    await waitFor(() =>
      expect(document.querySelector('.facade')?.classList.contains('nowrap')).toBe(true),
    );

    // The status line's switch turns it on again.
    await fireEvent.click(wrap());
    await waitFor(() =>
      expect(document.querySelector('.facade')?.classList.contains('nowrap')).toBe(false),
    );
    // Step 4: the note is unchanged.
    expect(mockHooks().mirrorText('Wide')).toBe(READING_TEXTS.Wide);
  });
});

describe('review: Alt+Z on macOS', () => {
  it('is taken by soft wrap, so no "Ω" lands in the note', async () => {
    const shell = await workbench({ ...SOURCE, platform: 'macos' });
    const field = await sourceField(shell, 'Wide');
    const event = new KeyboardEvent('keydown', {
      key: 'Ω',
      code: 'KeyZ',
      altKey: true,
      bubbles: true,
      cancelable: true,
    });
    field.dispatchEvent(event);
    expect(event.defaultPrevented).toBe(true);
    await waitFor(() => expect(shell.settings?.softWrap).toBe(false));
    expect(mockHooks().mirrorText('Wide')).toBe(READING_TEXTS.Wide);
  });
});

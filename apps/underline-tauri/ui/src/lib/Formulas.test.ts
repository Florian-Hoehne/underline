// Package 7b (FR-098): formulas typeset in the facade (UC-004 BR-019, A21,
// TC-004-38) and edited there (UC-005 BR-008, TC-005-42), against the mock
// backend's `/home/user/Reading`, whose formulas are a placeholder picture
// (the backend typesets them with RaTeX, tested in Rust). What the parser
// reads as a formula is the editor core's, in the WASM.

import { cleanup, render, waitFor } from '@testing-library/svelte';
import { clearMocks } from '@tauri-apps/api/mocks';
import { afterEach, describe, expect, it, vi } from 'vitest';
import App from '../App.svelte';
import { connectedShell } from '../test/shell';
import { activeFacade } from './facade/active';
import { mockHooks, READING, type MockStart } from './mock-backend';
import type { Shell } from './shell.svelte';

afterEach(() => {
  mockHooks().dispose();
  cleanup();
  clearMocks();
  vi.restoreAllMocks();
});

const FACADE: MockStart = { givenDir: READING };

async function workbench(): Promise<Shell> {
  const shell = await connectedShell(FACADE);
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

const pictures = (field: HTMLElement) =>
  Array.from(field.querySelectorAll<HTMLImageElement>('img')).map((img) =>
    img.alt.replace(/^Formula: /, ''),
  );

describe('TC-004-38 formulas', () => {
  it('step 1: inline and block formulas are typeset, a price stays text, an error shows the source with its reason', async () => {
    const shell = await workbench();
    const field = await facadeField(shell, 'Math');

    await waitFor(() =>
      expect(pictures(field)).toEqual(['E=mc^2', '\\sum_{i=1}^n i', '\\sqrt{2}']),
    );
    const first = field.querySelector<HTMLElement>('[data-b="0"] .text')!;
    expect(first.textContent).toBe('Energy  and price 5$ or 6$');
    const failed = field.querySelector<HTMLElement>('[data-b="3"]')!;
    expect(field.querySelector('img')?.alt).toBe('Formula: E=mc^2');
    expect(failed.querySelector('[role="status"]')?.textContent).toContain(
      'This formula cannot be typeset',
    );
    expect(failed.querySelector('.frame')).toHaveTextContent('$$\\frac{1}{$$');
  });

  it('step 3: the formulas of an AsciiDoc note with `:stem: latexmath`', async () => {
    const shell = await workbench();
    const field = await facadeField(shell, 'Math-AsciiDoc');

    await waitFor(() => expect(pictures(field)).toEqual(['a^2', 'b^2']));
  });

  it('step 4, OQ-222: AsciiMath with `:stem:` and `asciimath`, typeset from its LaTeX', async () => {
    const shell = await workbench();
    const field = await facadeField(shell, 'Math-AsciiMath');

    await waitFor(() =>
      expect(pictures(field)).toEqual(['\\frac{x^{2}}{2}', '\\sqrt{a}', '\\sum_{i=1}^{n} i']),
    );
  });
});

// UC-005 A28, OQ-302: how the spell menu opens and who has the keyboard
// when the corrections come late or not at all.

import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';

const answers: { resolve: (words: string[]) => void; reject: (error: unknown) => void }[] = [];

vi.mock('./spell', () => ({
  spellSuggestions: () =>
    new Promise<string[]>((resolve, reject) => answers.push({ resolve, reject })),
  addToDictionary: async () => null,
}));

const { default: SpellMenu } = await import('./SpellMenu.svelte');

afterEach(() => {
  cleanup();
  answers.length = 0;
});

function open(wait = 20) {
  const onChoose = vi.fn();
  render(SpellMenu, {
    props: { word: 'ferri', x: 10, y: 10, onChoose, onError: vi.fn(), onClose: vi.fn(), wait },
  });
  return onChoose;
}

const live = () => document.querySelector('[aria-live="polite"]');

describe('the spell menu (OQ-302)', () => {
  it('late corrections: the menu has the keyboard, then the first correction', async () => {
    open();
    const menu = await screen.findByRole('menu', { name: 'Spelling of ferri, misspelled' });
    await waitFor(() => expect(menu).toHaveFocus());
    expect(within(menu).getByText('Looking for corrections…')).toBeInTheDocument();
    expect(live()).toHaveTextContent('Looking for corrections');

    answers[0].resolve(['ferry', 'fern', 'berry']);
    const ferry = await within(menu).findByRole('menuitem', { name: 'ferry' });
    await waitFor(() => expect(ferry).toHaveFocus());
    expect(live()).toHaveTextContent('3 corrections');
  });

  it('late corrections after a key: the keyboard stays where the user put it', async () => {
    open();
    const menu = await screen.findByRole('menu');
    await waitFor(() => expect(menu).toHaveFocus());
    await fireEvent.keyDown(menu, { key: 'ArrowDown' });
    const add = within(menu).getByRole('menuitem', { name: 'Add "ferri" to dictionary' });
    expect(add).toHaveFocus();

    answers[0].resolve(['ferry']);
    await within(menu).findByRole('menuitem', { name: 'ferry' });
    await new Promise((resolve) => setTimeout(resolve, 20));
    expect(add).toHaveFocus();
  });

  it('no corrections: "Add …" has the keyboard; a failure says so', async () => {
    open(1000);
    answers[0].resolve([]);
    const menu = await screen.findByRole('menu');
    const add = within(menu).getByRole('menuitem', { name: 'Add "ferri" to dictionary' });
    await waitFor(() => expect(add).toHaveFocus());
    expect(within(menu).getByText('No corrections found')).toBeInTheDocument();
    cleanup();

    open(1000);
    answers[1].reject(new Error('no dictionary'));
    const failed = await screen.findByRole('menu');
    expect(within(failed).getByText('Corrections unavailable')).toBeInTheDocument();
  });
});

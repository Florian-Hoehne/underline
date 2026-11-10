import { cleanup, fireEvent, render, screen } from '@testing-library/svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';
import Enlarged from './Enlarged.svelte';

afterEach(() => cleanup());

const hint = () => document.querySelector('.hint')?.textContent ?? '';

describe('M-09.9 the enlarged view', () => {
  it('zooms with + = -, fits again with 0, and closes with Escape', async () => {
    const onClose = vi.fn();
    render(Enlarged, { props: { url: 'data:image/svg+xml,x', width: 200, height: 100, onClose } });
    const dialog = screen.getByRole('dialog', { name: 'Enlarged picture' });
    // jsdom lays nothing out: the window has no room, so the fit is the least zoom.
    expect(hint()).toMatch(/^10 % /);

    await fireEvent.keyDown(dialog, { key: '+' });
    expect(hint()).toMatch(/^13 % /);
    await fireEvent.keyDown(dialog, { key: '=' });
    expect(hint()).toMatch(/^16 % /);
    await fireEvent.keyDown(dialog, { key: '-' });
    expect(hint()).toMatch(/^13 % /);
    await fireEvent.wheel(dialog, { deltaY: -300 });
    expect(hint()).toMatch(/^34 % /);
    await fireEvent.keyDown(dialog, { key: '0' });
    expect(hint()).toMatch(/^10 % /);
    expect(onClose).not.toHaveBeenCalled();

    await fireEvent.keyDown(dialog, { key: 'Escape' });
    expect(onClose).toHaveBeenCalledOnce();
  });
});

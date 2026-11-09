import { render, screen } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';
import StatusBar from './StatusBar.svelte';

describe('M-04.7 status bar text', () => {
  it('joins the root path and the status with "  |  "', () => {
    render(StatusBar, { props: { root: '/home/me/notes', status: 'Saved Ideas' } });

    expect(screen.getByRole('status').textContent).toBe('/home/me/notes  |  Saved Ideas');
  });

  it('has the whole text as its tooltip', () => {
    render(StatusBar, { props: { root: '/notes', status: 'Saved' } });

    expect(screen.getByRole('status')).toHaveAttribute('title', '/notes  |  Saved');
  });

  it('shows the status alone without a workspace', () => {
    render(StatusBar, { props: { root: null, status: 'Wiki repository: x' } });

    expect(screen.getByRole('status').textContent).toBe('Wiki repository: x');
  });

  it('shows the root alone without a status', () => {
    render(StatusBar, { props: { root: '/notes', status: '' } });

    expect(screen.getByRole('status').textContent).toBe('/notes');
  });

  it('is empty with neither', () => {
    render(StatusBar, { props: { root: null, status: '' } });

    expect(screen.getByRole('status').textContent).toBe('');
  });
});

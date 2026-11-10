import { describe, expect, it } from 'vitest';
import type { KeysStatusView } from '../bindings';
import { statusTexts } from './status';

const vim: KeysStatusView = {
  keymap: 'VIM',
  mode: 'INSERT',
  color: 1,
  pending: '',
  hints: [],
  prompting: false,
};

describe('M-11.4 texts of the status line', () => {
  it('shows "{keymap} · {mode}", only the mode below 450 px, and Nano without a mode', () => {
    expect(statusTexts(800, vim).badge).toBe('VIM · INSERT');
    expect(statusTexts(449, vim).badge).toBe('INSERT');
    expect(statusTexts(300, { ...vim, keymap: 'NANO', mode: null }).badge).toBe('NANO');
    expect(statusTexts(800, null).badge).toBe('');
  });

  it('gives the switches their short names below 720 px', () => {
    expect(statusTexts(720, null)).toMatchObject({
      copy: 'Copy on select',
      minimap: 'Minimap',
      keysLabel: 'Keys:',
    });
    expect(statusTexts(719, null)).toMatchObject({ copy: 'Copy', minimap: 'Map', keysLabel: '' });
  });
});

// M-11.4 (FR-026): the texts of the editor's status line for its width, as
// the egui app picks them (`editor_status_line`, `mode_badge`): below
// 720 px the switches have their short names and the choice drops "Keys:";
// below 450 px the badge shows the mode only.

import type { KeysStatusView } from '../bindings';

/** Below this width the switches have their short names. */
export const NARROW = 720;
/** Below this width the badge shows only the mode. */
export const BADGE_NARROW = 450;

export type StatusTexts = {
  /** The badge; empty with the usual keys. */
  badge: string;
  copy: string;
  minimap: string;
  /** "Keys:" before the choice of the input mode, or nothing. */
  keysLabel: string;
};

/** The texts of the status line `width` pixels wide with the keys `keys`. */
export function statusTexts(width: number, keys: KeysStatusView | null): StatusTexts {
  const narrow = width < NARROW;
  let badge = '';
  if (keys) {
    if (keys.mode === null) {
      badge = keys.keymap;
    } else {
      badge = width < BADGE_NARROW ? keys.mode : `${keys.keymap} · ${keys.mode}`;
    }
  }
  return {
    badge,
    copy: narrow ? 'Copy' : 'Copy on select',
    minimap: narrow ? 'Map' : 'Minimap',
    keysLabel: narrow ? '' : 'Keys:',
  };
}

// M-01.6, FR-022: the window in the colours of the theme in effect. The
// backend sends the theme's palette (`PaletteView`); this module turns it
// into CSS custom properties on <html>. Every colour comes from the palette,
// none is computed here, except egui's fixed colours of the todos
// (`TODO_COLORS`), which are in no palette.

import type { PaletteView } from './bindings';

/**
 * The colours the shell's CSS uses, by role, each taken from one palette
 * field. The CSS reads them as `--ui-<role>`; `CONTRAST_PAIRS` lists which of
 * them meet, and the contrast test checks every pair in every theme.
 */
export type ShellColors = {
  /** Bars, menus and the central area (egui `panel_fill`). */
  surface: string;
  /** Text fields (egui `extreme_bg_color`). */
  field: string;
  /** Body text. */
  text: string;
  /** Hints such as shortcut texts, and disabled entries. */
  hint: string;
  /** Buttons that are not switched on (`Palette::button`). */
  button: string;
  /** Hovered and chosen entries (`Palette::chosen`). */
  chosen: string;
  /** Text on `chosen` (`Palette::chosen_text`). */
  chosenText: string;
  /** Outlines of buttons, fields and menus. */
  border: string;
  /** The keyboard focus ring. */
  focus: string;
  /**
   * Lines that only divide areas, such as under the top bar
   * (`Palette::inset`). They are decoration, not controls, so NFR-010 asks
   * no contrast of them.
   */
  rule: string;
};

/** The shell's colours for `palette`. */
export function shellColors(palette: PaletteView): ShellColors {
  return {
    surface: palette.panel,
    field: palette.background,
    text: palette.foreground,
    hint: palette.weak,
    button: palette.button,
    chosen: palette.chosen,
    chosenText: palette.chosenText,
    border: palette.weak,
    focus: palette.link,
    rule: palette.inset,
  };
}

/** What a colour pair is for, with the contrast NFR-010 asks of it. */
export type ContrastPair = {
  what: string;
  fg: keyof ShellColors;
  bg: keyof ShellColors;
  least: number;
};

/**
 * Every pair of shell colours that meets in the CSS. Body text needs 4.5:1,
 * interface controls 3:1 (NFR-010, WCAG 2.1 AA).
 */
export const CONTRAST_PAIRS: readonly ContrastPair[] = [
  { what: 'text on the bars', fg: 'text', bg: 'surface', least: 4.5 },
  { what: 'text in a field', fg: 'text', bg: 'field', least: 4.5 },
  { what: 'text on a button', fg: 'text', bg: 'button', least: 4.5 },
  { what: 'text on a chosen entry', fg: 'chosenText', bg: 'chosen', least: 4.5 },
  { what: 'hint on the bars', fg: 'hint', bg: 'surface', least: 3 },
  { what: 'outline against the bars', fg: 'border', bg: 'surface', least: 3 },
  { what: 'outline against a field', fg: 'border', bg: 'field', least: 3 },
  { what: 'outline against a button', fg: 'border', bg: 'button', least: 3 },
  { what: 'focus against the bars', fg: 'focus', bg: 'surface', least: 3 },
  { what: 'focus against a field', fg: 'focus', bg: 'field', least: 3 },
  { what: 'focus against a chosen entry', fg: 'focus', bg: 'chosen', least: 3 },
];

/** The WCAG 2.1 contrast ratio of two `#rrggbb` colours, from 1 to 21. */
export function contrast(a: string, b: string): number {
  const la = luminance(a);
  const lb = luminance(b);
  return (Math.max(la, lb) + 0.05) / (Math.min(la, lb) + 0.05);
}

function luminance(hex: string): number {
  const [r, g, b] = [1, 3, 5].map((at) => {
    const c = parseInt(hex.slice(at, at + 2), 16) / 255;
    return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
  });
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

/**
 * M-18.2, M-18.8, M-04.5 (feature F15): egui's fixed colours of the todos,
 * which no palette holds: the urgency High (egui's orange 220, 140, 20),
 * Critical (210, 50, 50) and the overdue todos (`error_fg_color`). As the
 * conflict colour of F13, each is darker on light themes and lighter on
 * dark ones, so text in it reaches 4.5:1 on the bars and in fields of every
 * theme (NFR-010, A-178). They are set as `--ui-urgency-high`,
 * `--ui-urgency-critical` and `--ui-overdue`.
 */
export const TODO_COLORS = {
  light: { urgencyHigh: '#8f4b00', urgencyCritical: '#a81c1c', overdue: '#a80000' },
  dark: { urgencyHigh: '#f5b456', urgencyCritical: '#ff9c9c', overdue: '#ff9c9c' },
} as const;

/** `codeBackground` → `code-background`. */
function kebab(name: string): string {
  return name.replace(/[A-Z]/g, (c) => `-${c.toLowerCase()}`);
}

/**
 * Applies `palette` to `root` (normally <html>): `--ui-<role>` for the
 * shell's colours, `--palette-<field>` for every colour of the palette (for
 * the editor and later views), and `data-scheme` after the theme's
 * darkness, which sets `color-scheme` (app.css), so scroll bars and form
 * controls follow it.
 */
export function applyPalette(
  palette: PaletteView,
  root: HTMLElement = document.documentElement,
): void {
  const style = root.style;
  for (const [role, colour] of Object.entries(shellColors(palette))) {
    style.setProperty(`--ui-${kebab(role)}`, colour);
  }
  for (const [field, value] of Object.entries(palette)) {
    if (typeof value === 'string' && value.startsWith('#')) {
      style.setProperty(`--palette-${kebab(field)}`, value);
    } else if (Array.isArray(value)) {
      value.forEach((colour, at) => style.setProperty(`--palette-${kebab(field)}-${at}`, colour));
    }
  }
  for (const [role, colour] of Object.entries(TODO_COLORS[palette.dark ? 'dark' : 'light'])) {
    style.setProperty(`--ui-${kebab(role)}`, colour);
  }
  root.dataset.scheme = palette.dark ? 'dark' : 'light';
  root.dataset.theme = palette.theme;
}

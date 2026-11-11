import { describe, expect, it } from 'vitest';
import { PALETTES } from './mock-backend';
import { applyPalette, CONTRAST_PAIRS, contrast, shellColors, TODO_COLORS } from './theme';

describe('M-01.6 theme palettes', () => {
  it('has all 11 themes', () => {
    expect(PALETTES.map((p) => p.theme)).toEqual([
      'Light',
      'Dark',
      'GitHub Light',
      'GitHub Dark',
      'Solarized Light',
      'Solarized Dark',
      'Night Owl',
      'Ubuntu',
      'Nord',
      'Dracula',
      'Gruvbox',
    ]);
  });

  // NFR-010, plan section 14: body text 4.5:1, interface controls 3:1, in
  // every theme, for every pair of colours the shell's CSS puts together.
  it.each(PALETTES.map((p) => [p.theme, p] as const))(
    'NFR-010 %s reaches the contrast of every pair',
    (_, palette) => {
      const colours = shellColors(palette);
      const failures = CONTRAST_PAIRS.filter(
        (pair) => contrast(colours[pair.fg], colours[pair.bg]) < pair.least,
      ).map(
        (pair) =>
          `${pair.what}: ${contrast(colours[pair.fg], colours[pair.bg]).toFixed(2)} < ${pair.least}`,
      );

      expect(failures).toEqual([]);
    },
  );

  it('computes the WCAG ratio', () => {
    expect(contrast('#000000', '#ffffff')).toBeCloseTo(21);
    expect(contrast('#777777', '#777777')).toBeCloseTo(1);
  });

  it.each(PALETTES.map((p) => [p.theme, p] as const))(
    'applies %s as CSS variables with its color-scheme',
    (_, palette) => {
      const root = document.createElement('html');

      applyPalette(palette, root);

      expect(root.style.getPropertyValue('--ui-surface')).toBe(palette.panel);
      expect(root.style.getPropertyValue('--ui-text')).toBe(palette.foreground);
      expect(root.style.getPropertyValue('--ui-chosen-text')).toBe(palette.chosenText);
      expect(root.style.getPropertyValue('--palette-code-background')).toBe(palette.codeBackground);
      expect(root.style.getPropertyValue('--palette-mode-colors-3')).toBe(palette.modeColors[3]);
      expect(root.dataset.scheme).toBe(palette.dark ? 'dark' : 'light');
      expect(root.dataset.theme).toBe(palette.theme);
    },
  );

  // F15 (M-18.2, M-18.8, M-04.5): egui's fixed todo colours as text on the
  // bars, in fields and on the chosen cell's neighbours, in every theme.
  it.each(PALETTES.map((p) => [p.theme, p] as const))(
    'NFR-010 %s reaches 4.5:1 with the todo colours',
    (_, palette) => {
      const colours = TODO_COLORS[palette.dark ? 'dark' : 'light'];
      const failures = Object.entries(colours).flatMap(([role, colour]) =>
        (['panel', 'background'] as const)
          .filter((bg) => contrast(colour, palette[bg]) < 4.5)
          .map((bg) => `${role} on ${bg}: ${contrast(colour, palette[bg]).toFixed(2)}`),
      );

      expect(failures).toEqual([]);
      const root = document.createElement('html');
      applyPalette(palette, root);
      expect(root.style.getPropertyValue('--ui-urgency-critical')).toBe(colours.urgencyCritical);
    },
  );

  // Review F15: the weak texts of the todos (`--todo-weak`, 35 % hint, 65 %
  // text in sRGB) reach 4.5:1 on the bars and in fields of every theme.
  it.each(PALETTES.map((p) => [p.theme, p] as const))(
    'NFR-010 %s reaches 4.5:1 with the weak texts of the todos',
    (_, palette) => {
      const channel = (hex: string, at: number) => parseInt(hex.slice(at, at + 2), 16);
      const mixed =
        '#' +
        [1, 3, 5]
          .map((at) =>
            Math.round(channel(palette.weak, at) * 0.35 + channel(palette.foreground, at) * 0.65)
              .toString(16)
              .padStart(2, '0'),
          )
          .join('');

      expect(contrast(mixed, palette.panel)).toBeGreaterThanOrEqual(4.5);
      expect(contrast(mixed, palette.background)).toBeGreaterThanOrEqual(4.5);
    },
  );
});

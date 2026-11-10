import { describe, expect, it } from 'vitest';
import {
  FOUND_OFF,
  FOUND_ON,
  INSET,
  MAX_SCALE,
  WIDTH,
  barRect,
  barsOf,
  hitRects,
  hitsShown,
  marksOf,
  minimapShown,
  scaleOf,
  scrollFor,
  visibleRect,
} from './minimap';

describe('M-12 minimap', () => {
  it('M-12.1 is 80 px wide and hidden below 240 px; the scale fits the note, at most 0.2', () => {
    expect(WIDTH).toBe(80);
    expect(minimapShown(true, 240)).toBe(true);
    expect(minimapShown(true, 239)).toBe(false);
    expect(minimapShown(false, 2000)).toBe(false);
    expect(scaleOf(800, 80_000)).toBe(0.01);
    expect(scaleOf(800, 1_000)).toBe(MAX_SCALE);
  });

  it('M-12.2 one bar per line, as long as the line, headings strong; empty lines none', () => {
    const marks = marksOf(
      [
        { top: 0, height: 20, chars: 40, heading: true },
        { top: 20, height: 20, chars: 0, heading: false },
        { top: 40, height: 40, chars: 200, heading: false },
      ],
      80,
    );
    expect(marks).toEqual([
      { top: 0, height: 14, width: 0.5, strong: true },
      { top: 40, height: 28, width: 1, strong: false },
    ]);
  });

  it('M-12.2 a pixel row shows its widest bar; a heading there makes it strong', () => {
    const bars = barsOf(
      [
        { top: 0, height: 5, width: 0.3, strong: true },
        { top: 0, height: 5, width: 0.8, strong: false },
        { top: 10, height: 5, width: 0.9, strong: false },
        { top: 10, height: 5, width: 0.2, strong: true },
      ],
      0.2,
      4,
    );
    expect(bars[0]).toEqual({ width: 0.8, strong: false });
    expect(bars[2]).toEqual({ width: 0.9, strong: true });
    expect(bars[1]).toBeNull();
    expect(barRect(0, { width: 0.01, strong: false }, WIDTH).width).toBeCloseTo(
      (WIDTH - 2 * INSET) * 0.05,
    );
  });

  it('M-12.3 the part in view is at least 4 px; a click centres the view, within the note', () => {
    expect(visibleRect(100, 101, 0.1, WIDTH)).toEqual({ x: 0, y: 10, width: WIDTH, height: 4 });
    expect(scrollFor(50, 0.1, [0, 200], 2000)).toBe(400);
    expect(scrollFor(0, 0.1, [0, 200], 2000)).toBe(0);
    expect(scrollFor(199, 0.1, [0, 200], 2000)).toBe(1800);
  });

  it('M-12.4 fake hits shine through at their word, at least 3 by 2 px, once per pixel', () => {
    const rects = hitRects(
      [
        { top: 100, height: 10, left: 0.5, width: 0.01 },
        { top: 100.2, height: 10, left: 0.5, width: 0.01 },
        { top: 500, height: 10, left: 0, width: 0.5 },
      ],
      0.1,
      WIDTH,
    );
    expect(rects).toHaveLength(2);
    expect(rects[0].core).toEqual({ x: INSET + 34, y: 10, width: 3, height: 2 });
    expect(rects[0].glow).toEqual({ x: INSET + 31.5, y: 7.5, width: 8, height: 7 });
    expect(rects[1].core.width).toBe(34);
  });

  it('M-12.4 hits blink 0.6 s on, 0.4 s off, unless blinking is off', () => {
    expect(FOUND_ON + FOUND_OFF).toBe(1000);
    expect(hitsShown(true, 100)).toBe(true);
    expect(hitsShown(true, 650)).toBe(false);
    expect(hitsShown(true, 1050)).toBe(true);
    expect(hitsShown(false, 650)).toBe(true);
  });
});

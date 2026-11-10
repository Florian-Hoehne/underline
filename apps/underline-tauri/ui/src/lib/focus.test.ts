import { describe, expect, it } from 'vitest';
import { nextRegion } from './focus';

describe('NFR-011 F6 between regions', () => {
  it('goes on and round, back with Shift, and starts at an end from outside', () => {
    expect(nextRegion(4, 0, false)).toBe(1);
    expect(nextRegion(4, 3, false)).toBe(0);
    expect(nextRegion(4, 0, true)).toBe(3);
    expect(nextRegion(4, -1, false)).toBe(0);
    expect(nextRegion(4, -1, true)).toBe(3);
    expect(nextRegion(0, -1, false)).toBe(-1);
  });
});

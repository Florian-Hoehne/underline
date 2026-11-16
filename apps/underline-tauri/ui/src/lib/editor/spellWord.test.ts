// UC-005 A28, TC-005-39: the word a mark shows and the change a correction makes.

import { describe, expect, it } from 'vitest';
import { correctionOf, shownWordOf } from './spellWord';

describe('spell corrections (UC-005 A28)', () => {
  it('tc005_39 replaces only the letters that differ', () => {
    const shown = shownWordOf('ferri', 4);
    expect(shown.word).toBe('ferri');
    expect(correctionOf(shown, 'ferry')).toEqual({ from: 8, to: 9, text: 'y' });
  });

  it('keeps inline markup inside the word', () => {
    const shown = shownWordOf('Under**lne', 0);
    expect(shown.word).toBe('Underlne');
    expect(correctionOf(shown, 'Underline')).toEqual({ from: 8, to: 8, text: 'i' });
  });

  it('replaces the whole word when nothing is shared', () => {
    expect(correctionOf(shownWordOf('teh', 10), 'the')).toEqual({ from: 11, to: 13, text: 'he' });
    expect(correctionOf(shownWordOf('abc', 0), 'xyz')).toEqual({ from: 0, to: 3, text: 'xyz' });
  });

  it('does not split a surrogate pair', () => {
    const change = correctionOf(shownWordOf('a😀', 0), 'a😃');
    expect(change).toEqual({ from: 1, to: 3, text: '😃' });
  });

  it('changes nothing for the same word', () => {
    expect(correctionOf(shownWordOf('word', 0), 'word')).toBeNull();
  });

  // Test monkey 8b: the mark of `[Under](http://x.example/a)lne` covers the
  // link's address, which no word shows; the correction must keep it.
  it('tc005_39 keeps the address of a link inside a word', () => {
    const markup = 'Under](http://x.example/a)lne';
    const shown = shownWordOf(markup, 0);
    expect(shown.word).toBe('Underlne');
    const change = correctionOf(shown, 'Underline');
    expect(change).not.toBeNull();
    const result = markup.slice(0, change!.from) + change!.text + markup.slice(change!.to);
    expect(result).toBe('Under](http://x.example/a)line');
  });

  // Test monkey 8b: markup between the changed letters is dropped, which
  // leaves its closing half behind (`te**h**` becomes `the**`).
  it('tc005_39 keeps the markup inside the changed letters', () => {
    const markup = 'te**h';
    const change = correctionOf(shownWordOf(markup, 0), 'the');
    expect(change).not.toBeNull();
    const result = markup.slice(0, change!.from) + change!.text + markup.slice(change!.to);
    expect(result).toContain('**');
  });

  // Test monkey 8b: a reference link `[Under][ref]lne` shows "Underlne".
  it('tc005_39 does not show the label of a reference link as part of the word', () => {
    expect(shownWordOf('Under][ref]lne', 0).word).toBe('Underlne');
  });
});

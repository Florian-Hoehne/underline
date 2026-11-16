// TC-005-40: the mock's rough copy of the skip rules of UC-005 BR-023, so
// the browser shows what the backend would mark.

import { describe, expect, it } from 'vitest';
import { checkedText, isCheckedWord, MockSpeller, SPELL_TEXTS } from './mock-spell';

describe('the mock spell check', () => {
  const speller = new MockSpeller(Object.values(SPELL_TEXTS));
  const words = (text: string, asciidoc = false) =>
    speller.marks(text, asciidoc).map(([from, to]) => text.slice(from, to));

  it('tc005_40 leaves out code, addresses, tags, capitals, digits, formulas and names', () => {
    expect(words(SPELL_TEXTS['Mixed-Spell'])).toEqual(['offen', 'Strasse', 'fehlr']);
    expect(isCheckedWord('HTTPX')).toBe(false);
    expect(isCheckedWord('abc123')).toBe(false);
    expect(checkedText('see https://x.org/a and #tag', false)).toBe('see                 and     ');
  });

  it('tc005_39 marks "ferri", suggests "ferry", and a word added is known', () => {
    expect(words(SPELL_TEXTS.Spell)).toEqual(['ferri']);
    expect(speller.suggest('ferri')).toContain('ferry');
    const own = new MockSpeller([]);
    expect(own.add('two words')).toBe(
      '"two words" holds a space and cannot be added to the dictionary',
    );
    expect(own.add('Kubernetes')).toBeNull();
    expect(own.knows('Kubernetes')).toBe(true);
  });

  it('asciidoc: listings and attribute names are left out', () => {
    const marked = words('= Note\n:statsu: here\n\n----\nfehlr\n----\nwordd\n', true);
    expect(marked).toEqual(['wordd']);
  });
});

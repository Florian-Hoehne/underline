// TC-005-40: the mock's rough copy of the skip rules of UC-005 BR-023, so
// the browser shows what the backend would mark.

import { describe, expect, it } from 'vitest';
import {
  checkedText,
  isCheckedWord,
  MockSpeller,
  noteLanguage,
  offeredLanguage,
  SPELL_TEXTS,
} from './mock-spell';

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

  it("oq208: a word added to the workspace dictionary is known, as the user's", () => {
    const own = new MockSpeller([]);
    expect(own.knows('Zettelkasten')).toBe(false);
    expect(own.add('Zettelkasten', 'workspace')).toBeNull();
    expect(own.knows('Zettelkasten')).toBe(true);
    expect([...own.workspace]).toEqual(['Zettelkasten']);
    expect([...own.user]).toEqual([]);
    expect(own.add('two words', 'workspace')).not.toBeNull();
  });

  it('oq207: a note names its language with `lang`, in any spelling of the code', () => {
    expect(noteLanguage('---\nlang: de-CH\n---\n# A\n', false)).toBe('de-CH');
    expect(noteLanguage("---\nlang: 'it_IT'\n---\n", false)).toBe('it_IT');
    expect(noteLanguage('# A\n\nlang: de-CH\n', false)).toBeNull();
    expect(noteLanguage('---\nlang:\n---\n', false)).toBeNull();
    expect(noteLanguage('= A\n:lang: de-ch\n\nText\n', true)).toBe('de-ch');
    expect(noteLanguage('= A\n\n:lang: de-ch\n', true)).toBeNull();
    expect(offeredLanguage('de_CH')).toBe('de-CH');
    expect(offeredLanguage('de-ch')).toBe('de-CH');
    expect(offeredLanguage('IT-it')).toBe('it-IT');
    expect(offeredLanguage('de-CH-1996')).toBe('de-CH');
    expect(offeredLanguage('gsw')).toBe('de-CH');
    expect(offeredLanguage('de')).toBe('de-DE');
    expect(offeredLanguage('it')).toBe('it-IT');
    expect(offeredLanguage('en-AU')).toBeNull();
    expect(noteLanguage('---\nlang: null\n---\n', false)).toBeNull();
    expect(noteLanguage('---\nlang: ~\n---\n', false)).toBeNull();
    expect(noteLanguage('---\nlang: off\n---\n', false)).toBeNull();
    expect(noteLanguage('= A\n:lang: de-CH\n:lang: it\n\nText\n', true)).toBe('it');
    expect(noteLanguage(`---\nlang: ${'x'.repeat(100)}\n---\n`, false)).toBe('x'.repeat(34) + '…');
    expect(offeredLanguage('off')).toBeNull();
    expect(offeredLanguage('pt-BR')).toBeNull();
  });

  it('oq207: the value of `lang` is a code, never marked', () => {
    expect(words('---\nlang: xx-YY\n---\nwordd\n')).toEqual(['wordd']);
    expect(words('= Note\n:lang: xx-YY\n\nwordd\n', true)).toEqual(['wordd']);
  });
});

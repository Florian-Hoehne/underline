// FR-099, FR-100, UC-005 BR-023, A28 (ADR-030): a stand-in for the spell
// check of the Rust backend (`underline-spell`), so the marks, the spell
// menu, "Add to dictionary" and "Add to workspace dictionary" (OQ-208) can
// be tried in a plain browser and in Vitest (TC-005-39, TC-005-40). A
// note's attribute `lang` overrides the language (OQ-207).
//
// The mock knows one language, English (US): every word of the mock's own
// notes and a few common ones, except the misspelled words of the spell
// workspace. Its skip rules are a rough copy of BR-023 for Markdown and
// AsciiDoc; the real rules are tested in Rust (`words.rs`). Other languages
// have no dictionary here, as when a dictionary is missing.

import { SPELL_LANGUAGES } from './spellLanguages';

export { SPELL_LANGUAGES };

/** Package 8b: the workspace of the spell check, with the notes of TC-005-39 and TC-005-40. */
export const SPELL = '/home/user/Spell';

/** The texts of the notes of `SPELL`, by note (`Mixed.md` of TC-005-40 is `Mixed-Spell`). */
export const SPELL_TEXTS: Readonly<Record<string, string>> = {
  Spell: '# Spell\n\nThe ferri leaves\n\nUnderline\n',
  Other: '# Other\n\nKubernetes runs here, and Kubernetes again.\n',
  'Mixed-Spell':
    '---\nstatsu: offen\n---\n# Mixed\n\nStrasse and fehlr, `fehlr`, [Link](https://exampel.org), ' +
    '#taggg, HTTPX, abc123, $fehlr$.\n\n```\nfehlr in code\n```\n',
  // OQ-207: a note naming its own language, which the mock has no dictionary for.
  'Swiss-Spell': '---\nlang: de_CH\n---\n# Swiss\n\nStrasse und ferri\n',
};

/** The words the mock's dictionary does not know, though its notes hold them. */
const MISSPELLED: ReadonlySet<string> = new Set([
  'ferri',
  'Kubernetes',
  'fehlr',
  'statsu',
  'offen',
  'Strasse',
  'taggg',
  'exampel',
]);

/** Words to type with, besides those of the notes. */
const COMMON = `a about after all also an and any are as at be because been but by can
could day do even first for from get give go good have he her here him his how i if in
into it its just know like look make me more most my new no not now of on one only or
other our out over people say see she so some take than that the their them then there
these they think this time to two up us use very want way we well what when which who
will with work would year you your ferry leaves berry fern note notes text word words
spell check dictionary mark marks wrong right test tests typed typing write written line`;

/** The language the mock has a dictionary for. */
export const MOCK_SPELL_LANGUAGE = 'en-US';

/** OQ-207: the most characters of a language not offered that are shown. */
export const MAX_SHOWN_TAG = 35;

/**
 * OQ-207: the language `text` names with its attribute `lang` (front matter
 * `lang: de-CH`, AsciiDoc `:lang: de-CH`, the last one counting), as
 * written, on one line and cut to `MAX_SHOWN_TAG` characters; `null` when it
 * names none (no value, `null`, `~`, `off`). Only the head counts, as in the
 * backend (`note_language`).
 */
export function noteLanguage(text: string, asciidoc: boolean): string | null {
  const front = asciidoc ? null : /^---\n([\s\S]*?)\n---(?:\n|$)/.exec(text);
  const values = asciidoc
    ? [...text.split(/\n\s*\n/, 1)[0].matchAll(/^:lang:(.*)$/gim)].map((match) => match[1])
    : [...(front?.[1] ?? '').matchAll(/^lang:(.*)$/gm)].map((match) => match[1]);
  const tag = (values.at(-1) ?? '').trim().replace(/^(['"])(.*)\1$/, '$2');
  if (tag === '' || tag === '~' || /^(null|off)$/i.test(tag) || tag === '[]') {
    return null;
  }
  const chars = [...tag];
  return chars.length <= MAX_SHOWN_TAG ? tag : chars.slice(0, MAX_SHOWN_TAG - 1).join('') + '…';
}

/**
 * OQ-207: the code of the offered language a tag names, as the backend's
 * `SpellCheckLanguage::of_tag` reads it: `de-CH`, `de_CH`, `de-ch`,
 * `de-CH-1996`, `gsw`; a language without a region (`de`, `it`); `null` for
 * any other.
 */
export function offeredLanguage(tag: string): string | null {
  const [language = '', ...rest] = tag.trim().toLowerCase().split(/[-_]/);
  const region = rest.find((part) => /^[a-z]{2}$/.test(part));
  if (language === 'en') {
    return region === undefined || region === 'us' ? 'en-US' : region === 'gb' ? 'en-GB' : null;
  }
  if (language === 'gsw' || (language === 'de' && (region === 'ch' || region === 'li'))) {
    return 'de-CH';
  }
  const any: Record<string, string> = { de: 'de-DE', fr: 'fr-FR', it: 'it-IT', es: 'es-ES' };
  return any[language] ?? null;
}

/** The name of the language `code`, as the settings window shows it. */
export function spellLanguageName(code: string): string {
  return SPELL_LANGUAGES.find(([c]) => c === code)?.[1] ?? code;
}

/** A word of a text, `[from, to]` in UTF-16 units. */
export type MockMark = [number, number];

/** `text` with `pattern`'s matches turned into spaces, so the places stay. */
function blank(text: string, pattern: RegExp): string {
  return text.replace(pattern, (match: string) => ' '.repeat(match.length));
}

/** BR-023, roughly: the text with what is not checked turned into spaces. */
export function checkedText(text: string, asciidoc: boolean): string {
  let out = text;
  if (asciidoc) {
    // Listing and passthrough blocks, attribute names, macros' targets.
    out = blank(out, /^(----|\+\+\+\+|\.\.\.\.)\n[\s\S]*?\n\1$/gm);
    out = blank(out, /^:lang:.*$/gim);
    out = blank(out, /^:[^:\n]+:/gm);
    out = blank(out, /\b(?:stem|latexmath|asciimath|link|xref|image):\S*?\[[^\]]*\]/g);
  } else {
    // The names of the front matter, fenced code, display formulas.
    const front = /^---\n([\s\S]*?)\n---(?:\n|$)/.exec(out);
    if (front) {
      // OQ-207: the value of `lang` is a language code, never checked.
      const head = blank(front[0], /^lang:.*$/gm);
      const names = head.replace(/^([^\s:#][^:\n]*):/gm, (name) => ' '.repeat(name.length));
      out = names.replace(/^---|---$/gm, '   ') + out.slice(front[0].length);
      out = out.replace(/tags:.*$/m, (line) => ' '.repeat(line.length));
    }
    out = blank(out, /^(```|~~~)[^\n]*\n[\s\S]*?\n\1[ \t]*$/gm);
    out = blank(out, /\$\$[\s\S]*?\$\$/g);
    out = blank(out, /\$[^$\n]+\$/g);
    out = blank(out, /\]\([^)]*\)/g);
    out = out.replace(
      /(^|[^\p{L}\p{N}&])(#[\p{L}\p{N}_/-]+)/gu,
      (_match: string, lead: string, tag: string) => lead + ' '.repeat(tag.length),
    );
  }
  out = blank(out, /`[^`\n]*`/g);
  out = blank(out, /<[^>\s]+>/g);
  out = blank(out, /\S*:\/\/\S*|\bwww\.\S+|\S+@\S+\.\S+/g);
  return out;
}

/** BR-023: whether the spell check looks at `word`: no digit, not all capitals. */
export function isCheckedWord(word: string): boolean {
  if (/\p{N}/u.test(word) || word.length > 100) {
    return false;
  }
  const letters = [...word].filter((c) => /\p{L}/u.test(c));
  return (
    letters.length > 0 && !letters.every((c) => c === c.toUpperCase() && c !== c.toLowerCase())
  );
}

/** The words of `text`, each with its place. */
function wordsOf(text: string): { word: string; from: number }[] {
  const words: { word: string; from: number }[] = [];
  for (const match of text.matchAll(/[\p{L}\p{M}\p{N}_]+(?:['’][\p{L}\p{M}]+)*/gu)) {
    words.push({ word: match[0], from: match.index });
  }
  return words;
}

/** How many single edits turn `a` into `b`, counting to 2. */
function edits(a: string, b: string): number {
  if (Math.abs(a.length - b.length) > 1) {
    return 2;
  }
  let before = Array.from({ length: b.length + 1 }, (_, j) => j);
  let previous = before.slice();
  for (let i = 1; i <= a.length; i++) {
    const row = [i];
    for (let j = 1; j <= b.length; j++) {
      const cost = a[i - 1] === b[j - 1] ? 0 : 1;
      row[j] = Math.min(previous[j] + 1, row[j - 1] + 1, previous[j - 1] + cost);
      if (i > 1 && j > 1 && a[i - 1] === b[j - 2] && a[i - 2] === b[j - 1]) {
        row[j] = Math.min(row[j], before[j - 2] + 1);
      }
    }
    before = previous;
    previous = row;
  }
  return Math.min(previous[b.length], 2);
}

/** The mock's checker for English (US) and the user's words. */
export class MockSpeller {
  readonly #known = new Set<string>();
  /** UC-005 A28: the user's words (entity DICTIONARY_WORD), sorted when listed. */
  readonly user = new Set<string>();
  /** OQ-208: the words of the workspace dictionary. */
  readonly workspace = new Set<string>();

  /** A checker knowing every word of `texts` but the misspelled ones. */
  constructor(texts: readonly string[]) {
    for (const word of COMMON.split(/\s+/)) {
      this.#known.add(word);
    }
    for (const text of texts) {
      for (const { word } of wordsOf(text)) {
        if (!MISSPELLED.has(word)) {
          this.#known.add(word);
          this.#known.add(word.toLowerCase());
        }
      }
    }
  }

  /** Whether `word` is known, also with a capital at the start. */
  knows(word: string): boolean {
    const lower = word.charAt(0).toLowerCase() + word.slice(1);
    return [this.#known, this.user, this.workspace].some(
      (words) => words.has(word) || words.has(lower),
    );
  }

  /** UC-005 BR-023: the marked words of `text`, sorted. */
  marks(text: string, asciidoc = false): MockMark[] {
    const checked = checkedText(text, asciidoc);
    return wordsOf(checked)
      .filter(({ word }) => isCheckedWord(word) && !this.knows(word))
      .map(({ word, from }) => [from, from + word.length]);
  }

  /** UC-005 A28: up to five known words one edit away from `word`. */
  suggest(word: string): string[] {
    const lower = word.charAt(0).toLowerCase() + word.slice(1);
    return [...this.#known]
      .filter((known) => edits(lower, known) === 1)
      .sort()
      .slice(0, 5);
  }

  /**
   * UC-005 A28 step 2: adds `word` to the user's words, or to the
   * workspace's (OQ-208); the reason when it cannot be added, as the
   * backend words it.
   */
  add(word: string, into: 'user' | 'workspace' = 'user'): string | null {
    const trimmed = word.trim();
    if (trimmed === '') {
      return 'There is no word to add';
    }
    if (/\s/.test(trimmed)) {
      return `"${trimmed}" holds a space and cannot be added to the dictionary`;
    }
    if ([...trimmed].length > 100) {
      return 'A word in the dictionary has at most 100 characters';
    }
    (into === 'user' ? this.user : this.workspace).add(trimmed);
    return null;
  }
}

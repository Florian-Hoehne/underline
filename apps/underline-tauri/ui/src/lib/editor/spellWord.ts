// UC-005 A28 (ADR-030): the word a spell mark stands for, and the change a
// correction makes. A mark covers the markup of a word, which may hold
// inline markup (`**Under**lne`, `[Under](http://…)lne`, `Under][ref]lne`);
// the menu shows and the dictionary is asked for the word as the facade
// shows it, and a correction changes only the characters that differ,
// keeping every piece of markup inside the word where it was.

/**
 * A marked word as shown, with the markup offset (web units) of each of its
 * units and of its end, and the markup it stands in (from `from` on), so a
 * correction keeps what lies between its units.
 */
export type ShownWord = { word: string; places: number[]; markup?: string; from?: number };

/** Characters of inline markup that are never part of a word as shown. */
const MARKUP = new Set(['*', '`', '~', '=', '^', '+', '\\', '#', '[', ']']);

/**
 * The end of the markup at `at` that a word does not show: a link's
 * address `](…)`, a reference `][…]`; `at` when there is none.
 */
function hiddenEnd(markup: string, at: number): number {
  if (markup.startsWith('](', at)) {
    let depth = 0;
    for (let i = at + 1; i < markup.length; i++) {
      if (markup[i] === '(') depth++;
      if (markup[i] === ')' && --depth === 0) return i + 1;
    }
    return markup.length;
  }
  if (markup.startsWith('][', at)) {
    const close = markup.indexOf(']', at + 2);
    return close < 0 ? markup.length : close + 1;
  }
  return at;
}

/**
 * The word the markup `markup`, starting at the web offset `from`, shows:
 * without inline markup, link addresses and references.
 */
export function shownWordOf(markup: string, from: number): ShownWord {
  let word = '';
  const places: number[] = [];
  let at = 0;
  while (at < markup.length) {
    const skip = hiddenEnd(markup, at);
    if (skip > at) {
      at = skip;
      continue;
    }
    if (!MARKUP.has(markup[at])) {
      word += markup[at];
      places.push(from + at);
    }
    at++;
  }
  places.push(from + markup.length);
  return { word, places, markup, from };
}

/** A change of the markup: `from`..`to` (web units) becomes `text`. */
export type Correction = { from: number; to: number; text: string };

/** Whether unit `at` of `text` is the second half of a surrogate pair. */
const low = (text: string, at: number) =>
  at > 0 && at < text.length && /[\uDC00-\uDFFF]/.test(text[at]);

/**
 * UC-005 A28 step 2: the change that turns the shown word into
 * `suggestion`: the units both share at the start and the end stay; the
 * units between are replaced where they stand, the markup between them
 * kept (`te**h` becomes `th**e`). `null` when nothing changes.
 */
export function correctionOf(shown: ShownWord, suggestion: string): Correction | null {
  const { word, places } = shown;
  if (word === suggestion || places.length !== word.length + 1) {
    return null;
  }
  let start = 0;
  while (start < word.length && start < suggestion.length && word[start] === suggestion[start]) {
    start++;
  }
  let end = 0;
  while (
    end < word.length - start &&
    end < suggestion.length - start &&
    word[word.length - 1 - end] === suggestion[suggestion.length - 1 - end]
  ) {
    end++;
  }
  // A unit of a surrogate pair is not split.
  while (start > 0 && (low(word, start) || low(suggestion, start))) {
    start--;
  }
  while (end > 0 && (low(word, word.length - end) || low(suggestion, suggestion.length - end))) {
    end--;
  }
  const lastChanged = word.length - end;
  const middle = suggestion.slice(start, suggestion.length - end);
  const from = places[start];
  if (lastChanged <= start) {
    // Units are only added.
    return { from, to: from, text: middle };
  }
  const to = places[lastChanged - 1] + 1;
  if (shown.markup === undefined || shown.from === undefined) {
    return { from, to, text: middle };
  }
  // The markup between the changed units stays: each shown character takes
  // the next of the new ones, the last takes the rest.
  const replaced = shown.markup.slice(from - shown.from, to - shown.from);
  const shownAt = new Set(places.slice(start, lastChanged).map((at) => at - from));
  const fresh = Array.from(middle);
  const slots = Array.from(word.slice(start, lastChanged)).length;
  let text = '';
  let slot = 0;
  for (let at = 0; at < replaced.length; at++) {
    if (!shownAt.has(at)) {
      text += replaced[at];
      continue;
    }
    if (low(replaced, at + 1) && shownAt.has(at + 1)) {
      // The first half of a pair: the character counts once.
      continue;
    }
    slot++;
    text += slot < slots ? (fresh.shift() ?? '') : fresh.splice(0).join('');
  }
  return { from, to, text };
}

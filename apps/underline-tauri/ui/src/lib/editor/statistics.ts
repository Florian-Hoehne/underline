// UC-004 BR-020 (FR-090, M-16.2): the text statistics of the active note
// for the note properties of the tool panel. The editor core in the webview
// counts them on the text it owns (ADR-025), so no key waits for the
// backend; the core keeps its answer for an unchanged text and selection,
// so the panel may ask every `STATISTICS_EVERY_MS` and the statistics follow
// every change within a second.

import type { TextStatisticsView } from '../bindings';
import { coreOf } from './sync';

/** How often the tool panel asks for the statistics, in ms (BR-020: within 1 second). */
export const STATISTICS_EVERY_MS = 400;

/** The statistics of `note` now; `null` while its core is not open or failed. */
export function statisticsOf(note: string): TextStatisticsView | null {
  const tab = coreOf(note);
  if (!tab?.alive) {
    return null;
  }
  try {
    return tab.core.textStatistics();
  } catch (error) {
    console.error('The text statistics could not be counted', error);
    return null;
  }
}

/** Whether `a` and `b` say the same. */
export function sameStatistics(
  a: TextStatisticsView | null,
  b: TextStatisticsView | null,
): boolean {
  return (
    a === b ||
    (a !== null &&
      b !== null &&
      a.words === b.words &&
      a.characters === b.characters &&
      a.readingMinutes === b.readingMinutes &&
      a.selection?.words === b.selection?.words &&
      a.selection?.characters === b.selection?.characters)
  );
}

/** A count as the panel writes it: `1,234`. */
function count(value: number): string {
  return value.toLocaleString('en-US');
}

/** What the note properties show of `stats`. */
export type StatisticsTexts = {
  words: string;
  characters: string;
  /** "1 minute", "3 minutes"; "0 minutes" without words. */
  reading: string;
  /**
   * The rows "Selected words" and "Selected characters"; `null` while
   * nothing is selected. Two short rows fit a narrow panel (UX review).
   */
  selection: { words: string; characters: string } | null;
};

/** UC-004 BR-020: the texts of the note properties for `stats`. */
export function statisticsTexts(stats: TextStatisticsView): StatisticsTexts {
  const minutes = stats.readingMinutes;
  return {
    words: count(stats.words),
    characters: count(stats.characters),
    reading: `${count(minutes)} ${minutes === 1 ? 'minute' : 'minutes'}`,
    selection:
      stats.selection === null
        ? null
        : { words: count(stats.selection.words), characters: count(stats.selection.characters) },
  };
}

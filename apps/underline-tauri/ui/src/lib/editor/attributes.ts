// UC-024 Edit Note Attributes (FR-093) and the tags of the note (FR-091,
// UC-004 BR-016) for the note properties of the tool panel. The editor core
// in the webview reads them from the text it owns (ADR-025), so the panel
// follows every change at once (A7: also one made outside, once the core
// has it); it keeps its answer for an unchanged text, so the panel may ask
// every `HEAD_EVERY_MS`. A change goes into the text through the view shown,
// as one undo step (BR-006), like the toolbar's edits; the core says why
// it writes nothing (A3 to A6). No rule of the attributes lives here.

import type {
  AttributeChangeView,
  AttributeKindName,
  AttributeValueView,
  NoteHeadView,
} from '../bindings';
import { activeFacade } from '../facade/active';
import { activeSourceView } from './active';
import type { Refusal } from './refusal';
import { coreOf } from './sync';

/** How often the tool panel asks for the attributes, in ms (as the statistics). */
export const HEAD_EVERY_MS = 400;

/** How long "Show in source view" waits for the source view, in ms. */
const SOURCE_WAITS_MS = 3000;

/** Says that the note is not shown in a view that could take a change. */
export const NOT_SHOWN = 'Show the note to change its attributes.';

/** The attributes and tags of `note` now; `null` while its core is not open or failed. */
export function headOf(note: string): NoteHeadView | null {
  const tab = coreOf(note);
  if (!tab?.alive) {
    return null;
  }
  try {
    return tab.core.noteHead();
  } catch (error) {
    console.error('The note attributes could not be read', error);
    return null;
  }
}

/** Whether `a` and `b` say the same. */
export function sameHead(a: NoteHeadView | null, b: NoteHeadView | null): boolean {
  return a === b || (a !== null && b !== null && JSON.stringify(a) === JSON.stringify(b));
}

/** What a view showing a note does for a change of its attributes. */
interface AttributeTarget {
  readonly note: string;
  attribute(change: AttributeChangeView): Promise<Refusal | null>;
  undo(): void;
}

/** The view showing `note` now, if any. */
function viewOf(note: string): AttributeTarget | null {
  const facade = activeFacade();
  if (facade?.note === note) {
    return facade;
  }
  const source = activeSourceView();
  return source?.note === note ? source : null;
}

/**
 * UC-024 step 4: writes `change` into `note` through the view showing it;
 * resolves with why nothing was written and what about (A3 to A6), else
 * `null`.
 */
export async function changeAttribute(
  note: string,
  change: AttributeChangeView,
): Promise<Refusal | null> {
  const view = viewOf(note);
  if (!view) {
    return { problem: NOT_SHOWN, about: 'note' };
  }
  try {
    return await view.attribute(change);
  } catch (error) {
    return { problem: error instanceof Error ? error.message : String(error), about: 'note' };
  }
}

/**
 * UC-024 BR-006: undoes the last change of `note` in the view showing it,
 * as Ctrl+Z there does; false when no view shows it.
 */
export function undoIn(note: string): boolean {
  const view = viewOf(note);
  view?.undo();
  return view !== null;
}

/** The kinds of values a new attribute may have, with their names (BR-002). */
export const KINDS: readonly { kind: AttributeKindName; label: string }[] = [
  { kind: 'text', label: 'Text' },
  { kind: 'list', label: 'List' },
  { kind: 'number', label: 'Number' },
  { kind: 'date', label: 'Date' },
  { kind: 'yesNo', label: 'Yes or no' },
];

/** BR-002: the kinds an AsciiDoc note holds. */
export function kindsFor(asciidoc: boolean): readonly { kind: AttributeKindName; label: string }[] {
  return asciidoc ? KINDS.filter((k) => k.kind === 'text' || k.kind === 'list') : KINDS;
}

/**
 * The value of kind `kind` as typed in one field: a list's items at
 * commas, a switch's `true`.
 */
export function valueOf(kind: AttributeKindName, typed: string): AttributeValueView {
  switch (kind) {
    case 'list':
      return {
        kind: 'list',
        items: typed
          .split(',')
          .map((item) => item.trim())
          .filter((item) => item !== ''),
      };
    case 'number':
      return { kind: 'number', text: typed };
    case 'date':
      return { kind: 'date', text: typed };
    case 'yesNo':
      return { kind: 'yesNo', yes: typed === 'true' };
    default:
      return { kind: 'text', text: typed };
  }
}

/**
 * UC-024 A5, A6: shows the line at `offset` of `note` in the source view:
 * the tab switches to it (`setView`), and the cursor goes there once it
 * shows.
 */
export async function showInSource(
  note: string,
  offset: number,
  setView: (note: string, view: 'source') => Promise<void>,
): Promise<void> {
  const source = activeSourceView();
  if (source?.note === note) {
    source.jump(offset);
    return;
  }
  await setView(note, 'source');
  const until = performance.now() + SOURCE_WAITS_MS;
  await new Promise<void>((resolve) => {
    const look = () => {
      const shown = activeSourceView();
      if (shown?.note === note) {
        shown.jump(offset);
        resolve();
      } else if (performance.now() > until) {
        resolve();
      } else {
        setTimeout(look, 30);
      }
    };
    look();
  });
}

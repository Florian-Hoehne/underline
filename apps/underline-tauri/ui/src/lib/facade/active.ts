// The facade shown now in each pane of the split view, for the window (the switch of the view, the
// toolbar) and for what jumps in it: the tool panel's outline reveals a
// block (M-09.16, M-16.5), a link to a heading puts the cursor there
// (M-07.3). Its blocks come from the window's editor
// core (ADR-025), not from the backend's events.

import type {
  AttributeChangeView,
  EditActionName,
  FindAnswer,
  FindSpec,
  FindStatus,
  FindStep,
  Insertion,
  LineTargetView,
} from '../bindings';

import type { PaneName } from '../bindings';
import type { Refusal } from '../editor/refusal';
import { focusedPane } from '../paneFocus';

/** UC-005 A20, A21: what replacing in the find field did. */
export type ReplaceDone = { replaced: number; status: FindStatus };

/** M-10.5: where the cursor stood (web units) in the text of a hash. */
export type CursorMark = { head: number; hash: string };

/** What the window asks of the facade shown. */
export interface ActiveFacade {
  readonly note: string;
  /** M-09.16: scrolls the block at the markup offset `offset` to the top, without moving the cursor. */
  reveal(offset: number): void;
  /**
   * M-07.3: the cursor to the markup offset `offset`, shown, with the
   * keyboard; with `rev`, only while the text is the backend's of `rev`;
   * with `to` (M-13.3), the text from `offset` to `to` selected.
   */
  jump(offset: number, rev?: number, to?: number): void;
  /**
   * UC-008 A6, A7, M-13.4: a step of the find field holding `spec`: the
   * hit is selected and shown; the keyboard stays where it is.
   */
  find(spec: FindSpec, step: FindStep): Promise<FindAnswer | null>;
  /**
   * UC-005 A20, A21, BR-017: replaces the selected occurrence of `spec`, or
   * all, in the text the facade shows; `null` once the facade is gone.
   */
  replace(spec: FindSpec, replacement: string, all: boolean): Promise<ReplaceDone | null>;
  focus(): void;
  /** M-10.3 to M-10.5: puts `insertion` in at the cursor; resolves once it is shown. */
  insert(insertion: Insertion): Promise<void>;
  /** M-10.5: the cursor to the place under the point (CSS pixels); false off the text. */
  placeAt(x: number, y: number): boolean;
  /** M-10.5: where the cursor is, once the inputs before are in; `null` once gone. */
  mark(): Promise<CursorMark | null>;
  /** M-10.5: the cursor back to `mark`, when the text did not change since. */
  back(mark: CursorMark): void;
  /** FR-080, OQ-247: link completion at the cursor, as Ctrl+Space asks for it. */
  completeLink(): void;
  /** UC-005 A26, A27: an action of the editing comfort from the command palette. */
  edit(action: EditActionName): void;
  /** UC-004 A16 step 1: how many lines the note has; `null` while unknown. */
  lineCount(): number | null;
  /** UC-004 A16 step 3, BR-017: the cursor to the block holding the line of `target`. */
  goToLine(target: LineTargetView): void;
  /**
   * UC-024 step 4: writes `change` of a note attribute, one undo step;
   * resolves once shown, with why nothing was written, else `null`.
   */
  attribute(change: AttributeChangeView): Promise<Refusal | null>;
  /** UC-024 BR-006: undoes the last change of the note, as Ctrl+Z (Cmd+Z) in it does. */
  undo(): void;
  /** UC-005 A28: the cursor to the next marked word (`back`: the one before); false without one. */
  stepSpell(back: boolean): boolean;
  /** UC-005 A28: the spell menu of the marked word at the cursor; false when none is there. */
  spellMenuAtCursor(): boolean;
}

const shown = new Map<PaneName, ActiveFacade>();

/** The facade shown now in the active pane (UC-004 BR-018), if any. */
export function activeFacade(): ActiveFacade | null {
  return shown.get(focusedPane()) ?? null;
}

/** FR-096: the facades shown now, in every pane. */
export function shownFacades(): ActiveFacade[] {
  return [...shown.values()];
}

/**
 * The facade `facade` is shown now in the pane `pane` (`null`: none); with
 * `was`, only when that one is still the pane's.
 */
export function setActiveFacade(
  facade: ActiveFacade | null,
  pane: PaneName = 'left',
  was: ActiveFacade | null = null,
): void {
  if (facade === null) {
    if (was === null || shown.get(pane) === was) {
      shown.delete(pane);
    }
  } else {
    shown.set(pane, facade);
  }
}

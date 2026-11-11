// The ids that join the tab bar (a `tablist`) and the note area (its
// `tabpanel`), plan section 14, and where the keyboard goes after a tab
// closed.

import { focusFallback } from './focus';

/** The element id of the tab of `note`. */
export function tabId(note: string): string {
  return `tab-${encodeURIComponent(note)}`;
}

/** M-06.5: the element id of the tab "Todos"; no note tab has it (`tabId`). */
export const TODO_TAB_ID = 'todo-list-tab';

/** The element id of the active note's area. */
export const NOTE_PANEL_ID = 'note-panel';

/**
 * After the tab at `at` closed: the keyboard goes to the tab now at its
 * place, else to the one left of it, else to the note overview.
 */
export function focusTabAt(at: number): void {
  const tabs = document.querySelectorAll<HTMLElement>('[role="tablist"] [role="tab"]');
  const next = tabs[Math.min(at, tabs.length - 1)];
  if (next) {
    next.focus();
  } else {
    focusFallback();
  }
}

/**
 * M-06.1: what the marks of a tab's label say, for screen readers: " ⚠"
 * the last save failed, " •" changes are not stored yet. The label itself
 * stays as egui shows it.
 */
export function markDescription(label: string, title: string): string | undefined {
  const marks = label.slice(title.length);
  if (marks.startsWith(' ⚠')) {
    return 'not stored';
  }
  if (marks.startsWith(' •')) {
    return 'unsaved changes';
  }
  return undefined;
}

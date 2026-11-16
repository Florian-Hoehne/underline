// FR-096, FR-097, UC-004 BR-018: the panes of the split view as the tab bar
// event (`TabsView`) gives them: the left pane's tabs are `tabs`, the right
// pane's are in `split` while the split view is shown. The rules (which
// pane a note opens in, what ending the split view does) are the
// backend's; these only read the view.

import type { EditingViewName, PaneName, TabsView, TabView } from './bindings';

/** One pane: its tabs, left to right, and the note of its active tab. */
export type PaneTabs = { pane: PaneName; tabs: TabView[]; active: string | null };

/** The panes shown, the left one first. */
export function panesOf(tabs: TabsView): PaneTabs[] {
  const split = tabs.split;
  if (!split) {
    return [{ pane: 'left', tabs: tabs.tabs, active: tabs.active }];
  }
  return [
    { pane: 'left', tabs: tabs.tabs, active: split.leftActive },
    { pane: 'right', tabs: split.right, active: split.rightActive },
  ];
}

/** The active pane: the one the user works in. */
export function activePaneOf(tabs: TabsView): PaneName {
  return tabs.split?.pane ?? 'left';
}

/** The pane `pane`; the left one when the split view is not shown. */
export function paneOf(tabs: TabsView, pane: PaneName): PaneTabs {
  const panes = panesOf(tabs);
  return panes.find((each) => each.pane === pane) ?? panes[0];
}

/** Every tab of both panes. */
export function allTabs(tabs: TabsView): TabView[] {
  return tabs.split ? [...tabs.tabs, ...tabs.split.right] : tabs.tabs;
}

/** The tab of `note` in the pane `pane`. */
export function tabIn(tabs: TabsView, pane: PaneName, note: string): TabView | undefined {
  return paneOf(tabs, pane).tabs.find((tab) => tab.note === note);
}

/** The pane a command about the tab of `note` means: the active one when it has the tab, else the other. */
export function paneWith(tabs: TabsView, note: string): PaneName {
  const active = activePaneOf(tabs);
  if (tabIn(tabs, active, note) || !tabs.split) {
    return active;
  }
  return active === 'left' ? 'right' : 'left';
}

/** UC-005 BR-016: whether `note` has a tab in the pane other than `pane` too. */
export function openElsewhere(tabs: TabsView, pane: PaneName, note: string): boolean {
  if (!tabs.split) {
    return false;
  }
  return tabIn(tabs, pane === 'left' ? 'right' : 'left', note) !== undefined;
}

/** M-07.2: the tab bar with the tab of `note` in `pane` showing `view`, until the backend confirms it. */
export function withView(
  tabs: TabsView,
  pane: PaneName,
  note: string,
  view: EditingViewName,
): TabsView {
  const change = (list: TabView[]) =>
    list.map((tab) => (tab.note === note ? { ...tab, view } : tab));
  if (pane === 'right' && tabs.split) {
    return { ...tabs, split: { ...tabs.split, right: change(tabs.split.right) } };
  }
  return { ...tabs, tabs: change(tabs.tabs) };
}

/** The pane on the other side. */
export function otherPane(pane: PaneName): PaneName {
  return pane === 'left' ? 'right' : 'left';
}

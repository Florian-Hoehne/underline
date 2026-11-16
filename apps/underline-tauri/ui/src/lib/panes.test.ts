// FR-096, UC-004 BR-018: reading the panes of the split view from the tab
// bar event, and which pane a command about a tab means.

import { describe, expect, it } from 'vitest';
import type { TabsView, TabView } from './bindings';
import { activePaneOf, allTabs, openElsewhere, panesOf, paneWith, tabIn, withView } from './panes';

const tab = (note: string, view: TabView['view'] = 'facade'): TabView => ({
  note,
  title: note,
  label: note,
  path: `/w/${note}.md`,
  view,
  saveError: null,
});

const single: TabsView = {
  tabs: [tab('Plan'), tab('Ideas')],
  active: 'Ideas',
  todoList: { open: false, active: false },
  split: null,
};

const split: TabsView = {
  tabs: [tab('Plan'), tab('Ideas')],
  active: 'Plan',
  todoList: { open: false, active: false },
  split: {
    right: [tab('Plan', 'source'), tab('Trip')],
    leftActive: 'Ideas',
    rightActive: 'Plan',
    pane: 'right',
  },
};

describe('UC-004 BR-018 the panes of the tab bar', () => {
  it('one pane without the split view, two with it', () => {
    expect(panesOf(single)).toEqual([{ pane: 'left', tabs: single.tabs, active: 'Ideas' }]);
    expect(panesOf(split).map((p) => [p.pane, p.active])).toEqual([
      ['left', 'Ideas'],
      ['right', 'Plan'],
    ]);
    expect(activePaneOf(single)).toBe('left');
    expect(activePaneOf(split)).toBe('right');
    expect(allTabs(split).map((t) => t.note)).toEqual(['Plan', 'Ideas', 'Plan', 'Trip']);
  });

  it('UC-005 BR-016: a note on both sides; a command means the active pane first', () => {
    expect(openElsewhere(split, 'right', 'Plan')).toBe(true);
    expect(openElsewhere(split, 'right', 'Trip')).toBe(false);
    expect(openElsewhere(single, 'left', 'Plan')).toBe(false);
    expect(paneWith(split, 'Plan')).toBe('right');
    expect(paneWith(split, 'Ideas')).toBe('left');
    expect(paneWith(single, 'Ideas')).toBe('left');
  });

  it('M-07.2: the view of one tab changes, the other tab of its note keeps its own', () => {
    const changed = withView(split, 'right', 'Plan', 'facade');
    expect(tabIn(changed, 'right', 'Plan')?.view).toBe('facade');
    expect(tabIn(changed, 'left', 'Plan')?.view).toBe('facade');
    const left = withView(split, 'left', 'Plan', 'source');
    expect(tabIn(left, 'left', 'Plan')?.view).toBe('source');
    expect(tabIn(left, 'right', 'Plan')?.view).toBe('source');
    expect(tabIn(left, 'right', 'Trip')?.view).toBe('facade');
  });
});

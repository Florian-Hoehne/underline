// Feature F13: rough stand-ins for `underline_core::branches`,
// `underline_core::sync` and `underline_core::conflict` in the mock backend
// (UC-013 to UC-015): the name of a new branch, the places git marks in a
// note in conflict and what a choice makes of them, and the texts of the
// status bar's sync part. The real rules are tested in Rust; these only
// need to look plausible in the browser.

import type { BranchNameView, ChoiceName, SyncBarView, TodoConflictView } from './bindings';

/** UC-013 BR-005: the name typed, cleaned, or why git would not take it (`branch_name`). */
export function mockBranchName(typed: string): BranchNameView {
  const name = typed.split(/\s+/).filter(Boolean).join('-');
  const refuse = (why: string): BranchNameView => ({ text: `${why}.`, ok: false });
  if (name === '') {
    return refuse('a branch needs a name');
  }
  if (name === '@') {
    return refuse('`@` alone is not a branch name');
  }
  const bad = [...name].find((c) => '~^:?*[\\'.includes(c) || c.charCodeAt(0) < 32);
  if (bad !== undefined) {
    return refuse(`a branch name cannot hold \`${bad}\``);
  }
  for (const sequence of ['..', '//', '@{']) {
    if (name.includes(sequence)) {
      return refuse(`a branch name cannot hold \`${sequence}\``);
    }
  }
  for (const start of ['-', '.', '/']) {
    if (name.startsWith(start)) {
      return refuse(`a branch name cannot start with \`${start}\``);
    }
  }
  for (const end of ['/', '.lock', '.']) {
    if (name.endsWith(end)) {
      return refuse(`a branch name cannot end with \`${end}\``);
    }
  }
  return { text: `Branch: ${name}`, ok: true };
}

/** A conflicting place of a text: where it is (UTF-16 units) and both sides. */
export type MockPlace = { start: number; end: number; line: number; here: string; remote: string };

/** UC-015 step 3: the places git marks in `text`, by merge (`conflict_places`). */
export function mockConflictPlaces(text: string): MockPlace[] {
  const places: MockPlace[] = [];
  const lines = text.match(/[^\n]*\n|[^\n]+$/g) ?? [];
  let at = 0;
  let open: { start: number; line: number; here: string; remote: string | null } | null = null;
  lines.forEach((line, index) => {
    const start = at;
    at += line.length;
    const content = line.replace(/\r?\n$/, '');
    if (open === null) {
      if (content.startsWith('<<<<<<<')) {
        open = { start, line: index + 1, here: '', remote: null };
      }
    } else if (content.startsWith('=======') && open.remote === null) {
      open.remote = '';
    } else if (content.startsWith('>>>>>>>') && open.remote !== null) {
      places.push({
        start: open.start,
        end: at,
        line: open.line,
        here: open.here,
        remote: open.remote,
      });
      open = null;
    } else if (open.remote === null) {
      open.here += line;
    } else {
      open.remote += line;
    }
  });
  return places;
}

/** UC-015 step 5: what `choice` puts in the place (`resolve_place`). */
export function mockResolvePlace(place: MockPlace, choice: ChoiceName): string {
  if (choice === 'here') {
    return place.here;
  }
  if (choice === 'remote') {
    return place.remote;
  }
  const here = place.here !== '' && !place.here.endsWith('\n') ? `${place.here}\n` : place.here;
  return here + place.remote;
}

/** How a fake sync ends (`SyncOutcome`). */
export type MockSyncOutcome =
  'synced' | 'offline' | 'failed' | 'signIn' | 'conflict' | 'inTheWay' | 'pushRejected';

/** The reasons the fake syncs give, in git's words. */
export const MOCK_SYNC_REASONS = {
  offline:
    "fatal: unable to access 'https://example.com/team.git/': Could not resolve host: example.com",
  failed: 'remote: error: GH006: Protected branch update failed for refs/heads/main.',
  signIn: "fatal: could not read Username for 'https://example.com': terminal prompts disabled",
} as const;

/** "just now", "N min ago", "N h ago" (egui's `ago`). */
function ago(ms: number): string {
  const secs = Math.floor(ms / 1000);
  return secs < 60
    ? 'just now'
    : secs < 3600
      ? `${Math.floor(secs / 60)} min ago`
      : `${Math.floor(secs / 3600)} h ago`;
}

/** The last fake sync: how it ended and why, the files in its way or in conflict. */
export type MockLastSync = { outcome: MockSyncOutcome; files: string[] };

/** M-04.3, UC-014 BR-004: the sync part of the status bar, as `Remote::bar` makes it. */
export function mockSyncBar(state: {
  running: boolean;
  last: MockLastSync | null;
  succeeded: number | null;
  ahead: number;
  behind: number;
  conflicts: number;
  now: number;
}): SyncBarView {
  const since = state.succeeded === null ? null : ago(state.now - state.succeeded);
  let text: string;
  let warning = false;
  let hover: string | null = null;
  const last = state.last;
  if (state.running) {
    text = 'Syncing…';
  } else if (last === null) {
    text = since === null ? 'Not synced yet' : `Synced ${since}`;
  } else {
    switch (last.outcome) {
      case 'synced':
        text = `Synced ${since ?? ''}`;
        break;
      case 'offline':
        text = since === null ? 'Offline, will retry' : `Offline, will retry (synced ${since})`;
        hover = MOCK_SYNC_REASONS.offline;
        break;
      case 'failed':
        text = 'Sync failed';
        warning = true;
        hover = MOCK_SYNC_REASONS.failed;
        break;
      case 'signIn':
        text = 'Sync failed: signing in';
        warning = true;
        hover = MOCK_SYNC_REASONS.signIn;
        break;
      case 'conflict':
        text = 'Sync stopped: conflict';
        warning = true;
        hover = last.files.join(', ');
        break;
      case 'inTheWay':
        text = 'Sync stopped';
        warning = true;
        hover = last.files.join(', ');
        break;
      case 'pushRejected':
        text = 'Push refused';
        warning = true;
        break;
    }
  }
  return {
    text,
    warning,
    hover,
    toPush: state.ahead > 0 ? `${state.ahead} to push` : null,
    toPull: state.behind > 0 ? `${state.behind} to pull` : null,
    running: state.running,
    canSync: !state.running && state.conflicts === 0,
  };
}

/** UC-015 A2: `img/shot.png` → `img/shot (remote).png` (`copy_name`). */
export function mockCopyName(path: string, label: string): string {
  const slash = path.lastIndexOf('/');
  const folder = slash >= 0 ? path.slice(0, slash + 1) : '';
  const name = path.slice(slash + 1);
  const dot = name.lastIndexOf('.');
  return dot > 0
    ? `${folder}${name.slice(0, dot)} (${label})${name.slice(dot)}`
    : `${folder}${name} (${label})`;
}

/** UC-020 A10: the todo in conflict of the fake sync. */
export const MOCK_TODO_CONFLICT: TodoConflictView = {
  number: 3,
  heading: 'Todo #3 was changed here and on the remote',
  rows: [
    { name: 'Title', here: 'Send the agenda', remote: 'Send the agenda today', differ: true },
    { name: 'Due', here: '2026-11-12', remote: '2026-11-12', differ: false },
    { name: 'Scheduled', here: '–', remote: '–', differ: false },
    { name: 'Urgency', here: 'Normal', remote: 'High', differ: true },
    { name: 'Status', here: 'open', remote: 'open', differ: false },
    { name: 'Notes', here: 'Meetings', remote: 'Meetings', differ: false },
  ],
};

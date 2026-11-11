// Feature F12: rough stand-ins for `underline_core::changes` in the mock
// backend (UC-011, UC-012): a line diff with three lines around each
// changed part, the words changed within a line, "only spaces" and "only
// the line ending", the lines a discard loses, and the proposed commit
// message. The real rules (git's own diff through `similar`) are tested in
// Rust; these only need to look plausible in the browser.

import type { DiffLineKind, DiffLineView, DiffPartView, HunkView } from './bindings';

/** The lines of `text`, each with its line ending, as `TextDiff::from_lines` cuts them. */
function linesOf(text: string): string[] {
  return text.match(/[^\n]*\n|[^\n]+$/g) ?? [];
}

type Op = { kind: DiffLineKind; old: number | null; new: number | null; text: string };

/** The lines of `a` and `b` as same, removed and added, removed first within a change (LCS). */
function lineOps(a: string[], b: string[]): Op[] {
  const n = a.length;
  const m = b.length;
  const lcs: number[][] = Array.from({ length: n + 1 }, () => new Array<number>(m + 1).fill(0));
  for (let i = n - 1; i >= 0; i -= 1) {
    for (let j = m - 1; j >= 0; j -= 1) {
      lcs[i][j] = a[i] === b[j] ? lcs[i + 1][j + 1] + 1 : Math.max(lcs[i + 1][j], lcs[i][j + 1]);
    }
  }
  const ops: Op[] = [];
  let i = 0;
  let j = 0;
  while (i < n || j < m) {
    if (i < n && j < m && a[i] === b[j]) {
      ops.push({ kind: 'same', old: i + 1, new: j + 1, text: a[i] });
      i += 1;
      j += 1;
    } else if (i < n && (j >= m || lcs[i + 1][j] >= lcs[i][j + 1])) {
      ops.push({ kind: 'removed', old: i + 1, new: null, text: a[i] });
      i += 1;
    } else {
      ops.push({ kind: 'added', old: null, new: j + 1, text: b[j] });
      j += 1;
    }
  }
  return ops;
}

/** `n` right-aligned in four places, or four spaces. */
function number(n: number | null): string {
  return n === null ? '    ' : String(n).padStart(4, ' ');
}

const ending = (line: string) => line.replace(/\r?\n$/, '');

/** The removed and added line differ only in spaces or their line ending (BR-004). */
function onlyDiffers(removed: string, added: string): string | null {
  if (ending(removed) === ending(added)) {
    return '  (only the line ending differs)';
  }
  const words = (line: string) => line.split(/\s+/).filter(Boolean).join(' ');
  return words(removed) === words(added) ? '  (only spaces differ)' : null;
}

/** The line in parts, the middle that differs from `other` marked strong. */
function partsOf(line: string, other: string | null): DiffPartView[] {
  const text = ending(line);
  if (other === null) {
    return [{ text, strong: false }];
  }
  const them = ending(other);
  let start = 0;
  while (start < text.length && start < them.length && text[start] === them[start]) {
    start += 1;
  }
  let end = 0;
  while (
    end < text.length - start &&
    end < them.length - start &&
    text[text.length - 1 - end] === them[them.length - 1 - end]
  ) {
    end += 1;
  }
  const parts: DiffPartView[] = [];
  if (start > 0) parts.push({ text: text.slice(0, start), strong: false });
  if (text.length - end > start)
    parts.push({ text: text.slice(start, text.length - end), strong: true });
  if (end > 0) parts.push({ text: text.slice(text.length - end), strong: false });
  return parts.length > 0 ? parts : [{ text: '', strong: false }];
}

/**
 * UC-011 BR-004: the changed parts of `now` against `old`, `context` lines
 * around each, joined when they come close. With `offsets`, each line
 * knows the web offset of its line in `now` (steps 7–8).
 */
export function mockDiff(old: string, now: string, context = 3, offsets = false): HunkView[] {
  const ops = lineOps(linesOf(old), linesOf(now));
  const starts: number[] = [0];
  for (const line of linesOf(now)) {
    starts.push(starts[starts.length - 1] + line.length);
  }
  const changed = ops.map((op, at) => (op.kind === 'same' ? -1 : at)).filter((at) => at >= 0);
  if (changed.length === 0) {
    return [];
  }
  // The ranges of ops to show, joined when they come within 2 × context.
  const ranges: [number, number][] = [];
  for (const at of changed) {
    const from = Math.max(0, at - context);
    const to = Math.min(ops.length - 1, at + context);
    const last = ranges[ranges.length - 1];
    if (last && from <= last[1] + 1) {
      last[1] = Math.max(last[1], to);
    } else {
      ranges.push([from, to]);
    }
  }
  return ranges.map(([from, to]) => {
    const shown = ops.slice(from, to + 1);
    const lines: DiffLineView[] = [];
    for (let at = 0; at < shown.length; at += 1) {
      const op = shown[at];
      // Pair the k-th removed line of a change with its k-th added line.
      let partner: string | null = null;
      if (op.kind !== 'same') {
        let first = at;
        while (first > 0 && shown[first - 1].kind !== 'same') first -= 1;
        let end = at;
        while (end < shown.length - 1 && shown[end + 1].kind !== 'same') end += 1;
        const run = shown.slice(first, end + 1);
        const removed = run.filter((o) => o.kind === 'removed');
        const added = run.filter((o) => o.kind === 'added');
        const index = (op.kind === 'removed' ? removed : added).indexOf(op);
        partner = (op.kind === 'removed' ? added : removed)[index]?.text ?? null;
      }
      const sign = op.kind === 'same' ? ' ' : op.kind === 'removed' ? '-' : '+';
      const line = op.new ?? op.old ?? 1;
      const lineCount = Math.max(starts.length - 1, 1);
      lines.push({
        kind: op.kind,
        numbers: `${number(op.old)} ${number(op.new)} ${sign} `,
        parts: partsOf(op.text, partner),
        only:
          partner === null
            ? null
            : op.kind === 'removed'
              ? onlyDiffers(op.text, partner)
              : onlyDiffers(partner, op.text),
        offset: offsets ? (starts[Math.min(line, lineCount) - 1] ?? 0) : null,
      });
    }
    return { lines };
  });
}

/** UC-011 A6 step 1: how many lines a discard loses. */
export function mockChangedLines(old: string, now: string): number {
  return mockDiff(old, now, 0)
    .flatMap((hunk) => hunk.lines)
    .filter((line) => line.kind !== 'same').length;
}

/** A change as the proposal counts it. */
export type MockProposalChange = {
  kind: 'new' | 'changed' | 'deleted' | 'renamed' | 'conflict';
  title: string | null;
  from: string | null;
};

/** UC-012 BR-002: the proposed message, as `proposed_message` makes it. */
export function mockProposedMessage(changes: MockProposalChange[]): string {
  const updated: string[] = [];
  const added: string[] = [];
  const deleted: string[] = [];
  const renamed: string[] = [];
  let others = 0;
  for (const change of changes) {
    if (change.title === null) {
      others += 1;
    } else if (change.kind === 'new') {
      added.push(change.title);
    } else if (change.kind === 'deleted') {
      deleted.push(change.title);
    } else if (change.kind === 'renamed') {
      renamed.push(`${change.from ?? ''} to ${change.title}`);
    } else {
      updated.push(change.title);
    }
  }
  const name = (verb: string, titles: string[]) =>
    titles.length === 0
      ? null
      : titles.length <= 3
        ? `${verb} ${titles.join(', ')}`
        : `${verb} ${titles.slice(0, 3).join(', ')} and ${titles.length - 3} more notes`;
  const parts = [
    name('Update', updated),
    name('Add', added),
    name('Delete', deleted),
    name('Rename', renamed),
  ].filter((part): part is string => part !== null);
  const files = others === 0 ? null : others === 1 ? '1 other file' : `${others} other files`;
  if (files !== null) {
    if (parts.length > 0) {
      parts[parts.length - 1] += ` and ${files}`;
    } else {
      parts.push(`Update ${files}`);
    }
  }
  return parts.join('; ');
}

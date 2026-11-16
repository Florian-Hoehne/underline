// UC-026 Restore Snapshot (FR-101, FR-102): the window's side of the
// snapshots of a note. It asks for the list (`snapshots_list`), the
// differences of the snapshot chosen with the note (`snapshot_diff`) or
// the whole snapshot (`snapshot_whole`, A2), and restores one
// (`snapshot_restore`) once the window's edits reached the backend
// (ADR-025). The backend says when the note or its snapshots changed
// (`snapshots-changed`, A4); the list and the differences are then asked
// for again.
//
// No domain logic lives here: when snapshots are taken, how long they are
// kept, what differs and what a restore writes are the backend's.

import {
  commands,
  type SnapshotDiffView,
  type SnapshotItemView,
  type SnapshotWholeView,
  type SnapshotsView,
  type UiError,
} from './bindings';
import { flushed } from './editor/sync';

/** What the right side of the dialog shows for the snapshot chosen. */
export type SnapshotShown = 'differences' | 'whole';

export class SnapshotsState {
  /** The note whose snapshots are shown; `null` while the dialog is closed. */
  note = $state<string | null>(null);
  /** Its title, until the list came. */
  title = $state('');
  /** The list, as the backend answered last; `null` while it is read the first time. */
  list = $state.raw<SnapshotsView | null>(null);
  /** The key of the snapshot chosen. */
  chosen = $state<string | null>(null);
  /** Step 4 or A2. */
  shown = $state<SnapshotShown>('differences');
  /** The differences of the snapshot chosen, of the newest request. */
  diff = $state.raw<SnapshotDiffView | null>(null);
  /** A2: the whole snapshot chosen. */
  whole = $state.raw<SnapshotWholeView | null>(null);
  /** The list, the differences or the whole snapshot are being read. */
  loading = $state(false);
  /** Step 6 runs. */
  restoring = $state(false);
  /** Why the last request failed; shown in the dialog. */
  problem = $state<string | null>(null);
  /** Counts the times the dialog was opened, so it takes the keyboard. */
  opened = $state(0);

  /** The number of the newest request; older answers are dropped. */
  #asked = 0;
  #fail: (command: string, error: UiError) => void;

  constructor(fail: (command: string, error: UiError) => void) {
    this.#fail = fail;
  }

  /** The snapshot chosen, as the list has it. */
  get chosenItem(): SnapshotItemView | null {
    return this.list?.items.find((item) => item.key === this.chosen) ?? null;
  }

  /** Steps 1–2: the snapshots of `note` (titled `title`) are shown. */
  async show(note: string, title: string): Promise<void> {
    this.note = note;
    this.title = title;
    this.list = null;
    this.chosen = null;
    this.shown = 'differences';
    this.diff = null;
    this.whole = null;
    this.problem = null;
    this.opened += 1;
    await this.#readList(true);
  }

  /** A4: the backend says the note or its snapshots changed. */
  async changed(note: string): Promise<void> {
    if (note !== this.note) {
      return;
    }
    await this.#readList(false);
  }

  /** Step 3: the snapshot `key` is chosen; its differences or itself show. */
  async choose(key: string): Promise<void> {
    if (this.note === null || key === this.chosen) {
      return;
    }
    this.chosen = key;
    this.diff = null;
    this.whole = null;
    await this.#readChosen();
  }

  /** Moves the choice `by` entries in the list, at its ends it stays. */
  async move(by: number): Promise<void> {
    const items = this.list?.items ?? [];
    if (items.length === 0) {
      return;
    }
    const at = items.findIndex((item) => item.key === this.chosen);
    const next = at < 0 ? 0 : Math.min(items.length - 1, Math.max(0, at + by));
    await this.choose(items[next].key);
  }

  /** A2: between the differences and the whole snapshot. */
  async showAs(shown: SnapshotShown): Promise<void> {
    if (shown === this.shown) {
      return;
    }
    this.shown = shown;
    await this.#readChosen();
  }

  /**
   * Steps 5–7: the snapshot chosen is restored, once the window's edits
   * reached the backend; the dialog closes and the status bar reports it.
   * When it cannot be restored, the dialog stays and says why.
   */
  async restore(): Promise<boolean> {
    const note = this.note;
    const key = this.chosen;
    if (note === null || key === null || this.restoring || this.chosenItem?.problem) {
      return false;
    }
    this.restoring = true;
    try {
      const answer = await flushed(() => commands.snapshotRestore(note, key));
      if (answer.status === 'error') {
        this.problem = answer.error.message;
        return false;
      }
      await this.close();
      return true;
    } finally {
      this.restoring = false;
    }
  }

  /** A6: the dialog closes; nothing changes. */
  async close(): Promise<void> {
    if (this.note === null) {
      return;
    }
    this.note = null;
    this.list = null;
    this.chosen = null;
    this.diff = null;
    this.whole = null;
    this.problem = null;
    this.#asked += 1;
    const answer = await commands.snapshotsClose();
    if (answer.status === 'error') {
      this.#fail('snapshots_close', answer.error);
    }
  }

  /** Reads the list; with `fresh`, the newest snapshot is chosen. */
  async #readList(fresh: boolean): Promise<void> {
    const note = this.note;
    if (note === null) {
      return;
    }
    const asked = ++this.#asked;
    this.loading = true;
    try {
      const answer = await commands.snapshotsList(note);
      if (asked !== this.#asked) {
        return;
      }
      if (answer.status === 'error') {
        this.problem = answer.error.message;
        return;
      }
      this.list = answer.data;
      this.title = answer.data.title;
      const items = answer.data.items;
      const still = items.some((item) => item.key === this.chosen);
      if (fresh || !still) {
        this.chosen = items[0]?.key ?? null;
      }
    } finally {
      if (asked === this.#asked) {
        this.loading = false;
      }
    }
    await this.#readChosen();
  }

  /** Reads what the right side shows of the snapshot chosen. */
  async #readChosen(): Promise<void> {
    const note = this.note;
    const key = this.chosen;
    if (note === null || key === null) {
      this.diff = null;
      this.whole = null;
      return;
    }
    const asked = ++this.#asked;
    this.loading = true;
    try {
      if (this.shown === 'differences') {
        const answer = await commands.snapshotDiff(note, key);
        if (asked === this.#asked) {
          if (answer.status === 'error') {
            this.problem = answer.error.message;
          } else {
            this.diff = answer.data;
          }
        }
      } else {
        const answer = await commands.snapshotWhole(note, key);
        if (asked === this.#asked) {
          if (answer.status === 'error') {
            this.problem = answer.error.message;
          } else {
            this.whole = answer.data;
          }
        }
      }
    } finally {
      if (asked === this.#asked) {
        this.loading = false;
      }
    }
  }
}

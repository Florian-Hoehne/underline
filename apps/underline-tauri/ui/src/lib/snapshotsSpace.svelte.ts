// UC-026 A7, OQ-294: the window's side of the space the snapshots of the
// open workspace take, in the part "This workspace" of the settings window,
// and of removing them all. The backend measures (`snapshots_space`) and
// removes (`snapshots_remove_all`), and says in the status bar what it
// removed (only there); the window asks before it removes anything.
//
// No domain logic lives here: what counts as a snapshot, the texts and what
// is removed are the backend's.

import { commands, type SnapshotsSpaceView, type UiError } from './bindings';

export class SnapshotsSpaceState {
  /** The space, as the backend measured it last; `null` before. */
  view = $state.raw<SnapshotsSpaceView | null>(null);
  /** The snapshots are being measured. */
  measuring = $state(false);
  /** The question before removing them all is shown. */
  asking = $state(false);
  /** They are being removed. */
  removing = $state(false);

  /** The number of the newest measurement; older answers are dropped. */
  #asked = 0;
  #fail: (command: string, error: UiError) => void;

  constructor(fail: (command: string, error: UiError) => void) {
    this.#fail = fail;
  }

  /** Measures the snapshots of the open workspace; the view shown so far stays until the answer. */
  async measure(): Promise<void> {
    const asked = ++this.#asked;
    this.measuring = true;
    try {
      const result = await commands.snapshotsSpace();
      if (asked !== this.#asked) {
        return;
      }
      if (result.status === 'ok') {
        this.view = result.data;
      } else {
        this.view = null;
        this.#fail('snapshots_space', result.error);
      }
    } catch (error) {
      if (asked === this.#asked) {
        this.#fail('snapshots_space', { kind: 'internal', message: String(error) });
      }
    } finally {
      if (asked === this.#asked) {
        this.measuring = false;
      }
    }
  }

  /** Another workspace: what was measured and reported of the one before goes. */
  forget(): void {
    this.#asked++;
    this.view = null;
    this.asking = false;
    this.measuring = false;
  }

  /** "Remove all snapshots…": the question, naming how many and their space. */
  ask(): void {
    if (this.view?.any && !this.removing) {
      this.asking = true;
    }
  }

  /** "Cancel" of the question: nothing is removed. */
  cancel(): void {
    this.asking = false;
  }

  /** "Remove all snapshots" of the question: they go, the report says what went, and they are measured again. */
  async removeAll(): Promise<void> {
    if (!this.asking || this.removing) {
      return;
    }
    this.asking = false;
    this.removing = true;
    try {
      // What went is said in the status bar, by the backend.
      const result = await commands.snapshotsRemoveAll();
      if (result.status === 'error') {
        this.#fail('snapshots_remove_all', result.error);
      }
    } catch (error) {
      this.#fail('snapshots_remove_all', { kind: 'internal', message: String(error) });
    } finally {
      this.removing = false;
    }
    await this.measure();
  }
}

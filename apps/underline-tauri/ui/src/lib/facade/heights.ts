// ADR-024, plan section 15: the facade is virtualized. Only the blocks near
// the part in view are in the DOM; every other block stands as its height:
// measured once it was drawn, estimated before. This module keeps those
// heights and finds blocks by their place (no `content-visibility`, which
// costs WebKitGTK hundreds of milliseconds per change).

/** The heights of the blocks, with the place of each. */
export class Heights {
  #heights: number[] = [];
  #measured: boolean[] = [];
  /** `#tops[i]` is the top of block `i`; one more for the end. */
  #tops: number[] = [0];
  #dirty = false;

  /** How many blocks there are. */
  get length(): number {
    return this.#heights.length;
  }

  /** All blocks anew, with the heights `estimates`, none measured. */
  reset(estimates: readonly number[]): void {
    this.#heights = estimates.slice();
    this.#measured = estimates.map(() => false);
    this.#dirty = true;
  }

  /**
   * `removed` blocks from `from` make way for blocks of the heights
   * `estimates`; the others keep theirs.
   */
  splice(from: number, removed: number, estimates: readonly number[]): void {
    this.#heights.splice(from, removed, ...estimates);
    this.#measured.splice(from, removed, ...estimates.map(() => false));
    this.#dirty = true;
  }

  /** Sets the height of block `index`; returns how much it changed. */
  set(index: number, height: number, measured = true): number {
    const before = this.#heights[index] ?? 0;
    if (index >= this.#heights.length || before === height) {
      if (index < this.#measured.length) {
        this.#measured[index] ||= measured;
      }
      return 0;
    }
    this.#heights[index] = height;
    this.#measured[index] ||= measured;
    this.#dirty = true;
    return height - before;
  }

  /** The height of block `index`. */
  get(index: number): number {
    return this.#heights[index] ?? 0;
  }

  /** Whether block `index` was measured. */
  measured(index: number): boolean {
    return this.#measured[index] ?? false;
  }

  /** Estimates the blocks not measured again, as after the width changed; all when `all`. */
  reestimate(estimate: (index: number) => number, all: boolean): void {
    for (let i = 0; i < this.#heights.length; i++) {
      if (all || !this.#measured[i]) {
        this.#heights[i] = estimate(i);
        this.#measured[i] = false;
      }
    }
    this.#dirty = true;
  }

  #update(): number[] {
    if (this.#dirty) {
      const tops = new Array<number>(this.#heights.length + 1);
      let y = 0;
      for (let i = 0; i < this.#heights.length; i++) {
        tops[i] = y;
        y += this.#heights[i];
      }
      tops[this.#heights.length] = y;
      this.#tops = tops;
      this.#dirty = false;
    }
    return this.#tops;
  }

  /** The top of block `index`; the total height for `length`. */
  top(index: number): number {
    const tops = this.#update();
    return tops[Math.min(Math.max(index, 0), tops.length - 1)];
  }

  /** The height of all blocks. */
  total(): number {
    return this.top(this.length);
  }

  /** The block at `y`: the last one starting at or above it; 0 without blocks. */
  indexAt(y: number): number {
    const tops = this.#update();
    let low = 0;
    let high = this.#heights.length;
    while (low < high) {
      const mid = (low + high) >> 1;
      if (tops[mid] <= y) {
        low = mid + 1;
      } else {
        high = mid;
      }
    }
    return Math.max(0, Math.min(low - 1, this.#heights.length - 1));
  }
}

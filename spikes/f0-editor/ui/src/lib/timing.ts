// Time stamps of one input: when the event was made, when the handler began,
// when the DOM was patched, and when the frame showing it was painted.

export interface Sample {
  /** Handler start to DOM patched (processing, a). */
  proc: number;
  /** event.timeStamp to the second requestAnimationFrame (end-to-end, b). */
  e2e: number;
  /** event.timeStamp to just after the next paint (rAF + setTimeout 0). */
  paint: number;
  /** The owner's own time (A: measured in Rust; B: the WASM call). */
  owner: number;
  /** Applying the patch to the DOM (CodeMirror dispatch or block swap). */
  dom: number;
  /** Whether the patch was in the DOM before the first frame after the
   * event, and that frame came within 17.5 ms of the event. */
  firstFrame: boolean;
  /** Whether the whole note was parsed again (facade). */
  reparsed?: boolean;
}

/** Resolves after the next paint (rAF, then a task) and after the frame
 * after it (double rAF); returns both times. */
export function afterPaint(): Promise<{ paint: number; e2e: number; stalled?: boolean }> {
  return new Promise((resolve) => {
    let paint = 0;
    // A hidden or covered window gets no frames: do not wait forever.
    const guard = setTimeout(() => {
      stalls++;
      resolve({ paint: NaN, e2e: NaN, stalled: true });
    }, 1000);
    requestAnimationFrame(() => {
      setTimeout(() => {
        paint = performance.now();
      }, 0);
      requestAnimationFrame(() => {
        const now = performance.now();
        clearTimeout(guard);
        resolve({ paint: paint || now, e2e: now });
      });
    });
  });
}

/** How often no frame came within a second. */
export let stalls = 0;

/** Two animation frames: the e2e mark of the task description. */
export function doubleRaf(): Promise<number> {
  return new Promise((resolve) => {
    const guard = setTimeout(() => {
      stalls++;
      resolve(performance.now());
    }, 1000);
    requestAnimationFrame(() =>
      requestAnimationFrame(() => {
        clearTimeout(guard);
        resolve(performance.now());
      }),
    );
  });
}

export interface Stats {
  n: number;
  median: number;
  p95: number;
  p99: number;
  max: number;
  mean: number;
}

export function stats(values: number[]): Stats {
  const v = values.filter((x) => Number.isFinite(x)).sort((a, b) => a - b);
  const q = (p: number) =>
    v.length ? v[Math.min(v.length - 1, Math.ceil(p * v.length) - 1)] : NaN;
  return {
    n: v.length,
    median: q(0.5),
    p95: q(0.95),
    p99: q(0.99),
    max: v.length ? v[v.length - 1] : NaN,
    mean: v.reduce((a, b) => a + b, 0) / (v.length || 1),
  };
}

export function isPromise<T>(v: T | Promise<T>): v is Promise<T> {
  return typeof (v as Promise<T>)?.then === 'function';
}

/** The time of the first animation frame after now (0 until it comes). */
export function nextFrame(): { time: number } {
  const frame = { time: 0 };
  requestAnimationFrame(() => {
    frame.time = performance.now();
  });
  return frame;
}

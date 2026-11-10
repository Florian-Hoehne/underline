import '@testing-library/jest-dom/vitest';

// jsdom lays nothing out; CodeMirror measures text ranges all the same.
if (!Range.prototype.getClientRects) {
  Range.prototype.getClientRects = function () {
    return Object.assign([], { item: () => null }) as unknown as DOMRectList;
  };
  Range.prototype.getBoundingClientRect = () => new DOMRect();
}

// jsdom has no ResizeObserver (the status line and the minimap measure with
// it, and Svelte's `bind:clientWidth` needs it); a stand-in that never calls
// back keeps the components working.
if (typeof globalThis.ResizeObserver === 'undefined') {
  globalThis.ResizeObserver = class {
    observe(): void {}
    unobserve(): void {}
    disconnect(): void {}
  };
}

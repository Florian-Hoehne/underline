import '@testing-library/jest-dom/vitest';

// jsdom lays nothing out; CodeMirror measures text ranges all the same.
if (!Range.prototype.getClientRects) {
  Range.prototype.getClientRects = function () {
    return Object.assign([], { item: () => null }) as unknown as DOMRectList;
  };
  Range.prototype.getBoundingClientRect = () => new DOMRect();
}

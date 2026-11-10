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

// ADR-025: the editor's WASM, loaded from its bytes, as Node fetches no
// file URLs. Node's `fs` is taken at run time, so the frontend needs no Node
// types.
{
  // jsdom gives `import.meta.url` an http address; Vitest runs in `ui/`.
  type Fs = { readFileSync(path: string): Uint8Array<ArrayBuffer> };
  const { process } = globalThis as unknown as {
    process: { cwd(): string; getBuiltinModule(id: string): unknown };
  };
  const fs = process.getBuiltinModule('node:fs') as Fs;
  const { loadEditorCoreSync } = await import('./lib/editor/core');
  loadEditorCoreSync(
    fs.readFileSync(`${process.cwd()}/src/lib/wasm/pkg/underline_editor_wasm_bg.wasm`),
  );
}

// ADR-025: the editor cores of one test's notes are not another test's.
{
  const { afterEach } = await import('vitest');
  const { dropAllCores } = await import('./lib/editor/sync');
  afterEach(() => dropAllCores());
}

// The mock's timers (its autosave) end with the test that made them.
{
  const { afterEach } = await import('vitest');
  afterEach(() => {
    (globalThis as unknown as { __ulMock?: { dispose(): void } }).__ulMock?.dispose();
  });
}

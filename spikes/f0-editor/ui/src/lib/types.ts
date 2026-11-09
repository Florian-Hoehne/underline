// The DTOs of `f0-model` (owner.rs), as they cross the IPC or the WASM
// boundary. Offsets are UTF-16 code units.

export type Input =
  | { type: 'insertText'; text: string }
  | { type: 'insertParagraph' }
  | { type: 'deleteBackward' }
  | { type: 'replace'; to: number; text: string };

export interface SourcePatch {
  revision: number;
  from: number;
  to: number;
  insert: string;
  cursor: number;
}

export interface FacadePatch {
  revision: number;
  from: number;
  to: number;
  html: string[];
  cursorBlock: number;
  cursorAt: number;
  reparsed: boolean;
  blocks: number;
}

export interface BenchPosition {
  name: string;
  block: number;
  at: number;
  source: number;
  line: number;
}

/** A value now (variant B) or later (variant A). */
export type MaybePromise<T> = T | Promise<T>;

/** Who owns the text: the backend (A) or the WASM core in the webview (B). */
export interface Owner {
  readonly name: string;
  /** Loads the note into the owner; returns its text. */
  open(): Promise<string>;
  sourceInput(at: number, input: Input): MaybePromise<SourcePatch>;
  facadeInput(block: number, at: number, input: Input): MaybePromise<FacadePatch>;
  renderAll(): MaybePromise<string>;
  renderRange(from: number, to: number): MaybePromise<string[]>;
  blockCount(): MaybePromise<number>;
  text(): MaybePromise<string>;
  /** The time the owner itself took for the last input, in ms (A only). */
  lastOwnerMs: number;
  /** Variant B: sends pending edits to the backend and waits for them. */
  flush(): Promise<void>;
  /** The text as the backend has it (A: the owner; B: the mirror). */
  backendText(): Promise<string>;
  positions(): MaybePromise<BenchPosition[]>;
  blockText(index: number): MaybePromise<string>;
  /** Mirror problems seen so far (B). */
  mirrorErrors: string[];
}

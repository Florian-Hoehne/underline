// ADR-025, step B3c: the editor's WASM gives the answers the same calls give
// natively (`crates/underline-editor-wasm/src/golden.rs` writes them to
// `golden/calls.json`): the patches of both views, the sections, the hits,
// the clipboard, the keys, the text and its hash.

import { describe, expect, it } from 'vitest';
import golden from '../../../../../../crates/underline-editor-wasm/golden/calls.json';
import type { FormatName } from '../bindings';
import { EditorCore, EditorCoreDead } from './core';

type Step = { call: string; args: unknown[]; out: unknown };
type Scenario = {
  name: string;
  open: { note: string; text: string; format: FormatName; mac: boolean };
  steps: Step[];
};

/**
 * Calls `call` with `args` on `core`, as the views do; an error as
 * `{ error }`, as the golden file holds it.
 */
function call(core: EditorCore, step: Step): unknown {
  const method = (core as unknown as Record<string, (...args: unknown[]) => unknown>)[step.call];
  if (typeof method !== 'function') {
    throw new Error(`EditorCore has no ${step.call}`);
  }
  try {
    return method.apply(core, step.args) ?? null;
  } catch (error) {
    return { error: error instanceof Error ? error.message : String(error) };
  }
}

describe('ADR-025: the WASM editor core', () => {
  for (const scenario of golden as unknown as Scenario[]) {
    it(`gives the golden answers natively made: ${scenario.name}`, () => {
      const { note, text, format, mac } = scenario.open;
      const core = EditorCore.open(note, text, format, mac);
      try {
        for (const [index, step] of scenario.steps.entries()) {
          expect(call(core, step), `step ${index}: ${step.call}`).toEqual(step.out);
        }
      } finally {
        core.free();
      }
    });
  }

  it('says why an argument is wrong, and the tab goes on', () => {
    const core = EditorCore.open('Home', 'a', 'Markdown', false);
    expect(() => core.setMode('Emacs' as never)).toThrow('Emacs is no input mode');
    expect(core.dead).toBe(false);
    expect(core.text()).toBe('a');
    core.free();
  });

  it('lets a tab go whose core failed, and says so on every later call', () => {
    let freed = 0;
    const trap = new WebAssembly.RuntimeError('unreachable');
    const tab = {
      text: () => {
        throw trap;
      },
      hash: () => 'h',
      free: () => {
        freed += 1;
      },
    };
    const core = EditorCore.over(tab as never, 'Home');
    const told: EditorCoreDead[] = [];
    core.onDead = (dead) => told.push(dead);
    const logged = console.error;
    console.error = () => {};
    try {
      expect(() => core.text()).toThrow(EditorCoreDead);
    } finally {
      console.error = logged;
    }
    expect(core.dead).toBe(true);
    expect(freed).toBe(1);
    expect(told).toHaveLength(1);
    expect(told[0].cause).toBe(trap);
    expect(() => core.hash()).toThrow("The editor's core of Home failed");
  });
});

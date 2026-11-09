// Times the WASM owner alone in Node (V8, not JavaScriptCore): what one
// facade input costs in wasm32 compared with the native owner_bench example.
//   node tools/wasm-owner-bench.mjs
import { readFileSync } from 'node:fs';
import { initSync, WasmOwner } from '../ui/src/wasm-pkg/f0_wasm.js';

const dir = new URL('../ui/src/wasm-pkg/', import.meta.url);
initSync({ module: readFileSync(new URL('f0_wasm_bg.wasm', dir)) });

// The note is generated in Rust; take it from a native run's text.
const note = readFileSync(process.argv[2] ?? '/tmp/f0-note.md', 'utf8');
const q = (v, p) => v[Math.min(v.length - 1, Math.ceil(p * v.length) - 1)];
const fmt = (v) => {
  v.sort((a, b) => a - b);
  return `median ${q(v, 0.5).toFixed(3)} ms, p99 ${q(v, 0.99).toFixed(3)} ms, max ${v.at(-1).toFixed(3)} ms`;
};
let t = performance.now();
const owner = new WasmOwner(note);
const html = owner.renderAll();
console.log(`open + parse + render all: ${(performance.now() - t).toFixed(1)} ms, ${html.length} bytes`);
const positions = JSON.parse(owner.benchPositions());
for (const pos of positions.reverse()) {
  const facade = [];
  for (let i = 0; i < 300; i++) {
    t = performance.now();
    owner.facadeInput(JSON.stringify({ block: pos.block, at: pos.at + i, input: { type: 'insertText', text: 'e' } }));
    facade.push(performance.now() - t);
  }
  const back = [];
  for (let i = 0; i < 50; i++) {
    t = performance.now();
    owner.facadeInput(JSON.stringify({ block: pos.block, at: pos.at + 300 - i, input: { type: 'deleteBackward' } }));
    back.push(performance.now() - t);
  }
  const source = [];
  for (let i = 0; i < 300; i++) {
    t = performance.now();
    owner.sourceInput(JSON.stringify({ at: pos.source + i, input: { type: 'insertText', text: 'e' } }));
    source.push(performance.now() - t);
  }
  console.log(`${pos.name} facade type: ${fmt(facade)}`);
  console.log(`${pos.name} facade backspace: ${fmt(back)}`);
  console.log(`${pos.name} source type: ${fmt(source)}`);
}

# Spike F0: who owns the note text in the Tauri editor

Throw-away spike for `docs/migration/plan.md` section 6 and ADR-021. It measures typing latency in a
10,000-line note for two owners of the text:

- **A, backend-authoritative:** the rope (`underline_editor_core::Document`) and the facade blocks live in
  the Tauri backend; every input is an IPC command that returns a patch.
- **B, WASM core in the webview:** the same owner code, compiled to `wasm32-unknown-unknown` with
  wasm-bindgen, runs in the webview; edits are mirrored to the backend after the frame with a base revision.

Results and the recommendation are in [`RESULTS.md`](RESULTS.md).

## Layout

| Path | What |
|---|---|
| `Cargo.toml` | Own `[workspace]`; the root workspace does not see the spike. |
| `model/` | `f0-model`: the owner (`owner.rs`), the test note (`note.rs`), block HTML (`render.rs`), the backend mirror of B (`mirror.rs`). With the feature `real-facade` it uses `underline_facade::blocks` (A); without it `blocks_copy.rs`, a verbatim copy that builds for wasm32 (B). |
| `wasm/` | `f0-wasm`: wasm-bindgen wrapper of the owner (B). |
| `src-tauri/` | `f0-editor`: Tauri 2 shell with the commands of A and the mirror of B. |
| `ui/` | Svelte 5 + TypeScript + Vite; `src/lib/sourceView.ts` (CodeMirror 6 as view only), `src/lib/facadeView.ts` (own `contenteditable`), `src/lib/bench.ts` (driver, measurement, checks), `src/lib/diag.ts` (the facade's DOM cost taken apart). |
| `run.sh` | Builds everything and runs the measurement once. |
| `summarize.py` | Turns a results JSON into the Markdown tables of `RESULTS.md`. |
| `results/` | Results JSON of the runs. |

## Prerequisites

- Rust with the `wasm32-unknown-unknown` target, Node from nvm, the Tauri Linux packages (see the root
  `CLAUDE.md`).
- `wasm-bindgen-cli` matching the crate version, installed locally (not into `~/.cargo`):

  ```sh
  cargo install wasm-bindgen-cli --version 0.2.130 --locked --root spikes/f0-editor/.tools
  ```

- `cd ui && npm install`

## Run

```sh
cd spikes/f0-editor
./run.sh                                  # everything: A, B, A-async, extras, checks (about 10 min)
./run.sh F0_KEYS=100 F0_VARIANTS=A,B      # fewer keys, some owners
./run.sh F0_NO_EXTRAS=1 F0_NO_CHECKS=1    # no CM6 highlighting run, no correctness checks
./run.sh F0_DIAG=0                        # only take the facade's DOM work apart (F0_DIAG=200: 200 blocks)
python3 -I summarize.py results/run-<time>.json
```

The window opens on the desktop for the whole run and closes itself; keep it visible (a covered or minimized
window gets no animation frames, and the run counts them as stalls).

Interactive: `./target/release/f0-editor` after `run.sh` built it (or `cargo tauri dev` in `src-tauri`
with `npm run dev` serving port 1430). Choose the owner and the view at the top; typing goes to the owner.

## Checks

```sh
cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
cd ui && npm run check && npm run format:check
```

`cargo test -p f0-model` (without `real-facade`) compares the copied blocks with `underline-facade` on the
test note.

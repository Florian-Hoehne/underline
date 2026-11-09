#!/usr/bin/env bash
# Builds the spike (WASM core, frontend, release binary) and runs the
# measurement once. Extra arguments are passed as environment, e.g.
#   ./run.sh F0_KEYS=100 F0_VARIANTS=A,B F0_NO_EXTRAS=1
set -euo pipefail
cd "$(dirname "$0")"
export PATH="$HOME/.cargo/bin:$PATH"
# shellcheck disable=SC1091
source "$HOME/.nvm/nvm.sh" >/dev/null
(cd ui && npm run --silent wasm && npm run --silent build)
# The frontend is embedded at compile time; make sure it is picked up.
touch src-tauri/src/main.rs
cargo build -p f0-editor --release --features custom-protocol
mkdir -p results
out="$PWD/results/run-$(date +%Y%m%d-%H%M%S).json"
env F0_AUTORUN=1 F0_OUT="$out" "$@" ./target/release/f0-editor
echo "results: $out"

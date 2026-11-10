// ADR-025, docs/migration/variante-b.md section 5: builds the editor's WASM
// (`crates/underline-editor-wasm`) and its JavaScript glue into
// `src/lib/wasm/pkg/`, which is generated, not checked in. The npm hooks
// `predev`, `prebuild`, `precheck` and `pretest` run it; without a change,
// cargo is done in about a second.
//
// The wasm-bindgen CLI must be the version `Cargo.lock` pins. It is looked
// for in the repository's `.tools/bin` (installed with the command below),
// then in `UL_WASM_BINDGEN`, then on the PATH.
import { execFileSync } from 'node:child_process';
import os from 'node:os';
import { existsSync, readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

const ui = fileURLToPath(new URL('..', import.meta.url));
const root = path.resolve(ui, '../../..');
const exe = process.platform === 'win32' ? '.exe' : '';
const out = path.join(ui, 'src', 'lib', 'wasm', 'pkg');
const wasm = path.join(
  root,
  'target',
  'wasm32-unknown-unknown',
  'release',
  'underline_editor_wasm.wasm',
);

/** The one version of wasm-bindgen `Cargo.lock` pins. */
function pinned() {
  const lock = readFileSync(path.join(root, 'Cargo.lock'), 'utf8');
  const versions = [...lock.matchAll(/name = "wasm-bindgen"\r?\nversion = "([^"]+)"/g)].map(
    (found) => found[1],
  );
  if (versions.length !== 1) {
    console.error(
      versions.length === 0
        ? 'Cargo.lock names no version of wasm-bindgen'
        : `Cargo.lock holds more than one wasm-bindgen (${versions.join(', ')}); the CLI fits only one`,
    );
    process.exit(1);
  }
  return versions[0];
}

/** The wasm-bindgen CLI to run, and its version. */
function cli(version) {
  const candidates = [
    path.join(root, '.tools', 'bin', `wasm-bindgen${exe}`),
    process.env.UL_WASM_BINDGEN,
    `wasm-bindgen${exe}`,
  ].filter(Boolean);
  for (const candidate of candidates) {
    const isPath =
      path.isAbsolute(candidate) || candidate.includes('/') || candidate.includes(path.sep);
    if (isPath && !existsSync(candidate)) {
      continue;
    }
    try {
      const said = execFileSync(candidate, ['--version'], { encoding: 'utf8' }).trim();
      if (said === `wasm-bindgen ${version}`) {
        return candidate;
      }
      console.error(`${candidate} is ${said}, not ${version}`);
    } catch {
      // Not there; the next one.
    }
  }
  console.error(
    `wasm-bindgen ${version} is missing. Install it with:\n` +
      `  cargo install wasm-bindgen-cli --version ${version} --locked --root .tools\n` +
      `in ${root}, and the target with: rustup target add wasm32-unknown-unknown`,
  );
  process.exit(1);
}

const bindgen = cli(pinned());
function run(command, args, hint) {
  try {
    execFileSync(command, args, { cwd: root, stdio: 'inherit' });
  } catch (error) {
    if (error.code === 'ENOENT') {
      console.error(`${command} was not found; is it on the PATH (for Rust: ~/.cargo/bin)?`);
    } else if (hint) {
      console.error(hint);
    }
    process.exit(1);
  }
}
/** Cargo: `CARGO`, else on the PATH, else where rustup puts it. */
function cargo() {
  if (process.env.CARGO) {
    return process.env.CARGO;
  }
  try {
    execFileSync(`cargo${exe}`, ['--version'], { stdio: 'ignore' });
    return `cargo${exe}`;
  } catch {
    const home = process.env.CARGO_HOME ?? path.join(os.homedir(), '.cargo');
    return path.join(home, 'bin', `cargo${exe}`);
  }
}

run(
  cargo(),
  [
    'build',
    '--locked',
    '-p',
    'underline-editor-wasm',
    '--release',
    '--target',
    'wasm32-unknown-unknown',
  ],
  'Building the WASM failed. Without the target, add it with: rustup target add wasm32-unknown-unknown',
);
run(bindgen, ['--target', 'web', '--out-dir', out, wasm]);

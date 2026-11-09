// Runs the Tauri CLI from `ui/`. The CLI looks for `tauri.conf.json` only
// below the current folder, so it is told where the Rust crate and the
// frontend are.
import { fileURLToPath } from 'node:url';
import { run } from '@tauri-apps/cli';

process.env.TAURI_APP_PATH ??= fileURLToPath(new URL('../../src-tauri', import.meta.url));
process.env.TAURI_FRONTEND_PATH ??= fileURLToPath(new URL('..', import.meta.url));

await run(process.argv.slice(2), 'npm run tauri');

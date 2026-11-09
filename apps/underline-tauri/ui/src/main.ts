import { mount, tick } from 'svelte';
import App from './App.svelte';
import { insideTauri } from './lib/mock-backend';
import { Shell } from './lib/shell.svelte';
import './app.css';

// Asked before the mock is installed, which makes every page look like Tauri.
const inTauri = insideTauri();

// Outside the Tauri window (a plain browser on the dev server), a mock
// backend answers the commands (plan section 10).
if (import.meta.env.DEV && !inTauri) {
  const { installMockBackend } = await import('./lib/mock-backend');
  installMockBackend();
}

const target = document.getElementById('app');
if (!target) {
  throw new Error('index.html lacks the #app element');
}

// The window starts hidden (tauri.conf.json). The theme's colours are set on
// <html> and the fonts loaded before the first render, and the window is
// shown after it, so it never shows other colours or fonts (plan section 14).
const shell = new Shell();
await Promise.all([shell.connect(), loadFonts()]);
const app = mount(App, { target, props: { shell } });
await tick();
// M-01.1: the workspace to start with opens while the window shows its
// loading state.
void shell.startWorkspace();
// Not in requestAnimationFrame: WebKitGTK runs no frames for a hidden window.
if (inTauri) {
  void showWindow();
}

async function loadFonts(): Promise<void> {
  try {
    await Promise.all([
      document.fonts.load('14px "DejaVu Sans"'),
      document.fonts.load('bold 14px "DejaVu Sans"'),
    ]);
  } catch (error) {
    console.error('The fonts could not be loaded', error);
  }
}

async function showWindow(): Promise<void> {
  try {
    const { getCurrentWindow } = await import('@tauri-apps/api/window');
    await getCurrentWindow().show();
  } catch (error) {
    console.error('The window could not be shown', error);
  }
}

export default app;

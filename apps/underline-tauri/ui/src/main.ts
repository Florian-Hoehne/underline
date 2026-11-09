import { mount } from 'svelte';
import App from './App.svelte';
import { insideTauri } from './lib/mock-backend';
import './app.css';

// Outside the Tauri window (a plain browser on the dev server), a mock
// backend answers the commands (plan section 10).
if (import.meta.env.DEV && !insideTauri()) {
  const { installMockBackend } = await import('./lib/mock-backend');
  installMockBackend();
}

const target = document.getElementById('app');
if (!target) {
  throw new Error('index.html lacks the #app element');
}

export default mount(App, { target });

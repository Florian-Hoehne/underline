import { mount } from 'svelte';
import App from './App.svelte';
import './app.css';
import { invoke } from '@tauri-apps/api/core';

// No devtools in a release build: errors go to the terminal.
const report = (what: string) => void invoke('log', { message: `ERROR ${what}` });
window.addEventListener('error', (e) => report(`${e.message} at ${e.filename}:${e.lineno}`));
window.addEventListener('unhandledrejection', (e) => report(String(e.reason?.stack ?? e.reason)));

const target = document.getElementById('app');
if (!target) throw new Error('index.html lacks #app');
export default mount(App, { target });

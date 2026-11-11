import { mount } from 'svelte';
import './app.css';
import App from './App.svelte';

// No native context menu (it offers things like "Print" that make no sense for a video player).
// Kept in dev builds so the inspector stays reachable.
if (!import.meta.env.DEV) {
  document.addEventListener('contextmenu', (e) => e.preventDefault());
}

const app = mount(App, { target: document.getElementById('app')! });

export default app;

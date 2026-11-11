import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';

export default defineConfig({
  plugins: [svelte()],
  clearScreen: false,
  server: { port: 1420, strictPort: true, watch: { ignored: ['**/src-tauri/**', '**/core/**', '**/target/**'] } },
  build: { target: 'es2021', outDir: 'dist', emptyOutDir: true },
});

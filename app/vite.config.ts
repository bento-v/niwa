import { svelte } from '@sveltejs/vite-plugin-svelte';
import { defineConfig } from 'vite';

export default defineConfig({
  plugins: [svelte()],
  // Tauri expects a fixed port and reports Rust errors itself.
  clearScreen: false,
  server: { port: 1420, strictPort: true },
  build: { target: 'esnext' },
});

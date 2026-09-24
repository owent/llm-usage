import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';

export default defineConfig({
  plugins: [svelte()],
  clearScreen: false,
  server: {
    host: '127.0.0.1',
    port: 1420,
    strictPort: true,
    watch: {
      ignored: ['**/src-tauri/target/**']
    }
  },
  build: {
    target: 'es2022',
    chunkSizeWarningLimit: 2000
  },
  envPrefix: ['VITE_', 'TAURI_']
});

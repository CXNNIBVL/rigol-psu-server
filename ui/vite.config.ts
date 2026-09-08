import { defineConfig } from 'vitest/config';
import { svelte } from '@sveltejs/vite-plugin-svelte';
import tailwindcss from '@tailwindcss/vite';

export default defineConfig({
  plugins: [tailwindcss(), svelte()],
  // Under Vitest, resolve Svelte's browser build. Without this the component
  // tests get the server renderer, where `mount()` does not exist.
  resolve: process.env.VITEST ? { conditions: ['browser'] } : undefined,
  build: {
    // Embedded into the Rust binary by rust-embed at compile time.
    outDir: 'dist',
    emptyOutDir: true,
  },
  server: {
    // `npm run dev` talks to a server started separately with `just dev`.
    proxy: {
      '/api': {
        target: 'http://127.0.0.1:8080',
        ws: true,
      },
    },
  },
  test: {
    environment: 'jsdom',
    globals: true,
    setupFiles: ['./vitest.setup.ts'],
    // Component tests only. The e2e/ specs are Playwright's and would fail
    // here with a confusing error about afterEach.
    include: ['src/**/*.test.ts'],
  },
});

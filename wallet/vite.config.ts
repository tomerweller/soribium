/// <reference types="vitest/config" />
import react from '@vitejs/plugin-react';
import { defineConfig } from 'vite';

export default defineConfig({
  plugins: [react()],
  server: {
    // Same-origin /api during dev → the mock server (npm run dev:mock) or a
    // real sequencer. In production nginx does this proxy.
    proxy: {
      '/api': { target: 'http://localhost:8787', changeOrigin: true, rewrite: (p) => p.replace(/^\/api/, '') },
    },
  },
  test: {
    environment: 'node',
  },
});

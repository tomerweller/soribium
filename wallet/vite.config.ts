/// <reference types="vitest/config" />
import react from '@vitejs/plugin-react';
import { defineConfig, type Plugin } from 'vite';

/**
 * Inject a Content-Security-Policy <meta> into production builds only
 * (dev needs HMR websockets/inline that CSP would break). GitHub Pages
 * can't set response headers, so the meta tag is the only delivery there;
 * it materially limits what an XSS can do around the localStorage keys
 * (security issue #2, H3/M5). connect-src covers the sequencer plus the
 * Soroban RPC the deposit flow talks to.
 */
function cspPlugin(): Plugin {
  const sequencer = process.env.VITE_SEQUENCER_URL || '';
  const connect = ["'self'", sequencer, 'https://soroban-testnet.stellar.org']
    .filter(Boolean)
    .join(' ');
  const csp = [
    "default-src 'self'",
    "script-src 'self'",
    "style-src 'self' 'unsafe-inline'", // React style attributes
    "img-src 'self' data:",
    "font-src 'self'",
    `connect-src ${connect}`,
    "object-src 'none'",
    "base-uri 'none'",
    "form-action 'self'",
  ].join('; ');
  return {
    name: 'inject-csp',
    apply: 'build',
    transformIndexHtml: (html) =>
      html.replace('<head>', `<head>\n    <meta http-equiv="Content-Security-Policy" content="${csp}" />`),
  };
}

export default defineConfig({
  // Subpath hosting (GitHub Pages serves at /soribium/); CI sets BASE_PATH.
  base: process.env.BASE_PATH ?? '/',
  plugins: [react(), cspPlugin()],
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

// app/vite.config.ts — Interface Prebench (Svelte 5), servie par Vite en développement.
//
// Les composants vivent hors de app/ (components/<id>/) : le serveur de
// développement est autorisé à lire la racine du dépôt.

import { svelte } from '@sveltejs/vite-plugin-svelte';
import { fileURLToPath } from 'node:url';
import { defineConfig } from 'vite';

const repoRoot = fileURLToPath(new URL('..', import.meta.url));

export default defineConfig({
  plugins: [svelte()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    fs: { allow: [repoRoot] },
  },
  build: {
    // WebKit (macOS) et WebView2 (Windows) : pas de fonctions trop récentes (section 12.2).
    target: ['es2021', 'chrome105', 'safari15'],
    outDir: 'dist',
    emptyOutDir: true,
  },
  worker: { format: 'es' },
});

// app/vite.config.ts — Interface Prebench (Svelte 5), servie par Vite en développement.
//
// Les composants vivent hors de app/ (components/<id>/) : le serveur de
// développement est autorisé à lire la racine du dépôt.

import { svelte } from '@sveltejs/vite-plugin-svelte';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { defineConfig } from 'vite';

const repoRoot = fileURLToPath(new URL('..', import.meta.url));
const pkg = JSON.parse(readFileSync(new URL('./package.json', import.meta.url), 'utf8')) as { version: string };

export default defineConfig({
  plugins: [svelte()],
  define: { __APP_VERSION__: JSON.stringify(pkg.version) },
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

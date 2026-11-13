// app/playwright.config.ts — Tests de l'interface dans un navigateur headless (phase 2, étape 1).
//
// L'interface construite (dist/, `npm run build`) est servie par `vite preview`
// et ouverte dans Chromium (moteur de WebView2, Windows) et WebKit (moteur de
// WKWebView, macOS). Le cœur Rust est remplacé par une imitation de l'IPC Tauri
// (tests-ui/fixture.ts). Exécuté uniquement dans la CI Linux.
//
//   npm run build && npm run test:ui

import { defineConfig, devices } from '@playwright/test';

const PORT = 4173;

export default defineConfig({
  testDir: 'tests-ui',
  outputDir: 'test-results',
  forbidOnly: !!process.env.CI,
  retries: 0,
  reporter: process.env.CI ? [['list'], ['github']] : 'list',
  use: {
    baseURL: `http://127.0.0.1:${PORT}`,
    viewport: { width: 1280, height: 800 },
    locale: 'fr-FR',
    trace: 'retain-on-failure',
  },
  projects: [
    { name: 'chromium', use: { ...devices['Desktop Chrome'], viewport: { width: 1280, height: 800 } } },
    { name: 'webkit', use: { ...devices['Desktop Safari'], viewport: { width: 1280, height: 800 } } },
  ],
  webServer: {
    command: `npx vite preview --host 127.0.0.1 --port ${PORT} --strictPort`,
    url: `http://127.0.0.1:${PORT}`,
    reuseExistingServer: !process.env.CI,
    timeout: 30_000,
  },
});

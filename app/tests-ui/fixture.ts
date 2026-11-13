// app/tests-ui/fixture.ts — Interface ouverte dans un navigateur, cœur Rust imité.
//
// - L'IPC Tauri (window.__TAURI_INTERNALS__) est remplacée par une imitation :
//   chaque commande répond avec des données fixes, chaque appel est enregistré,
//   et le test émet lui-même les événements du cœur (sim:states, sim:status…).
// - Le projet ouvert reprend la Nucleo-F401RE et les composants réellement
//   fournis (components/<id>/manifest.json et view.svelte lus sur le disque) :
//   les vues sont donc compilées à l'exécution, comme dans l'app.
// - La CSP de tauri.conf.json est appliquée à la page.
// - Toute erreur JavaScript ou de console fait échouer le test.

import { test as base, expect, type Page } from '@playwright/test';
import { existsSync, readdirSync, readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

const repoRoot = fileURLToPath(new URL('../../', import.meta.url));
const componentsDir = `${repoRoot}components/`;

type Json = null | boolean | number | string | Json[] | { [key: string]: Json };

interface ManifestJson {
  id: string;
  params?: Record<string, { default?: Json }>;
  [key: string]: Json | undefined;
}

/** Composants fournis qui ont une vue (components/<id>/view.svelte). */
export function providedViews(): string[] {
  return readdirSync(componentsDir)
    .filter((id) => existsSync(`${componentsDir}${id}/view.svelte`))
    .sort();
}

function component(id: string): { info: Json; defaults: Record<string, Json> } {
  const manifest = JSON.parse(readFileSync(`${componentsDir}${id}/manifest.json`, 'utf8')) as ManifestJson;
  const defaults: Record<string, Json> = {};
  for (const [name, p] of Object.entries(manifest.params ?? {})) {
    if (p.default !== undefined) defaults[name] = p.default;
  }
  const info = {
    id,
    origin: 'provided',
    manifest: manifest as unknown as Json,
    view_source: readFileSync(`${componentsDir}${id}/view.svelte`, 'utf8'),
  };
  return { info, defaults };
}

export const PROJECT_PATH = '/Users/demo/Documents/Prebench/blinky-stm32-hal';

/**
 * Projet de démonstration : LD2 et B1 intégrés à la carte (comme
 * boards/nucleo-f401re.toml), puis une instance de chaque autre composant fourni.
 */
export function demoProject(): Json {
  const ids = providedViews();
  const components: Record<string, Json> = {};
  const defaults: Record<string, Record<string, Json>> = {};
  for (const id of ids) {
    const c = component(id);
    components[id] = c.info;
    defaults[id] = c.defaults;
  }
  const instances: Json[] = [
    { id: 'LD2', type: 'led', params: { ...defaults.led, color: 'green' }, pos: null },
    { id: 'B1', type: 'button', params: { ...defaults.button, color: 'blue' }, pos: null },
  ];
  ids
    .filter((id) => id !== 'led' && id !== 'button')
    .forEach((id, i) => instances.push({ id, type: id, params: defaults[id] ?? {}, pos: [520 + i * 140, 80] }));
  return {
    path: PROJECT_PATH,
    framework: 'stm32-hal',
    board_name: 'Nucleo-F401RE',
    mcu: 'STM32F401RE',
    instances,
    components,
    warnings: [],
  };
}

/** Réponses des commandes du cœur. */
function responses(): Record<string, Json> {
  return {
    default_project: PROJECT_PATH,
    open_project: demoProject(),
    check_prerequisites: {
      tools: [
        { kind: 'cmake', label: 'CMake', path: '/opt/homebrew/bin/cmake', install: 'brew install cmake' },
        { kind: 'compiler', label: 'Compilateur C (clang)', path: '/usr/bin/clang', install: 'xcode-select --install' },
      ],
      readme: 'README, section « Prérequis »',
    },
    check_update: null,
    list_templates: [{ id: 'blinky-stm32-hal', suggested_dir: PROJECT_PATH }],
    start_simulation: null,
    stop_simulation: null,
    reset_simulation: null,
    pause_simulation: null,
    resume_simulation: null,
    component_control: null,
    open_in_editor: null,
    start_debug: {
      config_name: 'Prebench : déboguer le firmware',
      launch_json: `${PROJECT_PATH}/.vscode/launch.json`,
      launch_json_state: 'created',
      launch_json_error: null,
      tools: [
        { label: 'CodeLLDB (extension VS Code)', present: false, path: null, install: 'code --install-extension vadimcn.vscode-lldb' },
      ],
    },
  };
}

/** Exécuté dans la page avant tout script : imitation minimale de l'IPC Tauri 2. */
function installTauriMock(json: string): void {
  const data = JSON.parse(json) as Record<string, unknown>;
  type Callback = (payload: unknown) => void;
  const callbacks = new Map<number, Callback>();
  const listeners = new Map<string, number[]>();
  const calls: { cmd: string; args: unknown }[] = [];
  let nextId = 1;
  const w = window as unknown as Record<string, unknown>;
  w.__pbCalls = calls;
  w.__TAURI_INTERNALS__ = {
    metadata: { currentWindow: { label: 'main' }, currentWebview: { windowLabel: 'main', label: 'main' } },
    transformCallback(cb: Callback | undefined, once = false): number {
      const id = nextId++;
      callbacks.set(id, (payload) => {
        if (once) callbacks.delete(id);
        cb?.(payload);
      });
      return id;
    },
    unregisterCallback(id: number): void {
      callbacks.delete(id);
    },
    invoke(cmd: string, args: unknown): Promise<unknown> {
      calls.push({ cmd, args });
      if (cmd === 'plugin:event|listen') {
        const a = args as { event: string; handler: number };
        const ids = listeners.get(a.event) ?? [];
        ids.push(a.handler);
        listeners.set(a.event, ids);
        return Promise.resolve(a.handler);
      }
      if (cmd === 'plugin:event|unlisten') return Promise.resolve(null);
      if (cmd in data) return Promise.resolve(structuredClone(data[cmd]));
      return Promise.reject(new Error(`commande non imitée : ${cmd}`));
    },
  };
  w.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener(): void {} };
  w.__pbEmit = (event: string, payload: unknown): void => {
    for (const id of listeners.get(event) ?? []) callbacks.get(id)?.({ event, id, payload });
  };
}

function tauriCsp(): string {
  const conf = JSON.parse(readFileSync(`${repoRoot}app/src-tauri/tauri.conf.json`, 'utf8')) as {
    app: { security: { csp: string } };
  };
  return conf.app.security.csp;
}

export interface Bench {
  page: Page;
  /** Émet un événement du cœur vers l'interface. */
  emit(event: string, payload: unknown): Promise<void>;
  /** Commandes appelées par l'interface (hors événements). */
  calls(cmd: string): Promise<unknown[]>;
  /** Figure d'un composant de la paillasse, par identifiant d'instance. */
  figure(id: string): ReturnType<Page['locator']>;
}

export const test = base.extend<{ bench: Bench; errors: string[] }>({
  errors: async ({ page }, use) => {
    const errors: string[] = [];
    page.on('pageerror', (e) => errors.push(`pageerror : ${e.message}`));
    page.on('console', (m) => {
      if (m.type() === 'error') errors.push(`console : ${m.text()}`);
    });
    await use(errors);
    expect(errors, 'erreurs JavaScript dans la page').toEqual([]);
  },
  bench: async ({ page, errors }, use) => {
    void errors; // active la surveillance des erreurs pour chaque test
    const csp = tauriCsp();
    await page.route(
      (url) => url.pathname === '/' || url.pathname.endsWith('.html'),
      async (route) => {
        const response = await route.fetch();
        await route.fulfill({ response, headers: { ...response.headers(), 'content-security-policy': csp } });
      },
    );
    await page.addInitScript(installTauriMock, JSON.stringify(responses()));
    await page.goto('/');
    // Le projet par défaut est ouvert au démarrage, après l'abonnement aux événements.
    await page.waitForFunction(() =>
      (window as unknown as { __pbCalls: { cmd: string }[] }).__pbCalls.some((c) => c.cmd === 'open_project'),
    );
    await use({
      page,
      // Charge utile passée en JSON : c'est aussi ce que le cœur sérialise.
      emit: (event, payload) =>
        page.evaluate(
          ([e, json]) =>
            (window as unknown as { __pbEmit: (e: string, p: unknown) => void }).__pbEmit(e, JSON.parse(json)),
          [event, JSON.stringify(payload)] as [string, string],
        ),
      calls: (cmd) =>
        page.evaluate(
          (c) =>
            (window as unknown as { __pbCalls: { cmd: string; args: unknown }[] }).__pbCalls
              .filter((x) => x.cmd === c)
              .map((x) => x.args),
          cmd,
        ),
      figure: (id) => page.locator('figure.component').filter({ has: page.getByText(id, { exact: true }) }),
    });
  },
});

export { expect };

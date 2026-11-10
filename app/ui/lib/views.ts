// app/ui/lib/views.ts — Compilation des vues de composants à l'exécution.
//
// Les vues (view.svelte) des composants fournis ET de ceux du projet sont
// compilées ici, dans la WebView, avec le compilateur Svelte embarqué : un
// composant ajouté par l'utilisateur n'exige ni Node ni outil externe (D4).
//
// Le code compilé importe le runtime Svelte (« svelte/internal/client »…) :
// ces imports sont redirigés vers l'instance déjà chargée par l'app, pour que
// vues et app partagent le même runtime. Le module est chargé depuis une URL
// blob: (CSP : script-src 'self' blob:, toujours local).
//
// Les vues ne sont pas isolées : un projet est du code de confiance (11.4).

import type { Component } from 'svelte';
import * as svelte from 'svelte';
import * as svelteInternalClient from 'svelte/internal/client';
import { compileOptions, rewriteImports, shimSource } from './view-module';

/** Modules du runtime partagés avec les vues compilées. */
const SHARED: Record<string, Record<string, unknown>> = {
  svelte,
  'svelte/internal/client': svelteInternalClient,
};

declare global {
  // eslint-disable-next-line no-var
  var __prebenchShared: Record<string, Record<string, unknown>>;
}
globalThis.__prebenchShared = SHARED;

const shimUrls = new Map<string, string>();

function blobUrl(code: string): string {
  return URL.createObjectURL(new Blob([code], { type: 'text/javascript' }));
}

/** Module « shim » qui réexporte un module partagé. */
function shimFor(specifier: string): string {
  const cached = shimUrls.get(specifier);
  if (cached !== undefined) return cached;
  const shared = SHARED[specifier];
  if (shared === undefined) {
    throw new Error(`import non pris en charge dans une vue : ${specifier}`);
  }
  const code = shimSource(specifier, Object.keys(shared));
  const url = blobUrl(code);
  shimUrls.set(specifier, url);
  return url;
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export type ViewComponent = Component<any>;

const cache = new Map<string, Promise<ViewComponent>>();

/** Compile et charge une vue ; rejette en cas d'erreur (la vue de repli prend le relais). */
export function loadView(id: string, source: string): Promise<ViewComponent> {
  const key = `${id}\u0000${source}`;
  const cached = cache.get(key);
  if (cached !== undefined) return cached;
  const promise = (async () => {
    const { compile } = await import('svelte/compiler');
    const { js } = compile(source, compileOptions(id));
    const code = rewriteImports(js.code, shimFor);
    const module = (await import(/* @vite-ignore */ blobUrl(code))) as { default: ViewComponent };
    return module.default;
  })();
  cache.set(key, promise);
  return promise;
}

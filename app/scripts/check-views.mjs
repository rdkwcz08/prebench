// app/scripts/check-views.mjs — Vérifie que chaque vue fournie se compile et s'évalue
// comme un vrai module ES, avec exactement la même préparation que l'interface
// (ui/lib/view-module.ts). Une vue qui échouerait dans la fenêtre échoue ici, en CI.
//
//   npm run test:views

import { existsSync, readdirSync, readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import * as svelte from 'svelte';
import { compile } from 'svelte/compiler';
import * as svelteInternalClient from 'svelte/internal/client';
import { compileOptions, rewriteImports, shimSource } from '../ui/lib/view-module.ts';

globalThis.__prebenchShared = { svelte, 'svelte/internal/client': svelteInternalClient };

const componentsDir = fileURLToPath(new URL('../../components/', import.meta.url));
const dataUrl = (code) => `data:text/javascript;base64,${Buffer.from(code).toString('base64')}`;

const shims = new Map();
function urlFor(specifier) {
  if (!shims.has(specifier)) {
    const shared = globalThis.__prebenchShared[specifier];
    if (shared === undefined) throw new Error(`import non pris en charge dans une vue : ${specifier}`);
    shims.set(specifier, dataUrl(shimSource(specifier, Object.keys(shared))));
  }
  return shims.get(specifier);
}

const message = (e) => (e instanceof Error ? e.message : String(e));
let failures = 0;
let checked = 0;

// Chaque shim doit lui-même être un module valide (ex. export nommé « await »).
for (const specifier of Object.keys(globalThis.__prebenchShared)) {
  try {
    await import(urlFor(specifier));
  } catch (e) {
    failures += 1;
    console.error(`ÉCHEC shim ${specifier} : ${message(e)}`);
  }
}

for (const id of readdirSync(componentsDir).sort()) {
  const viewPath = `${componentsDir}${id}/view.svelte`;
  if (!existsSync(viewPath)) continue;
  checked += 1;
  try {
    const { js } = compile(readFileSync(viewPath, 'utf8'), compileOptions(id));
    const module = await import(dataUrl(rewriteImports(js.code, urlFor)));
    if (typeof module.default !== 'function') throw new Error('export par défaut absent');
    console.log(`ok   ${id}`);
  } catch (e) {
    failures += 1;
    console.error(`ÉCHEC ${id} : ${message(e)}`);
  }
}

console.log(`${checked} vue(s) vérifiée(s), ${failures} échec(s).`);
process.exit(failures === 0 ? 0 : 1);

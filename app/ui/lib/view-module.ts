// app/ui/lib/view-module.ts — Préparation d'une vue compilée pour un chargement en module ES.
//
// Fonctions pures, sans dépendance : utilisées par l'interface (views.ts) et
// par le test de CI (scripts/check-views.mjs), qui évalue les vues comme de
// vrais modules ES. Toute erreur de syntaxe du module généré échoue en CI.

/** Spécificateurs du runtime Svelte importés par le code compilé. */
export const IMPORT_PATTERN = /(from\s+|import\s+)(['"])(svelte(?:\/[^'"]*)?)\2/g;

/** Nom d'export acceptable (IdentifierName ES ; les mots réservés sont permis). */
const EXPORT_NAME = /^[A-Za-z_$][\w$]*$/;

/**
 * Source d'un module « shim » qui réexporte les exports d'un module partagé
 * (globalThis.__prebenchShared[specifier]).
 *
 * Chaque export passe par une liaison locale neutre (__e0, __e1…) puis est
 * exporté sous son nom : « export { __e0 as await } » est valide, alors que
 * « const { await } = … » est interdit dans un module (le runtime Svelte
 * exporte notamment `await`, pour les blocs {#await}).
 */
export function shimSource(specifier: string, exportNames: string[]): string {
  const names = exportNames.filter((n) => EXPORT_NAME.test(n) && n !== 'default');
  const lines = [`const m = globalThis.__prebenchShared[${JSON.stringify(specifier)}];`];
  names.forEach((name, i) => lines.push(`const __e${i} = m[${JSON.stringify(name)}];`));
  if (names.length > 0) {
    lines.push(`export { ${names.map((name, i) => `__e${i} as ${name}`).join(', ')} };`);
  }
  return `${lines.join('\n')}\n`;
}

/** Remplace les imports du runtime Svelte par les URL des shims. */
export function rewriteImports(code: string, urlFor: (specifier: string) => string): string {
  return code.replace(
    IMPORT_PATTERN,
    (_match, prefix: string, quote: string, specifier: string) => `${prefix}${quote}${urlFor(specifier)}${quote}`,
  );
}

/** Options de compilation communes à l'interface et au test. */
export function compileOptions(id: string) {
  return {
    filename: `${id}/view.svelte`,
    generate: 'client' as const,
    css: 'injected' as const,
    runes: true,
    discloseVersion: false,
    dev: false,
  };
}

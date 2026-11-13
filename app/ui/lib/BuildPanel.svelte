<!--
  app/ui/lib/BuildPanel.svelte — Onglet Build du panneau du bas (section 10.2, charte 5).
  Une phrase de résumé, puis une ligne par erreur ou avertissement : icône de
  sévérité, emplacement `fichier:ligne:colonne` en chasse fixe, cliquable
  (ouvre l'éditeur), puis le message du compilateur.
-->
<script lang="ts">
  import { bench } from './bench.svelte';
  import type { Diagnostic } from './types';
  import Icon from './ui/Icon.svelte';

  const result = $derived(bench.build.result);

  function plural(n: number, one: string, many: string): string {
    return `${n.toLocaleString('fr-FR')} ${n > 1 ? many : one}`;
  }

  function seconds(ms: number): string {
    return `${(ms / 1000).toLocaleString('fr-FR', { minimumFractionDigits: 1, maximumFractionDigits: 1 })} s`;
  }

  const summary = $derived.by(() => {
    if (bench.build.running) return 'Build en cours…';
    if (result === null) {
      return "Aucun build pour l'instant. Le build se lance au démarrage et à chaque sauvegarde dans votre éditeur.";
    }
    const warnings = result.warnings > 0 ? `, ${plural(result.warnings, 'avertissement', 'avertissements')}` : '';
    if (result.ok) return `Build réussi en ${seconds(result.duration_ms)}${warnings}.`;
    if (result.errors === 0) return 'Build en échec. Le détail est dans le Journal.';
    return `Build en échec : ${plural(result.errors, 'erreur', 'erreurs')}${warnings}. Cliquez sur un emplacement pour l'ouvrir dans l'éditeur, corrigez, puis enregistrez.`;
  });

  function location(d: Diagnostic): string {
    return `${d.display ?? ''}:${d.line ?? ''}${d.column !== null ? `:${d.column}` : ''}`;
  }
</script>

<div class="build">
  <p class="summary" class:failed={result !== null && !result.ok && !bench.build.running}>{summary}</p>
  {#if result !== null && result.diagnostics.length > 0}
    <ul>
      {#each result.diagnostics as d, i (i)}
        <li class={d.severity}>
          <span class="sev" aria-label={d.severity === 'error' ? 'Erreur' : 'Avertissement'}>
            <Icon name={d.severity === 'error' ? 'error' : 'warn'} />
          </span>
          {#if d.file !== null && d.line !== null}
            <button class="loc data" title="Ouvrir dans l'éditeur" onclick={() => bench.openInEditor(d)}>{location(d)}</button>
          {/if}
          <span class="msg data">{d.message}</span>
        </li>
      {/each}
    </ul>
  {/if}
</div>

<style>
  .build {
    font-family: var(--pb-font-ui);
    font-size: 13px;
  }
  .summary {
    margin: 0 0 4px;
    line-height: 20px;
    color: var(--pb-text-quiet);
  }
  .summary.failed {
    color: var(--pb-text);
  }
  ul {
    margin: 0;
    padding: 0;
    list-style: none;
  }
  li {
    display: flex;
    gap: 8px;
    align-items: flex-start;
    min-height: 24px;
    padding: 2px 0;
  }
  .sev {
    display: inline-flex;
    padding-top: 2px;
  }
  .error .sev {
    color: var(--pb-conflict);
  }
  .warning .sev {
    color: var(--pb-warn);
  }
  .loc {
    flex: none;
    padding: 0;
    border: 0;
    background: none;
    color: var(--pb-accent);
    font-size: 12px;
    line-height: 20px;
    cursor: pointer;
  }
  .loc:hover {
    text-decoration: underline;
  }
  .msg {
    font-size: 12px;
    line-height: 20px;
    overflow-wrap: anywhere;
  }
</style>

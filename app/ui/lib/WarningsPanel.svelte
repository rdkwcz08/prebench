<!--
  app/ui/lib/WarningsPanel.svelte — Onglet Avertissements (section 9.2, charte 5).
  D'abord les problèmes de câblage (calculés sans simulation : rôle de broche,
  bus, tension, court-circuit…), avec « Afficher » qui sélectionne la broche
  concernée ; puis les avertissements de la simulation (conflit, entrée lue
  flottante, fonction non simulée…). Sévérité : icône et couleur, jamais seules.
-->
<script lang="ts">
  import { bench } from './bench.svelte';
  import { serial } from './serial.svelte';

  const serialWarnings = $derived(serial.ports.map((p) => serial.mismatch(p)).filter((w): w is string => w !== null));
  import Icon from './ui/Icon.svelte';

  function show(endpoint: string): void {
    bench.selection = { kind: 'pin', endpoint };
  }

  /** Avertissement de la simulation qui signale un conflit (court-circuit). */
  function isConflict(text: string): boolean {
    return text.startsWith('Conflit') || text.startsWith('Court-circuit');
  }
</script>

<div class="warnings">
  {#if bench.checks.length === 0 && bench.warnings.length === 0 && serialWarnings.length === 0}
    <p class="empty">Aucun avertissement. Le câblage est vérifié à chaque modification, la simulation pendant qu'elle tourne.</p>
  {/if}
  {#if bench.checks.length > 0}
    <h3>Câblage</h3>
    <ul>
      {#each bench.checks as c, i (i)}
        <li class={c.severity}>
          <span class="sev" aria-label={c.severity === 'conflict' ? 'Conflit' : 'Avertissement'}>
            <Icon name={c.severity === 'conflict' ? 'error' : 'warn'} />
          </span>
          <span class="text">{c.text}</span>
          {#if c.endpoints[0]}
            {@const ep = c.endpoints[0]}
            <button class="show" onclick={() => show(ep)}>Afficher</button>
          {/if}
        </li>
      {/each}
    </ul>
  {/if}
  {#if bench.warnings.length > 0 || serialWarnings.length > 0}
    <h3>Simulation</h3>
    <ul>
      {#each serialWarnings as text (text)}
        <li class="warn">
          <span class="sev" aria-label="Avertissement"><Icon name="warn" /></span>
          <span class="text">{text}</span>
        </li>
      {/each}
      {#each bench.warnings as text (text)}
        {@const conflict = isConflict(text)}
        <li class:conflict class:warn={!conflict}>
          <span class="sev" aria-label={conflict ? 'Conflit' : 'Avertissement'}>
            <Icon name={conflict ? 'error' : 'warn'} />
          </span>
          <span class="text">{text}</span>
        </li>
      {/each}
    </ul>
  {/if}
</div>

<style>
  .warnings {
    font-family: var(--pb-font-ui);
    font-size: 13px;
  }
  h3 {
    margin: 4px 0;
    color: var(--pb-text-quiet);
    font-size: 12px;
    font-weight: 600;
  }
  ul {
    margin: 0 0 8px;
    padding: 0;
    list-style: none;
  }
  li {
    display: flex;
    gap: 8px;
    align-items: flex-start;
    min-height: 24px;
    padding: 2px 0;
    line-height: 20px;
  }
  .sev {
    display: inline-flex;
    padding-top: 2px;
  }
  .warn .sev {
    color: var(--pb-warn);
  }
  .conflict .sev {
    color: var(--pb-conflict);
  }
  .text {
    flex: 1;
    user-select: text;
  }
  .show {
    flex: none;
    padding: 0;
    border: 0;
    background: none;
    color: var(--pb-accent);
    font-size: 12px;
    line-height: 20px;
    cursor: pointer;
  }
  .show:hover {
    text-decoration: underline;
  }
  .empty {
    margin: 0;
    color: var(--pb-text-quiet);
  }
</style>

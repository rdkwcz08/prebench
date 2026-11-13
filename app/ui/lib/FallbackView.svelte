<!--
  app/ui/lib/FallbackView.svelte — Vue de repli d'un composant sans vue utilisable
  (view.svelte absente ou qui ne compile pas) : nom, broches et état brut.
  Le composant reste utilisable ; ses contrôles sont proposés en boutons.
  Bordure en tirets couleur d'avertissement, raison complète au survol.
-->
<script lang="ts">
  import type { Manifest } from './types';

  let {
    manifest,
    state,
    reason,
    control,
  }: {
    manifest: Manifest | undefined;
    state: unknown;
    reason: string;
    control: (id: string, value: unknown) => void;
  } = $props();

  const json = $derived(state === null ? '—' : JSON.stringify(state));
</script>

<div class="fallback" title={reason}>
  <strong>{manifest?.name ?? '?'}</strong>
  <code class="data">{json}</code>
  {#if manifest?.pins.length}
    <span class="pins">Broches <span class="data">{manifest.pins.map((p) => p.name).join(', ')}</span></span>
  {/if}
  {#each manifest?.controls ?? [] as c (c.id)}
    <button
      onpointerdown={() => control(c.id, true)}
      onpointerup={() => control(c.id, false)}>{c.label}</button
    >
  {/each}
</div>

<style>
  .fallback {
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-width: 72px;
    max-width: 160px;
    padding: 4px 8px;
    border: 1px dashed var(--pb-warn);
    border-radius: 3px;
    background: var(--pb-panel);
    color: var(--pb-text);
    font-size: 11px;
  }
  strong {
    font-weight: 600;
  }
  code {
    overflow-wrap: anywhere;
    color: var(--pb-text-quiet);
  }
  .pins {
    color: var(--pb-text-quiet);
  }
  button {
    height: 24px;
    padding: 0 8px;
    border: 1px solid var(--pb-line);
    border-radius: 3px;
    background: transparent;
    font-size: 11px;
    cursor: pointer;
  }
</style>

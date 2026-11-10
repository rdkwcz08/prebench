<!--
  app/ui/lib/FallbackView.svelte — Vue de repli d'un composant sans vue utilisable
  (view.svelte absente ou qui ne compile pas) : nom, broches et état brut.
  Le composant reste utilisable ; ses contrôles sont proposés en boutons.
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
  <code>{json}</code>
  {#if manifest?.pins.length}
    <span class="pins">{manifest.pins.map((p) => p.name).join(' · ')}</span>
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
    gap: 2px;
    min-width: 72px;
    max-width: 160px;
    padding: 4px 6px;
    border: 1px dashed var(--warn);
    border-radius: 6px;
    background: var(--panel);
    font-size: 11px;
  }
  code {
    overflow-wrap: anywhere;
    color: var(--text-muted);
  }
  .pins {
    color: var(--text-muted);
  }
  button {
    font-size: 11px;
  }
</style>

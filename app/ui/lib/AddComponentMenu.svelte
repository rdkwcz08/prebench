<!--
  app/ui/lib/AddComponentMenu.svelte — « Ajouter un composant » (barre d'outils, maquette) :
  menu flottant (charte 5) qui liste le catalogue (composants du projet, puis fournis).
  Le composant est posé au centre de la vue, aimanté, et la paillasse passe en Édition.
  La palette Cmd/Ctrl+K arrive en phase 3.
-->
<script lang="ts">
  import { bench } from './bench.svelte';
  import { freeId, type Point } from './geometry';
  import Button from './ui/Button.svelte';

  let { place, disabled = false }: { place: () => Point; disabled?: boolean } = $props();

  let open = $state(false);
  let root: HTMLDivElement | undefined = $state();

  const catalog = $derived(
    Object.values(bench.project?.components ?? {}).sort((a, b) => a.manifest.name.localeCompare(b.manifest.name, 'fr')),
  );

  async function add(type: string): Promise<void> {
    open = false;
    const used = new Set(bench.instances.map((i) => i.id));
    bench.mode = 'edit';
    await bench.addComponent(type, freeId(type, used), place());
  }

  function onwindowpointerdown(e: PointerEvent): void {
    if (open && root !== undefined && !root.contains(e.target as Node)) open = false;
  }

  function onwindowkeydown(e: KeyboardEvent): void {
    if (open && e.key === 'Escape') open = false;
  }
</script>

<svelte:window onpointerdown={onwindowpointerdown} onkeydown={onwindowkeydown} />

<div class="add" bind:this={root}>
  <Button icon="plus" {disabled} onclick={() => (open = !open)}>Ajouter un composant</Button>
  {#if open}
    <div class="menu" role="menu" aria-label="Composants">
      {#each catalog as c (c.id)}
        <button role="menuitem" onclick={() => add(c.id)}>
          <span>{c.manifest.name}</span>
          <span class="data quiet">{c.id}{c.origin === 'project' ? ', projet' : ''}</span>
        </button>
      {:else}
        <p class="quiet">Aucun composant disponible.</p>
      {/each}
    </div>
  {/if}
</div>

<style>
  .add {
    position: relative;
  }
  .menu {
    position: absolute;
    top: calc(100% + 4px);
    right: 0;
    z-index: 40;
    min-width: 240px;
    padding: 4px 0;
    border: 1px solid var(--pb-line);
    border-radius: 6px;
    background: var(--pb-panel);
    box-shadow: var(--pb-shadow);
  }
  button {
    width: 100%;
    height: 28px;
    padding: 0 12px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    border: 0;
    background: transparent;
    text-align: left;
    cursor: pointer;
  }
  button:hover,
  button:focus-visible {
    background: color-mix(in srgb, var(--pb-accent) 14%, transparent);
  }
  .quiet {
    color: var(--pb-text-quiet);
    font-size: 12px;
  }
  p {
    margin: 0;
    padding: 4px 12px;
  }
</style>

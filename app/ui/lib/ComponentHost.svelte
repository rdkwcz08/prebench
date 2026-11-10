<!--
  app/ui/lib/ComponentHost.svelte — Place un composant sur la paillasse et affiche sa vue
  (compilée à l'exécution) ou la vue de repli. Ne se redessine que si l'état change.
-->
<script lang="ts">
  import type { Instance } from './bench.svelte';
  import FallbackView from './FallbackView.svelte';

  let { inst, mode }: { inst: Instance; mode: 'edit' | 'simulate' } = $props();

  const View = $derived(inst.view);
  const manifest = $derived(inst.info?.manifest);
  const size = $derived(manifest?.view ?? { width: 40, height: 40 });

  function control(id: string, value: unknown): void {
    inst.control(id, value);
  }
</script>

<figure class="component" style:left="{inst.pos[0]}px" style:top="{inst.pos[1]}px">
  {#if View && inst.state !== null}
    <View state={inst.state} params={inst.params} {mode} {control} />
  {:else if View || (inst.viewError === '' && inst.info)}
    <div class="placeholder" style:width="{size.width}px" style:height="{size.height}px"></div>
  {:else}
    <FallbackView {manifest} state={inst.state} reason={inst.viewError} {control} />
  {/if}
  <figcaption>{inst.id}</figcaption>
</figure>

<style>
  .component {
    position: absolute;
    margin: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    user-select: none;
  }
  figcaption {
    font-size: 11px;
    color: var(--text-muted);
    margin-top: 2px;
  }
  .placeholder {
    border: 1px dashed var(--border);
    border-radius: 4px;
  }
</style>

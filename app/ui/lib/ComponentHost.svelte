<!--
  app/ui/lib/ComponentHost.svelte — Place un composant sur la paillasse (coordonnées monde)
  et affiche sa vue (compilée à l'exécution) ou la vue de repli.
  - Simulation : la vue reçoit les clics (B1, potentiomètre).
  - Édition : la vue est inerte ; le composant se déplace (Workbench.svelte, data-component).
  Repère : nom affiché, sinon l'identifiant ; double-clic en Édition pour le modifier.
  Sélection : contour --pb-accent. Composant ajouté pendant la simulation : hachuré,
  « actif au prochain démarrage » (forme + mot, charte 2.3).
-->
<script lang="ts">
  import type { Instance } from './bench.svelte';
  import FallbackView from './FallbackView.svelte';

  let {
    inst,
    mode,
    selected = false,
    pending = false,
    onrename,
  }: {
    inst: Instance;
    mode: 'edit' | 'simulate';
    selected?: boolean;
    pending?: boolean;
    onrename?: (name: string) => void;
  } = $props();

  let renaming = $state(false);

  function startRename(e: MouseEvent): void {
    if (mode !== 'edit' || inst.onboard || !onrename) return;
    e.stopPropagation();
    renaming = true;
  }

  function finish(input: HTMLInputElement): void {
    if (!renaming) return;
    renaming = false;
    onrename?.(input.value);
  }

  function key(e: KeyboardEvent): void {
    e.stopPropagation(); // Supprimer, E, Espace : touches du texte
    if (e.key === 'Enter') finish(e.currentTarget as HTMLInputElement);
    if (e.key === 'Escape') renaming = false;
  }

  function focusOnMount(el: HTMLInputElement): void {
    el.focus();
    el.select();
  }

  const View = $derived(inst.view);
  const manifest = $derived(inst.info?.manifest);
  const size = $derived(manifest?.view ?? { width: 40, height: 40 });

  function control(id: string, value: unknown): void {
    inst.control(id, value);
  }
</script>

<figure
  class="component"
  class:onboard={inst.onboard}
  class:editing={mode === 'edit'}
  class:movable={mode === 'edit' && !inst.onboard}
  class:selected
  data-component={inst.id}
  style:left="{inst.pos[0]}px"
  style:top="{inst.pos[1]}px"
  style:width="{size.width}px"
  style:height="{size.height}px"
>
  <div class="view">
    {#if View && inst.state !== null}
      <View state={inst.state} params={inst.params} {mode} {control} />
    {:else if View || (inst.viewError === '' && inst.info)}
      <div class="placeholder" style:width="{size.width}px" style:height="{size.height}px"></div>
    {:else}
      <FallbackView {manifest} state={inst.state} reason={inst.viewError} {control} />
    {/if}
  </div>
  {#if pending}
    <div class="hatch" aria-hidden="true"></div>
  {/if}
  <figcaption>
    {#if renaming}
      <input
        class="rename"
        value={inst.name}
        placeholder={inst.id}
        maxlength="40"
        aria-label="Nom affiché de {inst.id}"
        use:focusOnMount
        onkeydown={key}
        onblur={(e) => finish(e.currentTarget)}
      />
    {:else}
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <span class="name" class:renamable={mode === 'edit' && !inst.onboard} ondblclick={startRename}>{inst.name || inst.id}</span>
    {/if}
    {#if pending}<span class="pending">actif au prochain démarrage</span>{/if}
  </figcaption>
</figure>

<style>
  .component {
    position: absolute;
    margin: 0;
    user-select: none;
  }
  .view {
    position: relative;
    /* Couche à part : une vue qui change (LED, servo) se recompose seule, sans
       repeindre la carte ni le tapis dessous (section 11.3, wk/banc.sh). */
    will-change: transform;
  }
  .editing .view {
    pointer-events: none; /* en édition, la vue ne réagit pas : on déplace */
  }
  .movable {
    cursor: grab;
  }
  .selected {
    outline: 2px solid var(--pb-accent);
    outline-offset: 4px;
    border-radius: 3px;
  }
  .hatch {
    position: absolute;
    inset: 0;
    pointer-events: none;
    border-radius: 3px;
    background-image: repeating-linear-gradient(
      45deg,
      color-mix(in srgb, var(--pb-mat-ink) 45%, transparent) 0 2px,
      transparent 2px 7px
    );
  }
  figcaption {
    position: absolute;
    top: calc(100% + 4px);
    left: 50%;
    translate: -50% 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    white-space: nowrap;
    color: var(--pb-mat-ink);
    font-size: 12px;
    font-weight: 600;
    pointer-events: none;
  }
  .name.renamable {
    pointer-events: auto;
    cursor: text;
  }
  .rename {
    pointer-events: auto;
    width: 160px;
    height: 24px;
    padding: 0 4px;
    border: 1px solid var(--pb-line);
    border-radius: 3px;
    background: var(--pb-panel);
    color: var(--pb-text);
    font: inherit;
    text-align: center;
    user-select: text;
  }
  .rename:focus {
    outline: 2px solid var(--pb-accent);
    outline-offset: -1px;
  }
  .onboard figcaption {
    color: var(--pb-silk);
    font-size: 11px;
  }
  .pending {
    font-weight: 400;
    font-size: 11px;
  }
  .placeholder {
    border: 1px dashed var(--pb-mat-ink);
    border-radius: 3px;
    opacity: 0.5;
  }
</style>

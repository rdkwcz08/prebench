<!--
  app/ui/lib/ui/PropList.svelte — Liste de propriétés de l'inspecteur (charte, section 5).
  Libellé à gauche en texte secondaire, valeur à droite, en chasse fixe si
  c'est une donnée. Une valeur peut aussi être une pastille d'état.
-->
<script lang="ts" module>
  import type { StateTone } from './StatePill.svelte';

  export interface Prop {
    label: string;
    value: string;
    /** Valeur en chasse fixe (broche, chemin, mesure). */
    data?: boolean;
    /** Valeur affichée en pastille d'état. */
    tone?: StateTone;
    /** Texte complet au survol, pour une valeur tronquée. */
    full?: string;
    /** Échantillon de couleur avant la valeur (couleur d'un fil, charte 2.4). */
    swatch?: string;
  }
</script>

<script lang="ts">
  import StatePill from './StatePill.svelte';

  let { props }: { props: Prop[] } = $props();
</script>

<dl class="props">
  {#each props as p, i (i)}
    <dt>{p.label}</dt>
    <dd class:data={p.data === true} title={p.full}>
      {#if p.swatch}<span class="swatch" style:background={p.swatch}></span>{/if}{#if p.tone}<StatePill
          tone={p.tone}
          text={p.value}
        />{:else}{p.value}{/if}
    </dd>
  {/each}
</dl>

<style>
  .props {
    margin: 0;
    display: grid;
    grid-template-columns: auto minmax(0, 1fr);
    column-gap: 12px;
  }
  dt {
    color: var(--pb-text-quiet);
    line-height: 24px;
    white-space: nowrap;
  }
  dd {
    margin: 0;
    line-height: 24px;
    text-align: right;
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }
  dd.data {
    font-size: 12px;
  }
  /* Maquette : échantillon 10 × 10 avant la couleur d'un fil. */
  .swatch {
    display: inline-block;
    width: 10px;
    height: 10px;
    margin-right: 8px;
    border-radius: 2px;
    vertical-align: -1px;
  }
</style>

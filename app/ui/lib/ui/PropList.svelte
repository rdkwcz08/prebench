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
  }
</script>

<script lang="ts">
  import StatePill from './StatePill.svelte';

  let { props }: { props: Prop[] } = $props();
</script>

<dl class="props">
  {#each props as p (p.label)}
    <dt>{p.label}</dt>
    <dd class:data={p.data === true} title={p.full}>
      {#if p.tone}<StatePill tone={p.tone} text={p.value} />{:else}{p.value}{/if}
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
</style>

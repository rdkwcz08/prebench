<!--
  components/echo/view.svelte — Composant d'écho (diagnostic de latence), dessiné comme
  un petit module sur la paillasse : vernis et sérigraphie de la charte (section 2.2),
  nombre de pings reçus en chasse fixe.
-->
<script lang="ts">
  import type { ComponentViewProps } from '../component-api';
  import type { EchoParams, EchoState } from './model';

  let { state }: ComponentViewProps<EchoParams, EchoState> = $props();

  const count = $derived(state.pings.toLocaleString('fr-FR'));
</script>

<svg width="96" height="40" viewBox="0 0 96 40" role="img" aria-label="Écho : {count} pings reçus">
  <rect x="1" y="1" width="94" height="38" rx="3" class="pcb" />
  <circle cx="8" cy="8" r="2.5" class="hole" />
  <circle cx="88" cy="32" r="2.5" class="hole" />
  <text x="48" y="16" text-anchor="middle" class="silk label">ÉCHO</text>
  <text x="48" y="30" text-anchor="middle" class="silk value">{count} pings</text>
</svg>

<style>
  .pcb {
    fill: var(--pb-board, #1f3b5c);
    stroke: #162c45;
    stroke-width: 2;
  }
  .hole {
    fill: none;
    stroke: var(--pb-copper, #b87333);
    stroke-width: 1.5;
  }
  .silk {
    fill: var(--pb-silk, #e8ecef);
    font-family: var(--pb-font-data, ui-monospace, monospace);
    font-variant-numeric: tabular-nums;
  }
  .label {
    font-size: 8px;
    font-weight: 600;
  }
  .value {
    font-size: 11px;
  }
</style>

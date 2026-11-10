<!--
  components/echo/view.svelte — Rendu du composant d'écho : nombre de pings reçus.
-->
<script lang="ts">
  import type { ComponentViewProps } from '../component-api';
  import type { EchoParams, EchoState } from './model';

  let { state }: ComponentViewProps<EchoParams, EchoState> = $props();
</script>

<svg width="96" height="40" viewBox="0 0 96 40" role="img" aria-label="Écho : {state.pings} pings">
  <rect x="1" y="1" width="94" height="38" rx="6" fill="#263238" stroke="#90a4ae" />
  <text x="48" y="17" text-anchor="middle" font-size="10" fill="#cfd8dc">écho</text>
  <text x="48" y="31" text-anchor="middle" font-size="11" fill="#ffffff">{state.pings} pings</text>
</svg>

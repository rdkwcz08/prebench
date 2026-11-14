<!--
  components/buzzer/view.svelte — Buzzer piézo vu de dessus (Svelte 5).
  Quand il sonne : trois ondes à droite (forme) et la fréquence en chasse fixe
  (« 2,3 kHz »), à l'encre du tapis ; rien ne bouge (charte, section 6). Le son
  réel attendra l'audio (V2).
  Coordonnées identiques au manifeste : 96 × 64, broches + et − en x = 14 et 26.
-->
<script lang="ts">
  import type { ComponentViewProps } from '../component-api';
  import type { BuzzerParams, BuzzerState } from './model';

  let { state, params }: ComponentViewProps<BuzzerParams, BuzzerState> = $props();

  /** « 440 Hz », « 2,3 kHz », « 20 kHz ». */
  function frequency(hz: number): string {
    if (hz < 1000) return `${Math.round(hz).toLocaleString('fr-FR')} Hz`;
    return `${(hz / 1000).toLocaleString('fr-FR', { maximumFractionDigits: 1 })} kHz`;
  }

  const label = $derived(state.sounding && state.freq_hz !== null ? frequency(state.freq_hz) : '');
  const kind = $derived(params.kind === 'active' ? 'actif' : 'passif');
</script>

<svg
  width="96"
  height="64"
  viewBox="0 0 96 64"
  role="img"
  aria-label="Buzzer {kind}, {state.sounding ? `sonne à ${label}` : 'silencieux'}"
>
  <path d="M14 40V64M26 40V64" class="leg" />
  <circle cx="20" cy="22" r="18" class="case" />
  <circle cx="20" cy="22" r="12" class="ring" />
  <circle cx="20" cy="22" r="3" class="hole" />
  <path d="M7 11.5h5M9.5 9v5" class="plus" />
  {#if state.sounding}
    <g class="waves">
      <path d="M43 15a10 10 0 0 1 0 14" />
      <path d="M48 11a16 16 0 0 1 0 22" />
      <path d="M53 7a22 22 0 0 1 0 30" />
    </g>
    <text x="60" y="26" class="freq">{label}</text>
  {/if}
</svg>

<style>
  .leg {
    stroke: #b9c0c5;
    stroke-width: 2;
  }
  .case {
    fill: #1c1d1f;
  }
  .ring {
    fill: none;
    stroke: #3a3c40;
    stroke-width: 1.5;
  }
  .hole {
    fill: #000000;
  }
  .plus {
    stroke: #e8ecef;
    stroke-width: 1.5;
  }
  .waves path {
    fill: none;
    stroke: var(--pb-mat-ink);
    stroke-width: 1.5;
    stroke-linecap: round;
  }
  .freq {
    fill: var(--pb-mat-ink);
    font: 400 11px var(--pb-font-data, ui-monospace, monospace);
    font-variant-numeric: tabular-nums;
  }
</style>

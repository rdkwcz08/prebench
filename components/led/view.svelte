<!--
  components/led/view.svelte — Rendu de la LED (Svelte 5).
  Redessiné uniquement quand `state` change (le modèle appelle invalidate()).
  Coordonnées identiques au manifeste : 40 × 80, anode en x = 14, cathode en x = 26.
-->
<script lang="ts">
  import type { ComponentViewProps } from '../component-api';
  import type { LedColor, LedParams, LedState } from './model';

  let { state, params }: ComponentViewProps<LedParams, LedState> = $props();

  const COLORS: Record<LedColor, { on: string; off: string }> = {
    red: { on: '#ff3b30', off: '#5c1a17' },
    green: { on: '#34c759', off: '#17421f' },
    blue: { on: '#0a84ff', off: '#16305a' },
    yellow: { on: '#ffd60a', off: '#5a4c0c' },
    orange: { on: '#ff9f0a', off: '#5a3a0c' },
    white: { on: '#ffffff', off: '#5f5f5f' },
  };

  const palette = $derived(COLORS[params.color] ?? COLORS.red);
  const glow = $derived(Math.min(1, Math.max(0, state.brightness)));
</script>

<svg width="40" height="80" viewBox="0 0 40 80" role="img" aria-label="LED {params.color}">
  <!-- Pattes : anode plus longue, comme une vraie LED -->
  <line x1="14" y1="44" x2="14" y2="80" stroke="#9a9a9a" stroke-width="2" />
  <line x1="26" y1="44" x2="26" y2="74" stroke="#9a9a9a" stroke-width="2" />
  <line x1="26" y1="74" x2="26" y2="80" stroke="#9a9a9a" stroke-width="2" />

  <!-- Halo proportionnel à la luminosité -->
  {#if glow > 0}
    <circle cx="20" cy="22" r="19" fill={palette.on} opacity={0.35 * glow} />
  {/if}

  <!-- Corps : couleur éteinte, puis couleur allumée par-dessus -->
  <path d="M8 44 V18 a12 12 0 0 1 24 0 V44 Z" fill={palette.off} />
  <path d="M8 44 V18 a12 12 0 0 1 24 0 V44 Z" fill={palette.on} opacity={glow} />
  <rect x="6" y="42" width="28" height="4" rx="1" fill={palette.off} />
  <path d="M13 14 a8 8 0 0 1 6 -5" stroke="#ffffff" stroke-width="2" fill="none" opacity="0.5" />
</svg>

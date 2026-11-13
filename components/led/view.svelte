<!--
  components/led/view.svelte — LED 5 mm vue de dessus (Svelte 5), style de la maquette
  (docs/design/maquette-ecran-principal.html, LED1).
  Redessinée uniquement quand `state` change (le modèle appelle invalidate()).
  Coordonnées identiques au manifeste : 40 × 80, anode en x = 14, cathode en x = 26.
  Halo de 80 ms à l'allumage, extinction sans transition (charte, section 6).
-->
<script lang="ts">
  import type { ComponentViewProps } from '../component-api';
  import type { LedColor, LedParams, LedState } from './model';

  let { state, params }: ComponentViewProps<LedParams, LedState> = $props();

  /** Matériaux : couleur allumée, éteinte et halo de chaque LED. */
  const COLORS: Record<LedColor, { on: string; off: string; glow: string; name: string }> = {
    red: { on: '#ff5a4a', off: '#6e2a26', glow: '#ff6a5c', name: 'rouge' },
    green: { on: '#5cf59a', off: '#2f5e44', glow: '#7cffb0', name: 'verte' },
    blue: { on: '#5aa8ff', off: '#22406a', glow: '#7cbcff', name: 'bleue' },
    yellow: { on: '#ffd84a', off: '#6a5a1c', glow: '#ffe27c', name: 'jaune' },
    orange: { on: '#ffa04a', off: '#6e4220', glow: '#ffb47c', name: 'orange' },
    white: { on: '#f4f8ff', off: '#6b7076', glow: '#ffffff', name: 'blanche' },
  };

  const uid = $props.id();
  const palette = $derived(COLORS[params.color] ?? COLORS.red);
  const level = $derived(Math.min(1, Math.max(0, state.brightness)));
  const lit = $derived(level > 0);
</script>

<svg
  width="40"
  height="80"
  viewBox="0 0 40 80"
  class:lit
  role="img"
  aria-label="LED {palette.name}, {lit ? `allumée à ${Math.round(level * 100)} %` : 'éteinte'}"
>
  <defs>
    <radialGradient id="glow-{uid}">
      <stop offset="0" stop-color={palette.glow} stop-opacity="0.9" />
      <stop offset="1" stop-color={palette.glow} stop-opacity="0" />
    </radialGradient>
  </defs>
  <!-- Pattes : l'anode (gauche) dépasse, comme sur une vraie LED -->
  <path d="M14 34V80M26 34V80" class="leg" />
  <circle cx="20" cy="22" r="20" fill="url(#glow-{uid})" class="glow" opacity={level} />
  <circle cx="20" cy="22" r="13" fill={palette.off} />
  <circle cx="20" cy="22" r="13" fill={palette.on} class="dome" opacity={level} />
  <!-- Méplat de la cathode -->
  <path d="M30.5 14.5a13 13 0 0 1 0 15" class="flat" />
  <circle cx="15" cy="17" r="3.5" fill="#ffffff" opacity="0.35" />
</svg>

<style>
  .leg {
    stroke: #b9c0c5;
    stroke-width: 2;
  }
  .flat {
    fill: none;
    stroke: #000000;
    stroke-opacity: 0.25;
    stroke-width: 1.5;
  }
  .glow,
  .dome {
    transition: none;
  }
  .lit .glow,
  .lit .dome {
    transition: opacity var(--pb-glow-motion, 80ms);
  }
</style>

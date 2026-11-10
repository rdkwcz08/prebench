<!--
  components/button/view.svelte — Rendu du bouton poussoir (Svelte 5).
  Mode simulation : appui tant que le pointeur est enfoncé sur le bouton.
  Coordonnées identiques au manifeste : 48 × 56, broches en x = 12 et x = 36.
-->
<script lang="ts">
  import type { ComponentViewProps } from '../component-api';
  import type { ButtonColor, ButtonParams, ButtonState } from './model';

  let { state, params, mode, control }: ComponentViewProps<ButtonParams, ButtonState> = $props();

  const CAPS: Record<ButtonColor, string> = {
    black: '#2b2b2b',
    blue: '#1e5bd8',
    red: '#c62828',
    green: '#2e7d32',
    yellow: '#f9a825',
    white: '#eeeeee',
  };

  const cap = $derived(CAPS[params.color] ?? CAPS.black);
  let holding = false;

  function press(event: PointerEvent): void {
    if (mode !== 'simulate') return;
    holding = true;
    (event.currentTarget as Element).setPointerCapture(event.pointerId);
    control('press', true);
  }

  function release(): void {
    if (!holding) return;
    holding = false;
    control('press', false);
  }
</script>

<svg width="48" height="56" viewBox="0 0 48 56" role="img" aria-label="Bouton poussoir">
  <line x1="12" y1="40" x2="12" y2="56" stroke="#9a9a9a" stroke-width="2" />
  <line x1="36" y1="40" x2="36" y2="56" stroke="#9a9a9a" stroke-width="2" />
  <rect x="2" y="6" width="44" height="36" rx="4" fill="#3a3a3a" />
  <g
    role="button"
    tabindex="-1"
    aria-pressed={state.pressed}
    style:cursor={mode === 'simulate' ? 'pointer' : 'default'}
    onpointerdown={press}
    onpointerup={release}
    onpointercancel={release}
    onlostpointercapture={release}
  >
    <circle cx="24" cy="24" r={state.pressed ? 11 : 12} fill={cap} stroke="#111" stroke-width="1.5" />
    {#if !state.pressed}
      <circle cx="21" cy="20" r="4" fill="#ffffff" opacity="0.25" />
    {/if}
  </g>
</svg>

<!--
  components/led-rgb/view.svelte — LED RGB 5 mm diffusante vue de dessus (Svelte 5), dans le
  style de la LED simple. La teinte est le mélange des trois canaux, son
  opacité la luminosité du canal le plus fort ; halo de 80 ms à l'allumage,
  extinction sans transition (charte, section 6). La patte commune est la plus longue.
  Coordonnées identiques au manifeste : 48 × 80, pattes R, COM, G, B en x = 12, 20, 28, 36.
-->
<script lang="ts">
  import type { ComponentViewProps } from '../component-api';
  import type { LedRgbParams, LedRgbState } from './model';

  let { state, params }: ComponentViewProps<LedRgbParams, LedRgbState> = $props();

  const uid = $props.id();
  const clamp = (v: number): number => Math.min(1, Math.max(0, v));
  const r = $derived(clamp(state.r));
  const g = $derived(clamp(state.g));
  const b = $derived(clamp(state.b));
  const level = $derived(Math.max(r, g, b));
  const lit = $derived(level > 0);
  /** Teinte à pleine intensité ; l'opacité porte la luminosité. */
  const hue = $derived(
    lit ? `rgb(${Math.round((255 * r) / level)}, ${Math.round((255 * g) / level)}, ${Math.round((255 * b) / level)})` : '#ffffff',
  );
  const pct = (v: number): string => `${Math.round(v * 100)} %`;
</script>

<svg
  width="48"
  height="80"
  viewBox="0 0 48 80"
  class:lit
  role="img"
  aria-label="LED RGB, {params.common === 'anode' ? 'anode' : 'cathode'} commune, {lit
    ? `rouge ${pct(r)}, vert ${pct(g)}, bleu ${pct(b)}`
    : 'éteinte'}"
>
  <defs>
    <radialGradient id="glow-{uid}">
      <stop offset="0" stop-color={hue} stop-opacity="0.9" />
      <stop offset="1" stop-color={hue} stop-opacity="0" />
    </radialGradient>
  </defs>
  <path d="M12 34V80M28 34V80M36 34V80" class="leg" />
  <path d="M20 34V80" class="leg common" />
  <circle cx="24" cy="22" r="20" fill="url(#glow-{uid})" class="glow" opacity={level} />
  <circle cx="24" cy="22" r="13" class="diffuser" />
  <circle cx="24" cy="22" r="13" fill={hue} class="dome" opacity={level} />
  <path d="M34.5 14.5a13 13 0 0 1 0 15" class="flat" />
  <circle cx="19" cy="17" r="3.5" fill="#ffffff" opacity="0.35" />
</svg>

<style>
  .leg {
    stroke: #b9c0c5;
    stroke-width: 2;
  }
  .common {
    stroke-width: 2.5;
  }
  .diffuser {
    fill: #d6dadd;
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

<!--
  components/ili9341/view.svelte — Module TFT ILI9341 vu de dessus (Svelte 5) : circuit au
  vernis et à la sérigraphie de la carte (charte 2.2), dalle 240 × 320 à l'échelle 1.
  Les pixels sont dessinés par l'hôte dans le canvas attaché (display.attach),
  zone modifiée seulement, sans repasser par Svelte (section 11.3) ; le canvas
  garde ses pixels nets à tous les zooms (image-rendering: pixelated).
  Écran noir (non alimenté, en reset, en veille, éteint, sans rétroéclairage) :
  verre noir seul, la raison dans le nom accessible.
  Coordonnées identiques au manifeste : 264 × 384, neuf broches en haut, de
  x = 36 à 228 par pas de 24.
-->
<script lang="ts">
  import type { ComponentViewProps } from '../component-api';
  import type { Ili9341Params, Ili9341State } from './model';

  let { state: model, display }: ComponentViewProps<Ili9341Params, Ili9341State> = $props();

  const PINS = ['VCC', 'GND', 'CS', 'RST', 'DC', 'SDI', 'SCK', 'LED', 'SDO'];
  const label = $derived(`Écran TFT 240 × 320, ${model.lit ? 'allumé' : (model.dark ?? 'éteint')}`);
</script>

<div class="tft" role="img" aria-label={label}>
  <svg width="264" height="384" viewBox="0 0 264 384" aria-hidden="true">
    <rect x="0" y="0" width="264" height="384" rx="3" class="pcb" />
    {#each PINS as name, i (name)}
      <circle cx={36 + i * 24} cy="6" r="3.5" class="pad" />
      <text x={36 + i * 24} y="22" class="silk">{name}</text>
    {/each}
    <rect x="8" y="34" width="248" height="332" rx="1" class="glass" class:lit={model.lit} />
  </svg>
  {#if display && model.lit}
    {#key display}
      <canvas use:display.attach={{ invert: model.inverted }}></canvas>
    {/key}
  {/if}
</div>

<style>
  .tft {
    position: relative;
    width: 264px;
    height: 384px;
  }
  svg {
    position: absolute;
    inset: 0;
  }
  .pcb {
    fill: var(--pb-board);
  }
  .pad {
    fill: var(--pb-copper);
  }
  .silk {
    fill: var(--pb-silk);
    font: 400 11px var(--pb-font-data, ui-monospace, monospace);
    text-anchor: middle;
  }
  .glass {
    fill: #06090b;
  }
  canvas {
    position: absolute;
    left: 12px;
    top: 40px;
    width: 240px;
    height: 320px;
    image-rendering: pixelated;
  }
</style>

<!--
  components/ssd1306/view.svelte — Module OLED SSD1306 vu de dessus (Svelte 5) : circuit au
  vernis et à la sérigraphie de la carte (charte 2.2), verre noir, pixels à
  l'échelle 1 (128 × 64 ou 128 × 32 unités du canevas).
  Les pixels sont dessinés par l'hôte dans le canvas attaché (display.attach),
  zone modifiée seulement, sans repasser par Svelte (section 11.3). La vue ne
  se redessine que si l'état change : allumage, contraste, inversion.
  Jaune et bleu : les 16 premières lignes en jaune, comme les modules bicolores.
  Coordonnées identiques au manifeste : 152 × 112, broches GND, VCC, SCL, SDA
  en haut, en x = 40, 64, 88, 112.
-->
<script lang="ts">
  import type { ComponentViewProps } from '../component-api';
  import type { Ssd1306Params, Ssd1306State } from './model';

  let { state: model, params, display }: ComponentViewProps<Ssd1306Params, Ssd1306State> = $props();

  /** Matériaux : couleur d'un pixel allumé, verre éteint. */
  const LIT: Record<Ssd1306Params['color'], string> = {
    white: '#eaf4ff',
    blue: '#5cb8ff',
    'yellow-blue': '#5cb8ff',
  };
  const YELLOW = '#ffd54a';
  const GLASS = '#06090b';

  const height = $derived(model.height === 32 ? 32 : 64);
  const top = $derived(height === 32 ? 46 : 30);
  const on = $derived(LIT[params.color] ?? LIT.white);
  // Contraste : luminosité de 35 % (0x00) à 100 % (0xFF), sans filtre (charte 6).
  const brightness = $derived(0.35 + (0.65 * Math.min(255, Math.max(0, model.contrast))) / 255);
  const label = $derived(
    `Écran OLED ${height === 32 ? '128 × 32' : '128 × 64'}, ${model.lit ? 'allumé' : 'éteint'}`,
  );
</script>

<div class="oled" role="img" aria-label={label}>
  <svg width="152" height="112" viewBox="0 0 152 112" aria-hidden="true">
    <rect x="0" y="0" width="152" height="112" rx="3" class="pcb" />
    {#each [40, 64, 88, 112] as x (x)}
      <circle cx={x} cy="6" r="3.5" class="pad" />
    {/each}
    <text x="40" y="20" class="silk">GND</text>
    <text x="64" y="20" class="silk">VCC</text>
    <text x="88" y="20" class="silk">SCL</text>
    <text x="112" y="20" class="silk">SDA</text>
    <rect x="8" y={top - 4} width="136" height={height + 8} rx="1" class="glass" class:lit={model.lit} />
  </svg>
  {#if display && model.lit}
    <div class="screen" style:top="{top}px" style:height="{height}px" style:opacity={brightness}>
      {#if model.allOn}
        <div class="all" style:background={on}></div>
      {:else}
        {#key display}
          <canvas use:display.attach={{ on, off: GLASS, invert: model.inverted }}></canvas>
          {#if params.color === 'yellow-blue'}
            <canvas class="band" use:display.attach={{ on: YELLOW, off: GLASS, invert: model.inverted }}></canvas>
          {/if}
        {/key}
      {/if}
    </div>
  {/if}
</div>

<style>
  .oled {
    position: relative;
    width: 152px;
    height: 112px;
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
  .screen {
    position: absolute;
    left: 12px;
    width: 128px;
  }
  canvas,
  .all {
    position: absolute;
    inset: 0;
    width: 128px;
    height: 100%;
    image-rendering: pixelated;
  }
  .band {
    clip-path: inset(0 0 calc(100% - 16px) 0);
  }
</style>

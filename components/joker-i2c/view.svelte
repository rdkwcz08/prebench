<!--
  components/joker-i2c/view.svelte — Périphérique I2C générique vu de dessus (Svelte 5) :
  petit module (vernis et sérigraphie de la carte, charte 2.2) qui porte son
  adresse et un curseur par registre réglable (quatre au plus).
  Mode simulation : glisser le long d'un curseur, ou clavier (flèches : 1,
  Maj + flèches et Page : 16, Début et Fin : 0x00 et 0xFF).
  Coordonnées identiques au manifeste : 120 × 120, broches VCC, GND, SCL, SDA
  en x = 24, 48, 72, 96.
-->
<script lang="ts">
  import type { ComponentViewProps } from '../component-api';
  import type { JokerI2cParams, JokerI2cState } from './model';

  let { state: model, mode, control }: ComponentViewProps<JokerI2cParams, JokerI2cState> = $props();

  const TRACK_X = 32;
  const TRACK_W = 52;
  const ROW_Y = 36;
  const ROW_H = 16;

  const active = $derived(mode === 'simulate');
  const hex = (n: number): string => n.toString(16).toUpperCase().padStart(2, '0');

  let drag: { reg: number; left: number; scale: number } | null = null;

  function set(reg: number, value: number): void {
    control(`reg:${hex(reg)}`, Math.min(255, Math.max(0, Math.round(value))));
  }

  function pointerdown(event: PointerEvent, reg: number): void {
    if (!active) return;
    const target = event.currentTarget as SVGGraphicsElement;
    target.setPointerCapture(event.pointerId);
    const box = target.getBoundingClientRect();
    // Le groupe couvre la piste : position écran -> valeur, quel que soit le zoom.
    drag = { reg, left: box.left, scale: 255 / Math.max(1, box.width) };
    set(reg, (event.clientX - drag.left) * drag.scale);
  }

  function pointermove(event: PointerEvent): void {
    if (drag !== null) set(drag.reg, (event.clientX - drag.left) * drag.scale);
  }

  function pointerup(): void {
    drag = null;
  }

  function keydown(event: KeyboardEvent, reg: number, value: number): void {
    if (!active) return;
    const step = event.shiftKey ? 16 : 1;
    const next: Record<string, number> = {
      ArrowUp: value + step,
      ArrowRight: value + step,
      ArrowDown: value - step,
      ArrowLeft: value - step,
      PageUp: value + 16,
      PageDown: value - 16,
      Home: 0,
      End: 255,
    };
    const v = next[event.key];
    if (v === undefined) return;
    event.preventDefault();
    event.stopPropagation(); // les flèches ne déplacent pas la paillasse
    set(reg, v);
  }
</script>

<svg width="120" height="120" viewBox="0 0 120 120" role="img" aria-label="Périphérique I2C à l'adresse 0x{hex(model.address)}">
  <path d="M24 104V120M48 104V120M72 104V120M96 104V120" class="leg" />
  <rect x="0" y="0" width="120" height="104" rx="3" class="body" />
  <text x="8" y="17" class="silk">I2C 0x{hex(model.address)}</text>
  <rect x="96" y="6" width="16" height="16" rx="1" class="chip" />

  {#if model.sliders.length === 0}
    <text x="60" y="58" class="silk middle">Valeurs fixes</text>
  {/if}
  {#each model.sliders as { reg, value }, i (reg)}
    {@const y = ROW_Y + i * ROW_H}
    {@const x = TRACK_X + (value / 255) * TRACK_W}
    <text x="8" y={y + 4} class="silk">{hex(reg)}</text>
    <g
      role="slider"
      tabindex={active ? 0 : -1}
      aria-label="Registre 0x{hex(reg)}"
      aria-valuemin="0"
      aria-valuemax="255"
      aria-valuenow={value}
      aria-valuetext="0x{hex(value)}"
      class:active
      onpointerdown={(e) => pointerdown(e, reg)}
      onpointermove={pointermove}
      onpointerup={pointerup}
      onpointercancel={pointerup}
      onlostpointercapture={pointerup}
      onkeydown={(e) => keydown(e, reg, value)}
    >
      <rect x={TRACK_X} y={y - 6} width={TRACK_W} height="12" class="hit" />
      <path d="M{TRACK_X} {y}H{TRACK_X + TRACK_W}" class="track" />
      <circle cx={x} cy={y} r="5" class="thumb" />
    </g>
    <text x="112" y={y + 4} class="silk end">{hex(value)}</text>
  {/each}

  <text x="24" y="99" class="silk pin">VCC</text>
  <text x="48" y="99" class="silk pin">GND</text>
  <text x="72" y="99" class="silk pin">SCL</text>
  <text x="96" y="99" class="silk pin">SDA</text>
</svg>

<style>
  .leg {
    stroke: #b9c0c5;
    stroke-width: 2;
  }
  .body {
    fill: var(--pb-board);
  }
  .chip {
    fill: #2a2a2a;
  }
  .silk {
    fill: var(--pb-silk);
    font: 400 11px var(--pb-font-data, ui-monospace, monospace);
    font-variant-numeric: tabular-nums;
  }
  .middle,
  .pin {
    text-anchor: middle;
  }
  .end {
    text-anchor: end;
  }
  .hit {
    fill: transparent;
  }
  .track {
    stroke: var(--pb-silk);
    stroke-width: 2;
    stroke-linecap: round;
    opacity: 0.5;
  }
  .thumb {
    fill: var(--pb-silk);
  }
  g.active {
    cursor: ew-resize;
  }
  g:focus {
    outline: none;
  }
  g:focus-visible .thumb {
    stroke: var(--pb-accent);
    stroke-width: 2;
  }
</style>

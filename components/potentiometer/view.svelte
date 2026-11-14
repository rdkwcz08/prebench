<!--
  components/potentiometer/view.svelte — Potentiomètre rotatif vu de dessus (Svelte 5), style
  de la maquette (docs/design/maquette-ecran-principal.html, POT1).
  Mode simulation : glisser sur le bouton (vers le haut ou la droite : plus de
  tension), ou clavier (flèches : 1 %, Maj + flèches et Page : 10 %, Début et Fin).
  Le trait du bouton indique la position, de −135° (GND) à +135° (VCC).
  Coordonnées identiques au manifeste : 56 × 72, broches GND, W, VCC en x = 12, 28, 44.
-->
<script lang="ts">
  import type { ComponentViewProps } from '../component-api';
  import type { PotentiometerParams, PotentiometerState } from './model';

  let { state, params, mode, control }: ComponentViewProps<PotentiometerParams, PotentiometerState> = $props();

  /** Glisser 160 px parcourt toute la course. */
  const TRAVEL = 160;

  const active = $derived(mode === 'simulate');
  const position = $derived(Math.min(1, Math.max(0, state.position)));
  const angle = $derived(-135 + position * 270);
  const percent = $derived(Math.round(position * 100));
  const volts = $derived(
    state.volts === null ? 'sans tension' : `${state.volts.toLocaleString('fr-FR', { maximumFractionDigits: 2 })} V`,
  );
  const kohm = $derived(`${(params.resistance_kohm ?? 10).toLocaleString('fr-FR')} kΩ`);

  let drag: { x: number; y: number; from: number } | null = null;

  function set(value: number): void {
    control('position', Math.min(1, Math.max(0, Math.round(value * 100) / 100)));
  }

  function pointerdown(event: PointerEvent): void {
    if (!active) return;
    (event.currentTarget as Element).setPointerCapture(event.pointerId);
    drag = { x: event.clientX, y: event.clientY, from: position };
  }

  function pointermove(event: PointerEvent): void {
    if (drag === null) return;
    const delta = (event.clientX - drag.x - (event.clientY - drag.y)) / TRAVEL;
    set(drag.from + delta);
  }

  function pointerup(): void {
    drag = null;
  }

  function keydown(event: KeyboardEvent): void {
    if (!active) return;
    const step = event.shiftKey ? 0.1 : 0.01;
    const next: Record<string, number> = {
      ArrowUp: position + step,
      ArrowRight: position + step,
      ArrowDown: position - step,
      ArrowLeft: position - step,
      PageUp: position + 0.1,
      PageDown: position - 0.1,
      Home: 0,
      End: 1,
    };
    const value = next[event.key];
    if (value === undefined) return;
    event.preventDefault();
    event.stopPropagation(); // les flèches ne déplacent pas la paillasse
    set(value);
  }
</script>

<svg width="56" height="72" viewBox="0 0 56 72" role="img" aria-label="Potentiomètre {kohm}, {percent} %, {volts}">
  <path d="M12 48V72M28 48V72M44 48V72" class="leg" />
  <rect x="0" y="4" width="56" height="44" rx="3" class="body" />
  <text x="28" y="44" class="silk">{kohm}</text>
  <g
    role="slider"
    tabindex={active ? 0 : -1}
    aria-label="Position du potentiomètre"
    aria-valuemin="0"
    aria-valuemax="100"
    aria-valuenow={percent}
    aria-valuetext="{percent} %, {volts}"
    class:active
    onpointerdown={pointerdown}
    onpointermove={pointermove}
    onpointerup={pointerup}
    onpointercancel={pointerup}
    onlostpointercapture={pointerup}
    onkeydown={keydown}
  >
    <circle cx="28" cy="20" r="13" class="knob" />
    <path d="M28 20V9" class="mark" transform="rotate({angle} 28 20)" />
  </g>
</svg>

<style>
  .leg {
    stroke: #b9c0c5;
    stroke-width: 2;
  }
  .body {
    fill: #2f6fb0;
  }
  .knob {
    fill: #e6e9ec;
  }
  .mark {
    stroke: #2a2a2a;
    stroke-width: 2.5;
    stroke-linecap: round;
  }
  .silk {
    fill: #e8ecef;
    font: 400 11px var(--pb-font-data, ui-monospace, monospace);
    text-anchor: middle;
  }
  g.active {
    cursor: grab;
  }
  g:focus {
    outline: none;
  }
  g:focus-visible .knob {
    stroke: var(--pb-accent);
    stroke-width: 2;
  }
</style>

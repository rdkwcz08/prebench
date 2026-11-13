<!--
  components/button/view.svelte — Bouton poussoir vu de dessus (Svelte 5), style de
  la maquette (docs/design/maquette-ecran-principal.html, B1).
  Mode simulation : appui tant que le pointeur, ou la touche Espace ou Entrée,
  est enfoncé. Appuyé : capuchon plus petit et anneau, pas seulement une couleur.
  Coordonnées identiques au manifeste : 48 × 56, broches en x = 12 et x = 36.
-->
<script lang="ts">
  import type { ComponentViewProps } from '../component-api';
  import type { ButtonColor, ButtonParams, ButtonState } from './model';

  let { state, params, mode, control }: ComponentViewProps<ButtonParams, ButtonState> = $props();

  /** Matériaux : couleur du capuchon. */
  const CAPS: Record<ButtonColor, string> = {
    black: '#2b2b2b',
    blue: '#2e5fa8',
    red: '#c62828',
    green: '#2e7d32',
    yellow: '#e0a01e',
    white: '#eeeeee',
  };

  const cap = $derived(CAPS[params.color] ?? CAPS.black);
  const active = $derived(mode === 'simulate');
  let holding = false;

  function down(): void {
    if (!active || holding) return;
    holding = true;
    control('press', true);
  }

  function up(): void {
    if (!holding) return;
    holding = false;
    control('press', false);
  }

  function pointerdown(event: PointerEvent): void {
    if (!active) return;
    (event.currentTarget as Element).setPointerCapture(event.pointerId);
    down();
  }

  function keydown(event: KeyboardEvent): void {
    if (event.key !== ' ' && event.key !== 'Enter') return;
    event.preventDefault();
    event.stopPropagation(); // Espace ne déclenche pas un raccourci de l'app
    down();
  }

  function keyup(event: KeyboardEvent): void {
    if (event.key !== ' ' && event.key !== 'Enter') return;
    event.preventDefault();
    up();
  }
</script>

<svg width="48" height="56" viewBox="0 0 48 56" role="img" aria-label="Bouton poussoir">
  <path d="M12 40V56M36 40V56" class="leg" />
  <rect x="7" y="6" width="34" height="34" rx="3" class="body" />
  <g
    role="button"
    tabindex={active ? 0 : -1}
    aria-label="Appuyer"
    aria-pressed={state.pressed}
    class:active
    onpointerdown={pointerdown}
    onpointerup={up}
    onpointercancel={up}
    onlostpointercapture={up}
    onkeydown={keydown}
    onkeyup={keyup}
    onblur={up}
  >
    {#if state.pressed}
      <circle cx="24" cy="23" r="12.5" class="ring" />
    {/if}
    <circle cx="24" cy="23" r={state.pressed ? 9.5 : 11} fill={cap} />
    {#if !state.pressed}
      <circle cx="20.5" cy="19.5" r="3.5" fill="#ffffff" opacity="0.25" />
    {/if}
  </g>
</svg>

<style>
  .leg {
    stroke: #b9c0c5;
    stroke-width: 2;
  }
  .body {
    fill: #c7ccd0;
  }
  .ring {
    fill: none;
    stroke: #1a1c1e;
    stroke-opacity: 0.45;
    stroke-width: 2;
  }
  g.active {
    cursor: pointer;
  }
  g:focus {
    outline: none;
  }
  g:focus-visible circle:last-of-type {
    stroke: var(--pb-accent);
    stroke-width: 2;
  }
</style>

<!--
  components/ws2812/view.svelte — Ruban de LED WS2812 vu de dessus (Svelte 5) : ruban
  souple noir (matériau), LED 5050 en boîtier blanc, 16 par rangée, 3 rangées au plus.
  Une seule modification par LED et par image : la couleur (fill) de sa puce.
  Luminosité perçue : chaque composante est éclaircie (gamma 2,2), comme l'œil voit
  une LED à faible valeur ; une LED à 0 reste éteinte. Le nombre de LED allumées
  n'entre dans le nom accessible qu'au survol ou au focus (charte, section 6).
  Coordonnées identiques au manifeste : 408 × 96, broches 5V, DIN, GND en bas,
  en x = 12, 36, 60.
-->
<script lang="ts">
  import type { ComponentViewProps } from '../component-api';
  import type { Ws2812Params, Ws2812State } from './model';

  let { state: model, params }: ComponentViewProps<Ws2812Params, Ws2812State> = $props();

  const PER_ROW = 16;
  const count = $derived(Math.min(48, Math.max(1, Math.floor(Number(params.count) || 1))));
  const rows = $derived(Math.ceil(count / PER_ROW));

  /** Composante perçue (gamma 2,2), 0 reste 0. */
  const perceived = (v: number): number => (v === 0 ? 0 : Math.round(255 * Math.pow(v / 255, 1 / 2.2)));
  /** Couleur affichée d'une LED, ou null si elle est éteinte. */
  function shown(c: number): string | null {
    if (!c) return null;
    return `rgb(${perceived((c >> 16) & 0xff)}, ${perceived((c >> 8) & 0xff)}, ${perceived(c & 0xff)})`;
  }

  let described = $state(false);
  const lit = $derived(described ? (model.colors ?? []).slice(0, count).filter((c) => c !== 0).length : 0);
  const label = $derived(
    `Ruban de ${count} LED WS2812${described ? `, ${lit} allumée${lit > 1 ? 's' : ''}` : ''}`,
  );
</script>

<svg
  width="408"
  height="96"
  viewBox="0 0 408 96"
  role="img"
  aria-label={label}
  onpointerenter={() => (described = true)}
  onpointerleave={() => (described = false)}
  onfocusin={() => (described = true)}
  onfocusout={() => (described = false)}
>
  {#each { length: rows } as _, r (r)}
    <rect x="0" y={r * 24} width={Math.min(PER_ROW, count - r * PER_ROW) * 24 + 12} height="24" rx="2" class="flex" />
  {/each}
  <path d="M12 84V96M36 84V96M60 84V96" class="leg" />
  <text x="12" y="80" class="silk">5V</text>
  <text x="36" y="80" class="silk">DIN</text>
  <text x="60" y="80" class="silk">GND</text>
  {#each { length: count } as _, i (i)}
    {@const x = 6 + (i % PER_ROW) * 24}
    {@const y = Math.floor(i / PER_ROW) * 24 + 4}
    {@const color = shown(model.colors?.[i] ?? 0)}
    <rect {x} {y} width="16" height="16" rx="2" class="package" />
    <circle cx={x + 8} cy={y + 8} r="5" class="chip" style:fill={color} />
  {/each}
</svg>

<style>
  .flex {
    fill: #202124;
  }
  .leg {
    stroke: #b9c0c5;
    stroke-width: 2;
  }
  .silk {
    fill: var(--pb-mat-ink);
    font: 400 11px var(--pb-font-data, ui-monospace, monospace);
    text-anchor: middle;
  }
  .package {
    fill: #f2f2f0;
  }
  .chip {
    fill: #3a3a38;
  }
</style>

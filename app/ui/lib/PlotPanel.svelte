<!--
  app/ui/lib/PlotPanel.svelte — Traceur (traceur série Arduino) : courbes des lignes
  « clé:valeur » du port affiché. Couleurs de voies d'oscilloscope (charte 2.5) ;
  au-delà de quatre courbes, les couleurs reviennent en pointillés (forme).
  Échelle verticale automatique, valeurs extrêmes et dernière valeur en chasse fixe.
-->
<script lang="ts">
  import { MAX_POINTS, serial } from './serial.svelte';

  /** Voies de mesure (charte 2.5). */
  const CHANNELS = ['#E6B422', '#3FA7D6', '#D45AA8', '#5B7FE0'];
  const W = MAX_POINTS;
  const H = 100;

  const range = $derived.by(() => {
    const all = serial.series.flatMap((s) => s.points);
    if (all.length === 0) return { min: 0, max: 1 };
    let min = Math.min(...all);
    let max = Math.max(...all);
    if (min === max) {
      min -= 1;
      max += 1;
    }
    const pad = (max - min) * 0.08;
    return { min: min - pad, max: max + pad };
  });

  function points(values: number[]): string {
    const { min, max } = range;
    const offset = W - values.length;
    return values.map((v, i) => `${offset + i},${H - ((v - min) / (max - min)) * H}`).join(' ');
  }

  const fmt = (v: number) => v.toLocaleString('fr-FR', { maximumFractionDigits: 3 });
</script>

<div class="plot">
  {#if serial.series.length === 0}
    <p class="empty">
      Aucune courbe. Le traceur lit les lignes du port choisi au format <span class="data">temp:21.4,hum:55</span> (ou des nombres
      séparés par des espaces) et trace une courbe par nom.
    </p>
  {:else}
    <ul class="legend">
      {#each serial.series as s, i (s.name)}
        <li>
          <svg width="16" height="8" aria-hidden="true"
            ><line x1="0" y1="4" x2="16" y2="4" stroke={CHANNELS[i % 4]} stroke-width="2" stroke-dasharray={i >= 4 ? '3 2' : undefined} /></svg
          >
          <span>{s.name}</span>
          <span class="data value">{fmt(s.points.at(-1) ?? 0)}</span>
        </li>
      {/each}
    </ul>
    <div class="chart">
      <span class="axis top data">{fmt(range.max)}</span>
      <span class="axis bottom data">{fmt(range.min)}</span>
      <svg viewBox="0 0 {W} {H}" preserveAspectRatio="none" role="img" aria-label="Courbes du traceur">
        <line x1="0" y1={H / 2} x2={W} y2={H / 2} class="grid" />
        {#each serial.series as s, i (s.name)}
          <polyline
            points={points(s.points)}
            fill="none"
            stroke={CHANNELS[i % 4]}
            stroke-width="1.5"
            stroke-dasharray={i >= 4 ? '4 3' : undefined}
            vector-effect="non-scaling-stroke"
          />
        {/each}
      </svg>
    </div>
  {/if}
</div>

<style>
  .plot {
    display: grid;
    grid-template-columns: 180px minmax(0, 1fr);
    gap: 12px;
    height: 100%;
    font-family: var(--pb-font-ui);
    font-size: 12px;
  }
  .empty {
    grid-column: 1 / -1;
    margin: 0;
    color: var(--pb-text-quiet);
  }
  .legend {
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .legend li {
    display: flex;
    align-items: center;
    gap: 8px;
    height: 24px;
  }
  .value {
    margin-left: auto;
    font-variant-numeric: tabular-nums;
  }
  .chart {
    position: relative;
    min-height: 0;
    border-left: 1px solid var(--pb-line);
    border-bottom: 1px solid var(--pb-line);
  }
  .chart svg {
    width: 100%;
    height: 100%;
    display: block;
  }
  .grid {
    stroke: var(--pb-line);
    stroke-width: 1;
    stroke-dasharray: 4 4;
    vector-effect: non-scaling-stroke;
  }
  .axis {
    position: absolute;
    left: 4px;
    color: var(--pb-text-quiet);
    font-size: 11px;
  }
  .top {
    top: 0;
  }
  .bottom {
    bottom: 0;
  }
</style>

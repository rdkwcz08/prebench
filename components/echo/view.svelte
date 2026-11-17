<!--
  components/echo/view.svelte — Composant d'écho (latence et SPI en boucle), dessiné comme
  un petit module sur la paillasse : vernis et sérigraphie de la charte (section 2.2),
  compteurs en chasse fixe, broches SPI en bas avec leur nom (comme le MPU6050).
-->
<script lang="ts">
  import type { ComponentViewProps } from '../component-api';
  import type { EchoParams, EchoState } from './model';

  let { state }: ComponentViewProps<EchoParams, EchoState> = $props();

  const fr = (n: number, digits = 0): string =>
    n.toLocaleString('fr-FR', { maximumFractionDigits: digits });
  const pings = $derived(fr(state.pings));
  /** Octets SPI, en o, ko ou Mo pour tenir sur la largeur du module. */
  const bytes = $derived.by(() => {
    const n = state.spiBytes ?? 0;
    if (n < 1000) return `${fr(n)} o`;
    if (n < 1_000_000) return `${fr(n / 1000, 1)} ko`;
    return `${fr(n / 1_000_000, 1)} Mo`;
  });
</script>

<svg
  width="96"
  height="72"
  viewBox="0 0 96 72"
  role="img"
  aria-label="Écho : {pings} pings reçus, {bytes} reçus en SPI"
>
  <rect x="0" y="0" width="96" height="60" rx="3" class="pcb" />
  <path d="M12 60V72M36 60V72M60 60V72M84 60V72" class="leg" />
  <text x="48" y="14" class="silk middle label">ÉCHO</text>
  <text x="48" y="28" class="silk middle">{pings} pings</text>
  <text x="48" y="41" class="silk middle">SPI {bytes}</text>
  <text x="12" y="56" class="silk middle">SCK</text>
  <text x="36" y="56" class="silk middle">SDI</text>
  <text x="60" y="56" class="silk middle">SDO</text>
  <text x="84" y="56" class="silk middle">CS</text>
</svg>

<style>
  .pcb {
    fill: var(--pb-board);
  }
  .leg {
    stroke: #b9c0c5;
    stroke-width: 2;
  }
  .silk {
    fill: var(--pb-silk);
    font: 400 11px var(--pb-font-data, ui-monospace, monospace);
    font-variant-numeric: tabular-nums;
  }
  .label {
    font-weight: 600;
  }
  .middle {
    text-anchor: middle;
  }
</style>

<!--
  app/ui/lib/Board.svelte — Carte dessinée à plat sur le tapis (charte, sections 1 et 2.2),
  à partir de la table [layout] du fichier de carte : vernis, trous de fixation,
  boîtier du microcontrôleur, connecteurs (pastilles en cuivre, sérigraphie).
  Aucune carte n'est codée ici (règle 16.2.7) ; sans [layout], dessin générique.
  Matériaux identiques dans les deux thèmes ; posée en BOARD_ORIGIN (monde).
-->
<script lang="ts">
  import { BOARD_ORIGIN, GENERIC_BOARD, type Pad } from './geometry';
  import type { Layout } from './types';

  let { name, mcu, layout, pads }: { name: string; mcu: string; layout: Layout | null; pads: Pad[] } = $props();

  const size = $derived(layout?.size ?? GENERIC_BOARD);
  const W = $derived(size[0]);
  const H = $derived(size[1]);
  const holes = $derived(
    layout?.holes ?? [
      [16, 16],
      [W - 16, 16],
      [16, H - 16],
      [W - 30, H - 16],
    ],
  );
  const chip = $derived(layout?.chip ?? { pos: [W / 2 - 40, H / 2 + 30] as [number, number], size: [80, 80] as [number, number] });
  /** Pastilles en coordonnées de la carte. */
  const local = $derived(pads.map((p) => ({ ...p, x: p.pos[0] - BOARD_ORIGIN[0], y: p.pos[1] - BOARD_ORIGIN[1] })));
  /** Corps noir des connecteurs : rectangle englobant de leurs pastilles. */
  const bodies = $derived(
    (layout?.headers ?? []).map((h) => {
      const own = local.filter((p) => p.header === h.id);
      const xs = own.map((p) => p.x);
      const ys = own.map((p) => p.y);
      return { id: h.id, x: Math.min(...xs) - 6, y: Math.min(...ys) - 6, w: Math.max(...xs) - Math.min(...xs) + 12, h: Math.max(...ys) - Math.min(...ys) + 12 };
    }),
  );

  function labelPos(p: (typeof local)[number]): { x: number; y: number; anchor: string } {
    switch (p.side) {
      case 'left':
        return { x: p.x - 10, y: p.y + 3, anchor: 'end' };
      case 'above':
        return { x: p.x, y: p.y - 10, anchor: 'middle' };
      case 'below':
        return { x: p.x, y: p.y + 15, anchor: 'middle' };
      default:
        return { x: p.x + 10, y: p.y + 3, anchor: 'start' };
    }
  }

  function leads(n: number, len: number): number[] {
    const step = len / (n + 1);
    return Array.from({ length: n }, (_, i) => step * (i + 1));
  }
</script>

<svg
  class="board"
  width={W}
  height={H}
  viewBox="0 0 {W} {H}"
  style:left="{BOARD_ORIGIN[0]}px"
  style:top="{BOARD_ORIGIN[1]}px"
  role="img"
  aria-label="Carte {name}"
>
  <rect x="1" y="1" width={W - 2} height={H - 2} rx="10" class="pcb" />
  {#each holes as [x, y], i (i)}
    <circle cx={x} cy={y} r="5" class="hole" />
  {/each}
  <!-- microcontrôleur -->
  <g transform="translate({chip.pos[0]} {chip.pos[1]})">
    <g class="leads">
      {#each leads(7, chip.size[0]) as d (d)}
        <path d="M{d} -4v4M{d} {chip.size[1]}v4" />
      {/each}
      {#each leads(7, chip.size[1]) as d (d)}
        <path d="M-4 {d}h4M{chip.size[0]} {d}h4" />
      {/each}
    </g>
    <rect width={chip.size[0]} height={chip.size[1]} rx="3" class="package" />
    <circle cx="8" cy="8" r="2.5" class="dimple" />
    <text x={chip.size[0] / 2} y={chip.size[1] / 2 + 4} text-anchor="middle" class="silk mcu">{mcu}</text>
  </g>
  <!-- connecteurs : corps, pastilles, sérigraphie -->
  {#each bodies as b (b.id)}
    <rect x={b.x} y={b.y} width={b.w} height={b.h} rx="1" class="header" />
  {/each}
  {#each local as p (p.endpoint)}
    <rect x={p.x - 3} y={p.y - 3} width="6" height="6" class="pad" class:nc={!p.wireable} />
    {@const l = labelPos(p)}
    <text x={l.x} y={l.y} text-anchor={l.anchor} class="silk pin">{p.label}</text>
  {/each}
  <text x={W / 2} y={H - 30} text-anchor="middle" class="silk name">{name.toUpperCase()}</text>
</svg>

<style>
  .board {
    position: absolute;
    overflow: visible;
  }
  /* Matériaux (charte 2.2) : vernis, cuivre, sérigraphie, boîtier et connecteurs noirs */
  .pcb {
    fill: var(--pb-board);
    stroke: #162c45;
    stroke-width: 2;
  }
  .hole {
    fill: none;
    stroke: var(--pb-copper);
    stroke-width: 2;
  }
  .package {
    fill: #1a1c1e;
  }
  .dimple {
    fill: #3a3d40;
  }
  .leads {
    stroke: #9aa4ac;
    stroke-width: 1.5;
  }
  .header {
    fill: #141618;
  }
  .pad {
    fill: var(--pb-copper);
  }
  .pad.nc {
    opacity: 0.5;
  }
  .silk {
    fill: var(--pb-silk);
    font-family: var(--pb-font-data);
    font-weight: 600;
  }
  .pin {
    font-size: 8px;
  }
  .mcu {
    font-size: 9px;
  }
  .name {
    font-family: var(--pb-font-ui);
    font-size: 11px;
  }
</style>

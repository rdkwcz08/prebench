<!--
  app/ui/lib/Board.svelte — Carte dessinée à plat sur le tapis (charte, sections 1 et 2.2).
  Vernis, trous de fixation en cuivre, boîtier du microcontrôleur et sérigraphie ;
  les composants intégrés (LD2, B1) sont posés dessus. Matériaux identiques dans
  les deux thèmes. Les connecteurs et leurs broches arrivent avec le câblage
  (phase 2, étape 4), à partir du fichier de carte.
-->
<script lang="ts">
  import type { Snippet } from 'svelte';

  let { name, mcu, children }: { name: string; mcu: string; children: Snippet } = $props();

  const W = 280;
  const H = 400;
</script>

<section class="board" aria-label="Carte {name}" style:width="{W}px" style:height="{H}px">
  <svg width={W} height={H} viewBox="0 0 {W} {H}" aria-hidden="true">
    <rect x="1" y="1" width={W - 2} height={H - 2} rx="10" class="pcb" />
    <!-- trous de fixation -->
    <circle cx="16" cy="16" r="5" class="hole" />
    <circle cx={W - 16} cy="16" r="5" class="hole" />
    <circle cx="16" cy={H - 16} r="5" class="hole" />
    <circle cx={W - 30} cy={H - 16} r="5" class="hole" />
    <!-- microcontrôleur -->
    <g transform="translate({W / 2 - 40} 230)">
      <g class="leads">
        <path d="M8 -4v4M18 -4v4M28 -4v4M38 -4v4M48 -4v4M58 -4v4M68 -4v4" />
        <path d="M8 80v4M18 80v4M28 80v4M38 80v4M48 80v4M58 80v4M68 80v4" />
        <path d="M-4 8h4M-4 18h4M-4 28h4M-4 38h4M-4 48h4M-4 58h4M-4 68h4" />
        <path d="M80 8h4M80 18h4M80 28h4M80 38h4M80 48h4M80 58h4M80 68h4" />
      </g>
      <rect width="80" height="80" rx="3" class="package" />
      <circle cx="8" cy="8" r="2.5" class="dimple" />
      <text x="40" y="44" text-anchor="middle" class="silk mcu">{mcu}</text>
    </g>
    <text x={W / 2} y={H - 30} text-anchor="middle" class="silk name">{name.toUpperCase()}</text>
  </svg>
  {@render children()}
</section>

<style>
  .board {
    position: absolute;
    left: 48px;
    top: 48px;
  }
  svg {
    position: absolute;
    inset: 0;
  }
  /* Matériaux (charte 2.2) : vernis, cuivre, sérigraphie, boîtier noir */
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
  .silk {
    fill: var(--pb-silk);
    font-family: var(--pb-font-data);
    font-weight: 600;
  }
  .mcu {
    font-size: 9px;
  }
  .name {
    font-family: var(--pb-font-ui);
    font-size: 11px;
  }
</style>

<!--
  app/ui/lib/ui/Icon.svelte — Jeu d'icônes au trait de Prebench (charte, section 5).
  Grille de 16, trait de 1,5 px, extrémités arrondies, couleur du texte courant.
  Décoratives (aria-hidden) : le libellé est porté par le contrôle qui les contient.
-->
<script lang="ts" module>
  /** Tracés, dans un viewBox de 16 × 16. */
  const PATHS = {
    play: '<path d="M5 3.5v9l7-4.5z"/>',
    pause: '<path d="M5.5 3.5v9M10.5 3.5v9"/>',
    stop: '<rect x="4" y="4" width="8" height="8" rx="1"/>',
    reset: '<path d="M3.5 8a4.5 4.5 0 1 0 1.3-3.2"/><path d="M3.5 2.5v2.8h2.8"/>',
    power: '<path d="M8 2.5v5"/><path d="M4.8 4.6a4.8 4.8 0 1 0 6.4 0"/>',
    plus: '<path d="M8 3.5v9M3.5 8h9"/>',
    chevron: '<path d="M5 6.5l3 3 3-3"/>',
    'theme-system': '<circle cx="8" cy="8" r="5"/><path d="M8 3v10a5 5 0 0 0 0-10z" fill="currentColor"/>',
    'theme-light':
      '<circle cx="8" cy="8" r="2.5"/><path d="M8 2v1.5M8 12.5V14M2 8h1.5M12.5 8H14M3.8 3.8l1 1M11.2 11.2l1 1M3.8 12.2l1-1M11.2 4.8l1-1"/>',
    'theme-dark': '<path d="M12.5 9.5A5 5 0 0 1 6.5 3.5a5 5 0 1 0 6 6z"/>',
    clock: '<circle cx="8" cy="8" r="5.5"/><path d="M8 5v3l2 1.5"/>',
    hex: '<path d="M3 5.5h10M3 10.5h10M6 3l-1 10M11 3l-1 10"/>',
    trash: '<path d="M3.5 4.5h9M6.5 4.5V3h3v1.5M5 4.5l.6 8h4.8l.6-8"/>',
    export: '<path d="M8 2.5v7M5 6.5l3 3 3-3M3.5 11v2h9v-2"/>',
    warn: '<path d="M8 2.5l6 10.5H2z"/><path d="M8 6.5v3M8 11.2v.1"/>',
    error: '<circle cx="8" cy="8" r="5.5"/><path d="M6 6l4 4M10 6l-4 4"/>',
    info: '<circle cx="8" cy="8" r="5.5"/><path d="M8 7.5v3.5M8 5.2v.1"/>',
    check: '<path d="M3.5 8.5l3 3 6-7"/>',
    close: '<path d="M4.5 4.5l7 7M11.5 4.5l-7 7"/>',
    chip: '<rect x="4" y="4" width="8" height="8" rx="1"/><path d="M6 2v2M10 2v2M6 12v2M10 12v2M2 6h2M2 10h2M12 6h2M12 10h2"/>',
    bug: '<path d="M5.5 6.5a2.5 2.5 0 0 1 5 0v4a2.5 2.5 0 0 1-5 0z"/><path d="M8 7v5.5M3 8.5h2.5M10.5 8.5H13M3.5 5.5l2 1.5M12.5 5.5l-2 1.5M3.5 12l2-1M12.5 12l-2-1M6.5 4.2 5.5 2.8M9.5 4.2l1-1.4"/>',
    folder: '<path d="M2.5 4.5v8h11v-6.5H8L6.5 4.5z"/>',
    download: '<path d="M8 2.5v7M5 6.5l3 3 3-3M3.5 13h9"/>',
    'panel-right': '<rect x="2.5" y="3" width="11" height="10" rx="1"/><path d="M10 3v10"/>',
    'panel-bottom': '<rect x="2.5" y="3" width="11" height="10" rx="1"/><path d="M2.5 9.5h11"/>',
  } as const;

  export type IconName = keyof typeof PATHS;
</script>

<script lang="ts">
  let { name }: { name: IconName } = $props();
</script>

<svg class="icon" viewBox="0 0 16 16" aria-hidden="true">{@html PATHS[name]}</svg>

<style>
  .icon {
    width: 16px;
    height: 16px;
    flex: none;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.5;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
</style>

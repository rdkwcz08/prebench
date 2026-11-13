<!--
  app/ui/lib/ui/IconButton.svelte — Bouton icône 28 × 28 (charte, section 5).
  Infobulle obligatoire : libellé, puis raccourci clavier s'il existe. Elle
  apparaît au survol ou au focus clavier, sans animation (section 6) ; le
  libellé est aussi le nom accessible du bouton.
-->
<script lang="ts">
  import Icon, { type IconName } from './Icon.svelte';

  let {
    icon,
    label,
    shortcut,
    pressed,
    disabled = false,
    align = 'center',
    onclick,
  }: {
    icon: IconName;
    label: string;
    /** Touche affichée dans l'infobulle (« R », « Espace »). */
    shortcut?: string;
    /** Bouton bascule : état enfoncé (aria-pressed). */
    pressed?: boolean;
    disabled?: boolean;
    /** Alignement de l'infobulle : « end » près du bord droit de la fenêtre. */
    align?: 'start' | 'center' | 'end';
    onclick?: (e: MouseEvent) => void;
  } = $props();
</script>

<span class="wrap">
  <button class="icon-btn" class:on={pressed === true} aria-label={label} aria-pressed={pressed} {disabled} {onclick}>
    <Icon name={icon} />
  </button>
  <span class="tip {align}" role="tooltip">
    {label}{#if shortcut}<kbd>{shortcut}</kbd>{/if}
  </span>
</span>

<style>
  .wrap {
    position: relative;
    display: inline-flex;
    flex: none;
  }
  .icon-btn {
    width: 28px;
    height: 28px;
    padding: 0;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    border: 1px solid transparent;
    border-radius: 3px;
    background: transparent;
    cursor: pointer;
  }
  .icon-btn:hover:not(:disabled) {
    border-color: var(--pb-line);
  }
  .icon-btn.on {
    background: color-mix(in srgb, var(--pb-accent) 18%, transparent);
  }
  .icon-btn:disabled {
    opacity: 0.45;
    cursor: default;
  }
  .tip {
    position: absolute;
    top: calc(100% + 4px);
    z-index: 30;
    display: flex;
    gap: 8px;
    align-items: center;
    padding: 4px 8px;
    border: 1px solid var(--pb-line);
    border-radius: 6px;
    background: var(--pb-panel);
    color: var(--pb-text);
    font-size: 12px;
    font-weight: 400;
    white-space: nowrap;
    box-shadow: var(--pb-shadow);
    pointer-events: none;
    visibility: hidden;
  }
  .tip.center {
    left: 50%;
    translate: -50% 0;
  }
  .tip.start {
    left: 0;
  }
  .tip.end {
    right: 0;
  }
  .wrap:hover .tip,
  .icon-btn:focus-visible + .tip {
    visibility: visible;
    transition: visibility 0s 400ms;
  }
  .icon-btn:focus-visible + .tip {
    transition-delay: 0s;
  }
  kbd {
    padding: 0 4px;
    border: 1px solid var(--pb-line);
    border-radius: 3px;
    color: var(--pb-text-quiet);
    font: 400 11px/16px var(--pb-font-data);
  }
</style>

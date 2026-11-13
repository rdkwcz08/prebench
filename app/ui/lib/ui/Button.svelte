<!--
  app/ui/lib/ui/Button.svelte — Bouton texte (charte, section 5).
  « primary » : fond accent, un seul par zone. « secondary » : transparent, bordure.
  28 px de haut, rayon 3 px, icône facultative à gauche du libellé.
-->
<script lang="ts">
  import type { Snippet } from 'svelte';
  import Icon, { type IconName } from './Icon.svelte';

  let {
    variant = 'secondary',
    icon,
    type = 'button',
    disabled = false,
    title,
    onclick,
    children,
  }: {
    variant?: 'primary' | 'secondary';
    icon?: IconName;
    type?: 'button' | 'submit';
    disabled?: boolean;
    title?: string;
    onclick?: (e: MouseEvent) => void;
    children: Snippet;
  } = $props();
</script>

<button class="btn {variant}" {type} {disabled} {title} {onclick}>
  {#if icon}<Icon name={icon} />{/if}
  {@render children()}
</button>

<style>
  .btn {
    height: 28px;
    padding: 0 10px;
    display: inline-flex;
    align-items: center;
    gap: 6px;
    flex: none;
    border: 1px solid var(--pb-line);
    border-radius: 3px;
    background: transparent;
    white-space: nowrap;
    cursor: pointer;
  }
  .btn:hover:not(:disabled) {
    background: color-mix(in srgb, var(--pb-text) 6%, transparent);
  }
  .primary {
    background: var(--pb-accent);
    border-color: var(--pb-accent);
    color: var(--pb-on-accent);
    font-weight: 600;
  }
  .primary:hover:not(:disabled) {
    background: color-mix(in srgb, var(--pb-accent) 88%, var(--pb-text));
  }
  .btn:disabled {
    opacity: 0.45;
    cursor: default;
  }
</style>

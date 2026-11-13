<!--
  app/ui/lib/ui/Tabs.svelte — Onglets du panneau du bas (charte, section 5).
  Texte secondaire ; onglet actif en texte principal souligné de 2 px d'accent.
  Pastille de comptage facultative (avertissements). Flèches gauche et droite
  pour passer d'un onglet à l'autre (section 9). Outils de l'onglet à droite.
-->
<script lang="ts" module>
  import type { IconName } from './Icon.svelte';

  export interface TabDef {
    id: string;
    label: string;
    icon?: IconName;
    /** Pastille de comptage ; absente ou 0 : non affichée. */
    count?: number;
    /** Couleur de la pastille : avertissement (défaut) ou erreur. */
    tone?: 'warn' | 'conflict';
  }
</script>

<script lang="ts">
  import type { Snippet } from 'svelte';
  import Icon from './Icon.svelte';

  let {
    tabs,
    active = $bindable(),
    label,
    tools,
  }: { tabs: TabDef[]; active: string; label: string; tools?: Snippet } = $props();

  let list: HTMLDivElement | undefined = $state();

  function onkeydown(e: KeyboardEvent): void {
    if (e.key !== 'ArrowRight' && e.key !== 'ArrowLeft') return;
    e.preventDefault();
    const i = tabs.findIndex((t) => t.id === active);
    const next = tabs[(i + (e.key === 'ArrowRight' ? 1 : tabs.length - 1)) % tabs.length];
    if (next === undefined) return;
    active = next.id;
    list?.querySelector<HTMLButtonElement>(`[data-tab="${next.id}"]`)?.focus();
  }
</script>

<div class="tabs">
  <div class="list" role="tablist" aria-label={label} bind:this={list}>
    {#each tabs as t (t.id)}
      <button
        class="tab"
        role="tab"
        data-tab={t.id}
        aria-selected={t.id === active}
        tabindex={t.id === active ? 0 : -1}
        onclick={() => (active = t.id)}
        {onkeydown}
      >
        {#if t.icon}<Icon name={t.icon} />{/if}
        {t.label}
        {#if t.count}<span class="badge {t.tone ?? 'warn'}" aria-label="{t.count} au total">{t.count}</span>{/if}
      </button>
    {/each}
  </div>
  {#if tools}<div class="tools">{@render tools()}</div>{/if}
</div>

<style>
  .tabs {
    display: flex;
    align-items: stretch;
    height: 32px;
    padding: 0 8px;
    border-bottom: 1px solid var(--pb-line);
    background: var(--pb-chrome);
  }
  .list {
    display: flex;
    gap: 4px;
  }
  .tab {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    margin-bottom: -1px;
    padding: 0 10px;
    border: 0;
    border-bottom: 2px solid transparent;
    background: transparent;
    color: var(--pb-text-quiet);
    cursor: pointer;
  }
  .tab[aria-selected='true'] {
    color: var(--pb-text);
    border-bottom-color: var(--pb-accent);
    font-weight: 600;
  }
  .badge {
    min-width: 18px;
    padding: 0 5px;
    border-radius: 8px;
    --tone: var(--pb-warn);
    color: var(--tone);
    background: color-mix(in srgb, var(--tone) 18%, transparent);
    font-size: 11px;
    font-weight: 600;
    line-height: 16px;
    text-align: center;
    font-variant-numeric: tabular-nums;
  }
  .badge.conflict {
    --tone: var(--pb-conflict);
  }
  .tools {
    margin-left: auto;
    display: flex;
    align-items: center;
    gap: 4px;
  }
</style>

<!--
  app/ui/lib/ui/Banner.svelte — Bannière (charte, section 5).
  Bandeau pleine largeur sous la barre d'outils : icône de sévérité, phrase,
  actions. Remplace tout dialogue bloquant pour une information. Le liseré
  gauche et l'icône portent la sévérité ; la phrase dit quoi faire (section 7).
-->
<script lang="ts">
  import type { Snippet } from 'svelte';
  import Icon, { type IconName } from './Icon.svelte';

  let {
    tone,
    message,
    actions,
    children,
  }: {
    tone: 'info' | 'warn' | 'conflict';
    message: string;
    /** Boutons d'action, à droite. */
    actions?: Snippet;
    /** Détails sous la phrase (commandes, liste). */
    children?: Snippet;
  } = $props();

  const ICON: Record<typeof tone, IconName> = { info: 'info', warn: 'warn', conflict: 'error' };
</script>

<div class="banner {tone}" role={tone === 'info' ? 'status' : 'alert'}>
  <span class="sev"><Icon name={ICON[tone]} /></span>
  <div class="body">
    <p>{message}</p>
    {#if children}{@render children()}{/if}
  </div>
  {#if actions}<div class="actions">{@render actions()}</div>{/if}
</div>

<style>
  .banner {
    display: flex;
    gap: 8px;
    align-items: flex-start;
    padding: 6px 12px;
    border-bottom: 1px solid var(--pb-line);
    border-left: 3px solid var(--tone);
    background: var(--pb-panel);
    --tone: var(--pb-accent);
  }
  .warn {
    --tone: var(--pb-warn);
  }
  .conflict {
    --tone: var(--pb-conflict);
  }
  .sev {
    display: inline-flex;
    height: 28px;
    align-items: center;
    color: var(--tone);
  }
  .body {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  p {
    margin: 0;
    padding: 4px 0; /* première ligne centrée sur les boutons de 28 px */
    line-height: 20px;
  }
  .actions {
    display: flex;
    gap: 8px;
    align-items: center;
    flex: none;
  }
</style>

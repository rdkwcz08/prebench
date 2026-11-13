<!--
  app/ui/lib/ui/StatePill.svelte — Pastille d'état (charte, sections 2.3 et 5).
  Fond teinté à 15 % de la couleur d'état, texte de la couleur d'état, mot
  explicite obligatoire : la couleur ne porte jamais l'état seule.
-->
<script lang="ts" module>
  export type StateTone = 'high' | 'low' | 'ok' | 'warn' | 'conflict' | 'neutral';
</script>

<script lang="ts">
  let { tone, text }: { tone: StateTone; text: string } = $props();
</script>

<span class="pill {tone}">{text}</span>

<style>
  .pill {
    display: inline-block;
    padding: 0 8px;
    border-radius: 3px;
    font-size: 12px;
    font-weight: 600;
    line-height: 20px;
    white-space: nowrap;
    --tone: var(--pb-text-quiet);
    color: var(--tone);
    background: color-mix(in srgb, var(--tone) 15%, transparent);
  }
  .high {
    --tone: var(--pb-high);
  }
  .low {
    --tone: var(--pb-low);
  }
  .ok {
    --tone: var(--pb-ok);
  }
  .warn {
    --tone: var(--pb-warn);
  }
  .conflict {
    --tone: var(--pb-conflict);
  }
</style>

<!--
  app/ui/lib/ui/Field.svelte — Champ de saisie (charte, section 5).
  28 px, bordure --pb-line, focus en contour 2 px --pb-accent. Libellé au-dessus
  (12 px, texte secondaire) ; « data » passe la saisie en chasse fixe (chemins).
-->
<script lang="ts">
  let {
    label,
    value = $bindable(''),
    data = false,
    placeholder,
    disabled = false,
  }: {
    label: string;
    value?: string;
    data?: boolean;
    placeholder?: string;
    disabled?: boolean;
  } = $props();
</script>

<label class="field">
  <span class="label">{label}</span>
  <input class:data bind:value {placeholder} {disabled} spellcheck="false" autocomplete="off" />
</label>

<style>
  .field {
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-width: 0;
  }
  .label {
    font-size: 12px;
    color: var(--pb-text-quiet);
  }
  input {
    height: 28px;
    width: 100%;
    padding: 0 8px;
    border: 1px solid var(--pb-line);
    border-radius: 3px;
    background: var(--pb-chrome);
    user-select: text;
  }
  input.data {
    font-size: 12px;
  }
  input:focus {
    outline: 2px solid var(--pb-accent);
    outline-offset: -1px;
    border-color: transparent;
  }
  input:disabled {
    opacity: 0.45;
  }
</style>

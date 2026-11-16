<!--
  app/ui/lib/ParamTable.svelte — Paramètre « table » d'un composant dans l'inspecteur
  (charte, section 5 « Paramètre table ») : une ligne par entrée, un champ de
  28 px par colonne déclarée dans le manifeste (entier en chasse fixe,
  hexadécimal si `display: "hex"`, liste pour un choix, case à cocher), bouton
  icône « Supprimer la ligne », puis « Ajouter une ligne » tant que max_rows
  n'est pas atteint. Chaque modification part en une fois (historique d'édition).
-->
<script lang="ts">
  import { hex, parseInteger } from './format';
  import type { ParamDef, ScalarParamDef, TableRow } from './types';
  import Button from './ui/Button.svelte';
  import IconButton from './ui/IconButton.svelte';

  let {
    def,
    rows,
    words,
    onchange,
  }: {
    def: ParamDef;
    rows: TableRow[];
    /** Mots français des valeurs d'une liste (« fixed » -> « Valeur fixe »). */
    words: Record<string, string>;
    onchange: (rows: TableRow[]) => void;
  } = $props();

  const columns = $derived(Object.entries(def.columns ?? {}) as [string, ScalarParamDef][]);
  const max = $derived(def.max_rows ?? 256);

  function shown(col: ScalarParamDef, v: unknown): string {
    if (typeof v === 'number' && col.display === 'hex') return hex(v);
    return String(v ?? '');
  }

  function update(index: number, key: string, value: string | number | boolean): void {
    onchange(rows.map((r, i) => (i === index ? { ...r, [key]: value } : r)));
  }

  function remove(index: number): void {
    onchange(rows.filter((_, i) => i !== index));
  }

  function add(): void {
    const row: TableRow = {};
    for (const [key, col] of columns) {
      const d = col.default;
      row[key] = typeof d === 'number' || typeof d === 'string' || typeof d === 'boolean' ? d : 0;
    }
    // Entier en première colonne (numéro de registre) : le suivant du dernier.
    const [firstKey, first] = columns[0] ?? [];
    const last = rows[rows.length - 1]?.[firstKey ?? ''];
    if (firstKey !== undefined && first?.type === 'integer' && typeof last === 'number') {
      row[firstKey] = Math.min(first.max ?? last + 1, last + 1);
    }
    onchange([...rows, row]);
  }

  /** Entier ou nombre saisi : valide et dans la plage, sinon la valeur est rétablie. */
  function commitNumber(e: Event, index: number, key: string, col: ScalarParamDef, before: unknown): void {
    const input = e.currentTarget as HTMLInputElement;
    const n = col.type === 'integer' ? parseInteger(input.value, col.display === 'hex') : Number(input.value);
    const ok = Number.isFinite(n) && (col.min === undefined || n >= col.min) && (col.max === undefined || n <= col.max);
    if (ok) update(index, key, col.type === 'integer' ? Math.round(n) : n);
    else input.value = shown(col, before);
  }

  function keydown(e: KeyboardEvent, col: ScalarParamDef, before: unknown): void {
    e.stopPropagation(); // Supprimer, flèches : pas pour la paillasse
    const input = e.currentTarget as HTMLInputElement;
    if (e.key === 'Enter') input.blur();
    if (e.key === 'Escape') {
      input.value = shown(col, before);
      input.blur();
    }
  }
</script>

<div class="table" style:--cols={columns.length}>
  {#if rows.length > 0}
    <div class="row head" aria-hidden="true">
      {#each columns as [key, col] (key)}<span>{col.label ?? key}</span>{/each}
      <span></span>
    </div>
  {/if}
  {#each rows as row, index (index)}
    <div class="row">
      {#each columns as [key, col] (key)}
        {@const v = row[key] ?? col.default}
        {@const name = `${col.label ?? key}, ligne ${index + 1}`}
        {#if col.type === 'enum'}
          <select
            aria-label={name}
            value={String(v)}
            onkeydown={(e) => e.stopPropagation()}
            onchange={(e) => {
              const raw = e.currentTarget.value;
              const match = (col.values ?? []).find((x) => String(x) === raw);
              if (match !== undefined) update(index, key, match);
            }}
          >
            {#each col.values ?? [] as option (option)}
              <option value={String(option)}>{words[String(option)] ?? option}</option>
            {/each}
          </select>
        {:else if col.type === 'boolean'}
          <input
            type="checkbox"
            class="check"
            aria-label={name}
            checked={v === true}
            onchange={(e) => update(index, key, e.currentTarget.checked)}
          />
        {:else if col.type === 'integer' || col.type === 'number'}
          <input
            class="data"
            aria-label={name}
            inputmode={col.display === 'hex' ? 'text' : 'numeric'}
            value={shown(col, v)}
            onkeydown={(e) => keydown(e, col, v)}
            onchange={(e) => commitNumber(e, index, key, col, v)}
          />
        {:else}
          <input
            aria-label={name}
            value={String(v ?? '')}
            onkeydown={(e) => keydown(e, col, v)}
            onchange={(e) => update(index, key, e.currentTarget.value)}
          />
        {/if}
      {/each}
      <IconButton icon="close" label="Supprimer la ligne {index + 1}" align="end" onclick={() => remove(index)} />
    </div>
  {/each}
  {#if rows.length === 0}
    <p class="empty">Aucune ligne. Ajoutez-en une ci-dessous.</p>
  {/if}
  <div>
    <Button icon="plus" disabled={rows.length >= max} title={rows.length >= max ? `${max} lignes au plus` : undefined} onclick={add}>
      Ajouter une ligne
    </Button>
  </div>
</div>

<style>
  .table {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .row {
    display: grid;
    grid-template-columns: repeat(var(--cols), minmax(0, 1fr)) 28px;
    gap: 4px;
    align-items: center;
  }
  .head span {
    font-size: 12px;
    color: var(--pb-text-quiet);
  }
  input,
  select {
    height: 28px;
    min-width: 0;
    padding: 0 8px;
    border: 1px solid var(--pb-line);
    border-radius: 3px;
    background: var(--pb-panel);
    color: var(--pb-text);
    font-size: 13px;
  }
  input.data {
    font: 400 12px var(--pb-font-data);
    font-variant-numeric: tabular-nums;
  }
  input:focus,
  select:focus {
    outline: 2px solid var(--pb-accent);
    outline-offset: -1px;
  }
  .check {
    width: 16px;
    height: 16px;
    accent-color: var(--pb-accent);
  }
  .empty {
    margin: 0;
    font-size: 12px;
    color: var(--pb-text-quiet);
  }
</style>

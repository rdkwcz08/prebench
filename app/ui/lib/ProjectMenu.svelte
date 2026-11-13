<!--
  app/ui/lib/ProjectMenu.svelte — Sélecteur de projet de la barre d'outils.
  Bouton « puce + nom du projet », qui ouvre un menu flottant (charte, section 4 :
  rayon 6 px, ombre réservée à ce qui flotte) : ouvrir un dossier de projet, ou
  créer un projet à partir d'un modèle (section 10.1). Échap ou un clic en
  dehors referme le menu.
-->
<script lang="ts">
  import { bench } from './bench.svelte';
  import type { TemplateInfo } from './types';
  import Button from './ui/Button.svelte';
  import Field from './ui/Field.svelte';
  import Icon from './ui/Icon.svelte';

  let {
    open = $bindable(false),
    path = $bindable(''),
    disabled = false,
  }: { open?: boolean; path?: string; disabled?: boolean } = $props();

  let templates = $state<TemplateInfo[]>([]);
  let template = $state('');
  let dest = $state('');
  let error = $state('');
  let root: HTMLDivElement | undefined = $state();

  const name = $derived(bench.project ? basename(bench.project.path) : 'Aucun projet');

  function basename(p: string): string {
    return p.split(/[\\/]/).filter(Boolean).pop() ?? p;
  }

  /** Ouvre le menu ; `preselect` choisit un modèle (« Créer mon premier projet »). */
  export async function show(preselect?: string): Promise<void> {
    error = '';
    open = true;
    try {
      templates = await bench.listTemplates();
      const chosen = templates.find((t) => t.id === preselect) ?? templates[0];
      if (chosen !== undefined) {
        template = chosen.id;
        dest = chosen.suggested_dir;
      }
    } catch (e) {
      error = String(e);
    }
  }

  function chooseTemplate(id: string): void {
    template = id;
    dest = templates.find((t) => t.id === id)?.suggested_dir ?? dest;
  }

  async function openProject(): Promise<void> {
    error = '';
    try {
      await bench.open(path);
      open = false;
    } catch (e) {
      error = String(e);
    }
  }

  async function createProject(): Promise<void> {
    error = '';
    try {
      path = await bench.createProject(template, dest);
      await bench.open(path);
      open = false;
    } catch (e) {
      error = String(e);
    }
  }

  function onwindowpointerdown(e: PointerEvent): void {
    if (open && root !== undefined && !root.contains(e.target as Node)) open = false;
  }

  function onwindowkeydown(e: KeyboardEvent): void {
    if (open && e.key === 'Escape') open = false;
  }
</script>

<svelte:window onpointerdown={onwindowpointerdown} onkeydown={onwindowkeydown} />

<div class="project" bind:this={root}>
  <button
    class="picker"
    aria-haspopup="dialog"
    aria-expanded={open}
    title="Ouvrir ou créer un projet"
    {disabled}
    onclick={() => (open ? (open = false) : void show())}
  >
    <Icon name="chip" />
    <span class:quiet={!bench.project}>{name}</span>
    <Icon name="chevron" />
  </button>

  {#if open}
    <div class="menu" role="dialog" aria-label="Projet">
      <form onsubmit={(e) => { e.preventDefault(); void openProject(); }}>
        <h2>Ouvrir un projet</h2>
        <div class="row">
          <Field label="Dossier du projet" bind:value={path} data />
          <Button type="submit" disabled={path === ''}>Ouvrir</Button>
        </div>
      </form>
      <form onsubmit={(e) => { e.preventDefault(); void createProject(); }}>
        <h2>Nouveau projet</h2>
        <label class="select">
          <span>Modèle</span>
          <select value={template} onchange={(e) => chooseTemplate(e.currentTarget.value)}>
            {#each templates as t (t.id)}
              <option value={t.id}>{t.id}</option>
            {/each}
          </select>
        </label>
        <Field label="Dossier du nouveau projet" bind:value={dest} data />
        <div class="actions">
          <Button variant="primary" type="submit" disabled={template === '' || dest === ''}>Créer le projet</Button>
        </div>
      </form>
      {#if error}<p class="error" role="alert"><Icon name="error" />{error}</p>{/if}
    </div>
  {/if}
</div>

<style>
  .project {
    position: relative;
  }
  .picker {
    height: 28px;
    padding: 0 8px;
    display: inline-flex;
    align-items: center;
    gap: 6px;
    border: 1px solid transparent;
    border-radius: 3px;
    background: transparent;
    cursor: pointer;
  }
  .picker:hover:not(:disabled) {
    border-color: var(--pb-line);
  }
  .picker:disabled {
    opacity: 0.45;
    cursor: default;
  }
  .quiet {
    color: var(--pb-text-quiet);
  }
  .menu {
    position: absolute;
    top: calc(100% + 4px);
    left: 0;
    z-index: 40;
    width: 440px;
    padding: 4px 0;
    border: 1px solid var(--pb-line);
    border-radius: 6px;
    background: var(--pb-panel);
    box-shadow: var(--pb-shadow);
  }
  form {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 8px 12px 12px;
  }
  form + form {
    border-top: 1px solid var(--pb-line);
  }
  h2 {
    margin: 0;
    font-size: 13px;
    font-weight: 600;
  }
  .row {
    display: flex;
    gap: 8px;
    align-items: flex-end;
  }
  .row > :global(:first-child) {
    flex: 1;
  }
  .select {
    display: flex;
    flex-direction: column;
    gap: 4px;
    font-size: 12px;
    color: var(--pb-text-quiet);
  }
  select {
    height: 28px;
    padding: 0 6px;
    border: 1px solid var(--pb-line);
    border-radius: 3px;
    background: var(--pb-chrome);
    color: var(--pb-text);
    font-size: 13px;
  }
  .actions {
    display: flex;
    justify-content: flex-end;
  }
  .error {
    display: flex;
    gap: 8px;
    margin: 0;
    padding: 8px 12px;
    border-top: 1px solid var(--pb-line);
    color: var(--pb-conflict);
    user-select: text;
  }
</style>

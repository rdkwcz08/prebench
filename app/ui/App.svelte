<!--
  app/ui/App.svelte — Fenêtre de Prebench (phase 1).
  Barre d'outils, paillasse (carte et composants), panneaux Journal et ⚠
  (disposition de la section 10.3, réduite au prototype).
-->
<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { onMount } from 'svelte';
  import { bench } from './lib/bench.svelte';
  import ComponentHost from './lib/ComponentHost.svelte';
  import type { TemplateInfo } from './lib/types';

  let path = $state('');
  let tab = $state<'log' | 'warn'>('log');
  let update = $state<{ version: string } | null>(null);
  let updating = $state('');
  // Formulaire « Nouveau projet » : copie d'un modèle (section 10.1).
  let templates = $state<TemplateInfo[] | null>(null);
  let template = $state('');
  let dest = $state('');
  let createError = $state('');

  const running = $derived(['building', 'starting', 'running'].includes(bench.status.state));
  const onboard = $derived(bench.instances.filter((i) => i.onboard));
  const placed = $derived(bench.instances.filter((i) => !i.onboard));
  const missing = $derived(bench.prerequisites?.tools.filter((t) => t.path === null) ?? []);
  // Une commande partagée par deux outils (gcc et make de MSYS2) n'est affichée qu'une fois.
  const installs = $derived([...new Set(missing.map((t) => t.install))]);

  onMount(async () => {
    await bench.init();
    await bench.checkPrerequisites();
    const first = await bench.defaultProject();
    if (first !== null) {
      path = first;
      await open();
    }
    // Hors ligne, la vérification échoue sans bruit (D10).
    update = await invoke<{ version: string } | null>('check_update').catch(() => null);
  });

  async function installUpdate(): Promise<void> {
    updating = 'Téléchargement et vérification de la signature…';
    try {
      await invoke('install_update'); // redémarre l'app en cas de succès
    } catch (e) {
      updating = String(e);
    }
  }

  /** Modèle de « Créer mon premier projet ». */
  const FIRST_TEMPLATE = 'blinky-stm32-hal';

  async function showNewProject(preselect?: string): Promise<void> {
    createError = '';
    try {
      templates = await bench.listTemplates();
      const chosen = templates.find((t) => t.id === preselect) ?? templates[0];
      if (chosen !== undefined) {
        template = chosen.id;
        dest = chosen.suggested_dir;
      }
    } catch (e) {
      templates = null;
      bench.status = { state: 'error', message: String(e) };
    }
  }

  function chooseTemplate(id: string): void {
    template = id;
    dest = templates?.find((t) => t.id === id)?.suggested_dir ?? dest;
  }

  async function createProject(): Promise<void> {
    createError = '';
    try {
      path = await bench.createProject(template, dest);
      templates = null;
      await open();
    } catch (e) {
      createError = String(e);
    }
  }

  async function open(): Promise<void> {
    try {
      await bench.open(path);
    } catch (e) {
      bench.status = { state: 'error', message: String(e) };
    }
  }
</script>

<div class="layout">
  <div class="top">
  {#if update}
    <div class="update">
      Prebench {update.version} est disponible.
      {#if updating}
        <span>{updating}</span>
      {:else}
        <button onclick={installUpdate}>Installer et redémarrer</button>
        <button onclick={() => (update = null)}>Plus tard</button>
      {/if}
    </div>
  {/if}
  {#if missing.length > 0}
    <div class="prereq" role="alert">
      <div class="prereq-title">
        Prérequis manquants pour simuler :
        {#each bench.prerequisites?.tools ?? [] as t (t.kind)}
          <span class="tool">{t.path === null ? '❌' : '✅'} {t.label}</span>
        {/each}
        <button onclick={() => bench.checkPrerequisites()} disabled={running}>Revérifier</button>
      </div>
      <div>Commande à lancer dans un terminal, puis cliquer sur Revérifier :</div>
      {#each installs as command (command)}
        <pre>{command}</pre>
      {/each}
      <div class="readme">Voir le {bench.prerequisites?.readme}</div>
    </div>
  {/if}
  <header>
    <strong class="brand">Prebench</strong>
    <input bind:value={path} spellcheck="false" aria-label="Dossier du projet" disabled={running} />
    <button onclick={open} disabled={running}>Ouvrir</button>
    <button onclick={() => showNewProject()} disabled={running}>Nouveau projet…</button>
    <span class="sep"></span>
    <button class="primary" onclick={() => bench.start()} disabled={running || !bench.project}>▶ Démarrer</button>
    <button onclick={() => bench.stop()} disabled={!running}>■ Arrêter</button>
    <button onclick={() => bench.reset()} disabled={bench.status.state !== 'running'}>⟲ Reset</button>
    <span class="status {bench.status.state}" title={bench.status.message}>
      {bench.status.message || bench.status.state}
    </span>
  </header>
  {#if templates !== null}
    <form class="new-project" onsubmit={(e) => { e.preventDefault(); void createProject(); }}>
      <label>
        Modèle
        <select value={template} onchange={(e) => chooseTemplate(e.currentTarget.value)}>
          {#each templates as t (t.id)}
            <option value={t.id}>{t.id}</option>
          {/each}
        </select>
      </label>
      <label class="dest">
        Dossier du nouveau projet
        <input bind:value={dest} spellcheck="false" />
      </label>
      <button class="primary" type="submit" disabled={!template || !dest}>Créer</button>
      <button type="button" onclick={() => (templates = null)}>Annuler</button>
      {#if createError}
        <div class="create-error">{createError}</div>
      {/if}
    </form>
  {/if}
  </div>

  <main class="paillasse">
    {#if bench.project}
      <section class="board" aria-label={bench.project.board_name}>
        <div class="board-title">{bench.project.board_name}</div>
        <div class="board-mcu">{bench.project.mcu}</div>
        {#each onboard as inst (inst.id)}
          <ComponentHost {inst} mode="simulate" />
        {/each}
      </section>
      {#each placed as inst (inst.id)}
        <ComponentHost {inst} mode="simulate" />
      {/each}
    {:else if templates === null}
      <div class="welcome">
        <p>Bienvenue dans Prebench. Pour commencer, crée un projet à partir du modèle blinky STM32 HAL : il sera copié dans un dossier que tu pourras modifier.</p>
        <button class="primary" onclick={() => showNewProject(FIRST_TEMPLATE)}>Créer mon premier projet</button>
      </div>
    {/if}
  </main>

  <footer>
    <nav>
      <button class:active={tab === 'log'} onclick={() => (tab = 'log')}>Journal</button>
      <button class:active={tab === 'warn'} onclick={() => (tab = 'warn')}>⚠ {bench.warnings.length}</button>
    </nav>
    <div class="panel">
      {#if tab === 'log'}
        {#each bench.logs as line, i (i)}
          <div class="line"><span class="src">{line.source}</span>{line.text}</div>
        {/each}
      {:else}
        {#each bench.warnings as text (text)}
          <div class="line warn">⚠ {text}</div>
        {/each}
      {/if}
    </div>
  </footer>
</div>

<style>
  .layout {
    display: grid;
    grid-template-rows: auto 1fr 200px;
    height: 100%;
  }
  .update {
    display: flex;
    gap: 8px;
    align-items: center;
    padding: 6px 12px;
    background: var(--accent);
    color: #062016;
  }
  .prereq {
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: 8px 12px;
    background: var(--panel);
    border-bottom: 2px solid var(--error);
    font-size: 12px;
  }
  .prereq-title {
    display: flex;
    flex-wrap: wrap;
    gap: 12px;
    align-items: center;
    font-weight: 600;
  }
  .prereq .tool {
    font-weight: normal;
  }
  .prereq pre {
    margin: 0;
    padding: 4px 8px;
    border-radius: 4px;
    background: var(--bg);
    white-space: pre-wrap;
    user-select: text;
    font-family: ui-monospace, Menlo, Consolas, monospace;
  }
  .readme {
    color: var(--text-muted);
    user-select: text;
  }
  .new-project {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    align-items: flex-end;
    padding: 8px 12px;
    background: var(--panel);
    border-bottom: 1px solid var(--border);
    font-size: 12px;
  }
  .new-project label {
    display: flex;
    flex-direction: column;
    gap: 2px;
    color: var(--text-muted);
  }
  .new-project .dest {
    flex: 1;
  }
  .new-project select {
    padding: 5px 8px;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: var(--bg);
    color: var(--text);
  }
  .create-error {
    flex-basis: 100%;
    color: var(--error);
  }
  header {
    display: flex;
    gap: 8px;
    align-items: center;
    padding: 8px 12px;
    background: var(--panel);
    border-bottom: 1px solid var(--border);
  }
  .brand {
    margin-right: 8px;
  }
  input {
    flex: 1;
    min-width: 120px;
    padding: 5px 8px;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: var(--bg);
    color: var(--text);
    font-family: ui-monospace, Menlo, Consolas, monospace;
    font-size: 12px;
  }
  button {
    padding: 5px 10px;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: var(--bg);
    color: var(--text);
    cursor: pointer;
  }
  button:disabled {
    opacity: 0.45;
    cursor: default;
  }
  button.primary:not(:disabled) {
    background: var(--accent);
    border-color: var(--accent);
    color: #062016;
  }
  .sep {
    width: 8px;
  }
  .status {
    max-width: 320px;
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
    font-size: 12px;
    color: var(--text-muted);
  }
  .status.running {
    color: var(--accent);
  }
  .status.error {
    color: var(--error);
  }
  .paillasse {
    position: relative;
    overflow: auto;
    background-image: radial-gradient(var(--border) 1px, transparent 1px);
    background-size: 20px 20px;
  }
  .welcome {
    max-width: 420px;
    margin: 80px auto;
    padding: 20px;
    border: 1px solid var(--border);
    border-radius: 10px;
    background: var(--panel);
    text-align: center;
  }
  .welcome p {
    margin: 0 0 16px;
    line-height: 1.5;
  }
  .board {
    position: absolute;
    left: 40px;
    top: 40px;
    width: 420px;
    height: 260px;
    border-radius: 10px;
    background: var(--pcb);
    border: 3px solid var(--pcb-edge);
  }
  .board-title {
    position: absolute;
    left: 16px;
    top: 12px;
    font-weight: 600;
    color: #e8f5ee;
  }
  .board-mcu {
    position: absolute;
    left: 16px;
    top: 32px;
    font-size: 12px;
    color: #b9dcc9;
  }
  footer {
    display: flex;
    flex-direction: column;
    min-height: 0;
    border-top: 1px solid var(--border);
    background: var(--panel);
  }
  nav {
    display: flex;
    gap: 4px;
    padding: 4px 8px;
  }
  nav button {
    border: none;
    background: none;
    color: var(--text-muted);
  }
  nav button.active {
    color: var(--text);
    border-bottom: 2px solid var(--accent);
    border-radius: 0;
  }
  .panel {
    flex: 1;
    overflow: auto;
    padding: 4px 12px;
    font-family: ui-monospace, Menlo, Consolas, monospace;
    font-size: 12px;
  }
  .line {
    white-space: pre-wrap;
  }
  .src {
    display: inline-block;
    min-width: 72px;
    color: var(--text-muted);
  }
  .warn {
    color: var(--warn);
  }
</style>

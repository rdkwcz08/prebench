<!--
  app/ui/App.svelte — Écran principal de Prebench (charte docs/design.md, section 4 ;
  maquette docs/design/maquette-ecran-principal.html).

  Barre d'outils 40 px ; bannières pleine largeur ; paillasse (tapis ESD) et
  inspecteur 280 px repliable ; panneau d'onglets 220 px repliable ; barre
  d'état 24 px. Raccourcis (section 10.6) : Espace met en pause ou reprend
  (mode Simulation ; en Édition, il déplace la vue), R réinitialise le firmware,
  E bascule Édition / Simulation, Cmd/Ctrl+Z annule, Maj+Cmd/Ctrl+Z rétablit.
-->
<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { onMount } from 'svelte';
  import { bench } from './lib/bench.svelte';
  import AddComponentMenu from './lib/AddComponentMenu.svelte';
  import Workbench from './lib/Workbench.svelte';
  import BuildPanel from './lib/BuildPanel.svelte';
  import SelectionInspector from './lib/SelectionInspector.svelte';
  import WarningsPanel from './lib/WarningsPanel.svelte';
  import PlotPanel from './lib/PlotPanel.svelte';
  import SerialPanel from './lib/SerialPanel.svelte';
  import SerialTools from './lib/SerialTools.svelte';
  import { serial } from './lib/serial.svelte';
  import ProjectMenu from './lib/ProjectMenu.svelte';
  import { settings } from './lib/settings.svelte';
  import { THEME_LABEL, theme } from './lib/theme.svelte';
  import type { SimState } from './lib/types';
  import Banner from './lib/ui/Banner.svelte';
  import Button from './lib/ui/Button.svelte';
  import Icon from './lib/ui/Icon.svelte';
  import IconButton from './lib/ui/IconButton.svelte';
  import PropList, { type Prop } from './lib/ui/PropList.svelte';
  import Tabs from './lib/ui/Tabs.svelte';

  /** Touche de commande affichée dans les infobulles : ⌘ sur Mac, Ctrl ailleurs. */
  const MOD = /Mac/.test(navigator.userAgent) ? '⌘' : 'Ctrl+';

  /** Modèle de « Créer mon premier projet ». */
  const FIRST_TEMPLATE = 'blinky-stm32-hal';

  const STATE_LABEL: Record<SimState, string> = {
    idle: 'Prête',
    building: 'Compilation',
    starting: 'Démarrage',
    running: 'En cours',
    paused: 'En pause',
    'debug-wait': 'En attente du débogueur',
    breakpoint: "Arrêté sur un point d'arrêt",
    stopped: 'Arrêtée',
    error: 'Erreur',
  };

  const FRAMEWORK_LABEL: Record<string, string> = {
    'stm32-hal': 'STM32 HAL',
    'esp-idf': 'ESP-IDF',
    arduino: 'Arduino',
  };

  let path = $state('');
  let menuOpen = $state(false);
  let menu: ProjectMenu | undefined = $state();
  let tab = $state('serial');
  /** Alertes de débit du moniteur série (section 9.2), dans l'onglet Avertissements. */
  const serialWarnings = $derived(serial.ports.map((p) => serial.mismatch(p)).filter((w): w is string => w !== null));
  let inspectorOpen = $state(true);
  let dockOpen = $state(true);
  let update = $state<{ version: string } | null>(null);
  let updating = $state('');

  /** Simulation active : en compilation, au démarrage, en cours ou en pause. */
  const running = $derived(
    ['building', 'starting', 'running', 'paused', 'debug-wait', 'breakpoint'].includes(bench.status.state),
  );
  const missingDebugTools = $derived(bench.debug?.tools.filter((t) => !t.present) ?? []);
  const live = $derived(bench.status.state === 'running' || bench.status.state === 'paused');
  const tools = $derived(bench.prerequisites?.tools ?? []);
  const missing = $derived(tools.filter((t) => t.path === null));
  // Une commande partagée par deux outils (gcc et make de MSYS2) n'est affichée qu'une fois.
  const installs = $derived([...new Set(missing.map((t) => t.install))]);

  const projectProps = $derived<Prop[]>(
    bench.project
      ? [
          { label: 'Dossier', value: bench.project.path, data: true, full: bench.project.path },
          { label: 'Framework', value: FRAMEWORK_LABEL[bench.project.framework] ?? bench.project.framework },
          { label: 'Composants', value: String(bench.instances.length), data: true },
        ]
      : [],
  );
  const toolProps = $derived<Prop[]>(
    tools.map((t) => ({
      label: t.label,
      value: t.path === null ? 'Absent' : 'Présent',
      tone: t.path === null ? 'conflict' : 'ok',
      full: t.path ?? t.install,
    })),
  );

  // Build en échec : l'onglet Build s'affiche, panneau déplié (section 10.2).
  let shownFailure: object | null = null;
  $effect(() => {
    const r = bench.build.result;
    if (r !== null && r !== shownFailure && r.errors > 0) {
      shownFailure = r;
      tab = 'build';
      dockOpen = true;
    }
  });

  /** Durée d'un build, en secondes avec une décimale. */
  function formatSeconds(ms: number): string {
    return `${(ms / 1000).toLocaleString('fr-FR', { minimumFractionDigits: 1, maximumFractionDigits: 1 })} s`;
  }

  const debugProps = $derived<Prop[]>(
    (bench.debug?.tools ?? []).map((t) => ({
      label: t.label,
      value: t.present ? 'Présent' : 'Absent',
      tone: t.present ? 'ok' : 'conflict',
      full: t.path ?? t.install,
    })),
  );

  onMount(async () => {
    await settings.load();
    theme.adoptSetting();
    await bench.init();
    await serial.init();
    await bench.checkPrerequisites();
    const first = await bench.defaultProject();
    if (first !== null) {
      path = first;
      try {
        await bench.open(path);
      } catch (e) {
        bench.status = { state: 'error', message: String(e) };
      }
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

  function togglePause(): void {
    if (bench.status.state === 'running') void bench.pause();
    else if (bench.status.state === 'paused') void bench.resume();
  }

  /** CPU du firmware, en % d'un cœur, sans décimale. */
  function formatCpu(pct: number): string {
    return `${Math.round(pct).toLocaleString('fr-FR')} %`;
  }

  /** Mémoire en Mo : une décimale sous 10 Mo. */
  function formatMemory(bytes: number): string {
    const mo = bytes / (1024 * 1024);
    return `${mo.toLocaleString('fr-FR', { maximumFractionDigits: mo < 10 ? 1 : 0, minimumFractionDigits: mo < 10 ? 1 : 0 })} Mo`;
  }

  let benchView: Workbench | undefined = $state();

  function onkeydown(e: KeyboardEvent): void {
    const target = e.target as Element | null;
    if (target?.closest('input, select, textarea, [contenteditable]')) return;
    if ((e.metaKey || e.ctrlKey) && !e.altKey && (e.key === '0' || e.key === '1')) {
      e.preventDefault();
      if (e.key === '0') benchView?.fit();
      else benchView?.actualSize();
      return;
    }
    if ((e.metaKey || e.ctrlKey) && !e.altKey && (e.key === 'z' || e.key === 'Z' || e.key === 'y')) {
      e.preventDefault();
      if (e.key === 'y' || e.shiftKey) void bench.redo();
      else void bench.undo();
      return;
    }
    if (e.metaKey || e.ctrlKey || e.altKey || e.repeat) return;
    if (e.key === 'e' || e.key === 'E') {
      if (bench.project) bench.mode = bench.mode === 'edit' ? 'simulate' : 'edit';
    } else if (e.key === ' ') {
      if (bench.mode === 'edit') return; // Workbench.svelte : déplacement de la vue
      // Espace garde son rôle sur un bouton ou un onglet qui a le focus.
      if (target?.closest('button, [role="button"], [role="tab"], a')) return;
      e.preventDefault();
      togglePause();
    } else if (e.key === 'r' || e.key === 'R') {
      if (live) void bench.reset();
    }
  }
</script>

<svelte:window {onkeydown} />

<div class="app" class:no-inspector={!inspectorOpen} class:no-dock={!dockOpen}>
  <header class="toolbar">
    <ProjectMenu bind:this={menu} bind:open={menuOpen} bind:path disabled={running} />
    {#if bench.project}
      <span class="board-name"><span class="quiet">Carte</span>{bench.project.board_name}</span>
    {/if}
    <span class="sep"></span>
    {#if bench.status.state === 'running'}
      <Button variant="primary" icon="pause" title="Mettre en pause (Espace, en mode Simulation)" onclick={togglePause}>Mettre en pause</Button>
    {:else if bench.status.state === 'paused'}
      <Button variant="primary" icon="play" title="Reprendre (Espace, en mode Simulation)" onclick={togglePause}>Reprendre</Button>
    {:else}
      <Button variant="primary" icon="play" disabled={running || !bench.project} onclick={() => bench.start()}>Démarrer</Button>
    {/if}
    {#if !running}
      <Button icon="bug" disabled={!bench.project} title="Déboguer dans VS Code" onclick={() => bench.startDebug()}>Déboguer</Button>
    {/if}
    <IconButton icon="reset" label="Réinitialiser" shortcut="R" disabled={!live} onclick={() => bench.reset()} />
    <IconButton icon="stop" label="Arrêter la simulation" disabled={!running} onclick={() => bench.stop()} />
    <span class="sep"></span>
    <div class="segmented" role="group" aria-label="Mode">
      <button aria-pressed={bench.mode === 'edit'} title="Mode édition (E) : placer et câbler" disabled={!bench.project} onclick={() => (bench.mode = 'edit')}>Édition</button>
      <button aria-pressed={bench.mode === 'simulate'} title="Mode simulation (E) : interagir" disabled={!bench.project} onclick={() => (bench.mode = 'simulate')}>Simulation</button>
    </div>
    <span class="spacer"></span>
    <AddComponentMenu place={() => benchView?.center() ?? [480, 96]} disabled={!bench.project} />
    <IconButton icon="note" label="Ajouter une note" disabled={!bench.project} onclick={() => benchView?.addNoteAtCenter()} />
    <IconButton icon="fit" label="Ajuster la vue" shortcut="{MOD}0" disabled={!bench.project} onclick={() => benchView?.fit()} />
    <IconButton
      icon="panel-right"
      label={inspectorOpen ? "Masquer l'inspecteur" : "Afficher l'inspecteur"}
      onclick={() => (inspectorOpen = !inspectorOpen)}
    />
    <IconButton
      icon={`theme-${theme.preference}`}
      label="{THEME_LABEL[theme.preference]} (suivant : {THEME_LABEL[theme.next].toLowerCase()})"
      align="end"
      onclick={() => theme.cycle()}
    />
  </header>

  <div class="banners">
    {#if bench.pending.length > 0 && running}
      <Banner
        tone="info"
        message="{bench.pending.join(', ')} {bench.pending.length > 1 ? 'seront actifs' : 'sera actif'} au prochain démarrage de la simulation."
      >
        {#snippet actions()}
          <Button icon="reset" onclick={() => bench.restart()}>Redémarrer pour l'activer</Button>
        {/snippet}
      </Banner>
    {/if}
    {#if bench.status.state === 'debug-wait' && bench.debug}
      <Banner
        tone="info"
        message="Session de débogage prête. Dans VS Code, choisissez « {bench.debug.config_name} » puis lancez le débogage (F5).{bench.debug.launch_json_state === 'created' || bench.debug.launch_json_state === 'updated' ? ' La configuration a été ajoutée à .vscode/launch.json.' : ''}"
      >
        {#snippet actions()}
          <Button icon="stop" onclick={() => bench.stop()}>Arrêter le débogage</Button>
        {/snippet}
      </Banner>
    {/if}
    {#if bench.debug?.launch_json_error}
      <Banner tone="warn" message="{bench.debug.launch_json_error}." />
    {/if}
    {#if missingDebugTools.length > 0}
      <Banner
        tone="warn"
        message="{missingDebugTools.map((t) => t.label).join(', ')} introuvable : nécessaire pour déboguer. Installez-le avec la commande ci-dessous, puis relancez le débogage."
      >
        {#each missingDebugTools as t (t.label)}
          <pre class="data command">{t.install}</pre>
        {/each}
        <span class="quiet">Détails : README, section « Déboguer le firmware ».</span>
      </Banner>
    {/if}
    {#if bench.guard?.kind === 'cpu' && bench.status.state === 'paused'}
      <Banner tone="warn" message={bench.guard.text}>
        {#snippet actions()}
          <Button icon="play" onclick={() => bench.resume()}>Reprendre</Button>
        {/snippet}
      </Banner>
    {:else if bench.guard?.kind === 'memory'}
      <Banner tone="conflict" message={bench.guard.text}>
        {#snippet actions()}
          <Button onclick={() => (bench.guard = null)}>Fermer</Button>
        {/snippet}
      </Banner>
    {/if}
    {#if update}
      <Banner tone="info" message="Prebench {update.version} est disponible.">
        {#snippet actions()}
          {#if updating}
            <span class="quiet">{updating}</span>
          {:else}
            <Button variant="primary" icon="download" onclick={installUpdate}>Installer et redémarrer</Button>
            <Button onclick={() => (update = null)}>Plus tard</Button>
          {/if}
        {/snippet}
      </Banner>
    {/if}
    {#if missing.length > 0}
      <Banner
        tone="warn"
        message="{missing.map((t) => t.label).join(', ')} introuvable. Installez {missing.length > 1
          ? 'ces outils'
          : 'cet outil'} avec la commande ci-dessous dans un terminal, puis cliquez sur Revérifier."
      >
        {#each installs as command (command)}
          <pre class="data command">{command}</pre>
        {/each}
        <span class="quiet">Détails : {bench.prerequisites?.readme}</span>
        {#snippet actions()}
          <Button icon="reset" disabled={running} onclick={() => bench.checkPrerequisites()}>Revérifier</Button>
        {/snippet}
      </Banner>
    {/if}
  </div>

  <Workbench bind:this={benchView} />
  {#if !bench.project && !menuOpen}
    <div class="empty">
      <p>Aucun projet ouvert. Créez votre premier projet à partir du modèle blinky STM32 HAL : il sera copié dans un dossier que vous pourrez modifier.</p>
      <Button variant="primary" icon="plus" onclick={() => menu?.show(FIRST_TEMPLATE)}>Créer mon premier projet</Button>
    </div>
  {/if}

  <aside class="inspector" aria-label="Inspecteur" inert={!inspectorOpen}>
    {#if bench.selection}
      <SelectionInspector />
    {:else if bench.project}
      <div class="insp-head">
        <h2>{bench.project.board_name}</h2>
        <p>Microcontrôleur <span class="data">{bench.project.mcu}</span></p>
      </div>
      <section class="insp-section">
        <h3>Projet</h3>
        <PropList props={projectProps} />
      </section>
    {:else}
      <div class="insp-head">
        <h2>Aucun projet</h2>
        <p>Ouvrez ou créez un projet depuis la barre d'outils.</p>
      </div>
    {/if}
    {#if bench.debug}
      <section class="insp-section">
        <h3>Débogage</h3>
        <PropList props={debugProps} />
      </section>
    {/if}
    {#if toolProps.length > 0}
      <section class="insp-section">
        <h3>Outils de build</h3>
        <PropList props={toolProps} />
      </section>
    {/if}
  </aside>

  <section class="dock" aria-label="Instruments">
    <Tabs
      label="Instruments"
      bind:active={tab}
      tabs={[
        { id: 'serial', label: 'Série' },
        { id: 'plot', label: 'Traceur' },
        { id: 'log', label: 'Journal' },
        {
          id: 'build',
          label: 'Build',
          count: bench.build.result?.errors || bench.build.result?.warnings || 0,
          tone: bench.build.result?.errors ? 'conflict' : 'warn',
        },
        {
          id: 'warn',
          label: 'Avertissements',
          icon: 'warn',
          count: bench.checks.length + bench.warnings.length + serialWarnings.length,
          tone:
            bench.checks.some((c) => c.severity === 'conflict') ||
            bench.warnings.some((w) => w.startsWith('Conflit') || w.startsWith('Court-circuit'))
              ? 'conflict'
              : 'warn',
        },
      ]}
    >
      {#snippet tools()}
        {#if tab === 'serial' || tab === 'plot'}
          <SerialTools full={tab === 'serial'} />
          <span class="sep"></span>
        {/if}
        <IconButton
          icon="panel-bottom"
          label={dockOpen ? 'Replier le panneau' : 'Déplier le panneau'}
          align="end"
          onclick={() => (dockOpen = !dockOpen)}
        />
      {/snippet}
    </Tabs>
    <div class="dock-body" role="tabpanel" inert={!dockOpen}>
      {#if tab === 'serial'}
        <SerialPanel />
      {:else if tab === 'plot'}
        <PlotPanel />
      {:else if tab === 'build'}
        <BuildPanel />
      {:else if tab === 'log'}
        {#each bench.logs as line, i (i)}
          <div class="line"><span class="src">{line.source}</span>{line.text}</div>
        {:else}
          <p class="empty-line">Le journal est vide. Il affiche le build, le démarrage et les messages du firmware.</p>
        {/each}
      {:else}
        <WarningsPanel />
      {/if}
    </div>
  </section>

  <footer class="status">
    <span class="run {bench.status.state}"><span class="dot"></span>{STATE_LABEL[bench.status.state]}</span>
    {#if bench.status.message && bench.status.message !== STATE_LABEL[bench.status.state]}
      <span class="message" title={bench.status.message}>{bench.status.message}</span>
    {/if}
    <span class="right">
      {#if bench.build.running}
        <span>Build en cours</span>
      {:else if bench.build.result?.ok === false}
        <span class="build-failed"><Icon name="error" />Build en échec</span>
      {:else if bench.build.result}
        <span>Build sim <span class="data">{formatSeconds(bench.build.result.duration_ms)}</span></span>
      {/if}
      {#if bench.usage}
        <span title="Processeur utilisé par le firmware, en pourcentage d'un cœur. Pause automatique au-delà de 80 % pendant 5 s.">
          CPU <span class="data meter cpu">{formatCpu(bench.usage.cpu)}</span>
        </span>
        <span title="Mémoire du firmware. Arrêt au-delà de {formatMemory(bench.usage.memory_limit)}.">
          RAM <span class="data meter ram">{formatMemory(bench.usage.memory)}</span>
        </span>
      {/if}
      <span>Prebench <span class="data">{__APP_VERSION__}</span></span>
    </span>
  </footer>
</div>

<style>
  .app {
    display: grid;
    height: 100%;
    min-width: 960px;
    min-height: 600px;
    grid-template-rows: 40px auto minmax(0, 1fr) 220px 24px;
    grid-template-columns: minmax(0, 1fr) 280px;
    grid-template-areas:
      'toolbar toolbar'
      'banners banners'
      'bench inspector'
      'dock dock'
      'status status';
    transition:
      grid-template-columns var(--pb-panel-motion),
      grid-template-rows var(--pb-panel-motion);
  }
  .app.no-inspector {
    grid-template-columns: minmax(0, 1fr) 0;
  }
  .app.no-dock {
    grid-template-rows: 40px auto minmax(0, 1fr) 33px 24px;
  }

  /* Barre d'outils */
  .toolbar {
    grid-area: toolbar;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 0 8px;
    border-bottom: 1px solid var(--pb-line);
  }
  .board-name {
    display: inline-flex;
    gap: 6px;
    padding: 0 8px;
  }
  .quiet {
    color: var(--pb-text-quiet);
  }
  .sep {
    width: 1px;
    height: 20px;
    margin: 0 4px;
    background: var(--pb-line);
  }
  .spacer {
    flex: 1;
  }

  /* Bannières */
  .banners {
    grid-area: banners;
  }
  .command {
    margin: 0;
    padding: 4px 8px;
    border: 1px solid var(--pb-line);
    border-radius: 3px;
    background: var(--pb-chrome);
    font-size: 12px;
    white-space: pre-wrap;
  }

  /* Mode Édition / Simulation : contrôle segmenté (maquette) */
  .segmented {
    display: inline-flex;
    height: 28px;
    overflow: hidden;
    border: 1px solid var(--pb-line);
    border-radius: 3px;
  }
  .segmented button {
    padding: 0 10px;
    border: 0;
    background: transparent;
    color: var(--pb-text-quiet);
    cursor: pointer;
  }
  .segmented button + button {
    border-left: 1px solid var(--pb-line);
  }
  .segmented button[aria-pressed='true'] {
    background: color-mix(in srgb, var(--pb-accent) 18%, transparent);
    color: var(--pb-text);
    font-weight: 600;
  }
  .segmented button:disabled {
    cursor: default;
    opacity: 0.45;
  }
  .empty {
    grid-area: bench;
    z-index: 1;
    align-self: center;
    justify-self: center;
    width: 440px;
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 12px;
    color: var(--pb-mat-ink);
  }
  .empty p {
    margin: 0;
    font-size: 15px;
    line-height: 1.5;
  }

  /* Inspecteur */
  .inspector {
    grid-area: inspector;
    overflow: hidden auto;
    border-left: 1px solid var(--pb-line);
    background: var(--pb-panel);
  }
  .no-inspector .inspector {
    border-left: 0;
  }
  .insp-head {
    min-width: 280px;
    padding: 12px 12px 8px;
    border-bottom: 1px solid var(--pb-line);
  }
  .insp-head h2 {
    margin: 0;
    font-size: 15px;
    font-weight: 600;
  }
  .insp-head p {
    margin: 4px 0 0;
    color: var(--pb-text-quiet);
    font-size: 12px;
  }
  .insp-section {
    min-width: 280px;
    padding: 8px 12px 12px;
    border-bottom: 1px solid var(--pb-line);
  }
  .insp-section h3 {
    margin: 4px 0 8px;
    font-size: 13px;
    font-weight: 600;
  }

  /* Panneau du bas */
  .dock {
    grid-area: dock;
    display: grid;
    grid-template-rows: 32px minmax(0, 1fr);
    min-height: 0;
    border-top: 1px solid var(--pb-line);
    background: var(--pb-panel);
  }
  .dock-body {
    overflow: auto;
    padding: 6px 12px;
    font: 12px/20px var(--pb-font-data);
    font-variant-numeric: tabular-nums;
    user-select: text;
  }
  .line {
    white-space: pre-wrap;
  }
  .src {
    display: inline-block;
    min-width: 80px;
    color: var(--pb-text-quiet);
  }
  .empty-line {
    margin: 0;
    color: var(--pb-text-quiet);
    font-family: var(--pb-font-ui);
  }

  /* Barre d'état */
  .status {
    grid-area: status;
    display: flex;
    align-items: center;
    gap: 16px;
    min-width: 0;
    padding: 0 12px;
    border-top: 1px solid var(--pb-line);
    color: var(--pb-text-quiet);
    font-size: 11px;
  }
  .run {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    flex: none;
    color: var(--pb-text);
  }
  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--pb-low);
  }
  .running .dot {
    background: var(--pb-ok);
  }
  .building .dot,
  .starting .dot {
    background: transparent;
    border: 1.5px solid var(--pb-text-quiet);
  }
  .paused .dot,
  .breakpoint .dot {
    background: var(--pb-warn);
  }
  .debug-wait .dot {
    background: transparent;
    border: 1.5px solid var(--pb-accent);
  }
  .error .dot {
    background: var(--pb-conflict);
  }
  .message {
    min-width: 0;
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }
  .right {
    margin-left: auto;
    flex: none;
    display: flex;
    gap: 16px;
  }
  .build-failed {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    color: var(--pb-conflict);
  }
  .build-failed :global(.icon) {
    width: 12px;
    height: 12px;
  }
  /* Valeurs qui changent chaque seconde : largeur fixe, alignées à droite */
  .meter {
    display: inline-block;
    text-align: right;
  }
  .cpu {
    min-width: 5ch;
  }
  .ram {
    min-width: 6ch;
  }
</style>

// app/ui/lib/bench.svelte.ts — État de la paillasse côté interface.
//
// Les modèles tournent dans le cœur (QuickJS) : l'interface reçoit les états
// modifiés une fois par image (« sim:states ») et renvoie les actions des
// contrôles (component_control). Fenêtre masquée : la simulation continue,
// seul le rendu est suspendu (requestAnimationFrame, section 11.3).

import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type {
  BuildResultEvent,
  ComponentInfo,
  DebugView,
  Diagnostic,
  GuardEvent,
  InstanceInfo,
  LogEvent,
  Prerequisites,
  ProjectChangedEvent,
  ProjectView,
  SimState,
  StatesEvent,
  StatusEvent,
  TemplateInfo,
  UsageEvent,
  WarnEvent,
} from './types';
import { loadView, type ViewComponent } from './views';

/** Tampons bornés (règle 16.2.11). */
const MAX_LOG_LINES = 1000;
const MAX_WARNINGS = 200;

export interface LogLine {
  source: string;
  text: string;
}

export class Instance {
  readonly id: string;
  readonly type: string;
  readonly params: InstanceInfo['params'];
  readonly info: ComponentInfo | undefined;
  readonly pos: [number, number];
  readonly onboard: boolean;
  state = $state<unknown>(null);
  view = $state<ViewComponent | null>(null);
  viewError = $state('');

  constructor(spec: InstanceInfo, info: ComponentInfo | undefined, pos: [number, number]) {
    this.id = spec.id;
    this.type = spec.type;
    this.params = spec.params;
    this.info = info;
    this.pos = pos;
    this.onboard = spec.pos === null;
  }

  control(control: string, value: unknown): void {
    invoke('component_control', { component: this.id, control, value }).catch(() => {});
  }
}

export class Bench {
  project = $state<ProjectView | null>(null);
  instances = $state<Instance[]>([]);
  status = $state<{ state: SimState; message: string }>({ state: 'idle', message: '' });
  logs = $state<LogLine[]>([]);
  warnings = $state<string[]>([]);
  prerequisites = $state<Prerequisites | null>(null);
  /** Dernière mesure du firmware ; null hors simulation. */
  usage = $state<UsageEvent | null>(null);
  /** Build en cours, puis résultat du dernier build (onglet Build). */
  build = $state<{ running: boolean; result: BuildResultEvent | null }>({ running: false, result: null });
  /** Session de débogage en cours de préparation ou ouverte ; null sinon. */
  debug = $state<DebugView | null>(null);
  /** Dernière intervention d'un garde-fou, affichée en bannière. */
  guard = $state<GuardEvent | null>(null);
  private pending: StatesEvent = {};
  private frameRequested = false;

  async init(): Promise<void> {
    await listen<StatesEvent>('sim:states', (e) => this.onStates(e.payload));
    await listen<StatusEvent>('sim:status', (e) => {
      this.status = e.payload;
      const s = e.payload.state;
      if (s !== 'running' && s !== 'paused' && s !== 'breakpoint') this.usage = null;
      if (s === 'stopped' || s === 'error' || s === 'idle') this.debug = null;
      if (s === 'running' && this.guard?.kind === 'cpu') this.guard = null;
    });
    await listen<UsageEvent>('sim:usage', (e) => {
      this.usage = e.payload;
    });
    await listen<GuardEvent>('sim:guard', (e) => {
      this.guard = e.payload;
    });
    await listen<{ state: 'building' }>('build:state', () => {
      this.build.running = true;
    });
    await listen<BuildResultEvent>('build:result', (e) => {
      this.build = { running: false, result: e.payload };
    });
    await listen<ProjectChangedEvent>('project:changed', () => void this.refresh());
    await listen<LogEvent>('sim:log', (e) => this.log(e.payload.source, e.payload.text));
    await listen<WarnEvent>('sim:warn', (e) => this.warn(e.payload.text));
  }

  /** Vérifie CMake et le compilateur, et l'écrit dans le Journal. */
  async checkPrerequisites(): Promise<void> {
    try {
      const p = await invoke<Prerequisites>('check_prerequisites');
      this.prerequisites = p;
      for (const t of p.tools) {
        this.log('prebench', t.path === null ? `${t.label} introuvable` : `${t.label} : ${t.path}`);
      }
    } catch (e) {
      this.log('prebench', String(e));
    }
  }

  listTemplates(): Promise<TemplateInfo[]> {
    return invoke<TemplateInfo[]>('list_templates');
  }

  /** Copie un modèle dans `dest` ; renvoie le chemin du nouveau projet. */
  createProject(template: string, dest: string): Promise<string> {
    return invoke<string>('create_project', { template, dest });
  }

  /** Copie du blinky dans Documents/Prebench ; null : aucun projet encore. */
  defaultProject(): Promise<string | null> {
    return invoke<string | null>('default_project');
  }

  async open(path: string): Promise<void> {
    const project = await invoke<ProjectView>('open_project', { path });
    let onboardIndex = 0;
    const instances = project.instances.map((spec) => {
      // Composants intégrés : posés en rangée en haut de la carte (Board.svelte).
      const pos: [number, number] = spec.pos ?? [60 + onboardIndex++ * 112, 72];
      return new Instance(spec, project.components[spec.type], pos);
    });
    this.project = project;
    this.instances = instances;
    this.warnings = [];
    for (const w of project.warnings) this.warn(w);
    this.status = { state: 'idle', message: `${project.board_name} — ${project.framework}` };
    for (const inst of instances) void this.compileView(inst);
  }

  /**
   * Relit le projet après une modification dans l'éditeur (vue, modèle,
   * prebench.toml) : les vues sont recompilées, l'état des composants restants
   * est conservé jusqu'à la prochaine image du cœur.
   */
  async refresh(): Promise<void> {
    if (this.project === null) return;
    let project: ProjectView;
    try {
      project = await invoke<ProjectView>('open_project', { path: this.project.path });
    } catch (e) {
      this.warn(`Projet illisible après modification : ${String(e)}`);
      return;
    }
    const previous = new Map(this.instances.map((i) => [i.id, i]));
    let onboardIndex = 0;
    const instances = project.instances.map((spec) => {
      const pos: [number, number] = spec.pos ?? [60 + onboardIndex++ * 112, 72];
      const inst = new Instance(spec, project.components[spec.type], pos);
      const old = previous.get(spec.id);
      if (old !== undefined && old.type === spec.type) {
        inst.state = old.state;
        inst.view = old.view; // remplacée dès la nouvelle vue compilée
      }
      return inst;
    });
    this.project = project;
    this.instances = instances;
    for (const w of project.warnings) this.warn(w);
    for (const inst of instances) void this.compileView(inst);
  }

  /** Ouvre l'emplacement d'une erreur dans l'éditeur de l'utilisateur. */
  async openInEditor(d: Diagnostic): Promise<void> {
    if (d.file === null) return;
    try {
      await invoke('open_in_editor', { file: d.file, line: d.line, column: d.column });
    } catch (e) {
      this.log('prebench', String(e));
    }
  }

  private async compileView(inst: Instance): Promise<void> {
    const source = inst.info?.view_source;
    if (source === null || source === undefined) {
      inst.viewError = 'view.svelte absente';
      return;
    }
    try {
      inst.view = await loadView(inst.type, source);
      inst.viewError = '';
    } catch (e) {
      inst.view = null;
      inst.viewError = String(e);
      this.warn(`Vue du composant ${inst.type} inutilisable, vue simplifiée affichée : ${String(e)}`);
    }
  }

  async start(): Promise<void> {
    if (this.project === null) return;
    this.logs = [];
    this.guard = null;
    try {
      await invoke('start_simulation', { path: this.project.path });
    } catch (e) {
      this.status = { state: 'error', message: String(e) };
      this.log('prebench', String(e));
    }
  }

  /** Prépare une session de débogage : build, launch.json, attente de VS Code. */
  async startDebug(): Promise<void> {
    if (this.project === null) return;
    this.logs = [];
    this.guard = null;
    try {
      this.debug = await invoke<DebugView>('start_debug', { path: this.project.path });
    } catch (e) {
      this.debug = null;
      this.status = { state: 'error', message: String(e) };
      this.log('prebench', String(e));
    }
  }

  async stop(): Promise<void> {
    await invoke('stop_simulation').catch((e: unknown) => this.log('prebench', String(e)));
  }

  async pause(): Promise<void> {
    await invoke('pause_simulation').catch((e: unknown) => this.log('prebench', String(e)));
  }

  async resume(): Promise<void> {
    await invoke('resume_simulation').catch((e: unknown) => this.log('prebench', String(e)));
  }

  async reset(): Promise<void> {
    await invoke('reset_simulation').catch((e: unknown) => this.log('prebench', String(e)));
  }

  private onStates(states: StatesEvent): void {
    Object.assign(this.pending, states);
    if (this.frameRequested) return;
    this.frameRequested = true;
    requestAnimationFrame(() => {
      this.frameRequested = false;
      const pending = this.pending;
      this.pending = {};
      for (const inst of this.instances) {
        if (inst.id in pending) inst.state = pending[inst.id];
      }
    });
  }

  private log(source: string, text: string): void {
    this.logs.push({ source, text });
    if (this.logs.length > MAX_LOG_LINES) this.logs.splice(0, this.logs.length - MAX_LOG_LINES);
  }

  private warn(text: string): void {
    if (this.warnings.includes(text)) return;
    this.warnings.push(text);
    if (this.warnings.length > MAX_WARNINGS) this.warnings.splice(0, this.warnings.length - MAX_WARNINGS);
  }
}

export const bench = new Bench();

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
import { BOARD_ORIGIN } from './geometry';
import { serial } from './serial.svelte';
import { settings } from './settings.svelte';
import type { Check, Level, Note, PinConfig, PlacedComponent, SaveView, Wire } from './types';

/** États d'une session de simulation : le cœur envoie états, nets et broches. */
const SESSION: SimState[] = ['starting', 'running', 'paused', 'breakpoint', 'off'];
import { loadView, type ViewComponent } from './views';

/** Tampons bornés (règle 16.2.11). */
const MAX_LOG_LINES = 1000;
const MAX_WARNINGS = 200;
const MAX_HISTORY = 100;

/** Élément sélectionné sur la paillasse (inspecteur). */
export type Selection =
  | { kind: 'pin'; endpoint: string }
  | { kind: 'wire'; index: number }
  | { kind: 'component'; id: string }
  | { kind: 'note'; index: number };

/** Paillasse éditable : ce qui est enregistré dans prebench.toml. */
interface Snapshot {
  components: PlacedComponent[];
  wires: Wire[];
  notes: Note[];
}

export interface LogLine {
  source: string;
  text: string;
}

export class Instance {
  readonly id: string;
  readonly type: string;
  /** Paramètres (prebench.toml) ; modifiables dans l'inspecteur en mode Édition. */
  params = $state<InstanceInfo['params']>({});
  readonly info: ComponentInfo | undefined;
  /** Coin haut-gauche, coordonnées monde ; modifiable en mode Édition. */
  pos = $state<[number, number]>([0, 0]);
  /** Nom affiché ; vide : l'identifiant. */
  name = $state('');
  readonly onboard: boolean;
  state = $state<unknown>(null);
  view = $state<ViewComponent | null>(null);
  viewError = $state('');

  constructor(spec: InstanceInfo, info: ComponentInfo | undefined, pos: [number, number]) {
    this.id = spec.id;
    this.type = spec.type;
    this.params = spec.params;
    this.info = info;
    this.pos = [pos[0], pos[1]];
    this.name = spec.name ?? '';
    this.onboard = spec.pos === null;
  }

  control(control: string, value: unknown): void {
    invoke('component_control', { component: this.id, control, value }).catch(() => {});
  }
}

/** Position des composants intégrés : [layout] de la carte, sinon en rangée. */
function placeInstances(project: ProjectView, previous?: Map<string, Instance>): Instance[] {
  let onboardIndex = 0;
  return project.instances.map((spec) => {
    const fixed = project.layout?.onboard[spec.id];
    const pos: [number, number] =
      spec.pos ??
      (fixed
        ? [BOARD_ORIGIN[0] + fixed[0], BOARD_ORIGIN[1] + fixed[1]]
        : [BOARD_ORIGIN[0] + 60 + onboardIndex++ * 112, BOARD_ORIGIN[1] + 72]);
    const inst = new Instance(spec, project.components[spec.type], pos);
    const old = previous?.get(spec.id);
    if (old !== undefined && old.type === spec.type) {
      inst.state = old.state;
      inst.view = old.view; // remplacée dès la nouvelle vue compilée
    }
    return inst;
  });
}

export class Bench {
  project = $state<ProjectView | null>(null);
  instances = $state<Instance[]>([]);
  /** Fils de la paillasse (prebench.toml). */
  wires = $state<Wire[]>([]);
  /** Notes posées sur le tapis (prebench.toml). */
  notes = $state<Note[]>([]);
  /** Édition : placer et câbler ; Simulation : interagir (section 10.4). */
  mode = $state<'edit' | 'simulate'>('simulate');
  selection = $state<Selection | null>(null);
  /** Composants placés pendant la simulation : actifs au prochain démarrage. */
  pending = $state<string[]>([]);
  /** Niveaux des broches utilisées pendant la simulation (« sim:nets »). */
  nets = $state<Record<string, Level>>({});
  /** Configuration des broches par le firmware (« sim:pins ») : inspecteur. */
  pins = $state<Record<string, PinConfig>>({});
  /** Validations du câblage (section 9.2), recalculées à chaque enregistrement. */
  checks = $state<Check[]>([]);
  canUndo = $state(false);
  canRedo = $state(false);
  private undoStack: Snapshot[] = [];
  private redoStack: Snapshot[] = [];
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
  private frameStates: StatesEvent = {};
  private frameRequested = false;

  async init(): Promise<void> {
    this.watchBattery();
    await listen<StatesEvent>('sim:states', (e) => this.onStates(e.payload));
    await listen<StatusEvent>('sim:status', (e) => {
      const previous = this.status.state;
      this.status = e.payload;
      if (e.payload.state === 'building' || e.payload.state === 'debug-wait') {
        this.pending = [];
        serial.reset(); // nouvelle simulation : tampons série vidés
      }
      // Coupure et remise sous tension : repères dans le moniteur série (le
      // Journal reçoit les siens du cœur).
      if (e.payload.state === 'off' && previous !== 'off') serial.marker('Alimentation coupée');
      if (previous === 'off' && e.payload.state === 'starting') serial.marker('Alimentation rétablie');
      if (!SESSION.includes(e.payload.state)) {
        this.nets = {};
        this.pins = {};
      }
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
    await listen<Record<string, PinConfig>>('sim:pins', (e) => {
      this.pins = e.payload;
    });
    await listen<Record<string, Level>>('sim:nets', (e) => {
      this.nets = { ...this.nets, ...e.payload };
    });
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
    const instances = placeInstances(project);
    this.project = project;
    this.instances = instances;
    this.wires = project.wires;
    this.notes = project.notes ?? [];
    this.checks = project.checks ?? [];
    this.selection = null;
    this.pending = [];
    this.undoStack = [];
    this.redoStack = [];
    this.updateHistoryFlags();
    this.warnings = [];
    for (const w of project.warnings) this.warn(w);
    this.status = { state: 'idle', message: `${project.board_name} — ${project.framework}` };
    for (const inst of instances) void this.compileView(inst);
    await this.loadPreview();
  }

  /** Simulation en cours (états envoyés par le cœur), alimentation coupée comprise. */
  get live(): boolean {
    return SESSION.includes(this.status.state);
  }

  /** Session de débogage : reset et coupure appartiennent au débogueur. */
  get debugging(): boolean {
    return this.debug !== null || this.status.state === 'debug-wait' || this.status.state === 'breakpoint';
  }

  /** États initiaux hors simulation (modèles créés puis libérés par le cœur). */
  async loadPreview(): Promise<void> {
    if (this.project === null || this.live) return;
    try {
      const states = await invoke<Record<string, unknown>>('preview_states', { path: this.project.path });
      if (this.live) return;
      for (const inst of this.instances) {
        if (inst.id in states) inst.state = states[inst.id];
      }
    } catch {
      // Sans état initial, la vue reste en attente : sans conséquence.
    }
  }

  /* --- Édition de la paillasse (section 10.4) ---------------------------------------- */

  private snapshot(): Snapshot {
    return {
      components: this.instances
        .filter((i) => !i.onboard)
        .map((i) => ({
          id: i.id,
          type: i.type,
          ...(i.name.trim() ? { name: i.name.trim() } : {}),
          params: { ...i.params },
          pos: [i.pos[0], i.pos[1]] as [number, number],
        })),
      wires: this.wires.map((w) => ({ ...w })),
      notes: this.notes.map((n) => ({ text: n.text, pos: [n.pos[0], n.pos[1]] as [number, number] })),
    };
  }

  private restore(snap: Snapshot): void {
    if (this.project === null) return;
    const byId = new Map(this.instances.map((i) => [i.id, i]));
    const onboard = this.instances.filter((i) => i.onboard);
    const placed = snap.components.map((c) => {
      const old = byId.get(c.id);
      if (old !== undefined && old.type === c.type) {
        old.pos = [c.pos[0], c.pos[1]];
        old.name = c.name ?? '';
        old.params = { ...c.params };
        return old;
      }
      const inst = new Instance({ id: c.id, type: c.type, params: c.params, pos: c.pos }, this.project?.components[c.type], c.pos);
      void this.compileView(inst);
      return inst;
    });
    this.instances = [...onboard, ...placed];
    this.wires = snap.wires;
    this.notes = snap.notes;
    if (this.selection?.kind === 'note' && this.selection.index >= this.notes.length) this.selection = null;
    if (this.selection?.kind === 'wire' && this.selection.index >= this.wires.length) this.selection = null;
    if (this.selection?.kind === 'component' && !this.instances.some((i) => i.id === (this.selection as { id: string }).id)) {
      this.selection = null;
    }
  }

  private updateHistoryFlags(): void {
    this.canUndo = this.undoStack.length > 0;
    this.canRedo = this.redoStack.length > 0;
  }

  /** Enregistre un changement : `before` rejoint l'historique, puis prebench.toml est écrit. */
  async commit(before: Snapshot): Promise<void> {
    this.undoStack.push(before);
    if (this.undoStack.length > MAX_HISTORY) this.undoStack.shift();
    this.redoStack = [];
    this.updateHistoryFlags();
    await this.save();
  }

  /** État courant, à passer à commit() après une modification. */
  begin(): Snapshot {
    return this.snapshot();
  }

  async undo(): Promise<void> {
    const prev = this.undoStack.pop();
    if (prev === undefined) return;
    this.redoStack.push(this.snapshot());
    this.restore(prev);
    this.updateHistoryFlags();
    await this.save();
  }

  async redo(): Promise<void> {
    const next = this.redoStack.pop();
    if (next === undefined) return;
    this.undoStack.push(this.snapshot());
    this.restore(next);
    this.updateHistoryFlags();
    await this.save();
  }

  private async save(): Promise<void> {
    if (this.project === null) return;
    const snap = this.snapshot();
    try {
      const res = await invoke<SaveView>('save_bench', {
        path: this.project.path,
        components: snap.components,
        wires: snap.wires,
        notes: snap.notes,
      });
      this.pending = res.pending;
      this.checks = res.checks ?? [];
    } catch (e) {
      this.warn(`Paillasse non enregistrée : ${String(e)}`);
    }
    if (!this.live && this.instances.some((i) => i.state === null)) await this.loadPreview();
  }

  /** Ajoute un composant du catalogue en `pos` (coordonnées monde). */
  async addComponent(type: string, id: string, pos: [number, number]): Promise<void> {
    const info = this.project?.components[type];
    if (info === undefined) return;
    const before = this.snapshot();
    const params: Record<string, unknown> = {};
    for (const [name, p] of Object.entries(info.manifest.params ?? {})) {
      if (p.default !== undefined) params[name] = p.default;
    }
    const inst = new Instance({ id, type, params: params as InstanceInfo['params'], pos }, info, pos);
    this.instances = [...this.instances, inst];
    this.selection = { kind: 'component', id };
    void this.compileView(inst);
    await this.commit(before);
  }

  async addWire(from: string, to: string): Promise<void> {
    if (from === to || this.wires.some((w) => (w.from === from && w.to === to) || (w.from === to && w.to === from))) {
      return;
    }
    const before = this.snapshot();
    this.wires = [...this.wires, { from, to }];
    this.selection = { kind: 'wire', index: this.wires.length - 1 };
    await this.commit(before);
  }

  async setWireColor(index: number, color: Wire['color']): Promise<void> {
    const wire = this.wires[index];
    if (wire === undefined || wire.color === color) return;
    const before = this.snapshot();
    const next = { from: wire.from, to: wire.to, ...(color ? { color } : {}) };
    this.wires = this.wires.map((w, i) => (i === index ? next : w));
    await this.commit(before);
  }

  /** Renomme un composant (nom affiché) ; vide : l'identifiant s'affiche. */
  async renameComponent(id: string, name: string): Promise<void> {
    const inst = this.instances.find((i) => i.id === id);
    const next = name.trim().slice(0, 40);
    if (inst === undefined || inst.name === next) return;
    const before = this.snapshot();
    inst.name = next;
    await this.commit(before);
  }

  /** Modifie un paramètre d'un composant placé ; transmis au modèle en cours (onParamsChange). */
  async setParam(id: string, key: string, value: string | number | boolean): Promise<void> {
    const inst = this.instances.find((i) => i.id === id);
    if (inst === undefined || inst.onboard || inst.params[key] === value) return;
    const before = this.snapshot();
    inst.params = { ...inst.params, [key]: value };
    await this.commit(before);
  }

  /** Pose une note en `pos` (monde) ; renvoie son index. */
  async addNote(pos: [number, number], text = 'Note'): Promise<number> {
    const before = this.snapshot();
    this.notes = [...this.notes, { text, pos }];
    const index = this.notes.length - 1;
    this.selection = { kind: 'note', index };
    await this.commit(before);
    return index;
  }

  /** Modifie le texte d'une note ; vide : la note est supprimée. */
  async setNoteText(index: number, text: string): Promise<void> {
    const note = this.notes[index];
    const next = text.trim().slice(0, 200);
    if (note === undefined || note.text === next) return;
    const before = this.snapshot();
    if (next === '') {
      this.notes = this.notes.filter((_, i) => i !== index);
      this.selection = null;
    } else {
      this.notes = this.notes.map((n, i) => (i === index ? { text: next, pos: n.pos } : n));
    }
    await this.commit(before);
  }

  /** Supprime la sélection : fil, ou composant placé et ses fils (pas un composant intégré). */
  async deleteSelection(): Promise<void> {
    const sel = this.selection;
    if (sel === null) return;
    const before = this.snapshot();
    if (sel.kind === 'wire') {
      this.wires = this.wires.filter((_, i) => i !== sel.index);
    } else if (sel.kind === 'note') {
      this.notes = this.notes.filter((_, i) => i !== sel.index);
    } else if (sel.kind === 'component') {
      const inst = this.instances.find((i) => i.id === sel.id);
      if (inst === undefined || inst.onboard) return;
      this.instances = this.instances.filter((i) => i.id !== sel.id);
      const prefix = `${sel.id}.`;
      this.wires = this.wires.filter((w) => !w.from.startsWith(prefix) && !w.to.startsWith(prefix));
    } else {
      return;
    }
    this.selection = null;
    await this.commit(before);
  }

  /** Arrête puis relance la simulation (composants en attente d'activation). */
  async restart(): Promise<void> {
    await this.stop();
    await this.start();
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
    const instances = placeInstances(project, new Map(this.instances.map((i) => [i.id, i])));
    this.project = project;
    this.instances = instances;
    this.wires = project.wires;
    this.notes = project.notes ?? [];
    this.checks = project.checks ?? [];
    for (const w of project.warnings) this.warn(w);
    for (const inst of instances) void this.compileView(inst);
    await this.loadPreview();
  }

  /** Export série : fenêtre « Enregistrer sous » ouverte par le cœur. */
  async exportSerial(name: string, text: string): Promise<void> {
    try {
      const path = await invoke<string | null>('export_serial', { name, text });
      if (path !== null) this.log('prebench', `sortie série exportée : ${path}`);
    } catch (e) {
      this.log('prebench', `export impossible : ${String(e)}`);
    }
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

  /** Reset : le firmware redémarre, la paillasse garde son état. Hors historique d'édition. */
  async reset(): Promise<void> {
    await invoke('reset_simulation').catch((e: unknown) => this.log('prebench', String(e)));
  }

  /** Interrupteur d'alimentation (section 10.4). Hors historique d'édition. */
  async setPower(on: boolean): Promise<void> {
    await invoke('set_power', { on }).catch((e: unknown) => this.log('prebench', String(e)));
  }

  /** Sur batterie : 30 images/s au lieu de 60 (section 11.3). */
  private onBattery = false;
  private lastFrame = 0;

  /** API Batterie : Chromium (WebView2 sous Windows) ; absente de WebKit (macOS). */
  private watchBattery(): void {
    const nav = navigator as Navigator & {
      getBattery?: () => Promise<{ charging: boolean; addEventListener(t: string, f: () => void): void }>;
    };
    nav
      .getBattery?.()
      .then((b) => {
        const update = (): void => {
          this.onBattery = !b.charging;
        };
        update();
        b.addEventListener('chargingchange', update);
      })
      .catch(() => {});
  }

  /**
   * États reçus du cœur, appliqués une fois par image (requestAnimationFrame,
   * section 11.3) ; fenêtre masquée : aucune image, la simulation continue.
   */
  private onStates(states: StatesEvent): void {
    Object.assign(this.frameStates, states);
    if (this.frameRequested) return;
    this.frameRequested = true;
    const frame = (t: number): void => {
      const battery = 1000 / settings.number('frame_rate_battery', 30) - 1;
      if (this.onBattery && t - this.lastFrame < battery) {
        requestAnimationFrame(frame); // cadence réduite sur batterie (réglage)
        return;
      }
      this.lastFrame = t;
      this.frameRequested = false;
      const pending = this.frameStates;
      this.frameStates = {};
      for (const inst of this.instances) {
        if (inst.id in pending) inst.state = pending[inst.id];
      }
    };
    requestAnimationFrame(frame);
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

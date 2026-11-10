// app/ui/lib/bench.svelte.ts — État de la paillasse côté interface.
//
// Les modèles tournent dans le cœur (QuickJS) : l'interface reçoit les états
// modifiés une fois par image (« sim:states ») et renvoie les actions des
// contrôles (component_control). Fenêtre masquée : la simulation continue,
// seul le rendu est suspendu (requestAnimationFrame, section 11.3).

import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type {
  ComponentInfo,
  InstanceInfo,
  LogEvent,
  ProjectView,
  SimState,
  StatesEvent,
  StatusEvent,
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
  private pending: StatesEvent = {};
  private frameRequested = false;

  async init(): Promise<void> {
    await listen<StatesEvent>('sim:states', (e) => this.onStates(e.payload));
    await listen<StatusEvent>('sim:status', (e) => {
      this.status = e.payload;
    });
    await listen<LogEvent>('sim:log', (e) => this.log(e.payload.source, e.payload.text));
    await listen<WarnEvent>('sim:warn', (e) => this.warn(e.payload.text));
  }

  defaultProject(): Promise<string> {
    return invoke<string>('default_project');
  }

  async open(path: string): Promise<void> {
    const project = await invoke<ProjectView>('open_project', { path });
    let onboardIndex = 0;
    const instances = project.instances.map((spec) => {
      const pos: [number, number] = spec.pos ?? [90 + onboardIndex++ * 110, 130];
      return new Instance(spec, project.components[spec.type], pos);
    });
    this.project = project;
    this.instances = instances;
    this.warnings = [];
    for (const w of project.warnings) this.warn(w);
    this.status = { state: 'idle', message: `${project.board_name} — ${project.framework}` };
    for (const inst of instances) void this.compileView(inst);
  }

  private async compileView(inst: Instance): Promise<void> {
    const source = inst.info?.view_source;
    if (source === null || source === undefined) {
      inst.viewError = 'view.svelte absente';
      return;
    }
    try {
      inst.view = await loadView(inst.type, source);
    } catch (e) {
      inst.viewError = String(e);
      this.warn(`vue du composant ${inst.type} : ${String(e)} — vue de repli utilisée`);
    }
  }

  async start(): Promise<void> {
    if (this.project === null) return;
    this.logs = [];
    try {
      await invoke('start_simulation', { path: this.project.path });
    } catch (e) {
      this.status = { state: 'error', message: String(e) };
      this.log('prebench', String(e));
    }
  }

  async stop(): Promise<void> {
    await invoke('stop_simulation').catch((e: unknown) => this.log('prebench', String(e)));
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

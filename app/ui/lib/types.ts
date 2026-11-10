// app/ui/lib/types.ts — Types partagés de l'interface : données du cœur.
//
// Les modèles de composants s'exécutent dans le cœur Rust (QuickJS) ; l'interface
// n'affiche que les vues, à partir de l'état envoyé une fois par image.

export type ParamMap = Record<string, unknown>;

/** Manifeste d'un composant (components/manifest.schema.json, sous-ensemble utile). */
export interface Manifest {
  api_version: 1;
  id: string;
  name: string;
  view: { width: number; height: number };
  pins: { name: string; label?: string; role: string; x: number; y: number }[];
  controls?: { id: string; type: string; label: string }[];
}

/** Composant du catalogue (projet prioritaire sur les composants fournis). */
export interface ComponentInfo {
  id: string;
  origin: 'project' | 'provided';
  manifest: Manifest;
  /** Source de view.svelte, compilée à l'exécution ; null : vue de repli. */
  view_source: string | null;
}

export interface InstanceInfo {
  id: string;
  type: string;
  params: ParamMap;
  /** Position sur la paillasse ; null pour un composant intégré à la carte. */
  pos: [number, number] | null;
}

/** Projet ouvert (commande open_project). */
export interface ProjectView {
  path: string;
  framework: string;
  board_name: string;
  mcu: string;
  instances: InstanceInfo[];
  components: Record<string, ComponentInfo>;
  warnings: string[];
}

/* --- Événements du cœur --- */

/** États des composants modifiés depuis l'image précédente. */
export type StatesEvent = Record<string, unknown>;

export type SimState = 'idle' | 'building' | 'starting' | 'running' | 'stopped' | 'error';

export interface StatusEvent {
  state: SimState;
  message: string;
}

export interface LogEvent {
  source: 'firmware' | 'stdout' | 'stderr' | 'composant';
  level: number;
  text: string;
}

export interface WarnEvent {
  code: number;
  text: string;
}

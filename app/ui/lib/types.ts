// app/ui/lib/types.ts — Types partagés de l'interface : données du cœur.
//
// Les modèles de composants s'exécutent dans le cœur Rust (QuickJS) ; l'interface
// n'affiche que les vues, à partir de l'état envoyé une fois par image.

export type ParamMap = Record<string, unknown>;

/** Rôle d'une broche, vu du composant (manifest.schema.json). */
export type PinRole =
  | 'input'
  | 'output'
  | 'io'
  | 'open-drain'
  | 'analog-in'
  | 'analog-out'
  | 'power'
  | 'ground'
  | 'passive';

/** Manifeste d'un composant (components/manifest.schema.json, sous-ensemble utile). */
export interface Manifest {
  api_version: 1;
  id: string;
  name: string;
  description?: string;
  category?: string;
  view: { width: number; height: number };
  pins: { name: string; label?: string; role: PinRole; x: number; y: number }[];
  params?: Record<string, { type: string; label?: string; values?: string[]; default?: unknown }>;
  controls?: { id: string; type: string; label: string }[];
}

/** Fil de prebench.toml ; extrémité « board.CN5:6 » ou « led1.A ». */
export interface Wire {
  from: string;
  to: string;
  color?: WireColor;
}

export type WireColor = 'red' | 'black' | 'yellow' | 'green' | 'blue' | 'orange' | 'purple';

/** Composant placé, tel qu'enregistré dans prebench.toml. */
export interface PlacedComponent {
  id: string;
  type: string;
  /** Nom affiché (« LED rouge ») ; l'identifiant reste celui des fils. */
  name?: string;
  params: ParamMap;
  pos: [number, number];
}

/** Note libre posée sur le tapis (prebench.toml, [[notes]]). */
export interface Note {
  text: string;
  pos: [number, number];
}

/** Longueurs maximales (docs/prebench.schema.json). */
export const MAX_NAME_LEN = 40;
export const MAX_NOTE_LEN = 200;

/** Connecteur de la carte (boards/board.schema.json, layout.headers). */
export interface Header {
  id: string;
  pos: [number, number];
  dir: 'up' | 'down' | 'left' | 'right';
  pitch: number;
  label_side: 'left' | 'right' | 'above' | 'below';
  pins: string[];
}

export interface Layout {
  size: [number, number];
  holes: [number, number][];
  chip: { pos: [number, number]; size: [number, number] } | null;
  onboard: Record<string, [number, number]>;
  unconnected: string[];
  headers: Header[];
}

export interface PinDef {
  label: string | null;
  caps: string[];
  external_pull: string | null;
}

/** État d'un net (section 9.1) ; mêmes noms que NetLevel de component-api.d.ts. */
export type Level = 'high' | 'low' | 'pulled-high' | 'pulled-low' | 'floating' | 'conflict';

/** Problème de câblage (section 9.2), calculé par le cœur sans simulation. */
export interface Check {
  severity: 'warn' | 'conflict';
  text: string;
  /** Extrémités concernées : sélection au clic. */
  endpoints: string[];
}

/** Réponse de save_bench. */
export interface SaveView {
  /** Composants placés pendant la simulation : actifs au prochain démarrage. */
  pending: string[];
  checks: Check[];
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
  /** Nom affiché ; absent : l'identifiant. */
  name?: string | null;
}

/** Projet ouvert (commande open_project). */
export interface ProjectView {
  path: string;
  framework: string;
  board_name: string;
  mcu: string;
  instances: InstanceInfo[];
  /** Catalogue complet : menu « Ajouter un composant ». */
  components: Record<string, ComponentInfo>;
  warnings: string[];
  wires: Wire[];
  notes: Note[];
  layout: Layout | null;
  pins: Record<string, PinDef>;
  power: Record<string, number>;
  checks: Check[];
}

/* --- Événements du cœur --- */

/** États des composants modifiés depuis l'image précédente. */
export type StatesEvent = Record<string, unknown>;

export type SimState =
  | 'idle'
  | 'building'
  | 'starting'
  | 'running'
  | 'paused'
  | 'stopped'
  | 'error'
  /** Débogage : en attente du firmware lancé par VS Code. */
  | 'debug-wait'
  /** Débogage : firmware figé par le débogueur. */
  | 'breakpoint';

export interface StatusEvent {
  state: SimState;
  message: string;
}

export interface LogEvent {
  source: 'firmware' | 'stdout' | 'stderr' | 'composant' | 'prebench';
  level: number;
  text: string;
}

/** Mesure du firmware à 1 Hz, pendant une simulation (section 11.2). */
export interface UsageEvent {
  /** Pourcentage d'un cœur. */
  cpu: number;
  /** Octets. */
  memory: number;
  memory_limit: number;
}

/** Erreur ou avertissement d'un build « sim » (core : build/diagnostics.rs). */
export interface Diagnostic {
  severity: 'error' | 'warning';
  /** Chemin absolu, transmis à open_in_editor ; null : sans emplacement (éditeur de liens). */
  file: string | null;
  /** Chemin affiché, relatif au projet quand c'est possible. */
  display: string | null;
  line: number | null;
  column: number | null;
  message: string;
}

/** Résultat d'un build (démarrage ou sauvegarde). */
export interface BuildResultEvent {
  ok: boolean;
  duration_ms: number;
  errors: number;
  warnings: number;
  diagnostics: Diagnostic[];
}

/** Fichiers du projet modifiés dans l'éditeur (surveillance du dossier). */
export interface ProjectChangedEvent {
  firmware: boolean;
  views: string[];
  models: string[];
  project: boolean;
}

/** Outil de débogage vérifié (extension VS Code ou débogueur). */
export interface DebugTool {
  label: string;
  present: boolean;
  path: string | null;
  install: string;
}

/** Session de débogage préparée (commande start_debug). */
export interface DebugView {
  config_name: string;
  launch_json: string;
  launch_json_state: 'created' | 'updated' | 'unchanged' | null;
  launch_json_error: string | null;
  tools: DebugTool[];
}

/** Intervention d'un garde-fou : pause automatique (cpu) ou arrêt (memory). */
export interface GuardEvent {
  kind: 'cpu' | 'memory';
  text: string;
}

export interface WarnEvent {
  code: number;
  text: string;
}

/* --- Prérequis de build (commande check_prerequisites) --- */

export interface Prerequisite {
  kind: 'cmake' | 'compiler' | 'make';
  label: string;
  /** Chemin trouvé (PATH, puis emplacements standard) ; null : absent. */
  path: string | null;
  /** Commande d'installation, affichée quand l'outil manque. */
  install: string;
}

export interface Prerequisites {
  tools: Prerequisite[];
  /** Renvoi vers la section du README sur les prérequis. */
  readme: string;
}

/** Modèle de nouveau projet (commande list_templates). */
export interface TemplateInfo {
  id: string;
  /** Dossier proposé pour la copie : Documents/Prebench/<id>, libre. */
  suggested_dir: string;
}

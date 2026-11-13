// app/ui/lib/geometry.ts — Géométrie de la paillasse : pastilles de la carte, broches des
// composants, tracé et couleur des fils, compatibilité pendant le câblage (section 10.4).
//
// Coordonnées « monde » : unités du canevas, 1 = 1 px au zoom 1. La carte est
// posée en BOARD_ORIGIN ; ses pastilles viennent de [layout] (fichier de carte).

import type { Header, Layout, Level, PinDef, PinRole, ProjectView, Wire, WireColor } from './types';

/** Coin haut-gauche de la carte sur le tapis. */
export const BOARD_ORIGIN: [number, number] = [48, 48];
/** Pas du quadrillage du tapis : aimantation des composants (charte 2.2). */
export const GRID = 24;
/** Carte sans [layout] : dessin générique. */
export const GENERIC_BOARD: [number, number] = [280, 400];

export type Point = [number, number];

/** Pastille de la carte. */
export interface Pad {
  /** « board.CN5:6 ». */
  endpoint: string;
  header: string;
  /** Numéro sur le connecteur, à partir de 1. */
  index: number;
  /** Signal (PA5, GND) ou nom non câblable (NC). */
  signal: string;
  /** Étiquette de sérigraphie (D13, GND, NC). */
  label: string;
  wireable: boolean;
  pos: Point;
  side: Header['label_side'];
}

const STEP: Record<Header['dir'], Point> = { up: [0, -1], down: [0, 1], left: [-1, 0], right: [1, 0] };

export function snap(v: number): number {
  return Math.round(v / GRID) * GRID;
}

/** Pastilles de la carte en coordonnées monde. */
export function boardPads(project: ProjectView): Pad[] {
  const layout: Layout | null = project.layout;
  if (layout === null) return [];
  const [ox, oy] = BOARD_ORIGIN;
  return layout.headers.flatMap((h) => {
    const [dx, dy] = STEP[h.dir];
    return h.pins.map((signal, i) => {
      const wireable = signal in project.pins || signal in project.power;
      return {
        endpoint: `board.${h.id}:${i + 1}`,
        header: h.id,
        index: i + 1,
        signal,
        label: project.pins[signal]?.label ?? signal,
        wireable,
        pos: [ox + h.pos[0] + dx * h.pitch * i, oy + h.pos[1] + dy * h.pitch * i] as Point,
        side: h.label_side,
      };
    });
  });
}

/** Signal de la carte désigné par une extrémité (« board.CN5:6 » ou « board.PA5 »). */
export function boardSignal(project: ProjectView, pads: Pad[], endpoint: string): string | null {
  if (!endpoint.startsWith('board.')) return null;
  const pad = pads.find((p) => p.endpoint === endpoint);
  if (pad) return pad.wireable ? pad.signal : null;
  const signal = endpoint.slice('board.'.length);
  return signal in project.pins || signal in project.power ? signal : null;
}

/** Position d'une extrémité : pastille, signal (première pastille), ou broche de composant. */
export function endpointPos(
  project: ProjectView,
  pads: Pad[],
  placed: { id: string; type: string; pos: Point }[],
  endpoint: string,
): Point | null {
  if (endpoint.startsWith('board.')) {
    const pad = pads.find((p) => p.endpoint === endpoint);
    if (pad) return pad.pos;
    const signal = endpoint.slice('board.'.length);
    return pads.find((p) => p.signal === signal)?.pos ?? null;
  }
  const dot = endpoint.indexOf('.');
  const id = endpoint.slice(0, dot);
  const pin = endpoint.slice(dot + 1);
  const comp = placed.find((c) => c.id === id);
  const p = comp && project.components[comp.type]?.manifest.pins.find((x) => x.name === pin);
  return comp && p ? [comp.pos[0] + p.x, comp.pos[1] + p.y] : null;
}

/** Rôle de la broche d'un composant placé, ou null pour la carte. */
export function pinRole(
  project: ProjectView,
  placed: { id: string; type: string }[],
  endpoint: string,
): PinRole | null {
  if (endpoint.startsWith('board.')) return null;
  const dot = endpoint.indexOf('.');
  const comp = placed.find((c) => c.id === endpoint.slice(0, dot));
  const pin = endpoint.slice(dot + 1);
  return (comp && project.components[comp.type]?.manifest.pins.find((x) => x.name === pin)?.role) ?? null;
}

/** Une broche de composant de ce rôle peut-elle être reliée à ce signal de la carte ? */
export function compatible(role: PinRole, signal: string, pins: Record<string, PinDef>, power: Record<string, number>): boolean {
  const volts = power[signal];
  const caps = pins[signal]?.caps ?? [];
  switch (role) {
    case 'ground':
      return volts === 0;
    case 'power':
      return volts !== undefined && volts > 0;
    case 'analog-out': // le composant fournit une tension : entrée ADC
      return caps.some((c) => c.startsWith('adc'));
    case 'analog-in': // le composant lit une tension : PWM ou DAC
      return caps.some((c) => c.startsWith('pwm') || c.startsWith('dac'));
    default: // numérique ou passif : broche GPIO, alimentation ou masse
      return volts !== undefined || caps.includes('gpio');
  }
}

/** Couleurs des fils Dupont (charte, section 2.4) : matériaux, identiques dans les deux thèmes. */
export const WIRE_HEX: Record<WireColor, string> = {
  red: '#D9453B',
  black: '#2A2A2A',
  yellow: '#E8C33A',
  green: '#3E9B5C',
  blue: '#3D7FD1',
  orange: '#E07B39',
  purple: '#8A5CC7',
};

export const WIRE_LABEL: Record<WireColor, string> = {
  red: 'Rouge',
  black: 'Noir',
  yellow: 'Jaune',
  green: 'Vert',
  blue: 'Bleu',
  orange: 'Orange',
  purple: 'Violet',
};

const BUS_PIN = /^(SDA|SCL|SCK|MOSI|MISO|CS|NSS|TX|RX|DIN|DOUT)$/i;

/** Couleur par défaut selon la fonction (charte 2.4) : alimentation, masse, analogique, bus, signal. */
export function defaultColor(project: ProjectView, pads: Pad[], placed: { id: string; type: string }[], wire: Wire): WireColor {
  const ends = [wire.from, wire.to];
  for (const end of ends) {
    const signal = boardSignal(project, pads, end);
    const volts = signal === null ? undefined : project.power[signal];
    if (volts !== undefined) return volts > 0 ? 'red' : 'black';
  }
  const roles = ends.map((e) => pinRole(project, placed, e));
  if (roles.includes('ground')) return 'black';
  if (roles.includes('power')) return 'red';
  if (roles.includes('analog-in') || roles.includes('analog-out')) return 'green';
  const busPin = ends.some((e) => !e.startsWith('board.') && BUS_PIN.test(e.slice(e.indexOf('.') + 1)));
  const busSignal = ends.some((e) => {
    const s = boardSignal(project, pads, e);
    return s !== null && (project.pins[s]?.caps ?? []).some((c) => /^(i2c|spi|uart)/.test(c));
  });
  return busPin && busSignal ? 'blue' : 'yellow';
}

/** Tracé d'un fil Dupont : courbe qui retombe légèrement sous la corde (charte 2.4, 6). */
export function wirePath(a: Point, b: Point): string {
  const dist = Math.hypot(b[0] - a[0], b[1] - a[1]);
  const sag = Math.min(48, 12 + dist * 0.15);
  const dx = (b[0] - a[0]) * 0.3;
  return `M ${a[0]} ${a[1]} C ${a[0] + dx} ${a[1] + sag}, ${b[0] - dx} ${b[1] + sag}, ${b[0]} ${b[1]}`;
}

/** Identifiant libre pour un nouveau composant : « led1 », « led2 »… */
export function freeId(type: string, used: Set<string>): string {
  const base = type.toLowerCase().replace(/[^a-z0-9_]/g, '_').replace(/^[^a-z]+/, '') || 'c';
  for (let n = 1; ; n++) {
    const id = `${base.slice(0, 28)}${n}`;
    if (!used.has(id)) return id;
  }
}

/** Mot qui double la couleur d'un état de net (charte 2.3). */
export const LEVEL_WORD: Record<Level, string> = {
  high: 'Haut',
  low: 'Bas',
  'pulled-high': 'Tiré haut',
  'pulled-low': 'Tiré bas',
  floating: 'Flottant',
  conflict: 'Conflit',
};

/** Couleur de pastille d'état (StatePill) d'un état de net. */
export const LEVEL_TONE: Record<Level, 'high' | 'low' | 'neutral' | 'conflict'> = {
  high: 'high',
  low: 'low',
  'pulled-high': 'high',
  'pulled-low': 'low',
  floating: 'neutral',
  conflict: 'conflict',
};

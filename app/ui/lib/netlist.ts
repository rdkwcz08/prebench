// app/ui/lib/netlist.ts — Nets de la paillasse pour l'inspecteur (section 9.1).
//
// Mêmes liaisons que le réseau du cœur (core/net.rs), sans les niveaux :
// fils du projet, liaisons des composants intégrés (LD2.A -> PA5), et toutes
// les pastilles d'un même signal (deux pastilles GND sont un seul net).
// Les niveaux viennent de « sim:nets », par signal de la carte.

import { boardSignal, type Pad } from './geometry';
import type { Level, ProjectView, Wire } from './types';

/** Clé d'une extrémité dans un net : « sig:PA5 » pour la carte, « led1.A » sinon. */
export function netKey(project: ProjectView, pads: Pad[], endpoint: string): string | null {
  if (endpoint.startsWith('board.')) {
    const signal = boardSignal(project, pads, endpoint);
    return signal === null ? null : `sig:${signal}`;
  }
  return endpoint;
}

export class Netlist {
  private parent = new Map<string, string>();

  constructor(project: ProjectView, pads: Pad[], wires: Wire[]) {
    for (const [id, pins] of Object.entries(project.onboard_wires ?? {})) {
      for (const [pin, signal] of Object.entries(pins)) this.union(`${id}.${pin}`, `sig:${signal}`);
    }
    for (const w of wires) {
      const a = netKey(project, pads, w.from);
      const b = netKey(project, pads, w.to);
      if (a !== null && b !== null) this.union(a, b);
    }
  }

  private find(k: string): string {
    let cur = k;
    for (;;) {
      const p = this.parent.get(cur);
      if (p === undefined || p === cur) return cur;
      cur = p;
    }
  }

  private union(a: string, b: string): void {
    const ra = this.find(a);
    const rb = this.find(b);
    if (ra !== rb) this.parent.set(ra, rb);
    if (!this.parent.has(rb)) this.parent.set(rb, rb);
  }

  /** Membres du net de `key` (lui compris), dans un ordre stable. */
  members(key: string): string[] {
    const root = this.find(key);
    const all = new Set<string>([key]);
    for (const k of this.parent.keys()) if (this.find(k) === root) all.add(k);
    return [...all].sort((a, b) => Number(!a.startsWith('sig:')) - Number(!b.startsWith('sig:')) || a.localeCompare(b));
  }

  /** Niveau du net de `key`, d'après les signaux de la carte qu'il contient. */
  level(key: string, nets: Record<string, Level>): Level | undefined {
    for (const m of this.members(key)) {
      if (m.startsWith('sig:')) {
        const level = nets[m.slice(4)];
        if (level !== undefined) return level;
      }
    }
    return undefined;
  }
}

/** Libellé d'un membre : « PA5 (D13) », « 3V3 », « led1.A ». */
export function memberLabel(project: ProjectView, key: string): string {
  if (!key.startsWith('sig:')) return key;
  const signal = key.slice(4);
  const label = project.pins[signal]?.label;
  return label && label !== signal ? `${signal} (${label})` : signal;
}

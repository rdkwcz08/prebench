// app/ui/lib/serial.svelte.ts — Moniteur série et traceur (section 10.4, phase 2 étape 6).
//
// Un port par UART du firmware (« sim:uart », « sim:uart-config ») plus la
// sortie standard du firmware (printf de l'ordinateur hôte, lignes « stdout »).
// - Tampon borné à 1 Mo par port (section 11.2) : les plus anciens octets cèdent.
// - Débit du moniteur choisi par port ; différent du débit configuré par le
//   firmware : alerte (section 9.2), comme un terminal mal réglé.
// - Traceur : lignes « clé:valeur, clé2:valeur » (ou nombres séparés par des
//   espaces), 600 points par courbe au plus.

import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

/** Octets gardés par port (section 11.2). */
const MAX_BYTES = 1024 * 1024;
/** Points gardés par courbe du traceur. */
export const MAX_POINTS = 600;
/** Courbes du traceur au plus (couleurs de voies, charte 2.5, puis répétition). */
export const MAX_SERIES = 8;

export const BAUDS = [9600, 19200, 38400, 57600, 115200, 230400, 460800, 921600];
export const DEFAULT_BAUD = 115200;

export type LineEnding = 'lf' | 'crlf' | 'cr' | 'none';
export const LINE_ENDINGS: Record<LineEnding, { label: string; bytes: string }> = {
  lf: { label: 'LF', bytes: '\n' },
  crlf: { label: 'CR+LF', bytes: '\r\n' },
  cr: { label: 'CR', bytes: '\r' },
  none: { label: 'Aucune', bytes: '' },
};

/** Morceau reçu : instant (ms depuis le démarrage) et octets. */
export interface Chunk {
  t: number;
  data: Uint8Array;
  /** Repère de Prebench (« Alimentation coupée »), pas un octet du firmware. */
  sys?: boolean;
}

export interface Port {
  /** « 2 » pour USART2, « stdout » pour la sortie standard. */
  key: string;
  label: string;
  /** UART : numéro du périphérique ; null pour la sortie standard. */
  id: number | null;
  /** Débit configuré par le firmware ; null : inconnu ou sans objet. */
  firmwareBaud: number | null;
  chunks: Chunk[];
  bytes: number;
}

export interface Series {
  name: string;
  points: number[];
}

const decoder = new TextDecoder('utf-8', { fatal: false });
const encoder = new TextEncoder();

/** Nom d'un port : USART1, USART2, USART6 (les UART4/5 n'existent pas sur la F401). */
function uartLabel(id: number): string {
  return `USART${id}`;
}

class Serial {
  ports = $state<Port[]>([]);
  /** Port affiché. */
  current = $state<string>('');
  /** Débit du moniteur, par port. */
  monitorBaud = $state<Record<string, number>>({});
  timestamps = $state(false);
  hex = $state(false);
  ending = $state<LineEnding>('lf');
  /** Courbes du traceur, pour le port affiché. */
  series = $state<Series[]>([]);
  private started = performance.now();
  private partial: Record<string, string> = {};

  async init(): Promise<void> {
    await listen<{ id: number; baud: number }>('sim:uart-config', (e) => {
      const p = this.port(String(e.payload.id), uartLabel(e.payload.id), e.payload.id);
      p.firmwareBaud = e.payload.baud;
      if (this.monitorBaud[p.key] === undefined) {
        this.monitorBaud = { ...this.monitorBaud, [p.key]: DEFAULT_BAUD };
      }
    });
    await listen<{ id: number; data: number[] }>('sim:uart', (e) => {
      this.push(this.port(String(e.payload.id), uartLabel(e.payload.id), e.payload.id), Uint8Array.from(e.payload.data));
    });
    await listen<{ source: string; text: string }>('sim:log', (e) => {
      if (e.payload.source === 'stdout') {
        this.push(this.port('stdout', 'Sortie standard (printf)', null), encoder.encode(`${e.payload.text}\n`));
      }
    });
  }

  /** Nouvelle simulation : tampons vidés, débits du firmware à relire. */
  reset(): void {
    this.ports = [];
    this.series = [];
    this.partial = {};
    this.started = performance.now();
  }

  private port(key: string, label: string, id: number | null): Port {
    let p = this.ports.find((x) => x.key === key);
    if (p === undefined) {
      this.ports = [...this.ports, { key, label, id, firmwareBaud: null, chunks: [], bytes: 0 }];
      p = this.ports[this.ports.length - 1]!;
      if (this.current === '' || (this.current === 'stdout' && id !== null)) this.current = key;
    }
    return p;
  }

  private push(p: Port, data: Uint8Array): void {
    p.chunks.push({ t: performance.now() - this.started, data });
    p.bytes += data.length;
    while (p.bytes > MAX_BYTES && p.chunks.length > 1) {
      p.bytes -= p.chunks.shift()!.data.length;
    }
    if (p.key === this.current) this.plot(p.key, decoder.decode(data, { stream: false }));
  }

  /** Repère sur sa propre ligne dans chaque port (coupure d'alimentation…). */
  marker(text: string): void {
    const t = performance.now() - this.started;
    for (const p of this.ports) p.chunks.push({ t, data: encoder.encode(text), sys: true });
    this.partial = {};
    this.ports = [...this.ports];
  }

  /** Analyse les lignes complètes pour le traceur. */
  private plot(key: string, text: string): void {
    const all = (this.partial[key] ?? '') + text;
    const lines = all.split(/\r?\n/);
    this.partial[key] = lines.pop() ?? '';
    let changed = false;
    for (const line of lines) {
      const values = parsePlotLine(line);
      for (const [name, v] of values) {
        let s = this.series.find((x) => x.name === name);
        if (s === undefined) {
          if (this.series.length >= MAX_SERIES) continue;
          s = { name, points: [] };
          this.series.push(s);
        }
        s.points.push(v);
        if (s.points.length > MAX_POINTS) s.points.splice(0, s.points.length - MAX_POINTS);
        changed = true;
      }
    }
    if (changed) this.series = [...this.series];
  }

  get active(): Port | undefined {
    return this.ports.find((p) => p.key === this.current);
  }

  /** Débit du moniteur différent de celui du firmware ? Message, ou null. */
  mismatch(p: Port | undefined): string | null {
    if (!p || p.firmwareBaud === null) return null;
    const mon = this.monitorBaud[p.key] ?? DEFAULT_BAUD;
    if (mon === p.firmwareBaud) return null;
    const fr = (n: number) => n.toLocaleString('fr-FR');
    return `Débit différent sur ${p.label} : le firmware émet à ${fr(p.firmwareBaud)} bauds, le moniteur lit à ${fr(mon)} bauds. Choisissez ${fr(p.firmwareBaud)} bauds : sur une vraie carte, le texte serait illisible.`;
  }

  select(key: string): void {
    this.current = key;
    this.series = [];
    this.partial = {};
    const p = this.active;
    if (p) this.plot(p.key, p.chunks.map((c) => decoder.decode(c.data)).join(''));
  }

  clear(): void {
    const p = this.active;
    if (p) {
      p.chunks = [];
      p.bytes = 0;
    }
    this.series = [];
    this.partial = {};
    this.ports = [...this.ports];
  }

  /** Envoie `text` (et la fin de ligne choisie) à l'UART affichée. */
  async send(text: string): Promise<void> {
    const p = this.active;
    if (!p || p.id === null) return;
    const data = Array.from(encoder.encode(text + LINE_ENDINGS[this.ending].bytes));
    await invoke('uart_send', { id: p.id, data });
  }

  /** Texte du port affiché, pour l'export. */
  exportText(p: Port): string {
    return p.chunks
      .map((c) => {
        const at = this.timestamps ? `[${stamp(c.t)}] ` : '';
        return c.sys ? `\n${at}[${decoder.decode(c.data)}]\n` : at + decoder.decode(c.data);
      })
      .join('');
  }
}

/** « 00:12.345 » : minutes, secondes, millisecondes depuis le démarrage. */
export function stamp(ms: number): string {
  const total = Math.floor(ms);
  const m = Math.floor(total / 60000);
  const s = Math.floor((total % 60000) / 1000);
  const r = total % 1000;
  return `${String(m).padStart(2, '0')}:${String(s).padStart(2, '0')}.${String(r).padStart(3, '0')}`;
}

/**
 * Une ligne du traceur : « temp:21.4,hum:55 », « temp=21.4 hum=55 » ou
 * « 21.4 55 » (courbes 1, 2…). Les jetons non numériques sont ignorés.
 */
export function parsePlotLine(line: string): [string, number][] {
  const out: [string, number][] = [];
  const tokens = line.trim().split(/[,;\t ]+/).filter(Boolean);
  tokens.forEach((tok, i) => {
    const m = /^([^:=]+)[:=](-?\d+(?:[.,]\d+)?(?:e-?\d+)?)$/i.exec(tok);
    if (m) {
      out.push([m[1]!.trim(), Number(m[2]!.replace(',', '.'))]);
    } else if (/^-?\d+(?:\.\d+)?(?:e-?\d+)?$/i.test(tok)) {
      out.push([String(i + 1), Number(tok)]);
    }
  });
  return out.filter(([, v]) => Number.isFinite(v));
}

/** Texte en octets hexadécimaux : « 48 65 6C 6C 6F 0D 0A ». */
export function toHex(data: Uint8Array): string {
  return Array.from(data, (b) => b.toString(16).toUpperCase().padStart(2, '0')).join(' ');
}

export function decode(data: Uint8Array): string {
  return decoder.decode(data);
}

export const serial = new Serial();

// components/ssd1306/model.ts — Écran OLED SSD1306 en I2C (API composant v1, phase 3).
//
// Transaction I2C : octet de contrôle (Co, bit 7 ; D/C, bit 6), puis :
// - Co = 0 : tout le reste est une suite de commandes (D/C = 0) ou de
//   données (D/C = 1) ;
// - Co = 1 : un seul octet suit, puis un nouvel octet de contrôle.
// Mémoire d'affichage (GDDRAM) : 128 colonnes × 8 pages de 8 lignes, un octet
// vertical par colonne et par page (bit 0 en haut), comme la puce.
// Adressage page (défaut après reset), horizontal et vertical, fenêtres de
// colonnes (0x21) et de pages (0x22), pointeurs de page (0xB0-0xB7) et de
// colonne (0x00-0x1F) en mode page.
// Image physique : sens des segments (A0/A1) et des lignes (C0/C8) appliqués ;
// A1 + C8 (ce qu'envoient toutes les bibliothèques) donne l'image à l'endroit
// sur un module courant, A0 + C0 la retourne. Seules les pages visibles
// (4 en 128 × 32, 8 en 128 × 64) sont affichées.
// L'écran ne s'allume qu'avec la pompe de charge (0x8D 0x14) et 0xAF, et
// seulement si VCC et GND sont alimentés ; sinon la puce n'acquitte pas.
// Non simulés (avertissement unique) : défilement matériel, décalage de ligne
// de départ (0x40-0x7F autre que 0x40) et décalage d'affichage (0xD3).

import type { ComponentFactory, Display, I2CResponse, PinState } from '../component-api';

export type Ssd1306Params = {
  address: number;
  size: '128x64' | '128x32';
  color: 'white' | 'blue' | 'yellow-blue';
};

export type Ssd1306State = {
  /** Alimenté (VCC et GND), commande 0xAF et pompe de charge actives. */
  lit: boolean;
  /** Contraste, 0 à 255 (commande 0x81). */
  contrast: number;
  /** Image inversée (0xA7). */
  inverted: boolean;
  /** Tous les pixels allumés, mémoire ignorée (0xA5). */
  allOn: boolean;
  /** Hauteur en pixels (32 ou 64). */
  height: number;
};

const WIDTH = 128;
const RAM_PAGES = 8;

/** Nombre d'octets d'argument de chaque commande qui en a. */
const ARGS: Record<number, number> = {
  0x81: 1, 0x20: 1, 0x21: 2, 0x22: 2, 0xa8: 1, 0xd3: 1, 0xd5: 1, 0xd9: 1, 0xda: 1, 0xdb: 1,
  0x8d: 1, 0x26: 6, 0x27: 6, 0x29: 5, 0x2a: 5, 0xa3: 2,
};

/** Octet aux bits inversés (lignes retournées en C0), précalculé. */
const REVERSED = new Uint8Array(256);
for (let b = 0; b < 256; b += 1) {
  let r = 0;
  for (let i = 0; i < 8; i += 1) r |= ((b >> i) & 1) << (7 - i);
  REVERSED[b] = r;
}

const createSsd1306: ComponentFactory<Ssd1306Params, Ssd1306State> = (ctx) => {
  const ram = new Uint8Array(WIDTH * RAM_PAGES);
  let height = 64;
  let pages = 8;
  let phys = new Uint8Array(WIDTH * pages);
  let screen: Display = ctx.createDisplay({ width: WIDTH, height, format: 'mono1-vertical' });

  // Registres (valeurs après reset, fiche technique SSD1306).
  let on = false;
  let pump = false;
  let contrast = 0x7f;
  let inverted = false;
  let allOn = false;
  let mode = 2; // 0 horizontal, 1 vertical, 2 page
  let colStart = 0;
  let colEnd = WIDTH - 1;
  let pageStart = 0;
  let pageEnd = RAM_PAGES - 1;
  let col = 0;
  let page = 0;
  let segRemap = false; // A1
  let comReverse = false; // C8
  const warned = new Set<string>();

  // Commande en attente de ses arguments.
  let pending: number[] = [];
  let pendingNeed = 0;

  // Zone physique modifiée pendant la transaction (x0 > x1 : aucune).
  let dx0 = WIDTH;
  let dx1 = -1;
  let dp0 = RAM_PAGES;
  let dp1 = -1;

  function warnOnce(key: string, message: string): void {
    if (warned.has(key)) return;
    warned.add(key);
    ctx.warn(message);
  }

  function resize(): void {
    height = ctx.params.size === '128x32' ? 32 : 64;
    pages = height / 8;
    phys = new Uint8Array(WIDTH * pages);
    screen = ctx.createDisplay({ width: WIDTH, height, format: 'mono1-vertical' });
    rebuild();
  }

  /** Octet de GDDRAM vers l'image physique. */
  function project(c: number, p: number): void {
    if (p >= pages) return; // page hors de l'écran (128 × 32)
    const x = segRemap ? c : WIDTH - 1 - c;
    const pp = comReverse ? p : pages - 1 - p;
    const b = ram[p * WIDTH + c] ?? 0;
    phys[pp * WIDTH + x] = comReverse ? b : (REVERSED[b] ?? 0);
    if (x < dx0) dx0 = x;
    if (x > dx1) dx1 = x;
    if (pp < dp0) dp0 = pp;
    if (pp > dp1) dp1 = pp;
  }

  /** Image physique entière recalculée (sens des segments ou des lignes changé, reset). */
  function rebuild(): void {
    for (let p = 0; p < pages; p += 1) {
      const pp = comReverse ? p : pages - 1 - p;
      for (let c = 0; c < WIDTH; c += 1) {
        const b = ram[p * WIDTH + c] ?? 0;
        phys[pp * WIDTH + (segRemap ? c : WIDTH - 1 - c)] = comReverse ? b : (REVERSED[b] ?? 0);
      }
    }
    dx0 = 0;
    dx1 = WIDTH - 1;
    dp0 = 0;
    dp1 = pages - 1;
    flush();
  }

  /** Un seul blit par transaction : la zone physique touchée. */
  function flush(): void {
    if (dx0 > dx1) return;
    const [x0, x1, p0, p1] = [dx0, dx1, dp0, dp1];
    dx0 = WIDTH;
    dx1 = -1;
    dp0 = RAM_PAGES;
    dp1 = -1;
    const w = x1 - x0 + 1;
    if (w === WIDTH && p0 === 0 && p1 === pages - 1) {
      screen.blit(0, 0, WIDTH, height, phys); // écran entier : sans copie
      return;
    }
    const data = new Uint8Array(w * (p1 - p0 + 1));
    for (let p = p0; p <= p1; p += 1) data.set(phys.subarray(p * WIDTH + x0, p * WIDTH + x1 + 1), (p - p0) * w);
    screen.blit(x0, p0 * 8, w, (p1 - p0 + 1) * 8, data);
  }

  function writeData(b: number): void {
    ram[page * WIDTH + col] = b;
    project(col, page);
    if (mode === 0) {
      col += 1;
      if (col > colEnd) {
        col = colStart;
        page = page >= pageEnd ? pageStart : page + 1;
      }
    } else if (mode === 1) {
      page += 1;
      if (page > pageEnd) {
        page = pageStart;
        col = col >= colEnd ? colStart : col + 1;
      }
    } else {
      col = (col + 1) % WIDTH;
    }
  }

  function setState(next: () => void): void {
    next();
    ctx.invalidate();
  }

  function command(cmd: number, args: number[]): void {
    const a0 = args[0] ?? 0;
    const a1 = args[1] ?? 0;
    if (cmd >= 0xb0 && cmd <= 0xb7) page = cmd & 0x07;
    else if (cmd <= 0x0f) col = (col & 0xf0) | cmd;
    else if (cmd >= 0x10 && cmd <= 0x1f) col = ((cmd & 0x0f) << 4) | (col & 0x0f);
    else if (cmd >= 0x40 && cmd <= 0x7f) {
      if (cmd !== 0x40) warnOnce('start', 'ligne de départ (0x41-0x7F) non simulée : image affichée sans décalage.');
    } else
      switch (cmd) {
        case 0xae:
          return setState(() => (on = false));
        case 0xaf:
          return setState(() => (on = true));
        case 0x8d:
          return setState(() => (pump = (a0 & 0x04) !== 0));
        case 0x81:
          return setState(() => (contrast = a0));
        case 0xa6:
          return setState(() => (inverted = false));
        case 0xa7:
          return setState(() => (inverted = true));
        case 0xa4:
          return setState(() => (allOn = false));
        case 0xa5:
          return setState(() => (allOn = true));
        case 0x20:
          mode = a0 & 0x03;
          if (mode === 3) mode = 2;
          return;
        case 0x21:
          colStart = a0 & 0x7f;
          colEnd = a1 & 0x7f;
          col = colStart;
          return;
        case 0x22:
          pageStart = a0 & 0x07;
          pageEnd = a1 & 0x07;
          page = pageStart;
          return;
        case 0xa0:
        case 0xa1:
          if (segRemap !== (cmd === 0xa1)) {
            segRemap = cmd === 0xa1;
            rebuild();
          }
          return;
        case 0xc0:
        case 0xc8:
          if (comReverse !== (cmd === 0xc8)) {
            comReverse = cmd === 0xc8;
            rebuild();
          }
          return;
        case 0xd3:
          if (a0 !== 0) warnOnce('offset', "décalage d'affichage (0xD3) non simulé : image affichée sans décalage.");
          return;
        case 0x2f:
          return warnOnce('scroll', 'défilement matériel (0x2F) non simulé : image affichée fixe.');
        case 0xa8:
        case 0xd5:
        case 0xd9:
        case 0xda:
        case 0xdb:
        case 0x2e:
        case 0x26:
        case 0x27:
        case 0x29:
        case 0x2a:
        case 0xa3:
        case 0xe3:
          return; // réglages analogiques ou de défilement : sans effet sur l'image
        default:
          warnOnce(`cmd-${cmd}`, `commande 0x${cmd.toString(16).toUpperCase().padStart(2, '0')} inconnue, ignorée.`);
      }
  }

  function commandByte(b: number): void {
    if (pendingNeed > 0) {
      pending.push(b);
      if (pending.length - 1 === pendingNeed) {
        const [cmd = 0, ...args] = pending;
        pending = [];
        pendingNeed = 0;
        command(cmd, args);
      }
      return;
    }
    const need = ARGS[b] ?? 0;
    if (need > 0) {
      pending = [b];
      pendingNeed = need;
    } else command(b, []);
  }

  function reset(): void {
    ram.fill(0);
    on = false;
    pump = false;
    contrast = 0x7f;
    inverted = false;
    allOn = false;
    mode = 2;
    colStart = 0;
    colEnd = WIDTH - 1;
    pageStart = 0;
    pageEnd = RAM_PAGES - 1;
    col = 0;
    page = 0;
    segRemap = false;
    comReverse = false;
    pending = [];
    pendingNeed = 0;
    rebuild();
    ctx.invalidate();
  }

  function live(s: PinState): boolean {
    return s.level === 'high' || s.level === 'pulled-high' || (s.volts ?? 0) > 2;
  }

  function powered(): boolean {
    const gnd = ctx.pin('GND');
    return live(ctx.pin('VCC')) && (gnd.level === 'low' || gnd.volts === 0);
  }

  let wasPowered = powered();

  return {
    onI2C(request): I2CResponse {
      if (!powered()) return { ack: false, nack: 'address' };
      const tx = request.tx;
      let i = 0;
      while (i < tx.length) {
        const control = tx[i] ?? 0;
        i += 1;
        const data = (control & 0x40) !== 0;
        if ((control & 0x80) !== 0) {
          // Co = 1 : un seul octet, puis un nouvel octet de contrôle.
          if (i < tx.length) {
            const b = tx[i] ?? 0;
            if (data) writeData(b);
            else commandByte(b);
            i += 1;
          }
        } else {
          for (; i < tx.length; i += 1) {
            const b = tx[i] ?? 0;
            if (data) writeData(b);
            else commandByte(b);
          }
        }
      }
      flush();
      // Lecture : octet d'état (bit 6 : écran éteint), comme la puce.
      const rx = new Uint8Array(request.rxLength).fill(on ? 0x00 : 0x40);
      return { ack: true, rx };
    },
    onPinChange(): void {
      const now = powered();
      if (now !== wasPowered) {
        wasPowered = now;
        reset(); // la puce repart de zéro à la mise sous tension
      }
    },
    onPower(): void {
      wasPowered = powered();
      reset();
    },
    onParamsChange(): void {
      const next = ctx.params.size === '128x32' ? 32 : 64;
      if (next !== height) {
        resize();
        ctx.invalidate();
      }
    },
    getState(): Ssd1306State {
      return { lit: on && pump && wasPowered, contrast, inverted, allOn, height };
    },
  };
};

export default createSsd1306;

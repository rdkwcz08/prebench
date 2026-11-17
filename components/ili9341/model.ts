// components/ili9341/model.ts — Écran TFT ILI9341 en SPI (API composant v1, phase 3, étape 7).
//
// Une transaction SPI se lit avec la broche DC : DC bas, chaque octet est une
// commande ; DC haut, les octets sont les paramètres de la dernière commande
// (ou les pixels de RAMWR). Le cœur remet les transactions et les changements
// de broches dans l'ordre du firmware : DC lu ici est celui de la transaction.
//
// Pixels : RGB565, octet de poids fort d'abord (COLMOD 0x55). L'écriture
// mémoire suit la fenêtre CASET × PASET, ligne par ligne ; chaque transaction
// devient au plus trois blits (fin de ligne entamée, lignes entières, début de
// ligne), sans boucle JavaScript sur les pixels. Le cœur ne redessine que les
// pixels qui changent.
//
// Orientation (MADCTL) : traduite en ordre de balayage de l'écran
// (Display.setScan), appliqué par le cœur aux écritures suivantes, comme la
// puce (l'image déjà en mémoire ne bouge pas). La dalle d'un module courant
// est montée en miroir horizontal : MADCTL 0x48 (MX, BGR), ce qu'envoient les
// bibliothèques pour l'orientation 0, affiche l'image à l'endroit, connecteur
// en haut ; 0x28 (MV) la tourne d'un quart de tour à droite, 0x88 (MY) d'un
// demi-tour, 0xE8 (MY, MX, MV) d'un quart de tour à gauche.
// Ordre des couleurs : la dalle est BGR (paramètre « panel ») ; le bit BGR de
// MADCTL doit lui correspondre, sinon rouge et bleu sont échangés, comme sur
// un vrai module.
//
// Image visible : alimentation (VCC, GND), RST haut, sortie de veille (0x11),
// allumage (0x29) et rétroéclairage (LED alimentée).

import type { ComponentFactory, Display, PinState, SPIRequest } from '../component-api';

export type Ili9341Params = {
  panel: 'bgr' | 'rgb';
};

export type Ili9341State = {
  /** Image visible : alimenté, hors reset et hors veille, allumé, rétroéclairé. */
  lit: boolean;
  /** Raison de l'écran noir, en clair ; null quand l'image est visible. */
  dark: string | null;
  /** Couleurs inversées (0x21). */
  inverted: boolean;
  /** Dernière valeur de MADCTL (orientation et ordre des couleurs). */
  madctl: number;
};

/** Dalle physique : 240 colonnes × 320 lignes (portrait, connecteur en haut). */
const WIDTH = 240;
const HEIGHT = 320;

const MADCTL_MY = 0x80;
const MADCTL_MX = 0x40;
const MADCTL_MV = 0x20;
const MADCTL_BGR = 0x08;

/** Réglages analogiques et de synchronisation : acceptés sans effet sur l'image. */
const NO_EFFECT = new Set([
  0x00, 0x13, 0x12, 0x26, 0x30, 0x34, 0x35, 0x38, 0x39, 0x44, 0x51, 0x53, 0x55, 0x5e, 0xb0, 0xb1, 0xb2,
  0xb3, 0xb4, 0xb5, 0xb6, 0xb7, 0xb8, 0xc0, 0xc1, 0xc5, 0xc7, 0xcb, 0xcf, 0xe0, 0xe1, 0xe2, 0xe3,
  0xe8, 0xea, 0xed, 0xef, 0xf2, 0xf6, 0xf7,
]);

/** Réponses des commandes de lecture (octet factice, puis données). */
const READS: Record<number, number[]> = {
  0x04: [0x00, 0x00, 0x93, 0x41], // RDDID
  0xd3: [0x00, 0x00, 0x93, 0x41], // RDID4 : ILI9341
  0x09: [0x00, 0x00, 0x00, 0x00, 0x00], // RDDST
};

const createIli9341: ComponentFactory<Ili9341Params, Ili9341State> = (ctx) => {
  const screen: Display = ctx.createDisplay({ width: WIDTH, height: HEIGHT, format: 'rgb565' });
  const warned = new Set<string>();

  // Registres (valeurs après reset, fiche technique ILI9341).
  let sleeping = true;
  let on = false;
  let inverted = false;
  let madctl = 0;
  let colmod = 0x66;
  // Fenêtre d'écriture (coordonnées logiques, bornes incluses) et curseur.
  let xs = 0;
  let xe = WIDTH - 1;
  let ys = 0;
  let ye = HEIGHT - 1;
  let cx = 0;
  let cy = 0;
  // Commande courante et ses paramètres reçus.
  let cmd = -1;
  let params: number[] = [];
  /** Octet de poids fort d'un pixel coupé entre deux transactions (-1 : aucun). */
  let carry = -1;
  /** Réponse de la dernière commande de lecture. */
  let reply: number[] = [];

  function warnOnce(key: string, message: string): void {
    if (warned.has(key)) return;
    warned.add(key);
    ctx.warn(message);
  }

  /** Taille de l'image logique (axes échangés par MV). */
  function logical(): [number, number] {
    return (madctl & MADCTL_MV) !== 0 ? [HEIGHT, WIDTH] : [WIDTH, HEIGHT];
  }

  function applyScan(): void {
    const panelBgr = ctx.params.panel !== 'rgb';
    screen.setScan({
      swapXY: (madctl & MADCTL_MV) !== 0,
      mirrorX: (madctl & MADCTL_MX) === 0, // dalle montée en miroir
      mirrorY: (madctl & MADCTL_MY) !== 0,
      bgr: ((madctl & MADCTL_BGR) !== 0) !== panelBgr,
    });
  }

  function setState(next: () => void): void {
    next();
    ctx.invalidate();
  }

  function reset(): void {
    sleeping = true;
    on = false;
    inverted = false;
    madctl = 0;
    colmod = 0x66;
    [xs, xe, ys, ye] = [0, WIDTH - 1, 0, HEIGHT - 1];
    [cx, cy] = [0, 0];
    cmd = -1;
    params = [];
    carry = -1;
    reply = [];
    applyScan();
    ctx.invalidate();
  }

  /** Paramètres complets d'une commande qui en a. */
  function onParams(): void {
    const p = params;
    const word = (i: number): number => (((p[i] ?? 0) << 8) | (p[i + 1] ?? 0)) >>> 0;
    const [lw, lh] = logical();
    switch (cmd) {
      case 0x2a: // CASET
        if (p.length === 4) {
          xs = Math.min(word(0), lw - 1);
          xe = Math.min(Math.max(word(2), xs), lw - 1);
        }
        return;
      case 0x2b: // PASET
        if (p.length === 4) {
          ys = Math.min(word(0), lh - 1);
          ye = Math.min(Math.max(word(2), ys), lh - 1);
        }
        return;
      case 0x36: // MADCTL
        if (p.length === 1) {
          madctl = p[0] ?? 0;
          applyScan();
          setState(() => undefined);
        }
        return;
      case 0x3a: // COLMOD
        if (p.length === 1) {
          colmod = p[0] ?? 0;
          if ((colmod & 0x07) !== 0x05)
            warnOnce('colmod', 'pixels de 18 bits (COLMOD autre que 0x55) non simulés : envoyez des pixels RGB565.');
        }
        return;
      case 0x33:
      case 0x37:
        return warnOnce('scroll', 'défilement vertical (0x33, 0x37) non simulé : image affichée fixe.');
    }
  }

  function command(c: number): void {
    cmd = c;
    params = [];
    carry = -1;
    const read = READS[c];
    if (read !== undefined) {
      reply = [...read];
      return;
    }
    switch (c) {
      case 0x01:
        return reset();
      case 0x10:
        return setState(() => (sleeping = true));
      case 0x11:
        return setState(() => (sleeping = false));
      case 0x28:
        return setState(() => (on = false));
      case 0x29:
        return setState(() => (on = true));
      case 0x20:
        return setState(() => (inverted = false));
      case 0x21:
        return setState(() => (inverted = true));
      case 0x2c: // RAMWR : depuis le début de la fenêtre
        [cx, cy] = [xs, ys];
        return;
      case 0x3c: // RAMWRC : là où l'écriture s'était arrêtée
      case 0x2a:
      case 0x2b:
      case 0x36:
      case 0x3a:
      case 0x33:
      case 0x37:
        return;
      default:
        if (!NO_EFFECT.has(c))
          warnOnce(`cmd-${c}`, `commande 0x${c.toString(16).toUpperCase().padStart(2, '0')} inconnue, ignorée.`);
    }
  }

  /** Pixels de RAMWR : au plus trois blits par transaction. */
  function pixels(bytes: Uint8Array): void {
    let data = bytes;
    if (carry >= 0) {
      data = new Uint8Array(bytes.length + 1);
      data[0] = carry;
      data.set(bytes, 1);
      carry = -1;
    }
    let count = data.length >> 1;
    if ((data.length & 1) === 1) carry = data[data.length - 1] ?? 0;
    const w = xe - xs + 1;
    let at = 0; // octet courant dans data
    while (count > 0) {
      if (cx !== xs || count < w) {
        // Ligne entamée, ou moins d'une ligne : jusqu'à la fin de la ligne.
        const n = Math.min(xe - cx + 1, count);
        screen.blit(cx, cy, n, 1, data.subarray(at, at + n * 2));
        at += n * 2;
        count -= n;
        cx += n;
      } else {
        // Lignes entières, jusqu'au bas de la fenêtre.
        const rows = Math.min(Math.floor(count / w), ye - cy + 1);
        screen.blit(xs, cy, w, rows, data.subarray(at, at + rows * w * 2));
        at += rows * w * 2;
        count -= rows * w;
        cy += rows - 1;
        cx = xe + 1;
      }
      if (cx > xe) {
        cx = xs;
        cy = cy >= ye ? ys : cy + 1; // fin de fenêtre : retour au début, comme la puce
      }
    }
  }

  function live(s: PinState): boolean {
    return s.level === 'high' || s.level === 'pulled-high' || (s.volts ?? 0) > 2;
  }

  function powered(): boolean {
    const gnd = ctx.pin('GND');
    return live(ctx.pin('VCC')) && (gnd.level === 'low' || gnd.volts === 0);
  }

  /** Puce hors reset : RST haut ou tiré haut (relié à 3V3 sur certains montages). */
  function running(): boolean {
    return powered() && live(ctx.pin('RST'));
  }

  let wasRunning = running();
  let backlight = live(ctx.pin('LED'));
  applyScan();

  return {
    onSPI(request: SPIRequest): Uint8Array | void {
      if (!running()) return request.wantsRx ? new Uint8Array(request.tx.length).fill(0xff) : undefined;
      const tx = request.tx;
      const isData = live(ctx.pin('DC'));
      if (!isData) {
        for (const b of tx) command(b);
      } else if ((cmd === 0x2c || cmd === 0x3c) && (colmod & 0x07) === 0x05) {
        pixels(tx);
      } else if (cmd === 0x2c || cmd === 0x3c) {
        warnOnce('colmod', 'pixels de 18 bits (COLMOD autre que 0x55) non simulés : envoyez des pixels RGB565.');
      } else {
        for (const b of tx) {
          params.push(b);
          onParams();
        }
      }
      if (!request.wantsRx) return undefined;
      // Lecture (DC haut) : réponse de la dernière commande de lecture.
      const rx = new Uint8Array(tx.length);
      for (let i = 0; i < rx.length; i += 1) rx[i] = reply.shift() ?? 0x00;
      return rx;
    },
    onPinChange(): void {
      const now = running();
      if (now !== wasRunning) {
        wasRunning = now;
        reset(); // reset matériel ou mise sous tension
      }
      const light = live(ctx.pin('LED'));
      if (light !== backlight) setState(() => (backlight = light));
    },
    onPower(): void {
      wasRunning = running();
      backlight = live(ctx.pin('LED'));
      reset();
    },
    onParamsChange(): void {
      applyScan();
    },
    getState(): Ili9341State {
      const dark = !powered()
        ? 'non alimenté'
        : !wasRunning
          ? 'en reset'
          : sleeping
            ? 'en veille'
            : !on
              ? 'éteint'
              : !backlight
                ? 'rétroéclairage éteint'
                : null;
      return { lit: dark === null, dark, inverted, madctl };
    },
  };
};

export default createIli9341;

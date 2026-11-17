// components/ws2812/model.ts — Ruban de LED adressables WS2812 / SK6812 (API composant v1,
// phase 3, étape 8).
//
// Le timing du signal n'est pas simulé (section 5.4) : le cœur remet la trame
// entière, octets dans l'ordre du fil (onLedStrip). La première LED prend les
// premiers octets, comme sur le ruban ; une trame plus courte ne change que
// les premières LED, les octets en trop sont pour la suite du ruban (ignorés).
// Ordre : GRB (WS2812), RGB, ou GRBW (SK6812, quatre octets ; le blanc est
// ajouté aux trois couleurs pour l'affichage). Sans alimentation (5V, GND), le
// ruban est éteint et ignore les trames.

import type { ComponentFactory, PinState } from '../component-api';

export type Ws2812Params = {
  count: number;
  order: 'grb' | 'rgb' | 'grbw';
};

export type Ws2812State = {
  /** Couleur de chaque LED, 0xRRGGBB (0 : éteinte). */
  colors: number[];
};

/** Nombre de LED, borné par le manifeste. */
function ledCount(n: number): number {
  return Math.min(48, Math.max(1, Math.floor(Number(n) || 1)));
}

const createWs2812: ComponentFactory<Ws2812Params, Ws2812State> = (ctx) => {
  let colors: number[] = new Array<number>(ledCount(ctx.params.count)).fill(0);

  function live(s: PinState): boolean {
    return s.level === 'high' || s.level === 'pulled-high' || (s.volts ?? 0) > 2;
  }

  function powered(): boolean {
    const gnd = ctx.pin('GND');
    return live(ctx.pin('5V')) && (gnd.level === 'low' || gnd.volts === 0);
  }

  let wasPowered = powered();

  function clear(): void {
    colors = new Array<number>(colors.length).fill(0);
    ctx.invalidate();
  }

  return {
    onLedStrip(data: Uint8Array): void {
      if (!powered()) return;
      const order = ctx.params.order;
      const step = order === 'grbw' ? 4 : 3;
      const n = Math.min(colors.length, Math.floor(data.length / step));
      if (n === 0) return;
      const next = colors.slice();
      for (let i = 0; i < n; i += 1) {
        const o = i * step;
        const a = data[o] ?? 0;
        const b = data[o + 1] ?? 0;
        const c = data[o + 2] ?? 0;
        let [r, g, bl] = order === 'rgb' ? [a, b, c] : [b, a, c];
        if (order === 'grbw') {
          const w = data[o + 3] ?? 0;
          [r, g, bl] = [Math.min(255, r + w), Math.min(255, g + w), Math.min(255, bl + w)];
        }
        next[i] = (r << 16) | (g << 8) | bl;
      }
      colors = next;
      ctx.invalidate();
    },
    onPinChange(): void {
      const now = powered();
      if (now !== wasPowered) {
        wasPowered = now;
        clear(); // coupure ou mise sous tension : LED éteintes
      }
    },
    onPower(): void {
      wasPowered = powered();
      clear();
    },
    onParamsChange(): void {
      const n = ledCount(ctx.params.count);
      if (n !== colors.length) {
        colors = new Array<number>(n).fill(0).map((_, i) => colors[i] ?? 0);
        ctx.invalidate();
      }
    },
    getState(): Ws2812State {
      return { colors };
    },
  };
};

export default createWs2812;

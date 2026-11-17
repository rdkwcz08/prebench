// app/ui/lib/displays.svelte.ts — Écrans des composants (OLED, TFT) côté interface
// (phase 3, sections 8.1 et 11.3).
//
// Le cœur envoie, au plus une fois par image et seulement fenêtre visible, la
// zone modifiée de chaque écran en binaire (canal display_channel ; format du
// message : app/src-tauri/src/core/components/display.rs). Chaque écran garde
// ses pixels ici ; les vues attachent un canvas (DisplaySurface.attach) où
// l'hôte dessine lui-même la zone modifiée (putImageData), sans repasser par
// Svelte, dans l'image d'affichage commune (frame.ts).

import type { DisplayFormat, DisplaySurfaceOptions } from '../../../components/component-api';
import { onNextFrame } from './frame';

const FORMATS: DisplayFormat[] = ['mono1-vertical', 'mono1', 'rgb565'];

interface Rect {
  x: number;
  y: number;
  w: number;
  h: number;
}

function union(a: Rect | null, b: Rect): Rect {
  if (a === null) return { ...b };
  const x0 = Math.min(a.x, b.x);
  const y0 = Math.min(a.y, b.y);
  return { x: x0, y: y0, w: Math.max(a.x + a.w, b.x + b.w) - x0, h: Math.max(a.y + a.h, b.y + b.h) - y0 };
}

/** Couleur CSS en [r, g, b] (canvas 1 × 1) ; noir si illisible. */
const colorCache = new Map<string, [number, number, number]>();
function rgb(color: string): [number, number, number] {
  const known = colorCache.get(color);
  if (known) return known;
  let out: [number, number, number] = [0, 0, 0];
  const c = document.createElement('canvas').getContext('2d');
  if (c) {
    c.fillStyle = color;
    c.fillRect(0, 0, 1, 1);
    const d = c.getImageData(0, 0, 1, 1).data;
    out = [d[0] ?? 0, d[1] ?? 0, d[2] ?? 0];
  }
  colorCache.set(color, out);
  return out;
}

interface Attached {
  ctx: CanvasRenderingContext2D;
  image: ImageData;
  options: DisplaySurfaceOptions;
}

/** Un écran : pixels reçus du cœur, canvas attachés par la vue. */
export class DisplayHost {
  readonly width: number;
  readonly height: number;
  readonly format: DisplayFormat;
  /** 0 ou 1 en monochrome, valeur RGB565 en couleur ; une case par pixel. */
  private readonly values: Uint8Array | Uint16Array;
  private readonly canvases = new Map<HTMLCanvasElement, Attached>();
  private pending: Rect | null = null;

  constructor(width: number, height: number, format: DisplayFormat) {
    this.width = width;
    this.height = height;
    this.format = format;
    this.values = format === 'rgb565' ? new Uint16Array(width * height) : new Uint8Array(width * height);
  }

  /** Action Svelte : `<canvas use:display.attach={{ on, off, invert }} />`. */
  readonly attach = (
    node: { width: number; height: number },
    options: DisplaySurfaceOptions = {},
  ): { update(options?: DisplaySurfaceOptions): void; destroy(): void } => {
    const canvas = node as HTMLCanvasElement;
    canvas.width = this.width;
    canvas.height = this.height;
    const ctx = canvas.getContext('2d');
    if (ctx === null) return { update() {}, destroy() {} };
    this.canvases.set(canvas, { ctx, image: ctx.createImageData(this.width, this.height), options });
    this.draw(canvas, { x: 0, y: 0, w: this.width, h: this.height });
    return {
      update: (next = {}) => {
        const a = this.canvases.get(canvas);
        if (a === undefined) return;
        a.options = next;
        this.draw(canvas, { x: 0, y: 0, w: this.width, h: this.height });
      },
      destroy: () => {
        this.canvases.delete(canvas);
      },
    };
  };

  /** Zone reçue du cœur (mêmes règles d'octets qu'un blit). */
  apply(rect: Rect, data: Uint8Array): void {
    const { x, y, w, h } = rect;
    const W = this.width;
    const v = this.values;
    if (this.format === 'rgb565') {
      for (let sy = 0; sy < h; sy += 1) {
        let i = sy * w * 2;
        let o = (y + sy) * W + x;
        for (let sx = 0; sx < w; sx += 1, i += 2, o += 1) v[o] = ((data[i] ?? 0) << 8) | (data[i + 1] ?? 0);
      }
    } else if (this.format === 'mono1') {
      const stride = Math.ceil(w / 8);
      for (let sy = 0; sy < h; sy += 1)
        for (let sx = 0; sx < w; sx += 1)
          v[(y + sy) * W + x + sx] = ((data[sy * stride + (sx >> 3)] ?? 0) >> (7 - (sx & 7))) & 1;
    } else {
      for (let sy = 0; sy < h; sy += 1)
        for (let sx = 0; sx < w; sx += 1)
          v[(y + sy) * W + x + sx] = ((data[(sy >> 3) * w + sx] ?? 0) >> (sy & 7)) & 1;
    }
    this.pending = union(this.pending, rect);
    onNextFrame(this.flush);
  }

  /** Zone accumulée dessinée dans les canvas, dans l'image commune (frame.ts). */
  private readonly flush = (): void => {
    const r = this.pending;
    this.pending = null;
    if (r !== null) for (const canvas of this.canvases.keys()) this.draw(canvas, r);
  };

  /** Dessine une zone dans un canvas attaché, à ses couleurs. */
  private draw(canvas: HTMLCanvasElement, r: Rect): void {
    const a = this.canvases.get(canvas);
    if (a === undefined || r.w <= 0 || r.h <= 0) return;
    const px = a.image.data;
    const W = this.width;
    if (this.format === 'rgb565') {
      // Inversion (INVON d'un ILI9341) : chaque composante complémentée, sans filtre CSS.
      const v = this.values as Uint16Array;
      const flip = a.options.invert === true ? 0xffff : 0;
      for (let y = r.y; y < r.y + r.h; y += 1)
        for (let x = r.x; x < r.x + r.w; x += 1) {
          const c = (v[y * W + x] ?? 0) ^ flip;
          const o = (y * W + x) * 4;
          px[o] = (((c >> 11) & 0x1f) * 255) / 31;
          px[o + 1] = (((c >> 5) & 0x3f) * 255) / 63;
          px[o + 2] = ((c & 0x1f) * 255) / 31;
          px[o + 3] = 255;
        }
    } else {
      const lit = rgb(a.options.on ?? '#ffffff');
      const dark = rgb(a.options.off ?? '#000000');
      const [on, off] = a.options.invert === true ? [dark, lit] : [lit, dark];
      const v = this.values as Uint8Array;
      for (let y = r.y; y < r.y + r.h; y += 1)
        for (let x = r.x; x < r.x + r.w; x += 1) {
          const c = v[y * W + x] === 1 ? on : off;
          const o = (y * W + x) * 4;
          px[o] = c[0];
          px[o + 1] = c[1];
          px[o + 2] = c[2];
          px[o + 3] = 255;
        }
    }
    a.ctx.putImageData(a.image, 0, 0, r.x, r.y, r.w, r.h);
  }
}

/** Écrans de la simulation en cours, par identifiant de composant. */
class Displays {
  items = $state<Record<string, DisplayHost>>({});

  /**
   * Message « displays » du cœur. Un écran neuf (nouvelle simulation) ou
   * redimensionné remplace l'hôte ; un rechargement à chaud garde les modèles,
   * donc les écrans et leur image.
   */
  receive(buffer: ArrayBuffer): void {
    const view = new DataView(buffer);
    const bytes = new Uint8Array(buffer);
    const decoder = new TextDecoder();
    let o = 0;
    const count = view.getUint16(o, true);
    o += 2;
    for (let n = 0; n < count && o < buffer.byteLength; n += 1) {
      const idLen = view.getUint16(o, true);
      o += 2;
      const id = decoder.decode(bytes.subarray(o, o + idLen));
      o += idLen;
      const format = FORMATS[view.getUint8(o)] ?? 'mono1';
      const reset = (view.getUint8(o + 1) & 1) !== 0;
      o += 2;
      const [width, height, x, y, w, h] = [0, 2, 4, 6, 8, 10].map((k) => view.getUint16(o + k, true)) as [
        number,
        number,
        number,
        number,
        number,
        number,
      ];
      o += 12;
      const len = view.getUint32(o, true);
      o += 4;
      const data = bytes.subarray(o, o + len);
      o += len;
      let host = this.items[id];
      if (reset || host === undefined || host.width !== width || host.height !== height || host.format !== format) {
        host = new DisplayHost(width, height, format);
        this.items[id] = host;
      }
      host.apply({ x, y, w, h }, data);
    }
  }
}

export const displays = new Displays();

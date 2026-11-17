// app/tests-ui/displays.spec.ts — Écrans des composants (phase 3, étape 3).
//
// Le cœur envoie la zone modifiée de chaque écran en binaire (canal
// display_channel, message décrit dans core/components/display.rs) ; l'hôte
// dessine lui-même dans le canvas de la vue (OLED SSD1306), sans JSON.

import { expect, test, type Bench } from './fixture';

/** Message « displays » : un écran mono1-vertical 128 × 64, zone donnée. */
function message(id: string, reset: boolean, rect: [number, number, number, number], data: number[]): number[] {
  const u16 = (v: number): number[] => [v & 0xff, v >> 8];
  const out = [...u16(1), ...u16(id.length), ...[...id].map((c) => c.charCodeAt(0)), 0, reset ? 1 : 0];
  out.push(...u16(128), ...u16(64), ...rect.flatMap(u16));
  const len = data.length;
  out.push(len & 0xff, (len >> 8) & 0xff, (len >> 16) & 0xff, len >>> 24, ...data);
  return out;
}

/** Couleur [r, g, b] d'un pixel du canvas de l'écran. */
async function pixel(bench: Bench, x: number, y: number): Promise<number[]> {
  return bench.figure('ssd1306').locator('canvas').first().evaluate(
    (c, [px, py]) => {
      const d = (c as HTMLCanvasElement).getContext('2d')?.getImageData(px ?? 0, py ?? 0, 1, 1).data;
      return d ? [d[0] ?? 0, d[1] ?? 0, d[2] ?? 0] : [];
    },
    [x, y],
  );
}

test('écran OLED : image binaire du cœur, zone modifiée, inversion', async ({ bench }) => {
  await bench.emit('sim:states', { ssd1306: { lit: true, contrast: 255, inverted: false, allOn: false, height: 64 } });
  // Écran neuf : tout noir, sauf la colonne 0 de la page 0 (lignes 0 à 7).
  const full = new Array<number>(128 * 8).fill(0);
  full[0] = 0xff;
  await bench.display(message('ssd1306', true, [0, 0, 128, 64], full));
  const canvas = bench.figure('ssd1306').locator('canvas').first();
  await expect(canvas).toHaveJSProperty('width', 128);
  await expect.poll(() => pixel(bench, 0, 3)).toEqual([234, 244, 255]); // blanc du module
  expect(await pixel(bench, 1, 3)).toEqual([6, 9, 11]); // verre éteint

  // Zone modifiée seule : colonne 5, page 1 (ligne 8 allumée) ; le reste ne bouge pas.
  await bench.display(message('ssd1306', false, [5, 8, 1, 8], [0x01]));
  await expect.poll(() => pixel(bench, 5, 8)).toEqual([234, 244, 255]);
  expect(await pixel(bench, 0, 3)).toEqual([234, 244, 255]);

  // Inversion (0xA7) : couleurs échangées, sans nouveau message d'image.
  await bench.emit('sim:states', { ssd1306: { lit: true, contrast: 255, inverted: true, allOn: false, height: 64 } });
  await expect.poll(() => pixel(bench, 0, 3)).toEqual([6, 9, 11]);
  expect(await pixel(bench, 1, 3)).toEqual([234, 244, 255]);
});

test('écran OLED éteint : aucun canvas, verre noir', async ({ bench }) => {
  await bench.emit('sim:states', { ssd1306: { lit: false, contrast: 127, inverted: false, allOn: false, height: 64 } });
  await bench.display(message('ssd1306', true, [0, 0, 128, 64], new Array<number>(1024).fill(0xff)));
  await expect(bench.figure('ssd1306').locator('svg').first()).toBeVisible();
  await expect(bench.figure('ssd1306').locator('canvas')).toHaveCount(0);
});

/** Message « displays » : écran rgb565 240 × 320 (ILI9341), zone donnée. */
function rgbMessage(reset: boolean, rect: [number, number, number, number], pixels: number[]): number[] {
  const u16 = (v: number): number[] => [v & 0xff, v >> 8];
  const id = 'ili9341';
  const out = [...u16(1), ...u16(id.length), ...[...id].map((c) => c.charCodeAt(0)), 2, reset ? 1 : 0];
  out.push(...u16(240), ...u16(320), ...rect.flatMap(u16));
  const len = pixels.length * 2;
  out.push(len & 0xff, (len >> 8) & 0xff, (len >> 16) & 0xff, len >>> 24);
  for (const p of pixels) out.push(p >> 8, p & 0xff); // octet de poids fort d'abord
  return out;
}

const TFT_ON = { lit: true, dark: null, inverted: false, madctl: 0x48 };

test('écran TFT : couleurs RGB565, zone modifiée, inversion', async ({ bench }) => {
  await bench.emit('sim:states', { ili9341: TFT_ON });
  await bench.display(rgbMessage(true, [0, 0, 240, 320], new Array<number>(240 * 320).fill(0)));
  // Rouge, vert, bleu, blanc purs en (0..3, 0).
  await bench.display(rgbMessage(false, [0, 0, 4, 1], [0xf800, 0x07e0, 0x001f, 0xffff]));
  const canvas = bench.figure('ili9341').locator('canvas');
  await expect(canvas).toHaveJSProperty('width', 240);
  const at = (x: number, y: number): Promise<number[]> =>
    canvas.evaluate(
      (c, [px, py]) => {
        const d = (c as HTMLCanvasElement).getContext('2d')?.getImageData(px ?? 0, py ?? 0, 1, 1).data;
        return d ? [d[0] ?? 0, d[1] ?? 0, d[2] ?? 0] : [];
      },
      [x, y],
    );
  await expect.poll(() => at(0, 0)).toEqual([255, 0, 0]);
  expect(await at(1, 0)).toEqual([0, 255, 0]);
  expect(await at(2, 0)).toEqual([0, 0, 255]);
  expect(await at(3, 0)).toEqual([255, 255, 255]);
  expect(await at(4, 0)).toEqual([0, 0, 0]);
  // INVON : composantes complémentées, sans nouveau message d'image.
  await bench.emit('sim:states', { ili9341: { ...TFT_ON, inverted: true } });
  await expect.poll(() => at(0, 0)).toEqual([0, 255, 255]);
  expect(await at(4, 0)).toEqual([255, 255, 255]);
  // Écran noir (veille) : verre seul, la raison dans le nom accessible.
  await bench.emit('sim:states', { ili9341: { ...TFT_ON, lit: false, dark: 'en veille' } });
  await expect(canvas).toHaveCount(0);
  await expect(bench.figure('ili9341').getByRole('img', { name: 'Écran TFT 240 × 320, en veille' })).toBeVisible();
});

test.describe('netteté', () => {
  // Écran Retina : deux pixels de l'écran par pixel CSS.
  test.use({ deviceScaleFactor: 2 });

  test('écran TFT net au zoom maximal sur écran Retina : aucun pixel flou', async ({ bench, browser }) => {
    const { page } = bench;
    await bench.emit('sim:states', { ili9341: TFT_ON });
    // Damier noir et blanc d'un pixel sur 16 × 16 en haut à gauche.
    const pixels = new Array<number>(240 * 320).fill(0);
    for (let y = 0; y < 16; y += 1) for (let x = 0; x < 16; x += 1) pixels[y * 240 + x] = (x + y) % 2 ? 0xffff : 0;
    await bench.display(rgbMessage(true, [0, 0, 240, 320], pixels));
    const canvas = bench.figure('ili9341').locator('canvas');
    await expect(canvas).toHaveJSProperty('width', 240);
    // Zoom maximal (2,5) autour du damier : Ctrl/Cmd + molette.
    for (let i = 0; i < 12; i += 1) {
      const box = await canvas.boundingBox();
      if (box === null) throw new Error('canvas absent');
      await page.mouse.move(box.x + 8, box.y + 8);
      await page.keyboard.down('ControlOrMeta');
      await page.mouse.wheel(0, -400);
      await page.keyboard.up('ControlOrMeta');
    }
    await expect
      .poll(async () => Number(/scale\(([\d.]+)\)/.exec((await page.locator('.world').getAttribute('style')) ?? '')?.[1]))
      .toBeCloseTo(2.5, 2);
    const box = await canvas.boundingBox();
    if (box === null) throw new Error('canvas absent');
    // Intérieur du damier (16 pixels × 2,5 = 40 px CSS), bords exclus.
    const shot = await page.screenshot({ clip: { x: box.x + 2, y: box.y + 2, width: 34, height: 34 } });
    // Décodage du PNG dans une page vierge (sans la CSP de l'app).
    const blank = await browser.newPage();
    const blurred = await blank.evaluate(async (b64) => {
      const img = new Image();
      img.src = `data:image/png;base64,${b64}`;
      await img.decode();
      const c = document.createElement('canvas');
      [c.width, c.height] = [img.width, img.height];
      const g = c.getContext('2d');
      if (g === null) return -1;
      g.drawImage(img, 0, 0);
      const d = g.getImageData(0, 0, c.width, c.height).data;
      let gray = 0;
      for (let i = 0; i < d.length; i += 4) {
        const v = d[i] ?? 0;
        if (v > 24 && v < 231) gray += 1; // ni noir ni blanc : bord adouci
      }
      return gray;
    }, shot.toString('base64'));
    await blank.close();
    expect(blurred, 'pixels intermédiaires (image floue)').toBe(0);
  });
});

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

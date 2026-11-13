// app/tests-ui/canvas.spec.ts — Paillasse éditable (phase 2, étape 4) : modes, ajout,
// déplacement aimanté, câblage, annuler / rétablir, composants actifs au prochain démarrage.

import type { Page } from '@playwright/test';
import { expect, test, type Bench } from './fixture';

interface Saved {
  path: string;
  components: { id: string; type: string; pos: [number, number] }[];
  wires: { from: string; to: string; color?: string }[];
}

async function lastSave(bench: Bench): Promise<Saved> {
  const calls = (await bench.calls('save_bench')) as Saved[];
  const last = calls.at(-1);
  if (last === undefined) throw new Error('save_bench jamais appelé');
  return last;
}

async function center(page: Page, selector: string): Promise<[number, number]> {
  const box = await page.locator(selector).boundingBox();
  if (box === null) throw new Error(`${selector} absent`);
  return [box.x + box.width / 2, box.y + box.height / 2];
}

async function addLed(bench: Bench): Promise<void> {
  await bench.page.getByRole('button', { name: 'Ajouter un composant' }).click();
  await bench.page.getByRole('menuitem', { name: /LED/ }).click();
  await expect.poll(async () => (await bench.calls('save_bench')).length).toBeGreaterThan(0);
}

test('mode Édition / Simulation : contrôle segmenté et touche E', async ({ bench }) => {
  const { page } = bench;
  const edit = page.getByRole('button', { name: 'Édition' });
  await expect(page.getByRole('button', { name: 'Simulation', exact: true })).toHaveAttribute('aria-pressed', 'true');
  await page.keyboard.press('e');
  await expect(edit).toHaveAttribute('aria-pressed', 'true');
  await page.keyboard.press('e');
  await expect(edit).toHaveAttribute('aria-pressed', 'false');
});

test('ajout, déplacement aimanté au pas de 24, annuler et rétablir', async ({ bench }) => {
  const { page } = bench;
  await addLed(bench);
  await expect(page.getByRole('button', { name: 'Édition' })).toHaveAttribute('aria-pressed', 'true');
  let saved = await lastSave(bench);
  const led = saved.components.find((c) => c.id === 'led1');
  expect(led?.type).toBe('led');
  expect(led!.pos[0] % 24).toBe(0);
  expect(led!.pos[1] % 24).toBe(0);
  await expect(bench.figure('led1')).toHaveClass(/selected/);

  const [x, y] = await center(page, '[data-component="led1"]');
  await page.mouse.move(x, y);
  await page.mouse.down();
  await page.mouse.move(x + 37, y + 61, { steps: 5 });
  await page.mouse.up();
  await expect.poll(async () => (await lastSave(bench)).components.find((c) => c.id === 'led1')?.pos).toEqual([
    led!.pos[0] + 48,
    led!.pos[1] + 72,
  ]);

  await page.keyboard.press('ControlOrMeta+z');
  await expect.poll(async () => (await lastSave(bench)).components.find((c) => c.id === 'led1')?.pos).toEqual(led!.pos);
  const hasLed = async () => (await lastSave(bench)).components.some((c) => c.id === 'led1');
  await page.keyboard.press('ControlOrMeta+z');
  await expect.poll(hasLed).toBe(false);
  await expect(bench.figure('led1')).toHaveCount(0);
  await page.keyboard.press('ControlOrMeta+Shift+z');
  await expect.poll(hasLed).toBe(true);
  saved = await lastSave(bench);
  expect(saved.components.find((c) => c.id === 'led1')?.pos).toEqual(led!.pos);
});

test('câblage : broches compatibles en évidence, fil posé, Échap, Supprimer', async ({ bench }) => {
  const { page } = bench;
  await addLed(bench);
  const anode = '[data-endpoint="led1.A"]';
  const d13 = '[data-endpoint="board.CN5:6"]';

  // Échap abandonne un fil commencé par un clic.
  await page.locator(anode).click();
  await expect(page.locator('path.preview')).toHaveCount(1);
  await expect(page.locator(`${d13}.ok`)).toHaveCount(1); // D13 compatible
  await expect(page.locator('[data-endpoint="board.CN6:1"].dim')).toHaveCount(1); // NC atténuée
  await page.keyboard.press('Escape');
  await expect(page.locator('path.preview')).toHaveCount(0);

  // Glisser de l'anode vers D13 : fil posé et enregistré.
  const [ax, ay] = await center(page, anode);
  const [dx, dy] = await center(page, d13);
  await page.mouse.move(ax, ay);
  await page.mouse.down();
  await page.mouse.move(dx, dy, { steps: 8 });
  await page.mouse.up();
  await expect.poll(async () => (await lastSave(bench)).wires).toEqual([{ from: 'led1.A', to: 'board.CN5:6' }]);
  await expect(page.locator('[data-wire="0"]')).toHaveCount(1);

  // Inspecteur du fil : couleur choisie, puis suppression au clavier.
  await page.getByRole('radio', { name: 'Rouge' }).click();
  await expect.poll(async () => (await lastSave(bench)).wires[0]?.color).toBe('red');
  await page.keyboard.press('Delete');
  await expect.poll(async () => (await lastSave(bench)).wires).toEqual([]);
});

test('mode Simulation : rien ne bouge, broches des composants masquées', async ({ bench }) => {
  const { page } = bench;
  await addLed(bench);
  await page.keyboard.press('e'); // retour en Simulation
  const saves = (await bench.calls('save_bench')).length;
  const [x, y] = await center(page, '[data-component="led1"]');
  await page.mouse.move(x, y);
  await page.mouse.down();
  await page.mouse.move(x + 100, y + 100, { steps: 5 });
  await page.mouse.up();
  expect((await bench.calls('save_bench')).length).toBe(saves);
  await expect(page.locator('[data-endpoint="led1.A"].shown')).toHaveCount(0);
});

test('composant placé pendant la simulation : hachuré, « Redémarrer pour l\'activer »', async ({ bench }) => {
  const { page } = bench;
  await bench.emit('sim:status', { state: 'running', message: '' });
  await bench.respond('save_bench', { pending: ['led1'], checks: [] });
  await addLed(bench);
  await expect(bench.figure('led1')).toContainText('actif au prochain démarrage');
  const banner = page.getByRole('status').filter({ hasText: 'led1 sera actif au prochain démarrage' });
  await banner.getByRole('button', { name: "Redémarrer pour l'activer" }).click();
  await expect.poll(() => bench.calls('stop_simulation')).toHaveLength(1);
  await expect.poll(() => bench.calls('start_simulation')).toHaveLength(1);
});

/** Zoom courant, lu dans la transformation du monde. */
async function zoomOf(page: Page): Promise<number> {
  const t = (await page.locator('.world').getAttribute('style')) ?? '';
  return Number(/scale\(([\d.]+)\)/.exec(t)?.[1] ?? 'NaN');
}

test('ajuster la vue : au chargement, ⌘/Ctrl+0, ⌘/Ctrl+1 = 100 %, double-clic sur le tapis', async ({ bench }) => {
  const { page } = bench;
  // Au chargement : toute la carte est visible dans la paillasse.
  const benchBox = await page.locator('main.bench').boundingBox();
  const boardBox = await page.locator('svg.board').boundingBox();
  if (!benchBox || !boardBox) throw new Error('paillasse absente');
  expect(boardBox.y).toBeGreaterThanOrEqual(benchBox.y);
  expect(boardBox.y + boardBox.height).toBeLessThanOrEqual(benchBox.y + benchBox.height);
  const fitted = await zoomOf(page);

  await page.keyboard.press('ControlOrMeta+1');
  await expect.poll(() => zoomOf(page)).toBe(1);
  await page.keyboard.press('ControlOrMeta+0');
  await expect.poll(() => zoomOf(page)).toBeCloseTo(fitted, 3);

  await page.keyboard.press('ControlOrMeta+1');
  await page.mouse.dblclick(benchBox.x + benchBox.width - 40, benchBox.y + benchBox.height - 40);
  await expect.poll(() => zoomOf(page)).toBeCloseTo(fitted, 3);
});

test('mode Édition visible sur la paillasse : cadre et étiquette', async ({ bench }) => {
  const { page } = bench;
  await expect(page.locator('.mode-label')).toHaveCount(0);
  await page.keyboard.press('e');
  await expect(page.locator('.mode-label')).toHaveText('Édition');
  await expect(page.locator('main.bench')).toHaveClass(/editing/);
});

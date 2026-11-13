// app/tests-ui/edits.spec.ts — Nom affiché des composants, notes sur la paillasse, et
// chemin réel du câblage vers les validations (court-circuit signalé au test manuel).

import type { Page } from '@playwright/test';
import { expect, test, type Bench } from './fixture';

interface Saved {
  components: { id: string; name?: string; pos: [number, number] }[];
  wires: { from: string; to: string }[];
  notes: { text: string; pos: [number, number] }[];
}

async function last(bench: Bench): Promise<Saved> {
  const calls = (await bench.calls('save_bench')) as Saved[];
  const l = calls.at(-1);
  if (!l) throw new Error('save_bench jamais appelé');
  return l;
}

async function addLed(bench: Bench): Promise<void> {
  await bench.page.getByRole('button', { name: 'Ajouter un composant' }).click();
  await bench.page.getByRole('menuitem', { name: /LED/ }).click();
  await expect.poll(async () => (await bench.calls('save_bench')).length).toBeGreaterThan(0);
}

async function centerOf(page: Page, selector: string): Promise<[number, number]> {
  const b = await page.locator(selector).first().boundingBox();
  if (!b) throw new Error(`${selector} absent`);
  return [b.x + b.width / 2, b.y + b.height / 2];
}

test('nom affiché : inspecteur, puis double-clic sur l\'étiquette ; identifiant inchangé', async ({ bench }) => {
  const { page } = bench;
  await addLed(bench);
  const field = page.getByRole('textbox', { name: 'Nom affiché' });
  await field.fill('LED rouge');
  await field.press('Enter');
  await expect.poll(async () => (await last(bench)).components.find((c) => c.id === 'led1')?.name).toBe('LED rouge');
  await expect(page.locator('[data-component="led1"] figcaption')).toContainText('LED rouge');

  await page.locator('[data-component="led1"] .name').dblclick();
  const inline = page.getByRole('textbox', { name: 'Nom affiché de led1' });
  await inline.fill('Témoin');
  await inline.press('Enter');
  await expect.poll(async () => (await last(bench)).components.find((c) => c.id === 'led1')?.name).toBe('Témoin');
  expect((await last(bench)).components.some((c) => c.id === 'led1')).toBe(true);
});

test('notes : ajout, saisie, déplacement aimanté, annuler, suppression, clic droit', async ({ bench }) => {
  const { page } = bench;
  await page.getByRole('button', { name: 'Ajouter une note' }).click();
  const input = page.getByRole('textbox', { name: 'Texte de la note' });
  await expect(input).toBeFocused();
  await input.fill('Bloc alimentation');
  await input.press('Enter');
  await expect.poll(async () => (await last(bench)).notes.map((n) => n.text)).toEqual(['Bloc alimentation']);
  const pos0 = (await last(bench)).notes[0]!.pos;

  const [x, y] = await centerOf(page, '[data-note="0"]');
  await page.mouse.move(x, y);
  await page.mouse.down();
  await page.mouse.move(x + 50, y + 26, { steps: 5 });
  await page.mouse.up();
  await expect.poll(async () => (await last(bench)).notes[0]?.pos).toEqual([pos0[0] + 48, pos0[1] + 24]);
  await page.keyboard.press('ControlOrMeta+z');
  await expect.poll(async () => (await last(bench)).notes[0]?.pos).toEqual(pos0);

  // Double-clic : modification du texte.
  await page.locator('[data-note="0"]').dblclick();
  await page.getByRole('textbox', { name: 'Texte de la note' }).fill('Alimentation 3V3');
  await page.keyboard.press('Enter');
  await expect.poll(async () => (await last(bench)).notes[0]?.text).toBe('Alimentation 3V3');

  await page.locator('[data-note="0"]').click();
  await page.keyboard.press('Delete');
  await expect.poll(async () => (await last(bench)).notes).toEqual([]);

  // Clic droit sur le tapis vide (Édition) : une note à cet endroit.
  const box = await page.locator('main.bench').boundingBox();
  await page.mouse.click(box!.x + box!.width - 120, box!.y + box!.height - 80, { button: 'right' });
  await page.getByRole('textbox', { name: 'Texte de la note' }).fill('Capteurs');
  await page.keyboard.press('Enter');
  await expect.poll(async () => (await last(bench)).notes.map((n) => n.text)).toEqual(['Capteurs']);

  // Mode Simulation : visible, mais ne bouge pas.
  await page.keyboard.press('e');
  const saves = (await bench.calls('save_bench')).length;
  const [nx, ny] = await centerOf(page, '[data-note="0"]');
  await page.mouse.move(nx, ny);
  await page.mouse.down();
  await page.mouse.move(nx + 80, ny + 80, { steps: 4 });
  await page.mouse.up();
  expect((await bench.calls('save_bench')).length).toBe(saves);
  await expect(page.locator('[data-note="0"]')).toHaveText('Capteurs');
});

test('câblage réel d\'une LED entre 5V et 3V3 : fils enregistrés, court-circuit affiché', async ({ bench }) => {
  const { page } = bench;
  await addLed(bench);
  // Réponse du cœur pour ce câblage (calculée par core/checks.rs, test saved_wiring_returns_checks).
  await bench.respond('save_bench', {
    pending: [],
    checks: [
      {
        severity: 'conflict',
        text: "Court-circuit possible : led1 est posé directement entre deux alimentations (A sur 5V, K sur 3V3). Prebench ne calcule pas les courants.",
        endpoints: ['led1.A', 'led1.K'],
      },
    ],
  });
  const drag = async (from: string, to: string): Promise<void> => {
    const [ax, ay] = await centerOf(page, from);
    const [bx, by] = await centerOf(page, to);
    await page.mouse.move(ax, ay);
    await page.mouse.down();
    await page.mouse.move(bx, by, { steps: 8 });
    await page.mouse.up();
  };
  await drag('[data-endpoint="led1.A"]', '[data-endpoint="board.CN6:5"]'); // 5V
  await drag('[data-endpoint="led1.K"]', '[data-endpoint="board.CN6:4"]'); // 3V3
  await expect.poll(async () => (await last(bench)).wires).toEqual([
    { from: 'led1.A', to: 'board.CN6:5' },
    { from: 'led1.K', to: 'board.CN6:4' },
  ]);
  const tab = page.getByRole('tab', { name: /Avertissements/ });
  await expect(tab.locator('.badge.conflict')).toHaveText('1');
  await tab.click();
  await expect(page.getByText('Court-circuit possible : led1')).toBeVisible();
});

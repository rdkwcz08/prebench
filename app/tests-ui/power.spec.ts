// app/tests-ui/power.spec.ts — Reset et coupure d'alimentation (phase 2, étape 7).
//
// Coupure : bouton bascule, état « Alimentation coupée », repères dans le
// moniteur série, reset indisponible ; remise sous tension. Ni l'un ni
// l'autre n'entre dans l'historique d'édition. En débogage, les deux sont
// indisponibles et l'infobulle dit pourquoi.

import { expect, test } from './fixture';

const bytes = (s: string): number[] => Array.from(new TextEncoder().encode(s));

test('coupure puis remise sous tension : bouton, barre d\'état, moniteur série', async ({ bench }) => {
  const { page } = bench;
  await bench.emit('sim:status', { state: 'running', message: 'En cours' });
  await bench.emit('sim:uart-config', { id: 2, baud: 115200 });
  await bench.emit('sim:uart', { id: 2, data: bytes('temp:21.0\r\n') });

  await page.getByRole('button', { name: "Couper l'alimentation" }).click();
  await expect.poll(() => bench.calls('set_power')).toEqual([{ on: false }]);
  await bench.emit('sim:log', { source: 'prebench', level: 2, text: 'Alimentation coupée' });
  await bench.emit('sim:status', { state: 'off', message: "rétablissez l'alimentation pour relancer le firmware depuis zéro" });
  await bench.emit('sim:states', { LD2: { brightness: 0 } });

  const power = page.getByRole('button', { name: "Rétablir l'alimentation" });
  await expect(power).toHaveAttribute('aria-pressed', 'true');
  await expect(page.locator('footer')).toContainText('Alimentation coupée');
  await expect(page.getByRole('button', { name: 'Réinitialiser' })).toBeDisabled();
  await expect(page.getByRole('button', { name: 'Arrêter la simulation' })).toBeEnabled();
  const log = page.getByLabel('Sortie série');
  await expect(log.locator('.line.sys')).toHaveText(['Alimentation coupée']);
  await expect(page.getByRole('textbox', { name: 'Texte à envoyer' })).toBeDisabled();

  await power.click();
  await expect.poll(() => bench.calls('set_power')).toEqual([{ on: false }, { on: true }]);
  await bench.emit('sim:status', { state: 'starting', message: 'remise sous tension' });
  await bench.emit('sim:status', { state: 'running', message: 'En cours' });
  await expect(log.locator('.line.sys')).toHaveText(['Alimentation coupée', 'Alimentation rétablie']);
  // Le moniteur garde la sortie d'avant la coupure.
  await expect(log).toContainText('temp:21.0');
  await expect(page.getByRole('button', { name: "Couper l'alimentation" })).toHaveAttribute('aria-pressed', 'false');
});

test('reset et coupure hors de l\'historique d\'édition : annuler défait toujours la dernière édition', async ({ bench }) => {
  const { page } = bench;
  await bench.emit('sim:status', { state: 'running', message: 'En cours' });
  await page.getByRole('button', { name: 'Ajouter un composant' }).click();
  await page.getByRole('menuitem', { name: /^LED\s+led$/ }).click();
  const saves = async (): Promise<{ components: { id: string }[] }[]> =>
    (await bench.calls('save_bench')) as { components: { id: string }[] }[];
  await expect.poll(async () => (await saves()).at(-1)?.components.some((c) => c.id === 'led1')).toBe(true);

  await page.getByRole('button', { name: 'Réinitialiser' }).click();
  await page.getByRole('button', { name: "Couper l'alimentation" }).click();
  await bench.emit('sim:status', { state: 'off', message: "rétablissez l'alimentation pour relancer le firmware depuis zéro" });
  await page.getByRole('button', { name: "Rétablir l'alimentation" }).click();
  await bench.emit('sim:status', { state: 'running', message: 'En cours' });
  expect(await bench.calls('reset_simulation')).toHaveLength(1);

  await page.locator('main').click({ position: { x: 5, y: 5 } });
  await page.keyboard.press('ControlOrMeta+z');
  await expect.poll(async () => (await saves()).at(-1)?.components.some((c) => c.id === 'led1')).toBe(false);
  await page.keyboard.press('ControlOrMeta+Shift+z');
  await expect.poll(async () => (await saves()).at(-1)?.components.some((c) => c.id === 'led1')).toBe(true);
});

test('débogage : reset et coupure indisponibles, l\'infobulle dit pourquoi', async ({ bench }) => {
  const { page } = bench;
  await page.getByRole('button', { name: 'Déboguer' }).click();
  await bench.emit('sim:status', { state: 'debug-wait', message: 'en attente du débogueur' });
  await bench.emit('sim:status', { state: 'running', message: 'En cours (débogage)' });

  for (const name of ['Réinitialiser', "Couper l'alimentation"]) {
    const button = page.getByRole('button', { name });
    await expect(button).toHaveAttribute('aria-disabled', 'true');
    await button.hover();
    await expect(button.locator('xpath=following-sibling::*[@role="tooltip"]')).toContainText(
      'Indisponible pendant le débogage',
    );
    // Clic forcé : Playwright attend sinon qu'un bouton aria-disabled devienne actif.
    await button.click({ force: true });
  }
  await page.keyboard.press('r');
  expect(await bench.calls('reset_simulation')).toEqual([]);
  expect(await bench.calls('set_power')).toEqual([]);
});

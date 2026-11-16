// app/tests-ui/environment.spec.ts — Onglet Environnement (phase 3, étape 4, section 8.5).

import { expect, test } from './fixture';

test('environnement : curseurs au clavier, valeurs envoyées au cœur, défauts', async ({ bench }) => {
  const { page } = bench;
  await page.getByRole('tab', { name: 'Environnement' }).click();
  const temp = page.getByRole('slider', { name: 'Température' });
  await expect(temp).toHaveValue('22');
  await temp.focus();
  await page.keyboard.press('ArrowRight');
  await page.keyboard.press('ArrowRight');
  await expect(page.getByText('23 °C')).toBeVisible();
  await expect
    .poll(async () => ((await bench.calls('set_environment')).at(-1) as { environment: { temperatureC: number } } | undefined)?.environment.temperatureC)
    .toBe(23);

  // Inclinaison : l'accélération affichée est celle que renvoie le cœur (gravité).
  const roll = page.getByRole('slider', { name: 'Inclinaison, roulis' });
  await roll.focus();
  await page.keyboard.press('ArrowRight');
  await expect(page.getByText('x 0 g, y 0,5 g, z 0,87 g')).toBeVisible();

  // Luminosité : échelle logarithmique, 300 lx au départ.
  await expect(page.getByRole('slider', { name: 'Luminosité' })).toHaveAttribute('aria-valuetext', '300 lux');

  await page.getByRole('button', { name: 'Valeurs par défaut' }).click();
  await expect(temp).toHaveValue('22');
});

// app/tests-ui/nets.spec.ts — Réseau logique et onglet Avertissements (phase 2, étape 5).

import { expect, test } from './fixture';

const NETS = { PA5: 'high', PA0: 'floating', PC13: 'conflict', PA6: 'pulled-high', PB6: 'low' };

test('états des nets sur les pastilles, mot dans l\'inspecteur, effacés à l\'arrêt', async ({ bench }) => {
  const { page } = bench;
  await bench.emit('sim:status', { state: 'running', message: '' });
  await bench.emit('sim:nets', NETS);
  await expect(page.locator('g.net.high')).toHaveCount(1); // D13 (CN5:6)
  await expect(page.locator('g.net.floating')).toHaveCount(1); // A0
  await expect(page.locator('g.net.pulled-high')).toHaveCount(1);
  await expect(page.locator('g.net.low')).toHaveCount(1);
  await expect(page.locator('g.net.conflict path.cross')).toHaveCount(0); // PC13 : B1, pas de pastille

  await page.locator('[data-endpoint="board.CN5:6"]').click();
  await expect(page.getByRole('complementary', { name: 'Inspecteur' })).toContainText('Haut');
  await page.locator('[data-endpoint="board.CN8:1"]').click();
  await expect(page.getByRole('complementary', { name: 'Inspecteur' })).toContainText('Flottant');

  await bench.emit('sim:status', { state: 'stopped', message: '' });
  await expect(page.locator('g.net')).toHaveCount(0);
});

test('validations du câblage : onglet Avertissements, pastille rouge, « Afficher »', async ({ bench }) => {
  const { page } = bench;
  await bench.respond('save_bench', {
    pending: [],
    checks: [
      {
        severity: 'conflict',
        text: 'Court-circuit : 3V3 et 5V reliées par le même net. Retirez le fil qui les relie.',
        endpoints: ['board.CN6:4', 'board.CN6:5'],
      },
      { severity: 'warn', text: 'pot1.W fournit une tension analogique, mais D8 (PA9) n\'a pas d\'entrée ADC.', endpoints: [] },
    ],
  });
  await page.getByRole('button', { name: 'Ajouter un composant' }).click();
  await page.getByRole('menuitem', { name: /LED/ }).click();

  const tab = page.getByRole('tab', { name: /Avertissements/ });
  await expect(tab.locator('.badge.conflict')).toHaveText('2');
  await tab.click();
  await expect(page.getByText('Court-circuit : 3V3 et 5V')).toBeVisible();
  await expect(page.getByRole('button', { name: 'Afficher' })).toHaveCount(1);
  await page.getByRole('button', { name: 'Afficher' }).click();
  await expect(page.getByRole('complementary', { name: 'Inspecteur' })).toContainText('3V3');

  // Avertissement de la simulation, sous le câblage.
  await bench.emit('sim:warn', { code: 101, text: 'A0 (PA0) est lue flottante : activez GPIO_PULLUP.' });
  await expect(page.getByText('A0 (PA0) est lue flottante')).toBeVisible();
  await expect(tab.locator('.badge')).toHaveText('3');
});

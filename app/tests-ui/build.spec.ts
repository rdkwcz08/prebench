// app/tests-ui/build.spec.ts — Onglet Build, erreurs cliquables et rechargement des vues (phase 2, étape 3).

import { FAILED_BUILD, MAIN_C } from './data';
import { expect, test } from './fixture';

test('build en échec : onglet Build affiché, emplacements cliquables', async ({ bench }) => {
  const { page } = bench;
  await bench.emit('build:state', { state: 'building' });
  await expect(page.locator('footer')).toContainText('Build en cours');
  await bench.emit('build:result', FAILED_BUILD);

  const tab = page.getByRole('tab', { name: /Build/ });
  await expect(tab).toHaveAttribute('aria-selected', 'true');
  await expect(tab.locator('.badge.conflict')).toHaveText('2');
  await expect(page.getByText('Build en échec : 2 erreurs, 1 avertissement.')).toBeVisible();
  await expect(page.locator('footer')).toContainText('Build en échec');

  await page.getByRole('button', { name: 'Core/Src/main.c:34:30' }).click();
  await expect
    .poll(() => bench.calls('open_in_editor'))
    .toEqual([{ file: MAIN_C, line: 34, column: 30 }]);
  // Erreur sans emplacement (éditeur de liens) : message seul, pas de lien.
  await expect(page.getByText("undefined reference to `HAL_Foo'")).toBeVisible();
  await expect(page.locator('.loc')).toHaveCount(2);
});

test('build réussi : durée dans la barre d\'état', async ({ bench }) => {
  await bench.emit('build:result', { ok: true, duration_ms: 1234, errors: 0, warnings: 0, diagnostics: [] });
  await expect(bench.page.locator('footer')).toContainText('Build sim 1,2 s');
  await expect(bench.page.getByRole('tab', { name: 'Journal' })).toHaveAttribute('aria-selected', 'true');
});

test('fichier modifié dans l\'éditeur : projet relu, état des composants conservé', async ({ bench }) => {
  await bench.emit('sim:states', { LD2: { brightness: 1 }, B1: { pressed: false }, echo: { pings: 7 } });
  await expect(bench.figure('echo')).toContainText('7 pings');
  const before = (await bench.calls('open_project')).length;
  await bench.emit('project:changed', { firmware: false, views: ['echo'], models: [], project: false });
  await expect.poll(async () => (await bench.calls('open_project')).length).toBe(before + 1);
  await expect(bench.figure('echo')).toContainText('7 pings');
  await expect(bench.figure('LD2').locator('svg')).toBeVisible();
});

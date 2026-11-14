// app/tests-ui/inspector.spec.ts — Inspecteur (phase 2, étape 7) : Configuration, État,
// Net pour une broche de la carte, un fil et un composant (maquette).

import { demoProject, expect, test } from './fixture';

test('broche de la carte : mode, fonction, tirage, niveau, période, net', async ({ bench }) => {
  const { page } = bench;
  const inspector = page.getByRole('complementary', { name: 'Inspecteur' });
  await bench.emit('sim:status', { state: 'running', message: 'En cours' });
  await bench.emit('sim:pins', {
    PA5: { mode: 1, pull: 0, function: null, period_us: 1_000_000 },
    PA2: { mode: 3, pull: 0, function: 'uart2:TX', period_us: null },
  });
  await bench.emit('sim:nets', { PA5: 'high' });

  await page.locator('[data-endpoint="board.CN5:6"]').click();
  await expect(inspector.getByRole('heading', { level: 2 })).toHaveText('Broche PA5');
  await expect(inspector).toContainText('Connecteur CN5, repère D13');
  for (const h of ['Configuration', 'État', 'Net']) await expect(inspector.getByRole('heading', { name: h })).toBeVisible();
  await expect(inspector).toContainText('Sortie push-pull');
  await expect(inspector).toContainText('GPIO');
  await expect(inspector).toContainText('Aucune');
  await expect(inspector.locator('.pill.high')).toHaveText('Haut');
  // Donnée en chasse fixe : période et membres du net.
  const periodValue = inspector.locator('dd.data', { hasText: /1\s000 ms/ });
  await expect(periodValue).toBeVisible();
  await expect(inspector.locator('dd.data', { hasText: 'LD2.A' })).toBeVisible();
  // Rapport cyclique et ADC : étape 8.
  await expect(inspector).not.toContainText('Rapport cyclique');
});

test('fil et composant : trois sections, couleur et fonction, niveaux par broche', async ({ bench }) => {
  const { page } = bench;
  const inspector = page.getByRole('complementary', { name: 'Inspecteur' });
  const project = demoProject() as Record<string, unknown>;
  await bench.respond('open_project', { ...project, wires: [{ from: 'board.CN5:6', to: 'board.CN8:1' }] });
  await bench.emit('project:changed', { project: true, views: [], models: [] });
  await expect(page.locator('[data-wire="0"]')).toHaveCount(1);
  await bench.emit('sim:status', { state: 'running', message: 'En cours' });
  await bench.emit('sim:nets', { PA5: 'high', PA0: 'high' });

  await page.getByRole('button', { name: 'Édition', exact: true }).click();
  // Fil courbe : le centre de sa boîte n'est pas sur le tracé ; sélection au
  // pointerdown sur la zone de saisie, comme le fait la paillasse.
  await page.locator('[data-wire="0"] path.hit').dispatchEvent('pointerdown', { bubbles: true, button: 0, pointerId: 1 });
  await expect(inspector.getByRole('heading', { level: 2 })).toHaveText('Fil');
  for (const h of ['Configuration', 'État', 'Net']) await expect(inspector.getByRole('heading', { name: h })).toBeVisible();
  await expect(inspector).toContainText('Jaune, signal (auto)');
  await expect(inspector.locator('.pill.high')).toHaveText('Haut');
  await expect(inspector.locator('dd.data', { hasText: 'PA0 (A0)' })).toBeVisible();
  await expect(inspector.locator('dd.data', { hasText: 'LD2.A' })).toBeVisible();

  // Sélection d'un composant en Édition (en Simulation, le clic l'actionne).
  await bench.emit('sim:states', { LD2: { brightness: 1 } });
  await page.locator('[data-component="LD2"]').click();
  await expect(inspector.getByRole('heading', { level: 2 })).toHaveText('LD2');
  for (const h of ['Configuration', 'État', 'Net']) await expect(inspector.getByRole('heading', { name: h })).toBeVisible();
  await expect(inspector).toContainText('Intégré à la carte');
  await expect(inspector.locator('dd', { hasText: 'Vert' })).toBeVisible(); // couleur en mot
  await expect(inspector.locator('dd.data', { hasText: /^100 %$/ })).toBeVisible(); // luminosité
  await expect(inspector.locator('.pill.high')).toHaveText('Haut'); // anode sur PA5
  await expect(inspector.locator('dd.data', { hasText: 'PA5 (D13)' })).toBeVisible();
});

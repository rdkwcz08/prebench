// app/tests-ui/guards.spec.ts — Pause, reprise et garde-fous dans l'interface (phase 2, étape 2).

import { expect, test } from './fixture';

const MO = 1024 * 1024;

test('Espace met en pause puis reprend ; la barre d\'outils suit l\'état', async ({ bench }) => {
  const { page } = bench;
  await bench.emit('sim:status', { state: 'running', message: '' });
  await expect(page.getByRole('button', { name: 'Mettre en pause' })).toBeVisible();

  await page.keyboard.press('Space');
  await expect.poll(() => bench.calls('pause_simulation')).toHaveLength(1);

  await bench.emit('sim:status', { state: 'paused', message: 'simulation en pause' });
  await expect(page.locator('footer')).toContainText('En pause');
  await page.getByRole('button', { name: 'Reprendre' }).click();
  await expect.poll(() => bench.calls('resume_simulation')).toHaveLength(1);
});

test('indicateur CPU et RAM : chiffres en chasse fixe, masqué hors simulation', async ({ bench }) => {
  const { page } = bench;
  const footer = page.locator('footer');
  await expect(footer).not.toContainText('CPU');

  await bench.emit('sim:status', { state: 'running', message: '' });
  await bench.emit('sim:usage', { cpu: 7.6, memory: 2.6 * MO, memory_limit: 256 * MO });
  await expect(footer.locator('.cpu')).toHaveText('8 %');
  await expect(footer.locator('.ram')).toHaveText('2,6 Mo');
  await bench.emit('sim:usage', { cpu: 100, memory: 38 * MO, memory_limit: 256 * MO });
  await expect(footer.locator('.cpu')).toHaveText('100 %');
  await expect(footer.locator('.ram')).toHaveText('38 Mo');
  const font = await footer.locator('.cpu').evaluate((el) => getComputedStyle(el).fontVariantNumeric);
  expect(font).toContain('tabular-nums');

  await bench.emit('sim:status', { state: 'stopped', message: 'simulation arrêtée' });
  await expect(footer).not.toContainText('CPU');
});

test('pause automatique : bannière avec « Reprendre », retirée à la reprise', async ({ bench }) => {
  const { page } = bench;
  await bench.emit('sim:status', { state: 'running', message: '' });
  await bench.emit('sim:guard', { kind: 'cpu', text: 'Simulation mise en pause : test.' });
  await bench.emit('sim:status', { state: 'paused', message: 'pause automatique (CPU)' });
  const banner = page.getByRole('alert').filter({ hasText: 'Simulation mise en pause' });
  await expect(banner).toBeVisible();

  await banner.getByRole('button', { name: 'Reprendre' }).click();
  await expect.poll(() => bench.calls('resume_simulation')).toHaveLength(1);
  await bench.emit('sim:status', { state: 'running', message: '' });
  await expect(banner).toHaveCount(0);
});

test('limite mémoire : bannière d\'arrêt, fermée à la main', async ({ bench }) => {
  const { page } = bench;
  await bench.emit('sim:guard', { kind: 'memory', text: 'Firmware arrêté : il a dépassé la limite de 256 Mo de mémoire.' });
  await bench.emit('sim:status', { state: 'stopped', message: '' });
  const banner = page.getByRole('alert').filter({ hasText: '256 Mo' });
  await expect(banner).toBeVisible();
  await banner.getByRole('button', { name: 'Fermer' }).click();
  await expect(banner).toHaveCount(0);
});

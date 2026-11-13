// app/tests-ui/debug.spec.ts — Session de débogage dans l'interface (phase 2, étape 3 bis).

import { expect, test } from './fixture';

test('Déboguer : attente de VS Code, outil manquant, point d\'arrêt', async ({ bench }) => {
  const { page } = bench;
  await page.getByRole('button', { name: 'Déboguer' }).click();
  await expect.poll(() => bench.calls('start_debug')).toHaveLength(1);

  await bench.emit('sim:status', { state: 'debug-wait', message: 'en attente du débogueur' });
  const ready = page.getByRole('status').filter({ hasText: 'Session de débogage prête' });
  await expect(ready).toContainText('« Prebench : déboguer le firmware »');
  await expect(ready).toContainText('.vscode/launch.json');
  await expect(page.getByRole('alert').filter({ hasText: 'CodeLLDB' })).toContainText(
    'code --install-extension vadimcn.vscode-lldb',
  );
  await expect(page.locator('footer')).toContainText('En attente du débogueur');
  await expect(page.getByRole('button', { name: 'Déboguer' })).toHaveCount(0);

  await bench.emit('sim:status', { state: 'running', message: 'Nucleo-F401RE (débogage)' });
  await expect(ready).toHaveCount(0);
  await bench.emit('sim:status', { state: 'breakpoint', message: 'le débogueur a arrêté le firmware' });
  await expect(page.locator('footer')).toContainText("Arrêté sur un point d'arrêt");
  // Ni pause ni reset sur un firmware figé par le débogueur.
  await expect(page.getByRole('button', { name: 'Mettre en pause' })).toHaveCount(0);
  await expect(page.getByRole('button', { name: 'Réinitialiser' })).toBeDisabled();
  await page.keyboard.press('Space');
  await expect.poll(() => bench.calls('pause_simulation')).toHaveLength(0);

  await page.getByRole('button', { name: 'Arrêter la simulation' }).click();
  await expect.poll(() => bench.calls('stop_simulation')).toHaveLength(1);
});

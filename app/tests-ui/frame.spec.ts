// app/tests-ui/frame.spec.ts — Paillasse immobile : aucune image demandée (section 11.3).
//
// Simulation en cours, tous les composants du catalogue sur la paillasse, mais
// rien ne change : l'interface ne doit demander aucune image
// (requestAnimationFrame) ni faire tourner d'animation CSS sans fin, sinon
// WebKit redessine la fenêtre en permanence (CPU de WebContent et du GPU).
// Une image du cœur, des octets série ou une zone d'écran en demandent une
// seule, partagée (frame.ts).

import { expect, test } from './fixture';

type Counter = { __pbRaf: number };

test('paillasse immobile : aucune image demandée, aucune animation sans fin', async ({ bench }) => {
  const { page } = bench;
  // Compteur posé après le chargement : frame.ts appelle requestAnimationFrame
  // à chaque demande, il passe donc par l'enveloppe.
  await page.evaluate(() => {
    const w = window as unknown as Counter & Window;
    const raf = w.requestAnimationFrame.bind(w);
    w.__pbRaf = 0;
    w.requestAnimationFrame = (cb) => {
      w.__pbRaf += 1;
      return raf(cb);
    };
  });
  const count = (): Promise<number> => page.evaluate(() => (window as unknown as Counter).__pbRaf);

  await bench.emit('sim:status', { state: 'running', message: 'En cours' });
  await bench.emit('sim:uart-config', { id: 2, baud: 115200 });
  await bench.emit('sim:frame', {
    states: { LD2: { brightness: 1 } },
    nets: { PA5: 'high' },
    pins: { PA5: { mode: 1, pull: 0, function: null, period_us: null, pwm: null, adc: null } },
    uart: [{ id: 2, data: Array.from(new TextEncoder().encode('prêt\r\n')) }],
  });
  await expect(page.getByLabel('Sortie série')).toContainText('prêt');

  // Le temps que les dernières images (et leurs suites) passent.
  await expect
    .poll(async () => {
      const before = await count();
      await page.waitForTimeout(300);
      return (await count()) - before;
    })
    .toBe(0);

  // Deux secondes sans rien recevoir : aucune demande d'image.
  const before = await count();
  await page.waitForTimeout(2000);
  expect(await count(), "demandes d'image sur une paillasse immobile").toBe(before);

  // Aucune animation CSS ou SVG sans fin en cours.
  const endless = await page.evaluate(() =>
    document
      .getAnimations()
      .filter((a) => a.playState === 'running' && a.effect?.getComputedTiming().iterations === Infinity)
      .map((a) => (a.effect as KeyframeEffect | null)?.target?.toString() ?? '?'),
  );
  expect(endless, 'animations sans fin').toEqual([]);

  // Contrôle du compteur : une nouvelle image du cœur en demande une.
  await bench.emit('sim:frame', { states: { LD2: { brightness: 0 } } });
  await expect.poll(count).toBeGreaterThan(before);
});

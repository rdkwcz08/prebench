// app/tests-ui/views.spec.ts — Montage réel des vues de composants compilées à l'exécution.
//
// Pour chaque composant fourni : la vue se compile dans la page (compilateur
// Svelte embarqué, CSP de l'app), se monte sur la paillasse sans vue de repli,
// et se redessine quand le cœur envoie un nouvel état.

import { expect, providedViews, test } from './fixture';

/**
 * Deux états successifs par composant fourni. Un composant ajouté à
 * components/ sans entrée ici fait échouer le test : il faut le compléter.
 */
const STATES: Record<string, [unknown, unknown]> = {
  led: [{ brightness: 0 }, { brightness: 1 }],
  button: [{ pressed: false }, { pressed: true }],
  echo: [{ pings: 0 }, { pings: 42 }],
  'led-rgb': [
    { r: 0, g: 0, b: 0 },
    { r: 1, g: 0.5, b: 0 },
  ],
  servo: [
    { angle: 0, pulse_ms: 1, freq_hz: 50, status: 'ok' },
    { angle: 180, pulse_ms: 2, freq_hz: 50, status: 'ok' },
  ],
  buzzer: [
    { sounding: false, freq_hz: null },
    { sounding: true, freq_hz: 2300 },
  ],
  potentiometer: [
    { position: 0.5, volts: 1.65 },
    { position: 1, volts: 3.3 },
  ],
};

/** Instance de chaque composant dans le projet de démonstration (fixture.ts). */
const INSTANCE: Record<string, string> = { led: 'LD2', button: 'B1' };
const instanceOf = (type: string): string => INSTANCE[type] ?? type;

test('chaque composant fourni a des états de test', () => {
  expect(providedViews().filter((id) => !(id in STATES))).toEqual([]);
});

for (const type of providedViews()) {
  test(`vue ${type} : montée et redessinée`, async ({ bench }) => {
    const states = STATES[type];
    test.skip(states === undefined, 'états absents (voir le test précédent)');
    if (states === undefined) return;
    const id = instanceOf(type);
    const figure = bench.figure(id);

    await bench.emit('sim:states', { [id]: states[0] });
    const svg = figure.locator('svg').first();
    await expect(svg).toBeVisible();
    await expect(figure.locator('.placeholder')).toHaveCount(0);
    await expect(figure.locator('.fallback')).toHaveCount(0);
    const before = await svg.innerHTML();

    await bench.emit('sim:states', { [id]: states[1] });
    await expect.poll(() => svg.innerHTML()).not.toBe(before);
  });
}

test('bouton : appui et relâchement envoyés au cœur', async ({ bench }) => {
  await bench.emit('sim:states', { B1: { pressed: false } });
  const cap = bench.figure('B1').locator('[role="button"]');
  const box = await cap.boundingBox();
  if (box === null) throw new Error('bouton B1 non affiché');
  await bench.page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
  await bench.page.mouse.down();
  await bench.page.mouse.up();
  await expect
    .poll(() => bench.calls('component_control'))
    .toEqual([
      { component: 'B1', control: 'press', value: true },
      { component: 'B1', control: 'press', value: false },
    ]);
});

test('thème : système par défaut, puis clair et sombre au choix, mémorisé', async ({ bench }) => {
  const { page } = bench;
  await page.emulateMedia({ colorScheme: 'dark' });
  const html = page.locator('html');
  await expect(html).toHaveAttribute('data-theme', 'dark');
  await page.emulateMedia({ colorScheme: 'light' });
  await expect(html).toHaveAttribute('data-theme', 'light'); // suit le système en direct

  const toggle = page.getByRole('button', { name: /^Thème du système/ });
  await toggle.click(); // système -> clair
  await page.emulateMedia({ colorScheme: 'dark' });
  await expect(html).toHaveAttribute('data-theme', 'light'); // choix manuel prioritaire
  await page.getByRole('button', { name: /^Thème clair/ }).click(); // clair -> sombre
  await expect(html).toHaveAttribute('data-theme', 'dark');

  await page.reload();
  await expect(html).toHaveAttribute('data-theme', 'dark');
  await expect(page.getByRole('button', { name: /^Thème sombre/ })).toBeVisible();
});

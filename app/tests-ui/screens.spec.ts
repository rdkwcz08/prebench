// app/tests-ui/screens.spec.ts — Captures de l'écran principal pour la revue visuelle.
//
// Écran principal en simulation (blinky ouvert, LD2 allumée, Journal rempli),
// en thème clair et sombre, pour chaque navigateur. Les PNG sont écrits dans
// app/test-screens/ et publiés comme artefact de la CI Linux. Aucune
// comparaison automatique : la revue est faite à l'œil, contre la maquette.

import { mkdirSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { FAILED_BUILD } from './data';
import { demoProject, expect, test } from './fixture';

const outDir = fileURLToPath(new URL('../test-screens/', import.meta.url));

/** Texte réel du cœur (core/sim.rs, pause automatique). */
const CPU_GUARD_TEXT =
  "Simulation mise en pause : le firmware a calculé sans pause pendant 5 s, à plus de 80 % d'un cœur. " +
  "Une boucle sans délai tourne probablement à vide : ajoutez un HAL_Delay ou une attente d'interruption, puis reprenez.";

const THEMES = [
  { scheme: 'light', name: 'clair' },
  { scheme: 'dark', name: 'sombre' },
] as const;

for (const theme of THEMES) {
  test(`écran principal, thème ${theme.name}`, async ({ bench }, info) => {
    await bench.page.emulateMedia({ colorScheme: theme.scheme, reducedMotion: 'reduce' });
    await bench.emit('sim:status', { state: 'running', message: 'En cours' });
    await bench.emit('sim:log', { source: 'prebench', level: 1, text: 'Build « sim » réussi en 1,4 s' });
    await bench.emit('sim:log', { source: 'firmware', level: 1, text: 'Démarrage du blinky' });
    await bench.emit('sim:states', { LD2: { brightness: 1 }, B1: { pressed: false }, echo: { pings: 3 } });
    await bench.emit('sim:usage', { cpu: 4.2, memory: 2.6 * 1024 * 1024, memory_limit: 256 * 1024 * 1024 });
    await expect(bench.figure('LD2').locator('svg')).toBeVisible();
    await expect(bench.figure('B1').locator('svg')).toBeVisible();

    mkdirSync(outDir, { recursive: true });
    await bench.page.screenshot({
      path: `${outDir}ecran-principal-${theme.name}-${info.project.name}.png`,
      animations: 'disabled',
    });
  });

  test(`pause automatique, thème ${theme.name}`, async ({ bench }, info) => {
    await bench.page.emulateMedia({ colorScheme: theme.scheme, reducedMotion: 'reduce' });
    await bench.emit('sim:status', { state: 'running', message: 'En cours' });
    await bench.emit('sim:states', { LD2: { brightness: 1 }, B1: { pressed: false }, echo: { pings: 0 } });
    await bench.emit('sim:usage', { cpu: 99.6, memory: 2.6 * 1024 * 1024, memory_limit: 256 * 1024 * 1024 });
    await bench.emit('sim:guard', { kind: 'cpu', text: CPU_GUARD_TEXT });
    await bench.emit('sim:status', { state: 'paused', message: 'pause automatique (CPU)' });
    await expect(bench.page.getByRole('alert')).toBeVisible();

    mkdirSync(outDir, { recursive: true });
    await bench.page.screenshot({
      path: `${outDir}pause-automatique-${theme.name}-${info.project.name}.png`,
      animations: 'disabled',
    });
  });

  test(`build en échec, thème ${theme.name}`, async ({ bench }, info) => {
    await bench.page.emulateMedia({ colorScheme: theme.scheme, reducedMotion: 'reduce' });
    await bench.emit('sim:status', { state: 'running', message: 'En cours' });
    await bench.emit('sim:states', { LD2: { brightness: 1 }, B1: { pressed: false }, echo: { pings: 0 } });
    await bench.emit('sim:usage', { cpu: 3.1, memory: 2.6 * 1024 * 1024, memory_limit: 256 * 1024 * 1024 });
    await bench.emit('build:result', FAILED_BUILD);
    await expect(bench.page.getByRole('tab', { name: /Build/ })).toHaveAttribute('aria-selected', 'true');

    mkdirSync(outDir, { recursive: true });
    await bench.page.screenshot({
      path: `${outDir}build-en-echec-${theme.name}-${info.project.name}.png`,
      animations: 'disabled',
    });
  });

  test(`point d'arrêt, thème ${theme.name}`, async ({ bench }, info) => {
    await bench.page.emulateMedia({ colorScheme: theme.scheme, reducedMotion: 'reduce' });
    await bench.page.getByRole('button', { name: 'Déboguer' }).click();
    await bench.emit('sim:status', { state: 'debug-wait', message: 'en attente du débogueur' });
    await bench.emit('sim:states', { LD2: { brightness: 1 }, B1: { pressed: false }, echo: { pings: 0 } });
    await bench.emit('sim:usage', { cpu: 0, memory: 3.1 * 1024 * 1024, memory_limit: 256 * 1024 * 1024 });
    await bench.emit('sim:status', { state: 'breakpoint', message: 'le débogueur a arrêté le firmware' });
    await expect(bench.page.locator('footer')).toContainText("Arrêté sur un point d'arrêt");
    await bench.page.mouse.move(640, 790); // aucune infobulle sur la capture

    mkdirSync(outDir, { recursive: true });
    await bench.page.screenshot({
      path: `${outDir}point-d-arret-${theme.name}-${info.project.name}.png`,
      animations: 'disabled',
    });
  });

  test(`édition et câblage, thème ${theme.name}`, async ({ bench }, info) => {
    const { page } = bench;
    await page.emulateMedia({ colorScheme: theme.scheme, reducedMotion: 'reduce' });
    await page.getByRole('button', { name: 'Ajouter un composant' }).click();
    await page.getByRole('menuitem', { name: /^LED\s+led$/ }).click();
    const drag = async (from: string, to: string): Promise<void> => {
      const a = await page.locator(from).boundingBox();
      const b = await page.locator(to).boundingBox();
      if (!a || !b) throw new Error('broche absente');
      await page.mouse.move(a.x + a.width / 2, a.y + a.height / 2);
      await page.mouse.down();
      await page.mouse.move(b.x + b.width / 2, b.y + b.height / 2, { steps: 6 });
      await page.mouse.up();
    };
    await drag('[data-endpoint="led1.A"]', '[data-endpoint="board.CN5:6"]');
    await drag('[data-endpoint="led1.K"]', '[data-endpoint="board.CN5:7"]');
    await expect(page.locator('[data-wire]')).toHaveCount(2);
    // Tracé en cours depuis A0 : broches compatibles allumées.
    await page.locator('[data-endpoint="board.CN8:1"]').click();
    await page.mouse.move(700, 300);

    mkdirSync(outDir, { recursive: true });
    await page.screenshot({
      path: `${outDir}edition-cablage-${theme.name}-${info.project.name}.png`,
      animations: 'disabled',
    });
  });

  for (const mode of ['simulation', 'edition'] as const) {
    test(`mode ${mode}, thème ${theme.name}`, async ({ bench }, info) => {
      const { page } = bench;
      await page.emulateMedia({ colorScheme: theme.scheme, reducedMotion: 'reduce' });
      await bench.emit('sim:status', { state: 'running', message: 'En cours' });
      await bench.emit('sim:states', { LD2: { brightness: 1 }, B1: { pressed: false }, echo: { pings: 3 } });
      if (mode === 'edition') await page.keyboard.press('e');
      await page.mouse.move(5, 790);

      mkdirSync(outDir, { recursive: true });
      await page.screenshot({
        path: `${outDir}mode-${mode}-${theme.name}-${info.project.name}.png`,
        animations: 'disabled',
      });
    });
  }

  test(`réseau logique et avertissements, thème ${theme.name}`, async ({ bench }, info) => {
    const { page } = bench;
    await page.emulateMedia({ colorScheme: theme.scheme, reducedMotion: 'reduce' });
    await bench.emit('sim:status', { state: 'running', message: 'En cours' });
    await bench.emit('sim:states', { LD2: { brightness: 1 }, B1: { pressed: true }, echo: { pings: 0 } });
    await bench.emit('sim:nets', { PA5: 'high', PA0: 'floating', PA1: 'pulled-high', PB6: 'low', PA6: 'conflict' });
    await bench.emit('sim:warn', { code: 101, text: 'A0 (PA0) est lue flottante : ni résistance de tirage ni composant ne fixe son niveau. Activez GPIO_PULLUP ou GPIO_PULLDOWN, ou câblez la broche.' });
    await bench.emit('sim:warn', { code: 100, text: 'Conflit sur D12 (PA6) : deux sorties imposent des niveaux opposés (court-circuit). Retirez un fil ou passez une sortie en entrée.' });
    await page.getByRole('tab', { name: /Avertissements/ }).click();
    await page.locator('[data-endpoint="board.CN5:6"]').click();
    await page.mouse.move(5, 790);

    mkdirSync(outDir, { recursive: true });
    await page.screenshot({
      path: `${outDir}reseau-avertissements-${theme.name}-${info.project.name}.png`,
      animations: 'disabled',
    });
  });

  test(`moniteur série et traceur, thème ${theme.name}`, async ({ bench }, info) => {
    const { page } = bench;
    await page.emulateMedia({ colorScheme: theme.scheme, reducedMotion: 'reduce' });
    const b = (t: string) => Array.from(new TextEncoder().encode(t));
    await bench.emit('sim:status', { state: 'running', message: 'En cours' });
    await bench.emit('sim:uart-config', { id: 2, baud: 115200 });
    await bench.emit('sim:uart', { id: 2, data: b('Prebench : écho USART2 prêt\r\n') });
    for (let i = 0; i < 40; i++) {
      await bench.emit('sim:uart', { id: 2, data: b(`temp:${(21 + Math.sin(i / 4) * 2).toFixed(1)},hum:${55 + (i % 7)}\r\n`) });
    }
    await page.getByRole('button', { name: "Afficher l'horodatage" }).click();
    await page.mouse.move(5, 790);
    mkdirSync(outDir, { recursive: true });
    await page.screenshot({ path: `${outDir}moniteur-serie-${theme.name}-${info.project.name}.png`, animations: 'disabled' });
    await page.getByRole('tab', { name: 'Traceur' }).click();
    await page.mouse.move(5, 790);
    await page.screenshot({ path: `${outDir}traceur-${theme.name}-${info.project.name}.png`, animations: 'disabled' });
  });

  test(`inspecteur de broche, thème ${theme.name}`, async ({ bench }, info) => {
    const { page } = bench;
    await page.emulateMedia({ colorScheme: theme.scheme, reducedMotion: 'reduce' });
    await bench.emit('sim:status', { state: 'running', message: 'Nucleo-F401RE — stm32-hal, libvmcu 0.2.2' });
    await bench.emit('sim:states', { LD2: { brightness: 1 }, B1: { pressed: false }, echo: { pings: 0 } });
    await bench.emit('sim:pins', { PA5: { mode: 1, pull: 0, function: null, period_us: 1_000_000 } });
    await bench.emit('sim:nets', { PA5: 'high', PC13: 'pulled-high' });
    await bench.emit('sim:usage', { cpu: 2.4, memory: 2.6 * 1024 * 1024, memory_limit: 256 * 1024 * 1024 });
    await page.locator('[data-endpoint="board.CN5:6"]').click();
    await expect(page.getByRole('complementary', { name: 'Inspecteur' })).toContainText('Sortie push-pull');
    await page.mouse.move(5, 790);
    mkdirSync(outDir, { recursive: true });
    await page.screenshot({ path: `${outDir}inspecteur-${theme.name}-${info.project.name}.png`, animations: 'disabled' });
  });

  test(`PWM et ADC, thème ${theme.name}`, async ({ bench }, info) => {
    const { page } = bench;
    await page.emulateMedia({ colorScheme: theme.scheme, reducedMotion: 'reduce' });
    // Exemple tests/pot-servo : potentiomètre sur A0, servo sur D5 (50 Hz).
    const project = demoProject() as { instances: { id: string; pos: unknown }[] } & Record<string, unknown>;
    await bench.respond('open_project', {
      ...project,
      instances: [
        ...project.instances.filter((i) => i.pos === null),
        { id: 'pot1', type: 'potentiometer', params: { resistance_kohm: 10 }, pos: [-120, 216] },
        { id: 'servo1', type: 'servo', params: {}, pos: [392, 288] },
      ],
      wires: [
        { from: 'pot1.VCC', to: 'board.CN6:4' },
        { from: 'pot1.GND', to: 'board.CN6:6' },
        { from: 'pot1.W', to: 'board.CN8:1' },
        { from: 'servo1.VCC', to: 'board.CN6:5' },
        { from: 'servo1.GND', to: 'board.CN5:7' },
        { from: 'servo1.SIG', to: 'board.CN9:6' },
      ],
    });
    await bench.emit('project:changed', { project: true, views: [], models: [] });
    await page.getByRole('button', { name: 'Ajuster la vue' }).click();
    await bench.emit('sim:status', { state: 'running', message: 'Nucleo-F401RE — stm32-hal, libvmcu 0.2.2' });
    await bench.emit('sim:states', {
      LD2: { brightness: 0 },
      B1: { pressed: false },
      pot1: { position: 0.67, volts: 2.2 },
      servo1: { angle: 120, pulse_ms: 1.667, freq_hz: 50, status: 'ok' },
    });
    await bench.emit('sim:pins', {
      PA0: { mode: 5, pull: 0, function: null, period_us: null, pwm: null, adc: { volts: 2.2, raw: 2730, max: 4095 } },
      PB4: { mode: 3, pull: 0, function: 'pwm:TIM3_CH1', period_us: null, pwm: { freq_hz: 50, duty: 0.0833 }, adc: null },
    });
    await bench.emit('sim:nets', { PA0: 'floating', PB4: 'high', '3V3': 'high', '5V': 'high', GND: 'low' });
    await bench.emit('sim:usage', { cpu: 1.8, memory: 2.6 * 1024 * 1024, memory_limit: 256 * 1024 * 1024 });
    await page.locator('[data-endpoint="board.CN9:6"]').click();
    await expect(page.getByRole('complementary', { name: 'Inspecteur' })).toContainText('Rapport cyclique');
    await page.mouse.move(5, 790);
    mkdirSync(outDir, { recursive: true });
    await page.screenshot({ path: `${outDir}pwm-adc-${theme.name}-${info.project.name}.png`, animations: 'disabled' });
  });

  test(`alimentation coupée, thème ${theme.name}`, async ({ bench }, info) => {
    const { page } = bench;
    await page.emulateMedia({ colorScheme: theme.scheme, reducedMotion: 'reduce' });
    await bench.emit('sim:status', { state: 'running', message: 'En cours' });
    await bench.emit('sim:uart-config', { id: 2, baud: 115200 });
    await bench.emit('sim:uart', { id: 2, data: Array.from(new TextEncoder().encode('temp:21.4,hum:55\r\ntemp:21.5,hum:55\r\n')) });
    await bench.emit('sim:log', { source: 'prebench', level: 2, text: 'Alimentation coupée' });
    await bench.emit('sim:status', { state: 'off', message: "rétablissez l'alimentation pour relancer le firmware depuis zéro" });
    await bench.emit('sim:states', { LD2: { brightness: 0 }, B1: { pressed: false }, echo: { pings: 0 } });
    await bench.emit('sim:pins', {});
    await bench.emit('sim:nets', { PA5: 'low', PC13: 'floating' });
    await expect(page.getByRole('button', { name: "Rétablir l'alimentation" })).toBeVisible();
    await page.mouse.move(5, 790);
    mkdirSync(outDir, { recursive: true });
    await page.screenshot({ path: `${outDir}alimentation-coupee-${theme.name}-${info.project.name}.png`, animations: 'disabled' });
  });
}

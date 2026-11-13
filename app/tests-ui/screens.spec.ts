// app/tests-ui/screens.spec.ts — Captures de l'écran principal pour la revue visuelle.
//
// Écran principal en simulation (blinky ouvert, LD2 allumée, Journal rempli),
// en thème clair et sombre, pour chaque navigateur. Les PNG sont écrits dans
// app/test-screens/ et publiés comme artefact de la CI Linux. Aucune
// comparaison automatique : la revue est faite à l'œil, contre la maquette.

import { mkdirSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { FAILED_BUILD } from './data';
import { expect, test } from './fixture';

const outDir = fileURLToPath(new URL('../test-screens/', import.meta.url));

/** Texte réel du cœur (core/sim.rs, pause automatique). */
const CPU_GUARD_TEXT =
  "Simulation mise en pause : le firmware a occupé plus de 80 % d'un cœur pendant 5 s. " +
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
}

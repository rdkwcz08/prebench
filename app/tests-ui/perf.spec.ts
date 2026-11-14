// app/tests-ui/perf.spec.ts — Coût du rendu en simulation (section 11.1), mesure manuelle.
//
// Rejoue dans la vraie interface le flux d'événements que le cœur envoie pour
// la paillasse de tests/budget (enregistré par le test Rust
// budget_core_profile : app/src-tauri/target/budget-events.json), au même
// rythme, et mesure le temps CPU de tous les processus du navigateur
// (contenu, GPU…) pendant le rejeu. Chromium détaille aussi script, styles
// et mise en page.
//
//   cargo test --release --lib -- --ignored budget_core_profile   (dans app/src-tauri)
//   npm run build && PREBENCH_PERF=1 npx playwright test perf --workers=1
//
// Ignoré sans PREBENCH_PERF (CI comprise). Expériences : PREBENCH_PERF_SKIP
// (événements non rejoués, séparés par des virgules), PREBENCH_PERF_CSS
// (feuille de style ajoutée), PREBENCH_PERF_TAB (un seul onglet).

import { execSync } from 'node:child_process';
import { existsSync, readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { parse as parseToml } from 'smol-toml';
import { demoProject, expect, test } from './fixture';

const repo = fileURLToPath(new URL('../../', import.meta.url));
const EVENTS = `${repo}app/src-tauri/target/budget-events.json`;

/** Temps CPU cumulé (s) des processus des navigateurs de Playwright. */
function browserCpu(): number {
  const out = execSync('ps -A -o time=,command=', { encoding: 'utf8' });
  let total = 0;
  for (const line of out.split('\n')) {
    if (!line.includes('ms-playwright')) continue;
    const time = line.trim().split(/\s+/)[0] ?? '0';
    total += time.split(':').reduce((acc, part) => acc * 60 + Number(part.replace(',', '.')), 0);
  }
  return total;
}

/** Projet tests/budget : composants (paramètres par défaut) et fils de prebench.toml. */
function budgetProject(): Record<string, unknown> {
  const demo = demoProject() as Record<string, unknown> & {
    instances: { pos: unknown }[];
    components: Record<string, { manifest: { params?: Record<string, { default?: unknown }> } }>;
  };
  const toml = parseToml(readFileSync(`${repo}tests/budget/prebench.toml`, 'utf8')) as {
    components: { id: string; type: string; pos: [number, number] }[];
    wires: { from: string; to: string }[];
  };
  const placed = toml.components.map((c) => {
    const params: Record<string, unknown> = {};
    for (const [k, p] of Object.entries(demo.components[c.type]?.manifest.params ?? {})) params[k] = p.default;
    return { id: c.id, type: c.type, params, pos: c.pos };
  });
  return { ...demo, instances: [...demo.instances.filter((i) => i.pos === null), ...placed], wires: toml.wires };
}

const TABS = process.env.PREBENCH_PERF_TAB ? [process.env.PREBENCH_PERF_TAB] : ['Série', 'Traceur'];
const SKIP = (process.env.PREBENCH_PERF_SKIP ?? '').split(',').filter((x) => x !== '');

for (const tab of TABS) {
  test(`rendu de tests/budget, onglet ${tab}`, async ({ bench, browserName }) => {
    test.skip(!process.env.PREBENCH_PERF, 'mesure manuelle : PREBENCH_PERF=1');
    test.skip(!existsSync(EVENTS), 'flux absent : lancer budget_core_profile');
    test.setTimeout(120_000);
    const { page } = bench;
    // Expérience : cadence plafonnée comme sur batterie (réglage frame_rate_battery).
    const fps = Number(process.env.PREBENCH_PERF_FPS ?? 0);
    if (fps > 0) {
      await page.evaluate((f) => sessionStorage.setItem('__pbSettings', JSON.stringify({ frame_rate_battery: f })), fps);
      await page.addInitScript(() =>
        Object.defineProperty(navigator, 'getBattery', {
          value: () => Promise.resolve({ charging: false, addEventListener: () => {} }),
        }),
      );
      await page.reload();
      await page.waitForFunction(() =>
        (window as unknown as { __pbCalls: { cmd: string }[] }).__pbCalls.some((c) => c.cmd === 'open_project'),
      );
    }
    await bench.respond('open_project', budgetProject());
    await bench.emit('project:changed', { project: true, views: [], models: [] });
    await expect(page.locator('[data-component="servo1"]')).toHaveCount(1);
    await page.getByRole('tab', { name: tab }).click();
    await bench.emit('sim:status', { state: 'running', message: 'En cours' });

    if (process.env.PREBENCH_PERF_CSS) await page.addStyleTag({ content: process.env.PREBENCH_PERF_CSS });
    let list = (JSON.parse(readFileSync(EVENTS, 'utf8')) as { t: number; name: string }[]).filter(
      (e) => !SKIP.includes(e.name),
    );
    // PREBENCH_PERF_RATE : instants recalés pour obtenir ce nombre d'images
    // (sim:states) par seconde, et comparer deux mesures à cadence égale.
    const rate = Number(process.env.PREBENCH_PERF_RATE ?? 0);
    const frames = list.filter((e) => e.name === 'sim:states');
    const span = (frames.at(-1)?.t ?? 0) - (frames[0]?.t ?? 0);
    if (rate > 0 && frames.length > 1 && span > 0) {
      const scale = frames.length / (span / 1000) / rate;
      list = list.map((e) => ({ ...e, t: e.t * scale })).filter((e) => e.t <= 20_000);
    }
    const events = JSON.stringify(list);
    const cdp = browserName === 'chromium' ? await page.context().newCDPSession(page) : null;
    await cdp?.send('Performance.enable');
    const metrics = async (): Promise<Record<string, number>> =>
      Object.fromEntries(
        ((await cdp?.send('Performance.getMetrics'))?.metrics ?? []).map((m) => [m.name, m.value]),
      );
    const m0 = await metrics();
    const cpu0 = browserCpu();
    const t0 = Date.now();
    // Rejeu dans la page, au rythme enregistré.
    await page.evaluate(async (json) => {
      const list = JSON.parse(json) as { t: number; name: string; payload: unknown }[];
      const emit = (window as unknown as { __pbEmit: (e: string, p: unknown) => void }).__pbEmit;
      const start = performance.now();
      // Aucun événement gardé : mesure à vide sur 20 s (référence).
      if (list.length === 0) await new Promise((r) => setTimeout(r, 20_000));
      for (const e of list) {
        const wait = e.t - (performance.now() - start);
        if (wait > 0) await new Promise((r) => setTimeout(r, wait));
        emit(e.name, e.payload);
      }
    }, events);
    const seconds = (Date.now() - t0) / 1000;
    const cpu = ((browserCpu() - cpu0) / seconds) * 100;
    const m1 = await metrics();
    const share = (k: string): string => `${(((m1[k] ?? 0) - (m0[k] ?? 0)) / seconds * 100).toFixed(1)} %`;
    console.log(
      `${browserName}, onglet ${tab} : navigateur ${cpu.toFixed(1)} % d'un cœur sur ${seconds.toFixed(0)} s` +
        (cdp
          ? ` (tâches ${share('TaskDuration')}, script ${share('ScriptDuration')}, styles ${share('RecalcStyleDuration')}, mise en page ${share('LayoutDuration')})`
          : ''),
    );
  });
}

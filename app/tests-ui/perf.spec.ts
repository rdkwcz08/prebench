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
import { demoProject, expect, installTauriMock, responses, test } from './fixture';
import { mkdirSync, writeFileSync } from 'node:fs';

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
    // PREBENCH_PERF_PAINT : trace Chromium (événements Paint, comme l'onglet
    // Performance des outils de développement) : ce qui est réellement repeint.
    const tracePath = `${repo}app/test-results/perf-trace.json`;
    const tracing = cdp !== null && !!process.env.PREBENCH_PERF_PAINT;
    if (tracing) {
      await page.context().browser()!.startTracing(page, {
        path: tracePath,
        categories: [
          'devtools.timeline',
          'disabled-by-default-devtools.timeline',
          'disabled-by-default-devtools.timeline.invalidationTracking',
        ],
      });
    }
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
    if (tracing && cdp) {
      await page.context().browser()!.stopTracing();
      type Ev = { name: string; ts: number; dur?: number; args?: { data?: { nodeId?: number; clip?: number[]; layerId?: number } } };
      const events = (JSON.parse(readFileSync(tracePath, 'utf8')) as { traceEvents: Ev[] }).traceEvents;
      const byNode = new Map<number, { count: number; area: number; ms: number }>();
      let paintMs = 0;
      for (const e of events) {
        if (e.name !== 'Paint') continue;
        const d = e.args?.data;
        const q = d?.clip ?? [];
        const xs = [q[0] ?? 0, q[2] ?? 0, q[4] ?? 0, q[6] ?? 0];
        const ys = [q[1] ?? 0, q[3] ?? 0, q[5] ?? 0, q[7] ?? 0];
        const area = (Math.max(...xs) - Math.min(...xs)) * (Math.max(...ys) - Math.min(...ys));
        const k = d?.nodeId ?? -1;
        const p = byNode.get(k) ?? { count: 0, area: 0, ms: 0 };
        p.count++;
        p.area += area;
        p.ms += (e.dur ?? 0) / 1000;
        paintMs += (e.dur ?? 0) / 1000;
        byNode.set(k, p);
      }
      const count = (n: string): number => events.filter((e) => e.name === n).length;
      // Invalidations : quel élément, pour quelle raison (style, géométrie…).
      const inval = new Map<string, number>();
      for (const e of events) {
        if (e.name !== 'PaintInvalidationTracking' && e.name !== 'StyleRecalcInvalidationTracking') continue;
        const d = (e.args as { data?: { nodeName?: string; reason?: string } } | undefined)?.data;
        const k = `${e.name === 'PaintInvalidationTracking' ? 'peinture' : 'style'} : ${d?.nodeName ?? '?'} (${d?.reason ?? '?'})`;
        inval.set(k, (inval.get(k) ?? 0) + 1);
      }
      console.log(
        'Invalidations :\n' +
          [...inval].sort((a, b) => b[1] - a[1]).slice(0, 12).map(([k, n]) => `  ${(n / seconds).toFixed(1).padStart(6)}/s  ${k}`).join('\n'),
      );
      const rows: string[] = [];
      for (const [node, p] of [...byNode].sort((x, y) => y[1].area - x[1].area).slice(0, 8)) {
        let who = `nœud ${node}`;
        const d = node > 0 ? await cdp.send('DOM.describeNode', { backendNodeId: node }).catch(() => null) : null;
        if (d) {
          const attrs = d.node.attributes ?? [];
          const cls = attrs[attrs.indexOf('class') + 1];
          who = `${d.node.localName}${attrs.includes('class') && cls ? '.' + cls.split(' ').join('.') : ''}`;
        }
        rows.push(`  ${who.slice(0, 44).padEnd(44)} ${(p.count / seconds).toFixed(1).padStart(6)} /s ${Math.round(p.area / p.count).toString().padStart(9)} px² ${(p.ms / seconds).toFixed(2).padStart(6)} ms/s`);
      }
      console.log(
        `Repeintes : ${(byNode.size)} sources, peinture ${(paintMs / seconds).toFixed(1)} ms/s ; images composées ${(count('CompositeLayers') / seconds).toFixed(1)}/s, rasterisation ${(count('RasterTask') / seconds).toFixed(1)} tâches/s\n${rows.join('\n')}`,
      );
    }
    console.log(
      `${browserName}, onglet ${tab} : navigateur ${cpu.toFixed(1)} % d'un cœur sur ${seconds.toFixed(0)} s` +
        (cdp
          ? ` (tâches ${share('TaskDuration')}, script ${share('ScriptDuration')}, styles ${share('RecalcStyleDuration')}, mise en page ${share('LayoutDuration')})`
          : ''),
    );
  });
}

/**
 * Banc WKWebView (macOS) : écrit test-results/wk-init.js, injecté par
 * tests/budget/wkbanc.swift avant le chargement de l'interface (vite preview) :
 * imitation de l'IPC (projet tests/budget) et __pbReplay(), qui rejoue le flux
 * du cœur au même rythme. Le rendu est alors celui du vrai WKWebView de l'app.
 *   PREBENCH_PERF_EXPORT=1 npx playwright test perf -g export --project=chromium
 */
test('export du banc WKWebView', () => {
  test.skip(!process.env.PREBENCH_PERF_EXPORT, 'manuel : PREBENCH_PERF_EXPORT=1');
  const data = { ...responses(), open_project: budgetProject(), default_project: '/budget' };
  let list = (JSON.parse(readFileSync(EVENTS, 'utf8')) as { t: number; name: string }[]).filter(
    (e) => !SKIP.includes(e.name),
  );
  // Même recalage que plus haut (PREBENCH_PERF_RATE), sur 20 s au plus.
  const rate = Number(process.env.PREBENCH_PERF_RATE ?? 0);
  const frames = list.filter((e) => e.name === 'sim:states');
  const span = (frames.at(-1)?.t ?? 0) - (frames[0]?.t ?? 0);
  if (rate > 0 && frames.length > 1 && span > 0) {
    const scale = frames.length / (span / 1000) / rate;
    list = list.map((e) => ({ ...e, t: e.t * scale })).filter((e) => e.t <= 20_000);
  }
  // PREBENCH_PERF_FPS : cadence d'affichage, appliquée comme le fait le cœur
  // (une image par intervalle, qui regroupe tout ce qui a changé), puis par
  // l'interface (réglage frame_rate).
  const coreFps = Number(process.env.PREBENCH_PERF_FPS ?? 0);
  if (coreFps > 0) {
    type E = { t: number; name: string; payload: Record<string, unknown> };
    const step = 1000 / coreFps;
    const out: E[] = [];
    let bucket = new Map<string, E>();
    let end = step;
    const flush = (t: number): void => {
      for (const e of bucket.values()) out.push({ ...e, t });
      bucket = new Map();
    };
    for (const e of list as E[]) {
      while (e.t >= end) {
        flush(end);
        end += step;
      }
      const key = e.name === 'sim:uart' ? `uart${String(e.payload.id)}` : e.name;
      const prev = bucket.get(key);
      if (!prev || e.name === 'sim:pins') bucket.set(key, { ...e, payload: { ...e.payload } });
      else if (e.name === 'sim:uart')
        prev.payload.data = [...(prev.payload.data as number[]), ...(e.payload.data as number[])];
      else Object.assign(prev.payload, e.payload);
    }
    flush(end);
    list = out;
  }
  const fps = Number(process.env.PREBENCH_PERF_FPS ?? 0);
  const settingsInit = fps > 0 ? `sessionStorage.setItem('__pbSettings', JSON.stringify({ frame_rate: ${fps} }));` : '';
  const replay = `${settingsInit}
  window.__pbReplay = async () => {
    const list = ${JSON.stringify(list)};
    const emit = window.__pbEmit;
    emit('sim:status', { state: 'running', message: 'En cours' });
    const start = performance.now();
    for (const e of list) {
      const wait = e.t - (performance.now() - start);
      if (wait > 0) await new Promise((r) => setTimeout(r, wait));
      emit(e.name, e.payload);
    }
    return (performance.now() - start) / 1000;
  };
  ${process.env.PREBENCH_PERF_CSS ? `document.addEventListener('DOMContentLoaded', () => {
    const st = document.createElement('style');
    st.textContent = ${JSON.stringify(process.env.PREBENCH_PERF_CSS)};
    document.head.appendChild(st);
  });` : ''}`;
  mkdirSync(`${repo}app/test-results`, { recursive: true });
  writeFileSync(
    `${repo}app/test-results/wk-init.js`,
    `(${installTauriMock.toString()})(${JSON.stringify(JSON.stringify(data))});\n${replay}\n`,
  );
});

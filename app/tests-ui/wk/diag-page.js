// app/tests-ui/wk/diag-page.js — Relevé de la page dans le banc (BANC_DIAG=1).
// Même relevé que la version de diagnostic de l'app (src-tauri/src/diag.rs) :
// demandes d'image et modifications du DOM par élément, cumulées pendant le
// rejeu et recopiées dans window.__pbSent.diag (ligne ENVOIS du banc), pour
// comparer l'app et le banc à contenu égal.
(() => {
  const raf = window.requestAnimationFrame.bind(window);
  let total = 0;
  let fired = 0;
  let delay = 0;
  let frameMs = 0;
  let emitMs = 0;
  let emits = 0;
  window.requestAnimationFrame = (cb) => {
    total += 1;
    const asked = performance.now();
    return raf((t) => {
      fired += 1;
      const s0 = performance.now();
      delay += s0 - asked;
      cb(t);
      frameMs += performance.now() - s0;
    });
  };
  const touched = new Map();
  let mutations = 0;
  const label = (n) => {
    const el = n.nodeType === 1 ? n : n.parentElement;
    if (!el) return '?';
    const cls = (el.getAttribute('class') || '').split(' ').filter((c) => !c.startsWith('svelte-')).slice(0, 2).join('.');
    return el.tagName.toLowerCase() + (cls ? '.' + cls : '');
  };
  const observe = () => new MutationObserver((list) => {
    for (const m of list) {
      mutations += 1;
      const k = label(m.target) + (m.type === 'attributes' ? '@' + m.attributeName : m.type === 'characterData' ? '#texte' : '+enfants');
      touched.set(k, (touched.get(k) || 0) + 1);
    }
  }).observe(document.documentElement, { subtree: true, childList: true, attributes: true, characterData: true });
  if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', observe); else observe();
  // Script de chaque événement du cœur (sans le rendu).
  const wrapEmit = () => {
    const emit = window.__pbEmit;
    if (!emit || emit.__diag) return;
    const wrapped = (e, p) => {
      const s0 = performance.now();
      try { return emit(e, p); } finally { emitMs += performance.now() - s0; emits += 1; }
    };
    wrapped.__diag = true;
    window.__pbEmit = wrapped;
  };
  const start = performance.now();
  setInterval(() => {
    wrapEmit();
    if (!window.__pbSent) return;
    const s = (performance.now() - start) / 1000;
    const top = [...touched].sort((a, b) => b[1] - a[1]).slice(0, 8).map(([k, v]) => (v / s).toFixed(1) + ' ' + k).join(' | ');
    window.__pbSent.diag = 'rAF ' + (total / s).toFixed(1) + '/s (' + (fired / s).toFixed(1) + ' exécutées, ' + (delay / Math.max(fired, 1)).toFixed(0) + ' ms après la demande, script ' + (frameMs / Math.max(fired, 1)).toFixed(2) + ' ms par image), événements ' + (emits / s).toFixed(1) + '/s à ' + (emitMs / Math.max(emits, 1)).toFixed(2) + ' ms, ' + document.visibilityState + (document.hasFocus() ? ', active' : ', inactive') + ', DOM ' + (mutations / s).toFixed(0) + '/s : ' + top;
  }, 1000);
})();

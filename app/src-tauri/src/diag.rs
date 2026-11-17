// app/src-tauri/src/diag.rs — Version de diagnostic (feature Cargo « diag »).
//
// Jamais dans un build publié : release.yml vérifie que le binaire ne contient
// pas PROJECT_VAR. Build release (performances réelles) avec :
//   - Web Inspector (tauri/devtools : clic droit, « Inspecter l'élément ») ;
//   - PREBENCH_DIAG_PROJET=<dossier> : projet ouvert au lancement, et avec
//     PREBENCH_DIAG_DEMARRER=1, simulation démarrée aussitôt ;
//   - relevé des fils dans le Journal, toutes les 5 s : processus de l'app,
//     fil « modèles » et commandes reçues par ce fil.
// Usage (macOS) :
//   npm run tauri build -- --features diag --bundles app
//   PREBENCH_DIAG_PROJET=$PWD/../tests/budget PREBENCH_DIAG_DEMARRER=1 \
//     src-tauri/target/release/bundle/macos/Prebench.app/Contents/MacOS/prebench

/// Variable du projet ouvert au lancement.
pub const PROJECT_VAR: &str = "PREBENCH_DIAG_PROJET";
/// Variable du démarrage automatique de la simulation (« 1 »).
pub const START_VAR: &str = "PREBENCH_DIAG_DEMARRER";
/// Intervalle du relevé des fils.
pub const REPORT_EVERY: std::time::Duration = std::time::Duration::from_secs(5);

/// Projet à ouvrir au lancement.
pub fn project() -> Option<String> {
    std::env::var(PROJECT_VAR).ok().filter(|p| !p.is_empty())
}

/// Démarrer la simulation de ce projet dès son ouverture ?
pub fn autostart(path: &str) -> bool {
    std::env::var(START_VAR).is_ok_and(|v| v == "1") && project().as_deref() == Some(path)
}

/// Composant fictif par lequel le script de la page envoie son bilan
/// (commande component_control existante : aucune permission ajoutée).
pub const PAGE_COMPONENT: &str = "__diag";

/// Bilan de la page : sortie d'erreur et Journal (même forme que les
/// relevés des fils, source « prebench »).
pub fn page_report<R: tauri::Runtime>(app: &tauri::AppHandle<R>, text: &str) {
    use tauri::Emitter;
    let line = format!("diag : page {text}");
    eprintln!("{line}");
    let _ = app.emit(
        "sim:log",
        serde_json::json!({ "source": "prebench", "level": 2, "text": line }),
    );
}

/// Script injecté dans la page : toutes les 5 s, appels à requestAnimationFrame
/// (par appelant) et modifications du DOM (par élément), vers la sortie
/// d'erreur de l'app. Ne programme lui-même aucune image.
pub const PAGE_SCRIPT: &str = r#"
(() => {
  const raf = window.requestAnimationFrame.bind(window);
  const calls = new Map();
  let total = 0;
  let fired = 0;
  window.requestAnimationFrame = (cb) => {
    total += 1;
    const line = (new Error().stack || '').split('\n')[1] || '?';
    const key = line.replace(/^.*\//, '').slice(0, 60);
    calls.set(key, (calls.get(key) || 0) + 1);
    return raf((t) => { fired += 1; cb(t); });
  };
  const touched = new Map();
  const label = (n) => {
    const el = n.nodeType === 1 ? n : n.parentElement;
    if (!el) return '?';
    const cls = (el.getAttribute('class') || '').split(' ').filter((c) => !c.startsWith('svelte-')).slice(0, 2).join('.');
    return el.tagName.toLowerCase() + (cls ? '.' + cls : '');
  };
  let mutations = 0;
  const observe = () => new MutationObserver((list) => {
    for (const m of list) {
      mutations += 1;
      const k = label(m.target) + (m.type === 'attributes' ? '@' + m.attributeName : m.type === 'characterData' ? '#texte' : '+enfants');
      touched.set(k, (touched.get(k) || 0) + 1);
    }
  }).observe(document.documentElement, { subtree: true, childList: true, attributes: true, characterData: true });
  if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', observe); else observe();
  const top = (m) => [...m].sort((a, b) => b[1] - a[1]).slice(0, 8).map(([k, v]) => v + ' ' + k).join(' | ');
  setInterval(() => {
    const text = document.visibilityState + (document.hasFocus() ? ', active' : ', inactive') + ' ; rAF ' + total + ' en 5 s (' + fired + ' exécutées) : ' + top(calls) + ' || DOM ' + mutations + ' : ' + top(touched);
    calls.clear(); touched.clear(); total = 0; fired = 0; mutations = 0;
    window.__TAURI_INTERNALS__?.invoke('component_control', { component: '__diag', control: 'page', value: text }).catch(() => {});
  }, 5000);
})();
"#;

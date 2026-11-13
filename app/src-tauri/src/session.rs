// app/src-tauri/src/session.rs — Surveillance du dossier du projet ouvert (section 10.2, D9).
//
// L'utilisateur garde son éditeur : à chaque sauvegarde, l'app détecte le
// changement et réagit. Purement événementiel (règle 16.2.9) : le système
// signale les écritures (FSEvents, ReadDirectoryChangesW, inotify via la crate
// `notify`), rien ne tourne au repos. Les changements rapprochés (un « Tout
// enregistrer », un éditeur qui écrit puis renomme) sont regroupés pendant
// DEBOUNCE, puis traités par commands::on_project_changed :
//   - sources du firmware (.c, .h, CMakeLists.txt…) : build « sim », puis
//     rechargement du firmware si la simulation tourne ;
//   - components/<id>/view.svelte : rechargement de la vue dans l'interface ;
//   - components/<id>/model.ts ou manifest.json, prebench.toml : projet
//     relu, simulation relancée si elle tourne.
// Dossiers ignorés : cachés (.git, .vscode…) et dossiers de build.
// Une source n'est retenue que si sa date de modification a changé : un
// événement sans écriture (métadonnées, lecture par l'éditeur) ne lance aucun
// build. L'app ne modifie jamais les fichiers du projet.

use std::collections::{BTreeSet, HashMap};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use notify::{EventKind, RecursiveMode, Watcher};
use serde::Serialize;
use tauri::{AppHandle, Runtime};
use tokio::sync::mpsc;

use crate::error::{CoreError, CoreResult};

/// Regroupement des changements rapprochés.
const DEBOUNCE: Duration = Duration::from_millis(300);
/// File bornée (règle 16.2.11) ; au-delà, tout est considéré comme modifié.
const WATCH_QUEUE: usize = 256;
/// Sources modifiées retenues par rafale.
const MAX_FILES: usize = 256;
/// Dates de modification connues des sources (règle 16.2.11).
const MAX_KNOWN: usize = 20_000;

/// Extensions des sources du firmware.
const SOURCE_EXT: &[&str] = &[
    "c", "h", "cc", "cpp", "cxx", "hpp", "hh", "hxx", "s", "S", "inc", "cmake",
];

/// Nature d'un fichier modifié.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Change {
    Firmware,
    View(String),
    Model(String),
    Project,
}

/// Changements regroupés d'une rafale de sauvegardes.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Changes {
    pub firmware: bool,
    /// Composants du projet dont la vue a changé.
    pub views: BTreeSet<String>,
    /// Composants du projet dont le modèle ou le manifeste a changé.
    pub models: BTreeSet<String>,
    /// prebench.toml modifié.
    pub project: bool,
    /// Sources du firmware modifiées.
    #[serde(skip)]
    pub files: BTreeSet<PathBuf>,
}

impl Changes {
    fn add(&mut self, (c, path): (Change, PathBuf)) {
        match c {
            Change::Firmware => {
                self.firmware = true;
                if self.files.len() < MAX_FILES {
                    self.files.insert(path);
                }
            }
            Change::View(id) => {
                self.views.insert(id);
            }
            Change::Model(id) => {
                self.models.insert(id);
            }
            Change::Project => self.project = true,
        }
    }

    /// Le projet (carte, composants, câblage) doit être relu.
    pub fn needs_reload(&self) -> bool {
        self.project || !self.models.is_empty()
    }
}

/// Classe un chemin modifié du projet `root` ; None : sans effet.
pub fn classify(root: &Path, path: &Path) -> Option<Change> {
    let rel = path.strip_prefix(root).ok()?;
    let parts: Vec<String> = rel
        .components()
        .map(|c| match c {
            Component::Normal(s) => Some(s.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect::<Option<Vec<_>>>()?;
    let (name, dirs) = parts.split_last()?;
    if dirs.iter().any(|d| ignored_dir(d)) || name.starts_with('.') {
        return None;
    }
    if dirs.is_empty() && name == "prebench.toml" {
        return Some(Change::Project);
    }
    if dirs.is_empty() && name == "components" {
        // Dossier components/ créé ou supprimé (même raison qu'un dossier de
        // composant, ci-dessous) : le projet est relu.
        return Some(Change::Project);
    }
    if dirs.len() == 1 && dirs[0] == "components" && Path::new(name).extension().is_none() {
        // Dossier de composant créé ou supprimé : sous Linux (inotify), un
        // fichier écrit aussitôt dedans peut échapper à la surveillance, qui
        // n'y est posée qu'après l'annonce du dossier. Le projet est relu.
        return Some(Change::Model(name.clone()));
    }
    if dirs.first().map(String::as_str) == Some("components") {
        let id = dirs.get(1)?.clone();
        if dirs.len() != 2 {
            return None;
        }
        return match name.as_str() {
            "view.svelte" => Some(Change::View(id)),
            "model.ts" | "manifest.json" => Some(Change::Model(id)),
            _ => None,
        };
    }
    let ext = Path::new(name).extension().map(|e| e.to_string_lossy());
    let source = ext.is_some_and(|e| SOURCE_EXT.contains(&e.as_ref()));
    (source || name == "CMakeLists.txt").then_some(Change::Firmware)
}

/// Dossiers cachés et dossiers de build (build, build-sim, cmake-build-debug…).
fn ignored_dir(name: &str) -> bool {
    name.starts_with('.')
        || name == "build"
        || name.starts_with("build-")
        || name.starts_with("cmake-build-")
}

/// Surveillance active d'un projet ; arrêtée quand la session est abandonnée.
pub struct Session {
    pub dir: PathBuf,
    _watcher: notify::RecommendedWatcher,
    task: tauri::async_runtime::JoinHandle<()>,
}

impl Drop for Session {
    fn drop(&mut self) {
        self.task.abort();
    }
}

/// Démarre la surveillance de `dir` ; chaque rafale de changements est
/// confiée à `on_change`.
pub fn start<R, F, Fut>(app: AppHandle<R>, dir: PathBuf, on_change: F) -> CoreResult<Session>
where
    R: Runtime,
    F: Fn(AppHandle<R>, PathBuf, Changes) -> Fut + Send + 'static,
    Fut: std::future::Future<Output = ()> + Send,
{
    // macOS signale les chemins canoniques (/private/var… pour /var…).
    let canonical = dunce::canonicalize(&dir).unwrap_or_else(|_| dir.clone());
    let (tx, mut rx) = mpsc::channel::<(Change, PathBuf)>(WATCH_QUEUE);
    let overflow = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&overflow);
    let (root_a, root_b) = (canonical.clone(), dir.clone());
    let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        let Ok(event) = res else { return };
        if matches!(event.kind, EventKind::Access(_)) {
            return;
        }
        for path in &event.paths {
            let change = classify(&root_a, path).or_else(|| classify(&root_b, path));
            if let Some(c) = change {
                if tx.try_send((c, path.clone())).is_err() {
                    flag.store(true, Ordering::Relaxed);
                }
            }
        }
    })
    .map_err(|e| CoreError::Watch(e.to_string()))?;
    watcher
        .watch(&canonical, RecursiveMode::Recursive)
        .map_err(|e| CoreError::Watch(e.to_string()))?;

    let project = dir.clone();
    let task = tauri::async_runtime::spawn(async move {
        let root = canonical.clone();
        let mut known = tauri::async_runtime::spawn_blocking(move || scan_sources(&root))
            .await
            .unwrap_or_default();
        while let Some(first) = rx.recv().await {
            let mut changes = Changes::default();
            changes.add(first);
            loop {
                tokio::select! {
                    () = tokio::time::sleep(DEBOUNCE) => break,
                    next = rx.recv() => match next {
                        Some(c) => changes.add(c),
                        None => return,
                    },
                }
            }
            if overflow.swap(false, Ordering::Relaxed) {
                changes.firmware = true;
                changes.project = true;
            } else if changes.firmware {
                keep_modified(&mut changes, &mut known);
            }
            if changes == Changes::default() {
                continue; // aucun fichier réellement modifié
            }
            on_change(app.clone(), project.clone(), changes).await;
        }
    });
    Ok(Session {
        dir,
        _watcher: watcher,
        task,
    })
}

fn mtime(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

/// Ne garde que les sources dont la date de modification a changé.
fn keep_modified(changes: &mut Changes, known: &mut HashMap<PathBuf, Option<SystemTime>>) {
    changes.files.retain(|f| {
        let now = mtime(f);
        let changed = known.get(f) != Some(&now);
        if known.len() < MAX_KNOWN || known.contains_key(f) {
            known.insert(f.clone(), now);
        }
        changed
    });
    changes.firmware = !changes.files.is_empty();
}

/// Dates de modification des sources du projet à l'ouverture.
fn scan_sources(root: &Path) -> HashMap<PathBuf, Option<SystemTime>> {
    let mut known = HashMap::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            if known.len() >= MAX_KNOWN {
                return known;
            }
            let path = entry.path();
            let is_dir = entry.file_type().is_ok_and(|t| t.is_dir());
            if is_dir {
                let name = entry.file_name();
                if !ignored_dir(&name.to_string_lossy()) {
                    stack.push(path);
                }
            } else if classify(root, &path) == Some(Change::Firmware) {
                known.insert(path.clone(), mtime(&path));
            }
        }
    }
    known
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classification() {
        let root = Path::new("/p");
        let c = |rel: &str| classify(root, &root.join(rel));
        assert_eq!(c("Core/Src/main.c"), Some(Change::Firmware));
        assert_eq!(c("Core/Inc/main.h"), Some(Change::Firmware));
        assert_eq!(c("CMakeLists.txt"), Some(Change::Firmware));
        assert_eq!(c("cmake/gcc.cmake"), Some(Change::Firmware));
        assert_eq!(c("prebench.toml"), Some(Change::Project));
        assert_eq!(
            c("components/pot/view.svelte"),
            Some(Change::View("pot".into()))
        );
        assert_eq!(
            c("components/pot/model.ts"),
            Some(Change::Model("pot".into()))
        );
        assert_eq!(
            c("components/pot/manifest.json"),
            Some(Change::Model("pot".into()))
        );
        assert_eq!(c("components/component-api.d.ts"), None);
        assert_eq!(c("components/probe"), Some(Change::Model("probe".into())));
        assert_eq!(c("components"), Some(Change::Project));
        assert_eq!(c("components/pot/notes.md"), None);
        assert_eq!(c("build/main.c.o"), None);
        assert_eq!(c("build-sim/x.c"), None);
        assert_eq!(c("cmake-build-debug/a.c"), None);
        assert_eq!(c(".git/index"), None);
        assert_eq!(c("Core/Src/.main.c.swp"), None);
        assert_eq!(c("README.md"), None);
        assert_eq!(c("sub/prebench.toml"), None);
        assert_eq!(classify(root, Path::new("/other/main.c")), None);
    }
}

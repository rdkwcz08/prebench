// app/src-tauri/src/commands.rs — Commandes du cœur appelables par l'interface.
//
// Liste fermée (build.rs, capabilities/default.json) : l'interface ne peut rien
// faire d'autre (section 11.4).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, Runtime, State};
use tokio::sync::Mutex;

use crate::build;
use crate::build::diagnostics::{self, Diagnostic, Severity};
use crate::core::board::Board;
use crate::core::components::engine::InstanceSpec;
use crate::core::components::{sync_api_declarations, Catalog, ComponentSource};
use crate::core::sim::{self, DebugConfig, SimConfig, SimHandle, SimMsg};
use crate::debug;
use crate::error::{CoreError, CoreResult};
use crate::platform::paths::simplified;
use crate::platform::toolchain::{ToolKind, Toolchain};
use crate::project::{templates, Project};
use crate::session::{self, Changes, Session};

/// Une seule carte en phase 1 (multi-cartes : E2, V2).
const BOARD_ID: u16 = 1;

#[derive(Default)]
pub struct AppState {
    sim: Mutex<Option<SimHandle>>,
    /// Surveillance du projet ouvert (section 10.2).
    session: Mutex<Option<Session>>,
    /// Un seul build à la fois : démarrage et sauvegardes partagent le dossier de build.
    build_lock: Mutex<()>,
    /// Fin du dernier build : voir build::invalidate_stale_objects.
    last_build_end: std::sync::Mutex<Option<std::time::SystemTime>>,
}

/// Lignes de la sortie d'un build en échec recopiées dans le Journal.
const MAX_BUILD_LOG_LINES: usize = 200;

/// Racine contenant libvmcu/, boards/, components/, templates/ : le dépôt
/// Prebench en développement (les modifications sont prises en compte sans
/// recompiler l'app), les ressources « sdk/ » de l'app installée sinon.
/// Chemin simplifié : Windows renvoie le dossier des ressources au format
/// \\?\, illisible pour make (platform/paths.rs).
pub(crate) fn prebench_root<R: Runtime>(app: &AppHandle<R>) -> PathBuf {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
    if cfg!(debug_assertions) && repo.join("libvmcu").is_dir() {
        return repo;
    }
    if let Ok(resources) = app.path().resource_dir() {
        let root = simplified(&resources).join("sdk");
        if root.join("libvmcu").is_dir() {
            return root;
        }
    }
    repo
}

/// Cache des builds « sim » : ~/Library/Caches/dev.prebench.app (macOS),
/// %LOCALAPPDATA%\dev.prebench.app (Windows), ~/.cache/dev.prebench.app (Linux).
fn cache_dir<R: Runtime>(app: &AppHandle<R>) -> CoreResult<PathBuf> {
    app.path()
        .app_cache_dir()
        .map(|d| simplified(&d))
        .map_err(|e| CoreError::Build(format!("dossier de cache introuvable : {e}")))
}

/// Dossier proposé pour les nouveaux projets : Documents/Prebench.
fn projects_parent<R: Runtime>(app: &AppHandle<R>) -> PathBuf {
    app.path()
        .document_dir()
        .or_else(|_| app.path().home_dir())
        .map(|d| simplified(&d).join("Prebench"))
        .unwrap_or_else(|_| PathBuf::from("Prebench"))
}

#[derive(Serialize)]
pub struct InstanceView {
    id: String,
    #[serde(rename = "type")]
    kind: String,
    params: serde_json::Map<String, serde_json::Value>,
    /// Position sur la paillasse ; absente pour un composant intégré à la carte.
    pos: Option<[f64; 2]>,
}

#[derive(Serialize)]
pub struct ComponentView {
    #[serde(flatten)]
    source: ComponentSource,
    /// Source de view.svelte, compilée par l'interface ; None : vue de repli.
    view_source: Option<String>,
}

#[derive(Serialize)]
pub struct ProjectView {
    path: String,
    framework: String,
    board_name: String,
    mcu: String,
    instances: Vec<InstanceView>,
    components: BTreeMap<String, ComponentView>,
    warnings: Vec<String>,
}

pub(crate) struct Resolved {
    project: Project,
    board: Board,
    catalog: Catalog,
    instances: Vec<(InstanceView, Option<ComponentSource>)>,
}

/// Projet, carte, catalogue et instances (composants intégrés puis du projet).
fn resolve(root: &Path, dir: &Path) -> CoreResult<Resolved> {
    let project = Project::load(dir)?;
    let board = Board::load(&root.join("boards"), &project.board)?;
    let mut catalog = Catalog::load(&root.join("components"), dir);
    let mut instances = Vec::new();
    let onboard = board.onboard.iter().map(|(id, o)| {
        let params = o
            .params
            .iter()
            .map(|(k, v)| (k.clone(), toml_to_json(v)))
            .collect();
        (id.clone(), o.kind.clone(), params, None)
    });
    let placed = project.components.iter().map(|c| {
        let params = c
            .params
            .iter()
            .map(|(k, v)| (k.clone(), toml_to_json(v)))
            .collect();
        (c.id.clone(), c.kind.clone(), params, Some(c.pos))
    });
    for (id, kind, given, pos) in onboard.chain(placed).collect::<Vec<_>>() {
        let source = catalog.components.get(&kind).cloned();
        if source.is_none() {
            catalog
                .warnings
                .push(format!("{id} : composant « {kind} » introuvable"));
        }
        let params = source
            .as_ref()
            .map(|s| s.params_with_defaults(&given))
            .unwrap_or_else(|| given.into_iter().collect());
        instances.push((
            InstanceView {
                id,
                kind,
                params,
                pos,
            },
            source,
        ));
    }
    Ok(Resolved {
        project,
        board,
        catalog,
        instances,
    })
}

fn toml_to_json(value: &toml::Value) -> serde_json::Value {
    serde_json::to_value(value).unwrap_or(serde_json::Value::Null)
}

/// Modèle de « Créer mon premier projet ».
const FIRST_TEMPLATE: &str = "blinky-stm32-hal";

/// Projet ouvert au démarrage : la copie du blinky dans Documents/Prebench.
/// Sans copie, None : l'interface propose « Créer mon premier projet » plutôt
/// que d'ouvrir le modèle installé, non modifiable.
#[tauri::command]
pub fn default_project(app: AppHandle) -> Option<String> {
    let copy = projects_parent(&app).join(FIRST_TEMPLATE);
    copy.join("prebench.toml")
        .is_file()
        .then(|| copy.display().to_string())
}

#[derive(Serialize)]
pub struct TemplateView {
    id: String,
    /// Dossier proposé pour la copie (Documents/Prebench/<id>, libre).
    suggested_dir: String,
}

/// Modèles de nouveaux projets (section 10.1).
#[tauri::command]
pub fn list_templates(app: AppHandle) -> Vec<TemplateView> {
    let parent = projects_parent(&app);
    templates::list(&prebench_root(&app).join("templates"))
        .into_iter()
        .map(|id| TemplateView {
            suggested_dir: templates::suggested_dir(&parent, &id).display().to_string(),
            id,
        })
        .collect()
}

/// Copie un modèle dans un dossier de l'utilisateur ; renvoie son chemin.
#[tauri::command]
pub fn create_project(app: AppHandle, template: String, dest: String) -> CoreResult<String> {
    let dest = simplified(Path::new(&dest));
    templates::create(&prebench_root(&app).join("templates"), &template, &dest)?;
    Ok(dest.display().to_string())
}

/// Ouvre un projet et surveille son dossier (section 10.2). Rappelée par
/// l'interface après un changement de composant ou de prebench.toml.
#[tauri::command]
pub async fn open_project(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
) -> CoreResult<ProjectView> {
    let root = prebench_root(&app);
    let dir = simplified(Path::new(&path));
    let view = project_view(&root, &dir, path)?;
    let mut session = state.session.lock().await;
    if session.as_ref().map(|s| s.dir.as_path()) != Some(dir.as_path()) {
        *session = None; // arrête la surveillance du projet précédent
        *session = Some(session::start(app.clone(), dir, on_project_changed)?);
    }
    Ok(view)
}

fn project_view(root: &Path, dir: &Path, path: String) -> CoreResult<ProjectView> {
    let mut r = resolve(root, dir)?;
    if sync_api_declarations(&root.join("components"), dir)? {
        r.catalog
            .warnings
            .push("components/component-api.d.ts mis à jour dans le projet".into());
    }
    let used: Vec<String> = r.instances.iter().map(|(i, _)| i.kind.clone()).collect();
    let components = r
        .catalog
        .components
        .into_iter()
        .filter(|(id, _)| used.contains(id))
        .map(|(id, source)| {
            let view_source = std::fs::read_to_string(source.view_path()).ok();
            (
                id,
                ComponentView {
                    source,
                    view_source,
                },
            )
        })
        .collect();
    Ok(ProjectView {
        path,
        framework: r.project.framework,
        board_name: r.board.name,
        mcu: r.board.mcu,
        instances: r.instances.into_iter().map(|(i, _)| i).collect(),
        components,
        warnings: r.catalog.warnings,
    })
}

/// Prérequis de build vérifié (outil trouvé ou commande pour l'installer).
#[derive(Serialize)]
pub struct PrerequisiteView {
    kind: ToolKind,
    label: &'static str,
    path: Option<String>,
    install: &'static str,
}

/// Résultat de check_prerequisites, affiché au démarrage de l'app.
#[derive(Serialize)]
pub struct PrerequisitesView {
    tools: Vec<PrerequisiteView>,
    readme: &'static str,
}

fn prerequisites_view(toolchain: &Toolchain) -> PrerequisitesView {
    PrerequisitesView {
        tools: toolchain
            .tools
            .iter()
            .map(|t| PrerequisiteView {
                kind: t.kind,
                label: t.label,
                path: t.path.as_ref().map(|p| p.display().to_string()),
                install: t.install,
            })
            .collect(),
        readme: build::README_PREREQUISITES,
    }
}

/// Recherche CMake et le compilateur (PATH, puis emplacements standard).
#[tauri::command]
pub async fn check_prerequisites() -> CoreResult<PrerequisitesView> {
    let toolchain = tauri::async_runtime::spawn_blocking(Toolchain::detect)
        .await
        .map_err(|e| CoreError::Build(format!("vérification des prérequis : {e}")))?;
    Ok(prerequisites_view(&toolchain))
}

#[derive(Clone, Serialize)]
struct LogEvent {
    source: &'static str,
    level: u8,
    text: String,
}

#[derive(Clone, Serialize)]
struct StatusEvent {
    state: &'static str,
    message: String,
}

#[tauri::command]
pub async fn start_simulation(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
) -> CoreResult<()> {
    let mut slot = state.sim.lock().await;
    if slot.as_ref().is_some_and(SimHandle::is_alive) {
        return Err(CoreError::AlreadyRunning);
    }
    *slot = Some(launch(&app, Path::new(&path)).await?);
    Ok(())
}

/// Build « sim » dans le cache, puis lancement de la simulation. Partagé par
/// ▶ Démarrer et la vérification de l'app installée (install_check.rs).
pub(crate) async fn launch<R: Runtime>(app: &AppHandle<R>, dir: &Path) -> CoreResult<SimHandle> {
    let root = prebench_root(app);
    let dir = simplified(dir);
    resolve(&root, &dir)?; // prebench.toml et carte valides avant de compiler

    let _ = app.emit(
        "sim:status",
        StatusEvent {
            state: "building",
            message: "build « sim » en cours…".into(),
        },
    );
    // Détection à chaque démarrage : un outil installé pendant que l'app est
    // ouverte est pris en compte sans la relancer.
    let toolchain = tauri::async_runtime::spawn_blocking(Toolchain::detect)
        .await
        .map_err(|e| CoreError::Build(format!("recherche des outils : {e}")))?;
    for tool in &toolchain.tools {
        let text = match &tool.path {
            Some(path) => format!("{} : {}", tool.label, path.display()),
            None => format!("{} introuvable", tool.label),
        };
        let _ = app.emit(
            "sim:log",
            LogEvent {
                source: "prebench",
                level: 2,
                text,
            },
        );
    }
    let _ = app.emit(
        "sim:log",
        LogEvent {
            source: "prebench",
            level: 2,
            text: format!(
                "dossier de build : {}",
                build::build_dir_for(&cache_dir(app)?, &dir).display()
            ),
        },
    );
    let firmware = build_project(app, &root, &dir, &toolchain).await?.firmware;
    start_sim(app, &root, &dir, firmware, None)
}

/// Lance la simulation d'un firmware déjà compilé.
fn start_sim<R: Runtime>(
    app: &AppHandle<R>,
    root: &Path,
    dir: &Path,
    firmware: PathBuf,
    debug: Option<DebugConfig>,
) -> CoreResult<SimHandle> {
    let r = resolve(root, dir)?;
    let components = instance_specs(&r);
    let _ = app.emit(
        "sim:status",
        StatusEvent {
            state: "starting",
            message: "lancement du firmware…".into(),
        },
    );

    Ok(sim::start(
        app.clone(),
        SimConfig {
            project_dir: dir.to_path_buf(),
            project: r.project,
            board: r.board,
            firmware,
            board_id: BOARD_ID,
            components,
            debug,
        },
    ))
}

#[derive(Clone, Serialize)]
struct BuildStateEvent {
    state: &'static str,
}

/// Résultat d'un build, pour l'onglet Build et la barre d'état.
#[derive(Clone, Serialize)]
struct BuildResultEvent {
    ok: bool,
    duration_ms: u64,
    errors: usize,
    warnings: usize,
    diagnostics: Vec<Diagnostic>,
}

/// Firmware prêt à lancer.
pub(crate) struct Built {
    /// Copie de l'exécutable à lancer (build::run_copy).
    pub firmware: PathBuf,
    /// L'exécutable a été réécrit : sinon, inutile de recharger le firmware.
    pub changed: bool,
}

/// Build « sim » d'un projet, un seul à la fois. Annonce « build:state » puis
/// « build:result » (erreurs et avertissements cliquables).
pub(crate) async fn build_project<R: Runtime>(
    app: &AppHandle<R>,
    root: &Path,
    dir: &Path,
    toolchain: &Toolchain,
) -> CoreResult<Built> {
    let state = app.state::<AppState>();
    let _one_at_a_time = state.build_lock.lock().await;
    let _ = app.emit("build:state", BuildStateEvent { state: "building" });
    let started = std::time::Instant::now();
    let build_dir = build::build_dir_for(&cache_dir(app)?, dir);
    let stamp = || {
        std::fs::metadata(build_dir.join(crate::platform::firmware_file_name()))
            .and_then(|m| m.modified())
            .ok()
    };
    let before = stamp();
    let result = build::build_sim_logged(dir, root, &build_dir, toolchain).await;
    let duration_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    if let Ok(mut end) = state.last_build_end.lock() {
        *end = Some(std::time::SystemTime::now());
    }
    let (diagnostics, outcome) = match result {
        Ok((exe, log)) => {
            configure_intellisense(app, dir, &build_dir);
            (
                diagnostics::parse(&log, dir),
                build::run_copy(&exe, &build_dir).map(|firmware| Built {
                    firmware,
                    changed: before.is_none() || stamp() != before,
                }),
            )
        }
        Err(CoreError::Build(log)) => {
            let diags = diagnostics::parse(&log, dir);
            let lines: Vec<&str> = log.lines().collect();
            for line in &lines[lines.len().saturating_sub(MAX_BUILD_LOG_LINES)..] {
                let _ = app.emit(
                    "sim:log",
                    LogEvent {
                        source: "build",
                        level: 2,
                        text: (*line).to_string(),
                    },
                );
            }
            let errors = count(&diags, Severity::Error);
            let err = if errors > 0 {
                CoreError::BuildFailed { errors }
            } else {
                CoreError::Build(log)
            };
            (diags, Err(err))
        }
        Err(e) => (Vec::new(), Err(e)),
    };
    let _ = app.emit(
        "build:result",
        BuildResultEvent {
            ok: outcome.is_ok(),
            duration_ms,
            errors: count(&diagnostics, Severity::Error),
            warnings: count(&diagnostics, Severity::Warning),
            diagnostics,
        },
    );
    outcome
}

/// IntelliSense de VS Code : compile_commands.json du build « sim » inscrit
/// dans .vscode/settings.json (dossier caché, non surveillé). Une erreur
/// n'empêche pas la simulation : elle est écrite dans le Journal.
fn configure_intellisense<R: Runtime>(app: &AppHandle<R>, dir: &Path, build_dir: &Path) {
    let cc = build::compile_commands(build_dir);
    if !cc.is_file() {
        return;
    }
    match debug::upsert_settings(dir, &cc) {
        Ok(true) => log(
            app,
            format!(
                "IntelliSense : {} inscrit dans .vscode/settings.json ({})",
                cc.display(),
                debug::COMPILE_COMMANDS_KEY
            ),
        ),
        Ok(false) => {}
        Err(e) => log(app, e.to_string()),
    }
}

fn count(diags: &[Diagnostic], severity: Severity) -> usize {
    diags.iter().filter(|d| d.severity == severity).count()
}

/// Changements du projet surveillé (session.rs) : rechargement de l'interface,
/// build et rechargement du firmware, ou relance de la simulation.
async fn on_project_changed<R: Runtime>(app: AppHandle<R>, dir: PathBuf, changes: Changes) {
    let state = app.state::<AppState>();
    let running = state
        .sim
        .lock()
        .await
        .as_ref()
        .is_some_and(SimHandle::is_alive);
    if !changes.views.is_empty() || changes.needs_reload() {
        let _ = app.emit("project:changed", changes.clone());
    }
    if !(changes.firmware || (running && changes.needs_reload())) {
        return;
    }
    let last_build_end = state.last_build_end.lock().ok().and_then(|e| *e);
    if let Ok(cache) = cache_dir(&app) {
        let files: Vec<PathBuf> = changes.files.iter().cloned().collect();
        build::invalidate_stale_objects(
            &build::build_dir_for(&cache, &dir),
            &files,
            last_build_end,
        );
    }
    let root = prebench_root(&app);
    let toolchain = match tauri::async_runtime::spawn_blocking(Toolchain::detect).await {
        Ok(t) => t,
        Err(_) => return,
    };
    let built = match build_project(&app, &root, &dir, &toolchain).await {
        Ok(b) => b,
        Err(CoreError::BuildFailed { .. }) => return, // onglet Build ; l'ancien firmware continue
        Err(e) => {
            log(&app, e.to_string());
            return;
        }
    };
    // Build sans effet (sauvegarde identique, écritures arrivées pendant le
    // build précédent) : le firmware en cours n'est pas relancé.
    if !running || !(built.changed || changes.needs_reload()) {
        return;
    }
    let firmware = built.firmware;
    let mut slot = state.sim.lock().await;
    let Some(handle) = slot.as_ref().filter(|h| h.is_alive()) else {
        return; // arrêtée pendant le build
    };
    if changes.needs_reload() {
        // Composants ou câblage modifiés : nouvelle paillasse.
        let debug = handle.debug.clone();
        handle.stop_and_wait().await;
        match start_sim(&app, &root, &dir, firmware, debug) {
            Ok(h) => *slot = Some(h),
            Err(e) => status(&app, "error", e.to_string()),
        }
    } else if handle.send(SimMsg::Reload(firmware)).await.is_err() {
        log(&app, "rechargement impossible : simulation arrêtée".into());
    }
}

fn log<R: Runtime>(app: &AppHandle<R>, text: String) {
    let _ = app.emit(
        "sim:log",
        LogEvent {
            source: "prebench",
            level: 2,
            text,
        },
    );
}

fn status<R: Runtime>(app: &AppHandle<R>, state: &'static str, message: String) {
    let _ = app.emit("sim:status", StatusEvent { state, message });
}

/// Ouvre `file` à la ligne indiquée dans l'éditeur (section 10.2) : VS Code
/// par son schéma vscode://, sinon l'application par défaut. Limité aux
/// fichiers du projet ouvert et du SDK de Prebench (libvmcu, shims).
#[tauri::command]
pub async fn open_in_editor(
    app: AppHandle,
    state: State<'_, AppState>,
    file: String,
    line: Option<u32>,
    column: Option<u32>,
) -> CoreResult<()> {
    let path = dunce::canonicalize(Path::new(&file))
        .map_err(|_| CoreError::Open(format!("{file} introuvable")))?;
    let project = state.session.lock().await.as_ref().map(|s| s.dir.clone());
    let allowed = project
        .into_iter()
        .chain(std::iter::once(prebench_root(&app)))
        .filter_map(|d| dunce::canonicalize(d).ok())
        .any(|d| path.starts_with(d));
    if !allowed || !path.is_file() {
        return Err(CoreError::Open(format!(
            "{file} n'appartient pas au projet ouvert"
        )));
    }
    let url = editor_url(&path, line, column);
    tauri::async_runtime::spawn_blocking(move || crate::platform::open_location(&url, &path))
        .await
        .map_err(|e| CoreError::Open(e.to_string()))?
        .map_err(|e| CoreError::Open(e.to_string()))
}

/// `vscode://file/<chemin>:<ligne>:<colonne>`, chemin encodé (espaces, accents).
fn editor_url(path: &Path, line: Option<u32>, column: Option<u32>) -> String {
    let raw = path.to_string_lossy().replace('\\', "/");
    let mut url = String::from("vscode://file/");
    for b in raw.trim_start_matches('/').bytes() {
        if b.is_ascii_alphanumeric() || b"-._~/:".contains(&b) {
            url.push(char::from(b));
        } else {
            url.push_str(&format!("%{b:02X}"));
        }
    }
    if let Some(l) = line {
        url.push_str(&format!(":{l}"));
        if let Some(c) = column {
            url.push_str(&format!(":{c}"));
        }
    }
    url
}

/// Session de débogage préparée (section 10.2).
#[derive(Serialize)]
pub struct DebugView {
    config_name: &'static str,
    /// Chemin de .vscode/launch.json.
    launch_json: String,
    /// created, updated, unchanged ; absent si le fichier n'a pas pu être modifié.
    launch_json_state: Option<debug::LaunchJson>,
    launch_json_error: Option<String>,
    /// Extension VS Code et débogueur : présents, ou commande d'installation.
    tools: Vec<debug::DebugTool>,
}

/// Prépare une session de débogage : build, copie de l'exécutable pour le
/// débogueur, configuration dans .vscode/launch.json, puis attente du
/// firmware lancé par VS Code.
#[tauri::command]
pub async fn start_debug(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
) -> CoreResult<DebugView> {
    let mut slot = state.sim.lock().await;
    if slot.as_ref().is_some_and(SimHandle::is_alive) {
        return Err(CoreError::AlreadyRunning);
    }
    let root = prebench_root(&app);
    let dir = simplified(Path::new(&path));
    resolve(&root, &dir)?;
    status(&app, "building", "build « sim » en cours…".into());
    let toolchain = tauri::async_runtime::spawn_blocking(Toolchain::detect)
        .await
        .map_err(|e| CoreError::Build(format!("recherche des outils : {e}")))?;
    let built = build_project(&app, &root, &dir, &toolchain).await?;
    let program = debug::program(&build::build_dir_for(&cache_dir(&app)?, &dir));
    let (from, to) = (built.firmware.clone(), program.clone());
    tauri::async_runtime::spawn_blocking(move || debug::install_program(&from, &to))
        .await
        .map_err(|e| CoreError::Debug(e.to_string()))??;

    let home = app.path().home_dir().ok();
    let (tools, gdb) = debug::tools(home.as_deref(), &toolchain.search_path);
    for t in &tools {
        log(
            &app,
            if t.present {
                format!("{} : présent", t.label)
            } else {
                format!("{} absent ; pour l'installer : {}", t.label, t.install)
            },
        );
    }
    let address = debug::address(&dir);
    let config = debug::launch_config(&program, &dir, &address, gdb.as_deref());
    let (launch_json_state, launch_json_error) = match debug::upsert_launch_json(&dir, &config) {
        Ok(s) => (Some(s), None),
        Err(e) => {
            let text = serde_json::to_string_pretty(&config).unwrap_or_default();
            log(
                &app,
                format!("configuration à ajouter dans .vscode/launch.json :\n{text}"),
            );
            (None, Some(e.to_string()))
        }
    };
    *slot = Some(start_sim(
        &app,
        &root,
        &dir,
        built.firmware,
        Some(DebugConfig { address, program }),
    )?);
    Ok(DebugView {
        config_name: debug::CONFIG_NAME,
        launch_json: dir
            .join(".vscode")
            .join("launch.json")
            .display()
            .to_string(),
        launch_json_state,
        launch_json_error,
        tools,
    })
}

/// Instances dont le composant existe, avec les paramètres complétés.
fn instance_specs(r: &Resolved) -> Vec<InstanceSpec> {
    let volts = r.board.logic_voltage;
    r.instances
        .iter()
        .filter_map(|(view, source)| {
            source.as_ref().map(|s| InstanceSpec {
                id: view.id.clone(),
                source: s.clone(),
                params: view.params.clone(),
                logic_voltage: volts,
            })
        })
        .collect()
}

async fn send(state: &State<'_, AppState>, m: SimMsg) -> CoreResult<()> {
    let slot = state.sim.lock().await;
    match slot.as_ref() {
        Some(handle) if handle.is_alive() => handle.send(m).await,
        _ => Err(CoreError::NotRunning),
    }
}

#[tauri::command]
pub async fn stop_simulation(state: State<'_, AppState>) -> CoreResult<()> {
    send(&state, SimMsg::Stop).await
}

#[tauri::command]
pub async fn reset_simulation(state: State<'_, AppState>) -> CoreResult<()> {
    send(&state, SimMsg::Reset).await
}

/// Pause de la simulation (barre d'outils, touche Espace) : horloge du firmware gelée.
#[tauri::command]
pub async fn pause_simulation(state: State<'_, AppState>) -> CoreResult<()> {
    send(&state, SimMsg::Pause).await
}

/// Reprise après une pause, manuelle ou automatique ; le temps repart sans saut.
#[tauri::command]
pub async fn resume_simulation(state: State<'_, AppState>) -> CoreResult<()> {
    send(&state, SimMsg::Resume).await
}

/// Action sur un contrôle déclaré dans le manifeste (bouton, curseur…).
#[tauri::command]
pub async fn component_control(
    state: State<'_, AppState>,
    component: String,
    control: String,
    value: serde_json::Value,
) -> CoreResult<()> {
    send(
        &state,
        SimMsg::Control {
            component,
            control,
            value_json: value.to_string(),
        },
    )
    .await
}

#[cfg(test)]
pub fn resolve_for_tests(
    root: &Path,
    dir: &Path,
) -> CoreResult<(Project, Board, Vec<InstanceSpec>)> {
    let r = resolve(root, dir)?;
    let specs = instance_specs(&r);
    Ok((r.project, r.board, specs))
}

#[cfg(test)]
mod editor_tests {
    use super::editor_url;
    use std::path::Path;

    #[test]
    fn vscode_urls() {
        assert_eq!(
            editor_url(Path::new("/Users/me/Mes projets/é.c"), Some(12), Some(3)),
            "vscode://file/Users/me/Mes%20projets/%C3%A9.c:12:3"
        );
        assert_eq!(
            editor_url(Path::new("/p/CMakeLists.txt"), Some(7), None),
            "vscode://file/p/CMakeLists.txt:7"
        );
        assert_eq!(
            editor_url(Path::new("/p/a#b.c"), None, Some(1)),
            "vscode://file/p/a%23b.c"
        );
    }
}

/// Cycle édition → simulation (section 10.2) de bout en bout, sans fenêtre :
/// une copie du blinky est surveillée ; chaque sauvegarde recompile et
/// recharge le firmware, une erreur est listée avec son fichier et sa ligne
/// pendant que l'ancien firmware continue de tourner.
#[cfg(test)]
mod reload_tests {
    use std::path::{Path, PathBuf};
    use std::sync::mpsc;
    use std::time::{Duration, Instant};

    use serde_json::Value;
    use tauri::test::{mock_builder, mock_context, noop_assets};
    use tauri::{Listener, Manager};

    use super::*;

    fn repo_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
    }

    fn copy_dir(from: &Path, to: &Path) {
        std::fs::create_dir_all(to).expect("dossier");
        for entry in std::fs::read_dir(from).expect("lecture").flatten() {
            let name = entry.file_name();
            if name.to_string_lossy().starts_with("build") {
                continue;
            }
            let target = to.join(&name);
            if entry.path().is_dir() {
                copy_dir(&entry.path(), &target);
            } else {
                std::fs::copy(entry.path(), &target).expect("copie");
            }
        }
    }

    struct Events(
        mpsc::Receiver<(String, Value)>,
        std::cell::RefCell<Vec<String>>,
        Instant,
    );

    impl Events {
        /// Attend un événement ; à l'expiration, affiche les derniers reçus.
        fn wait(&self, timeout: Duration, mut pred: impl FnMut(&str, &Value) -> bool) -> Value {
            let deadline = Instant::now() + timeout;
            loop {
                let left = deadline.saturating_duration_since(Instant::now());
                let Ok((name, v)) = self.0.recv_timeout(left) else {
                    panic!(
                        "événement attendu non reçu ; derniers : {:#?}",
                        self.1.borrow()
                    );
                };
                {
                    let mut last = self.1.borrow_mut();
                    if name != "sim:states" {
                        let mut line = format!("{:>6} ms {name} {v}", self.2.elapsed().as_millis());
                        line.truncate(240);
                        last.push(line);
                    }
                    let excess = last.len().saturating_sub(24);
                    last.drain(..excess);
                }
                if pred(&name, &v) {
                    return v;
                }
            }
        }

        /// Période moyenne de LD2 sur `n` bascules, en ms.
        fn ld2_period(&self, n: usize) -> f64 {
            let mut stamps = Vec::new();
            let mut last = None;
            while stamps.len() < n + 1 {
                let v = self.wait(Duration::from_secs(5), |name, v| {
                    name == "sim:states" && v["LD2"]["brightness"].is_number()
                });
                let level = v["LD2"]["brightness"].as_f64().map(|b| b > 0.5);
                if level != last {
                    last = level;
                    stamps.push(Instant::now());
                }
            }
            (stamps[n] - stamps[1]).as_secs_f64() * 1000.0 / (n as f64 - 1.0)
        }
    }

    fn edit(file: &Path, from: &str, to: &str) {
        let text = std::fs::read_to_string(file).expect("lecture");
        assert!(text.contains(from), "{from} absent de {}", file.display());
        std::fs::write(file, text.replacen(from, to, 1)).expect("écriture");
    }

    #[test]
    fn save_rebuilds_and_reloads() {
        let dir = std::env::temp_dir().join(format!("prebench-reload-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        copy_dir(
            &repo_root().join("templates").join("blinky-stm32-hal"),
            &dir,
        );
        let dir = dunce::canonicalize(&dir).expect("chemin");
        let main_c = dir.join("Core").join("Src").join("main.c");

        let app = mock_builder()
            .manage(AppState::default())
            .build(mock_context(noop_assets()))
            .expect("app simulée");
        let (tx, rx) = mpsc::channel();
        for name in [
            "sim:states",
            "sim:status",
            "sim:log",
            "build:result",
            "project:changed",
        ] {
            let tx = tx.clone();
            app.handle().listen_any(name, move |e| {
                let v = serde_json::from_str(e.payload()).unwrap_or(Value::Null);
                let _ = tx.send((name.to_string(), v));
            });
        }
        let events = Events(rx, std::cell::RefCell::new(Vec::new()), Instant::now());
        let handle = app.handle().clone();
        let state = app.state::<AppState>();
        tauri::async_runtime::block_on(async {
            let sim = launch(&handle, &dir).await.expect("démarrage");
            *state.sim.lock().await = Some(sim);
            let watch = session::start(handle.clone(), dir.clone(), on_project_changed)
                .expect("surveillance");
            *state.session.lock().await = Some(watch);
        });
        // Premier build complet : long sous Windows (MinGW) en CI.
        events.wait(Duration::from_secs(180), |n, v| {
            n == "sim:status" && v["state"] == "running"
        });
        let slow = events.ld2_period(3);
        assert!((400.0..700.0).contains(&slow), "période initiale {slow} ms");
        std::thread::sleep(Duration::from_millis(400)); // FSEvents : ignore la création du dossier

        eprintln!("étape : 1. Sauvegarde");
        // 1. Sauvegarde : nouvelle période, firmware rechargé sans relance de l'app.
        edit(&main_c, "blink_period_ms = 500;", "blink_period_ms = 100;");
        let result = events.wait(Duration::from_secs(180), |n, _| n == "build:result");
        assert_eq!(result["ok"], true, "{result}");
        events.wait(Duration::from_secs(10), |n, v| {
            n == "sim:log" && v["text"] == "firmware rechargé après le build"
        });
        events.wait(Duration::from_secs(10), |n, v| {
            n == "sim:status" && v["state"] == "running"
        });
        let fast = events.ld2_period(5);
        println!("période {slow:.0} ms, puis {fast:.0} ms après rechargement");
        assert!(
            (60.0..200.0).contains(&fast),
            "période après rechargement {fast} ms"
        );

        eprintln!("étape : 2. Erreur de compilation");
        // 2. Erreur de compilation : fichier et ligne ; l'ancien firmware continue.
        let text = std::fs::read_to_string(&main_c).expect("lecture");
        let line = text
            .lines()
            .position(|l| l.contains("HAL_Delay(blink_period_ms);"))
            .expect("ligne")
            + 1;
        edit(
            &main_c,
            "HAL_Delay(blink_period_ms);",
            "HAL_Delay(blink_period_ms)",
        );
        // Un build réussi peut précéder : écritures de l'étape 1 signalées pendant son build.
        let result = events.wait(Duration::from_secs(180), |n, v| {
            n == "build:result" && v["ok"] == false
        });
        let first = &result["diagnostics"][0];
        println!(
            "{} : {}:{} {}",
            first["severity"], first["display"], first["line"], first["message"]
        );
        assert_eq!(first["severity"], "error");
        assert_eq!(first["display"], "Core/Src/main.c");
        let reported = first["line"].as_u64().unwrap_or(0) as usize;
        assert!(
            reported == line || reported == line + 1,
            "ligne {reported}, attendue {line}"
        );
        let still = events.ld2_period(3);
        assert!(
            (60.0..200.0).contains(&still),
            "firmware arrêté par l'erreur ({still} ms)"
        );

        eprintln!("étape : 3. Vue d'un composant");
        // 3. Vue d'un composant du projet : signalée à l'interface.
        let view = dir.join("components").join("probe");
        std::fs::create_dir_all(&view).expect("dossier");
        std::fs::write(view.join("view.svelte"), "<p>sonde</p>").expect("écriture");
        let changed = events.wait(Duration::from_secs(10), |n, _| n == "project:changed");
        let has = |k: &str| {
            changed[k]
                .as_array()
                .is_some_and(|a| a.contains(&Value::from("probe")))
        };
        // Linux : la création de components/ relit tout le projet.
        assert!(
            has("views") || has("models") || changed["project"] == true,
            "{changed}"
        );

        tauri::async_runtime::block_on(async {
            *state.session.lock().await = None;
            if let Some(sim) = state.sim.lock().await.take() {
                sim.stop_and_wait().await;
            }
        });
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Copie du blinky surveillée par une app simulée (sans simulation lancée).
    fn setup(name: &str) -> (tauri::App<tauri::test::MockRuntime>, Events, PathBuf) {
        let dir = std::env::temp_dir().join(format!("prebench-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        copy_dir(
            &repo_root().join("templates").join("blinky-stm32-hal"),
            &dir,
        );
        let dir = dunce::canonicalize(&dir).expect("chemin");
        let app = mock_builder()
            .manage(AppState::default())
            .build(mock_context(noop_assets()))
            .expect("app simulée");
        let (tx, rx) = mpsc::channel();
        for name in [
            "sim:states",
            "sim:status",
            "sim:log",
            "build:result",
            "project:changed",
        ] {
            let tx = tx.clone();
            app.handle().listen_any(name, move |e| {
                let v = serde_json::from_str(e.payload()).unwrap_or(Value::Null);
                let _ = tx.send((name.to_string(), v));
            });
        }
        (
            app,
            Events(rx, std::cell::RefCell::new(Vec::new()), Instant::now()),
            dir,
        )
    }

    impl Events {
        /// Événements `name` reçus pendant `during`.
        fn count(&self, during: Duration, name: &str) -> Vec<Value> {
            let deadline = Instant::now() + during;
            let mut found = Vec::new();
            while let Some(left) = deadline.checked_duration_since(Instant::now()) {
                if let Ok((n, v)) = self.0.recv_timeout(left) {
                    if n == name {
                        found.push(v);
                    }
                }
            }
            found
        }
    }

    fn modified(f: &Path) -> std::time::SystemTime {
        std::fs::metadata(f)
            .and_then(|m| m.modified())
            .expect("date")
    }

    /// Non-régression de la boucle de builds : sans sauvegarde, aucun build
    /// (même quand l'éditeur touche aux métadonnées des sources) ; une
    /// sauvegarde donne exactement un build ; une sauvegarde dans la seconde
    /// qui suit ce build est compilée (make à la seconde près) ; l'app ne
    /// modifie jamais les fichiers du projet.
    #[test]
    fn builds_only_on_real_saves() {
        let (app, events, dir) = setup("saves");
        let main_c = dir.join("Core").join("Src").join("main.c");
        let handle = app.handle().clone();
        let state = app.state::<AppState>();
        tauri::async_runtime::block_on(async {
            *state.sim.lock().await = Some(launch(&handle, &dir).await.expect("démarrage"));
            *state.session.lock().await = Some(
                session::start(handle.clone(), dir.clone(), on_project_changed)
                    .expect("surveillance"),
            );
        });
        events.wait(Duration::from_secs(180), |n, v| {
            n == "sim:status" && v["state"] == "running"
        });
        let _ = events.count(Duration::from_millis(400), "build:result"); // build du démarrage

        // 10 s sans sauvegarde ; lecture, droits et date d'accès touchés.
        let toucher = std::thread::spawn({
            let main_c = main_c.clone();
            move || {
                std::thread::sleep(Duration::from_secs(2));
                let _ = std::fs::read_to_string(&main_c);
                if let Ok(meta) = std::fs::metadata(&main_c) {
                    let _ = std::fs::set_permissions(&main_c, meta.permissions());
                }
                if let Ok(f) = std::fs::File::options().write(true).open(&main_c) {
                    let _ = f.set_times(
                        std::fs::FileTimes::new().set_accessed(std::time::SystemTime::now()),
                    );
                }
            }
        });
        let idle = events.count(Duration::from_secs(10), "build:result");
        let _ = toucher.join();
        assert!(idle.is_empty(), "build sans sauvegarde : {idle:?}");

        // Une sauvegarde : exactement un build.
        edit(&main_c, "blink_period_ms = 500;", "blink_period_ms = 200;");
        let saved_at = modified(&main_c);
        events.wait(Duration::from_secs(180), |n, _| n == "build:result");
        let extra = events.count(Duration::from_secs(5), "build:result");
        assert!(
            extra.is_empty(),
            "builds en trop après une sauvegarde : {extra:?}"
        );
        assert_eq!(modified(&main_c), saved_at, "main.c modifié par l'app");
        let period = events.ld2_period(3);
        assert!(
            (150.0..300.0).contains(&period),
            "période {period} ms après la sauvegarde"
        );

        // Sauvegarde juste après la fin d'un build (même seconde possible).
        edit(&main_c, "blink_period_ms = 200;", "blink_period_ms = 100;");
        events.wait(Duration::from_secs(180), |n, _| n == "build:result");
        let saved_at = modified(&main_c);
        edit(&main_c, "blink_period_ms = 100;", "blink_period_ms = 300;");
        let second_save = modified(&main_c);
        let result = events.wait(Duration::from_secs(180), |n, _| n == "build:result");
        assert_eq!(result["ok"], true, "{result}");
        let _ = events.count(Duration::from_secs(2), "build:result");
        assert_ne!(saved_at, std::time::SystemTime::UNIX_EPOCH);
        assert_eq!(modified(&main_c), second_save, "main.c modifié par l'app");
        let period = events.ld2_period(3);
        println!("période après deux sauvegardes rapprochées : {period:.0} ms");
        assert!(
            (220.0..400.0).contains(&period),
            "dernière sauvegarde non compilée ({period} ms)"
        );

        tauri::async_runtime::block_on(async {
            *state.session.lock().await = None;
            if let Some(sim) = state.sim.lock().await.take() {
                sim.stop_and_wait().await;
            }
        });
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Débogage : une sauvegarde compile sans recharger le firmware lancé par
    /// le débogueur ; « relancez la session » n'apparaît qu'une fois.
    #[test]
    fn debug_session_save_builds_without_reload() {
        let (app, events, dir) = setup("debug-save");
        let main_c = dir.join("Core").join("Src").join("main.c");
        let handle = app.handle().clone();
        let state = app.state::<AppState>();
        let root = repo_root();
        let address = crate::platform::debug_socket_address(u64::from(std::process::id()) ^ 0xdeb);
        let program = std::env::temp_dir().join(format!(
            "prebench-dbg-program-{}{}",
            std::process::id(),
            if cfg!(windows) { ".exe" } else { "" }
        ));
        tauri::async_runtime::block_on(async {
            let built = build_project(&handle, &root, &dir, &Toolchain::detect())
                .await
                .expect("build");
            std::fs::copy(&built.firmware, &program).expect("copie");
            let debug = DebugConfig {
                address: address.clone(),
                program: program.clone(),
            };
            *state.sim.lock().await = Some(
                start_sim(&handle, &root, &dir, built.firmware, Some(debug)).expect("débogage"),
            );
            *state.session.lock().await = Some(
                session::start(handle.clone(), dir.clone(), on_project_changed)
                    .expect("surveillance"),
            );
        });
        events.wait(Duration::from_secs(10), |n, v| {
            n == "sim:status" && v["state"] == "debug-wait"
        });
        let mut child = std::process::Command::new(&program)
            .env("PREBENCH_SOCKET", &address)
            .env("PREBENCH_BOARD_ID", "1")
            .env("PREBENCH_DEBUG", "1")
            .current_dir(&dir)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .expect("firmware lancé par le « débogueur »");
        events.wait(Duration::from_secs(10), |n, v| {
            n == "sim:status" && v["state"] == "running"
        });
        let _ = events.count(Duration::from_millis(400), "build:result");

        let mut restarts = 0;
        let mut relaunch_messages = 0;
        for (from, to) in [("= 500;", "= 200;"), ("= 200;", "= 300;")] {
            edit(
                &main_c,
                &format!("blink_period_ms {from}"),
                &format!("blink_period_ms {to}"),
            );
            let deadline = Instant::now() + Duration::from_secs(180);
            loop {
                let left = deadline.saturating_duration_since(Instant::now());
                let (n, v) = events.0.recv_timeout(left).expect("build attendu");
                match n.as_str() {
                    "build:result" => {
                        assert_eq!(v["ok"], true, "{v}");
                        break;
                    }
                    "sim:status" if v["state"] == "starting" => restarts += 1,
                    "sim:log"
                        if v["text"]
                            .as_str()
                            .is_some_and(|t| t.contains("relancez la session")) =>
                    {
                        relaunch_messages += 1
                    }
                    _ => {}
                }
            }
        }
        for v in events.count(Duration::from_secs(2), "sim:log") {
            if v["text"]
                .as_str()
                .is_some_and(|t| t.contains("relancez la session"))
            {
                relaunch_messages += 1;
            }
        }
        assert_eq!(
            restarts, 0,
            "firmware relancé pendant la session de débogage"
        );
        assert_eq!(relaunch_messages, 1, "message « relancez la session »");
        // Le firmware du débogueur tourne toujours, à sa période d'origine.
        let period = events.ld2_period(3);
        assert!(
            (400.0..700.0).contains(&period),
            "firmware remplacé ({period} ms)"
        );

        let _ = child.kill();
        let _ = child.wait();
        events.wait(Duration::from_secs(5), |n, v| {
            n == "sim:status" && v["state"] == "debug-wait"
        });
        tauri::async_runtime::block_on(async {
            *state.session.lock().await = None;
            if let Some(sim) = state.sim.lock().await.take() {
                sim.stop_and_wait().await;
            }
        });
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_file(&program);
    }

    /// Débogueur en ligne de commande pour le test : lldb (macOS), gdb sinon.
    fn command_line_debugger() -> Option<PathBuf> {
        if cfg!(target_os = "macos") {
            return Some(PathBuf::from("/usr/bin/lldb")).filter(|p| p.is_file());
        }
        let names: &[&str] = if cfg!(windows) {
            &["gdb.exe"]
        } else {
            &["gdb"]
        };
        std::env::split_paths(&Toolchain::detect().search_path)
            .flat_map(|d| names.iter().map(move |n| d.join(n)))
            .find(|p| p.is_file())
    }

    /// Chemin réel d'une session de débogage : build « sim » (Debug), copie dans
    /// debug/ avec ses informations de débogage, objets de main.c supprimés
    /// (correctif de make), puis point d'arrêt posé par chemin complet et ligne,
    /// comme VS Code : il doit être résolu sur le main.c du projet.
    /// Vérifie aussi compile_commands.json et .vscode/settings.json (IntelliSense).
    #[test]
    fn breakpoint_resolves_on_debug_program() {
        let Some(debugger) = command_line_debugger() else {
            assert!(
                std::env::var_os("CI").is_none(),
                "débogueur absent de la CI"
            );
            println!("débogueur absent : test ignoré");
            return;
        };
        let (app, _events, dir) = setup("breakpoint");
        let main_c = dir.join("Core").join("Src").join("main.c");
        let line = std::fs::read_to_string(&main_c)
            .expect("lecture")
            .lines()
            .position(|l| l.contains("HAL_GPIO_TogglePin(LD2_GPIO_Port, LD2_Pin);"))
            .expect("ligne du toggle")
            + 1;
        let handle = app.handle().clone();
        let root = repo_root();
        let built = tauri::async_runtime::block_on(build_project(
            &handle,
            &root,
            &dir,
            &Toolchain::detect(),
        ))
        .expect("build");
        let build_dir = build::build_dir_for(&cache_dir(&handle).expect("cache"), &dir);
        let program = debug::program(&build_dir);
        debug::install_program(&built.firmware, &program).expect("exécutable du débogueur");
        build::invalidate_stale_objects(
            &build_dir,
            std::slice::from_ref(&main_c),
            Some(std::time::SystemTime::now()),
        );

        // IntelliSense : base de compilation avec les en-têtes du shim, inscrite pour VS Code.
        let cc = std::fs::read_to_string(build::compile_commands(&build_dir))
            .expect("compile_commands.json");
        assert!(
            cc.contains("main.c") && cc.contains("stm32-hal"),
            "compile_commands.json incomplet"
        );
        let settings = std::fs::read_to_string(dir.join(".vscode").join("settings.json"))
            .expect("settings.json");
        assert!(settings.contains(debug::COMPILE_COMMANDS_KEY), "{settings}");

        let target = format!("{}:{line}", main_c.display());
        let out = if cfg!(target_os = "macos") {
            std::process::Command::new(&debugger)
                .args(["-b", "-o"])
                .arg(format!(
                    "breakpoint set --file {} --line {line}",
                    main_c.display()
                ))
                .arg(&program)
                .output()
        } else {
            std::process::Command::new(&debugger)
                .args(["-batch", "-nx", "-ex"])
                .arg(format!("break {target}"))
                .arg(&program)
                .output()
        }
        .expect("débogueur");
        let text = format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        println!("{}", text.trim());
        let resolved = if cfg!(target_os = "macos") {
            text.contains("where = ") && !text.contains("no locations")
        } else {
            text.contains("Breakpoint 1 at ") && text.contains(&format!("line {line}"))
        };
        assert!(resolved, "point d'arrêt {target} non résolu :\n{text}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

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
use crate::core::board::Board;
use crate::core::components::engine::InstanceSpec;
use crate::core::components::{sync_api_declarations, Catalog, ComponentSource};
use crate::core::sim::{self, SimConfig, SimHandle, SimMsg};
use crate::error::{CoreError, CoreResult};
use crate::platform::paths::simplified;
use crate::platform::toolchain::{ToolKind, Toolchain};
use crate::project::{templates, Project};

/// Une seule carte en phase 1 (multi-cartes : E2, V2).
const BOARD_ID: u16 = 1;

#[derive(Default)]
pub struct AppState {
    sim: Mutex<Option<SimHandle>>,
}

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

#[tauri::command]
pub fn open_project(app: AppHandle, path: String) -> CoreResult<ProjectView> {
    let root = prebench_root(&app);
    let dir = PathBuf::from(&path);
    let mut r = resolve(&root, &dir)?;
    if sync_api_declarations(&root.join("components"), &dir)? {
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
    let r = resolve(&root, &dir)?;
    let components = instance_specs(&r);

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
    let build_dir = build::build_dir_for(&cache_dir(app)?, &dir);
    let _ = app.emit(
        "sim:log",
        LogEvent {
            source: "prebench",
            level: 2,
            text: format!("dossier de build : {}", build_dir.display()),
        },
    );
    let firmware = build::build_sim(&dir, &root, &build_dir, &toolchain).await?;
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
            project_dir: dir,
            project: r.project,
            board: r.board,
            firmware,
            board_id: BOARD_ID,
            components,
        },
    ))
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

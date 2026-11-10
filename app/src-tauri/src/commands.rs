// app/src-tauri/src/commands.rs — Commandes du cœur appelables par l'interface.
//
// Liste fermée (build.rs, capabilities/default.json) : l'interface ne peut rien
// faire d'autre (section 11.4).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::sync::Mutex;

use crate::build;
use crate::core::board::Board;
use crate::core::components::engine::InstanceSpec;
use crate::core::components::{sync_api_declarations, Catalog, ComponentSource};
use crate::core::sim::{self, SimConfig, SimHandle, SimMsg};
use crate::error::{CoreError, CoreResult};
use crate::project::Project;

/// Une seule carte en phase 1 (multi-cartes : E2, V2).
const BOARD_ID: u16 = 1;

#[derive(Default)]
pub struct AppState {
    sim: Mutex<Option<SimHandle>>,
}

/// Racine contenant libvmcu/, boards/, components/, templates/ : le dépôt
/// Prebench en développement (les modifications sont prises en compte sans
/// recompiler l'app), les ressources « sdk/ » de l'app installée sinon.
fn prebench_root(app: &AppHandle) -> PathBuf {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
    if cfg!(debug_assertions) && repo.join("libvmcu").is_dir() {
        return repo;
    }
    if let Ok(resources) = app.path().resource_dir() {
        let root = resources.join("sdk");
        if root.join("libvmcu").is_dir() {
            return root;
        }
    }
    repo
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

#[tauri::command]
pub fn default_project(app: AppHandle) -> String {
    prebench_root(&app)
        .join("templates")
        .join("blinky-stm32-hal")
        .display()
        .to_string()
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
    let root = prebench_root(&app);
    let dir = PathBuf::from(&path);
    let r = resolve(&root, &dir)?;
    let components = instance_specs(&r);

    let _ = app.emit(
        "sim:status",
        StatusEvent {
            state: "building",
            message: "build « sim » en cours…".into(),
        },
    );
    let firmware = build::build_sim(&dir, &root).await?;
    let _ = app.emit(
        "sim:status",
        StatusEvent {
            state: "starting",
            message: "lancement du firmware…".into(),
        },
    );

    *slot = Some(sim::start(
        app.clone(),
        SimConfig {
            project_dir: dir,
            project: r.project,
            board: r.board,
            firmware,
            board_id: BOARD_ID,
            components,
        },
    ));
    Ok(())
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

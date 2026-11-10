// app/src-tauri/src/core/components/mod.rs — Catalogue des composants (section 8, D4).
//
// Un composant est un dossier <id>/ (manifest.json, model.ts, view.svelte) :
//   1. <projet>/components/<id>/   composants du projet (prioritaires)
//   2. <sdk>/components/<id>/      composants fournis avec Prebench
// Un composant du projet portant l'identifiant d'un composant fourni le
// remplace, avec un avertissement ⚠.

pub mod engine;
pub mod transpile;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::Value;

/// Version de l'API composant implémentée par cette app.
pub const API_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Origin {
    Project,
    Provided,
}

#[derive(Debug, Clone, Serialize)]
pub struct ComponentSource {
    pub id: String,
    pub origin: Origin,
    #[serde(skip)]
    pub dir: PathBuf,
    /// manifest.json tel quel (schéma : components/manifest.schema.json).
    pub manifest: Value,
}

impl ComponentSource {
    pub fn model_path(&self) -> PathBuf {
        self.dir.join("model.ts")
    }

    pub fn view_path(&self) -> PathBuf {
        self.dir.join("view.svelte")
    }

    pub fn tick_hz(&self) -> f64 {
        self.manifest["tick_hz"].as_f64().unwrap_or(0.0)
    }

    /// Paramètres d'une instance complétés par les défauts du manifeste.
    pub fn params_with_defaults(
        &self,
        given: &BTreeMap<String, Value>,
    ) -> serde_json::Map<String, Value> {
        let mut params = serde_json::Map::new();
        if let Some(defs) = self.manifest["params"].as_object() {
            for (name, def) in defs {
                params.insert(name.clone(), def["default"].clone());
            }
        }
        for (name, value) in given {
            params.insert(name.clone(), value.clone());
        }
        params
    }
}

#[derive(Debug, Default)]
pub struct Catalog {
    pub components: BTreeMap<String, ComponentSource>,
    pub warnings: Vec<String>,
}

impl Catalog {
    /// Lit les composants fournis puis ceux du projet (prioritaires).
    pub fn load(provided_dir: &Path, project_dir: &Path) -> Self {
        let mut catalog = Catalog::default();
        catalog.scan(provided_dir, Origin::Provided);
        catalog.scan(&project_dir.join("components"), Origin::Project);
        catalog
    }

    fn scan(&mut self, dir: &Path, origin: Origin) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        let mut dirs: Vec<PathBuf> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .collect();
        dirs.sort();
        for path in dirs {
            let manifest_path = path.join("manifest.json");
            if !manifest_path.is_file() {
                continue;
            }
            let manifest: Value = match std::fs::read_to_string(&manifest_path)
                .map_err(|e| e.to_string())
                .and_then(|t| serde_json::from_str(&t).map_err(|e| e.to_string()))
            {
                Ok(m) => m,
                Err(e) => {
                    self.warnings
                        .push(format!("{} illisible : {e}", manifest_path.display()));
                    continue;
                }
            };
            let id = manifest["id"].as_str().unwrap_or_default().to_string();
            let dir_name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or_default();
            if id != dir_name {
                self.warnings.push(format!(
                    "{} : l'identifiant « {id} » ne correspond pas au dossier « {dir_name} »",
                    manifest_path.display()
                ));
                continue;
            }
            if manifest["api_version"].as_u64() != Some(u64::from(API_VERSION)) {
                self.warnings.push(format!(
                    "composant {id} : api_version non prise en charge (attendu {API_VERSION})"
                ));
                continue;
            }
            if origin == Origin::Project && self.components.contains_key(&id) {
                self.warnings.push(format!(
                    "composant {id} du projet utilisé à la place du composant fourni"
                ));
            }
            self.components.insert(
                id.clone(),
                ComponentSource {
                    id,
                    origin,
                    dir: path,
                    manifest,
                },
            );
        }
    }
}

/// En-tête de la copie de component-api.d.ts écrite dans un projet.
fn api_header() -> String {
    format!(
        "// Généré par Prebench {} — ne pas modifier. API composant v{API_VERSION}.\n",
        env!("CARGO_PKG_VERSION")
    )
}

/// Écrit <projet>/components/component-api.d.ts si le projet a des composants
/// et si la copie est absente ou d'une autre version. Renvoie true si écrite.
pub fn sync_api_declarations(provided_dir: &Path, project_dir: &Path) -> std::io::Result<bool> {
    let target_dir = project_dir.join("components");
    if !target_dir.is_dir() {
        return Ok(false);
    }
    let target = target_dir.join("component-api.d.ts");
    let header = api_header();
    if let Ok(existing) = std::fs::read_to_string(&target) {
        if existing.starts_with(&header) {
            return Ok(false);
        }
    }
    let source = std::fs::read_to_string(provided_dir.join("component-api.d.ts"))?;
    std::fs::write(&target, format!("{header}{source}"))?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn provided() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../components")
    }

    fn temp_project(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("prebench-test-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("components/led")).expect("dossier temporaire");
        dir
    }

    #[test]
    fn provided_components_are_found() {
        let catalog = Catalog::load(&provided(), Path::new("/inexistant"));
        for id in ["led", "button", "echo"] {
            assert_eq!(catalog.components[id].origin, Origin::Provided);
        }
        assert!(catalog.warnings.is_empty(), "{:?}", catalog.warnings);
    }

    #[test]
    fn project_component_overrides_provided_one() {
        let project = temp_project("override");
        let manifest =
            std::fs::read_to_string(provided().join("led/manifest.json")).expect("manifeste");
        std::fs::write(project.join("components/led/manifest.json"), manifest).expect("écriture");
        let catalog = Catalog::load(&provided(), &project);
        assert_eq!(catalog.components["led"].origin, Origin::Project);
        assert!(catalog.warnings.iter().any(|w| w.contains("led du projet")));

        assert!(sync_api_declarations(&provided(), &project).expect("copie"));
        assert!(!sync_api_declarations(&provided(), &project).expect("copie à jour"));
        let copy = std::fs::read_to_string(project.join("components/component-api.d.ts"))
            .expect("lecture");
        assert!(copy.starts_with("// Généré par Prebench"));
        let _ = std::fs::remove_dir_all(&project);
    }
}

// app/src-tauri/src/project/mod.rs — Lecture de prebench.toml (section 10.1).
//
// Format : docs/prebench.schema.json. Un dossier sans prebench.toml (projet
// STM32CubeMX brut) est traité comme une Nucleo-F401RE en STM32 HAL.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::{CoreError, CoreResult};

pub mod templates;

pub const DEFAULT_BOARD: &str = "nucleo-f401re";
pub const DEFAULT_FRAMEWORK: &str = "stm32-hal";

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Project {
    pub board: String,
    pub framework: String,
    #[serde(default)]
    pub components: Vec<ComponentInstance>,
    #[serde(default)]
    pub wires: Vec<Wire>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ComponentInstance {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub params: BTreeMap<String, toml::Value>,
    pub pos: [f64; 2],
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Wire {
    pub from: String,
    pub to: String,
}

impl Project {
    pub fn load(dir: &Path) -> CoreResult<Self> {
        let path = dir.join("prebench.toml");
        if !path.exists() {
            return Ok(Project {
                board: DEFAULT_BOARD.into(),
                framework: DEFAULT_FRAMEWORK.into(),
                components: Vec::new(),
                wires: Vec::new(),
            });
        }
        let text = std::fs::read_to_string(&path)?;
        toml::from_str(&text).map_err(|e| CoreError::Project {
            path: path.display().to_string(),
            message: e.to_string(),
        })
    }
}

/// Extrémité de câble « <id>.<broche> » ; « board.PB9 » désigne la carte.
pub fn split_endpoint(endpoint: &str) -> Option<(&str, &str)> {
    endpoint.split_once('.')
}

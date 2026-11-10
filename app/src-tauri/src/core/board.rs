// app/src-tauri/src/core/board.rs — Fichiers de définition de cartes (section 7).
//
// Format : boards/board.schema.json. Ajouter une carte = ajouter un fichier :
// aucune carte n'est codée en dur ici.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::{CoreError, CoreResult};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Board {
    #[serde(skip_deserializing)]
    pub id: String,
    pub name: String,
    pub mcu: String,
    pub family: String,
    pub framework: Vec<String>,
    pub logic_voltage: f64,
    pub adc_bits: u8,
    #[serde(default)]
    pub reset: Option<String>,
    pub power: BTreeMap<String, f64>,
    pub pins: BTreeMap<String, PinDef>,
    #[serde(default)]
    pub onboard: BTreeMap<String, Onboard>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PinDef {
    #[serde(default)]
    pub label: Option<String>,
    pub caps: Vec<String>,
    #[serde(default)]
    pub external_pull: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Onboard {
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub params: BTreeMap<String, toml::Value>,
    pub wires: BTreeMap<String, String>,
}

impl Board {
    pub fn load(boards_dir: &Path, id: &str) -> CoreResult<Self> {
        let path = boards_dir.join(format!("{id}.toml"));
        let text = std::fs::read_to_string(&path).map_err(|e| CoreError::Board {
            path: path.display().to_string(),
            message: e.to_string(),
        })?;
        let mut board: Board = toml::from_str(&text).map_err(|e| CoreError::Board {
            path: path.display().to_string(),
            message: e.to_string(),
        })?;
        board.id = id.to_string();
        Ok(board)
    }
}

/// Nom de broche -> (port, pin) du protocole (docs/protocol.md, 1.2).
/// STM32 : « PA5 » -> (0, 5) ; ESP32 : « GPIO5 » -> (0, 5).
pub fn pin_address(name: &str) -> Option<(u8, u8)> {
    if let Some(n) = name.strip_prefix("GPIO") {
        return n.parse::<u8>().ok().filter(|p| *p < 64).map(|p| (0, p));
    }
    let mut chars = name.chars();
    if chars.next()? != 'P' {
        return None;
    }
    let letter = chars.next()?;
    if !letter.is_ascii_uppercase() {
        return None;
    }
    let pin = chars.as_str().parse::<u8>().ok().filter(|p| *p < 16)?;
    Some((letter as u8 - b'A', pin))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pin_names() {
        assert_eq!(pin_address("PA5"), Some((0, 5)));
        assert_eq!(pin_address("PC13"), Some((2, 13)));
        assert_eq!(pin_address("GPIO34"), Some((0, 34)));
        assert_eq!(pin_address("GND"), None);
        assert_eq!(pin_address("PA16"), None);
    }

    #[test]
    fn loads_nucleo() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../boards");
        let board = Board::load(&dir, "nucleo-f401re").expect("carte de référence lisible");
        assert_eq!(board.name, "Nucleo-F401RE");
        assert_eq!(board.onboard["LD2"].wires["A"], "PA5");
        assert_eq!(board.pins["PC13"].external_pull.as_deref(), Some("up"));
    }
}

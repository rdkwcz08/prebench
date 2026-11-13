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
    /// Disposition sur la paillasse (connecteurs, pastilles) ; facultative.
    #[serde(default)]
    pub layout: Option<Layout>,
}

/// Disposition physique (boards/board.schema.json, layout).
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Layout {
    pub size: [f64; 2],
    #[serde(default)]
    pub holes: Vec<[f64; 2]>,
    #[serde(default)]
    pub chip: Option<Rect>,
    #[serde(default)]
    pub onboard: BTreeMap<String, [f64; 2]>,
    #[serde(default)]
    pub unconnected: Vec<String>,
    pub headers: Vec<Header>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Rect {
    pub pos: [f64; 2],
    pub size: [f64; 2],
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Header {
    pub id: String,
    pub pos: [f64; 2],
    pub dir: String,
    #[serde(default = "default_pitch")]
    pub pitch: f64,
    #[serde(default = "default_label_side")]
    pub label_side: String,
    pub pins: Vec<String>,
}

fn default_pitch() -> f64 {
    16.0
}

fn default_label_side() -> String {
    "right".into()
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PinDef {
    #[serde(default)]
    pub label: Option<String>,
    pub caps: Vec<String>,
    #[serde(default)]
    pub external_pull: Option<String>,
    /// Contraintes signalées (input-only, strapping, flash ; section 7.2).
    #[serde(default)]
    pub constraints: Vec<String>,
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

impl Board {
    /// Signal d'une extrémité côté carte : pastille « CN6:4 » (à partir de 1)
    /// ou signal « PA5 », « GND ». None : inconnu ou non câblable (NC…).
    pub fn signal_of(&self, endpoint: &str) -> Option<String> {
        let signal = match endpoint.split_once(':') {
            Some((header, n)) => {
                let n: usize = n.parse().ok()?;
                let header = self
                    .layout
                    .as_ref()?
                    .headers
                    .iter()
                    .find(|h| h.id == header)?;
                header.pins.get(n.checked_sub(1)?)?.clone()
            }
            None => endpoint.to_string(),
        };
        (self.pins.contains_key(&signal) || self.power.contains_key(&signal)).then_some(signal)
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

    /// Pastilles physiques (« CN5:6 », à partir de 1) et signaux de la Nucleo.
    #[test]
    fn pads_resolve_to_signals() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../boards");
        let board = Board::load(&dir, "nucleo-f401re").expect("carte");
        assert_eq!(board.signal_of("CN5:6").as_deref(), Some("PA5")); // D13
        assert_eq!(board.signal_of("CN6:4").as_deref(), Some("3V3"));
        assert_eq!(board.signal_of("CN6:6").as_deref(), Some("GND"));
        assert_eq!(board.signal_of("CN5:7").as_deref(), Some("GND")); // autre pastille GND
        assert_eq!(board.signal_of("CN8:5").as_deref(), Some("PC1")); // A4
        assert_eq!(board.signal_of("CN6:1"), None); // NC
        assert_eq!(board.signal_of("CN9:1"), None); // D0 sans pont de soudure
        assert_eq!(board.signal_of("CN5:0"), None);
        assert_eq!(board.signal_of("CN5:11"), None);
        assert_eq!(board.signal_of("CN99:1"), None);
        assert_eq!(board.signal_of("PA5").as_deref(), Some("PA5"));
        assert_eq!(board.signal_of("PZ9"), None);
    }
}

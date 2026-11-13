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
    /// Notes posées sur la paillasse pour se repérer.
    #[serde(default)]
    pub notes: Vec<Note>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ComponentInstance {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    /// Nom affiché (« LED rouge ») ; l'identifiant reste celui des fils.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default)]
    pub params: BTreeMap<String, toml::Value>,
    pub pos: [f64; 2],
}

impl ComponentInstance {
    /// « LED rouge (led1) », ou l'identifiant seul sans nom affiché.
    pub fn label(&self) -> String {
        match self
            .name
            .as_deref()
            .map(str::trim)
            .filter(|n| !n.is_empty())
        {
            Some(n) => format!("{n} ({})", self.id),
            None => self.id.clone(),
        }
    }
}

/// Note libre posée sur le tapis (« Bloc alimentation »).
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct Note {
    pub text: String,
    pub pos: [f64; 2],
}

/// Longueurs maximales (docs/prebench.schema.json).
pub const MAX_NAME_LEN: usize = 40;
pub const MAX_NOTE_LEN: usize = 200;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Wire {
    pub from: String,
    pub to: String,
    /// Couleur choisie (charte, section 2.4) ; absente : selon la fonction.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
}

/// Couleurs de fil autorisées (docs/prebench.schema.json).
pub const WIRE_COLORS: &[&str] = &[
    "red", "black", "yellow", "green", "blue", "orange", "purple",
];

impl Project {
    pub fn load(dir: &Path) -> CoreResult<Self> {
        let path = dir.join("prebench.toml");
        if !path.exists() {
            return Ok(Project {
                board: DEFAULT_BOARD.into(),
                framework: DEFAULT_FRAMEWORK.into(),
                components: Vec::new(),
                wires: Vec::new(),
                notes: Vec::new(),
            });
        }
        let text = std::fs::read_to_string(&path)?;
        toml::from_str(&text).map_err(|e| CoreError::Project {
            path: path.display().to_string(),
            message: e.to_string(),
        })
    }
}

/// Enregistre composants et fils dans prebench.toml (paillasse éditée dans
/// l'app). Seuls les tableaux [[components]] et [[wires]] sont réécrits : le
/// reste du fichier, commentaires compris, est conservé (toml_edit).
pub fn save_bench(
    dir: &Path,
    components: &[ComponentInstance],
    wires: &[Wire],
    notes: &[Note],
) -> CoreResult<()> {
    let path = dir.join("prebench.toml");
    let invalid = |message: String| CoreError::Project {
        path: path.display().to_string(),
        message,
    };
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => format!(
            "#:schema https://raw.githubusercontent.com/rdkwcz08/prebench/main/docs/prebench.schema.json\n\
             format_version = 1\nboard          = \"{DEFAULT_BOARD}\"\nframework      = \"{DEFAULT_FRAMEWORK}\"\n"
        ),
        Err(e) => return Err(e.into()),
    };
    let mut doc: toml_edit::DocumentMut = text
        .parse()
        .map_err(|e: toml_edit::TomlError| invalid(e.to_string()))?;
    doc.remove("components");
    doc.remove("wires");
    doc.remove("notes");

    let number = |v: f64| -> toml_edit::Value {
        if v.fract() == 0.0 && v.abs() < 1e15 {
            toml_edit::Value::from(v as i64)
        } else {
            toml_edit::Value::from(v)
        }
    };
    let mut comps = toml_edit::ArrayOfTables::new();
    for c in components {
        let mut t = toml_edit::Table::new();
        t["id"] = toml_edit::value(c.id.as_str());
        t["type"] = toml_edit::value(c.kind.as_str());
        if let Some(name) = c.name.as_deref().map(str::trim).filter(|n| !n.is_empty()) {
            t["name"] = toml_edit::value(name);
        }
        let mut pos = toml_edit::Array::new();
        pos.push(number(c.pos[0]));
        pos.push(number(c.pos[1]));
        t["pos"] = toml_edit::value(pos);
        if !c.params.is_empty() {
            let mut params = toml_edit::InlineTable::new();
            for (k, v) in &c.params {
                let value: toml_edit::Value = v
                    .to_string()
                    .parse()
                    .map_err(|e: toml_edit::TomlError| invalid(e.to_string()))?;
                params.insert(k, value);
            }
            t["params"] = toml_edit::value(params);
        }
        comps.push(t);
    }
    let mut ws = toml_edit::ArrayOfTables::new();
    for w in wires {
        let mut t = toml_edit::Table::new();
        t["from"] = toml_edit::value(w.from.as_str());
        t["to"] = toml_edit::value(w.to.as_str());
        if let Some(color) = &w.color {
            t["color"] = toml_edit::value(color.as_str());
        }
        ws.push(t);
    }
    if !comps.is_empty() {
        doc.insert("components", toml_edit::Item::ArrayOfTables(comps));
    }
    if !ws.is_empty() {
        doc.insert("wires", toml_edit::Item::ArrayOfTables(ws));
    }
    let mut ns = toml_edit::ArrayOfTables::new();
    for n in notes {
        let mut t = toml_edit::Table::new();
        t["text"] = toml_edit::value(n.text.as_str());
        let mut pos = toml_edit::Array::new();
        pos.push(number(n.pos[0]));
        pos.push(number(n.pos[1]));
        t["pos"] = toml_edit::value(pos);
        ns.push(t);
    }
    if !ns.is_empty() {
        doc.insert("notes", toml_edit::Item::ArrayOfTables(ns));
    }
    std::fs::write(&path, doc.to_string())?;
    Ok(())
}

/// Extrémité de câble « <id>.<broche> » ; « board.CN6:4 » (pastille) ou
/// « board.PB9 » (signal) désigne la carte (Board::signal_of).
pub fn split_endpoint(endpoint: &str) -> Option<(&str, &str)> {
    endpoint.split_once('.')
}

#[cfg(test)]
mod tests {
    use super::*;

    /// La paillasse enregistrée garde commentaires et réglages, et se relit à l'identique.
    #[test]
    fn save_bench_keeps_the_rest_of_the_file() {
        let dir = std::env::temp_dir().join(format!("prebench-save-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("dossier");
        std::fs::write(
            dir.join("prebench.toml"),
            "# Mon projet\nformat_version = 1\nboard = \"nucleo-f401re\"\nframework = \"stm32-hal\" # HAL\n\n[build]\nsystem = \"cmake\"\n\n[[components]]\nid = \"old\"\ntype = \"led\"\npos = [0, 0]\n",
        )
        .expect("écriture");
        let comps = vec![ComponentInstance {
            id: "led1".into(),
            kind: "led".into(),
            name: Some("LED rouge".into()),
            params: BTreeMap::from([("color".to_string(), toml::Value::from("red"))]),
            pos: [720.0, 192.0],
        }];
        let wires = vec![
            Wire {
                from: "board.CN5:6".into(),
                to: "led1.A".into(),
                color: None,
            },
            Wire {
                from: "board.CN5:7".into(),
                to: "led1.K".into(),
                color: Some("black".into()),
            },
        ];
        let notes = vec![Note {
            text: "Bloc alimentation".into(),
            pos: [96.0, 480.0],
        }];
        save_bench(&dir, &comps, &wires, &notes).expect("enregistrement");
        let text = std::fs::read_to_string(dir.join("prebench.toml")).expect("lecture");
        assert!(
            text.contains("# Mon projet")
                && text.contains("# HAL")
                && text.contains("system = \"cmake\""),
            "{text}"
        );
        assert!(!text.contains("old"), "{text}");
        assert!(text.contains("pos = [720, 192]"), "{text}");
        let p = Project::load(&dir).expect("relecture");
        assert_eq!(p.components.len(), 1);
        assert_eq!(p.components[0].params["color"].as_str(), Some("red"));
        assert_eq!(p.wires.len(), 2);
        assert_eq!(p.wires[1].color.as_deref(), Some("black"));
        assert_eq!(p.components[0].name.as_deref(), Some("LED rouge"));
        assert_eq!(p.notes, notes);
        let _ = std::fs::remove_dir_all(&dir);
    }
}

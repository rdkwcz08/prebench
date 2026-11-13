// app/src-tauri/src/settings.rs — Réglages de Prebench (cahier des charges, section 10.7).
//
// Toute valeur réglable est déclarée ICI, une seule fois, avec sa portée et sa
// valeur par défaut : le cœur lit les constantes de `defaults`, l'interface
// reçoit la liste par get_settings. Deux niveaux :
//   - app : fichier settings.toml du dossier de configuration de l'app
//     (~/Library/Application Support/dev.prebench.app, %APPDATA%\dev.prebench.app) ;
//   - projet : table [settings] de prebench.toml, ajoutée au schéma avec le
//     premier réglage de projet modifiable (aucun pour l'instant).
// Un réglage « fixe » a sa valeur par défaut déclarée ici, sans encore pouvoir
// être modifié : l'écran des réglages et chaque modification arrivent quand ils
// deviennent utiles (section 10.7).

use std::path::Path;

use serde::Serialize;
use serde_json::Value;

use crate::error::{CoreError, CoreResult};

/// Valeurs par défaut lues par le cœur.
pub mod defaults {
    /// Pause automatique : CPU du firmware, en % d'un cœur…
    pub const CPU_LIMIT_PERCENT: f32 = 80.0;
    /// … pendant cette durée, en secondes.
    pub const CPU_LIMIT_SECONDS: u64 = 5;
    /// Plafond mémoire d'un firmware, en Mo.
    pub const MEMORY_LIMIT_MB: u64 = 256;
    /// Images par seconde de l'interface, sur secteur puis sur batterie.
    pub const FRAME_RATE: u64 = 60;
    pub const FRAME_RATE_BATTERY: u64 = 30;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Scope {
    App,
    Project,
}

/// Un réglage déclaré.
#[derive(Clone, Debug, Serialize)]
pub struct Setting {
    pub key: &'static str,
    pub scope: Scope,
    pub label: &'static str,
    /// Valeurs permises (texte) ; vide : nombre ou texte libre.
    pub values: &'static [&'static str],
    /// Modifiable par l'utilisateur dès maintenant ; sinon, valeur fixe.
    pub editable: bool,
    #[serde(skip)]
    pub default: fn() -> Value,
}

/// Tous les réglages (section 10.7).
pub fn declared() -> Vec<Setting> {
    vec![
        Setting {
            key: "theme",
            scope: Scope::App,
            label: "Thème",
            values: &["system", "light", "dark"],
            editable: true,
            default: || Value::from("system"),
        },
        Setting {
            key: "editor",
            scope: Scope::App,
            label: "Éditeur pour ouvrir fichier:ligne",
            values: &["vscode"],
            editable: false,
            default: || Value::from("vscode"),
        },
        Setting {
            key: "frame_rate",
            scope: Scope::App,
            label: "Images par seconde",
            values: &[],
            editable: false,
            default: || Value::from(defaults::FRAME_RATE),
        },
        Setting {
            key: "frame_rate_battery",
            scope: Scope::App,
            label: "Images par seconde sur batterie",
            values: &[],
            editable: false,
            default: || Value::from(defaults::FRAME_RATE_BATTERY),
        },
        Setting {
            key: "cpu_limit_percent",
            scope: Scope::Project,
            label: "Pause automatique : CPU du firmware (% d'un cœur)",
            values: &[],
            editable: false,
            default: || Value::from(defaults::CPU_LIMIT_PERCENT),
        },
        Setting {
            key: "cpu_limit_seconds",
            scope: Scope::Project,
            label: "Pause automatique : durée (s)",
            values: &[],
            editable: false,
            default: || Value::from(defaults::CPU_LIMIT_SECONDS),
        },
        Setting {
            key: "memory_limit_mb",
            scope: Scope::Project,
            label: "Plafond mémoire du firmware (Mo)",
            values: &[],
            editable: false,
            default: || Value::from(defaults::MEMORY_LIMIT_MB),
        },
    ]
}

const FILE: &str = "settings.toml";

/// Valeurs effectives : valeur par défaut, remplacée par celle du fichier de
/// l'app pour les réglages modifiables. Un fichier illisible est ignoré.
pub fn effective(config_dir: &Path) -> serde_json::Map<String, Value> {
    let saved: toml::Table = std::fs::read_to_string(config_dir.join(FILE))
        .ok()
        .and_then(|t| toml::from_str(&t).ok())
        .unwrap_or_default();
    declared()
        .into_iter()
        .map(|s| {
            let value = saved
                .get(s.key)
                .filter(|_| s.editable && s.scope == Scope::App)
                .and_then(|v| serde_json::to_value(v).ok())
                .filter(|v| accepts(&s, v))
                .unwrap_or_else(s.default);
            (s.key.to_string(), value)
        })
        .collect()
}

fn accepts(s: &Setting, v: &Value) -> bool {
    if s.values.is_empty() {
        return v.is_number() || v.is_string();
    }
    v.as_str().is_some_and(|x| s.values.contains(&x))
}

/// Modifie un réglage de l'app ; les autres réglages du fichier sont conservés.
pub fn set(config_dir: &Path, key: &str, value: &Value) -> CoreResult<()> {
    let invalid = |m: String| CoreError::Settings(m);
    let s = declared()
        .into_iter()
        .find(|s| s.key == key)
        .ok_or_else(|| invalid(format!("réglage inconnu : {key}")))?;
    if !s.editable || s.scope != Scope::App {
        return Err(invalid(format!(
            "le réglage {key} n'est pas encore modifiable"
        )));
    }
    if !accepts(&s, value) {
        return Err(invalid(format!("valeur refusée pour {key} : {value}")));
    }
    let path = config_dir.join(FILE);
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    let mut doc: toml_edit::DocumentMut = text.parse().unwrap_or_default();
    doc[key] = match value {
        Value::String(x) => toml_edit::value(x.as_str()),
        Value::Number(n) if n.is_i64() => toml_edit::value(n.as_i64().unwrap_or_default()),
        Value::Number(n) => toml_edit::value(n.as_f64().unwrap_or_default()),
        _ => return Err(invalid(format!("valeur refusée pour {key}"))),
    };
    std::fs::create_dir_all(config_dir)?;
    std::fs::write(&path, doc.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_then_saved_value() {
        let dir = std::env::temp_dir().join(format!("prebench-settings-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let v = effective(&dir);
        assert_eq!(v["theme"], "system");
        assert_eq!(v["memory_limit_mb"], defaults::MEMORY_LIMIT_MB);
        set(&dir, "theme", &Value::from("dark")).expect("thème");
        assert_eq!(effective(&dir)["theme"], "dark");
        assert!(set(&dir, "theme", &Value::from("violet")).is_err());
        assert!(set(&dir, "memory_limit_mb", &Value::from(512)).is_err()); // pas encore modifiable
        assert!(set(&dir, "inconnu", &Value::from(1)).is_err());
        // Fichier modifié à la main : valeur invalide ignorée, défaut conservé.
        std::fs::write(dir.join(FILE), "theme = \"violet\"\n").expect("écriture");
        assert_eq!(effective(&dir)["theme"], "system");
        let keys: Vec<&str> = declared().iter().map(|s| s.key).collect();
        let mut unique = keys.clone();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(keys.len(), unique.len(), "clé en double");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

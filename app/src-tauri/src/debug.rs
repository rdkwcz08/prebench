// app/src-tauri/src/debug.rs — Débogage du firmware dans VS Code (section 10.2).
//
// L'app ne lance pas le firmware : elle écrit dans .vscode/launch.json du
// projet une configuration « Prebench : déboguer le firmware » qui le lance
// sous le débogueur (CodeLLDB sur macOS, extension C/C++ et gdb sous Windows et
// Linux), avec l'adresse fixe du canal du projet ; puis elle attend sa
// connexion (core/sim.rs, DebugConfig). Les autres configurations du fichier
// sont conservées ; ses commentaires ne le sont pas quand il est réécrit.

use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::{json, Value};

use crate::build;
use crate::error::{CoreError, CoreResult};
use crate::platform;

/// Nom de la configuration dans launch.json.
pub const CONFIG_NAME: &str = "Prebench : déboguer le firmware";

/// Adresse fixe du canal de débogage du projet.
pub fn address(project: &Path) -> String {
    let key = build::fnv1a(
        platform::paths::simplified(project)
            .to_string_lossy()
            .as_bytes(),
    );
    platform::debug_socket_address(key)
}

/// Exécutable lancé par le débogueur : une copie dans le dossier de build,
/// remplacée entre deux sessions (Windows verrouille celui qui tourne).
pub fn program(build_dir: &Path) -> PathBuf {
    build_dir.join("debug").join(platform::firmware_file_name())
}

/// Installe `firmware` comme exécutable du débogueur : copie, puis
/// informations de débogage autonomes (.dSYM sous macOS), pour que les points
/// d'arrêt se résolvent même après la suppression ou la recompilation des .o.
/// Bloquant (dsymutil) : à appeler hors de la boucle asynchrone.
pub fn install_program(firmware: &Path, program: &Path) -> CoreResult<()> {
    if let Some(parent) = program.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::copy(firmware, program)?;
    let mut dsym = program.as_os_str().to_owned();
    dsym.push(".dSYM");
    // Ignoré : absent au premier lancement ; remplacé juste après.
    let _ = std::fs::remove_dir_all(PathBuf::from(dsym));
    platform::bundle_debug_symbols(program)
        .map_err(|e| CoreError::Debug(format!("informations de débogage (dsymutil) : {e}")))
}

/// Configuration VS Code pour ce système.
pub fn launch_config(program: &Path, project: &Path, address: &str, gdb: Option<&Path>) -> Value {
    let env = [
        ("PREBENCH_SOCKET", address.to_string()),
        ("PREBENCH_BOARD_ID", "1".to_string()),
        ("PREBENCH_DEBUG", "1".to_string()),
    ];
    if cfg!(target_os = "macos") {
        let env: serde_json::Map<String, Value> = env
            .into_iter()
            .map(|(k, v)| (k.to_string(), Value::from(v)))
            .collect();
        json!({
            "name": CONFIG_NAME,
            "type": "lldb",
            "request": "launch",
            "program": program,
            "cwd": project,
            "env": env,
        })
    } else {
        let mut environment: Vec<Value> = env
            .into_iter()
            .map(|(k, v)| json!({ "name": k, "value": v }))
            .collect();
        if cfg!(windows) {
            // DLL d'exécution de MinGW-w64, à côté de gdb (MSYS2 UCRT64).
            if let Some(dir) = gdb.and_then(Path::parent) {
                environment.push(json!({
                    "name": "PATH",
                    "value": format!("{};${{env:PATH}}", dir.display()),
                }));
            }
        }
        json!({
            "name": CONFIG_NAME,
            "type": "cppdbg",
            "request": "launch",
            "program": program,
            "cwd": project,
            "environment": environment,
            "MIMode": "gdb",
            "miDebuggerPath": gdb.map_or_else(|| "gdb".to_string(), |g| g.display().to_string()),
            "stopAtEntry": false,
            "externalConsole": false,
        })
    }
}

/// Ce qui a été fait de .vscode/launch.json.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LaunchJson {
    Created,
    Updated,
    Unchanged,
}

/// Ajoute ou remplace la configuration Prebench dans `<projet>/.vscode/launch.json`.
pub fn upsert_launch_json(project: &Path, config: &Value) -> CoreResult<LaunchJson> {
    let path = project.join(".vscode").join("launch.json");
    let (mut root, outcome) = match std::fs::read_to_string(&path) {
        Ok(text) => {
            let root: Value = serde_json::from_str(&strip_jsonc(&text)).map_err(|e| {
                CoreError::Debug(format!(
                    "{} illisible ({e}) : ajoutez à la main la configuration affichée dans le Journal",
                    path.display()
                ))
            })?;
            (root, LaunchJson::Updated)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => (
            json!({ "version": "0.2.0", "configurations": [] }),
            LaunchJson::Created,
        ),
        Err(e) => return Err(e.into()),
    };
    let configs = root
        .get_mut("configurations")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| {
            CoreError::Debug(format!(
                "{} : tableau « configurations » absent",
                path.display()
            ))
        })?;
    match configs.iter_mut().find(|c| c["name"] == CONFIG_NAME) {
        Some(existing) if existing == config => return Ok(LaunchJson::Unchanged),
        Some(existing) => *existing = config.clone(),
        None => configs.push(config.clone()),
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let text = serde_json::to_string_pretty(&root).map_err(|e| CoreError::Debug(e.to_string()))?;
    std::fs::write(&path, text + "\n")?;
    Ok(outcome)
}

/// Réglage d'IntelliSense (extension C/C++) qui désigne la base de compilation.
pub const COMPILE_COMMANDS_KEY: &str = "C_Cpp.default.compileCommands";

/// Inscrit `compile_commands` dans `<projet>/.vscode/settings.json` pour
/// qu'IntelliSense trouve les en-têtes du shim (stm32f4xx_hal.h…). Les autres
/// réglages sont conservés ; le fichier n'est réécrit que si la valeur change.
/// Renvoie true si le fichier a été écrit.
pub fn upsert_settings(project: &Path, compile_commands: &Path) -> CoreResult<bool> {
    let path = project.join(".vscode").join("settings.json");
    let value = Value::from(compile_commands.display().to_string());
    let mut root: Value = match std::fs::read_to_string(&path) {
        Ok(text) if text.trim().is_empty() => json!({}),
        Ok(text) => serde_json::from_str(&strip_jsonc(&text)).map_err(|e| {
            CoreError::Debug(format!(
                "{} illisible ({e}) : IntelliSense non configuré",
                path.display()
            ))
        })?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => json!({}),
        Err(e) => return Err(e.into()),
    };
    let obj = root
        .as_object_mut()
        .ok_or_else(|| CoreError::Debug(format!("{} : objet JSON attendu", path.display())))?;
    if obj.get(COMPILE_COMMANDS_KEY) == Some(&value) {
        return Ok(false);
    }
    obj.insert(COMPILE_COMMANDS_KEY.to_string(), value);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let text = serde_json::to_string_pretty(&root).map_err(|e| CoreError::Debug(e.to_string()))?;
    std::fs::write(&path, text + "\n")?;
    Ok(true)
}

/// JSON avec commentaires (format de VS Code) vers JSON strict : commentaires
/// `//` et `/* */` hors des chaînes, et virgules finales, retirés.
pub fn strip_jsonc(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    let mut in_string = false;
    while let Some(c) = chars.next() {
        if in_string {
            out.push(c);
            if c == '\\' {
                if let Some(n) = chars.next() {
                    out.push(n);
                }
            } else if c == '"' {
                in_string = false;
            }
            continue;
        }
        match (c, chars.peek()) {
            ('"', _) => {
                in_string = true;
                out.push(c);
            }
            ('/', Some('/')) => {
                for n in chars.by_ref() {
                    if n == '\n' {
                        out.push('\n');
                        break;
                    }
                }
            }
            ('/', Some('*')) => {
                chars.next();
                let mut prev = ' ';
                for n in chars.by_ref() {
                    if prev == '*' && n == '/' {
                        break;
                    }
                    prev = n;
                }
            }
            _ => out.push(c),
        }
    }
    // Virgules finales : « , » suivie (aux blancs près) de « } » ou « ] ».
    let bytes: Vec<char> = out.chars().collect();
    let mut cleaned = String::with_capacity(out.len());
    let mut in_string = false;
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        if in_string {
            if c == '\\' {
                cleaned.push(c);
                i += 1;
                if let Some(n) = bytes.get(i) {
                    cleaned.push(*n);
                }
                i += 1;
                continue;
            }
            if c == '"' {
                in_string = false;
            }
        } else if c == '"' {
            in_string = true;
        } else if c == ',' {
            let next = bytes[i + 1..].iter().find(|n| !n.is_whitespace());
            if matches!(next, Some('}') | Some(']')) {
                i += 1;
                continue;
            }
        }
        cleaned.push(c);
        i += 1;
    }
    cleaned
}

/// Outil de débogage vérifié (extension ou débogueur).
#[derive(Clone, Debug, Serialize)]
pub struct DebugTool {
    pub label: String,
    pub present: bool,
    /// Chemin trouvé (débogueur) ou dossier de l'extension.
    pub path: Option<String>,
    /// Commande d'installation.
    pub install: String,
}

/// Extension VS Code et débogueur : présents ou commande d'installation.
pub fn tools(
    home: Option<&Path>,
    search_path: &std::ffi::OsStr,
) -> (Vec<DebugTool>, Option<PathBuf>) {
    let req = platform::debug_requirements();
    let mut out = Vec::new();
    let extension = home.and_then(|h| find_extension(h, req.extension_id));
    out.push(DebugTool {
        label: req.extension_label.to_string(),
        present: extension.is_some(),
        path: extension.map(|p| p.display().to_string()),
        install: format!("code --install-extension {}", req.extension_id),
    });
    let mut gdb = None;
    if let Some((label, names)) = req.debugger {
        gdb = std::env::split_paths(search_path)
            .flat_map(|d| names.iter().map(move |n| d.join(n)))
            .find(|p| p.is_file());
        out.push(DebugTool {
            label: label.to_string(),
            present: gdb.is_some(),
            path: gdb.as_ref().map(|p| p.display().to_string()),
            install: req.debugger_install.to_string(),
        });
    }
    (out, gdb)
}

/// Dossier d'une extension installée : `~/.vscode/extensions/<id>-<version>…`
/// (VS Code, puis VS Code Insiders).
pub fn find_extension(home: &Path, id: &str) -> Option<PathBuf> {
    let prefix = format!("{}-", id.to_ascii_lowercase());
    [".vscode", ".vscode-insiders"].iter().find_map(|d| {
        std::fs::read_dir(home.join(d).join("extensions"))
            .ok()?
            .flatten()
            .map(|e| e.path())
            .find(|p| {
                p.is_dir()
                    && p.file_name().is_some_and(|n| {
                        n.to_string_lossy()
                            .to_ascii_lowercase()
                            .starts_with(&prefix)
                    })
            })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("prebench-debug-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).expect("dossier");
        d
    }

    #[test]
    fn jsonc_comments_and_trailing_commas() {
        let text = r#"{
            // Use IntelliSense to learn about possible attributes.
            "version": "0.2.0", /* bloc */
            "configurations": [
                { "name": "a // pas un commentaire", "url": "http://x/*y*/", "q": "\"," },
            ],
        }"#;
        let v: Value = serde_json::from_str(&strip_jsonc(text)).expect("JSON valide");
        assert_eq!(v["configurations"][0]["name"], "a // pas un commentaire");
        assert_eq!(v["configurations"][0]["url"], "http://x/*y*/");
        assert_eq!(v["configurations"][0]["q"], "\",");
    }

    #[test]
    fn launch_json_created_updated_and_kept() {
        let dir = temp("launch");
        let program = dir.join("build").join("firmware");
        let cfg = launch_config(&program, &dir, &address(&dir), None);
        assert_eq!(cfg["name"], CONFIG_NAME);
        assert_eq!(
            upsert_launch_json(&dir, &cfg).expect("création"),
            LaunchJson::Created
        );
        assert_eq!(
            upsert_launch_json(&dir, &cfg).expect("relecture"),
            LaunchJson::Unchanged
        );

        // Fichier de l'utilisateur avec commentaires et une autre configuration.
        let path = dir.join(".vscode").join("launch.json");
        std::fs::write(
            &path,
            "{\n // mes réglages\n \"version\": \"0.2.0\",\n \"configurations\": [\n  {\"name\": \"Autre\", \"type\": \"node\"},\n ]\n}\n",
        )
        .expect("écriture");
        assert_eq!(
            upsert_launch_json(&dir, &cfg).expect("ajout"),
            LaunchJson::Updated
        );
        let v: Value =
            serde_json::from_str(&std::fs::read_to_string(&path).expect("lecture")).expect("JSON");
        let names: Vec<&str> = v["configurations"]
            .as_array()
            .expect("tableau")
            .iter()
            .filter_map(|c| c["name"].as_str())
            .collect();
        assert_eq!(names, ["Autre", CONFIG_NAME]);

        std::fs::write(&path, "{ pas du json").expect("écriture");
        assert!(upsert_launch_json(&dir, &cfg).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn settings_keep_user_values() {
        let dir = temp("settings");
        let cc = dir.join("build").join("compile_commands.json");
        assert!(upsert_settings(&dir, &cc).expect("création"));
        assert!(!upsert_settings(&dir, &cc).expect("inchangé"));
        let path = dir.join(".vscode").join("settings.json");
        std::fs::write(&path, "{\n // à moi\n \"editor.tabSize\": 2,\n}\n").expect("écriture");
        assert!(upsert_settings(&dir, &cc).expect("ajout"));
        let v: Value =
            serde_json::from_str(&std::fs::read_to_string(&path).expect("lecture")).expect("JSON");
        assert_eq!(v["editor.tabSize"], 2);
        assert_eq!(v[COMPILE_COMMANDS_KEY], cc.display().to_string());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn address_is_stable_per_project() {
        let a = address(Path::new("/Users/me/p1"));
        assert_eq!(a, address(Path::new("/Users/me/p1")));
        assert_ne!(a, address(Path::new("/Users/me/p2")));
        assert!(a.len() < 100, "adresse trop longue pour macOS : {a}");
    }

    #[test]
    fn extension_detection() {
        let home = temp("home");
        let id = platform::debug_requirements().extension_id;
        assert!(find_extension(&home, id).is_none());
        std::fs::create_dir_all(
            home.join(".vscode")
                .join("extensions")
                .join(format!("{id}-1.11.0-darwin-arm64")),
        )
        .expect("dossier");
        assert!(find_extension(&home, id).is_some());
        let (tools, _) = tools(Some(&home), std::ffi::OsStr::new(""));
        assert!(tools[0].present);
        assert!(tools[0].install.contains(id));
        let _ = std::fs::remove_dir_all(&home);
    }
}

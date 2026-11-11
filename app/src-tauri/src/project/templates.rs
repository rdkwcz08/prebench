// app/src-tauri/src/project/templates.rs — Nouveau projet par copie d'un modèle
// (section 10.1).
//
// Les modèles sont dans <racine Prebench>/templates/<id>/. Un nouveau projet
// en est une copie dans un dossier de l'utilisateur : le modèle installé reste
// intact (le dossier d'installation est en lecture seule sous Windows).

use std::path::{Path, PathBuf};

use crate::error::{CoreError, CoreResult};

/// Profondeur maximale d'un modèle (règle 16.2.11 : tout est borné).
const MAX_DEPTH: usize = 16;

/// Dossiers jamais copiés : builds locaux et fichiers du système.
fn is_skipped(name: &str, is_dir: bool) -> bool {
    if is_dir {
        name == "build" || name.starts_with("build-") || name.starts_with("cmake-build-")
    } else {
        matches!(name, ".DS_Store" | "Thumbs.db" | "Desktop.ini")
    }
}

/// Identifiants des modèles disponibles (dossiers contenant prebench.toml).
pub fn list(templates_dir: &Path) -> Vec<String> {
    let mut ids: Vec<String> = std::fs::read_dir(templates_dir)
        .map(|entries| {
            entries
                .flatten()
                .filter(|e| e.path().join("prebench.toml").is_file())
                .filter_map(|e| e.file_name().to_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default();
    ids.sort();
    ids
}

/// Dossier proposé pour un nouveau projet : `<parent>/<id>`, puis `<id>-2`,
/// `<id>-3`… si le dossier existe déjà.
pub fn suggested_dir(parent: &Path, id: &str) -> PathBuf {
    let first = parent.join(id);
    if !first.exists() {
        return first;
    }
    (2..1000)
        .map(|n| parent.join(format!("{id}-{n}")))
        .find(|p| !p.exists())
        .unwrap_or(first)
}

/// Copie le modèle `id` dans `dest`, qui doit être absent ou vide.
pub fn create(templates_dir: &Path, id: &str, dest: &Path) -> CoreResult<()> {
    let err = |message: String| CoreError::Project {
        path: dest.display().to_string(),
        message,
    };
    // L'identifiant vient de l'interface : seul un modèle listé est accepté.
    if !list(templates_dir).iter().any(|t| t == id) {
        return Err(err(format!("modèle « {id} » inconnu")));
    }
    if dest.is_file() {
        return Err(err("un fichier porte déjà ce nom".into()));
    }
    if dest.is_dir() && std::fs::read_dir(dest)?.next().is_some() {
        return Err(err("le dossier existe déjà et n'est pas vide".into()));
    }
    copy_dir(&templates_dir.join(id), dest, 0)
}

/// Copie récursive, sans les builds locaux ni les liens symboliques.
pub fn copy_dir(src: &Path, dest: &Path, depth: usize) -> CoreResult<()> {
    if depth > MAX_DEPTH {
        return Err(CoreError::Project {
            path: src.display().to_string(),
            message: "arborescence trop profonde".into(),
        });
    }
    std::fs::create_dir_all(dest)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        let name = entry.file_name();
        let name_text = name.to_string_lossy();
        if kind.is_symlink() || is_skipped(&name_text, kind.is_dir()) {
            continue;
        }
        let target = dest.join(&name);
        if kind.is_dir() {
            copy_dir(&entry.path(), &target, depth + 1)?;
        } else {
            std::fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo_templates() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("templates")
    }

    fn temp_dir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("prebench-templates-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn creates_a_copy_without_local_builds() {
        let dest = temp_dir("copie").join("Mon blinky");
        create(&repo_templates(), "blinky-stm32-hal", &dest).expect("copie du modèle");
        assert!(dest.join("prebench.toml").is_file());
        assert!(dest.join("Core").join("Src").join("main.c").is_file());
        assert!(!dest.join("build-sim").exists());
        // Un dossier non vide est refusé.
        assert!(create(&repo_templates(), "blinky-stm32-hal", &dest).is_err());
        let _ = std::fs::remove_dir_all(dest.parent().unwrap_or(&dest));
    }

    #[test]
    fn unknown_template_is_refused() {
        let dest = temp_dir("inconnu");
        assert!(create(&repo_templates(), "../libvmcu", &dest).is_err());
        assert!(!dest.exists());
    }

    #[test]
    fn suggested_dir_avoids_existing_folders() {
        let parent = temp_dir("suggestion");
        std::fs::create_dir_all(parent.join("blinky")).expect("dossier");
        assert_eq!(suggested_dir(&parent, "blinky"), parent.join("blinky-2"));
        let _ = std::fs::remove_dir_all(parent);
    }
}

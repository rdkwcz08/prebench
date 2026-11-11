// app/src-tauri/src/build/mod.rs — Build « sim » d'un projet (phase 1 : à la demande).
//
// - Projet avec CMakeLists.txt Prebench (modèles) : configuré directement,
//   avec -DPREBENCH_ROOT.
// - Projet STM32CubeMX brut (Core/Src, CMakeLists.txt de CubeMX ou absent) :
//   compilé via libvmcu/sim, sans le modifier.
//
// Le dossier de build est dans le cache de l'app, jamais dans le projet ni
// dans le dossier d'installation (en lecture seule sous Windows). Il est
// supprimé quand la configuration CMake échoue : un cache CMake corrompu ne
// bloque pas le build suivant. Une erreur de compilation du code le garde,
// pour que la correction ne relance pas une recompilation complète.
// La surveillance du dossier et la recompilation à chaque sauvegarde arrivent
// en phase 2 (section 10.2).

use std::path::{Path, PathBuf};
use std::process::Stdio;

use crate::error::{CoreError, CoreResult};
use crate::platform;
use crate::platform::paths::cmake_path;
use crate::platform::toolchain::{ToolKind, Toolchain};

/// Renvoi vers la documentation d'installation des prérequis.
pub const README_PREREQUISITES: &str =
    "README, section « Prérequis utilisateur » : https://github.com/rdkwcz08/prebench#prérequis-utilisateur-simuler-un-firmware";

/// Longueur maximale du nom lisible d'un dossier de build.
const MAX_NAME_LEN: usize = 40;

fn is_raw_cubemx(project: &Path) -> bool {
    if !project.join("Core").join("Src").is_dir() {
        return false;
    }
    match std::fs::read_to_string(project.join("CMakeLists.txt")) {
        Ok(text) => !text.contains("PREBENCH_ROOT"),
        Err(_) => true,
    }
}

/// Empreinte FNV-1a 64 bits, stable d'une version à l'autre de Rust (le
/// hacheur de la bibliothèque standard ne l'est pas).
fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |h, b| {
        (h ^ u64::from(*b)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

/// Dossier de build d'un projet : `<cache>/builds/<nom>-<empreinte du chemin>`.
/// Deux projets de même nom dans des dossiers différents ne partagent rien.
pub fn build_dir_for(cache_dir: &Path, project: &Path) -> PathBuf {
    let project = platform::paths::simplified(project);
    let name: String = project
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .take(MAX_NAME_LEN)
        .collect();
    let hash = fnv1a(project.to_string_lossy().as_bytes());
    cache_dir.join("builds").join(format!("{name}-{hash:016x}"))
}

async fn run(
    program: &Path,
    args: &[String],
    toolchain: &Toolchain,
    log: &mut String,
) -> CoreResult<()> {
    let output = platform::tool_command(program)
        .args(args)
        .env("PATH", &toolchain.search_path)
        .stdin(Stdio::null())
        .output()
        .await
        .map_err(|e| {
            CoreError::Build(format!("{} impossible à lancer : {e}", program.display()))
        })?;
    log.push_str(&String::from_utf8_lossy(&output.stdout));
    log.push_str(&String::from_utf8_lossy(&output.stderr));
    if output.status.success() {
        Ok(())
    } else {
        Err(CoreError::Build(log.clone()))
    }
}

/// Message d'erreur listant les outils absents et leur commande d'installation.
fn missing_tools_message(toolchain: &Toolchain) -> Option<String> {
    let missing: Vec<String> = toolchain
        .missing()
        .map(|t| {
            format!(
                "❌ {} introuvable. Pour l'installer :\n{}",
                t.label, t.install
            )
        })
        .collect();
    if missing.is_empty() {
        None
    } else {
        Some(format!(
            "{}\nVoir le {README_PREREQUISITES}",
            missing.join("\n")
        ))
    }
}

/// Configure et compile dans `build_dir` ; renvoie le chemin de l'exécutable
/// « sim ». Si la configuration échoue, `build_dir` est supprimé.
pub async fn build_sim(
    project: &Path,
    prebench_root: &Path,
    build_dir: &Path,
    toolchain: &Toolchain,
) -> CoreResult<PathBuf> {
    if let Some(message) = missing_tools_message(toolchain) {
        return Err(CoreError::Build(message));
    }
    let cmake = toolchain
        .path_of(ToolKind::Cmake)
        .ok_or_else(|| CoreError::Build("CMake introuvable".into()))?;
    let mut log = String::new();
    if let Err(e) = configure(
        cmake,
        project,
        prebench_root,
        build_dir,
        toolchain,
        &mut log,
    )
    .await
    {
        // Ignoré : le dossier peut ne jamais avoir été créé.
        let _ = std::fs::remove_dir_all(build_dir);
        return Err(e);
    }
    let build_args = vec![
        "--build".to_string(),
        cmake_path(build_dir),
        "--parallel".to_string(),
    ];
    run(cmake, &build_args, toolchain, &mut log).await?;

    let exe = build_dir.join(platform::firmware_file_name());
    if exe.is_file() {
        Ok(exe)
    } else {
        Err(CoreError::Build(format!(
            "exécutable introuvable : {}\n{log}",
            exe.display()
        )))
    }
}

/// Étape de configuration CMake (génère ou met à jour le cache).
async fn configure(
    cmake: &Path,
    project: &Path,
    prebench_root: &Path,
    build_dir: &Path,
    toolchain: &Toolchain,
    log: &mut String,
) -> CoreResult<()> {
    std::fs::create_dir_all(build_dir)?;
    let mut args: Vec<String> = vec!["-S".into()];
    if is_raw_cubemx(project) {
        args.push(cmake_path(&prebench_root.join("libvmcu").join("sim")));
        args.push(format!("-DPREBENCH_PROJECT={}", cmake_path(project)));
    } else {
        args.push(cmake_path(project));
        args.push(format!("-DPREBENCH_ROOT={}", cmake_path(prebench_root)));
    }
    args.push("-B".into());
    args.push(cmake_path(build_dir));
    args.extend(platform::cmake_generator_args());

    run(cmake, &args, toolchain, log).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_dirs_are_distinct_and_readable() {
        let cache = Path::new("/cache");
        let a = build_dir_for(cache, Path::new("/home/a/Mon projet"));
        let b = build_dir_for(cache, Path::new("/home/b/Mon projet"));
        assert_ne!(a, b);
        let name = a.file_name().map(|n| n.to_string_lossy().into_owned());
        assert!(name.is_some_and(|n| n.starts_with("Mon_projet-")));
        assert_eq!(a.parent(), Some(cache.join("builds").as_path()));
        // Stable : même projet, même dossier.
        assert_eq!(a, build_dir_for(cache, Path::new("/home/a/Mon projet")));
    }
}

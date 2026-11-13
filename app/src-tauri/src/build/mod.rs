// app/src-tauri/src/build/mod.rs — Build « sim » d'un projet.
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
// Le build est lancé au démarrage et à chaque sauvegarde (session.rs,
// section 10.2) ; la simulation exécute une copie de l'exécutable (`run_copy`).

pub mod diagnostics;

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{SystemTime, UNIX_EPOCH};

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
pub(crate) fn fnv1a(bytes: &[u8]) -> u64 {
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
                "{} introuvable. Installez-le avec la commande ci-dessous :\n{}",
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
#[cfg_attr(not(test), allow(dead_code))] // utilisé par les tests du cœur
pub async fn build_sim(
    project: &Path,
    prebench_root: &Path,
    build_dir: &Path,
    toolchain: &Toolchain,
) -> CoreResult<PathBuf> {
    build_sim_logged(project, prebench_root, build_dir, toolchain)
        .await
        .map(|(exe, _)| exe)
}

/// Comme `build_sim`, avec la sortie de CMake et du compilateur (avertissements).
pub async fn build_sim_logged(
    project: &Path,
    prebench_root: &Path,
    build_dir: &Path,
    toolchain: &Toolchain,
) -> CoreResult<(PathBuf, String)> {
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
        Ok((exe, log))
    } else {
        Err(CoreError::Build(format!(
            "exécutable introuvable : {}\n{log}",
            exe.display()
        )))
    }
}

/// Copie de l'exécutable lancée par la simulation, dans `<build>/run/` : le build
/// suivant peut réécrire l'original pendant qu'elle tourne (Windows verrouille
/// un .exe en cours d'exécution). Les anciennes copies sont supprimées quand
/// elles ne servent plus ; sous Windows, celle qui tourne encore reste.
pub fn run_copy(exe: &Path, build_dir: &Path) -> CoreResult<PathBuf> {
    let dir = build_dir.join("run");
    std::fs::create_dir_all(&dir)?;
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            // Ignoré : une copie encore en cours d'exécution est verrouillée.
            let _ = std::fs::remove_file(entry.path());
        }
    }
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or_default();
    let stem = exe
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "firmware".into());
    let ext = exe
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_default();
    let dest = dir.join(format!("{stem}-{stamp}{ext}"));
    std::fs::copy(exe, &dest)?;
    Ok(dest)
}

/// Base de compilation produite par CMake (IntelliSense de VS Code).
pub fn compile_commands(build_dir: &Path) -> PathBuf {
    build_dir.join("compile_commands.json")
}

/// make compare les dates à la seconde près (mingw32-make sous Windows) : une
/// source enregistrée dans la même seconde que la fin du build précédent
/// paraîtrait aussi récente que son objet, et ne serait pas recompilée. Son
/// objet est supprimé du dossier de build (dans le cache, jamais dans le
/// projet) ; pour un en-tête ou un fichier CMake, tous les objets du projet.
pub fn invalidate_stale_objects(
    build_dir: &Path,
    files: &[PathBuf],
    last_build_end: Option<SystemTime>,
) {
    let Some(end) = last_build_end else { return };
    let second = |t: SystemTime| {
        t.duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    };
    let mut names = Vec::new();
    let mut all = false;
    for f in files {
        let Ok(m) = std::fs::metadata(f).and_then(|m| m.modified()) else {
            continue;
        };
        if second(m) > second(end) {
            continue;
        }
        let compiled = f.extension().is_some_and(|e| {
            ["c", "cc", "cpp", "cxx", "s", "S"].contains(&e.to_string_lossy().as_ref())
        });
        match f.file_name() {
            Some(name) if compiled => names.push(name.to_string_lossy().into_owned()),
            _ => all = true,
        }
    }
    if names.is_empty() && !all {
        return;
    }
    let mut stack = vec![build_dir.to_path_buf()];
    let mut visited = 0_usize;
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            visited += 1;
            if visited > 100_000 {
                return; // borne (règle 16.2.11)
            }
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            if entry.file_type().is_ok_and(|t| t.is_dir()) {
                // libvmcu ne dépend pas des sources du projet ; run/ et debug/ : exécutables.
                if !["libvmcu", "run", "debug"].contains(&name.as_str()) {
                    stack.push(path);
                }
                continue;
            }
            let Some(source) = name
                .strip_suffix(".obj")
                .or_else(|| name.strip_suffix(".o"))
            else {
                continue;
            };
            if all || names.iter().any(|n| n == source) {
                // Ignoré : un objet déjà absent sera de toute façon recompilé.
                let _ = std::fs::remove_file(&path);
            }
        }
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
    // Débogage dans VS Code (section 10.2) : informations de débogage, sans
    // optimisation (-g, -O0 implicite) ; compile_commands.json pour IntelliSense.
    args.push("-DCMAKE_BUILD_TYPE=Debug".into());
    args.push("-DCMAKE_EXPORT_COMPILE_COMMANDS=ON".into());
    args.extend(platform::cmake_generator_args());

    run(cmake, &args, toolchain, log).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_cubemx_detection() {
        let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
        // CMakeLists.txt généré par CubeMX : compilé via libvmcu/sim.
        assert!(is_raw_cubemx(
            &repo.join("tests").join("cubemx-nucleo-f401re")
        ));
        // Modèle Prebench (PREBENCH_ROOT) : configuré directement.
        assert!(!is_raw_cubemx(
            &repo.join("templates").join("blinky-stm32-hal")
        ));
    }

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

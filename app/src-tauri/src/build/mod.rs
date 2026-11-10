// app/src-tauri/src/build/mod.rs — Build « sim » d'un projet (phase 1 : à la demande).
//
// - Projet avec CMakeLists.txt Prebench (modèles) : configuré directement,
//   avec -DPREBENCH_ROOT.
// - Projet STM32CubeMX brut (Core/Src, CMakeLists.txt de CubeMX ou absent) :
//   compilé via libvmcu/sim, sans le modifier.
// La surveillance du dossier et la recompilation à chaque sauvegarde arrivent
// en phase 2 (section 10.2).

use std::path::{Path, PathBuf};
use std::process::Stdio;

use crate::error::{CoreError, CoreResult};
use crate::platform;

pub const BUILD_DIR: &str = "build-sim";

fn is_raw_cubemx(project: &Path) -> bool {
    if !project.join("Core").join("Src").is_dir() {
        return false;
    }
    match std::fs::read_to_string(project.join("CMakeLists.txt")) {
        Ok(text) => !text.contains("PREBENCH_ROOT"),
        Err(_) => true,
    }
}

async fn run(program: &str, args: &[String], log: &mut String) -> CoreResult<()> {
    let output = platform::tool_command(program)
        .args(args)
        .env("PATH", platform::tool_path())
        .stdin(Stdio::null())
        .output()
        .await
        .map_err(|e| {
            CoreError::Build(format!(
                "{program} introuvable ou impossible à lancer : {e}"
            ))
        })?;
    log.push_str(&String::from_utf8_lossy(&output.stdout));
    log.push_str(&String::from_utf8_lossy(&output.stderr));
    if output.status.success() {
        Ok(())
    } else {
        Err(CoreError::Build(log.clone()))
    }
}

/// Configure et compile ; renvoie le chemin de l'exécutable « sim ».
pub async fn build_sim(project: &Path, prebench_root: &Path) -> CoreResult<PathBuf> {
    let build_dir = project.join(BUILD_DIR);
    let mut args: Vec<String> = Vec::new();
    if is_raw_cubemx(project) {
        args.push("-S".into());
        args.push(
            prebench_root
                .join("libvmcu")
                .join("sim")
                .display()
                .to_string(),
        );
        args.push(format!("-DPREBENCH_PROJECT={}", project.display()));
    } else {
        args.push("-S".into());
        args.push(project.display().to_string());
        args.push(format!("-DPREBENCH_ROOT={}", prebench_root.display()));
    }
    args.push("-B".into());
    args.push(build_dir.display().to_string());
    args.extend(platform::cmake_generator_args());

    let mut log = String::new();
    run("cmake", &args, &mut log).await?;
    let build_args = vec![
        "--build".to_string(),
        build_dir.display().to_string(),
        "--parallel".to_string(),
    ];
    run("cmake", &build_args, &mut log).await?;

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

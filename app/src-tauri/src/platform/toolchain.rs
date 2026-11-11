// app/src-tauri/src/platform/toolchain.rs — Outils de build du firmware.
//
// Une app lancée depuis le Finder (macOS) ou installée par le .msi (Windows)
// ne voit pas forcément les outils dans son PATH : l'installateur de CMake
// pour Windows ne l'y ajoute pas par défaut, ni celui de cmake.org sur macOS.
// On cherche donc chaque outil dans le PATH, puis aux emplacements standard
// propres au système (macos.rs, windows.rs). Le PATH ainsi complété est
// transmis à CMake, qui y trouve à son tour le compilateur et make.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use serde::Serialize;

/// Rôle d'un outil dans le build « sim ».
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ToolKind {
    Cmake,
    Compiler,
    /// Linux et Windows ; sous macOS, make vient avec les outils Xcode.
    #[cfg_attr(target_os = "macos", allow(dead_code))]
    Make,
}

/// Outil requis sur ce système.
pub struct ToolSpec {
    pub kind: ToolKind,
    pub label: &'static str,
    /// Noms de fichier acceptés, par ordre de préférence.
    pub names: &'static [&'static str],
    /// Commande d'installation affichée quand l'outil manque.
    pub install: &'static str,
    /// Vérification supplémentaire d'un fichier trouvé (macOS : /usr/bin/cc
    /// existe toujours, mais ne fonctionne qu'avec les outils Xcode).
    pub usable: Option<fn(&Path) -> bool>,
}

/// Résultat de la recherche d'un outil.
#[derive(Clone, Debug)]
pub struct ToolStatus {
    pub kind: ToolKind,
    pub label: &'static str,
    pub path: Option<PathBuf>,
    pub install: &'static str,
}

/// Outils trouvés et PATH à transmettre aux outils de build.
#[derive(Clone, Debug)]
pub struct Toolchain {
    pub tools: Vec<ToolStatus>,
    pub search_path: OsString,
}

impl Toolchain {
    /// Recherche dans le PATH de l'app, puis aux emplacements standard.
    pub fn detect() -> Self {
        detect_in(
            std::env::var_os("PATH"),
            &super::standard_tool_dirs(),
            &super::required_tools(),
        )
    }

    pub fn path_of(&self, kind: ToolKind) -> Option<&Path> {
        self.tools
            .iter()
            .find(|t| t.kind == kind)
            .and_then(|t| t.path.as_deref())
    }

    pub fn missing(&self) -> impl Iterator<Item = &ToolStatus> {
        self.tools.iter().filter(|t| t.path.is_none())
    }
}

/// Cœur de la détection, sans dépendance à l'environnement (testable).
pub fn detect_in(
    path_env: Option<OsString>,
    standard_dirs: &[PathBuf],
    specs: &[ToolSpec],
) -> Toolchain {
    let mut dirs: Vec<PathBuf> = path_env
        .map(|p| std::env::split_paths(&p).collect())
        .unwrap_or_default();
    for dir in standard_dirs {
        if dir.is_dir() && !dirs.contains(dir) {
            dirs.push(dir.clone());
        }
    }
    // Un dossier que le séparateur du PATH ne peut pas représenter est écarté
    // plutôt que de faire échouer tout le PATH.
    dirs.retain(|d| !d.as_os_str().is_empty() && std::env::join_paths([d]).is_ok());

    let tools = specs
        .iter()
        .map(|spec| ToolStatus {
            kind: spec.kind,
            label: spec.label,
            path: find(&dirs, spec),
            install: spec.install,
        })
        .collect();
    let search_path = std::env::join_paths(&dirs).unwrap_or_default();
    Toolchain { tools, search_path }
}

fn find(dirs: &[PathBuf], spec: &ToolSpec) -> Option<PathBuf> {
    dirs.iter()
        .flat_map(|dir| spec.names.iter().map(move |name| dir.join(name)))
        .find(|candidate| candidate.is_file() && spec.usable.is_none_or(|ok| ok(candidate)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("prebench-toolchain-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("dossier temporaire");
        dir
    }

    const SPECS: &[ToolSpec] = &[
        ToolSpec {
            kind: ToolKind::Cmake,
            label: "CMake",
            names: &["fake-cmake"],
            install: "installer cmake",
            usable: None,
        },
        ToolSpec {
            kind: ToolKind::Compiler,
            label: "Compilateur C",
            names: &["fake-cc"],
            install: "installer cc",
            usable: None,
        },
    ];

    #[test]
    fn path_first_then_standard_dirs() {
        let in_path = temp_dir("path");
        let standard = temp_dir("standard");
        std::fs::write(in_path.join("fake-cmake"), "").expect("fichier");
        std::fs::write(standard.join("fake-cmake"), "").expect("fichier");
        std::fs::write(standard.join("fake-cc"), "").expect("fichier");

        let path_env = std::env::join_paths([&in_path]).expect("PATH");
        let tc = detect_in(Some(path_env), std::slice::from_ref(&standard), SPECS);

        assert_eq!(
            tc.path_of(ToolKind::Cmake),
            Some(in_path.join("fake-cmake").as_path())
        );
        assert_eq!(
            tc.path_of(ToolKind::Compiler),
            Some(standard.join("fake-cc").as_path())
        );
        assert_eq!(tc.missing().count(), 0);
        // Le dossier standard est ajouté au PATH transmis à CMake.
        let dirs: Vec<PathBuf> = std::env::split_paths(&tc.search_path).collect();
        assert_eq!(dirs, vec![in_path.clone(), standard.clone()]);

        let _ = std::fs::remove_dir_all(in_path);
        let _ = std::fs::remove_dir_all(standard);
    }

    #[test]
    fn missing_tool_and_absent_standard_dir() {
        let empty = temp_dir("empty");
        let absent = empty.join("absent");
        let tc = detect_in(None, &[absent], SPECS);
        let missing: Vec<&str> = tc.missing().map(|t| t.label).collect();
        assert_eq!(missing, vec!["CMake", "Compilateur C"]);
        assert!(tc.search_path.is_empty());
        let _ = std::fs::remove_dir_all(empty);
    }
}

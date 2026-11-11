// app/src-tauri/src/platform/macos.rs — macOS (et Linux).

use std::path::PathBuf;

use super::toolchain::{ToolKind, ToolSpec};

/// Adresse du socket Unix d'une carte. Chemin court dans le dossier temporaire :
/// macOS limite un chemin de socket à 104 octets (docs/protocol.md, 1.1).
pub fn socket_address(board_id: u16) -> String {
    let dir = PathBuf::from("/tmp");
    dir.join(format!("prebench-{}-{board_id}.sock", std::process::id()))
        .display()
        .to_string()
}

/// Nom de l'exécutable produit par le build « sim ».
pub fn firmware_file_name() -> &'static str {
    "firmware"
}

/// Générateur CMake : celui par défaut (Makefiles Unix).
pub fn cmake_generator_args() -> Vec<String> {
    Vec::new()
}

/// Emplacements standard des outils de build, cherchés après le PATH : une
/// app lancée depuis le Finder ne reçoit pas le PATH du shell.
#[cfg(target_os = "macos")]
pub fn standard_tool_dirs() -> Vec<PathBuf> {
    [
        "/opt/homebrew/bin",                    // Homebrew (Apple Silicon)
        "/usr/local/bin",                       // Homebrew (Intel), liens de CMake.app
        "/Applications/CMake.app/Contents/bin", // installateur de cmake.org
        "/usr/bin",
        "/bin",
    ]
    .iter()
    .map(PathBuf::from)
    .collect()
}

#[cfg(not(target_os = "macos"))]
pub fn standard_tool_dirs() -> Vec<PathBuf> {
    ["/usr/local/bin", "/usr/bin", "/bin"]
        .iter()
        .map(PathBuf::from)
        .collect()
}

/// Outils requis : CMake et clang des outils en ligne de commande Xcode
/// (make est fourni avec eux).
#[cfg(target_os = "macos")]
pub fn required_tools() -> Vec<ToolSpec> {
    vec![
        ToolSpec {
            kind: ToolKind::Cmake,
            label: "CMake",
            names: &["cmake"],
            install: "brew install cmake",
            usable: None,
        },
        ToolSpec {
            kind: ToolKind::Compiler,
            label: "Compilateur C (clang)",
            names: &["cc"],
            install: "xcode-select --install",
            usable: Some(xcode_tools_installed),
        },
    ]
}

/// /usr/bin/cc existe toujours sous macOS, mais sans les outils Xcode il ne
/// fait qu'ouvrir la fenêtre d'installation : `xcode-select -p` le vérifie
/// sans effet de bord.
#[cfg(target_os = "macos")]
fn xcode_tools_installed(_cc: &std::path::Path) -> bool {
    std::process::Command::new("/usr/bin/xcode-select")
        .arg("-p")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

#[cfg(not(target_os = "macos"))]
pub fn required_tools() -> Vec<ToolSpec> {
    vec![
        ToolSpec {
            kind: ToolKind::Cmake,
            label: "CMake",
            names: &["cmake"],
            install: "sudo apt install cmake",
            usable: None,
        },
        ToolSpec {
            kind: ToolKind::Compiler,
            label: "Compilateur C",
            names: &["cc", "gcc", "clang"],
            install: "sudo apt install build-essential",
            usable: None,
        },
        ToolSpec {
            kind: ToolKind::Make,
            label: "make",
            names: &["make"],
            install: "sudo apt install build-essential",
            usable: None,
        },
    ]
}

/// Commande de lancement du firmware, en priorité basse (section 11.2).
/// macOS : `taskpolicy -c utility` plafonne la QoS de tous les fils du
/// processus à « utility » (cœurs économes). Linux : `nice -n 10`.
pub fn firmware_command(exe: &std::path::Path) -> tokio::process::Command {
    #[cfg(target_os = "macos")]
    let mut cmd = tokio::process::Command::new("/usr/sbin/taskpolicy");
    #[cfg(target_os = "macos")]
    cmd.args(["-c", "utility"]);
    #[cfg(not(target_os = "macos"))]
    let mut cmd = tokio::process::Command::new("nice");
    #[cfg(not(target_os = "macos"))]
    cmd.args(["-n", "10"]);
    cmd.arg(exe);
    cmd
}

/// Réglages après lancement : rien de plus sous macOS et Linux.
pub fn after_firmware_spawn(_child: &tokio::process::Child) {}

/// Commande d'un outil de build (cmake).
pub fn tool_command(program: &std::path::Path) -> tokio::process::Command {
    tokio::process::Command::new(program)
}

// app/src-tauri/src/platform/macos.rs — macOS (et Linux).

use std::ffi::OsString;
use std::path::PathBuf;

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

/// PATH des outils de build. Une app lancée depuis le Finder ne reçoit pas le
/// PATH du shell : on ajoute les emplacements de Homebrew.
pub fn tool_path() -> OsString {
    let mut paths: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).collect())
        .unwrap_or_default();
    for extra in ["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin", "/bin"] {
        let extra = PathBuf::from(extra);
        if !paths.contains(&extra) {
            paths.push(extra);
        }
    }
    std::env::join_paths(paths).unwrap_or_default()
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
pub fn tool_command(program: &str) -> tokio::process::Command {
    tokio::process::Command::new(program)
}

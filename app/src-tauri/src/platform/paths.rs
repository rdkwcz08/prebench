// app/src-tauri/src/platform/paths.rs — Chemins transmis aux outils de build.
//
// Sous Windows, le dossier des ressources de l'app peut être renvoyé au format
// étendu « \\?\C:\Program Files\Prebench\sdk ». CMake le transforme en
// « //?/C:/… », que make ne sait pas lire (« target pattern contains no '%' ») :
// tout chemin passé à un outil est d'abord simplifié. Sans effet sous macOS
// et Linux.

use std::path::{Path, PathBuf};

/// Chemin sans le préfixe \\?\ quand il n'est pas nécessaire.
pub fn simplified(path: &Path) -> PathBuf {
    dunce::simplified(path).to_path_buf()
}

/// Chemin pour la ligne de commande de CMake : simplifié, séparateurs « / »
/// (CMake les accepte partout et évite ainsi toute séquence d'échappement).
/// Les espaces ne posent pas de problème : chaque argument est passé à part,
/// sans shell.
pub fn cmake_path(path: &Path) -> String {
    let text = simplified(path).display().to_string();
    if cfg!(windows) {
        text.replace('\\', "/")
    } else {
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(windows)]
    #[test]
    fn extended_prefix_is_removed() {
        let p = Path::new(r"\\?\C:\Program Files\Prebench\sdk");
        assert_eq!(
            simplified(p),
            PathBuf::from(r"C:\Program Files\Prebench\sdk")
        );
        assert_eq!(cmake_path(p), "C:/Program Files/Prebench/sdk");
    }

    #[cfg(unix)]
    #[test]
    fn unix_paths_are_unchanged() {
        let p = Path::new("/Applications/Prebench.app/Contents/Resources/sdk");
        assert_eq!(cmake_path(p), p.display().to_string());
    }
}

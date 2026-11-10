// app/src-tauri/src/main.rs — Point d'entrée de l'exécutable Prebench.

// Pas de console supplémentaire sous Windows en release.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    prebench_lib::run();
}

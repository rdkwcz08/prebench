// app/src-tauri/src/lib.rs — Cœur Rust de Prebench (section 4).
//
// core/      orchestrateur, routage, réseau logique
// platform/  tout ce qui dépend du système (règle 16.2.6)
// build/     build « sim » du firmware
// project/   lecture de prebench.toml

mod build;
mod commands;
mod core;
mod error;
mod platform;
mod project;
mod update;

pub fn run() {
    let result = tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(commands::AppState::default())
        .invoke_handler(tauri::generate_handler![
            commands::default_project,
            commands::open_project,
            commands::start_simulation,
            commands::stop_simulation,
            commands::reset_simulation,
            commands::component_control,
            update::check_update,
            update::install_update,
        ])
        .run(tauri::generate_context!());
    if let Err(e) = result {
        eprintln!("Prebench n'a pas pu démarrer : {e}");
        std::process::exit(1);
    }
}

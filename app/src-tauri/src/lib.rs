// app/src-tauri/src/lib.rs — Cœur Rust de Prebench (section 4).
//
// core/      orchestrateur, routage, réseau logique
// platform/  tout ce qui dépend du système (règle 16.2.6)
// build/     build « sim » du firmware
// project/   lecture de prebench.toml, nouveaux projets depuis un modèle
// install_check.rs  vérification de l'app installée (release.yml)

mod build;
mod commands;
mod core;
mod error;
mod install_check;
mod platform;
mod project;
mod update;

pub fn run() {
    let result = tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(commands::AppState::default())
        .setup(|app| {
            // Vérification de l'app installée : sans fenêtre (install_check.rs).
            if let Some(request) = install_check::request_from_args() {
                install_check::spawn(app.handle().clone(), request);
                return Ok(());
            }
            // Fenêtre principale, déclarée avec "create": false dans tauri.conf.json.
            let config = app
                .config()
                .app
                .windows
                .iter()
                .find(|w| w.label == "main")
                .cloned()
                .ok_or("fenêtre « main » absente de tauri.conf.json")?;
            tauri::WebviewWindowBuilder::from_config(app.handle(), &config)?.build()?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::default_project,
            commands::open_project,
            commands::start_simulation,
            commands::stop_simulation,
            commands::reset_simulation,
            commands::component_control,
            commands::check_prerequisites,
            commands::list_templates,
            commands::create_project,
            update::check_update,
            update::install_update,
        ])
        .run(tauri::generate_context!());
    if let Err(e) = result {
        eprintln!("Prebench n'a pas pu démarrer : {e}");
        std::process::exit(1);
    }
}

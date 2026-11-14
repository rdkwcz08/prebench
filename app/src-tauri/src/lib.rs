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
mod debug;
mod error;
mod install_check;
mod platform;
mod project;
mod session;
mod settings;
mod update;

pub fn run() {
    let result = tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        // Fenêtre « Enregistrer sous » ouverte par le cœur (export série) ;
        // aucune permission de dialogue n'est donnée à l'interface.
        .plugin(tauri_plugin_dialog::init())
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
            commands::set_power,
            commands::pause_simulation,
            commands::resume_simulation,
            commands::component_control,
            commands::uart_send,
            commands::export_serial,
            commands::open_in_editor,
            commands::start_debug,
            commands::save_bench,
            commands::preview_states,
            commands::get_settings,
            commands::set_setting,
            commands::check_prerequisites,
            commands::list_templates,
            commands::create_project,
            update::check_update,
            update::install_update,
        ])
        .build(tauri::generate_context!());
    match result {
        Ok(app) => app.run(|handle, event| {
            if let tauri::RunEvent::Exit = event {
                let state = tauri::Manager::state::<commands::AppState>(handle);
                tauri::async_runtime::block_on(commands::shutdown(&state));
            }
        }),
        Err(e) => {
            eprintln!("Prebench n'a pas pu démarrer : {e}");
            std::process::exit(1);
        }
    }
}

// app/src-tauri/build.rs — Contexte Tauri et liste stricte des commandes du cœur
// accessibles à l'interface (section 11.4) : chaque commande devient une
// permission « allow-<commande> », accordée dans capabilities/default.json.

const COMMANDS: &[&str] = &[
    "default_project",
    "open_project",
    "start_simulation",
    "stop_simulation",
    "reset_simulation",
    "pause_simulation",
    "resume_simulation",
    "component_control",
    "open_in_editor",
    "start_debug",
    "check_prerequisites",
    "list_templates",
    "create_project",
    "check_update",
    "install_update",
];

fn main() {
    // Windows : manifeste (Common Controls v6) intégré par l'éditeur de liens
    // MSVC dans tous les exécutables, tests compris. tauri-build ne l'ajoute
    // qu'à l'app : les tests échouaient avec STATUS_ENTRYPOINT_NOT_FOUND.
    let windows = tauri_build::WindowsAttributes::new_without_app_manifest();
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
        && std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc")
    {
        let manifest =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("windows-app-manifest.xml");
        println!("cargo:rerun-if-changed=windows-app-manifest.xml");
        println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
        println!("cargo:rustc-link-arg=/MANIFESTINPUT:{}", manifest.display());
    }

    let attributes = tauri_build::Attributes::new()
        .windows_attributes(windows)
        .app_manifest(tauri_build::AppManifest::new().commands(COMMANDS));
    if let Err(e) = tauri_build::try_build(attributes) {
        eprintln!("échec de tauri-build : {e:#}");
        std::process::exit(1);
    }
}

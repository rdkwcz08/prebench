// app/src-tauri/src/platform/windows.rs — Windows.

use std::ffi::OsString;

/// Named pipe d'une carte (docs/protocol.md, 1.1).
pub fn socket_address(board_id: u16) -> String {
    format!(r"\\.\pipe\prebench-{}-{board_id}", std::process::id())
}

/// Nom de l'exécutable produit par le build « sim ».
pub fn firmware_file_name() -> &'static str {
    "firmware.exe"
}

/// Le firmware se compile avec gcc de MinGW-w64, jamais MSVC (section 12.3).
pub fn cmake_generator_args() -> Vec<String> {
    vec!["-G".into(), "MinGW Makefiles".into()]
}

/// PATH des outils de build : celui de l'utilisateur (MSYS2 UCRT64, CMake).
pub fn tool_path() -> OsString {
    std::env::var_os("PATH").unwrap_or_default()
}

use windows_sys::Win32::System::Threading::{
    ProcessPowerThrottling, SetProcessInformation, BELOW_NORMAL_PRIORITY_CLASS,
    PROCESS_POWER_THROTTLING_CURRENT_VERSION, PROCESS_POWER_THROTTLING_EXECUTION_SPEED,
    PROCESS_POWER_THROTTLING_STATE,
};

/// Pas de fenêtre console pour les processus lancés par l'app.
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Commande de lancement du firmware : priorité « inférieure à la normale »
/// (section 11.2), sans console.
pub fn firmware_command(exe: &std::path::Path) -> tokio::process::Command {
    let mut cmd = tokio::process::Command::new(exe);
    cmd.creation_flags(BELOW_NORMAL_PRIORITY_CLASS | CREATE_NO_WINDOW);
    cmd
}

/// EcoQoS (Windows 11) : demande à l'ordonnanceur de privilégier l'efficacité
/// énergétique pour le firmware. Sans effet sur les versions qui l'ignorent.
pub fn after_firmware_spawn(child: &tokio::process::Child) {
    let Some(handle) = child.raw_handle() else {
        return;
    };
    let state = PROCESS_POWER_THROTTLING_STATE {
        Version: PROCESS_POWER_THROTTLING_CURRENT_VERSION,
        ControlMask: PROCESS_POWER_THROTTLING_EXECUTION_SPEED,
        StateMask: PROCESS_POWER_THROTTLING_EXECUTION_SPEED,
    };
    // SAFETY : `handle` est le handle valide du processus enfant, encore
    // vivant ; `state` est une structure initialisée de la taille annoncée.
    unsafe {
        SetProcessInformation(
            handle as _,
            ProcessPowerThrottling,
            std::ptr::from_ref(&state).cast(),
            std::mem::size_of::<PROCESS_POWER_THROTTLING_STATE>() as u32,
        );
    }
}

/// Commande d'un outil de build (cmake), sans console.
pub fn tool_command(program: &str) -> tokio::process::Command {
    let mut cmd = tokio::process::Command::new(program);
    cmd.creation_flags(CREATE_NO_WINDOW);
    cmd
}

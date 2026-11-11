// app/src-tauri/src/platform/windows.rs — Windows.

use std::path::PathBuf;

use super::toolchain::{ToolKind, ToolSpec};

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

/// Emplacements standard des outils de build, cherchés après le PATH :
/// l'installateur de CMake n'ajoute pas CMake au PATH par défaut, et MSYS2
/// n'y ajoute jamais l'environnement UCRT64.
pub fn standard_tool_dirs() -> Vec<PathBuf> {
    let program_files = std::env::var_os("ProgramFiles")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Program Files"));
    vec![
        program_files.join("CMake").join("bin"),
        PathBuf::from(r"C:\msys64\ucrt64\bin"),
    ]
}

const MSYS2_GCC_INSTALL: &str = "winget install --id MSYS2.MSYS2 -e   # si MSYS2 n'est pas installé\n\
C:\\msys64\\usr\\bin\\bash.exe -lc \"pacman -S --needed --noconfirm mingw-w64-ucrt-x86_64-gcc mingw-w64-ucrt-x86_64-make\"";

/// Outils requis : CMake, gcc et mingw32-make de MSYS2 UCRT64 (jamais MSVC,
/// section 12.3).
pub fn required_tools() -> Vec<ToolSpec> {
    vec![
        ToolSpec {
            kind: ToolKind::Cmake,
            label: "CMake",
            names: &["cmake.exe"],
            install: "winget install --id Kitware.CMake -e --custom \"ADD_CMAKE_TO_PATH=System\"",
            usable: None,
        },
        ToolSpec {
            kind: ToolKind::Compiler,
            label: "Compilateur C (gcc MinGW-w64)",
            names: &["gcc.exe"],
            install: MSYS2_GCC_INSTALL,
            usable: None,
        },
        ToolSpec {
            kind: ToolKind::Make,
            label: "mingw32-make",
            names: &["mingw32-make.exe"],
            install: MSYS2_GCC_INSTALL,
            usable: None,
        },
    ]
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
/// Le PATH complété (MSYS2 UCRT64) permet au firmware de trouver les DLL
/// d'exécution de MinGW-w64 s'il en dépend.
pub fn firmware_command(exe: &std::path::Path) -> tokio::process::Command {
    let mut cmd = tokio::process::Command::new(exe);
    cmd.creation_flags(BELOW_NORMAL_PRIORITY_CLASS | CREATE_NO_WINDOW);
    cmd.env("PATH", super::toolchain::Toolchain::detect().search_path);
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
pub fn tool_command(program: &std::path::Path) -> tokio::process::Command {
    let mut cmd = tokio::process::Command::new(program);
    cmd.creation_flags(CREATE_NO_WINDOW);
    cmd
}

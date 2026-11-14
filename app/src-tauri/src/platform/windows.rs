// app/src-tauri/src/platform/windows.rs — Windows.

use std::path::PathBuf;

use super::toolchain::{ToolKind, ToolSpec};

/// Named pipe d'une carte, propre à chaque lancement du firmware (`launch`) :
/// tant que l'instance du firmware précédent n'est pas fermée, une nouvelle
/// instance du même nom est refusée (FILE_FLAG_FIRST_PIPE_INSTANCE).
pub fn socket_address(board_id: u16, launch: u32) -> String {
    format!(
        r"\\.\pipe\prebench-{}-{board_id}-{launch}",
        std::process::id()
    )
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

use windows_sys::Win32::Foundation::{CloseHandle, FILETIME, HANDLE, INVALID_HANDLE_VALUE};
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Thread32First, Thread32Next, TH32CS_SNAPTHREAD, THREADENTRY32,
};
use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
    SetInformationJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
    JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE, JOB_OBJECT_LIMIT_PROCESS_MEMORY,
};
use windows_sys::Win32::System::ProcessStatus::{
    K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS, PROCESS_MEMORY_COUNTERS_EX,
};
use windows_sys::Win32::System::Threading::{
    GetProcessTimes, OpenProcess, OpenThread, ProcessPowerThrottling, ResumeThread,
    SetProcessInformation, SuspendThread, BELOW_NORMAL_PRIORITY_CLASS,
    PROCESS_POWER_THROTTLING_CURRENT_VERSION, PROCESS_POWER_THROTTLING_EXECUTION_SPEED,
    PROCESS_POWER_THROTTLING_STATE, PROCESS_QUERY_LIMITED_INFORMATION, THREAD_SUSPEND_RESUME,
};

/// Pas de fenêtre console pour les processus lancés par l'app.
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Commande de lancement du firmware : priorité « inférieure à la normale »
/// (section 11.2), sans console. Le plafond mémoire est posé après le
/// lancement, par le Job Object (`after_firmware_spawn`).
/// Le PATH complété (MSYS2 UCRT64) permet au firmware de trouver les DLL
/// d'exécution de MinGW-w64 s'il en dépend.
pub fn firmware_command(exe: &std::path::Path, _memory_backstop: u64) -> tokio::process::Command {
    let mut cmd = tokio::process::Command::new(exe);
    cmd.creation_flags(BELOW_NORMAL_PRIORITY_CLASS | CREATE_NO_WINDOW);
    cmd.env("PATH", super::toolchain::Toolchain::detect().search_path);
    cmd
}

/// Job Object du firmware : plafond mémoire et « kill on close ». Fermer le
/// handle (fin de la simulation, ou de l'app, même brutale) tue le firmware :
/// aucun processus fantôme (section 11.2).
pub struct FirmwareGuard {
    job: HANDLE,
}

// SAFETY : un handle de Job Object est utilisable depuis n'importe quel fil.
unsafe impl Send for FirmwareGuard {}

impl Drop for FirmwareGuard {
    fn drop(&mut self) {
        if !self.job.is_null() {
            // SAFETY : handle créé par CreateJobObjectW, fermé une seule fois.
            unsafe { CloseHandle(self.job) };
        }
    }
}

/// EcoQoS (Windows 11) puis Job Object (plafond mémoire, kill on close).
/// EcoQoS demande à l'ordonnanceur de privilégier l'efficacité énergétique ;
/// sans effet sur les versions qui l'ignorent.
pub fn after_firmware_spawn(child: &tokio::process::Child, memory_backstop: u64) -> FirmwareGuard {
    let Some(handle) = child.raw_handle() else {
        return FirmwareGuard {
            job: std::ptr::null_mut(),
        };
    };
    let state = PROCESS_POWER_THROTTLING_STATE {
        Version: PROCESS_POWER_THROTTLING_CURRENT_VERSION,
        ControlMask: PROCESS_POWER_THROTTLING_EXECUTION_SPEED,
        StateMask: PROCESS_POWER_THROTTLING_EXECUTION_SPEED,
    };
    // SAFETY : `handle` est le handle valide du processus enfant, encore
    // vivant ; les structures sont initialisées et de la taille annoncée ;
    // le Job Object créé est fermé par FirmwareGuard.
    unsafe {
        SetProcessInformation(
            handle as HANDLE,
            ProcessPowerThrottling,
            std::ptr::from_ref(&state).cast(),
            std::mem::size_of::<PROCESS_POWER_THROTTLING_STATE>() as u32,
        );
        let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
        if job.is_null() {
            return FirmwareGuard { job };
        }
        let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
        info.BasicLimitInformation.LimitFlags =
            JOB_OBJECT_LIMIT_PROCESS_MEMORY | JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        info.ProcessMemoryLimit = usize::try_from(memory_backstop).unwrap_or(usize::MAX);
        SetInformationJobObject(
            job,
            JobObjectExtendedLimitInformation,
            std::ptr::from_ref(&info).cast(),
            std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
        );
        AssignProcessToJobObject(job, handle as HANDLE);
        FirmwareGuard { job }
    }
}

/// Temps CPU cumulé et mémoire d'un processus.
#[derive(Clone, Copy, Debug)]
pub struct ProcessUsage {
    pub cpu_time: std::time::Duration,
    /// Mémoire privée engagée (PrivateUsage), en octets.
    pub memory: u64,
}

fn filetime_100ns(t: FILETIME) -> u64 {
    (u64::from(t.dwHighDateTime) << 32) | u64::from(t.dwLowDateTime)
}

/// Mesure d'un processus vivant ; `None` s'il n'existe plus.
pub fn process_usage(pid: u32) -> Option<ProcessUsage> {
    // SAFETY : handle ouvert ici et fermé avant de rendre la main ; les
    // structures passées sont initialisées et de la taille annoncée.
    unsafe {
        let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if h.is_null() {
            return None;
        }
        let zero = FILETIME {
            dwLowDateTime: 0,
            dwHighDateTime: 0,
        };
        let (mut created, mut exited, mut kernel, mut user) = (zero, zero, zero, zero);
        let mut pmc: PROCESS_MEMORY_COUNTERS_EX = std::mem::zeroed();
        pmc.cb = std::mem::size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32;
        let ok_times = GetProcessTimes(h, &mut created, &mut exited, &mut kernel, &mut user) != 0;
        let ok_mem = K32GetProcessMemoryInfo(
            h,
            std::ptr::from_mut(&mut pmc).cast::<PROCESS_MEMORY_COUNTERS>(),
            pmc.cb,
        ) != 0;
        CloseHandle(h);
        if !ok_times || !ok_mem {
            return None;
        }
        let ticks = filetime_100ns(kernel).saturating_add(filetime_100ns(user));
        Some(ProcessUsage {
            cpu_time: std::time::Duration::from_nanos(ticks.saturating_mul(100)),
            memory: pmc.PrivateUsage as u64,
        })
    }
}

/// Suspend tous les fils du processus (pause, section 11.2), y compris une
/// boucle qui n'appelle jamais libvmcu.
pub fn suspend_process(pid: u32) -> std::io::Result<()> {
    for_each_thread(pid, |t| {
        // SAFETY : handle de fil valide, ouvert avec THREAD_SUSPEND_RESUME.
        unsafe { SuspendThread(t) };
    })
}

/// Reprend les fils suspendus par `suspend_process`.
pub fn resume_process(pid: u32) -> std::io::Result<()> {
    for_each_thread(pid, |t| {
        // SAFETY : handle de fil valide, ouvert avec THREAD_SUSPEND_RESUME.
        unsafe { ResumeThread(t) };
    })
}

fn for_each_thread(pid: u32, mut f: impl FnMut(HANDLE)) -> std::io::Result<()> {
    // SAFETY : instantané ToolHelp et handles de fils ouverts ici, fermés
    // avant de rendre la main ; THREADENTRY32 initialisée avec sa taille.
    unsafe {
        let snap = CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0);
        if snap == INVALID_HANDLE_VALUE {
            return Err(std::io::Error::last_os_error());
        }
        let mut entry: THREADENTRY32 = std::mem::zeroed();
        entry.dwSize = std::mem::size_of::<THREADENTRY32>() as u32;
        let mut more = Thread32First(snap, &mut entry) != 0;
        while more {
            if entry.th32OwnerProcessID == pid {
                let t = OpenThread(THREAD_SUSPEND_RESUME, 0, entry.th32ThreadID);
                if !t.is_null() {
                    f(t);
                    CloseHandle(t);
                }
            }
            more = Thread32Next(snap, &mut entry) != 0;
        }
        CloseHandle(snap);
    }
    Ok(())
}

/// Commande d'un outil de build (cmake), sans console.
pub fn tool_command(program: &std::path::Path) -> tokio::process::Command {
    let mut cmd = tokio::process::Command::new(program);
    cmd.creation_flags(CREATE_NO_WINDOW);
    cmd
}

/// Ouvre `url` (schéma vscode://) ; sinon, le fichier avec l'application
/// associée. ShellExecuteW renvoie une valeur > 32 en cas de succès.
pub fn open_location(url: &str, file: &std::path::Path) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::UI::Shell::ShellExecuteW;
    use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

    let wide = |s: &std::ffi::OsStr| {
        s.encode_wide()
            .chain(std::iter::once(0))
            .collect::<Vec<u16>>()
    };
    let verb = wide(std::ffi::OsStr::new("open"));
    let shell = |target: &[u16]| {
        // SAFETY : chaînes UTF-16 terminées par un zéro, vivantes pendant l'appel.
        let r = unsafe {
            ShellExecuteW(
                std::ptr::null_mut(),
                verb.as_ptr(),
                target.as_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                SW_SHOWNORMAL,
            )
        };
        r as isize > 32
    };
    if shell(&wide(std::ffi::OsStr::new(url))) || shell(&wide(file.as_os_str())) {
        Ok(())
    } else {
        Err(std::io::Error::other("ShellExecuteW a échoué"))
    }
}

/// Adresse fixe du canal de débogage d'un projet (`key` : empreinte de son
/// chemin), inscrite dans .vscode/launch.json (section 10.2).
pub fn debug_socket_address(key: u64) -> String {
    format!(r"\\.\pipe\prebench-debug-{key:016x}")
}

/// Extension VS Code et débogueur nécessaires pour déboguer le firmware :
/// gdb de MSYS2 UCRT64, comme gcc (jamais le débogueur de MSVC).
pub fn debug_requirements() -> super::DebugRequirements {
    super::DebugRequirements {
        extension_id: "ms-vscode.cpptools",
        extension_label: "C/C++ (extension VS Code de Microsoft)",
        debugger: Some(("gdb", &["gdb.exe"])),
        debugger_install: "C:\\msys64\\usr\\bin\\bash.exe -lc \"pacman -S --needed --noconfirm mingw-w64-ucrt-x86_64-gdb\"",
    }
}

/// Informations de débogage de l'exécutable lancé par le débogueur : gcc de
/// MinGW-w64 les garde dans l'exécutable (DWARF), rien à faire.
pub fn bundle_debug_symbols(_program: &std::path::Path) -> std::io::Result<()> {
    Ok(())
}

/// Temps CPU consommé par le fil appelant, en ns : budget des modèles de
/// composants (section 8.1), indépendant de la charge de la machine.
/// GetThreadTimes n'avance que par pas d'environ 15,6 ms (un appel court qui
/// chevauche un pas compterait 15,6 ms) : on lit les cycles du fil
/// (QueryThreadCycleTime, cycles du compteur TSC), convertis en ns par une
/// fréquence du TSC mesurée une fois. Repli sur l'horloge monotone hors x86_64
/// ou si l'appel échoue.
pub fn thread_cpu_ns() -> u64 {
    #[cfg(target_arch = "x86_64")]
    {
        use windows_sys::Win32::System::Threading::GetCurrentThread;
        use windows_sys::Win32::System::WindowsProgramming::QueryThreadCycleTime;
        let mut cycles: u64 = 0;
        // SAFETY : pseudo-handle du fil courant, toujours valide ; `cycles` est écrit par l'appel.
        let ok = unsafe { QueryThreadCycleTime(GetCurrentThread(), &mut cycles) } != 0;
        if ok {
            let per_ns = tsc_cycles_per_ns();
            if per_ns > 0.0 {
                return (cycles as f64 / per_ns) as u64;
            }
        }
    }
    super::monotonic_ns()
}

/// Cycles du TSC par nanoseconde, mesurés une fois sur 20 ms d'horloge.
#[cfg(target_arch = "x86_64")]
fn tsc_cycles_per_ns() -> f64 {
    static PER_NS: std::sync::OnceLock<f64> = std::sync::OnceLock::new();
    *PER_NS.get_or_init(|| {
        let t0 = std::time::Instant::now();
        // SAFETY : instruction RDTSC, disponible sur tout processeur x86_64.
        let c0 = unsafe { core::arch::x86_64::_rdtsc() };
        std::thread::sleep(std::time::Duration::from_millis(20));
        // SAFETY : idem.
        let c1 = unsafe { core::arch::x86_64::_rdtsc() };
        let ns = t0.elapsed().as_nanos() as f64;
        if ns > 0.0 {
            c1.wrapping_sub(c0) as f64 / ns
        } else {
            0.0
        }
    })
}

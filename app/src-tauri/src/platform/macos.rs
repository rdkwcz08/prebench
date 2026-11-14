// app/src-tauri/src/platform/macos.rs — macOS (et Linux).

use std::path::PathBuf;

use super::toolchain::{ToolKind, ToolSpec};

/// Adresse du socket Unix d'une carte, propre à chaque lancement du firmware
/// (`launch`). Chemin court dans le dossier temporaire : macOS limite un
/// chemin de socket à 104 octets (docs/protocol.md, 1.1).
pub fn socket_address(board_id: u16, launch: u32) -> String {
    let dir = PathBuf::from("/tmp");
    dir.join(format!(
        "prebench-{}-{board_id}-{launch}.sock",
        std::process::id()
    ))
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

/// Emplacements standard des outils de build, cherchés après le PATH : une
/// app lancée depuis le Finder ne reçoit pas le PATH du shell.
#[cfg(target_os = "macos")]
pub fn standard_tool_dirs() -> Vec<PathBuf> {
    [
        "/opt/homebrew/bin",                    // Homebrew (Apple Silicon)
        "/usr/local/bin",                       // Homebrew (Intel), liens de CMake.app
        "/Applications/CMake.app/Contents/bin", // installateur de cmake.org
        "/usr/bin",
        "/bin",
    ]
    .iter()
    .map(PathBuf::from)
    .collect()
}

#[cfg(not(target_os = "macos"))]
pub fn standard_tool_dirs() -> Vec<PathBuf> {
    ["/usr/local/bin", "/usr/bin", "/bin"]
        .iter()
        .map(PathBuf::from)
        .collect()
}

/// Outils requis : CMake et clang des outils en ligne de commande Xcode
/// (make est fourni avec eux).
#[cfg(target_os = "macos")]
pub fn required_tools() -> Vec<ToolSpec> {
    vec![
        ToolSpec {
            kind: ToolKind::Cmake,
            label: "CMake",
            names: &["cmake"],
            install: "brew install cmake",
            usable: None,
        },
        ToolSpec {
            kind: ToolKind::Compiler,
            label: "Compilateur C (clang)",
            names: &["cc"],
            install: "xcode-select --install",
            usable: Some(xcode_tools_installed),
        },
    ]
}

/// /usr/bin/cc existe toujours sous macOS, mais sans les outils Xcode il ne
/// fait qu'ouvrir la fenêtre d'installation : `xcode-select -p` le vérifie
/// sans effet de bord.
#[cfg(target_os = "macos")]
fn xcode_tools_installed(_cc: &std::path::Path) -> bool {
    std::process::Command::new("/usr/bin/xcode-select")
        .arg("-p")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

#[cfg(not(target_os = "macos"))]
pub fn required_tools() -> Vec<ToolSpec> {
    vec![
        ToolSpec {
            kind: ToolKind::Cmake,
            label: "CMake",
            names: &["cmake"],
            install: "sudo apt install cmake",
            usable: None,
        },
        ToolSpec {
            kind: ToolKind::Compiler,
            label: "Compilateur C",
            names: &["cc", "gcc", "clang"],
            install: "sudo apt install build-essential",
            usable: None,
        },
        ToolSpec {
            kind: ToolKind::Make,
            label: "make",
            names: &["make"],
            install: "sudo apt install build-essential",
            usable: None,
        },
    ]
}

/// Commande de lancement du firmware, en priorité basse (section 11.2).
/// macOS : `taskpolicy -c utility` plafonne la QoS de tous les fils du
/// processus à « utility » (cœurs économes). Linux : `nice -n 10`.
/// Les deux remplacent leur image par celle du firmware (exec) : le pid de
/// l'enfant est celui du firmware.
///
/// Plafond mémoire : Linux applique `RLIMIT_AS` (`memory_backstop` octets).
/// macOS refuse `RLIMIT_AS` et `RLIMIT_DATA` (setrlimit renvoie une erreur et
/// rien n'est plafonné) : le plafond y repose sur la surveillance à 1 Hz de
/// l'empreinte mémoire (`process_usage`), qui arrête le firmware.
pub fn firmware_command(exe: &std::path::Path, memory_backstop: u64) -> tokio::process::Command {
    #[cfg(target_os = "macos")]
    let mut cmd = tokio::process::Command::new("/usr/sbin/taskpolicy");
    #[cfg(target_os = "macos")]
    {
        let _ = memory_backstop;
        cmd.args(["-c", "utility"]);
    }
    #[cfg(not(target_os = "macos"))]
    let mut cmd = tokio::process::Command::new("nice");
    #[cfg(not(target_os = "macos"))]
    {
        cmd.args(["-n", "10"]);
        let limit = memory_backstop as libc::rlim_t;
        // SAFETY : la fermeture ne fait qu'un appel système async-signal-safe
        // (setrlimit), entre fork et exec, sans allocation ni verrou.
        unsafe {
            cmd.pre_exec(move || {
                let lim = libc::rlimit {
                    rlim_cur: limit,
                    rlim_max: limit,
                };
                if libc::setrlimit(libc::RLIMIT_AS, &lim) != 0 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
    }
    cmd.arg(exe);
    cmd
}

/// Garde du processus firmware : rien à libérer sous macOS et Linux (l'enfant
/// est tué par `kill_on_drop` et se termine à la fermeture du socket).
pub struct FirmwareGuard;

/// Réglages après lancement : rien de plus sous macOS et Linux.
pub fn after_firmware_spawn(
    _child: &tokio::process::Child,
    _memory_backstop: u64,
) -> FirmwareGuard {
    FirmwareGuard
}

/// Temps CPU cumulé et mémoire d'un processus.
#[derive(Clone, Copy, Debug)]
pub struct ProcessUsage {
    pub cpu_time: std::time::Duration,
    /// Empreinte mémoire (macOS), mémoire résidente (Linux), en octets.
    pub memory: u64,
}

/// Mesure d'un processus vivant ; `None` s'il n'existe plus.
#[cfg(target_os = "macos")]
pub fn process_usage(pid: u32) -> Option<ProcessUsage> {
    let pid = i32::try_from(pid).ok()?;
    let mut info = std::mem::MaybeUninit::<libc::rusage_info_v2>::zeroed();
    // SAFETY : `info` est une zone de la taille de rusage_info_v2, la saveur
    // demandée (RUSAGE_INFO_V2) ; elle n'est lue que si l'appel réussit.
    let info = unsafe {
        if libc::proc_pid_rusage(pid, libc::RUSAGE_INFO_V2, info.as_mut_ptr().cast()) != 0 {
            return None;
        }
        info.assume_init()
    };
    // Temps en unités de mach_absolute_time (≠ ns sur Apple Silicon).
    let ticks = info.ri_user_time.saturating_add(info.ri_system_time);
    let (numer, denom) = mach_timebase();
    let ns = u128::from(ticks) * u128::from(numer) / u128::from(denom.max(1));
    Some(ProcessUsage {
        cpu_time: std::time::Duration::from_nanos(u64::try_from(ns).unwrap_or(u64::MAX)),
        memory: info.ri_phys_footprint,
    })
}

#[cfg(target_os = "macos")]
#[allow(deprecated)] // mach_timebase_info : stable côté système, déplacée vers mach2 dans libc
fn mach_timebase() -> (u32, u32) {
    static TIMEBASE: std::sync::OnceLock<(u32, u32)> = std::sync::OnceLock::new();
    *TIMEBASE.get_or_init(|| {
        let mut tb = libc::mach_timebase_info { numer: 1, denom: 1 };
        // SAFETY : `tb` est une structure valide, remplie par l'appel.
        if unsafe { libc::mach_timebase_info(&mut tb) } != 0 {
            return (1, 1);
        }
        (tb.numer, tb.denom)
    })
}

/// Linux : /proc/<pid>/stat (utime, stime) et /proc/<pid>/statm (résident).
#[cfg(not(target_os = "macos"))]
pub fn process_usage(pid: u32) -> Option<ProcessUsage> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    // Le nom du processus (champ 2) peut contenir des espaces : on repart de « ) ».
    let fields: Vec<&str> = stat.rsplit_once(')')?.1.split_whitespace().collect();
    let utime: u64 = fields.get(11)?.parse().ok()?;
    let stime: u64 = fields.get(12)?.parse().ok()?;
    let statm = std::fs::read_to_string(format!("/proc/{pid}/statm")).ok()?;
    let resident: u64 = statm.split_whitespace().nth(1)?.parse().ok()?;
    // SAFETY : sysconf est sans effet de bord.
    let (tck, page) = unsafe {
        (
            libc::sysconf(libc::_SC_CLK_TCK),
            libc::sysconf(libc::_SC_PAGESIZE),
        )
    };
    let tck = u64::try_from(tck).ok().filter(|t| *t > 0)?;
    let page = u64::try_from(page).ok()?;
    Some(ProcessUsage {
        cpu_time: std::time::Duration::from_nanos(
            (utime + stime).saturating_mul(1_000_000_000) / tck,
        ),
        memory: resident.saturating_mul(page),
    })
}

/// Suspend le processus (pause, section 11.2), y compris une boucle qui
/// n'appelle jamais libvmcu.
pub fn suspend_process(pid: u32) -> std::io::Result<()> {
    signal(pid, libc::SIGSTOP)
}

/// Reprend un processus suspendu.
pub fn resume_process(pid: u32) -> std::io::Result<()> {
    signal(pid, libc::SIGCONT)
}

fn signal(pid: u32, sig: libc::c_int) -> std::io::Result<()> {
    let pid = libc::pid_t::try_from(pid)
        .map_err(|_| std::io::Error::from(std::io::ErrorKind::InvalidInput))?;
    // SAFETY : envoi d'un signal à un pid ; aucune mémoire partagée.
    if unsafe { libc::kill(pid, sig) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}

/// Commande d'un outil de build (cmake).
pub fn tool_command(program: &std::path::Path) -> tokio::process::Command {
    tokio::process::Command::new(program)
}

/// Ouvre `url` (schéma vscode://) ; sinon, le fichier avec l'application par
/// défaut. macOS : `open` échoue s'il n'y a aucune application pour le schéma.
/// Linux : `xdg-open`. Bloquant : à appeler hors de la boucle asynchrone.
pub fn open_location(url: &str, file: &std::path::Path) -> std::io::Result<()> {
    #[cfg(target_os = "macos")]
    const OPENER: &str = "/usr/bin/open";
    #[cfg(not(target_os = "macos"))]
    const OPENER: &str = "xdg-open";
    let run = |arg: &std::ffi::OsStr| {
        std::process::Command::new(OPENER)
            .arg(arg)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok_and(|s| s.success())
    };
    if run(std::ffi::OsStr::new(url)) || run(file.as_os_str()) {
        Ok(())
    } else {
        Err(std::io::Error::other(format!("{OPENER} a échoué")))
    }
}

/// Adresse fixe du canal de débogage d'un projet (`key` : empreinte de son
/// chemin), inscrite dans .vscode/launch.json (section 10.2).
pub fn debug_socket_address(key: u64) -> String {
    format!("/tmp/prebench-debug-{:08x}.sock", key as u32)
}

/// Extension VS Code et débogueur nécessaires pour déboguer le firmware.
#[cfg(target_os = "macos")]
pub fn debug_requirements() -> super::DebugRequirements {
    super::DebugRequirements {
        extension_id: "vadimcn.vscode-lldb",
        extension_label: "CodeLLDB (extension VS Code)",
        debugger: None, // lldb est fourni avec CodeLLDB
        debugger_install: "",
    }
}

#[cfg(not(target_os = "macos"))]
pub fn debug_requirements() -> super::DebugRequirements {
    super::DebugRequirements {
        extension_id: "ms-vscode.cpptools",
        extension_label: "C/C++ (extension VS Code de Microsoft)",
        debugger: Some(("gdb", &["gdb"])),
        debugger_install: "sudo apt install gdb",
    }
}

/// Informations de débogage autonomes de l'exécutable lancé par le débogueur.
/// macOS : l'éditeur de liens ne garde qu'une table de renvoi vers les .o du
/// dossier de build ; dsymutil rassemble le DWARF dans `<programme>.dSYM`, à
/// côté de la copie, pour que lldb n'ait plus besoin des .o (supprimés ou
/// recompilés ensuite). Linux : DWARF déjà dans l'exécutable.
/// Bloquant : à appeler hors de la boucle asynchrone.
pub fn bundle_debug_symbols(program: &std::path::Path) -> std::io::Result<()> {
    #[cfg(target_os = "macos")]
    {
        let mut dsym = program.as_os_str().to_owned();
        dsym.push(".dSYM");
        let out = std::process::Command::new("/usr/bin/dsymutil")
            .arg(program)
            .arg("-o")
            .arg(&dsym)
            .stdin(std::process::Stdio::null())
            .output()?;
        if !out.status.success() {
            return Err(std::io::Error::other(
                String::from_utf8_lossy(&out.stderr).trim().to_string(),
            ));
        }
    }
    #[cfg(not(target_os = "macos"))]
    let _ = program;
    Ok(())
}

/// Temps CPU consommé par le fil appelant, en ns (CLOCK_THREAD_CPUTIME_ID) :
/// budget des modèles de composants (section 8.1), indépendant de la charge
/// de la machine. Repli sur l'horloge monotone si l'appel échoue.
pub fn thread_cpu_ns() -> u64 {
    let mut ts = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // SAFETY : `ts` est une structure valide, écrite par clock_gettime.
    let ok = unsafe { libc::clock_gettime(libc::CLOCK_THREAD_CPUTIME_ID, &mut ts) } == 0;
    if !ok {
        return super::monotonic_ns();
    }
    u64::try_from(ts.tv_sec)
        .unwrap_or(0)
        .saturating_mul(1_000_000_000)
        .saturating_add(u64::try_from(ts.tv_nsec).unwrap_or(0))
}

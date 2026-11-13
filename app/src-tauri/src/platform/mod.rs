// app/src-tauri/src/platform/mod.rs — Tout ce qui dépend du système (règle 16.2.6).
//
// macos.rs sert aussi pour Linux (cfg(unix)) ; windows.rs pour Windows.
// toolchain.rs : recherche des outils de build ; paths.rs : chemins passés
// aux outils. Tous deux communs aux systèmes.

pub mod paths;
pub mod toolchain;

#[cfg(unix)]
mod macos;
#[cfg(unix)]
pub use macos::*;

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::*;

/// Outils pour déboguer le firmware dans VS Code (section 10.2).
pub struct DebugRequirements {
    /// Identifiant de l'extension (`code --install-extension <id>`).
    pub extension_id: &'static str,
    pub extension_label: &'static str,
    /// Débogueur à trouver hors de l'extension : libellé et noms de fichier.
    pub debugger: Option<(&'static str, &'static [&'static str])>,
    pub debugger_install: &'static str,
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::{process_usage, resume_process, suspend_process};

    const BUSY_ENV: &str = "PREBENCH_TEST_BUSY_CHILD";

    /// Processus enfant occupé : ce binaire de test, relancé avec BUSY_ENV,
    /// tourne à vide dans ce test (sans BUSY_ENV, il ne fait rien).
    #[test]
    fn busy_child() {
        if std::env::var_os(BUSY_ENV).is_none() {
            return;
        }
        let mut x = 0u64;
        loop {
            x = std::hint::black_box(x.wrapping_add(1));
        }
    }

    /// CPU du processus, en % d'un cœur, mesuré sur `over`.
    fn cpu_pct(pid: u32, over: Duration) -> f64 {
        let a = process_usage(pid).expect("mesure");
        let t = Instant::now();
        std::thread::sleep(over);
        let b = process_usage(pid).expect("mesure");
        b.cpu_time.saturating_sub(a.cpu_time).as_secs_f64() / t.elapsed().as_secs_f64() * 100.0
    }

    /// Mesure CPU / mémoire, suspension et reprise d'un processus occupé
    /// (garde-fous de la section 11.2), sur chaque système.
    #[test]
    fn usage_suspend_resume() {
        let exe = std::env::current_exe().expect("binaire de test");
        let mut child = std::process::Command::new(exe)
            .args(["--exact", "platform::tests::busy_child"])
            .env(BUSY_ENV, "1")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .expect("processus occupé");
        let pid = child.id();
        std::thread::sleep(Duration::from_millis(300));

        let busy = cpu_pct(pid, Duration::from_secs(1));
        let memory = process_usage(pid).map(|u| u.memory).unwrap_or(0);
        println!("occupé : {busy:.0} % d'un cœur, {} Ko", memory / 1024);
        assert!(busy > 50.0, "processus occupé mesuré à {busy:.0} %");
        assert!(memory > 0, "mémoire mesurée nulle");

        suspend_process(pid).expect("suspension");
        std::thread::sleep(Duration::from_millis(200));
        let idle = cpu_pct(pid, Duration::from_secs(1));
        println!("suspendu : {idle:.1} % d'un cœur");

        resume_process(pid).expect("reprise");
        std::thread::sleep(Duration::from_millis(200));
        let again = cpu_pct(pid, Duration::from_secs(1));
        println!("repris : {again:.0} % d'un cœur");

        let _ = child.kill();
        let _ = child.wait();
        assert!(idle < 5.0, "processus suspendu mesuré à {idle:.1} %");
        assert!(again > 50.0, "processus repris mesuré à {again:.0} %");
        assert!(
            process_usage(u32::MAX - 7).is_none(),
            "pid inexistant mesuré"
        );
    }
}

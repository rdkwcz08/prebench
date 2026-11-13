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

/// Fenêtre native « Enregistrer sous » (NSSavePanel sous macOS,
/// IFileSaveDialog sous Windows, portail XDG ou GTK sous Linux), ouverte dans
/// `dir` avec le nom `name` proposé. None : l'utilisateur a annulé.
pub async fn ask_save_path<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    title: &str,
    dir: &std::path::Path,
    name: &str,
) -> Option<std::path::PathBuf> {
    use tauri_plugin_dialog::DialogExt;
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .set_title(title)
        .set_directory(dir)
        .set_file_name(name)
        .add_filter("Texte", &["txt"])
        .set_can_create_directories(true)
        .save_file(move |path| {
            let _ = tx.send(path);
        });
    rx.await
        .ok()
        .flatten()
        .and_then(|p| p.as_path().map(std::path::Path::to_path_buf))
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

    /// Temps CPU cumulé du processus.
    fn cpu_time(pid: u32) -> Duration {
        process_usage(pid).expect("mesure").cpu_time
    }

    /// Attend que le processus consomme `amount` de temps CPU. Indépendant de
    /// la charge de la machine : un processus occupé finit toujours par
    /// consommer ; plafond de sécurité de 60 s, qui n'est pas une mesure.
    fn consumes(pid: u32, amount: Duration) -> bool {
        let from = cpu_time(pid);
        let deadline = Instant::now() + Duration::from_secs(60);
        while Instant::now() < deadline {
            if cpu_time(pid).saturating_sub(from) >= amount {
                return true;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        false
    }

    /// Mesure CPU / mémoire, suspension et reprise d'un processus occupé
    /// (garde-fous de la section 11.2), sur chaque système. Tout est jugé en
    /// temps CPU consommé, jamais en pourcentage de l'horloge murale.
    #[test]
    fn usage_suspend_resume() {
        let exe = std::env::current_exe().expect("binaire de test");
        let child = std::process::Command::new(exe)
            .args(["--exact", "platform::tests::busy_child"])
            .env(BUSY_ENV, "1")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .expect("processus occupé");
        let pid = child.id();
        // Processus occupé tué à la fin du test, même en cas d'échec.
        struct KillOnDrop(std::process::Child);
        impl Drop for KillOnDrop {
            fn drop(&mut self) {
                let _ = self.0.kill();
                let _ = self.0.wait();
            }
        }
        let child = KillOnDrop(child);

        let busy = consumes(pid, Duration::from_millis(300));
        let memory = process_usage(pid).map(|u| u.memory).unwrap_or(0);

        suspend_process(pid).expect("suspension");
        // La suspension prend effet avant le retour ; la dernière tranche de
        // l'ordonnanceur peut encore être comptée : mesure à partir de 50 ms.
        std::thread::sleep(Duration::from_millis(50));
        let before = cpu_time(pid);
        std::thread::sleep(Duration::from_secs(1));
        let idle = cpu_time(pid).saturating_sub(before);
        println!("suspendu : {idle:?} de CPU en 1 s");

        resume_process(pid).expect("reprise");
        let again = consumes(pid, Duration::from_millis(300));

        drop(child);
        assert!(busy, "le processus occupé ne consomme pas de CPU");
        assert!(memory > 0, "mémoire mesurée nulle");
        assert!(
            idle < Duration::from_millis(10),
            "processus suspendu : {idle:?} de CPU"
        );
        assert!(again, "le processus repris ne consomme plus de CPU");
        assert!(
            process_usage(u32::MAX - 7).is_none(),
            "pid inexistant mesuré"
        );
    }
}

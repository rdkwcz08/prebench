// app/src-tauri/src/install_check.rs — Vérification de l'app installée.
//
//   Prebench --verifier-installation <dossier du projet de test> <rapport>
//
// Lancé par release.yml après l'installation silencieuse du .msi (Windows) ou
// la copie de Prebench.app depuis le .dmg (macOS), avant toute publication.
// Sans fenêtre, l'app fait ce que fait l'utilisateur au premier lancement :
//   1. copie du modèle blinky dans le dossier de test (nouveau projet) ;
//   2. ▶ Démarrer : recherche des outils, build « sim » dans le cache avec le
//      sdk installé, lancement du firmware ;
//   3. attente de plusieurs bascules de LD2 (modèle led dans QuickJS).
// Le rapport (journal du build et de la simulation) est écrit dans <rapport> :
// une app Windows sans console n'a pas de sortie standard. Code de sortie :
// 0 si tout a fonctionné, 1 sinon.

use std::path::PathBuf;
use std::time::Duration;

use tauri::{AppHandle, Listener, Runtime};
use tokio::sync::mpsc;

use crate::commands;
use crate::core::sim::SimMsg;
use crate::project::templates;

pub const FLAG: &str = "--verifier-installation";
const TEMPLATE: &str = "blinky-stm32-hal";
/// Bascules de LD2 attendues : le blinky clignote toutes les 500 ms.
const TOGGLES: usize = 4;
const TIMEOUT: Duration = Duration::from_secs(120);
/// Lignes de rapport conservées (règle 16.2.11).
const MAX_REPORT_LINES: usize = 2000;

pub struct Request {
    project_dir: PathBuf,
    report: PathBuf,
}

/// Arguments `--verifier-installation <projet> <rapport>`, s'ils sont présents.
pub fn request_from_args() -> Option<Request> {
    let args: Vec<String> = std::env::args().collect();
    let i = args.iter().position(|a| a == FLAG)?;
    Some(Request {
        project_dir: PathBuf::from(args.get(i + 1)?),
        report: PathBuf::from(args.get(i + 2)?),
    })
}

/// Lance la vérification, écrit le rapport et quitte le processus.
pub fn spawn<R: Runtime>(app: AppHandle<R>, req: Request) {
    tauri::async_runtime::spawn(async move {
        let mut report = Vec::new();
        let result = tokio::time::timeout(TIMEOUT, check(&app, &req, &mut report))
            .await
            .unwrap_or_else(|_| Err(format!("délai de {} s dépassé", TIMEOUT.as_secs())));
        let code = match result {
            Ok(()) => {
                report.push("✅ installation vérifiée".into());
                0
            }
            Err(e) => {
                report.push(format!("❌ {e}"));
                1
            }
        };
        report.truncate(MAX_REPORT_LINES);
        let text = report.join("\n") + "\n";
        // Le rapport est affiché par le workflow (fichier) : il n'est écrit sur
        // stderr que si le fichier n'a pas pu l'être, pour ne pas le doubler.
        if let Err(e) = std::fs::write(&req.report, &text) {
            eprintln!("rapport non écrit ({}) : {e}", req.report.display());
            eprint!("{text}");
        }
        std::process::exit(code);
    });
}

/// Événements de la simulation utiles au rapport.
enum Seen {
    Log(String),
    Status(String, String),
    Ld2(String),
}

async fn check<R: Runtime>(
    app: &AppHandle<R>,
    req: &Request,
    report: &mut Vec<String>,
) -> Result<(), String> {
    let root = commands::prebench_root(app);
    report.push(format!("sdk : {}", root.display()));
    if root.display().to_string().starts_with(r"\\?\") {
        return Err("chemin du sdk au format \\\\?\\".into());
    }

    templates::create(&root.join("templates"), TEMPLATE, &req.project_dir)
        .map_err(|e| format!("nouveau projet : {e}"))?;
    report.push(format!("projet : {}", req.project_dir.display()));

    let (tx, mut rx) = mpsc::unbounded_channel::<Seen>();
    for name in ["sim:log", "sim:status", "sim:states"] {
        let tx = tx.clone();
        app.listen_any(name, move |event| {
            let v: serde_json::Value =
                serde_json::from_str(event.payload()).unwrap_or(serde_json::Value::Null);
            let seen = match name {
                "sim:log" => Seen::Log(v["text"].as_str().unwrap_or_default().to_owned()),
                "sim:status" => Seen::Status(
                    v["state"].as_str().unwrap_or_default().to_owned(),
                    v["message"].as_str().unwrap_or_default().to_owned(),
                ),
                _ => match v["LD2"]["brightness"].as_f64() {
                    Some(b) => Seen::Ld2(format!("{b}")),
                    None => return,
                },
            };
            let _ = tx.send(seen);
        });
    }

    let sim = commands::launch(app, &req.project_dir)
        .await
        .map_err(|e| e.to_string());
    // Les journaux du build (chemins des outils, dossier de build) d'abord.
    while let Ok(seen) = rx.try_recv() {
        if let Seen::Log(text) = seen {
            report.push(text);
        }
    }
    let sim = sim?;

    let mut last: Option<String> = None;
    let mut toggles = 0;
    while toggles < TOGGLES {
        match rx.recv().await {
            Some(Seen::Log(text)) => report.push(text),
            Some(Seen::Status(state, message)) => {
                report.push(format!("état : {state} {message}"));
                if state == "error" || state == "stopped" {
                    return Err(format!("simulation interrompue : {message}"));
                }
            }
            Some(Seen::Ld2(level)) => {
                if last.as_ref() != Some(&level) {
                    toggles += 1;
                    report.push(format!("LD2 : {level}"));
                    last = Some(level);
                }
            }
            None => return Err("fin des événements".into()),
        }
    }
    let _ = sim.send(SimMsg::Stop).await;
    Ok(())
}

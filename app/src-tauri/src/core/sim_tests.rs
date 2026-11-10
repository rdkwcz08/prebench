// app/src-tauri/src/core/sim_tests.rs — Acteur de simulation face au vrai firmware.
//
// Le runtime simulé de Tauri remplace la fenêtre ; les firmwares sont compilés
// par le vrai build « sim » (CMake + clang/MinGW). L'interface est jouée par
// le test : il écoute les événements et répond comme le feraient les Workers.
//
// cargo test -- --nocapture affiche les mesures.

use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use serde_json::Value;
use tauri::test::{mock_builder, mock_context, noop_assets};
use tauri::Listener;

use super::*;
use crate::build::build_sim;
use crate::commands::resolve_for_tests;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

/// Événements reçus par le faux client, dans l'ordre d'arrivée.
struct Recorder {
    rx: mpsc::Receiver<(String, Value)>,
}

impl Recorder {
    /// Attend un événement satisfaisant `pred` ; panique à l'expiration.
    fn wait(
        &self,
        timeout: Duration,
        mut pred: impl FnMut(&str, &Value) -> bool,
    ) -> (String, Value) {
        let deadline = Instant::now() + timeout;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            let (name, v) = self
                .rx
                .recv_timeout(left)
                .expect("événement attendu non reçu à temps");
            if name == "sim:status" && v["state"] == "error" {
                panic!("erreur de simulation : {}", v["message"]);
            }
            if pred(&name, &v) {
                return (name, v);
            }
        }
    }
}

fn start_project(dir: &Path) -> (tauri::App<tauri::test::MockRuntime>, SimHandle, Recorder) {
    let app = mock_builder()
        .build(mock_context(noop_assets()))
        .expect("app simulée");
    let root = repo_root();
    let (project, board, components) = resolve_for_tests(&root, dir).expect("projet");
    let firmware = tauri::async_runtime::block_on(build_sim(dir, &root)).expect("build « sim »");

    let (tx, rx) = mpsc::channel();
    for name in ["sim:states", "sim:status", "sim:log", "sim:warn"] {
        let tx = tx.clone();
        app.handle().listen_any(name, move |event| {
            let value = serde_json::from_str(event.payload()).unwrap_or(Value::Null);
            let _ = tx.send((name.to_string(), value));
        });
    }
    let handle = start(
        app.handle().clone(),
        SimConfig {
            project_dir: dir.to_path_buf(),
            project,
            board,
            firmware,
            board_id: 1,
            components,
        },
    );
    (app, handle, Recorder { rx })
}

/// Luminosité de LD2 calculée par le modèle led (QuickJS).
fn ld2_level(v: &Value) -> Option<String> {
    v["LD2"]["brightness"].as_f64().map(|b| format!("{b}"))
}

/// Temps moyen entre `n` bascules de LD2 (horodatage du firmware, en ms).
fn mean_toggle_ms(rec: &Recorder, n: usize) -> f64 {
    let mut stamps = Vec::new();
    let mut last: Option<String> = None;
    while stamps.len() < n + 1 {
        let (_, v) = rec.wait(Duration::from_secs(5), |name, v| {
            name == "sim:states" && ld2_level(v).is_some()
        });
        let level = ld2_level(&v);
        if level != last {
            last = level;
            stamps.push(Instant::now());
        }
    }
    (stamps[n] - stamps[1]).as_secs_f64() * 1000.0 / (n as f64 - 1.0)
}

/// Critère de passage (partie cœur) : le blinky fait clignoter LD2 et B1
/// déclenche HAL_GPIO_EXTI_Callback, à travers le réseau logique.
#[test]
fn blinky_led_and_button() {
    let dir = repo_root().join("templates").join("blinky-stm32-hal");
    let (_app, sim, rec) = start_project(&dir);
    rec.wait(Duration::from_secs(10), |name, v| {
        name == "sim:status" && v["state"] == "running"
    });

    // Priorité effective lue par le firmware lui-même (section 11.2).
    let (_, log) = rec.wait(Duration::from_secs(5), |name, v| {
        name == "sim:log"
            && v["text"]
                .as_str()
                .is_some_and(|t| t.starts_with("priorité du firmware"))
    });
    println!("{}", log["text"].as_str().unwrap_or_default());

    let slow = mean_toggle_ms(&rec, 4);
    println!("LD2 : période {slow:.1} ms avant l'appui sur B1");
    assert!((450.0..650.0).contains(&slow), "période lente {slow} ms");

    // Appui sur B1 par son contrôle : modèle button (QuickJS) -> drive -> net -> PC13.
    for pressed in ["true", "false"] {
        sim.tx
            .try_send(SimMsg::Control {
                component: "B1".into(),
                control: "press".into(),
                value_json: pressed.into(),
            })
            .expect("commande vers l'acteur");
        std::thread::sleep(Duration::from_millis(30));
    }
    // Laisse passer le délai de 500 ms en cours, puis mesure.
    std::thread::sleep(Duration::from_millis(600));
    let fast = mean_toggle_ms(&rec, 6);
    println!("LD2 : période {fast:.1} ms après l'appui sur B1");
    assert!((70.0..200.0).contains(&fast), "période rapide {fast} ms");

    let _ = sim.tx.try_send(SimMsg::Stop);
}

/// Outil de latence : le composant d'écho tourne dans QuickJS (fil « modèles »).
/// Mesure firmware <-> cœur et firmware <-> cœur <-> modèle, le chemin réel
/// d'une transaction I2C ou SPI (section 8.1).
#[test]
fn latency_tool() {
    let dir = repo_root().join("tests").join("latency");
    let (_app, _sim, rec) = start_project(&dir);
    let mut results = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(60);
    while results.len() < 2 && Instant::now() < deadline {
        let (_, v) = rec.wait(Duration::from_secs(30), |name, _| name == "sim:log");
        if let Some(text) = v["text"].as_str() {
            if text.contains("médiane") {
                println!("{text}");
                results.push(text.to_string());
            } else if text.contains(':')
                && !text.starts_with("Mesure")
                && !text.starts_with("priorité du firmware")
            {
                panic!("mesure en échec : {text}");
            }
        }
    }
    assert_eq!(results.len(), 2, "deux mesures attendues : {results:?}");
}

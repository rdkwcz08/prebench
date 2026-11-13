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

/// Plafond de sécurité d'une attente : il évite qu'un test bloqué tourne sans
/// fin. Ce n'est pas une mesure : les tests attendent des événements et
/// mesurent le temps du firmware, jamais l'heure de réception.
const CEILING: Duration = Duration::from_secs(120);

/// Événements reçus par le faux client, dans l'ordre d'arrivée.
struct Recorder {
    rx: mpsc::Receiver<(String, Value)>,
    t0: Instant,
    seen: std::cell::RefCell<Vec<String>>,
    /// Dernier niveau écrit sur LD2 (PA5) par le firmware en cours.
    ld2_level: std::cell::Cell<Option<u64>>,
    /// Bascules de LD2 vues par `wait`, en temps du firmware (µs).
    ld2: std::cell::RefCell<Vec<u64>>,
}

impl Recorder {
    /// Attend un événement satisfaisant `pred` ; panique au plafond de sécurité
    /// en affichant les derniers événements reçus.
    fn wait(&self, mut pred: impl FnMut(&str, &Value) -> bool) -> (String, Value) {
        let deadline = Instant::now() + CEILING;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            let Ok((name, v)) = self.rx.recv_timeout(left) else {
                panic!(
                    "événement attendu non reçu ; derniers : {:#?}",
                    self.seen.borrow()
                );
            };
            {
                let mut seen = self.seen.borrow_mut();
                if seen.len() == 12 {
                    seen.remove(0);
                }
                let mut line = format!("{:>6} ms {name} {v}", self.t0.elapsed().as_millis());
                line.truncate(200);
                seen.push(line);
            }
            if name == "sim:status" && v["state"] == "error" {
                panic!("erreur de simulation : {}", v["message"]);
            }
            if name == "sim:status"
                && ["starting", "debug-wait", "stopped"]
                    .contains(&v["state"].as_str().unwrap_or(""))
            {
                // Nouveau lancement : l'horloge du firmware repart de zéro.
                self.ld2_level.set(None);
            }
            if name == "sim:test-gpio" && v["pin"] == "PA5" {
                let level = v["level"].as_u64();
                if self.ld2_level.get().is_some() && level != self.ld2_level.get() {
                    self.ld2.borrow_mut().push(v["t_us"].as_u64().unwrap_or(0));
                }
                self.ld2_level.set(level);
            }
            if pred(&name, &v) {
                return (name, v);
            }
        }
    }
}

/// Simulation d'un test, arrêtée à la fin du test même s'il échoue : sans
/// cela, un firmware suspendu par le garde-fou resterait figé (état T) après
/// la fin du binaire de test.
struct Sim(SimHandle);

impl std::ops::Deref for Sim {
    type Target = SimHandle;
    fn deref(&self) -> &SimHandle {
        &self.0
    }
}

impl Drop for Sim {
    fn drop(&mut self) {
        tauri::async_runtime::block_on(self.0.stop_and_wait());
    }
}

fn start_project(dir: &Path) -> (tauri::App<tauri::test::MockRuntime>, Sim, Recorder) {
    start_project_with(&repo_root(), dir, &test_build_dir(dir))
}

/// Dossier de build d'un test, hors du projet (comme le cache de l'app).
fn test_build_dir(dir: &Path) -> PathBuf {
    crate::build::build_dir_for(
        &std::env::temp_dir().join(format!("prebench-tests-{}", std::process::id())),
        dir,
    )
}

/// Build « sim » d'un projet. Plusieurs tests compilent le même projet dans
/// le même dossier de build : un verrou par dossier évite deux CMake à la fois
/// (l'app, elle, ne lance qu'un build à la fois).
fn build_locked(root: &Path, dir: &Path, build_dir: &Path) -> PathBuf {
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex, OnceLock};
    static LOCKS: OnceLock<Mutex<HashMap<PathBuf, Arc<Mutex<()>>>>> = OnceLock::new();
    let lock = LOCKS
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .entry(build_dir.to_path_buf())
        .or_default()
        .clone();
    let _guard = lock.lock().unwrap_or_else(|e| e.into_inner());
    tauri::async_runtime::block_on(build_sim(
        dir,
        root,
        build_dir,
        &crate::platform::toolchain::Toolchain::detect(),
    ))
    .expect("build « sim »")
}

fn start_project_with(
    root: &Path,
    dir: &Path,
    build_dir: &Path,
) -> (tauri::App<tauri::test::MockRuntime>, Sim, Recorder) {
    let (app, handle, rec, _) = start_project_full(root, dir, build_dir, None);
    (app, handle, rec)
}

/// Comme start_project_with ; renvoie aussi l'exécutable compilé.
fn start_project_full(
    root: &Path,
    dir: &Path,
    build_dir: &Path,
    debug: Option<DebugConfig>,
) -> (tauri::App<tauri::test::MockRuntime>, Sim, Recorder, PathBuf) {
    let app = mock_builder()
        .build(mock_context(noop_assets()))
        .expect("app simulée");
    let (project, board, components) = resolve_for_tests(root, dir).expect("projet");
    let firmware = build_locked(root, dir, build_dir);

    let (tx, rx) = mpsc::channel();
    for name in [
        "sim:states",
        "sim:status",
        "sim:log",
        "sim:warn",
        "sim:usage",
        "sim:guard",
        "sim:nets",
        "sim:uart",
        "sim:uart-config",
        "sim:test-gpio",
    ] {
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
            firmware: firmware.clone(),
            board_id: 1,
            components,
            debug,
        },
    );
    (
        app,
        Sim(handle),
        Recorder {
            rx,
            t0: Instant::now(),
            seen: Default::default(),
            ld2_level: Default::default(),
            ld2: Default::default(),
        },
        firmware,
    )
}

/// Instants (temps du firmware, µs) des `n` prochaines bascules de LD2 (PA5).
/// Horodatage des trames : indépendant de la charge et du délai de livraison.
fn ld2_toggles(rec: &Recorder, n: usize) -> Vec<u64> {
    let start = rec.ld2.borrow().len();
    while rec.ld2.borrow().len() < start + n {
        rec.wait(|name, v| name == "sim:test-gpio" && v["pin"] == "PA5");
    }
    rec.ld2.borrow()[start..start + n].to_vec()
}

/// Période moyenne de LD2 sur `n` intervalles, en ms de temps du firmware.
fn mean_toggle_ms(rec: &Recorder, n: usize) -> f64 {
    let t = ld2_toggles(rec, n + 1);
    (t[n] - t[0]) as f64 / 1000.0 / n as f64
}

/// Intervalles successifs d'une suite d'instants, en ms.
fn intervals_ms(t: &[u64]) -> Vec<f64> {
    t.windows(2)
        .map(|w| (w[1] - w[0]) as f64 / 1000.0)
        .collect()
}

/// Critère de passage (partie cœur) : le blinky fait clignoter LD2 et B1
/// déclenche HAL_GPIO_EXTI_Callback, à travers le réseau logique.
#[test]
fn blinky_led_and_button() {
    let dir = repo_root().join("templates").join("blinky-stm32-hal");
    let (_app, sim, rec) = start_project(&dir);
    rec.wait(|name, v| name == "sim:status" && v["state"] == "running");

    // Priorité effective lue par le firmware lui-même (section 11.2).
    let (_, log) = rec.wait(|name, v| {
        name == "sim:log"
            && v["text"]
                .as_str()
                .is_some_and(|t| t.starts_with("priorité du firmware"))
    });
    println!("{}", log["text"].as_str().unwrap_or_default());

    let slow = mean_toggle_ms(&rec, 4);
    println!("LD2 : période {slow:.1} ms avant l'appui sur B1");
    // Bornes en temps du firmware : un HAL_Delay ne finit jamais en avance
    // (borne basse stricte) ; la borne haute sépare seulement 500 ms de 100 ms,
    // une machine chargée pouvant retarder le réveil.
    assert!((450.0..1000.0).contains(&slow), "période lente {slow} ms");

    // Appui sur B1 par son contrôle : modèle button (QuickJS) -> drive -> net -> PC13.
    // Relâché une fois l'appui arrivé sur le net.
    for (pressed, level) in [("true", "low"), ("false", "pulled-high")] {
        sim.tx
            .try_send(SimMsg::Control {
                component: "B1".into(),
                control: "press".into(),
                value_json: pressed.into(),
            })
            .expect("commande vers l'acteur");
        rec.wait(|name, v| name == "sim:nets" && v["PC13"] == level);
    }
    // Le délai de 500 ms en cours se termine : on attend la première période
    // courte, puis on mesure.
    let mut previous = ld2_toggles(&rec, 1)[0];
    loop {
        let t = ld2_toggles(&rec, 1)[0];
        if t - previous < 300_000 {
            break;
        }
        previous = t;
    }
    let fast = mean_toggle_ms(&rec, 6);
    println!("LD2 : période {fast:.1} ms après l'appui sur B1");
    assert!((90.0..400.0).contains(&fast), "période rapide {fast} ms");

    let _ = sim.tx.try_send(SimMsg::Stop);
}

/// Reset (docs/protocol.md, 5.4) : le firmware se termine (code 71) et le cœur
/// le relance sur un nouveau canal. Sous Windows, le nom du canal doit changer
/// à chaque lancement (FILE_FLAG_FIRST_PIPE_INSTANCE) : deux resets de suite.
#[test]
fn reset_relaunches_firmware() {
    let dir = repo_root().join("templates").join("blinky-stm32-hal");
    let (_app, sim, rec) = start_project(&dir);
    rec.wait(|name, v| name == "sim:status" && v["state"] == "running");
    for round in 1..=2 {
        sim.tx.try_send(SimMsg::Reset).expect("reset");
        rec.wait(|name, v| {
            name == "sim:status"
                && v["state"] == "starting"
                && v["message"].as_str().is_some_and(|m| m.contains("reset"))
        });
        rec.wait(|name, v| name == "sim:status" && v["state"] == "running");
        let period = mean_toggle_ms(&rec, 3);
        println!("reset {round} : LD2 repart, période {period:.0} ms");
        assert!(
            (450.0..1000.0).contains(&period),
            "période {period} ms après le reset"
        );
    }
    let _ = sim.tx.try_send(SimMsg::Stop);
}

/// Débogage (section 10.2) : le test joue le rôle de VS Code. Il lance le
/// firmware avec les variables de launch.json, puis le fige comme un point
/// d'arrêt (tous les fils suspendus) : la barre d'état l'indique, aucun arrêt
/// ni pause CPU, et à la reprise le temps du firmware repart sans saut.
#[test]
fn debug_session_breakpoint_without_time_jump() {
    let dir = repo_root().join("templates").join("blinky-stm32-hal");
    let address = crate::platform::debug_socket_address(u64::from(std::process::id()) ^ 0x5eed);
    let program = std::env::temp_dir().join(format!(
        "prebench-debug-program-{}{}",
        std::process::id(),
        if cfg!(windows) { ".exe" } else { "" }
    ));
    let (_app, sim, rec, firmware) = start_project_full(
        &repo_root(),
        &dir,
        &test_build_dir(&dir),
        Some(DebugConfig {
            address: address.clone(),
            program: program.clone(),
        }),
    );
    rec.wait(|name, v| name == "sim:status" && v["state"] == "debug-wait");

    // « F5 » : le débogueur lance le firmware.
    let mut child = std::process::Command::new(&firmware)
        .env("PREBENCH_SOCKET", &address)
        .env("PREBENCH_BOARD_ID", "1")
        .env("PREBENCH_DEBUG", "1")
        .current_dir(&dir)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .expect("firmware lancé par le « débogueur »");
    rec.wait(|name, v| name == "sim:status" && v["state"] == "running");
    let before = mean_toggle_ms(&rec, 3);

    // Point d'arrêt juste après une bascule de LD2 (les bascules déjà reçues
    // mais pas encore lues sont gardées : la mesure porte sur toute la suite).
    ld2_toggles(&rec, 1);
    let first = rec.ld2.borrow().len() - 1;
    crate::platform::suspend_process(child.id()).expect("arrêt");
    let frozen_at = Instant::now();
    let (_, status) = rec.wait(|name, v| name == "sim:status" && v["state"] != "running");
    assert_eq!(status["state"], "breakpoint", "{status}");
    // Durée de l'arrêt : une donnée du scénario, pas une attente de résultat.
    std::thread::sleep(Duration::from_millis(1500).saturating_sub(frozen_at.elapsed()));

    crate::platform::resume_process(child.id()).expect("reprise");
    ld2_toggles(&rec, 2);
    let gaps = intervals_ms(&rec.ld2.borrow()[first..]);
    println!(
        "période {before:.0} ms ; intervalles autour de l'arrêt (temps du firmware) : {gaps:.0?}"
    );
    // Sans saut de temps, l'intervalle qui contient l'arrêt de 1,5 s reste
    // une période ordinaire de 500 ms dans le temps du firmware (un saut
    // donnerait au moins 1 500 ms).
    assert!(
        gaps.iter().all(|g| *g < 1200.0),
        "saut de temps : intervalles {gaps:.0?} ms"
    );

    // Fin de session dans le débogueur : la paillasse attend la suivante.
    let _ = child.kill();
    let _ = child.wait();
    rec.wait(|name, v| name == "sim:status" && v["state"] == "debug-wait");
    let _ = sim.tx.try_send(SimMsg::Stop);
    let _ = std::fs::remove_file(&program);
}

/// Validations du réseau logique (section 9.2) pendant la simulation : entrée
/// lue flottante, conflit de deux sorties ; niveaux des broches utilisées
/// envoyés à l'interface (« sim:nets »).
#[test]
fn net_checks_floating_and_conflict() {
    let dir = repo_root().join("tests").join("net-checks");
    let (_app, sim, rec) = start_project(&dir);
    rec.wait(|name, v| name == "sim:status" && v["state"] == "running");
    let (_, floating) = rec.wait(|name, v| name == "sim:warn" && v["code"] == 101);
    assert!(
        floating["text"]
            .as_str()
            .is_some_and(|t| t.contains("A0 (PA0) est lue flottante")),
        "{floating}"
    );
    rec.wait(|name, v| name == "sim:nets" && v["PA0"] == "floating" && v["PC13"] == "high");

    // B1 appuyé : la masse contre la sortie haute du firmware.
    sim.tx
        .try_send(SimMsg::Control {
            component: "B1".into(),
            control: "press".into(),
            value_json: "true".into(),
        })
        .expect("appui");
    let (_, conflict) = rec.wait(|name, v| name == "sim:warn" && v["code"] == 100);
    assert!(
        conflict["text"]
            .as_str()
            .is_some_and(|t| t.contains("Conflit sur B1 (PC13)")),
        "{conflict}"
    );
    rec.wait(|name, v| name == "sim:nets" && v["PC13"] == "conflict");
    let _ = sim.tx.try_send(SimMsg::Stop);
}

/// Moniteur série (étape 6) : débit annoncé, octets émis par le firmware,
/// envoi depuis le moniteur et écho, mesures au format du traceur.
#[test]
fn uart_echo_and_plotter_lines() {
    let dir = repo_root().join("tests").join("uart-echo");
    let (_app, sim, rec) = start_project(&dir);
    rec.wait(|name, v| name == "sim:status" && v["state"] == "running");
    let text = |v: &Value| -> String {
        v["data"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|b| b.as_u64().map(|x| x as u8))
                    .collect::<Vec<u8>>()
            })
            .map(|b| String::from_utf8_lossy(&b).into_owned())
            .unwrap_or_default()
    };
    let mut received = String::new();
    let mut baud = None;
    rec.wait(|name, v| {
        if name == "sim:uart-config" && v["id"] == 2 {
            baud = v["baud"].as_u64();
        }
        if name == "sim:uart" && v["id"] == 2 {
            received.push_str(&text(v));
        }
        received.contains("temp:")
    });
    assert_eq!(baud, Some(115_200));
    assert!(received.contains("écho USART2 prêt"), "{received}");

    sim.tx
        .try_send(SimMsg::UartSend {
            id: 2,
            data: b"Bonjour\n".to_vec(),
        })
        .expect("envoi");
    let mut echoed = String::new();
    rec.wait(|name, v| {
        if name == "sim:uart" && v["id"] == 2 {
            echoed.push_str(&text(v));
        }
        echoed.contains("Bonjour")
    });
    let _ = sim.tx.try_send(SimMsg::Stop);
}

/// Critère de passage de la phase 2 (garde-fou CPU, section 11.2) : une boucle
/// sans délai est mise en pause au bout de 5 s, suspendue (plus aucune mesure),
/// puis reprend sur demande et se remet en pause.
#[test]
fn guard_busy_loop_pauses() {
    let dir = repo_root().join("tests").join("guard-busy-loop");
    let (_app, sim, rec) = start_project(&dir);
    rec.wait(|name, v| name == "sim:status" && v["state"] == "running");
    // Pause après CPU_LIMIT_TIME de temps CPU consommé par le firmware (mesure
    // du processus, indépendante de la charge de la machine).
    let mut cpu_ms = 0_u64;
    let mut starved = 0_u32;
    let mut shares = Vec::new();
    let (name, guard) = rec.wait(|name, v| {
        if name == "sim:usage" {
            cpu_ms = v["cpu_time_ms"].as_u64().unwrap_or(0);
            let share = v["cpu"].as_f64().unwrap_or(0.0);
            shares.push(share.round());
            starved = if share > f64::from(CPU_LIMIT_PCT) {
                0
            } else {
                starved + 1
            };
        }
        name == "sim:guard" || starved >= 30
    });
    // Machine saturée : le firmware, en priorité basse (section 11.2), n'obtient
    // pas CPU_LIMIT_PCT % d'un cœur ; le garde-fou ne peut pas se déclencher.
    assert_eq!(
        name,
        "sim:guard",
        "machine saturée : la boucle du firmware n'a obtenu que {:?} % d'un cœur \
         (garde-fou au-delà de {CPU_LIMIT_PCT:.0} %)",
        &shares[shares.len().saturating_sub(10)..]
    );
    let needed = u64::try_from(CPU_LIMIT_TIME.as_millis()).unwrap_or(u64::MAX);
    println!("pause automatique après {cpu_ms} ms de temps CPU du firmware");
    assert_eq!(guard["kind"], "cpu");
    assert!(
        cpu_ms >= needed,
        "pause après {cpu_ms} ms de CPU (au moins {needed})"
    );
    rec.wait(|name, v| name == "sim:status" && v["state"] == "paused");

    // En pause : minuteur arrêté (aucune mesure) et processus suspendu.
    let quiet_until = Instant::now() + Duration::from_millis(1500);
    while let Some(left) = quiet_until.checked_duration_since(Instant::now()) {
        if let Ok((name, _)) = rec.rx.recv_timeout(left) {
            assert_ne!(name, "sim:usage", "mesure reçue pendant la pause");
        }
    }
    sim.tx.try_send(SimMsg::Resume).expect("reprise");
    let resumed_at = Instant::now();
    rec.wait(|name, v| name == "sim:status" && v["state"] == "running");
    let (_, usage) = rec.wait(|name, _| name == "sim:usage");
    let since_resume = u64::try_from(resumed_at.elapsed().as_millis()).unwrap_or(u64::MAX);
    let gained = usage["cpu_time_ms"]
        .as_u64()
        .unwrap_or(0)
        .saturating_sub(cpu_ms);
    // Suspendu, le firmware n'a consommé que depuis la reprise ; sinon, il
    // aurait aussi tourné pendant les 1,5 s de pause.
    assert!(
        gained <= since_resume + 500,
        "{gained} ms de CPU pour {since_resume} ms depuis la reprise : pause non suspendue"
    );
    let (_, again) = rec.wait(|name, _| name == "sim:guard");
    assert_eq!(again["kind"], "cpu");

    // Arrêt d'un firmware suspendu : aucun processus fantôme.
    sim.tx.try_send(SimMsg::Stop).expect("arrêt");
    rec.wait(|name, v| name == "sim:status" && v["state"] == "stopped");
}

/// Garde-fou mémoire (section 11.2) : une fuite de 80 Mo/s est arrêtée
/// au-delà de 256 Mo, avec un message qui dit pourquoi.
#[test]
fn guard_memory_leak_stops() {
    let dir = repo_root().join("tests").join("guard-memory-leak");
    let (_app, _sim, rec) = start_project(&dir);
    rec.wait(|name, v| name == "sim:status" && v["state"] == "running");
    // Au-delà de 128 Mo, mesure à 4 Hz : arrêt à moins de 40 Mo au-delà de la
    // limite pour une fuite de 80 Mo/s (jusqu'à 80 Mo de dépassement à 1 Hz).
    let mut peak = 0_u64;
    let mut fast = 0_u32;
    let (_, guard) = rec.wait(|name, v| {
        if name == "sim:usage" {
            let memory = v["memory"].as_u64().unwrap_or(0);
            if memory > MEMORY_LIMIT / 2 {
                fast += 1;
            }
            peak = peak.max(memory);
        }
        name == "sim:guard"
    });
    assert_eq!(guard["kind"], "memory");
    let mo = |b: u64| b / (1024 * 1024);
    println!("arrêt à {} Mo, {fast} mesures au-delà de 128 Mo", mo(peak));
    assert!(
        peak <= MEMORY_LIMIT + 40 * 1024 * 1024,
        "arrêt à {} Mo",
        mo(peak)
    );
    assert!(
        fast >= 3,
        "{fast} mesures au-delà de 128 Mo : pas de mesure rapprochée"
    );
    let (_, status) = rec.wait(|name, v| name == "sim:status" && v["state"] == "stopped");
    let message = status["message"].as_str().unwrap_or_default();
    println!("{message}");
    assert!(message.contains("256 Mo"), "message : {message}");
}

/// Outil de latence : le composant d'écho tourne dans QuickJS (fil « modèles »).
/// Mesure firmware <-> cœur et firmware <-> cœur <-> modèle, le chemin réel
/// d'une transaction I2C ou SPI (section 8.1).
#[test]
fn latency_tool() {
    let dir = repo_root().join("tests").join("latency");
    let (_app, _sim, rec) = start_project(&dir);
    let mut results = Vec::new();
    while results.len() < 2 {
        let (_, v) = rec.wait(|name, v| name == "sim:log" && v["source"] == "stdout");
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

/// Dossier temporaire dont le nom contient des espaces.
fn spaced_temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("prebench test {} {name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

/// Comme l'app installée (Windows : C:\Program Files\Prebench\sdk) : sdk,
/// projet créé depuis le modèle et cache de build, tous dans des chemins avec
/// espaces, hors du dépôt. Le blinky compile et LD2 clignote.
#[test]
fn installed_layout_with_spaces() {
    let base = spaced_temp_dir("installation");
    let sdk = base.join("Program Files").join("Prebench").join("sdk");
    for part in ["libvmcu", "boards", "components", "templates"] {
        crate::project::templates::copy_dir(&repo_root().join(part), &sdk.join(part), 0)
            .expect("copie du sdk");
    }
    let project = base.join("Mes Documents").join("Mon blinky");
    crate::project::templates::create(&sdk.join("templates"), "blinky-stm32-hal", &project)
        .expect("nouveau projet");
    let build_dir = crate::build::build_dir_for(&base.join("Cache Prebench"), &project);

    let (_app, sim, rec) = start_project_with(&sdk, &project, &build_dir);
    rec.wait(|name, v| name == "sim:status" && v["state"] == "running");
    let period = mean_toggle_ms(&rec, 3);
    assert!((450.0..1000.0).contains(&period), "période {period} ms");
    let _ = sim.tx.try_send(SimMsg::Stop);
    // Rien n'est écrit dans le sdk ni dans le projet.
    assert!(!sdk
        .join("templates")
        .join("blinky-stm32-hal")
        .join("build-sim")
        .exists());
    assert!(!project.join("build-sim").exists());
    rec.wait(|name, v| name == "sim:status" && v["state"] == "stopped");
    let _ = std::fs::remove_dir_all(base);
}

/// Erreur de compilation : le cache est gardé (pas de recompilation complète).
/// Échec de la configuration CMake : le cache est supprimé.
#[test]
fn build_cache_after_failures() {
    let base = spaced_temp_dir("echec");
    let project = base.join("projet");
    crate::project::templates::create(&repo_root().join("templates"), "blinky-stm32-hal", &project)
        .expect("nouveau projet");
    let build_dir = crate::build::build_dir_for(&base.join("cache"), &project);
    let toolchain = crate::platform::toolchain::Toolchain::detect();
    let build = || {
        tauri::async_runtime::block_on(build_sim(&project, &repo_root(), &build_dir, &toolchain))
    };

    // Erreur de compilation : échec, cache conservé, puis build après correction.
    let main_c = project.join("Core").join("Src").join("main.c");
    let source = std::fs::read_to_string(&main_c).expect("main.c");
    std::fs::write(&main_c, format!("{source}\nerreur de syntaxe volontaire\n")).expect("écriture");
    assert!(build().is_err(), "le build aurait dû échouer");
    assert!(
        build_dir.join("CMakeCache.txt").is_file(),
        "cache supprimé après une erreur de compilation"
    );
    std::fs::write(&main_c, &source).expect("écriture");
    assert!(build().expect("build après correction").is_file());

    // Échec de la configuration : dossier de build supprimé.
    let cmake_lists = project.join("CMakeLists.txt");
    let lists = std::fs::read_to_string(&cmake_lists).expect("CMakeLists.txt");
    std::fs::write(
        &cmake_lists,
        format!("{lists}\nmessage(FATAL_ERROR \"échec volontaire\")\n"),
    )
    .expect("écriture");
    assert!(build().is_err(), "la configuration aurait dû échouer");
    assert!(
        !build_dir.exists(),
        "dossier de build conservé après l'échec de la configuration"
    );
    std::fs::write(&cmake_lists, lists).expect("écriture");
    assert!(build().expect("build après correction").is_file());
    let _ = std::fs::remove_dir_all(base);
}

/// Fichiers objets produits par un build « sim » (noms seuls).
fn object_names(dir: &Path, out: &mut Vec<String>) {
    for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let path = entry.path();
        if path.is_dir() {
            object_names(&path, out);
        } else if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            if name.ends_with(".o") || name.ends_with(".obj") {
                out.push(name.to_owned());
            }
        }
    }
}

/// Critère de passage : le projet STM32CubeMX généré (chaîne CMake), non
/// modifié, est reconnu par son CMakeLists.txt et compilé via libvmcu/sim ;
/// system_stm32f4xx.c, syscalls.c et sysmem.c sont exclus, le startup .s et le
/// .ld ignorés ; l'initialisation va jusqu'à USART2, simulée (115 200 bauds).
#[test]
fn cubemx_generated_project() {
    let dir = repo_root().join("tests").join("cubemx-nucleo-f401re");
    for generated in [
        "CMakeLists.txt",
        "startup_stm32f401xe.s",
        "STM32F401xx_FLASH.ld",
        "Core/Src/system_stm32f4xx.c",
        "Core/Src/syscalls.c",
        "Core/Src/sysmem.c",
    ] {
        assert!(
            dir.join(generated).is_file(),
            "{generated} absent : génération incomplète"
        );
    }
    let build_dir = test_build_dir(&dir);
    let _ = std::fs::remove_dir_all(&build_dir);
    let (_app, sim, rec) = start_project(&dir);

    // Détection : configuré depuis libvmcu/sim, pas depuis le CMakeLists.txt de CubeMX.
    let cache = std::fs::read_to_string(build_dir.join("CMakeCache.txt")).expect("CMakeCache.txt");
    assert!(
        cache.contains("CMAKE_PROJECT_NAME:STATIC=prebench_sim"),
        "projet CubeMX non détecté"
    );

    let mut objects = Vec::new();
    object_names(&build_dir, &mut objects);
    for kept in ["main.c", "stm32f4xx_it.c", "stm32f4xx_hal_msp.c"] {
        assert!(
            objects.iter().any(|o| o.starts_with(kept)),
            "{kept} non compilé : {objects:?}"
        );
    }
    for excluded in [
        "system_stm32f4xx",
        "syscalls",
        "sysmem",
        "startup_stm32f401xe",
    ] {
        assert!(
            !objects.iter().any(|o| o.starts_with(excluded)),
            "{excluded} compilé à tort : {objects:?}"
        );
    }

    rec.wait(|name, v| name == "sim:status" && v["state"] == "running");
    // USART2 simulée (étape 6) : débit annoncé, aucun « HAL_UART_Init non simulé ».
    let mut warned = false;
    rec.wait(|name, v| {
        if name == "sim:warn"
            && v["text"]
                .as_str()
                .is_some_and(|t| t.contains("HAL_UART_Init"))
        {
            warned = true;
        }
        name == "sim:uart-config" && v["id"] == 2 && v["baud"] == 115_200
    });
    assert!(!warned, "HAL_UART_Init encore signalé non simulé");
    let _ = sim.tx.try_send(SimMsg::Stop);
}

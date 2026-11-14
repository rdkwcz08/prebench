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
            // Sortie série et mesures CPU : trop fréquentes, elles masqueraient
            // les événements utiles dans le diagnostic d'un échec.
            if name != "sim:uart" && name != "sim:usage" {
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
            // Composant arrêté (budget de temps, exception) : le test ne peut plus aboutir.
            if name == "sim:warn"
                && v["text"]
                    .as_str()
                    .is_some_and(|t| t.starts_with("composant ") && t.contains(" arrêté"))
            {
                panic!("{}", v["text"]);
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
    let (app, handle, rec, _) = start_project_full(root, dir, build_dir, None, None);
    (app, handle, rec)
}

/// Comme start_project_with ; renvoie aussi l'exécutable compilé.
fn start_project_full(
    root: &Path,
    dir: &Path,
    build_dir: &Path,
    debug: Option<DebugConfig>,
    fps: Option<u64>,
) -> (tauri::App<tauri::test::MockRuntime>, Sim, Recorder, PathBuf) {
    let app = mock_builder()
        .build(mock_context(noop_assets()))
        .expect("app simulée");
    let (project, board, components) = resolve_for_tests(root, dir).expect("projet");
    let firmware = build_locked(root, dir, build_dir);

    let (tx, rx) = mpsc::channel();
    // Images du cœur : découpées en événements séparés pour les tests.
    {
        let tx = tx.clone();
        let pins = std::sync::Mutex::new(serde_json::Map::new());
        app.handle().listen_any("sim:frame", move |event| {
            let value = serde_json::from_str(event.payload()).unwrap_or(Value::Null);
            let mut pins = pins.lock().unwrap_or_else(|e| e.into_inner());
            for part in super::split_frame(&value, &mut pins) {
                let _ = tx.send(part);
            }
        });
    }
    for name in [
        "sim:status",
        "sim:log",
        "sim:warn",
        "sim:usage",
        "sim:guard",
        "sim:uart-config",
        "sim:test-gpio",
        "sim:test-pwm",
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
            // Cadence d'affichage : 60 i/s, sauf PREBENCH_FRAME_RATE (mesures).
            frame_interval: frame_interval(fps.unwrap_or_else(|| {
                std::env::var("PREBENCH_FRAME_RATE")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(60)
            })),
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
    // Inspecteur : PA5 en sortie push-pull, période entre fronts montants
    // (deux bascules) mesurée par le cœur en temps du firmware.
    let (_, pins) = rec.wait(|name, v| {
        name == "sim:pins" && v["PA5"]["period_us"].as_u64().is_some_and(|p| p > 0)
    });
    assert_eq!(pins["PA5"]["mode"], 1, "{pins}");
    assert_eq!(pins["PA5"]["function"], Value::Null, "{pins}");
    let period = pins["PA5"]["period_us"].as_u64().unwrap_or(0);
    assert!(
        (900_000..2_000_000).contains(&period),
        "période mesurée {period} µs"
    );
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

/// Coupure d'alimentation (section 10.4) : firmware arrêté, LD2 éteinte,
/// broches relâchées, ligne au Journal ; à la remise sous tension, le
/// firmware repart de zéro (son horloge aussi) et LD2 clignote de nouveau.
#[test]
fn power_cut_and_restore() {
    let dir = repo_root().join("templates").join("blinky-stm32-hal");
    let (_app, sim, rec) = start_project(&dir);
    rec.wait(|name, v| name == "sim:status" && v["state"] == "running");
    ld2_toggles(&rec, 2);

    sim.tx.try_send(SimMsg::Power(false)).expect("coupure");
    // Journal, état, broches relâchées, LD2 éteinte, et PC13 flottante : le
    // pull-up de la carte n'est plus alimenté. Dans l'ordre d'arrivée, à
    // l'image près.
    let (mut off, mut logged, mut cleared, mut dark, mut floating) =
        (false, false, false, false, false);
    rec.wait(|name, v| {
        off |= name == "sim:status" && v["state"] == "off";
        logged |= name == "sim:log" && v["text"] == "Alimentation coupée";
        cleared |= name == "sim:pins" && v.as_object().is_some_and(|m| m.is_empty());
        dark |= name == "sim:states" && v["LD2"]["brightness"] == 0;
        floating |= name == "sim:nets" && v["PC13"] == "floating";
        off && logged && cleared && dark && floating
    });
    // Coupée : ni reset ni pause n'ont d'effet, aucune écriture du firmware.
    sim.tx.try_send(SimMsg::Reset).expect("reset");
    sim.tx.try_send(SimMsg::Pause).expect("pause");

    sim.tx
        .try_send(SimMsg::Power(true))
        .expect("remise sous tension");
    rec.wait(|name, v| name == "sim:log" && v["text"] == "Alimentation rétablie");
    rec.wait(|name, v| name == "sim:status" && v["state"] == "running");
    let restarted = ld2_toggles(&rec, 2);
    // Horloge du firmware repartie de zéro : processus neuf, RAM perdue.
    assert!(
        restarted[0] < 2_000_000,
        "temps du firmware {} µs",
        restarted[0]
    );
    let period = mean_toggle_ms(&rec, 2);
    assert!((450.0..1000.0).contains(&period), "période {period} ms");
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
        None,
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

/// Étape 8 : fondu de LD2 par TIM2_CH1 sur PA5, configuré comme dans CubeMX.
/// La broche porte la fonction PWM, 1 kHz ; la luminosité de LD2 (modèle led)
/// suit le rapport cyclique : pleine, éteinte et intermédiaire.
#[test]
fn pwm_fade_drives_led() {
    let dir = repo_root().join("tests").join("pwm-fade");
    let (_app, sim, rec) = start_project(&dir);
    rec.wait(|name, v| name == "sim:status" && v["state"] == "running");
    let (_, pins) =
        rec.wait(|name, v| name == "sim:pins" && v["PA5"]["pwm"]["freq_hz"].as_u64().is_some());
    assert_eq!(pins["PA5"]["mode"], 3, "{pins}");
    assert_eq!(pins["PA5"]["function"], "pwm:TIM2_CH1", "{pins}");
    assert_eq!(pins["PA5"]["pwm"]["freq_hz"], 1000, "{pins}");

    let (mut low, mut mid, mut high) = (false, false, false);
    rec.wait(|name, v| {
        if let Some(b) = (name == "sim:states")
            .then(|| v["LD2"]["brightness"].as_f64())
            .flatten()
        {
            low |= b <= 0.05;
            mid |= (0.3..0.7).contains(&b);
            high |= b >= 0.95;
        }
        low && mid && high
    });
    let _ = sim.tx.try_send(SimMsg::Stop);
}

/// Cadence d'affichage appliquée par le cœur (réglage frame_rate) : à 15 i/s,
/// jamais deux « sim:frame » à moins de 66 ms, alors que le fondu de LD2 change
/// toutes les 10 ms ; chaque image regroupe ce qui a changé entre-temps.
#[test]
fn frame_rate_caps_core_frames() {
    let dir = repo_root().join("tests").join("pwm-fade");
    let app = mock_builder()
        .build(mock_context(noop_assets()))
        .expect("app simulée");
    drop(app);
    let (app, sim, rec, _) =
        start_project_full(&repo_root(), &dir, &test_build_dir(&dir), None, Some(15));
    let times = std::sync::Arc::new(std::sync::Mutex::new(Vec::<Instant>::new()));
    {
        let times = std::sync::Arc::clone(&times);
        app.handle().listen_any("sim:frame", move |_| {
            times
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .push(Instant::now());
        });
    }
    rec.wait(|name, v| name == "sim:status" && v["state"] == "running");
    // 40 images d'états de LD2 (le fondu change sans cesse).
    let mut n = 0;
    rec.wait(|name, v| {
        if name == "sim:states" && v["LD2"].is_object() {
            n += 1;
        }
        n >= 40
    });
    let t = times.lock().unwrap_or_else(|e| e.into_inner()).clone();
    let gaps: Vec<f64> = t
        .windows(2)
        .map(|w| (w[1] - w[0]).as_secs_f64() * 1000.0)
        .collect();
    let min = gaps.iter().copied().fold(f64::INFINITY, f64::min);
    println!("Cadence 15 : {} images, écart minimal {min:.1} ms", t.len());
    // Horloge de réception : l'acteur n'émet jamais plus tôt que 66 ms ; la
    // livraison peut rapprocher deux images de quelques ms au plus.
    assert!(
        min >= 60.0,
        "écart minimal {min:.1} ms entre deux images (66 ms attendus)"
    );
    let _ = sim.tx.try_send(SimMsg::Stop);
}

/// Étape 8 : PWM à 20 kHz dont la consigne change toutes les 100 µs pendant
/// 3 s. Rien ne transite pour la PWM elle-même ; les consignes sont regroupées
/// (au plus une par lot de 1 ms) et le firmware ne sature pas le CPU.
#[test]
fn pwm_20khz_no_overload() {
    let dir = repo_root().join("tests").join("pwm-20khz");
    let (_app, sim, rec) = start_project(&dir);
    rec.wait(|name, v| name == "sim:status" && v["state"] == "running");
    let mut frames = 0u64;
    let mut freq = None;
    let mut cpu_ms = Vec::new();
    let mut guard = None;
    let (_, log) = rec.wait(|name, v| {
        match name {
            "sim:test-pwm" => {
                frames += 1;
                freq = v["freq_hz"].as_u64();
            }
            "sim:usage" => cpu_ms.push(v["cpu_time_ms"].as_u64().unwrap_or(0)),
            "sim:guard" => guard = Some(v.clone()),
            _ => {}
        }
        name == "sim:log"
            && v["text"]
                .as_str()
                .is_some_and(|t| t.starts_with("pwm-20khz :"))
    });
    let text = log["text"].as_str().unwrap_or_default();
    let consignes: u64 = text
        .split_whitespace()
        .find_map(|w| w.parse().ok())
        .unwrap_or(0);
    let cpu = cpu_ms.last().copied().unwrap_or(0);
    println!(
        "PWM 20 kHz : {consignes} consignes -> {frames} trames PWM ; CPU du firmware {cpu} ms"
    );
    assert_eq!(freq, Some(20_000));
    assert!(guard.is_none(), "garde-fou déclenché : {guard:?}");
    // 3 s de consignes : au plus un lot par milliseconde (marge de 20 %).
    assert!(
        frames <= 3_600,
        "{frames} trames PWM pour {consignes} consignes"
    );
    assert!(
        consignes > frames,
        "aucun regroupement : {consignes} consignes, {frames} trames"
    );
    // Temps CPU du firmware sur toute la durée : bien sous un cœur plein.
    assert!(
        cpu < 2_000,
        "CPU du firmware : {cpu} ms en un peu plus de 3 s"
    );
    // Fin de rafale : 50 % sur LD2.
    rec.wait(|name, v| {
        name == "sim:states"
            && v["LD2"]["brightness"]
                .as_f64()
                .is_some_and(|b| (b - 0.5).abs() < 0.01)
    });
    let _ = sim.tx.try_send(SimMsg::Stop);
}

/// Étape 8 : le potentiomètre (curseur sur A0) commande le servo (D5, 50 Hz).
/// ADC : tension et valeur brute selon adc_bits et adc_vref de la carte ;
/// servo : angle déduit de la largeur d'impulsion.
#[test]
fn potentiometer_drives_servo() {
    let dir = repo_root().join("tests").join("pot-servo");
    let (_app, sim, rec) = start_project(&dir);
    rec.wait(|name, v| name == "sim:status" && v["state"] == "running");
    let (_, pins) = rec.wait(|name, v| {
        name == "sim:pins"
            && v["PB4"]["pwm"]["freq_hz"] == 50
            && v["PA0"]["adc"]["raw"].as_u64().is_some()
    });
    assert_eq!(pins["PB4"]["function"], "pwm:TIM3_CH1", "{pins}");
    assert_eq!(pins["PA0"]["mode"], 5, "{pins}");
    assert_eq!(pins["PA0"]["adc"]["max"], 4095, "{pins}");

    for (position, raw, angle, expected_volts) in [
        ("0", 0u64, 0.0, 0.0),
        ("1", 4095, 180.0, 3.3),
        ("0.5", 2048, 90.0, 1.65),
    ] {
        sim.tx
            .try_send(SimMsg::Control {
                component: "pot1".into(),
                control: "position".into(),
                value_json: position.into(),
            })
            .expect("commande vers l'acteur");
        // Valeur ADC et angle du servo, dans n'importe quel ordre : l'état des
        // modèles et l'instantané des broches partent par deux chemins.
        let mut volts = None;
        let mut servo_ok = false;
        rec.wait(|name, v| {
            if name == "sim:pins" && v["PA0"]["adc"]["raw"] == raw {
                volts = v["PA0"]["adc"]["volts"].as_f64();
            }
            if name == "sim:states"
                && v["servo1"]["status"] == "ok"
                && v["servo1"]["angle"]
                    .as_f64()
                    .is_some_and(|a| (a - angle).abs() <= 1.0)
            {
                servo_ok = true;
            }
            volts.is_some() && servo_ok
        });
        // Tension du curseur : 3V3 × position.
        let volts = volts.unwrap_or(-1.0);
        assert!((volts - expected_volts).abs() < 0.001, "{volts} V");
    }
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
    // Inspecteur : PA2 en fonction alternative AF7, nommée par la carte.
    let (_, pins) = rec.wait(|name, v| name == "sim:pins" && v["PA2"].is_object());
    assert_eq!(pins["PA2"]["mode"], 3, "{pins}");
    assert_eq!(pins["PA2"]["function"], "uart2:TX", "{pins}");
    let _ = sim.tx.try_send(SimMsg::Stop);
}

/// Temps CPU consommé par ce processus (cœur, fil « modèles », test), en ms.
fn self_cpu_ms() -> u64 {
    crate::platform::process_usage(std::process::id())
        .map(|u| u64::try_from(u.cpu_time.as_millis()).unwrap_or(0))
        .unwrap_or(0)
}

/// Mesure du cœur sur la paillasse de tests/budget (section 11.1), hors
/// interface : CPU du cœur et du firmware, événements envoyés à la WebView
/// (nombre et taille par seconde). Enregistre le flux dans
/// target/budget-events.json pour le rejouer dans l'interface
/// (app/tests-ui/perf.spec.ts). À lancer à la main, en release :
///   cargo test --release --lib -- --ignored budget_core_profile --nocapture
#[test]
#[ignore = "mesure manuelle (release)"]
fn budget_core_profile() {
    let dir = repo_root().join("tests").join("budget");
    let (_app, sim, rec) = start_project(&dir);
    rec.wait(|name, v| name == "sim:status" && v["state"] == "running");
    // Pot à mi-course, puis 3 s de mise en route.
    let _ = sim.tx.try_send(SimMsg::Control {
        component: "pot1".into(),
        control: "position".into(),
        value_json: "0.6".into(),
    });
    let warm = Instant::now() + Duration::from_secs(3);
    rec.wait(|_, _| Instant::now() >= warm);

    const SECONDS: u64 = 20;
    let cpu0 = self_cpu_ms();
    let mut fw0 = None;
    let mut fw1 = 0;
    let t0 = Instant::now();
    let end = t0 + Duration::from_secs(SECONDS);
    let mut stats: std::collections::BTreeMap<String, (u64, u64)> = Default::default();
    let mut log = Vec::new();
    rec.wait(|name, v| {
        let text = v.to_string();
        let s = stats.entry(name.to_string()).or_default();
        s.0 += 1;
        s.1 += text.len() as u64;
        if name == "sim:usage" {
            let ms = v["cpu_time_ms"].as_u64().unwrap_or(0);
            fw0.get_or_insert(ms);
            fw1 = ms;
        }
        if !name.starts_with("sim:test-") && name != "sim:usage" {
            log.push(serde_json::json!({ "t": t0.elapsed().as_millis() as u64, "name": name, "payload": v }));
        }
        Instant::now() >= end
    });
    let wall = t0.elapsed().as_secs_f64();
    let core = (self_cpu_ms() - cpu0) as f64 / 10.0 / wall;
    let fw = (fw1 - fw0.unwrap_or(fw1)) as f64 / 10.0 / wall;
    println!("Cœur (processus de test) : {core:.1} % d'un cœur ; firmware : {fw:.1} %");
    println!("Événements vers l'interface, par seconde :");
    for (name, (n, bytes)) in &stats {
        println!(
            "  {name:<16} {:>7.1} /s {:>9.0} octets/s",
            *n as f64 / wall,
            *bytes as f64 / wall
        );
    }
    let out = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("budget-events.json");
    std::fs::write(&out, serde_json::Value::Array(log).to_string()).expect("écriture du flux");
    println!("Flux enregistré : {}", out.display());
    let _ = sim.tx.try_send(SimMsg::Stop);
}

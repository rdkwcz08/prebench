// app/src-tauri/src/core/sim.rs — Acteur de simulation d'une carte.
//
// Une tâche unique, purement événementielle (règle 16.2.9) : elle ne se
// réveille que sur une trame du firmware, une commande de l'interface, la fin
// du processus ou l'échéance d'un regroupement par image. Elle possède le
// réseau logique, le canal vers le firmware et le fil « modèles » (QuickJS).
//
// Chemins :
//   firmware -> lecteur -> acteur -> réseau -> GPIO_INPUT (firmware) / onPinChange (modèles)
//   modèle -> drive/pull -> acteur -> réseau -> ...
//   vue -> component_control -> acteur -> onControl (modèles)
//   modèles -> invalidate -> une fois par image : getState -> « sim:states » (vues)
//   réseau -> niveaux des broches utilisées, une fois par image -> « sim:nets » ;
//   PWM_CONFIG / PWM_DUTY -> broche en fonction alternative du timer -> réseau
//     (fréquence et rapport cyclique, jamais les fronts) -> modèles (PinState.pwm) ;
//   modèle -> setVoltage -> réseau -> ADC_VALUE (tension -> valeur brute selon
//     adc_bits et adc_vref de la carte) ; valeurs initiales avant WELCOME ;
//   UART_CONFIG -> « sim:uart-config » ; UART_TX -> une fois par image -> « sim:uart » ;
//   moniteur série -> uart_send -> UART_RX (trames de 4 Kio au plus)
//   conflit ou entrée lue flottante -> avertissement (section 9.2)
//   PING Worker : firmware -> acteur -> onPing du composant d'écho -> PONG (sans WebView)
//
// Garde-fous (section 11.2) : pendant une simulation, et seulement alors, un
// minuteur à 1 Hz mesure le CPU et la mémoire du firmware (« sim:usage »).
// Plus de 80 % d'un cœur pendant 5 s : pause automatique (PAUSE, puis
// suspension du processus pour une boucle qui n'appelle jamais libvmcu).
// Plus de 256 Mo : arrêt du firmware. Au-delà de 128 Mo, la mesure passe à
// 4 Hz pour arrêter une fuite rapide plus près de la limite (macOS n'a pas de
// plafond système). Le minuteur s'arrête en pause.
//
// Débogage (section 10.2) : le firmware est lancé par le débogueur de VS Code
// sur une adresse fixe du projet (`DebugConfig`) ; l'acteur l'attend sans
// délai, le reprend après chaque fin de session, et suit ses HEARTBEAT
// (protocole 1.1) : sans HEARTBEAT pendant BREAKPOINT_AFTER, le firmware est
// figé par le débogueur (« Arrêté sur un point d'arrêt »). Ni pause CPU, ni
// suspension, ni arrêt pour mémoire pendant une session de débogage.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::time::{Duration, Instant};

use interprocess::local_socket::tokio::prelude::*;
use interprocess::local_socket::tokio::Stream as LocalStream;
use interprocess::local_socket::{GenericFilePath, ListenerOptions};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Runtime};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::sync::{mpsc, oneshot};

use super::board::{pin_address, Board};
use super::components::engine::{self, InstanceSpec, ModelCmd, ModelEvent, Models};
use super::net::{Drive, Level, NetState, Network, Node, Pull, Pwm, Source};
use super::protocol::{self, msg, Header, Reader, Writer, FLAG_REPLY, HEADER_SIZE};
use crate::error::{CoreError, CoreResult};
use crate::platform;
use crate::project::{split_endpoint, Project, Wire};

/// Capacités des canaux : tous les tampons sont bornés (règle 16.2.11).
const ACTOR_QUEUE: usize = 4096;
const WRITER_QUEUE: usize = 4096;
/// Regroupement des états vers l'interface : une fois par image (section 11.3).
const UI_FRAME: Duration = Duration::from_millis(16);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const SYNC_TIMEOUT_MS: u32 = 1000;
const EXIT_RESET: i32 = 71;

/// Plafond mémoire d'un firmware (section 11.1), surveillé à 1 Hz.
pub const MEMORY_LIMIT: u64 = crate::settings::defaults::MEMORY_LIMIT_MB * 1024 * 1024;
/// Plafond posé par le système (Linux : RLIMIT_AS, mémoire virtuelle ; Windows :
/// Job Object, mémoire engagée), au double du plafond surveillé : la mesure
/// arrête une fuite ordinaire avec un message clair, ce plafond-ci une
/// allocation trop rapide pour elle. macOS n'en a pas (platform/macos.rs).
const MEMORY_BACKSTOP: u64 = 2 * MEMORY_LIMIT;
/// Pause automatique : plus de CPU_LIMIT_PCT % d'un cœur pendant CPU_LIMIT_TIME.
const CPU_LIMIT_PCT: f32 = crate::settings::defaults::CPU_LIMIT_PERCENT;
const CPU_LIMIT_TIME: Duration = Duration::from_secs(crate::settings::defaults::CPU_LIMIT_SECONDS);
/// Numéro du prochain lancement de firmware, unique dans le processus.
static LAUNCHES: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(1);
/// Attente de la fin du processus firmware à l'arrêt de la simulation.
const KILL_WAIT: Duration = Duration::from_secs(2);
const MONITOR_PERIOD: Duration = Duration::from_secs(1);
/// Mesure rapprochée (4 Hz) au-delà de la moitié du plafond mémoire.
const MONITOR_PERIOD_FAST: Duration = Duration::from_millis(250);
/// Délai entre PAUSE et la suspension du processus : laisse le fil E/S de
/// libvmcu geler l'horloge, pour que le temps reparte sans saut.
const SUSPEND_DELAY: Duration = Duration::from_millis(20);
/// Trame UART_RX : 4 Kio au plus (docs/protocol.md).
const UART_CHUNK: usize = 4096;
/// Octets UART en attente d'une image, par UART (règle 16.2.11).
const UART_OUT_MAX: usize = 64 * 1024;
/// Avertissements du réseau logique (section 9.2), hors des codes du firmware.
const WARN_CONFLICT: u16 = 100;
const WARN_FLOATING: u16 = 101;
/// PWM démarrée sans broche configurée sur sa fonction alternative.
const WARN_PWM_NO_PIN: u16 = 102;
/// Tension au-delà de la référence de l'ADC.
const WARN_ADC_RANGE: u16 = 103;
/// Débogage : sans HEARTBEAT (toutes les 200 ms) pendant ce délai, le firmware
/// est arrêté sur un point d'arrêt.
const BREAKPOINT_AFTER: Duration = Duration::from_millis(600);
/// Débogage : délai des transactions synchrones (WELCOME), une heure : un fil
/// arrêté dans le débogueur ne doit pas faire expirer une transaction.
const DEBUG_SYNC_TIMEOUT_MS: u32 = 3_600_000;
/// Débogage : nouvelle tentative de création du canal, le temps que Windows
/// ferme l'instance de la session précédente.
const DEBUG_LISTEN_RETRY: Duration = Duration::from_millis(50);
const DEBUG_LISTEN_TRIES: u32 = 40;

pub enum SimMsg {
    Frame(Header, Vec<u8>),
    Disconnected,
    Exited(Option<i32>),
    /// Action de l'utilisateur sur un contrôle d'une vue.
    Control {
        component: String,
        control: String,
        value_json: String,
    },
    /// Événement du fil « modèles ».
    Model(ModelEvent),
    FlushUi,
    Reset,
    /// Interrupteur d'alimentation : false coupe, true rétablit (section 10.4).
    Power(bool),
    Stop,
    /// Mise en pause demandée par l'utilisateur.
    Pause,
    /// Reprise demandée par l'utilisateur.
    Resume,
    /// Mesure du firmware : CPU en % d'un cœur depuis la mesure précédente,
    /// temps CPU consommé depuis (`used`) et cumulé, mémoire en octets.
    Usage {
        cpu_pct: f32,
        used: Duration,
        cpu_time: Duration,
        memory: u64,
    },
    /// Suspension du processus, `SUSPEND_DELAY` après PAUSE (numéro de pause).
    Suspend(u64),
    /// Rechargement à chaud (section 10.2) : le firmware en cours est arrêté
    /// et remplacé par ce nouvel exécutable ; paillasse, réseau et modèles
    /// sont conservés. En débogage : l'exécutable sert à la session suivante.
    Reload(PathBuf),
    /// Débogage : firmware lancé par le débogueur, connecté.
    DebugAttach(LocalStream),
    /// Débogage : échéance du HEARTBEAT numéro donné.
    BeatTimeout(u64),
    /// Câblage modifié dans l'app (section 10.4) : appliqué sans relance.
    SetWiring(Vec<crate::project::Wire>),
    /// Paramètres des composants (complétés par les défauts), par identifiant :
    /// ceux qui ont changé sont transmis aux modèles (onParamsChange).
    SetParams(BTreeMap<String, serde_json::Map<String, serde_json::Value>>),
    /// Moniteur série : octets vers l'UART `id` du firmware.
    UartSend {
        id: u8,
        data: Vec<u8>,
    },
}

pub struct SimHandle {
    pub tx: mpsc::Sender<SimMsg>,
    /// Session de débogage : reprise telle quelle si la simulation est relancée.
    pub debug: Option<DebugConfig>,
    /// Composants dont le modèle tourne : un composant ajouté ensuite n'est
    /// actif qu'au prochain démarrage.
    pub components: HashSet<String>,
}

impl SimHandle {
    pub fn is_alive(&self) -> bool {
        !self.tx.is_closed()
    }

    pub async fn send(&self, m: SimMsg) -> CoreResult<()> {
        self.tx.send(m).await.map_err(|_| CoreError::ActorGone)
    }

    /// Arrête la simulation et attend la fin de l'acteur (firmware tué).
    pub async fn stop_and_wait(&self) {
        if self.tx.send(SimMsg::Stop).await.is_ok() {
            self.tx.closed().await;
        }
    }
}

pub struct SimConfig {
    pub project_dir: PathBuf,
    pub project: Project,
    pub board: Board,
    pub firmware: PathBuf,
    pub board_id: u16,
    /// Instances de composants (carte puis projet), modèles résolus.
    pub components: Vec<InstanceSpec>,
    /// Session de débogage : le firmware est lancé par le débogueur.
    pub debug: Option<DebugConfig>,
}

/// Session de débogage (section 10.2).
#[derive(Clone)]
pub struct DebugConfig {
    /// Adresse fixe du projet, inscrite dans .vscode/launch.json.
    pub address: String,
    /// Exécutable lancé par le débogueur (copie, remplacée entre deux sessions).
    pub program: PathBuf,
}

// --- Événements vers l'interface ------------------------------------------------

#[derive(Clone, Serialize)]
struct StatusEvent {
    state: &'static str,
    message: String,
}

#[derive(Clone, Serialize)]
struct LogEvent {
    source: &'static str,
    level: u8,
    text: String,
}

#[derive(Clone, Serialize)]
struct WarnEvent {
    code: u16,
    text: String,
}

/// Débit configuré par le firmware (alerte de débit du moniteur série).
#[derive(Clone, Serialize)]
struct UartConfigEvent {
    id: u8,
    baud: u32,
}

/// Octets émis par une UART du firmware depuis l'image précédente.
#[derive(Clone, Serialize)]
struct UartEvent {
    id: u8,
    data: Vec<u8>,
}

/// Mesure du firmware pour l'indicateur de la barre d'état.
#[derive(Clone, Serialize)]
struct UsageEvent {
    /// Pourcentage d'un cœur.
    cpu: f32,
    /// Temps CPU consommé par le firmware depuis son lancement, en ms.
    cpu_time_ms: u64,
    /// Octets.
    memory: u64,
    memory_limit: u64,
}

/// Intervention d'un garde-fou, annoncée par une bannière.
#[derive(Clone, Serialize)]
struct GuardEvent {
    /// « cpu » (pause automatique) ou « memory » (firmware arrêté).
    kind: &'static str,
    text: String,
}

// --- Liaison avec le processus firmware --------------------------------------------

struct FirmwareLink {
    writer: mpsc::Sender<Vec<u8>>,
    kill: Option<oneshot::Sender<()>>,
    /// Lecture du canal : interrompue pour fermer le canal (fin de session de débogage).
    reader: tauri::async_runtime::JoinHandle<()>,
}

/// État d'une broche vu du firmware (GPIO_CONFIG / GPIO_WRITE).
#[derive(Clone, Copy, Default)]
struct FwPin {
    mode: u8,
    pull: u8,
    out: u8,
    /// Fonction alternative (GPIO_AFx) ; 0xFF : aucune.
    af: u8,
    /// Dernier front montant écrit par le firmware (temps du firmware, µs).
    last_rise_us: Option<u64>,
    /// Période entre les deux derniers fronts montants (µs).
    period_us: Option<u64>,
}

/// Configuration d'une broche par le firmware, pour l'inspecteur (« sim:pins »).
#[derive(Serialize, Clone)]
struct PinConfigEvent {
    /// GPIO_CONFIG (docs/protocol.md) : 0 entrée, 1 sortie push-pull, 2 sortie
    /// drain ouvert, 3 et 4 alternative, 5 analogique, 6 désactivée.
    mode: u8,
    /// 0 aucune, 1 pull-up, 2 pull-down.
    pull: u8,
    /// Capacité portée en mode alternatif (« uart2:TX ») ; « AF7 » si la carte
    /// ne la connaît pas ; null pour une broche GPIO.
    function: Option<String>,
    /// Période mesurée entre deux fronts montants écrits par le firmware (µs).
    period_us: Option<u64>,
    /// Sortie PWM active : fréquence et rapport cyclique.
    pwm: Option<PwmView>,
    /// Entrée analogique : tension et valeur brute lue par l'ADC.
    adc: Option<AdcView>,
}

#[derive(Serialize, Clone, Copy)]
struct PwmView {
    freq_hz: u32,
    /// Rapport cyclique de 0 à 1.
    duty: f64,
}

#[derive(Serialize, Clone, Copy)]
struct AdcView {
    volts: f64,
    raw: u16,
    /// Valeur brute maximale (4095 en 12 bits).
    max: u16,
}

/// Canal PWM du firmware (unité, canal) : STM32, timer et canal 1..4.
#[derive(Clone, Copy, Default)]
struct PwmChannel {
    freq_hz: u32,
    duty: u16,
}

impl FwPin {
    fn drive(self) -> Drive {
        match (self.mode, self.out) {
            (1, 1) => Drive::High, // sortie push-pull
            (1, _) => Drive::Low,
            (2, 0) => Drive::Low, // drain ouvert : ne tire que vers le bas
            _ => Drive::Z,
        }
    }

    fn pull(self) -> Pull {
        match self.pull {
            1 => Pull::Up,
            2 => Pull::Down,
            _ => Pull::None,
        }
    }

    /// La broche lit le net (entrée, drain ouvert, analogique…).
    fn reads_net(self) -> bool {
        !matches!(self.mode, 1 | 3)
    }
}

struct Actor<R: Runtime> {
    app: AppHandle<R>,
    cfg: SimConfig,
    tx_self: mpsc::Sender<SimMsg>,
    net: Network,
    link: Option<FirmwareLink>,
    fw_pins: HashMap<String, FwPin>,
    /// Configuration des broches modifiée : « sim:pins » à l'image suivante.
    pins_dirty: bool,
    /// Alimentation de la paillasse (section 10.4) : coupée, le firmware est
    /// arrêté et les alimentations de la carte retombent.
    powered: bool,
    /// Coupure demandée : la fin du processus firmware achève la coupure.
    power_off_pending: bool,
    addr_to_pin: HashMap<(u8, u8), String>,
    last_input: HashMap<String, (u8, u8)>,
    models: Option<Models>,
    states_dirty: bool,
    /// Octets émis par chaque UART du firmware, envoyés à l'image suivante (« sim:uart »).
    uart_out: BTreeMap<u8, Vec<u8>>,
    /// Niveaux des broches de la carte à envoyer à l'image suivante (« sim:nets »).
    nets_out: BTreeMap<String, Level>,
    /// Broches de la carte câblées (fils du projet, composants intégrés).
    wired: HashSet<String>,
    /// Avertissements du réseau déjà émis (code, broche) : une fois par broche.
    net_warned: HashSet<(u16, String)>,
    /// Canaux PWM du firmware ; une broche en fonction alternative du timer les porte.
    pwm_channels: BTreeMap<(u8, u8), PwmChannel>,
    /// Broche choisie explicitement par PWM_ATTACH (ESP32).
    pwm_attached: BTreeMap<(u8, u8), String>,
    /// Broches de la carte reliées à un canal ADC (capacités adcN:CHx).
    adc_pins: BTreeMap<String, (u8, u8)>,
    /// Broches dont le canal ADC est déclaré par le firmware (ADC_CONFIG).
    adc_configured: BTreeSet<String>,
    /// Dernière valeur brute envoyée au firmware, par broche.
    last_adc: HashMap<String, u16>,
    /// Paramètres connus des modèles, par composant.
    params: HashMap<String, serde_json::Map<String, serde_json::Value>>,
    ui_flush_scheduled: bool,
    out_seq: u32,
    echo: Option<String>,
    unknown_warned: HashSet<u16>,
    unknown_pins: HashSet<(u8, u8)>,
    guards: Guards,
    /// Débogage : attente de connexion du firmware lancé par le débogueur.
    debug_accept: Option<tauri::async_runtime::JoinHandle<()>>,
    /// Débogage : numéro du dernier HEARTBEAT, firmware figé.
    beat_seq: u64,
    at_breakpoint: bool,
}

/// État des garde-fous du firmware en cours.
#[derive(Default)]
struct Guards {
    pid: Option<u32>,
    monitor: Option<tauri::async_runtime::JoinHandle<()>>,
    paused: bool,
    suspended: bool,
    pause_seq: u64,
    /// Temps CPU consommé par le firmware tant qu'il reste au-delà de
    /// CPU_LIMIT_PCT, remis à zéro sous le seuil.
    busy: Duration,
    last_memory: u64,
    /// Message de fin imposé par un garde-fou (arrêt pour mémoire).
    stop_reason: Option<String>,
    /// Message d'état « en cours », rétabli à la reprise.
    running_message: String,
    /// Exécutable qui remplace le firmware arrêté (rechargement à chaud).
    reload: Option<PathBuf>,
}

pub fn start<R: Runtime>(app: AppHandle<R>, cfg: SimConfig) -> SimHandle {
    let (tx, rx) = mpsc::channel(ACTOR_QUEUE);
    let debug = cfg.debug.clone();
    let components = cfg.components.iter().map(|c| c.id.clone()).collect();
    let echo = cfg
        .components
        .iter()
        .find(|c| c.source.id == "echo")
        .map(|c| c.id.clone());
    let params = cfg
        .components
        .iter()
        .map(|c| (c.id.clone(), c.params.clone()))
        .collect();
    let adc_pins = cfg
        .board
        .pins
        .keys()
        .filter_map(|name| Some((name.clone(), cfg.board.adc_channel(name)?)))
        .collect();
    let mut actor = Actor {
        app,
        cfg,
        tx_self: tx.clone(),
        net: Network::default(),
        link: None,
        fw_pins: HashMap::new(),
        pins_dirty: false,
        powered: true,
        power_off_pending: false,
        addr_to_pin: HashMap::new(),
        last_input: HashMap::new(),
        models: None,
        states_dirty: false,
        nets_out: BTreeMap::new(),
        uart_out: BTreeMap::new(),
        wired: HashSet::new(),
        net_warned: HashSet::new(),
        pwm_channels: BTreeMap::new(),
        pwm_attached: BTreeMap::new(),
        adc_pins,
        adc_configured: BTreeSet::new(),
        last_adc: HashMap::new(),
        params,
        ui_flush_scheduled: false,
        out_seq: 0,
        echo,
        unknown_warned: HashSet::new(),
        unknown_pins: HashSet::new(),
        guards: Guards::default(),
        debug_accept: None,
        beat_seq: 0,
        at_breakpoint: false,
    };
    tauri::async_runtime::spawn(async move { actor.run(rx).await });
    SimHandle {
        tx,
        debug,
        components,
    }
}

impl<R: Runtime> Actor<R> {
    async fn run(&mut self, mut rx: mpsc::Receiver<SimMsg>) {
        let specs = std::mem::take(&mut self.cfg.components);
        let tx = self.tx_self.clone();
        match engine::spawn(
            specs,
            Box::new(move |e| {
                let _ = tx.blocking_send(SimMsg::Model(e));
            }),
        ) {
            Ok(models) => self.models = Some(models),
            Err(e) => self.warn(0, format!("fil des modèles impossible à démarrer : {e}")),
        }
        self.build_network();
        if let Err(e) = self.launch().await {
            self.status("error", e.to_string());
            return;
        }
        while let Some(m) = rx.recv().await {
            match m {
                SimMsg::Frame(h, payload) => self.on_frame(h, &payload).await,
                SimMsg::Disconnected => {
                    if self.cfg.debug.is_some() {
                        self.on_debug_session_end().await;
                    }
                }
                SimMsg::DebugAttach(stream) => {
                    self.attach(stream, None);
                    self.status(
                        "starting",
                        "débogueur connecté, démarrage du firmware".into(),
                    );
                }
                SimMsg::BeatTimeout(seq) => self.on_beat_timeout(seq),
                SimMsg::SetWiring(wires) => self.rewire(wires).await,
                SimMsg::SetParams(all) => self.set_params(all),
                SimMsg::UartSend { id, data } => {
                    for chunk in data.chunks(UART_CHUNK) {
                        let mut w = Writer::default();
                        w.u8(id).u8(0).u16(chunk.len() as u16).bytes(chunk);
                        self.send(msg::UART_RX, 0, 0, &w.buf).await;
                    }
                }
                SimMsg::Exited(code) => {
                    self.stop_monitor();
                    let reload = self.guards.reload.take();
                    if std::mem::take(&mut self.power_off_pending) {
                        if let Some(firmware) = reload {
                            self.cfg.firmware = firmware; // pris à la remise sous tension
                        }
                        self.power_down().await;
                    } else if let Some(firmware) = reload {
                        self.cfg.firmware = firmware;
                        self.status("starting", "rechargement du firmware".into());
                        self.log("firmware rechargé après le build");
                        self.reset_firmware_state();
                        if let Err(e) = self.launch().await {
                            self.status("error", e.to_string());
                            return;
                        }
                    } else if code == Some(EXIT_RESET) {
                        self.status("starting", "redémarrage du firmware (reset)".into());
                        self.reset_firmware_state();
                        if let Err(e) = self.launch().await {
                            self.status("error", e.to_string());
                            return;
                        }
                    } else {
                        let msg = self.exit_message(code);
                        self.status("stopped", msg);
                        return;
                    }
                }
                SimMsg::Control {
                    component,
                    control,
                    value_json,
                } => self.send_model(ModelCmd::Control {
                    id: component,
                    control,
                    value_json,
                }),
                SimMsg::Model(event) => self.on_model(event).await,
                SimMsg::FlushUi => self.flush_ui(),
                SimMsg::Reset => {
                    self.resume().await;
                    self.send(msg::RESET, 0, 0, &[]).await;
                }
                SimMsg::Power(false) => self.power_off().await,
                SimMsg::Power(true) => {
                    if let Err(e) = self.power_on().await {
                        self.status("error", e.to_string());
                        return;
                    }
                }
                SimMsg::Pause => self.pause(None).await,
                SimMsg::Resume => self.resume().await,
                SimMsg::Usage {
                    cpu_pct,
                    used,
                    cpu_time,
                    memory,
                } => self.on_usage(cpu_pct, used, cpu_time, memory).await,
                SimMsg::Suspend(seq) => self.suspend(seq),
                SimMsg::Reload(firmware) if self.cfg.debug.is_some() => {
                    self.debug_reload(firmware);
                }
                SimMsg::Reload(firmware) => {
                    if let Some(kill) = self.link.as_mut().and_then(|l| l.kill.take()) {
                        self.guards.reload = Some(firmware);
                        let _ = kill.send(());
                    } else {
                        // Firmware pas encore lancé : remplacé au prochain lancement.
                        self.cfg.firmware = firmware;
                    }
                }
                SimMsg::Stop => {
                    self.stop_monitor();
                    if let Some(task) = self.debug_accept.take() {
                        task.abort();
                    }
                    if let Some(link) = self.link.take() {
                        link.reader.abort(); // ferme le canal : fin de la session de débogage
                        if let Some(kill) = link.kill {
                            let _ = kill.send(());
                            // Fin effective du processus, même suspendu (SIGKILL) :
                            // l'app peut se fermer juste après.
                            let _ = tokio::time::timeout(KILL_WAIT, async {
                                while let Some(m) = rx.recv().await {
                                    if matches!(m, SimMsg::Exited(_)) {
                                        break;
                                    }
                                }
                            })
                            .await;
                        }
                    }
                    self.status("stopped", "simulation arrêtée".into());
                    return;
                }
            }
        }
    }

    // --- Réseau ---------------------------------------------------------------------

    fn build_network(&mut self) {
        self.connect_all();
        // État initial de toutes les broches de composants vers les modèles.
        for node in self.net.resolve_all() {
            self.notify_model(&node);
        }
        self.watch_all();
    }

    /// Broches utilisées (câblées ou configurées par le firmware) : niveaux vers l'interface.
    fn watch_all(&mut self) {
        let names: Vec<String> = self
            .wired
            .iter()
            .chain(self.fw_pins.keys())
            .cloned()
            .collect();
        for name in names {
            self.watch_net(&name);
        }
    }

    /// Niveau d'une broche de la carte : envoyé à l'image suivante si elle est
    /// utilisée ; conflit ou entrée lue flottante signalés une fois (section 9.2).
    fn watch_net(&mut self, name: &str) {
        let fw = self.fw_pins.get(name).copied();
        if !self.wired.contains(name) && fw.is_none() {
            return;
        }
        let level = self.net.level(&Node::Board(name.to_string()));
        self.nets_out.insert(name.to_string(), level);
        self.schedule_ui();
        let label = match self
            .cfg
            .board
            .pins
            .get(name)
            .and_then(|p| p.label.as_deref())
        {
            Some(l) if l != name => format!("{l} ({name})"),
            _ => name.to_string(),
        };
        if level == Level::Conflict && self.net_warned.insert((WARN_CONFLICT, name.to_string())) {
            self.warn(
                WARN_CONFLICT,
                format!("Conflit sur {label} : deux sorties imposent des niveaux opposés (court-circuit). Retirez un fil ou passez une sortie en entrée."),
            );
        }
        let reads = fw.is_some_and(|f| f.mode == 0 && f.pull == 0);
        if level == Level::Floating
            && reads
            && self.net_warned.insert((WARN_FLOATING, name.to_string()))
        {
            self.warn(
                WARN_FLOATING,
                format!("{label} est lue flottante : ni résistance de tirage ni composant ne fixe son niveau. Activez GPIO_PULLUP ou GPIO_PULLDOWN, ou câblez la broche."),
            );
        }
    }

    /// Liaisons et contributions de la carte : broches, alimentations,
    /// composants intégrés, fils du projet.
    fn connect_all(&mut self) {
        let board = self.cfg.board.clone();
        self.net.set_logic_voltage(board.logic_voltage);
        for name in board.pins.keys() {
            self.net.add_node(Node::Board(name.clone()));
            if let Some(addr) = pin_address(name) {
                self.addr_to_pin.insert(addr, name.clone());
            }
        }
        self.apply_supply();
        for (id, onboard) in &board.onboard {
            for (pin, target) in &onboard.wires {
                self.net.connect(
                    Node::Component {
                        id: id.clone(),
                        pin: pin.clone(),
                    },
                    Node::Board(target.clone()),
                );
            }
        }
        self.wired = board
            .onboard
            .values()
            .flat_map(|o| o.wires.values().cloned())
            .collect();
        for wire in &self.cfg.project.wires {
            let a = endpoint_node(&board, &wire.from);
            let b = endpoint_node(&board, &wire.to);
            for n in [&a, &b].into_iter().flatten() {
                if let Node::Board(s) = n {
                    self.wired.insert(s.clone());
                }
            }
            if let (Some(a), Some(b)) = (a, b) {
                self.net.connect(a, b);
            }
        }
    }

    /// Alimentations et résistances de tirage soudées sur la carte, selon
    /// l'interrupteur : coupé, les alimentations positives et les tirages
    /// retombent (flottants), la masse reste à 0 V. Renvoie les nœuds changés.
    fn apply_supply(&mut self) -> Vec<Node> {
        let mut changed = Vec::new();
        for (name, def) in &self.cfg.board.pins {
            let pull = match (self.powered, def.external_pull.as_deref()) {
                (true, Some("up")) => Pull::Up,
                (true, Some("down")) => Pull::Down,
                _ => Pull::None,
            };
            let node = Node::Board(name.clone());
            changed.extend(self.net.set_pull(&node, Source::Board, pull));
        }
        for (name, volts) in &self.cfg.board.power {
            let (drive, v) = match (*volts > 0.0, self.powered) {
                (false, _) => (Drive::Low, Some(0.0)),
                (true, true) => (Drive::High, Some(*volts)),
                (true, false) => (Drive::Z, None),
            };
            let node = Node::Board(name.clone());
            changed.extend(self.net.set_drive(&node, Source::Board, drive));
            changed.extend(self.net.set_volts(&node, Source::Board, v));
        }
        changed
    }

    /// Coupure de l'alimentation (section 10.4) : le firmware est arrêté (sa
    /// RAM est perdue), la coupure s'achève à la fin du processus. Sans effet
    /// en débogage : le processus appartient au débogueur.
    async fn power_off(&mut self) {
        if !self.powered || self.cfg.debug.is_some() {
            return;
        }
        self.powered = false;
        self.stop_monitor();
        if let Some(kill) = self.link.as_mut().and_then(|l| l.kill.take()) {
            self.power_off_pending = true;
            let _ = kill.send(());
        } else {
            self.power_down().await;
        }
    }

    /// Fin de la coupure : sorties du firmware relâchées, alimentations
    /// retombées, composants prévenus (onPower), interface informée.
    async fn power_down(&mut self) {
        if let Some(link) = self.link.take() {
            link.reader.abort();
        }
        self.guards.paused = false;
        self.guards.suspended = false;
        self.guards.busy = Duration::ZERO;
        self.reset_firmware_state();
        let changed = self.apply_supply();
        self.propagate(changed).await;
        self.send_model(ModelCmd::Power(false));
        self.states_dirty = true;
        self.schedule_ui();
        self.log("Alimentation coupée");
        self.status(
            "off",
            "rétablissez l'alimentation pour relancer le firmware depuis zéro".into(),
        );
    }

    /// Remise sous tension : alimentations rétablies, firmware relancé depuis
    /// zéro (la flash, c'est-à-dire l'exécutable, est conservée).
    async fn power_on(&mut self) -> CoreResult<()> {
        if self.powered {
            return Ok(());
        }
        self.powered = true;
        let changed = self.apply_supply();
        self.propagate(changed).await;
        self.send_model(ModelCmd::Power(true));
        self.log("Alimentation rétablie");
        self.status("starting", "remise sous tension".into());
        self.launch().await
    }

    /// Nouveau câblage pendant la simulation : réseau reconstruit, sorties du
    /// firmware et des composants reprises, niveaux changés propagés.
    async fn rewire(&mut self, wires: Vec<Wire>) {
        self.cfg.project.wires = wires;
        let old = std::mem::take(&mut self.net);
        self.connect_all();
        self.net.carry_over(&old);
        let nodes = self.net.resolve_all();
        let changed: Vec<Node> = nodes
            .into_iter()
            .filter(|n| old.level(n) != self.net.level(n))
            .collect();
        self.propagate(changed).await;
        self.watch_all();
    }

    fn send_model(&mut self, cmd: ModelCmd) {
        let sent = self.models.as_ref().is_some_and(|m| m.send(cmd));
        if !sent && self.unknown_warned.insert(0) {
            self.warn(
                0,
                "modèles de composants saturés : événements abandonnés".into(),
            );
        }
    }

    fn notify_model(&mut self, node: &Node) {
        if let Node::Component { id, pin } = node {
            let state = self.net.state(node);
            self.send_model(ModelCmd::Pin {
                id: id.clone(),
                pin: pin.clone(),
                state,
            });
        }
    }

    /// Paramètres modifiés dans l'inspecteur : transmis aux modèles concernés.
    fn set_params(&mut self, all: BTreeMap<String, serde_json::Map<String, serde_json::Value>>) {
        for (id, params) in all {
            match self.params.get(&id) {
                Some(old) if *old != params => {}
                _ => continue, // inchangés, ou composant en attente du prochain démarrage
            }
            let params_json = serde_json::Value::Object(params.clone()).to_string();
            self.params.insert(id.clone(), params);
            self.send_model(ModelCmd::Params { id, params_json });
        }
    }

    async fn on_model(&mut self, event: ModelEvent) {
        match event {
            ModelEvent::Drive { id, pin, drive } => {
                let changed =
                    self.net
                        .set_drive(&Node::Component { id, pin }, Source::Component, drive);
                self.propagate(changed).await;
            }
            ModelEvent::Pull { id, pin, pull } => {
                let changed =
                    self.net
                        .set_pull(&Node::Component { id, pin }, Source::Component, pull);
                self.propagate(changed).await;
            }
            ModelEvent::Volts { id, pin, volts } => {
                let changed =
                    self.net
                        .set_volts(&Node::Component { id, pin }, Source::Component, volts);
                self.propagate(changed).await;
            }
            ModelEvent::Pong { seq, handler_us } => self.pong(seq, 0, handler_us).await,
            ModelEvent::Dirty => {
                self.states_dirty = true;
                self.schedule_ui();
            }
            ModelEvent::States(states) => {
                let map: serde_json::Map<String, serde_json::Value> = states
                    .into_iter()
                    .map(|(id, json)| {
                        (
                            id,
                            serde_json::from_str(&json).unwrap_or(serde_json::Value::Null),
                        )
                    })
                    .collect();
                let _ = self.app.emit("sim:states", map);
            }
            ModelEvent::Warn(text) => self.warn(0, text),
            ModelEvent::Log(text) => {
                let _ = self.app.emit(
                    "sim:log",
                    LogEvent {
                        source: "composant",
                        level: 2,
                        text,
                    },
                );
            }
        }
    }

    fn reset_firmware_state(&mut self) {
        let pins: Vec<String> = self.fw_pins.keys().cloned().collect();
        self.fw_pins.clear();
        self.pwm_channels.clear();
        self.pwm_attached.clear();
        self.adc_configured.clear();
        self.last_adc.clear();
        self.pins_dirty = true;
        self.schedule_ui();
        self.last_input.clear();
        for name in pins {
            let node = Node::Board(name);
            self.net.set_drive(&node, Source::Firmware, Drive::Z);
            self.net.set_pull(&node, Source::Firmware, Pull::None);
        }
    }

    /// Répercute un changement de nets : entrées du firmware et interface.
    async fn propagate(&mut self, changed: Vec<Node>) {
        for node in changed {
            match &node {
                Node::Board(name) => {
                    let fw = self.fw_pins.get(name).copied().unwrap_or_default();
                    if self.link.is_some() && fw.reads_net() {
                        self.send_input(name.clone()).await;
                    }
                    if self.link.is_some() && self.adc_pins.contains_key(name) {
                        self.send_adc(name).await;
                    }
                    self.watch_net(name);
                }
                Node::Component { .. } => self.notify_model(&node),
            }
        }
    }

    async fn send_input(&mut self, name: String) {
        let Some(&(port, pin)) = self
            .addr_to_pin
            .iter()
            .find(|(_, n)| **n == name)
            .map(|(a, _)| a)
        else {
            return;
        };
        let level = self.net.level(&Node::Board(name.clone()));
        let flags = match level {
            Level::Floating => 1,
            Level::Conflict => 2,
            _ => 0,
        };
        let value = (level.as_bit(), flags);
        if self.last_input.get(&name) == Some(&value) {
            return;
        }
        self.last_input.insert(name, value);
        self.send(msg::GPIO_INPUT, 0, 0, &[port, pin, value.0, value.1])
            .await;
    }

    /// Tension vue par l'ADC sur une broche : tension connue du net, sinon
    /// déduite du niveau (rapport cyclique × tension logique pour une PWM).
    fn adc_volts(&self, name: &str) -> f64 {
        let state: NetState = self.net.state(&Node::Board(name.to_string()));
        let logic = self.cfg.board.logic_voltage;
        state.volts.unwrap_or(match (state.pwm, state.level) {
            (Some(p), Level::High | Level::Low) => p.ratio() * logic,
            (_, Level::High | Level::PulledHigh) => logic,
            _ => 0.0,
        })
    }

    /// Valeur brute de l'ADC pour une tension : adc_bits et adc_vref de la carte.
    fn adc_raw(&self, volts: f64) -> (u16, u16) {
        let bits = u32::from(self.cfg.board.adc_bits.clamp(1, 16));
        let max = ((1u32 << bits) - 1) as u16;
        let ratio = (volts / self.cfg.board.adc_vref()).clamp(0.0, 1.0);
        ((ratio * f64::from(max)).round() as u16, max)
    }

    /// Broche lue par l'ADC du firmware (mode analogique ou ADC_CONFIG).
    fn adc_used(&self, name: &str) -> bool {
        self.adc_configured.contains(name) || self.fw_pins.get(name).is_some_and(|f| f.mode == 5)
    }

    /// ADC_VALUE si la valeur brute de la broche a changé ; tension hors de la
    /// plage de l'ADC signalée une fois.
    async fn send_adc(&mut self, name: &str) {
        let Some(&(unit, channel)) = self.adc_pins.get(name) else {
            return;
        };
        let volts = self.adc_volts(name);
        let (raw, max) = self.adc_raw(volts);
        let vref = self.cfg.board.adc_vref();
        if self.adc_used(name)
            && volts > vref + 0.01
            && self.net_warned.insert((WARN_ADC_RANGE, name.to_string()))
        {
            let label = self.pin_label(name);
            self.warn(
                WARN_ADC_RANGE,
                format!(
                    "{label} reçoit {} V, au-delà de la référence de l'ADC ({} V) : la valeur sature à {max}. \
                     Alimentez le capteur en {} V ou ajoutez un pont diviseur.",
                    fr_volts(volts),
                    fr_volts(vref),
                    fr_volts(vref)
                ),
            );
        }
        if self.last_adc.get(name) == Some(&raw) {
            return;
        }
        self.last_adc.insert(name.to_string(), raw);
        let mut w = Writer::default();
        w.u8(unit).u8(channel).u16(raw);
        self.send(msg::ADC_VALUE, 0, 0, &w.buf).await;
        if self.adc_used(name) {
            self.pins_dirty = true;
            self.schedule_ui();
        }
    }

    /// « D13 (PA5) », ou le nom seul.
    fn pin_label(&self, name: &str) -> String {
        match self
            .cfg
            .board
            .pins
            .get(name)
            .and_then(|p| p.label.as_deref())
        {
            Some(l) if l != name => format!("{l} ({name})"),
            _ => name.to_string(),
        }
    }

    // --- PWM -------------------------------------------------------------------------------

    /// Broche qui porte un canal PWM : choisie par PWM_ATTACH, sinon la broche
    /// configurée en fonction alternative « pwm:TIMx_CHy » (af de la carte).
    fn pwm_pin(&self, key: (u8, u8)) -> Option<String> {
        if let Some(pin) = self.pwm_attached.get(&key) {
            return Some(pin.clone());
        }
        let cap = format!("pwm:TIM{}_CH{}", key.0, key.1);
        self.cfg
            .board
            .pins
            .iter()
            .find(|(name, def)| {
                def.af.get(&cap).is_some_and(|af| {
                    self.fw_pins
                        .get(*name)
                        .is_some_and(|fw| matches!(fw.mode, 3 | 4) && fw.af == *af)
                })
            })
            .map(|(name, _)| name.clone())
    }

    /// Canal PWM porté par une broche.
    fn pwm_channel_of(&self, name: &str) -> Option<PwmChannel> {
        self.pwm_channels
            .iter()
            .find(|(key, _)| self.pwm_pin(**key).as_deref() == Some(name))
            .map(|(_, c)| *c)
    }

    /// Ce que la broche impose au réseau : PWM du timer en fonction
    /// alternative (timer arrêté : niveau bas), sinon la sortie GPIO.
    fn pin_drive(&self, name: &str, fw: FwPin) -> Drive {
        if matches!(fw.mode, 3 | 4) {
            if let Some(c) = self.pwm_channel_of(name) {
                return if c.freq_hz > 0 {
                    Drive::Pwm(Pwm {
                        freq_hz: c.freq_hz,
                        duty: c.duty,
                    })
                } else {
                    Drive::Low
                };
            }
        }
        fw.drive()
    }

    /// Canal PWM modifié : sa broche est mise à jour (réseau, modèles, inspecteur).
    async fn pwm_changed(&mut self, key: (u8, u8), previous_pin: Option<String>) {
        let pin = self.pwm_pin(key);
        for name in previous_pin
            .iter()
            .chain(pin.iter())
            .cloned()
            .collect::<BTreeSet<_>>()
        {
            if let Some(fw) = self.fw_pins.get(&name).copied() {
                self.apply_firmware_pin(&name, fw).await;
            }
        }
        self.pins_dirty = true;
        self.schedule_ui();
        let running = self.pwm_channels.get(&key).is_some_and(|c| c.freq_hz > 0);
        if pin.is_none() && running {
            let cap = format!("pwm:TIM{}_CH{}", key.0, key.1);
            let candidates: Vec<String> = self
                .cfg
                .board
                .pins
                .iter()
                .filter_map(|(name, def)| {
                    def.af
                        .get(&cap)
                        .map(|af| format!("{} (GPIO_AF{af})", self.pin_label(name)))
                })
                .collect();
            let tag = format!("TIM{}_CH{}", key.0, key.1);
            if self.net_warned.insert((WARN_PWM_NO_PIN, tag.clone())) {
                let hint = if candidates.is_empty() {
                    format!(
                        "aucune broche de la carte {} ne porte cette fonction",
                        self.cfg.board.name
                    )
                } else {
                    format!("configurez {} en GPIO_MODE_AF_PP", candidates.join(" ou "))
                };
                self.warn(
                    WARN_PWM_NO_PIN,
                    format!("{tag} produit une PWM, mais aucune broche n'est configurée sur cette fonction : {hint}."),
                );
            }
        }
    }

    // --- Interface ----------------------------------------------------------------------

    /// Une image : getState des modèles modifiés au plus une fois par 16 ms.
    fn schedule_ui(&mut self) {
        if (!self.states_dirty
            && !self.pins_dirty
            && self.nets_out.is_empty()
            && self.uart_out.is_empty())
            || self.ui_flush_scheduled
        {
            return;
        }
        self.ui_flush_scheduled = true;
        let tx = self.tx_self.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(UI_FRAME).await;
            let _ = tx.send(SimMsg::FlushUi).await;
        });
    }

    fn flush_ui(&mut self) {
        self.ui_flush_scheduled = false;
        if std::mem::take(&mut self.states_dirty) {
            self.send_model(ModelCmd::FlushStates);
        }
        if !self.nets_out.is_empty() {
            let nets = std::mem::take(&mut self.nets_out);
            let _ = self.app.emit("sim:nets", nets);
        }
        for (id, data) in std::mem::take(&mut self.uart_out) {
            let _ = self.app.emit("sim:uart", UartEvent { id, data });
        }
        if std::mem::take(&mut self.pins_dirty) {
            let _ = self.app.emit("sim:pins", self.pin_configs());
        }
    }

    /// Configuration de toutes les broches réglées par le firmware (instantané).
    fn pin_configs(&self) -> BTreeMap<String, PinConfigEvent> {
        let analog = FwPin {
            mode: 5,
            af: 0xFF,
            ..Default::default()
        };
        let adc_only: Vec<(String, FwPin)> = self
            .adc_configured
            .iter()
            .filter(|n| !self.fw_pins.contains_key(*n))
            .map(|n| (n.clone(), analog))
            .collect();
        self.fw_pins
            .iter()
            .map(|(n, f)| (n.clone(), *f))
            .chain(adc_only)
            .map(|(name, fw)| {
                let name = &name;
                let function = matches!(fw.mode, 3 | 4).then(|| {
                    self.cfg
                        .board
                        .pins
                        .get(name)
                        .and_then(|def| def.af.iter().find(|(_, n)| **n == fw.af))
                        .map(|(cap, _)| cap.clone())
                        .unwrap_or_else(|| format!("AF{}", fw.af))
                });
                (
                    name.clone(),
                    PinConfigEvent {
                        mode: fw.mode,
                        pull: fw.pull,
                        function,
                        period_us: fw.period_us,
                        pwm: matches!(fw.mode, 3 | 4)
                            .then(|| self.pwm_channel_of(name))
                            .flatten()
                            .filter(|c| c.freq_hz > 0)
                            .map(|c| PwmView {
                                freq_hz: c.freq_hz,
                                duty: Pwm {
                                    freq_hz: c.freq_hz,
                                    duty: c.duty,
                                }
                                .ratio(),
                            }),
                        adc: (self.adc_used(name) && self.adc_pins.contains_key(name)).then(|| {
                            let volts = self.adc_volts(name);
                            let (raw, max) = self.adc_raw(volts);
                            AdcView { volts, raw, max }
                        }),
                    },
                )
            })
            .collect()
    }

    fn status(&self, state: &'static str, message: String) {
        let _ = self.app.emit("sim:status", StatusEvent { state, message });
    }

    fn log(&self, text: &str) {
        let _ = self.app.emit(
            "sim:log",
            LogEvent {
                source: "prebench",
                level: 2,
                text: text.to_string(),
            },
        );
    }

    fn warn(&self, code: u16, text: String) {
        let _ = self.app.emit("sim:warn", WarnEvent { code, text });
    }

    // --- Firmware -------------------------------------------------------------------------

    async fn send(&mut self, msg_type: u16, flags: u16, reply_seq: u32, payload: &[u8]) {
        let Some(link) = &self.link else { return };
        let seq = if flags & FLAG_REPLY != 0 {
            reply_seq
        } else {
            self.out_seq = self.out_seq.wrapping_add(1);
            self.out_seq
        };
        let frame = protocol::frame(msg_type, self.cfg.board_id, flags, seq, payload);
        // Contre-pression : si le firmware ne lit plus, l'acteur attend (aucune perte).
        let _ = link.writer.send(frame).await;
    }

    async fn pong(&mut self, seq: u32, status: u8, handler_us: u32) {
        let mut w = Writer::default();
        w.u8(1).u8(status).u16(0).u32(handler_us);
        self.send(msg::PONG, FLAG_REPLY, seq, &w.buf).await;
    }

    async fn on_frame(&mut self, h: Header, payload: &[u8]) {
        let mut r = Reader::new(payload);
        match h.msg_type {
            msg::HELLO => self.on_hello(h, &mut r).await,
            msg::GPIO_CONFIG => {
                let (port, pin, mode, pull) = (r.u8(), r.u8(), r.u8(), r.u8());
                let (_edge, af) = (r.u8(), r.u8());
                if let Some(name) = self.pin_name(port, pin) {
                    let fw = self.fw_pins.entry(name.clone()).or_default();
                    fw.mode = mode;
                    fw.pull = pull;
                    fw.af = af;
                    let fw = *fw;
                    self.pins_dirty = true;
                    self.schedule_ui();
                    self.apply_firmware_pin(&name, fw).await;
                }
            }
            msg::GPIO_WRITE => {
                let (port, pin, level) = (r.u8(), r.u8(), r.u8());
                if let Some(name) = self.pin_name(port, pin) {
                    // Tests : écriture horodatée par le temps du firmware, qui ne
                    // dépend ni de la charge de la machine ni du délai de livraison.
                    #[cfg(test)]
                    let _ = self.app.emit(
                        "sim:test-gpio",
                        serde_json::json!({ "pin": name, "level": level, "t_us": h.timestamp_us }),
                    );
                    let fw = self.fw_pins.entry(name.clone()).or_default();
                    if level == 1 && fw.out == 0 {
                        // Front montant : période en temps du firmware (inspecteur).
                        let t = h.timestamp_us;
                        let period = fw.last_rise_us.map(|prev| t.saturating_sub(prev));
                        if period.is_some() && period != fw.period_us {
                            fw.period_us = period;
                            self.pins_dirty = true;
                        }
                        fw.last_rise_us = Some(t);
                    }
                    fw.out = level;
                    let fw = *fw;
                    if self.pins_dirty {
                        self.schedule_ui();
                    }
                    self.apply_firmware_pin(&name, fw).await;
                }
            }
            msg::PWM_CONFIG | msg::PWM_DUTY | msg::PWM_ATTACH => {
                let key = (r.u8(), r.u8());
                let previous = self.pwm_pin(key);
                match h.msg_type {
                    msg::PWM_CONFIG => {
                        let _ = r.u16();
                        self.pwm_channels.entry(key).or_default().freq_hz = r.u32();
                    }
                    msg::PWM_DUTY => self.pwm_channels.entry(key).or_default().duty = r.u16(),
                    _ => {
                        let (port, pin) = (r.u8(), r.u8());
                        if let Some(name) = self.pin_name(port, pin) {
                            self.pwm_attached.insert(key, name);
                        }
                        self.pwm_channels.entry(key).or_default();
                    }
                }
                // Tests : chaque trame PWM reçue, pour compter ce qui transite.
                #[cfg(test)]
                {
                    let c = self.pwm_channels.get(&key).copied().unwrap_or_default();
                    let _ = self.app.emit(
                        "sim:test-pwm",
                        serde_json::json!({ "unit": key.0, "channel": key.1, "freq_hz": c.freq_hz, "duty": c.duty, "t_us": h.timestamp_us }),
                    );
                }
                self.pwm_changed(key, previous).await;
            }
            msg::ADC_CONFIG => {
                let key = (r.u8(), r.u8());
                let pin = self
                    .adc_pins
                    .iter()
                    .find(|(_, k)| **k == key)
                    .map(|(n, _)| n.clone());
                match pin {
                    Some(name) => {
                        self.adc_configured.insert(name.clone());
                        self.pins_dirty = true;
                        self.schedule_ui();
                        self.watch_net(&name);
                        self.send_adc(&name).await;
                    }
                    None => self.warn(
                        4,
                        format!(
                            "ADC{} canal {} absent de la carte {}",
                            key.0, key.1, self.cfg.board.name
                        ),
                    ),
                }
            }
            msg::WARN => {
                let code = r.u16();
                let text = r.str();
                let text = if code == 1 {
                    format!("{text} non simulé")
                } else {
                    text
                };
                self.warn(code, text);
            }
            msg::UART_CONFIG => {
                let id = r.u8();
                let _ = (r.u8(), r.u8(), r.u8());
                let baud = r.u32();
                let _ = self
                    .app
                    .emit("sim:uart-config", UartConfigEvent { id, baud });
            }
            msg::UART_TX => {
                let id = r.u8();
                let _ = r.u8();
                let len = r.u16() as usize;
                let data = r.bytes(len);
                let out = self.uart_out.entry(id).or_default();
                out.extend_from_slice(&data);
                if out.len() > UART_OUT_MAX {
                    // Interface en retard : les plus anciens octets cèdent la place (borné).
                    let excess = out.len() - UART_OUT_MAX;
                    out.drain(..excess);
                }
                self.schedule_ui();
            }
            msg::LOG => {
                let level = r.u8();
                let _ = r.u8();
                let text = r.str();
                let _ = self.app.emit(
                    "sim:log",
                    LogEvent {
                        source: "firmware",
                        level,
                        text,
                    },
                );
            }
            msg::HEARTBEAT => self.on_heartbeat(),
            msg::PING => {
                let target = r.u8();
                match (target, &self.echo) {
                    (0, _) => self.pong(h.seq, 0, 0).await,
                    (_, Some(echo)) => {
                        // Chemin réel d'une transaction synchrone : pas de regroupement.
                        let id = echo.clone();
                        self.send_model(ModelCmd::Ping { id, seq: h.seq });
                    }
                    (_, None) => self.pong(h.seq, 1, 0).await,
                }
            }
            other => {
                if self.unknown_warned.insert(other) {
                    self.warn(
                        0,
                        format!("message 0x{other:04X} pas encore géré par le cœur"),
                    );
                }
            }
        }
    }

    fn pin_name(&mut self, port: u8, pin: u8) -> Option<String> {
        let name = self.addr_to_pin.get(&(port, pin)).cloned();
        if name.is_none() && self.unknown_pins.insert((port, pin)) {
            self.warn(
                4,
                format!(
                    "broche (port {port}, pin {pin}) absente de la carte {}",
                    self.cfg.board.name
                ),
            );
        }
        name
    }

    async fn apply_firmware_pin(&mut self, name: &str, fw: FwPin) {
        let node = Node::Board(name.to_string());
        let drive = self.pin_drive(name, fw);
        let mut changed = self.net.set_drive(&node, Source::Firmware, drive);
        changed.extend(self.net.set_pull(&node, Source::Firmware, fw.pull()));
        self.propagate(changed).await;
        self.watch_net(name); // configurée par le firmware : utilisée, même sans changement de niveau
    }

    async fn on_hello(&mut self, h: Header, r: &mut Reader<'_>) {
        let major = r.u8();
        let minor = r.u8();
        let _api = (r.u8(), r.u8());
        let pid = r.u32();
        let board = r.str();
        let framework = r.str();
        let version = r.str();

        let reject = if major != protocol::PROTOCOL_MAJOR {
            Some((
                protocol::reject::VERSION,
                format!(
                    "protocole {major}.x, l'app attend {}.x",
                    protocol::PROTOCOL_MAJOR
                ),
            ))
        } else if h.board_id != self.cfg.board_id {
            Some((
                protocol::reject::BAD_BOARD_ID,
                "identifiant de carte inattendu".to_string(),
            ))
        } else if board != self.cfg.board.id {
            Some((
                protocol::reject::UNKNOWN_BOARD,
                format!(
                    "firmware compilé pour {board}, projet sur {}",
                    self.cfg.board.id
                ),
            ))
        } else if !self.cfg.board.framework.contains(&framework) {
            Some((
                protocol::reject::FRAMEWORK,
                format!(
                    "framework {framework} non pris en charge par {}",
                    self.cfg.board.name
                ),
            ))
        } else {
            None
        };
        if let Some((reason, text)) = reject {
            let mut w = Writer::default();
            w.u16(reason).str(&text);
            self.send(msg::REJECT, FLAG_REPLY, h.seq, &w.buf).await;
            self.status("error", format!("connexion refusée : {text}"));
            return;
        }

        // État initial des entrées, puis WELCOME (docs/protocol.md, 3.1).
        let names: Vec<String> = self.addr_to_pin.values().cloned().collect();
        for name in names {
            self.send_input(name).await;
        }
        self.last_adc.clear();
        let adc: Vec<String> = self.adc_pins.keys().cloned().collect();
        for name in adc {
            self.send_adc(&name).await;
        }
        let mut w = Writer::default();
        // Version retenue : la plus petite des deux mineures (docs/protocol.md, 3.2).
        w.u8(protocol::PROTOCOL_MAJOR)
            .u8(minor.min(protocol::PROTOCOL_MINOR))
            .u8(self.cfg.board.adc_bits)
            .u8(0)
            .u32(if self.cfg.debug.is_some() {
                DEBUG_SYNC_TIMEOUT_MS
            } else {
                SYNC_TIMEOUT_MS
            });
        self.send(msg::WELCOME, FLAG_REPLY, h.seq, &w.buf).await;
        self.guards.running_message =
            format!("{} — {framework}, libvmcu {version}", self.cfg.board.name);
        if self.cfg.debug.is_some() {
            // Processus lancé par le débogueur : mesuré pour l'indicateur seulement.
            self.guards.pid = Some(pid);
            self.guards.paused = false;
            self.guards.busy = Duration::ZERO;
            self.start_monitor();
            self.at_breakpoint = false;
            self.on_heartbeat();
            self.guards.running_message.push_str(" (débogage)");
            self.log(
                "débogage : le firmware est lancé par le débogueur, à la priorité que celui-ci lui donne \
                 (ni priorité basse, ni cœurs économes) ; pause automatique CPU désactivée pendant la session",
            );
        }
        self.status("running", self.guards.running_message.clone());
    }

    /// Lance le processus firmware et attend sa connexion ; en débogage, attend
    /// le firmware lancé par le débogueur.
    async fn launch(&mut self) -> CoreResult<()> {
        if self.cfg.debug.is_some() {
            self.listen_debug();
            return Ok(());
        }
        // Compteur commun à toutes les simulations du processus : deux acteurs
        // (une simulation et la suivante, ou des tests en parallèle) n'ouvrent
        // jamais le même socket, que try_overwrite remplacerait.
        let launch = LAUNCHES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let address = platform::socket_address(self.cfg.board_id, launch);
        let name = address.as_str().to_fs_name::<GenericFilePath>()?;
        let listener = ListenerOptions::new()
            .name(name)
            .try_overwrite(true)
            .create_tokio()?;

        let mut child = platform::firmware_command(&self.cfg.firmware, MEMORY_BACKSTOP)
            .env("PREBENCH_SOCKET", &address)
            .env("PREBENCH_BOARD_ID", self.cfg.board_id.to_string())
            .current_dir(&self.cfg.project_dir)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()?;
        let guard = platform::after_firmware_spawn(&child, MEMORY_BACKSTOP);
        self.guards.pid = child.id();
        self.guards.paused = false;
        self.guards.suspended = false;
        self.guards.busy = Duration::ZERO;
        self.start_monitor();
        if let Some(out) = child.stdout.take() {
            self.forward_lines(out, "stdout");
        }
        if let Some(err) = child.stderr.take() {
            self.forward_lines(err, "stderr");
        }

        let (kill_tx, kill_rx) = oneshot::channel::<()>();
        let tx = self.tx_self.clone();
        tauri::async_runtime::spawn(async move {
            // Le Job Object (Windows) vit aussi longtemps que le processus.
            let _guard = guard;
            let code = tokio::select! {
                status = child.wait() => status.ok().and_then(|s| s.code()),
                _ = kill_rx => {
                    let _ = child.kill().await;
                    None
                }
            };
            let _ = tx.send(SimMsg::Exited(code)).await;
        });

        let stream = tokio::time::timeout(CONNECT_TIMEOUT, listener.accept())
            .await
            .map_err(|_| CoreError::ConnectTimeout)??;
        drop(listener);
        self.attach(stream, Some(kill_tx));
        Ok(())
    }

    /// Relie l'acteur à un firmware connecté : écriture par lots et lecture des trames.
    fn attach(&mut self, stream: LocalStream, kill: Option<oneshot::Sender<()>>) {
        // &Stream implémente AsyncRead et AsyncWrite : lecture et écriture
        // concurrentes sur le même flux partagé.
        let stream = Arc::new(stream);
        let recv = Arc::clone(&stream);
        let send = stream;

        // Écriture : chaque réveil envoie tout ce qui attend en un seul lot.
        let (wtx, mut wrx) = mpsc::channel::<Vec<u8>>(WRITER_QUEUE);
        tauri::async_runtime::spawn(async move {
            while let Some(first) = wrx.recv().await {
                let mut batch = first;
                while let Ok(more) = wrx.try_recv() {
                    batch.extend_from_slice(&more);
                }
                let mut writer = &*send;
                if writer.write_all(&batch).await.is_err() {
                    break;
                }
            }
        });

        // Lecture : trames du firmware vers l'acteur.
        let tx = self.tx_self.clone();
        let board_id = self.cfg.board_id;
        let reader = tauri::async_runtime::spawn(async move {
            let mut raw = [0u8; HEADER_SIZE];
            let mut reader = &*recv;
            loop {
                if reader.read_exact(&mut raw).await.is_err() {
                    break;
                }
                let header = match Header::decode(&raw) {
                    Ok(h) if h.board_id == board_id => h,
                    _ => break, // erreur de protocole : fermeture, le firmware s'arrête (code 70)
                };
                let mut payload = vec![0u8; header.length as usize];
                if reader.read_exact(&mut payload).await.is_err() {
                    break;
                }
                if tx.send(SimMsg::Frame(header, payload)).await.is_err() {
                    return;
                }
            }
            let _ = tx.send(SimMsg::Disconnected).await;
        });

        self.link = Some(FirmwareLink {
            writer: wtx,
            kill,
            reader,
        });
    }

    // --- Débogage (section 10.2) ---------------------------------------------------------

    /// Attend, sans délai, le firmware lancé par le débogueur sur l'adresse du projet.
    fn listen_debug(&mut self) {
        let Some(debug) = &self.cfg.debug else { return };
        let address = debug.address.clone();
        let tx = self.tx_self.clone();
        let app = self.app.clone();
        self.status(
            "debug-wait",
            "en attente du débogueur : lancez « Prebench : déboguer le firmware » dans VS Code (F5)"
                .into(),
        );
        self.debug_accept = Some(tauri::async_runtime::spawn(async move {
            let mut last_error = String::new();
            for _ in 0..DEBUG_LISTEN_TRIES {
                let listener = address
                    .as_str()
                    .to_fs_name::<GenericFilePath>()
                    .and_then(|name| {
                        ListenerOptions::new()
                            .name(name)
                            .try_overwrite(true)
                            .create_tokio()
                    });
                match listener {
                    Ok(listener) => {
                        if let Ok(stream) = listener.accept().await {
                            let _ = tx.send(SimMsg::DebugAttach(stream)).await;
                        }
                        return;
                    }
                    Err(e) => {
                        last_error = e.to_string();
                        tokio::time::sleep(DEBUG_LISTEN_RETRY).await;
                    }
                }
            }
            let _ = app.emit(
                "sim:status",
                StatusEvent {
                    state: "error",
                    message: format!("canal de débogage impossible à ouvrir : {last_error}"),
                },
            );
        }));
    }

    /// Fin d'une session (firmware terminé, débogueur arrêté, reset) : la
    /// paillasse reste, l'acteur attend la session suivante.
    async fn on_debug_session_end(&mut self) {
        self.stop_monitor();
        self.link = None;
        self.guards.pid = None;
        self.at_breakpoint = false;
        self.beat_seq += 1; // annule l'échéance en cours
        self.reset_firmware_state();
        if let Some(firmware) = self.guards.reload.take() {
            self.debug_reload(firmware);
        }
        self.log("session de débogage terminée ; relancez-la dans VS Code (F5)");
        self.listen_debug();
    }

    /// Nouvel exécutable compilé pendant une session de débogage : copié pour
    /// la session suivante (celui en cours est verrouillé sous Windows).
    fn debug_reload(&mut self, firmware: PathBuf) {
        let Some(debug) = &self.cfg.debug else { return };
        if self.link.is_some() {
            // Une seule fois par session : les builds suivants remplacent le précédent.
            if self.guards.reload.replace(firmware).is_none() {
                self.log("firmware recompilé : arrêtez puis relancez la session de débogage (Maj+F5, puis F5) pour l'utiliser");
            }
            return;
        }
        let program = debug.program.clone();
        let app = self.app.clone();
        tauri::async_runtime::spawn_blocking(move || {
            if let Err(e) = crate::debug::install_program(&firmware, &program) {
                let _ = app.emit(
                    "sim:warn",
                    WarnEvent {
                        code: 0,
                        text: format!("firmware pour le débogueur non mis à jour : {e}"),
                    },
                );
            }
        });
    }

    fn on_heartbeat(&mut self) {
        self.beat_seq += 1;
        if self.at_breakpoint {
            self.at_breakpoint = false;
            let message = self.guards.running_message.clone();
            self.status("running", message);
        }
        let seq = self.beat_seq;
        let tx = self.tx_self.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(BREAKPOINT_AFTER).await;
            let _ = tx.send(SimMsg::BeatTimeout(seq)).await;
        });
    }

    fn on_beat_timeout(&mut self, seq: u64) {
        if seq != self.beat_seq || self.at_breakpoint || self.link.is_none() {
            return;
        }
        self.at_breakpoint = true;
        self.status("breakpoint", "le débogueur a arrêté le firmware".into());
    }

    // --- Garde-fous (section 11.2) -----------------------------------------------------

    /// Minuteur à 1 Hz (4 Hz au-delà de la moitié du plafond mémoire), actif
    /// seulement pendant que le firmware tourne.
    fn start_monitor(&mut self) {
        self.stop_monitor();
        let Some(pid) = self.guards.pid else { return };
        let tx = self.tx_self.clone();
        self.guards.monitor = Some(tauri::async_runtime::spawn(async move {
            let mut prev: Option<(Instant, Duration)> = None;
            while let Some(usage) = platform::process_usage(pid) {
                let now = Instant::now();
                if let Some((then, cpu)) = prev {
                    let used = usage.cpu_time.saturating_sub(cpu);
                    let wall = now.duration_since(then).as_secs_f64();
                    let cpu_pct = if wall > 0.0 {
                        (used.as_secs_f64() / wall * 100.0) as f32
                    } else {
                        0.0
                    };
                    let m = SimMsg::Usage {
                        cpu_pct,
                        used,
                        cpu_time: usage.cpu_time,
                        memory: usage.memory,
                    };
                    if tx.send(m).await.is_err() {
                        break;
                    }
                }
                prev = Some((now, usage.cpu_time));
                let period = if usage.memory > MEMORY_LIMIT / 2 {
                    MONITOR_PERIOD_FAST
                } else {
                    MONITOR_PERIOD
                };
                tokio::time::sleep(period).await;
            }
        }));
    }

    fn stop_monitor(&mut self) {
        if let Some(task) = self.guards.monitor.take() {
            task.abort();
        }
    }

    async fn on_usage(&mut self, cpu_pct: f32, used: Duration, cpu_time: Duration, memory: u64) {
        if self.guards.paused {
            return;
        }
        self.guards.last_memory = memory;
        let _ = self.app.emit(
            "sim:usage",
            UsageEvent {
                cpu: cpu_pct,
                cpu_time_ms: u64::try_from(cpu_time.as_millis()).unwrap_or(u64::MAX),
                memory,
                memory_limit: MEMORY_LIMIT,
            },
        );
        if self.cfg.debug.is_some() {
            return; // session de débogage : l'utilisateur garde la main
        }
        if memory > MEMORY_LIMIT {
            let text = format!(
                "Firmware arrêté : il a dépassé la limite de {} Mo de mémoire. \
                 Cherchez une allocation répétée sans libération (malloc sans free).",
                MEMORY_LIMIT / (1024 * 1024)
            );
            self.guard_event("memory", text.clone());
            self.guards.stop_reason = Some(text);
            self.stop_monitor();
            if let Some(kill) = self.link.as_mut().and_then(|l| l.kill.take()) {
                let _ = kill.send(());
            }
            return;
        }
        // Temps CPU consommé, pas durée écoulée : sur une machine chargée, une
        // mesure espacée ou un firmware ralenti ne gonfle pas le compte.
        if cpu_pct > CPU_LIMIT_PCT {
            self.guards.busy += used;
            if self.guards.busy >= CPU_LIMIT_TIME {
                let text = format!(
                    "Simulation mise en pause : le firmware a calculé sans pause pendant {} s, \
                     à plus de {CPU_LIMIT_PCT:.0} % d'un cœur. Une boucle sans délai tourne \
                     probablement à vide : ajoutez un HAL_Delay ou une attente d'interruption, \
                     puis reprenez.",
                    CPU_LIMIT_TIME.as_secs()
                );
                self.pause(Some(text)).await;
            }
        } else {
            self.guards.busy = Duration::ZERO;
        }
    }

    /// PAUSE au firmware, puis suspension du processus après `SUSPEND_DELAY`.
    /// `reason` : pause automatique, annoncée par une bannière.
    async fn pause(&mut self, reason: Option<String>) {
        if self.guards.paused || self.link.is_none() {
            return;
        }
        self.send(msg::PAUSE, 0, 0, &[]).await;
        self.guards.paused = true;
        self.guards.busy = Duration::ZERO;
        self.guards.pause_seq += 1;
        self.stop_monitor();
        let seq = self.guards.pause_seq;
        let tx = self.tx_self.clone();
        if self.cfg.debug.is_none() {
            // En débogage, le processus appartient au débogueur : PAUSE seulement.
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(SUSPEND_DELAY).await;
                let _ = tx.send(SimMsg::Suspend(seq)).await;
            });
        }
        match reason {
            Some(text) => {
                self.guard_event("cpu", text);
                self.status("paused", "pause automatique (CPU)".into());
            }
            None => self.status("paused", "simulation en pause".into()),
        }
    }

    fn suspend(&mut self, seq: u64) {
        if !self.guards.paused || self.guards.suspended || seq != self.guards.pause_seq {
            return;
        }
        let Some(pid) = self.guards.pid else { return };
        match platform::suspend_process(pid) {
            Ok(()) => self.guards.suspended = true,
            Err(e) => self.warn(0, format!("suspension du firmware impossible : {e}")),
        }
    }

    async fn resume(&mut self) {
        if !self.guards.paused {
            return;
        }
        if self.guards.suspended {
            if let Some(pid) = self.guards.pid {
                if let Err(e) = platform::resume_process(pid) {
                    self.warn(0, format!("reprise du firmware impossible : {e}"));
                }
            }
            self.guards.suspended = false;
        }
        self.guards.paused = false;
        self.send(msg::RESUME, 0, 0, &[]).await;
        self.start_monitor();
        self.status("running", self.guards.running_message.clone());
    }

    fn guard_event(&self, kind: &'static str, text: String) {
        let _ = self.app.emit("sim:guard", GuardEvent { kind, text });
    }

    /// Message de fin du firmware ; signale une limite mémoire probable.
    fn exit_message(&mut self, code: Option<i32>) -> String {
        if let Some(reason) = self.guards.stop_reason.take() {
            return reason;
        }
        let base = match code {
            Some(c) => format!("firmware arrêté (code {c})"),
            None => "firmware arrêté par un signal (plantage ?)".into(),
        };
        if code != Some(0) && self.guards.last_memory >= MEMORY_LIMIT / 4 * 3 {
            format!(
                "{base} : mémoire proche de la limite de {} Mo, une allocation a probablement été refusée",
                MEMORY_LIMIT / (1024 * 1024)
            )
        } else {
            base
        }
    }

    fn forward_lines<S>(&self, stream: S, source: &'static str)
    where
        S: tokio::io::AsyncRead + Unpin + Send + 'static,
    {
        let app = self.app.clone();
        tauri::async_runtime::spawn(async move {
            let mut lines = BufReader::new(stream).lines();
            while let Ok(Some(text)) = lines.next_line().await {
                let _ = app.emit(
                    "sim:log",
                    LogEvent {
                        source,
                        level: 2,
                        text,
                    },
                );
            }
        });
    }
}

/// Tension en français : « 3,3 », « 4,98 ».
fn fr_volts(v: f64) -> String {
    let text = format!("{:.2}", v);
    let text = text.trim_end_matches('0').trim_end_matches('.');
    text.replace('.', ",")
}

fn endpoint_node(board: &Board, endpoint: &str) -> Option<Node> {
    let (id, pin) = split_endpoint(endpoint)?;
    Some(if id == "board" {
        // Pastille « CN5:6 » ou signal « PA5 » : les pastilles d'un même signal
        // forment un seul nœud (fusion des GND, broches présentes deux fois).
        Node::Board(board.signal_of(pin)?)
    } else {
        Node::Component {
            id: id.to_string(),
            pin: pin.to_string(),
        }
    })
}

#[cfg(test)]
#[path = "sim_tests.rs"]
mod tests;

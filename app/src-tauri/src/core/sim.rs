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

use std::collections::{HashMap, HashSet};
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
use super::net::{Drive, Level, Network, Node, Pull, Source};
use super::protocol::{self, msg, Header, Reader, Writer, FLAG_REPLY, HEADER_SIZE};
use crate::error::{CoreError, CoreResult};
use crate::platform;
use crate::project::{split_endpoint, Project};

/// Capacités des canaux : tous les tampons sont bornés (règle 16.2.11).
const ACTOR_QUEUE: usize = 4096;
const WRITER_QUEUE: usize = 4096;
/// Regroupement des états vers l'interface : une fois par image (section 11.3).
const UI_FRAME: Duration = Duration::from_millis(16);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const SYNC_TIMEOUT_MS: u32 = 1000;
const EXIT_RESET: i32 = 71;

/// Plafond mémoire d'un firmware (section 11.1), surveillé à 1 Hz.
pub const MEMORY_LIMIT: u64 = 256 * 1024 * 1024;
/// Plafond posé par le système (Linux : RLIMIT_AS, mémoire virtuelle ; Windows :
/// Job Object, mémoire engagée), au double du plafond surveillé : la mesure
/// arrête une fuite ordinaire avec un message clair, ce plafond-ci une
/// allocation trop rapide pour elle. macOS n'en a pas (platform/macos.rs).
const MEMORY_BACKSTOP: u64 = 2 * MEMORY_LIMIT;
/// Pause automatique : plus de CPU_LIMIT_PCT % d'un cœur pendant CPU_LIMIT_TIME.
const CPU_LIMIT_PCT: f32 = 80.0;
const CPU_LIMIT_TIME: Duration = Duration::from_secs(5);
const MONITOR_PERIOD: Duration = Duration::from_secs(1);
/// Mesure rapprochée (4 Hz) au-delà de la moitié du plafond mémoire.
const MONITOR_PERIOD_FAST: Duration = Duration::from_millis(250);
/// Délai entre PAUSE et la suspension du processus : laisse le fil E/S de
/// libvmcu geler l'horloge, pour que le temps reparte sans saut.
const SUSPEND_DELAY: Duration = Duration::from_millis(20);
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
    Stop,
    /// Mise en pause demandée par l'utilisateur.
    Pause,
    /// Reprise demandée par l'utilisateur.
    Resume,
    /// Mesure du firmware : CPU en % d'un cœur sur `span`, mémoire en octets.
    Usage {
        cpu_pct: f32,
        memory: u64,
        span: Duration,
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
}

pub struct SimHandle {
    pub tx: mpsc::Sender<SimMsg>,
    /// Session de débogage : reprise telle quelle si la simulation est relancée.
    pub debug: Option<DebugConfig>,
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

/// Mesure du firmware pour l'indicateur de la barre d'état.
#[derive(Clone, Serialize)]
struct UsageEvent {
    /// Pourcentage d'un cœur.
    cpu: f32,
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
    addr_to_pin: HashMap<(u8, u8), String>,
    last_input: HashMap<String, (u8, u8)>,
    models: Option<Models>,
    states_dirty: bool,
    ui_flush_scheduled: bool,
    out_seq: u32,
    echo: Option<String>,
    unknown_warned: HashSet<u16>,
    unknown_pins: HashSet<(u8, u8)>,
    guards: Guards,
    /// Nombre de lancements du firmware : nom de canal propre à chacun.
    launches: u32,
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
    /// Durée cumulée au-delà de CPU_LIMIT_PCT, remise à zéro sous le seuil.
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
    let echo = cfg
        .components
        .iter()
        .find(|c| c.source.id == "echo")
        .map(|c| c.id.clone());
    let mut actor = Actor {
        app,
        cfg,
        tx_self: tx.clone(),
        net: Network::default(),
        link: None,
        fw_pins: HashMap::new(),
        addr_to_pin: HashMap::new(),
        last_input: HashMap::new(),
        models: None,
        states_dirty: false,
        ui_flush_scheduled: false,
        out_seq: 0,
        echo,
        unknown_warned: HashSet::new(),
        unknown_pins: HashSet::new(),
        guards: Guards::default(),
        launches: 0,
        debug_accept: None,
        beat_seq: 0,
        at_breakpoint: false,
    };
    tauri::async_runtime::spawn(async move { actor.run(rx).await });
    SimHandle { tx, debug }
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
                SimMsg::Exited(code) => {
                    self.stop_monitor();
                    let reload = self.guards.reload.take();
                    if let Some(firmware) = reload {
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
                SimMsg::Pause => self.pause(None).await,
                SimMsg::Resume => self.resume().await,
                SimMsg::Usage {
                    cpu_pct,
                    memory,
                    span,
                } => self.on_usage(cpu_pct, memory, span).await,
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
                        }
                        self.status("stopped", "simulation arrêtée".into());
                        return;
                    }
                    if let Some(kill) = self.link.as_mut().and_then(|l| l.kill.take()) {
                        let _ = kill.send(());
                    }
                    self.status("stopped", "simulation arrêtée".into());
                    return;
                }
            }
        }
    }

    // --- Réseau ---------------------------------------------------------------------

    fn build_network(&mut self) {
        let board = self.cfg.board.clone();
        for (name, def) in &board.pins {
            let node = Node::Board(name.clone());
            self.net.add_node(node.clone());
            let pull = match def.external_pull.as_deref() {
                Some("up") => Pull::Up,
                Some("down") => Pull::Down,
                _ => Pull::None,
            };
            if pull != Pull::None {
                self.net.set_pull(&node, Source::Board, pull);
            }
            if let Some(addr) = pin_address(name) {
                self.addr_to_pin.insert(addr, name.clone());
            }
        }
        for (name, volts) in &board.power {
            let drive = if *volts > 0.0 {
                Drive::High
            } else {
                Drive::Low
            };
            self.net
                .set_drive(&Node::Board(name.clone()), Source::Board, drive);
        }
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
        for wire in &self.cfg.project.wires {
            if let (Some(a), Some(b)) = (endpoint_node(&wire.from), endpoint_node(&wire.to)) {
                self.net.connect(a, b);
            }
        }
        // État initial de toutes les broches de composants vers les modèles.
        for node in self.net.resolve_all() {
            self.notify_model(&node);
        }
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
            let level = self.net.level(node);
            self.send_model(ModelCmd::Pin {
                id: id.clone(),
                pin: pin.clone(),
                level,
            });
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

    // --- Interface ----------------------------------------------------------------------

    /// Une image : getState des modèles modifiés au plus une fois par 16 ms.
    fn schedule_ui(&mut self) {
        if !self.states_dirty || self.ui_flush_scheduled {
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
                if let Some(name) = self.pin_name(port, pin) {
                    let fw = self.fw_pins.entry(name.clone()).or_default();
                    fw.mode = mode;
                    fw.pull = pull;
                    let fw = *fw;
                    self.apply_firmware_pin(&name, fw).await;
                }
            }
            msg::GPIO_WRITE => {
                let (port, pin, level) = (r.u8(), r.u8(), r.u8());
                if let Some(name) = self.pin_name(port, pin) {
                    let fw = self.fw_pins.entry(name.clone()).or_default();
                    fw.out = level;
                    let fw = *fw;
                    self.apply_firmware_pin(&name, fw).await;
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
        let mut changed = self.net.set_drive(&node, Source::Firmware, fw.drive());
        changed.extend(self.net.set_pull(&node, Source::Firmware, fw.pull()));
        self.propagate(changed).await;
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
        self.launches = self.launches.wrapping_add(1);
        let address = platform::socket_address(self.cfg.board_id, self.launches);
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
                    let span = now.duration_since(then);
                    let busy = usage.cpu_time.saturating_sub(cpu).as_secs_f64();
                    let wall = span.as_secs_f64();
                    let cpu_pct = if wall > 0.0 {
                        (busy / wall * 100.0) as f32
                    } else {
                        0.0
                    };
                    let m = SimMsg::Usage {
                        cpu_pct,
                        memory: usage.memory,
                        span,
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

    async fn on_usage(&mut self, cpu_pct: f32, memory: u64, span: Duration) {
        if self.guards.paused {
            return;
        }
        self.guards.last_memory = memory;
        let _ = self.app.emit(
            "sim:usage",
            UsageEvent {
                cpu: cpu_pct,
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
        if cpu_pct > CPU_LIMIT_PCT {
            self.guards.busy += span;
            if self.guards.busy >= CPU_LIMIT_TIME {
                let text = format!(
                    "Simulation mise en pause : le firmware a occupé plus de {CPU_LIMIT_PCT:.0} % \
                     d'un cœur pendant {} s. Une boucle sans délai tourne \
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

fn endpoint_node(endpoint: &str) -> Option<Node> {
    let (id, pin) = split_endpoint(endpoint)?;
    Some(if id == "board" {
        Node::Board(pin.to_string())
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

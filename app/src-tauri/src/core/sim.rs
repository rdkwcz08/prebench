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

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use interprocess::local_socket::tokio::prelude::*;
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
}

pub struct SimHandle {
    pub tx: mpsc::Sender<SimMsg>,
}

impl SimHandle {
    pub fn is_alive(&self) -> bool {
        !self.tx.is_closed()
    }

    pub async fn send(&self, m: SimMsg) -> CoreResult<()> {
        self.tx.send(m).await.map_err(|_| CoreError::ActorGone)
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

// --- Liaison avec le processus firmware --------------------------------------------

struct FirmwareLink {
    writer: mpsc::Sender<Vec<u8>>,
    kill: Option<oneshot::Sender<()>>,
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
}

pub fn start<R: Runtime>(app: AppHandle<R>, cfg: SimConfig) -> SimHandle {
    let (tx, rx) = mpsc::channel(ACTOR_QUEUE);
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
    };
    tauri::async_runtime::spawn(async move { actor.run(rx).await });
    SimHandle { tx }
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
                SimMsg::Disconnected => {}
                SimMsg::Exited(code) => {
                    if code == Some(EXIT_RESET) {
                        self.status("starting", "redémarrage du firmware (reset)".into());
                        self.reset_firmware_state();
                        if let Err(e) = self.launch().await {
                            self.status("error", e.to_string());
                            return;
                        }
                    } else {
                        let msg = match code {
                            Some(c) => format!("firmware arrêté (code {c})"),
                            None => "firmware arrêté par un signal (plantage ?)".into(),
                        };
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
                SimMsg::Reset => self.send(msg::RESET, 0, 0, &[]).await,
                SimMsg::Stop => {
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
        let _minor = r.u8();
        let _api = (r.u8(), r.u8());
        let _pid = r.u32();
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
        w.u8(protocol::PROTOCOL_MAJOR)
            .u8(protocol::PROTOCOL_MINOR)
            .u8(self.cfg.board.adc_bits)
            .u8(0)
            .u32(SYNC_TIMEOUT_MS);
        self.send(msg::WELCOME, FLAG_REPLY, h.seq, &w.buf).await;
        self.status(
            "running",
            format!("{} — {framework}, libvmcu {version}", self.cfg.board.name),
        );
    }

    /// Lance le processus firmware et attend sa connexion.
    async fn launch(&mut self) -> CoreResult<()> {
        let address = platform::socket_address(self.cfg.board_id);
        let name = address.as_str().to_fs_name::<GenericFilePath>()?;
        let listener = ListenerOptions::new()
            .name(name)
            .try_overwrite(true)
            .create_tokio()?;

        let mut child = platform::firmware_command(&self.cfg.firmware)
            .env("PREBENCH_SOCKET", &address)
            .env("PREBENCH_BOARD_ID", self.cfg.board_id.to_string())
            .current_dir(&self.cfg.project_dir)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()?;
        platform::after_firmware_spawn(&child);
        if let Some(out) = child.stdout.take() {
            self.forward_lines(out, "stdout");
        }
        if let Some(err) = child.stderr.take() {
            self.forward_lines(err, "stderr");
        }

        let (kill_tx, kill_rx) = oneshot::channel::<()>();
        let tx = self.tx_self.clone();
        tauri::async_runtime::spawn(async move {
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
        tauri::async_runtime::spawn(async move {
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
            kill: Some(kill_tx),
        });
        Ok(())
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

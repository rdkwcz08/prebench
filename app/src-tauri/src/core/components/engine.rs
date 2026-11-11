// app/src-tauri/src/core/components/engine.rs — Exécution des modèles de composants.
//
// Un fil « modèles » par carte ; dans ce fil, un runtime QuickJS par instance
// (tas, ramasse-miettes et limites propres). Le modèle ne voit que les
// fonctions de ComponentContext implémentées ici : ni disque, ni réseau, ni
// système (règle 16.2.8). Interface model.ts inchangée (D5).
//
// Limites par instance : mémoire 16 Mo, pile 256 Ko, 50 ms pour charger et
// initialiser le module, 5 ms par appel (onPinChange, onControl, onTick,
// onPing, getState…). Dépassement ou exception : le composant passe en
// défaut, un avertissement ⚠ est émis, les autres continuent.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError, SyncSender};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use rquickjs::loader::{Loader, Resolver};
use rquickjs::{Context, Ctx, Function, Module, Object, Persistent, Runtime};

use super::transpile::strip_types;
use super::ComponentSource;
use crate::core::net::{Drive, Level, Pull};

pub const MEMORY_LIMIT: usize = 16 * 1024 * 1024;
pub const STACK_LIMIT: usize = 256 * 1024;
pub const CALL_BUDGET: Duration = Duration::from_millis(5);
pub const INIT_BUDGET: Duration = Duration::from_millis(50);
const COMMAND_QUEUE: usize = 4096;

/// Commandes du cœur vers le fil « modèles ».
pub enum ModelCmd {
    Pin {
        id: String,
        pin: String,
        level: Level,
    },
    Control {
        id: String,
        control: String,
        value_json: String,
    },
    Ping {
        id: String,
        seq: u32,
    },
    /// Interrupteur d'alimentation de la paillasse (phase 2, section 10.4).
    #[allow(dead_code)]
    Power(bool),
    FlushStates,
    Stop,
}

/// Événements du fil « modèles » vers le cœur.
#[derive(Debug)]
pub enum ModelEvent {
    Drive {
        id: String,
        pin: String,
        drive: Drive,
    },
    Pull {
        id: String,
        pin: String,
        pull: Pull,
    },
    Pong {
        seq: u32,
        handler_us: u32,
    },
    /// Au moins une instance a appelé invalidate() depuis le dernier FlushStates.
    Dirty,
    /// (identifiant, état JSON) des instances modifiées.
    States(Vec<(String, String)>),
    Warn(String),
    Log(String),
}

pub struct InstanceSpec {
    pub id: String,
    pub source: ComponentSource,
    pub params: serde_json::Map<String, serde_json::Value>,
    pub logic_voltage: f64,
}

pub struct Models {
    tx: SyncSender<ModelCmd>,
}

impl Models {
    /// Envoie une commande ; ne bloque jamais le cœur. File pleine (modèle
    /// bloqué) : la commande est abandonnée et signalée.
    pub fn send(&self, cmd: ModelCmd) -> bool {
        self.tx.try_send(cmd).is_ok()
    }
}

impl Drop for Models {
    fn drop(&mut self) {
        let _ = self.tx.try_send(ModelCmd::Stop);
    }
}

/// Démarre le fil « modèles » d'une carte.
pub fn spawn(
    specs: Vec<InstanceSpec>,
    emit: Box<dyn Fn(ModelEvent) + Send>,
) -> std::io::Result<Models> {
    let (tx, rx) = mpsc::sync_channel(COMMAND_QUEUE);
    std::thread::Builder::new()
        .name("prebench-modeles".into())
        .spawn(move || run(specs, &rx, emit.as_ref()))?;
    Ok(Models { tx })
}

// --- Horloge pour les délais d'exécution ---------------------------------------------

fn now_ns() -> u64 {
    static START: OnceLock<Instant> = OnceLock::new();
    let start = START.get_or_init(Instant::now);
    u64::try_from(start.elapsed().as_nanos()).unwrap_or(u64::MAX)
}

// --- Instance ------------------------------------------------------------------------

#[derive(Default)]
struct HostState {
    pins: HashMap<String, Level>,
    out: Vec<ModelEvent>,
    dirty: bool,
}

struct Instance {
    id: String,
    tick: Option<Duration>,
    next_tick: Instant,
    host: Rc<RefCell<HostState>>,
    deadline: Arc<AtomicU64>,
    // Ordre de libération : fonctions, puis contexte, puis runtime.
    dispatch: Persistent<Function<'static>>,
    state_fn: Persistent<Function<'static>>,
    context: Context,
    _runtime: Runtime,
    faulted: bool,
}

/// Résout les imports relatifs d'un modèle (fichiers .ts / .js du composant).
struct ModelResolver;

impl Resolver for ModelResolver {
    fn resolve<'js>(
        &mut self,
        _ctx: &Ctx<'js>,
        base: &str,
        name: &str,
        _attributes: Option<rquickjs::loader::ImportAttributes<'js>>,
    ) -> rquickjs::Result<String> {
        let path = if name.starts_with('.') {
            Path::new(base).parent().unwrap_or(Path::new("")).join(name)
        } else {
            PathBuf::from(name)
        };
        let candidates = [
            path.clone(),
            path.with_extension("ts"),
            path.with_extension("js"),
        ];
        candidates
            .into_iter()
            .find(|p| p.is_file())
            .map(|p| p.display().to_string())
            .ok_or_else(|| rquickjs::Error::new_resolving(base, name))
    }
}

/// Charge un module en retirant les types (oxc).
/// Chargeur des modèles : lit model.ts et le traduit en JS (oxc). Ce travail
/// du cœur est exclu du budget de chargement du modèle (INIT_BUDGET) : le
/// délai est suspendu pendant la lecture et la traduction, puis repoussé du
/// temps qu'elles ont pris. Sans cela, un build de debug sur une machine
/// chargée dépassait les 50 ms avant même d'exécuter le modèle.
struct ModelLoader {
    deadline: Arc<AtomicU64>,
}

impl Loader for ModelLoader {
    fn load<'js>(
        &mut self,
        ctx: &Ctx<'js>,
        name: &str,
        _attributes: Option<rquickjs::loader::ImportAttributes<'js>>,
    ) -> rquickjs::Result<Module<'js, rquickjs::module::Declared>> {
        let path = Path::new(name);
        let saved = self.deadline.swap(u64::MAX, Ordering::Relaxed);
        let t0 = now_ns();
        let js = std::fs::read_to_string(path)
            .map_err(|e| rquickjs::Error::new_loading_message(name, e.to_string()))
            .and_then(|source| {
                strip_types(path, &source)
                    .map_err(|e| rquickjs::Error::new_loading_message(name, e))
            });
        if saved != u64::MAX {
            let spent = now_ns().saturating_sub(t0);
            self.deadline
                .store(saved.saturating_add(spent), Ordering::Relaxed);
        }
        Module::declare(ctx.clone(), name, js?)
    }
}

/// Module JS qui construit le contexte du modèle et expose deux points d'entrée.
fn glue_source(spec: &InstanceSpec) -> String {
    let model =
        serde_json::to_string(&spec.source.model_path().display().to_string()).unwrap_or_default();
    let id = serde_json::to_string(&spec.id).unwrap_or_default();
    format!(
        r#"import create from {model};
const h = globalThis.__host;
const ctx = Object.freeze({{
  apiVersion: 1,
  id: {id},
  params: Object.freeze(JSON.parse(h.params())),
  logicVoltage: {volts},
  pin: (name) => ({{ level: h.pinLevel(String(name)) }}),
  drive: (pin, level) => h.drive(String(pin), String(level)),
  pull: (pin, kind) => h.pull(String(pin), String(kind)),
  setVoltage: () => h.warn("sortie analogique non simulée (phase 2)"),
  uartWrite: () => h.warn("UART non simulé (phase 2)"),
  invalidate: () => h.invalidate(),
  warn: (m) => h.warn(String(m)),
  log: (m) => h.log(String(m)),
  now: () => h.now(),
}});
const model = create(ctx);
export function dispatch(kind, a, b) {{
  switch (kind) {{
    case "pin": model.onPinChange?.(a, {{ level: b }}); break;
    case "control": model.onControl?.(a, JSON.parse(b)); break;
    case "tick": model.onTick?.(Number(a)); break;
    case "ping": model.onPing?.(); break;
    case "power": model.onPower?.(a === "1"); break;
  }}
}}
export function state() {{ return JSON.stringify(model.getState()); }}
"#,
        volts = spec.logic_voltage
    )
}

/// Message d'une exception JavaScript (ou d'une interruption).
fn describe(ctx: &Ctx<'_>, error: rquickjs::Error) -> String {
    if !error.is_exception() {
        return error.to_string();
    }
    let value = ctx.catch();
    if let Some(exception) = value.as_exception() {
        let message = exception.message().unwrap_or_default();
        if message.contains("interrupted") {
            return "temps d'exécution dépassé".into();
        }
        return message;
    }
    value
        .as_string()
        .and_then(|s| s.to_string().ok())
        .unwrap_or_else(|| "exception JavaScript".into())
}

fn install_host(
    ctx: &Ctx<'_>,
    host: &Rc<RefCell<HostState>>,
    id: &str,
    params_json: String,
) -> rquickjs::Result<()> {
    let obj = Object::new(ctx.clone())?;

    let h = Rc::clone(host);
    obj.set(
        "pinLevel",
        Function::new(ctx.clone(), move |name: String| {
            level_name(
                h.borrow()
                    .pins
                    .get(&name)
                    .copied()
                    .unwrap_or(Level::Floating),
            )
        })?,
    )?;

    let (h, owner) = (Rc::clone(host), id.to_string());
    obj.set(
        "drive",
        Function::new(ctx.clone(), move |pin: String, level: String| {
            let drive = match level.as_str() {
                "high" => Drive::High,
                "low" => Drive::Low,
                _ => Drive::Z,
            };
            h.borrow_mut().out.push(ModelEvent::Drive {
                id: owner.clone(),
                pin,
                drive,
            });
        })?,
    )?;

    let (h, owner) = (Rc::clone(host), id.to_string());
    obj.set(
        "pull",
        Function::new(ctx.clone(), move |pin: String, kind: String| {
            let pull = match kind.as_str() {
                "up" => Pull::Up,
                "down" => Pull::Down,
                _ => Pull::None,
            };
            h.borrow_mut().out.push(ModelEvent::Pull {
                id: owner.clone(),
                pin,
                pull,
            });
        })?,
    )?;

    let h = Rc::clone(host);
    obj.set(
        "invalidate",
        Function::new(ctx.clone(), move || h.borrow_mut().dirty = true)?,
    )?;

    let (h, owner) = (Rc::clone(host), id.to_string());
    obj.set(
        "warn",
        Function::new(ctx.clone(), move |m: String| {
            h.borrow_mut()
                .out
                .push(ModelEvent::Warn(format!("{owner} : {m}")))
        })?,
    )?;

    let (h, owner) = (Rc::clone(host), id.to_string());
    obj.set(
        "log",
        Function::new(ctx.clone(), move |m: String| {
            h.borrow_mut()
                .out
                .push(ModelEvent::Log(format!("{owner} : {m}")))
        })?,
    )?;

    obj.set(
        "now",
        Function::new(ctx.clone(), || now_ns() as f64 / 1000.0)?,
    )?;
    obj.set(
        "params",
        Function::new(ctx.clone(), move || params_json.clone())?,
    )?;
    ctx.globals().set("__host", obj)
}

pub fn level_name(level: Level) -> &'static str {
    match level {
        Level::High => "high",
        Level::Low => "low",
        Level::PulledHigh => "pulled-high",
        Level::PulledLow => "pulled-low",
        Level::Floating => "floating",
        Level::Conflict => "conflict",
    }
}

impl Instance {
    fn load(spec: &InstanceSpec) -> Result<Self, String> {
        let runtime = Runtime::new().map_err(|e| e.to_string())?;
        runtime.set_memory_limit(MEMORY_LIMIT);
        runtime.set_max_stack_size(STACK_LIMIT);
        let deadline = Arc::new(AtomicU64::new(u64::MAX));
        runtime.set_loader(
            ModelResolver,
            ModelLoader {
                deadline: Arc::clone(&deadline),
            },
        );
        let d = Arc::clone(&deadline);
        runtime.set_interrupt_handler(Some(Box::new(move || now_ns() > d.load(Ordering::Relaxed))));
        let context = Context::full(&runtime).map_err(|e| e.to_string())?;
        let host = Rc::new(RefCell::new(HostState::default()));
        let params_json = serde_json::Value::Object(spec.params.clone()).to_string();

        deadline.store(now_ns() + INIT_BUDGET.as_nanos() as u64, Ordering::Relaxed);
        let loaded = context.with(|ctx| {
            install_host(&ctx, &host, &spec.id, params_json).map_err(|e| describe(&ctx, e))?;
            let name = format!("prebench:instance:{}", spec.id);
            let (module, promise) = Module::declare(ctx.clone(), name, glue_source(spec))
                .and_then(|m| m.eval())
                .map_err(|e| describe(&ctx, e))?;
            promise.finish::<()>().map_err(|e| describe(&ctx, e))?;
            let dispatch: Function = module.get("dispatch").map_err(|e| describe(&ctx, e))?;
            let state: Function = module.get("state").map_err(|e| describe(&ctx, e))?;
            Ok::<_, String>((
                Persistent::save(&ctx, dispatch),
                Persistent::save(&ctx, state),
            ))
        });
        deadline.store(u64::MAX, Ordering::Relaxed);
        let (dispatch, state_fn) = loaded?;

        let tick = match spec.source.tick_hz() {
            hz if hz > 0.0 => Some(Duration::from_secs_f64(1.0 / hz.min(60.0))),
            _ => None,
        };
        Ok(Instance {
            id: spec.id.clone(),
            tick,
            next_tick: Instant::now(),
            host,
            deadline,
            dispatch,
            state_fn,
            context,
            _runtime: runtime,
            faulted: false,
        })
    }

    /// Appelle dispatch(kind, a, b) dans le budget de temps.
    fn call(&mut self, kind: &str, a: &str, b: &str) -> Result<(), String> {
        if self.faulted {
            return Ok(());
        }
        self.deadline
            .store(now_ns() + CALL_BUDGET.as_nanos() as u64, Ordering::Relaxed);
        let result = self.context.with(|ctx| {
            let f = self
                .dispatch
                .clone()
                .restore(&ctx)
                .map_err(|e| describe(&ctx, e))?;
            f.call::<_, ()>((kind, a, b)).map_err(|e| describe(&ctx, e))
        });
        self.deadline.store(u64::MAX, Ordering::Relaxed);
        result.map_err(|e| self.fault(e))
    }

    fn state(&mut self) -> Result<String, String> {
        self.deadline
            .store(now_ns() + CALL_BUDGET.as_nanos() as u64, Ordering::Relaxed);
        let result = self.context.with(|ctx| {
            let f = self
                .state_fn
                .clone()
                .restore(&ctx)
                .map_err(|e| describe(&ctx, e))?;
            f.call::<_, String>(()).map_err(|e| describe(&ctx, e))
        });
        self.deadline.store(u64::MAX, Ordering::Relaxed);
        result.map_err(|e| self.fault(e))
    }

    fn fault(&mut self, error: String) -> String {
        self.faulted = true;
        format!("composant {} arrêté : {error}", self.id)
    }
}

// --- Boucle du fil « modèles » ------------------------------------------------------------

fn run(specs: Vec<InstanceSpec>, rx: &mpsc::Receiver<ModelCmd>, emit: &dyn Fn(ModelEvent)) {
    let mut instances: Vec<Instance> = Vec::new();
    for spec in &specs {
        match Instance::load(spec) {
            Ok(inst) => instances.push(inst),
            Err(e) => emit(ModelEvent::Warn(format!(
                "composant {} non chargé : {e}",
                spec.id
            ))),
        }
    }
    let mut dirty_notified = false;
    // État initial de chaque vue.
    for inst in &instances {
        inst.host.borrow_mut().dirty = true;
    }
    flush_events(&mut instances, &mut dirty_notified, emit);

    loop {
        let next_tick = instances
            .iter()
            .filter(|i| i.tick.is_some() && !i.faulted)
            .map(|i| i.next_tick)
            .min();
        let cmd = match next_tick {
            Some(at) => match rx.recv_timeout(at.saturating_duration_since(Instant::now())) {
                Ok(cmd) => Some(cmd),
                Err(RecvTimeoutError::Timeout) => None,
                Err(RecvTimeoutError::Disconnected) => return,
            },
            None => match rx.recv() {
                Ok(cmd) => Some(cmd),
                Err(_) => return,
            },
        };
        match cmd {
            None => run_ticks(&mut instances, emit),
            Some(ModelCmd::Stop) => return,
            Some(ModelCmd::Pin { id, pin, level }) => {
                if let Some(inst) = instances.iter_mut().find(|i| i.id == id) {
                    inst.host.borrow_mut().pins.insert(pin.clone(), level);
                    report(inst.call("pin", &pin, level_name(level)), emit);
                }
            }
            Some(ModelCmd::Control {
                id,
                control,
                value_json,
            }) => {
                if let Some(inst) = instances.iter_mut().find(|i| i.id == id) {
                    report(inst.call("control", &control, &value_json), emit);
                }
            }
            Some(ModelCmd::Ping { id, seq }) => {
                let t0 = Instant::now();
                if let Some(inst) = instances.iter_mut().find(|i| i.id == id) {
                    report(inst.call("ping", "", ""), emit);
                }
                let handler_us = u32::try_from(t0.elapsed().as_micros()).unwrap_or(u32::MAX);
                emit(ModelEvent::Pong { seq, handler_us });
            }
            Some(ModelCmd::Power(on)) => {
                for inst in &mut instances {
                    report(inst.call("power", if on { "1" } else { "0" }, ""), emit);
                }
            }
            Some(ModelCmd::FlushStates) => {
                dirty_notified = false;
                let mut states = Vec::new();
                for inst in &mut instances {
                    let was_dirty = std::mem::take(&mut inst.host.borrow_mut().dirty);
                    if was_dirty && !inst.faulted {
                        match inst.state() {
                            Ok(json) => states.push((inst.id.clone(), json)),
                            Err(e) => emit(ModelEvent::Warn(e)),
                        }
                    }
                }
                if !states.is_empty() {
                    emit(ModelEvent::States(states));
                }
            }
        }
        flush_events(&mut instances, &mut dirty_notified, emit);
    }
}

fn run_ticks(instances: &mut [Instance], emit: &dyn Fn(ModelEvent)) {
    let now = Instant::now();
    for inst in instances.iter_mut() {
        if let Some(period) = inst.tick {
            if !inst.faulted && inst.next_tick <= now {
                inst.next_tick = now + period;
                let dt = format!("{}", period.as_secs_f64() * 1000.0);
                report(inst.call("tick", &dt, ""), emit);
            }
        }
    }
}

fn report(result: Result<(), String>, emit: &dyn Fn(ModelEvent)) {
    if let Err(e) = result {
        emit(ModelEvent::Warn(e));
    }
}

/// Transmet les actions des modèles (drive, pull, journal) et signale une
/// seule fois qu'un état est à redessiner.
fn flush_events(instances: &mut [Instance], dirty_notified: &mut bool, emit: &dyn Fn(ModelEvent)) {
    let mut any_dirty = false;
    for inst in instances.iter_mut() {
        let mut host = inst.host.borrow_mut();
        for event in host.out.drain(..) {
            emit(event);
        }
        any_dirty |= host.dirty;
    }
    if any_dirty && !*dirty_notified {
        *dirty_notified = true;
        emit(ModelEvent::Dirty);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::components::{Catalog, ComponentSource, Origin};

    fn provided(id: &str) -> ComponentSource {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../components");
        let catalog = Catalog::load(&dir, Path::new("/inexistant"));
        catalog.components[id].clone()
    }

    fn start(specs: Vec<InstanceSpec>) -> (Models, mpsc::Receiver<ModelEvent>) {
        let (tx, rx) = mpsc::channel();
        let models = spawn(
            specs,
            Box::new(move |e| {
                let _ = tx.send(e);
            }),
        )
        .expect("fil modèles");
        (models, rx)
    }

    fn spec(id: &str, source: ComponentSource) -> InstanceSpec {
        let params = source.params_with_defaults(&Default::default());
        InstanceSpec {
            id: id.into(),
            source,
            params,
            logic_voltage: 3.3,
        }
    }

    fn states(models: &Models, rx: &mpsc::Receiver<ModelEvent>) -> Vec<(String, String)> {
        assert!(models.send(ModelCmd::FlushStates));
        loop {
            match rx.recv_timeout(Duration::from_secs(5)).expect("événement") {
                ModelEvent::States(s) => return s,
                ModelEvent::Warn(w) => panic!("avertissement inattendu : {w}"),
                _ => {}
            }
        }
    }

    #[test]
    fn led_model_runs_in_quickjs() {
        let (models, rx) = start(vec![spec("LD2", provided("led"))]);
        let initial = states(&models, &rx);
        assert_eq!(initial, vec![("LD2".into(), r#"{"brightness":0}"#.into())]);
        models.send(ModelCmd::Pin {
            id: "LD2".into(),
            pin: "K".into(),
            level: Level::Low,
        });
        models.send(ModelCmd::Pin {
            id: "LD2".into(),
            pin: "A".into(),
            level: Level::High,
        });
        assert_eq!(
            states(&models, &rx),
            vec![("LD2".into(), r#"{"brightness":1}"#.into())]
        );
    }

    #[test]
    fn button_drives_its_pin() {
        let (models, rx) = start(vec![spec("B1", provided("button"))]);
        let _ = states(&models, &rx);
        models.send(ModelCmd::Pin {
            id: "B1".into(),
            pin: "2".into(),
            level: Level::Low,
        });
        models.send(ModelCmd::Control {
            id: "B1".into(),
            control: "press".into(),
            value_json: "true".into(),
        });
        let drive = loop {
            if let ModelEvent::Drive { pin, drive, .. } =
                rx.recv_timeout(Duration::from_secs(5)).expect("drive")
            {
                break (pin, drive);
            }
        };
        assert_eq!(drive, ("1".to_string(), Drive::Low));
    }

    fn temp_component(name: &str, model: &str) -> ComponentSource {
        let dir =
            std::env::temp_dir().join(format!("prebench-model-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("dossier");
        std::fs::write(dir.join("model.ts"), model).expect("model.ts");
        ComponentSource {
            id: name.into(),
            origin: Origin::Project,
            dir,
            manifest: serde_json::json!({ "id": name, "api_version": 1 }),
        }
    }

    #[test]
    fn infinite_loop_is_interrupted() {
        let source = temp_component(
            "boucle",
            "export default (ctx: unknown) => ({ onControl(): void { for (;;) {} }, getState: () => ({}) });",
        );
        let (models, rx) = start(vec![spec("x", source)]);
        let _ = rx.recv_timeout(Duration::from_secs(5));
        let t0 = Instant::now();
        models.send(ModelCmd::Control {
            id: "x".into(),
            control: "c".into(),
            value_json: "1".into(),
        });
        let warn = loop {
            if let ModelEvent::Warn(w) = rx
                .recv_timeout(Duration::from_secs(5))
                .expect("avertissement")
            {
                break w;
            }
        };
        assert!(warn.contains("temps d'exécution dépassé"), "{warn}");
        assert!(
            t0.elapsed() < Duration::from_millis(200),
            "interrompu trop tard : {:?}",
            t0.elapsed()
        );
    }

    #[test]
    fn memory_limit_is_enforced() {
        let source = temp_component(
            "memoire",
            "export default () => ({ onControl(): void { const a: string[] = []; for (;;) a.push('x'.repeat(1 << 20)); }, getState: () => ({}) });",
        );
        let (models, rx) = start(vec![spec("m", source)]);
        let _ = rx.recv_timeout(Duration::from_secs(5));
        models.send(ModelCmd::Control {
            id: "m".into(),
            control: "c".into(),
            value_json: "1".into(),
        });
        let warn = loop {
            if let ModelEvent::Warn(w) = rx
                .recv_timeout(Duration::from_secs(5))
                .expect("avertissement")
            {
                break w;
            }
        };
        assert!(warn.contains("composant m arrêté"), "{warn}");
    }
}

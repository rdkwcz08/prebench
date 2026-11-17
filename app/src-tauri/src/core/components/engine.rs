// app/src-tauri/src/core/components/engine.rs — Exécution des modèles de composants.
//
// Un fil « modèles » par carte ; dans ce fil, un runtime QuickJS par instance
// (tas, ramasse-miettes et limites propres). Le modèle ne voit que les
// fonctions de ComponentContext implémentées ici : ni disque, ni réseau, ni
// système (règle 16.2.8). Interface model.ts inchangée (D5).
//
// Limites par instance : mémoire 16 Mo, pile 256 Ko, 50 ms pour charger et
// initialiser le module ; par appel (onPinChange, onControl, onTick, onI2C,
// getState…), budget de 5 ms et plafond dur de 50 ms. Ces durées sont du
// temps CPU consommé par le fil « modèles » (platform::thread_cpu_ns), pas du
// temps écoulé : une machine chargée qui fait attendre le fil n'arrête pas un
// composant à tort.
// Un dépassement isolé du budget (ramasse-miettes, pic de charge) n'est
// qu'écrit au Journal ; trois dépassements consécutifs, une exception, ou un
// appel interrompu au plafond dur (boucle infinie) mettent le composant en
// défaut : un avertissement ⚠ est émis, les autres continuent.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError, SyncSender};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use rquickjs::loader::{Loader, Resolver};
use rquickjs::{
    Context, Ctx, Exception, Function, Module, Object, Persistent, Runtime, TypedArray,
};

use super::display::{self, DisplayFormat, Framebuffer};
use super::js_bytes::{self, Bytes};
use super::transpile::strip_types;
use super::ComponentSource;
use crate::core::net::{Drive, Level, NetState, Pull};

pub const MEMORY_LIMIT: usize = 16 * 1024 * 1024;
pub const STACK_LIMIT: usize = 256 * 1024;
pub const CALL_BUDGET: Duration = Duration::from_millis(5);
/// Plafond dur d'un appel : interruption immédiate (boucle infinie).
pub const CALL_HARD_LIMIT: Duration = Duration::from_millis(50);
/// Dépassements consécutifs du budget qui mettent un composant en défaut.
pub const MAX_OVERRUNS: u32 = 3;
pub const INIT_BUDGET: Duration = Duration::from_millis(50);
const COMMAND_QUEUE: usize = 4096;

/// Commandes du cœur vers le fil « modèles ».
pub enum ModelCmd {
    Pin {
        id: String,
        pin: String,
        state: NetState,
    },
    /// Paramètres modifiés dans l'inspecteur (objet JSON complet).
    Params {
        id: String,
        params_json: String,
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
    /// Transaction I2C adressée au composant `id` (onI2C), réponse I2cReply.
    I2c {
        id: String,
        seq: u32,
        address: u8,
        tx: Vec<u8>,
        rx_len: usize,
    },
    /// Transaction SPI pendant que le composant `id` est sélectionné (onSPI) :
    /// `tx` vide = le firmware ne fait que lire (le modèle voit `len` octets
    /// 0xFF). Avec `seq`, le firmware attend SpiReply (exactement `len` octets).
    Spi {
        id: String,
        seq: Option<u32>,
        tx: Vec<u8>,
        len: usize,
    },
    /// Trame d'un ruban de LED adressables sur l'entrée `iface` du composant
    /// `id` (onLedStrip), octets dans l'ordre du fil.
    LedStrip {
        id: String,
        iface: String,
        data: Vec<u8>,
    },
    /// Interrupteur d'alimentation de la paillasse (phase 2, section 10.4).
    Power(bool),
    /// Grandeurs du panneau environnement (JSON d'Environment), à tous les
    /// modèles : au démarrage, puis à chaque changement (onEnvironment).
    Environment(String),
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
    /// Tension fixée par setVoltage ; None : retirée.
    Volts {
        id: String,
        pin: String,
        volts: Option<f64>,
    },
    Pong {
        seq: u32,
        handler_us: u32,
    },
    /// Réponse à ModelCmd::I2c : statut I2C_REPLY (0 ACK, 1 NACK adresse,
    /// 2 NACK donnée) et exactement rx_len octets si ACK.
    I2cReply {
        seq: u32,
        status: u8,
        rx: Vec<u8>,
    },
    /// Réponse à ModelCmd::Spi avec seq : exactement `len` octets.
    SpiReply {
        seq: u32,
        rx: Vec<u8>,
    },
    /// Au moins une instance a appelé invalidate() depuis le dernier FlushStates.
    Dirty,
    /// Zones modifiées des écrans (message « displays », display.rs), juste
    /// avant States, au plus une fois par image.
    Displays(Vec<u8>),
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

/// Démarre le fil « modèles » d'une carte. Les événements partent au cœur par
/// lots, dans l'ordre (`emit` reçoit un lot non vide) : un réveil du cœur par
/// lot plutôt qu'un par événement.
pub fn spawn(
    specs: Vec<InstanceSpec>,
    emit: Box<dyn Fn(Vec<ModelEvent>) + Send>,
) -> std::io::Result<Models> {
    let (tx, rx) = mpsc::sync_channel(COMMAND_QUEUE);
    std::thread::Builder::new()
        .name("prebench-modeles".into())
        .spawn(move || {
            let batch = RefCell::new(Vec::new());
            let push = |e: ModelEvent| batch.borrow_mut().push(e);
            let send = || {
                let events = std::mem::take(&mut *batch.borrow_mut());
                if !events.is_empty() {
                    emit(events);
                }
            };
            run(specs, &rx, &push, &send);
            send();
        })?;
    Ok(Models { tx })
}

/// Commandes traitées au plus avant d'envoyer le lot d'événements (latence bornée).
const BATCH_COMMANDS: u32 = 64;

// --- Horloges ----------------------------------------------------------------------

/// Temps écoulé (ctx.now() des modèles).
fn now_ns() -> u64 {
    static START: OnceLock<Instant> = OnceLock::new();
    let start = START.get_or_init(Instant::now);
    u64::try_from(start.elapsed().as_nanos()).unwrap_or(u64::MAX)
}

/// Temps CPU du fil « modèles » : budgets de chargement et d'appel.
fn cpu_ns() -> u64 {
    crate::platform::thread_cpu_ns()
}

// --- Instance ------------------------------------------------------------------------

#[derive(Default)]
struct HostState {
    pins: HashMap<String, NetState>,
    out: Vec<ModelEvent>,
    /// getState à rappeler (invalidate).
    dirty: bool,
    /// Écran du composant (ctx.createDisplay), hors de la mémoire de QuickJS.
    display: Option<Framebuffer>,
    /// Pixels modifiés depuis la dernière image (blit) : image à envoyer,
    /// sans rappeler getState.
    display_dirty: bool,
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
    i2c_fn: Persistent<Function<'static>>,
    spi_fn: Persistent<Function<'static>>,
    strip_fn: Persistent<Function<'static>>,
    context: Context,
    _runtime: Runtime,
    faulted: bool,
    /// Dépassements consécutifs du budget d'appel (CALL_BUDGET).
    overruns: u32,
    /// Réponse I2C de mauvaise longueur déjà signalée (une fois par instance).
    rx_len_warned: bool,
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
        let t0 = cpu_ns();
        let js = std::fs::read_to_string(path)
            .map_err(|e| rquickjs::Error::new_loading_message(name, e.to_string()))
            .and_then(|source| {
                strip_types(path, &source)
                    .map_err(|e| rquickjs::Error::new_loading_message(name, e))
            });
        if saved != u64::MAX {
            let spent = cpu_ns().saturating_sub(t0);
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
let params = Object.freeze(JSON.parse(h.params()));
const ctx = Object.freeze({{
  apiVersion: 1,
  id: {id},
  logicVoltage: {volts},
  get params() {{ return params; }},
  pin: (name) => JSON.parse(h.pinState(String(name))),
  drive: (pin, level) => h.drive(String(pin), String(level)),
  pull: (pin, kind) => h.pull(String(pin), String(kind)),
  setVoltage: (pin, volts) => h.volts(String(pin), Number(volts)),
  uartWrite: () => h.warn("émission UART d'un composant pas encore simulée"),
  invalidate: () => h.invalidate(),
  createDisplay: (o) => {{
    const width = Number(o.width), height = Number(o.height), format = String(o.format);
    h.createDisplay(width, height, format);
    return Object.freeze({{
      width, height, format,
      blit: (x, y, w, hh, data) => h.blit(Number(x), Number(y), Number(w), Number(hh), data),
      setScan: (sc) => h.setScan(!!sc.swapXY, !!sc.mirrorX, !!sc.mirrorY, !!sc.bgr),
    }});
  }},
  warn: (m) => h.warn(String(m)),
  log: (m) => h.log(String(m)),
  now: () => h.now(),
}});
const model = create(ctx);
export function dispatch(kind, a, b) {{
  switch (kind) {{
    case "pin": model.onPinChange?.(a, JSON.parse(b)); break;
    case "params": params = Object.freeze(JSON.parse(a)); model.onParamsChange?.(params); break;
    case "control": model.onControl?.(a, JSON.parse(b)); break;
    case "tick": model.onTick?.(Number(a)); break;
    case "ping": model.onPing?.(); break;
    case "power": model.onPower?.(a === "1"); break;
    case "env": model.onEnvironment?.(Object.freeze(JSON.parse(a))); break;
  }}
}}
export function state() {{ return JSON.stringify(model.getState()); }}
export function i2c(address, tx, rxLength) {{
  if (typeof model.onI2C !== "function") return {{ s: 1 }};
  const r = model.onI2C(Object.freeze({{ address, tx, rxLength }}));
  if (r && r.ack === true) {{
    const rx = r.rx === undefined ? new Uint8Array(0)
      : r.rx instanceof Uint8Array ? r.rx : Uint8Array.from(r.rx);
    return {{ s: 0, rx }};
  }}
  return {{ s: r && r.nack === "data" ? 2 : 1 }};
}}
export function ledStrip(data, iface) {{
  model.onLedStrip?.(data, iface);
}}
export function spi(tx, wantsRx) {{
  if (typeof model.onSPI !== "function") return undefined;
  const r = model.onSPI(Object.freeze({{ tx, wantsRx }}));
  if (!wantsRx || r === undefined || r === null) return undefined;
  return r instanceof Uint8Array ? r : Uint8Array.from(r);
}}
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
            return "temps d'exécution dépassé (plafond de 50 ms par appel)".into();
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
        "pinState",
        Function::new(ctx.clone(), move |name: String| {
            pin_state_json(h.borrow().pins.get(&name).copied().unwrap_or_default())
        })?,
    )?;

    let (h, owner) = (Rc::clone(host), id.to_string());
    obj.set(
        "volts",
        Function::new(ctx.clone(), move |pin: String, volts: f64| {
            h.borrow_mut().out.push(ModelEvent::Volts {
                id: owner.clone(),
                pin,
                volts: volts.is_finite().then_some(volts),
            });
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

    let h = Rc::clone(host);
    obj.set(
        "createDisplay",
        Function::new(
            ctx.clone(),
            move |ctx: Ctx<'_>, width: f64, height: f64, format: String| -> rquickjs::Result<()> {
                let fb = DisplayFormat::parse(&format)
                    .ok_or(display::DisplayError::UnknownFormat(format))
                    .and_then(|f| Framebuffer::new(side(width), side(height), f))
                    .map_err(|e| Exception::throw_message(&ctx, &e.to_string()))?;
                let mut host = h.borrow_mut();
                host.display = Some(fb);
                host.display_dirty = true;
                Ok(())
            },
        )?,
    )?;

    let h = Rc::clone(host);
    obj.set(
        "blit",
        Function::new(
            ctx.clone(),
            move |ctx: Ctx<'_>,
                  x: f64,
                  y: f64,
                  w: f64,
                  hh: f64,
                  data: TypedArray<'_, u8>|
                  -> rquickjs::Result<()> {
                let throw = |m: String| Exception::throw_message(&ctx, &m);
                let mut host = h.borrow_mut();
                let fb = host
                    .display
                    .as_mut()
                    .ok_or_else(|| throw("blit avant ctx.createDisplay".into()))?;
                let (x, y, w, hh) = (x.trunc() as i64, y.trunc() as i64, side(w), side(hh));
                let need = fb.blit_len(y, w, hh).map_err(|e| throw(e.to_string()))?;
                let bytes = js_bytes::copy(&data, need).map_err(|e| throw(e.to_string()))?;
                fb.blit(x, y, w, hh, &bytes.data)
                    .map_err(|e| throw(e.to_string()))?;
                host.display_dirty = true;
                Ok(())
            },
        )?,
    )?;

    let h = Rc::clone(host);
    obj.set(
        "setScan",
        Function::new(
            ctx.clone(),
            move |ctx: Ctx<'_>,
                  swap_xy: bool,
                  mirror_x: bool,
                  mirror_y: bool,
                  bgr: bool|
                  -> rquickjs::Result<()> {
                let throw = |m: String| Exception::throw_message(&ctx, &m);
                let mut host = h.borrow_mut();
                let fb = host
                    .display
                    .as_mut()
                    .ok_or_else(|| throw("setScan avant ctx.createDisplay".into()))?;
                fb.set_scan(display::Scan {
                    swap_xy,
                    mirror_x,
                    mirror_y,
                    bgr,
                })
                .map_err(|e| throw(e.to_string()))
            },
        )?,
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

/// Côté d'écran ou de rectangle venu de JavaScript : entier positif (NaN,
/// négatif ou trop grand : 0 ou u32::MAX, refusés ensuite par display.rs).
fn side(v: f64) -> u32 {
    if v.is_finite() && v > 0.0 {
        v.min(f64::from(u32::MAX)) as u32
    } else {
        0
    }
}

/// PinState de component-api.d.ts, en JSON : niveau, PWM et tension connue.
pub fn pin_state_json(state: NetState) -> String {
    let mut v = serde_json::json!({ "level": level_name(state.level) });
    if let Some(p) = state.pwm {
        v["pwm"] = serde_json::json!({ "freqHz": p.freq_hz, "duty": p.ratio() });
    }
    if let Some(volts) = state.volts {
        v["volts"] = serde_json::json!(volts);
    }
    v.to_string()
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
        runtime.set_interrupt_handler(Some(Box::new(move || cpu_ns() > d.load(Ordering::Relaxed))));
        let context = Context::full(&runtime).map_err(|e| e.to_string())?;
        let host = Rc::new(RefCell::new(HostState::default()));
        let params_json = serde_json::Value::Object(spec.params.clone()).to_string();

        deadline.store(cpu_ns() + INIT_BUDGET.as_nanos() as u64, Ordering::Relaxed);
        let loaded = context.with(|ctx| {
            install_host(&ctx, &host, &spec.id, params_json).map_err(|e| describe(&ctx, e))?;
            let name = format!("prebench:instance:{}", spec.id);
            let (module, promise) = Module::declare(ctx.clone(), name, glue_source(spec))
                .and_then(|m| m.eval())
                .map_err(|e| describe(&ctx, e))?;
            promise.finish::<()>().map_err(|e| describe(&ctx, e))?;
            let dispatch: Function = module.get("dispatch").map_err(|e| describe(&ctx, e))?;
            let state: Function = module.get("state").map_err(|e| describe(&ctx, e))?;
            let i2c: Function = module.get("i2c").map_err(|e| describe(&ctx, e))?;
            let spi: Function = module.get("spi").map_err(|e| describe(&ctx, e))?;
            let strip: Function = module.get("ledStrip").map_err(|e| describe(&ctx, e))?;
            Ok::<_, String>((
                Persistent::save(&ctx, dispatch),
                Persistent::save(&ctx, state),
                Persistent::save(&ctx, i2c),
                Persistent::save(&ctx, spi),
                Persistent::save(&ctx, strip),
            ))
        });
        deadline.store(u64::MAX, Ordering::Relaxed);
        let (dispatch, state_fn, i2c_fn, spi_fn, strip_fn) = loaded?;

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
            i2c_fn,
            spi_fn,
            strip_fn,
            context,
            _runtime: runtime,
            faulted: false,
            overruns: 0,
            rx_len_warned: false,
        })
    }

    /// Début d'un appel : échéance au plafond dur ; renvoie le temps CPU de départ.
    fn begin(&self) -> u64 {
        let t0 = cpu_ns();
        self.deadline
            .store(t0 + CALL_HARD_LIMIT.as_nanos() as u64, Ordering::Relaxed);
        t0
    }

    /// Fin d'un appel terminé : compte les dépassements consécutifs du budget.
    fn end(&mut self, t0: u64, what: &str) -> Result<(), String> {
        self.deadline.store(u64::MAX, Ordering::Relaxed);
        let spent = Duration::from_nanos(cpu_ns().saturating_sub(t0));
        if spent <= CALL_BUDGET {
            self.overruns = 0;
            return Ok(());
        }
        self.overruns += 1;
        let ms = spent.as_secs_f64() * 1000.0;
        if self.overruns >= MAX_OVERRUNS {
            return Err(self.fault(format!(
                "{MAX_OVERRUNS} appels de suite au-delà de 5 ms (dernier : {what}, {ms:.1} ms)"
            )));
        }
        self.host.borrow_mut().out.push(ModelEvent::Log(format!(
            "{} : {what} a pris {ms:.1} ms (budget 5 ms, dépassement {}/{MAX_OVERRUNS})",
            self.id, self.overruns
        )));
        Ok(())
    }

    /// Appelle dispatch(kind, a, b) dans le budget de temps.
    fn call(&mut self, kind: &str, a: &str, b: &str) -> Result<(), String> {
        if self.faulted {
            return Ok(());
        }
        let t0 = self.begin();
        let result = self.context.with(|ctx| {
            let f = self
                .dispatch
                .clone()
                .restore(&ctx)
                .map_err(|e| describe(&ctx, e))?;
            f.call::<_, ()>((kind, a, b)).map_err(|e| describe(&ctx, e))
        });
        self.deadline.store(u64::MAX, Ordering::Relaxed);
        result.map_err(|e| self.fault(e))?;
        let what = match kind {
            "pin" => "onPinChange",
            "params" => "onParamsChange",
            "control" => "onControl",
            "tick" => "onTick",
            "ping" => "onPing",
            "power" => "onPower",
            "env" => "onEnvironment",
            other => other,
        };
        self.end(t0, what)
    }

    fn state(&mut self) -> Result<String, String> {
        let t0 = self.begin();
        let result = self.context.with(|ctx| {
            let f = self
                .state_fn
                .clone()
                .restore(&ctx)
                .map_err(|e| describe(&ctx, e))?;
            f.call::<_, String>(()).map_err(|e| describe(&ctx, e))
        });
        self.deadline.store(u64::MAX, Ordering::Relaxed);
        let json = result.map_err(|e| self.fault(e))?;
        self.end(t0, "getState")?;
        Ok(json)
    }

    /// Transaction I2C : (statut, octets lus, au plus rx_len, et longueur
    /// rendue par le modèle). Un composant en défaut ne répond plus, comme une
    /// puce absente (NACK adresse).
    fn i2c(&mut self, address: u8, tx: &[u8], rx_len: usize) -> Result<(u8, Bytes), String> {
        let none = || Bytes {
            data: Vec::new(),
            total: 0,
        };
        if self.faulted {
            return Ok((1, none()));
        }
        let t0 = self.begin();
        let result = self.context.with(|ctx| {
            let run = || -> rquickjs::Result<(u8, Option<TypedArray<u8>>)> {
                let f = self.i2c_fn.clone().restore(&ctx)?;
                let tx = TypedArray::<u8>::new(ctx.clone(), tx.to_vec())?;
                let r: Object = f.call((address, tx, rx_len as u32))?;
                let status: u8 = r.get("s")?;
                let rx = if status == 0 {
                    Some(r.get("rx")?)
                } else {
                    None
                };
                Ok((status, rx))
            };
            let (status, rx) = run().map_err(|e| describe(&ctx, e))?;
            match rx {
                Some(array) => js_bytes::copy(&array, rx_len)
                    .map(|b| (status, b))
                    .map_err(|e| e.to_string()),
                None => Ok((status, none())),
            }
        });
        self.deadline.store(u64::MAX, Ordering::Relaxed);
        let reply = result.map_err(|e| self.fault(e))?;
        self.end(t0, "onI2C")?;
        Ok(reply)
    }

    /// Transaction SPI : octets rendus par le modèle (au plus `len`, avec la
    /// longueur rendue), ou None s'il ne répond rien. Un composant en défaut
    /// ne répond plus (MISO à 0xFF).
    fn spi(&mut self, tx: &[u8], len: usize, wants_rx: bool) -> Result<Option<Bytes>, String> {
        if self.faulted {
            return Ok(None);
        }
        let t0 = self.begin();
        let result = self.context.with(|ctx| {
            let run = || -> rquickjs::Result<Option<TypedArray<u8>>> {
                let f = self.spi_fn.clone().restore(&ctx)?;
                let data = if tx.is_empty() {
                    vec![0xFF; len]
                } else {
                    tx.to_vec()
                };
                let tx = TypedArray::<u8>::new(ctx.clone(), data)?;
                f.call((tx, wants_rx))
            };
            let rx = run().map_err(|e| describe(&ctx, e))?;
            match rx {
                Some(array) => js_bytes::copy(&array, len)
                    .map(Some)
                    .map_err(|e| e.to_string()),
                None => Ok(None),
            }
        });
        self.deadline.store(u64::MAX, Ordering::Relaxed);
        let reply = result.map_err(|e| self.fault(e))?;
        self.end(t0, "onSPI")?;
        Ok(reply)
    }

    /// Trame d'un ruban (onLedStrip). Un composant en défaut ne reçoit plus rien.
    fn led_strip(&mut self, data: &[u8], iface: &str) -> Result<(), String> {
        if self.faulted {
            return Ok(());
        }
        let t0 = self.begin();
        let result = self.context.with(|ctx| {
            let run = || -> rquickjs::Result<()> {
                let f = self.strip_fn.clone().restore(&ctx)?;
                let data = TypedArray::<u8>::new(ctx.clone(), data.to_vec())?;
                f.call((data, iface))
            };
            run().map_err(|e| describe(&ctx, e))
        });
        self.deadline.store(u64::MAX, Ordering::Relaxed);
        result.map_err(|e| self.fault(e))?;
        self.end(t0, "onLedStrip")?;
        Ok(())
    }

    fn fault(&mut self, error: String) -> String {
        self.faulted = true;
        format!("composant {} arrêté : {error}", self.id)
    }
}

/// Réponse I2C à exactement `rx_len` octets : complétée par 0xFF ou tronquée,
/// avec un avertissement la première fois (component-api.d.ts, I2CResponse).
fn fit_rx(
    inst: &mut Instance,
    bus: &str,
    rx: Bytes,
    rx_len: usize,
    emit: &dyn Fn(ModelEvent),
) -> Vec<u8> {
    if rx.total != rx_len && !inst.rx_len_warned {
        inst.rx_len_warned = true;
        emit(ModelEvent::Warn(format!(
            "{} : réponse {bus} de {} octets pour {rx_len} demandés (complétée par 0xFF ou tronquée)",
            inst.id, rx.total
        )));
    }
    let mut data = rx.data;
    data.resize(rx_len, 0xFF);
    data
}

// --- Boucle du fil « modèles » ------------------------------------------------------------

/// Boucle du fil : `emit` range un événement dans le lot, `send` l'envoie.
/// Les événements de chaque commande sont rangés juste après elle (ordre
/// conservé) ; le lot part quand la file est vide, après une réponse
/// synchrone (ping, I2C) ou une image (FlushStates), ou toutes les
/// BATCH_COMMANDS commandes.
fn run(
    specs: Vec<InstanceSpec>,
    rx: &mpsc::Receiver<ModelCmd>,
    emit: &dyn Fn(ModelEvent),
    send: &dyn Fn(),
) {
    let _ = cpu_ns(); // Windows : étalonnage du compteur de cycles, hors de tout budget
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
    send();

    let mut pending: Option<ModelCmd> = None;
    let mut since_send = 0u32;
    // Version de diagnostic : temps CPU de ce fil et commandes reçues, au Journal.
    #[cfg(feature = "diag")]
    let mut diag = (Instant::now(), cpu_ns(), 0u64);
    loop {
        #[cfg(feature = "diag")]
        {
            diag.2 += 1;
            let elapsed = diag.0.elapsed();
            if elapsed >= crate::diag::REPORT_EVERY {
                let cpu =
                    cpu_ns().saturating_sub(diag.1) as f64 / elapsed.as_nanos() as f64 * 100.0;
                let rate = diag.2 as f64 / elapsed.as_secs_f64();
                let text =
                    format!("diag : fil « modèles » {cpu:.1} % d'un cœur, {rate:.0} commandes/s");
                eprintln!("{text}");
                emit(ModelEvent::Log(text));
                diag = (Instant::now(), cpu_ns(), 0);
            }
        }
        let cmd = match pending.take() {
            Some(cmd) => Some(cmd),
            None => {
                let next_tick = instances
                    .iter()
                    .filter(|i| i.tick.is_some() && !i.faulted)
                    .map(|i| i.next_tick)
                    .min();
                match next_tick {
                    Some(at) => {
                        match rx.recv_timeout(at.saturating_duration_since(Instant::now())) {
                            Ok(cmd) => Some(cmd),
                            Err(RecvTimeoutError::Timeout) => None,
                            Err(RecvTimeoutError::Disconnected) => return,
                        }
                    }
                    None => match rx.recv() {
                        Ok(cmd) => Some(cmd),
                        Err(_) => return,
                    },
                }
            }
        };
        // Réponse attendue aussitôt : le lot part juste après cette commande.
        let urgent = matches!(
            cmd,
            Some(
                ModelCmd::Ping { .. }
                    | ModelCmd::I2c { .. }
                    | ModelCmd::Spi { seq: Some(_), .. }
                    | ModelCmd::FlushStates
            )
        );
        match cmd {
            None => run_ticks(&mut instances, emit),
            Some(ModelCmd::Stop) => return,
            Some(ModelCmd::Pin { id, pin, state }) => {
                if let Some(inst) = instances.iter_mut().find(|i| i.id == id) {
                    inst.host.borrow_mut().pins.insert(pin.clone(), state);
                    report(inst.call("pin", &pin, &pin_state_json(state)), emit);
                }
            }
            Some(ModelCmd::Params { id, params_json }) => {
                if let Some(inst) = instances.iter_mut().find(|i| i.id == id) {
                    report(inst.call("params", &params_json, ""), emit);
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
            Some(ModelCmd::I2c {
                id,
                seq,
                address,
                tx,
                rx_len,
            }) => {
                let (status, rx) = match instances.iter_mut().find(|i| i.id == id) {
                    Some(inst) => match inst.i2c(address, &tx, rx_len) {
                        Ok((0, rx)) => (0, fit_rx(inst, "I2C", rx, rx_len, emit)),
                        Ok((status, _)) => (status, Vec::new()),
                        Err(e) => {
                            emit(ModelEvent::Warn(e));
                            (1, Vec::new())
                        }
                    },
                    None => (1, Vec::new()),
                };
                emit(ModelEvent::I2cReply { seq, status, rx });
            }
            Some(ModelCmd::Spi { id, seq, tx, len }) => {
                // Écriture seule (trame d'écran) : aucune réponse construite.
                let rx = match instances.iter_mut().find(|i| i.id == id) {
                    Some(inst) => match inst.spi(&tx, len, seq.is_some()) {
                        Ok(Some(rx)) if seq.is_some() => Some(fit_rx(inst, "SPI", rx, len, emit)),
                        Ok(_) => None,
                        Err(e) => {
                            emit(ModelEvent::Warn(e));
                            None
                        }
                    },
                    None => None,
                };
                if let Some(seq) = seq {
                    let rx = rx.unwrap_or_else(|| vec![0xFF; len]); // MISO sans réponse
                    emit(ModelEvent::SpiReply { seq, rx });
                }
            }
            Some(ModelCmd::LedStrip { id, iface, data }) => {
                if let Some(inst) = instances.iter_mut().find(|i| i.id == id) {
                    report(inst.led_strip(&data, &iface), emit);
                }
            }
            Some(ModelCmd::Environment(json)) => {
                for inst in &mut instances {
                    report(inst.call("env", &json, ""), emit);
                }
            }
            Some(ModelCmd::Power(on)) => {
                for inst in &mut instances {
                    report(inst.call("power", if on { "1" } else { "0" }, ""), emit);
                }
            }
            Some(ModelCmd::FlushStates) => {
                dirty_notified = false;
                let (mut count, mut records) = (0u16, Vec::new());
                for inst in &mut instances {
                    let mut host = inst.host.borrow_mut();
                    if std::mem::take(&mut host.display_dirty) {
                        if let Some(fb) = host.display.as_mut() {
                            if let Some(update) = fb.take_update() {
                                display::encode_update(&mut records, &inst.id, fb, &update);
                                count += 1;
                            }
                        }
                    }
                }
                if count > 0 {
                    emit(ModelEvent::Displays(display::finish_message(
                        count, records,
                    )));
                }
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
                // Toujours une réponse, même vide : l'image du cœur l'attend.
                emit(ModelEvent::States(states));
            }
        }
        flush_events(&mut instances, &mut dirty_notified, emit);
        since_send += 1;
        if urgent || since_send >= BATCH_COMMANDS {
            send();
            since_send = 0;
            continue;
        }
        match rx.try_recv() {
            Ok(next) => pending = Some(next),
            Err(mpsc::TryRecvError::Empty) => {
                send();
                since_send = 0;
            }
            Err(mpsc::TryRecvError::Disconnected) => return,
        }
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
        any_dirty |= host.dirty || host.display_dirty;
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
            Box::new(move |events: Vec<ModelEvent>| {
                for e in events {
                    let _ = tx.send(e);
                }
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
            state: NetState {
                level: Level::Low,
                ..Default::default()
            },
        });
        models.send(ModelCmd::Pin {
            id: "LD2".into(),
            pin: "A".into(),
            state: NetState {
                level: Level::High,
                ..Default::default()
            },
        });
        assert_eq!(
            states(&models, &rx),
            vec![("LD2".into(), r#"{"brightness":1}"#.into())]
        );
    }

    /// Ruban WS2812 : la même trame lue selon l'ordre des couleurs (GRB, RGB,
    /// GRBW avec le blanc ajouté), trame courte (les LED suivantes gardent leur
    /// couleur), rien sans alimentation.
    #[test]
    fn ws2812_orders() {
        let frame = vec![0x10, 0x20, 0x30, 0x40, 0x50, 0x60, 0x70, 0x80];
        for (order, first, second) in [
            ("grb", 0x201030, 0x504060),
            ("rgb", 0x102030, 0x405060),
            ("grbw", 0x605070, 0xE0D0F0),
        ] {
            let mut s = spec("strip", provided("ws2812"));
            s.params.insert("count".into(), 3.into());
            s.params.insert("order".into(), order.into());
            let (models, rx) = start(vec![s]);
            let _ = states(&models, &rx);
            let strip = |data: Vec<u8>| {
                models.send(ModelCmd::LedStrip {
                    id: "strip".into(),
                    iface: "DIN".into(),
                    data,
                });
            };
            strip(frame.clone()); // non alimenté : ignorée
            assert_eq!(
                states(&models, &rx),
                Vec::<(String, String)>::new(),
                "{order}"
            );
            for (pin, level) in [("GND", Level::Low), ("5V", Level::High)] {
                models.send(ModelCmd::Pin {
                    id: "strip".into(),
                    pin: pin.into(),
                    state: NetState {
                        level,
                        ..Default::default()
                    },
                });
            }
            strip(frame.clone());
            let colors = |st: Vec<(String, String)>| -> Vec<u64> {
                let v: serde_json::Value =
                    serde_json::from_str(&st.last().expect("état").1).expect("JSON");
                v["colors"]
                    .as_array()
                    .expect("couleurs")
                    .iter()
                    .filter_map(serde_json::Value::as_u64)
                    .collect()
            };
            assert_eq!(
                colors(states(&models, &rx)),
                vec![first, second, 0],
                "{order}"
            );
            // Trame d'une seule LED : les suivantes gardent leur couleur.
            strip(vec![0, 0, 0, 0]);
            assert_eq!(colors(states(&models, &rx)), vec![0, second, 0], "{order}");
        }
    }

    /// MPU6050 : en veille au reset (mesures à 0), réveillé par PWR_MGMT_1 = 0 ;
    /// le gyroscope lit une rotation positive autour de x juste après un roulis.
    #[test]
    fn mpu6050_gyro_follows_tilt_change() {
        let (models, rx) = start(vec![spec("imu", provided("mpu6050"))]);
        let _ = states(&models, &rx);
        for (pin, level) in [("GND", Level::Low), ("VCC", Level::High)] {
            models.send(ModelCmd::Pin {
                id: "imu".into(),
                pin: pin.into(),
                state: NetState {
                    level,
                    ..Default::default()
                },
            });
        }
        let env = |roll: f64| {
            let e = crate::core::environment::Environment {
                tilt_deg: crate::core::environment::Tilt { roll, pitch: 0.0 },
                ..Default::default()
            }
            .settle();
            ModelCmd::Environment(serde_json::to_string(&e).expect("JSON"))
        };
        let i2c = |seq: u32, tx: Vec<u8>, rx_len: usize| -> Vec<u8> {
            models.send(ModelCmd::I2c {
                id: "imu".into(),
                seq,
                address: 0x68,
                tx,
                rx_len,
            });
            loop {
                if let ModelEvent::I2cReply {
                    seq: s,
                    status,
                    rx: r,
                } = rx.recv_timeout(Duration::from_secs(5)).expect("réponse")
                {
                    if s == seq {
                        assert_eq!(status, 0);
                        return r;
                    }
                }
            }
        };
        models.send(env(0.0));
        // En veille : mesures à 0.
        assert_eq!(i2c(1, vec![0x3F], 2), vec![0, 0]);
        i2c(2, vec![0x6B, 0x00], 0); // réveil
        let az = i2c(3, vec![0x3F], 2);
        assert_eq!(
            i16::from_be_bytes([az[0], az[1]]),
            16384,
            "1 g sur z à plat"
        );
        models.send(env(10.0));
        let gx = i2c(4, vec![0x43], 2);
        assert!(
            i16::from_be_bytes([gx[0], gx[1]]) > 0,
            "rotation autour de x : {gx:?}"
        );
        assert_eq!(i2c(5, vec![0x75], 1), vec![0x68]);
    }

    /// Écran SSD1306 : image entière (1 024 octets) et reconstruction après
    /// changement de sens des lignes, 50 fois, sans jamais dépasser le budget
    /// de 5 ms par appel (le composant serait arrêté) ; chaque réponse ACK.
    #[test]
    fn ssd1306_full_frames_within_budget() {
        let (models, rx) = start(vec![spec("oled1", provided("ssd1306"))]);
        let _ = states(&models, &rx);
        for (pin, level) in [("GND", Level::Low), ("VCC", Level::High)] {
            models.send(ModelCmd::Pin {
                id: "oled1".into(),
                pin: pin.into(),
                state: NetState {
                    level,
                    ..Default::default()
                },
            });
        }
        let mut frame = vec![0x40u8];
        frame.extend((0..1024).map(|i| (i * 37 % 251) as u8));
        let mut worst = 0;
        for n in 0..50u32 {
            // Commandes (dont C0/C8 : image entière recalculée), puis l'image.
            let cmds = vec![
                0x00,
                0x8D,
                0x14,
                0x20,
                0x00,
                if n % 2 == 0 { 0xC8 } else { 0xC0 },
                0xA1,
                0xAF,
            ];
            for (seq, tx) in [(2 * n, cmds), (2 * n + 1, frame.clone())] {
                models.send(ModelCmd::I2c {
                    id: "oled1".into(),
                    seq,
                    address: 0x3C,
                    tx,
                    rx_len: 0,
                });
                let t0 = Instant::now();
                loop {
                    match rx
                        .recv_timeout(Duration::from_secs(5))
                        .expect("réponse I2C")
                    {
                        ModelEvent::I2cReply { seq: s, status, .. } if s == seq => {
                            assert_eq!(status, 0, "NACK à la transaction {seq}");
                            break;
                        }
                        ModelEvent::Warn(w) => panic!("avertissement : {w}"),
                        _ => {}
                    }
                }
                worst = worst.max(t0.elapsed().as_micros());
            }
        }
        println!("SSD1306 : transaction la plus longue {worst} µs (budget 5 000 µs de CPU)");
    }

    #[test]
    fn button_drives_its_pin() {
        let (models, rx) = start(vec![spec("B1", provided("button"))]);
        let _ = states(&models, &rx);
        models.send(ModelCmd::Pin {
            id: "B1".into(),
            pin: "2".into(),
            state: NetState {
                level: Level::Low,
                ..Default::default()
            },
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

    /// Potentiomètre : la position du bouton fixe la tension du curseur.
    #[test]
    fn potentiometer_sets_wiper_voltage() {
        let (models, rx) = start(vec![spec("pot1", provided("potentiometer"))]);
        let _ = states(&models, &rx);
        for (pin, level, volts) in [("VCC", Level::High, 3.3), ("GND", Level::Low, 0.0)] {
            models.send(ModelCmd::Pin {
                id: "pot1".into(),
                pin: pin.into(),
                state: NetState {
                    level,
                    pwm: None,
                    volts: Some(volts),
                },
            });
        }
        let wiper = |expected: f64| loop {
            match rx.recv_timeout(Duration::from_secs(5)).expect("tension") {
                ModelEvent::Volts { pin, volts, .. } if pin == "W" && volts == Some(expected) => {
                    break
                }
                ModelEvent::Warn(w) => panic!("avertissement inattendu : {w}"),
                _ => {}
            }
        };
        wiper(1.65);
        models.send(ModelCmd::Control {
            id: "pot1".into(),
            control: "position".into(),
            value_json: "0".into(),
        });
        wiper(0.0);
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
        // Plafond dur : 50 ms de temps CPU. Sur une machine chargée (runner de
        // CI), le temps écoulé peut en être plusieurs fois plus long : la borne
        // vérifie seulement que la boucle est arrêtée, pas laissée tourner.
        assert!(
            t0.elapsed() < Duration::from_secs(1),
            "interrompu trop tard : {:?}",
            t0.elapsed()
        );
    }

    /// Règle du budget : un appel de 10 ms (au-delà des 5 ms) est toléré et
    /// écrit au Journal ; un appel rapide remet le compte à zéro ; trois
    /// appels lents de suite mettent le composant en défaut.
    #[test]
    fn isolated_overrun_is_tolerated() {
        let source = temp_component(
            "lent",
            "export default (ctx: unknown) => ({ onControl(_c: string, ms: number): void { const t = Date.now(); while (Date.now() - t < ms) {} }, getState: () => ({}) });",
        );
        let (models, rx) = start(vec![spec("x", source)]);
        let _ = rx.recv_timeout(Duration::from_secs(5));
        let control = |ms: u32| {
            models.send(ModelCmd::Control {
                id: "x".into(),
                control: "c".into(),
                value_json: ms.to_string(),
            });
        };
        // Lent, rapide, lent, lent : jamais trois de suite, aucun défaut.
        // 30 ms de temps écoulé : au-delà des 5 ms de CPU même si le fil n'a
        // qu'une part du processeur (runner chargé), sous le plafond de 50 ms.
        for ms in [30, 0, 30, 30] {
            control(ms);
        }
        let mut logs = Vec::new();
        let deadline = Instant::now() + Duration::from_secs(2);
        while let Ok(e) = rx.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
            match e {
                ModelEvent::Log(l) => logs.push(l),
                ModelEvent::Warn(w) => panic!("défaut inattendu : {w}"),
                _ => {}
            }
        }
        assert_eq!(logs.len(), 3, "{logs:#?}");
        assert!(
            logs[0].contains("onControl") && logs[0].contains("1/3"),
            "{logs:#?}"
        );
        assert!(
            logs[2].contains("2/3"),
            "remis à zéro par l'appel rapide : {logs:#?}"
        );
        // Troisième lent de suite : défaut.
        control(30);
        let warn = loop {
            if let ModelEvent::Warn(w) = rx.recv_timeout(Duration::from_secs(5)).expect("défaut") {
                break w;
            }
        };
        assert!(warn.contains("3 appels de suite"), "{warn}");
    }

    /// Panneau environnement : objet Environment remis à onEnvironment.
    #[test]
    fn environment_reaches_models() {
        let source = temp_component(
            "env",
            "export default (ctx: { invalidate(): void }) => { let t = 0, z = 0; return { onEnvironment(e: { temperatureC: number; accelerationG: { z: number } }): void { t = e.temperatureC; z = e.accelerationG.z; ctx.invalidate(); }, getState: () => ({ t, z }) }; };",
        );
        let (models, rx) = start(vec![spec("e", source)]);
        let _ = states(&models, &rx);
        let env = crate::core::environment::Environment {
            temperature_c: 31.5,
            ..Default::default()
        }
        .settle();
        models.send(ModelCmd::Environment(
            serde_json::to_string(&env).expect("JSON"),
        ));
        assert_eq!(
            states(&models, &rx),
            vec![("e".into(), r#"{"t":31.5,"z":1}"#.into())]
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

// app/src-tauri/src/core/checks.rs — Validations du câblage (section 9.2), sans simulation.
//
// Calculées à l'ouverture du projet et à chaque enregistrement de la paillasse,
// à partir du fichier de carte, des fils de prebench.toml et des manifestes :
//   - rôle d'une broche incompatible avec la pastille (masse, alimentation, ADC) ;
//   - broche d'un bus (I2C, SPI, UART) sur une broche sans cette fonction ;
//   - tension logique d'un composant différente de celle de la carte ;
//   - 5 V relié à une broche du microcontrôleur ;
//   - deux alimentations différentes sur le même net (court-circuit) ;
//   - composant posé directement entre deux alimentations (court-circuit possible) ;
//   - broche CS partagée par deux composants SPI ;
//   - broches à contrainte (entrée seule, strapping, flash).
// Les validations de la simulation (conflit, entrée flottante) viennent du
// réseau logique, pendant la simulation (sim.rs).

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;
use serde_json::Value;

use super::board::Board;
use super::net::{Network, Node};
use crate::project::{split_endpoint, Project};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Warn,
    Conflict,
}

/// Un problème de câblage, affiché dans l'onglet Avertissements.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Check {
    pub severity: Severity,
    pub text: String,
    /// Extrémités concernées (« board.CN5:6 », « led1.A ») : sélection au clic.
    pub endpoints: Vec<String>,
}

/// Rôle et manifeste d'une broche de composant placé.
fn component_pin<'a>(
    project: &Project,
    manifests: &'a BTreeMap<String, Value>,
    id: &str,
    pin: &str,
) -> Option<(&'a Value, &'a Value)> {
    let kind = &project.components.iter().find(|c| c.id == id)?.kind;
    let manifest = manifests.get(kind)?;
    let def = manifest["pins"]
        .as_array()?
        .iter()
        .find(|p| p["name"] == pin)?;
    Some((manifest, def))
}

/// « led1.A (LED rouge) » : broche d'un composant, avec son nom affiché.
fn pin_label(project: &Project, id: &str, pin: &str) -> String {
    match project
        .components
        .iter()
        .find(|c| c.id == id)
        .and_then(|c| c.name.as_deref())
        .map(str::trim)
        .filter(|n| !n.is_empty())
    {
        Some(n) => format!("{id}.{pin} ({n})"),
        None => format!("{id}.{pin}"),
    }
}

fn label(board: &Board, signal: &str) -> String {
    match board.pins.get(signal).and_then(|p| p.label.as_deref()) {
        Some(l) if l != signal => format!("{l} ({signal})"),
        _ => signal.to_string(),
    }
}

pub fn check(board: &Board, project: &Project, manifests: &BTreeMap<String, Value>) -> Vec<Check> {
    let mut out = Vec::new();
    let mut net = Network::default();
    let mut ends: Vec<(String, Node)> = Vec::new();

    for w in &project.wires {
        let resolve = |e: &str| -> Option<Node> {
            let (id, pin) = split_endpoint(e)?;
            if id == "board" {
                board.signal_of(pin).map(Node::Board)
            } else {
                Some(Node::Component {
                    id: id.into(),
                    pin: pin.into(),
                })
            }
        };
        if let (Some(a), Some(b)) = (resolve(&w.from), resolve(&w.to)) {
            ends.push((w.from.clone(), a.clone()));
            ends.push((w.to.clone(), b.clone()));
            net.connect(a, b);
        }
    }

    // Rôle de chaque broche de composant face aux broches de la carte de son net.
    let mut seen = BTreeSet::new();
    for (endpoint, node) in &ends {
        let Node::Component { id, pin } = node else {
            continue;
        };
        if !seen.insert(endpoint.clone()) {
            continue;
        }
        let Some((_, def)) = component_pin(project, manifests, id, pin) else {
            continue;
        };
        let role = def["role"].as_str().unwrap_or("passive");
        let who = pin_label(project, id, pin);
        for member in net.members(node) {
            let Node::Board(signal) = member else {
                continue;
            };
            let volts = board.power.get(&signal).copied();
            let caps = board
                .pins
                .get(&signal)
                .map(|p| p.caps.clone())
                .unwrap_or_default();
            let problem = match role {
                "ground" if volts != Some(0.0) => Some(format!(
                    "{who} est une masse, reliée à {} : reliez-la à une pastille GND.",
                    label(board, &signal)
                )),
                "power" if !volts.is_some_and(|v| v > 0.0) => Some(format!(
                    "{who} est une alimentation, reliée à {} : reliez-la à 3V3 ou 5V.",
                    label(board, &signal)
                )),
                "analog-out" if !caps.iter().any(|c| c.starts_with("adc")) => Some(format!(
                    "{who} fournit une tension analogique, mais {} n'a pas d'entrée ADC : choisissez une broche A0 à A5.",
                    label(board, &signal)
                )),
                _ => None,
            };
            if let Some(text) = problem {
                out.push(Check {
                    severity: Severity::Warn,
                    text,
                    endpoints: vec![endpoint.clone()],
                });
            }
        }
    }

    // Bus déclarés par les manifestes : la broche de la carte doit porter la fonction.
    let mut cs_users: BTreeMap<Node, Vec<String>> = BTreeMap::new();
    for comp in &project.components {
        let Some(manifest) = manifests.get(&comp.kind) else {
            continue;
        };
        for iface in manifest["interfaces"].as_array().into_iter().flatten() {
            let kind = iface["type"].as_str().unwrap_or_default();
            for (key, role) in [
                ("sda", "SDA"),
                ("scl", "SCL"),
                ("sck", "SCK"),
                ("mosi", "MOSI"),
                ("miso", "MISO"),
                ("tx", "TX"),
                ("rx", "RX"),
                ("cs", "CS"),
            ] {
                let Some(pin) = iface[key].as_str() else {
                    continue;
                };
                let node = Node::Component {
                    id: comp.id.clone(),
                    pin: pin.into(),
                };
                if key == "cs" {
                    cs_users
                        .entry(root_board(&net, &node).unwrap_or(node.clone()))
                        .or_default()
                        .push(format!("{}.{pin}", comp.id));
                    continue;
                }
                // TX du composant sur RX de la carte, et inversement.
                let wanted = match role {
                    "TX" => "RX",
                    "RX" => "TX",
                    r => r,
                };
                for member in net.members(&node) {
                    let Node::Board(signal) = member else {
                        continue;
                    };
                    let Some(def) = board.pins.get(&signal) else {
                        continue;
                    };
                    let ok = def.caps.iter().any(|c| {
                        c.starts_with(kind)
                            && (c.ends_with(&format!(":{wanted}")) || c.as_str() == kind)
                    });
                    if !ok {
                        out.push(Check {
                            severity: Severity::Warn,
                            text: format!(
                                "{} ({} {role}) est reliée à {}, qui n'a pas la fonction {} {wanted}.",
                                pin_label(project, &comp.id, pin),
                                kind.to_uppercase(),
                                label(board, &signal),
                                kind.to_uppercase()
                            ),
                            endpoints: vec![format!("{}.{pin}", comp.id)],
                        });
                    }
                }
            }
        }
        // Tension logique du composant.
        if let Some(volts) = manifest["logic_voltages"].as_array() {
            let ok = volts
                .iter()
                .filter_map(Value::as_f64)
                .any(|v| (v - board.logic_voltage).abs() < 0.05);
            let wired = project.wires.iter().any(|w| {
                w.from.starts_with(&format!("{}.", comp.id))
                    || w.to.starts_with(&format!("{}.", comp.id))
            });
            if !ok && wired {
                let list: Vec<String> = volts
                    .iter()
                    .filter_map(Value::as_f64)
                    .map(|v| format!("{v} V"))
                    .collect();
                out.push(Check {
                    severity: Severity::Warn,
                    text: format!(
                        "{} fonctionne en {}, la carte en {} V : niveaux logiques incompatibles.",
                        comp.label(),
                        list.join(" ou "),
                        board.logic_voltage
                    ),
                    endpoints: vec![],
                });
            }
        }
    }
    // Composant posé directement entre deux alimentations différentes : ses
    // broches de signal (ni alimentation ni masse déclarées) touchent chacune
    // une alimentation. Sans calcul analogique, rien ne limite le courant
    // (LED sans résistance, bouton entre 3V3 et GND…) : court-circuit possible.
    for comp in &project.components {
        let mut supplied: BTreeMap<String, (String, f64)> = BTreeMap::new();
        let Some(manifest) = manifests.get(&comp.kind) else {
            continue;
        };
        for def in manifest["pins"].as_array().into_iter().flatten() {
            let (Some(pin), role) = (
                def["name"].as_str(),
                def["role"].as_str().unwrap_or("passive"),
            ) else {
                continue;
            };
            if role == "power" || role == "ground" {
                continue;
            }
            let node = Node::Component {
                id: comp.id.clone(),
                pin: pin.into(),
            };
            let supply = net.members(&node).into_iter().find_map(|m| match m {
                Node::Board(s) => board.power.get(&s).map(|v| (s, *v)),
                Node::Component { .. } => None,
            });
            if let Some(s) = supply {
                supplied.insert(pin.to_string(), s);
            }
        }
        let volts: BTreeSet<String> = supplied.values().map(|(_, v)| format!("{v}")).collect();
        if volts.len() > 1 {
            let pins: Vec<String> = supplied
                .iter()
                .map(|(p, (s, _))| format!("{p} sur {s}"))
                .collect();
            out.push(Check {
                severity: Severity::Conflict,
                text: format!(
                    "Court-circuit possible : {} est posé directement entre deux alimentations ({}). \
                     Prebench ne calcule pas les courants : un composant dont les broches touchent chacune \
                     une alimentation différente, sans résistance ni broche du microcontrôleur entre elles, \
                     est signalé comme un court-circuit. Ajoutez une résistance en série ou reliez une \
                     broche à une sortie du microcontrôleur.",
                    comp.label(),
                    pins.join(", ")
                ),
                endpoints: supplied.keys().map(|p| format!("{}.{p}", comp.id)).collect(),
            });
        }
    }
    for (_, users) in cs_users {
        if users.len() > 1 {
            out.push(Check {
                severity: Severity::Warn,
                text: format!(
                    "Broche CS partagée par {} : chaque composant SPI doit avoir la sienne.",
                    users.join(", ")
                ),
                endpoints: users,
            });
        }
    }

    // Nets de la carte : alimentations différentes (court-circuit), 5 V sur une broche, contraintes.
    let roots: BTreeSet<Node> = ends.iter().map(|(_, n)| n.clone()).collect();
    let mut done: BTreeSet<Vec<Node>> = BTreeSet::new();
    for node in roots {
        let mut members = net.members(&node);
        members.sort();
        if !done.insert(members.clone()) {
            continue;
        }
        let signals: Vec<String> = members
            .iter()
            .filter_map(|m| match m {
                Node::Board(s) => Some(s.clone()),
                Node::Component { .. } => None,
            })
            .collect();
        let supplies: BTreeSet<String> = signals
            .iter()
            .filter_map(|s| board.power.get(s).map(|v| format!("{v}")))
            .collect();
        let pads: Vec<String> = ends
            .iter()
            .filter(|(_, n)| members.contains(n) && matches!(n, Node::Board(_)))
            .map(|(e, _)| e.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        if supplies.len() > 1 {
            let names: Vec<&String> = signals
                .iter()
                .filter(|s| board.power.contains_key(*s))
                .collect();
            out.push(Check {
                severity: Severity::Conflict,
                text: format!(
                    "Court-circuit : {} sont reliées par des fils (même net), chaque alimentation imposant sa tension. \
                     Prebench ne calcule pas les courants : deux alimentations différentes reliées sans composant \
                     entre elles sont un court-circuit. Retirez le fil qui les relie.",
                    names
                        .iter()
                        .map(|s| s.as_str())
                        .collect::<Vec<_>>()
                        .join(" et ")
                ),
                endpoints: pads.clone(),
            });
        }
        let five = signals.iter().any(|s| {
            board
                .power
                .get(s)
                .is_some_and(|v| *v > board.logic_voltage + 0.5)
        });
        for s in signals.iter().filter(|s| board.pins.contains_key(*s)) {
            if five {
                out.push(Check {
                    severity: Severity::Warn,
                    text: format!(
                        "{} reliée à une alimentation de plus de {} V : la broche n'est pas prévue pour ce niveau.",
                        label(board, s),
                        board.logic_voltage
                    ),
                    endpoints: pads.clone(),
                });
            }
            for c in board
                .pins
                .get(s)
                .map(|p| p.constraints.clone())
                .unwrap_or_default()
            {
                let text = match c.as_str() {
                    "input-only" => format!("{} est en entrée seule : elle ne peut pas piloter un composant.", label(board, s)),
                    "strapping" => format!(
                        "{} est une broche de démarrage (strapping) : un composant peut empêcher la carte de démarrer.",
                        label(board, s)
                    ),
                    "flash" => format!("{} est reliée à la mémoire flash : ne l'utilisez pas.", label(board, s)),
                    _ => continue,
                };
                out.push(Check {
                    severity: Severity::Warn,
                    text,
                    endpoints: pads.clone(),
                });
            }
        }
    }
    out.dedup();
    out
}

/// Première broche de la carte du net d'un nœud (clé stable d'un net).
fn root_board(net: &Network, node: &Node) -> Option<Node> {
    let mut members = net.members(node);
    members.sort();
    members.into_iter().find(|m| matches!(m, Node::Board(_)))
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;
    use crate::project::{ComponentInstance, Wire};

    fn board() -> Board {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../boards");
        Board::load(&dir, "nucleo-f401re").expect("carte")
    }

    fn project(components: Vec<(&str, &str)>, wires: &[(&str, &str)]) -> Project {
        Project {
            board: "nucleo-f401re".into(),
            framework: "stm32-hal".into(),
            components: components
                .into_iter()
                .map(|(id, kind)| ComponentInstance {
                    id: id.into(),
                    kind: kind.into(),
                    name: None,
                    params: Default::default(),
                    pos: [0.0, 0.0],
                })
                .collect(),
            wires: wires
                .iter()
                .map(|(a, b)| Wire {
                    from: (*a).into(),
                    to: (*b).into(),
                    color: None,
                })
                .collect(),
            notes: Vec::new(),
        }
    }

    fn manifests() -> BTreeMap<String, Value> {
        BTreeMap::from([
            (
                "pot".to_string(),
                serde_json::json!({ "pins": [
                    { "name": "GND", "role": "ground" }, { "name": "VCC", "role": "power" }, { "name": "W", "role": "analog-out" }
                ], "logic_voltages": [3.3, 5] }),
            ),
            (
                "oled".to_string(),
                serde_json::json!({ "pins": [
                    { "name": "SDA", "role": "io" }, { "name": "SCL", "role": "input" }
                ], "interfaces": [{ "type": "i2c", "sda": "SDA", "scl": "SCL" }], "logic_voltages": [5] }),
            ),
            (
                "tft".to_string(),
                serde_json::json!({ "pins": [{ "name": "CS", "role": "input" }],
                    "interfaces": [{ "type": "spi", "cs": "CS" }] }),
            ),
        ])
    }

    #[test]
    fn correct_wiring_has_no_check() {
        let p = project(
            vec![("pot1", "pot")],
            &[
                ("board.CN6:6", "pot1.GND"),
                ("board.CN6:4", "pot1.VCC"),
                ("board.CN8:1", "pot1.W"),
            ],
        );
        assert_eq!(check(&board(), &p, &manifests()), vec![]);
    }

    #[test]
    fn roles_bus_voltage_and_shorts() {
        let p = project(
            vec![
                ("pot1", "pot"),
                ("oled1", "oled"),
                ("t1", "tft"),
                ("t2", "tft"),
            ],
            &[
                ("board.CN5:6", "pot1.GND"),  // masse sur D13
                ("board.CN5:1", "pot1.W"),    // analogique sur D8 (sans ADC)
                ("board.CN5:6", "oled1.SDA"), // SDA sur D13 (sans I2C)
                ("board.CN5:4", "t1.CS"),
                ("board.CN5:4", "t2.CS"), // CS partagée
                ("board.CN6:4", "pot1.VCC"),
                ("board.CN6:5", "pot1.VCC"), // 3V3 et 5V reliés : court-circuit
            ],
        );
        let texts: Vec<String> = check(&board(), &p, &manifests())
            .into_iter()
            .map(|c| c.text)
            .collect();
        let has = |s: &str| texts.iter().any(|t| t.contains(s));
        assert!(has("pot1.GND est une masse"), "{texts:#?}");
        assert!(has("pas d'entrée ADC"), "{texts:#?}");
        assert!(has("fonction I2C SDA"), "{texts:#?}");
        assert!(has("oled1 fonctionne en 5 V"), "{texts:#?}");
        assert!(has("Broche CS partagée"), "{texts:#?}");
        assert!(has("Court-circuit : 3V3 et 5V"), "{texts:#?}");
    }

    #[test]
    fn five_volts_on_a_pin() {
        let p = project(
            vec![("pot1", "pot")],
            &[("board.CN6:5", "pot1.W"), ("board.CN8:1", "pot1.W")],
        );
        let texts: Vec<String> = check(&board(), &p, &manifests())
            .into_iter()
            .map(|c| c.text)
            .collect();
        assert!(
            texts
                .iter()
                .any(|t| t.contains("A0 (PA0) reliée à une alimentation de plus de 3.3 V")),
            "{texts:#?}"
        );
    }

    /// Composant posé entre deux alimentations (cas signalé au test manuel) :
    /// LED entre 5V et 3V3, LED ou bouton entre 3V3 et GND ; pas une LED pilotée.
    #[test]
    fn component_between_supplies() {
        let m = BTreeMap::from([
            (
                "led".to_string(),
                serde_json::json!({ "pins": [{ "name": "A", "role": "input" }, { "name": "K", "role": "input" }] }),
            ),
            (
                "button".to_string(),
                serde_json::json!({ "pins": [{ "name": "1", "role": "passive" }, { "name": "2", "role": "passive" }] }),
            ),
        ]);
        let short = |c: Vec<(&str, &str)>, w: &[(&str, &str)]| {
            check(&board(), &project(c, w), &m).into_iter().any(|c| {
                c.severity == Severity::Conflict && c.text.starts_with("Court-circuit possible")
            })
        };
        assert!(short(
            vec![("led1", "led")],
            &[("board.CN6:5", "led1.A"), ("board.CN6:4", "led1.K")]
        ));
        assert!(short(
            vec![("led1", "led")],
            &[("board.CN6:4", "led1.A"), ("board.CN6:6", "led1.K")]
        ));
        assert!(short(
            vec![("b1", "button")],
            &[("board.CN6:4", "b1.1"), ("board.CN5:7", "b1.2")]
        ));
        assert!(!short(
            vec![("led1", "led")],
            &[("board.CN5:6", "led1.A"), ("board.CN6:6", "led1.K")]
        ));
    }
}

// app/src-tauri/src/core/net.rs — Réseau électrique logique (section 9, D8).
//
// Chaque câble relie deux nœuds ; les nœuds reliés forment un net. L'état d'un
// net se calcule à partir des sorties qui le pilotent et des résistances de
// tirage, sans aucune simulation analogique. Deux informations s'y ajoutent
// (étape 8) : la PWM d'une sortie (fréquence et rapport cyclique, jamais les
// fronts) et une tension quand elle est connue (alimentation de la carte, ou
// composant comme un potentiomètre), lue par l'ADC.

use std::collections::{BTreeSet, HashMap};

use serde::Serialize;

/// État d'un net (section 9.1) ; mêmes noms que NetLevel de component-api.d.ts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Level {
    High,
    Low,
    PulledHigh,
    PulledLow,
    Floating,
    Conflict,
}

impl Level {
    /// Valeur lue par une entrée numérique (flottant et conflit lus à 0).
    pub fn as_bit(self) -> u8 {
        matches!(self, Level::High | Level::PulledHigh) as u8
    }
}

/// Signal PWM : fréquence (Hz) et rapport cyclique (0..65535 = 0..100 %).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Pwm {
    pub freq_hz: u32,
    pub duty: u16,
}

impl Pwm {
    /// Rapport cyclique de 0 à 1.
    pub fn ratio(self) -> f64 {
        f64::from(self.duty) / 65535.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Drive {
    High,
    Low,
    Z,
    /// Sortie PWM active : niveau bas si le rapport cyclique est nul, haut sinon.
    Pwm(Pwm),
}

/// État complet d'un net : niveau logique, PWM et tension connue.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NetState {
    pub level: Level,
    pub pwm: Option<Pwm>,
    pub volts: Option<f64>,
}

impl Default for NetState {
    fn default() -> Self {
        NetState {
            level: Level::Floating,
            pwm: None,
            volts: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pull {
    Up,
    Down,
    None,
}

/// Nœud du réseau : broche ou alimentation de la carte, ou broche de composant.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Node {
    Board(String),
    Component { id: String, pin: String },
}

/// Source d'une contribution sur un nœud : plusieurs peuvent coexister
/// (pull-up soudé sur la carte et pull-up configuré par le firmware).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Source {
    Board,
    Firmware,
    Component,
}

#[derive(Default)]
pub struct Network {
    parent: HashMap<Node, Node>,
    drives: HashMap<(Node, Source), Drive>,
    pulls: HashMap<(Node, Source), Pull>,
    /// Tensions imposées : composant (potentiomètre) ou alimentation de la carte.
    volts: HashMap<(Node, Source), f64>,
    states: HashMap<Node, NetState>, // par racine de net
    /// Tension logique de la carte : seuils de lecture d'une tension (70 % et 30 %).
    logic_volts: Option<f64>,
}

impl Network {
    fn find(&self, node: &Node) -> Node {
        let mut cur = node.clone();
        while let Some(p) = self.parent.get(&cur) {
            if *p == cur {
                break;
            }
            cur = p.clone();
        }
        cur
    }

    /// Seuils numériques d'un net dont seule une tension est connue
    /// (curseur de potentiomètre) : haut au-dessus de 70 % de la tension
    /// logique, bas sous 30 %, flottant (indéterminé) entre les deux.
    pub fn set_logic_voltage(&mut self, volts: f64) {
        self.logic_volts = Some(volts);
    }

    pub fn add_node(&mut self, node: Node) {
        self.parent.entry(node.clone()).or_insert(node);
    }

    /// Relie deux nœuds (câble).
    pub fn connect(&mut self, a: Node, b: Node) {
        self.add_node(a.clone());
        self.add_node(b.clone());
        let ra = self.find(&a);
        let rb = self.find(&b);
        if ra != rb {
            self.parent.insert(ra, rb);
        }
    }

    /// Nœuds du même net.
    pub fn members(&self, node: &Node) -> Vec<Node> {
        let root = self.find(node);
        self.parent
            .keys()
            .filter(|n| self.find(n) == root)
            .cloned()
            .collect()
    }

    pub fn level(&self, node: &Node) -> Level {
        self.state(node).level
    }

    /// Niveau, PWM et tension du net d'un nœud.
    pub fn state(&self, node: &Node) -> NetState {
        self.states
            .get(&self.find(node))
            .copied()
            .unwrap_or_default()
    }

    /// Fixe (Some) ou retire (None) une tension sur un nœud.
    pub fn set_volts(&mut self, node: &Node, source: Source, volts: Option<f64>) -> Vec<Node> {
        self.add_node(node.clone());
        let key = (node.clone(), source);
        match volts {
            Some(v) if v.is_finite() => {
                self.volts.insert(key, v);
            }
            _ => {
                self.volts.remove(&key);
            }
        }
        self.update(node)
    }

    pub fn set_drive(&mut self, node: &Node, source: Source, drive: Drive) -> Vec<Node> {
        self.add_node(node.clone());
        self.drives.insert((node.clone(), source), drive);
        self.update(node)
    }

    pub fn set_pull(&mut self, node: &Node, source: Source, pull: Pull) -> Vec<Node> {
        self.add_node(node.clone());
        self.pulls.insert((node.clone(), source), pull);
        self.update(node)
    }

    /// Reprend les sorties et tirages du firmware et des composants d'un ancien
    /// réseau (câblage modifié pendant la simulation : le nouveau réseau a ses
    /// propres liaisons, les contributions de la carte sont déjà posées).
    pub fn carry_over(&mut self, old: &Network) {
        for ((node, source), drive) in &old.drives {
            if *source != Source::Board {
                self.add_node(node.clone());
                self.drives.insert((node.clone(), *source), *drive);
            }
        }
        for ((node, source), pull) in &old.pulls {
            if *source != Source::Board {
                self.add_node(node.clone());
                self.pulls.insert((node.clone(), *source), *pull);
            }
        }
        for ((node, source), v) in &old.volts {
            if *source != Source::Board {
                self.add_node(node.clone());
                self.volts.insert((node.clone(), *source), *v);
            }
        }
    }

    /// Recalcule tous les nets (après construction). Renvoie tous les nœuds.
    pub fn resolve_all(&mut self) -> Vec<Node> {
        let roots: BTreeSet<Node> = self.parent.keys().map(|n| self.find(n)).collect();
        for root in roots {
            let state = self.compute(&root);
            self.states.insert(root, state);
        }
        self.parent.keys().cloned().collect()
    }

    /// Tension fixée par un composant sur un des nœuds (le premier dans l'ordre).
    fn component_volts(&self, members: &[Node]) -> Option<f64> {
        self.volts
            .iter()
            .filter(|((n, s), _)| *s == Source::Component && members.contains(n))
            .min_by(|a, b| a.0 .0.cmp(&b.0 .0))
            .map(|(_, v)| *v)
    }

    /// Recalcule le net du nœud ; renvoie ses membres si son état a changé.
    fn update(&mut self, node: &Node) -> Vec<Node> {
        let root = self.find(node);
        let state = self.compute(&root);
        if self.states.insert(root, state) == Some(state) {
            return Vec::new();
        }
        self.members(node)
    }

    fn compute(&self, root: &Node) -> NetState {
        let members: Vec<Node> = self
            .parent
            .keys()
            .filter(|n| self.find(n) == *root)
            .cloned()
            .collect();
        let (mut high, mut low, mut up, mut down) = (false, false, false, false);
        let mut pwm = None;
        for ((node, _), drive) in &self.drives {
            if members.contains(node) {
                match drive {
                    Drive::High => high = true,
                    Drive::Low => low = true,
                    Drive::Pwm(p) => {
                        pwm = Some(*p);
                        if p.duty == 0 {
                            low = true;
                        } else {
                            high = true;
                        }
                    }
                    Drive::Z => {}
                }
            }
        }
        for ((node, _), pull) in &self.pulls {
            if members.contains(node) {
                up |= *pull == Pull::Up;
                down |= *pull == Pull::Down;
            }
        }
        let level = match (high, low, up, down) {
            // Tension imposée par un composant, sans sortie numérique : seuils logiques.
            (false, false, _, _) if self.component_volts(&members).is_some() => {
                let v = self.component_volts(&members).unwrap_or(0.0);
                match self.logic_volts {
                    Some(l) if v >= 0.7 * l => Level::High,
                    Some(l) if v <= 0.3 * l => Level::Low,
                    _ => Level::Floating,
                }
            }
            (true, true, _, _) => Level::Conflict,
            (true, false, _, _) => Level::High,
            (false, true, _, _) => Level::Low,
            (false, false, true, false) => Level::PulledHigh,
            (false, false, false, true) => Level::PulledLow,
            _ => Level::Floating,
        };
        // Tension : celle d'un composant d'abord (dans un ordre stable), sinon
        // celle d'une alimentation de la carte.
        let mut sources: Vec<(&Node, Source, f64)> = self
            .volts
            .iter()
            .filter(|((n, _), _)| members.contains(n))
            .map(|((n, s), v)| (n, *s, *v))
            .collect();
        sources.sort_by(|a, b| {
            (a.1 == Source::Board)
                .cmp(&(b.1 == Source::Board))
                .then_with(|| a.0.cmp(b.0))
        });
        NetState {
            level,
            pwm: if level == Level::Conflict { None } else { pwm },
            volts: sources.first().map(|s| s.2),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn board(n: &str) -> Node {
        Node::Board(n.into())
    }

    fn comp(id: &str, pin: &str) -> Node {
        Node::Component {
            id: id.into(),
            pin: pin.into(),
        }
    }

    /// B1 de la Nucleo : PC13 tiré haut, le bouton le relie à la masse.
    #[test]
    fn button_with_pull_up() {
        let mut net = Network::default();
        net.connect(board("PC13"), comp("B1", "1"));
        net.connect(board("GND"), comp("B1", "2"));
        net.set_drive(&board("GND"), Source::Board, Drive::Low);
        net.set_pull(&board("PC13"), Source::Board, Pull::Up);
        net.resolve_all();
        assert_eq!(net.level(&board("PC13")), Level::PulledHigh);

        let changed = net.set_drive(&comp("B1", "1"), Source::Component, Drive::Low);
        assert!(changed.contains(&board("PC13")));
        assert_eq!(net.level(&board("PC13")), Level::Low);

        net.set_drive(&comp("B1", "1"), Source::Component, Drive::Z);
        assert_eq!(net.level(&board("PC13")), Level::PulledHigh);
    }

    #[test]
    fn output_conflict_and_floating() {
        let mut net = Network::default();
        net.connect(board("PA5"), comp("led", "A"));
        net.resolve_all();
        assert_eq!(net.level(&comp("led", "A")), Level::Floating);
        net.set_drive(&board("PA5"), Source::Firmware, Drive::High);
        assert_eq!(net.level(&comp("led", "A")), Level::High);
        net.set_drive(&comp("led", "A"), Source::Component, Drive::Low);
        assert_eq!(net.level(&board("PA5")), Level::Conflict);
    }

    /// Câblage modifié en cours de simulation : la sortie du firmware est
    /// reprise, le nouveau fil propage son niveau.
    #[test]
    fn rewire_keeps_firmware_outputs() {
        let mut old = Network::default();
        old.add_node(board("PA5"));
        old.set_drive(&board("PA5"), Source::Firmware, Drive::High);
        old.resolve_all();
        let mut new = Network::default();
        new.connect(board("PA5"), comp("led1", "A"));
        new.carry_over(&old);
        new.resolve_all();
        assert_eq!(new.level(&comp("led1", "A")), Level::High);
    }

    /// PWM : niveau haut sauf à 0 %, changement de rapport signalé même sans
    /// changement de niveau ; tension d'un composant prioritaire sur l'alimentation.
    #[test]
    fn pwm_and_voltage() {
        let mut net = Network::default();
        net.connect(board("PA5"), comp("led", "A"));
        net.connect(board("3V3"), comp("pot", "3"));
        net.connect(board("PA0"), comp("pot", "W"));
        net.resolve_all();
        let half = Pwm {
            freq_hz: 20_000,
            duty: 32768,
        };
        net.set_drive(&board("PA5"), Source::Firmware, Drive::Pwm(half));
        assert_eq!(net.level(&comp("led", "A")), Level::High);
        assert_eq!(net.state(&comp("led", "A")).pwm, Some(half));
        let quarter = Pwm {
            duty: 16384,
            ..half
        };
        let changed = net.set_drive(&board("PA5"), Source::Firmware, Drive::Pwm(quarter));
        assert!(
            changed.contains(&comp("led", "A")),
            "rapport changé : signalé"
        );
        net.set_drive(
            &board("PA5"),
            Source::Firmware,
            Drive::Pwm(Pwm { duty: 0, ..half }),
        );
        assert_eq!(net.level(&board("PA5")), Level::Low);

        net.set_volts(&board("3V3"), Source::Board, Some(3.3));
        assert_eq!(net.state(&comp("pot", "3")).volts, Some(3.3));
        assert!(!net
            .set_volts(&comp("pot", "W"), Source::Component, Some(1.65))
            .is_empty());
        assert_eq!(net.state(&board("PA0")).volts, Some(1.65));
        net.set_logic_voltage(3.3);
        net.set_volts(&comp("pot", "W"), Source::Component, Some(3.0));
        assert_eq!(net.level(&board("PA0")), Level::High, "au-dessus de 70 %");
        net.set_volts(&comp("pot", "W"), Source::Component, Some(0.5));
        assert_eq!(net.level(&board("PA0")), Level::Low, "sous 30 %");
        net.set_volts(&comp("pot", "W"), Source::Component, None);
        assert_eq!(net.state(&board("PA0")).volts, None);
    }

    #[test]
    fn unchanged_level_reports_nothing() {
        let mut net = Network::default();
        net.add_node(board("PA0"));
        net.resolve_all();
        assert!(!net
            .set_pull(&board("PA0"), Source::Firmware, Pull::Up)
            .is_empty());
        assert!(net
            .set_pull(&board("PA0"), Source::Firmware, Pull::Up)
            .is_empty());
    }
}

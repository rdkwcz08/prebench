// app/src-tauri/src/core/net.rs — Réseau électrique logique (section 9, D8).
//
// Chaque câble relie deux nœuds ; les nœuds reliés forment un net. L'état d'un
// net se calcule à partir des sorties qui le pilotent et des résistances de
// tirage, sans aucune simulation analogique.

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Drive {
    High,
    Low,
    Z,
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
    levels: HashMap<Node, Level>, // par racine de net
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
        self.levels
            .get(&self.find(node))
            .copied()
            .unwrap_or(Level::Floating)
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

    /// Recalcule tous les nets (après construction). Renvoie tous les nœuds.
    pub fn resolve_all(&mut self) -> Vec<Node> {
        let roots: BTreeSet<Node> = self.parent.keys().map(|n| self.find(n)).collect();
        for root in roots {
            let level = self.compute(&root);
            self.levels.insert(root, level);
        }
        self.parent.keys().cloned().collect()
    }

    /// Recalcule le net du nœud ; renvoie ses membres si son état a changé.
    fn update(&mut self, node: &Node) -> Vec<Node> {
        let root = self.find(node);
        let level = self.compute(&root);
        if self.levels.insert(root, level) == Some(level) {
            return Vec::new();
        }
        self.members(node)
    }

    fn compute(&self, root: &Node) -> Level {
        let members: Vec<Node> = self
            .parent
            .keys()
            .filter(|n| self.find(n) == *root)
            .cloned()
            .collect();
        let (mut high, mut low, mut up, mut down) = (false, false, false, false);
        for ((node, _), drive) in &self.drives {
            if members.contains(node) {
                high |= *drive == Drive::High;
                low |= *drive == Drive::Low;
            }
        }
        for ((node, _), pull) in &self.pulls {
            if members.contains(node) {
                up |= *pull == Pull::Up;
                down |= *pull == Pull::Down;
            }
        }
        match (high, low, up, down) {
            (true, true, _, _) => Level::Conflict,
            (true, false, _, _) => Level::High,
            (false, true, _, _) => Level::Low,
            (false, false, true, false) => Level::PulledHigh,
            (false, false, false, true) => Level::PulledLow,
            _ => Level::Floating,
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

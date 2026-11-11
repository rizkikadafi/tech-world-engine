use std::collections::{BTreeMap, BTreeSet, HashMap};

use serde::{Deserialize, Serialize};

use crate::document::{CircuitDocument, PartId, PinRef};
use crate::error::SchematicError;

/// Identifier of a resolved electrical net.
///
/// Net `0` is reserved for ground. Non-ground nets are numbered from `1` in a
/// deterministic order (sorted by their canonical pin).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct NetId(pub u32);

impl NetId {
    pub const GROUND: NetId = NetId(0);

    pub fn is_ground(self) -> bool {
        self.0 == 0
    }
}

impl std::fmt::Display for NetId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_ground() {
            f.write_str("GND")
        } else {
            write!(f, "N{}", self.0)
        }
    }
}

/// One set of pins that are electrically common.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResolvedNet {
    pub id: NetId,
    pub pins: Vec<PinRef>,
    pub is_ground: bool,
}

/// Result of turning wires into electrical nets.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NetMap {
    pub nets: Vec<ResolvedNet>,
    pub pin_to_net: BTreeMap<PinRef, NetId>,
}

impl NetMap {
    pub fn net_for_pin(&self, pin: &PinRef) -> Option<NetId> {
        self.pin_to_net.get(pin).copied()
    }

    pub fn net(&self, id: NetId) -> Option<&ResolvedNet> {
        self.nets.iter().find(|net| net.id == id)
    }
}

/// Resolve every wire into a net using union-find over pins.
///
/// Every pin of every part participates, even if it has no wire, so the result
/// always covers the complete document. Validation is responsible for warning
/// about those dangling pins.
pub fn resolve(document: &CircuitDocument) -> Result<NetMap, SchematicError> {
    let mut seen_parts = BTreeSet::new();
    for part in document.parts() {
        if !seen_parts.insert(part.id.clone()) {
            return Err(SchematicError::DuplicatePartId(part.id.clone()));
        }
    }

    let mut pins = Vec::new();
    for part in document.parts() {
        for pin_name in part.pin_names() {
            pins.push(PinRef::new(part.id.clone(), pin_name));
        }
    }

    let mut index: HashMap<PinRef, usize> = HashMap::with_capacity(pins.len());
    for (position, pin) in pins.iter().enumerate() {
        index.insert(pin.clone(), position);
    }

    let mut union_find = UnionFind::new(pins.len());
    for wire in document.wires() {
        let from = *index
            .get(&wire.from)
            .ok_or_else(|| pin_error(document, &wire.from))?;
        let to = *index
            .get(&wire.to)
            .ok_or_else(|| pin_error(document, &wire.to))?;
        union_find.union(from, to);
    }

    let mut groups: BTreeMap<usize, Vec<PinRef>> = BTreeMap::new();
    for (position, pin) in pins.iter().enumerate() {
        groups
            .entry(union_find.find(position))
            .or_default()
            .push(pin.clone());
    }

    let ground_parts: BTreeSet<PartId> = document
        .parts()
        .iter()
        .filter(|part| part.kind.is_ground())
        .map(|part| part.id.clone())
        .collect();

    let mut ground_pins = Vec::new();
    let mut signal_groups = Vec::new();
    for mut group in groups.into_values() {
        group.sort();
        if group
            .iter()
            .any(|pin| ground_parts.contains(&pin.part))
        {
            ground_pins.extend(group);
        } else {
            signal_groups.push(group);
        }
    }

    ground_pins.sort();
    signal_groups.sort_by(|left, right| left[0].cmp(&right[0]));

    let mut nets = Vec::new();
    let mut pin_to_net = BTreeMap::new();

    if !ground_pins.is_empty() {
        for pin in &ground_pins {
            pin_to_net.insert(pin.clone(), NetId::GROUND);
        }
        nets.push(ResolvedNet {
            id: NetId::GROUND,
            pins: ground_pins,
            is_ground: true,
        });
    }

    for (position, group) in signal_groups.into_iter().enumerate() {
        let id = NetId(position as u32 + 1);
        for pin in &group {
            pin_to_net.insert(pin.clone(), id);
        }
        nets.push(ResolvedNet {
            id,
            pins: group,
            is_ground: false,
        });
    }

    Ok(NetMap { nets, pin_to_net })
}

fn pin_error(document: &CircuitDocument, pin: &PinRef) -> SchematicError {
    match document.parts().iter().find(|part| part.id == pin.part) {
        None => SchematicError::UnknownPart(pin.part.clone()),
        Some(_) => SchematicError::UnknownPin {
            part: pin.part.clone(),
            pin: pin.pin.clone(),
        },
    }
}

struct UnionFind {
    parent: Vec<usize>,
    rank: Vec<u8>,
}

impl UnionFind {
    fn new(size: usize) -> Self {
        Self {
            parent: (0..size).collect(),
            rank: vec![0; size],
        }
    }

    fn find(&mut self, value: usize) -> usize {
        if self.parent[value] != value {
            self.parent[value] = self.find(self.parent[value]);
        }
        self.parent[value]
    }

    fn union(&mut self, left: usize, right: usize) {
        let left = self.find(left);
        let right = self.find(right);
        if left == right {
            return;
        }

        match self.rank[left].cmp(&self.rank[right]) {
            std::cmp::Ordering::Less => self.parent[left] = right,
            std::cmp::Ordering::Greater => self.parent[right] = left,
            std::cmp::Ordering::Equal => {
                self.parent[right] = left;
                self.rank[left] += 1;
            }
        }
    }
}

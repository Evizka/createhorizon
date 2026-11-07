use alloc::collections::{BTreeMap, BTreeSet};
use alloc::vec::Vec;
use core::convert::TryFrom;

use crate::{
    Arena, Axis, BlockPos, Fx, GearCoupling, GearSize, Gearwheel, Handle, Rat,
    additional_gear_offsets, coupling_between,
};

const FACE_OFFSETS: [[i32; 3]; 6] = [
    [-1, 0, 0],
    [0, -1, 0],
    [0, 0, -1],
    [0, 0, 1],
    [0, 1, 0],
    [1, 0, 0],
];

/// A closed set of rotational components. Balance values are data, not
/// trait-object allocations in the graph's hot path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KineticPart {
    Shaft {
        position: BlockPos,
        axis: Axis,
    },
    Cogwheel(Gearwheel),
    HandCrank {
        position: BlockPos,
        axis: Axis,
        speed: Fx,
        capacity: Fx,
    },
    Load {
        position: BlockPos,
        axis: Axis,
        impact: Fx,
    },
}

pub trait Rotational {
    fn position(&self) -> BlockPos;
    fn axis(&self) -> Axis;
}

pub trait KineticSource {
    fn source_speed(&self) -> Option<Fx>;
}

pub trait StressImpact {
    fn stress_impact(&self, speed: Fx) -> Fx;
}

pub trait StressCapacity {
    fn stress_capacity(&self, speed: Fx) -> Fx;
}

impl Rotational for KineticPart {
    fn position(&self) -> BlockPos {
        match self {
            Self::Shaft { position, .. }
            | Self::HandCrank { position, .. }
            | Self::Load { position, .. } => *position,
            Self::Cogwheel(wheel) => wheel.position,
        }
    }

    fn axis(&self) -> Axis {
        match self {
            Self::Shaft { axis, .. } | Self::HandCrank { axis, .. } | Self::Load { axis, .. } => {
                *axis
            }
            Self::Cogwheel(wheel) => wheel.axis,
        }
    }
}

impl KineticSource for KineticPart {
    fn source_speed(&self) -> Option<Fx> {
        match self {
            Self::HandCrank { speed, .. } => Some(*speed),
            _ => None,
        }
    }
}

impl StressImpact for KineticPart {
    fn stress_impact(&self, speed: Fx) -> Fx {
        match self {
            Self::Load { impact, .. } if !speed.is_zero() => *impact,
            _ => Fx::ZERO,
        }
    }
}

impl StressCapacity for KineticPart {
    fn stress_capacity(&self, speed: Fx) -> Fx {
        match self {
            Self::HandCrank { capacity, .. } if !speed.is_zero() => *capacity,
            _ => Fx::ZERO,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AddPartError {
    PositionOccupied,
    ArenaFull,
}

/// One directed edge. Travelling against `from -> to` uses `ratio.inverse()`;
/// rotation inversion is symmetric in either direction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KineticLink {
    pub from: Handle<KineticPart>,
    pub to: Handle<KineticPart>,
    pub coupling: GearCoupling,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct KineticState {
    pub handle: Option<Handle<KineticPart>>,
    pub position: BlockPos,
    pub speed: Fx,
    pub network: u32,
    pub overstressed: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NetworkSummary {
    pub part_count: usize,
    pub link_count: usize,
    pub network_count: u32,
    pub driven_parts: usize,
    pub overstressed_networks: u32,
    pub max_speed: Fx,
    pub stress: Fx,
    pub capacity: Fx,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Transform {
    ratio: Rat,
    inverted: bool,
}

/// Deterministic per-world kinetic graph. Positions use an ordered index;
/// solving and state output follow stable arena order, never hash iteration.
pub struct Kinetics {
    parts: Arena<KineticPart>,
    by_position: BTreeMap<BlockPos, Handle<KineticPart>>,
    links: Vec<KineticLink>,
    states: Vec<KineticState>,
}

impl Default for Kinetics {
    fn default() -> Self {
        Self::new()
    }
}

impl Kinetics {
    #[must_use]
    pub fn new() -> Self {
        Self {
            parts: Arena::new(),
            by_position: BTreeMap::new(),
            links: Vec::new(),
            states: Vec::new(),
        }
    }

    /// Builds in position order so slot assignment and tie-breaking do not
    /// depend on the order in which a schematic was discovered.
    #[must_use]
    pub fn from_parts(mut parts: Vec<KineticPart>) -> Self {
        parts.sort_by_key(|part| part.position());
        let mut kinetics = Self::new();
        for part in parts {
            let _ = kinetics.add_part(part);
        }
        kinetics
    }

    pub fn add_part(&mut self, part: KineticPart) -> Result<Handle<KineticPart>, AddPartError> {
        let position = part.position();
        if self.by_position.contains_key(&position) {
            return Err(AddPartError::PositionOccupied);
        }
        let handle = self
            .parts
            .insert(part)
            .map_err(|_| AddPartError::ArenaFull)?;
        self.by_position.insert(position, handle);
        self.links.clear();
        self.states.clear();
        Ok(handle)
    }

    pub fn remove_part(&mut self, handle: Handle<KineticPart>) -> Option<KineticPart> {
        let part = self.parts.remove(handle)?;
        self.by_position.remove(&part.position());
        self.links.clear();
        self.states.clear();
        Some(part)
    }

    #[must_use]
    pub fn part_at(&self, position: BlockPos) -> Option<(Handle<KineticPart>, &KineticPart)> {
        let handle = *self.by_position.get(&position)?;
        Some((handle, self.parts.get(handle)?))
    }

    pub fn parts(&self) -> impl Iterator<Item = (Handle<KineticPart>, &KineticPart)> + '_ {
        self.parts.iter()
    }

    #[must_use]
    pub fn links(&self) -> &[KineticLink] {
        &self.links
    }

    #[must_use]
    pub fn states(&self) -> &[KineticState] {
        &self.states
    }

    /// Rebuilds contacts, selects one strongest source per component, propagates
    /// exact rational speed ratios and applies the strict stress > capacity rule.
    pub fn solve(&mut self) -> NetworkSummary {
        self.rebuild_links();
        let nodes: Vec<_> = self
            .parts
            .iter()
            .map(|(handle, part)| (handle, *part))
            .collect();
        let count = nodes.len();
        let mut summary = NetworkSummary {
            part_count: count,
            link_count: self.links.len(),
            ..NetworkSummary::default()
        };
        self.states = nodes
            .iter()
            .map(|(handle, part)| KineticState {
                handle: Some(*handle),
                position: part.position(),
                ..KineticState::default()
            })
            .collect();
        if count == 0 {
            return summary;
        }

        let mut dense_by_slot = alloc::vec![None; self.parts.slot_count()];
        for (dense, (handle, _)) in nodes.iter().enumerate() {
            let index = usize::try_from(handle.index()).expect("u32 slot index fits usize");
            dense_by_slot[index] = Some(dense);
        }
        let mut adjacency: Vec<Vec<(usize, usize)>> = (0..count).map(|_| Vec::new()).collect();
        for (edge, link) in self.links.iter().enumerate() {
            let from_slot = usize::try_from(link.from.index()).expect("u32 slot index fits usize");
            let to_slot = usize::try_from(link.to.index()).expect("u32 slot index fits usize");
            let (Some(from), Some(to)) = (dense_by_slot[from_slot], dense_by_slot[to_slot]) else {
                continue;
            };
            adjacency[from].push((to, edge));
            adjacency[to].push((from, edge));
        }

        let mut visited = alloc::vec![false; count];
        let mut transforms: Vec<Option<Transform>> = alloc::vec![None; count];
        for root in 0..count {
            if visited[root] {
                continue;
            }
            summary.network_count = summary.network_count.saturating_add(1);
            let mut component = Vec::new();
            let mut queue = Vec::new();
            queue.push(root);
            visited[root] = true;
            let mut cursor = 0;
            while cursor < queue.len() {
                let current = queue[cursor];
                cursor += 1;
                component.push(current);
                for &(neighbor, _) in &adjacency[current] {
                    if !visited[neighbor] {
                        visited[neighbor] = true;
                        queue.push(neighbor);
                    }
                }
            }

            let Some(leader) = strongest_source(&component, &nodes) else {
                continue;
            };
            let source_speed = nodes[leader].1.source_speed().unwrap_or(Fx::ZERO);
            transforms[leader] = Some(Transform {
                ratio: Rat::ONE,
                inverted: false,
            });
            let mut path_queue = Vec::new();
            path_queue.push(leader);
            let mut path_cursor = 0;
            let mut inconsistent = false;
            while path_cursor < path_queue.len() {
                let current = path_queue[path_cursor];
                path_cursor += 1;
                let Some(current_transform) = transforms[current] else {
                    inconsistent = true;
                    continue;
                };
                for &(neighbor, edge_index) in &adjacency[current] {
                    let link = self.links[edge_index];
                    let edge_transform = if link.from == nodes[current].0 {
                        Transform {
                            ratio: link.coupling.ratio,
                            inverted: link.coupling.inverted,
                        }
                    } else {
                        let Some(ratio) = link.coupling.ratio.inverse() else {
                            inconsistent = true;
                            continue;
                        };
                        Transform {
                            ratio,
                            inverted: link.coupling.inverted,
                        }
                    };
                    let Some(ratio) = current_transform.ratio.checked_mul(edge_transform.ratio)
                    else {
                        inconsistent = true;
                        continue;
                    };
                    let expected = Transform {
                        ratio,
                        inverted: current_transform.inverted ^ edge_transform.inverted,
                    };
                    match transforms[neighbor] {
                        None => {
                            transforms[neighbor] = Some(expected);
                            path_queue.push(neighbor);
                        }
                        Some(existing) if existing != expected => inconsistent = true,
                        Some(_) => {}
                    }
                }
            }

            let mut network_stress = Fx::ZERO;
            let mut network_capacity = Fx::ZERO;
            let mut source_conflict = false;
            for &index in &component {
                let Some(transform) = transforms[index] else {
                    inconsistent = true;
                    continue;
                };
                let mut speed = transform.ratio.apply(source_speed);
                if transform.inverted {
                    speed = -speed;
                }
                if let Some(other_speed) = nodes[index].1.source_speed() {
                    if other_speed != speed {
                        source_conflict = true;
                    }
                }
                network_stress += nodes[index].1.stress_impact(speed.abs());
                network_capacity += nodes[index].1.stress_capacity(speed.abs());
                self.states[index].speed = speed;
                self.states[index].network = summary.network_count - 1;
            }
            let overstressed = inconsistent || source_conflict || network_stress > network_capacity;
            summary.stress += network_stress;
            summary.capacity += network_capacity;
            if overstressed {
                summary.overstressed_networks = summary.overstressed_networks.saturating_add(1);
                for &index in &component {
                    self.states[index].speed = Fx::ZERO;
                    self.states[index].overstressed = true;
                }
            } else {
                for &index in &component {
                    let speed = self.states[index].speed;
                    if !speed.is_zero() {
                        summary.driven_parts += 1;
                    }
                    summary.max_speed = summary.max_speed.max(speed.abs());
                }
            }
        }
        summary
    }

    /// Stable FNV-1a hash of positions, component kinds, propagated speed,
    /// network index, and overload state. All bytes are explicitly little-endian.
    #[must_use]
    pub fn state_hash(&self) -> u64 {
        let mut rows: Vec<_> = self
            .states
            .iter()
            .filter_map(|state| {
                let handle = state.handle?;
                let part = *self.parts.get(handle)?;
                Some((state.position, part, *state))
            })
            .collect();
        rows.sort_by_key(|(position, _, _)| *position);
        let mut hash = 0xcbf2_9ce4_8422_2325_u64;
        for (position, part, state) in rows {
            fn feed(hash: &mut u64, bytes: &[u8]) {
                for byte in bytes {
                    *hash ^= u64::from(*byte);
                    *hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
                }
            }
            for coordinate in [position.x, position.y, position.z] {
                feed(&mut hash, &coordinate.to_le_bytes());
            }
            let (kind, axis, size) = match part {
                KineticPart::Shaft { axis, .. } => (0_u8, axis_code(axis), 0_u8),
                KineticPart::Cogwheel(wheel) => (
                    1,
                    axis_code(wheel.axis),
                    match wheel.size {
                        GearSize::Small => 0,
                        GearSize::Large => 1,
                    },
                ),
                KineticPart::HandCrank { axis, .. } => (2, axis_code(axis), 0),
                KineticPart::Load { axis, .. } => (3, axis_code(axis), 0),
            };
            feed(&mut hash, &[kind, axis, size]);
            match part {
                KineticPart::HandCrank {
                    speed, capacity, ..
                } => {
                    feed(&mut hash, &speed.raw().to_le_bytes());
                    feed(&mut hash, &capacity.raw().to_le_bytes());
                }
                KineticPart::Load { impact, .. } => {
                    feed(&mut hash, &impact.raw().to_le_bytes());
                }
                KineticPart::Shaft { .. } | KineticPart::Cogwheel(_) => {}
            }
            feed(&mut hash, &state.speed.raw().to_le_bytes());
            feed(&mut hash, &state.network.to_le_bytes());
            feed(&mut hash, &[u8::from(state.overstressed)]);
        }
        hash
    }

    fn rebuild_links(&mut self) {
        self.links.clear();
        let mut candidates = Vec::new();
        for (handle, part) in self.parts.iter() {
            for offset in FACE_OFFSETS {
                candidates.push((handle, part.position(), offset));
            }
            if let KineticPart::Cogwheel(wheel) = part {
                for offset in additional_gear_offsets(*wheel) {
                    candidates.push((handle, part.position(), offset));
                }
            }
        }

        let mut unique = BTreeSet::new();
        for (source, position, offset) in candidates {
            let Some(target_position) = position.checked_offset(offset) else {
                continue;
            };
            let Some(&target) = self.by_position.get(&target_position) else {
                continue;
            };
            if source == target {
                continue;
            }
            let (from, to) = if source < target {
                (source, target)
            } else {
                (target, source)
            };
            if !unique.insert((from.index(), to.index())) {
                continue;
            }
            let (Some(from_part), Some(to_part)) = (self.parts.get(from), self.parts.get(to))
            else {
                continue;
            };
            if let Some(coupling) = coupling_for_parts(*from_part, *to_part) {
                self.links.push(KineticLink { from, to, coupling });
            }
        }
    }
}

const fn axis_code(axis: Axis) -> u8 {
    match axis {
        Axis::X => 0,
        Axis::Y => 1,
        Axis::Z => 2,
    }
}

fn strongest_source(
    component: &[usize],
    nodes: &[(Handle<KineticPart>, KineticPart)],
) -> Option<usize> {
    component
        .iter()
        .copied()
        .filter_map(|index| {
            let part = nodes[index].1;
            let speed = part.source_speed()?;
            let capacity = part.stress_capacity(speed.abs());
            let power = speed.abs() * capacity;
            Some((index, power, part.position()))
        })
        .max_by(|left, right| {
            left.1
                .cmp(&right.1)
                // Stable position is a deterministic tie-break independent of
                // the order in which blocks were discovered or inserted.
                .then_with(|| right.2.cmp(&left.2))
        })
        .map(|(index, _, _)| index)
}

fn coupling_for_parts(from: KineticPart, to: KineticPart) -> Option<GearCoupling> {
    if let (KineticPart::Cogwheel(from_wheel), KineticPart::Cogwheel(to_wheel)) = (from, to) {
        if let Some(coupling) = coupling_between(from_wheel, to_wheel) {
            return Some(coupling);
        }
    }

    let delta = from.position().delta_to(to.position());
    if from.axis() != to.axis() {
        return None;
    }
    let axis = from.axis();
    if axis.component(delta).unsigned_abs() != 1
        || axis
            .other_axes()
            .into_iter()
            .any(|other| other.component(delta) != 0)
    {
        return None;
    }
    Some(GearCoupling {
        ratio: Rat::ONE,
        inverted: false,
    })
}

#[cfg(test)]
mod tests {
    use alloc::vec;

    use super::{AddPartError, KineticPart, Kinetics};
    use crate::{Axis, BlockPos, Fx, GearSize, Gearwheel};

    fn pos(x: i32, y: i32, z: i32) -> BlockPos {
        BlockPos::new(x, y, z)
    }

    fn crank(position: BlockPos, speed: i32, capacity: i32) -> KineticPart {
        KineticPart::HandCrank {
            position,
            axis: Axis::Z,
            speed: Fx::from_i32(speed),
            capacity: Fx::from_i32(capacity),
        }
    }

    fn shaft(position: BlockPos) -> KineticPart {
        KineticPart::Shaft {
            position,
            axis: Axis::Z,
        }
    }

    fn gear(position: BlockPos, size: GearSize) -> KineticPart {
        KineticPart::Cogwheel(Gearwheel::new(position, Axis::Z, size))
    }

    fn state_at(kinetics: &Kinetics, position: BlockPos) -> super::KineticState {
        *kinetics
            .states()
            .iter()
            .find(|state| state.position == position)
            .unwrap()
    }

    #[test]
    fn crank_speed_propagates_through_shaft_and_gears_exactly() {
        let parts = vec![
            crank(pos(0, 0, 0), 16, 16),
            shaft(pos(0, 0, 1)),
            gear(pos(0, 0, 2), GearSize::Large),
            gear(pos(1, 1, 2), GearSize::Small),
        ];
        let mut kinetics = Kinetics::from_parts(parts);
        let summary = kinetics.solve();
        assert_eq!(summary.part_count, 4);
        assert_eq!(summary.link_count, 3);
        assert_eq!(summary.network_count, 1);
        assert_eq!(summary.driven_parts, 4);
        assert_eq!(summary.max_speed, Fx::from_i32(32));
        assert_eq!(state_at(&kinetics, pos(0, 0, 0)).speed, Fx::from_i32(16));
        assert_eq!(state_at(&kinetics, pos(1, 1, 2)).speed, Fx::from_i32(-32));
        assert_eq!(summary.overstressed_networks, 0);
    }

    #[test]
    fn stress_equal_to_capacity_is_operational_but_greater_stops_network() {
        let parts = vec![
            crank(pos(0, 0, 0), 16, 16),
            KineticPart::Load {
                position: pos(0, 0, 1),
                axis: Axis::Z,
                impact: Fx::from_i32(16),
            },
        ];
        let mut equal = Kinetics::from_parts(parts.clone());
        let equal_summary = equal.solve();
        assert_eq!(equal_summary.stress, Fx::from_i32(16));
        assert_eq!(equal_summary.capacity, Fx::from_i32(16));
        assert_eq!(equal_summary.overstressed_networks, 0);

        let over = vec![
            crank(pos(0, 0, 0), 16, 16),
            KineticPart::Load {
                position: pos(0, 0, 1),
                axis: Axis::Z,
                impact: Fx::from_i32(17),
            },
        ];
        let mut overstressed = Kinetics::from_parts(over);
        let over_summary = overstressed.solve();
        assert_eq!(over_summary.overstressed_networks, 1);
        assert_eq!(over_summary.driven_parts, 0);
        assert!(
            overstressed
                .states()
                .iter()
                .all(|state| { state.speed == Fx::ZERO && state.overstressed })
        );
    }

    #[test]
    fn insertion_order_does_not_change_sorted_build_or_state_hash() {
        let parts = vec![
            crank(pos(4, 0, 0), 16, 16),
            crank(pos(0, 0, 0), 16, 16),
            shaft(pos(1, 0, 0)),
        ];
        let mut forward = Kinetics::from_parts(parts.clone());
        let mut reversed_parts = parts;
        reversed_parts.reverse();
        let mut reversed = Kinetics::from_parts(reversed_parts);
        assert_eq!(forward.solve(), reversed.solve());
        assert_eq!(forward.state_hash(), reversed.state_hash());
    }

    #[test]
    fn duplicate_positions_are_rejected_without_corrupting_the_index() {
        let mut kinetics = Kinetics::new();
        let position = pos(0, 0, 0);
        assert!(kinetics.add_part(shaft(position)).is_ok());
        assert_eq!(
            kinetics.add_part(crank(position, 16, 16)),
            Err(AddPartError::PositionOccupied)
        );
        assert_eq!(kinetics.parts().count(), 1);
    }
}

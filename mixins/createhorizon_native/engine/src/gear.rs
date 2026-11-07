use alloc::vec::Vec;

use crate::Rat;

use crate::{Axis, BlockPos};

/// The two cogwheel sizes modeled by Create's standard gear contacts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum GearSize {
    Small,
    Large,
}

/// The position, axle axis, and size needed to resolve a gear-to-gear contact.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Gearwheel {
    pub position: BlockPos,
    pub axis: Axis,
    pub size: GearSize,
}

impl Gearwheel {
    #[must_use]
    pub const fn new(position: BlockPos, axis: Axis, size: GearSize) -> Self {
        Self {
            position,
            axis,
            size,
        }
    }
}

/// Directed transmission from the first gear to the second.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GearCoupling {
    /// Absolute speed ratio. Direction reversal is represented independently.
    pub ratio: Rat,
    pub inverted: bool,
}

/// Resolve Create's standard cogwheel geometry for two gear blocks.
///
/// This function is symmetric as a contact test. `ratio` is directed: a large
/// gear drives a small one at 2:1, while the reverse traversal is 1:2.
///
/// The geometry follows Create's `RotationPropagator` rules:
/// - small/small gears mesh face-adjacent, with parallel axles;
/// - large/small gears mesh diagonally in the plane perpendicular to their
///   shared axle, with a 2:1 ratio;
/// - large/large gears mesh diagonally in the plane spanned by their distinct
///   axle axes, with direction determined by the signs along both axes.
#[must_use]
pub fn coupling_between(from: Gearwheel, to: Gearwheel) -> Option<GearCoupling> {
    let delta = from.position.delta_to(to.position);
    let manhattan = delta[0].unsigned_abs() + delta[1].unsigned_abs() + delta[2].unsigned_abs();
    if manhattan == 0 {
        return None;
    }

    match (from.size, to.size) {
        (GearSize::Small, GearSize::Small) => {
            if from.axis != to.axis || manhattan != 1 || from.axis.component(delta) != 0 {
                return None;
            }
            Some(GearCoupling {
                ratio: Rat::ONE,
                inverted: true,
            })
        }
        (GearSize::Large, GearSize::Large) => {
            if from.axis == to.axis {
                return None;
            }
            let source_delta = from.axis.component(delta);
            let target_delta = to.axis.component(delta);
            let remaining_axis = from
                .axis
                .other_axes()
                .into_iter()
                .find(|axis| *axis != to.axis)?;
            let remaining_delta = remaining_axis.component(delta);
            if source_delta.unsigned_abs() != 1
                || target_delta.unsigned_abs() != 1
                || remaining_delta != 0
            {
                return None;
            }
            Some(GearCoupling {
                ratio: Rat::ONE,
                inverted: (source_delta > 0) != (target_delta > 0),
            })
        }
        (GearSize::Large, GearSize::Small) | (GearSize::Small, GearSize::Large) => {
            if from.axis != to.axis || from.axis.component(delta) != 0 {
                return None;
            }
            let [first_axis, second_axis] = from.axis.other_axes();
            if first_axis.component(delta).unsigned_abs() != 1
                || second_axis.component(delta).unsigned_abs() != 1
            {
                return None;
            }
            let ratio = match from.size {
                GearSize::Large => Rat::TWO,
                GearSize::Small => Rat::HALF,
            };
            Some(GearCoupling {
                ratio,
                inverted: true,
            })
        }
    }
}

/// Additional diagonal positions to inspect beyond the six face neighbors.
/// The returned `Vec` is produced in a fixed lexicographic loop order, so it
/// is stable across targets and independent of hash iteration.
#[must_use]
pub fn additional_gear_offsets(gear: Gearwheel) -> Vec<[i32; 3]> {
    let mut offsets = Vec::with_capacity(match gear.size {
        GearSize::Small => 4,
        GearSize::Large => 12,
    });
    for x in -1..=1 {
        for y in -1..=1 {
            for z in -1..=1 {
                let non_zero = u8::from(x != 0) + u8::from(y != 0) + u8::from(z != 0);
                if non_zero != 2 {
                    continue;
                }
                let offset = [i64::from(x), i64::from(y), i64::from(z)];
                if gear.size == GearSize::Small && gear.axis.component(offset) != 0 {
                    continue;
                }
                offsets.push([x, y, z]);
            }
        }
    }
    offsets
}

#[cfg(test)]
mod tests {
    use super::{GearSize, Gearwheel, additional_gear_offsets, coupling_between};
    use crate::Rat;
    use crate::{Axis, BlockPos};

    fn gear(position: [i32; 3], axis: Axis, size: GearSize) -> Gearwheel {
        Gearwheel::new(
            BlockPos::new(position[0], position[1], position[2]),
            axis,
            size,
        )
    }

    #[test]
    fn small_gears_mesh_only_face_to_face_on_parallel_axles() {
        let source = gear([0, 0, 0], Axis::Z, GearSize::Small);
        let target = gear([1, 0, 0], Axis::Z, GearSize::Small);
        assert_eq!(
            coupling_between(source, target),
            Some(super::GearCoupling {
                ratio: Rat::ONE,
                inverted: true,
            })
        );
        assert_eq!(
            coupling_between(source, gear([0, 0, 1], Axis::Z, GearSize::Small)),
            None
        );
        assert_eq!(
            coupling_between(source, gear([1, 0, 0], Axis::Y, GearSize::Small)),
            None
        );
        assert_eq!(
            coupling_between(source, gear([1, 1, 0], Axis::Z, GearSize::Small)),
            None
        );
    }

    #[test]
    fn large_small_contact_uses_exact_directed_two_to_one_ratio() {
        let large = gear([0, 0, 0], Axis::Z, GearSize::Large);
        let small = gear([1, 1, 0], Axis::Z, GearSize::Small);
        assert_eq!(coupling_between(large, small).unwrap().ratio, Rat::TWO);
        assert!(coupling_between(large, small).unwrap().inverted);
        assert_eq!(coupling_between(small, large).unwrap().ratio, Rat::HALF);
        assert!(coupling_between(small, large).unwrap().inverted);
        assert_eq!(
            coupling_between(large, gear([1, 0, 1], Axis::Z, GearSize::Small)),
            None
        );
        assert_eq!(
            coupling_between(large, gear([2, 1, 0], Axis::Z, GearSize::Small)),
            None
        );
    }

    #[test]
    fn large_gears_mesh_on_distinct_axes_with_sign_sensitive_rotation() {
        let x_gear = gear([0, 0, 0], Axis::X, GearSize::Large);
        let same_sign = gear([1, 1, 0], Axis::Y, GearSize::Large);
        let opposite_sign = gear([1, -1, 0], Axis::Y, GearSize::Large);
        assert_eq!(coupling_between(x_gear, same_sign).unwrap().ratio, Rat::ONE);
        assert!(!coupling_between(x_gear, same_sign).unwrap().inverted);
        assert!(coupling_between(x_gear, opposite_sign).unwrap().inverted);
        assert_eq!(
            coupling_between(x_gear, gear([1, 1, 0], Axis::X, GearSize::Large)),
            None
        );
        assert_eq!(
            coupling_between(x_gear, gear([1, 0, 0], Axis::Y, GearSize::Large)),
            None
        );
        assert_eq!(
            coupling_between(x_gear, gear([1, 1, 1], Axis::Y, GearSize::Large)),
            None
        );
    }

    #[test]
    fn diagonal_candidate_offsets_match_cog_size_and_are_deterministic() {
        let small = gear([0, 0, 0], Axis::Z, GearSize::Small);
        let large = gear([0, 0, 0], Axis::Z, GearSize::Large);
        let small_offsets = additional_gear_offsets(small);
        let large_offsets = additional_gear_offsets(large);
        assert_eq!(small_offsets.len(), 4);
        assert_eq!(large_offsets.len(), 12);
        assert_eq!(small_offsets, additional_gear_offsets(small));
        assert!(small_offsets.iter().all(|offset| offset[2] == 0));
        assert!(large_offsets.iter().all(|offset| {
            u8::from(offset[0] != 0) + u8::from(offset[1] != 0) + u8::from(offset[2] != 0) == 2
        }));
        for (index, offset) in large_offsets.iter().enumerate() {
            assert!(!large_offsets[..index].contains(offset));
        }
    }
}

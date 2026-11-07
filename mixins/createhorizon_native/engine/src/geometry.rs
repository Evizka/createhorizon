/// Integer axis-aligned block position.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BlockPos {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl BlockPos {
    pub const ZERO: Self = Self { x: 0, y: 0, z: 0 };

    #[must_use]
    pub const fn new(x: i32, y: i32, z: i32) -> Self {
        Self { x, y, z }
    }

    /// Delta from this position to `other`, widened before subtraction so the
    /// result cannot overflow for any pair of i32 coordinates.
    #[must_use]
    pub fn delta_to(self, other: Self) -> [i64; 3] {
        [
            i64::from(other.x) - i64::from(self.x),
            i64::from(other.y) - i64::from(self.y),
            i64::from(other.z) - i64::from(self.z),
        ]
    }

    #[must_use]
    pub fn checked_offset(self, delta: [i32; 3]) -> Option<Self> {
        let x = self.x.checked_add(delta[0])?;
        let y = self.y.checked_add(delta[1])?;
        let z = self.z.checked_add(delta[2])?;
        Some(Self { x, y, z })
    }
}

/// World-space cardinal axis; the sign of a direction is carried by offsets,
/// not by the axis itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Axis {
    X,
    Y,
    Z,
}

impl Axis {
    /// The three axes in a fixed order, for the code that must iterate them.
    pub const ALL: [Self; 3] = [Self::X, Self::Y, Self::Z];

    #[must_use]
    pub const fn index(self) -> usize {
        match self {
            Self::X => 0,
            Self::Y => 1,
            Self::Z => 2,
        }
    }

    #[must_use]
    pub const fn component(self, vector: [i64; 3]) -> i64 {
        match self {
            Self::X => vector[0],
            Self::Y => vector[1],
            Self::Z => vector[2],
        }
    }

    #[must_use]
    pub const fn other_axes(self) -> [Self; 2] {
        match self {
            Self::X => [Self::Y, Self::Z],
            Self::Y => [Self::X, Self::Z],
            Self::Z => [Self::X, Self::Y],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::BlockPos;

    #[test]
    fn deltas_widen_before_subtracting() {
        assert_eq!(
            BlockPos::new(i32::MIN, 0, i32::MAX).delta_to(BlockPos::new(i32::MAX, 1, i32::MIN)),
            [i64::from(u32::MAX), 1, -i64::from(u32::MAX)]
        );
    }

    #[test]
    fn checked_offset_rejects_coordinate_overflow() {
        assert_eq!(
            BlockPos::ZERO.checked_offset([1, -2, 3]),
            Some(BlockPos::new(1, -2, 3))
        );
        assert_eq!(
            BlockPos::new(i32::MAX, 0, 0).checked_offset([1, 0, 0]),
            None
        );
    }
}

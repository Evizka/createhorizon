use core::ops::{Add, Neg, Sub};

use crate::Fx;

/// Three-dimensional vector in Q32.32 world units.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Vec3 {
    pub x: Fx,
    pub y: Fx,
    pub z: Fx,
}

impl Vec3 {
    pub const ZERO: Self = Self {
        x: Fx::ZERO,
        y: Fx::ZERO,
        z: Fx::ZERO,
    };

    #[must_use]
    pub const fn new(x: Fx, y: Fx, z: Fx) -> Self {
        Self { x, y, z }
    }

    #[must_use]
    pub fn dot(self, rhs: Self) -> Fx {
        self.x * rhs.x + self.y * rhs.y + self.z * rhs.z
    }

    #[must_use]
    pub fn cross(self, rhs: Self) -> Self {
        Self {
            x: self.y * rhs.z - self.z * rhs.y,
            y: self.z * rhs.x - self.x * rhs.z,
            z: self.x * rhs.y - self.y * rhs.x,
        }
    }

    #[must_use]
    pub fn length_squared(self) -> Fx {
        self.dot(self)
    }

    #[must_use]
    pub fn length(self) -> Fx {
        self.length_squared().sqrt()
    }

    #[must_use]
    pub fn normalized(self) -> Option<Self> {
        let length = self.length();
        if length.is_zero() {
            return None;
        }
        Some(Self {
            x: self.x.checked_div(length)?,
            y: self.y.checked_div(length)?,
            z: self.z.checked_div(length)?,
        })
    }

    #[must_use]
    pub fn scale(self, amount: Fx) -> Self {
        Self {
            x: self.x * amount,
            y: self.y * amount,
            z: self.z * amount,
        }
    }
}

impl Add for Vec3 {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
            z: self.z + rhs.z,
        }
    }
}

impl Sub for Vec3 {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
            z: self.z - rhs.z,
        }
    }
}

impl Neg for Vec3 {
    type Output = Self;

    fn neg(self) -> Self::Output {
        Self {
            x: -self.x,
            y: -self.y,
            z: -self.z,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Vec3;
    use crate::Fx;

    #[test]
    fn integer_vector_geometry_has_fixed_raw_results() {
        let x = Vec3::new(Fx::ONE, Fx::ZERO, Fx::ZERO);
        let y = Vec3::new(Fx::ZERO, Fx::ONE, Fx::ZERO);
        assert_eq!(x.dot(y), Fx::ZERO);
        assert_eq!(x.cross(y), Vec3::new(Fx::ZERO, Fx::ZERO, Fx::ONE));
        assert_eq!(
            Vec3::new(Fx::from_i32(3), Fx::from_i32(4), Fx::ZERO).length(),
            Fx::from_i32(5)
        );
    }
}

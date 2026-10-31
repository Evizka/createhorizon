use core::convert::TryFrom;

use crate::Fx;

/// A reduced exact rational value with a signed numerator and positive
/// denominator. Ratios remain exact until applied to fixed-point values.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Rat {
    numerator: i64,
    denominator: u64,
}

impl Rat {
    pub const ZERO: Self = Self {
        numerator: 0,
        denominator: 1,
    };
    pub const ONE: Self = Self {
        numerator: 1,
        denominator: 1,
    };
    pub const HALF: Self = Self {
        numerator: 1,
        denominator: 2,
    };
    pub const TWO: Self = Self {
        numerator: 2,
        denominator: 1,
    };

    /// Creates a reduced rational; returns `None` for a zero denominator or a
    /// normalized numerator that cannot fit in `i64`.
    #[must_use]
    pub fn new(numerator: i64, denominator: i64) -> Option<Self> {
        Self::from_wide(i128::from(numerator), i128::from(denominator))
    }

    #[must_use]
    pub const fn integer(value: i64) -> Self {
        Self {
            numerator: value,
            denominator: 1,
        }
    }

    #[must_use]
    pub const fn numerator(self) -> i64 {
        self.numerator
    }

    #[must_use]
    pub const fn denominator(self) -> u64 {
        self.denominator
    }

    #[must_use]
    pub const fn is_zero(self) -> bool {
        self.numerator == 0
    }

    #[must_use]
    pub fn inverse(self) -> Option<Self> {
        if self.numerator == 0 {
            return None;
        }
        Self::from_wide(i128::from(self.denominator), i128::from(self.numerator))
    }

    #[must_use]
    pub fn checked_mul(self, rhs: Self) -> Option<Self> {
        let numerator = i128::from(self.numerator) * i128::from(rhs.numerator);
        let denominator = i128::from(self.denominator)
            .checked_mul(i128::from(rhs.denominator))?;
        Self::from_wide(numerator, denominator)
    }

    #[must_use]
    pub fn checked_div(self, rhs: Self) -> Option<Self> {
        self.checked_mul(rhs.inverse()?)
    }

    /// Applies this exact ratio to an Fx value, rounding only once at the
    /// Q32.32 boundary with ties away from zero.
    #[must_use]
    pub fn apply(self, value: Fx) -> Fx {
        let numerator = i128::from(value.raw()) * i128::from(self.numerator);
        Fx::from_raw_ratio(numerator, i128::from(self.denominator))
    }

    fn from_wide(mut numerator: i128, mut denominator: i128) -> Option<Self> {
        if denominator == 0 {
            return None;
        }
        if denominator < 0 {
            numerator = numerator.checked_neg()?;
            denominator = denominator.checked_neg()?;
        }

        let denominator_magnitude = u128::try_from(denominator).ok()?;
        let divisor = gcd(numerator.unsigned_abs(), denominator_magnitude);
        let divisor = i128::try_from(divisor).ok()?;
        numerator /= divisor;
        denominator /= divisor;
        Some(Self {
            numerator: i64::try_from(numerator).ok()?,
            denominator: u64::try_from(denominator).ok()?,
        })
    }
}

fn gcd(mut left: u128, mut right: u128) -> u128 {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left
}

#[cfg(test)]
mod tests {
    use super::Rat;
    use crate::Fx;

    #[test]
    fn ratios_are_reduced_and_sign_is_normalized() {
        assert_eq!(Rat::new(12, -18), Some(Rat::new(-2, 3).unwrap()));
        assert_eq!(Rat::new(0, -7), Some(Rat::ZERO));
        assert_eq!(Rat::new(1, 0), None);
    }

    #[test]
    fn gear_ratios_compose_exactly_and_reverse() {
        let two_to_one = Rat::TWO;
        let one_to_two = two_to_one.inverse().unwrap();
        assert_eq!(one_to_two, Rat::HALF);
        assert_eq!(two_to_one.checked_mul(one_to_two), Some(Rat::ONE));
        assert_eq!(Rat::new(3, 2).unwrap().checked_mul(Rat::new(2, 3).unwrap()), Some(Rat::ONE));
    }

    #[test]
    fn ratio_applies_with_one_deterministic_rounding() {
        assert_eq!(Rat::new(1, 3).unwrap().apply(Fx::ONE).raw(), 0x0000_0000_5555_5555);
        assert_eq!(Rat::new(-1, 3).unwrap().apply(Fx::ONE).raw(), -0x0000_0000_5555_5555);
        assert_eq!(Rat::new(2, 1).unwrap().apply(Fx::from_raw(i64::MAX)).raw(), i64::MAX);
    }
}

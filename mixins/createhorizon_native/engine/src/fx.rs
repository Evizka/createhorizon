use core::convert::TryFrom;
use core::ops::{Add, AddAssign, Div, Mul, Neg, Sub, SubAssign};

/// Signed Q32.32 fixed-point value.
///
/// `raw / 2^32` is the represented number. The representable interval is
/// `[-2^31, 2^31 - 2^-32]`.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Fx(i64);

impl Fx {
    pub const FRAC_BITS: u32 = 32;
    pub const SCALE: i64 = 1_i64 << Self::FRAC_BITS;
    pub const ZERO: Self = Self(0);
    pub const ONE: Self = Self(Self::SCALE);
    pub const HALF: Self = Self(Self::SCALE / 2);
    pub const MIN: Self = Self(i64::MIN);
    pub const MAX: Self = Self(i64::MAX);

    #[must_use]
    pub const fn from_raw(raw: i64) -> Self {
        Self(raw)
    }

    #[must_use]
    pub const fn raw(self) -> i64 {
        self.0
    }

    #[must_use]
    #[allow(clippy::cast_lossless)] // `From` is not const; preserve const construction.
    pub const fn from_i32(value: i32) -> Self {
        Self((value as i64) << Self::FRAC_BITS)
    }

    /// Converts an integer, saturating outside the Q32.32 integer range.
    #[must_use]
    pub const fn from_i64(value: i64) -> Self {
        if value > (i64::MAX >> Self::FRAC_BITS) {
            Self::MAX
        } else if value < (i64::MIN >> Self::FRAC_BITS) {
            Self::MIN
        } else {
            Self(value << Self::FRAC_BITS)
        }
    }

    /// Converts an exact integer fraction, rounded to nearest, ties away from zero.
    /// A zero denominator saturates by the numerator's sign; `0 / 0` maps to zero.
    #[must_use]
    pub fn from_ratio(numerator: i64, denominator: i64) -> Self {
        if denominator == 0 {
            return Self::saturating_by_sign(i128::from(numerator));
        }
        let scaled = i128::from(numerator) << Self::FRAC_BITS;
        let Some(raw) = div_round_away(scaled, i128::from(denominator)) else {
            return Self::saturating_by_sign(i128::from(numerator));
        };
        Self::saturating_from_i128(raw)
    }

    #[must_use]
    pub const fn is_zero(self) -> bool {
        self.0 == 0
    }

    #[must_use]
    pub const fn signum(self) -> i8 {
        if self.0 > 0 {
            1
        } else if self.0 < 0 {
            -1
        } else {
            0
        }
    }

    #[must_use]
    pub const fn saturating_add(self, rhs: Self) -> Self {
        Self(self.0.saturating_add(rhs.0))
    }

    #[must_use]
    pub const fn saturating_sub(self, rhs: Self) -> Self {
        Self(self.0.saturating_sub(rhs.0))
    }

    /// Q32.32 multiplication with round-to-nearest, ties-away-from-zero.
    #[must_use]
    pub fn saturating_mul(self, rhs: Self) -> Self {
        let product = i128::from(self.0) * i128::from(rhs.0);
        let raw =
            div_round_away(product, i128::from(Self::SCALE)).expect("Q32.32 scale is non-zero");
        Self::saturating_from_i128(raw)
    }

    /// Q32.32 division with round-to-nearest, ties-away-from-zero.
    /// A zero divisor saturates by the dividend's sign; `0 / 0` maps to zero.
    #[must_use]
    pub fn saturating_div(self, rhs: Self) -> Self {
        if rhs.0 == 0 {
            return Self::saturating_by_sign(i128::from(self.0));
        }
        let numerator = i128::from(self.0) << Self::FRAC_BITS;
        let raw = div_round_away(numerator, i128::from(rhs.0))
            .expect("the divisor was checked to be non-zero");
        Self::saturating_from_i128(raw)
    }

    /// Returns `None` on division by zero; otherwise uses the same deterministic
    /// rounding as [`Self::saturating_div`].
    #[must_use]
    pub fn checked_div(self, rhs: Self) -> Option<Self> {
        if rhs.0 == 0 {
            return None;
        }
        Some(self.saturating_div(rhs))
    }

    #[must_use]
    pub const fn saturating_neg(self) -> Self {
        Self(self.0.saturating_neg())
    }

    #[must_use]
    pub const fn abs(self) -> Self {
        if self.0 == i64::MIN {
            Self::MAX
        } else if self.0 < 0 {
            Self(-self.0)
        } else {
            self
        }
    }

    /// Integer square root in Q32.32, rounded to nearest (ties upward).
    /// Negative inputs saturate to zero rather than producing NaN.
    #[must_use]
    pub fn sqrt(self) -> Self {
        if self.0 <= 0 {
            return Self::ZERO;
        }
        let positive_raw = u64::try_from(self.0).expect("positive raw value fits u64");
        let radicand = u128::from(positive_raw) << Self::FRAC_BITS;
        let lower = integer_sqrt(radicand);
        let upper = lower + 1;
        let lower_error = radicand - lower * lower;
        let upper_error = upper * upper - radicand;
        let rounded = if upper_error <= lower_error {
            upper
        } else {
            lower
        };
        Self::saturating_from_i128(
            i128::try_from(rounded).expect("fixed-point square root fits i128"),
        )
    }

    pub(crate) fn from_raw_ratio(numerator: i128, denominator: i128) -> Self {
        if denominator == 0 {
            return Self::saturating_by_sign(numerator);
        }
        let raw = div_round_away(numerator, denominator)
            .expect("the denominator was checked to be non-zero");
        Self::saturating_from_i128(raw)
    }

    pub(crate) fn saturating_from_i128(value: i128) -> Self {
        if value > i128::from(i64::MAX) {
            Self::MAX
        } else if value < i128::from(i64::MIN) {
            Self::MIN
        } else {
            Self(i64::try_from(value).expect("value was checked to fit i64"))
        }
    }

    fn saturating_by_sign(value: i128) -> Self {
        if value > 0 {
            Self::MAX
        } else if value < 0 {
            Self::MIN
        } else {
            Self::ZERO
        }
    }
}

/// Divide and round to nearest, with exact halfway cases away from zero.
pub(crate) fn div_round_away(numerator: i128, denominator: i128) -> Option<i128> {
    if denominator == 0 {
        return None;
    }
    let negative = (numerator < 0) ^ (denominator < 0);
    let magnitude = numerator.unsigned_abs();
    let divisor = denominator.unsigned_abs();
    let mut quotient = magnitude / divisor;
    let remainder = magnitude % divisor;
    let round_threshold = divisor.div_ceil(2);
    if remainder >= round_threshold {
        quotient = quotient.saturating_add(1);
    }

    if negative {
        let min_magnitude = 1_u128 << 127;
        if quotient >= min_magnitude {
            Some(i128::MIN)
        } else {
            Some(-i128::try_from(quotient).expect("magnitude is below i128::MIN magnitude"))
        }
    } else if quotient > i128::MAX.unsigned_abs() {
        Some(i128::MAX)
    } else {
        Some(i128::try_from(quotient).expect("magnitude was checked to fit i128"))
    }
}

fn integer_sqrt(value: u128) -> u128 {
    if value < 2 {
        return value;
    }
    let bit_len = 128 - value.leading_zeros();
    let mut estimate = 1_u128 << bit_len.div_ceil(2);
    loop {
        let next = (estimate + value / estimate) / 2;
        if next >= estimate {
            return estimate;
        }
        estimate = next;
    }
}

impl Add for Fx {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        self.saturating_add(rhs)
    }
}

impl AddAssign for Fx {
    fn add_assign(&mut self, rhs: Self) {
        *self = self.saturating_add(rhs);
    }
}

impl Sub for Fx {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        self.saturating_sub(rhs)
    }
}

impl SubAssign for Fx {
    fn sub_assign(&mut self, rhs: Self) {
        *self = self.saturating_sub(rhs);
    }
}

impl Mul for Fx {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        self.saturating_mul(rhs)
    }
}

impl Div for Fx {
    type Output = Self;

    fn div(self, rhs: Self) -> Self::Output {
        self.saturating_div(rhs)
    }
}

impl Neg for Fx {
    type Output = Self;

    fn neg(self) -> Self::Output {
        self.saturating_neg()
    }
}

#[cfg(test)]
mod tests {
    use super::Fx;

    #[test]
    fn q32_32_integer_and_fraction_golden_raw_bits() {
        assert_eq!(Fx::from_i32(1).raw(), 0x0000_0001_0000_0000);
        assert_eq!(Fx::from_i32(-2).raw(), -0x0000_0002_0000_0000);
        assert_eq!(Fx::from_ratio(1, 3).raw(), 0x0000_0000_5555_5555);
        assert_eq!(Fx::from_ratio(-1, 3).raw(), -0x0000_0000_5555_5555);
    }

    #[test]
    fn multiplication_and_division_round_half_away_from_zero() {
        // A product that lands exactly on half a raw unit keeps its unit:
        // 2^-32 * 2^-1 is 2^-33, which rounds away from zero to 2^-32.
        let half_raw = Fx::from_raw(1) * Fx::from_raw(1_i64 << 31);
        let negative_half_raw = Fx::from_raw(-1) * Fx::from_raw(1_i64 << 31);
        assert_eq!(half_raw.raw(), 1);
        assert_eq!(negative_half_raw.raw(), -1);

        // Exact quotients stay exact: one raw unit over two is half a unit,
        // which is 2^31 raw, not one raw.
        assert_eq!((Fx::from_raw(1) / Fx::from_raw(2)).raw(), 1_i64 << 31);
        assert_eq!((Fx::from_raw(-1) / Fx::from_raw(2)).raw(), -(1_i64 << 31));

        // The same halfway case as the product above, on the division side:
        // 2^32 / 2^33 is a quotient of zero with a remainder of half the
        // divisor, so it rounds away from zero to one raw unit.
        assert_eq!((Fx::from_raw(1) / Fx::from_raw(1_i64 << 33)).raw(), 1);
        assert_eq!((Fx::from_raw(-1) / Fx::from_raw(1_i64 << 33)).raw(), -1);
    }

    #[test]
    fn overflow_saturates_and_zero_division_is_defined() {
        assert_eq!((Fx::MAX + Fx::ONE).raw(), i64::MAX);
        assert_eq!((Fx::MIN - Fx::ONE).raw(), i64::MIN);
        assert_eq!((-Fx::MIN).raw(), i64::MAX);
        assert_eq!((Fx::ONE / Fx::ZERO).raw(), i64::MAX);
        assert_eq!((Fx::ZERO / Fx::ZERO).raw(), 0);
        assert!(Fx::ONE.checked_div(Fx::ZERO).is_none());
    }

    #[test]
    fn integer_square_root_has_stable_raw_result() {
        assert_eq!(Fx::from_i32(4).sqrt().raw(), Fx::from_i32(2).raw());
        assert_eq!(Fx::from_i32(2).sqrt().raw(), 6_074_001_000);
        assert_eq!(Fx::from_i32(-1).sqrt(), Fx::ZERO);
    }
}

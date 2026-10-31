use core::convert::TryFrom;

use crate::fx::div_round_away;
use crate::Fx;

const QUARTER_TURN: u32 = 1_u32 << 30;
const HALF_TURN: u32 = 1_u32 << 31;
const THREE_QUARTERS_TURN: u32 = 3_u32 << 30;
const QUARTER_MASK: u32 = QUARTER_TURN - 1;
const CORDIC_SCALE: i128 = 1_i128 << 32;
const HALF_TURN_Q64: i128 = 1_i128 << 63;

// atan(2^-i) expressed as Q0.64 turns. Constants are rounded once, offline;
// runtime math uses only integer operations.
const CORDIC_ATAN_TURNS_Q64: [i128; 32] = [
    2_305_843_009_213_693_952,
    1_361_218_612_134_873_190,
    719_230_530_580_881_038,
    365_092_647_525_521_947,
    183_254_791_493_294_829,
    91_716_730_292_036_216,
    45_869_556_482_713_130,
    22_936_177_926_750_895,
    11_468_263_948_075_831,
    5_734_153_847_876_408,
    2_867_079_658_191_483,
    1_433_540_170_878_135,
    716_770_128_161_890,
    358_385_069_421_298,
    179_192_535_378_193,
    89_596_267_772_540,
    44_798_133_896_700,
    22_399_066_949_654,
    11_199_533_474_990,
    5_599_766_737_515,
    2_799_883_368_760,
    1_399_941_684_380,
    699_970_842_190,
    349_985_421_095,
    174_992_710_548,
    87_496_355_274,
    43_748_177_637,
    21_874_088_818,
    10_937_044_409,
    5_468_522_205,
    2_734_261_102,
    1_367_130_551,
];

// Product_i (1 + 2^(-2i))^(-1/2), in Q64.64.
const CORDIC_GAIN_INVERSE_Q64: i128 = 11_201_839_480_117_811_816;

/// A wrapped angle in turns, with one full revolution represented by 2^32 raw
/// units. Thus `Angle::from_turns(Fx::ONE)` wraps to zero.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Angle(u32);

impl Angle {
    pub const ZERO: Self = Self(0);
    pub const QUARTER: Self = Self(QUARTER_TURN);
    pub const HALF: Self = Self(HALF_TURN);
    pub const THREE_QUARTERS: Self = Self(THREE_QUARTERS_TURN);

    #[must_use]
    pub const fn from_raw(raw: u32) -> Self {
        Self(raw)
    }

    #[must_use]
    pub const fn raw(self) -> u32 {
        self.0
    }

    /// Interprets the fractional part of a Q32.32 turn and discards all higher
    /// bits, which makes wrapping well-defined for negative and large angles.
    #[must_use]
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    pub const fn from_turns(turns: Fx) -> Self {
        // Low-bit extraction is the specified wrapping operation.
        Self(turns.raw() as u32)
    }

    /// Converts the wrapped turn to a non-negative Q32.32 value in `[0, 1)`.
    #[must_use]
    #[allow(clippy::cast_lossless)] // `From` is not const.
    pub const fn turns(self) -> Fx {
        Fx::from_raw(self.0 as i64)
    }

    #[must_use]
    pub const fn wrapping_add(self, rhs: Self) -> Self {
        Self(self.0.wrapping_add(rhs.0))
    }

    #[must_use]
    pub const fn wrapping_sub(self, rhs: Self) -> Self {
        Self(self.0.wrapping_sub(rhs.0))
    }

    /// Returns `(sin, cos)` in Q32.32 using 32 integer CORDIC iterations.
    /// The Q64.64 internal state limits shift truncation before final rounding.
    #[must_use]
    pub fn sin_cos(self) -> (Fx, Fx) {
        let quadrant = self.0 >> 30;
        let offset = self.0 & QUARTER_MASK;
        if offset == 0 {
            return match quadrant {
                0 => (Fx::ZERO, Fx::ONE),
                1 => (Fx::ONE, Fx::ZERO),
                2 => (Fx::ZERO, -Fx::ONE),
                _ => (-Fx::ONE, Fx::ZERO),
            };
        }

        let (sin, cos) = cordic_rotate(i128::from(offset) << 32);
        let (sin, cos) = match quadrant {
            0 => (sin, cos),
            1 => (cos, -sin),
            2 => (-sin, -cos),
            _ => (-cos, sin),
        };
        (Fx::from_raw(sin), Fx::from_raw(cos))
    }

    /// Integer CORDIC vectoring. Positive `y` rotates counter-clockwise from
    /// the positive x-axis. `(0, 0)` is defined as angle zero.
    #[must_use]
    pub fn atan2(y: Fx, x: Fx) -> Self {
        let (mut x, mut y) = (i128::from(x.raw()), i128::from(y.raw()));
        if x == 0 && y == 0 {
            return Self::ZERO;
        }
        if y == 0 {
            return if x < 0 { Self::HALF } else { Self::ZERO };
        }
        if x == 0 {
            return if y > 0 {
                Self::QUARTER
            } else {
                Self::THREE_QUARTERS
            };
        }

        let mut angle_q64 = 0_i128;
        if x < 0 {
            angle_q64 = if y > 0 {
                HALF_TURN_Q64
            } else {
                -HALF_TURN_Q64
            };
            x = -x;
            y = -y;
        }

        for (i, &step) in CORDIC_ATAN_TURNS_Q64.iter().enumerate() {
            if y > 0 {
                let old_x = x;
                x += y >> i;
                y -= old_x >> i;
                angle_q64 += step;
            } else if y < 0 {
                let old_x = x;
                x -= y >> i;
                y += old_x >> i;
                angle_q64 -= step;
            }
        }
        let raw = div_round_away(angle_q64, CORDIC_SCALE)
            .expect("CORDIC angle scale is non-zero")
            .rem_euclid(1_i128 << 32);
        Self(u32::try_from(raw).expect("wrapped CORDIC angle fits u32"))
    }
}

fn cordic_rotate(mut angle_q64: i128) -> (i64, i64) {
    let mut x = CORDIC_GAIN_INVERSE_Q64;
    let mut y = 0_i128;
    for (i, &step) in CORDIC_ATAN_TURNS_Q64.iter().enumerate() {
        let old_x = x;
        if angle_q64 >= 0 {
            x -= y >> i;
            y += old_x >> i;
            angle_q64 -= step;
        } else {
            x += y >> i;
            y -= old_x >> i;
            angle_q64 += step;
        }
    }
    let sin = div_round_away(y, CORDIC_SCALE).expect("CORDIC scale is non-zero");
    let cos = div_round_away(x, CORDIC_SCALE).expect("CORDIC scale is non-zero");
    (
        i64::try_from(sin).expect("bounded CORDIC sine fits i64"),
        i64::try_from(cos).expect("bounded CORDIC cosine fits i64"),
    )
}

#[cfg(test)]
mod tests {
    use super::Angle;
    use crate::Fx;

    #[test]
    fn turns_wrap_by_discarding_high_bits() {
        assert_eq!(Angle::from_turns(Fx::ONE), Angle::ZERO);
        assert_eq!(Angle::from_turns(Fx::from_i32(-1)).raw(), 0);
        assert_eq!(Angle::from_turns(Fx::from_raw(0x0001_8000_0000)).raw(), 0x8000_0000);
        assert_eq!(Angle::from_raw(u32::MAX).wrapping_add(Angle::from_raw(1)), Angle::ZERO);
    }

    #[test]
    fn cardinal_sine_and_cosine_are_exact() {
        assert_eq!(Angle::ZERO.sin_cos(), (Fx::ZERO, Fx::ONE));
        assert_eq!(Angle::QUARTER.sin_cos(), (Fx::ONE, Fx::ZERO));
        assert_eq!(Angle::HALF.sin_cos(), (Fx::ZERO, -Fx::ONE));
        assert_eq!(Angle::THREE_QUARTERS.sin_cos(), (-Fx::ONE, Fx::ZERO));
    }

    #[test]
    fn cordic_diagonal_uses_golden_raw_bits() {
        let (sin, cos) = Angle::from_raw(0x2000_0000).sin_cos();
        assert_eq!(sin.raw(), 3_037_000_500);
        assert_eq!(cos.raw(), 3_037_000_500);
        assert_eq!(Angle::atan2(sin, cos).raw(), 0x2000_0000);
    }

    #[test]
    fn non_cardinal_cordic_results_have_stable_raw_bits() {
        let (sin, cos) = Angle::from_raw(0x1234_5678).sin_cos();
        assert_eq!((sin.raw(), cos.raw()), (1_855_794_156, 3_873_341_209));
        assert_eq!(
            Angle::atan2(Fx::from_raw(123_456_789), Fx::from_raw(-987_654_321)).raw(),
            0x7aee_ee2e
        );
    }

    #[test]
    fn atan2_preserves_quadrants_and_defined_zero() {
        let one = Fx::ONE;
        assert_eq!(Angle::atan2(Fx::ZERO, Fx::ZERO), Angle::ZERO);
        assert_eq!(Angle::atan2(Fx::ZERO, -one), Angle::HALF);
        assert_eq!(Angle::atan2(one, Fx::ZERO), Angle::QUARTER);
        assert_eq!(Angle::atan2(-one, Fx::ZERO), Angle::THREE_QUARTERS);
        assert_eq!(Angle::atan2(one, -one).raw(), 0x6000_0000);
        assert_eq!(Angle::atan2(-one, -one).raw(), 0xa000_0000);
    }
}

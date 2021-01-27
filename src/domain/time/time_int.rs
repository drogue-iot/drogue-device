use crate::domain::time::fraction::Fraction;
use core::{fmt, ops};

/// The core inner-type trait for time-related types
#[doc(hidden)]
pub trait TimeInt:
    Copy
    + num::Integer
    + num::Bounded
    + num::traits::WrappingAdd
    + num::traits::WrappingSub
    + num::CheckedAdd
    + num::CheckedSub
    + num::CheckedMul
    + num::CheckedDiv
    + From<u32>
    + ops::Mul<Fraction, Output = Self>
    + ops::Div<Fraction, Output = Self>
    + fmt::Display
    + fmt::Debug
{
    /// Checked integer × [`Fraction`] = integer
    ///
    /// Returns truncated (rounded toward `0`) integer or [`None`] upon failure
    fn checked_mul_fraction(&self, fraction: &Fraction) -> Option<Self> {
        self.checked_mul(&(*fraction.numerator()).into())?
            .checked_div(&(*fraction.denominator()).into())
    }

    /// Checked integer / [`Fraction`] = integer
    ///
    /// Returns truncated (rounded toward `0`) integer or [`None`] upon failure
    fn checked_div_fraction(&self, fraction: &Fraction) -> Option<Self> {
        self.checked_mul_fraction(&fraction.recip())
    }
}

impl TimeInt for u32 {}
impl TimeInt for u64 {}

#[cfg(test)]
mod tests {
    use crate::{fraction::Fraction, time_int::TimeInt};

    #[test]
    fn checked_integer_mul_fraction() {
        assert_eq!(
            8_u32.checked_mul_fraction(&Fraction::new(1, 2)),
            Some(4_u32)
        );

        // the result is not rounded, but truncated (8×(1/3)=2.66)
        assert_eq!(
            8_u32.checked_mul_fraction(&Fraction::new(1, 3)),
            Some(2_u32)
        );
    }

    #[test]
    fn checked_integer_div_fraction() {
        assert_eq!(
            8_u32.checked_div_fraction(&Fraction::new(1, 2)),
            Some(16_u32)
        );

        // the result is not rounded, but truncated (8/3=2.66)
        assert_eq!(
            8_u32.checked_div_fraction(&Fraction::new(3, 1)),
            Some(2_u32)
        );
    }
}

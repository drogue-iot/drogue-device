//! Fractional/Rational values
use crate::domain::time::ConversionError;
use core::ops;
use num::{rational::Ratio, CheckedDiv, CheckedMul, Zero};

/// A fractional value
///
/// Used primarily to define the _scaling factor_ for the [`Duration`], [`Rate`], [`Instant`] and
/// [`Clock`] traits and types.
///
/// [`Duration`]: duration/trait.Duration.html
/// [`Rate`]: rate/trait.Rate.html
/// [`Clock`]: clock/trait.Clock.html
/// [`Instant`]: instant/struct.Instant.html
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub struct Fraction(Ratio<u32>);

impl Fraction {
    /// Construct a new `Fraction`.
    ///
    /// A reduction is **not** performed. Also there is no check for a denominator of `0`. If these
    /// features are needed, use [`Fraction::new_reduce()`]
    pub const fn new(numerator: u32, denominator: u32) -> Self {
        Self(Ratio::new_raw(numerator, denominator))
    }

    /// Return the numerator of the fraction
    pub const fn numerator(&self) -> &u32 {
        self.0.numer()
    }

    /// Return the denominator of the fraction
    pub const fn denominator(&self) -> &u32 {
        self.0.denom()
    }
}

impl Fraction {
    /// Construct a new `Fraction`.
    ///
    /// A reduction and `denominator == 0` check **are** performed.
    ///
    /// # Errors
    ///
    /// [`ConversionError::DivByZero`] : A `0` denominator was detected
    // TODO: add example
    pub fn new_reduce(numerator: u32, denominator: u32) -> Result<Self, ConversionError> {
        if !denominator.is_zero() {
            Ok(Self(Ratio::new(numerator, denominator)))
        } else {
            Err(ConversionError::DivByZero)
        }
    }

    /// Returns the value truncated to an integer
    pub fn to_integer(&self) -> u32 {
        self.0.to_integer()
    }

    /// Constructs a `Fraction` from an integer.
    ///
    /// Equivalent to `Fraction::new(value,1)`.
    pub fn from_integer(value: u32) -> Self {
        Self(Ratio::from_integer(value))
    }

    /// Returns the reciprocal of the fraction
    pub fn recip(self) -> Self {
        Self(self.0.recip())
    }

    /// Checked `Fraction` × `Fraction` = `Fraction`
    ///
    /// Returns [`None`] for any errors
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use drogue_device::domain::time::{fraction::Fraction, ConversionError};
    /// #
    /// assert_eq!(Fraction::new(1000, 1).checked_mul(&Fraction::new(5,5)),
    ///     Some(Fraction::new(5_000, 5)));
    ///
    /// assert_eq!(Fraction::new(u32::MAX, 1).checked_mul(&Fraction::new(2,1)),
    ///     None);
    /// ```
    pub fn checked_mul(&self, v: &Self) -> Option<Self> {
        self.0.checked_mul(&v.0).map(Self)
    }

    /// Checked `Fraction` / `Fraction` = `Fraction`
    ///
    /// Returns [`None`] for any errors
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use drogue_device::domain::time::{fraction::Fraction, ConversionError};
    /// #
    /// assert_eq!(Fraction::new(1000, 1).checked_div(&Fraction::new(10, 1000)),
    ///     Some(Fraction::new(1_000_000, 10)));
    ///
    /// assert_eq!(Fraction::new(1, u32::MAX).checked_div(&Fraction::new(2,1)),
    ///     None);
    /// ```
    pub fn checked_div(&self, v: &Self) -> Option<Self> {
        self.0.checked_div(&v.0).map(Self)
    }
}

impl ops::Mul<Fraction> for u32 {
    type Output = Self;

    /// Panicky u32 × `Fraction` = u32
    fn mul(self, rhs: Fraction) -> Self::Output {
        (rhs.0 * self).to_integer()
    }
}

impl ops::Div<Fraction> for u32 {
    type Output = Self;

    /// Panicky u32 / `Fraction` = u32
    #[allow(clippy::suspicious_arithmetic_impl)]
    fn div(self, rhs: Fraction) -> Self::Output {
        (rhs.0.recip() * self).to_integer()
    }
}

impl ops::Mul<Fraction> for u64 {
    type Output = Self;

    /// Panicky u64 × `Fraction` = u64
    fn mul(self, rhs: Fraction) -> Self::Output {
        (Ratio::new_raw((*rhs.numerator()).into(), (*rhs.denominator()).into()) * self).to_integer()
    }
}

impl ops::Div<Fraction> for u64 {
    type Output = Self;

    /// Panicky u64 / `Fraction` = u64
    #[allow(clippy::suspicious_arithmetic_impl)]
    fn div(self, rhs: Fraction) -> Self::Output {
        (Ratio::new_raw((*rhs.denominator()).into(), (*rhs.numerator()).into()) * self).to_integer()
    }
}

impl ops::Mul for Fraction {
    type Output = Self;

    /// Panicky `Fraction` × `Fraction` = `Fraction`
    ///
    /// # Panics
    ///
    /// The same reason the integer operation would panic. Namely, if the
    /// result overflows the type.
    fn mul(self, rhs: Self) -> Self::Output {
        Self(self.0 * rhs.0)
    }
}

impl ops::Div for Fraction {
    type Output = Self;

    /// Panicky `Fraction` / `Fraction` = `Fraction`
    ///
    /// # Panics
    ///
    /// The same reason the integer operation would panic. Namely, if the
    /// result overflows the type.
    fn div(self, rhs: Self) -> Self::Output {
        Self(self.0 / rhs.0)
    }
}

impl Default for Fraction {
    fn default() -> Self {
        Self::new(1, 1)
    }
}

#[cfg(test)]
mod tests {}

//! Rate-based types/units

pub use crate::domain::time::fraction::Fraction;
use crate::domain::time::{
    duration,
    fixed_point::{self, FixedPoint},
    time_int::TimeInt,
    ConversionError,
};
use core::{convert::TryFrom, mem::size_of, prelude::v1::*};
#[doc(hidden)]
pub use fixed_point::FixedPoint as _;
use num::{CheckedDiv, CheckedMul};
#[doc(inline)]
pub use units::*;

/// An unsigned, fixed-point rate type
///
/// Each implementation defines an _integer_ type and a _scaling factor_ [`Fraction`].
///
/// # Constructing a rate
///
/// ```rust
/// use embedded_time::rate::*;
///
/// let _ = <Kilohertz>::new(5);
/// let _ = Kilohertz(5_u32);
/// let _ = 5_u32.kHz();
/// ```
///
/// # Get the integer part
///
/// ```rust
/// use embedded_time::rate::*;
///
/// assert_eq!(Hertz(45_u32).integer(), &45_u32);
/// ```
///
/// # Formatting
///
/// Just forwards the underlying integer to [`core::fmt::Display::fmt()`]
///
/// ```rust
/// use embedded_time::rate::*;
///
/// assert_eq!(format!("{}", Hertz(123_u32)), "123");
/// ```
///
/// # Converting between `Rate`s
///
/// Many intra-rate conversions can be done using `From`/`Into`:
///
/// ```rust
/// use embedded_time::rate::*;
///
/// let kilohertz = Kilohertz::<u32>::from(23_000_u32.Hz());
/// assert_eq!(kilohertz.integer(), &23_u32);
///
/// let kilohertz: Kilohertz<u32> = 23_000_u32.Hz().into();
/// assert_eq!(kilohertz.integer(), &23_u32);
/// ```
///
/// Others require the use of `TryFrom`/`TryInto`:
///
/// ```rust
/// use embedded_time::rate::*;
/// use std::convert::{TryInto, TryFrom};
///
/// let hertz = Hertz::<u32>::try_from(23_u32.kHz()).unwrap();
/// assert_eq!(hertz.integer(), &23_000_u32);
///
/// let hertz: Hertz<u32> = 23_u32.kHz().try_into().unwrap();
/// assert_eq!(hertz.integer(), &23_000_u32);
/// ```
///
/// # Converting from a [`Generic`] `Rate`
///
/// ## Examples
///
/// ```rust
/// use embedded_time::rate::*;
/// use core::convert::{TryFrom, TryInto};
///
/// // A generic rate of 20_000 events/second
/// let generic_rate = Generic::new(10_u32, Fraction::new(2_000, 1));
///
/// let rate = KilobitsPerSecond::<u32>::try_from(generic_rate).unwrap();
/// assert_eq!(rate, 20_u32.kbps());
///
/// let rate: KilobitsPerSecond<u32> = generic_rate.try_into().unwrap();
/// assert_eq!(rate, 20_u32.kbps());
/// ```
///
/// ## Errors
///
/// Failure will only occur if the provided value does not fit in the selected destination type.
///
/// ---
///
/// [`ConversionError::Unspecified`]
///
/// ```rust
/// use embedded_time::{rate::*, ConversionError};
/// use core::convert::TryFrom;
///
/// assert_eq!(
///     Hertz::<u32>::try_from(Generic::new(u32::MAX, Fraction::new(10,1))),
///     Err(ConversionError::Unspecified)
/// );
/// ```
///
/// ---
///
/// [`ConversionError::ConversionFailure`] : The _integer_ conversion to that of the
/// destination type fails.
///
/// ```rust
/// use embedded_time::{rate::*, ConversionError};
/// # use core::convert::TryFrom;
///
/// assert_eq!(
///     Hertz::<u32>::try_from(Generic::new(u32::MAX as u64 + 1, Fraction::new(1,1))),
///     Err(ConversionError::ConversionFailure)
/// );
/// ```
///
/// # Converting to a [`Generic`] `Rate` with the same _scaling factor_
///
/// ```rust
/// use embedded_time::rate::*;
///
/// let generic_rate = Generic::<u32>::from(5_u32.Hz());
/// let generic_rate: Generic<u32> = 5_u32.Hz().into();
///
/// assert_eq!(generic_rate.integer(), &5_u32);
/// ```
///
/// # Converting to a [`Generic`] `Rate` with a different _scaling factor_
///
/// See [`Rate::to_generic()`]
///
/// # Converting to a _named_ `Duration`
///
/// See [`Rate::to_duration()`]
///
/// # Add/Sub
///
/// The result of the operation is the LHS type
///
/// ## Examples
///
/// ```rust
/// use embedded_time::rate::*;
///
/// assert_eq!((Hertz(1_u32) + 1_u32.kHz()),
///     Hertz(1_001_u32));
///
/// assert_eq!((Hertz(2_001_u32) - 1_u32.kHz()),
///     Hertz(1_001_u32));
/// ```
///
/// ## Panics
///
/// The same reason the integer operation would panic. Namely, if the result overflows the type.
///
/// ```rust,should_panic
/// use embedded_time::rate::*;
///
/// let _ = Hertz(u32::MAX) + Hertz(1_u32);
/// ```
///
/// # Mul/Div
///
/// Rates may also be multiplied and divided by integers. The result is of the LHS type. Both
/// _panicky_ and _checked_ operations are available.
///
/// # Comparisons
///
/// ```rust
/// use embedded_time::rate::*;
///
/// assert_eq!(Kilohertz(2_u32), Hertz(2_000_u32));
/// assert_ne!(Kilohertz(2_u32), Hertz(2_001_u32));
///
/// assert!(Kilohertz(2_u32) < Hertz(2_001_u32));
/// assert!(Kilohertz(2_u32) > Hertz(1_999_u32));
/// ```
///
/// # Remainder
///
/// ```rust
/// use embedded_time::rate::*;
///
/// assert_eq!(Hertz(2_037_u32) % Kilohertz(1_u32), Hertz(37_u32));
/// ```
pub trait Rate: Sized + Copy {
    /// Construct a `Generic` `Rate` from a _named_ `Rate` (eg. [`Kilohertz`])
    ///
    /// # Examples
    ///
    /// ```rust
    /// use embedded_time::rate::*;
    ///
    /// let kilobits = KilobitsPerSecond(20_u32);
    ///
    /// // convert into a generic rate with a different _scaling factor_
    /// let generic = kilobits.to_generic::<u32>(Fraction::new(500, 1)).unwrap();
    ///
    /// assert_eq!(generic.integer(), &40_u32);
    /// ```
    ///
    /// # Errors
    ///
    /// Failure will only occur if the provided value does not fit in the selected destination type.
    ///
    /// ---
    ///
    /// [`ConversionError::Unspecified`]
    ///
    /// ```rust
    /// # use embedded_time::{rate::*, ConversionError};
    ///
    /// assert_eq!(
    ///     Hertz(u32::MAX).to_generic::<u32>(Fraction::new(1, 2)),
    ///     Err(ConversionError::Unspecified)
    /// );
    /// ```
    ///
    /// ---
    ///
    /// [`ConversionError::ConversionFailure`] : The integer conversion to that of the destination
    /// type fails.
    ///
    /// ```rust
    /// use embedded_time::{fraction::Fraction, rate::*, ConversionError};
    ///
    /// assert_eq!(
    ///     Hertz(u32::MAX as u64 + 1).to_generic::<u32>(Fraction::new(1, 1)),
    ///     Err(ConversionError::ConversionFailure)
    /// );
    /// ```
    fn to_generic<DestInt: TimeInt>(
        self,
        scaling_factor: Fraction,
    ) -> Result<Generic<DestInt>, ConversionError>
    where
        Self: FixedPoint,
        DestInt: TryFrom<Self::T>,
    {
        Ok(Generic::<DestInt>::new(
            self.into_ticks(scaling_factor)?,
            scaling_factor,
        ))
    }

    /// Convert to _named_ [`Duration`](duration::Duration)
    ///
    /// (the rate is equal to the reciprocal of the duration)
    ///
    /// # Examples
    ///
    /// ```rust
    /// use embedded_time::{duration::*, rate::*};
    ///
    /// assert_eq!(
    ///     Kilohertz(500_u32).to_duration(),
    ///     Ok(Microseconds(2_u32))
    /// );
    /// ```
    ///
    /// # Errors
    ///
    /// Failure will only occur if the provided value does not fit in the selected destination type.
    ///
    /// ---
    ///
    /// [`ConversionError::Overflow`] : The conversion of the _scaling factor_ causes an overflow.
    ///
    /// ```rust
    /// use embedded_time::{duration::*, rate::*, ConversionError};
    ///
    /// assert_eq!(
    ///     Megahertz(u32::MAX).to_duration::<Hours<u32>>(),
    ///     Err(ConversionError::Overflow)
    /// );
    /// ```
    ///
    /// ---
    ///
    /// [`ConversionError::DivByZero`] : The rate is `0`, therefore the reciprocal is undefined.
    ///
    /// ```rust
    /// use embedded_time::{duration::*, rate::*, ConversionError};
    ///
    /// assert_eq!(
    ///     Hertz(0_u32).to_duration::<Seconds<u32>>(),
    ///     Err(ConversionError::DivByZero)
    /// );
    /// ```
    fn to_duration<Duration: duration::Duration>(&self) -> Result<Duration, ConversionError>
    where
        Duration: FixedPoint,
        Self: FixedPoint,
        Duration::T: TryFrom<Self::T>,
    {
        let conversion_factor = Self::SCALING_FACTOR
            .checked_mul(&Duration::SCALING_FACTOR)
            .ok_or(ConversionError::Unspecified)?
            .recip();

        if size_of::<Self::T>() >= size_of::<Duration::T>() {
            fixed_point::FixedPoint::from_ticks(
                Self::T::from(*conversion_factor.numerator())
                    .checked_div(
                        &self
                            .integer()
                            .checked_mul(&Self::T::from(*conversion_factor.denominator()))
                            .ok_or(ConversionError::Overflow)?,
                    )
                    .ok_or(ConversionError::DivByZero)?,
                Duration::SCALING_FACTOR,
            )
        } else {
            fixed_point::FixedPoint::from_ticks(
                Duration::T::from(*conversion_factor.numerator())
                    .checked_div(
                        &Duration::T::try_from(*self.integer())
                            .ok()
                            .unwrap()
                            .checked_mul(&Duration::T::from(*conversion_factor.denominator()))
                            .ok_or(ConversionError::Overflow)?,
                    )
                    .ok_or(ConversionError::DivByZero)?,
                Duration::SCALING_FACTOR,
            )
        }
    }
}

/// The `Generic` `Rate` type allows an arbitrary _scaling factor_ to be used without having to
/// impl `FixedPoint`.
///
/// The purpose of this type is to allow a simple `Rate` object that can be defined at run-time.
/// It does this by replacing the `const` _scaling factor_ with a struct field.
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Default)]
pub struct Generic<T> {
    integer: T,
    scaling_factor: Fraction,
}

impl<T> Generic<T> {
    /// Constructs a new fixed-point `Generic` `Rate` value
    pub const fn new(integer: T, scaling_factor: Fraction) -> Self {
        Self {
            integer,
            scaling_factor,
        }
    }

    /// Returns the _integer_ part
    pub const fn integer(&self) -> &T {
        &self.integer
    }

    /// Returns the _scaling factor_ [`Fraction`] part
    pub const fn scaling_factor(&self) -> &Fraction {
        &self.scaling_factor
    }
}

impl<T: TimeInt> Rate for Generic<T> {}

/// Rate-type units
#[doc(hidden)]
pub mod units {
    use super::*;
    use crate::domain::time::{
        fixed_point::{self, FixedPoint},
        fraction::Fraction,
        time_int::TimeInt,
        ConversionError,
    };
    use core::{
        cmp,
        convert::TryFrom,
        fmt::{self, Formatter},
        ops,
    };
    #[doc(hidden)]
    pub use Extensions as _;

    macro_rules! impl_rate {
        ( $name:ident, ($numer:expr, $denom:expr), $desc:literal ) => {
            #[doc = $desc]
            #[derive(Copy, Clone, Eq, Ord, Hash, Debug, Default)]
            pub struct $name<T: TimeInt = u32>(pub T);

            impl<T: TimeInt> $name<T> {
                /// See [Constructing a rate](trait.Rate.html#constructing-a-rate)
                pub fn new(value: T) -> Self {
                    Self(value)
                }
            }

            impl<T: TimeInt> Rate for $name<T> {}

            impl<T: TimeInt> FixedPoint for $name<T> {
                type T = T;
                const SCALING_FACTOR: Fraction = Fraction::new($numer, $denom);

                /// See [Constructing a rate](trait.Rate.html#constructing-a-rate)
                fn new(value: Self::T) -> Self {
                    Self(value)
                }

                /// See [Get the integer part](trait.Rate.html#get-the-integer-part)
                fn integer(&self) -> &Self::T {
                    &self.0
                }
            }

            impl<T: TimeInt> fmt::Display for $name<T> {
                /// See [Formatting](trait.Rate.html#formatting)
                fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
                    fmt::Display::fmt(&self.0, f)
                }
            }

            impl<T: TimeInt, Rhs: Rate> ops::Add<Rhs> for $name<T>
            where
                Rhs: FixedPoint,
                Self: TryFrom<Rhs>,
            {
                type Output = Self;

                /// See [Add/Sub](trait.Rate.html#addsub)
                fn add(self, rhs: Rhs) -> Self::Output {
                    <Self as FixedPoint>::add(self, rhs)
                }
            }

            impl<T: TimeInt, Rhs: Rate> ops::Sub<Rhs> for $name<T>
            where
                Self: TryFrom<Rhs>,
                Rhs: FixedPoint,
            {
                type Output = Self;

                /// See [Add/Sub](trait.Rate.html#addsub)
                fn sub(self, rhs: Rhs) -> Self::Output {
                    <Self as FixedPoint>::sub(self, rhs)
                }
            }

            impl<T: TimeInt> ops::Mul<T> for $name<T> {
                type Output = Self;

                /// See [Mul/Div](trait.Rate.html#muldiv)
                fn mul(self, rhs: T) -> Self::Output {
                    <Self as FixedPoint>::mul(self, rhs)
                }
            }

            impl<T: TimeInt> ops::Div<T> for $name<T> {
                type Output = Self;

                /// See [Mul/Div](trait.Rate.html#muldiv)
                fn div(self, rhs: T) -> Self::Output {
                    <Self as FixedPoint>::div(self, rhs)
                }
            }

            impl<T: TimeInt, Rhs: Rate> ops::Rem<Rhs> for $name<T>
            where
                Self: TryFrom<Rhs>,
                Rhs: FixedPoint,
            {
                type Output = Self;

                /// See [Remainder](trait.Rate.html#remainder)
                fn rem(self, rhs: Rhs) -> Self::Output {
                    <Self as FixedPoint>::rem(self, rhs)
                }
            }

            impl<SourceInt: TimeInt, DestInt: TimeInt> TryFrom<Generic<SourceInt>>
                for $name<DestInt>
            where
                DestInt: TryFrom<SourceInt>,
            {
                type Error = ConversionError;

                /// See [Converting from a `Generic` `Rate`](trait.Rate.html#converting-from-a-generic-rate)
                fn try_from(generic_rate: Generic<SourceInt>) -> Result<Self, Self::Error> {
                    fixed_point::FixedPoint::from_ticks(generic_rate.integer, generic_rate.scaling_factor)
                }
            }

            impl<T: TimeInt> From<$name<T>> for Generic<T> {
                /// See [Converting to a `Generic` `Rate`](trait.Rate.html#converting-to-a-generic-rate)
                fn from(rate: $name<T>) -> Self {
                    Self::new(*rate.integer(), $name::<T>::SCALING_FACTOR)
                }
            }
        };
    }
    impl_rate![Mebihertz, (1_048_576, 1), "Hertz × 1,048,576"];
    impl_rate![Megahertz, (1_000_000, 1), "Hertz × 1,000,000"];
    impl_rate![Kibihertz, (1_024, 1), "Hertz × 1,024"];
    impl_rate![Kilohertz, (1_000, 1), "Hertz × 1,000"];
    impl_rate![Hertz, (1, 1), "Hertz"];
    impl_rate![Decihertz, (1, 10), "Hertz / 10"];
    impl_rate![Centihertz, (1, 100), "Hertz / 100"];
    impl_rate![Millihertz, (1, 1_000), "Hertz / 1000"];
    impl_rate![Microhertz, (1, 1_000_000), "Hertz / 1,000,000"];
    impl_rate![
        MebibytesPerSecond,
        (1_048_576 * 8, 1),
        "Bytes/s × 1,048,576"
    ];
    impl_rate![
        MegabytesPerSecond,
        (1_000_000 * 8, 1),
        "Bytes/s × 1,000,000"
    ];
    impl_rate![KibibytesPerSecond, (1_024 * 8, 1), "Bytes/s × 1,024"];
    impl_rate![KilobytesPerSecond, (1_000 * 8, 1), "Bytes/s × 1,000"];
    impl_rate![BytesPerSecond, (8, 1), "Bytes/s"];
    impl_rate![MebibitsPerSecond, (1_048_576, 1), "Bits/s × 1,048,576"];
    impl_rate![MegabitsPerSecond, (1_000_000, 1), "Bits/s × 1,000,000"];
    impl_rate![KibibitsPerSecond, (1_024, 1), "Bits/s × 1,024"];
    impl_rate![KilobitsPerSecond, (1_000, 1), "Bits/s × 1,000"];
    impl_rate![BitsPerSecond, (1, 1), "Bits/s"];
    impl_rate![Mebibaud, (1_048_576, 1), "Baud × 1,048,576"];
    impl_rate![Megabaud, (1_000_000, 1), "Baud × 1,000,000"];
    impl_rate![Kibibaud, (1_024, 1), "Baud × 1,024"];
    impl_rate![Kilobaud, (1_000, 1), "Baud × 1,000"];
    impl_rate![Baud, (1, 1), "Baud"];

    macro_rules! impl_conversion {
        ($name:ident) => {
            impl From<$name<u32>> for $name<u64> {
                /// See [Converting between `Rate`s](trait.Rate.html#converting-between-rates)
                fn from(source: $name<u32>) -> Self {
                    Self::new(u64::from(*source.integer()))
                }
            }

            impl TryFrom<$name<u64>> for $name<u32> {
                type Error = ConversionError;

                /// See [Converting between `Rate`s](trait.Rate.html#converting-between-rates)
                fn try_from(source: $name<u64>) -> Result<Self, Self::Error> {
                    fixed_point::FixedPoint::from_ticks(
                        *source.integer(),
                        $name::<u64>::SCALING_FACTOR,
                    )
                }
            }

            impl<T: TimeInt, RhsInt: TimeInt> cmp::PartialEq<$name<RhsInt>> for $name<T>
            where
                T: TryFrom<RhsInt>,
            {
                /// See [Comparisons](trait.Rate.html#comparisons)
                fn eq(&self, rhs: &$name<RhsInt>) -> bool {
                    match T::try_from(*rhs.integer()) {
                        Ok(rhs_value) => *self.integer() == rhs_value,
                        Err(_) => false
                    }
                }
            }

            impl<T: TimeInt, RhsInt: TimeInt> PartialOrd<$name<RhsInt>> for $name<T>
            where
                T: TryFrom<RhsInt>,
            {
                /// See [Comparisons](trait.Rate.html#comparisons)
                fn partial_cmp(&self, rhs: &$name<RhsInt>) -> Option<core::cmp::Ordering> {
                    match T::try_from(*rhs.integer()) {
                        Ok(rhs_integer) => Some(self.integer().cmp(&rhs_integer)),
                        Err(_) => Some(core::cmp::Ordering::Less),
                    }
                }
            }
        };

        (once, $big:ident, $small:ident) => {
            impl<T: TimeInt> From<$small<T>> for $big<T>
            {
                /// See [Converting between `Rate`s](trait.Rate.html#converting-between-rates)
                fn from(small: $small<T>) -> Self {
                    fixed_point::FixedPoint::from_ticks(*small.integer(), $small::<T>::SCALING_FACTOR).ok().unwrap()
                }
            }

            impl From<$small<u32>> for $big<u64>
            {
                /// See [Converting between `Rate`s](trait.Rate.html#converting-between-rates)
                fn from(small: $small<u32>) -> Self {
                    fixed_point::FixedPoint::from_ticks(*small.integer(), $small::<u32>::SCALING_FACTOR).ok().unwrap()
                }
            }

            impl TryFrom<$small<u64>> for $big<u32>
            {
                type Error = ConversionError;

                /// See [Converting between `Rate`s](trait.Rate.html#converting-between-rates)
                fn try_from(small: $small<u64>) -> Result<Self, Self::Error> {
                    fixed_point::FixedPoint::from_ticks(
                        *small.integer(),
                        $small::<u64>::SCALING_FACTOR,
                    )
                }
            }


            impl From<$big<u32>> for $small<u64>
            {
               /// See [Converting between `Rate`s](trait.Rate.html#converting-between-rates)
                fn from(big: $big<u32>) -> Self {
                    fixed_point::FixedPoint::from_ticks(*big.integer(), $big::<u32>::SCALING_FACTOR).ok().unwrap()
                }
            }

            impl<T: TimeInt> TryFrom<$big<T>> for $small<T>
            {
                type Error = ConversionError;

                /// See [Converting between `Rate`s](trait.Rate.html#converting-between-rates)
                fn try_from(big: $big<T>) -> Result<Self, Self::Error> {
                    fixed_point::FixedPoint::from_ticks(
                        *big.integer(),
                        $big::<T>::SCALING_FACTOR,
                    )
                }
            }

            impl TryFrom<$big<u64>> for $small<u32>
            {
                type Error = ConversionError;

                /// See [Converting between `Rate`s](trait.Rate.html#converting-between-rates)
                fn try_from(big: $big<u64>) -> Result<Self, Self::Error> {
                    fixed_point::FixedPoint::from_ticks(
                        *big.integer(),
                        $big::<u64>::SCALING_FACTOR,
                    )
                }
            }

            impl<T: TimeInt, RhsInt: TimeInt> cmp::PartialEq<$small<RhsInt>> for $big<T>
            where
                $small<RhsInt>: PartialEq<$big<T>>,
            {
                /// See [Comparisons](trait.Rate.html#comparisons)
                fn eq(&self, rhs: &$small<RhsInt>) -> bool {
                    <$small::<RhsInt> as PartialEq<$big<T>>>::eq(rhs, self)
                }
            }

            impl<T: TimeInt, RhsInt: TimeInt> cmp::PartialEq<$big<RhsInt>> for $small<T>
            where
                Self: TryFrom<$big<RhsInt>>,
            {
                /// See [Comparisons](trait.Rate.html#comparisons)
                fn eq(&self, rhs: &$big<RhsInt>) -> bool {
                    match Self::try_from(*rhs) {
                        Ok(rhs) => *self == rhs,
                        Err(_) => false
                    }
                }
            }

            impl<T: TimeInt, RhsInt: TimeInt> PartialOrd<$small<RhsInt>> for $big<T>
            where
                $small<RhsInt>: TryFrom<Self> + Ord,
            {
                /// See [Comparisons](trait.Rate.html#comparisons)
                fn partial_cmp(&self, rhs: &$small<RhsInt>) -> Option<core::cmp::Ordering> {
                    match $small::<RhsInt>::try_from(*self) {
                        Ok(lhs) => Some(lhs.cmp(&rhs)),
                        Err(_) => Some(core::cmp::Ordering::Greater),
                    }

                }
            }

            impl<T: TimeInt, RhsInt: TimeInt> PartialOrd<$big<RhsInt>> for $small<T>
            where
                Self: TryFrom<$big<RhsInt>>,
            {
                /// See [Comparisons](trait.Rate.html#comparisons)
                fn partial_cmp(&self, rhs: &$big<RhsInt>) -> Option<core::cmp::Ordering> {
                    match Self::try_from(*rhs) {
                        Ok(rhs) => Some((*self).cmp(&rhs)),
                        Err(_) => Some(core::cmp::Ordering::Less),
                    }
                }
            }
        };
        ($big:ident; $($small:ident),+) => {
            impl_conversion![$big];
            $(
                impl_conversion![once, $big, $small];
            )+
        };
        // ($big:ident, $($small:ident),+) => {
        //     $(
        //         impl_from_smaller![once, $big, $small];
        //     )+
        //
        //     impl_from_smaller![$($small),+];
        // };

    }

    impl_conversion![Mebihertz; Kibihertz, Hertz];
    impl_conversion![Kibihertz; Hertz];
    impl_conversion![Megahertz; Kilohertz, Hertz];
    impl_conversion![Kilohertz; Hertz];
    impl_conversion![Hertz];
    impl_conversion![Decihertz; Hertz];
    impl_conversion![Centihertz; Hertz];
    impl_conversion![Millihertz; Hertz];
    impl_conversion![Microhertz; Hertz];

    // The first arg implements From/TryFrom all following
    impl_conversion![MebibytesPerSecond; MebibitsPerSecond, KibibytesPerSecond, KibibitsPerSecond, BytesPerSecond, BitsPerSecond];
    impl_conversion![MebibitsPerSecond; KibibytesPerSecond, KibibitsPerSecond, BytesPerSecond, BitsPerSecond];
    impl_conversion![KibibytesPerSecond; KibibitsPerSecond, BytesPerSecond, BitsPerSecond];
    impl_conversion![KibibitsPerSecond; BytesPerSecond, BitsPerSecond];

    impl_conversion![MegabytesPerSecond; MegabitsPerSecond, KilobytesPerSecond, KilobitsPerSecond, BytesPerSecond, BitsPerSecond];
    impl_conversion![MegabitsPerSecond; KilobytesPerSecond, KilobitsPerSecond, BytesPerSecond, BitsPerSecond  ];
    impl_conversion![KilobytesPerSecond; KilobitsPerSecond, BytesPerSecond, BitsPerSecond ];
    impl_conversion![KilobitsPerSecond; BytesPerSecond, BitsPerSecond];

    impl_conversion![BytesPerSecond; BitsPerSecond];
    impl_conversion![BitsPerSecond];

    impl_conversion![Mebibaud; Kibibaud, Baud];
    impl_conversion![Kibibaud; Baud];
    impl_conversion![Megabaud; Kilobaud, Baud];
    impl_conversion![Kilobaud; Baud];
    impl_conversion![Baud];

    /// Create rate-based extensions from primitive numeric types.
    ///
    /// ```rust
    /// # use embedded_time::{rate::*};
    /// assert_eq!(5_u32.MiHz(), Mebihertz(5_u32));
    /// assert_eq!(5_u32.MHz(), Megahertz(5_u32));
    /// assert_eq!(5_u32.KiHz(), Kibihertz(5_u32));
    /// assert_eq!(5_u32.kHz(), Kilohertz(5_u32));
    /// assert_eq!(5_u32.Hz(), Hertz(5_u32));
    /// assert_eq!(5_u32.MiBps(), MebibytesPerSecond(5_u32));
    /// assert_eq!(5_u32.MBps(), MegabytesPerSecond(5_u32));
    /// assert_eq!(5_u32.KiBps(), KibibytesPerSecond(5_u32));
    /// assert_eq!(5_u32.kBps(), KilobytesPerSecond(5_u32));
    /// assert_eq!(5_u32.Bps(), BytesPerSecond(5_u32));
    /// assert_eq!(5_u32.Mibps(), MebibitsPerSecond(5_u32));
    /// assert_eq!(5_u32.Mbps(), MegabitsPerSecond(5_u32));
    /// assert_eq!(5_u32.Kibps(), KibibitsPerSecond(5_u32));
    /// assert_eq!(5_u32.kbps(), KilobitsPerSecond(5_u32));
    /// assert_eq!(5_u32.bps(), BitsPerSecond(5_u32));
    /// assert_eq!(5_u32.MiBd(), Mebibaud(5_u32));
    /// assert_eq!(5_u32.MBd(), Megabaud(5_u32));
    /// assert_eq!(5_u32.KiBd(), Kibibaud(5_u32));
    /// assert_eq!(5_u32.kBd(), Kilobaud(5_u32));
    /// assert_eq!(5_u32.Bd(), Baud(5_u32));
    /// ```
    #[allow(non_snake_case)]
    pub trait Extensions: TimeInt {
        /// mebihertz
        fn MiHz(self) -> Mebihertz<Self> {
            Mebihertz::new(self)
        }

        /// megahertz
        fn MHz(self) -> Megahertz<Self> {
            Megahertz::new(self)
        }

        /// kibihertz
        fn KiHz(self) -> Kibihertz<Self> {
            Kibihertz::new(self)
        }

        /// kilohertz
        fn kHz(self) -> Kilohertz<Self> {
            Kilohertz::new(self)
        }

        /// hertz
        fn Hz(self) -> Hertz<Self> {
            Hertz::new(self)
        }

        /// mebibytes per second
        fn MiBps(self) -> MebibytesPerSecond<Self> {
            MebibytesPerSecond::new(self)
        }

        /// megabytes per second
        fn MBps(self) -> MegabytesPerSecond<Self> {
            MegabytesPerSecond::new(self)
        }

        /// kibibytes per second
        fn KiBps(self) -> KibibytesPerSecond<Self> {
            KibibytesPerSecond::new(self)
        }

        /// kiloBytes per second
        fn kBps(self) -> KilobytesPerSecond<Self> {
            KilobytesPerSecond::new(self)
        }

        /// bytes per second
        fn Bps(self) -> BytesPerSecond<Self> {
            BytesPerSecond::new(self)
        }

        /// mebibits per second
        fn Mibps(self) -> MebibitsPerSecond<Self> {
            MebibitsPerSecond::new(self)
        }

        /// megabits per second
        fn Mbps(self) -> MegabitsPerSecond<Self> {
            MegabitsPerSecond::new(self)
        }

        /// kibibits per second
        fn Kibps(self) -> KibibitsPerSecond<Self> {
            KibibitsPerSecond::new(self)
        }

        /// kilobits per second
        fn kbps(self) -> KilobitsPerSecond<Self> {
            KilobitsPerSecond::new(self)
        }

        /// bits per second
        fn bps(self) -> BitsPerSecond<Self> {
            BitsPerSecond::new(self)
        }

        /// mebibaud
        fn MiBd(self) -> Mebibaud<Self> {
            Mebibaud::new(self)
        }

        /// megabaud
        fn MBd(self) -> Megabaud<Self> {
            Megabaud::new(self)
        }

        /// kibibaud
        fn KiBd(self) -> Kibibaud<Self> {
            Kibibaud::new(self)
        }

        /// kilobaud
        fn kBd(self) -> Kilobaud<Self> {
            Kilobaud::new(self)
        }

        /// baud
        fn Bd(self) -> Baud<Self> {
            Baud::new(self)
        }
    }

    impl Extensions for u32 {}
    impl Extensions for u64 {}
}

#[cfg(test)]
mod tests {}

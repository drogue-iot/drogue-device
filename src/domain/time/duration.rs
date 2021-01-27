//! Duration types/units

pub use crate::domain::time::fraction::Fraction;
use crate::domain::time::{
    fixed_point::{self, FixedPoint},
    rate,
    time_int::TimeInt,
    ConversionError,
};
use core::{convert::TryFrom, mem::size_of, prelude::v1::*};
#[doc(hidden)]
pub use fixed_point::FixedPoint as _;
use num::{CheckedDiv, CheckedMul};
#[doc(inline)]
pub use units::*;

/// An unsigned, fixed-point duration type
///
/// Each implementation defines an _integer_ type and a _scaling factor_ [`Fraction`].
///
/// # Constructing a duration
///
/// ```rust
/// use embedded_time::duration::*;
///
/// let millis = <Milliseconds>::new(5);
/// let millis = Milliseconds(5_u32);
/// let millis = 5_u32.milliseconds();
/// ```
///
/// # Get the integer part
///
/// ```rust
/// use embedded_time::duration::*;
///
/// let millis = Milliseconds(23_u32).integer();
///
/// assert_eq!(millis, &23_u32);
/// ```
///
/// # Formatting
///
/// Just forwards the underlying integer to [`core::fmt::Display::fmt()`]
///
/// ```rust
/// use embedded_time::duration::*;
///
/// assert_eq!(format!("{}", Seconds(123_u32)), "123");
/// ```
///
/// # Getting H:M:S.MS... Components
///
/// ```rust
/// use embedded_time::duration::*;
///
/// // (the default duration _integer_ type is `u32`)
/// let duration = 38_238_479_u32.microseconds();
/// let hours: Hours = duration.into();
/// let minutes = <Minutes>::from(duration) % Hours(1_u32);
/// let seconds = <Seconds>::from(duration) % Minutes(1_u32);
/// let milliseconds = <Milliseconds>::from(duration) % Seconds(1_u32);
/// // ...
/// ```
///
/// # Converting between `Duration`s
///
/// Many intra-duration conversions can be done using `From`/`Into`:
///
/// ```rust
/// use embedded_time::duration::*;
///
/// let seconds = Seconds::<u32>::from(23_000_u32.milliseconds());
/// assert_eq!(seconds.integer(), &23_u32);
///
/// let seconds: Seconds<u32> = 23_000_u32.milliseconds().into();
/// assert_eq!(seconds.integer(), &23_u32);
/// ```
///
/// Others require the use of `TryFrom`/`TryInto`:
///
/// ```rust
/// use embedded_time::duration::*;
/// use std::convert::{TryInto, TryFrom};
///
/// let millis = Milliseconds::<u32>::try_from(23_u32.seconds()).unwrap();
/// assert_eq!(millis.integer(), &23_000_u32);
///
/// let millis: Milliseconds<u32> = 23_u32.seconds().try_into().unwrap();
/// assert_eq!(millis.integer(), &23_000_u32);
/// ```
///
/// # Converting to `core` types
///
/// ([`core::time::Duration`])
///
/// **Note**: Due to the inner types used by `core::time::Duration`, a lot of code bloat occurs when
/// it is used.
///
/// ## Examples
///
/// ```rust
/// use embedded_time::duration::*;
/// use core::convert::TryFrom;
///
/// let core_duration = core::time::Duration::try_from(2_569_u32.milliseconds()).unwrap();
///
/// assert_eq!(core_duration.as_secs(), 2);
/// assert_eq!(core_duration.subsec_nanos(), 569_000_000);
/// ```
///
/// ```rust
/// use embedded_time::duration::*;
/// use core::convert::TryInto;
///
/// let core_duration: core::time::Duration = 2_569_u32.milliseconds().try_into().unwrap();
///
/// assert_eq!(core_duration.as_secs(), 2);
/// assert_eq!(core_duration.subsec_nanos(), 569_000_000);
/// ```
///
/// # Converting from `core` types
///
/// ([`core::time::Duration`])
///
/// **Note**: Due to the inner types used by `core::time::Duration`, a lot of code bloat occurs when
/// it is used.
///
/// ## Examples
///
/// ```rust
/// use embedded_time::duration::*;
/// use core::convert::TryFrom;
///
/// let core_duration = core::time::Duration::new(5, 730_023_852);
///
/// assert_eq!(Milliseconds::<u32>::try_from(core_duration), Ok(5_730.milliseconds()));
/// ```
///
/// ```rust
/// use embedded_time::duration::*;
/// # use core::convert::TryInto;
///
/// let duration: Result<Milliseconds<u32>, _> = core::time::Duration::new(5, 730023852).try_into();
///
/// assert_eq!(duration, Ok(5_730.milliseconds()));
/// ```
///
/// ## Errors
///
/// [`ConversionError::ConversionFailure`] : The duration doesn't fit in the type specified
///
/// ```rust
/// use embedded_time::{duration::*, ConversionError};
/// # use core::convert::{TryFrom, TryInto};
///
/// assert_eq!(
///     Milliseconds::<u32>::try_from(
///         core::time::Duration::from_millis((u32::MAX as u64) + 1)
///     ),
///     Err(ConversionError::ConversionFailure)
/// );
///
/// let duration: Result<Milliseconds<u32>, _> =
///     core::time::Duration::from_millis((u32::MAX as u64) + 1).try_into();
/// assert_eq!(duration, Err(ConversionError::ConversionFailure));
/// ```
///
/// # Converting from a [`Generic`] `Duration`
///
/// ## Examples
///
/// ```rust
/// use embedded_time::duration::*;
/// # use core::convert::{TryFrom, TryInto};
///
/// // A generic duration of 2 seconds
/// let generic_duration = Generic::new(2_000_u32, Fraction::new(1, 1_000));
///
/// let secs = Seconds::<u64>::try_from(generic_duration).unwrap();
/// assert_eq!(secs.integer(), &2_u64);
///
/// let secs: Seconds<u64> = generic_duration.try_into().unwrap();
/// assert_eq!(secs.integer(), &2_u64);
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
/// use embedded_time::{duration::*, ConversionError};
/// # use core::convert::TryFrom;
///
/// assert_eq!(
///     Seconds::<u32>::try_from(Generic::new(u32::MAX, Fraction::new(10,1))),
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
/// use embedded_time::{duration::*, ConversionError};
/// # use core::convert::TryFrom;
///
/// assert_eq!(
///     Seconds::<u32>::try_from(Generic::new(u32::MAX as u64 + 1, Fraction::new(1,1))),
///     Err(ConversionError::ConversionFailure)
/// );
/// ```
///
/// # Converting to a [`Generic`] `Duration` with the same _scaling factor_
///
/// ```rust
/// use embedded_time::duration::*;
///
/// let generic_duration = Generic::<u32>::from(5_u32.seconds());
/// let generic_duration: Generic<u32> = 5_u32.seconds().into();
///
/// assert_eq!(generic_duration.integer(), &5_u32);
/// ```
///
/// # Converting to a [`Generic`] `Duration` with a different _scaling factor_
///
/// See [`Duration::to_generic()`]
///
/// # Converting to a _named_ `Rate`
///
/// See [`Duration::to_rate()`]
///
/// # Add/Sub
///
/// The result of the operation is the LHS type
///
/// ## Examples
///
/// ```rust
/// use embedded_time::duration::*;
///
/// assert_eq!((Milliseconds(1_u32) + Seconds(1_u32)),
///     Milliseconds(1_001_u32));
///
/// assert_eq!((Milliseconds(2_001_u32) - Seconds(1_u32)),
///     Milliseconds(1_001_u32));
/// ```
///
/// ## Panics
///
/// The same reason the integer operation would panic. Namely, if the result overflows the type.
///
/// ```rust,should_panic
/// use embedded_time::duration::*;
///
/// let _ = Seconds(u32::MAX) + Seconds(1_u32);
/// ```
///
/// # Mul/Div
///
/// Durations may also be multiplied and divided by integers. The result is of the LHS type. Both
/// _panicky_ and _checked_ operations are available.
///
/// # Comparisons
///
/// ```rust
/// use embedded_time::duration::*;
///
/// assert_eq!(Seconds(2_u32), Milliseconds(2_000_u32));
/// assert_ne!(Seconds(2_u32), Milliseconds(2_001_u32));
///
/// assert!(Seconds(2_u32) < Milliseconds(2_001_u32));
/// assert!(Seconds(2_u32) > Milliseconds(1_999_u32));
/// ```
///
/// # Remainder
///
/// ```rust
/// use embedded_time::duration::*;
///
/// assert_eq!(Minutes(62_u32) % Hours(1_u32), Minutes(2_u32));
/// ```
pub trait Duration: Sized + Copy {
    /// Construct a `Generic` `Duration` from a _named_ `Duration` (eg.
    /// [`Milliseconds`])
    ///
    /// # Examples
    ///
    /// ```rust
    /// use embedded_time::duration::*;
    ///
    /// let millis = Milliseconds(20_u32);
    ///
    /// // convert into a generic duration with a different _scaling factor_
    /// let generic = millis.to_generic::<u32>(Fraction::new(1, 2_000)).unwrap();
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
    /// use embedded_time::{duration::*, ConversionError};
    ///
    /// assert_eq!(
    ///     Seconds(u32::MAX).to_generic::<u32>(Fraction::new(1, 2)),
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
    /// use embedded_time::{duration::*, ConversionError};
    ///
    /// assert_eq!(Seconds(u32::MAX as u64 + 1).to_generic::<u32>(Fraction::new(1, 1)),
    ///     Err(ConversionError::ConversionFailure));
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

    /// Convert to _named_ [`Rate`](rate::Rate)
    ///
    /// (the duration is equal to the reciprocal of the rate)
    ///
    /// # Examples
    ///
    /// ```rust
    /// use embedded_time::{duration::*, rate::*};
    ///
    /// assert_eq!(
    ///     Microseconds(500_u32).to_rate(),
    ///     Ok(Kilohertz(2_u32))
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
    ///     Hours(u32::MAX).to_rate::<Megahertz<u32>>(),
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
    ///     Seconds(0_u32).to_rate::<Hertz<u32>>(),
    ///     Err(ConversionError::DivByZero)
    /// );
    /// ```
    fn to_rate<Rate: rate::Rate>(&self) -> Result<Rate, ConversionError>
    where
        Rate: FixedPoint,
        Self: FixedPoint,
        Rate::T: TryFrom<Self::T>,
    {
        let conversion_factor = Self::SCALING_FACTOR
            .checked_mul(&Rate::SCALING_FACTOR)
            .ok_or(ConversionError::Unspecified)?
            .recip();

        if size_of::<Self::T>() >= size_of::<Rate::T>() {
            fixed_point::FixedPoint::from_ticks(
                Self::T::from(*conversion_factor.numerator())
                    .checked_div(
                        &self
                            .integer()
                            .checked_mul(&Self::T::from(*conversion_factor.denominator()))
                            .ok_or(ConversionError::Overflow)?,
                    )
                    .ok_or(ConversionError::DivByZero)?,
                Rate::SCALING_FACTOR,
            )
        } else {
            fixed_point::FixedPoint::from_ticks(
                Rate::T::from(*conversion_factor.numerator())
                    .checked_div(
                        &Rate::T::try_from(*self.integer())
                            .ok()
                            .unwrap()
                            .checked_mul(&Rate::T::from(*conversion_factor.denominator()))
                            .ok_or(ConversionError::Overflow)?,
                    )
                    .ok_or(ConversionError::DivByZero)?,
                Rate::SCALING_FACTOR,
            )
        }
    }
}

/// The `Generic` `Duration` type allows an arbitrary _scaling factor_ to be used without having to
/// impl `FixedPoint`.
///
/// The purpose of this type is to allow a simple `Duration` object that can be defined at run-time.
/// It does this by replacing the `const` _scaling factor_ with a struct field.
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Default)]
pub struct Generic<T> {
    integer: T,
    scaling_factor: Fraction,
}

impl<T> Generic<T> {
    /// Constructs a new fixed-point `Generic` `Duration` value
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

impl<T: TimeInt> Duration for Generic<T> {}

/// Duration units
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
        convert::{TryFrom, TryInto},
        fmt::{self, Formatter},
        ops,
    };
    #[doc(hidden)]
    pub use Extensions as _;

    macro_rules! impl_duration {
        ( $name:ident, ($numer:expr, $denom:expr) ) => {
            /// A duration unit type
            #[derive(Copy, Clone, Eq, Ord, Hash, Debug, Default)]
            pub struct $name<T: TimeInt = u32>(pub T);

            impl<T: TimeInt> $name<T> {
                /// See [Constructing a duration](trait.Duration.html#constructing-a-duration)
                pub fn new(value: T) -> Self {
                    Self(value)
                }
            }

            impl<T: TimeInt> Duration for $name<T> {}

            impl<T: TimeInt> FixedPoint for $name<T> {
                type T = T;
                const SCALING_FACTOR: Fraction = Fraction::new($numer, $denom);

                /// See [Constructing a duration](trait.Duration.html#constructing-a-duration)
                fn new(value: Self::T) -> Self {
                    Self(value)
                }

                /// See [Get the integer part](trait.Duration.html#get-the-integer-part)
                fn integer(&self) -> &Self::T {
                    &self.0
                }
            }

            impl<T: TimeInt> fmt::Display for $name<T> {
                /// See [Formatting](trait.Duration.html#formatting)
                fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
                    fmt::Display::fmt(&self.0, f)
                }
            }

            impl<T: TimeInt, Rhs: Duration> ops::Add<Rhs> for $name<T>
            where
                Rhs: FixedPoint,
                Self: TryFrom<Rhs>,
            {
                type Output = Self;

                /// See [Add/Sub](trait.Duration.html#addsub)
                fn add(self, rhs: Rhs) -> Self::Output {
                    <Self as FixedPoint>::add(self, rhs)
                }
            }

            impl<T: TimeInt, Rhs: Duration> ops::Sub<Rhs> for $name<T>
            where
                Self: TryFrom<Rhs>,
                Rhs: FixedPoint,
            {
                type Output = Self;

                /// See [Add/Sub](trait.Duration.html#addsub)
                fn sub(self, rhs: Rhs) -> Self::Output {
                    <Self as FixedPoint>::sub(self, rhs)
                }
            }

            impl<T: TimeInt, Clock: crate::domain::time::Clock> ops::Add<crate::domain::time::Instant<Clock>> for $name<T>
            where
                Clock::T: TryFrom<T>,
            {
                type Output = crate::domain::time::Instant<Clock>;

                // Symmetric version of Instant + Duration
                fn add(self, rhs: crate::domain::time::Instant<Clock>) -> Self::Output {
                    rhs.checked_add(self).unwrap()
                }
            }

            impl<T: TimeInt> ops::Mul<T> for $name<T> {
                type Output = Self;

                /// See [Mul/Div](trait.Duration.html#muldiv)
                fn mul(self, rhs: T) -> Self::Output {
                    <Self as FixedPoint>::mul(self, rhs)
                }
            }

            impl<T: TimeInt> ops::Div<T> for $name<T> {
                type Output = Self;

                /// See [Mul/Div](trait.Duration.html#muldiv)
                fn div(self, rhs: T) -> Self::Output {
                    <Self as FixedPoint>::div(self, rhs)
                }
            }

            impl<T: TimeInt, Rhs: Duration> ops::Rem<Rhs> for $name<T>
            where
                Self: TryFrom<Rhs>,
                Rhs: FixedPoint,
            {
                type Output = Self;

                /// See [Remainder](trait.Duration.html#remainder)
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

                /// See [Converting from a `Generic`
                /// `Duration`](trait.Duration.html#converting-from-a-generic-duration)
                fn try_from(generic_duration: Generic<SourceInt>) -> Result<Self, Self::Error> {
                    fixed_point::FixedPoint::from_ticks(
                        generic_duration.integer,
                        generic_duration.scaling_factor,
                    )
                }
            }

            impl<T: TimeInt> From<$name<T>> for Generic<T> {
                /// See [Converting to a `Generic`
                /// `Duration`](trait.Duration.html#converting-to-a-generic-duration)
                fn from(duration: $name<T>) -> Self {
                    Self::new(*duration.integer(), $name::<T>::SCALING_FACTOR)
                }
            }
        };

        ( $name:ident, ($numer:expr, $denom:expr), ge_secs ) => {
            impl_duration![$name, ($numer, $denom)];

            // TODO: Make this more custom (seconds and higher<u32> can be `From`) and comprehensive
            // (allow u64 durations)
            impl TryFrom<$name<u32>> for core::time::Duration {
                type Error = ConversionError;

                /// See [Converting to `core`
                /// types](trait.Duration.html#converting-to-core-types)
                fn try_from(duration: $name<u32>) -> Result<Self, Self::Error> {
                    let seconds: Seconds<u64> = duration.into();
                    Ok(Self::from_secs(*seconds.integer()))
                }
            }

            impl TryFrom<core::time::Duration> for $name<u32> {
                type Error = ConversionError;

                /// See [Converting from `core`
                /// types](trait.Duration.html#converting-from-core-types)
                fn try_from(core_duration: core::time::Duration) -> Result<Self, Self::Error> {
                    let seconds = Seconds(core_duration.as_secs());
                    seconds.try_into()
                }
            }

            impl From<core::time::Duration> for $name<u64> {
                /// See [Converting from `core`
                /// types](trait.Duration.html#converting-from-core-types)
                fn from(core_duration: core::time::Duration) -> Self {
                    let seconds = Seconds(core_duration.as_secs());
                    seconds.into()
                }
            }
        };
        ( $name:ident, ($numer:expr, $denom:expr), $from_core_dur:ident, $as_core_dur:ident ) => {
            impl_duration![$name, ($numer, $denom)];

            impl<T: TimeInt> TryFrom<$name<T>> for core::time::Duration
            where
                u64: From<T>,
            {
                type Error = ConversionError;

                /// See [Converting to `core` types](trait.Duration.html#converting-to-core-types)
                fn try_from(duration: $name<T>) -> Result<Self, Self::Error> {
                    Ok(Self::$from_core_dur((*duration.integer()).into()))
                }
            }

            impl<T: TimeInt> TryFrom<core::time::Duration> for $name<T>
            where
                T: TryFrom<u128>,
            {
                type Error = ConversionError;

                /// See [Converting from `core`
                /// types](trait.Duration.html#converting-from-core-types)
                fn try_from(core_duration: core::time::Duration) -> Result<Self, Self::Error> {
                    Ok(Self(
                        core_duration
                            .$as_core_dur()
                            .try_into()
                            .map_err(|_| ConversionError::ConversionFailure)?,
                    ))
                }
            }
        };
    }
    impl_duration![Hours, (3600, 1), ge_secs];
    impl_duration![Minutes, (60, 1), ge_secs];
    impl_duration![Seconds, (1, 1), ge_secs];
    impl_duration![Milliseconds, (1, 1_000), from_millis, as_millis];
    impl_duration![Microseconds, (1, 1_000_000), from_micros, as_micros];
    impl_duration![Nanoseconds, (1, 1_000_000_000), from_nanos, as_nanos];

    macro_rules! impl_partial_eq {
        ($name:ident) => {
            impl<T: TimeInt, RhsInt: TimeInt> cmp::PartialEq<$name<RhsInt>> for $name<T>
            where
                T: TryFrom<RhsInt>,
            {
                /// See [Comparisons](trait.Duration.html#comparisons)
                fn eq(&self, rhs: &$name<RhsInt>) -> bool {
                    match T::try_from(*rhs.integer()) {
                        Ok(rhs_integer) => *self.integer() == rhs_integer,
                        Err(_) => false,
                    }
                }
            }
        };
    }
    impl_partial_eq![Hours];
    impl_partial_eq![Minutes];
    impl_partial_eq![Seconds];
    impl_partial_eq![Milliseconds];
    impl_partial_eq![Microseconds];
    impl_partial_eq![Nanoseconds];

    macro_rules! impl_big_partial_eq_small {
        ($big:ident) => {};
        ($big:ident, $($small:ident),+) => {
            $(
                impl<T: TimeInt, RhsInt: TimeInt> cmp::PartialEq<$small<RhsInt>> for $big<T>
                where
                    $small<RhsInt>: TryFrom<Self>,
                {
                    /// See [Comparisons](trait.Duration.html#comparisons)
                    fn eq(&self, rhs: &$small<RhsInt>) -> bool {
                        match $small::<RhsInt>::try_from(*self) {
                            Ok(lhs) => lhs.integer() == rhs.integer(),
                            Err(_) => false,
                        }
                    }
                }
            )+

            impl_big_partial_eq_small![$($small),+];
        };
    }
    impl_big_partial_eq_small![
        Hours,
        Minutes,
        Seconds,
        Milliseconds,
        Microseconds,
        Nanoseconds
    ];

    macro_rules! impl_small_partial_eq_big {
        ($small:ident) => {};
        ($small:ident, $($big:ident),+) => {
            $(
                impl<T: TimeInt, RhsInt: TimeInt> cmp::PartialEq<$big<RhsInt>> for $small<T>
                where
                    Self: TryFrom<$big<RhsInt>>,
                {
                    /// See [Comparisons](trait.Duration.html#comparisons)
                    fn eq(&self, rhs: &$big<RhsInt>) -> bool {
                        match Self::try_from(*rhs) {
                            Ok(rhs) => self.integer() == rhs.integer(),
                            Err(_) => false,
                        }
                    }
                }
            )+

            impl_small_partial_eq_big![$($big),+];
        };

    }
    impl_small_partial_eq_big![
        Nanoseconds,
        Microseconds,
        Milliseconds,
        Seconds,
        Minutes,
        Hours
    ];

    macro_rules! impl_partial_ord {
        ($name:ident) => {
            impl<T: TimeInt, RhsInt: TimeInt> PartialOrd<$name<RhsInt>> for $name<T>
            where
                T: TryFrom<RhsInt>,
            {
                /// See [Comparisons](trait.Duration.html#comparisons)
                fn partial_cmp(&self, rhs: &$name<RhsInt>) -> Option<core::cmp::Ordering> {
                    match T::try_from(*rhs.integer()) {
                        Ok(rhs_integer) => Some(self.integer().cmp(&rhs_integer)),
                        Err(_) => Some(core::cmp::Ordering::Less),
                    }
                }
            }
        };
    }
    impl_partial_ord![Hours];
    impl_partial_ord![Minutes];
    impl_partial_ord![Seconds];
    impl_partial_ord![Milliseconds];
    impl_partial_ord![Microseconds];
    impl_partial_ord![Nanoseconds];

    macro_rules! impl_big_partial_ord_small {
        ($big:ident) => {};
        ($big:ident, $($small:ident),+) => {
            $(
                impl<T: TimeInt, RhsInt: TimeInt> PartialOrd<$small<RhsInt>> for $big<T>
                where
                    $small<RhsInt>: TryFrom<Self>,
                {
                    /// See [Comparisons](trait.Duration.html#comparisons)
                    fn partial_cmp(&self, rhs: &$small<RhsInt>) -> Option<core::cmp::Ordering> {
                        match $small::<RhsInt>::try_from(*self) {
                            Ok(lhs) => Some(lhs.integer().cmp(rhs.integer())),
                            Err(_) => Some(core::cmp::Ordering::Greater),
                        }
                    }
                }
            )+

            impl_big_partial_ord_small![$($small),+];
        };
    }
    impl_big_partial_ord_small![
        Hours,
        Minutes,
        Seconds,
        Milliseconds,
        Microseconds,
        Nanoseconds
    ];

    macro_rules! impl_small_partial_ord_big {
        ($small:ident) => {};
        ($small:ident, $($big:ident),+) => {
            $(
                impl<T: TimeInt, RhsInt: TimeInt> PartialOrd<$big<RhsInt>> for $small<T>
                where
                    Self: TryFrom<$big<RhsInt>>,
                {
                    /// See [Comparisons](trait.Duration.html#comparisons)
                    fn partial_cmp(&self, rhs: &$big<RhsInt>) -> Option<core::cmp::Ordering> {
                        match Self::try_from(*rhs) {
                        Ok(rhs) => Some(self.integer().cmp(rhs.integer())),
                        Err(_) => Some(core::cmp::Ordering::Less),
                    }
                    }
                }
            )+

            impl_small_partial_ord_big![$($big),+];
        };

    }
    impl_small_partial_ord_big![
        Nanoseconds,
        Microseconds,
        Milliseconds,
        Seconds,
        Minutes,
        Hours
    ];

    macro_rules! impl_from {
        ($name:ident) => {
            impl From<$name<u32>> for $name<u64> {
                /// See [Converting between
                /// `Duration`s](trait.Duration.html#converting-between-durations)
                fn from(source: $name<u32>) -> Self {
                    Self::new(u64::from(*source.integer()))
                }
            }

            impl TryFrom<$name<u64>> for $name<u32> {
                type Error = ConversionError;

                /// See [Converting between
                /// `Duration`s](trait.Duration.html#converting-between-durations)
                fn try_from(source: $name<u64>) -> Result<Self, Self::Error> {
                    fixed_point::FixedPoint::from_ticks(
                        *source.integer(),
                        $name::<u64>::SCALING_FACTOR,
                    )
                }
            }
        };
    }
    impl_from![Hours];
    impl_from![Minutes];
    impl_from![Seconds];
    impl_from![Milliseconds];
    impl_from![Microseconds];
    impl_from![Nanoseconds];

    macro_rules! impl_from_smaller {
        ($name:ident) => {};
        ($big:ident, $($small:ident),+) => {
            $(
                impl<T: TimeInt> From<$small<T>> for $big<T>
                {
                    /// See [Converting between `Duration`s](trait.Duration.html#converting-between-durations)
                    fn from(small: $small<T>) -> Self {
                        fixed_point::FixedPoint::from_ticks(*small.integer(), $small::<T>::SCALING_FACTOR).ok().unwrap()
                    }
                }

                impl From<$small<u32>> for $big<u64>
                {
                    /// See [Converting between `Duration`s](trait.Duration.html#converting-between-durations)
                    fn from(small: $small<u32>) -> Self {
                        fixed_point::FixedPoint::from_ticks(*small.integer(), $small::<u32>::SCALING_FACTOR).ok().unwrap()
                    }
                }

                impl TryFrom<$small<u64>> for $big<u32>
                {
                    type Error = ConversionError;

                    /// See [Converting between `Duration`s](trait.Duration.html#converting-between-durations)
                    fn try_from(small: $small<u64>) -> Result<Self, Self::Error> {
                        fixed_point::FixedPoint::from_ticks(
                            *small.integer(),
                            $small::<u64>::SCALING_FACTOR,
                        )
                    }
                }
            )+

            impl_from_smaller![$($small),+];
        };

    }
    impl_from_smaller![
        Hours,
        Minutes,
        Seconds,
        Milliseconds,
        Microseconds,
        Nanoseconds
    ];

    macro_rules! impl_from_bigger {
        ($small:ident) => {};
        ($small:ident, $($big:ident),+) => {
            $(
                impl From<$big<u32>> for $small<u64>
                {
                    /// See [Converting between `Duration`s](trait.Duration.html#converting-between-durations)
                    fn from(big: $big<u32>) -> Self {
                        fixed_point::FixedPoint::from_ticks(*big.integer(), $big::<u32>::SCALING_FACTOR).ok().unwrap()
                    }
                }

                impl<T: TimeInt> TryFrom<$big<T>> for $small<T>
                {
                    type Error = ConversionError;

                    /// See [Converting between `Duration`s](trait.Duration.html#converting-between-durations)
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

                    /// See [Converting between `Duration`s](trait.Duration.html#converting-between-durations)
                    fn try_from(big: $big<u64>) -> Result<Self, Self::Error> {
                        fixed_point::FixedPoint::from_ticks(
                            *big.integer(),
                            $big::<u64>::SCALING_FACTOR,
                        )
                    }
                }
            )+

            impl_from_bigger![$($big),+];
        };
    }

    impl_from_bigger![
        Nanoseconds,
        Microseconds,
        Milliseconds,
        Seconds,
        Minutes,
        Hours
    ];

    /// Create duration-based extensions from primitive numeric types.
    ///
    /// ```rust
    /// use embedded_time::duration::*;
    ///
    /// assert_eq!(5_u32.nanoseconds(), Nanoseconds(5_u32));
    /// assert_eq!(5_u32.microseconds(), Microseconds(5_u32));
    /// assert_eq!(5_u32.milliseconds(), Milliseconds(5_u32));
    /// assert_eq!(5_u32.seconds(), Seconds(5_u32));
    /// assert_eq!(5_u32.minutes(), Minutes(5_u32));
    /// assert_eq!(5_u32.hours(), Hours(5_u32));
    /// ```
    pub trait Extensions: TimeInt {
        /// nanoseconds
        fn nanoseconds(self) -> Nanoseconds<Self> {
            Nanoseconds::new(self)
        }
        /// microseconds
        fn microseconds(self) -> Microseconds<Self> {
            Microseconds::new(self)
        }
        /// milliseconds
        fn milliseconds(self) -> Milliseconds<Self> {
            Milliseconds::new(self)
        }
        /// seconds
        fn seconds(self) -> Seconds<Self> {
            Seconds::new(self)
        }
        /// minutes
        fn minutes(self) -> Minutes<Self> {
            Minutes::new(self)
        }
        /// hours
        fn hours(self) -> Hours<Self> {
            Hours::new(self)
        }
    }

    impl Extensions for u32 {}

    impl Extensions for u64 {}
}

#[cfg(test)]
mod tests {
    use super::*;
}

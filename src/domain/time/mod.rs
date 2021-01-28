//! `embedded-time` provides a comprehensive library of [`Duration`] and [`Rate`] types as well as
//! a [`Clock`] abstractions for hardware timers/clocks and the associated [`Instant`] type for
//! in embedded systems.
//!
//! Additionally, an implementation of software timers is provided that work seemlessly with all
//! the types in this crate.
//!
//! ```rust
//! use embedded_time::{duration::*, rate::*};
//! # use core::convert::TryInto;
//!
//! let micros = 200_000_u32.microseconds();                // 200_000 μs
//! let millis: Milliseconds = micros.into();               // 200 ms
//! let frequency: Result<Hertz,_> = millis.to_rate();      // 5 Hz
//!
//! assert_eq!(frequency, Ok(5_u32.Hz()));
//! ```
//!
//! # Motivation
//!
//! The handling of time on embedded systems is generally much different than that of OSs. For
//! instance, on an OS, the time is measured against an arbitrary epoch. Embedded systems generally
//! don't know (nor do they care) what the *real* time is, but rather how much time has passed since
//! the system has started.
//!
//! ## Drawbacks of the standard library types
//!
//! ### Duration
//!
//! - The storage is `u64` seconds and `u32` nanoseconds.
//! - This is huge overkill and adds needless complexity beyond what is required (or desired) for
//!   embedded systems.
//! - Any read (with the exception of seconds and nanoseconds) requires arithmetic to convert to the
//!   requested units
//! - This is much slower than this project's implementation of what is analogous to a tagged union
//!   of time units.
//!
//! ### Instant
//!
//! - The `Instant` type requires `std`.
//!
//! ## Drawbacks of the [`time`](https://crates.io/crates/time) crate
//!
//! The `time` crate is a remarkable library but isn't geared for embedded systems (although it does
//! support a subset of features in `no_std` contexts). It suffers from some of the same drawbacks
//! as the core::Duration type (namely the storage format) and the `Instant` struct dependency on
//! `std`. It also adds a lot of functionally that would seldom be useful in an embedded context.
//! For instance it has a comprehensive date/time formatting, timezone, and calendar support.
//!
//! ## Background
//!
//! ### What is an Instant?
//!
//! In the Rust ecosystem, it appears to be idiomatic to call a `now()` associated function from an
//! Instant type. There is generally no concept of a "Clock". I believe that using the `Instant` in
//! this way is a violation of the *separation of concerns* principle. What is an `Instant`? Is it a
//! time-keeping entity from which you read the current instant in time, or is it that instant in
//! time itself. In this case, it's both.
//!
//! As an alternative, the current instant in time is read from a **Clock**. The `Instant` read from
//! the `Clock` has the same precision and width (inner type) as the `Clock`. Requesting the
//! difference between two `Instant`s gives a `Duration` which can have different precision and/or
//! width.
//!
//! # Overview
//!
//! The approach taken is similar to the C++ `chrono` library. [`Duration`]s and [`Rate`]s are
//! fixed-point values as in they are comprised of _integer_ and _scaling factor_ values.
//! The _scaling factor_ is a `const` [`Fraction`](fraction::Fraction). One benefit of this
//! structure is that it avoids unnecessary arithmetic. For example, if the [`Duration`] type is
//! [`Milliseconds`], a call to the [`Duration::integer()`] method simply returns the _integer_
//! part directly which in the case is the number of milliseconds represented by the [`Duration`].
//! Conversion arithmetic is only performed when explicitly converting between time units (eg.
//! [`Milliseconds`] --> [`Seconds`]).
//!
//! In addition, a wide range of rate-type types are available including [`Hertz`],
//! [`BitsPerSecond`], [`KibibytesPerSecond`], [`Baud`], etc.
//!
//! A [`Duration`] type can be converted to a [`Rate`] type and vica-versa.
//!
//! [`Seconds`]: duration::units::Seconds
//! [`Milliseconds`]: duration::units::Milliseconds
//! [`Clock`]: clock::Clock
//! [`Instant`]: instant::Instant
//! [`Rate`]: rate::Rate
//! [`Hertz`]: rate::units::Hertz
//! [`BitsPerSecond`]: rate::units::BitsPerSecond
//! [`KibibytesPerSecond`]: rate::units::KibibytesPerSecond
//! [`Baud`]: rate::units::Baud
//! [`Duration`]: duration::Duration
//! [`Duration::integer()`]: duration/trait.Duration.html#tymethod.integer
//!
//! ## Definitions
//!
//! **Clock**: Any entity that periodically counts (ie an external or peripheral hardware
//! timer/counter). Generally, this needs to be monotonic. A wrapping clock is considered monotonic
//! in this context as long as it fulfills the other requirements.
//!
//! **Wrapping Clock**: A clock that when at its maximum value, the next count is the minimum
//! value.
//!
//! **Timer**: An entity that counts toward an expiration.
//!
//! **Instant**: A specific instant in time ("time-point") read from a clock.
//!
//! **Duration**: The difference of two instants. The time that has elapsed since an instant. A
//! span of time.
//!
//! **Rate**: A measure of events per time such as frequency, data-rate, etc.
//!
//! # Imports
//!
//! The suggested use statements are as follows depending on what is needed:
//!
//! ```rust
//! use embedded_time::duration::*;    // imports all duration-related types and traits
//! use embedded_time::rate::*;        // imports all rate-related types and traits
//! use embedded_time::clock;
//! use embedded_time::Instant;
//! use embedded_time::Timer;
//! ```
//!
//! # Duration Types
//!
//! | Units        | Extension    |
//! | :----------- | :----------- |
//! | Hours        | hours        |
//! | Minutes      | minutes      |
//! | Seconds      | seconds      |
//! | Milliseconds | milliseconds |
//! | Microseconds | microseconds |
//! | Nanoseconds  | nanoseconds  |
//!
//! - Conversion from `Rate` types
//! ```rust
//! use embedded_time::{duration::*, rate::*};
//!
//! # assert!(
//! Microseconds(500_u32).to_rate() == Ok(Kilohertz(2_u32))
//! # );
//! ```
//!
//! - Conversion to/from `Generic` `Duration` type
//!
//! ```rust
//! use embedded_time::{duration::*};
//! # use core::convert::TryFrom;
//!
//! # assert!(
//! Seconds(2_u64).to_generic(Fraction::new(1, 2_000)) == Ok(Generic::new(4_000_u32, Fraction::new(1, 2_000)))
//! # );
//! # assert!(
//! Seconds::<u64>::try_from(Generic::new(2_000_u32, Fraction::new(1, 1_000))) == Ok(Seconds(2_u64))
//! # );
//! ```
//!
//! ## `core` Compatibility
//!
//! - Conversion to/from `core::time::Duration`
//!
//! ### Benchmark Comparisons to `core` duration type
//!
//! #### Construct and Read Milliseconds
//!
//! ```rust
//! use embedded_time::duration::*;
//!
//! # let ms = 100;
//! let duration = Milliseconds::<u64>(ms); // 8 bytes
//! let count = duration.integer();
//! ```
//!
//! _(the size of `embedded-time` duration types is only the size of the inner type)_
//!
//! ```rust
//! use std::time::Duration;
//!
//! # let ms = 100;
//! let core_duration = Duration::from_millis(ms); // 12 bytes
//! let count = core_duration.as_millis();
//! ```
//!
//! _(the size of `core` duration type is 12 B)_
//!
//! ![](resources/duration_violin_v0.7.0.svg)
//!
//! # Rate Types
//!
//! ## Frequency
//! | Units             | Extension |
//! | :---------------- | :-------- |
//! | Mebihertz         | MiHz      |
//! | Megahertz         | MHz       |
//! | Kibihertz         | KiHz      |
//! | Kilohertz         | kHz       |
//! | Hertz             | Hz        |
//!
//! ## Data Rate
//! | Units             | Extension |
//! | :---------------- | :-------- |
//! | MebibytePerSecond | MiBps     |
//! | MegabytePerSecond | MBps      |
//! | KibibytePerSecond | KiBps     |
//! | KiloBytePerSecond | KBps      |
//! | BytePerSecond     | Bps       |
//! |                   |           |
//! | MebibitPerSecond  | Mibps     |
//! | MegabitPerSecond  | Mbps      |
//! | KibibitPerSecond  | Kibps     |
//! | KilobitPerSecond  | kbps      |
//! | BitPerSecond      | bps       |
//!
//! ## Symbol Rate
//! | Units             | Extension |
//! | :---------------- | :-------- |
//! | Mebibaud          | MiBd      |
//! | Megabaud          | MBd       |
//! | Kibibaud          | KiBd      |
//! | Kilobaud          | kBd       |
//! | Baud              | Bd        |
//!
//! - Conversion from/to all other rate types within the same class (frequency, data rate, etc.) and
//!   _base_ (mega, mebi, kilo, kibi). For example, MiBps (mebibytes per second) --> Kibps (kibibits
//!   per second) and MBps (megabytes per second) --> kbps (kilobits per second).
//!
//! - Conversion from `Duration` types
//!
//! ```rust
//! use embedded_time::{duration::*, rate::*};
//! # use core::convert::TryFrom;
//!
//! # assert!(
//! Kilohertz(500_u32).to_duration() == Ok(Microseconds(2_u32))
//! # );
//! ```
//!
//! - Conversion to/from `Generic` `Rate` type
//!
//! ```rust
//! use embedded_time::rate::*;
//! # use core::convert::TryFrom;
//!
//! # assert!(
//! Hertz(2_u64).to_generic(Fraction::new(1,2_000)) == Ok(Generic::new(4_000_u32, Fraction::new(1,2_000)))
//! # );
//! # assert!(
//! Hertz::<u64>::try_from(Generic::new(2_000_u32, Fraction::new(1,1_000))) == Ok(Hertz(2_u64))
//! # );
//! ```
//!
//! # Hardware Abstraction
//!
//! - `Clock` trait allowing abstraction of hardware timers/clocks for timekeeping.
//!
//! # Timers
//!
//! - Software timers spawned from a `Clock` impl object.
//! - One-shot or periodic/continuous
//! - Blocking delay
//! - Poll for expiration
//! - Read elapsed/remaining duration
//!
//! # Reliability and Usability
//! - Extensive tests
//! - Thorough documentation with examples
//! - Example for the nRF52_DK board
//!
//! # Notes
//! Some parts of this crate were derived from various sources:
//! - [`RTIC`](https://github.com/rtic-rs/cortex-m-rtic)
//! - [`time`](https://docs.rs/time/latest/time) (Specifically the [`time::NumbericalDuration`](https://docs.rs/time/latest/time/trait.NumericalDuration.html)
//!   implementations for primitive integers)
#![doc(html_root_url = "https://docs.rs/embedded-time/0.10.1")]
#![deny(unsafe_code)]
#![warn(missing_docs)]
#![deny(broken_intra_doc_links)]

pub mod clock;
pub mod duration;
pub mod fixed_point;
pub mod fraction;
mod instant;
pub mod rate;
mod time_int;
mod timer;

pub use clock::Clock;
pub use instant::Instant;
pub use timer::Timer;

/// Crate errors
#[non_exhaustive]
#[derive(Debug, Eq, PartialEq, Hash)]
pub enum TimeError {
    /// Exact cause of failure is unknown
    Unspecified,
    /// Attempted type conversion failed
    ConversionFailure,
    /// Result is outside of those valid for this type
    Overflow,
    /// Attempted to divide by zero
    DivByZero,
    /// Resulting [`Duration`](duration/trait.Duration.html) is negative (not allowed)
    NegDuration,
    /// [`Clock`]-implementation-specific error
    Clock(clock::Error),
}

impl From<clock::Error> for TimeError {
    fn from(clock_error: clock::Error) -> Self {
        TimeError::Clock(clock_error)
    }
}

impl Default for TimeError {
    fn default() -> Self {
        Self::Unspecified
    }
}

/// Conversion errors
#[non_exhaustive]
#[derive(Debug, Eq, PartialEq, Hash)]
pub enum ConversionError {
    /// Exact cause of failure is unknown
    Unspecified,
    /// Attempted type conversion failed
    ConversionFailure,
    /// Result is outside of those valid for this type
    Overflow,
    /// Attempted to divide by zero
    DivByZero,
    /// Resulting [`Duration`](duration/trait.Duration.html) is negative (not allowed)
    NegDuration,
}

impl From<ConversionError> for TimeError {
    fn from(error: ConversionError) -> Self {
        match error {
            ConversionError::Unspecified => TimeError::Unspecified,
            ConversionError::ConversionFailure => TimeError::ConversionFailure,
            ConversionError::Overflow => TimeError::Overflow,
            ConversionError::DivByZero => TimeError::DivByZero,
            ConversionError::NegDuration => TimeError::NegDuration,
        }
    }
}

impl Default for ConversionError {
    fn default() -> Self {
        Self::Unspecified
    }
}

#[cfg(test)]
mod tests {}

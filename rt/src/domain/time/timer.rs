use crate::domain::time::fraction::Fraction;
use crate::domain::time::{
    duration::{self, *},
    fixed_point::FixedPoint,
    timer::param::*,
    ConversionError, Instant, TimeError,
};
use core::{convert::TryFrom, marker::PhantomData, ops::Add, prelude::v1::*};

pub(crate) mod param {
    #[derive(Debug, Hash)]
    pub struct None;

    #[derive(Debug, Hash)]
    pub struct Armed;

    #[derive(Debug, Hash)]
    pub struct Running;

    #[derive(Debug, Hash)]
    pub struct Periodic;

    #[derive(Debug, Hash)]
    pub struct OneShot;
}

/// A `Timer` counts toward an expiration, can be polled for elapsed and remaining time, and can be
/// one-shot or continuous/periodic.
#[derive(Debug, Hash)]
pub struct Timer<'a, Type, State, Clock: crate::domain::time::Clock, Dur: Duration> {
    clock: &'a Clock,
    duration: Dur,
    expiration: Instant<Clock>,
    _type: PhantomData<Type>,
    _state: PhantomData<State>,
}

impl<'a, Clock: crate::domain::time::Clock, Dur: Duration>
    Timer<'_, param::None, param::None, Clock, Dur>
{
    /// Construct a new, `OneShot` `Timer`
    #[allow(clippy::new_ret_no_self)]
    pub fn new(clock: &Clock, duration: Dur) -> Timer<OneShot, Armed, Clock, Dur> {
        Timer::<OneShot, Armed, Clock, Dur> {
            clock,
            duration,
            expiration: Instant::new(Clock::T::from(0)),
            _type: PhantomData,
            _state: PhantomData,
        }
    }
}

impl<'a, Type, State, Clock: crate::domain::time::Clock, Dur: Duration>
    Timer<'a, Type, State, Clock, Dur>
{
    /// Change timer type to one-shot
    pub fn into_oneshot(self) -> Timer<'a, OneShot, State, Clock, Dur> {
        Timer::<OneShot, State, Clock, Dur> {
            clock: self.clock,
            duration: self.duration,
            expiration: self.expiration,
            _type: PhantomData,
            _state: PhantomData,
        }
    }

    /// Change timer type into periodic
    pub fn into_periodic(self) -> Timer<'a, Periodic, State, Clock, Dur> {
        Timer::<Periodic, State, Clock, Dur> {
            clock: self.clock,
            duration: self.duration,
            expiration: self.expiration,
            _type: PhantomData,
            _state: PhantomData,
        }
    }
}

impl<'a, Type, Clock: crate::domain::time::Clock, Dur: Duration>
    Timer<'a, Type, Armed, Clock, Dur>
{
    /// Start the timer from this instant
    pub fn start(self) -> Result<Timer<'a, Type, Running, Clock, Dur>, TimeError>
    where
        Clock::T: TryFrom<Dur::T>,
        Dur: FixedPoint,
    {
        Ok(Timer::<Type, Running, Clock, Dur> {
            clock: self.clock,
            duration: self.duration,
            expiration: self
                .clock
                .try_now()?
                .checked_add(self.duration)
                .ok_or(ConversionError::Overflow)?,
            _type: PhantomData,
            _state: PhantomData,
        })
    }
}

impl<Type, Clock: crate::domain::time::Clock, Dur: Duration> Timer<'_, Type, Running, Clock, Dur> {
    fn _is_expired(&self) -> Result<bool, TimeError> {
        Ok(self.clock.try_now()? >= self.expiration)
    }

    /// Returns the [`Duration`] of time elapsed since it was started
    ///
    /// **The duration is truncated, not rounded**.
    ///
    /// The units of the [`Duration`] are the same as that used to construct the `Timer`.
    pub fn elapsed(&self) -> Result<Dur, TimeError>
    where
        Dur: FixedPoint + TryFrom<duration::Generic<Clock::T>, Error = ConversionError>,
        Dur::T: TryFrom<Clock::T>,
        Clock::T: TryFrom<Dur::T>,
    {
        let generic_duration = self
            .clock
            .try_now()?
            .checked_duration_since(
                &(self
                    .expiration
                    .checked_sub(self.duration)
                    .ok_or(ConversionError::Overflow)?),
            )
            .ok_or(TimeError::Overflow)?;

        Ok(Dur::try_from(generic_duration)?)
    }

    /// Returns the [`Duration`] until the expiration of the timer
    ///
    /// **The duration is truncated, not rounded**.
    ///
    /// The units of the [`Duration`] are the same as that used to construct the `Timer`.
    pub fn remaining(&self) -> Result<Dur, TimeError>
    where
        Dur: FixedPoint + TryFrom<duration::Generic<Clock::T>, Error = ConversionError>,
        Dur::T: TryFrom<u32> + TryFrom<Clock::T>,
        Clock::T: TryFrom<Dur::T>,
    {
        let result = self
            .expiration
            .checked_duration_since(&self.clock.try_now()?)
            .or_else(|| {
                Some(duration::Generic::<Clock::T>::new(
                    0.into(),
                    Fraction::default(),
                ))
            })
            .ok_or(TimeError::NegDuration)?;

        Ok(Dur::try_from(result)?)
    }
}

impl<'a, Clock: crate::domain::time::Clock, Dur: Duration> Timer<'a, OneShot, Running, Clock, Dur> {
    /// Block until the timer has expired
    pub fn wait(self) -> Result<Timer<'a, OneShot, Armed, Clock, Dur>, TimeError> {
        // since the timer is running, _is_expired() will return a value
        while !self._is_expired()? {}

        Ok(Timer::<param::None, param::None, Clock, Dur>::new(
            self.clock,
            self.duration,
        ))
    }

    /// Check whether the timer has expired
    ///
    /// The timer is not restarted
    pub fn is_expired(&self) -> Result<bool, TimeError> {
        self._is_expired()
    }
}

impl<Clock: crate::domain::time::Clock, Dur: Duration> Timer<'_, Periodic, Running, Clock, Dur> {
    /// Block until the timer has expired
    ///
    /// The timer is restarted
    pub fn wait(self) -> Result<Self, TimeError>
    where
        Instant<Clock>: Add<Dur, Output = Instant<Clock>>,
    {
        // since the timer is running, _is_expired() will return a value
        while !self._is_expired()? {}

        Ok(Self {
            clock: self.clock,
            duration: self.duration,
            // The `+` will never panic since this duration has already applied to the same
            // `Instant` type without a problem
            expiration: self.expiration + self.duration,
            _type: PhantomData,
            _state: PhantomData,
        })
    }

    /// Check whether a _periodic_ timer has elapsed
    ///
    /// The timer is restarted if it has elapsed.
    pub fn period_complete(&mut self) -> Result<bool, TimeError>
    where
        Instant<Clock>: Add<Dur, Output = Instant<Clock>>,
    {
        // since the timer is running, _is_expired() will return a value
        if self._is_expired()? {
            // The `+` will never panic since this duration has already applied to the same
            // `Instant` type without a problem
            self.expiration = self.expiration + self.duration;

            Ok(true)
        } else {
            Ok(false)
        }
    }
}

#[cfg(test)]
mod test {}

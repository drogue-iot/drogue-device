//! Types and traits related to temperature.

use core::fmt::{Debug, Display, Formatter};
use core::marker::PhantomData;
use core::ops::{Add, Div, Sub};

/// Trait representing a temperature scale.
pub trait TemperatureScale: Send {
    const LETTER: char;
}

/// Discriminant for the _Kelvin_ temperature scale.
#[derive(Clone)]
pub struct Kelvin;

impl TemperatureScale for Kelvin {
    const LETTER: char = 'K';
}

impl Debug for Kelvin {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        f.write_str("°K")
    }
}

#[cfg(feature = "defmt")]
impl defmt::Format for Kelvin {
    fn format(&self, f: defmt::Formatter<'_>) {
        defmt::write!(f, "°K");
    }
}

/// Discriminant for the _Celsius_ temperature scale.
#[derive(Clone)]
pub struct Celsius;

impl Debug for Celsius {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        f.write_str("°C")
    }
}

#[cfg(feature = "defmt")]
impl defmt::Format for Celsius {
    fn format(&self, f: defmt::Formatter<'_>) {
        defmt::write!(f, "°C");
    }
}

impl TemperatureScale for Celsius {
    const LETTER: char = 'C';
}

/// Discriminant for the _Fahrenheit_ temperature scale.
#[derive(Clone)]
pub struct Fahrenheit;

impl Debug for Fahrenheit {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        f.write_str("°F")
    }
}

#[cfg(feature = "defmt")]
impl defmt::Format for Fahrenheit {
    fn format(&self, f: defmt::Formatter<'_>) {
        defmt::write!(f, "°F");
    }
}

impl TemperatureScale for Fahrenheit {
    const LETTER: char = 'F';
}

/// A temperature value with its associated scale.
pub struct Temperature<S: TemperatureScale> {
    value: f32,
    _marker: PhantomData<S>,
}

impl<S: TemperatureScale> Clone for Temperature<S> {
    fn clone(&self) -> Self {
        Self {
            value: self.value,
            _marker: PhantomData::default(),
        }
    }
}

impl<S: TemperatureScale> Debug for Temperature<S> {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}°{}", &self.value, S::LETTER)
    }
}

#[cfg(feature = "defmt")]
impl<S: TemperatureScale> defmt::Format for Temperature<S> {
    fn format(&self, f: defmt::Formatter<'_>) {
        defmt::write!(f, "{}°{}", &self.value, S::LETTER)
    }
}

impl<S: TemperatureScale> Copy for Temperature<S> {}

impl<S: TemperatureScale> Temperature<S> {
    pub fn new(value: f32) -> Self {
        Self {
            value,
            _marker: PhantomData::default(),
        }
    }

    pub fn raw_value(&self) -> f32 {
        self.value
    }
}

impl Temperature<Celsius> {
    pub fn into_fahrenheit(self) -> Temperature<Fahrenheit> {
        Temperature::new((self.value * 9.0 / 5.0) + 32.0)
    }
}

impl Into<Temperature<Celsius>> for i16 {
    fn into(self) -> Temperature<Celsius> {
        Temperature::<Celsius>::new(self as f32)
    }
}

impl Into<Temperature<Celsius>> for f32 {
    fn into(self) -> Temperature<Celsius> {
        Temperature::<Celsius>::new(self as f32)
    }
}

impl<S: TemperatureScale> Sub for Temperature<S> {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self::new(self.value - rhs.value)
    }
}

impl<S: TemperatureScale> Add for Temperature<S> {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self::new(self.value + rhs.value)
    }
}

impl<S: TemperatureScale> Add<f32> for Temperature<S> {
    type Output = Self;

    fn add(self, rhs: f32) -> Self::Output {
        Self::new(self.value + rhs)
    }
}

impl<S: TemperatureScale> Div<f32> for Temperature<S> {
    type Output = f32;

    fn div(self, rhs: f32) -> Self::Output {
        self.value / rhs
    }
}

impl<S: TemperatureScale> Display for Temperature<S> {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        Display::fmt(&self.value, f)?;
        write!(f, "°{}", S::LETTER)
    }
}

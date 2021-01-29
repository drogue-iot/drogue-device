//! Types and traits related to temperature.


use core::fmt::{Display, Formatter, Debug};
use core::marker::PhantomData;
use core::ops::{Add, Div, Sub};

/// Trait representing a temperature scale.
pub trait TemperatureScale {}

/// Discriminant for the _Kelvin_ temperature scale.
pub struct Kelvin;

impl TemperatureScale for Kelvin {}

impl Debug for Kelvin {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        f.write_str( "°K")
    }
}

/// Discriminant for the _Celsius_ temperature scale.
pub struct Celsius;

impl Debug for Celsius {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        f.write_str( "°C")
    }
}

impl TemperatureScale for Celsius {}

/// Discriminant for the _Fahrenheit_ temperature scale.
pub struct Fahrenheit;

impl Debug for Fahrenheit {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        f.write_str( "°F")
    }
}

impl TemperatureScale for Fahrenheit {}


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

impl Debug for Temperature<Celsius> {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        Debug::fmt( &self.value, f )?;
        write!(f, "°C")
    }
}

impl Debug for Temperature<Fahrenheit> {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        Debug::fmt( &self.value, f )?;
        write!(f, "°F")
    }
}

impl Debug for Temperature<Kelvin> {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        Debug::fmt( &self.value, f )?;
        write!(f, "°K")
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
        Display::fmt(&self.value, f)
    }
}

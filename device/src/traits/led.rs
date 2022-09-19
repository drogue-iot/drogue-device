use crate::domain::led::matrix::*;
use core::convert::Infallible;
use embedded_hal::digital::v2::OutputPin;

pub trait Led {
    type Error;
    fn on(&mut self) -> Result<(), Self::Error>;
    fn off(&mut self) -> Result<(), Self::Error>;
}

impl<P> Led for P
where
    P: OutputPin,
{
    type Error = Infallible;
    fn on(&mut self) -> Result<(), Self::Error> {
        self.set_high().ok();
        Ok(())
    }

    fn off(&mut self) -> Result<(), Self::Error> {
        self.set_low().ok();
        Ok(())
    }
}

#[cfg(feature = "time")]
pub trait TextDisplay {
    type Error;

    type ScrollFuture<'m>: core::future::Future<Output = Result<(), Self::Error>>
    where
        Self: 'm;
    fn scroll<'m>(&'m mut self, text: &'m str) -> Self::ScrollFuture<'m>;

    type ScrollWithSpeedFuture<'m>: core::future::Future<Output = Result<(), Self::Error>>
    where
        Self: 'm;

    fn scroll_with_speed<'m>(
        &'m mut self,
        text: &'m str,
        speed: embassy_time::Duration,
    ) -> Self::ScrollWithSpeedFuture<'m>;

    type DisplayFuture<'m>: core::future::Future<Output = Result<(), Self::Error>>
    where
        Self: 'm;
    fn display<'m>(
        &'m mut self,
        c: char,
        duration: embassy_time::Duration,
    ) -> Self::DisplayFuture<'m>;
}
/*
pub trait LedMatrix<const ROWS: usize, const COLS: usize> {
    type Error;

    type OnFuture<'m>: core::future::Future<Output = Result<(), Self::Error>>
    where
        Self: 'm;
    fn on<'m>(&'m mut self, x: usize, y: usize) -> Self::OnFuture<'m>;

    type OffFuture<'m>: core::future::Future<Output = Result<(), Self::Error>>
    where
        Self: 'm;
    fn off<'m>(&'m mut self, x: usize, y: usize) -> Self::OffFuture<'m>;

    type ClearFuture<'m>: core::future::Future<Output = Result<(), Self::Error>>
    where
        Self: 'm;
    fn clear<'m>(&'m mut self) -> Self::ClearFuture<'m>;

    type ApplyFuture<'m>: core::future::Future<Output = Result<(), Self::Error>>
    where
        Self: 'm;
    fn apply<'m>(&'m mut self, frame: &'m dyn ToFrame<COLS, ROWS>) -> Self::ApplyFuture<'m>;

    fn max_brightness(&mut self) -> Result<(), Self::Error>;
    fn min_brightness(&mut self) -> Result<(), Self::Error>;
    fn increase_brightness(&mut self) -> Result<(), Self::Error>;
    fn decrease_brightness(&mut self) -> Result<(), Self::Error>;
    fn enable(&mut self) -> Result<(), Self::Error>;
    fn disable(&mut self) -> Result<(), Self::Error>;
}
*/

pub trait ToFrame<const XSIZE: usize, const YSIZE: usize>: Sync {
    fn to_frame(&self) -> Frame<XSIZE, YSIZE>;
}

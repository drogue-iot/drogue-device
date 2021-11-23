use crate::domain::led::matrix::*;

pub trait Led {
    type Error;
    fn on(&mut self) -> Result<(), Self::Error>;
    fn off(&mut self) -> Result<(), Self::Error>;
    fn toggle(&mut self) -> Result<(), Self::Error>;
    fn state(&self) -> Result<bool, Self::Error>;
}

pub trait TextDisplay {
    type Error;

    type ScrollFuture<'m>: core::future::Future<Output = Result<(), Self::Error>>
    where
        Self: 'm;
    fn scroll<'m>(&'m mut self, text: &'m str) -> Self::ScrollFuture<'m>;

    fn putc(&mut self, c: char) -> Result<(), Self::Error>;
}

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

    fn increase_brightness(&mut self) -> Result<(), Self::Error>;
    fn decrease_brightness(&mut self) -> Result<(), Self::Error>;
}

pub trait ToFrame<const XSIZE: usize, const YSIZE: usize>: Sync {
    fn to_frame(&self) -> Frame<XSIZE, YSIZE>;
}

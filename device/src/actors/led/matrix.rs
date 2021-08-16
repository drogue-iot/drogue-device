use crate::drivers::led::matrix::*;
use crate::kernel::{actor::Actor, actor::Address, actor::Inbox};
use core::future::Future;
use embassy::time::{with_timeout, Duration, TimeoutError};
use embedded_hal::digital::v2::OutputPin;

<<<<<<< HEAD
pub trait LedMatrixAddress {
    fn apply(&mut self, frame: &'static dyn ToFrame);
}

impl<P, const ROWS: usize, const COLS: usize> LedMatrixAddress
    for Address<'static, LedMatrixActor<P, ROWS, COLS>>
where
    P: OutputPin + 'static,
{
    fn apply(&mut self, frame: &'static dyn ToFrame) {
        self.notify(MatrixCommand::ApplyFrame(frame)).unwrap();
    }
}

pub struct LedMatrixActor<P, const ROWS: usize, const COLS: usize>
=======
pub struct LedMatrixDriver<P, const ROWS: usize, const COLS: usize>
>>>>>>> ca05993 (Fix)
where
    P: OutputPin + 'static,
{
    matrix: LedMatrix<P, ROWS, COLS>,
}

impl<P, const ROWS: usize, const COLS: usize> LedMatrixActor<P, ROWS, COLS>
where
    P: OutputPin + 'static,
{
    fn new(
        refresh_interval: Duration,
        matrix: LedMatrix<P, ROWS, COLS>,
    ) -> LedMatrixActor<P, ROWS, COLS> {
        Self {
            refresh_interval,
            matrix,
        }
    }
}

impl<P, const ROWS: usize, const COLS: usize> Actor for LedMatrixActor<P, ROWS, COLS>
where
    P: OutputPin,
{
    #[rustfmt::skip]
    type Message<'m> = MatrixCommand<'m>;
    #[rustfmt::skip]
    type OnMountFuture<'m, M> where P: 'm, M: 'm = impl Future<Output = ()> + 'm;

    fn on_mount<'m, M>(
        &'m mut self,
        _: Self::Configuration,
        _: Address<'static, Self>,
        inbox: &'m mut M,
    ) -> Self::OnMountFuture<'m, M>
    where
        M: Inbox<'m, Self> + 'm,
    {
        async move {
            loop {
                match with_timeout(self.refresh_interval, inbox.next()).await {
                    Ok(Some(mut m)) => match *m.message() {
                        MatrixCommand::ApplyFrame(f) => self.matrix.apply(f.to_frame()),
                        MatrixCommand::On(x, y) => self.matrix.on(x, y),
                        MatrixCommand::Off(x, y) => self.matrix.off(x, y),
                        MatrixCommand::Clear => self.matrix.clear(),
                        MatrixCommand::Render => {
                            self.matrix.render();
                        }
                    },
                    Err(TimeoutError) => {
                        self.matrix.render();
                    }
                    _ => {}
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum MatrixCommand<'m> {
    On(usize, usize),
    Off(usize, usize),
    Clear,
    Render,
    ApplyFrame(&'m dyn ToFrame),
}

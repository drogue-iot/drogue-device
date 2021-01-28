use crate::bind::Bind;
use crate::domain::time::duration::Milliseconds;
use crate::domain::time::rate::{Hertz, Rate};

use crate::driver::timer::Timer;
use crate::hal::timer::Timer as HalTimer;
use crate::prelude::*;
use embedded_hal::digital::v2::OutputPin;
use heapless::{ArrayLength, Vec};

// Led matrix driver supporting up to 32x32 led matrices.
pub struct LEDMatrix<D, P, ROWS, COLS, T>
where
    D: Device,
    P: OutputPin,
    ROWS: ArrayLength<P>,
    COLS: ArrayLength<P>,
    T: HalTimer,
{
    address: Option<Address<D, Self>>,
    pin_rows: Vec<P, ROWS>,
    pin_cols: Vec<P, COLS>,
    frame_buffer: FrameBuffer,
    row_p: usize,
    timer: Option<Address<D, Timer<D, T>>>,
    refresh_rate: Hertz,
}

struct FrameBuffer(u32, u32);

impl<D, P, ROWS, COLS, T> LEDMatrix<D, P, ROWS, COLS, T>
where
    D: Device,
    P: OutputPin,
    ROWS: ArrayLength<P>,
    COLS: ArrayLength<P>,
    T: HalTimer,
{
    pub fn new(pin_rows: Vec<P, ROWS>, pin_cols: Vec<P, COLS>, refresh_rate: Hertz) -> Self {
        LEDMatrix {
            address: None,
            pin_rows,
            pin_cols,
            frame_buffer: FrameBuffer(0, 0),
            row_p: 0,
            refresh_rate,
            timer: None,
        }
    }

    pub fn clear(&mut self) {
        self.frame_buffer.0 = 0;
        self.frame_buffer.1 = 0;
    }

    pub fn on(&mut self, x: usize, y: usize) {
        self.frame_buffer.0 |= 1 << x;
        self.frame_buffer.1 |= 1 << y;
    }

    pub fn off(&mut self, x: usize, y: usize) {
        self.frame_buffer.0 &= !(1 << x);
        self.frame_buffer.1 &= !(1 << y);
    }

    pub fn render(&mut self) {
        for row in self.pin_rows.iter_mut() {
            row.set_low().ok();
        }

        let mut cid = 0;
        for col in self.pin_cols.iter_mut() {
            if (self.frame_buffer.0 & (1 << self.row_p) == 1)
                && (self.frame_buffer.1 & (1 << cid) == 1)
            {
                col.set_low().ok();
            } else {
                col.set_high().ok();
            }
            cid += 1;
        }
        self.pin_rows[self.row_p].set_high().ok();
        self.row_p = (self.row_p + 1) % self.pin_rows.len();
    }
}

impl<D, P, ROWS, COLS, T> Bind<D, Timer<D, T>> for LEDMatrix<D, P, ROWS, COLS, T>
where
    D: Device,
    P: OutputPin,
    ROWS: ArrayLength<P>,
    COLS: ArrayLength<P>,
    T: HalTimer,
{
    fn on_bind(&'static mut self, address: Address<D, Timer<D, T>>) {
        self.timer.replace(address);
    }
}

impl<D, P, ROWS, COLS, T> Actor<D> for LEDMatrix<D, P, ROWS, COLS, T>
where
    D: Device,
    P: OutputPin,
    ROWS: ArrayLength<P>,
    COLS: ArrayLength<P>,
    T: HalTimer,
{
    fn mount(&mut self, address: Address<D, Self>, _: EventBus<D>) {
        self.address.replace(address);
    }
}

impl<D, P, ROWS, COLS, T> NotificationHandler<Lifecycle> for LEDMatrix<D, P, ROWS, COLS, T>
where
    D: Device,
    P: OutputPin,
    ROWS: ArrayLength<P>,
    COLS: ArrayLength<P>,
    T: HalTimer,
{
    fn on_notification(&'static mut self, message: Lifecycle) -> Completion {
        if let Lifecycle::Start = message {
            if let Some(address) = &self.address {
                self.timer.as_ref().unwrap().schedule(
                    self.refresh_rate.to_duration::<Milliseconds>().unwrap(),
                    MatrixCommand::Render,
                    address.clone(),
                );
            }
            Completion::immediate()
        } else {
            Completion::immediate()
        }
    }
}

impl<D, P, ROWS, COLS, T> NotificationHandler<MatrixCommand> for LEDMatrix<D, P, ROWS, COLS, T>
where
    D: Device,
    P: OutputPin,
    ROWS: ArrayLength<P>,
    COLS: ArrayLength<P>,
    T: HalTimer,
{
    fn on_notification(&'static mut self, command: MatrixCommand) -> Completion {
        match command {
            MatrixCommand::On(x, y) => {
                self.on(x, y);
            }
            MatrixCommand::Off(x, y) => {
                self.off(x, y);
            }
            MatrixCommand::Render => {
                self.render();
                if let Some(address) = &self.address {
                    self.timer.as_ref().unwrap().schedule(
                        self.refresh_rate.to_duration::<Milliseconds>().unwrap(),
                        MatrixCommand::Render,
                        address.clone(),
                    );
                }
            }
        }
        Completion::immediate()
    }
}

#[derive(Debug, PartialEq, Copy, Clone, Eq)]
pub enum MatrixCommand {
    On(usize, usize),
    Off(usize, usize),
    Render,
}

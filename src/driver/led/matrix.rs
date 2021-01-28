use crate::bind::Bind;
use crate::domain::time::duration::Milliseconds;
use crate::domain::time::rate::{Hertz, Rate};

use crate::driver::timer::{HardwareTimer, Timer};
use crate::prelude::*;
use embedded_hal::digital::v2::OutputPin;
use heapless::{ArrayLength, Vec};

// Led matrix driver supporting up to 32x32 led matrices.
pub struct LEDMatrix<D, P, ROWS, COLS, TIM, T>
where
    D: Device,
    P: OutputPin,
    ROWS: ArrayLength<P>,
    COLS: ArrayLength<P>,
    T: HardwareTimer<TIM>,
{
    address: Option<Address<D, Self>>,
    pin_rows: Vec<P, ROWS>,
    pin_cols: Vec<P, COLS>,
    frame_buffer: FrameBuffer,
    row_p: usize,
    timer: Option<Address<D, Timer<D, TIM, T>>>,
    refresh_rate: Hertz,
}

struct FrameBuffer(u32, u32);

impl<D, P, ROWS, COLS, TIM, T> LEDMatrix<D, P, ROWS, COLS, TIM, T>
where
    D: Device,
    P: OutputPin,
    ROWS: ArrayLength<P>,
    COLS: ArrayLength<P>,
    T: HardwareTimer<TIM>,
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

impl<D, P, ROWS, COLS, TIM, T> Bind<D, Timer<D, TIM, T>> for LEDMatrix<D, P, ROWS, COLS, TIM, T>
where
    D: Device,
    P: OutputPin,
    ROWS: ArrayLength<P>,
    COLS: ArrayLength<P>,
    T: HardwareTimer<TIM>,
{
    fn on_bind(&'static mut self, address: Address<D, Timer<D, TIM, T>>) {
        self.timer.replace(address);
    }
}

impl<D, P, ROWS, COLS, TIM, T> Actor<D> for LEDMatrix<D, P, ROWS, COLS, TIM, T>
where
    D: Device,
    P: OutputPin,
    ROWS: ArrayLength<P>,
    COLS: ArrayLength<P>,
    T: HardwareTimer<TIM>,
{
    fn mount(&mut self, address: Address<D, Self>, _: EventBus<D>) {
        self.address.replace(address);
    }
}

impl<D, P, ROWS, COLS, TIM, T> NotificationHandler<Lifecycle>
    for LEDMatrix<D, P, ROWS, COLS, TIM, T>
where
    D: Device,
    P: OutputPin,
    ROWS: ArrayLength<P>,
    COLS: ArrayLength<P>,
    T: HardwareTimer<TIM>,
{
    fn on_notification(&'static mut self, message: Lifecycle) -> Completion {
        if let Lifecycle::Start = message {
            /*
            Completion::defer(async move {
                loop {
                    self.timer
                        .as_ref()
                        .unwrap()
                        .delay(self.refresh_rate.to_duration::<Milliseconds>().unwrap())
                        .await;
                    log::info!("RENDER");
                    self.render();
                }
            })*/
            if let Some(address) = &self.address {
                log::info!("Scheduling event");
                self.timer.as_ref().unwrap().schedule(
                    self.refresh_rate.to_duration::<Milliseconds>().unwrap(),
                    MatrixCommand::Render,
                    address.clone(),
                );
                log::info!("Awaiting render");
            }
            Completion::immediate()
        } else {
            Completion::immediate()
        }
    }
}

impl<D, P, ROWS, COLS, TIM, T> NotificationHandler<MatrixCommand>
    for LEDMatrix<D, P, ROWS, COLS, TIM, T>
where
    D: Device,
    P: OutputPin,
    ROWS: ArrayLength<P>,
    COLS: ArrayLength<P>,
    T: HardwareTimer<TIM>,
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
                log::info!("Going to render!");
                self.render();
                if let Some(address) = &self.address {
                    self.timer.as_ref().unwrap().schedule(
                        self.refresh_rate.to_duration::<Milliseconds>().unwrap(),
                        MatrixCommand::Render,
                        address.clone(),
                    );
                    log::info!("Scheduled again");
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

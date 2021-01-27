use crate::prelude::*;
use embedded_hal::digital::v2::OutputPin;
use heapless::{ArrayLength, Vec};

// Led matrix driver supporting up to 32x32 led matrices.
pub struct LEDMatrix<P, ROWS, COLS>
where
    P: OutputPin,
    ROWS: ArrayLength<P>,
    COLS: ArrayLength<P>,
{
    pin_rows: Vec<P, ROWS>,
    pin_cols: Vec<P, COLS>,
    frame_buffer: FrameBuffer,
    row_p: usize,
}

struct FrameBuffer(u32, u32);

impl<P, ROWS, COLS> LEDMatrix<P, ROWS, COLS>
where
    P: OutputPin,
    ROWS: ArrayLength<P>,
    COLS: ArrayLength<P>,
{
    pub fn new(pin_rows: Vec<P, ROWS>, pin_cols: Vec<P, COLS>) -> Self {
        LEDMatrix {
            pin_rows,
            pin_cols,
            frame_buffer: FrameBuffer(0, 0),
            row_p: 0,
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

impl<D: Device, P, ROWS, COLS> Actor<D> for LEDMatrix<P, ROWS, COLS>
where
    P: OutputPin,
    ROWS: ArrayLength<P>,
    COLS: ArrayLength<P>,
{
}

impl<P, ROWS, COLS> NotificationHandler<Lifecycle> for LEDMatrix<P, ROWS, COLS>
where
    P: OutputPin,
    ROWS: ArrayLength<P>,
    COLS: ArrayLength<P>,
{
    fn on_notification(&'static mut self, message: Lifecycle) -> Completion {
        Completion::immediate()
    }
}

impl<P, ROWS, COLS> NotificationHandler<MatrixCommand> for LEDMatrix<P, ROWS, COLS>
where
    P: OutputPin,
    ROWS: ArrayLength<P>,
    COLS: ArrayLength<P>,
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

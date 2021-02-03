use crate::bind::Bind;
use crate::domain::time::duration::Milliseconds;
use crate::domain::time::rate::{Hertz, Rate};

use crate::driver::timer::Timer;
use crate::hal::timer::Timer as HalTimer;
use crate::prelude::*;
use embedded_hal::digital::v2::OutputPin;
use heapless::{ArrayLength, Vec};

// Led matrix driver supporting up to 32x32 led matrices.
pub struct LEDMatrix<P, ROWS, COLS, T>
where
    P: OutputPin,
    ROWS: ArrayLength<P>,
    COLS: ArrayLength<P>,
    T: HalTimer,
{
    address: Option<Address<Self>>,
    pin_rows: Vec<P, ROWS>,
    pin_cols: Vec<P, COLS>,
    frame_buffer: Frame,
    row_p: usize,
    timer: Option<Address<Timer<T>>>,
    refresh_rate: Hertz,
}

/**
 * A 32x32 bitmap that can be displayed on a LED matrix.
 */
pub struct Frame {
    bitmap: [u32; 32],
}

impl Frame {
    fn new(bitmap: [u32; 32]) -> Self {
        Self { bitmap }
    }

    fn clear(&mut self) {
        for m in self.bitmap.iter_mut() {
            *m = 0;
        }
    }

    fn set(&mut self, x: usize, y: usize) {
        self.bitmap[x] |= 1 << y;
    }

    fn unset(&mut self, x: usize, y: usize) {
        self.bitmap[x] &= !(1 << y);
    }

    fn is_set(&self, x: usize, y: usize) -> bool {
        (self.bitmap[x] & (1u32 << y)) >> y == 1
    }
}

impl<P, ROWS, COLS, T> LEDMatrix<P, ROWS, COLS, T>
where
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
            frame_buffer: Frame::new([0; 32]),
            row_p: 0,
            refresh_rate,
            timer: None,
        }
    }

    pub fn clear(&mut self) {
        self.frame_buffer.clear();
    }

    pub fn on(&mut self, x: usize, y: usize) {
        self.frame_buffer.set(x, y);
    }

    pub fn off(&mut self, x: usize, y: usize) {
        self.frame_buffer.unset(x, y);
    }

    pub fn apply(&mut self, frame: Frame) {
        self.frame_buffer = frame;
    }

    pub fn render(&mut self) {
        for row in self.pin_rows.iter_mut() {
            row.set_low().ok();
        }

        let mut cid = 0;
        for col in self.pin_cols.iter_mut() {
            if self.frame_buffer.is_set(self.row_p, cid) {
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

impl<P, ROWS, COLS, T> Bind<Timer<T>> for LEDMatrix<P, ROWS, COLS, T>
where
    P: OutputPin,
    ROWS: ArrayLength<P>,
    COLS: ArrayLength<P>,
    T: HalTimer,
{
    fn on_bind(&mut self, address: Address<Timer<T>>) {
        self.timer.replace(address);
    }
}

impl<P, ROWS, COLS, T> Actor for LEDMatrix<P, ROWS, COLS, T>
where
    P: OutputPin,
    ROWS: ArrayLength<P>,
    COLS: ArrayLength<P>,
    T: HalTimer,
{
    fn on_mount(&mut self, address: Address<Self>) {
        self.address.replace(address);
    }

    fn on_start(&'static mut self) -> Completion<Self> {
        if let Some(address) = &self.address {
            self.timer.as_ref().unwrap().schedule(
                self.refresh_rate.to_duration::<Milliseconds>().unwrap(),
                MatrixCommand::Render,
                address.clone(),
            );
        }
        Completion::immediate(self)
    }
}

impl<P, ROWS, COLS, T> NotifyHandler<MatrixCommand> for LEDMatrix<P, ROWS, COLS, T>
where
    P: OutputPin,
    ROWS: ArrayLength<P>,
    COLS: ArrayLength<P>,
    T: HalTimer,
{
    fn on_notify(&'static mut self, command: MatrixCommand) -> Completion<Self> {
        match command {
            MatrixCommand::On(x, y) => {
                self.on(x, y);
            }
            MatrixCommand::Off(x, y) => {
                self.off(x, y);
            }
            MatrixCommand::ApplyAscii(x) => {
                self.apply(x.to_frame());
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
        Completion::immediate(self)
    }
}

#[derive(Debug, PartialEq, Copy, Clone, Eq)]
pub enum MatrixCommand {
    On(usize, usize),
    Off(usize, usize),
    ApplyAscii(char),
    Render,
}

pub trait ToFrame {
    fn to_frame(&self) -> Frame;
}

// These are for 5x5 only
impl ToFrame for char {
    fn to_frame(&self) -> Frame {
        match self {
            'd' | 'D' => Frame::new([
                0x000F, 0x0011, 0x0011, 0x0011, 0x00F, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            ]),
            'r' | 'R' => Frame::new([
                0x001F, 0x0011, 0x001F, 0x0009, 0x0011, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            ]),
            'o' | 'O' => Frame::new([
                0x001F, 0x0011, 0x0011, 0x0011, 0x001F, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            ]),
            'u' | 'U' => Frame::new([
                0x0011, 0x0011, 0x0011, 0x0011, 0x001F, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            ]),
            'g' | 'G' => Frame::new([
                0x001F, 0x0001, 0x001D, 0x0011, 0x001F, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            ]),
            'e' | 'E' => Frame::new([
                0x001F, 0x0001, 0x000F, 0x0001, 0x001F, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            ]),
            _ => Frame::new([0; 32]),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_frame() {
        let frame = 'D'.to_frame();

        assert!(frame.is_set(0, 0));
        assert!(frame.is_set(0, 1));
        assert!(frame.is_set(0, 2));
        assert!(frame.is_set(0, 3));
        assert!(!frame.is_set(0, 4));

        assert!(frame.is_set(1, 0));
        assert!(!frame.is_set(1, 1));
        assert!(!frame.is_set(1, 2));
        assert!(!frame.is_set(1, 3));
        assert!(frame.is_set(1, 4));

        assert!(frame.is_set(2, 0));
        assert!(!frame.is_set(2, 1));
        assert!(!frame.is_set(2, 2));
        assert!(!frame.is_set(2, 3));
        assert!(frame.is_set(2, 4));

        assert!(frame.is_set(3, 0));
        assert!(!frame.is_set(3, 1));
        assert!(!frame.is_set(3, 2));
        assert!(!frame.is_set(3, 3));
        assert!(frame.is_set(3, 4));

        assert!(frame.is_set(4, 0));
        assert!(frame.is_set(4, 1));
        assert!(frame.is_set(4, 2));
        assert!(frame.is_set(4, 3));
        assert!(!frame.is_set(4, 4));
    }
}

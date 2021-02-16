use crate::domain::time::duration::Milliseconds;
use crate::domain::time::rate::{Hertz, Rate};

use crate::hal::scheduler::Scheduler;
use crate::prelude::*;
use embedded_hal::digital::v2::OutputPin;
use heapless::{ArrayLength, Vec};

// Led matrix driver supporting up to 32x32 led matrices.
pub struct LEDMatrix<P, ROWS, COLS, S>
where
    P: OutputPin + 'static,
    ROWS: ArrayLength<P> + 'static,
    COLS: ArrayLength<P> + 'static,
    S: Scheduler + 'static,
{
    address: Option<Address<Self>>,
    pin_rows: Vec<P, ROWS>,
    pin_cols: Vec<P, COLS>,
    frame_buffer: Frame,
    row_p: usize,
    timer: Option<Address<S>>,
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

impl<P, ROWS, COLS, S> LEDMatrix<P, ROWS, COLS, S>
where
    P: OutputPin,
    ROWS: ArrayLength<P>,
    COLS: ArrayLength<P>,
    S: Scheduler,
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

        for (cid, col) in self.pin_cols.iter_mut().enumerate() {
            if self.frame_buffer.is_set(self.row_p, cid) {
                col.set_low().ok();
            } else {
                col.set_high().ok();
            }
        }
        self.pin_rows[self.row_p].set_high().ok();
        self.row_p = (self.row_p + 1) % self.pin_rows.len();
    }
}

impl<P, ROWS, COLS, S> Actor for LEDMatrix<P, ROWS, COLS, S>
where
    P: OutputPin,
    ROWS: ArrayLength<P>,
    COLS: ArrayLength<P>,
    S: Scheduler,
{
    type Configuration = Address<S>;

    fn on_mount(&mut self, address: Address<Self>, config: Self::Configuration) {
        self.address.replace(address);
        self.timer.replace(config);
    }

    fn on_start(self) -> Completion<Self> {
        if let Some(address) = self.address {
            if let Some(timer) = self.timer {
                timer.schedule(
                    self.refresh_rate.to_duration::<Milliseconds>().unwrap(),
                    Render,
                    address,
                );
            }
        }
        Completion::immediate(self)
    }
}

impl<P, ROWS, COLS, S, F> NotifyHandler<Apply<F>> for LEDMatrix<P, ROWS, COLS, S>
where
    P: OutputPin,
    ROWS: ArrayLength<P>,
    COLS: ArrayLength<P>,
    S: Scheduler,
    F: ToFrame,
{
    fn on_notify(mut self, message: Apply<F>) -> Completion<Self> {
        self.apply(message.0.to_frame());
        Completion::immediate(self)
    }
}

impl<P, ROWS, COLS, S> NotifyHandler<On> for LEDMatrix<P, ROWS, COLS, S>
where
    P: OutputPin,
    ROWS: ArrayLength<P>,
    COLS: ArrayLength<P>,
    S: Scheduler,
{
    fn on_notify(mut self, message: On) -> Completion<Self> {
        self.on(message.0, message.1);
        Completion::immediate(self)
    }
}

impl<P, ROWS, COLS, S> NotifyHandler<Off> for LEDMatrix<P, ROWS, COLS, S>
where
    P: OutputPin,
    ROWS: ArrayLength<P>,
    COLS: ArrayLength<P>,
    S: Scheduler,
{
    fn on_notify(mut self, message: Off) -> Completion<Self> {
        self.off(message.0, message.1);
        Completion::immediate(self)
    }
}

impl<P, ROWS, COLS, S> NotifyHandler<Clear> for LEDMatrix<P, ROWS, COLS, S>
where
    P: OutputPin,
    ROWS: ArrayLength<P>,
    COLS: ArrayLength<P>,
    S: Scheduler,
{
    fn on_notify(mut self, message: Clear) -> Completion<Self> {
        self.clear();
        Completion::immediate(self)
    }
}

impl<P, ROWS, COLS, S> NotifyHandler<Render> for LEDMatrix<P, ROWS, COLS, S>
where
    P: OutputPin,
    ROWS: ArrayLength<P>,
    COLS: ArrayLength<P>,
    S: Scheduler,
{
    fn on_notify(mut self, message: Render) -> Completion<Self> {
        self.render();
        if let Some(address) = self.address {
            self.timer.unwrap().schedule(
                self.refresh_rate.to_duration::<Milliseconds>().unwrap(),
                Render,
                address,
            );
        }
        Completion::immediate(self)
    }
}

#[derive(Debug)]
pub struct On(pub usize, pub usize);
#[derive(Debug)]
pub struct Off(pub usize, pub usize);
#[derive(Debug)]
pub struct Clear;
#[derive(Debug, Clone)]
pub struct Render;
#[derive(Debug)]
pub struct Apply<F>(pub F)
where
    F: ToFrame;

pub trait ToFrame: Copy + Clone + core::fmt::Debug {
    fn to_frame(&self) -> Frame;
}

#[cfg(feature = "fonts")]
pub mod fonts {
    use super::*;

    fn frame_5x5(input: &[u8; 5]) -> Frame {
        // Mirror
        let mut bitmap: [u32; 32] = [0; 32];
        for (i, bm) in input.iter().enumerate() {
            let bm = *bm as u32;
            bitmap[i] = ((bm & 0x01) << 4)
                | ((bm & 0x02) << 2)
                | (bm & 0x04)
                | ((bm & 0x08) >> 2)
                | ((bm & 0x10) >> 4);
        }
        //for i in 5..bitmap.len() {
        for item in bitmap.iter_mut().skip(5) {
            //bitmap[i] = 0;
            *item = 0;
        }
        Frame::new(bitmap)
    }

    // These are for 5x5 only
    impl ToFrame for char {
        #[rustfmt::skip]
        fn to_frame(&self) -> Frame {
        match self {
            'a' | 'A' => frame_5x5(&[
                0b11111,
                0b10001,
                0b11111,
                0b10001,
                0b10001,
            ]),
            'b' | 'B' => frame_5x5(&[
                0b11110,
                0b10001,
                0b11111,
                0b10001,
                0b11110,
            ]),
            'c' | 'C' => frame_5x5(&[
                0b11111,
                0b10000,
                0b10000,
                0b10000,
                0b11111,
            ]),
            'd' | 'D' => frame_5x5(&[
                0b11110,
                0b10001,
                0b10001,
                0b10001,
                0b11110,
            ]),
            'e' | 'E' => frame_5x5(&[
                0b11111,
                0b10000,
                0b11110,
                0b10000,
                0b11111,
            ]),
            'f' | 'F' => frame_5x5(&[
                0b11111,
                0b10000,
                0b11110,
                0b10000,
                0b10000,
            ]),
            'g' | 'G' => frame_5x5(&[
                0b11111,
                0b10000,
                0b10111,
                0b10001,
                0b11111,
            ]),
            'h' | 'H' => frame_5x5(&[
                0b10001,
                0b10001,
                0b11111,
                0b10001,
                0b10001,
            ]),
            'i' | 'I' => frame_5x5(&[
                0b100100,
                0b100100,
                0b100100,
                0b100100,
                0b100100,
            ]),
            'j' | 'J' => frame_5x5(&[
                0b11111,
                0b00010,
                0b00010,
                0b10010,
                0b11110,
            ]),
            'k' | 'K' => frame_5x5(&[
                0b10010,
                0b10100,
                0b11000,
                0b10100,
                0b10010,
            ]),
            'l' | 'L' => frame_5x5(&[
                0b10000,
                0b10000,
                0b10000,
                0b10000,
                0b11111,
            ]),
            'm' | 'M' => frame_5x5(&[
                0b10001,
                0b11011,
                0b10101,
                0b10001,
                0b10001,
            ]),
            'n' | 'N' => frame_5x5(&[
                0b10001,
                0b11001,
                0b10101,
                0b10011,
                0b10001,
            ]),
            'o' | 'O' => frame_5x5(&[
                0b11111,
                0b10001,
                0b10001,
                0b10001,
                0b11111,
            ]),
            'p' | 'P' => frame_5x5(&[
                0b11111,
                0b10001,
                0b11111,
                0b10000,
                0b10000,
            ]),
            'q' | 'Q' => frame_5x5(&[
                0b11111,
                0b10001,
                0b10001,
                0b10011,
                0b11111,
            ]),
            'r' | 'R' => frame_5x5(&[
                0b11111,
                0b10001,
                0b11111,
                0b10010,
                0b10001,
            ]),
            's' | 'S' => frame_5x5(&[
                0b11111,
                0b10000,
                0b11111,
                0b00001,
                0b11111,
            ]),
            't' | 'T' => frame_5x5(&[
                0b11111,
                0b00100,
                0b00100,
                0b00100,
                0b00100,
            ]),
            'u' | 'U' => frame_5x5(&[
                0b10001,
                0b10001,
                0b10001,
                0b10001,
                0b11111,
            ]),
            'v' | 'V' => frame_5x5(&[
                0b10001,
                0b10001,
                0b01010,
                0b01010,
                0b00100,
            ]),
            'w' | 'W' => frame_5x5(&[
                0b10001,
                0b10001,
                0b10101,
                0b11011,
                0b10001,
            ]),
            'x' | 'X' => frame_5x5(&[
                0b10001,
                0b01010,
                0b00100,
                0b01010,
                0b10001,
            ]),
            'y' | 'Y' => frame_5x5(&[
                0b10001,
                0b01010,
                0b00100,
                0b00100,
                0b00100,
            ]),
            'z' | 'Z' => frame_5x5(&[
                0b11111,
                0b00010,
                0b00100,
                0b01000,
                0b11111,
            ]),
            '!' => frame_5x5(&[
                0b00100,
                0b00100,
                0b00100,
                0b00000,
                0b00100,
            ]),
            '?' => frame_5x5(&[
                0b11111,
                0b00001,
                0b00111,
                0b00000,
                0b00100,
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
}

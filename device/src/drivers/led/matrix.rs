use crate::traits::led::ToFrame;
use crate::{domain::led::matrix::*, traits::led::TextDisplay};
use core::convert::Infallible;
use core::future::Future;
use embassy_time::{block_for, Duration, Instant, Timer};
use embedded_hal::digital::v2::OutputPin;

const REFRESH_INTERVAL: Duration = Duration::from_micros(500);

// Led matrix driver supporting up to 32x32 led matrices.
pub struct LedMatrix<P, const ROWS: usize, const COLS: usize>
where
    P: OutputPin + 'static,
{
    pin_rows: [P; ROWS],
    pin_cols: [P; COLS],
    frame_buffer: Frame<COLS, ROWS>,
    row_p: usize,
    brightness: Brightness,
}

impl<P, const ROWS: usize, const COLS: usize> LedMatrix<P, ROWS, COLS>
where
    P: OutputPin,
{
    pub fn new(pin_rows: [P; ROWS], pin_cols: [P; COLS]) -> Self {
        LedMatrix {
            pin_rows,
            pin_cols,
            frame_buffer: Frame::empty(),
            row_p: 0,
            brightness: Default::default(),
        }
    }

    pub fn clear(&mut self) {
        self.frame_buffer.clear();
        for row in self.pin_rows.iter_mut() {
            row.set_high().ok();
        }

        for col in self.pin_cols.iter_mut() {
            col.set_high().ok();
        }
    }

    pub fn on(&mut self, x: usize, y: usize) {
        self.frame_buffer.set(x, y);
    }

    pub fn off(&mut self, x: usize, y: usize) {
        self.frame_buffer.unset(x, y);
    }

    pub fn apply(&mut self, frame: Frame<COLS, ROWS>) {
        self.frame_buffer = frame;
    }

    pub fn set_row_high(&mut self, row: usize) {
        self.pin_rows[row].set_high().ok().unwrap();
    }

    pub fn set_row_low(&mut self, row: usize) {
        self.pin_rows[row].set_low().ok().unwrap();
    }

    pub fn set_col_high(&mut self, col: usize) {
        self.pin_cols[col].set_high().ok().unwrap();
    }

    pub fn set_col_low(&mut self, col: usize) {
        self.pin_cols[col].set_low().ok().unwrap();
    }

    pub fn set_brightness(&mut self, brightness: Brightness) {
        self.brightness = brightness;
    }

    pub fn increase_brightness(&mut self) {
        self.brightness += 1;
    }

    pub fn decrease_brightness(&mut self) {
        self.brightness -= 1;
    }

    pub fn render(&mut self) {
        for row in self.pin_rows.iter_mut() {
            row.set_low().ok();
        }

        for (cid, col) in self.pin_cols.iter_mut().enumerate() {
            if self.frame_buffer.is_set(cid, self.row_p) {
                col.set_low().ok();
            } else {
                col.set_high().ok();
            }
        }

        // Adjust interval will impact brightness of the LEDs
        block_for(Duration::from_micros(
            ((Brightness::MAX.level() - self.brightness.level()) as u64) * 6000
                / Brightness::MAX.level() as u64,
        ));

        self.pin_rows[self.row_p].set_high().ok();

        self.row_p = (self.row_p + 1) % self.pin_rows.len();
    }

    pub async fn display(&mut self, frame: Frame<COLS, ROWS>, length: Duration) {
        self.apply(frame);
        let end = Instant::now() + length;
        while Instant::now() < end {
            self.render();
            Timer::after(REFRESH_INTERVAL).await;
        }
        self.clear();
    }

    pub async fn scroll(&mut self, text: &str) {
        self.scroll_with_speed(text, Duration::from_secs((text.len() / 2) as u64))
            .await;
    }

    pub async fn scroll_with_speed(&mut self, text: &str, speed: Duration) {
        self.animate(text.as_bytes(), AnimationEffect::Slide, speed)
            .await;
    }

    pub async fn animate(&mut self, data: &[u8], effect: AnimationEffect, duration: Duration) {
        let mut animation: Animation<'_, COLS, ROWS> =
            Animation::new(AnimationData::Bytes(data), effect, duration).unwrap();
        loop {
            match animation.next(Instant::now()) {
                AnimationState::Apply(f) => {
                    self.apply(f);
                }
                AnimationState::Wait => {}
                AnimationState::Done => {
                    break;
                }
            }
            self.render();
            Timer::after(REFRESH_INTERVAL).await;
        }
        self.clear();
    }

    pub async fn animate_frames(
        &mut self,
        data: &[&dyn ToFrame<COLS, ROWS>],
        effect: AnimationEffect,
        duration: Duration,
    ) {
        let mut animation: Animation<'_, COLS, ROWS> =
            Animation::new(AnimationData::Frames(data), effect, duration).unwrap();
        loop {
            match animation.next(Instant::now()) {
                AnimationState::Apply(f) => {
                    self.apply(f);
                }
                AnimationState::Wait => {}
                AnimationState::Done => {
                    break;
                }
            }
            self.render();
            Timer::after(REFRESH_INTERVAL).await;
        }
        self.clear();
    }
}

impl<P, const ROWS: usize, const COLS: usize> TextDisplay for LedMatrix<P, ROWS, COLS>
where
    P: OutputPin + 'static,
{
    type Error = Infallible;
    type ScrollFuture<'m> = impl Future<Output = Result<(), Self::Error>> + 'm where Self: 'm;

    fn scroll<'m>(&'m mut self, text: &'m str) -> Self::ScrollFuture<'m> {
        async move {
            self.scroll(text).await;
            Ok(())
        }
    }

    type ScrollWithSpeedFuture<'m> = impl Future<Output = Result<(), Self::Error>> + 'm where Self: 'm;

    fn scroll_with_speed<'m>(
        &'m mut self,
        text: &'m str,
        speed: Duration,
    ) -> Self::ScrollWithSpeedFuture<'m> {
        async move {
            self.scroll_with_speed(text, speed).await;
            Ok(())
        }
    }

    type DisplayFuture<'m> = impl Future<Output = Result<(), Self::Error>> + 'm where Self: 'm;
    fn display<'m>(&'m mut self, c: char, speed: Duration) -> Self::DisplayFuture<'m> {
        async move {
            self.display(c.to_frame(), speed).await;
            Ok(())
        }
    }
}

impl<const XSIZE: usize, const YSIZE: usize> ToFrame<XSIZE, YSIZE> for Frame<XSIZE, YSIZE> {
    fn to_frame(&self) -> Frame<XSIZE, YSIZE> {
        *self
    }
}

#[derive(Clone, Copy)]
pub enum AnimationEffect {
    None,
    Slide,
}

pub enum AnimationData<'a, const XSIZE: usize, const YSIZE: usize> {
    Frames(&'a [&'a dyn ToFrame<XSIZE, YSIZE>]),
    Bytes(&'a [u8]),
}

impl<'a, const XSIZE: usize, const YSIZE: usize> AnimationData<'a, XSIZE, YSIZE> {
    fn len(&self) -> usize {
        match self {
            AnimationData::Frames(f) => f.len(),
            AnimationData::Bytes(f) => f.len(),
        }
    }

    fn frame(&self, idx: usize) -> Frame<XSIZE, YSIZE> {
        match self {
            AnimationData::Frames(f) => f[idx].to_frame(),
            AnimationData::Bytes(f) => f[idx].to_frame(),
        }
    }
}

pub struct Animation<'a, const XSIZE: usize, const YSIZE: usize> {
    frames: AnimationData<'a, XSIZE, YSIZE>,
    sequence: usize,
    frame_index: usize,
    index: usize,
    length: usize,
    effect: AnimationEffect,
    wait: Duration,
    next: Instant,
}

#[derive(PartialEq, Debug)]
pub enum AnimationState<const XSIZE: usize, const YSIZE: usize> {
    Wait,
    Apply(Frame<XSIZE, YSIZE>),
    Done,
}

impl<'a, const XSIZE: usize, const YSIZE: usize> Animation<'a, XSIZE, YSIZE> {
    pub fn new(
        frames: AnimationData<'a, XSIZE, YSIZE>,
        effect: AnimationEffect,
        duration: Duration,
    ) -> Result<Self, AnimationError> {
        assert!(frames.len() > 0);
        let length = match effect {
            AnimationEffect::Slide => frames.len() * XSIZE,
            AnimationEffect::None => frames.len(),
        };

        if let Some(wait) = duration.checked_div(length as u32) {
            Ok(Self {
                frames,
                frame_index: 0,
                sequence: 0,
                index: 0,
                length,
                effect,
                wait,
                next: Instant::now(),
            })
        } else {
            Err(AnimationError::TooFast)
        }
    }
    fn current(&self) -> Frame<XSIZE, YSIZE> {
        let mut current = self.frames.frame(self.frame_index);

        let mut next = if self.frame_index < self.frames.len() - 1 {
            self.frames.frame(self.frame_index + 1)
        } else {
            Frame::empty()
        };

        current.shift_left(self.sequence);
        next.shift_right(XSIZE - self.sequence);

        current.or(&next);
        current
    }

    fn next(&mut self, now: Instant) -> AnimationState<XSIZE, YSIZE> {
        if self.next <= now {
            if self.index < self.length {
                let current = self.current();
                if self.sequence >= XSIZE - 1 {
                    self.sequence = match self.effect {
                        AnimationEffect::None => XSIZE,
                        AnimationEffect::Slide => 0,
                    };
                    self.frame_index += 1;
                } else {
                    self.sequence += 1;
                }

                self.index += 1;
                self.next += self.wait;
                AnimationState::Apply(current)
            } else {
                AnimationState::Done
            }
        } else {
            AnimationState::Wait
        }
    }
}

#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum AnimationError {
    BufferTooSmall,
    TooFast,
}

pub mod fonts {
    use super::*;

    impl<const XSIZE: usize, const YSIZE: usize> ToFrame<XSIZE, YSIZE> for &[u8; 5] {
        fn to_frame(&self) -> Frame<XSIZE, YSIZE> {
            frame_5x5(self)
        }
    }

    mod bitmaps {
        #[rustfmt::skip]
        pub const CHECK_MARK: &[u8; 5] = &[
            0b00000,
            0b00001,
            0b00010,
            0b10100,
            0b01000,
        ];

        #[rustfmt::skip]
        pub const CROSS_MARK: &[u8; 5] = &[
            0b00000,
            0b01010,
            0b00100,
            0b01010,
            0b00000,
        ];
    }

    pub use bitmaps::*;

    pub const fn frame_5x5<const XSIZE: usize, const YSIZE: usize>(
        input: &[u8; 5],
    ) -> Frame<XSIZE, YSIZE> {
        //assert!(XSIZE == 5);
        //assert!(YSIZE == 5);
        let mut data = [Bitmap::empty(5); YSIZE];
        data[0] = Bitmap::new(input[0], 5);
        data[1] = Bitmap::new(input[1], 5);
        data[2] = Bitmap::new(input[2], 5);
        data[3] = Bitmap::new(input[3], 5);
        data[4] = Bitmap::new(input[4], 5);
        Frame::new(data)
    }

    // These are for 5x5 only
    impl<const XSIZE: usize, const YSIZE: usize> ToFrame<XSIZE, YSIZE> for u8 {
        fn to_frame(&self) -> Frame<XSIZE, YSIZE> {
            (*self as char).to_frame()
        }
    }

    // These are for 5x5 only
    impl<const XSIZE: usize, const YSIZE: usize> ToFrame<XSIZE, YSIZE> for char {
        #[rustfmt::skip]
        fn to_frame(&self) -> Frame<XSIZE, YSIZE> {
            assert!(XSIZE == 5);
            assert!(YSIZE == 5);

            let n = *self as usize;
            if n > pendolino::PRINTABLE_START && n < pendolino::PRINTABLE_START + pendolino::PRINTABLE_COUNT {
                frame_5x5(&pendolino::PENDOLINO3[n - pendolino::PRINTABLE_START])
            } else {
                frame_5x5(&[
                    0,
                    0,
                    0,
                    0,
                    0,
                ])
            }
        }
    }

    pub mod pendolino {
        /*
        The license and copyright applies to the pendolino module.

        The MIT License (MIT)

        Copyright (c) 2016 British Broadcasting Corporation.
        This software is provided by Lancaster University by arrangement with the BBC.

        Permission is hereby granted, free of charge, to any person obtaining a
        copy of this software and associated documentation files (the "Software"),
        to deal in the Software without restriction, including without limitation
        the rights to use, copy, modify, merge, publish, distribute, sublicense,
        and/or sell copies of the Software, and to permit persons to whom the
        Software is furnished to do so, subject to the following conditions:

        The above copyright notice and this permission notice shall be included in
        all copies or substantial portions of the Software.

        THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
        IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
        FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL
        THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
        LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
        FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
        DEALINGS IN THE SOFTWARE.
        */
        /// Index of the first character in the standard font
        pub const PRINTABLE_START: usize = 32;

        /// Number of characters in the standard font
        pub const PRINTABLE_COUNT: usize = 95;

        // From lancaster-university/microbit-dal source/core/MicroBitFont.cpp
        // as of v2.1.1
        pub(super) const PENDOLINO3: [[u8; 5]; PRINTABLE_COUNT] = [
            [0x0, 0x0, 0x0, 0x0, 0x0],
            [0x8, 0x8, 0x8, 0x0, 0x8],
            [0xa, 0x4a, 0x40, 0x0, 0x0],
            [0xa, 0x5f, 0xea, 0x5f, 0xea],
            [0xe, 0xd9, 0x2e, 0xd3, 0x6e],
            [0x19, 0x32, 0x44, 0x89, 0x33],
            [0xc, 0x92, 0x4c, 0x92, 0x4d],
            [0x8, 0x8, 0x0, 0x0, 0x0],
            [0x4, 0x88, 0x8, 0x8, 0x4],
            [0x8, 0x4, 0x84, 0x84, 0x88],
            [0x0, 0xa, 0x44, 0x8a, 0x40],
            [0x0, 0x4, 0x8e, 0xc4, 0x80],
            [0x0, 0x0, 0x0, 0x4, 0x88],
            [0x0, 0x0, 0xe, 0xc0, 0x0],
            [0x0, 0x0, 0x0, 0x8, 0x0],
            [0x1, 0x22, 0x44, 0x88, 0x10],
            [0xc, 0x92, 0x52, 0x52, 0x4c],
            [0x4, 0x8c, 0x84, 0x84, 0x8e],
            [0x1c, 0x82, 0x4c, 0x90, 0x1e],
            [0x1e, 0xc2, 0x44, 0x92, 0x4c],
            [0x6, 0xca, 0x52, 0x5f, 0xe2],
            [0x1f, 0xf0, 0x1e, 0xc1, 0x3e],
            [0x2, 0x44, 0x8e, 0xd1, 0x2e],
            [0x1f, 0xe2, 0x44, 0x88, 0x10],
            [0xe, 0xd1, 0x2e, 0xd1, 0x2e],
            [0xe, 0xd1, 0x2e, 0xc4, 0x88],
            [0x0, 0x8, 0x0, 0x8, 0x0],
            [0x0, 0x4, 0x80, 0x4, 0x88],
            [0x2, 0x44, 0x88, 0x4, 0x82],
            [0x0, 0xe, 0xc0, 0xe, 0xc0],
            [0x8, 0x4, 0x82, 0x44, 0x88],
            [0xe, 0xd1, 0x26, 0xc0, 0x4],
            [0xe, 0xd1, 0x35, 0xb3, 0x6c],
            [0xc, 0x92, 0x5e, 0xd2, 0x52],
            [0x1c, 0x92, 0x5c, 0x92, 0x5c],
            [0xe, 0xd0, 0x10, 0x10, 0xe],
            [0x1c, 0x92, 0x52, 0x52, 0x5c],
            [0x1e, 0xd0, 0x1c, 0x90, 0x1e],
            [0x1e, 0xd0, 0x1c, 0x90, 0x10],
            [0xe, 0xd0, 0x13, 0x71, 0x2e],
            [0x12, 0x52, 0x5e, 0xd2, 0x52],
            [0x1c, 0x88, 0x8, 0x8, 0x1c],
            [0x1f, 0xe2, 0x42, 0x52, 0x4c],
            [0x12, 0x54, 0x98, 0x14, 0x92],
            [0x10, 0x10, 0x10, 0x10, 0x1e],
            [0x11, 0x3b, 0x75, 0xb1, 0x31],
            [0x11, 0x39, 0x35, 0xb3, 0x71],
            [0xc, 0x92, 0x52, 0x52, 0x4c],
            [0x1c, 0x92, 0x5c, 0x90, 0x10],
            [0xc, 0x92, 0x52, 0x4c, 0x86],
            [0x1c, 0x92, 0x5c, 0x92, 0x51],
            [0xe, 0xd0, 0xc, 0x82, 0x5c],
            [0x1f, 0xe4, 0x84, 0x84, 0x84],
            [0x12, 0x52, 0x52, 0x52, 0x4c],
            [0x11, 0x31, 0x31, 0x2a, 0x44],
            [0x11, 0x31, 0x35, 0xbb, 0x71],
            [0x12, 0x52, 0x4c, 0x92, 0x52],
            [0x11, 0x2a, 0x44, 0x84, 0x84],
            [0x1e, 0xc4, 0x88, 0x10, 0x1e],
            [0xe, 0xc8, 0x8, 0x8, 0xe],
            [0x10, 0x8, 0x4, 0x82, 0x41],
            [0xe, 0xc2, 0x42, 0x42, 0x4e],
            [0x4, 0x8a, 0x40, 0x0, 0x0],
            [0x0, 0x0, 0x0, 0x0, 0x1f],
            [0x8, 0x4, 0x80, 0x0, 0x0],
            [0x0, 0xe, 0xd2, 0x52, 0x4f],
            [0x10, 0x10, 0x1c, 0x92, 0x5c],
            [0x0, 0xe, 0xd0, 0x10, 0xe],
            [0x2, 0x42, 0x4e, 0xd2, 0x4e],
            [0xc, 0x92, 0x5c, 0x90, 0xe],
            [0x6, 0xc8, 0x1c, 0x88, 0x8],
            [0xe, 0xd2, 0x4e, 0xc2, 0x4c],
            [0x10, 0x10, 0x1c, 0x92, 0x52],
            [0x8, 0x0, 0x8, 0x8, 0x8],
            [0x2, 0x40, 0x2, 0x42, 0x4c],
            [0x10, 0x14, 0x98, 0x14, 0x92],
            [0x8, 0x8, 0x8, 0x8, 0x6],
            [0x0, 0x1b, 0x75, 0xb1, 0x31],
            [0x0, 0x1c, 0x92, 0x52, 0x52],
            [0x0, 0xc, 0x92, 0x52, 0x4c],
            [0x0, 0x1c, 0x92, 0x5c, 0x90],
            [0x0, 0xe, 0xd2, 0x4e, 0xc2],
            [0x0, 0xe, 0xd0, 0x10, 0x10],
            [0x0, 0x6, 0xc8, 0x4, 0x98],
            [0x8, 0x8, 0xe, 0xc8, 0x7],
            [0x0, 0x12, 0x52, 0x52, 0x4f],
            [0x0, 0x11, 0x31, 0x2a, 0x44],
            [0x0, 0x11, 0x31, 0x35, 0xbb],
            [0x0, 0x12, 0x4c, 0x8c, 0x92],
            [0x0, 0x11, 0x2a, 0x44, 0x98],
            [0x0, 0x1e, 0xc4, 0x88, 0x1e],
            [0x6, 0xc4, 0x8c, 0x84, 0x86],
            [0x8, 0x8, 0x8, 0x8, 0x8],
            [0x18, 0x8, 0xc, 0x88, 0x18],
            [0x0, 0x0, 0xc, 0x83, 0x60],
        ];
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn test_to_frame() {
            let frame: Frame<5, 5> = 'D'.to_frame();

            assert!(frame.is_set(0, 0));
            assert!(frame.is_set(1, 0));
            assert!(frame.is_set(2, 0));
            assert!(!frame.is_set(3, 0));
            assert!(!frame.is_set(4, 0));

            assert!(frame.is_set(0, 1));
            assert!(!frame.is_set(1, 1));
            assert!(!frame.is_set(2, 1));
            assert!(frame.is_set(3, 1));
            assert!(!frame.is_set(4, 1));

            assert!(frame.is_set(0, 2));
            assert!(!frame.is_set(1, 2));
            assert!(!frame.is_set(2, 2));
            assert!(frame.is_set(3, 2));
            assert!(!frame.is_set(4, 2));

            assert!(frame.is_set(0, 3));
            assert!(!frame.is_set(1, 3));
            assert!(!frame.is_set(2, 3));
            assert!(frame.is_set(3, 3));
            assert!(!frame.is_set(4, 3));

            assert!(frame.is_set(0, 4));
            assert!(frame.is_set(1, 4));
            assert!(frame.is_set(2, 4));
            assert!(!frame.is_set(3, 4));
            assert!(!frame.is_set(4, 4));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_animation() {
        let mut animation: Animation<5, 5> = Animation::new(
            AnimationData::Bytes(b"12"),
            AnimationEffect::Slide,
            Duration::from_secs(1),
        )
        .unwrap();

        let expected = animation.length;
        let mut n = 0;
        while n < expected {
            if let AnimationState::Apply(c) =
                animation.next(Instant::now() + Duration::from_secs(1))
            {
                println!("C ({}): \n{:#?}", n, c);
                n += 1;
            } else {
                break;
            }
        }
        assert!(animation.next(Instant::now() + Duration::from_secs(1)) == AnimationState::Done);
    }

    #[test]
    fn test_animation_length() {
        let animation: Animation<5, 5> = Animation::new(
            AnimationData::Bytes(b"12"),
            AnimationEffect::Slide,
            Duration::from_secs(1),
        )
        .unwrap();

        assert_eq!(animation.length, 10);

        let animation: Animation<5, 5> = Animation::new(
            AnimationData::Bytes(b"123"),
            AnimationEffect::Slide,
            Duration::from_secs(1),
        )
        .unwrap();

        assert_eq!(animation.length, 15);

        let animation: Animation<5, 5> = Animation::new(
            AnimationData::Bytes(b"1234"),
            AnimationEffect::Slide,
            Duration::from_secs(1),
        )
        .unwrap();

        assert_eq!(animation.length, 20);
    }
}

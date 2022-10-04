//! Driver a NxM LED matrix display
//!
//! * Can display 5x5 bitmaps from raw data or characters
//! * Methods for scrolling text across LED matrix or displaying a bitmap for a duration
use embassy_time::{block_for, Duration, Instant, Timer};
use embedded_hal::digital::OutputPin;

pub mod fonts;

mod types;
pub use types::*;

const REFRESH_INTERVAL: Duration = Duration::from_micros(500);

/// Led matrix driver supporting arbitrary sized led matrixes.
///
/// NOTE: Currently restricted by 8 bits width
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
    /// Create a new instance of an LED matrix using the provided pins
    pub fn new(pin_rows: [P; ROWS], pin_cols: [P; COLS]) -> Self {
        LedMatrix {
            pin_rows,
            pin_cols,
            frame_buffer: Frame::empty(),
            row_p: 0,
            brightness: Default::default(),
        }
    }

    /// Clear all LEDs
    pub fn clear(&mut self) {
        self.frame_buffer.clear();
        for row in self.pin_rows.iter_mut() {
            row.set_high().ok();
        }

        for col in self.pin_cols.iter_mut() {
            col.set_high().ok();
        }
    }

    /// Turn on point (x,y) in the frame buffer
    pub fn on(&mut self, x: usize, y: usize) {
        self.frame_buffer.set(x, y);
    }

    /// Turn off point (x,y) in the frame buffer
    pub fn off(&mut self, x: usize, y: usize) {
        self.frame_buffer.unset(x, y);
    }

    /// Apply the provided frame onto the frame buffer
    pub fn apply(&mut self, frame: Frame<COLS, ROWS>) {
        self.frame_buffer = frame;
    }

    /// Adjust the brightness level
    pub fn set_brightness(&mut self, brightness: Brightness) {
        self.brightness = brightness;
    }

    /// Increase brightness relative to current setting
    pub fn increase_brightness(&mut self) {
        self.brightness += 1;
    }

    /// Decrease brightness relative to current setting
    pub fn decrease_brightness(&mut self) {
        self.brightness -= 1;
    }

    /// Perform a full refresh of the display based on the current frame buffer
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

    /// Display the provided frame for the duration. Handles screen refresh
    /// in an async display loop.
    pub async fn display(&mut self, frame: Frame<COLS, ROWS>, length: Duration) {
        self.apply(frame);
        let end = Instant::now() + length;
        while Instant::now() < end {
            self.render();
            Timer::after(REFRESH_INTERVAL).await;
        }
        self.clear();
    }

    /// Scroll the provided text across the LED display using default duration based on text length
    pub async fn scroll(&mut self, text: &str) {
        self.scroll_with_speed(text, Duration::from_secs((text.len() / 2) as u64))
            .await;
    }

    /// Scroll the provided text across the screen within the provided duration
    pub async fn scroll_with_speed(&mut self, text: &str, speed: Duration) {
        self.animate(text.as_bytes(), AnimationEffect::Slide, speed)
            .await;
    }

    /// Apply animation based on data with the given effect during the provided duration
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

    /// Animate a slice of frames using the provided effect during the provided duration
    pub async fn animate_frames(
        &mut self,
        data: &[Frame<COLS, ROWS>],
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

/// An effect filter to apply for an animation
#[derive(Clone, Copy)]
pub enum AnimationEffect {
    /// No effect
    None,
    /// Sliding effect
    Slide,
}

enum AnimationData<'a, const XSIZE: usize, const YSIZE: usize> {
    Frames(&'a [Frame<XSIZE, YSIZE>]),
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
            AnimationData::Frames(f) => f[idx],
            AnimationData::Bytes(f) => f[idx].into(),
        }
    }
}

struct Animation<'a, const XSIZE: usize, const YSIZE: usize> {
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
enum AnimationState<const XSIZE: usize, const YSIZE: usize> {
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
/// Errors produced when running animations
pub enum AnimationError {
    /// Animation scroll is too fast to keep up with the refresh rate
    TooFast,
}

/*
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
*/

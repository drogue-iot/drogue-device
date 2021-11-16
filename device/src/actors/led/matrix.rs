use crate::drivers::led::matrix::*;
use crate::kernel::{actor::Actor, actor::Address, actor::Inbox};
use crate::traits::led::TextDisplay;
use core::future::Future;
use embassy::time::{with_timeout, Duration, Instant, TimeoutError, Timer};
use embedded_hal::digital::v2::OutputPin;

pub trait LedMatrixAddress<const ROWS: usize, const COLS: usize> {
    fn apply(&mut self, frame: &'static dyn ToFrame<COLS, ROWS>);
}

impl<P, const ROWS: usize, const COLS: usize> LedMatrixAddress<ROWS, COLS>
    for Address<'static, LedMatrixActor<P, ROWS, COLS>>
where
    P: OutputPin + 'static,
{
    fn apply(&mut self, frame: &'static dyn ToFrame<COLS, ROWS>) {
        self.notify(MatrixCommand::ApplyFrame(frame)).unwrap();
    }
}

impl<P, const ROWS: usize, const COLS: usize> TextDisplay
    for Address<'static, LedMatrixActor<P, ROWS, COLS>>
where
    P: OutputPin + 'static,
{
    type Error = ();
    type ScrollFuture<'m>
    where
        P: 'm,
    = impl Future<Output = Result<(), Self::Error>> + 'm;

    fn scroll<'m>(&'m mut self, text: &'m str) -> Self::ScrollFuture<'m> {
        async move {
            self.request(MatrixCommand::ApplyText(
                text,
                AnimationEffect::Slide,
                Duration::from_secs((text.len() / 2) as u64),
            ))
            .unwrap()
            .await;
            Ok(())
        }
    }

    fn putc(&mut self, c: char) -> Result<(), Self::Error> {
        let _ = self.notify(MatrixCommand::ApplyAsciiChar(c));
        Ok(())
    }
}

pub struct LedMatrixActor<P, const ROWS: usize, const COLS: usize>
where
    P: OutputPin + 'static,
{
    refresh_interval: Duration,
    matrix: LedMatrix<P, ROWS, COLS>,
}

impl<P, const ROWS: usize, const COLS: usize> LedMatrixActor<P, ROWS, COLS>
where
    P: OutputPin + 'static,
{
    pub fn new(
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
    type Message<'m> = MatrixCommand<'m, COLS, ROWS>;

    type OnMountFuture<'m, M>
    where
        P: 'm,
        M: 'm,
    = impl Future<Output = ()> + 'm;

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
            match with_timeout(self.refresh_interval, inbox.next()).await {
                Ok(Some(mut m)) => match *m.message() {
                    MatrixCommand::ApplyAsciiChar(c) => self.matrix.apply(c.to_frame()),
                    MatrixCommand::ApplyFrame(f) => self.matrix.apply(f.to_frame()),
                    MatrixCommand::ApplyText(s, effect, duration) => {
                        let mut animation: Animation<'m, COLS, ROWS> =
                            Animation::new(AnimationData::Bytes(s.as_bytes()), effect, duration)
                                .unwrap();
                        loop {
                            match animation.next(Instant::now()) {
                                AnimationState::Apply(f) => {
                                    self.matrix.apply(f);
                                }
                                AnimationState::Wait => {}
                                AnimationState::Done => {
                                    break;
                                }
                            }
                            self.matrix.render();
                            Timer::after(self.refresh_interval).await;
                        }
                    }
                    MatrixCommand::ApplyAnimation(a, effect, duration) => {
                        let mut animation =
                            Animation::new(AnimationData::Frames(a), effect, duration).unwrap();

                        loop {
                            match animation.next(Instant::now()) {
                                AnimationState::Apply(f) => {
                                    self.matrix.apply(f);
                                }
                                AnimationState::Wait => {}
                                AnimationState::Done => {
                                    break;
                                }
                            }
                            self.matrix.render();
                            Timer::after(self.refresh_interval).await;
                        }
                    }
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

#[derive(Clone, Copy)]
pub enum MatrixCommand<'m, const XSIZE: usize, const YSIZE: usize> {
    On(usize, usize),
    Off(usize, usize),
    Clear,
    Render,
    ApplyAsciiChar(char),
    ApplyFrame(&'m dyn ToFrame<XSIZE, YSIZE>),
    ApplyText(&'m str, AnimationEffect, Duration),
    ApplyAnimation(
        &'m [&'m dyn ToFrame<XSIZE, YSIZE>],
        AnimationEffect,
        Duration,
    ),
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

use crate::drivers::led::matrix::*;
use crate::kernel::{actor::Actor, actor::Address, actor::Inbox};
use core::future::Future;
use embassy::time::{with_timeout, Duration, Instant, TimeoutError};
use embedded_hal::digital::v2::OutputPin;

pub trait LedMatrixAddress<const ROWS: usize, const COLS: usize> {
    fn apply(&mut self, frame: &'static dyn ToFrame<COLS, ROWS>);
}

impl<P, const ROWS: usize, const COLS: usize, const ANIMATION_BUFFER_SIZE: usize>
    LedMatrixAddress<ROWS, COLS>
    for Address<'static, LedMatrixActor<P, ROWS, COLS, ANIMATION_BUFFER_SIZE>>
where
    P: OutputPin + 'static,
{
    fn apply(&mut self, frame: &'static dyn ToFrame<COLS, ROWS>) {
        self.notify(MatrixCommand::ApplyFrame(frame)).unwrap();
    }
}

pub struct LedMatrixActor<
    P,
    const ROWS: usize,
    const COLS: usize,
    const ANIMATION_BUFFER_SIZE: usize,
> where
    P: OutputPin + 'static,
{
    refresh_interval: Duration,
    animation: Option<Animation<COLS, ROWS, ANIMATION_BUFFER_SIZE>>,
    matrix: LedMatrix<P, ROWS, COLS>,
}

impl<P, const ROWS: usize, const COLS: usize, const ANIMATION_BUFFER_SIZE: usize>
    LedMatrixActor<P, ROWS, COLS, ANIMATION_BUFFER_SIZE>
where
    P: OutputPin + 'static,
{
    pub fn new(
        refresh_interval: Duration,
        matrix: LedMatrix<P, ROWS, COLS>,
    ) -> LedMatrixActor<P, ROWS, COLS, ANIMATION_BUFFER_SIZE> {
        Self {
            animation: None,
            refresh_interval,
            matrix,
        }
    }
}

impl<P, const ROWS: usize, const COLS: usize, const ANIMATION_BUFFER_SIZE: usize> Actor
    for LedMatrixActor<P, ROWS, COLS, ANIMATION_BUFFER_SIZE>
where
    P: OutputPin,
{
    #[rustfmt::skip]
    type Message<'m> = MatrixCommand<'m, COLS, ROWS>;
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
                    Ok(mut m) => match *m.message() {
                        MatrixCommand::ApplyFrame(f) => self.matrix.apply(f.to_frame()),
                        MatrixCommand::ApplyText(s, effect, duration) => {
                            self.animation.replace(
                                Animation::from_bytes(s.as_bytes(), effect, duration).unwrap(),
                            );
                        }
                        MatrixCommand::ApplyAnimation(a, effect, duration) => {
                            self.animation
                                .replace(Animation::from_frames(a, effect, duration).unwrap());
                        }
                        MatrixCommand::On(x, y) => self.matrix.on(x, y),
                        MatrixCommand::Off(x, y) => self.matrix.off(x, y),
                        MatrixCommand::Clear => self.matrix.clear(),
                        MatrixCommand::Render => {
                            self.matrix.render();
                        }
                    },
                    Err(TimeoutError) => {
                        if let Some(a) = &mut self.animation {
                            match a.next(Instant::now()) {
                                AnimationState::Apply(frame) => {
                                    self.matrix.apply(*frame);
                                }
                                AnimationState::Waiting => {}
                                AnimationState::Done => {
                                    self.animation.take().unwrap();
                                    self.matrix.clear();
                                }
                            }
                        }
                        self.matrix.render();
                    }
                }
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

pub struct Animation<const XSIZE: usize, const YSIZE: usize, const N: usize> {
    frames: [Frame<XSIZE, YSIZE>; N],
    index: usize,
    length: usize,
    wait: Duration,
    next: Instant,
}

pub enum AnimationState<'a, const XSIZE: usize, const YSIZE: usize> {
    Waiting,
    Apply(&'a Frame<XSIZE, YSIZE>),
    Done,
}

impl<const XSIZE: usize, const YSIZE: usize, const N: usize> Animation<XSIZE, YSIZE, N> {
    fn next(&mut self, now: Instant) -> AnimationState<XSIZE, YSIZE> {
        if self.next <= now {
            if self.index < self.length {
                // TODO: Handle skipping?
                let current = &self.frames[self.index];
                self.next += self.wait;
                self.index += 1;
                AnimationState::Apply(current)
            } else {
                AnimationState::Done
            }
        } else {
            AnimationState::Waiting
        }
    }

    // TODO: Reuse from_frames implementation
    fn from_bytes(
        from: &[u8],
        effect: AnimationEffect,
        duration: Duration,
    ) -> Result<Self, AnimationError> {
        let (frames, length) = match effect {
            AnimationEffect::None => {
                if from.len() < N {
                    let mut frames: [Frame<XSIZE, YSIZE>; N] = [Frame::empty(); N];
                    let mut length = 0;
                    while length < from.len() {
                        frames[length] = from[length].to_frame();
                        length += 1;
                    }
                    Ok((frames, length))
                } else {
                    Err(AnimationError::BufferTooSmall)
                }
            }
            AnimationEffect::Slide => {
                if (from.len() * (XSIZE + 2)) < N {
                    let mut frames: [Frame<XSIZE, YSIZE>; N] = [Frame::empty(); N];
                    let mut length = 0;
                    let mut f = 0;
                    while f < from.len() && length < N {
                        let frame: Frame<XSIZE, YSIZE> = from[f].to_frame();
                        let next = if f < from.len() - 1 {
                            from[f + 1].to_frame()
                        } else {
                            Frame::empty()
                        };

                        // First frame is base frame;
                        frames[length] = frame;
                        length += 1;

                        // Add spacing before next frame
                        if length < N {
                            frames[length] = frame;
                            frames[length].shift_left(1);
                            length += 1;
                        }

                        // Add transition to next frame;
                        let mut d = 0;
                        while d < XSIZE && length < N {
                            frames[length] = frames[length - 1];
                            frames[length].shift_left(1);

                            // Or with next transition
                            let mut n = next;
                            n.shift_right(XSIZE - d);
                            frames[length].or(&n);

                            length += 1;
                            d += 1;
                        }

                        f += 1;
                    }
                    Ok((frames, length))
                } else {
                    Err(AnimationError::BufferTooSmall)
                }
            }
        }?;

        if let Some(wait) = duration.checked_div(length as u32) {
            Ok(Animation {
                frames,
                index: 0,
                length,
                wait,
                next: Instant::now(),
            })
        } else {
            Err(AnimationError::TooFast)
        }
    }

    fn from_frames(
        from: &[&dyn ToFrame<XSIZE, YSIZE>],
        effect: AnimationEffect,
        duration: Duration,
    ) -> Result<Self, AnimationError> {
        let (frames, length) = match effect {
            AnimationEffect::None => {
                if from.len() < N {
                    let mut frames: [Frame<XSIZE, YSIZE>; N] = [Frame::empty(); N];
                    let mut length = 0;
                    while length < from.len() {
                        frames[length] = from[length].to_frame();
                        length += 1;
                    }
                    Ok((frames, length))
                } else {
                    Err(AnimationError::BufferTooSmall)
                }
            }
            AnimationEffect::Slide => {
                if (from.len() * (XSIZE + 2)) < N {
                    let mut frames: [Frame<XSIZE, YSIZE>; N] = [Frame::empty(); N];
                    let mut length = 0;
                    let mut f = 0;
                    while f < from.len() && length < N {
                        let frame: Frame<XSIZE, YSIZE> = from[f].to_frame();
                        let next = if f < from.len() - 1 {
                            from[f + 1].to_frame()
                        } else {
                            Frame::empty()
                        };

                        // First frame is base frame;
                        frames[length] = frame;
                        length += 1;

                        // Add spacing before next frame
                        if length < N {
                            frames[length] = frame;
                            frames[length].shift_left(1);
                            length += 1;
                        }

                        // Add transition to next frame;
                        let mut d = 0;
                        while d < XSIZE && length < N {
                            frames[length] = frames[length - 1];
                            frames[length].shift_left(1);

                            // Or with next transition
                            let mut n = next;
                            n.shift_right(XSIZE - d);
                            frames[length].or(&n);

                            length += 1;
                            d += 1;
                        }

                        f += 1;
                    }
                    Ok((frames, length))
                } else {
                    Err(AnimationError::BufferTooSmall)
                }
            }
        }?;

        if let Some(wait) = duration.checked_div(length as u32) {
            Ok(Animation {
                frames,
                index: 0,
                length,
                wait,
                next: Instant::now(),
            })
        } else {
            Err(AnimationError::TooFast)
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
        let animation: Animation<5, 5, 15> =
            Animation::from_bytes(b"12", AnimationEffect::Slide, Duration::from_secs(1)).unwrap();

        assert_eq!(animation.length, 14);
    }
}

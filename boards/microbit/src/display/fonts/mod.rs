//! Bitmaps and fonts for the micro:bit

use super::types::*;
mod pendolino;

mod bitmaps {
    use super::*;

    #[rustfmt::skip]
    /// A check-mark bitmap
    pub const CHECK_MARK: Frame<5, 5> = frame_5x5(&[
        0b00000,
        0b00001,
        0b00010,
        0b10100,
        0b01000,
    ]);

    #[rustfmt::skip]
    /// A cross-mark bitmap
    pub const CROSS_MARK: Frame<5, 5> = frame_5x5(&[
        0b00000,
        0b01010,
        0b00100,
        0b01010,
        0b00000,
    ]);

    #[rustfmt::skip]
    /// A left arrow bitmap
    pub const ARROW_LEFT: Frame<5, 5> = frame_5x5(&[
        0b00100,
        0b01000,
        0b11111,
        0b01000,
        0b00100,
    ]);

    #[rustfmt::skip]
    /// A right arrow bitmap
    pub const ARROW_RIGHT: Frame<5, 5> = frame_5x5(&[
        0b00100,
        0b00010,
        0b11111,
        0b00010,
        0b00100,
    ]);

    /// Construct a 5x5 frame from a byte slice
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
}

pub use bitmaps::*;

impl<const XSIZE: usize, const YSIZE: usize> Into<Frame<XSIZE, YSIZE>> for u8 {
    fn into(self) -> Frame<XSIZE, YSIZE> {
        (self as char).into()
    }
}

impl<const XSIZE: usize, const YSIZE: usize> Into<Frame<XSIZE, YSIZE>> for char {
    fn into(self) -> Frame<XSIZE, YSIZE> {
        assert!(XSIZE == 5);
        assert!(YSIZE == 5);

        let n = self as usize;
        if n > pendolino::PRINTABLE_START
            && n < pendolino::PRINTABLE_START + pendolino::PRINTABLE_COUNT
        {
            frame_5x5(&pendolino::PENDOLINO3[n - pendolino::PRINTABLE_START])
        } else {
            frame_5x5(&[0, 0, 0, 0, 0])
        }
    }
}

/*
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
*/

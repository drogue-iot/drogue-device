use embedded_hal::digital::v2::OutputPin;

// Led matrix driver supporting up to 32x32 led matrices.
pub struct LedMatrix<P, const ROWS: usize, const COLS: usize>
where
    P: OutputPin + 'static,
{
    pin_rows: [P; ROWS],
    pin_cols: [P; COLS],
    frame_buffer: Frame,
    row_p: usize,
}

/**
 * A 32x32 bitmap that can be displayed on a LED matrix.
 */
#[derive(Clone, Copy, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct Frame {
    bitmap: [u32; 32],
}

impl core::fmt::Debug for Frame {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        for i in 0..self.bitmap.len() {
            for j in 0..32 {
                if self.is_set(i, j) {
                    write!(f, "1")?;
                } else {
                    write!(f, "0")?;
                }
            }
            writeln!(f)?;
        }
        Ok(())
    }
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

    /*
    fn transition<const COLS: usize>(&self, to: Frame) -> [Frame; COLS + 2]
    where
        [(); COLS + 2]: ,
    {
        let f: Frame = Default::default();
        let mut sequence = [f; { COLS + 2 }];
        for r in 0..5 {
            sequence[0].bitmap[r] = self.bitmap[r];
        }
        for r in 0..5 {
            println!("FROM {:#07b}", sequence[0].bitmap[r]);
            for c in 1..(COLS + 1) {
                println!("AT {}x{}", r, c);
                let from = sequence[c - 1].bitmap[r];
                let to = if c == 1 { 0 } else { to.bitmap[r] };

                //let d = (from & 0b11111) & ((to << to_pos) & 0b11111);
                println!(
                    "From bitmap {:#07b} shift left {:#07b}",
                    from,
                    (from << 1) & 0b11111
                );

                let to_pos = COLS - (c - 1);
                let to_bit = (to >> to_pos) & 0x1;
                let d = (((from << 1) & 0b11111) | to_bit) << (32 - 5);
                println!(
                    "To bitmap {:#07b} shift left {}, To bit: {}",
                    to, to_pos, to_bit
                );
                println!("Result: {:#07b}", d);

                // TODO: Dynamic mask based on COLS
                sequence[c].bitmap[r] = d;
            }
        }
        sequence[COLS + 1] = to;
        sequence
    }
    */
}

impl Default for Frame {
    fn default() -> Frame {
        Frame::new([0; 32])
    }
}

impl<P, const ROWS: usize, const COLS: usize> LedMatrix<P, ROWS, COLS>
where
    P: OutputPin,
{
    pub fn new(pin_rows: [P; ROWS], pin_cols: [P; COLS]) -> Self {
        LedMatrix {
            pin_rows,
            pin_cols,
            frame_buffer: Frame::new([0; 32]),
            row_p: 0,
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

#[cfg(feature = "defmt")]
pub trait ToFrame: core::fmt::Debug + defmt::Format + Sync {
    fn to_frame(&self) -> Frame;
}

#[cfg(not(feature = "defmt"))]
pub trait ToFrame: core::fmt::Debug + Sync {
    fn to_frame(&self) -> Frame;
}

#[cfg(feature = "fonts")]
pub mod fonts {
    use super::*;

    impl ToFrame for &[u8; 5] {
        fn to_frame(&self) -> Frame {
            frame_5x5(self)
        }
    }

    mod bitmaps {
        #[rustfmt::skip]
        pub const CHECK_MARK: &[u8; 5] = &[
            0b00000,
            0b00001,
            0b10010,
            0b10100,
            0b01000,
        ];
    }

    pub use bitmaps::*;

    pub fn frame_5x5(input: &[u8; 5]) -> Frame {
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
            '-' => frame_5x5(&[
                0b00000,
                0b00000,
                0b11111,
                0b00000,
                0b00000,
            ]),
            '0' => frame_5x5(&[
                0b11111,
                0b10001,
                0b10101,
                0b10001,
                0b11111,
            ]),
            '1' => frame_5x5(&[
                0b11100,
                0b00101,
                0b00101,
                0b00101,
                0b01110,
            ]),
            '2' => frame_5x5(&[
                0b11111,
                0b00001,
                0b11111,
                0b10000,
                0b11111,
            ]),
            '3' => frame_5x5(&[
                0b11111,
                0b00001,
                0b11111,
                0b00001,
                0b11111,
            ]),
            '4' => frame_5x5(&[
                0b10001,
                0b10001,
                0b11111,
                0b00001,
                0b00001,
            ]),
            '5' => frame_5x5(&[
                0b11111,
                0b10000,
                0b11110,
                0b00001,
                0b11110,
            ]),
            '6' => frame_5x5(&[
                0b01111,
                0b10000,
                0b10111,
                0b10001,
                0b01110,
            ]),
            '7' => frame_5x5(&[
                0b11111,
                0b00010,
                0b00100,
                0b01000,
                0b10000,
            ]),
            '8' => frame_5x5(&[
                0b01110,
                0b10001,
                0b01110,
                0b10001,
                0b01110,
            ]),
            '9' => frame_5x5(&[
                0b01111,
                0b10001,
                0b11101,
                0b00001,
                0b01110,
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

        /*
        #[test]
        fn test_transition() {
            let from = '7'.to_frame();
            let sequence = from.transition::<5>(from);

            assert_eq!(sequence[0], '7'.to_frame());

            #[rustfmt::skip]
            assert_eq!(sequence[1], frame_5x5(&[
                0b11110,
                0b00100,
                0b01000,
                0b10000,
                0b00000,
            ]));

            #[rustfmt::skip]
            assert_eq!(sequence[2], frame_5x5(&[
                0b11101,
                0b01000,
                0b10000,
                0b00000,
                0b00001,
            ]));

            #[rustfmt::skip]
            assert_eq!(sequence[3], frame_5x5(&[
                0b11111,
                0b10000,
                0b00001,
                0b00010,
                0b00100,
            ]));

            #[rustfmt::skip]
            assert_eq!(sequence[4], frame_5x5(&[
                0b11111,
                0b00001,
                0b00010,
                0b00100,
                0b01000,
            ]));

            #[rustfmt::skip]
            assert_eq!(sequence[5], frame_5x5(&[
                0b11111,
                0b00010,
                0b00100,
                0b01000,
                0b10000,
            ]));
        }
        */

        #[test]
        fn bitops() {
            let f = 0b10000;
            println!("F shift left: {:#b}", f << 1);
            println!("F shift right: {:#b}", f >> 1);
            let s = 0b11111;
            let t = 0b11111;
            let d = ((s << 1) & 0b11111) | ((t >> (5 - 1)) & 0b11111);
            println!("1: {:#b}", (s << 1) & 0b11111);
            println!("2: {:#b}", (t >> (5 - 1)) & 0b11111);
            println!("{:#b}", d);
            assert_eq!(0b11111, d);
        }
    }
}

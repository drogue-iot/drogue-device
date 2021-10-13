use embedded_hal::digital::v2::OutputPin;

// Led matrix driver supporting up to 32x32 led matrices.
pub struct LedMatrix<P, const ROWS: usize, const COLS: usize>
where
    P: OutputPin + 'static,
{
    pin_rows: [P; ROWS],
    pin_cols: [P; COLS],
    frame_buffer: Frame<COLS, ROWS>,
    row_p: usize,
}

/**
 * A 32 bit bitmap based
 *
 * TODO: Use const generic expressions to derive data size when stabilized
 */
const BITMAP_WIDTH: usize = 1;
// Using u8 for each word
const BITMAP_WORD_SIZE: usize = 8;
#[derive(Clone, Copy, PartialEq)]
pub struct Bitmap {
    data: [u8; BITMAP_WIDTH],
    nbits: usize,
}

impl core::fmt::Debug for Bitmap {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        for i in 0..self.nbits {
            if self.is_set(i) {
                write!(f, "1")?;
            } else {
                write!(f, "0")?;
            }
        }
        Ok(())
    }
}

#[cfg(feature = "defmt")]
impl defmt::Format for Bitmap {
    fn format(&self, f: defmt::Formatter<'_>) {
        let mut s: heapless::String<32> = heapless::String::new();
        for i in 0..self.nbits {
            if self.is_set(i) {
                s.push('1').unwrap();
            } else {
                s.push('0').unwrap();
            }
        }
        defmt::write!(f, "{}", s.as_str());
    }
}

impl Bitmap {
    // TODO: Change input to array when const generics...
    pub const fn new(input: u8, nbits: usize) -> Self {
        let mut data = [0; BITMAP_WIDTH];
        //for i in 0..input.len() {
        if nbits < BITMAP_WORD_SIZE {
            data[0] = input << (BITMAP_WORD_SIZE - nbits);
        } else {
            data[0] = input;
        }
        //}
        Self { data, nbits }
    }

    pub const fn empty(nbits: usize) -> Self {
        Self {
            data: [0; 1],
            nbits,
        }
    }

    pub fn set(&mut self, bit: usize) {
        assert!(bit < self.nbits);
        let idx: usize = bit / BITMAP_WORD_SIZE;
        let p: usize = bit % BITMAP_WORD_SIZE;
        self.data[idx] |= 1 << ((BITMAP_WORD_SIZE - 1) - p);
    }

    pub fn clear_all(&mut self) {
        for i in 0..self.data.len() {
            self.data[i] = 0;
        }
    }

    pub fn clear(&mut self, bit: usize) {
        assert!(bit < self.nbits);
        let idx: usize = bit / BITMAP_WORD_SIZE;
        let p: usize = bit % BITMAP_WORD_SIZE;
        self.data[idx] &= !(1 << ((BITMAP_WORD_SIZE - 1) - p));
    }

    pub fn is_set(&self, bit: usize) -> bool {
        assert!(bit < self.nbits);
        let idx: usize = bit / BITMAP_WORD_SIZE;
        let p: usize = bit % BITMAP_WORD_SIZE;
        (self.data[idx] & (1 << ((BITMAP_WORD_SIZE - 1) - p))) != 0
    }

    // Shift left by nbits bits
    pub fn shift_left(&mut self, nbits: usize) {
        for b in self.data.iter_mut() {
            *b <<= nbits;
        }
    }

    pub fn shift_right(&mut self, nbits: usize) {
        for b in self.data.iter_mut() {
            *b >>= nbits;
        }
    }

    pub fn or(&mut self, other: &Bitmap) {
        for i in 0..self.data.len() {
            self.data[i] |= other.data[i];
        }
    }

    pub fn and(&mut self, other: &Bitmap) {
        for i in 0..self.data.len() {
            self.data[i] &= other.data[i];
        }
    }
}

/**
 * A 32x32 matrix that can be displayed on a LED matrix.
 */
#[derive(Clone, Copy, PartialEq)]
pub struct Frame<const XSIZE: usize, const YSIZE: usize> {
    bitmap: [Bitmap; YSIZE],
}

impl<const XSIZE: usize, const YSIZE: usize> core::fmt::Debug for Frame<XSIZE, YSIZE> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        for (i, b) in self.bitmap.iter().enumerate() {
            for j in 0..b.nbits {
                if self.bitmap[i].is_set(j) {
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

#[cfg(feature = "defmt")]
impl<const XSIZE: usize, const YSIZE: usize> defmt::Format for Frame<XSIZE, YSIZE> {
    fn format(&self, f: defmt::Formatter<'_>) {
        let mut s: heapless::String<1056> = heapless::String::new();
        for (i, b) in self.bitmap.iter().enumerate() {
            for j in 0..b.nbits {
                if self.bitmap[i].is_set(j) {
                    s.push('1').unwrap();
                } else {
                    s.push('0').unwrap();
                }
            }
            s.push('\n').unwrap();
        }
        defmt::write!(f, "{}", s.as_str());
    }
}

impl<const XSIZE: usize, const YSIZE: usize> Frame<XSIZE, YSIZE> {
    pub const fn empty() -> Self {
        Self {
            bitmap: [Bitmap::empty(XSIZE); YSIZE],
        }
    }

    pub fn new(bitmap: [Bitmap; YSIZE]) -> Self {
        Self { bitmap }
    }

    fn clear(&mut self) {
        for m in self.bitmap.iter_mut() {
            m.clear_all();
        }
    }

    fn set(&mut self, x: usize, y: usize) {
        self.bitmap[y].set(x);
    }

    fn unset(&mut self, x: usize, y: usize) {
        self.bitmap[y].clear(x);
    }

    fn is_set(&self, x: usize, y: usize) -> bool {
        self.bitmap[y].is_set(x)
    }

    pub fn or(&mut self, other: &Frame<XSIZE, YSIZE>) {
        for i in 0..self.bitmap.len() {
            self.bitmap[i].or(&other.bitmap[i]);
        }
    }

    pub fn shift_left(&mut self, nbits: usize) {
        for i in 0..self.bitmap.len() {
            self.bitmap[i].shift_left(nbits);
        }
    }

    pub fn shift_right(&mut self, nbits: usize) {
        for i in 0..self.bitmap.len() {
            self.bitmap[i].shift_right(nbits);
        }
    }

    pub fn and(&mut self, other: &Frame<XSIZE, YSIZE>) {
        for i in 0..self.bitmap.len() {
            self.bitmap[i].and(&other.bitmap[i]);
        }
    }

    /*
    fn transition(&self, to: Frame<XSIZE, YSIZE>) -> Animation
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

impl<const XSIZE: usize, const YSIZE: usize> Default for Frame<XSIZE, YSIZE> {
    fn default() -> Self {
        Frame::empty()
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
            frame_buffer: Frame::empty(),
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

    pub fn apply(&mut self, frame: Frame<COLS, ROWS>) {
        self.frame_buffer = frame;
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
        self.pin_rows[self.row_p].set_high().ok();
        self.row_p = (self.row_p + 1) % self.pin_rows.len();
    }
}

pub trait ToFrame<const XSIZE: usize, const YSIZE: usize>: Sync {
    fn to_frame(&self) -> Frame<XSIZE, YSIZE>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bitmap() {
        let mut b: Bitmap = Bitmap::empty(5);
        b.set(0);
        b.set(2);
        b.set(4);
        assert!(b.is_set(0));
        assert!(!b.is_set(1));
        assert!(b.is_set(2));
        assert!(!b.is_set(3));
        assert!(b.is_set(4));

        b.clear(2);
        b.set(3);
        assert!(b.is_set(0));
        assert!(!b.is_set(1));
        assert!(!b.is_set(2));
        assert!(b.is_set(3));
        assert!(b.is_set(4));

        /*
        TODO: When const expressions is allowed
        let mut b: Bitmap = Bitmap::empty(33);
        b.set(16);
        b.set(32);
        assert!(b.is_set(16));
        assert!(b.is_set(32));
        */

        let b: Bitmap = Bitmap::new(0b01000, 5);
        assert!(!b.is_set(0));
        assert!(b.is_set(1));
        assert!(!b.is_set(2));
        assert!(!b.is_set(3));
        assert!(!b.is_set(4));

        let b: Bitmap = Bitmap::new(0b11110, 5);
        assert!(b.is_set(0));
        assert!(b.is_set(1));
        assert!(b.is_set(2));
        assert!(b.is_set(3));
        assert!(!b.is_set(4));

        let mut b: Bitmap = Bitmap::new(0b01110, 5);
        b.shift_left(1);
        assert!(b.is_set(0));
        assert!(b.is_set(1));
        assert!(b.is_set(2));
        assert!(!b.is_set(3));
        assert!(!b.is_set(4));

        b.shift_right(1);
        assert!(!b.is_set(0));
        assert!(b.is_set(1));
        assert!(b.is_set(2));
        assert!(b.is_set(3));
        assert!(!b.is_set(4));
    }
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

    pub fn frame_5x5<const XSIZE: usize, const YSIZE: usize>(
        input: &[u8; 5],
    ) -> Frame<XSIZE, YSIZE> {
        assert!(XSIZE == 5);
        assert!(YSIZE == 5);
        let mut data = [Bitmap::empty(5); YSIZE];
        for (i, bm) in input.iter().enumerate() {
            data[i] = Bitmap::new(*bm, 5);
        }
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
            _ => frame_5x5(&[
                0,
                0,
                0,
                0,
                0,
                ])
        }
    }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn test_frame() {
            let frame: Frame<5, 5> = 'D'.to_frame();

            assert!(frame.is_set(0, 0));
            assert!(frame.is_set(1, 0));
            assert!(frame.is_set(2, 0));
            assert!(frame.is_set(3, 0));
            assert!(!frame.is_set(4, 0));

            assert!(frame.is_set(0, 1));
            assert!(!frame.is_set(1, 1));
            assert!(!frame.is_set(2, 1));
            assert!(!frame.is_set(3, 1));
            assert!(frame.is_set(4, 1));

            assert!(frame.is_set(0, 2));
            assert!(!frame.is_set(1, 2));
            assert!(!frame.is_set(2, 2));
            assert!(!frame.is_set(3, 2));
            assert!(frame.is_set(4, 2));

            assert!(frame.is_set(0, 3));
            assert!(!frame.is_set(1, 3));
            assert!(!frame.is_set(2, 3));
            assert!(!frame.is_set(3, 3));
            assert!(frame.is_set(4, 3));

            assert!(frame.is_set(0, 4));
            assert!(frame.is_set(1, 4));
            assert!(frame.is_set(2, 4));
            assert!(frame.is_set(3, 4));
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

use core::ops::{AddAssign, SubAssign};
// TODO: Use const generic expressions to derive data size when stabilized
const BITMAP_WIDTH: usize = 1;
// Using u8 for each word
const BITMAP_WORD_SIZE: usize = 8;

/// A bitmap with room for 8 bits used by Frame to create a compact frame buffer
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
    /// Create a new bitmap with initial input and number of bits
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

    /// Create an empty bitmap with nbits bits
    pub const fn empty(nbits: usize) -> Self {
        Self {
            data: [0; 1],
            nbits,
        }
    }

    /// Set bit n in bitmap
    pub fn set(&mut self, bit: usize) {
        assert!(bit < self.nbits);
        let idx: usize = bit / BITMAP_WORD_SIZE;
        let p: usize = bit % BITMAP_WORD_SIZE;
        self.data[idx] |= 1 << ((BITMAP_WORD_SIZE - 1) - p);
    }

    /// Clear all bits in bitmap
    pub fn clear_all(&mut self) {
        for i in 0..self.data.len() {
            self.data[i] = 0;
        }
    }

    /// Clear bit n in bitmap
    pub fn clear(&mut self, bit: usize) {
        assert!(bit < self.nbits);
        let idx: usize = bit / BITMAP_WORD_SIZE;
        let p: usize = bit % BITMAP_WORD_SIZE;
        self.data[idx] &= !(1 << ((BITMAP_WORD_SIZE - 1) - p));
    }

    /// Check if bit n is set in bitmap
    pub fn is_set(&self, bit: usize) -> bool {
        assert!(bit < self.nbits);
        let idx: usize = bit / BITMAP_WORD_SIZE;
        let p: usize = bit % BITMAP_WORD_SIZE;
        (self.data[idx] & (1 << ((BITMAP_WORD_SIZE - 1) - p))) != 0
    }

    /// Shift left by nbits bits
    pub fn shift_left(&mut self, nbits: usize) {
        for b in self.data.iter_mut() {
            *b <<= nbits;
        }
    }

    /// Shift right by nbits bits
    pub fn shift_right(&mut self, nbits: usize) {
        for b in self.data.iter_mut() {
            *b >>= nbits;
        }
    }

    /// Logical OR with another bitmap
    pub fn or(&mut self, other: &Bitmap) {
        for i in 0..self.data.len() {
            self.data[i] |= other.data[i];
        }
    }

    /// Logical AND with another bitmap
    pub fn and(&mut self, other: &Bitmap) {
        for i in 0..self.data.len() {
            self.data[i] &= other.data[i];
        }
    }
}

/// An NxM frame that can be displayed on a LED matrix.
///
/// NOTE: Currently restricted to 8 bit width
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
    /// Create an empty frame
    pub const fn empty() -> Self {
        Self {
            bitmap: [Bitmap::empty(XSIZE); YSIZE],
        }
    }

    /// Create a frame from a bitmap array
    pub const fn new(bitmap: [Bitmap; YSIZE]) -> Self {
        Self { bitmap }
    }

    /// Clear this frame (empty)
    pub fn clear(&mut self) {
        for m in self.bitmap.iter_mut() {
            m.clear_all();
        }
    }

    /// Enable (x, y) on this frame
    pub fn set(&mut self, x: usize, y: usize) {
        self.bitmap[y].set(x);
    }

    /// Disable (x, y) on this frame
    pub fn unset(&mut self, x: usize, y: usize) {
        self.bitmap[y].clear(x);
    }

    /// Check if (x, y) is set on this frame
    pub fn is_set(&self, x: usize, y: usize) -> bool {
        self.bitmap[y].is_set(x)
    }

    /// Logical OR with another frame
    pub fn or(&mut self, other: &Frame<XSIZE, YSIZE>) {
        for i in 0..self.bitmap.len() {
            self.bitmap[i].or(&other.bitmap[i]);
        }
    }

    /// Shift all rows left
    pub fn shift_left(&mut self, nbits: usize) {
        for i in 0..self.bitmap.len() {
            self.bitmap[i].shift_left(nbits);
        }
    }

    /// Shift all rows right
    pub fn shift_right(&mut self, nbits: usize) {
        for i in 0..self.bitmap.len() {
            self.bitmap[i].shift_right(nbits);
        }
    }

    /// Logical AND with another frame
    pub fn and(&mut self, other: &Frame<XSIZE, YSIZE>) {
        for i in 0..self.bitmap.len() {
            self.bitmap[i].and(&other.bitmap[i]);
        }
    }
}

impl<const XSIZE: usize, const YSIZE: usize> Default for Frame<XSIZE, YSIZE> {
    fn default() -> Self {
        Frame::empty()
    }
}

/// A brightness setting for the display.
#[derive(Clone, Copy)]
pub struct Brightness(u8);

impl Brightness {
    /// Maximum brightness
    pub const MAX: Brightness = Brightness(10);

    /// Lowest brightness
    pub const MIN: Brightness = Brightness(0);

    /// Create a new brightness with a custom level
    pub fn new(level: u8) -> Self {
        Self(level.clamp(Self::MIN.0, Self::MAX.0))
    }

    /// Return the level value
    pub fn level(&self) -> u8 {
        self.0
    }
}

impl Default for Brightness {
    fn default() -> Self {
        Self(5)
    }
}

impl AddAssign<u8> for Brightness {
    fn add_assign(&mut self, rhs: u8) {
        self.0 += core::cmp::min(Self::MAX.level() - self.0, rhs);
    }
}

impl SubAssign<u8> for Brightness {
    fn sub_assign(&mut self, rhs: u8) {
        self.0 -= core::cmp::min(self.0, rhs);
    }
}

/*
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
*/

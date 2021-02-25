pub(crate) fn ascii_to_digit(character: u8) -> Option<u8> {
    match character {
        b'0' => Some(0),
        b'1' => Some(1),
        b'2' => Some(2),
        b'3' => Some(3),
        b'4' => Some(4),
        b'5' => Some(5),
        b'6' => Some(6),
        b'7' => Some(7),
        b'8' => Some(8),
        b'9' => Some(9),
        _ => None,
    }
}

pub(crate) fn atoi_u8(digits: &[u8]) -> Option<u8> {
    let mut num: u8 = 0;
    let len = digits.len();
    for (i, digit) in digits.iter().enumerate() {
        let digit = ascii_to_digit(*digit)?;
        let mut exp = 1;
        for _ in 0..(len - i - 1) {
            exp *= 10;
        }
        num += exp * digit;
    }
    Some(num)
}

pub(crate) fn atoi_usize(digits: &[u8]) -> Option<usize> {
    let mut num: usize = 0;
    let len = digits.len();
    for (i, digit) in digits.iter().enumerate() {
        let digit = ascii_to_digit(*digit)? as usize;
        let mut exp = 1;
        for _ in 0..(len - i - 1) {
            exp *= 10;
        }
        num += exp * digit;
    }
    Some(num)
}

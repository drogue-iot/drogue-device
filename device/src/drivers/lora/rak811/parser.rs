use nom::alt;
use nom::char;
use nom::character::streaming::digit1;
use nom::do_parse;
use nom::named;
use nom::opt;
use nom::tag;
use nom::take;
use nom::IResult;

use super::{protocol::Decoder, EventCode, FirmwareInfo, LoraRegion, Response};

fn ascii_to_digit(character: u8) -> Option<u8> {
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

fn atoi_u32(digits: &[u8]) -> Option<u32> {
    let mut num: u32 = 0;
    let len = digits.len();
    for (i, digit) in digits.iter().enumerate() {
        let digit = ascii_to_digit(*digit as u8)? as u32;
        let mut exp: u32 = 1;
        for _ in 0..(len - i - 1) {
            exp *= 10;
        }
        num += exp * digit;
    }
    Some(num)
}

fn atoi_u8(digits: &[u8]) -> Option<u8> {
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

fn parse_u8(input: &[u8]) -> IResult<&[u8], u8> {
    let (input, digits) = digit1(input)?;
    IResult::Ok((input, atoi_u8(digits).unwrap()))
}

fn parse_u32(input: &[u8]) -> IResult<&[u8], u32> {
    let (input, digits) = digit1(input)?;
    IResult::Ok((input, atoi_u32(digits).unwrap()))
}

#[rustfmt::skip]
named!(
    crlf,
    tag!("\r\n")
);

#[rustfmt::skip]
named!(
    pub mode_info<Response>,
    do_parse!(
        opt!(crlf) >>
        opt!(crlf) >>
        tag!("Selected LoraWAN ") >>
        major: parse_u8 >>
        tag!(".") >>
        minor: parse_u8 >>
        tag!(".") >>
        patch: parse_u8 >>
        tag!(" Region: ") >>
        region: lora_region >>
        tag!(" ") >>
        crlf >>
        crlf >>
        tag!("OK") >>
        crlf >>
        (
            Response::Ok
        )
    )
);

#[rustfmt::skip]
named!(
    pub ok<Response>,
    do_parse!(
        opt!(crlf) >>
        opt!(crlf) >>
        tag!("OK") >>
        crlf >>
        (
            Response::Ok
        )
    )
);

#[rustfmt::skip]
named!(
    pub error<Response>,
    do_parse!(
        opt!(crlf) >>
        opt!(crlf) >>
        tag!("ERROR") >>
        sign: opt!(char!('-')) >>
        code: parse_u8 >>
        crlf >>
        (
            Response::Error(sign.map(|s| if s == '-' { - (code as i8) } else {code as i8}).unwrap_or(code as i8))
        )
    )
);

#[rustfmt::skip]
named!(
    pub firmware_info<Response>,
    do_parse!(
        opt!(crlf) >>
        opt!(crlf) >>
        tag!("OK") >>
        major: parse_u8 >>
        tag!(".") >>
        minor: parse_u8 >>
        tag!(".") >>
        patch: parse_u8 >>
        tag!(".") >>
        build: parse_u8 >>
        crlf >>
        (
            Response::FirmwareInfo(FirmwareInfo{major, minor, patch, build})
        )
    )
);

#[rustfmt::skip]
named!(
    pub lora_region<LoraRegion>,
    do_parse!(
        region: alt!(
            tag!("EU868") |
            tag!("US915") |
            tag!("AU915") |
            tag!("KR920") |
            tag!("AS923") |
            tag!("IN865")
        ) >>
        (
                LoraRegion::decode(region)
        )
    )
);

#[rustfmt::skip]
named!(
    pub lora_band<Response>,
    do_parse!(
        tag!("OK") >>
        region: lora_region >>
        crlf >>
        (
            Response::LoraBand(region)
        )
    )
);

#[rustfmt::skip]
named!(
    pub status<Response>,
    do_parse!(
        tag!("OK") >>
        tx_ok: parse_u8 >>
        char!(',') >>
        tx_err: parse_u8 >>
        char!(',') >>
        rx_ok: parse_u8 >>
        char!(',') >>
        rx_timeout: parse_u8 >>
        char!(',') >>
        rx_err: parse_u8 >>
        char!(',') >>
        rssi_sign: opt!(char!('-')) >>
        rssi: parse_u8 >>
        char!(',') >>
        snr: parse_u32 >>
        crlf >>
        ( {
            Response::Status {
                tx_ok,
                tx_err,
                rx_ok,
                rx_timeout,
                rx_err,
                rssi: rssi_sign.map(|s| if s == '-' { - (rssi as i8) } else {rssi as i8}).unwrap_or(rssi as i8),
                snr,
            }
          }
        )
    )
);

#[rustfmt::skip]
named!(
    pub welcome<Response>,
    do_parse!(
        tag!("Welcome to RAK811") >>
        crlf >>
        crlf >>
        tag!("Selected LoraWAN ") >>
        major: parse_u8 >>
        tag!(".") >>
        minor: parse_u8 >>
        tag!(".") >>
        patch: parse_u8 >>
        tag!(" Region: ") >>
        region: lora_region >>
        tag!(" ") >>
        crlf >>
        crlf >>
        (
            Response::Initialized(region)
        )
    )
);

#[rustfmt::skip]
named!(
    pub recv<Response>,
    do_parse!(
        tag!("at+recv=") >>
        status: parse_u8 >>
        char!(',') >>
        port: parse_u8 >>
        char!(',') >>
        len: parse_u8 >>
        data: take!(len) >>
        crlf >>
        ( {
            let rx = if len > 0 {
                let mut buf: [u8; super::RECV_BUFFER_LEN] = [0; super::RECV_BUFFER_LEN];
                for (i, b) in data.iter().enumerate() {
                    buf[i] = *b;
                }
                Some(buf)
            } else {
                None
            };
            Response::Recv(EventCode::parse(status), port, len as usize, rx)
          }
        )
    )
);

named!(
    pub parse<Response>,
    alt!(
          ok
        | error
        | firmware_info
        | lora_band
        | mode_info
        | recv
        | status
        | welcome
    )
);

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}

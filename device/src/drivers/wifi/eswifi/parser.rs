//use drogue_nom_utils::parse_usize;
use nom::{alt, char, complete, do_parse, error::ErrorKind, named, tag, take_until};
use nom::{
    character::streaming::{crlf, digit1},
    IResult,
};

use embedded_nal_async::{IpAddr, Ipv4Addr};
//use crate::util::nom::{parse_u8, parse_usize};

named!(
    pub ok,
    tag!("OK\r\n")
);

named!(
    pub error,
    tag!("ERROR\r\n")
);

named!(
    pub prompt,
    tag!("> ")
);

#[derive(Debug)]
pub(crate) enum JoinResponse {
    Ok(IpAddr),
    JoinError,
}

#[rustfmt::skip]
named!(
    ip_addr<Ipv4Addr>,
    do_parse!(
        a: parse_u8 >>
        char!('.') >>
        b: parse_u8 >>
        char!('.') >>
        c: parse_u8 >>
        char!('.') >>
        d: parse_u8 >>
        (
            Ipv4Addr::new(a, b, c, d)
        )
    )
);

// [JOIN   ] drogue,192.168.1.174,0,0
#[rustfmt::skip]
named!(
    pub(crate) join<JoinResponse>,
    do_parse!(
        tag!("[JOIN   ] ") >>
        _ssid: take_until!(",") >>
        char!(',') >>
        //ip: take_until!(",") >>
        ip: ip_addr >>
        char!(',') >>
        tag!("0,0") >>
        tag!("\r\n") >>
        ok >>
        (
            //log::info!("ip --> {:?}", ip );
            JoinResponse::Ok(IpAddr::V4(ip))
        )
    )
);

// [JOIN   ] drogue
// [JOIN   ] Failed
named!(
    pub(crate) join_error<JoinResponse>,
    do_parse!(
        take_until!( "ERROR" ) >>
        error >>
        (
            JoinResponse::JoinError
        )
    )
);

named!(
    pub(crate) join_response<JoinResponse>,
    do_parse!(
        tag!("\r\n") >>
        response:
        alt!(
              complete!(join)
            | complete!(join_error)
        ) >>
        prompt >>
        (
            response
        )

    )
);

#[derive(Debug)]
pub(crate) enum ConnectResponse {
    Ok,
    Error,
}

named!(
    pub(crate) connected<ConnectResponse>,
    do_parse!(
        tag!("\r\n") >>
        tag!("[TCP  RC] Connecting to ") >>
        take_until!( "\r\n") >>
        tag!("\r\n") >>
        ok >>
        prompt >>
        (
            ConnectResponse::Ok
        )
    )
);

named!(
    pub(crate) connection_failure<ConnectResponse>,
    do_parse!(
        take_until!( "ERROR" ) >>
        error >>
        (
            ConnectResponse::Error
        )
    )
);

named!(
    pub(crate) connect_response<ConnectResponse>,
    alt!(
        complete!(connected)
        | complete!(connection_failure)
    )
);

#[derive(Debug)]
pub(crate) enum CloseResponse {
    Ok,
    Error,
}

named!(
    pub(crate) closed<CloseResponse>,
    do_parse!(
        tag!("\r\n") >>
        tag!("\r\n") >>
        ok >>
        prompt >>
        (
            CloseResponse::Ok
        )
    )
);

named!(
    pub(crate) close_error<CloseResponse>,
    do_parse!(
        tag!("\r\n") >>
        take_until!( "ERROR" ) >>
        error >>
        prompt >>
        (
            CloseResponse::Error
        )
    )
);

named!(
    pub(crate) close_response<CloseResponse>,
    alt!(
          complete!(closed)
        | complete!(close_error)
    )
);

#[derive(Debug)]
pub(crate) enum WriteResponse {
    Ok(usize),
    Error,
}

named!(
    pub(crate) write_ok<WriteResponse>,
    do_parse!(
        tag!("\r\n") >>
        len: parse_usize >>
        tag!("\r\n") >>
        ok >>
        prompt >>
        (
            WriteResponse::Ok(len)
        )
    )
);

named!(
    pub(crate) write_error<WriteResponse>,
    do_parse!(
        tag!("\r\n") >>
        tag!("-1") >>
        tag!("\r\n") >>
        ok >>
        prompt >>
        (
            WriteResponse::Error
        )
    )
);

named!(
    pub(crate) write_response<WriteResponse>,
    alt!(
          complete!(write_ok)
        | complete!(write_error)
    )
);

#[derive(Debug)]
pub enum ReadResponse<'a> {
    Ok(&'a [u8]),
    Err,
}

pub fn parse_response<'m>(input: &'m [u8]) -> IResult<&'m [u8], ReadResponse<'m>> {
    let (input, _) = crlf(input)?;

    const OK: &[u8] = b"\r\nOK\r\n> ";
    const ERROR: &[u8] = b"-1\r\nERROR\r\n> ";

    if input.len() >= OK.len() && &input[input.len() - OK.len()..] == OK {
        IResult::Ok((&[], ReadResponse::Ok(&input[..input.len() - OK.len()])))
    } else if input.len() >= ERROR.len() && &input[input.len() - ERROR.len()..] == ERROR {
        IResult::Ok((&[], ReadResponse::Err))
    } else {
        IResult::Err(nom::Err::Failure(nom::error::Error::new(
            input,
            ErrorKind::IsNot,
        )))
    }
}

pub fn parse_u8(input: &[u8]) -> IResult<&[u8], u8> {
    let (input, digits) = digit1(input)?;
    IResult::Ok((input, atoi_u8(digits).unwrap()))
}

pub fn parse_usize(input: &[u8]) -> IResult<&[u8], usize> {
    let (input, digits) = digit1(input)?;
    let num = atoi_usize(digits).unwrap();
    IResult::Ok((input, num))
}

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

#[cfg(test)]
mod tests {
    #[test]
    fn test_response_parser_eagerness() {
        let payload = &[
            0x01, 0x02, 0x0d, 0x0a, 0x4f, 0x4b, 0x0d, 0x0a, 0x3e, 0x20, 0x03, 0x04,
        ];
        let input = &[
            0x0d, 0x0a, 0x01, 0x02, 0x0d, 0x0a, 0x4f, 0x4b, 0x0d, 0x0a, 0x3e, 0x20, 0x03, 0x04,
            0x0d, 0x0a, 0x4f, 0x4b, 0x0d, 0x0a, 0x3e, 0x20,
        ];
        let result = super::parse_response(input);
        assert!(result.is_ok());
        let (_, response) = result.unwrap();
        if let super::ReadResponse::Ok(data) = response {
            assert_eq!(data, payload);
        } else {
            assert!(false);
        }
    }

    #[test]
    fn test_response_parser_expected_error() {
        let input = &[
            0x0d, 0x0a, 0x2d, 0x31, 0x0d, 0x0a, 0x45, 0x52, 0x52, 0x4f, 0x52, 0x0d, 0x0a, 0x3e,
            0x20,
        ];
        let result = super::parse_response(input);
        assert!(result.is_ok());
        let (_, response) = result.unwrap();
        assert!(matches!(response, super::ReadResponse::Err));
    }

    #[test]
    fn test_response_parser_unexpected_error() {
        let input = &[
            0x0d, 0x0a, 0x2d, 0x31, 0x0d, 0x0a, 0x52, 0x0d, 0x0a, 0x3e, 0x20,
        ];
        let result = super::parse_response(input);
        assert!(result.is_err());
    }
}

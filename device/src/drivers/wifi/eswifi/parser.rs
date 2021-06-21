//use drogue_nom_utils::parse_usize;
use nom::{alt, char, complete, do_parse, named, tag, take_until};
use nom::{character::streaming::digit1, IResult};

use crate::traits::ip::{IpAddress, IpAddressV4};
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
    Ok(IpAddress),
    JoinError,
}

#[rustfmt::skip]
named!(
    ip_addr<IpAddressV4>,
    do_parse!(
        a: parse_u8 >>
        char!('.') >>
        b: parse_u8 >>
        char!('.') >>
        c: parse_u8 >>
        char!('.') >>
        d: parse_u8 >>
        (
            IpAddressV4::new(a, b, c, d)
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
            JoinResponse::Ok(IpAddress::V4(ip))
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
pub(crate) enum ReadResponse<'a> {
    Ok(&'a [u8]),
    Err,
}

named!(
    pub(crate) read_data<ReadResponse>,
    do_parse!(
        tag!("\r\n") >>
        data: take_until!("\r\nOK\r\n>") >>
        tag!("\r\n") >>
        ok >>
        prompt >>
        (
            ReadResponse::Ok(data)
        )
    )
);

named!(
    pub(crate) read_error<ReadResponse>,
    do_parse!(
        tag!("\r\n") >>
        tag!("-1") >>
        tag!("\r\n") >>
        ok >>
        prompt >>
        (
            ReadResponse::Err
        )
    )
);

named!(
    pub(crate) read_response<ReadResponse>,
    alt!(
          complete!(read_data)
        | complete!(read_error)
    )
);

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

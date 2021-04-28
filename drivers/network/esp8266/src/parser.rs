use nom::alt;
use nom::char;
use nom::character::streaming::digit1;
use nom::do_parse;
use nom::named;
use nom::opt;
use nom::tag;
use nom::take;
use nom::take_until;
use nom::tuple;
use nom::IResult;

use drogue_network::ip::{IpAddress, IpAddressV4};

use crate::{
    num::{atoi_u8, atoi_usize},
    protocol::{FirmwareInfo, IpAddresses, ResolverAddresses, Response, WifiConnectionFailure},
    BUFFER_LEN,
};

fn parse_u8(input: &[u8]) -> IResult<&[u8], u8> {
    let (input, digits) = digit1(input)?;
    IResult::Ok((input, atoi_u8(digits).unwrap()))
}

fn parse_usize(input: &[u8]) -> IResult<&[u8], usize> {
    let (input, digits) = digit1(input)?;
    let num = atoi_usize(digits).unwrap();
    IResult::Ok((input, num))
}

#[rustfmt::skip]
named!(
    crlf,
    tag!("\r\n")
);

#[rustfmt::skip]
named!(
    pub ok<Response>,
    do_parse!(
        tuple!(
            opt!(crlf),
            opt!(crlf),
            tag!("OK"),
            crlf
        ) >>
        (
            Response::Ok
        )
    )
);

named!(
    pub reason<()>,
    do_parse!(
        // use "!alt" once we identified an additional reason
        link_invalid
        >> ()
    )
);

named!(
    pub link_invalid<()>,
    do_parse!(
        tag!("link is not valid") >> ()
    )
);

named!(
    pub error<Response>,
    do_parse!(
        opt!(reason) >>
        opt!(crlf) >>
        opt!(crlf) >>
        tag!("ERROR") >>
        crlf >>
        (
            Response::Error
        )
    )
);

#[rustfmt::skip]
named!(
    pub wifi_connected<Response>,
    do_parse!(
        tuple!(
            tag!("WIFI CONNECTED"),
            crlf
        ) >>
        (
            Response::WifiConnected
        )
    )
);

#[rustfmt::skip]
named!(
    pub wifi_disconnect<Response>,
    do_parse!(
        tuple!(
            tag!("WIFI DISCONNECT"),
            crlf
        ) >>
        (
            Response::WifiDisconnect
        )
    )
);

#[rustfmt::skip]
named!(
    pub got_ip<Response>,
    do_parse!(
        tuple!(
            tag!("WIFI GOT IP"),
            crlf
        ) >>
        (
            Response::GotIp
        )
    )
);

named!(
    pub wifi_connection_failure<Response>,
    do_parse!(
        tag!("+CWJAP:") >>
        code: parse_u8 >>
        crlf >>
        crlf >>
        tag!("FAIL") >>
        crlf >>
        (
            Response::WifiConnectionFailure(WifiConnectionFailure::from(code))
        )
    )
);

#[rustfmt::skip]
named!(
    pub firmware_info<Response>,
    do_parse!(
        tag!("AT version:") >>
        major: parse_u8 >>
        tag!(".") >>
        minor: parse_u8 >>
        tag!(".") >>
        patch: parse_u8 >>
        tag!(".") >>
        build: parse_u8 >>
        take_until!("OK") >>
        ok >>
        (
            Response::FirmwareInfo(FirmwareInfo{major, minor, patch, build})
        )
    )
);

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

#[rustfmt::skip]
named!(
    pub ip_addresses<Response>,
    do_parse!(
        tag!("+CIPSTA_CUR:ip:\"") >>
        ip: ip_addr >>
        tag!("\"") >>
        crlf >>
        tag!("+CIPSTA_CUR:gateway:\"") >>
        gateway: ip_addr >>
        tag!("\"") >>
        crlf >>
        tag!("+CIPSTA_CUR:netmask:\"") >>
        netmask: ip_addr >>
        tag!("\"") >>
        crlf >>
        crlf >>
        ok >>
        (
            Response::IpAddresses(
                IpAddresses {
                    ip,
                    gateway,
                    netmask,
                }
            )
        )
    )
);

#[rustfmt::skip]
named!(
    pub connect<Response>,
    do_parse!(
        link_id: parse_u8 >>
        tag!(",CONNECT") >>
        crlf >>
        ok >>
        (
            Response::Connect(link_id as usize)
        )
    )
);

named!(
    pub ready_for_data<Response>,
    do_parse!(
        tag!("> ") >>
        (
            Response::ReadyForData
        )
    )
);

named!(
    pub received_data_to_send<Response>,
    do_parse!(
        opt!( crlf ) >>
        tag!("Recv ") >>
        len: parse_usize >>
        tag!(" bytes") >>
        crlf >>
        (
            Response::ReceivedDataToSend(len)
        )
    )
);

named!(
    pub send_ok<Response>,
    do_parse!(
        opt!( crlf ) >>
        tag!("SEND OK") >>
        crlf >>
        (
            Response::SendOk
        )
    )
);

named!(
    pub send_fail<Response>,
    do_parse!(
        opt!( crlf ) >>
        tag!("SEND FAIL") >>
        crlf >>
        (
            Response::SendFail
        )
    )
);

named!(
    pub data_available<Response>,
    do_parse!(
        opt!( crlf ) >>
        tag!( "+IPD,") >>
        link_id: parse_usize >>
        char!(',') >>
        len: parse_usize >>
        crlf >>
        (
            Response::DataAvailable {link_id, len }
        )
    )
);

named!(
    pub closed<Response>,
    do_parse!(
        opt!(crlf) >>
        link_id: parse_usize >>
        tag!(",CLOSED") >>
        crlf >>
        (
            Response::Closed(link_id)
        )
    )
);

named!(
    pub data_received<Response>,
    do_parse!(
        opt!(tag!("\r")) >>
        opt!(tag!("\n")) >>
        tag!("+CIPRECVDATA,") >>
        len: parse_usize >>
        char!(':') >>
        data: take!(len) >>
        crlf >>
        ok >>
        ( {
            let mut buf = [0; BUFFER_LEN];
            for (i, b) in data.iter().enumerate() {
                //log::info!( "copy {} @ {}", *b as char, i);
                buf[i] = *b;
            }
            //log::info!("------------> onwards {:?}", buf);
            Response::DataReceived(buf, len)
        } )
    )
);

named!(
    pub dns_resolvers<Response>,
    do_parse!(
        tag!("+CIPDNS_CUR:") >>
        ns1: ip_addr >> crlf >>
        ns2: opt!(
            do_parse!(
                tag!("+CIPDNS_CUR:") >>
                ns2: ip_addr >> crlf >>
                (
                    ns2
                )
            )
        ) >>
        ok >>
        (
            Response::Resolvers(
                ResolverAddresses{
                    resolver1: ns1,
                    resolver2: ns2, /* ns2 is an Option */
                }
            )
        )
    )
);

named!(
    pub dns_lookup<Response>,
    do_parse!(
        tag!("+CIPDOMAIN:") >>
        ip_addr: ip_addr >>
        crlf >>
        ok >>
        (
            Response::IpAddress(IpAddress::V4(ip_addr))
        )
    )
);

named!(
    pub dns_fail<Response>,
    do_parse!(
        tag!("DNS Fail") >>
        crlf >>
        error >>
        (
            Response::DnsFail
        )
    )
);

named!(
    pub unlink_fail<Response>,
    do_parse!(
        opt!(crlf) >>
        tag!("UNLINK") >>
        crlf >>
        error >>
        (
            Response::UnlinkFail
        )
    )
);

named!(
    pub parse<Response>,
    alt!(
          ok
        | error
        | firmware_info
        | wifi_connected
        | wifi_disconnect
        | wifi_connection_failure
        | got_ip
        | ip_addresses
        | connect
        | closed
        | ready_for_data
        | received_data_to_send
        | send_ok
        | send_fail
        | data_available
        | data_received
        | dns_resolvers
        | dns_lookup
        | dns_fail
        | unlink_fail
    )
);

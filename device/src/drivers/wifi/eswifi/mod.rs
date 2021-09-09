mod parser;

use embedded_hal::digital::v2::OutputPin;
use embedded_hal::digital::v2::InputPin;

use crate::actors::wifi::Adapter;
use crate::traits::{
    ip::{IpAddress, IpProtocol, SocketAddress},
    tcp::{TcpError, TcpStack},
    wifi::{Join, JoinError, WifiSupplicant},
};

use embedded_hal::blocking::spi::{Transfer, Write};
use heapless::String;
use core::fmt::Write as FmtWrite;
use core::future::Future;
use embassy::time::{Duration, Timer};

use parser::{
    JoinResponse,
};

#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Error<SPI, CS, RESET, READY> {
    Uninformative,
    VersionMismatch(u8),
    CS(CS),
    Reset(RESET),
    SPI(SPI),
    READY(READY),
    Transmitting,
}

use Error::*;
const NAK: u8 = 0x15;

macro_rules! command {
    ($size:tt, $($arg:tt)*) => ({
        //let mut c = String::new();
        //c
        let mut c = String::<$size>::new();
        write!(c, $($arg)*).unwrap();
        c.push_str("\r").unwrap();
        c
    })
}

pub struct EsWifiController<SPI, CS, RESET, WAKEUP, READY, E>
where
    SPI: Transfer<u8, Error = E> + Write<u8, Error = E>,
    CS: OutputPin + 'static,
    RESET: OutputPin + 'static,
    WAKEUP: OutputPin + 'static,
    READY: InputPin + 'static,
    E: 'static,
{
    spi: SPI,
    cs: CS,
    reset: RESET,
    wakeup: WAKEUP,
    ready: READY,
}

impl<SPI, CS, RESET, WAKEUP, READY, E> EsWifiController<SPI, CS, RESET, WAKEUP, READY, E>
where
    SPI: Transfer<u8, Error = E> + Write<u8, Error = E>,
    CS: OutputPin + 'static,
    RESET: OutputPin + 'static,
    WAKEUP: OutputPin + 'static,
    READY: InputPin + 'static,
    E: 'static,
{
    pub fn new(
        spi: SPI,
        cs: CS,
        reset: RESET,
        wakeup: WAKEUP,
        ready: READY,
    ) -> Self {
        Self {
            spi,
            cs,
            reset,
            wakeup,
            ready,
        }
    }

    async fn wakeup(&mut self) {
        self.wakeup.set_low().ok().unwrap();
        Timer::after(Duration::from_millis(50)).await;
        self.wakeup.set_high().ok().unwrap();
        Timer::after(Duration::from_millis(50)).await;
    }

    async fn reset(&mut self) {
        self.reset.set_low().ok().unwrap();
        Timer::after(Duration::from_millis(50)).await;
        self.reset.set_high().ok().unwrap();
        Timer::after(Duration::from_millis(50)).await;
    }

    pub async fn start(&mut self) -> Result<(), Error<E, CS::Error, RESET::Error, READY::Error>>{
        info!("Starting eS-WiFi adapter!");

        self.reset().await;
        self.wakeup().await;

        let mut response = [0; 4];
        let mut pos = 0;

        while self.ready.is_low().map_err(READY)? {}

        loop {
            if pos >= response.len() {
                break;
            }

            let mut chunk = [0x0A, 0x0A];
            self.cs.set_low().map_err(CS)?;
            while self.ready.is_low().map_err(READY)? {}
            self.spi.transfer(&mut chunk).map_err(SPI)?;
            self.cs.set_high().map_err(CS)?;

            // reverse order going from 16 -> 2*8 bits
            if chunk[1] != NAK {
                response[pos] = chunk[1];
                pos += 1;
            }
            if chunk[0] != NAK {
                response[pos] = chunk[0];
                pos += 1;
            }
        }

        let needle = &[b'\r', b'\n', b'>', b' '];

        if !response[0..pos].starts_with(needle) {
            info!(
                "eS-WiFi adapter failed to initialize {:?}",
                &response[0..pos]
            );
        } else {
            // disable verbosity
            let mut resp = [0; 16];
            self.send_string(&command!(8, "MT=1"), &mut resp)
                .await?;
            //self.state = State::Ready;
            info!("eS-WiFi adapter is ready");
        }

        Ok(())
    }

    pub async fn join_wep(&mut self, ssid: &str, password: &str) -> Result<IpAddress, JoinError> {
        let mut response = [0; 1024];

        self.send_string(&command!(36, "CB=2"), &mut response)
            .await
            .map_err(|_| JoinError::InvalidSsid)?;

        self.send(&command!(36, "C1={}", ssid).as_bytes(), &mut response)
            .await
            .map_err(|_| JoinError::InvalidSsid)?;

        self.send(&command!(72, "C2={}", password).as_bytes(), &mut response)
            .await
            .map_err(|_| JoinError::InvalidPassword)?;

        self.send(&command!(8, "C3=4").as_bytes(), &mut response)
            .await
            .map_err(|_| JoinError::Unknown)?;

        let response = self
            .send(&command!(4, "C0").as_bytes(), &mut response)
            .await
            .map_err(|_| JoinError::Unknown)?;

        //info!("[[{}]]", response);

        let parse_result = parser::join_response(&response);

        match parse_result {
            Ok((_, response)) => match response {
                JoinResponse::Ok(ip) => Ok(ip),
                JoinResponse::JoinError => Err(JoinError::UnableToAssociate),
            },
            Err(_) => {
                info!("{:?}", &response);
                Err(JoinError::UnableToAssociate)
            }
        }
    }

    async fn send_string<'a, const N: usize>(
        &'a mut self,
        command: &String<N>,
        response: &'a mut [u8],
    ) -> Result<&'a [u8], Error<E, CS::Error, RESET::Error, READY::Error>> {
        self.send(command.as_bytes(), response).await
    }

    async fn send<'a>(
        &'a mut self,
        command: &[u8],
        response: &'a mut [u8],
    ) -> Result<&'a [u8], Error<E, CS::Error, RESET::Error, READY::Error>> {
        //info!("send {:?}", core::str::from_utf8(command).unwrap());

        while self.ready.is_low().map_err(READY)? {}

        self.cs.set_low().map_err(CS)?;
        for chunk in command.chunks(2) {
            let mut xfer: [u8; 2] = [0; 2];
            xfer[1] = chunk[0];
            if chunk.len() == 2 {
                xfer[0] = chunk[1]
            } else {
                xfer[0] = 0x0A
            }

            self.spi.transfer(&mut xfer).map_err(SPI)?;
        }
        self.cs.set_high().map_err(CS)?;

        self.receive(response).await
    }

    async fn receive<'a>(&'a mut self, response: &'a mut [u8]) -> Result<&'a [u8], Error<E, CS::Error, RESET::Error, READY::Error>> {
        let mut pos = 0;

        self.cs.set_low().map_err(CS)?;
        while self.ready.is_low().map_err(READY)? {}

        self.cs.set_low().map_err(CS)?;
        loop {
            if pos >= response.len() {
                break;
            }

            let mut xfer: [u8; 2] = [0x0A, 0x0A];

            self.spi.transfer(&mut xfer).map_err(SPI)?;

            response[pos] = xfer[1];
            pos += 1;
            if xfer[0] != NAK {
                response[pos] = xfer[0];
                pos += 1;
            }

            if self.ready.is_low().map_err(READY)? {
                break;
            }
        }
        self.cs.set_high().map_err(CS)?;

        Ok(&response[0..pos])
    }

}

impl<SPI, CS, RESET, WAKEUP, READY, E> WifiSupplicant for EsWifiController<SPI, CS, RESET, WAKEUP, READY, E>
where
    SPI: Transfer<u8, Error = E> + Write<u8, Error = E>,
    CS: OutputPin + 'static,
    RESET: OutputPin + 'static,
    WAKEUP: OutputPin + 'static,
    READY: InputPin + 'static,
    E: 'static,
{
    #[rustfmt::skip]
    type JoinFuture<'m> where SPI: 'm = impl Future<Output = Result<IpAddress, JoinError>> + 'm;
    fn join<'m>(&'m mut self, join_info: Join<'m>) -> Self::JoinFuture<'m> {
        async move { todo!() }
    }
}

impl<SPI, CS, RESET, WAKEUP, READY, E> TcpStack for EsWifiController<SPI, CS, RESET, WAKEUP, READY, E>
where
    SPI: Transfer<u8, Error = E> + Write<u8, Error = E>,
    CS: OutputPin + 'static,
    RESET: OutputPin + 'static,
    WAKEUP: OutputPin + 'static,
    READY: InputPin + 'static,
    E: 'static,
{
    type SocketHandle = u8;

    #[rustfmt::skip]
    type OpenFuture<'m> where SPI: 'm = impl Future<Output = Self::SocketHandle> + 'm;
    fn open<'m>(&'m mut self) -> Self::OpenFuture<'m> {
        async move { todo!() }
    }

    #[rustfmt::skip]
    type ConnectFuture<'m> where SPI: 'm = impl Future<Output = Result<(), TcpError>> + 'm;
    fn connect<'m>(
        &'m mut self,
        handle: Self::SocketHandle,
        _: IpProtocol,
        dst: SocketAddress,
    ) -> Self::ConnectFuture<'m> {
        async move { todo!() }
    }

    #[rustfmt::skip]
    type WriteFuture<'m> where SPI: 'm = impl Future<Output = Result<usize, TcpError>> + 'm;
    fn write<'m>(&'m mut self, handle: Self::SocketHandle, buf: &'m [u8]) -> Self::WriteFuture<'m> {
        async move { todo!() }
    }

    #[rustfmt::skip]
    type ReadFuture<'m> where SPI: 'm = impl Future<Output = Result<usize, TcpError>> + 'm;
    fn read<'m>(
        &'m mut self,
        handle: Self::SocketHandle,
        buf: &'m mut [u8],
    ) -> Self::ReadFuture<'m> {
        async move { todo!() }
    }

    #[rustfmt::skip]
    type CloseFuture<'m> where SPI: 'm,  = impl Future<Output = ()> + 'm;
    fn close<'m>(&'m mut self, handle: Self::SocketHandle) -> Self::CloseFuture<'m> {
        async move { todo!() }
    }
}

impl<SPI, CS, RESET, WAKEUP, READY, E> Adapter for EsWifiController<SPI, CS, RESET, WAKEUP, READY, E>
where
    SPI: Transfer<u8, Error = E> + Write<u8, Error = E>,
    CS: OutputPin + 'static,
    RESET: OutputPin + 'static,
    WAKEUP: OutputPin + 'static,
    READY: InputPin + 'static,
    E: 'static,
{}

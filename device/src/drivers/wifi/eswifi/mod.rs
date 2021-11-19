mod parser;

use crate::drivers::common::socket_pool::SocketPool;

use embedded_hal::digital::v2::InputPin;
use embedded_hal::digital::v2::OutputPin;

use crate::actors::wifi::Adapter;
use crate::traits::{
    ip::{IpAddress, IpProtocol, SocketAddress},
    tcp::{TcpError, TcpStack},
    wifi::{Join, JoinError, WifiSupplicant},
};

use core::fmt::Write as FmtWrite;
use core::future::Future;
use embassy::time::{block_for, Duration, Timer};
use embassy::traits::gpio::WaitForAnyEdge;
//use embassy::traits::spi::*;
use embedded_hal::blocking::spi::*;
use heapless::String;

use parser::{CloseResponse, ConnectResponse, JoinResponse, ReadResponse, WriteResponse};

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

pub struct Cs<'a, CS: OutputPin + 'a> {
    cs: &'a mut CS,
}

impl<'a, CS: OutputPin + 'a> Cs<'a, CS> {
    fn new(cs: &'a mut CS) -> Result<Self, CS::Error> {
        cs.set_low()?;
        block_for(Duration::from_micros(1000));
        Ok(Self { cs })
    }
}

impl<'a, CS: OutputPin + 'a> Drop for Cs<'a, CS> {
    fn drop(&mut self) {
        let _ = self.cs.set_high();
        block_for(Duration::from_micros(15));
    }
}

pub struct EsWifiController<SPI, CS, RESET, WAKEUP, READY, E>
where
    SPI: Transfer<u8, Error = E>,
    CS: OutputPin + 'static,
    RESET: OutputPin + 'static,
    WAKEUP: OutputPin + 'static,
    READY: InputPin + WaitForAnyEdge + 'static,
    E: 'static,
{
    spi: SPI,
    cs: CS,
    reset: RESET,
    wakeup: WAKEUP,
    ready: READY,
    socket_pool: SocketPool,
}

impl<SPI, CS, RESET, WAKEUP, READY, E> EsWifiController<SPI, CS, RESET, WAKEUP, READY, E>
where
    SPI: Transfer<u8, Error = E>,
    CS: OutputPin + 'static,
    RESET: OutputPin + 'static,
    WAKEUP: OutputPin + 'static,
    READY: InputPin + WaitForAnyEdge + 'static,
    E: 'static,
{
    pub fn new(spi: SPI, cs: CS, reset: RESET, wakeup: WAKEUP, ready: READY) -> Self {
        Self {
            spi,
            cs,
            reset,
            wakeup,
            ready,
            socket_pool: SocketPool::new(),
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

    async fn wait_ready(&mut self) -> Result<(), Error<E, CS::Error, RESET::Error, READY::Error>> {
        while self.ready.is_low().map_err(READY)? {
            // self.ready.wait_for_any_edge().await;
        }
        Ok(())
    }

    async fn wait_not_ready(
        &mut self,
    ) -> Result<(), Error<E, CS::Error, RESET::Error, READY::Error>> {
        while self.ready.is_high().map_err(READY)? {
            self.ready.wait_for_any_edge().await;
        }
        Ok(())
    }

    pub async fn start(&mut self) -> Result<(), Error<E, CS::Error, RESET::Error, READY::Error>> {
        info!("Starting eS-WiFi adapter!");

        self.reset().await;
        self.wakeup().await;

        let mut response = [0; 4];
        let mut pos = 0;

        self.wait_ready().await?;
        {
            let _cs = Cs::new(&mut self.cs).map_err(CS)?;
            loop {
                if self.ready.is_low().map_err(READY)? {
                    break;
                }

                if pos >= response.len() {
                    break;
                }

                let mut chunk = [0x0A, 0x0A];
                Self::spi_transfer(&mut self.spi, &mut chunk, &[0x0A, 0x0A]).await?;

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
            self.send_string(command!(8, "MT=1"), &mut resp).await?;
            //self.state = State::Ready;
            info!("eS-WiFi adapter is ready");
        }

        Ok(())
    }

    pub async fn join_wep(&mut self, ssid: &str, password: &str) -> Result<IpAddress, JoinError> {
        let mut response = [0; 1024];

        self.send_string(command!(36, "CB=2"), &mut response)
            .await
            .map_err(|_| JoinError::InvalidSsid)?;

        self.send_string(command!(36, "C1={}", ssid), &mut response)
            .await
            .map_err(|_| JoinError::InvalidSsid)?;

        self.send_string(command!(72, "C2={}", password), &mut response)
            .await
            .map_err(|_| JoinError::InvalidPassword)?;

        self.send_string(command!(8, "C3=4"), &mut response)
            .await
            .map_err(|_| JoinError::Unknown)?;

        let response = self
            .send_string(command!(4, "C0"), &mut response)
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
        mut command: String<N>,
        response: &'a mut [u8],
    ) -> Result<&'a [u8], Error<E, CS::Error, RESET::Error, READY::Error>> {
        if command.len() % 2 != 0 {
            command.push('\n').unwrap();
        }
        self.send(command.as_bytes(), response).await
    }

    async fn send<'a>(
        &'a mut self,
        command: &[u8],
        response: &'a mut [u8],
    ) -> Result<&'a [u8], Error<E, CS::Error, RESET::Error, READY::Error>> {
        //trace!("send {:?}", core::str::from_utf8(&command[..]).unwrap());

        self.wait_ready().await?;
        {
            let _cs = Cs::new(&mut self.cs).map_err(CS)?;
            for chunk in command.chunks(2) {
                let mut xfer: [u8; 2] = [0; 2];
                xfer[1] = chunk[0];
                if chunk.len() == 2 {
                    xfer[0] = chunk[1]
                } else {
                    xfer[0] = 0x0A
                }

                let a = xfer[0];
                let b = xfer[1];
                Self::spi_transfer(&mut self.spi, &mut xfer[..], &[a, b]).await?;
            }
            /*assert!(command.len() % 2 == 0);
            self.spi.transfer(&mut command[..]).map_err(SPI)?;*/
        }
        //info!("sent! awaiting response");

        self.receive(response, 0).await
    }

    async fn spi_transfer(
        spi: &mut SPI,
        rx: &mut [u8],
        _tx: &[u8],
    ) -> Result<(), Error<E, CS::Error, RESET::Error, READY::Error>> {
        spi.transfer(rx).map_err(SPI)?;
        Ok(())
    }

    async fn receive<'a>(
        &'a mut self,
        response: &'a mut [u8],
        min_len: usize,
    ) -> Result<&'a [u8], Error<E, CS::Error, RESET::Error, READY::Error>> {
        let mut pos = 0;

        //trace!("Awaiting response ready");
        self.wait_ready().await?;
        //trace!("Response ready... reading");

        let _cs = Cs::new(&mut self.cs).map_err(CS)?;

        while self.ready.is_high().map_err(READY)? && response.len() - pos > 0 {
            //trace!("Receive pos({}), len({})", pos, response.len());

            let mut xfer: [u8; 2] = [0x0A, 0x0A];
            Self::spi_transfer(&mut self.spi, &mut xfer, &[0x0A, 0x0A]).await?;

            response[pos] = xfer[1];
            pos += 1;

            if xfer[0] != NAK || pos <= min_len {
                response[pos] = xfer[0];
                pos += 1;
            }
        }

        // Flush data
        let mut count = 0;
        while self.ready.is_high().map_err(READY)? {
            if count % 10000 == 0 {
                break;
            }
            let mut xfer: [u8; 2] = [0x0A, 0x0A];
            Self::spi_transfer(&mut self.spi, &mut xfer, &[0x0A, 0x0A]).await?;
            count += 1;
        }
        /*
        if count > 0 {
            info!("flushed {} bytes after receive", count);
            trace!(
                "response {} bytes:  {:?}",
                pos,
                core::str::from_utf8(&response[0..pos]).unwrap()
            );
        }
        */

        Ok(&response[0..pos])
    }
}

impl<SPI, CS, RESET, WAKEUP, READY, E> WifiSupplicant
    for EsWifiController<SPI, CS, RESET, WAKEUP, READY, E>
where
    SPI: Transfer<u8, Error = E>,
    CS: OutputPin + 'static,
    RESET: OutputPin + 'static,
    WAKEUP: OutputPin + 'static,
    READY: InputPin + WaitForAnyEdge + 'static,
    E: 'static,
{
    type JoinFuture<'m>
    where
        SPI: 'm,
    = impl Future<Output = Result<IpAddress, JoinError>> + 'm;
    fn join<'m>(&'m mut self, join_info: Join<'m>) -> Self::JoinFuture<'m> {
        async move {
            match join_info {
                Join::Open => Err(JoinError::Unknown),
                Join::Wpa { ssid, password } => self.join_wep(ssid, password).await,
            }
        }
    }
}

impl<SPI, CS, RESET, WAKEUP, READY, E> TcpStack
    for EsWifiController<SPI, CS, RESET, WAKEUP, READY, E>
where
    SPI: Transfer<u8, Error = E>,
    CS: OutputPin + 'static,
    RESET: OutputPin + 'static,
    WAKEUP: OutputPin + 'static,
    READY: InputPin + WaitForAnyEdge + 'static,
    E: 'static,
{
    type SocketHandle = u8;

    type OpenFuture<'m>
    where
        SPI: 'm,
    = impl Future<Output = Result<Self::SocketHandle, TcpError>> + 'm;
    fn open<'m>(&'m mut self) -> Self::OpenFuture<'m> {
        async move {
            let h = self
                .socket_pool
                .open()
                .await
                .map_err(|_| TcpError::OpenError)?;
            trace!("Opened socket {}", h);
            Ok(h)
        }
    }

    type ConnectFuture<'m>
    where
        SPI: 'm,
    = impl Future<Output = Result<(), TcpError>> + 'm;
    fn connect<'m>(
        &'m mut self,
        handle: Self::SocketHandle,
        proto: IpProtocol,
        dst: SocketAddress,
    ) -> Self::ConnectFuture<'m> {
        async move {
            let mut response = [0u8; 1024];

            let result = async {
                self.send_string(command!(8, "P0={}", handle), &mut response)
                    .await
                    .map_err(|_| TcpError::ConnectError)?;

                match proto {
                    IpProtocol::Tcp => {
                        self.send_string(command!(8, "P1=0"), &mut response)
                            .await
                            .map_err(|_| TcpError::ConnectError)?;
                    }
                    IpProtocol::Udp => {
                        self.send_string(command!(8, "P1=1"), &mut response)
                            .await
                            .map_err(|_| TcpError::ConnectError)?;
                    }
                }

                self.send_string(command!(32, "P3={}", dst.ip()), &mut response)
                    .await
                    .map_err(|_| TcpError::ConnectError)?;

                self.send_string(command!(32, "P4={}", dst.port()), &mut response)
                    .await
                    .map_err(|_| TcpError::ConnectError)?;

                let response = self
                    .send_string(command!(8, "P6=1"), &mut response)
                    .await
                    .map_err(|_| TcpError::ConnectError)?;

                if let Ok((_, ConnectResponse::Ok)) = parser::connect_response(&response) {
                    Ok(())
                } else {
                    Err(TcpError::ConnectError)
                }
            }
            .await;
            result
        }
    }

    type WriteFuture<'m>
    where
        SPI: 'm,
    = impl Future<Output = Result<usize, TcpError>> + 'm;
    fn write<'m>(&'m mut self, handle: Self::SocketHandle, buf: &'m [u8]) -> Self::WriteFuture<'m> {
        async move {
            let mut response = [0u8; 32];
            let mut remaining = buf.len();
            trace!("Write request with {} bytes", remaining);
            self.send_string(command!(8, "P0={}", handle), &mut response)
                .await
                .map_err(|_| TcpError::WriteError)?;
            while remaining > 0 {
                // info!("Writing buf with len {}", len);

                let to_send = core::cmp::min(1460, remaining);
                trace!("Writing {} bytes to adapter", to_send);

                remaining -= to_send;
                async {
                    let mut prefix = command!(16, "S3={}", to_send).into_bytes();

                    let (prefix, data) = if prefix.len() % 2 == 0 {
                        (&prefix[..], &buf[..to_send])
                    } else {
                        prefix.push(buf[0]).unwrap();
                        (&prefix[..], &buf[1..to_send])
                    };

                    self.wait_ready().await.map_err(|_| TcpError::WriteError)?;

                    {
                        let _cs = Cs::new(&mut self.cs).map_err(|_| TcpError::WriteError)?;
                        for chunk in prefix.chunks(2) {
                            let mut xfer: [u8; 2] = [0; 2];
                            xfer[1] = chunk[0];
                            if chunk.len() == 2 {
                                xfer[0] = chunk[1]
                            } else {
                                xfer[0] = 0x0A
                            }

                            let a = xfer[0];
                            let b = xfer[1];

                            Self::spi_transfer(&mut self.spi, &mut xfer, &[a, b])
                                .await
                                .map_err(|_| TcpError::WriteError)?;
                        }

                        for chunk in data.chunks(2) {
                            let mut xfer: [u8; 2] = [0; 2];
                            xfer[1] = chunk[0];
                            if chunk.len() == 2 {
                                xfer[0] = chunk[1]
                            } else {
                                xfer[0] = 0x0A
                            }

                            let a = xfer[0];
                            let b = xfer[1];

                            Self::spi_transfer(&mut self.spi, &mut xfer, &[a, b])
                                .await
                                .map_err(|_| TcpError::WriteError)?;
                        }
                    }

                    let response = self
                        .receive(&mut response, 0)
                        .await
                        .map_err(|_| TcpError::WriteError)?;

                    if let Ok((_, WriteResponse::Ok(len))) = parser::write_response(response) {
                        Ok(len)
                    } else {
                        //info!("response:  {:?}", core::str::from_utf8(&response).unwrap());
                        Err(TcpError::WriteError)
                    }
                }
                .await?;
            }
            Ok(buf.len())
        }
    }

    type ReadFuture<'m>
    where
        SPI: 'm,
    = impl Future<Output = Result<usize, TcpError>> + 'm;
    fn read<'m>(
        &'m mut self,
        handle: Self::SocketHandle,
        buf: &'m mut [u8],
    ) -> Self::ReadFuture<'m> {
        async move {
            let mut pos = 0;
            //let buf_len = buf.len();
            loop {
                let result = async {
                    let mut response = [0u8; 1470];

                    self.send_string(command!(8, "P0={}", handle), &mut response)
                        .await
                        .map_err(|_| TcpError::ReadError)?;

                    let maxlen = buf.len() - pos;
                    let len = core::cmp::min(response.len() - 10, maxlen);

                    self.send_string(command!(16, "R1={}", len), &mut response)
                        .await
                        .map_err(|_| TcpError::ReadError)?;

                    /*
                    self.send_string(&command!(8, "R2=1000"), &mut response)
                        .await
                        .map_err(|_| TcpError::ReadError)?;
                    */

                    self.send_string(command!(8, "R3=1"), &mut response)
                        .await
                        .map_err(|_| TcpError::ReadError)?;

                    self.wait_ready().await.map_err(|_| TcpError::ReadError)?;

                    {
                        let _cs = Cs::new(&mut self.cs).map_err(|_| TcpError::ReadError)?;

                        let mut xfer = [b'0', b'R'];
                        Self::spi_transfer(&mut self.spi, &mut xfer, &[b'0', b'R'])
                            .await
                            .map_err(|_| TcpError::ReadError)?;

                        xfer = [b'\n', b'\r'];
                        Self::spi_transfer(&mut self.spi, &mut xfer, &[b'\n', b'\r'])
                            .await
                            .map_err(|_| TcpError::ReadError)?;
                    }

                    trace!(
                        "Receiving {} bytes, total buffer size is {}, pos is {}",
                        len,
                        buf.len(),
                        pos
                    );
                    let response = self
                        .receive(&mut response, len)
                        .await
                        .map_err(|_| TcpError::ReadError)?;

                    if let Ok((_, ReadResponse::Ok(data))) = parser::read_response(&response) {
                        if pos + data.len() > buf.len() {
                            info!(
                                "Buf len is {}, pos is {}, Len is {}, data len is {}",
                                buf.len(),
                                pos,
                                len,
                                data.len()
                            );
                            if let Ok(s) = core::str::from_utf8(&data) {
                                warn!("response parsed:  {:?}", s);
                            }
                            warn!("response raw data: {:?}", response);
                            Err(TcpError::ReadError)
                        } else {
                            for (i, b) in data.iter().enumerate() {
                                buf[pos + i] = *b;
                            }
                            Ok(data.len())
                        }
                    } else {
                        /*info!(
                            "ERR: response parsed:  {:?}",
                            core::str::from_utf8(&response).unwrap()
                        );*/
                        Err(TcpError::ReadError)
                    }
                }
                .await;

                match result {
                    Ok(len) => {
                        pos += len;
                        if len == 0 || pos == buf.len() {
                            return Ok(pos);
                        }
                    }
                    Err(e) => {
                        if pos == 0 {
                            return Err(e);
                        } else {
                            return Ok(pos);
                        }
                    }
                }
            }
        }
    }

    type CloseFuture<'m>
    where
        SPI: 'm,
    = impl Future<Output = Result<(), TcpError>> + 'm;
    fn close<'m>(&'m mut self, handle: Self::SocketHandle) -> Self::CloseFuture<'m> {
        async move {
            trace!("Closing connection for {}", handle);
            self.socket_pool.close(handle);
            let mut response = [0u8; 32];

            self.send_string(command!(8, "P0={}", handle), &mut response)
                .await
                .map_err(|_| TcpError::CloseError)?;

            let response = self
                .send_string(command!(8, "P6=0"), &mut response)
                .await
                .map_err(|_| TcpError::CloseError)?;

            if let Ok((_, CloseResponse::Ok)) = parser::close_response(&response) {
                trace!("Connection closed");
                self.socket_pool.close(handle);
                Ok(())
            } else {
                trace!("Error closing connection");
                /*info!(
                    "close response:  {:?}",
                    core::str::from_utf8(&response).unwrap()
                );*/
                self.socket_pool.close(handle);
                Err(TcpError::CloseError)
            }
        }
    }
}

impl<SPI, CS, RESET, WAKEUP, READY, E> Adapter
    for EsWifiController<SPI, CS, RESET, WAKEUP, READY, E>
where
    SPI: Transfer<u8, Error = E>,
    CS: OutputPin + 'static,
    RESET: OutputPin + 'static,
    WAKEUP: OutputPin + 'static,
    READY: InputPin + WaitForAnyEdge + 'static,
    E: 'static,
{
}

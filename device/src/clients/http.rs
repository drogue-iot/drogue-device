use crate::traits::{
    ip::{IpAddress, IpProtocol, SocketAddress},
    tcp::TcpSocket,
};
use core::fmt::Write;
use heapless::{consts, String};

pub struct HttpClient<'a, S>
where
    S: TcpSocket + 'static,
{
    socket: &'a mut S,
    ip: IpAddress,
    port: u16,
    username: &'a str,
    password: &'a str,
}

impl<'a, S> HttpClient<'a, S>
where
    S: TcpSocket + 'static,
{
    pub fn new(
        socket: &'a mut S,
        ip: IpAddress,
        port: u16,
        username: &'a str,
        password: &'a str,
    ) -> Self {
        Self {
            socket,
            ip,
            port,
            username,
            password,
        }
    }

    pub async fn post(
        &mut self,
        path: &str,
        payload: &[u8],
        content_type: &str,
        rx_buf: &mut [u8],
    ) -> Result<usize, ()> {
        match self
            .socket
            .connect(IpProtocol::Tcp, SocketAddress::new(self.ip, self.port))
            .await
        {
            Ok(_) => {
                info!("Connected to {}:{}", self.ip, self.port);
                let mut combined: String<consts::U128> = String::new();
                write!(combined, "{}:{}", self.username, self.password).unwrap();
                let mut authz = [0; 256];
                let authz_len =
                    base64::encode_config_slice(combined.as_bytes(), base64::STANDARD, &mut authz);
                let mut request: String<consts::U1024> = String::new();
                write!(request, "POST {} HTTP/1.1\r\n", path).unwrap();
                write!(request, "Authorization: Basic {}\r\n", unsafe {
                    core::str::from_utf8_unchecked(&authz[..authz_len])
                })
                .unwrap();
                write!(request, "Content-Type: {}\r\n", content_type).unwrap();
                write!(request, "Content-Length: {}\r\n\r\n", payload.len()).unwrap();
                let result = self
                    .socket
                    .write(&request.as_bytes()[..request.len()])
                    .await;
                match result {
                    Ok(_) => {
                        let result = self.socket.write(payload).await;
                        match result {
                            Ok(_) => {
                                info!("Request sent");
                                let response = self
                                    .socket
                                    .read(&mut rx_buf[..])
                                    .await
                                    .expect("error reading response");
                                info!("Got {} bytes in response", response);
                                return Ok(response);
                            }
                            Err(e) => {
                                warn!("Error sending data: {:?}", e);
                            }
                        }
                    }
                    Err(e) => {
                        warn!("Error sending headers: {:?}", e);
                    }
                }
            }
            Err(e) => {
                warn!("Error connecting to {}:{}: {:?}", self.ip, self.port, e);
            }
        }
        Err(())
    }
}

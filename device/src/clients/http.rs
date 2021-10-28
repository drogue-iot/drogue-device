use crate::traits::{
    ip::{IpAddress, IpProtocol, SocketAddress},
    tcp::TcpSocket,
};
use core::convert::{TryFrom, TryInto};
use core::fmt::{Display, Write};
use heapless::String;

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

    pub async fn request<'m>(
        &mut self,
        request: Request<'m>,
        rx_buf: &'m mut [u8],
    ) -> Result<Response<'m>, ()> {
        match self
            .socket
            .connect(IpProtocol::Tcp, SocketAddress::new(self.ip, self.port))
            .await
        {
            Ok(_) => {
                info!("Connected to {}:{}", self.ip, self.port);
                let mut combined: String<128> = String::new();
                write!(combined, "{}:{}", self.username, self.password).unwrap();
                let mut authz = [0; 256];
                let authz_len =
                    base64::encode_config_slice(combined.as_bytes(), base64::STANDARD, &mut authz);
                let mut data: String<1024> = String::new();
                write!(
                    data,
                    "{} {} HTTP/1.1\r\n",
                    request.method,
                    request.path.unwrap_or("/")
                )
                .unwrap();
                write!(data, "Authorization: Basic {}\r\n", unsafe {
                    core::str::from_utf8_unchecked(&authz[..authz_len])
                })
                .unwrap();
                if let Some(content_type) = request.content_type {
                    write!(data, "Content-Type: {}\r\n", content_type).unwrap();
                }
                if let Some(payload) = request.payload {
                    write!(data, "Content-Length: {}\r\n\r\n", payload.len()).unwrap();
                }
                let result = self.socket.write(&data.as_bytes()[..data.len()]).await;
                match result {
                    Ok(_) => {
                        info!("Request sent");
                        match request.payload {
                            None => {
                                return self.read_response(rx_buf).await;
                            }
                            Some(payload) => {
                                let result = self.socket.write(payload).await;
                                match result {
                                    Ok(_) => {
                                        info!("Payload sent");
                                        return self.read_response(rx_buf).await;
                                    }
                                    Err(e) => {
                                        warn!("Error sending data: {:?}", e);
                                    }
                                }
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

    async fn read_response<'m>(&mut self, rx_buf: &'m mut [u8]) -> Result<Response<'m>, ()> {
        let mut pos = 0;
        let mut done = false;
        let mut buf: [u8; 1024] = [0; 1024];
        let mut header_end = 0;
        while pos < buf.len() && !done {
            let n = self.socket.read(&mut buf[pos..]).await.map_err(|_| ())?;

            /*
            info!(
                "read data from socket:  {:?}",
                core::str::from_utf8(&buf[pos..pos + n]).unwrap()
            );
            */
            pos += n;

            // Look for header end
            if pos >= 4 {
                //trace!("Looking for header between 0..{}", pos);
                for p in 0..pos - 4 {
                    if &buf[p..p + 4] == b"\r\n\r\n" {
                        header_end = p + 4;
                        done = true;
                        break;
                    }
                }
            }
        }

        // Parse header
        let mut status = Status::BadRequest;
        let mut content_type = None;
        let mut content_length = 0;

        let header = core::str::from_utf8(&buf[..header_end]).map_err(|_| ())?;

        let lines = header.split("\r\n");
        for line in lines {
            if line.starts_with("HTTP") {
                let pos = b"HTTP/N.N ".len();
                status = line[pos..pos + 3]
                    .parse::<u32>()
                    .map_err(|_| ())?
                    .try_into()?;
            // FIXME: Make it case insensitive
            } else if line.starts_with("Content-Type: ") {
                content_type.replace(line["Content-Type: ".len()..].into());
            } else if line.starts_with("Content-Length: ") {
                content_length = line["Content-Length: ".len()..]
                    .parse::<usize>()
                    .map_err(|_| ())?;
            }
        }

        let mut payload = None;
        if content_length > 0 {
            //            trace!("READING {} bytes of content", content_length);
            let to_read = core::cmp::min(rx_buf.len(), content_length);

            let to_copy = core::cmp::min(to_read, pos - header_end);
            /*
            trace!(
                "to_read({}), to_copy({}), header_end({}), pos({})",
                to_read,
                to_copy,
                header_end,
                pos
            );
            */
            rx_buf[..to_copy].copy_from_slice(&buf[header_end..header_end + to_copy]);

            let len = if to_copy < to_read {
                // Fetch rest from socket
                let to_read = to_read - to_copy;
                let mut pos = 0;
                while pos < to_read {
                    let n = self
                        .socket
                        .read(&mut rx_buf[to_copy + pos..to_copy + to_read])
                        .await
                        .map_err(|_| ())?;
                    pos += n;
                }
                to_copy + to_read
            } else {
                to_copy
            };
            payload.replace(&rx_buf[..len]);
        }

        let response = Response {
            status,
            content_type,
            payload,
        };
        trace!("HTTP response: {:?}", response);
        Ok(response)
    }
}

pub struct Request<'a> {
    method: Method,
    path: Option<&'a str>,
    payload: Option<&'a [u8]>,
    content_type: Option<ContentType>,
}

impl<'a> Request<'a> {
    pub fn post() -> Self {
        Self {
            method: Method::POST,
            path: None,
            content_type: None,
            payload: None,
        }
    }

    pub fn path(mut self, path: &'a str) -> Self {
        self.path.replace(path);
        self
    }

    pub fn payload(mut self, payload: &'a [u8]) -> Self {
        self.payload.replace(payload);
        self
    }

    pub fn content_type(mut self, content_type: ContentType) -> Self {
        self.content_type.replace(content_type);
        self
    }
}

pub enum Method {
    POST,
}

impl Display for Method {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> Result<(), core::fmt::Error> {
        match self {
            Method::POST => {
                write!(f, "POST")?;
            }
        }
        Ok(())
    }
}

#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct Response<'a> {
    pub status: Status,
    pub content_type: Option<ContentType>,
    pub payload: Option<&'a [u8]>,
}

#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Status {
    Ok = 200,
    Created = 201,
    BadRequest = 400,
    Unauthorized = 401,
    NotFound = 404,
}

impl TryFrom<u32> for Status {
    type Error = ();
    fn try_from(from: u32) -> Result<Status, Self::Error> {
        match from {
            200 => Ok(Status::Ok),
            201 => Ok(Status::Created),
            400 => Ok(Status::BadRequest),
            401 => Ok(Status::Unauthorized),
            404 => Ok(Status::NotFound),
            _ => Err(()),
        }
    }
}

#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum ContentType {
    ApplicationJson,
    ApplicationOctetStream,
}

impl<'a> From<&'a str> for ContentType {
    fn from(from: &'a str) -> ContentType {
        match from {
            "application/json" => ContentType::ApplicationJson,
            _ => ContentType::ApplicationOctetStream,
        }
    }
}

impl Display for ContentType {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> Result<(), core::fmt::Error> {
        match self {
            ContentType::ApplicationJson => {
                write!(f, "application/json")?;
            }
            ContentType::ApplicationOctetStream => {
                write!(f, "application/octet-stream")?;
            }
        }
        Ok(())
    }
}

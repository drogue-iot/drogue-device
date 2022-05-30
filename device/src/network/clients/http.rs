use core::fmt::{Display, Write as _};
use core::{num::ParseIntError, str::Utf8Error};
use embedded_io::{
    asynch::{Read, Write},
    Error as _,
};
use heapless::String;

pub trait Network: Read + Write {}
impl<N: Read + Write> Network for N {}

pub struct HttpClient<'a, N>
where
    N: Network + 'a,
{
    connection: &'a mut N,
    host: &'a str,
    username: &'a str,
    password: &'a str,
}

impl<'a, N> HttpClient<'a, N>
where
    N: Network + 'a,
{
    pub fn new(connection: &'a mut N, host: &'a str, username: &'a str, password: &'a str) -> Self {
        Self {
            connection,
            host,
            username,
            password,
        }
    }

    pub async fn request<'m>(
        &mut self,
        request: Request<'m>,
        rx_buf: &'m mut [u8],
    ) -> Result<Response<'m>, Error> {
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
        write!(data, "Host: {}\r\n", self.host).unwrap();
        write!(data, "Authorization: Basic {}\r\n", unsafe {
            core::str::from_utf8_unchecked(&authz[..authz_len])
        })
        .unwrap();
        if let Some(content_type) = request.content_type {
            write!(data, "Content-Type: {}\r\n", content_type).unwrap();
        }
        if let Some(payload) = request.payload {
            write!(data, "Content-Length: {}\r\n", payload.len()).unwrap();
        }
        if let Some(extra_headers) = request.extra_headers {
            for (header, value) in extra_headers.iter() {
                write!(data, "{}: {}\r\n", header, value).unwrap();
            }
        }
        write!(data, "\r\n").unwrap();
        trace!("Writing header");
        let result = self
            .connection
            .write(&data.as_bytes()[..data.len()])
            .await
            .map_err(|e| e.kind());
        let result = match result {
            Ok(_) => {
                trace!("Header written");
                match request.payload {
                    None => Self::read_response(&mut self.connection, rx_buf).await,
                    Some(payload) => {
                        trace!("Writing data");
                        let result = self.connection.write(payload).await;
                        match result {
                            Ok(_) => Self::read_response(&mut self.connection, rx_buf).await,
                            Err(e) => {
                                warn!("Error sending data: {:?}", e.kind());
                                Err(Error::Network(e.kind()))
                            }
                        }
                    }
                }
            }
            Err(e) => {
                warn!("Error sending headers: {:?}", e);
                Err(e.into())
            }
        };
        result
    }

    async fn read_response<'m>(
        connection: &mut N,
        rx_buf: &'m mut [u8],
    ) -> Result<Response<'m>, Error> {
        let mut pos = 0;
        let mut buf: [u8; 1024] = [0; 1024];
        let mut header_end = 0;
        while pos < buf.len() {
            let n = connection
                .read(&mut buf[pos..])
                .await
                .map_err(|e| e.kind())?;

            /*
            info!(
                "read data from socket:  {:?}",
                core::str::from_utf8(&buf[pos..pos + n]).unwrap()
            );*/
            pos += n;

            // Look for header end
            if let Some(n) = find_sequence(&buf[..pos], b"\r\n\r\n") {
                header_end = n + 4;
                break;
            }
        }

        // Parse header
        let mut status = Status::BadRequest;
        let mut content_type = None;
        let mut content_length = 0;

        let header = core::str::from_utf8(&mut buf[..header_end])?;

        trace!("Received header: {}", header);

        let lines = header.split("\r\n");
        for line in lines {
            if line.starts_with("HTTP") {
                let pos = b"HTTP/N.N ".len();
                status = line[pos..pos + 3].parse::<u32>()?.into();
            } else if match_header(line, "content-type") {
                content_type.replace(line["content-type:".len()..].trim_start().into());
            } else if match_header(line, "content-length") {
                content_length = line["content-length:".len()..]
                    .trim_start()
                    .parse::<usize>()?;
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
                // Fetch rest from connection
                let to_read = to_read - to_copy;
                let mut pos = 0;
                while pos < to_read {
                    let n = connection
                        .read(&mut rx_buf[to_copy + pos..to_copy + to_read])
                        .await
                        .map_err(|e| e.kind())?;
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
    extra_headers: Option<&'a [(&'a str, &'a str)]>,
}

impl<'a> Request<'a> {
    pub fn post() -> Self {
        Self {
            method: Method::POST,
            path: None,
            content_type: None,
            payload: None,
            extra_headers: None,
        }
    }

    pub fn headers(mut self, headers: &'a [(&'a str, &'a str)]) -> Self {
        self.extra_headers.replace(headers);
        self
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
pub enum Error {
    Network(embedded_io::ErrorKind),
    DnsLookupFailed,
    Parse,
}

impl From<embedded_io::ErrorKind> for Error {
    fn from(e: embedded_io::ErrorKind) -> Error {
        Error::Network(e)
    }
}

impl From<ParseIntError> for Error {
    fn from(_: ParseIntError) -> Error {
        Error::Parse
    }
}

impl From<Utf8Error> for Error {
    fn from(_: Utf8Error) -> Error {
        Error::Parse
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
    Accepted = 202,
    BadRequest = 400,
    Unauthorized = 401,
    Forbidden = 403,
    NotFound = 404,
    Unknown = 0,
}

impl From<u32> for Status {
    fn from(from: u32) -> Status {
        match from {
            200 => Status::Ok,
            201 => Status::Created,
            202 => Status::Accepted,
            400 => Status::BadRequest,
            401 => Status::Unauthorized,
            403 => Status::Forbidden,
            404 => Status::NotFound,
            n => {
                warn!("Unknown status code: {:?}", n);
                Status::Unknown
            }
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

// Find the needle sequence in the haystack. If found, return the hackstack position
// where the sequence was found.
fn find_sequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if haystack.len() < needle.len() {
        None
    } else {
        let mut p = 0;
        let mut windows = haystack.windows(needle.len());
        loop {
            if let Some(w) = windows.next() {
                if w == needle {
                    return Some(p);
                }
                p += 1;
            } else {
                return None;
            }
        }
    }
}

fn match_header(line: &str, hdr: &str) -> bool {
    if line.len() >= hdr.len() {
        line[0..hdr.len()].eq_ignore_ascii_case(hdr)
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sequence() {
        assert_eq!(Some(0), find_sequence(b"\r\n\r\n", b"\r\n\r\n"));
        assert_eq!(Some(3), find_sequence(b"foo\r\n\r\n", b"\r\n\r\n"));
        assert_eq!(Some(0), find_sequence(b"\r\n\r\nfoo", b"\r\n\r\n"));
        assert_eq!(Some(3), find_sequence(b"foo\r\n\r\nbar", b"\r\n\r\n"));
        assert_eq!(None, find_sequence(b"foobar\r\n\rother", b"\r\n\r\n"));
        assert_eq!(None, find_sequence(b"foo", b"\r\n\r\n"));
    }

    #[test]
    fn test_match_header() {
        assert!(match_header("Content-Length: 4", "Content-Length"));
        assert!(match_header("content-length: 4", "Content-Length"));
        assert!(match_header("Content-length: 4", "Content-Length"));
        assert!(!match_header(
            "Content-type: application/json",
            "Content-Length"
        ));
    }
}

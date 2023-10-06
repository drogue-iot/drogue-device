use {
    embedded_nal_async::{Dns, TcpConnect},
    embedded_update::{Command, Status, UpdateService},
    reqwless::{
        client::HttpClient,
        headers::ContentType,
        request::{Method, RequestBuilder},
        response::Status as ResponseStatus,
        Error as HttpError,
    },
    serde::Serialize,
};

/// An update service implementation for the Drogue Cloud update service.
pub struct HttpUpdater<'a, TCP, DNS, const MTU: usize>
where
    TCP: TcpConnect + 'a,
    DNS: Dns + 'a,
{
    client: HttpClient<'a, TCP, DNS>,
    url: &'a str,
    username: &'a str,
    password: &'a str,
    buf: [u8; MTU],
}

impl<'a, TCP, DNS, const MTU: usize> HttpUpdater<'a, TCP, DNS, MTU>
where
    TCP: TcpConnect + 'a,
    DNS: Dns + 'a,
{
    /// Construct a new Drogue update service
    pub fn new(
        client: &'a TCP,
        dns: &'a DNS,
        tls: reqwless::client::TlsConfig<'a>,
        url: &'a str,
        username: &'a str,
        password: &'a str,
    ) -> Self {
        Self {
            client: HttpClient::new_with_tls(client, dns, tls),
            url,
            username,
            password,
            buf: [0; MTU],
        }
    }
}

/// An error returned from the update service.
#[derive(Debug)]
pub enum Error<N, H, C> {
    /// Error from the underlying network
    Network(N),
    /// Error from HTTP client
    Http(H),
    /// Error from TLS
    Tls,
    /// Error in encoding or decoding of the payload
    Codec(C),
    /// Error in the firmware update protocol
    Protocol,
}

impl<'a, TCP, DNS, const MTU: usize> UpdateService for HttpUpdater<'a, TCP, DNS, MTU>
where
    TCP: TcpConnect + 'a,
    DNS: Dns + 'a,
{
    type Error = Error<TCP::Error, HttpError, serde_cbor::Error>;

    async fn request<'m>(&'m mut self, status: &'m Status<'m>) -> Result<Command<'m>, Self::Error> {
        let mut payload = [0; 64];
        let writer = serde_cbor::ser::SliceWrite::new(&mut payload[..]);
        let mut ser = serde_cbor::Serializer::new(writer).packed_format();
        status.serialize(&mut ser).map_err(Error::Codec)?;
        let writer = ser.into_inner();
        let size = writer.bytes_written();
        debug!("Status payload is {} bytes", size);

        let req = self
            .client
            .request(Method::POST, self.url)
            .await
            .map_err(Error::Http)?;

        let mut req = req
            .body(&payload[..size])
            .basic_auth(self.username, self.password)
            .content_type(ContentType::ApplicationCbor);

        let response = req.send(&mut self.buf[..]).await.map_err(Error::Http)?;

        if response.status == ResponseStatus::Ok
            || response.status == ResponseStatus::Accepted
            || response.status == ResponseStatus::Created
        {
            if let Ok(payload) = response.body().read_to_end().await {
                let command: Command<'m> =
                    serde_cbor::de::from_mut_slice(payload).map_err(Error::Codec)?;
                Ok(command)
            } else {
                Ok(Command::new_wait(Some(10), None))
            }
        } else {
            Err(Error::Protocol)
        }
    }
}

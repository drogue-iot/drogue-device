use crate::api::ip::tcp::{TcpSocket, TcpStack};
use crate::driver::tls::TlsError;
use core::fmt::{Debug, Formatter};
use heapless::{consts::*, Vec};

pub struct ApplicationData {
    pub(crate) header: Vec<u8, U16>,
    pub(crate) data: Vec<u8, U8192>,
}

impl Debug for ApplicationData {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        write!(f, "ApplicationData {:x?}", self.data)
    }
}

impl ApplicationData {
    pub async fn read<T: TcpStack>(
        socket: &mut TcpSocket<T>,
        len: u16,
        header: &[u8],
    ) -> Result<Self, TlsError> {
        log::info!("application data of len={}", len);
        //let mut buf: [u8; 8192] = [0; 8192];
        let mut buf = Vec::<u8, U8192>::new();
        buf.resize(len as usize, 0);

        let mut num_read = 0;

        loop {
            num_read += socket
                .read(&mut buf[num_read..len as usize])
                .await
                .map_err(|_| TlsError::InvalidApplicationData)?;

            if num_read == len as usize {
                log::info!("read application data fully");
                break;
            }
        }
        Ok(Self {
            header: Vec::from_slice(header).unwrap(),
            data: buf,
            //data: Vec::from_slice(&buf[0..len as usize])
            //.map_err(|_| TlsError::InvalidApplicationData)?,
        })
    }
}

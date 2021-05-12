use crate::fmt::*;
use bbqueue::*;
use core::{
    pin::Pin,
    task::{Context, Poll},
};
use embassy::io::{AsyncBufRead, AsyncWrite, Result};
use embassy::traits::uart::{ReadUntilIdle, Write};
use embassy::util::{AtomicWaker, Unborrow};

// TODO: Use typenum
const BUFFER_SIZE: usize = 255;
type BufSize = consts::U2048;

pub struct Serial {
    tx: BBBuffer<BufSize>,
    tx_waker: AtomicWaker,

    rx: BBBuffer<BufSize>,
    rx_waker: AtomicWaker,
}

impl Serial {
    pub fn new() -> Self {
        Self {
            tx: BBBuffer::new(),
            tx_waker: AtomicWaker::new(),
            rx: BBBuffer::new(),
            rx_waker: AtomicWaker::new(),
        }
    }

    pub fn initialize<'a, U>(&'a mut self, uart: U) -> Result<(SerialApi<'a>, SerialDriver<'a, U>)>
    where
        U: Write + ReadUntilIdle,
    {
        let (tx_prod, tx_cons) = self.tx.try_split().map_err(|_| embassy::io::Error::Other)?;
        let (rx_prod, rx_cons) = self.rx.try_split().map_err(|_| embassy::io::Error::Other)?;

        let api = SerialApi {
            tx: tx_prod,
            tx_waker: &self.tx_waker,

            rx: rx_cons,
            current_rx: None,
            rx_waker: &self.rx_waker,
        };

        let driver = SerialDriver {
            uart,

            tx: tx_cons,
            tx_waker: &self.tx_waker,

            rx: rx_prod,
            rx_waker: &self.rx_waker,
        };

        Ok((api, driver))
    }
}

pub struct SerialApi<'a> {
    tx: Producer<'a, BufSize>,
    tx_waker: &'a AtomicWaker,

    rx: Consumer<'a, BufSize>,
    current_rx: Option<GrantR<'a, BufSize>>,
    rx_waker: &'a AtomicWaker,
}

impl<'a> AsyncBufRead for SerialApi<'a> {
    fn poll_fill_buf(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<&[u8]>> {
        let this = unsafe { self.get_unchecked_mut() };
        let grant = this.rx.read();
        match grant {
            Ok(grant) => {
                let buf = unsafe { grant.as_static_buf() };
                this.current_rx.replace(grant);
                Poll::Ready(Ok(buf))
            }
            Err(Error::InsufficientSize) => {
                this.rx_waker.register(cx.waker());
                Poll::Pending
            }
            Err(_) => Poll::Ready(Err(embassy::io::Error::Other)),
        }
    }
    fn consume(self: Pin<&mut Self>, amt: usize) {
        let this = unsafe { self.get_unchecked_mut() };
        if let Some(grant) = this.current_rx.take() {
            grant.release(amt);
        }
    }
}

impl<'a> AsyncWrite for SerialApi<'a> {
    fn poll_write(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<Result<usize>> {
        let this = unsafe { self.get_unchecked_mut() };
        let grant = this.tx.grant_max_remaining(buf.len());
        match grant {
            Ok(mut grant) => {
                let tx_buf = grant.buf();
                let n = core::cmp::min(tx_buf.len(), buf.len());
                tx_buf[..n].copy_from_slice(&buf[..n]);
                grant.commit(n);
                Poll::Ready(Ok(n))
            }
            Err(Error::InsufficientSize) => {
                this.tx_waker.register(cx.waker());
                Poll::Pending
            }
            Err(_) => Poll::Ready(Err(embassy::io::Error::Other)),
        }
    }
}

pub struct SerialDriver<'a, U>
where
    U: Write + ReadUntilIdle,
{
    uart: U,

    tx: Consumer<'a, BufSize>,
    tx_waker: &'a AtomicWaker,

    rx: Producer<'a, BufSize>,
    rx_waker: &'a AtomicWaker,
}

impl<'a, U> SerialDriver<'a, U>
where
    U: Write + ReadUntilIdle,
{
    pub async fn run(&mut self) {
        info!("Running driver");
        loop {
            // Write all buffered data
            match self.tx.read() {
                Ok(grant) => {
                    let buf = grant.buf();
                    self.uart.write(buf).await;
                    self.tx_waker.wake();
                }
                _ => {
                    // Nothing to write
                }
            }

            // Read as much data as we can
            match self.rx.grant_max_remaining(BUFFER_SIZE) {
                Ok(mut grant) => {
                    let buf = grant.buf();
                    if let Ok(n) = self.uart.read_until_idle(buf).await {
                        grant.commit(n);
                        self.rx_waker.wake();
                    } else {
                        grant.commit(0);
                    }
                }
                _ => {
                    // Skipping
                }
            }
        }
    }
}

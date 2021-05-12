use crate::kernel::channel::*;
use bbqueue::*;
use core::{
    pin::Pin,
    task::{Context, Poll},
};
use embassy::interrupt::{Interrupt, InterruptExt};
use embassy::io::{AsyncBufRead, AsyncWrite, Result};
use embassy::util::{AtomicWaker, Unborrow};
use embassy_extras::peripheral::*;
use embedded_hal::serial::*;
use nb;

const BUFFER_SIZE: usize = 2048;
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

    pub fn initialize<'a, W, R, IRQ>(
        &'a mut self,
        w: W,
        r: R,
        irq: IRQ,
    ) -> Result<(SerialApi<'a>, PeripheralMutex<SerialDriver<'a, W, R, IRQ>>)>
    where
        R: Read<u8>,
        W: Write<u8>,
        IRQ: Interrupt,
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
            w,
            r,
            _irq: core::marker::PhantomData,

            tx: tx_cons,
            tx_waker: &self.tx_waker,

            rx: rx_prod,
            rx_waker: &self.rx_waker,
        };

        Ok((api, PeripheralMutex::new(driver, irq)))
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

pub struct SerialDriver<'a, W, R, IRQ>
where
    W: Write<u8>,
    R: Read<u8>,
    IRQ: Interrupt,
{
    w: W,
    r: R,

    _irq: core::marker::PhantomData<IRQ>,

    tx: Consumer<'a, BufSize>,
    tx_waker: &'a AtomicWaker,

    rx: Producer<'a, BufSize>,
    rx_waker: &'a AtomicWaker,
}

impl<'a, W, R, IRQ> PeripheralState for SerialDriver<'a, W, R, IRQ>
where
    W: Write<u8>,
    R: Read<u8>,
    IRQ: Interrupt,
{
    type Interrupt = IRQ;

    fn on_interrupt(&mut self) {
        // Read as much data as we can
        match self.rx.grant_max_remaining(BUFFER_SIZE) {
            Ok(mut grant) => {
                let buf = grant.buf();
                let mut i = 0;
                while i < buf.len() {
                    match self.r.read() {
                        Ok(b) => {
                            buf[i] = b;
                            i += 1;
                        }
                        Err(_) => {
                            break;
                        }
                    }
                }
                grant.commit(i);
            }
            _ => {
                // Skipping
            }
        }

        // Write all buffered data
        match self.tx.read() {
            Ok(grant) => {
                let buf = grant.buf();
                for b in buf.iter() {
                    loop {
                        match self.w.write(*b) {
                            Err(nb::Error::WouldBlock) => {
                                let _ = nb::block!(self.w.flush());
                            }
                            Err(_) => return,
                            _ => break,
                        }
                    }
                }
                let _ = nb::block!(self.w.flush());
            }
            _ => {
                // Nothing to write
            }
        }
    }
}

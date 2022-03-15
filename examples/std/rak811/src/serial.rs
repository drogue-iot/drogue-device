use embedded_hal_async::serial::{Error, ErrorKind, ErrorType, Read, Write};
/// Copied from embassy project
use nix::fcntl::OFlag;
use nix::sys::termios;
use std::io;
use std::os::unix::io::{AsRawFd, RawFd};

pub struct SerialPort {
    fd: RawFd,
}

impl SerialPort {
    pub fn new<'a, P: ?Sized + nix::NixPath>(
        path: &P,
        baudrate: termios::BaudRate,
    ) -> io::Result<Self> {
        let fd = nix::fcntl::open(
            path,
            OFlag::O_RDWR | OFlag::O_NOCTTY | OFlag::O_NONBLOCK,
            nix::sys::stat::Mode::empty(),
        )
        .map_err(to_io_error)?;

        let mut cfg = termios::tcgetattr(fd).map_err(to_io_error)?;
        cfg.input_flags = termios::InputFlags::empty();
        cfg.output_flags = termios::OutputFlags::empty();
        cfg.control_flags = termios::ControlFlags::empty();
        cfg.local_flags = termios::LocalFlags::empty();
        termios::cfmakeraw(&mut cfg);
        cfg.input_flags |= termios::InputFlags::IGNBRK;
        cfg.control_flags |= termios::ControlFlags::CREAD;
        //cfg.control_flags |= termios::ControlFlags::CRTSCTS;
        termios::cfsetospeed(&mut cfg, baudrate).map_err(to_io_error)?;
        termios::cfsetispeed(&mut cfg, baudrate).map_err(to_io_error)?;
        termios::cfsetspeed(&mut cfg, baudrate).map_err(to_io_error)?;
        // Set VMIN = 1 to block until at least one character is received.
        cfg.control_chars[termios::SpecialCharacterIndices::VMIN as usize] = 1;
        termios::tcsetattr(fd, termios::SetArg::TCSANOW, &cfg).map_err(to_io_error)?;
        termios::tcflush(fd, termios::FlushArg::TCIOFLUSH).map_err(to_io_error)?;

        Ok(Self { fd })
    }

    pub fn split(self) -> (SerialWriter, SerialReader) {
        (SerialWriter { fd: self.fd }, SerialReader { fd: self.fd })
    }
}

pub struct SerialWriter {
    fd: RawFd,
}

pub struct SerialReader {
    fd: RawFd,
}

impl AsRawFd for SerialWriter {
    fn as_raw_fd(&self) -> RawFd {
        self.fd
    }
}

impl AsRawFd for SerialReader {
    fn as_raw_fd(&self) -> RawFd {
        self.fd
    }
}

impl ErrorType for SerialReader {
    type Error = SerialError;
}
impl ErrorType for SerialWriter {
    type Error = SerialError;
}

impl Error for SerialError {
    fn kind(&self) -> ErrorKind {
        ErrorKind::Other
    }
}

impl From<nix::Error> for SerialError {
    fn from(e: nix::Error) -> Self {
        Self(e)
    }
}

#[derive(Debug)]
pub struct SerialError(nix::Error);

impl Read for SerialReader {
    type ReadFuture<'m> = impl core::future::Future<Output = Result<(), Self::Error>> + 'm where Self: 'm;
    fn read<'m>(&'m mut self, buf: &'m mut [u8]) -> Self::ReadFuture<'m> {
        async move {
            nix::unistd::read(self.fd, buf)?;
            Ok(())
        }
    }
}

impl Write for SerialWriter {
    type WriteFuture<'m> = impl core::future::Future<Output = Result<(), Self::Error>> + 'm where Self: 'm;
    fn write<'m>(&'m mut self, buf: &'m [u8]) -> Self::WriteFuture<'m> {
        async move {
            nix::unistd::write(self.fd, buf)?;
            Ok(())
        }
    }

    type FlushFuture<'m> = impl core::future::Future<Output = Result<(), Self::Error>> + 'm where Self: 'm;
    fn flush<'m>(&'m mut self) -> Self::FlushFuture<'m> {
        async move { Ok(()) }
    }
}

fn to_io_error(e: nix::Error) -> io::Error {
    match e {
        nix::Error::Sys(errno) => errno.into(),
        e => io::Error::new(io::ErrorKind::InvalidInput, e),
    }
}

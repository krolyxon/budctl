//! Byte transports. Device protocols only see the [`Transport`] trait, so they can be tested
//! against a scripted mock instead of real buds.

use std::io;
use std::time::Instant;

use crate::debug;
use crate::error::{Error, Result};

pub mod rfcomm;
pub mod sdp;
mod socket;

#[cfg(test)]
pub(crate) mod mock;

/// A bidirectional byte stream to the buds.
pub trait Transport: Send {
    /// Send all of `bytes`.
    fn send(&mut self, bytes: &[u8]) -> Result<()>;

    /// The next bytes from the buds, or `None` once `deadline` passes with nothing to read.
    fn recv(&mut self, deadline: Instant) -> Result<Option<Vec<u8>>>;
}

/// Wraps a transport and prints every chunk sent and received (`--debug`).
pub struct Hexdump {
    inner: Box<dyn Transport>,
    start: Instant,
}

impl Hexdump {
    pub fn new(inner: Box<dyn Transport>) -> Self {
        Self {
            inner,
            start: Instant::now(),
        }
    }

    fn print(&self, direction: &str, bytes: &[u8]) {
        let elapsed = self.start.elapsed().as_secs_f64();
        eprintln!("{elapsed:>7.3} {direction} {}", debug::hex(bytes));
    }
}

impl Transport for Hexdump {
    fn send(&mut self, bytes: &[u8]) -> Result<()> {
        self.print("TX", bytes);
        self.inner.send(bytes)
    }

    fn recv(&mut self, deadline: Instant) -> Result<Option<Vec<u8>>> {
        let received = self.inner.recv(deadline)?;
        if let Some(bytes) = &received {
            self.print("RX", bytes);
        }
        Ok(received)
    }
}

/// What an OS error on a Bluetooth socket means for budctl.
/// `may_be_busy` is set only for the RFCOMM connect, where a refusal means another client
/// holds the channel. Spec: transport.md#errno
pub(crate) fn classify(error: io::Error, context: &'static str, may_be_busy: bool) -> Error {
    let detail = || format!("{context}: {error}");
    match error.raw_os_error() {
        Some(libc::EBUSY | libc::EADDRINUSE | libc::ECONNREFUSED) if may_be_busy => Error::ChannelBusy(detail()),
        Some(
            libc::EHOSTDOWN
            | libc::EHOSTUNREACH
            | libc::ENOTCONN
            | libc::ECONNRESET
            | libc::ECONNABORTED
            | libc::EPIPE
            | libc::ENODEV
            | libc::ENETDOWN,
        ) => Error::NotConnected(detail()),
        Some(libc::ETIMEDOUT | libc::EINPROGRESS | libc::EAGAIN) => Error::Timeout(detail()),
        _ => Error::Os { context, source: error },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::Kind;

    fn kind_of(errno: i32, may_be_busy: bool) -> Kind {
        classify(io::Error::from_raw_os_error(errno), "test", may_be_busy).kind()
    }

    #[test]
    fn errno_maps_to_the_documented_kinds() {
        assert_eq!(kind_of(libc::ECONNREFUSED, true), Kind::Busy);
        assert_eq!(kind_of(libc::EBUSY, true), Kind::Busy);
        assert_eq!(kind_of(libc::ECONNREFUSED, false), Kind::NotFound);
        assert_eq!(kind_of(libc::EHOSTDOWN, true), Kind::NotFound);
        assert_eq!(kind_of(libc::ECONNRESET, false), Kind::NotFound);
        assert_eq!(kind_of(libc::EINPROGRESS, true), Kind::Failed);
        assert_eq!(kind_of(libc::ETIMEDOUT, false), Kind::Failed);
        assert_eq!(kind_of(libc::EACCES, false), Kind::NotFound);
    }
}

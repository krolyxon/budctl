//! The RFCOMM control link: a byte stream to one channel on the buds.

use std::os::fd::OwnedFd;
use std::time::{Duration, Instant};

use super::{Transport, classify, socket};
use crate::bt::BdAddr;
use crate::error::{Error, Result};

/// Upper bound for the connect and for each send. The audio link is already up, so a healthy
/// connect takes well under a second.
const TIMEOUT: Duration = Duration::from_secs(8);

pub struct Rfcomm {
    fd: OwnedFd,
}

impl Rfcomm {
    pub fn connect(address: BdAddr, channel: u8) -> Result<Self> {
        let fd = socket::connect(address, socket::Target::Rfcomm { channel }, TIMEOUT)
            .map_err(|e| classify(e, "connecting to the control channel", true))?;
        Ok(Self { fd })
    }
}

impl Transport for Rfcomm {
    fn send(&mut self, bytes: &[u8]) -> Result<()> {
        socket::write_all(&self.fd, bytes).map_err(|e| classify(e, "sending to the buds", false))
    }

    fn recv(&mut self, deadline: Instant) -> Result<Option<Vec<u8>>> {
        let readable =
            socket::wait_readable(&self.fd, deadline).map_err(|e| classify(e, "waiting for the buds", false))?;
        if !readable {
            return Ok(None);
        }
        let mut buffer = vec![0u8; 1024];
        let n = socket::read(&self.fd, &mut buffer).map_err(|e| classify(e, "reading from the buds", false))?;
        if n == 0 {
            return Err(Error::NotConnected(
                "the buds closed the control link (case lid closed?)".to_owned(),
            ));
        }
        buffer.truncate(n);
        Ok(Some(buffer))
    }
}

//! Raw Linux Bluetooth sockets through libc. This is the only module in budctl allowed to use
//! `unsafe`; everything above it gets safe functions that return `io::Result`.
//! Constants and struct layouts: docs/protocol/transport.md#sockets.
#![allow(unsafe_code)]

use std::io;
use std::mem::size_of;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::time::{Duration, Instant};

use crate::bt::BdAddr;

/// Spec: transport.md#sockets
const BTPROTO_L2CAP: libc::c_int = 0;
/// Spec: transport.md#sockets
const BTPROTO_RFCOMM: libc::c_int = 3;

/// `struct sockaddr_rc`. Spec: transport.md#sockets
#[repr(C)]
struct SockaddrRc {
    family: libc::sa_family_t,
    bdaddr: [u8; 6],
    channel: u8,
}

/// `struct sockaddr_l2`. Spec: transport.md#sockets
#[repr(C)]
struct SockaddrL2 {
    family: libc::sa_family_t,
    psm: u16,
    bdaddr: [u8; 6],
    cid: u16,
    bdaddr_type: u8,
}

pub(crate) enum Target {
    /// A stream socket to an RFCOMM channel.
    Rfcomm { channel: u8 },
    /// A packet socket to an L2CAP PSM (SDP is PSM 1).
    L2cap { psm: u16 },
}

/// Open a Bluetooth socket and connect it. `timeout` bounds the connect and every later send.
pub(crate) fn connect(address: BdAddr, target: Target, timeout: Duration) -> io::Result<OwnedFd> {
    let (kind, protocol) = match target {
        Target::Rfcomm { .. } => (libc::SOCK_STREAM, BTPROTO_RFCOMM),
        Target::L2cap { .. } => (libc::SOCK_SEQPACKET, BTPROTO_L2CAP),
    };
    // SAFETY: a plain syscall with constant arguments; the result is checked below.
    let raw = unsafe { libc::socket(libc::AF_BLUETOOTH, kind | libc::SOCK_CLOEXEC, protocol) };
    if raw < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: `raw` is a fresh, valid descriptor that nothing else owns.
    let fd = unsafe { OwnedFd::from_raw_fd(raw) };

    // A blocking connect waits at most SO_SNDTIMEO, then fails with EINPROGRESS.
    // Spec: transport.md#connect-timeout
    set_timeout(&fd, libc::SO_SNDTIMEO, timeout)?;

    let family = libc::sa_family_t::try_from(libc::AF_BLUETOOTH).map_err(io::Error::other)?;
    let bdaddr = address.kernel_order();
    match target {
        Target::Rfcomm { channel } => connect_to(
            &fd,
            &SockaddrRc {
                family,
                bdaddr,
                channel,
            },
        )?,
        Target::L2cap { psm } => connect_to(
            &fd,
            &SockaddrL2 {
                family,
                psm: psm.to_le(),
                bdaddr,
                cid: 0,
                bdaddr_type: 0,
            },
        )?,
    }
    Ok(fd)
}

fn connect_to<T>(fd: &OwnedFd, address: &T) -> io::Result<()> {
    let len = libc::socklen_t::try_from(size_of::<T>()).map_err(io::Error::other)?;
    // SAFETY: `address` points to a live, initialised #[repr(C)] sockaddr of exactly `len` bytes.
    let rc = unsafe {
        libc::connect(
            fd.as_raw_fd(),
            std::ptr::from_ref(address).cast::<libc::sockaddr>(),
            len,
        )
    };
    if rc < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

fn set_timeout(fd: &OwnedFd, option: libc::c_int, timeout: Duration) -> io::Result<()> {
    let tv = libc::timeval {
        tv_sec: libc::time_t::try_from(timeout.as_secs()).map_err(io::Error::other)?,
        tv_usec: libc::suseconds_t::from(timeout.subsec_micros()),
    };
    let len = libc::socklen_t::try_from(size_of::<libc::timeval>()).map_err(io::Error::other)?;
    // SAFETY: `tv` lives for the duration of the call and `len` is its size.
    let rc = unsafe {
        libc::setsockopt(
            fd.as_raw_fd(),
            libc::SOL_SOCKET,
            option,
            std::ptr::from_ref(&tv).cast::<libc::c_void>(),
            len,
        )
    };
    if rc < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

/// Wait until the socket has something to read (data, end of stream or an error).
/// `Ok(false)` means `deadline` passed first.
pub(crate) fn wait_readable(fd: &OwnedFd, deadline: Instant) -> io::Result<bool> {
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        let ms = libc::c_int::try_from(left.as_millis()).unwrap_or(libc::c_int::MAX);
        let mut poll = libc::pollfd {
            fd: fd.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        // SAFETY: `poll` is one valid pollfd that lives for the duration of the call.
        let rc = unsafe { libc::poll(&mut poll, 1, ms) };
        if rc >= 0 {
            return Ok(rc > 0);
        }
        let error = io::Error::last_os_error();
        if error.kind() != io::ErrorKind::Interrupted {
            return Err(error);
        }
    }
}

/// One `recv()`. Returns 0 when the other side closed the connection.
pub(crate) fn read(fd: &OwnedFd, buffer: &mut [u8]) -> io::Result<usize> {
    loop {
        // SAFETY: `buffer` is valid for writes of `buffer.len()` bytes.
        let n = unsafe { libc::recv(fd.as_raw_fd(), buffer.as_mut_ptr().cast(), buffer.len(), 0) };
        if let Ok(n) = usize::try_from(n) {
            return Ok(n);
        }
        let error = io::Error::last_os_error();
        if error.kind() != io::ErrorKind::Interrupted {
            return Err(error);
        }
    }
}

/// Send every byte. `MSG_NOSIGNAL` turns a dead link into `EPIPE` instead of `SIGPIPE`.
pub(crate) fn write_all(fd: &OwnedFd, bytes: &[u8]) -> io::Result<()> {
    let mut rest = bytes;
    while !rest.is_empty() {
        // SAFETY: `rest` is valid for reads of `rest.len()` bytes.
        let n = unsafe { libc::send(fd.as_raw_fd(), rest.as_ptr().cast(), rest.len(), libc::MSG_NOSIGNAL) };
        match usize::try_from(n) {
            Ok(0) => return Err(io::ErrorKind::WriteZero.into()),
            Ok(sent) => rest = rest.get(sent..).unwrap_or_default(),
            Err(_) => {
                let error = io::Error::last_os_error();
                if error.kind() != io::ErrorKind::Interrupted {
                    return Err(error);
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sockaddr_layouts_match_the_kernel() {
        // Spec: transport.md#sockets
        assert_eq!(size_of::<SockaddrRc>(), 10);
        assert_eq!(size_of::<SockaddrL2>(), 14);
        assert_eq!(std::mem::offset_of!(SockaddrRc, channel), 8);
        assert_eq!(std::mem::offset_of!(SockaddrL2, cid), 10);
        assert_eq!(std::mem::offset_of!(SockaddrL2, bdaddr_type), 12);
    }
}

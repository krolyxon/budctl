//! Every way budctl can fail, as one typed error.
//!
//! Front ends decide what to do with each [`Kind`]; the CLI maps them to its exit codes.

use std::fmt;
use std::io;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    /// The request makes no sense for this device (bad value, several devices and no choice made…).
    Usage(String),
    /// The buds do not offer this feature.
    Unsupported(String),
    /// No paired device matches, or it is not a model budctl supports.
    NoDevice(String),
    /// The buds are paired but not connected, or the link dropped.
    NotConnected(String),
    /// The buds' SDP record has none of the model's control services.
    NoService(String),
    /// The buds identified themselves as a different model.
    WrongModel { expected: &'static str, found: String },
    /// BlueZ could not be reached over D-Bus.
    Bluez(String),
    /// Another client holds the control channel.
    ChannelBusy(String),
    /// The buds did not answer in time.
    Timeout(String),
    /// The buds answered with a non-zero status byte.
    Rejected { what: String, status: u8 },
    /// The buds sent something budctl cannot parse.
    Protocol(String),
    /// Any other operating-system error.
    Os { context: &'static str, source: io::Error },
}

/// How a front end should treat an error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// The user asked for something invalid. (CLI exit code 1)
    Usage,
    /// The device was not found or is not reachable. (CLI exit code 2)
    NotFound,
    /// Another app holds the control channel. (CLI exit code 3)
    Busy,
    /// The buds rejected the command, timed out or sent garbage. (CLI exit code 4)
    Failed,
}

impl Error {
    pub fn kind(&self) -> Kind {
        match self {
            Error::Usage(_) | Error::Unsupported(_) => Kind::Usage,
            Error::NoDevice(_)
            | Error::NotConnected(_)
            | Error::NoService(_)
            | Error::WrongModel { .. }
            | Error::Bluez(_)
            | Error::Os { .. } => Kind::NotFound,
            Error::ChannelBusy(_) => Kind::Busy,
            Error::Timeout(_) | Error::Rejected { .. } | Error::Protocol(_) => Kind::Failed,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Usage(msg) | Error::Unsupported(msg) | Error::NoDevice(msg) => f.write_str(msg),
            Error::NotConnected(msg) => f.write_str(msg),
            Error::NoService(address) => write!(
                f,
                "{address} offers no control service budctl knows (checked its SDP record)"
            ),
            Error::WrongModel { expected, found } => {
                write!(f, "these buds report product id {found}, which is not a {expected}")
            }
            Error::Bluez(msg) => write!(f, "cannot reach BlueZ over D-Bus: {msg} (is bluetoothd running?)"),
            Error::ChannelBusy(detail) => {
                write!(f, "control channel busy — is another app connected? ({detail})")
            }
            Error::Timeout(what) => write!(f, "no answer from the buds: {what}"),
            Error::Rejected { what, status } => {
                write!(f, "the buds rejected {what} (status 0x{status:02X})")
            }
            Error::Protocol(msg) => write!(f, "unexpected data from the buds: {msg}"),
            Error::Os { context, source } => write!(f, "{context}: {source}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Os { source, .. } => Some(source),
            _ => None,
        }
    }
}

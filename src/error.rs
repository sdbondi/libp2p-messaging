use futures_bounded::Timeout;
use std::fmt::{Debug, Display, Formatter};
use std::io;

#[derive(Debug)]
pub enum Error {
    DecodeError(io::Error),
    ConnectionClosed,
    Timeout(Timeout),
    DialFailure,
    DialUpgradeError,
    ProtocolNotSupported,
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DecodeError(err) => write!(f, "Decode error: {}", err),
            Self::ConnectionClosed => write!(f, "Connection closed"),
            Self::Timeout(err) => write!(f, "Timeout: {}", err),
            Self::DialFailure => write!(f, "Dial failure"),
            Self::DialUpgradeError => write!(f, "Dial upgrade error"),
            Self::ProtocolNotSupported => write!(f, "Protocol not supported"),
        }
    }
}

impl std::error::Error for Error {}

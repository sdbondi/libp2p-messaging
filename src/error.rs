use std::fmt::{Debug, Display, Formatter};
use std::io;

#[derive(Debug)]
pub enum Error {
    DecodeError(io::Error),
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DecodeError(err) => write!(f, "Decode error: {}", err),
        }
    }
}

impl std::error::Error for Error {}

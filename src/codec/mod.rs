#[cfg(feature = "prost")]
pub mod prost;

use libp2p::futures::{AsyncRead, AsyncWrite};
use std::{fmt, io};

/// A `Codec` defines the request and response types
/// for a request-response [`Behaviour`](crate::Behaviour) protocol or
/// protocol family and how they are encoded / decoded on an I/O stream.
#[async_trait::async_trait]
pub trait Codec: Default {
    /// The type of inbound and outbound message.
    type Message: fmt::Debug + Send;

    /// Reads a message from the given I/O stream according to the
    /// negotiated protocol.
    async fn decode_from<R>(&mut self, reader: &mut R) -> io::Result<Self::Message>
    where
        R: AsyncRead + Unpin + Send;

    /// Writes a request to the given I/O stream according to the
    /// negotiated protocol.
    async fn encode_to<W>(&mut self, writer: &mut W, message: Self::Message) -> io::Result<()>
    where
        W: AsyncWrite + Unpin + Send;
}

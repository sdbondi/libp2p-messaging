use crate::Message;
use libp2p::futures::{AsyncRead, AsyncWrite};
use std::io;

/// A `Codec` defines the request and response types
/// for a request-response [`Behaviour`](crate::Behaviour) protocol or
/// protocol family and how they are encoded / decoded on an I/O stream.
#[async_trait::async_trait]
pub trait Codec: Default {
    /// The type of protocol(s) or protocol versions being negotiated.
    type Protocol: AsRef<str> + Send + Clone;
    /// The type of inbound and outbound message.
    type Message: Send;

    /// Reads a message from the given I/O stream according to the
    /// negotiated protocol.
    async fn decode_from<R>(
        &mut self,
        protocol: &Self::Protocol,
        reader: &mut R,
    ) -> io::Result<Message<Self>>
    where
        R: AsyncRead + Unpin + Send;

    /// Writes a request to the given I/O stream according to the
    /// negotiated protocol.
    async fn encode_to<W>(
        &mut self,
        protocol: &Self::Protocol,
        writer: &mut W,
        message: Self::Message,
    ) -> io::Result<()>
    where
        W: AsyncWrite + Unpin + Send;
}

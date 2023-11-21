use crate::codec::Codec;
use crate::Message;
use async_trait::async_trait;
use libp2p::futures::{AsyncRead, AsyncReadExt, AsyncWrite};
use libp2p::StreamProtocol;
use std::fmt;
use std::marker::PhantomData;

const MAX_MESSAGE_SIZE: usize = 4 * 1024 * 1024;

pub type Behaviour<TMsg> = crate::Behaviour<BincodeSerdeCodec<TMsg>>;

pub type BincodeSerdeCodec<TMsg> = BincodeCodec<Compat<TMsg>>;

pub struct BincodeCodec<TMsg>(PhantomData<TMsg>);

impl<TMsg> Default for BincodeCodec<TMsg> {
    fn default() -> Self {
        Self(PhantomData)
    }
}

#[async_trait]
impl<TMsg> Codec for BincodeCodec<TMsg>
where
    TMsg: bincode::Decode + bincode::Encode + Send + 'static,
{
    type Protocol = StreamProtocol;
    type Message = TMsg;

    async fn decode_from<R>(
        &mut self,
        protocol: &Self::Protocol,
        reader: &mut R,
    ) -> std::io::Result<Message<Self>>
    where
        R: AsyncRead + Unpin + Send,
    {
        let mut len_buf = [0u8; 4];
        reader.read_exact(&mut len_buf).await?;
        let len = u32::from_be_bytes(len_buf) as usize;
        if len > MAX_MESSAGE_SIZE {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "message too large",
            ));
        }
        let mut buf = vec![0u8; len];
        reader.read_exact(&mut buf).await?;
        let (message, bytes_read) =
            bincode::decode_from_slice(&buf, bincode::config::standard())
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        if bytes_read != len {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "bytes remaining in buffer",
            ));
        }
        Ok(Message {
            message,
            protocol: protocol.clone(),
        })
    }

    async fn encode_to<W>(
        &mut self,
        protocol: &Self::Protocol,
        writer: &mut W,
        message: Self::Message,
    ) -> std::io::Result<()>
    where
        W: AsyncWrite + Unpin + Send,
    {
        todo!()
    }
}

impl<TMsg> Clone for BincodeCodec<TMsg> {
    fn clone(&self) -> Self {
        Self(PhantomData)
    }
}

impl<TMsg> fmt::Debug for BincodeCodec<TMsg> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BincodeCodec").finish()
    }
}

/// Wrapper struct that implements [Decode] and [Encode] on any type that implements serde's [DeserializeOwned] and [Serialize] respectively.
///
/// This works for most types, but if you're dealing with borrowed data consider using [BorrowCompat] instead.
/// Changes: This version implements Clone and Debug
///
/// [Decode]: ../de/trait.Decode.html
/// [Encode]: ../enc/trait.Encode.html
/// [DeserializeOwned]: https://docs.rs/serde/1/serde/de/trait.DeserializeOwned.html
/// [Serialize]: https://docs.rs/serde/1/serde/trait.Serialize.html
pub struct Compat<T>(pub T);

impl<T> bincode::Decode for Compat<T>
where
    T: serde::de::DeserializeOwned,
{
    fn decode<D: bincode::de::Decoder>(
        decoder: &mut D,
    ) -> Result<Self, bincode::error::DecodeError> {
        let serde_decoder = bincode::serde::de_owned::SerdeDecoder { de: decoder };
        T::deserialize(serde_decoder).map(Compat)
    }
}
impl<'de, T> bincode::BorrowDecode<'de> for Compat<T>
where
    T: serde::de::DeserializeOwned,
{
    fn borrow_decode<D: bincode::de::BorrowDecoder<'de>>(
        decoder: &mut D,
    ) -> Result<Self, bincode::error::DecodeError> {
        let serde_decoder = de_owned::SerdeDecoder { de: decoder };
        T::deserialize(serde_decoder).map(Compat)
    }
}

impl<T> bincode::Encode for Compat<T>
where
    T: serde::Serialize,
{
    fn encode<E: bincode::enc::Encoder>(
        &self,
        encoder: &mut E,
    ) -> Result<(), bincode::error::EncodeError> {
        let serializer = ser::SerdeEncoder { enc: encoder };
        self.0.serialize(serializer)?;
        Ok(())
    }
}

impl<T: Clone> Clone for Compat<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T> fmt::Debug for Compat<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Compat").finish()
    }
}

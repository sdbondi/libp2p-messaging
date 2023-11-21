use crate::codec::Codec;
use crate::error::Error;
use crate::Message;
use libp2p::bytes::Bytes;
use libp2p::PeerId;

#[derive(Debug)]
pub enum Event<TCodec: Codec> {
    ReceivedMessage {
        peer_id: PeerId,
        message: Message<TCodec>,
    },
    Error(Error),
}


use crate::error::Error;
use crate::MessageId;
use libp2p::PeerId;

#[derive(Debug)]
pub enum Event<TMsg> {
    ReceivedMessage {
        peer_id: PeerId,
        message: TMsg,
    },
    MessageSent {
        message_id: MessageId,
    },
    InboundFailure {
        peer_id: PeerId,
        message_id: MessageId,
        error: Error,
    },
    OutboundFailure {
        peer_id: PeerId,
        message_id: MessageId,
        error: Error,
    },
    Error(Error),
}

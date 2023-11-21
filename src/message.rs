use libp2p::PeerId;

pub type MessageId = u64;

#[derive(Debug, Clone)]
pub struct OutboundMessage<TMsg> {
    pub peer_id: PeerId,
    pub message: TMsg,
    pub message_id: MessageId,
}

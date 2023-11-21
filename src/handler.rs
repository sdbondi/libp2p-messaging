use crate::codec::Codec;
use crate::error::Error;
use crate::event::Event;
use crate::{Config, OutboundMessage, EMPTY_QUEUE_SHRINK_THRESHOLD};
use libp2p::core::UpgradeInfo;
use libp2p::futures::FutureExt;
use libp2p::swarm::handler::{
    ConnectionEvent, DialUpgradeError, FullyNegotiatedInbound, FullyNegotiatedOutbound,
    ListenUpgradeError,
};
use libp2p::swarm::{
    ConnectionHandler, ConnectionHandlerEvent, StreamUpgradeError, SubstreamProtocol,
};
use libp2p::{InboundUpgrade, OutboundUpgrade, PeerId, Stream, StreamProtocol};
use std::collections::VecDeque;
use std::convert::Infallible;
use std::future::{ready, Ready};
use std::task::{Context, Poll};

pub struct Handler<TCodec: Codec> {
    peer_id: PeerId,
    protocol: StreamProtocol,
    requested_outbound: VecDeque<OutboundMessage<TCodec::Message>>,
    pending_outbound: VecDeque<OutboundMessage<TCodec::Message>>,
    pending_events: VecDeque<Event<TCodec::Message>>,
    codec: TCodec,
    tasks: futures_bounded::FuturesSet<Event<TCodec::Message>>,
}

impl<TCodec: Codec> Handler<TCodec> {
    pub fn new(peer_id: PeerId, protocol: StreamProtocol, config: &Config) -> Self {
        Self {
            peer_id,
            protocol,
            requested_outbound: VecDeque::new(),
            pending_outbound: VecDeque::new(),
            pending_events: VecDeque::new(),
            codec: TCodec::default(),
            tasks: futures_bounded::FuturesSet::new(
                config.send_recv_timeout,
                config.max_concurrent_streams,
            ),
        }
    }
}

impl<TCodec> Handler<TCodec>
where
    TCodec: Codec + Send + Clone + 'static,
{
    fn on_listen_upgrade_error(&self, error: ListenUpgradeError<(), Protocol<StreamProtocol>>) {
        tracing::warn!("unexpected listen upgrade error: {:?}", error.error);
    }

    fn on_dial_upgrade_error(&mut self, error: DialUpgradeError<(), Protocol<StreamProtocol>>) {
        let message = self
            .requested_outbound
            .pop_front()
            .expect("negotiated a stream without a pending message");

        match error.error {
            StreamUpgradeError::Timeout => {
                self.pending_events.push_back(Event::OutboundFailure {
                    peer_id: self.peer_id,
                    message_id: message.message_id,
                    error: Error::DialUpgradeError,
                });
            }
            StreamUpgradeError::NegotiationFailed => {
                // The remote merely doesn't support the protocol(s) we requested.
                // This is no reason to close the connection, which may
                // successfully communicate with other protocols already.
                // An event is reported to permit user code to react to the fact that
                // the remote peer does not support the requested protocol(s).
                self.pending_events.push_back(Event::OutboundFailure {
                    peer_id: self.peer_id,
                    message_id: message.message_id,
                    error: Error::ProtocolNotSupported,
                });
            }
            StreamUpgradeError::Apply(_) => {}
            StreamUpgradeError::Io(e) => {
                tracing::debug!(
                    "outbound stream for request {} failed: {e}, retrying",
                    message.message_id
                );
                self.requested_outbound.push_back(message);
            }
        }
    }

    fn on_fully_negotiated_outbound(
        &mut self,
        outbound: FullyNegotiatedOutbound<Protocol<StreamProtocol>, ()>,
    ) {
        let mut codec = self.codec.clone();
        let (mut stream, _protocol) = outbound.protocol;

        let message = self
            .requested_outbound
            .pop_front()
            .expect("negotiated a stream without a pending message");

        let fut = async move {
            match codec.encode_to(&mut stream, message.message).await {
                Ok(_) => Event::MessageSent {
                    message_id: message.message_id,
                },
                Err(e) => Event::Error(Error::DecodeError(e)),
            }
        }
        .boxed();

        if self.tasks.try_push(fut).is_err() {
            tracing::warn!("Dropping outbound stream because we are at capacity")
        }
    }

    fn on_fully_negotiated_inbound(
        &mut self,
        inbound: FullyNegotiatedInbound<Protocol<StreamProtocol>, ()>,
    ) {
        let mut codec = self.codec.clone();
        let peer_id = self.peer_id;
        let (mut stream, _protocol) = inbound.protocol;

        let fut = async move {
            match codec.decode_from(&mut stream).await {
                Ok(message) => Event::ReceivedMessage { peer_id, message },
                Err(e) => Event::Error(Error::DecodeError(e)),
            }
        }
        .boxed();

        if self.tasks.try_push(fut).is_err() {
            tracing::warn!("Dropping inbound stream because we are at capacity")
        }
    }
}

impl<TCodec> ConnectionHandler for Handler<TCodec>
where
    TCodec: Codec + Send + Clone + 'static,
{
    type FromBehaviour = OutboundMessage<TCodec::Message>;
    type ToBehaviour = Event<TCodec::Message>;
    type InboundProtocol = Protocol<StreamProtocol>;
    type OutboundProtocol = Protocol<StreamProtocol>;
    type InboundOpenInfo = ();
    type OutboundOpenInfo = ();

    fn listen_protocol(&self) -> SubstreamProtocol<Self::InboundProtocol, Self::InboundOpenInfo> {
        SubstreamProtocol::new(
            Protocol {
                protocol: self.protocol.clone(),
            },
            (),
        )
    }

    fn poll(
        &mut self,
        cx: &mut Context<'_>,
    ) -> Poll<
        ConnectionHandlerEvent<Self::OutboundProtocol, Self::OutboundOpenInfo, Self::ToBehaviour>,
    > {
        match self.tasks.poll_unpin(cx) {
            Poll::Ready(Ok(event)) => {
                return Poll::Ready(ConnectionHandlerEvent::NotifyBehaviour(event));
            }
            Poll::Ready(Err(err)) => {
                return Poll::Ready(ConnectionHandlerEvent::NotifyBehaviour(Event::Error(
                    Error::Timeout(err),
                )));
            }
            Poll::Pending => {}
        }

        // Drain pending events that were produced by `worker_streams`.
        if let Some(event) = self.pending_events.pop_front() {
            return Poll::Ready(ConnectionHandlerEvent::NotifyBehaviour(event));
        } else if self.pending_events.capacity() > EMPTY_QUEUE_SHRINK_THRESHOLD {
            self.pending_events.shrink_to_fit();
        }

        // // Check for inbound requests.
        // if let Poll::Ready(Some((id, rq, rs_sender))) = self.inbound_receiver.poll_next_unpin(cx) {
        //     // We received an inbound request.
        //
        //     return Poll::Ready(ConnectionHandlerEvent::NotifyBehaviour(Event::Request {
        //         request_id: id,
        //         request: rq,
        //         sender: rs_sender,
        //     }));
        // }

        // Emit outbound requests.
        if let Some(message) = self.pending_outbound.pop_front() {
            let protocol = self.protocol.clone();
            self.requested_outbound.push_back(message);

            return Poll::Ready(ConnectionHandlerEvent::OutboundSubstreamRequest {
                protocol: SubstreamProtocol::new(Protocol { protocol }, ()),
            });
        }

        debug_assert!(self.pending_outbound.is_empty());

        if self.pending_outbound.capacity() > EMPTY_QUEUE_SHRINK_THRESHOLD {
            self.pending_outbound.shrink_to_fit();
        }

        Poll::Pending
    }

    fn on_behaviour_event(&mut self, msg: Self::FromBehaviour) {
        self.pending_outbound.push_back(msg);
    }

    fn on_connection_event(
        &mut self,
        event: ConnectionEvent<
            Self::InboundProtocol,
            Self::OutboundProtocol,
            Self::InboundOpenInfo,
            Self::OutboundOpenInfo,
        >,
    ) {
        match event {
            ConnectionEvent::FullyNegotiatedInbound(fully_negotiated_inbound) => {
                self.on_fully_negotiated_inbound(fully_negotiated_inbound)
            }
            ConnectionEvent::FullyNegotiatedOutbound(fully_negotiated_outbound) => {
                self.on_fully_negotiated_outbound(fully_negotiated_outbound)
            }
            ConnectionEvent::DialUpgradeError(dial_upgrade_error) => {
                self.on_dial_upgrade_error(dial_upgrade_error)
            }
            ConnectionEvent::ListenUpgradeError(listen_upgrade_error) => {
                self.on_listen_upgrade_error(listen_upgrade_error)
            }
            _ => {}
        }
    }
}

pub struct Protocol<P> {
    pub(crate) protocol: P,
}

impl<P> UpgradeInfo for Protocol<P>
where
    P: AsRef<str> + Clone,
{
    type Info = P;
    type InfoIter = std::option::IntoIter<Self::Info>;

    fn protocol_info(&self) -> Self::InfoIter {
        Some(self.protocol.clone()).into_iter()
    }
}

impl<P> InboundUpgrade<Stream> for Protocol<P>
where
    P: AsRef<str> + Clone,
{
    type Output = (Stream, P);
    type Error = Infallible;
    type Future = Ready<Result<Self::Output, Self::Error>>;

    fn upgrade_inbound(self, io: Stream, protocol: Self::Info) -> Self::Future {
        ready(Ok((io, protocol)))
    }
}

impl<P> OutboundUpgrade<Stream> for Protocol<P>
where
    P: AsRef<str> + Clone,
{
    type Output = (Stream, P);
    type Error = Infallible;
    type Future = Ready<Result<Self::Output, Self::Error>>;

    fn upgrade_outbound(self, io: Stream, protocol: Self::Info) -> Self::Future {
        ready(Ok((io, protocol)))
    }
}

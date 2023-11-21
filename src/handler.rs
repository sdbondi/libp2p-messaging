use crate::codec::Codec;
use crate::error::Error;
use crate::event::Event;
use crate::message::Message;
use libp2p::bytes::Bytes;
use libp2p::core::{Endpoint, UpgradeInfo};
use libp2p::futures::future::BoxFuture;
use libp2p::futures::stream::FuturesUnordered;
use libp2p::futures::FutureExt;
use libp2p::swarm::handler::{
    ConnectionEvent, DialUpgradeError, FullyNegotiatedInbound, FullyNegotiatedOutbound,
    ListenUpgradeError,
};
use libp2p::swarm::{ConnectionHandler, ConnectionHandlerEvent, SubstreamProtocol};
use libp2p::{InboundUpgrade, OutboundUpgrade, PeerId, Stream};
use std::collections::VecDeque;
use std::convert::Infallible;
use std::fmt;
use std::future::{ready, Ready};
use std::task::{Context, Poll};

pub struct Handler<TCodec: Codec> {
    peer_id: PeerId,
    endpoint: Endpoint,
    protocol: TCodec::Protocol,
    pending_outbound: VecDeque<Message<TCodec>>,
    codec: TCodec,
    workers: FuturesUnordered<BoxFuture<'static, Event<TCodec>>>,
}

impl<TCodec: Codec> Handler<TCodec> {
    pub fn new(peer_id: PeerId, endpoint: Endpoint, protocol: TCodec::Protocol) -> Self {
        Self {
            peer_id,
            endpoint,
            protocol,
            pending_outbound: VecDeque::new(),
            codec: TCodec::default(),
            workers: FuturesUnordered::new(),
        }
    }
}

impl<TCodec> Handler<TCodec>
where
    TCodec: Codec + Send + Clone + 'static,
{
    fn on_listen_upgrade_error(&self, _error: ListenUpgradeError<(), Protocol<TCodec::Protocol>>) {}

    fn on_dial_upgrade_error(&self, _error: DialUpgradeError<(), Protocol<TCodec::Protocol>>) {}

    fn on_fully_negotiated_outbound(
        &self,
        outbound: FullyNegotiatedOutbound<Protocol<TCodec::Protocol>, ()>,
    ) {
        let mut codec = self.codec.clone();
        let peer_id = self.peer_id;
        let (mut stream, protocol) = outbound.protocol;

        // if protocol != self.protocol {
        //     return;
        // }

        let fut = async move {
            match codec.decode_from(&protocol, &mut stream).await {
                Ok(message) => Event::ReceivedMessage { peer_id, message },
                Err(e) => Event::Error(Error::DecodeError(e)),
            }
        }
        .boxed();

        self.workers.push(fut);
    }

    fn on_fully_negotiated_inbound(
        &self,
        inbound: FullyNegotiatedInbound<Protocol<TCodec::Protocol>, ()>,
    ) {
        todo!()
    }
}

impl<TCodec> ConnectionHandler for Handler<TCodec>
where
    TCodec: Codec + Send + Clone + 'static,
{
    type FromBehaviour = Message<TCodec>;
    type ToBehaviour = Bytes;
    type InboundProtocol = Protocol<TCodec::Protocol>;
    type OutboundProtocol = Protocol<TCodec::Protocol>;
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
        todo!()
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

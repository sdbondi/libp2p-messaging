use crate::codec::Codec;
use crate::event::Event;
use crate::handler::Handler;
use libp2p::core::Endpoint;
use libp2p::swarm::{
    ConnectionDenied, ConnectionId, FromSwarm, NetworkBehaviour, THandler, THandlerInEvent,
    THandlerOutEvent, ToSwarm,
};
use libp2p::{Multiaddr, PeerId};
use std::fmt;
use std::task::{Context, Poll};

#[derive(Debug)]
pub struct Behaviour<TCodec: Codec> {
    protocol: TCodec::Protocol,
}

impl<TCodec: Codec> Behaviour<TCodec> {
    pub fn new(protocol: TCodec::Protocol) -> Self {
        Self { protocol }
    }
}

impl<TCodec> NetworkBehaviour for Behaviour<TCodec>
where
    TCodec: Codec + Send + Clone + 'static,
    TCodec::Message: fmt::Debug,
{
    type ConnectionHandler = Handler<TCodec>;
    type ToSwarm = Event<TCodec>;

    fn handle_established_inbound_connection(
        &mut self,
        _connection_id: ConnectionId,
        peer: PeerId,
        _local_addr: &Multiaddr,
        _remote_addr: &Multiaddr,
    ) -> Result<THandler<Self>, ConnectionDenied> {
        Ok(Handler::<TCodec>::new(
            peer,
            Endpoint::Listener,
            self.protocol.clone(),
        ))
    }

    fn handle_established_outbound_connection(
        &mut self,
        _connection_id: ConnectionId,
        peer: PeerId,
        _addr: &Multiaddr,
        role_override: Endpoint,
    ) -> Result<THandler<Self>, ConnectionDenied> {
        Ok(Handler::new(peer, role_override, self.protocol.clone()))
    }

    fn on_swarm_event(&mut self, event: FromSwarm) {
        todo!()
    }

    fn on_connection_handler_event(
        &mut self,
        _peer_id: PeerId,
        _connection_id: ConnectionId,
        _event: THandlerOutEvent<Self>,
    ) {
    }

    fn poll(
        &mut self,
        cx: &mut Context<'_>,
    ) -> Poll<ToSwarm<Self::ToSwarm, THandlerInEvent<Self>>> {
        todo!()
    }
}

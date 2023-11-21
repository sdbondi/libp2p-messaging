use crate::codec::Codec;
use crate::error::Error;
use crate::event::Event;
use crate::handler::Handler;
use crate::{Config, MessageId, OutboundMessage};
use libp2p::core::Endpoint;
use libp2p::swarm::dial_opts::DialOpts;
use libp2p::swarm::{
    AddressChange, ConnectionClosed, ConnectionDenied, ConnectionHandler, ConnectionId,
    DialFailure, FromSwarm, NetworkBehaviour, NotifyHandler, THandler, THandlerInEvent,
    THandlerOutEvent, ToSwarm,
};
use libp2p::{Multiaddr, PeerId, StreamProtocol};
use smallvec::SmallVec;
use std::collections::{HashMap, HashSet, VecDeque};
use std::task::{Context, Poll};

/// Internal threshold for when to shrink the capacity
/// of empty queues. If the capacity of an empty queue
/// exceeds this threshold, the associated memory is
/// released.
pub const EMPTY_QUEUE_SHRINK_THRESHOLD: usize = 100;

#[derive(Debug)]
pub struct Behaviour<TCodec>
where
    TCodec: Codec + Send + Clone + 'static,
{
    protocol: StreamProtocol,
    config: Config,
    pending_events: VecDeque<ToSwarm<Event<TCodec::Message>, THandlerInEvent<Self>>>,
    pending_outbound_messages: HashMap<PeerId, SmallVec<OutboundMessage<TCodec::Message>, 10>>,
    /// The currently connected peers, their pending outbound and inbound responses and their known,
    /// reachable addresses, if any.
    connected: HashMap<PeerId, SmallVec<Connection, 2>>,
    next_outbound_message_id: MessageId,
}

impl<TCodec> Behaviour<TCodec>
where
    TCodec: Codec + Send + Clone + 'static,
{
    pub fn new(protocol: StreamProtocol, config: Config) -> Self {
        Self {
            protocol,
            config,
            pending_events: VecDeque::new(),
            pending_outbound_messages: HashMap::new(),
            connected: HashMap::new(),
            next_outbound_message_id: 0,
        }
    }

    pub fn send_message(&mut self, peer_id: PeerId, message: TCodec::Message) {
        let message_id = self.next_outbound_message_id();
        let message = OutboundMessage {
            peer_id,
            message_id,
            message,
        };

        if let Some(message) = self.try_send_request(message) {
            self.pending_events.push_back(ToSwarm::Dial {
                opts: DialOpts::peer_id(peer_id).build(),
            });
            self.pending_outbound_messages
                .entry(peer_id)
                .or_default()
                .push(message);
        }
    }

    fn next_outbound_message_id(&mut self) -> MessageId {
        let request_id = self.next_outbound_message_id;
        self.next_outbound_message_id = self.next_outbound_message_id.wrapping_add(1);
        request_id
    }

    fn try_send_request(
        &mut self,
        message: OutboundMessage<TCodec::Message>,
    ) -> Option<OutboundMessage<TCodec::Message>> {
        if let Some(connections) = self.connected.get_mut(&message.peer_id) {
            if connections.is_empty() {
                return Some(message);
            }
            let ix = (message.message_id as usize) % connections.len();
            let conn = &mut connections[ix];
            conn.pending_messages.insert(message.message_id);
            self.pending_events.push_back(ToSwarm::NotifyHandler {
                peer_id: message.peer_id,
                handler: NotifyHandler::One(conn.id),
                event: message,
            });
            None
        } else {
            Some(message)
        }
    }

    fn on_connection_closed(
        &mut self,
        ConnectionClosed {
            peer_id,
            connection_id,
            remaining_established,
            ..
        }: ConnectionClosed,
    ) {
        let connections = self
            .connected
            .get_mut(&peer_id)
            .expect("Expected some established connection to peer before closing.");

        let connection = connections
            .iter()
            .position(|c| c.id == connection_id)
            .map(|p: usize| connections.remove(p))
            .expect("Expected connection to be established before closing.");

        debug_assert_eq!(connections.is_empty(), remaining_established == 0);
        if connections.is_empty() {
            self.connected.remove(&peer_id);
        }

        for message_id in connection.pending_messages {
            self.pending_events
                .push_back(ToSwarm::GenerateEvent(Event::InboundFailure {
                    peer_id,
                    message_id,
                    error: Error::ConnectionClosed,
                }));
        }
    }

    fn on_address_change(&mut self, address_change: AddressChange) {
        let AddressChange {
            peer_id,
            connection_id,
            new,
            ..
        } = address_change;
        if let Some(connections) = self.connected.get_mut(&peer_id) {
            for connection in connections {
                if connection.id == connection_id {
                    connection.remote_address = Some(new.get_remote_address().clone());
                    return;
                }
            }
        }
    }

    fn on_dial_failure(&mut self, DialFailure { peer_id, .. }: DialFailure) {
        if let Some(peer) = peer_id {
            // If there are pending outgoing messages when a dial failure occurs,
            // it is implied that we are not connected to the peer, since pending
            // outgoing messages are drained when a connection is established and
            // only created when a peer is not connected when a request is made.
            // Thus these requests must be considered failed, even if there is
            // another, concurrent dialing attempt ongoing.
            if let Some(pending) = self.pending_outbound_messages.remove(&peer) {
                for request in pending {
                    self.pending_events
                        .push_back(ToSwarm::GenerateEvent(Event::OutboundFailure {
                            peer_id: peer,
                            message_id: request.message_id,
                            error: Error::DialFailure,
                        }));
                }
            }
        }
    }

    fn on_connection_established(
        &mut self,
        handler: &mut Handler<TCodec>,
        peer_id: PeerId,
        connection_id: ConnectionId,
        remote_address: Option<Multiaddr>,
    ) {
        let mut connection = Connection::new(connection_id, remote_address);

        if let Some(pending_messages) = self.pending_outbound_messages.remove(&peer_id) {
            for message in pending_messages {
                connection.pending_messages.insert(message.message_id);
                handler.on_behaviour_event(message);
            }
        }

        self.connected.entry(peer_id).or_default().push(connection);
    }
}

impl<TCodec> NetworkBehaviour for Behaviour<TCodec>
where
    TCodec: Codec + Send + Clone + 'static,
{
    type ConnectionHandler = Handler<TCodec>;
    type ToSwarm = Event<TCodec::Message>;

    fn handle_established_inbound_connection(
        &mut self,
        connection_id: ConnectionId,
        peer: PeerId,
        _local_addr: &Multiaddr,
        remote_addr: &Multiaddr,
    ) -> Result<THandler<Self>, ConnectionDenied> {
        let mut handler = Handler::<TCodec>::new(peer, self.protocol.clone(), &self.config);
        self.on_connection_established(
            &mut handler,
            peer,
            connection_id,
            Some(remote_addr.clone()),
        );

        Ok(handler)
    }

    fn handle_established_outbound_connection(
        &mut self,
        connection_id: ConnectionId,
        peer: PeerId,
        remote_addr: &Multiaddr,
        _role_override: Endpoint,
    ) -> Result<THandler<Self>, ConnectionDenied> {
        let mut handler = Handler::new(peer, self.protocol.clone(), &self.config);
        self.on_connection_established(
            &mut handler,
            peer,
            connection_id,
            Some(remote_addr.clone()),
        );
        Ok(handler)
    }

    fn on_swarm_event(&mut self, event: FromSwarm) {
        match event {
            FromSwarm::ConnectionEstablished(_) => {}
            FromSwarm::ConnectionClosed(connection_closed) => {
                self.on_connection_closed(connection_closed)
            }
            FromSwarm::AddressChange(address_change) => self.on_address_change(address_change),
            FromSwarm::DialFailure(dial_failure) => self.on_dial_failure(dial_failure),
            _ => {}
        }
    }
    fn on_connection_handler_event(
        &mut self,
        _peer_id: PeerId,
        _connection_id: ConnectionId,
        event: THandlerOutEvent<Self>,
    ) {
        self.pending_events.push_back(ToSwarm::GenerateEvent(event));
    }

    fn poll(
        &mut self,
        _cx: &mut Context<'_>,
    ) -> Poll<ToSwarm<Self::ToSwarm, THandlerInEvent<Self>>> {
        if let Some(event) = self.pending_events.pop_front() {
            return Poll::Ready(event);
        } else if self.pending_events.capacity() > EMPTY_QUEUE_SHRINK_THRESHOLD {
            self.pending_events.shrink_to_fit();
        }

        Poll::Pending
    }
}

/// Internal information tracked for an established connection.
#[derive(Debug)]
struct Connection {
    id: ConnectionId,
    remote_address: Option<Multiaddr>,
    pending_messages: HashSet<MessageId>,
}

impl Connection {
    fn new(id: ConnectionId, remote_address: Option<Multiaddr>) -> Self {
        Self {
            id,
            remote_address,
            pending_messages: HashSet::new(),
        }
    }
}

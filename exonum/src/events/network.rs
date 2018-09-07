// Copyright 2018 The Exonum Team
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//   http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use failure;
use futures::{
    future, future::{err, Either}, stream::SplitStream, sync::mpsc, unsync, Future, IntoFuture,
    Poll, Sink, Stream,
};
use tokio_codec::Framed;
use tokio_core::{
    net::{TcpListener, TcpStream}, reactor::Handle,
};
use tokio_retry::{
    strategy::{jitter, FixedInterval}, Retry,
};

use std::{cell::RefCell, collections::HashMap, net::SocketAddr, rc::Rc, time::Duration};

use super::{
    error::{log_error, result_ok}, to_box,
};
use events::{
    codec::MessagesCodec, error::into_failure, noise::{Handshake, HandshakeParams, NoiseHandshake},
};
use helpers::Milliseconds;
use messages::{Any, Connect, Message, RawMessage};

const OUTGOING_CHANNEL_SIZE: usize = 10;

#[derive(Debug)]
pub enum NetworkEvent {
    MessageReceived(SocketAddr, RawMessage),
    PeerConnected(SocketAddr, Connect),
    PeerDisconnected(SocketAddr),
    UnableConnectToPeer(SocketAddr),
}

#[derive(Debug, Clone)]
pub enum NetworkRequest {
    SendMessage(SocketAddr, RawMessage),
    DisconnectWithPeer(SocketAddr),
    Shutdown,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub struct NetworkConfiguration {
    // TODO: Think more about config parameters. (ECR-162)
    pub max_incoming_connections: usize,
    pub max_outgoing_connections: usize,
    pub tcp_nodelay: bool,
    pub tcp_keep_alive: Option<u64>,
    pub tcp_connect_retry_timeout: Milliseconds,
    pub tcp_connect_max_retries: u64,
}

impl Default for NetworkConfiguration {
    fn default() -> Self {
        Self {
            max_incoming_connections: 128,
            max_outgoing_connections: 128,
            tcp_keep_alive: None,
            tcp_nodelay: true,
            tcp_connect_retry_timeout: 15_000,
            tcp_connect_max_retries: 10,
        }
    }
}

#[derive(Debug)]
pub struct NetworkPart {
    pub our_connect_message: Connect,
    pub listen_address: SocketAddr,
    pub network_config: NetworkConfiguration,
    pub max_message_len: u32,
    pub network_requests: (mpsc::Sender<NetworkRequest>, mpsc::Receiver<NetworkRequest>),
    pub network_tx: mpsc::Sender<NetworkEvent>,
}

#[derive(Debug, Default, Clone)]
struct ConnectionsPool {
    inner: Rc<RefCell<HashMap<SocketAddr, mpsc::Sender<RawMessage>>>>,
}

impl ConnectionsPool {
    fn new() -> Self {
        Self::default()
    }

    fn insert(&self, peer: SocketAddr, sender: &mpsc::Sender<RawMessage>) {
        self.inner.borrow_mut().insert(peer, sender.clone());
    }

    fn remove(&self, peer: &SocketAddr) -> Result<mpsc::Sender<RawMessage>, failure::Error> {
        self.inner
            .borrow_mut()
            .remove(peer)
            .ok_or_else(|| format_err!("there is no sender in the connection pool"))
    }

    fn get(&self, peer: SocketAddr) -> Option<mpsc::Sender<RawMessage>> {
        self.inner.borrow_mut().get(&peer).cloned()
    }

    fn len(&self) -> usize {
        self.inner.borrow_mut().len()
    }

    fn connect_to_peer(
        self,
        network_config: NetworkConfiguration,
        peer: SocketAddr,
        network_tx: mpsc::Sender<NetworkEvent>,
        handle: &Handle,
        handshake_params: &HandshakeParams,
    ) -> Option<mpsc::Sender<RawMessage>> {
        let limit = network_config.max_outgoing_connections;
        if self.len() >= limit {
            warn!(
                "Rejected outgoing connection with peer={}, \
                 connections limit reached.",
                peer
            );
            return None;
        }
        // Register outgoing channel.
        let (conn_tx, conn_rx) = mpsc::channel(OUTGOING_CHANNEL_SIZE);
        self.insert(peer, &conn_tx);
        // Enable retry feature for outgoing connection.
        let timeout = network_config.tcp_connect_retry_timeout;
        let max_tries = network_config.tcp_connect_max_retries as usize;
        let strategy = FixedInterval::from_millis(timeout)
            .map(jitter)
            .take(max_tries);
        let handle_cloned = handle.clone();
        let handshake_params = handshake_params.clone();

        let action = move || TcpStream::connect(&peer, &handle_cloned);
        let connect_handle = Retry::spawn(strategy, action)
            .map_err(into_failure)
            .and_then(move |socket| Self::configure_socket(socket, network_config))
            .and_then(move |socket| {
                Self::build_handshake_initiator(socket, &peer, &handshake_params)
            })
            .and_then(move |stream| {
                trace!("Established connection with peer={}", peer);
                Self::process_outgoing_messages(stream, conn_rx)
            })
            .then(move |res| {
                trace!("Disconnection with peer={}, reason={:?}", peer, res);
                self.disconnect_with_peer(peer, network_tx.clone())
            })
            .map_err(log_error);
        handle.spawn(connect_handle);
        Some(conn_tx)
    }

    fn configure_socket(
        socket: TcpStream,
        network_config: NetworkConfiguration,
    ) -> Result<TcpStream, failure::Error> {
        socket.set_nodelay(network_config.tcp_nodelay)?;
        let duration = network_config.tcp_keep_alive.map(Duration::from_millis);
        socket.set_keepalive(duration)?;
        Ok(socket)
    }

    fn disconnect_with_peer(
        &self,
        peer: SocketAddr,
        network_tx: mpsc::Sender<NetworkEvent>,
    ) -> Box<dyn Future<Item = (), Error = failure::Error>> {
        let fut = self.remove(&peer)
            .into_future()
            .and_then(move |_| {
                network_tx
                    .send(NetworkEvent::PeerDisconnected(peer))
                    .map_err(|_| format_err!("can't send disconnect"))
            })
            .map(drop);
        to_box(fut)
    }

    fn build_handshake_initiator(
        stream: TcpStream,
        peer: &SocketAddr,
        handshake_params: &HandshakeParams,
    ) -> impl Future<Item = Framed<TcpStream, MessagesCodec>, Error = failure::Error> {
        let connect_list = &handshake_params.connect_list.clone();
        if let Some(remote_public_key) = connect_list.find_key_by_address(&peer) {
            let mut handshake_params = handshake_params.clone();
            handshake_params.set_remote_key(remote_public_key);
            NoiseHandshake::initiator(&handshake_params, peer).send(stream)
        } else {
            Box::new(err(format_err!(
                "Attempt to connect to the peer with address {:?} which \
                 is not in the ConnectList",
                peer
            )))
        }
    }

    // Connect socket with the outgoing channel
    fn process_outgoing_messages(
        stream: Framed<TcpStream, MessagesCodec>,
        conn_rx: mpsc::Receiver<RawMessage>,
    ) -> impl Future<Item = &'static str, Error = failure::Error> {
        let (sink, stream) = stream.split();

        let writer = conn_rx
            .map_err(|_| format_err!("Can't send data into socket"))
            .forward(sink);
        let reader = stream.for_each(result_ok);

        reader
            .select2(writer)
            .map_err(|_| format_err!("Socket error"))
            .and_then(|res| match res {
                Either::A((_, _reader)) => Ok("by reader"),
                Either::B((_, _writer)) => Ok("by writer"),
            })
    }
}

impl NetworkPart {
    pub fn run(
        self,
        handle: &Handle,
        handshake_params: &HandshakeParams,
    ) -> Box<dyn Future<Item = (), Error = failure::Error>> {
        let network_config = self.network_config;
        // Cancellation token
        let (cancel_sender, cancel_handler) = unsync::oneshot::channel();

        let request_handler = RequestHandler::from(
            self.our_connect_message,
            network_config,
            self.network_tx.clone(),
            handle.clone(),
            handshake_params.clone(),
        ).into_handler(self.network_requests.1, cancel_sender);

        // TODO Don't use unwrap here! (ECR-1633)
        let server = Listener::bind(
            network_config,
            self.listen_address,
            handle.clone(),
            &self.network_tx,
            handshake_params,
        ).unwrap();

        let cancel_handler = cancel_handler.or_else(|e| {
            trace!("Requests handler closed: {}", e);
            Ok(())
        });
        let fut = server
            .join(request_handler)
            .map(drop)
            .select(cancel_handler)
            .map_err(|(e, _)| e);
        to_box(fut)
    }
}

struct RequestHandler {
    connect_message: Connect,
    network_config: NetworkConfiguration,
    network_tx: mpsc::Sender<NetworkEvent>,
    handle: Handle,
    handshake_params: HandshakeParams,
    outgoing_connections: ConnectionsPool,
}

impl RequestHandler {
    fn from(
        connect_message: Connect,
        network_config: NetworkConfiguration,
        network_tx: mpsc::Sender<NetworkEvent>,
        handle: Handle,
        handshake_params: HandshakeParams,
    ) -> Self {
        RequestHandler {
            connect_message,
            network_config,
            network_tx,
            handle,
            handshake_params,
            outgoing_connections: ConnectionsPool::new(),
        }
    }

    fn into_handler(
        self,
        receiver: mpsc::Receiver<NetworkRequest>,
        cancel_sender: unsync::oneshot::Sender<()>,
    ) -> impl Future<Item = (), Error = failure::Error> {
        let mut cancel_sender = Some(cancel_sender);
        receiver
            .map_err(|_| format_err!("no network requests"))
            .for_each(move |request| {
                match request {
                    NetworkRequest::SendMessage(peer, message) => {
                        self.handle_send_message(peer, message)
                    }
                    NetworkRequest::DisconnectWithPeer(peer) => self.outgoing_connections
                        .disconnect_with_peer(peer, self.network_tx.clone()),
                    // Immediately stop the event loop.
                    NetworkRequest::Shutdown => to_box(
                        cancel_sender
                            .take()
                            .ok_or_else(|| format_err!("shutdown twice"))
                            .into_future(),
                    ),
                }
            })
    }

    fn handle_send_message(
        &self,
        peer: SocketAddr,
        message: RawMessage,
    ) -> Box<dyn Future<Error = failure::Error, Item = ()> + 'static> {
        let conn_tx = self.outgoing_connections
            .get(peer)
            .map(|conn_tx| to_future(Ok(conn_tx)))
            .or_else(|| {
                self.outgoing_connections
                    .clone()
                    .connect_to_peer(
                        self.network_config,
                        peer,
                        self.network_tx.clone(),
                        &self.handle,
                        &self.handshake_params,
                    )
                    .map(|conn_tx|
                        // if we create new connect, we should send connect message
                        if &message == self.connect_message.raw() {
                            to_future(Ok(conn_tx))
                        } else {
                            to_future(conn_tx.send(self.connect_message.raw().clone())
                                .map_err(|_| {
                                    format_err!("can't send message to a connection")
                                }))
                        })
            });
        if let Some(conn_tx) = conn_tx {
            let fut = conn_tx.and_then(|conn_tx| {
                conn_tx
                    .send(message)
                    .map_err(|_| format_err!("can't send message to a connection"))
            });
            to_box(fut)
        } else {
            let event = NetworkEvent::UnableConnectToPeer(peer);
            let fut = self.network_tx
                .clone()
                .send(event)
                .map_err(|_| format_err!("can't send network event"))
                .into_future();
            to_box(fut)
        }
    }
}

struct Listener(Box<dyn Future<Item = (), Error = failure::Error>>);

impl Listener {
    fn bind(
        network_config: NetworkConfiguration,
        listen_address: SocketAddr,
        handle: Handle,
        network_tx: &mpsc::Sender<NetworkEvent>,
        handshake_params: &HandshakeParams,
    ) -> Result<Self, failure::Error> {
        // Incoming connections limiter
        let incoming_connections_limit = network_config.max_incoming_connections;
        let mut incoming_connections_counter = 0;
        // Incoming connections handler
        let listener = TcpListener::bind(&listen_address, &handle)?;
        let network_tx = network_tx.clone();
        let handshake_params = handshake_params.clone();
        let server = listener
            .incoming()
            .for_each(move |(sock, address)| {
                // Check incoming connections count
                incoming_connections_counter += 1;
                if incoming_connections_counter > incoming_connections_limit {
                    warn!(
                        "Rejected incoming connection with peer={}, \
                         connections limit reached.",
                        address
                    );
                    return to_box(future::ok(()));
                }
                trace!("Accepted incoming connection with peer={}", address);
                let network_tx = network_tx.clone();

                let handshake = NoiseHandshake::responder(&handshake_params, &address);
                let connection_handler = handshake
                    .listen(sock)
                    .and_then(move |sock| {
                        Self::handle_incoming_connection(sock, address, network_tx)
                    })
                    .then(move |result| {
                        incoming_connections_counter -= 1;
                        result
                    })
                    .map_err(|e| {
                        error!("Connection terminated: {}: {}", e, e.find_root_cause());
                    });

                handle.spawn(to_box(connection_handler));
                to_box(future::ok(()))
            })
            .map_err(into_failure);

        Ok(Listener(to_box(server)))
    }

    fn handle_incoming_connection(
        sock: Framed<TcpStream, MessagesCodec>,
        address: SocketAddr,
        network_tx: mpsc::Sender<NetworkEvent>,
    ) -> impl Future<Item = (), Error = failure::Error> {
        let (_, stream) = sock.split();
        stream
            .into_future()
            .map_err(|e| e.0)
            .and_then(|(raw, stream)| (Self::parse_connect_message(raw), Ok(stream)))
            .and_then(move |(connect, stream)| {
                trace!("Received handshake message={:?}", connect);
                Self::process_incoming_messages(stream, network_tx, connect, address)
            })
    }

    fn parse_connect_message(raw: Option<RawMessage>) -> Result<Connect, failure::Error> {
        let raw = raw.ok_or_else(|| format_err!("Incoming socket closed"))?;
        let message = Any::from_raw(raw).map_err(into_failure)?;
        match message {
            Any::Connect(connect) => Ok(connect),
            other => bail!(
                "First message from a remote peer is not Connect, got={:?}",
                other
            ),
        }
    }

    fn process_incoming_messages<S>(
        stream: SplitStream<S>,
        network_tx: mpsc::Sender<NetworkEvent>,
        connect: Connect,
        address: SocketAddr,
    ) -> impl Future<Item = (), Error = failure::Error>
    where
        S: Stream<Item = RawMessage, Error = failure::Error>,
    {
        let event = NetworkEvent::PeerConnected(address, connect);
        let stream = stream.map(move |raw| NetworkEvent::MessageReceived(address, raw));

        network_tx
            .send(event)
            .map_err(into_failure)
            .and_then(|sender| sender.sink_map_err(into_failure).send_all(stream))
            .map(|_| ())
    }
}

impl Future for Listener {
    type Item = ();
    type Error = failure::Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        self.0.poll()
    }
}

fn to_future<F, I>(fut: F) -> Box<dyn Future<Item = I, Error = failure::Error>>
where
    F: IntoFuture<Item = I, Error = failure::Error> + 'static,
{
    Box::new(fut.into_future())
}

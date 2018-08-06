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

use futures::{
    future, future::{err, Either}, stream::SplitStream, sync::mpsc, unsync, Future, IntoFuture,
    Poll, Sink, Stream,
};
use tokio_core::{
    net::{TcpListener, TcpStream}, reactor::Handle,
};
use tokio_io::codec::Framed;
use tokio_retry::{
    strategy::{jitter, FixedInterval}, Retry,
};

use std::{cell::RefCell, collections::HashMap, io, net::SocketAddr, rc::Rc, time::Duration};

use super::{
    error::{into_other, log_error, other_error, result_ok}, to_box,
};
use events::{
    codec::MessagesCodec, noise::{Handshake, HandshakeParams, NoiseHandshake},
};
use helpers::Milliseconds;
use crypto::x25519;
use messages::{Any, Connect, Message, RawMessage};
use node::ConnectInfo;

const OUTGOING_CHANNEL_SIZE: usize = 10;

#[derive(Debug)]
pub enum NetworkEvent {
    MessageReceived(SocketAddr, RawMessage),
    PeerConnected(ConnectInfo),
    PeerDisconnected(SocketAddr),
    UnableConnectToPeer(SocketAddr),
}

#[derive(Debug, Clone)]
pub enum NetworkRequest {
    SendMessage(SocketAddr, RawMessage),
    ConnectToPeer(SocketAddr),
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

    fn remove(&self, peer: &SocketAddr) -> Result<mpsc::Sender<RawMessage>, &'static str> {
        self.inner
            .borrow_mut()
            .remove(peer)
            .ok_or("there is no sender in the connection pool")
    }

    fn get(&self, peer: SocketAddr) -> Option<mpsc::Sender<RawMessage>> {
        self.inner.borrow_mut().get(&peer).cloned()
    }

    fn len(&self) -> usize {
        self.inner.borrow_mut().len()
    }

    fn get_or_create(&self,
                     peer: SocketAddr,
                     network_tx: mpsc::Sender<NetworkEvent>,
                     handle: &Handle, network_config: NetworkConfiguration, handshake_params: &HandshakeParams)
                     -> Option<Box<dyn Future<Item = mpsc::Sender<RawMessage>, Error = io::Error>>> {
        let conn_tx = self.get(peer)
            .map(|conn_tx| conn_fut(Ok(conn_tx).into_future()))
            .or_else(|| {
               self.clone().connect_to_peer(
                        network_config,
                        peer,
                        network_tx.clone(),
                        &handle,
                        &handshake_params,
                    )
                    .map(|conn_tx| {
                        let fut =
                            conn_fut(Ok(conn_tx).into_future());
                        fut
                    })
            });

        conn_tx
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
        let connect_handle = Retry::spawn(handle.clone(), strategy, action)
            .map_err(into_other)
            // Configure socket
            .and_then(move |sock| {
                sock.set_nodelay(network_config.tcp_nodelay)?;
                let duration =
                    network_config.tcp_keep_alive.map(Duration::from_millis);
                sock.set_keepalive(duration)?;
                Ok(sock)
            })
            .and_then(move |sock| {
                Self::build_handshake_initiator(sock, &peer, &handshake_params)
            })
            // Connect socket with the outgoing channel
            .and_then(move |(stream, _)| {
                trace!("Established connection with peer={}", peer);
                let (sink, stream) = stream.split();

                let writer = conn_rx
                    .map_err(|_| other_error("Can't send data into socket"))
                    .forward(sink);
                let reader = stream.for_each(result_ok);

                reader
                    .select2(writer)
                    .map_err(|_| other_error("Socket error"))
                    .and_then(|res| match res {
                        Either::A((_, _reader)) => Ok("by reader"),
                        Either::B((_, _writer)) => Ok("by writer"),
                    })
            })
            .then(move |res| {
                trace!(
                    "Disconnection with peer={}, reason={:?}",
                    peer,
                    res
                );
                self.disconnect_with_peer(peer, network_tx.clone())
            })
            .map_err(log_error);
        handle.spawn(connect_handle);
        Some(conn_tx)
    }

    fn disconnect_with_peer(
        &self,
        peer: SocketAddr,
        network_tx: mpsc::Sender<NetworkEvent>,
    ) -> Box<dyn Future<Item = (), Error = io::Error>> {
        let fut = self.remove(&peer)
            .into_future()
            .map_err(other_error)
            .and_then(move |_| {
                network_tx
                    .send(NetworkEvent::PeerDisconnected(peer))
                    .map_err(|_| other_error("can't send disconnect"))
            })
            .map(drop);
        to_box(fut)
    }

    fn build_handshake_initiator(
        stream: TcpStream,
        peer: &SocketAddr,
        handshake_params: &HandshakeParams,
    ) -> impl Future<Item = (Framed<TcpStream, MessagesCodec>, x25519::PublicKey), Error = io::Error> {
        let connect_list = &handshake_params.connect_list.clone();
        if let Some(remote_public_key) = connect_list.find_key_by_address(&peer) {
            let mut handshake_params = handshake_params.clone();
            handshake_params.set_remote_key(remote_public_key);
            NoiseHandshake::initiator(&handshake_params).send(stream)
        } else {
            Box::new(err(other_error(format!(
                "Attempt to connect to the peer with address {:?} which \
                 is not in the ConnectList",
                peer
            ))))
        }
    }
}

impl NetworkPart {
    pub fn run(
        self,
        handle: &Handle,
        handshake_params: &HandshakeParams,
    ) -> Box<dyn Future<Item = (), Error = io::Error>> {
        let network_config = self.network_config;
        // Cancellation token
        let (cancel_sender, cancel_handler) = unsync::oneshot::channel();

        let requests_handle = RequestHandler::new(
            self.our_connect_message,
            network_config,
            self.network_tx.clone(),
            handle.clone(),
            self.network_requests.1,
            cancel_sender,
            handshake_params,
        );

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
            .join(requests_handle)
            .map(drop)
            .select(cancel_handler)
            .map_err(|(e, _)| e);
        to_box(fut)
    }
}

struct RequestHandler(
    // TODO: Replace with concrete type. (ECR-1634)
    Box<dyn Future<Item = (), Error = io::Error>>,
);

impl RequestHandler {
    fn new(
        connect_message: Connect,
        network_config: NetworkConfiguration,
        network_tx: mpsc::Sender<NetworkEvent>,
        handle: Handle,
        receiver: mpsc::Receiver<NetworkRequest>,
        cancel_sender: unsync::oneshot::Sender<()>,
        handshake_params: &HandshakeParams,
    ) -> Self {
        let mut cancel_sender = Some(cancel_sender);
        let outgoing_connections = ConnectionsPool::new();
        let handshake_params = handshake_params.clone();
        let requests_handler = receiver
            .map_err(|_| other_error("no network requests"))
            .for_each(move |request| {
                match request {
                    NetworkRequest::ConnectToPeer(peer) => {
                        let conn_tx = outgoing_connections.get_or_create(peer, network_tx.clone(),
                                                                         &handle, network_config, &handshake_params);

                        if let Some(conn_tx) = conn_tx {
                            to_box(conn_tx)
                        } else {
                            to_box(Self::send_unable_connect(peer, network_tx.clone()))
                        }
                    }
                    NetworkRequest::SendMessage(peer, msg) => {
                        let conn_tx = outgoing_connections.get_or_create(peer, network_tx.clone(),
                                                                         &handle, network_config, &handshake_params);

                        if let Some(conn_tx) = conn_tx {
                            let fut = conn_tx.and_then(|conn_tx| {
                                conn_tx
                                    .send(msg)
                                    .map_err(|_| other_error("can't send message to a connection"))
                            });
                            to_box(fut)
                        } else {
                            to_box(Self::send_unable_connect(peer, network_tx.clone()))
                        }
                    }
                    NetworkRequest::DisconnectWithPeer(peer) => {
                        outgoing_connections.disconnect_with_peer(peer, network_tx.clone())
                    }
                    // Immediately stop the event loop.
                    NetworkRequest::Shutdown => to_box(
                        cancel_sender
                            .take()
                            .ok_or_else(|| other_error("shutdown twice"))
                            .into_future(),
                    ),
                }
            });
        RequestHandler(to_box(requests_handler))
    }

    fn send_unable_connect(peer: SocketAddr, network_tx: mpsc::Sender<NetworkEvent>) -> impl Future<Item=mpsc::Sender<NetworkEvent>, Error=io::Error> {
        let event = NetworkEvent::UnableConnectToPeer(peer);
        let fut = network_tx
            .clone()
            .send(event)
            .map_err(|_| other_error("can't send network event"))
            .into_future();
        fut
    }
}

impl Future for RequestHandler {
    type Item = ();
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        self.0.poll()
    }
}

struct Listener(Box<dyn Future<Item = (), Error = io::Error>>);

impl Listener {
    fn bind(
        network_config: NetworkConfiguration,
        listen_address: SocketAddr,
        handle: Handle,
        network_tx: &mpsc::Sender<NetworkEvent>,
        handshake_params: &HandshakeParams,
    ) -> Result<Self, io::Error> {
        // Incoming connections limiter
        let incoming_connections_limit = network_config.max_incoming_connections;
        // The reference counter is used to automatically count the number of the open connections.
        let incoming_connections_counter: Rc<()> = Rc::default();
        // Incoming connections handler
        let listener = TcpListener::bind(&listen_address, &handle)?;
        let network_tx = network_tx.clone();
        let handshake_params = handshake_params.clone();
        let server = listener.incoming().for_each(move |(sock, address)| {
            let holder = Rc::downgrade(&incoming_connections_counter);
            // Check incoming connections count
            let connections_count = Rc::weak_count(&incoming_connections_counter);
            if connections_count > incoming_connections_limit {
                warn!(
                    "Rejected incoming connection with peer={}, \
                     connections limit reached.",
                    address
                );
                return to_box(future::ok(()));
            }
            trace!("Accepted incoming connection with peer={}", address);
            let network_tx = network_tx.clone();

            let handshake = NoiseHandshake::responder(&handshake_params);
            let connection_handler = handshake
                .listen(sock)
                .and_then(move |(sock, _public_key)| {
                    trace!("Remote connection established with socket={:?}", sock);
                    let (_, stream) = sock.split();

                    Self::process_incoming_messages(stream, network_tx, address)
                        .map(|_| {
                            // Ensure that holder lives until the stream ends.
                            let _holder = holder;
                        })
                })
                .map_err(log_error);

            handle.spawn(to_box(connection_handler));
            to_box(future::ok(()))
        });

        Ok(Listener(to_box(server)))
    }

    fn process_incoming_messages<S>(
        stream: SplitStream<S>,
        network_tx: mpsc::Sender<NetworkEvent>,
        address: SocketAddr,
    ) -> impl Future<Item = (), Error = io::Error>
    where
        S: Stream<Item = RawMessage, Error = io::Error>,
    {
        use crypto::PublicKey;
        let info = ConnectInfo { address, public_key: PublicKey::zero() };
        let event = NetworkEvent::PeerConnected(info);
        let stream = stream.map(move |raw| NetworkEvent::MessageReceived(address, raw));

        network_tx
            .send(event)
            .map_err(into_other)
            .and_then(|sender| sender.sink_map_err(into_other).send_all(stream))
            .map(|_| ())
    }
}

impl Future for Listener {
    type Item = ();
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        self.0.poll()
    }
}

fn conn_fut<F>(fut: F) -> Box<dyn Future<Item = mpsc::Sender<RawMessage>, Error = io::Error>>
where
    F: Future<Item = mpsc::Sender<RawMessage>, Error = io::Error> + 'static,
{
    Box::new(fut)
}

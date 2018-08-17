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

use rand::{self, Rng};

use std::{error::Error, net::SocketAddr};

use super::{NodeHandler, NodeRole, RequestData};
use helpers::Height;
use messages::{Any, Message, PeersExchange, PeersRequest, RawMessage, Status};
use node::ConnectInfo;

impl NodeHandler {
    /// Redirects message to the corresponding `handle_...` function.
    pub fn handle_message(&mut self, raw: RawMessage) {
        match Any::from_raw(raw) {
            Ok(Any::Status(msg)) => self.handle_status(&msg),
            Ok(Any::Consensus(msg)) => self.handle_consensus(msg),
            Ok(Any::Request(msg)) => self.handle_request(msg),
            Ok(Any::Block(msg)) => self.handle_block(&msg),
            Ok(Any::Transaction(msg)) => self.handle_tx(msg),
            Ok(Any::TransactionsBatch(msg)) => self.handle_txs_batch(&msg),
            Ok(Any::PeersExchange(msg)) => self.handle_peers_exchange(&msg),
            Err(err) => {
                error!("Invalid message received: {:?}", err.description());
            }
        }
    }

    /// Handles the incoming connection. Node connects to the sender
    /// if received `PeersExchange` is correct.
    pub fn handle_connected(&mut self, peers_exchange: &PeersExchange) {
        info!("Received PeersExchange  from peer: {:?}", peers_exchange);
        self.handle_peers_exchange(&peers_exchange);
    }

    /// Handles the `Disconnected` event. Node will try to connect to that address again if it was
    /// in the validators list.
    pub fn handle_disconnected(&mut self, addr: SocketAddr) {
        info!("Disconnected from: {}", addr);
        self.remove_peer_with_addr(addr);
    }

    /// Handles the `UnableConnectToPeer` event. Node will try to connect to that address again
    /// if it was in the validators list.
    pub fn handle_unable_to_connect(&mut self, addr: SocketAddr) {
        info!("Could not connect to: {}", addr);
        self.remove_peer_with_addr(addr);
    }

    /// Removes peer from the state and from the cache. Node will try to connect to that address
    /// again if it was in the validators list.
    fn remove_peer_with_addr(&mut self, addr: SocketAddr) {
        let need_reconnect = self.state.remove_peer_with_addr(&addr);
        if need_reconnect {
            self.connect(&addr);
        }
        self.blockchain.remove_peer_with_addr(&addr);
    }

    /// Handles the `ConnectInfo` and connects to a peer as result.
    pub fn handle_peers_exchange(&mut self, peers_exchange: &PeersExchange) {
        // TODO Add spam protection. (ECR-170)
        if !self.state
            .connect_list()
            .is_peer_allowed(peers_exchange.from())
        {
            error!(
                "Received PeersExchange message from peer = {:?} which not in ConnectList.",
                peers_exchange.from()
            );
            return;
        }

        for peer in peers_exchange.peers() {
            let address = peer.address;
            if address == self.state.our_connect_info().address {
                trace!("Received ConnectInfo with same address as our external_address.");
                return;
            }

            let public_key = peer.public_key;
            if public_key == self.state.our_connect_info().public_key {
                trace!(
                    "Received ConnectInfo with same public key {:?} as ours.",
                    public_key
                );
                return;
            }

            // Check if we have another connect message from peer with the given public_key.
            let mut need_connect = true;
            if let Some(saved_message) = self.state.peers().get(&public_key) {
                if saved_message.address == peer.address {
                    need_connect = false;
                } else {
                    error!("Received weird ConnectInfo from {}", address);
                    return;
                }
            }
            self.state.add_peer(public_key, peer);
            info!("Received ConnectInfo  from {}, {}", address, need_connect,);
            self.blockchain.save_peer(&public_key, peer);
            if need_connect {
                info!("Connecting to {}", address);
                self.connect(&address);
            }
        }
    }

    /// Handles the `Status` message. Node sends `BlockRequest` as response if height in the
    /// message is higher than node's height.
    pub fn handle_status(&mut self, msg: &Status) {
        let height = self.state.height();
        trace!(
            "HANDLE STATUS: current height = {}, msg height = {}",
            height,
            msg.height()
        );

        if !self.state.connect_list().is_peer_allowed(msg.from()) {
            error!(
                "Received status message from peer = {:?} which not in ConnectList.",
                msg.from()
            );
            return;
        }

        // Handle message from future height
        if msg.height() > height {
            let peer = msg.from();

            if !msg.verify_signature(peer) {
                error!(
                    "Received status message with incorrect signature, msg={:?}",
                    msg
                );
                return;
            }

            // Check validator height info
            if msg.height() > self.state.node_height(peer) {
                // Update validator height
                self.state.set_node_height(*peer, msg.height());
            }

            // Request block
            self.request(RequestData::Block(height), *peer);
        }
    }

    /// Handles the `PeersRequest` message. Node sends known peers to message sender.
    pub fn handle_request_peers(&mut self, msg: &PeersRequest) {
        let peers: Vec<ConnectInfo> = self.state.peers().values().cloned().collect();

        info!(
            "HANDLE REQUEST PEERS: Sending {:?} peers to {:?}",
            peers,
            msg.from()
        );

        let peers_request = PeersExchange::new(
            &self.state().consensus_public_key(),
            &msg.from(),
            peers,
            &self.state().consensus_secret_key(),
        );

        self.send_to_peer(*msg.from(), peers_request.raw())
    }

    /// Handles `NodeTimeout::Status`, broadcasts the `Status` message if it isn't outdated as
    /// result.
    pub fn handle_status_timeout(&mut self, height: Height) {
        if self.state.height() == height {
            self.broadcast_status();
            self.add_status_timeout();
        }
    }
    /// Handles `NodeTimeout::PeerExchange`. Node sends the `PeersRequest` to a random peer.
    pub fn handle_peer_exchange_timeout(&mut self) {
        if !self.state.peers().is_empty() {
            let to = self.state.peers().len();
            let gen_peer_id = || -> usize {
                let mut rng = rand::thread_rng();
                rng.gen_range(0, to)
            };

            let peer = self.state
                .peers()
                .iter()
                .map(|x| *x.1)
                .nth(gen_peer_id())
                .unwrap();

            let msg = PeersRequest::new(
                self.state.consensus_public_key(),
                &peer.public_key,
                self.state.consensus_secret_key(),
            );
            trace!("Request peers from peer with addr {:?}", peer.address);
            self.send_to_peer(peer.public_key, msg.raw());
        }
        self.add_peer_exchange_timeout();
    }
    /// Handles `NodeTimeout::UpdateApiState`.
    /// Node update internal `ApiState` and `NodeRole`.
    pub fn handle_update_api_state_timeout(&mut self) {
        self.api_state.update_node_state(&self.state);
        self.node_role = NodeRole::new(self.state.validator_id());
        self.add_update_api_state_timeout();
    }

    /// Broadcasts the `Status` message to all peers.
    pub fn broadcast_status(&mut self) {
        let hash = self.blockchain.last_hash();
        let status = Status::new(
            self.state.consensus_public_key(),
            self.state.height(),
            &hash,
            self.state.consensus_secret_key(),
        );
        trace!("Broadcast status: {:?}", status);
        self.broadcast(status.raw());
    }
}

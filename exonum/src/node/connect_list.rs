// Copyright 2019 The Exonum Team
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

//! Mapping between peers public keys and IP-addresses.

use std::collections::BTreeMap;

use crate::crypto::{kx, PublicKey};
use crate::node::{ConnectInfo, ConnectListConfig};

/// Network address of the peer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerAddress {
    /// External address of the peer hostname:port.
    pub address: String,
}

impl PeerAddress {
    /// New unresolved address.
    pub fn new(address: String) -> Self {
        PeerAddress { address }
    }
}

/// `ConnectList` stores mapping between IP-addresses and public keys.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ConnectList {
    /// Peers to which we can connect.
    #[serde(default)]
    pub peers: BTreeMap<PublicKey, PeerAddress>,
    /// Mapping between consensus and identity keys.
    pub identity: BTreeMap<PublicKey, kx::PublicKey>,
}

impl ConnectList {
    /// Creates `ConnectList` from config.
    pub fn from_config(config: ConnectListConfig) -> Self {
        let mut identity = BTreeMap::new();
        let peers: BTreeMap<PublicKey, PeerAddress> = config
            .peers
            .into_iter()
            .map(|peer| {
                identity.insert(peer.public_key, peer.identity_key);
                (peer.public_key, PeerAddress::new(peer.address))
            })
            .collect();

        ConnectList { peers, identity }
    }

    /// Returns `true` if a peer with the given public key can connect.
    pub fn is_peer_allowed(&self, peer: &PublicKey) -> bool {
        self.peers.contains_key(peer)
    }

    /// Check if we allow to connect to `address`.
    pub fn is_address_allowed(&self, address: &str) -> bool {
        self.peers.values().any(|a| a.address == address)
    }

    /// Get peer address with public key.
    pub fn find_address_by_pubkey(&self, key: &PublicKey) -> Option<&PeerAddress> {
        self.peers.get(key)
    }

    /// Adds peer to the ConnectList.
    pub fn add(&mut self, peer: ConnectInfo) {
        self.peers
            .insert(peer.public_key, PeerAddress::new(peer.address));

        self.identity.insert(peer.public_key, peer.identity_key);
    }

    /// Update peer address.
    pub fn update_peer(&mut self, public_key: &PublicKey, address: String) {
        self.peers.insert(*public_key, PeerAddress::new(address));
    }
}

#[cfg(test)]
mod test {
    use rand::{rngs::StdRng, RngCore, SeedableRng};

    use super::*;
    use crate::crypto::{gen_keypair, PublicKey, PUBLIC_KEY_LENGTH};
    use crate::node::ConnectInfo;

    const SEED_LENGTH: usize = 32;
    static VALIDATORS: [[u8; SEED_LENGTH]; 2] = [[1; SEED_LENGTH], [2; SEED_LENGTH]];
    static REGULAR_PEERS: [u8; SEED_LENGTH] = [3; SEED_LENGTH];

    fn make_keys(source: [u8; SEED_LENGTH], count: usize) -> (Vec<PublicKey>, Vec<kx::PublicKey>) {
        let mut rng: StdRng = SeedableRng::from_seed(source);
        (0..count)
            .map(|_| {
                let mut key = [0; PUBLIC_KEY_LENGTH];
                rng.fill_bytes(&mut key);
                (
                    PublicKey::from_slice(&key).unwrap(),
                    kx::PublicKey::from_slice(&key).unwrap(),
                )
            })
            .unzip()
    }

    fn check_in_connect_list(
        connect_list: &ConnectList,
        keys: &[PublicKey],
        in_connect_list: &[usize],
        not_in_connect_list: &[usize],
    ) {
        for i in in_connect_list {
            assert_eq!(connect_list.is_peer_allowed(&keys[*i]), true);
        }
        for i in not_in_connect_list {
            assert_eq!(connect_list.is_peer_allowed(&keys[*i]), false);
        }
    }

    fn assert_identity_keys(
        connect_list: &ConnectList,
        peers: &(Vec<PublicKey>, Vec<kx::PublicKey>),
    ) {
        for (pk, pk_kx) in peers.0.iter().zip(peers.1.clone()) {
            assert_eq!(connect_list.identity.get(pk).unwrap(), &pk_kx)
        }
    }

    #[test]
    fn test_whitelist() {
        let regular = make_keys(REGULAR_PEERS, 4);
        let address = "127.0.0.1:80".to_owned();

        let mut connect_list = ConnectList::default();
        check_in_connect_list(&connect_list, &regular.0, &[], &[0, 1, 2, 3]);
        connect_list.add(ConnectInfo {
            public_key: regular.0[0],
            address: address.clone(),
            identity_key: regular.1[0],
        });
        check_in_connect_list(&connect_list, &regular.0, &[0], &[1, 2, 3]);
        connect_list.add(ConnectInfo {
            public_key: regular.0[2],
            address: address.clone(),
            identity_key: regular.1[2],
        });
        check_in_connect_list(&connect_list, &regular.0, &[0, 2], &[1, 3]);

        assert_eq!(connect_list.peers.len(), 2);
    }

    #[test]
    fn test_validators_in_whitelist() {
        let regular = make_keys(REGULAR_PEERS, 4);
        let validators = make_keys(VALIDATORS[0], 2);
        let mut connect_list = ConnectList::default();
        check_in_connect_list(&connect_list, &regular.0, &[], &[0, 1, 2, 3]);
        check_in_connect_list(&connect_list, &validators.0, &[], &[0, 1]);
        assert_eq!(connect_list.peers.len(), 0);
        assert!(connect_list.identity.get(&regular.0[0]).is_none());
        assert!(connect_list.identity.get(&validators.0[0]).is_none());

        add_to_connect_list(&mut connect_list, &validators);
        assert_eq!(connect_list.peers.len(), 2);
        check_in_connect_list(&connect_list, &regular.0, &[], &[0, 1, 2, 3]);
        check_in_connect_list(&connect_list, &validators.0, &[0, 1], &[]);
        assert!(connect_list.identity.get(&regular.0[0]).is_none());
        assert_identity_keys(&connect_list, &validators);
    }

    fn add_to_connect_list(
        connect_list: &mut ConnectList,
        peers: &(Vec<PublicKey>, Vec<kx::PublicKey>),
    ) {
        let address = "127.0.0.1:80".to_owned();
        for (pk, pk_kx) in peers.0.iter().zip(peers.1.clone()) {
            connect_list.add(ConnectInfo {
                public_key: *pk,
                address: address.clone(),
                identity_key: pk_kx,
            })
        }
    }

    #[test]
    fn test_update_validators() {
        let validators0 = make_keys(VALIDATORS[0], 2);
        let validators1 = make_keys(VALIDATORS[1], 2);
        let mut connect_list = ConnectList::default();
        assert_eq!(connect_list.peers.len(), 0);
        add_to_connect_list(&mut connect_list, &validators0);
        assert_eq!(connect_list.peers.len(), 2);
        assert_identity_keys(&connect_list, &validators0);
        check_in_connect_list(&connect_list, &validators0.0, &[0, 1], &[]);
        check_in_connect_list(&connect_list, &validators1.0, &[], &[0, 1]);
        add_to_connect_list(&mut connect_list, &validators1);
        assert_eq!(connect_list.peers.len(), 4);
        check_in_connect_list(&connect_list, &validators0.0, &[0, 1], &[]);
        check_in_connect_list(&connect_list, &validators1.0, &[0, 1], &[]);
        assert_eq!(
            connect_list.identity.get(&validators0.0[0]).unwrap(),
            &validators0.1[0]
        );
        assert_identity_keys(&connect_list, &validators0);
        assert_identity_keys(&connect_list, &validators1);
    }

    #[test]
    fn test_address_allowed() {
        let (public_key, _) = gen_keypair();
        let (identity, _) = kx::gen_keypair();
        let address = "127.0.0.1:80".to_owned();

        let mut connect_list = ConnectList::default();
        assert!(!connect_list.is_address_allowed(&address));

        connect_list.add(ConnectInfo {
            public_key,
            address: address.clone(),
            identity_key: identity,
        });
        assert!(connect_list.is_address_allowed(&address));
    }

    #[test]
    fn identity_key_added() {
        let (public_key, _) = gen_keypair();
        let (identity, _) = kx::gen_keypair();
        let address = "127.0.0.1:80".to_owned();

        let mut connect_list = ConnectList::default();

        let connect_info = ConnectInfo {
            public_key,
            address: address.clone(),
            identity_key: identity,
        };

        connect_list.add(connect_info.clone());
        assert!(connect_list.identity.contains_key(&public_key));

        let connect_list_config = ConnectListConfig {
            peers: vec![connect_info],
        };

        let connect_list = ConnectList::from_config(connect_list_config);
        assert!(connect_list.identity.contains_key(&public_key));
    }

}

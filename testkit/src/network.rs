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

use serde::{Deserialize, Serialize};

use exonum::{
    blockchain::{ConsensusConfig, ValidatorKeys},
    crypto::{self, PublicKey, SecretKey},
    helpers::{Height, Round, ValidatorId},
    keys::Keys,
    messages::{Precommit, Propose, Verified},
    proto::Any,
};
use exonum_merkledb::ObjectHash;

/// Emulated test network.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TestNetwork {
    us: TestNode,
    validators: Vec<TestNode>,
}

impl TestNetwork {
    /// Creates a new emulated network.
    pub fn new(validator_count: u16) -> Self {
        Self::with_our_role(Some(ValidatorId(0)), validator_count)
    }

    /// Creates a new emulated network with a specific role of the node
    /// the network will be viewed from.
    pub fn with_our_role(us: Option<ValidatorId>, validator_count: u16) -> Self {
        assert!(
            validator_count > 0,
            "At least one validator should be present in the network."
        );

        let validators = (0..validator_count)
            .map(ValidatorId)
            .map(TestNode::new_validator)
            .collect::<Vec<_>>();

        let us = if let Some(ValidatorId(id)) = us {
            validators[id as usize].clone()
        } else {
            TestNode::new_auditor()
        };
        TestNetwork { validators, us }
    }

    /// Returns the node in the emulated network, from whose perspective the testkit operates.
    pub fn us(&self) -> &TestNode {
        &self.us
    }

    /// Returns a slice of all validators in the network.
    pub fn validators(&self) -> &[TestNode] {
        &self.validators
    }

    /// Returns config encoding the network structure usable for creating the genesis block of
    /// a blockchain.
    pub fn genesis_config(&self) -> ConsensusConfig {
        ConsensusConfig {
            validator_keys: self.validators.iter().map(TestNode::public_keys).collect(),
            ..ConsensusConfig::default()
        }
    }

    /// Updates the test network by the new set of nodes.
    pub fn update<I: IntoIterator<Item = TestNode>>(&mut self, mut us: TestNode, validators: I) {
        let validators = validators
            .into_iter()
            .enumerate()
            .map(|(id, mut validator)| {
                let validator_id = ValidatorId(id as u16);
                validator.change_role(Some(validator_id));
                if us.public_keys().consensus_key == validator.public_keys().consensus_key {
                    us.change_role(Some(validator_id));
                }
                validator
            })
            .collect::<Vec<_>>();
        self.validators = validators;
        self.us.clone_from(&us);
    }

    /// Updates the test network with a new configuration.
    pub fn update_configuration(&mut self, config: TestNetworkConfiguration) {
        self.update(config.us, config.validators);
    }

    /// Returns service public key of the validator with given id.
    pub fn service_public_key_of(&self, id: ValidatorId) -> Option<PublicKey> {
        self.validators()
            .get(id.0 as usize)
            .map(|x| x.keys.service_pk())
    }

    /// Returns consensus public key of the validator with given id.
    pub fn consensus_public_key_of(&self, id: ValidatorId) -> Option<PublicKey> {
        self.validators()
            .get(id.0 as usize)
            .map(|x| x.keys.consensus_pk())
    }
}

/// An emulated node in the test network.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TestNode {
    keys: Keys,
    validator_id: Option<ValidatorId>,
}

impl TestNode {
    /// Creates a new auditor.
    pub fn new_auditor() -> Self {
        let (consensus_pk, consensus_sk) = crypto::gen_keypair();
        let (service_pk, service_sk) = crypto::gen_keypair();

        TestNode {
            keys: Keys::from_keys(consensus_pk, consensus_sk, service_pk, service_sk),
            validator_id: None,
        }
    }

    /// Creates a new validator with the given id.
    pub fn new_validator(validator_id: ValidatorId) -> Self {
        let (consensus_pk, consensus_sk) = crypto::gen_keypair();
        let (service_pk, service_sk) = crypto::gen_keypair();

        TestNode {
            keys: Keys::from_keys(consensus_pk, consensus_sk, service_pk, service_sk),
            validator_id: Some(validator_id),
        }
    }

    /// Constructs a new node from the given keypairs.
    pub fn from_parts(
        consensus_keypair: (PublicKey, SecretKey),
        service_keypair: (PublicKey, SecretKey),
        validator_id: Option<ValidatorId>,
    ) -> TestNode {
        TestNode {
            keys: Keys::from_keys(
                consensus_keypair.0,
                consensus_keypair.1,
                service_keypair.0,
                service_keypair.1,
            ),
            validator_id,
        }
    }

    /// Creates a `Propose` message signed by this validator.
    pub fn create_propose(
        &self,
        height: Height,
        last_hash: crypto::Hash,
        tx_hashes: impl IntoIterator<Item = crypto::Hash>,
    ) -> Verified<Propose> {
        Verified::from_value(
            Propose::new(
                self.validator_id
                    .expect("An attempt to create propose from a non-validator node."),
                height,
                Round::first(),
                last_hash,
                tx_hashes,
            ),
            self.keys.consensus_pk(),
            &self.keys.consensus_sk(),
        )
    }

    /// Creates a `Precommit` message signed by this validator.
    pub fn create_precommit(
        &self,
        propose: &Propose,
        block_hash: crypto::Hash,
    ) -> Verified<Precommit> {
        use std::time::SystemTime;

        Verified::from_value(
            Precommit::new(
                self.validator_id
                    .expect("An attempt to create propose from a non-validator node."),
                propose.height(),
                propose.round(),
                propose.object_hash(),
                block_hash,
                SystemTime::now().into(),
            ),
            self.keys.consensus_pk(),
            &self.keys.consensus_sk(),
        )
    }

    /// Returns public keys of the node.
    pub fn public_keys(&self) -> ValidatorKeys {
        ValidatorKeys {
            consensus_key: self.keys.consensus_pk(),
            service_key: self.keys.service_pk(),
        }
    }

    /// Returns the current validator id of node if it is validator of the test network.
    pub fn validator_id(&self) -> Option<ValidatorId> {
        self.validator_id
    }

    /// Changes node role.
    pub fn change_role(&mut self, role: Option<ValidatorId>) {
        self.validator_id = role;
    }

    /// Returns the service keypair of the node.
    pub fn service_keypair(&self) -> (PublicKey, SecretKey) {
        (self.keys.service_pk(), self.keys.service_sk().clone())
    }

    /// Returns the consensus keypair of the node.
    pub fn consensus_keypair(&self) -> (PublicKey, &SecretKey) {
        (self.keys.consensus_pk(), &self.keys.consensus_sk())
    }
}

impl From<TestNode> for ValidatorKeys {
    fn from(node: TestNode) -> Self {
        node.public_keys()
    }
}

/// A configuration of the test network.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TestNetworkConfiguration {
    us: TestNode,
    validators: Vec<TestNode>,
    consensus_config: ConsensusConfig,
    actual_from: Height,
}

impl TestNetworkConfiguration {
    pub(crate) fn new(network: &TestNetwork, consensus_config: ConsensusConfig) -> Self {
        TestNetworkConfiguration {
            us: network.us().clone(),
            validators: network.validators().into(),
            consensus_config,
            actual_from: Height::zero(),
        }
    }

    /// Returns the node from whose perspective the testkit operates.
    pub fn us(&self) -> &TestNode {
        &self.us
    }

    /// Modifies the node from whose perspective the testkit operates.
    pub fn set_us(&mut self, us: TestNode) {
        self.us = us;
        self.update_our_role();
    }

    /// Returns the test network validators.
    pub fn validators(&self) -> &[TestNode] {
        self.validators.as_ref()
    }

    /// Returns the current consensus configuration.
    pub fn consensus_config(&self) -> &ConsensusConfig {
        &self.consensus_config
    }

    /// Return the height, starting from which this configuration becomes actual.
    pub fn actual_from(&self) -> Height {
        self.actual_from
    }

    /// Modifies the height, starting from which this configuration becomes actual.
    pub fn set_actual_from(&mut self, actual_from: Height) {
        self.actual_from = actual_from;
    }

    /// Modifies the current consensus configuration.
    pub fn set_consensus_config(&mut self, consensus_config: ConsensusConfig) {
        self.consensus_config = consensus_config;
    }

    /// Modifies the validators list.
    pub fn set_validators<I>(&mut self, validators: I)
    where
        I: IntoIterator<Item = TestNode>,
    {
        self.validators = validators
            .into_iter()
            .enumerate()
            .map(|(idx, mut node)| {
                node.change_role(Some(ValidatorId(idx as u16)));
                node
            })
            .collect();
        self.consensus_config.validator_keys = self
            .validators
            .iter()
            .cloned()
            .map(ValidatorKeys::from)
            .collect();
        self.update_our_role();
    }

    /// Returns the configuration for service with the given identifier.
    pub fn service_config<D>(&self, _id: &str) -> D
    where
        for<'de> D: Deserialize<'de>,
    {
        unimplemented!();
    }

    /// Modifies the configuration of the service with the given identifier.
    pub fn set_service_config<D>(&mut self, _id: &str, _config: D)
    where
        D: Into<Any>,
    {
        unimplemented!();
    }

    fn update_our_role(&mut self) {
        let validator_id = self
            .validators
            .iter()
            .position(|x| x.public_keys().service_key == self.us.keys.service_pk())
            .map(|x| ValidatorId(x as u16));
        self.us.validator_id = validator_id;
    }
}

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

use snow::CryptoResolver;
use snow::params::DHChoice;
use snow::types::Random;
use snow::types::Dh;
use snow::params::HashChoice;
use snow::types::Hash;
use snow::params::CipherChoice;
use snow::types::Cipher;
use snow::wrappers::rand_wrapper::RandomOs;
use snow::wrappers::crypto_wrapper::HashSHA256;
use snow::wrappers::crypto_wrapper::HashSHA512;
use snow::wrappers::crypto_wrapper::HashBLAKE2s;
use snow::wrappers::crypto_wrapper::{HashBLAKE2b, CipherAESGCM, CipherChaChaPoly};
use rust_crypto::curve25519::curve25519_base;
use rust_crypto::curve25519::curve25519;
use snow::DefaultResolver;


pub struct ExonumResolver{
    parent: DefaultResolver
}

impl ExonumResolver {
    pub fn new() -> Self {
        ExonumResolver{ parent: DefaultResolver }
    }
}

impl CryptoResolver for ExonumResolver {
    fn resolve_rng(&self) -> Option<Box<Random + Send>> {
        self.parent.resolve_rng()
    }

    fn resolve_dh(&self, choice: &DHChoice) -> Option<Box<Dh + Send>> {
        match *choice {
            DHChoice::Curve25519 => Some(Box::new(Dh25519::default())),
            _                    => None,
        }
    }

    fn resolve_hash(&self, choice: &HashChoice) -> Option<Box<Hash + Send>> {
        self.parent.resolve_hash(choice)
    }

    fn resolve_cipher(&self, choice: &CipherChoice) -> Option<Box<Cipher + Send>> {
        self.parent.resolve_cipher(choice)
    }
}

pub struct Dh25519 {
    pubkey:  [u8; 32],
    privkey: [u8; 64],
}

impl Default for Dh25519 {
    fn default() -> Self {
        Dh25519 {
            pubkey: [0; 32],
            privkey: [0; 64]
        }
    }
}

impl Dh for Dh25519 {

    fn name(&self) -> &'static str {
        static NAME: &'static str = "25519";
        NAME
    }

    fn pub_len(&self) -> usize {
        32
    }

    fn priv_len(&self) -> usize {
        64
    }

    fn set(&mut self, privkey: &[u8]) {
        copy_memory(privkey, &mut self.privkey);
        let pubkey = curve25519_base(&self.privkey);
        copy_memory(&pubkey, &mut self.pubkey);
    }

    fn generate(&mut self, rng: &mut Random) {
        rng.fill_bytes(&mut self.privkey);
        self.privkey[0]  &= 248;
        self.privkey[31] &= 127;
        self.privkey[31] |= 64;
        let pubkey = curve25519_base(&self.privkey);
        copy_memory(&pubkey, &mut self.pubkey);
    }

    fn pubkey(&self) -> &[u8] {
        &self.pubkey
    }

    fn privkey(&self) -> &[u8] {
        &self.privkey
    }

    fn dh(&self, pubkey: &[u8], out: &mut [u8]) {
        let result = curve25519(&self.privkey, pubkey);
        copy_memory(&result, out);
    }
}

pub fn copy_memory(input: &[u8], out: &mut [u8]) -> usize {
    for count in 0..input.len() {out[count] = input[count];}
    input.len()
}

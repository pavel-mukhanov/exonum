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

use std::{borrow::Cow, fmt::Debug, io::Write};

use byteorder::{ByteOrder, LittleEndian, ReadBytesExt, WriteBytesExt};
use criterion::{black_box, Bencher, Criterion};
use failure::{self, format_err};
use rand::{RngCore, SeedableRng};
use rand_xorshift::XorShiftRng;

use exonum_crypto::{self, Hash};
use exonum_merkledb::{
    proof_map_index::{BranchNode, ProofPath},
    BinaryKey, BinaryValue, UniqueHash,
};

const CHUNK_SIZE: usize = 64;
const SEED: [u8; 16] = [100; 16];

#[derive(Debug, Clone, Copy, PartialEq)]
struct SimpleData {
    id: u16,
    class: i16,
    value: i32,
    hash: Hash,
}

impl BinaryValue for SimpleData {
    fn to_bytes(&self) -> Vec<u8> {
        let mut buffer = vec![0; 40];
        LittleEndian::write_u16(&mut buffer[0..2], self.id);
        LittleEndian::write_i16(&mut buffer[2..4], self.class);
        LittleEndian::write_i32(&mut buffer[4..8], self.value);
        buffer[8..].copy_from_slice(self.hash.as_ref());
        buffer
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Result<Self, failure::Error> {
        let bytes = bytes.as_ref();
        let id = LittleEndian::read_u16(&bytes[0..2]);
        let class = LittleEndian::read_i16(&bytes[2..4]);
        let value = LittleEndian::read_i32(&bytes[4..8]);
        let hash = Hash::from_slice(&bytes[8..40]).unwrap();
        Ok(Self {
            id,
            class,
            value,
            hash,
        })
    }
}

impl UniqueHash for SimpleData {}

#[derive(Debug, Clone, Copy, PartialEq)]
struct CursorData {
    id: u16,
    class: i16,
    value: i32,
    hash: Hash,
}

impl BinaryValue for CursorData {
    fn to_bytes(&self) -> Vec<u8> {
        let mut buf = vec![0; 40];
        let mut cursor = buf.as_mut_slice();
        cursor.write_u16::<LittleEndian>(self.id).unwrap();
        cursor.write_i16::<LittleEndian>(self.class).unwrap();
        cursor.write_i32::<LittleEndian>(self.value).unwrap();
        cursor.write_all(self.hash.as_ref()).unwrap();
        buf
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Result<Self, failure::Error> {
        let mut bytes = bytes.as_ref();
        let id = bytes.read_u16::<LittleEndian>()?;
        let class = bytes.read_i16::<LittleEndian>()?;
        let value = bytes.read_i32::<LittleEndian>()?;
        let hash = Hash::from_slice(bytes).ok_or_else(|| format_err!("Unable to decode hash value"))?;
        Ok(Self {
            id,
            class,
            value,
            hash,
        })
    }
}

impl UniqueHash for CursorData {}

fn gen_bytes_data() -> Vec<u8> {
    let mut rng = XorShiftRng::from_seed(SEED);
    let mut v = vec![0; CHUNK_SIZE];
    rng.fill_bytes(&mut v);
    v
}

fn gen_sample_data() -> SimpleData {
    SimpleData {
        id: 1,
        class: -5,
        value: 127,
        hash: exonum_crypto::hash(&[1, 2, 3]),
    }
}

fn gen_cursor_data() -> CursorData {
    CursorData {
        id: 1,
        class: -5,
        value: 127,
        hash: exonum_crypto::hash(&[1, 2, 3]),
    }
}

fn gen_branch_node_data() -> BranchNode {
    BranchNode::empty()
}

fn bench_binary_value<F, V>(c: &mut Criterion, name: &str, f: F)
where
    F: Fn() -> V + 'static + Clone + Copy,
    V: BinaryValue + UniqueHash + PartialEq + Debug,
{
    // Checks that binary value is correct.
    let val = f();
    let bytes = val.to_bytes();
    let val2 = V::from_bytes(bytes.into()).unwrap();
    assert_eq!(val, val2);
    // Runs benchmarks.
    c.bench_function(
        &format!("encoding/{}/to_bytes", name),
        move |b: &mut Bencher| {
            b.iter_with_setup(f, |data| black_box(data.to_bytes()));
        },
    );
    c.bench_function(
        &format!("encoding/{}/into_bytes", name),
        move |b: &mut Bencher| {
            b.iter_with_setup(f, |data| black_box(data.into_bytes()));
        },
    );
    c.bench_function(
        &format!("encoding/{}/from_bytes", name),
        move |b: &mut Bencher| {
            b.iter_with_setup(
                || {
                    let val = f();
                    val.to_bytes()
                },
                |bytes| black_box(V::from_bytes(bytes.into()).unwrap()),
            );
        },
    );
    c.bench_function(
        &format!("encoding/{}/hash", name),
        move |b: &mut Bencher| {
            b.iter_with_setup(f, |data| black_box(data.hash()));
        },
    );
}

fn bench_binary_key_concat(b: &mut Bencher) {
    b.iter_with_setup(
        || ("prefixed.key", Hash::zero(), ProofPath::new(&Hash::zero())),
        |(prefix, key, path)| {
            let mut v = vec![0; prefix.size() + key.size() + path.size()];
            let mut pos = prefix.write(&mut v);
            pos += key.write(&mut v[pos..]);
            path.write(&mut v[pos..]);
            black_box(v);
        },
    );
}

pub fn bench_encoding(c: &mut Criterion) {
    exonum_crypto::init();
    bench_binary_value(c, "bytes", gen_bytes_data);
    bench_binary_value(c, "simple", gen_sample_data);
    bench_binary_value(c, "cursor", gen_cursor_data);
    bench_binary_value(c, "branch_node", gen_branch_node_data);
    c.bench_function("encoding/storage_key/concat", bench_binary_key_concat);
}

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

//! Property testing for proofs of existence / absence in `ProofMapIndex`.
//!
//! To adjust the number of test cases for each test, set the `PROPTEST_CASES` environment
//! variable as per `proptest` docs. The number of test cases for large tests will be scaled
//! back automatically. A reasonable value for `PROPTEST_CASES` is `256`
//! (default; results in running time ~30 sec for larger tests) or more. The run time
//! scales linearly with the number of cases.

// cspell:ignore proptest

use exonum_merkledb::{
    proof_map_index::ProofPath, BinaryKey, Database, IndexAccess, MapProof, ProofMapIndex,
    TemporaryDB,
};
use proptest::{
    prelude::prop::{
        array,
        collection::{btree_map, vec},
    },
    prelude::*,
    test_runner::{Config, TestCaseError, TestCaseResult},
};

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Debug,
    ops::{Range, RangeInclusive},
};

use exonum_merkledb::{BinaryValue, ObjectHash};

const INDEX_NAME: &str = "index";

type Data = BTreeMap<[u8; 32], u64>;

fn check_map_proof<T, K, V>(
    proof: &MapProof<K, V>,
    key: Option<K>,
    table: &ProofMapIndex<T, K, V>,
) -> TestCaseResult
where
    T: IndexAccess,
    K: BinaryKey + ObjectHash + PartialEq + Debug,
    V: BinaryValue + PartialEq + Debug,
{
    let entry = key.map(|key| {
        let value = table.get(&key).unwrap();
        (key, value)
    });
    let proof = proof
        .check_against_hash(table.object_hash())
        .map_err(|e| TestCaseError::fail(e.to_string()))?;
    prop_assert!(proof.entries().eq(entry.as_ref().map(|(k, v)| (k, v))));
    Ok(())
}

fn check_map_multiproof<T, K, V>(
    proof: &MapProof<K, V>,
    keys: BTreeSet<&K>,
    table: &ProofMapIndex<T, K, V>,
) -> TestCaseResult
where
    T: IndexAccess,
    K: BinaryKey + ObjectHash + PartialEq + Debug,
    V: BinaryValue + PartialEq + Debug,
{
    let mut entries: Vec<(&K, V)> = Vec::new();
    let mut missing_keys: Vec<&K> = Vec::new();

    for key in keys {
        if table.contains(key) {
            let value = table.get(key).unwrap();
            entries.push((key, value));
        } else {
            missing_keys.push(key);
        }
    }

    // Sort entries and missing keys by the order imposed by the `ProofPath`
    // serialization of the keys
    entries.sort_unstable_by(|(x, _), (y, _)| {
        ProofPath::new(*x).partial_cmp(&ProofPath::new(*y)).unwrap()
    });
    missing_keys
        .sort_unstable_by(|&x, &y| ProofPath::new(x).partial_cmp(&ProofPath::new(y)).unwrap());

    let unchecked_proof = proof;
    let proof = proof
        .check()
        .map_err(|e| TestCaseError::fail(e.to_string()))?;
    prop_assert!(proof
        .all_entries()
        .eq(unchecked_proof.all_entries_unchecked()));
    prop_assert_eq!(proof.index_hash(), table.object_hash());

    let mut actual_keys: Vec<&K> = proof.missing_keys().collect();
    actual_keys
        .sort_unstable_by(|&x, &y| ProofPath::new(x).partial_cmp(&ProofPath::new(y)).unwrap());
    prop_assert_eq!(missing_keys, actual_keys);

    let mut actual_entries: Vec<(&K, &V)> = proof.entries().collect();
    actual_entries.sort_unstable_by(|&(x, _), &(y, _)| {
        ProofPath::new(x).partial_cmp(&ProofPath::new(y)).unwrap()
    });
    prop_assert!(entries.iter().map(|(k, v)| (*k, v)).eq(actual_entries));
    Ok(())
}

/// Writes raw data to a database.
fn write_data(db: &TemporaryDB, data: Data) {
    let fork = db.fork();
    {
        let mut table = ProofMapIndex::new(INDEX_NAME, &fork);
        table.clear();
        for (key, value) in data {
            table.put(&key, value);
        }
    }
    db.merge(fork.into_patch()).unwrap();
}

/// Creates data for a random-filled `ProofMapIndex<_, [u8; 32], u64>`.
fn index_data(
    key_bytes: impl Strategy<Value = u8>,
    sizes: Range<usize>,
) -> impl Strategy<Value = Data> {
    btree_map(array::uniform32(key_bytes), any::<u64>(), sizes)
}

/// Generates data to test a proof of presence.
fn data_for_proof_of_presence(
    key_bytes: impl Strategy<Value = u8>,
    sizes: Range<usize>,
) -> impl Strategy<Value = ([u8; 32], Data)> {
    index_data(key_bytes, sizes)
        .prop_flat_map(|data| (0..data.len(), Just(data)))
        .prop_map(|(index, data)| (*data.keys().nth(index).unwrap(), data))
}

fn data_for_multiproof(
    key_bytes: impl Strategy<Value = u8>,
    sizes: Range<usize>,
) -> impl Strategy<Value = (Vec<[u8; 32]>, Data)> {
    index_data(key_bytes, sizes)
        .prop_flat_map(|data| (vec(0..data.len(), data.len() / 5), Just(data)))
        .prop_map(|(indexes, data)| {
            // Note that keys may coincide; this is intentional.
            let keys: Vec<_> = indexes
                .into_iter()
                .map(|i| *data.keys().nth(i).unwrap())
                .collect();
            (keys, data)
        })
}

fn test_proof(db: &TemporaryDB, key: [u8; 32]) -> TestCaseResult {
    let snapshot = db.snapshot();
    let table: ProofMapIndex<_, [u8; 32], u64> = ProofMapIndex::new(INDEX_NAME, &snapshot);
    let proof = table.get_proof(key);
    let expected_key = if table.contains(&key) {
        Some(key)
    } else {
        None
    };
    check_map_proof(&proof, expected_key, &table)
}

fn test_multiproof(db: &TemporaryDB, keys: &[[u8; 32]]) -> TestCaseResult {
    let snapshot = db.snapshot();
    let table: ProofMapIndex<_, [u8; 32], u64> = ProofMapIndex::new(INDEX_NAME, &snapshot);
    let proof = table.get_multiproof(keys.to_vec());
    let unique_keys: BTreeSet<_> = keys.iter().collect();
    check_map_multiproof(&proof, unique_keys, &table)
}

#[derive(Debug, Clone)]
struct TestParams {
    key_bytes: RangeInclusive<u8>,
    index_sizes: Range<usize>,
    test_cases_divider: u32,
}

impl TestParams {
    fn key_bytes(&self) -> RangeInclusive<u8> {
        self.key_bytes.clone()
    }

    fn index_sizes(&self) -> Range<usize> {
        self.index_sizes.clone()
    }

    fn config(&self) -> Config {
        Config::with_cases(Config::default().cases / self.test_cases_divider)
    }

    fn proof_of_presence(&self) {
        let db = TemporaryDB::new();
        let strategy = data_for_proof_of_presence(self.key_bytes(), self.index_sizes());
        proptest!(self.config(), |((key, data) in strategy)| {
            write_data(&db, data);
            test_proof(&db, key)?;
        });
    }

    fn proof_of_absence(&self) {
        let db = TemporaryDB::new();
        let key_strategy = array::uniform32(self.key_bytes());
        let data_strategy = index_data(self.key_bytes(), self.index_sizes());
        proptest!(self.config(), |(key in key_strategy, data in data_strategy)| {
            write_data(&db, data);
            test_proof(&db, key)?;
        });
    }

    fn multiproof_of_existing_elements(&self) {
        let db = TemporaryDB::new();
        let strategy = data_for_multiproof(self.key_bytes(), self.index_sizes());
        proptest!(self.config(), |((keys, data) in strategy)| {
            write_data(&db, data);
            test_multiproof(&db, &keys)?;
        });
    }

    fn multiproof_of_absent_elements(&self) {
        let db = TemporaryDB::new();
        let keys_strategy = vec(array::uniform32(self.key_bytes()), 20);
        let data_strategy = index_data(self.key_bytes(), self.index_sizes());
        proptest!(self.config(), |(keys in keys_strategy, data in data_strategy)| {
            write_data(&db, data);
            test_multiproof(&db, &keys)?;
        });
    }

    fn mixed_multiproof(&self) {
        let db = TemporaryDB::new();
        let strategy = data_for_multiproof(self.key_bytes(), self.index_sizes());
        let absent_keys_strategy = vec(array::uniform32(self.key_bytes()), 20);
        proptest!(
            self.config(),
            |((mut keys, data) in strategy, absent_keys in absent_keys_strategy)| {
                write_data(&db, data);
                keys.extend_from_slice(&absent_keys);
                test_multiproof(&db, &keys)?;
            }
        );
    }
}

mod small_index {
    use super::*;

    const PARAMS: TestParams = TestParams {
        key_bytes: 0..=255,
        index_sizes: 10..100,
        test_cases_divider: 1,
    };

    #[test]
    fn proof_of_presence() {
        PARAMS.proof_of_presence();
    }

    #[test]
    fn proof_of_absence() {
        PARAMS.proof_of_absence();
    }

    #[test]
    fn multiproof_of_existing_elements() {
        PARAMS.multiproof_of_existing_elements();
    }

    #[test]
    fn multiproof_of_absent_elements() {
        PARAMS.multiproof_of_absent_elements();
    }

    #[test]
    fn mixed_multiproof() {
        PARAMS.mixed_multiproof();
    }
}

mod small_index_skewed {
    use super::*;

    const PARAMS: TestParams = TestParams {
        key_bytes: 0..=2,
        index_sizes: 10..100,
        test_cases_divider: 1,
    };

    #[test]
    fn proof_of_presence() {
        PARAMS.proof_of_presence();
    }

    #[test]
    fn proof_of_absence() {
        PARAMS.proof_of_absence();
    }

    #[test]
    fn multiproof_of_existing_elements() {
        PARAMS.multiproof_of_existing_elements();
    }

    #[test]
    fn multiproof_of_absent_elements() {
        PARAMS.multiproof_of_absent_elements();
    }

    #[test]
    fn mixed_multiproof() {
        PARAMS.mixed_multiproof();
    }
}

mod large_index {
    use super::*;

    const PARAMS: TestParams = TestParams {
        key_bytes: 0..=255,
        index_sizes: 5_000..10_000,
        test_cases_divider: 32,
    };

    #[test]
    fn proof_of_presence() {
        PARAMS.proof_of_presence();
    }

    #[test]
    fn proof_of_absence() {
        PARAMS.proof_of_absence();
    }

    #[test]
    fn multiproof_of_existing_elements() {
        PARAMS.multiproof_of_existing_elements();
    }

    #[test]
    fn multiproof_of_absent_elements() {
        PARAMS.multiproof_of_absent_elements();
    }

    #[test]
    fn mixed_multiproof() {
        PARAMS.mixed_multiproof();
    }
}

mod large_index_skewed {
    use super::*;

    const PARAMS: TestParams = TestParams {
        key_bytes: 0..=2,
        index_sizes: 5_000..10_000,
        test_cases_divider: 32,
    };

    #[test]
    fn proof_of_presence() {
        PARAMS.proof_of_presence();
    }

    #[test]
    fn proof_of_absence() {
        PARAMS.proof_of_absence();
    }

    #[test]
    fn multiproof_of_existing_elements() {
        PARAMS.multiproof_of_existing_elements();
    }

    #[test]
    fn multiproof_of_absent_elements() {
        PARAMS.multiproof_of_absent_elements();
    }

    #[test]
    fn mixed_multiproof() {
        PARAMS.mixed_multiproof();
    }
}

//TODO: change uncomment
//#[test]
//fn map_proof_serialize() {
//    let db = TemporaryDB::default();
//    let storage = db.fork();

//    let mut table = ProofMapIndex::new("index", &storage);

//    let proof = table.get_proof(0);
//    assert_proof_roundtrip(proof);

//    for i in 0..10 {
//        table.put(&i, i);
//    }

//    let proof = table.get_proof(5);
//    assert_proof_roundtrip(proof);

//    let proof = table.get_multiproof(5..15);
//    assert_proof_roundtrip(proof);
//}

//fn assert_proof_roundtrip<K, V>(proof: MapProof<K, V>)
//    where
//        K: BinaryKey + ObjectHash + fmt::Debug,
//        V: BinaryValue + ObjectHash + fmt::Debug,
//        MapProof<K, V>: ProtobufConvert + PartialEq,
//{
//    let pb = proof.to_pb();
//    let deserialized: MapProof<K, V> = MapProof::from_pb(pb).unwrap();
//    let checked_proof = deserialized
//        .check()
//        .expect("deserialized proof is not valid");

//    assert_eq!(proof, deserialized);
//    assert_eq!(
//        checked_proof.index_hash(),
//        proof.check().unwrap().index_hash()
//    );
//}

//#[test]
//fn map_proof_malformed_serialize() {
//    use self::schema::proof::{MapProof, MapProofEntry, OptionalEntry};
//    let mut proof = MapProof::new();
//    let mut proof_entry = MapProofEntry::new();
//    proof_entry.set_proof_path(vec![0_u8; 33]);
//    proof.set_proof(RepeatedField::from_vec(vec![proof_entry]));

//    let res = exonum_merkledb::MapProof::<u8, u8>::from_pb(proof.clone());
//    assert!(res
//        .unwrap_err()
//        .to_string()
//        .contains("Not valid proof path"));

//    let mut proof_entry = MapProofEntry::new();
//    let mut hash = schema::helpers::Hash::new();
//    hash.set_data(vec![0_u8; 31]);
//    proof_entry.set_hash(hash);
//    proof_entry.set_proof_path(vec![0_u8; 34]);
//    proof.set_proof(RepeatedField::from_vec(vec![proof_entry]));

//    let res = exonum_merkledb::MapProof::<u8, u8>::from_pb(proof.clone());
//    assert!(res.unwrap_err().to_string().contains("Wrong Hash size"));

//    let mut entry = OptionalEntry::new();
//    entry.set_key(vec![0_u8; 31]);
//    proof.clear_proof();
//    proof.set_entries(RepeatedField::from_vec(vec![entry]));

//    // TODO: will panic at runtime, should change BinaryKey::read signature
//    let _res = exonum_merkledb::MapProof::<crypto::PublicKey, u8>::from_pb(proof.clone());
//}
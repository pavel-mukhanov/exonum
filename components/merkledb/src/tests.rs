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

use exonum_crypto::Hash;

use rocksdb::{Options, DB};

use crate::{RocksDB, DbOptions, Database,
    Entry, Fork, KeySetIndex, ListIndex, MapIndex, ProofListIndex, ProofMapIndex, SparseListIndex,
    ValueSetIndex,
};
use std::path::PathBuf;

// This should compile to ensure ?Sized bound on `new_in_family` (see #1024).
#[allow(dead_code, unreachable_code, unused_variables)]
fn should_compile() {
    let fork: Fork = unimplemented!();
    let _: Entry<_, ()> = Entry::new_in_family("", "", &fork);
    let _: KeySetIndex<_, Hash> = KeySetIndex::new_in_family("", "", &fork);
    let _: ListIndex<_, ()> = ListIndex::new_in_family("", "", &fork);
    let _: MapIndex<_, Hash, ()> = MapIndex::new_in_family("", "", &fork);
    let _: ProofListIndex<_, ()> = ProofListIndex::new_in_family("", "", &fork);
    let _: ProofMapIndex<_, Hash, ()> = ProofMapIndex::new_in_family("", "", &fork);
    let _: SparseListIndex<_, ()> = SparseListIndex::new_in_family("", "", &fork);
    let _: ValueSetIndex<_, ()> = ValueSetIndex::new_in_family("", "", &fork);
}

//#[test]
fn db_options_rocks() {
    let path = PathBuf::from("./db/");
    let mut options = Options::default();
    options.set_write_buffer_size(69108866);
    options.create_if_missing(true);

    let db = DB::open(&options, path);

    dbg!(db);
}

#[test]
fn db_options() {
    let path = PathBuf::from("./db/");
    let mut options = DbOptions::default();

    let db = RocksDB::open(path, &options);

    dbg!(db);
}

#[test]
fn pool_in_family() {
    env_logger::try_init();
    let path = PathBuf::from("./db/");
    let mut options = DbOptions::default();

    let db = RocksDB::open(path, &options).unwrap();
    let fork = db.fork();
    {
        let mut pool = KeySetIndex::new("transactions_pool", &fork);

        pool.insert(1);
    }

    db.merge(fork.into_patch());

}

/*
min: 10, max: 482, avrg: 305, current: 0, last height: 615   64 mb
min: 105, max: 513, avrg: 339, current: 0, last height: 759   128 mb
min: 4, max: 503, avrg: 331, current: 0, last height: 274 192 mb
min: 189, max: 519, avrg: 346, current: 0, last height: 256 mb
min: 172, max: 510, avrg: 343, current: 0, last height: 320 mb
min: 79, max: 512, avrg: 336, current: 0, last height: 384 mb
*/
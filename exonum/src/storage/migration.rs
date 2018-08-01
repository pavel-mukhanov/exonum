use std::rc::Rc;
use std::cell::RefCell;
use storage::Fork;
use storage::Snapshot;
use storage::Iter;
use storage::ListIndex;
use storage::StorageValue;
use storage::base_index::BaseIndex;
use storage::StorageKey;

#[derive(Debug)]
pub struct MigrationFork {
    fork : Rc<RefCell<Fork>>,
}

impl MigrationFork {
    fn fork(&self) -> Rc<RefCell<Fork>> {
        self.fork.clone()
    }
}

impl Snapshot for MigrationFork {
    fn get(&self, name: &str, key: &[u8]) -> Option<Vec<u8>> {
        unimplemented!()
    }

    fn contains(&self, name: &str, key: &[u8]) -> bool {
        unimplemented!()
    }

    fn iter<'a>(&'a self, name: &str, from: &[u8]) -> Iter<'a> {
        unimplemented!()
    }
}

impl AsRef<dyn Snapshot> for MigrationFork {
    fn as_ref(&self) -> &dyn Snapshot {
        self
    }
}

impl<'a, V> ListIndex<&'a MigrationFork, V>
    where
        V: StorageValue,
{

    pub fn push(&mut self, value: V) {
        let len = self.len();
        self.base.put(&len, value);
//        self.set_len(len + 1)
    }
}


impl<'a> BaseIndex<&'a MigrationFork> {

    /// Inserts the key-value pair into the index. Both key and value may be of *any* types.
    pub fn put<K, V>(&mut self, key: &K, value: V)
        where
            K: StorageKey,
            V: StorageValue,
    {
//        self.set_index_type();
//        let key = self.prefixed_key(key);
//        self.view.put(&self.name, key, value.into_bytes());
    }
}

mod tests {
    use tempdir::TempDir;
    use storage::migration::MigrationFork;
    use std::rc::Rc;
    use std::cell::RefCell;
    use std::path::Path;
    use storage::RocksDB;
    use storage::DbOptions;
    use storage::db::Database;
    use storage::ListIndex;

    #[test]
    fn test_rocksdb_multiple_index() {
        let dir = TempDir::new("exonum_rocksdb2").unwrap();
        let path = dir.path();

        let rocksdb = rocksdb_database(path);
        let fork = rocksdb.fork();

        let mut fork = MigrationFork { fork : Rc::new(RefCell::new(fork)) };

        let mut index = ListIndex::new("list_index", &fork.fork());

        index.push("string".to_string());

//        println!("index 0 {:?}", index.get(0));
    }

    fn rocksdb_database(path: &Path) -> RocksDB {
        let options = DbOptions::default();
        RocksDB::open(path, &options).unwrap()
    }
}

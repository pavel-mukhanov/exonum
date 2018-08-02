use std::cell::RefCell;
use std::rc::Rc;
use storage::base_index::BaseIndex;
use storage::indexes_metadata;
use storage::Fork;
use storage::Iter;
use storage::ListIndex;
use storage::Patch;
use storage::Snapshot;
use storage::StorageKey;
use storage::StorageValue;

#[derive(Debug, Clone)]
pub struct MigrationFork {
    fork: Rc<RefCell<Fork>>,
}

impl MigrationFork {
    fn fork(&self) -> Rc<RefCell<Fork>> {
        self.fork.clone()
    }

    fn into_patch(self) -> Patch {
        // To succeed `try_unwrap` there must be only one strong reference to `fork`.
        let fork = Rc::try_unwrap(self.fork).unwrap();
        fork.into_inner().into_patch()
    }
}

impl Snapshot for MigrationFork {
    fn get(&self, name: &str, key: &[u8]) -> Option<Vec<u8>> {
        self.fork.borrow().get(name, key)
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
    fn set_len(&mut self, len: u64) {
        self.base.put(&(), len);
        self.length.set(Some(len));
    }

    pub fn push(&mut self, value: V) {
        let len = self.len();
        self.base.put(&len, value);
        self.set_len(len + 1)
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
        let key = self.prefixed_key(key);
        self.view
            .fork
            .borrow_mut()
            .put(&self.name, key, value.into_bytes());
    }
}

mod tests {
    use tempdir::TempDir;

    use std::{cell::RefCell, path::Path, rc::Rc};

    use crypto::{self, PublicKey};
    use storage::Fork;
    use storage::Snapshot;
    use storage::{db::Database, migration::MigrationFork, DbOptions, ListIndex, RocksDB};

    fn rocksdb_database(path: &Path) -> RocksDB {
        let options = DbOptions::default();
        RocksDB::open(path, &options).unwrap()
    }

    #[test]
    fn test_rocksdb_multiple_index() {
        let dir = TempDir::new("exonum_rocksdb2").unwrap();
        let path = dir.path();

        let rocksdb = rocksdb_database(path);
        let fork = rocksdb.fork();

        let fork = MigrationFork {
            fork: Rc::new(RefCell::new(fork)),
        };

        let mut index1 = ListIndex::new("list_index1", &fork);
        let mut index2 = ListIndex::new("list_index2", &fork);

        let data = String::from("important data");
        index1.push(data.clone());

        let data_from_db = index1.get(0).unwrap();
        index2.push(data_from_db);

        assert_eq!(data, index2.get(0).unwrap());
    }

    #[test]
    fn test_rocksdb_multiple_index_merge() {
        let dir = TempDir::new("exonum_rocksdb2").unwrap();
        let path = dir.path();

        let rocksdb = rocksdb_database(path);

        let fork = rocksdb.fork();

        let migration_fork = MigrationFork {
            fork: Rc::new(RefCell::new(fork)),
        };

        {
            let mut index1 = ListIndex::new("list_index1", &migration_fork);
            index1.push("data".to_string());
        }

        let patch = migration_fork.into_patch();

        rocksdb.merge(patch);
    }

    encoding_struct! {
        struct Wallet {
            pub_key:            &PublicKey,
            balance:            u64,
        }
    }

    impl Wallet {
        pub fn set_balance(self, balance: u64) -> Self {
            Self::new(self.pub_key(), balance)
        }
    }

    encoding_struct! {
        struct NewWallet {
            pub_key:            &PublicKey,
            balance:            u64,
            name:               &str,
        }
    }

    impl NewWallet {
        pub fn set_balance(self, balance: u64) -> Self {
            Self::new(self.pub_key(), balance, self.name())
        }
    }

    #[derive(Debug)]
    pub struct CurrencySchema<T> {
        view: T,
    }

    impl<T> AsMut<T> for CurrencySchema<T> {
        fn as_mut(&mut self) -> &mut T {
            &mut self.view
        }
    }

    impl<T> CurrencySchema<T>
        where
            T: AsRef<dyn Snapshot>,
    {
        pub fn new(view: T) -> Self {
            CurrencySchema { view }
        }

        pub fn wallets(&self) -> ListIndex<&T, Wallet> {
            ListIndex::new("wallets", &self.view)
        }

        pub fn wallet(&self, index: u64) -> Option<Wallet> {
            self.wallets().get(index)
        }
    }

    impl<'a> CurrencySchema<&'a mut Fork> {
        pub fn wallets_mut(&mut self) -> ListIndex<&mut Fork, Wallet> {
            ListIndex::new("wallets", &mut self.view)
        }

        pub fn create_wallet(&mut self, key: &PublicKey) {
            let mut wallets = self.wallets_mut();

            let wallet = Wallet::new(key, 0);
            wallets.push(wallet);
        }
    }

    impl<'a> CurrencySchema<&'a MigrationFork> {
        pub fn wallets_mut(&self) -> ListIndex<&MigrationFork, Wallet> {
            ListIndex::new("wallets", &self.view)
        }
    }

    #[derive(Debug)]
    pub struct NewCurrencySchema<T> {
        view: T,
    }

    impl<T> AsMut<T> for NewCurrencySchema<T> {
        fn as_mut(&mut self) -> &mut T {
            &mut self.view
        }
    }

    impl<T> NewCurrencySchema<T>
        where
            T: AsRef<dyn Snapshot>,
    {
        pub fn new(view: T) -> Self {
            NewCurrencySchema { view }
        }
    }

    // We need to create `MigrationFork` impls for old and new schema's.
    // It will lead to duplicate code.
    impl<'a> NewCurrencySchema<&'a MigrationFork> {
        pub fn wallets_mut(&self) -> ListIndex<&MigrationFork, NewWallet> {
            ListIndex::new("wallets", &self.view)
        }

        pub fn create_wallet(&mut self, key: &PublicKey, balance: u64, name: &str) {
            let mut wallets = self.wallets_mut();

            let wallet = NewWallet::new(key, balance, name);
            wallets.push(wallet);
        }
    }

    #[test]
    fn test_rocksdb_migrate() {
        let dir = TempDir::new("exonum_rocksdb1").unwrap();
        let path = dir.path();

        let rocksdb = rocksdb_database(path);

        {
            let mut fork = {
                let mut fork = rocksdb.fork();
                {
                    let mut schema = CurrencySchema::new(&mut fork);
                    let (public_key, _) = crypto::gen_keypair();
                    schema.create_wallet(&public_key);
                }
                fork.into_patch()
            };

            rocksdb.merge_sync(fork);
        }

        let fork = rocksdb.fork();

        let fork = MigrationFork {
            fork: Rc::new(RefCell::new(fork)),
        };

        let mut old_schema = CurrencySchema::new(&fork);
        let mut new_schema = NewCurrencySchema::new(&fork);

        let old_wallet = old_schema.wallet(0).unwrap();

        new_schema.create_wallet(
            &old_wallet.pub_key(),
            old_wallet.balance(),
            "new wallet name",
        );
        let new_wallet = new_schema.wallets_mut().get(0).unwrap();

        assert_eq!(old_wallet.pub_key(), new_wallet.pub_key());
    }

}

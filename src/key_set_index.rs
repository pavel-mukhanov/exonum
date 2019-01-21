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

//! An implementation of a set for items that utilize the `BinaryKey` trait.
//!
//! `KeySetIndex` implements a set that stores elements as keys with empty values.
//! The given section contains information on the methods related to `KeySetIndex`
//! and the iterator over the items of this set.

use std::{borrow::Borrow, marker::PhantomData};

use crate::{
    views::{Iter as ViewIter, View, IndexAccess}, Fork, BinaryKey,
};
use crate::views::IndexAddress;
use crate::views::Mount;

/// A set of key items.
///
/// `KeySetIndex` implements a set that stores the elements as keys with empty values.
/// `KeySetIndex` requires that elements should implement the [`BinaryKey`] trait.
///
/// [`BinaryKey`]: ../trait.BinaryKey.html
#[derive(Debug)]
pub struct KeySetIndex<T: IndexAccess, K> {
    base: View<T>,
    _k: PhantomData<K>,
}

/// Returns an iterator over the items of a `KeySetIndex`.
///
/// This struct is created by the [`iter`] or
/// [`iter_from`] method on [`KeySetIndex`]. See its documentation for details.
///
/// [`iter`]: struct.KeySetIndex.html#method.iter
/// [`iter_from`]: struct.KeySetIndex.html#method.iter_from
/// [`KeySetIndex`]: struct.KeySetIndex.html
#[derive(Debug)]
pub struct KeySetIndexIter<'a, K> {
    base_iter: ViewIter<'a, K, ()>,
}

impl<T, K> KeySetIndex<T, K>
    where
        T: IndexAccess,
        K: BinaryKey,
{

    pub fn new<S: AsRef<str>>(index_name: S, view: T) -> Self {

        Self {
            base: Mount::new(view).mount(index_name),
            _k: PhantomData,
        }
    }

    /// Returns `true` if the set contains the indicated value.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum::storage::{MemoryDB, Database, KeySetIndex};
    ///
    /// let db = MemoryDB::new();
    /// let name = "name";
    /// let mut fork = db.fork();
    /// let mut index = KeySetIndex::new(name, &mut fork);
    /// assert!(!index.contains(&1));
    ///
    /// index.insert(1);
    /// assert!(index.contains(&1));
    /// ```
    pub fn contains<Q>(&self, item: &Q) -> bool
        where
            K: Borrow<Q>,
            Q: BinaryKey + ?Sized,
    {
        self.base.contains(item)
    }

    /// Returns an iterator visiting all elements in ascending order. The iterator element type is K.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum::storage::{MemoryDB, Database, KeySetIndex};
    ///
    /// let db = MemoryDB::new();
    /// let name = "name";
    /// let snapshot = db.snapshot();
    /// let index: KeySetIndex<_, u8> = KeySetIndex::new(name, &snapshot);
    ///
    /// for val in index.iter() {
    ///     println!("{}", val);
    /// }
    /// ```
    pub fn iter(&self) -> KeySetIndexIter<K> {
        KeySetIndexIter {
            base_iter: self.base.iter(&()),
        }
    }

    /// Returns an iterator visiting all elements in arbitrary order starting from the specified value.
    /// The iterator element type is K.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum::storage::{MemoryDB, Database, KeySetIndex};
    ///
    /// let db = MemoryDB::new();
    /// let name = "name";
    /// let snapshot = db.snapshot();
    /// let index: KeySetIndex<_, u8> = KeySetIndex::new(name, &snapshot);
    ///
    /// for val in index.iter_from(&2) {
    ///     println!("{}", val);
    /// }
    /// ```
    pub fn iter_from(&self, from: &K) -> KeySetIndexIter<K> {
        KeySetIndexIter {
            base_iter: self.base.iter_from(&(), from),
        }
    }
}

impl<'a, K> KeySetIndex<&'a Fork, K>
    where
        K: BinaryKey,
{
    /// Adds a key to the set.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum::storage::{MemoryDB, Database, KeySetIndex};
    ///
    /// let db = MemoryDB::new();
    /// let name = "name";
    /// let mut fork = db.fork();
    /// let mut index = KeySetIndex::new(name, &mut fork);
    ///
    /// index.insert(1);
    /// assert!(index.contains(&1));
    /// ```
    #[cfg_attr(feature = "cargo-clippy", allow(needless_pass_by_value))]
    pub fn insert(&mut self, item: K) {
        self.base.put(&item, ())
    }

    /// Removes a key from the set.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum::storage::{MemoryDB, Database, KeySetIndex};
    ///
    /// let db = MemoryDB::new();
    /// let name = "name";
    /// let mut fork = db.fork();
    /// let mut index = KeySetIndex::new(name, &mut fork);
    ///
    /// index.insert(1);
    /// assert!(index.contains(&1));
    ///
    /// index.remove(&1);
    /// assert!(!index.contains(&1));
    /// ```
    pub fn remove<Q>(&mut self, item: &Q)
        where
            K: Borrow<Q>,
            Q: BinaryKey + ?Sized,
    {
        self.base.remove(item)
    }

    /// Clears the set, removing all values.
    ///
    /// # Notes
    /// Currently, this method is not optimized to delete a large set of data. During the execution of
    /// this method, the amount of allocated memory is linearly dependent on the number of elements
    /// in the index.
    ///
    /// # Examples
    ///
    /// ```
    /// use exonum::storage::{MemoryDB, Database, KeySetIndex};
    ///
    /// let db = MemoryDB::new();
    /// let name = "name";
    /// let mut fork = db.fork();
    /// let mut index = KeySetIndex::new(name, &mut fork);
    ///
    /// index.insert(1);
    /// assert!(index.contains(&1));
    ///
    /// index.clear();
    /// assert!(!index.contains(&1));
    /// ```
    pub fn clear(&mut self) {
        self.base.clear()
    }
}

impl<'r, T, K> ::std::iter::IntoIterator for &'r KeySetIndex<T, K>
    where
        T: IndexAccess,
        K: BinaryKey,
{
    type Item = K::Owned;
    type IntoIter = KeySetIndexIter<'r, K>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, K> Iterator for KeySetIndexIter<'a, K>
    where
        K: BinaryKey,
{
    type Item = K::Owned;

    fn next(&mut self) -> Option<Self::Item> {
        self.base_iter.next().map(|(k, ..)| k)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Database, TemporaryDB};

    const INDEX_NAME: &str = "test_index_name";

    #[test]
    fn str_key() {
        let db = TemporaryDB::new();
        let fork = db.fork();
        let mut index: KeySetIndex<_, String> = KeySetIndex::new(INDEX_NAME, &fork);

        const KEY: &str = "key_1";

        assert_eq!(false, index.contains(KEY));

        index.insert(KEY.to_owned());
        assert_eq!(true, index.contains(KEY));

        index.remove(KEY);
        assert_eq!(false, index.contains(KEY));
    }
//
//    #[test]
//    fn u8_slice_key() {
//        let db = MemoryDB::new();
//        let fork = db.fork();
//
//        const KEY: &[u8] = &[1, 2, 3];
//
//        let mut index: KeySetIndex<_, Vec<u8>> = fork.mount_root().named_child(INDEX_NAME).mount();
//        assert_eq!(false, index.contains(KEY));
//
//        index.insert(KEY.to_owned());
//        assert_eq!(true, index.contains(KEY));
//
//        index.remove(KEY);
//        assert_eq!(false, index.contains(KEY));
//    }
}

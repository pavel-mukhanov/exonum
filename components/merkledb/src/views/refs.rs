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

use std::ops::{Deref, DerefMut};

use crate::{
    views::{IndexAddress, IndexType, View},
    BinaryKey, BinaryValue, Fork, IndexAccess, ObjectHash, Snapshot,
};

pub trait AnyObject<'a, T: IndexAccess<'a>> {
    fn view(self) -> View<'a, T>;
    fn object_type(&self) -> IndexType;
    fn metadata(&self) -> Vec<u8>;
}

pub trait FromView<'a, T: IndexAccess<'a>>
where
    Self: Sized,
{
    fn create<I: Into<IndexAddress>>(address: I, access: T) -> Self;
    fn get<I: Into<IndexAddress>>(address: I, access: T) -> Option<Self>;
}

/// Trait used to obtain references to database objects.
pub trait ObjectAccess<'a>: IndexAccess<'a> {
    /// Returns an immutable reference to an existing database object or `None` if
    /// the object with provided `address` is not found.
    ///
    /// ```
    /// use exonum_merkledb::{Database, TemporaryDB, ObjectAccess, ListIndex, Ref};
    ///
    /// let db = TemporaryDB::new();
    /// let snapshot = &db.snapshot();
    ///
    /// let index: Option<Ref<ListIndex<_, u8>>> = snapshot.get_object_existed("index");
    /// assert!(index.is_none());
    /// ```
    fn get_object_existed<I, T>(&self, address: I) -> Option<Ref<T>>
    where
        I: Into<IndexAddress>,
        T: FromView<'a, Self>,
    {
        T::get(address, self.clone()).map(|value| Ref { value })
    }

    /// Returns a mutable reference to an existing database object or `None` if
    /// the object with provided `address` is not found.
    ///
    /// ```
    /// use exonum_merkledb::{Database, TemporaryDB, ListIndex, RefMut};
    ///
    /// let db = TemporaryDB::new();
    /// let fork = db.fork();
    ///
    /// let index: Option<RefMut<ListIndex<_, u8>>> = fork.get_object_existed_mut("index");
    /// ```
    fn get_object_existed_mut<T, I>(&self, address: I) -> Option<RefMut<T>>
    where
        T: FromView<'a, Self>,
        I: Into<IndexAddress>,
    {
        T::get(address, self.clone()).map(|value| RefMut { value })
    }

    /// Returns a mutable reference to a database object. If the object does not exist, the method
    /// creates it.
    ///
    /// ```
    /// use exonum_merkledb::{Database, TemporaryDB, ListIndex, Ref, RefMut};
    ///
    /// let db = TemporaryDB::new();
    /// let fork = db.fork();
    ///
    /// let mut index: RefMut<ListIndex<_, u8>> = fork.get_object("index");
    /// index.push(1);
    /// ```
    fn get_object<I, T>(&self, address: I) -> RefMut<T>
    where
        I: Into<IndexAddress>,
        T: FromView<'a, Self>,
    {
        let address = address.into();
        let object = T::get(address.clone(), self.clone()).map(|value| RefMut { value });

        match object {
            Some(object) => object,
            _ => RefMut {
                value: T::create(address, self.clone()),
            },
        }
    }
}
//TODO: revert
//impl ObjectAccess<'_> for &Box<dyn Snapshot> {}
//
//impl ObjectAccess<'_> for &Fork<'_> {}
//
//impl<T> ObjectAccess<'_> for T where T: Deref<Target = dyn Snapshot> + Clone {}
//
//impl Fork<'_> {
//    /// See: [ObjectAccess::get_object][1].
//    ///
//    /// [1]: trait.ObjectAccess.html#method.get_object
//    pub fn get_object<'a, I, T>(&'a self, address: I) -> RefMut<T>
//    where
//        I: Into<IndexAddress>,
//        T: FromView<'a, &'a Self>,
//    {
//        let address = address.into();
//        let object = T::get(address.clone(), self).map(|value| RefMut { value });
//
//        match object {
//            Some(object) => object,
//            _ => RefMut {
//                value: T::create(address, self),
//            },
//        }
//    }
//
//    /// See: [ObjectAccess::get_object_existed][1].
//    ///
//    /// [1]: trait.ObjectAccess.html#method.get_object_existed
//    pub fn get_object_existed<'a, T, I>(&'a self, address: I) -> Option<Ref<T>>
//    where
//        T: FromView<'a, &'a Self>,
//        I: Into<IndexAddress>,
//    {
//        T::get(address, self).map(|value| Ref { value })
//    }
//
//    /// See: [ObjectAccess::get_object_existed_mut][1].
//    ///
//    /// [1]: trait.ObjectAccess.html#method.get_object_existed_mut
//    pub fn get_object_existed_mut<'a, T, I>(&'a self, address: I) -> Option<RefMut<T>>
//    where
//        T: FromView<'a, &'a Self>,
//        I: Into<IndexAddress>,
//    {
//        T::get(address, self).map(|value| RefMut { value })
//    }
//}

#[derive(Debug)]
/// Utility trait to provide immutable references to `MerkleDB` objects.
/// Similar to `core::cell::Ref`, but with `Deref` implementation.
pub struct Ref<T> {
    value: T,
}

#[derive(Debug)]
/// Utility trait to provide mutable references to `MerkleDB` objects.
pub struct RefMut<T> {
    value: T,
}

impl<T> Deref for Ref<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl<T> Deref for RefMut<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl<T> DerefMut for RefMut<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.value
    }
}

//TODO: revert
#[cfg(test2)]
mod tests {
    use crate::{
        db::Database,
        views::refs::{ObjectAccess, Ref, RefMut},
        ListIndex, TemporaryDB,
    };

    #[test]
    fn basic_object_refs() {
        let db = TemporaryDB::new();
        let fork = db.fork();
        {
            let mut index: RefMut<ListIndex<_, u32>> = fork.get_object("index");
            index.push(1);
        }

        db.merge(fork.into_patch()).unwrap();

        let fork = db.fork();
        {
            let mut index: RefMut<ListIndex<_, u32>> =
                fork.get_object_existed_mut("index").unwrap();
            index.push(2);
        }

        db.merge(fork.into_patch()).unwrap();

        let snapshot = &db.snapshot();
        let index: Ref<ListIndex<_, u32>> = snapshot.get_object_existed("index").unwrap();

        assert_eq!(index.get(0), Some(1));
        assert_eq!(index.get(1), Some(2));
    }

    #[test]
    fn get_non_existent_index() {
        let db = TemporaryDB::new();
        let snapshot = &db.snapshot();
        let index: Option<Ref<ListIndex<_, u32>>> = snapshot.get_object_existed("index");

        assert!(index.is_none());
    }

    #[test]
    fn fork_get_object() {
        let db = TemporaryDB::new();
        let fork = db.fork();
        {
            let _list: RefMut<ListIndex<_, u32>> = fork.get_object("index");
        }

        db.merge(fork.into_patch()).unwrap();

        let fork = db.fork();
        {
            let mut list: RefMut<ListIndex<_, u32>> = fork.get_object("index");
            list.push(1);
        }

        db.merge(fork.into_patch()).unwrap();

        let snapshot = &db.snapshot();
        let list: Ref<ListIndex<_, u32>> = snapshot.get_object_existed("index").unwrap();

        assert_eq!(list.get(0), Some(1));
    }
}

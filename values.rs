// Copyright 2017 The Exonum Team
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

//! A definition of `StorageValue` trait and implementations for common types.
use byteorder::{ByteOrder, LittleEndian};

use std::mem;
use std::borrow::Cow;

use crypto::{Hash, hash, PublicKey};
use messages::{RawMessage, MessageBuffer, Message};

/// A trait that defines serialization of corresponding types as storage values.
///
/// For compatibility with modern architectures the little-endian encoding is used.
///
/// # Examples
///
/// Implementing `StorageValue` for the type:
///
/// ```
/// # extern crate exonum;
/// # extern crate byteorder;
///
/// use std::borrow::Cow;
///
/// use exonum::storage::StorageValue;
/// use exonum::crypto::{self, Hash};
/// use byteorder::{LittleEndian, ByteOrder};
///
/// struct Data {
///     a: i16,
///     b: u32,
/// }
///
/// impl StorageValue for Data {
///     fn into_bytes(self) -> Vec<u8> {
///         let mut buffer = vec![0; 6];
///         LittleEndian::write_i16(&mut buffer[0..2], self.a);
///         LittleEndian::write_u32(&mut buffer[2..6], self.b);
///         buffer
///     }
///
///     fn from_bytes(value: Cow<[u8]>) -> Self {
///         let a = LittleEndian::read_i16(&value[0..2]);
///         let b = LittleEndian::read_u32(&value[2..6]);
///         Data { a, b }
///     }
///
///     fn hash(&self) -> Hash {
///         let mut buffer = [0; 6];
///         LittleEndian::write_i16(&mut buffer[0..2], self.a);
///         LittleEndian::write_u32(&mut buffer[2..6], self.b);
///         crypto::hash(&buffer)
///     }
/// }
/// # fn main() {}
/// ```
pub trait StorageValue: Sized {
    /// Returns a hash of the value.
    ///
    /// This method is actively used to build indices, so the hashing strategy must satisfy
    /// the basic requirements of cryptographic hashing: equal values must have the same hash and
    /// not equal values must have different hashes.
    fn hash(&self) -> Hash;

    /// Serialize a value into a vector of bytes.
    fn into_bytes(self) -> Vec<u8>;

    /// Deserialize a value from bytes.
    fn from_bytes(value: Cow<[u8]>) -> Self;
}

impl StorageValue for () {
    fn into_bytes(self) -> Vec<u8> {
        Vec::new()
    }

    fn from_bytes(_value: Cow<[u8]>) -> Self {
        ()
    }

    fn hash(&self) -> Hash {
        Hash::zero()
    }
}

impl StorageValue for u8 {
    fn into_bytes(self) -> Vec<u8> {
        vec![self]
    }

    fn from_bytes(value: Cow<[u8]>) -> Self {
        assert_eq!(value.len(), 1);
        value[0]
    }

    fn hash(&self) -> Hash {
        hash(&[*self])
    }
}

impl StorageValue for u16 {
    fn into_bytes(self) -> Vec<u8> {
        let mut v = vec![0; 2];
        LittleEndian::write_u16(&mut v, self);
        v
    }

    fn from_bytes(value: Cow<[u8]>) -> Self {
        LittleEndian::read_u16(value.as_ref())
    }

    fn hash(&self) -> Hash {
        let mut v = [0; 2];
        LittleEndian::write_u16(&mut v, *self);
        hash(&v)
    }
}

impl StorageValue for u32 {
    fn into_bytes(self) -> Vec<u8> {
        let mut v = vec![0; 4];
        LittleEndian::write_u32(&mut v, self);
        v
    }

    fn from_bytes(value: Cow<[u8]>) -> Self {
        LittleEndian::read_u32(value.as_ref())
    }

    fn hash(&self) -> Hash {
        let mut v = [0; 4];
        LittleEndian::write_u32(&mut v, *self);
        hash(&v)
    }
}

impl StorageValue for u64 {
    fn into_bytes(self) -> Vec<u8> {
        let mut v = vec![0; mem::size_of::<u64>()];
        LittleEndian::write_u64(&mut v, self);
        v
    }

    fn from_bytes(value: Cow<[u8]>) -> Self {
        LittleEndian::read_u64(value.as_ref())
    }

    fn hash(&self) -> Hash {
        let mut v = [0; 8];
        LittleEndian::write_u64(&mut v, *self);
        hash(&v)
    }
}

impl StorageValue for i8 {
    fn into_bytes(self) -> Vec<u8> {
        vec![self as u8]
    }

    fn from_bytes(value: Cow<[u8]>) -> Self {
        assert_eq!(value.len(), 1);
        value[0] as i8
    }

    fn hash(&self) -> Hash {
        hash(&[*self as u8])
    }
}

impl StorageValue for i16 {
    fn into_bytes(self) -> Vec<u8> {
        let mut v = vec![0; 2];
        LittleEndian::write_i16(&mut v, self);
        v
    }

    fn from_bytes(value: Cow<[u8]>) -> Self {
        LittleEndian::read_i16(value.as_ref())
    }

    fn hash(&self) -> Hash {
        let mut v = [0; 2];
        LittleEndian::write_i16(&mut v, *self);
        hash(&v)
    }
}

impl StorageValue for i32 {
    fn into_bytes(self) -> Vec<u8> {
        let mut v = vec![0; 4];
        LittleEndian::write_i32(&mut v, self);
        v
    }

    fn from_bytes(value: Cow<[u8]>) -> Self {
        LittleEndian::read_i32(value.as_ref())
    }

    fn hash(&self) -> Hash {
        let mut v = [0; 4];
        LittleEndian::write_i32(&mut v, *self);
        hash(&v)
    }
}

impl StorageValue for i64 {
    fn into_bytes(self) -> Vec<u8> {
        let mut v = vec![0; 8];
        LittleEndian::write_i64(&mut v, self);
        v
    }

    fn from_bytes(value: Cow<[u8]>) -> Self {
        LittleEndian::read_i64(value.as_ref())
    }

    fn hash(&self) -> Hash {
        let mut v = [0; 8];
        LittleEndian::write_i64(&mut v, *self);
        hash(&v)
    }
}

impl StorageValue for bool {
    fn into_bytes(self) -> Vec<u8> {
        vec![self as u8]
    }

    fn from_bytes(value: Cow<[u8]>) -> Self {
        assert_eq!(value.len(), 1);

        match value[0] {
            0 => false,
            1 => true,
            _ => unreachable!(),
        }
    }

    fn hash(&self) -> Hash {
        hash(&[*self as u8])
    }
}

impl StorageValue for Hash {
    fn into_bytes(self) -> Vec<u8> {
        self.as_ref().to_vec()
    }

    fn from_bytes(value: Cow<[u8]>) -> Self {
        Self::from_slice(value.as_ref()).unwrap()
    }

    fn hash(&self) -> Hash {
        *self
    }
}

impl StorageValue for PublicKey {
    fn into_bytes(self) -> Vec<u8> {
        self.as_ref().to_vec()
    }

    fn from_bytes(value: Cow<[u8]>) -> Self {
        PublicKey::from_slice(value.as_ref()).unwrap()
    }

    fn hash(&self) -> Hash {
        hash(self.as_ref())
    }
}

impl StorageValue for RawMessage {
    fn into_bytes(self) -> Vec<u8> {
        self.as_ref().to_vec()
    }

    fn from_bytes(value: Cow<[u8]>) -> Self {
        Self::new(MessageBuffer::from_vec(value.into_owned()))
    }

    fn hash(&self) -> Hash {
        Message::hash(self)
    }
}

impl StorageValue for Vec<u8> {
    fn into_bytes(self) -> Vec<u8> {
        self
    }

    fn from_bytes(value: Cow<[u8]>) -> Self {
        value.into_owned()
    }

    fn hash(&self) -> Hash {
        hash(self)
    }
}

impl StorageValue for String {
    fn into_bytes(self) -> Vec<u8> {
        String::into_bytes(self)
    }

    fn from_bytes(value: Cow<[u8]>) -> Self {
        String::from_utf8(value.into_owned()).unwrap()
    }

    fn hash(&self) -> Hash {
        hash(self.as_ref())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn u8_round_trip() {
        let values = [u8::min_value(), 1, u8::max_value()];
        for value in values.iter() {
            let bytes = value.into_bytes();
            assert_eq!(*value, u8::from_bytes(Cow::Borrowed(&bytes)));
        }
    }

    #[test]
    fn i8_round_trip() {
        let values = [i8::min_value(), -1, 0, 1, i8::max_value()];
        for value in values.iter() {
            let bytes = value.into_bytes();
            assert_eq!(*value, i8::from_bytes(Cow::Borrowed(&bytes)));
        }
    }

    #[test]
    fn u16_round_trip() {
        let values = [u16::min_value(), 1, u16::max_value()];
        for value in values.iter() {
            let bytes = value.into_bytes();
            assert_eq!(*value, u16::from_bytes(Cow::Borrowed(&bytes)));
        }
    }

    #[test]
    fn i16_round_trip() {
        let values = [i16::min_value(), -1, 0, 1, i16::max_value()];
        for value in values.iter() {
            let bytes = value.into_bytes();
            assert_eq!(*value, i16::from_bytes(Cow::Borrowed(&bytes)));
        }
    }

    #[test]
    fn u32_round_trip() {
        let values = [u32::min_value(), 1, u32::max_value()];
        for value in values.iter() {
            let bytes = value.into_bytes();
            assert_eq!(*value, u32::from_bytes(Cow::Borrowed(&bytes)));
        }
    }

    #[test]
    fn i32_round_trip() {
        let values = [i32::min_value(), -1, 0, 1, i32::max_value()];
        for value in values.iter() {
            let bytes = value.into_bytes();
            assert_eq!(*value, i32::from_bytes(Cow::Borrowed(&bytes)));
        }
    }

    #[test]
    fn u64_round_trip() {
        let values = [u64::min_value(), 1, u64::max_value()];
        for value in values.iter() {
            let bytes = value.into_bytes();
            assert_eq!(*value, u64::from_bytes(Cow::Borrowed(&bytes)));
        }
    }

    #[test]
    fn i64_round_trip() {
        let values = [i64::min_value(), -1, 0, 1, i64::max_value()];
        for value in values.iter() {
            let bytes = value.into_bytes();
            assert_eq!(*value, i64::from_bytes(Cow::Borrowed(&bytes)));
        }
    }

    #[test]
    fn bool_round_trip() {
        let values = [false, true];
        for value in values.iter() {
            let bytes = value.into_bytes();
            assert_eq!(*value, bool::from_bytes(Cow::Borrowed(&bytes)));
        }
    }

    #[test]
    fn vec_round_trip() {
        let values = [vec![], vec![1], vec![1, 2, 3], vec![255; 100]];
        for value in values.iter() {
            let bytes = value.clone().into_bytes();
            assert_eq!(*value, Vec::<u8>::from_bytes(Cow::Borrowed(&bytes)));
        }
    }

    #[test]
    fn string_round_trip() {
        let values: Vec<_> = ["", "e", "2", "hello"].iter().map(|v| v.to_string()).collect();
        for value in values.iter() {
            let bytes = value.clone().into_bytes();
            assert_eq!(*value, String::from_bytes(Cow::Borrowed(&bytes)));
        }
    }
}

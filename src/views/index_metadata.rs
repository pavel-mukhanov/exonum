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

use std::borrow::Cow;

use enum_primitive_derive::Primitive;
use failure::{self, ensure, format_err};
use num_traits::{FromPrimitive, ToPrimitive};
use serde_derive::{Deserialize, Serialize};

use super::{IndexAccess, IndexAddress, View};
use crate::{BinaryValue, Fork};

const INDEX_METADATA_NAME: &str = "__INDEX_METADATA__";

#[derive(Debug, Copy, Clone, PartialEq, Primitive, Serialize, Deserialize)]
pub enum IndexType {
    Map = 1,
    List = 2,
    Entry = 3,
    ValueSet = 4,
    KeySet = 5,
    SparseList = 6,
    ProofList = 7,
    ProofMap = 8,
}

impl BinaryValue for IndexType {
    fn to_bytes(&self) -> Vec<u8> {
        // `.unwrap()` is safe: IndexType is always in range 1..255
        vec![self.to_u8().unwrap()]
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Result<Self, failure::Error> {
        let bytes = bytes.as_ref();
        ensure!(
            bytes.len() == 1,
            "Wrong buffer size: actual {}, expected 1",
            bytes.len()
        );

        let value = bytes[0];
        Self::from_u8(value).ok_or_else(|| format_err!("Unknown value: {}", value))
    }
}

#[allow(unsafe_code)]
pub fn check_or_create_metadata<T: IndexAccess>(
    snapshot: T,
    address: &IndexAddress,
    index_type: IndexType,
    has_parent: bool,
) {
    let address = {
        let mut metadata_address = address.append_name(INDEX_METADATA_NAME);
        // Uses a single metadata insance for the all indexes in family.
        metadata_address.bytes = None;
        metadata_address
    };

    let maybe_fork = {
        let metadata = IndexMetadata {
            view: View::new(snapshot, address.clone()),
        };
        if let Some(saved_index_type) = metadata.index_type() {
            let saved_has_parent = metadata
                .has_parent()
                .expect("Index metadata is inconsistent");
            assert_eq!(
                saved_index_type, index_type,
                "Saved index type doesn't match specified"
            );
            assert_eq!(
                saved_has_parent, has_parent,
                "Saved index family doesn't match specified"
            );
            return;
        }
        unsafe { metadata.view.snapshot.fork() }
    };

    if let Some(fork) = maybe_fork {
        let mut metadata_mut = IndexMetadata {
            view: View::new(fork, address.clone()),
        };
        metadata_mut.set_index_type(index_type);
        metadata_mut.set_has_parent(has_parent);
    }
}

struct IndexMetadata<T: IndexAccess> {
    view: View<T>,
}

impl<T: IndexAccess> IndexMetadata<T> {
    fn index_type(&self) -> Option<IndexType> {
        self.attribute("index_type")
    }

    fn has_parent(&self) -> Option<bool> {
        self.attribute("has_parent")
    }

    fn attribute<V: BinaryValue>(&self, name: &str) -> Option<V> {
        self.view.get(name)
    }
}

impl IndexMetadata<&Fork> {
    fn set_index_type(&mut self, index_type: IndexType) {
        self.set_attribute("index_type", index_type)
    }

    fn set_has_parent(&mut self, has_parent: bool) {
        self.set_attribute("has_parent", has_parent)
    }

    fn set_attribute<V: BinaryValue>(&mut self, name: &str, value: V) {
        self.view.put(name, value)
    }
}

#[cfg(test)]
mod tests {
    use super::IndexType;
    use crate::BinaryValue;
    use std::borrow::Cow;

    #[test]
    fn test_index_type_binary_value_correct() {
        let index_type = IndexType::ProofMap;
        let buf = index_type.to_bytes();
        assert_eq!(IndexType::from_bytes(Cow::Owned(buf)).unwrap(), index_type);
    }

    #[test]
    #[should_panic(expected = "Wrong buffer size: actual 2, expected 1")]
    fn test_index_type_binary_value_incorrect_buffer_len() {
        let buf = vec![1, 2];
        IndexType::from_bytes(Cow::Owned(buf)).unwrap();
    }

    #[test]
    #[should_panic(expected = "Unknown value: 127")]
    fn test_index_type_binary_value_incorrect_value() {
        let buf = vec![127];
        IndexType::from_bytes(Cow::Owned(buf)).unwrap();
    }
}

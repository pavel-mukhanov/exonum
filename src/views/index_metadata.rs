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

#[derive(Debug, Clone, PartialEq)]
pub struct IndexMetadataAddress(IndexAddress);

impl From<&IndexAddress> for IndexMetadataAddress {
    fn from(address: &IndexAddress) -> Self {
        let mut address = address.append_name(INDEX_METADATA_NAME);
        // We uses a single metadata insance for the all indexes in family.
        address.bytes = None;
        IndexMetadataAddress(address)
    }
}

/// Metadata for each index that currently stored in the merkledb.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IndexMetadata {
    /// Type of the specified index.
    pub index_type: IndexType,
    /// Property that indicates whether the index is a family.
    pub has_parent: bool,
}

pub fn check_or_create_metadata<T: IndexAccess, I: Into<IndexMetadataAddress>>(
    snapshot: T,
    address: I,
    metadata: &IndexMetadata,
) {
    let address = address.into();

    let snapshot = {
        let metadata_view = IndexMetadataView::new(snapshot, address.clone());
        if let Some(saved_metadata) = metadata_view.index_metadata() {
            assert_eq!(
                metadata, &saved_metadata,
                "Saved metadata doesn't match specified"
            )
        }
        metadata_view.view.snapshot
    };

    // Unsafe method `snapshot.fork()` here is safe because we never use fork outside this block.
    #[allow(unsafe_code)]
    unsafe {
        if let Some(fork) = snapshot.fork() {
            let mut metadata_view = IndexMetadataView::new(fork, address);
            metadata_view.set_index_metadata(&metadata);
        }
    }
}

pub struct IndexMetadataView<T: IndexAccess> {
    view: View<T>,
}

impl<T: IndexAccess> IndexMetadataView<T> {
    pub fn new<I: Into<IndexMetadataAddress>>(snapshot: T, address: I) -> Self {
        let address = address.into().0;
        Self {
            view: View::new(snapshot, address),
        }
    }

    pub fn index_metadata(&self) -> Option<IndexMetadata> {
        let index_type = self.view.get("index_type")?;
        let has_parent = self
            .view
            .get("has_parent")
            .expect("Index metadata is inconsistent");

        Some(IndexMetadata {
            index_type,
            has_parent,
        })
    }
}

impl IndexMetadataView<&Fork> {
    fn set_index_metadata(&mut self, metadata: &IndexMetadata) {
        self.view.put("index_type", metadata.index_type);
        self.view.put("has_parent", metadata.has_parent);
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

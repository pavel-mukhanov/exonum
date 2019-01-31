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
const INDEX_TYPE_NAME: &str = "index_type";

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
        let address = address.append_name(INDEX_METADATA_NAME);
        IndexMetadataAddress(address)
    }
}

/// Metadata for each index that currently stored in the merkledb.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IndexMetadata {
    /// Type of the specified index.
    pub index_type: IndexType,
}

pub fn check_or_create_metadata<T: IndexAccess, I: Into<IndexMetadataAddress>>(
    index_access: T,
    address: I,
    metadata: &IndexMetadata,
) {
    let address = address.into();

    let index_access = {
        let metadata_view = IndexMetadataView::new(index_access, address.clone());
        if let Some(saved_metadata) = metadata_view.index_metadata() {
            assert_eq!(
                metadata, &saved_metadata,
                "Saved metadata doesn't match specified"
            )
        }
        metadata_view.view.index_access
    };

    // Unsafe method `index_access.fork()` here is safe because we never use fork outside this block.
    #[allow(unsafe_code)]
    unsafe {
        if let Some(fork) = index_access.fork() {
            let mut metadata_view = IndexMetadataView::new(fork, address);
            metadata_view.set_index_metadata(&metadata);
        }
    }
}

pub struct IndexMetadataView<T: IndexAccess> {
    view: View<T>,
}

impl<T: IndexAccess> IndexMetadataView<T> {
    pub fn new<I>(index_access: T, address: I) -> Self
    where
        I: Into<IndexMetadataAddress>,
    {
        let address = address.into().0;
        Self {
            view: View::new(index_access, address),
        }
    }

    pub fn index_metadata(&self) -> Option<IndexMetadata> {
        self.view
            .get(INDEX_TYPE_NAME)
            .map(|index_type| IndexMetadata { index_type })
    }
}

impl IndexMetadataView<&Fork> {
    fn set_index_metadata(&mut self, metadata: &IndexMetadata) {
        self.view.put(INDEX_TYPE_NAME, metadata.index_type);
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

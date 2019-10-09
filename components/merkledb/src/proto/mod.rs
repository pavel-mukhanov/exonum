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

//! Module of the rust-protobuf generated files.

// For protobuf generated files.
#![allow(bare_trait_objects)]

include!(concat!(env!("OUT_DIR"), "/protobuf_mod.rs"));

use exonum_crypto::proto::*;
use exonum_crypto::PublicKey;
use exonum_proto::ProtobufConvert;
use crate::proto;
use failure::Error;

pub struct Proof {
    key: PublicKey,
}

impl ProtobufConvert for Proof {
    type ProtoStruct = proto::Proof;


    /// Struct -> ProtoStruct
    fn to_pb(&self) -> Self::ProtoStruct {
        unimplemented!()
    }

    /// ProtoStruct -> Struct
    fn from_pb(pb: Self::ProtoStruct) -> Result<Self, Error> {
        unimplemented!()
    }
}

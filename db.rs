use std::collections::BTreeMap;

use super::{Map, Error};

pub trait Fork : Map<[u8], Vec<u8>> + Sized {
    fn changes(&self) -> Patch;
    fn merge(&self, patch: Patch);
}

// TODO In this implementation there are extra memory allocations when key is passed into specific database.
// Think about key type. Maybe we can use keys with fixed length?
pub trait Database: Map<[u8], Vec<u8>> + Sized {
    type Fork: Fork;

    fn fork(&self) -> Self::Fork;
    fn merge(&self, patch: Patch) -> Result<(), Error>;
}

#[derive(Clone)]
pub enum Change {
    Put(Vec<u8>),
    Delete,
}

pub type Patch = BTreeMap<Vec<u8>, Change>;

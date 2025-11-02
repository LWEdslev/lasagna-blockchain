use serde::{Deserialize, Serialize};

use crate::util::MiniLas;

pub mod blockchain;
pub mod block;
pub mod ledger;
pub mod transaction;
pub mod keys;
pub mod draw;
pub mod util;
pub mod actors;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Las(pub u64);

impl Las {
    pub fn into_minilas(self) -> MiniLas {
        self.0 * 1_000_000
    }

    pub fn from_minilas(minilas: MiniLas) -> Self {
        Self(minilas / 1_000_000)
    }
}

impl From<MiniLas> for Las {
    fn from(value: MiniLas) -> Self {
        Las::from_minilas(value)
    }
}

impl From<Las> for MiniLas {
    fn from(value: Las) -> Self {
        value.into_minilas()
    }
}
use std::collections::HashMap;

use crate::{keys::PublicKey, util::MiniLas};

// Used to take a snapshot of the accounts that appear in a transaction before processing the instructions
#[derive(Default, Debug)]
pub struct Snapshot {
    pub balances: HashMap<PublicKey, Option<u64>>,
}

impl Snapshot {
    pub fn new() -> Self {
        Self { balances: HashMap::new() }
    }

    pub fn snapshot_balance(&mut self, key: &PublicKey, state: &HashMap<PublicKey, MiniLas>) {
        self.balances.entry(key.clone()).or_insert_with(|| state.get(&key).copied());
    }
}

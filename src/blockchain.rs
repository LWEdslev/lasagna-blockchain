use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::{block::Block, keys::PublicKey, ledger::Ledger, transaction::Transaction, util::{BlockPtr, MiniLas, Sha256Hash}};


pub const BLOCK_REWARD: MiniLas = 3_000000;
pub const TRANSACTION_FEE: MiniLas = 0_010000;

#[derive(Clone, Serialize, Deserialize)]
pub struct Blockchain {
    pub blocks: Vec<HashMap<Sha256Hash, Block>>,
    pub best_path: Vec<BlockPtr>,
    pub ledger: Ledger,
    pub root_accounts: Vec<PublicKey>,
    pub orphans: HashMap<Sha256Hash, Vec<Block>>,
    pub transaction_buffer: HashSet<Transaction>,
    start_time: u128
}


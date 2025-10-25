use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::{block::Block, keys::{PublicKey, SecretKey}, ledger::Ledger, transaction::{self, Transaction}, util::{BlockPtr, MiniLas, Sha256Hash}};
use anyhow::{Result, anyhow};

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

impl Blockchain {
    pub fn start(root_accounts: Vec<PublicKey>, any_sk: &SecretKey) -> Self {
        todo!()
    }

    pub fn best_path_head(&self) -> &BlockPtr {
        self.best_path.last().expect("no blocks in best path")
    }
    
    fn check_seed(&self, block: &Block) -> Result<()> {
        todo!()
    }

    pub fn stake() {
        todo!()
    }

    pub fn add_transaction(&mut self, transaction: Transaction) -> Result<()> {
        todo!()
    }

    pub fn add_block(&mut self, block: Block) -> Result<()> {
        todo!()
    }

    pub fn rollback(&mut self, from: BlockPtr, to: BlockPtr) {
        todo!()
    }

    pub fn get_block(&self, ptr: &BlockPtr) -> Option<&Block> {
        self.blocks.get(ptr.depth as usize).and_then(|d| d.get(&ptr.hash))
    }

    pub fn verify_chain(&self) -> Result<()> {
        todo!()
    }

    pub fn calculate_reward(&self, block: &Block) -> MiniLas {
        block.transactions.len() as MiniLas * TRANSACTION_FEE + BLOCK_REWARD 
    }
}
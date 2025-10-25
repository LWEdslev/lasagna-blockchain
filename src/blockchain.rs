use std::collections::{HashMap, HashSet};

use num_bigint::BigUint;
use rand::rand_core::block;
use serde::{Deserialize, Serialize};

use crate::{
    block::Block,
    draw::{self, Draw, Seed},
    keys::{PublicKey, SecretKey},
    ledger::{self, Ledger},
    transaction::{self, Transaction},
    util::{BlockPtr, MiniLas, Sha256Hash, START_TIME},
};
use anyhow::{Result, anyhow};

pub const BLOCK_REWARD: MiniLas = 3_000000;
pub const ROOT_AMOUNT: MiniLas = 100_000000;
pub const TRANSACTION_FEE: MiniLas = 0_010000;

#[derive(Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct Blockchain {
    pub blocks: Vec<HashMap<Sha256Hash, Block>>,
    pub best_path: Vec<BlockPtr>,
    pub ledger: Ledger,
    pub root_accounts: Vec<PublicKey>,
    pub orphans: HashMap<Sha256Hash, Vec<Block>>,
    pub transaction_buffer: HashSet<Transaction>,
    pub prev_transactions: HashSet<Sha256Hash>,
    start_time: u128,
}

impl Blockchain {
    pub fn produce_genesis_block(root_accounts: Vec<PublicKey>, any_sk: &SecretKey) -> Block {
        let genesis_hash = Block::produce_genesis_hash(&root_accounts);
        let seed = Seed {
            block_ptr: BlockPtr {
                hash: genesis_hash,
                depth: 0,
            },
        };

        Block::new(0, genesis_hash, 0, Vec::new(), any_sk, seed)
    }

    pub fn start(root_accounts: Vec<PublicKey>, genesis_block: Block) -> Self {
        let block = genesis_block;
        let hash = block.hash;
        let mut map = HashMap::new();
        map.insert(hash, block);

        let mut ledger = Ledger::new(root_accounts.clone());
        root_accounts
            .iter()
            .for_each(|accnt| ledger.reward_winner(accnt, ROOT_AMOUNT));

        let blocks = vec![map];
        let best_path = vec![BlockPtr { hash, depth: 0 }];

        Self {
            blocks,
            best_path,
            ledger,
            root_accounts,
            orphans: Default::default(),
            transaction_buffer: Default::default(),
            prev_transactions: Default::default(),
            start_time: START_TIME,
        }
    }

    pub fn best_path_head(&self) -> &BlockPtr {
        self.best_path.last().expect("no blocks in best path")
    }
    
    fn check_seed(&self, block: &Block) -> Result<bool> {
        let block_seed = &block.draw.seed;
        let depth = block.depth;
        if depth < SEED_AGE && depth > 0 {
            // Block is close to genesis and must have the same seed as the genesis block
            let genesis_block_ptr = &self.best_path[0];
            let genesis_block = self.get_block(&genesis_block_ptr).ok_or_else(|| anyhow!("Could not find genesis block"))?;
            let genesis_seed = &genesis_block.draw.seed;

            if block_seed != genesis_seed {
                return Ok(false);
            }
        } else {
            // Block seed should be the hash of the block from 50 rounds ago
            let seed_depth = (depth - SEED_AGE) as usize;
            let seed_block_ptr = &self.best_path[seed_depth];
            let seed_block = &self.get_block(seed_block_ptr).ok_or_else(|| anyhow!("Could not find seed block"))?;

            let seed = Seed{
                block_ptr: BlockPtr { hash: seed_block.hash, depth: depth }
            };

            if block_seed != &seed {
                return Ok(false);
            }
        }

        Ok(true)
    }

    pub fn stake(&self, draw: Draw, wallet: &PublicKey, depth: i64) -> bool {
        is_winner(&self.ledger, draw, wallet, depth)
    }

    pub fn add_transaction(&mut self, transaction: Transaction) -> Result<()> {
        todo!()
    }

    pub fn add_block(&mut self, block: Block) -> Result<()> {
        // Check if signature is valid
        block.verify_signature()?;

        // Check if the prev_block is valid
        let depth = block.depth as usize;
        let parent_hash = block.prev_hash;
        let parent_ptr = BlockPtr{hash: parent_hash, depth: block.depth - 1};
        let parent_block = Self::get_block(&self, &parent_ptr);
        let Some(parent_block) = parent_block else {
            // This block is an orphan
            if let Some(orphans) = self.orphans.get_mut(&block.prev_hash){
                orphans.push(block);
            } else {
                self.orphans.insert(block.prev_hash, vec![block]);
            }
            return Err(anyhow!("No parent was found to the block"));
        };

        // Check if timeslot is valid
        if block.timeslot <= parent_block.timeslot || block.timeslot > self.calculate_timeslot() {
            return Err(anyhow!("Invalid timeslot"))
        }

        // Check if seed is valid
        if !self.check_seed(&block)?{
            return Err(anyhow!("Invalid seed"))
        }

        while depth >= self.blocks.len() {
            // Create empty hashmaps if the block is in the future, this will usually just be done once
            self.blocks.push(HashMap::new());
        }

        // Add block to the chain
        self.blocks.get_mut(depth).expect("unreachable").insert(block.hash, block.clone());

        // Remove transactions from the block from the transaction buffer
        for t in block.transactions.iter() {
            self.transaction_buffer.remove(t);
        }

        let block_ptr = BlockPtr{ hash: block.hash, depth: block.depth };
        let old_best_path = self.best_path_head().clone();

        if block.depth > old_best_path.depth {
            // This is the new best path
            if old_best_path.hash != parent_hash {
                // Rollback if we branch change
                self.rollback(&old_best_path, &block_ptr);
            } else {
                self.proccess_transactions(&block.transactions, block.depth);
                self.ledger.reward_winner(&block.draw.signed_by, BLOCK_REWARD);
                self.best_path.push(block_ptr);
            }
        } else if block.depth == old_best_path.depth {
            let new_block = &block;
            let curr_best_block = self.blocks[old_best_path.depth as usize]
            .get(&old_best_path.hash)
            .ok_or_else(|| anyhow!("Could not find old best block"))?;
            
            if new_block > curr_best_block {
                self.rollback(&old_best_path, &block_ptr);
            }
        }

        // Check if this block has any orphans. If yes, add them after
        if let Some(orphans) = self.orphans.remove(&block.hash) {
            for orphan in orphans {
                let res = self.add_block(orphan.clone());
                println!("added orphan, result = {:?}", res);
            }
        }

        // Return whether the best_path has been updated
        (&old_best_path != self.best_path_head())
        .then_some(())
        .ok_or_else(|| anyhow!("Best path not updated"))
    }

    pub fn rollback(&mut self, from: &BlockPtr, to: &BlockPtr) {
        todo!()
    }

    pub fn make_block(&self, sk: &SecretKey) -> Block {
        todo!()
    }

    pub fn get_block(&self, ptr: &BlockPtr) -> Option<&Block> {
        self.blocks
            .get(ptr.depth as usize)
            .and_then(|d| d.get(&ptr.hash))
    }

    pub fn get_parent(&self, ptr: &BlockPtr) -> Option<&Block> {
        let block = self.get_block(ptr)?;
        let parent_hash = block.prev_hash;
        let parent_depth = ptr.depth - 1;
        let parent_ptr = BlockPtr::new(parent_hash, parent_depth);
        self.get_block(&parent_ptr)
    }

    pub fn verify_chain(&self) -> Result<()> {
        let genesis_block = {
            let mut blocks = self.blocks[0].values();
            if blocks.len() == 1 {
                BlockPtr::new(blocks.next().unwrap().hash, 0)
            } else {
                return Err(anyhow!("There are too many blocks in genesis depth"))
            }
        };

        let genesis_block = self.get_block(&genesis_block).ok_or(anyhow!("No genesis block"))?.clone();

        // We take all the blocks and add them to a new blockchain, if we get the same then it is ok
        let mut track_blockchain = Blockchain::start(self.root_accounts.clone(), genesis_block);

        let max_depth = self.best_path.len();
        for depth in 1..max_depth {
            let blocks_at_depth = self.blocks[depth].values();
            for block in blocks_at_depth {
                track_blockchain.add_block(block.clone())?;
            }
        }

        // We also add the orphans
        let orphan_blocks = self.orphans.values().flatten();
        for block in orphan_blocks {
            track_blockchain.add_block(block.clone())?;
        }

        for transaction in self.transaction_buffer.iter() {
            transaction.verify_signature()?;
            if self.prev_transactions.contains(&transaction.hash) {
                return Err(anyhow!("Transaction both in prev and in buffer"));
            }
        }

        if self != &track_blockchain {
            return Err(anyhow!("Mismatch in resulting blockchains"));
        }

        Ok(())
    }

    pub fn calculate_reward(&self, block: &Block) -> MiniLas {
        block.transactions.len() as MiniLas * TRANSACTION_FEE + BLOCK_REWARD
    }

    pub fn calculate_timeslot(&self) -> Timeslot {
        let now = get_unix_timestamp();
        let start = self.start_time;
        let timeslot = (now - start) / SLOT_LENGTH;
        timeslot as _
    }

    fn proccess_transactions(&mut self, transactions: &Vec<Transaction>, depth: i64) {
        for t in transactions.iter() {
            self.ledger.process_transaction(t, depth);
        }
    }
}

fn is_winner(ledger: &Ledger, draw: Draw, wallet: &PublicKey, depth: i64) -> bool {
    if !ledger.can_stake(wallet, depth) {
        return false;
    }

    let balance = BigUint::from(ledger.get_balance(wallet));
    let total_money = ledger.get_total_money_in_ledger();
    let max_hash = BigUint::from(2u64).pow(256);

    // the entire network has a total 10% chance of beating this at a given timeslot
    let hardness = BigUint::from(10421u64) * (BigUint::from(10u64).pow(73));

    // we must map the draw value which is in [0, 2^256] to [0, h + c(2^256 - h)] where h is hardness and c is the ratio of money we have
    // we can map this by multiplying the draw with (h + c(2^256 - h))/(2^256)
    // we can describe c as balance/total_money. Therefore we can multiply total_money to the hardness and write the multiplication factor as:
    let mult_factor =
        (hardness.clone() * total_money) + (balance * (max_hash.clone() - hardness.clone()));

    // We win if we have a good draw and a big enough fraction of the money
    draw.value.clone() * mult_factor > hardness * total_money * max_hash.clone()
}

use std::collections::{HashMap, HashSet};

use num_bigint::BigUint;
use serde::{Deserialize, Serialize};

use crate::{
    block::Block,
    draw::{Draw, SEED_AGE, Seed},
    keys::{PublicKey, SecretKey},
    ledger::Ledger,
    transaction::Transaction,
    util::{BlockPtr, MiniLas, START_TIME, Sha256Hash, calculate_timeslot},
};
use anyhow::{Result, anyhow, ensure};

pub const BLOCK_REWARD: MiniLas = 3_000000;
pub const ROOT_AMOUNT: MiniLas = 100_000000;
pub const TRANSACTION_FEE: MiniLas = 0_010000;

#[derive(Clone, Serialize, Deserialize, Eq, PartialEq, Debug)]
pub struct Blockchain {
    pub blocks: Vec<HashMap<Sha256Hash, Block>>,
    pub best_path: Vec<BlockPtr>,
    pub dynamic_ledger: Ledger,
    pub static_ledger: Ledger,
    pub root_accounts: Vec<PublicKey>,
    pub orphans: HashMap<Sha256Hash, Vec<Block>>,
    pub transaction_buffer: HashSet<Transaction>,
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

        let static_ledger = ledger.clone();
        let dynamic_ledger = ledger;

        Self {
            blocks,
            best_path,
            static_ledger,
            dynamic_ledger,
            root_accounts,
            orphans: Default::default(),
            transaction_buffer: Default::default(),
            start_time: START_TIME,
        }
    }

    pub fn best_path_head(&self) -> &BlockPtr {
        self.best_path.last().expect("no blocks in best path")
    }

    fn check_seed(&self, block: &Block) -> Result<()> {
        let block_seed = &block.draw.seed;
        let depth = block.depth;
        if depth < SEED_AGE {
            // Block is close to genesis and must have the same seed as the genesis block
            let genesis_block_ptr = &self.best_path[0];
            let genesis_block = self
                .get_block(&genesis_block_ptr)
                .ok_or_else(|| anyhow!("Could not find genesis block"))?;
            let genesis_seed = &genesis_block.draw.seed;

            if block_seed != genesis_seed {
                return Err(anyhow!("seed mismatch"));
            }
        } else {
            // Block seed should be the hash of the block from 50 rounds ago
            let seed_depth = (depth - SEED_AGE) as usize;
            let seed_block_ptr = &self.best_path[seed_depth];
            let _seed_block = &self
                .get_block(seed_block_ptr)
                .ok_or_else(|| anyhow!("Could not find seed block"))?;

            let seed = Seed {
                block_ptr: seed_block_ptr.clone(),
            };

            if block_seed != &seed {
                return Err(anyhow!("seed mismatch"));
            }
        }

        Ok(())
    }

    pub fn stake(&self, draw: Draw, wallet: &PublicKey) -> bool {
        is_winner(&self.static_ledger, draw, wallet)
    }

    pub fn add_transaction(&mut self, transaction: Transaction) -> Result<()> {
        transaction.verify_signature()?;
        self.dynamic_ledger.is_transaction_valid(&transaction)?;
        self.transaction_buffer.insert(transaction);
        Ok(())
    }

    pub fn can_block_be_added(&self, block: &Block) -> Result<()> {
        block.verify_signature()?;

        for t in block.transactions.iter() {
            self.dynamic_ledger.is_transaction_valid(t)?
        }
        self.check_seed(&block)?;

        if block.timeslot > calculate_timeslot(START_TIME) {
            return Err(anyhow!("Invalid timeslot"));
        }

        let parent = self.get_parent(&block);

        if let Some(parent) = parent {
            if block.timeslot <= parent.timeslot {
                return Err(anyhow!("Invalid timeslot in relation to parents"));
            }

            if parent.hash == block.hash {
                return Err(anyhow!("Duplicate hash"));
            }
        }

        ensure!(is_winner(
            &self.get_static_ledger_of(block.depth)?,
            block.draw.clone(),
            &block.draw.signed_by
        ));

        Ok(())
    }

    pub fn get_static_ledger_of(&self, dynamic_depth: i64) -> Result<Ledger> {
        let current_static_ledger = &self.static_ledger;
        let current_static_ptr = self.get_static_block_ptr(self.best_path.len() as _);

        let target_static_ptr = self.get_static_block_ptr(dynamic_depth as _);

        if current_static_ptr == target_static_ptr {
            return Ok(current_static_ledger.clone());
        }

        let mut current_static_ledger = current_static_ledger.clone();
        if current_static_ptr.depth > target_static_ptr.depth {
            let from = current_static_ptr.depth as usize;
            let to = target_static_ptr.depth as usize;
            let path = &self.best_path[to..from];
            for ptr in path.iter().rev() {
                let block = self.get_block(ptr).ok_or(anyhow!("invalid deref"))?;
                let reward = self.calculate_reward(block);

                current_static_ledger.rollback_reward(&block.draw.signed_by, reward);
                for t in &block.transactions {
                    current_static_ledger.rollback_transaction(&t, block.depth);
                }
            }

            return Ok(current_static_ledger);
        } else {
            let from = current_static_ptr.depth as usize;
            let to = target_static_ptr.depth as usize;
            let path = &self.best_path[from..to];
            for ptr in path.iter() {
                let block = self.get_block(ptr).ok_or(anyhow!("invalid deref"))?;
                let reward = self.calculate_reward(block);
                current_static_ledger.reward_winner(&block.draw.signed_by, reward);
                for t in &block.transactions {
                    current_static_ledger.process_transaction(&t)?;
                }
            }

            return Ok(current_static_ledger);
        }
    }

    pub fn add_block(&mut self, block: Block) -> Result<()> {
        self.can_block_be_added(&block)?;

        // Check if the prev_block is valid
        let parent_block = self.get_parent(&block);
        let Some(_) = parent_block else {
            // This block is an orphan
            if let Some(orphans) = self.orphans.get_mut(&block.prev_hash) {
                orphans.push(block);
            } else {
                self.orphans.insert(block.prev_hash, vec![block]);
            }
            return Ok(());
        };

        while block.depth as usize >= self.blocks.len() {
            // Create empty hashmaps if the block is in the future, this will usually just be done once
            self.blocks.push(HashMap::new());
        }

        // Add block to the chain
        self.blocks
            .get_mut(block.depth as usize)
            .expect("unreachable")
            .insert(block.hash, block.clone());

        let block_ptr = &block.ptr();
        let parent_ptr = self
            .get_parent(&block)
            .expect("no parent but we are not in orphan case")
            .ptr();
        let old_best_path = self.best_path_head().clone();

        if old_best_path == parent_ptr {
            // This is an extension of the best path
            // Remove transactions from the block from the transaction buffer
            for t in block.transactions.iter() {
                self.transaction_buffer.remove(t);
            }

            self.proccess_transactions(&block.transactions)?;
            self.dynamic_ledger
                .reward_winner(&block.draw.signed_by, self.calculate_reward(&block));
            self.best_path.push(block_ptr.clone());
        } else if block > *self.get_block(&old_best_path).expect("unreachable") {
            // This block is the new best one and we must rollback
            self.rollback(&old_best_path, &block_ptr)?;
        }

        // Check if this block has any orphans. If yes, add them after
        if let Some(orphans) = self.orphans.remove(&block.hash) {
            for orphan in orphans {
                self.add_block(orphan.clone())?;
            }
        }

        self.update_static_ledger()?;

        Ok(())
    }

    fn update_static_ledger(&mut self) -> Result<()> {
        let new_depth = self.best_path.len() as i64;
        let new_static_ledger = self.get_static_ledger_of(new_depth)?;
        self.static_ledger = new_static_ledger;
        Ok(())
    }

    pub fn rollback(&mut self, from: &BlockPtr, to: &BlockPtr) -> Result<()> {
        // Now we are at from, we must first find the common ancestor of from and to
        let common = self
            .find_common_ancestor(from.clone(), to.clone())
            .ok_or(anyhow!("No common ancestor of the rollback"))?;

        // Revert from `from` to `common`
        let mut from = from.clone();
        while from != common {
            self.rollback_block(&from)?;
            from = self
                .get_parent_from_ptr(&from)
                .ok_or(anyhow!("no parent"))?
                .ptr();
        }

        // Apply from `common` to `to`
        // First we travers from `to` to `common`` to get the path to add
        let mut path = Vec::new();
        let mut to = to.clone();
        while to != common {
            path.push(to.clone());
            to = self
                .get_parent_from_ptr(&to)
                .ok_or(anyhow!("no parent"))?
                .ptr();
        }

        // Now we apply
        while let Some(block_ptr) = path.pop() {
            let block_to_add = self.get_block(&block_ptr).ok_or(anyhow!("No block"))?;
            self.add_block(block_to_add.clone())?;
        }

        Ok(())
    }

    fn rollback_block(&mut self, block_ptr: &BlockPtr) -> Result<()> {
        if block_ptr != self.best_path_head() {
            return Err(anyhow!("Cannot rollback a block that is not best"));
        }

        self.best_path
            .pop()
            .ok_or(anyhow!("Cannot rollback genesis"))?;

        let block = self
            .get_block(block_ptr)
            .ok_or(anyhow!("Cannot rollback a block that doesn't exist"))?
            .clone();
        for t in block.transactions.iter().rev() {
            self.dynamic_ledger.rollback_transaction(t, block.depth);
            self.transaction_buffer.insert(t.clone());
        }

        self.dynamic_ledger
            .rollback_reward(&block.draw.signed_by, self.calculate_reward(&block));

        self.blocks[block.depth as usize]
            .remove_entry(&block_ptr.hash)
            .ok_or(anyhow!("No block to remove"))?;

        if block.depth >= self.best_path.len() as i64
            && self.blocks[block.depth as usize].len() == 0
        {
            self.blocks.remove(block.depth as usize);
        }

        self.update_static_ledger()?;

        Ok(())
    }

    fn find_common_ancestor(&self, mut left: BlockPtr, mut right: BlockPtr) -> Option<BlockPtr> {
        while left.depth < right.depth {
            right = self.get_parent_from_ptr(&right)?.ptr();
        }

        while right.depth < left.depth {
            left = self.get_parent_from_ptr(&left)?.ptr();
        }

        // Now left and right are at the same depth
        // Thus we can move to each of their parents until they are equal
        while left != right {
            left = self.get_parent_from_ptr(&left)?.ptr();
            right = self.get_parent_from_ptr(&right)?.ptr();

            if left.depth == 0 || right.depth == 0 {
                return None;
            }
        }

        // now left == right, we have found the common ancestor
        Some(left)
    }

    pub fn make_block(&self, sk: &SecretKey) -> Option<Block> {
        let depth = self.best_path_head().depth + 1;
        let timeslot = calculate_timeslot(START_TIME);
        let prev_hash = self.best_path_head().hash;
        let transactions = self.transaction_buffer.clone().into_iter().collect();
        let seed = {
            if depth >= SEED_AGE {
                Seed {
                    block_ptr: self.best_path[(depth - SEED_AGE) as usize].clone(),
                }
            } else {
                let genesis_block = self.get_block(&self.best_path[0]).unwrap();
                genesis_block.draw.seed.clone()
            }
        };
        let new_static_ledger = self
            .get_static_ledger_of(depth)
            .expect("unable to create new static ledger");
        if is_winner(
            &new_static_ledger,
            Draw::new(timeslot, seed.clone(), sk),
            &sk.get_public_key(),
        ) {
            let block = Block::new(timeslot, prev_hash, depth, transactions, sk, seed);

            Some(block)
        } else {
            None
        }
    }

    pub fn get_block(&self, ptr: &BlockPtr) -> Option<&Block> {
        self.blocks
            .get(ptr.depth as usize)
            .and_then(|d| d.get(&ptr.hash))
    }

    pub fn get_parent(&self, block: &Block) -> Option<&Block> {
        let parent_hash = block.prev_hash;
        let parent_depth = block.depth - 1;
        let parent_ptr = BlockPtr::new(parent_hash, parent_depth);
        self.get_block(&parent_ptr)
    }

    // Use this is the parent is already added
    pub fn get_parent_from_ptr(&self, ptr: &BlockPtr) -> Option<&Block> {
        let block = self.get_block(ptr)?;
        self.get_parent(block)
    }

    pub fn verify_chain(&self) -> Result<()> {
        let genesis_block = {
            let mut blocks = self.blocks[0].values();
            if blocks.len() == 1 {
                BlockPtr::new(blocks.next().unwrap().hash, 0)
            } else {
                return Err(anyhow!("There are too many blocks in genesis depth"));
            }
        };

        let genesis_block = self
            .get_block(&genesis_block)
            .ok_or(anyhow!("No genesis block"))?
            .clone();

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
            if self
                .dynamic_ledger
                .previous_transactions
                .contains(&transaction.hash)
            {
                return Err(anyhow!("Transaction both in prev and in buffer"));
            }
        }

        if self != &track_blockchain {
            return Err(anyhow!("Mismatch in resulting blockchains"));
        }

        Ok(())
    }

    pub fn get_static_block_ptr(&self, dynamic_depth: i64) -> &BlockPtr {
        let dynamic_depth = dynamic_depth as usize;
        let idx = dynamic_depth.saturating_sub(SEED_AGE as _);
        &self.best_path[idx]
    }

    pub fn calculate_reward(&self, block: &Block) -> MiniLas {
        block.transactions.len() as MiniLas * TRANSACTION_FEE + BLOCK_REWARD
    }

    fn proccess_transactions(&mut self, transactions: &Vec<Transaction>) -> Result<()> {
        for t in transactions.iter() {
            self.dynamic_ledger.process_transaction(t)?;
        }
        Ok(())
    }
}

fn is_winner(ledger: &Ledger, draw: Draw, wallet: &PublicKey) -> bool {
    if !ledger.can_stake(wallet) {
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

#[cfg(test)]
impl Blockchain {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Las;
    use pretty_assertions::assert_eq;

    fn mine_new_block(blockchain: &Blockchain, sk: &SecretKey) -> Option<Block> {
        let mut max_iter = 10_000;
        let mut new_block = None;
        while new_block == None && max_iter > 0 {
            new_block = blockchain.make_block(sk);
            max_iter -= 1;
        }

        return new_block;
    }

    #[test]
    fn test_start() {
        let sk1 = SecretKey::generate();
        let sk2 = SecretKey::generate();

        let transaction_amount = Las(5);

        let transaction = Transaction::new(&sk1, sk2.get_public_key(), transaction_amount, 42);

        let root_accounts = vec![sk1.get_public_key(), sk2.get_public_key()];
        let genesis_block = Blockchain::produce_genesis_block(root_accounts.clone(), &sk1);
        let mut blockchain = Blockchain::start(root_accounts, genesis_block);

        blockchain.add_transaction(transaction).unwrap();
        assert_eq!(blockchain.best_path.len(), 1);
        assert_eq!(
            blockchain.dynamic_ledger.get_balance(&sk1.get_public_key()),
            ROOT_AMOUNT
        );

        let new_block = mine_new_block(&blockchain, &sk1).unwrap();
        blockchain.add_block(new_block).unwrap();

        assert_eq!(blockchain.best_path.len(), 2);
        assert_eq!(
            blockchain.dynamic_ledger.get_balance(&sk1.get_public_key()),
            ROOT_AMOUNT + BLOCK_REWARD - transaction_amount.into_minilas()
        );

        assert_eq!(
            blockchain.dynamic_ledger.get_balance(&sk2.get_public_key()),
            ROOT_AMOUNT + transaction_amount.into_minilas()
        );

        let transaction2_amount = Las(2);

        let transaction = Transaction::new(&sk2, sk1.get_public_key(), transaction2_amount, 54);
        blockchain.add_transaction(transaction.clone()).unwrap();
        let new_block = mine_new_block(&blockchain, &sk1).unwrap();
        blockchain.add_block(new_block).unwrap();

        assert_eq!(blockchain.best_path.len(), 3);
        assert_eq!(
            blockchain.dynamic_ledger.get_balance(&sk1.get_public_key()),
            ROOT_AMOUNT + 2 * BLOCK_REWARD + TRANSACTION_FEE + transaction2_amount.into_minilas()
                - transaction_amount.into_minilas()
        );

        assert_eq!(
            blockchain.dynamic_ledger.get_balance(&sk2.get_public_key()),
            ROOT_AMOUNT - TRANSACTION_FEE - transaction2_amount.into_minilas()
                + transaction_amount.into_minilas()
        );
        assert_eq!(blockchain.transaction_buffer, vec![].into_iter().collect());

        blockchain
            .rollback_block(&blockchain.best_path_head().clone())
            .unwrap();

        assert_eq!(blockchain.best_path.len(), 2);
        assert_eq!(
            blockchain.dynamic_ledger.get_balance(&sk1.get_public_key()),
            ROOT_AMOUNT + BLOCK_REWARD - transaction_amount.into_minilas()
        );

        assert_eq!(
            blockchain.dynamic_ledger.get_balance(&sk2.get_public_key()),
            ROOT_AMOUNT + transaction_amount.into_minilas()
        );

        assert_eq!(
            blockchain.transaction_buffer,
            vec![transaction].into_iter().collect()
        );
    }

    #[test]
    fn test_simple_rollback() {
        let sk = SecretKey::generate();
        let root_accounts = vec![sk.get_public_key()];
        let genesis_block = Blockchain::produce_genesis_block(root_accounts.clone(), &sk);
        let mut blockchain = Blockchain::start(root_accounts, genesis_block);
        assert_eq!(blockchain.best_path.len(), 1);
        assert_eq!(
            blockchain.dynamic_ledger.get_balance(&sk.get_public_key()),
            ROOT_AMOUNT
        );

        let initial_blockchain = blockchain.clone();

        let new_block = mine_new_block(&blockchain, &sk).unwrap();
        blockchain.add_block(new_block).unwrap();

        assert_eq!(blockchain.best_path.len(), 2);
        assert!(blockchain.dynamic_ledger.get_balance(&sk.get_public_key()) > ROOT_AMOUNT);

        blockchain
            .rollback_block(&blockchain.best_path_head().clone())
            .unwrap();

        assert_eq!(blockchain.best_path.len(), 1);
        assert_eq!(
            blockchain.dynamic_ledger.get_balance(&sk.get_public_key()),
            ROOT_AMOUNT
        );
        assert_eq!(blockchain, initial_blockchain);
    }

    #[test]
    fn test_multiple_blocks() {
        let sk = SecretKey::generate();
        let root_accounts = vec![sk.get_public_key()];
        let genesis_block = Blockchain::produce_genesis_block(root_accounts.clone(), &sk);
        let mut blockchain = Blockchain::start(root_accounts, genesis_block);
        assert_eq!(blockchain.best_path.len(), 1);
        assert_eq!(
            blockchain.dynamic_ledger.get_balance(&sk.get_public_key()),
            ROOT_AMOUNT
        );

        let new_block = mine_new_block(&blockchain, &sk).unwrap();
        let new_block2 = mine_new_block(&blockchain, &sk).unwrap();
        blockchain.add_block(new_block).unwrap();
        blockchain.add_block(new_block2).unwrap();

        assert_eq!(blockchain.best_path.len(), 2);
        assert!(blockchain.dynamic_ledger.get_balance(&sk.get_public_key()) > ROOT_AMOUNT);
    }

    #[test]
    fn many_blocks_and_verify_chain() {
        let sk1 = SecretKey::generate();
        let sk2 = SecretKey::generate();
        let transaction_amount = Las(1);

        let root_accounts = vec![sk1.get_public_key()];
        let genesis_block = Blockchain::produce_genesis_block(root_accounts.clone(), &sk1);
        let mut blockchain = Blockchain::start(root_accounts, genesis_block);
        assert_eq!(blockchain.best_path.len(), 1);
        assert_eq!(
            blockchain.dynamic_ledger.get_balance(&sk1.get_public_key()),
            ROOT_AMOUNT
        );

        for nonce in 1..150 {
            let transaction =
                Transaction::new(&sk1, sk2.get_public_key(), transaction_amount, nonce);
            blockchain.add_transaction(transaction).unwrap();
            let new_block = mine_new_block(&blockchain, &sk1).unwrap();
            blockchain.add_block(new_block).unwrap();
        }

        assert_eq!(blockchain.best_path.len(), 150);
        blockchain.verify_chain().unwrap();
        blockchain.best_path = blockchain.best_path[..(blockchain.best_path.len() - 1)]
            .iter()
            .cloned()
            .collect();
        assert!(blockchain.verify_chain().is_err());
    }

    #[test]
    fn test_account_publishing() {
        let sk1 = SecretKey::generate();
        let sk2 = SecretKey::generate();
        let transaction_amount = Las(1);

        let root_accounts = vec![sk1.get_public_key()];
        let genesis_block = Blockchain::produce_genesis_block(root_accounts.clone(), &sk1);
        let mut blockchain = Blockchain::start(root_accounts, genesis_block);
        assert_eq!(blockchain.best_path.len(), 1);
        assert_eq!(
            blockchain.dynamic_ledger.get_balance(&sk1.get_public_key()),
            ROOT_AMOUNT
        );

        for nonce in 0..50 {
            if nonce == 5 {
                let transaction =
                    Transaction::new(&sk1, sk2.get_public_key(), transaction_amount, nonce);
                blockchain.add_transaction(transaction).unwrap();
            }
            let new_block = mine_new_block(&blockchain, &sk1).unwrap();
            blockchain.add_block(new_block).unwrap();
        }

        assert!(!blockchain.static_ledger.can_stake(&sk2.get_public_key()));

        for _ in 0..50 {
            let new_block = mine_new_block(&blockchain, &sk1).unwrap();
            blockchain.add_block(new_block).unwrap();
        }

        assert!(!blockchain.static_ledger.can_stake(&sk2.get_public_key()));
    }
}

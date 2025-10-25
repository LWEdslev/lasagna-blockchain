use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::{draw::{Draw, Seed}, keys::{PublicKey, SecretKey, Signature}, transaction::Transaction, util::{hash, BlockPtr, SerToBytes, Sha256Hash, Timeslot}};
use anyhow::{anyhow, Result};

#[derive(Debug, Clone, Serialize, Deserialize, Eq)]
pub struct Block {
    pub timeslot: Timeslot,
    pub prev_hash: Sha256Hash,
    pub depth: i64,
    pub transactions: Vec<Transaction>,
    pub draw: Draw,
    pub signature: Signature,
    pub hash: Sha256Hash,
}

impl Block {
    pub fn new(
        timeslot: Timeslot,
        prev_hash: Sha256Hash,
        depth: i64,
        transactions: Vec<Transaction>,
        sk: &SecretKey,
        seed: Seed,
    ) -> Self {
        let draw = Draw::new(timeslot, seed, sk);
        let data = (timeslot, prev_hash, depth, &draw, &transactions).into_bytes();
        let hash = hash(&data);
        let signature = Signature::sign(sk, &hash);
        Self {
            timeslot,
            prev_hash,
            depth,
            transactions,
            draw,
            signature,
            hash,
        }
    } 
    
    pub fn verify_signature(&self) -> Result<()> {
        let timeslot = self.timeslot;
        let prev_hash = self.prev_hash;
        let depth = self.depth;
        let draw = &self.draw;
        let transactions = &self.transactions;
    
        let data = (timeslot, prev_hash, depth, draw, transactions).into_bytes();
        let hash = hash(&data);
        if hash != self.hash {
            return Err(anyhow!("Computed hash does not match provided hash"));
        }

        self.signature.verify(&self.draw.signed_by, &hash)
    }

    pub fn verify_transactions(&self, prev_transactions: &HashSet<Sha256Hash>) -> Result<()> {
        if !self.transactions.iter().all(|t| {
            t.verify_signature().is_ok() && !prev_transactions.contains(&t.hash)
        }) {
            return Err(anyhow!("Unable to verify some signatures"))
        }
        
        Ok(())
    }

    pub fn verify_all(&self, prev_transactions: &HashSet<Sha256Hash>) -> Result<()> {
        self.verify_signature()?;
        self.verify_signature()?;
        self.verify_transactions(prev_transactions)?;
        Ok(())
    }

    pub fn verify_geneis(&self, root_accounts: &Vec<PublicKey>) -> Result<()> {
        let genesis_hash = Self::produce_genesis_hash(root_accounts);
        if !self.transactions.is_empty() {
            return Err(anyhow!("Transactions can't be in the genesis block"));
        }

        if self.prev_hash != genesis_hash {
            return Err(anyhow!("Seed hash does not match root accounts"));
        }
        
        self.verify_signature()
    }

    pub fn is_genesis(&self) -> bool {
        self.depth == 0
    }

    pub fn produce_genesis_hash(root_accounts: &Vec<PublicKey>) -> Sha256Hash {
        let data = root_accounts.iter().map(|accnt| accnt.into_bytes()).flatten().collect::<Vec<u8>>();
        let seed_hash = hash(&data);
        seed_hash
    }

    pub fn ptr(&self) -> BlockPtr {
        BlockPtr::new(self.hash, self.depth)
    }
}

impl PartialEq for Block {
    fn eq(&self, other: &Self) -> bool {
        self.hash == other.hash
    }
}

impl PartialOrd for Block {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        use std::cmp::Ordering::*;
        
        if self.timeslot < other.timeslot {
            return Some(Greater)
        }
        if self.timeslot > other.timeslot {
            return Some(Less)
        }

        if self.transactions.len() > other.transactions.len() {
            return Some(Greater)
        }
        else if self.transactions.len() > other.transactions.len() {
            return Some(Less);
        }

        // Third tiebreak, lexicographically hash
        Some(self.hash.cmp(&other.hash))
    }
}

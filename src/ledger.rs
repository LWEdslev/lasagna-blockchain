use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::{
    blockchain::{BLOCK_REWARD, TRANSACTION_FEE}, draw::SEED_AGE, keys::PublicKey, transaction::Transaction, util::{MiniLas, Sha256Hash}
};
use anyhow::{anyhow, Result};

// You must have this much and h SEED_AGE blocks to be considered stakable
pub const MINIMUM_STAKE_AMOUNT: MiniLas = 10_000000;

#[derive(Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct Ledger {
    pub map: HashMap<PublicKey, MiniLas>,
    pub previous_transactions: HashSet<Sha256Hash>,
    pub published_accounts: HashMap<PublicKey, i64>, // Maps to the depth where the account was published
    pub root_accounts: Vec<PublicKey>,
}

impl Ledger {
    pub fn new(root_accounts: Vec<PublicKey>) -> Self {
        let stakeable_accounts = root_accounts.iter().map(|ra| (ra.clone(), 0)).collect();
        Self {
            map: Default::default(),
            previous_transactions: Default::default(),
            published_accounts: stakeable_accounts,
            root_accounts,
        }
    }

    pub fn process_transaction(&mut self, transaction: &Transaction, depth: i64) -> Result<()> {
        transaction.verify_signature()?;        
        
        let amount = transaction.amount;
        
        if amount < TRANSACTION_FEE {
            return Err(anyhow!("Cannot send less than transaction fee. Tried to send {}, fee {}", transaction.amount, TRANSACTION_FEE));
        }

        let from = &transaction.from;
        let to = &transaction.to;

        self.add_acount_if_absent(from);
        self.add_acount_if_absent(to);

        let from_balance = self.map.get_mut(from).unwrap();

        if *from_balance < amount + TRANSACTION_FEE {
            return Err(anyhow!("Cannot send more than in account, including transaction fee"));
        }

        if !self.previous_transactions.insert(transaction.hash) {
            return Err(anyhow!("Transaction was executed previously"));
        }

        *from_balance -= amount + TRANSACTION_FEE;
        
        let to_balance = self.map.get_mut(to).unwrap();
        *to_balance += amount;

        // If `to` has not been published we must check if they have enough in their account for a publish
        if !self.published_accounts.contains_key(to) && *to_balance >= MINIMUM_STAKE_AMOUNT {
            self.published_accounts.insert(to.clone(), depth);
        }

        Ok(())
    }

    pub fn rollback_transaction(&mut self, transaction: &Transaction, depth: i64) {
        let from = &transaction.from;
        let to = &transaction.to;
        let amount = transaction.amount;

        let from_balance = self.map.get_mut(from).unwrap();
        *from_balance += amount + TRANSACTION_FEE;
        let to_balance = self.map.get_mut(from).unwrap();
        *to_balance -= amount;
        
        if let Some(published_at) = self.published_accounts.get(to) {
            let published_at = *published_at;
            if published_at == depth {
                self.published_accounts.remove(to);
            }
        }
    }

    pub fn reward_winner(&mut self, winner: &PublicKey, amount: MiniLas) {
        self.map
            .entry(winner.clone())
            .and_modify(|minilas| *minilas += amount)
            .or_insert(0);
    }

    pub fn rollback_reward(&mut self, winner: &PublicKey) {
        self.add_acount_if_absent(winner);
        let balance = self.map.get_mut(winner).unwrap();
        *balance -= BLOCK_REWARD;
    }

    pub fn add_acount_if_absent(&mut self, account: &PublicKey) {
        if !self.map.contains_key(account) {
            self.map.insert(account.clone(), 0);
        }
    }

    pub fn get_balance(&self, account: &PublicKey) -> u64 {
        *self.map.get(account).unwrap_or(&0)
    }

    pub fn can_stake(&self, account: &PublicKey, at_depth: i64) -> bool {
        if self.root_accounts.contains(account) {
            return true; // root accounts can stake immediately
        }

        let Some(publ_depth) = self.published_accounts.get(account) else {
            return false;
        };

        at_depth - publ_depth > 2 * SEED_AGE
    }

    pub fn get_total_money_in_ledger(&self) -> MiniLas {
        self.map.values().sum()
    }
}

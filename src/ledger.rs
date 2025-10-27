use std::{collections::{HashMap, HashSet}};

use serde::{Deserialize, Serialize};

use crate::{
    blockchain::TRANSACTION_FEE, draw::SEED_AGE, instruction::CompiledInstruction, keys::PublicKey, message::{TransactionMessage}, transaction::Transaction, util::{MiniLas, Sha256Hash}, snapshot::{Snapshot}
};
use anyhow::{anyhow, ensure, Result};

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
     
    pub fn is_transaction_valid(&self, transaction: &Transaction) -> Result<()> {
        transaction.validate()?;

        if self.previous_transactions.contains(&transaction.hash) {
            return Err(anyhow!("Transaction was executed previously"));
        }

        Ok(())
    }

    pub fn process_transaction(&mut self, transaction: &Transaction, depth: i64) -> Result<()> {
        self.is_transaction_valid(transaction)?;

        // Snapshot the accounts in the transaction before executing the transaction
        let mut snapshot = Snapshot::new();
        for pk in &transaction.message.accounts {
            snapshot.snapshot_balance(&pk, &self.map);
        }

        let payer = transaction.message.accounts.get(0).unwrap();
        self.add_acount_if_absent(payer);
        let payer_balance = self.map.get_mut(payer).unwrap();

        ensure!(*payer_balance > TRANSACTION_FEE, "Payer does not have enough LAS in account to pay transaction fee");

        *payer_balance -= TRANSACTION_FEE;


        if !self.previous_transactions.insert(transaction.hash) {
            return Err(anyhow!("Transaction was executed previously"));
        }

        for ix in &transaction.message.instructions {
            let result = self.process_instruction(ix, &transaction.message, depth);
            match result {
                Ok(_) => (),
                Err(e) => {
                    self.rollback_to_snapshot(&snapshot, transaction);
                    return Err(anyhow!(e));
                },
            }
        }

        Ok(())
    }

    fn process_instruction(&mut self, instruction: &CompiledInstruction, message: &TransactionMessage, depth: i64) -> Result<()>{
        let from_idx = instruction.account_indices.get(0).unwrap();
        let to_idx = instruction.account_indices.get(1).unwrap();

        let from = message.accounts.get(*from_idx).ok_or_else(|| anyhow!("Failed to get sending public key during instruction processing"))?;
        let to = message.accounts.get(*to_idx).ok_or_else(|| anyhow!("Failed to get receiving public key during instruction processing"))?;

        self.add_acount_if_absent(from);
        self.add_acount_if_absent(to);

        let from_balance = self.map.get_mut(from).unwrap();

        if *from_balance < instruction.amount {
            return Err(anyhow!("The sender does not have enoug MiniLas to perform the instruction"));
        }

        *from_balance -= instruction.amount;

        let to_balance = self.map.get_mut(to).unwrap();
        
        *to_balance += instruction.amount;

        // If `to` has not been published we must check if they have enough in their account for a publish
        if !self.published_accounts.contains_key(to) && *to_balance >= MINIMUM_STAKE_AMOUNT {
            self.published_accounts.insert(to.clone(), depth);
        }


        Ok(())
    }

    fn rollback_to_snapshot(&mut self, snapshot: &Snapshot, transaction: &Transaction){
        println!("rolling back");
        for (pk, amount) in &snapshot.balances {
            match amount {
                Some(a) => {
                    let balance = self.map.get_mut(pk).unwrap();
                    *balance = *a
                },
                None => {
                    self.delete_account(pk);
                }
            }
            
        }

        self.previous_transactions.remove(&transaction.hash);
    }

    pub fn rollback_transaction(&mut self, transaction: &Transaction, depth: i64) {
        for ix in &transaction.message.instructions {
            self.rollback_instruction(&ix, &transaction.message);
        }

        self.previous_transactions.remove(&transaction.hash);

        for pk in &transaction.message.accounts {
            if let Some(published_at) = self.published_accounts.get(&pk) {
                let published_at = *published_at;
                if published_at == depth {
                    self.published_accounts.remove(&pk);
                }
            }
        }
    }

    pub fn rollback_instruction(&mut self, instruction: &CompiledInstruction, message: &TransactionMessage) {
        let from_idx = instruction.account_indices.get(0).unwrap();
        let to_idx = instruction.account_indices.get(1).unwrap();

        let from = message.accounts.get(*from_idx).unwrap();
        let to = message.accounts.get(*to_idx).unwrap();
        let amount = instruction.amount;

        let from_balance = self.map.get_mut(from).unwrap();
        *from_balance += amount;
        let to_balance = self.map.get_mut(to).unwrap();
        *to_balance -= amount;
    }

    pub fn reward_winner(&mut self, winner: &PublicKey, amount: MiniLas) {
        self.map
            .entry(winner.clone())
            .and_modify(|minilas| *minilas += amount)
            .or_insert(amount);
    }

    pub fn rollback_reward(&mut self, winner: &PublicKey, amount: MiniLas) {
        self.add_acount_if_absent(winner);
        let balance = self.map.get_mut(winner).unwrap();
        *balance -= amount;
    }

    pub fn add_acount_if_absent(&mut self, account: &PublicKey) {
        if !self.map.contains_key(account) {
            self.map.insert(account.clone(), 0);
        }
    }

    pub fn delete_account(&mut self, account: &PublicKey){
        self.published_accounts.remove(account);
        self.map.remove(account);
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

#[cfg(test)]
mod tests {
    use crate::{instruction::Instruction, keys::SecretKey};

    use super::*;

    #[test]
    fn test_transfer_should_succeed(){
        let sk1 = SecretKey::generate();
        let sk2 = SecretKey::generate();

        let root_accounts = Vec::from([sk1.get_public_key(), sk2.get_public_key()]);
        let mut ledger = Ledger::new(root_accounts);

        ledger.add_acount_if_absent(&sk1.get_public_key());
        let reward = 1000000;
        ledger.reward_winner(&sk1.get_public_key(), reward);

        let sk1_balance = ledger.get_balance(&sk1.get_public_key());
        assert_eq!(sk1_balance, reward);

        let transfered_amount = 100001;
        let ix = Instruction::new(sk1.get_public_key(), sk2.get_public_key(), transfered_amount);
        let ixs = Vec::from([ix]);

        let signers = Vec::from([sk1.clone()]);
        let tx = Transaction::new(signers, &ixs, 1).unwrap();

        let result = ledger.process_transaction(&tx, 1);

        assert!(result.is_ok());

        let sk1_balance = ledger.get_balance(&sk1.get_public_key());
        let sk2_balance = ledger.get_balance(&sk2.get_public_key());
        assert_eq!(reward - (transfered_amount + TRANSACTION_FEE), sk1_balance);
        assert_eq!(transfered_amount, sk2_balance);
    }

    #[test]
    fn test_transfer_should_rollback(){
        let sk1 = SecretKey::generate();
        let sk2 = SecretKey::generate();

        let root_accounts = Vec::from([sk1.get_public_key(), sk2.get_public_key()]);
        let mut ledger = Ledger::new(root_accounts);

        ledger.add_acount_if_absent(&sk1.get_public_key());
        let reward = 100000;
        ledger.reward_winner(&sk1.get_public_key(), reward);

        let sk1_balance = ledger.get_balance(&sk1.get_public_key());
        assert_eq!(sk1_balance, reward);

        let transfered_amount = 10000;
        let ix = Instruction::new(sk1.get_public_key(), sk2.get_public_key(), transfered_amount); 

        let transfered_amount2 = 100001;
        let ix2 = Instruction::new(sk1.get_public_key(), sk2.get_public_key(), transfered_amount2);
        let ixs = Vec::from([ix, ix2]);

        let signers = Vec::from([sk1.clone()]);
        let tx = Transaction::new(signers, &ixs, 1).unwrap();

        let result = ledger.process_transaction(&tx, 1);

        assert!(result.is_err());

        let sk1_balance = ledger.get_balance(&sk1.get_public_key());
        let sk2_balance = ledger.get_balance(&sk2.get_public_key());

        // The transaction fee is still deducted even though the transaction failed
        assert_eq!(reward - TRANSACTION_FEE, sk1_balance);
        assert_eq!(0, sk2_balance);
    }
}

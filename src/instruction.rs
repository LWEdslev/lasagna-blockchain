use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

use crate::{blockchain::TRANSACTION_FEE, instruction, keys::PublicKey, util::Sha256Hash};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Instruction{
    // A list of accounts (public keys) needed to process the instruction
    // As long as the blockchain only supports native token transfers, this list will only contain 2 accounts
    // The first account is the sender and the second account is the receiver
    pub accounts: Vec<PublicKey>,
    pub amount: u64, 
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct CompiledInstruction{
    // A list of account indexes with the index where to find the public key in the accounts list on the TransactionMessage
    // As long as the blockchain only supports native token transfers, this list will only contain 2 accounts
    // The first account is the sender and the second account is the receiver
    // The index of the sender is also the index where the signatures list on the transaction stores the signature that the sender has signed
    pub account_indices: Vec<usize>,
    pub amount: u64,
}

impl Instruction {
    pub fn new(public_keys: Vec<PublicKey>, amount: u64) -> Self {
        Self { accounts: public_keys, amount }
    }
}

impl CompiledInstruction{
    pub fn new(public_keys_index: Vec<usize>, instruction: &Instruction) -> Self {
        Self { account_indices: public_keys_index, amount: instruction.amount }
    }

    pub fn validate(&self) -> Result<()>{
        let num_pks = self.account_indices.len();

        if num_pks != 2 {
            return Err(anyhow!("Instructions need to have exactly 2 pks, one for sending and one for receiving"))
        }

        if self.amount < TRANSACTION_FEE {
            return Err(anyhow!("Transfer can not be smaller than the transaction fee"))
        }

        Ok(())

    }
}
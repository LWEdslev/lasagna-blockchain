use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use anyhow::{anyhow, ensure, Result};

use crate::{instruction::{CompiledInstruction, Instruction}, keys::{PublicKey, SecretKey}, util::Sha256Hash};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct TransactionMessageHeader{
    pub num_required_signatures: u8,
    pub num_required_public_keys: u8,
}

impl TransactionMessageHeader{
    pub fn new(num_required_signatures: u8, num_required_public_keys: u8) -> Self {
        Self{
            num_required_signatures: num_required_signatures,
            num_required_public_keys: num_required_public_keys,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct TransactionMessage{
    pub header: TransactionMessageHeader,
    // A list of accounts (public keys) that appear in the instructions
    // The account on index 0 is the payer for the transaction and does not need to appear in any instruction
    pub accounts: Vec<PublicKey>, 
    pub instructions: Vec<CompiledInstruction>
}

impl TransactionMessage{
    pub fn new(
        signers: &Vec<SecretKey>,
        instrs: &Vec<Instruction>,
    ) -> Self {
        let mut public_keys: Vec<PublicKey> = Vec::new();
        let mut key_index: HashMap<PublicKey, usize> = HashMap::new();
        let mut num_required_public_keys: u8 = 0;
        let mut intern = |pk: &PublicKey| -> usize {
            if let Some(&i) = key_index.get(pk) {
                i
            } else {
                num_required_public_keys += 1;
                let i = public_keys.len() as usize;
                public_keys.push(pk.clone());
                key_index.insert(pk.clone(), i);
                i
            }
        };

        // make sure the signers are the in the same order in the accounts list as the signature list on the transaction
        signers.iter().for_each(|sk| {
            intern(&sk.get_public_key());
        });

        let mut compiled_instructions: Vec<CompiledInstruction> = Vec::with_capacity(instrs.len());
        for ix in instrs {
            let mut acct_indices = Vec::with_capacity(ix.accounts.len());
            for pk in &ix.accounts {
                let idx = intern(&pk);
                acct_indices.push(idx);
            }

            compiled_instructions.push(CompiledInstruction::new(acct_indices, ix));
        }

        let header = TransactionMessageHeader::new(signers.len() as u8, num_required_public_keys);

        Self {
            header,
            accounts: public_keys,
            instructions: compiled_instructions,
        }
    }

    pub fn validate(&self) -> Result<()> {
        let num_instructions = self.instructions.len();
        if num_instructions + 1 != self.header.num_required_signatures as usize {
            return Err(anyhow!("The required amount of signatures was {}, expected {}", self.header.num_required_signatures, num_instructions));
        }

        self.validate_public_keys()?;

        Ok(())
    }

    pub fn validate_public_keys(&self) -> Result<()> {
        let num_required_keys = self.header.num_required_public_keys;
        let actual_key_amount = self.accounts.len();

        ensure!(num_required_keys as usize == actual_key_amount, "The message contained {} public keys, but expected {}", actual_key_amount, num_required_keys);

        Ok(())
    }
}
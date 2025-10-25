use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use anyhow::{Result, anyhow};

use crate::{instruction::{CompiledInstruction, Instruction}, keys::{PublicKey, SecretKey}, message, util::Sha256Hash};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct TransactionMessageHeader{
    pub num_required_signatures: u8,
}

impl TransactionMessageHeader{
    pub fn new() -> Self {
        Self { num_required_signatures: 0 }
    }

    pub fn from_instructions(instructions: &Vec<CompiledInstruction>) -> Self {
        let num_instructions = instructions.len();

        Self{
            num_required_signatures: (num_instructions + 1) as u8
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct TransactionMessage{
    pub header: TransactionMessageHeader,
    pub public_keys: Vec<PublicKey>, // The first public key is the payer for the transaction and does not need to appear in any instruction
    pub recent_blockhash: Sha256Hash,
    pub instructions: Vec<CompiledInstruction>
}

impl TransactionMessage{
    pub fn new(
        payer: PublicKey,
        instrs: Vec<Instruction>,
        recent_blockhash: [u8; 32],
    ) -> Self {
        // 1) Canonical account list + index (payer first)
        let mut public_keys: Vec<PublicKey> = Vec::new();
        let mut key_index: HashMap<PublicKey, usize> = HashMap::new();
        let mut intern = |pk: &PublicKey| -> usize {
            if let Some(&i) = key_index.get(pk) {
                i
            } else {
                let i = public_keys.len() as usize;
                public_keys.push(pk.clone());
                key_index.insert(pk.clone(), i);
                i
            }
        };

        // Ensure payer at index 0
        intern(&payer);

        // 3) Compile instructions
        let mut compiled_instructions: Vec<CompiledInstruction> = Vec::with_capacity(instrs.len());
        for ix in &instrs {
            // accounts
            let mut acct_indices = Vec::with_capacity(ix.public_keys.len());
            for pk in &ix.public_keys {
                let idx = intern(&pk);
                acct_indices.push(idx);
            }

            compiled_instructions.push(CompiledInstruction::new(acct_indices, ix));
        }

        let header = TransactionMessageHeader::from_instructions(&compiled_instructions);

        Self {
            header,
            public_keys,
            recent_blockhash,
            instructions: compiled_instructions,
        }
    }

    pub fn validate(&self, expected_hash: Sha256Hash) -> Result<()> {
        let num_instructions = self.instructions.len();
        if num_instructions + 1 != self.header.num_required_signatures as usize {
            return Err(anyhow!("The required amount of signatures was {}, expected {}", self.header.num_required_signatures, num_instructions));
        }

        self.validate_recent_block_hash(expected_hash)?;
        self.validate_public_keys()?;

        Ok(())
    }

    fn validate_recent_block_hash(&self, expected_hash: Sha256Hash) -> Result<()> {
        if self.recent_blockhash != expected_hash {
            return Err(anyhow!("The recent block hash did not match the expected block hash"));
        }

        Ok(())
    }

    fn validate_public_keys(&self) -> Result<()> {
        todo!()
    }
}
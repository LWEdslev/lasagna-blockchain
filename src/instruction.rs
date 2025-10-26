use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

use crate::{blockchain::TRANSACTION_FEE, instruction, keys::PublicKey, util::Sha256Hash};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Instruction{
    pub public_keys: Vec<PublicKey>, // The first public key is the message signer
    pub amount: u64, 
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct CompiledInstruction{
    pub public_keys_index: Vec<usize>, // The first public key is the message signer
    pub amount: u64,
}

impl Instruction {
    pub fn new(public_keys: Vec<PublicKey>, amount: u64) -> Self {
        Self { public_keys, amount }
    }
}

impl CompiledInstruction{
    pub fn new(public_keys_index: Vec<usize>, instruction: &Instruction) -> Self {
        Self { public_keys_index, amount: instruction.amount }
    }

    // This validate assumes that instructions can only send LAS, rewrite when more functionality is applied
    pub fn validate(&self) -> Result<()>{
        let num_pks = self.public_keys_index.len();
        if num_pks != 2 {
            return Err(anyhow!("Instructions need to have exactly 2 pks, one for sending and one for receiving"))
        }

        if self.amount < TRANSACTION_FEE {
            return Err(anyhow!("Transfer can not be smaller than the transaction fee"))
        }

        Ok(())

    }
}
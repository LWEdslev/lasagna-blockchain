use serde::{Deserialize, Serialize};

use crate::{instruction, keys::PublicKey, util::Sha256Hash};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Instruction{
    pub public_keys: Vec<PublicKey>, // The first public key is the message signer
    pub program_id: Sha256Hash,
    pub data: Vec<u8>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct CompiledInstruction{
    pub public_keys_index: Vec<usize>, // The first public key is the message signer
    pub program_id: Sha256Hash,
    pub data: Vec<u8>,
}

impl Instruction {
    pub fn new(public_keys: Vec<PublicKey>, program_id: Sha256Hash, data: Vec<u8>) -> Self {
        Self { public_keys, program_id, data }
    }
}

impl CompiledInstruction{
    pub fn new(public_keys_index: Vec<usize>, instruction: &Instruction) -> Self {
        Self { public_keys_index, program_id: instruction.program_id, data: instruction.data.clone() }
    }
}
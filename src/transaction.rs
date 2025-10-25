use anyhow::{Result, anyhow};
use ed25519_dalek::ed25519::signature;
use serde::{Deserialize, Serialize};

use crate::{
    instruction, keys::{PublicKey, SecretKey, Signature}, message::{self, TransactionMessage}, util::{hash, SerToBytes, Sha256Hash}
};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Transaction {
    pub hash: Sha256Hash,
    pub message: TransactionMessage,
    pub signatures: Vec<Signature> // The first signature is the payer for the transaction and does not need to appear in any instruction
}

impl Transaction {
    pub fn new(message: TransactionMessage, signers: Vec<SecretKey>) -> Result<Self> {
        let message_bytes = message.into_bytes();
        let signatures: Vec<Signature> = signers.iter().map(|sk| Signature::sign(sk, &message_bytes)).collect();
        
        let hash = hash(&(message_bytes, signatures.clone()).into_bytes());

        Ok(Self{ hash, message, signatures })
    }

    pub fn verify_signatures(&self) -> Result<()> {
        let message_bytes = &self.message.into_bytes();

        // Verify payer signature
        let payer = self.message.public_keys.get(0).unwrap();
        let payer_signature = self.signatures.get(0).unwrap();
        payer_signature.verify(payer, &message_bytes)?;

        // Verify instruction signatures
        for (index, instruction) in self.message.instructions.iter().enumerate() {
            let pk_index = instruction.public_keys_index.get(0).ok_or_else(|| anyhow!("No signer was provided for instruction"))?;
            let pk = self.message.public_keys.get(index + 1).ok_or_else(|| anyhow!("No public key was provided for the signer of the instruction"))?;
            let signature = self.signatures.get(*pk_index).ok_or_else(|| anyhow!("No Signature was provided for the signer of the instruction"))?;
            signature.verify(pk, &message_bytes)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::instruction::Instruction;

    use super::*;

    #[test]
    fn test_payer_signature_should_succeed(){
        let sk1 = SecretKey::generate();
        let payer = sk1.get_public_key();
        let instrs: Vec<Instruction> = Vec::new();
        let hash = hash(&"Test".into_bytes());

        let message = TransactionMessage::new(payer, instrs, hash);

        let signers = Vec::from([sk1]);
        let tx = Transaction::new(message, signers).unwrap();
        
        assert!(!tx.verify_signatures().is_err());
    }

    #[test]
    fn test_payer_signature_should_fail(){
        let sk1 = SecretKey::generate();
        let sk2 = SecretKey::generate();
        let payer = sk1.get_public_key();
        let ixs: Vec<Instruction> = Vec::new();
        let hash = hash(&"Test".into_bytes());

        let message = TransactionMessage::new(payer, ixs, hash);

        let signers = Vec::from([sk1]);
        let mut tx = Transaction::new(message, signers).unwrap();
        
        assert!(!tx.verify_signatures().is_err());

        tx.message.public_keys = Vec::from([sk2.get_public_key()]);

        assert!(tx.verify_signatures().is_err())
    }

    #[test]
    fn test_instruction_signature_should_succeed() {
        let sk1 = SecretKey::generate();
        let sk2 = SecretKey::generate();
        let payer = sk1.get_public_key();

        // note the signing key is firs in the list
        let pks = Vec::from([sk2.get_public_key(), sk1.get_public_key()]);
        let program_id = hash(&"PROGRAM_ID=TOKEN_TRANSFER".into_bytes());
        let minilas: u64 = 100000;
        let data = minilas.into_bytes();
        let ix = Instruction::new(pks, program_id, data);
        let ixs = Vec::from([ix]);
        let test_block_hash = hash(&"recent_block".into_bytes());
        
        // note the payer is first in the list of signers
        let signers = Vec::from([sk1, sk2]);
        let message = TransactionMessage::new(payer, ixs, test_block_hash);
        let tx = Transaction::new(message, signers).unwrap();
        
        assert!(!tx.verify_signatures().is_err())
    }

    #[test]
    fn test_instruction_signature_should_fail() {
        let sk1 = SecretKey::generate();
        let sk2 = SecretKey::generate();
        let payer = sk1.get_public_key();

        // The signing key is not first in the list, so the test signature cannot be verified
        let pks = Vec::from([sk1.get_public_key(), sk2.get_public_key()]);
        let program_id = hash(&"PROGRAM_ID=TOKEN_TRANSFER".into_bytes());
        let minilas: u64 = 100000;
        let data = minilas.into_bytes();
        let ix = Instruction::new(pks, program_id, data);
        let ixs = Vec::from([ix]);
        let test_block_hash = hash(&"recent_block".into_bytes());
        
        let signers = Vec::from([sk1.clone(), sk2.clone()]);
        let message = TransactionMessage::new(payer.clone(), ixs.clone(), test_block_hash);
        let tx = Transaction::new(message, signers).unwrap();
        
        assert!(tx.verify_signatures().is_err());

        let signers = Vec::from([sk2, sk1]);
        let message = TransactionMessage::new(payer, ixs, test_block_hash);
        let tx = Transaction::new(message, signers).unwrap();

        assert!(tx.verify_signatures().is_err());
    }
}
use anyhow::{anyhow, ensure, Result};
use ed25519_dalek::ed25519::signature;
use serde::{Deserialize, Serialize};

use crate::{
    instruction::{self, Instruction}, keys::{PublicKey, SecretKey, Signature}, message::{self, TransactionMessage}, util::{hash, SerToBytes, Sha256Hash}
};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Transaction {
    pub hash: Sha256Hash,
    pub message: TransactionMessage,
    pub signatures: Vec<Signature> // The first signature is the payer for the transaction and does not need to appear in any instruction
}

impl Transaction {
    pub fn new(signers: Vec<SecretKey>, instructions: &Vec<Instruction>, recent_blockhash: Sha256Hash) -> Result<Self> {
        let mut signer_public_keys = Vec::new();
        for signer in &signers {
            signer_public_keys.push(signer.get_public_key());
        }
        
        let message = TransactionMessage::new(&signers, instructions, recent_blockhash);
        let message_bytes = message.into_bytes();
        let signatures: Vec<Signature> = signers.iter().map(|sk| Signature::sign(sk, &message_bytes)).collect();
        
        let hash = hash(&(message_bytes, signatures.clone()).into_bytes());

        Ok(Self{ hash, message, signatures })
    }

    pub fn verify_signatures(&self) -> Result<()> {
        let message_bytes = &self.message.into_bytes();

        let required_signature_amount = self.message.header.num_required_signatures;
        let num_signatures = self.signatures.len();
        ensure!(required_signature_amount as usize == num_signatures, "Transaction requires {} signatures, but only has {}", required_signature_amount, num_signatures);

        println!("made it through");

        // Verify payer signature
        let payer = self.message.public_keys.get(0).unwrap();
        let payer_signature = self.signatures.get(0).unwrap();
        payer_signature.verify(payer, &message_bytes)?;

        // Verify instruction signatures
        for (index, instruction) in self.message.instructions.iter().enumerate() {
            println!("{}", index);
            let pk_index = instruction.public_keys_index.get(0).ok_or_else(|| anyhow!("No signer was provided for instruction"))?;
            println!("pk_index {}", pk_index);
            let pk = self.message.public_keys.get(*pk_index).ok_or_else(|| anyhow!("No public key was provided for the signer of the instruction"))?;
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
        let ixs: Vec<Instruction> = Vec::new();
        let hash = hash(&"Test".into_bytes());


        let signers = Vec::from([sk1]);
        let tx = Transaction::new(signers, &ixs, hash).unwrap();
        
        assert!(!tx.verify_signatures().is_err());
    }

    #[test]
    fn test_payer_signature_should_fail(){
        let sk1 = SecretKey::generate();
        let sk2 = SecretKey::generate();
        let ixs: Vec<Instruction> = Vec::new();
        let hash = hash(&"Test".into_bytes());

        let signers = Vec::from([sk1]);
        let mut tx = Transaction::new(signers, &ixs, hash).unwrap();
        
        assert!(!tx.verify_signatures().is_err());

        tx.message.public_keys = Vec::from([sk2.get_public_key()]);

        assert!(tx.verify_signatures().is_err())
    }

    #[test]
    fn test_instruction_signature_should_succeed() {
        let sk1 = SecretKey::generate();
        let sk2 = SecretKey::generate();

        // note the signing key is firs in the list
        let pks = Vec::from([sk2.get_public_key(), sk1.get_public_key()]);
        let minilas: u64 = 100000;
        let ix = Instruction::new(pks, minilas);
        let ixs = Vec::from([ix]);
        let test_block_hash = hash(&"recent_block".into_bytes());
        
        // note the payer is first in the list of signers
        let signers = Vec::from([sk1, sk2]);
        let tx = Transaction::new(signers, &ixs, test_block_hash).unwrap();
        
        assert!(!tx.verify_signatures().is_err())
    }

    #[test]
    fn test_instruction_signature_should_fail() {
        let sk1 = SecretKey::generate();
        let sk2 = SecretKey::generate();

        // The signing key is not first in the list, so the test signature cannot be verified
        let pks = Vec::from([sk1.get_public_key(), sk2.get_public_key()]);
        let minilas: u64 = 100000;
        let ix = Instruction::new(pks, minilas);
        let ixs = Vec::from([ix]);
        let test_block_hash = hash(&"recent_block".into_bytes());
        
        let signers = Vec::from([sk2.clone()]);
        let tx = Transaction::new(signers, &ixs, test_block_hash).unwrap();

        println!("tx {:?}", tx.signatures.len());
        
        assert!(tx.verify_signatures().is_err());

        // Insert the correct signer (the sender) and the transaction should no longer fail
        let signers = Vec::from([sk1]);
        let tx = Transaction::new(signers, &ixs, test_block_hash).unwrap();

        assert!(!tx.verify_signatures().is_err());
    }

    #[test]
    fn test_instructions_signature_should_succeed() {
        // Generate keys
        let sk1 = SecretKey::generate();
        let sk2 = SecretKey::generate();
        let sk3 = SecretKey::generate();
        let sk4 = SecretKey::generate();
        let sk5 = SecretKey::generate();

        let minilas: u64 = 100000;

        // Create instructions, index 0 is the sender and index 1 is the receiver
        let ix = Instruction::new(Vec::from([sk1.get_public_key(), sk2.get_public_key()]), minilas);
        let ix2 = Instruction::new(Vec::from([sk3.get_public_key(), sk4.get_public_key()]), minilas);
        let ix3 = Instruction::new(Vec::from([sk1.get_public_key(), sk4.get_public_key()]), minilas);
        let ix4 = Instruction::new(Vec::from([sk2.get_public_key(), sk3.get_public_key()]), minilas);
        let ix5 = Instruction::new(Vec::from([sk3.get_public_key(), sk1.get_public_key()]), minilas);
        let ix6 = Instruction::new(Vec::from([sk4.get_public_key(), sk2.get_public_key()]), minilas);
        let ix7 = Instruction::new(Vec::from([sk2.get_public_key(), sk1.get_public_key()]), minilas);
        let ix8 = Instruction::new(Vec::from([sk3.get_public_key(), sk4.get_public_key()]), minilas);
        let ix9 = Instruction::new(Vec::from([sk2.get_public_key(), sk4.get_public_key()]), minilas);
        let ix10 = Instruction::new(Vec::from([sk1.get_public_key(), sk5.get_public_key()]), minilas);
        let ixs = Vec::from([ix, ix2, ix3, ix4, ix5, ix6, ix7, ix8, ix9, ix10]);

        let test_block_hash = hash(&"recent_block".into_bytes());
        
        // create transaction, the signer on index 0 is the payer
        let signers = Vec::from([sk1.clone(), sk2.clone(), sk3.clone(), sk4.clone()]);
        let tx = Transaction::new(signers, &ixs, test_block_hash).unwrap();

        assert!(!tx.verify_signatures().is_err());

        // Remove one of the senders from the signers list
        // The transaction can no longer be verified because a sender has not signed
        let signers = Vec::from([sk1, sk2, sk3]);
        let tx = Transaction::new(signers, &ixs, test_block_hash).unwrap();

        assert!(tx.verify_signatures().is_err());


    }
}
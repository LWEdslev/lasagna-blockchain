use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::{
    keys::{PublicKey, SecretKey, Signature},
    util::{hash, SerToBytes, Sha256Hash},
};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Transaction {
    pub from: PublicKey,
    pub to: PublicKey,
    pub amount: u64,
    pub nonce: u64,
    pub signature: Signature,
    pub hash: Sha256Hash,
}

impl Transaction {
    pub fn new(from: &SecretKey, to: PublicKey, amount: u64, nonce: u64) -> Self {
        let from_pk = from.get_public_key().clone();
        let public_values = ("Transaction", &from_pk, &to, amount, nonce);
        let signature = Signature::sign(from, &public_values.into_bytes());
        
        let hash = hash(&(public_values, signature.clone()).into_bytes());

        Self {
            from: from_pk,
            to: to.clone(),
            amount,
            nonce,
            signature,
            hash,
        }
    }

    pub fn verify_signature(&self) -> Result<()> {
        let public_values = ("Transaction", &self.from, &self.to, self.amount, self.nonce);
        self.signature.verify(&self.from, &public_values.into_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_signature() {
        let sk1 = SecretKey::generate();

        let sk2 = SecretKey::generate();
        let pk2 = sk2.get_public_key();
        let mut transaction = Transaction::new(&sk1, pk2, 42, 1);

        transaction.verify_signature().unwrap();

        transaction.amount = 41;

        assert!(transaction.verify_signature().is_err());
    }
}
use std::hash::Hash;

use ed25519_dalek::{ed25519::signature::Signer, SigningKey};
use ed25519_dalek::Verifier;
use rand::{rng};
use serde::{Deserialize, Serialize};
use anyhow::Result;


#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct PublicKey(ed25519_dalek::VerifyingKey);

impl From<ed25519_dalek::VerifyingKey> for PublicKey {
    fn from(value: ed25519_dalek::VerifyingKey) -> Self {
        Self(value)
    }
}

impl From<iroh::PublicKey> for PublicKey {
    fn from(value: iroh::PublicKey) -> Self {
        Self(value.public())
    }
}

impl Hash for PublicKey {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.as_bytes().hash(state);
    }
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct SecretKey(ed25519_dalek::SigningKey);

impl SecretKey {
    pub fn get_public_key(&self) -> PublicKey {
        PublicKey(self.0.verifying_key())
    }

    pub fn generate() -> Self {
        SigningKey::generate(&mut rng()).into()
    }
}

impl Hash for SecretKey {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.as_bytes().hash(state);
    }
}

impl From<ed25519_dalek::SigningKey> for SecretKey {
    fn from(value: ed25519_dalek::SigningKey) -> Self {
        Self(value)
    }
}

impl From<iroh::SecretKey> for SecretKey {
    fn from(value: iroh::SecretKey) -> Self {
        Self(value.secret().clone().into())
    }
}


#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct Signature(ed25519_dalek::Signature);

impl Signature {
    pub fn sign(sk: &SecretKey, data: &[u8]) -> Signature {
        Signature(sk.0.sign(data))
    }

    pub fn verify(&self, pk: &PublicKey, data: &[u8]) -> Result<()> {
        pk.0.verify(data, &self.0).map_err(Into::into)
    }
}

impl Hash for Signature {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.to_bytes().hash(state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_signature_test() {
        let sk = SecretKey::generate();
        let pk = sk.get_public_key();

        let data = b"Hello world!";
        let signature = Signature::sign(&sk, data);

        let verification = signature.verify(&pk, data);

        verification.unwrap();
    }
}
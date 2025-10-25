use num_bigint::BigUint;
use serde::{Deserialize, Serialize};
use anyhow::Result;
use crate::{keys::{PublicKey, SecretKey, Signature}, util::{hash, BlockPtr, SerToBytes, Timeslot}};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Draw {
    pub value: BigUint,
    pub timeslot: Timeslot,
    pub signature: Signature,
    pub signed_by: PublicKey,
    pub seed: Seed,
}

impl Draw {
    pub fn new(
        timeslot: Timeslot,
        seed: Seed,
        sk: &SecretKey,
    ) -> Self {
        let data_to_sign = ("Lottery", timeslot, seed.clone()).into_bytes();
        let signature = Signature::sign(sk, &data_to_sign);
        
        let vk = sk.get_public_key();
        let data_to_hash = ("Lottery", seed.clone(), timeslot, vk, signature.clone()).into_bytes();

        let hash = hash(&data_to_hash);
        let value = BigUint::from_bytes_be(&hash);

        Self {
            value,
            timeslot,
            signature,
            signed_by: sk.get_public_key(),
            seed,
        }
    }

    pub fn verify(&self) -> Result<()> {
        let timeslot = self.timeslot;
        let seed = &self.seed;
        let data_to_sign = ("Lottery", timeslot, seed).into_bytes();
        let signature = self.signature.clone();

        let data_to_hash = ("Lottery", seed.clone(), timeslot, &self.signed_by, signature.clone()).into_bytes();
        let hash = hash(&data_to_hash);

        let value = BigUint::from_bytes_be(&hash);
        if value != self.value {
            return Err(anyhow::anyhow!("Recomputed {value} is not equal to proposed value {}", self.value));
        }

        signature.verify(&self.signed_by, &data_to_sign)
    }
}

pub const SEED_AGE: i64 = 50;

// The seed starts being the hash of the genesis block, once we reach block depth 51
// it will be depth-50, this way the seed is unpredictable
// we only allow peers to stake if they have had enough money 
// in the ledger for 100 rounds, that way they would have to predict the hash in 50 blocks
// and we consider a rollback of more than 50 blocks very unlikely so it only makes sense 
// to produce 1 single draw
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Seed {
    pub block_ptr: BlockPtr,
}

impl Seed {
    pub fn correct_age(&self, best_depth: i64) -> bool {
        let seed_age = best_depth - self.block_ptr.depth;
        seed_age == SEED_AGE
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_draw_verification() {
        let sk = SecretKey::generate();
        let seed = Seed {
            block_ptr: BlockPtr {
                hash: [0u8; 32],
                depth: 100,
            },
        };
        let draw = Draw::new(150, seed, &sk);
        assert!(draw.verify().is_ok());

        // Test tampering of the value
        let mut bad_draw = draw.clone();
        bad_draw.value += 1u32;
        assert!(bad_draw.verify().is_err());

        // Test tampering of the signature
        let mut bad_draw = draw.clone();
        bad_draw.signature = Signature::sign(&SecretKey::generate(), &("Lottery", bad_draw.timeslot, &bad_draw.seed).into_bytes());
        assert!(bad_draw.verify().is_err());
    }

    #[test]
    fn test_seed_in_range() {
        let seed = Seed {
            block_ptr: BlockPtr {
                hash: [0u8; 32],
                depth: 100,
            },
        };
        assert!(seed.correct_age(150));
        assert!(!seed.correct_age(149));
        assert!(!seed.correct_age(151));
    }
}
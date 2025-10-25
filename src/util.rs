use bincode::config::Configuration;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub type Sha256Hash = [u8; 32];

pub fn hash(bytes: &[u8]) -> Sha256Hash {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let result = hasher.finalize();
    result.into()
}

pub trait SerToBytes {
    fn into_bytes(&self) -> Vec<u8>;
}

impl<T: Serialize> SerToBytes for T {
    fn into_bytes(&self) -> Vec<u8> {
        bincode::serde::encode_to_vec::<_, Configuration>(
            self,
            bincode::config::Configuration::default(),
        )
        .expect("Unable to serialize")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BlockPtr {
    pub hash: Sha256Hash,
    pub depth: i64,
}

pub type Timeslot = u64;

#[cfg(not(test))]
pub const SLOT_LENGTH: u128 = 1_000_000;
#[cfg(test)]
pub const SLOT_LENGTH: u128 = 1; // 0.001 millisecond for testing

pub const START_TIME: u128 = 1761384740000000;

pub type MiniLas = u64; // 1 millionth of a LAS

pub fn calculate_timeslot(start_time: u128) -> Timeslot {
    let now = get_unix_timestamp();
    let start = start_time;
    let timeslot = (now - start) / SLOT_LENGTH;
    timeslot as _
}

pub fn get_unix_timestamp() -> u128 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_micros()
}
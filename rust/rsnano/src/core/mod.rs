use blake2::{
    digest::{Update, VariableOutput},
    VarBlake2b,
};

use rsnano_core::{
    utils::{Deserialize, Serialize, Stream},
    BlockHash, RawKey,
};

mod difficulty;
#[cfg(test)]
pub(crate) use difficulty::StubDifficulty;
pub use difficulty::{Difficulty, DifficultyV1, WorkVersion};

mod endpoint_key;
pub use endpoint_key::EndpointKey;

mod blocks;
pub use blocks::*;

mod unchecked_info;
pub use unchecked_info::{SignatureVerification, UncheckedInfo, UncheckedKey};

mod uniquer;
pub use uniquer::Uniquer;

mod hardened_constants;
pub mod messages;
pub(crate) use hardened_constants::HardenedConstants;

use std::net::Ipv6Addr;

pub trait FullHash {
    fn full_hash(&self) -> BlockHash;
}

#[derive(PartialEq, Eq, Debug)]
pub struct NoValue {}

impl Serialize for NoValue {
    fn serialized_size() -> usize {
        0
    }

    fn serialize(&self, _stream: &mut dyn Stream) -> anyhow::Result<()> {
        Ok(())
    }
}

impl Deserialize for NoValue {
    type Target = Self;
    fn deserialize(_stream: &mut dyn Stream) -> anyhow::Result<NoValue> {
        Ok(NoValue {})
    }
}

pub fn ip_address_hash_raw(address: &Ipv6Addr, port: u16) -> u64 {
    let address_bytes = address.octets();
    let mut hasher = VarBlake2b::new(8).unwrap();
    hasher.update(&HardenedConstants::get().random_128.to_be_bytes());
    if port != 0 {
        hasher.update(port.to_ne_bytes());
    }
    hasher.update(address_bytes);
    let mut result = 0;
    hasher.finalize_variable(|res| result = u64::from_ne_bytes(res.try_into().unwrap()));
    result
}

pub fn deterministic_key(seed: &RawKey, index: u32) -> RawKey {
    let mut hasher = VarBlake2b::new(32).unwrap();
    hasher.update(seed.as_bytes());
    hasher.update(&index.to_be_bytes());
    let mut result = RawKey::zero();
    hasher.finalize_variable(|res| result = RawKey::from_bytes(res.try_into().unwrap()));
    result
}

/**
 * Network variants with different genesis blocks and network parameters
 */
#[repr(u16)]
#[derive(Clone, Copy, FromPrimitive, PartialEq, Eq)]
pub enum Networks {
    Invalid = 0x0,
    // Low work parameters, publicly known genesis key, dev IP ports
    NanoDevNetwork = 0x5241, // 'R', 'A'
    // Normal work parameters, secret beta genesis key, beta IP ports
    NanoBetaNetwork = 0x5242, // 'R', 'B'
    // Normal work parameters, secret live key, live IP ports
    NanoLiveNetwork = 0x5243, // 'R', 'C'
    // Normal work parameters, secret test genesis key, test IP ports
    NanoTestNetwork = 0x5258, // 'R', 'X'
}

impl Networks {
    pub fn as_str(&self) -> &str {
        match self {
            Networks::Invalid => "invalid",
            Networks::NanoDevNetwork => "dev",
            Networks::NanoBetaNetwork => "beta",
            Networks::NanoLiveNetwork => "live",
            Networks::NanoTestNetwork => "test",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deterministic_key() {
        let seed = RawKey::from(1);
        let key = deterministic_key(&seed, 3);
        assert_eq!(
            key,
            RawKey::decode_hex("89A518E3B70A0843DE8470F87FF851F9C980B1B2802267A05A089677B8FA1926")
                .unwrap()
        );
    }
}

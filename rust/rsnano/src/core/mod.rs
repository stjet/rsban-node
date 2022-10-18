mod public_key;
use blake2::{
    digest::{Update, VariableOutput},
    VarBlake2b,
};
pub use public_key::PublicKey;

mod raw_key;
pub use raw_key::RawKey;

mod block_hash;
pub use block_hash::{BlockHash, BlockHashBuilder};

mod signature;
pub use signature::Signature;

mod hash_or_account;
pub use hash_or_account::HashOrAccount;

mod link;
pub use link::Link;

mod root;
pub use root::Root;

mod qualified_root;
pub use qualified_root::QualifiedRoot;

mod key_pair;
pub use key_pair::{sign_message, validate_message, validate_message_batch, KeyPair};

mod wallet_id;
pub use wallet_id::WalletId;

mod pending_key;
pub use pending_key::PendingKey;

mod pending_info;
pub use pending_info::PendingInfo;

mod amount;
pub use amount::Amount;

mod account;
pub use account::Account;

mod difficulty;
pub use difficulty::Difficulty;

mod account_info;
pub use account_info::AccountInfo;

mod endpoint_key;
pub use endpoint_key::EndpointKey;

mod fan;
pub use fan::Fan;

mod epoch;
pub use epoch::{Epoch, Epochs};

mod blocks;
pub use blocks::*;

mod unchecked_info;
pub use unchecked_info::{SignatureVerification, UncheckedInfo, UncheckedKey};

mod confirmation_height_info;
pub use confirmation_height_info::ConfirmationHeightInfo;

mod uniquer;
pub use uniquer::Uniquer;

mod hardened_constants;
pub mod messages;
pub(crate) use hardened_constants::HardenedConstants;

use once_cell::sync::Lazy;
use std::{fmt::Write, net::Ipv6Addr, num::ParseIntError};

use crate::utils::{Deserialize, Serialize, Stream};

pub(crate) fn encode_hex(i: u128) -> String {
    let mut result = String::with_capacity(32);
    for byte in i.to_ne_bytes() {
        write!(&mut result, "{:02X}", byte).unwrap();
    }
    result
}

pub(crate) fn write_hex_bytes(
    bytes: &[u8],
    f: &mut std::fmt::Formatter,
) -> Result<(), std::fmt::Error> {
    for &byte in bytes {
        write!(f, "{:02X}", byte)?;
    }
    Ok(())
}

pub fn to_hex_string(i: u64) -> String {
    format!("{:016X}", i)
}

pub fn u64_from_hex_str(s: impl AsRef<str>) -> Result<u64, ParseIntError> {
    u64::from_str_radix(s.as_ref(), 16)
}

pub static XRB_RATIO: Lazy<u128> = Lazy::new(|| str::parse("1000000000000000000000000").unwrap()); // 10^24
pub static KXRB_RATIO: Lazy<u128> =
    Lazy::new(|| str::parse("1000000000000000000000000000").unwrap()); // 10^27
pub static MXRB_RATIO: Lazy<u128> =
    Lazy::new(|| str::parse("1000000000000000000000000000000").unwrap()); // 10^30
pub static GXRB_RATIO: Lazy<u128> =
    Lazy::new(|| str::parse("1000000000000000000000000000000000").unwrap()); // 10^33

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

impl Serialize for [u8; 64] {
    fn serialized_size() -> usize {
        64
    }

    fn serialize(&self, stream: &mut dyn Stream) -> anyhow::Result<()> {
        stream.write_bytes(self)
    }
}

impl Deserialize for [u8; 64] {
    type Target = Self;

    fn deserialize(stream: &mut dyn Stream) -> anyhow::Result<Self::Target> {
        let mut buffer = [0; 64];
        stream.read_bytes(&mut buffer, 64)?;
        Ok(buffer)
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
    let mut result = RawKey::new();
    hasher.finalize_variable(|res| result = RawKey::from_bytes(res.try_into().unwrap()));
    result
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

#[macro_use]
extern crate anyhow;

#[macro_use]
extern crate num_derive;

#[macro_use]
extern crate static_assertions;

mod account;
pub use account::Account;

mod amount;
pub use amount::{Amount, GXRB_RATIO, KXRB_RATIO, MXRB_RATIO, XRB_RATIO};

mod block_hash;
use blake2::{
    digest::{Update, VariableOutput},
    Blake2bVar,
};
pub use block_hash::{BlockHash, BlockHashBuilder};

mod key_pair;
pub use key_pair::{sign_message, validate_block_signature, validate_message, KeyPair};

mod raw_key;
pub use raw_key::RawKey;

mod signature;
pub use signature::Signature;

mod u256_struct;
pub use u256_struct::*;

pub mod utils;

mod qualified_root;
pub use qualified_root::QualifiedRoot;

mod account_info;
pub use account_info::AccountInfo;

mod epoch;
pub use epoch::{Epoch, Epochs};

mod confirmation_height_info;
pub use confirmation_height_info::ConfirmationHeightInfo;

mod pending_key;
pub use pending_key::PendingKey;

mod pending_info;
pub use pending_info::PendingInfo;

mod difficulty;
pub use difficulty::{Difficulty, DifficultyV1, StubDifficulty, WorkVersion};

mod endpoint_key;
pub use endpoint_key::EndpointKey;

mod blocks;
pub use blocks::*;

pub mod work;

mod unchecked_info;
pub use unchecked_info::{UncheckedInfo, UncheckedKey};

mod kdf;
pub use kdf::KeyDerivationFunction;
use utils::BufferWriter;

use std::fmt::Write;
use std::num::ParseIntError;

pub fn encode_hex(i: u128) -> String {
    let mut result = String::with_capacity(32);
    for byte in i.to_ne_bytes() {
        write!(&mut result, "{:02X}", byte).unwrap();
    }
    result
}

pub fn write_hex_bytes(bytes: &[u8], f: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
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

u256_struct!(HashOrAccount);
u256_struct!(Link);
u256_struct!(PublicKey);
u256_struct!(Root);
u256_struct!(WalletId);

impl From<HashOrAccount> for Account {
    fn from(source: HashOrAccount) -> Self {
        Account::from_bytes(*source.as_bytes())
    }
}

impl From<&HashOrAccount> for Account {
    fn from(source: &HashOrAccount) -> Self {
        Account::from_bytes(*source.as_bytes())
    }
}

impl From<Link> for Account {
    fn from(link: Link) -> Self {
        Account::from_bytes(*link.as_bytes())
    }
}

impl From<&Link> for Account {
    fn from(link: &Link) -> Self {
        Account::from_bytes(*link.as_bytes())
    }
}

impl From<Root> for Account {
    fn from(root: Root) -> Self {
        Account::from_bytes(*root.as_bytes())
    }
}

impl From<Account> for Link {
    fn from(account: Account) -> Self {
        Link::from_bytes(*account.as_bytes())
    }
}

impl From<&Account> for Link {
    fn from(account: &Account) -> Self {
        Link::from_bytes(*account.as_bytes())
    }
}

impl From<BlockHash> for Link {
    fn from(hash: BlockHash) -> Self {
        Link::from_bytes(*hash.as_bytes())
    }
}

impl From<HashOrAccount> for BlockHash {
    fn from(source: HashOrAccount) -> Self {
        BlockHash::from_bytes(*source.as_bytes())
    }
}

impl From<&HashOrAccount> for BlockHash {
    fn from(source: &HashOrAccount) -> Self {
        BlockHash::from_bytes(*source.as_bytes())
    }
}
impl From<Link> for BlockHash {
    fn from(link: Link) -> Self {
        BlockHash::from_bytes(*link.as_bytes())
    }
}

impl From<Root> for BlockHash {
    fn from(root: Root) -> Self {
        BlockHash::from_bytes(*root.as_bytes())
    }
}

impl From<Account> for HashOrAccount {
    fn from(account: Account) -> Self {
        HashOrAccount::from_bytes(*account.as_bytes())
    }
}

impl From<BlockHash> for HashOrAccount {
    fn from(hash: BlockHash) -> Self {
        HashOrAccount::from_bytes(*hash.as_bytes())
    }
}

impl From<Link> for HashOrAccount {
    fn from(link: Link) -> Self {
        HashOrAccount::from_bytes(*link.as_bytes())
    }
}

impl From<&Link> for HashOrAccount {
    fn from(link: &Link) -> Self {
        HashOrAccount::from_bytes(*link.as_bytes())
    }
}

impl From<PublicKey> for Root {
    fn from(key: PublicKey) -> Self {
        Root::from_bytes(*key.as_bytes())
    }
}

impl From<&Account> for Root {
    fn from(hash: &Account) -> Self {
        Root::from_bytes(*hash.as_bytes())
    }
}

impl From<BlockHash> for Root {
    fn from(hash: BlockHash) -> Self {
        Root::from_bytes(*hash.as_bytes())
    }
}

impl From<&BlockHash> for Root {
    fn from(hash: &BlockHash) -> Self {
        Root::from_bytes(*hash.as_bytes())
    }
}

impl PublicKey {
    /// IV for Key encryption
    pub fn initialization_vector(&self) -> [u8; 16] {
        self.0[..16].try_into().unwrap()
    }
}

impl TryFrom<&RawKey> for PublicKey {
    type Error = anyhow::Error;
    fn try_from(prv: &RawKey) -> Result<Self, Self::Error> {
        let secret = ed25519_dalek_blake2b::SecretKey::from_bytes(prv.as_bytes())
            .map_err(|_| anyhow!("could not extract secret key"))?;
        let public = ed25519_dalek_blake2b::PublicKey::from(&secret);
        Ok(PublicKey::from_bytes(public.to_bytes()))
    }
}

pub trait FullHash {
    fn full_hash(&self) -> BlockHash;
}

#[derive(PartialEq, Eq, Debug)]
pub struct NoValue {}

impl utils::FixedSizeSerialize for NoValue {
    fn serialized_size() -> usize {
        0
    }
}

impl utils::Serialize for NoValue {
    fn serialize(&self, _writer: &mut dyn BufferWriter) {}
}

impl utils::Deserialize for NoValue {
    type Target = Self;
    fn deserialize(_stream: &mut dyn utils::Stream) -> anyhow::Result<NoValue> {
        Ok(NoValue {})
    }
}

pub fn deterministic_key(seed: &RawKey, index: u32) -> RawKey {
    let mut buffer = [0; 32];
    let mut hasher = Blake2bVar::new(buffer.len()).unwrap();
    hasher.update(seed.as_bytes());
    hasher.update(&index.to_be_bytes());
    hasher.finalize_variable(&mut buffer).unwrap();
    RawKey::from_bytes(buffer)
}

/**
 * Network variants with different genesis blocks and network parameters
 */
#[repr(u16)]
#[derive(Clone, Copy, FromPrimitive, PartialEq, Eq, Debug)]
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

/// A slice of blocks of an account chain
#[derive(Clone, PartialEq, Eq, Default)]
pub struct BlockChainSection {
    pub account: Account,
    /// The lowest block (inclusive)
    pub bottom_hash: BlockHash,
    /// block height of the lowest block
    pub bottom_height: u64,
    /// The highest block (inclusive)
    pub top_hash: BlockHash,
    /// block height of the highest block
    pub top_height: u64,
}

impl BlockChainSection {
    pub fn block_count(&self) -> u64 {
        self.top_height - self.bottom_height + 1
    }
}

impl std::fmt::Debug for BlockChainSection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WriteDetails")
            .field("account", &self.account.encode_account())
            .field("bottom_height", &self.bottom_height)
            .field("bottom_hash", &self.bottom_hash.encode_hex())
            .field("top_height", &self.top_height)
            .field("top_hash", &self.top_hash.encode_hex())
            .finish()
    }
}

pub fn epoch_v1_link() -> Link {
    let mut link_bytes = [0u8; 32];
    link_bytes[..14].copy_from_slice(b"epoch v1 block");
    Link::from_bytes(link_bytes)
}

pub fn epoch_v2_link() -> Link {
    let mut link_bytes = [0u8; 32];
    link_bytes[..14].copy_from_slice(b"epoch v2 block");
    Link::from_bytes(link_bytes)
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

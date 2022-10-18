use super::Account;
use super::{write_hex_bytes, BlockHash};
use crate::utils::Stream;
use std::fmt::Display;
use std::fmt::Write;

#[derive(Clone, PartialEq, Eq, Default, Debug, Copy, Hash)]
pub struct HashOrAccount {
    bytes: [u8; 32],
}

impl HashOrAccount {
    pub fn new() -> Self {
        Self { bytes: [0u8; 32] }
    }

    pub fn is_zero(&self) -> bool {
        self.bytes == [0u8; 32]
    }

    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self { bytes }
    }

    pub fn from_slice(bytes: &[u8]) -> Option<Self> {
        if bytes.len() != 32 {
            None
        } else {
            let mut result = Self { bytes: [0; 32] };
            result.bytes.copy_from_slice(bytes);
            Some(result)
        }
    }

    pub const fn serialized_size() -> usize {
        32
    }

    pub fn serialize(&self, stream: &mut dyn Stream) -> anyhow::Result<()> {
        stream.write_bytes(&self.bytes)
    }

    pub fn deserialize(stream: &mut dyn Stream) -> anyhow::Result<Self> {
        let mut result = Self::new();
        stream.read_bytes(&mut result.bytes, 32)?;
        Ok(result)
    }

    pub fn to_bytes(self) -> [u8; 32] {
        self.bytes
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.bytes
    }

    pub fn encode_hex(&self) -> String {
        let mut result = String::with_capacity(64);
        for byte in self.bytes {
            write!(&mut result, "{:02X}", byte).unwrap();
        }
        result
    }

    pub fn decode_hex(s: impl AsRef<str>) -> anyhow::Result<Self> {
        let mut bytes = [0u8; 32];
        hex::decode_to_slice(s.as_ref(), &mut bytes)?;
        Ok(Self::from_bytes(bytes))
    }

    pub fn to_account(self) -> Account {
        Account::from_bytes(self.bytes)
    }

    pub fn to_block_hash(self) -> BlockHash {
        BlockHash::from_bytes(self.bytes)
    }
}

impl Display for HashOrAccount {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write_hex_bytes(&self.bytes, f)
    }
}

impl From<u64> for HashOrAccount {
    fn from(value: u64) -> Self {
        let mut result = Self::new();
        result.bytes[24..].copy_from_slice(&value.to_be_bytes());
        result
    }
}

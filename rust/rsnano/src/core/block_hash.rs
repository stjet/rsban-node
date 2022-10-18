use blake2::digest::Update;
use blake2::digest::VariableOutput;
use primitive_types::U256;
use rand::thread_rng;
use rand::Rng;
use std::fmt::Display;
use std::fmt::Write;

use crate::utils::{Deserialize, Serialize, Stream};

use super::write_hex_bytes;

#[derive(Clone, Copy, PartialEq, Eq, Default, Debug, Hash)]
pub struct BlockHash {
    value: [u8; 32], //big endian
}

const ZERO_BLOCK_HASH: BlockHash = BlockHash { value: [0; 32] };

impl BlockHash {
    pub fn new() -> Self {
        Self { value: [0; 32] }
    }

    pub fn zero() -> &'static Self {
        &ZERO_BLOCK_HASH
    }

    pub fn is_zero(&self) -> bool {
        self.value == [0u8; 32]
    }

    pub fn random() -> Self {
        BlockHash::from_bytes(thread_rng().gen())
    }

    pub fn from_bytes(value: [u8; 32]) -> Self {
        Self { value }
    }

    pub fn from_slice(bytes: &[u8]) -> Option<Self> {
        if bytes.len() != 32 {
            None
        } else {
            let mut result = Self::new();
            result.value.copy_from_slice(bytes);
            Some(result)
        }
    }

    pub fn to_bytes(self) -> [u8; 32] {
        self.value
    }

    pub fn as_bytes(&'_ self) -> &'_ [u8; 32] {
        &self.value
    }

    pub fn encode_hex(&self) -> String {
        let mut result = String::with_capacity(64);
        for &byte in self.value.iter() {
            write!(&mut result, "{:02X}", byte).unwrap();
        }
        result
    }

    pub fn decode_hex(s: impl AsRef<str>) -> anyhow::Result<BlockHash> {
        let mut bytes = [0u8; 32];
        hex::decode_to_slice(s.as_ref(), &mut bytes)?;
        Ok(BlockHash::from_bytes(bytes))
    }
}

impl Deserialize for BlockHash {
    type Target = Self;
    fn deserialize(stream: &mut dyn Stream) -> anyhow::Result<Self> {
        let mut result = Self::new();
        stream.read_bytes(&mut result.value, 32)?;
        Ok(result)
    }
}

impl From<u64> for BlockHash {
    fn from(value: u64) -> Self {
        let mut result = Self { value: [0; 32] };
        result.value[24..].copy_from_slice(&value.to_be_bytes());
        result
    }
}

impl From<U256> for BlockHash {
    fn from(value: U256) -> Self {
        let mut hash = BlockHash::new();
        value.to_big_endian(&mut hash.value);
        hash
    }
}

impl Display for BlockHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write_hex_bytes(&self.value, f)
    }
}

impl Serialize for BlockHash {
    fn serialized_size() -> usize {
        32
    }

    fn serialize(&self, stream: &mut dyn Stream) -> anyhow::Result<()> {
        stream.write_bytes(&self.value)
    }
}

pub struct BlockHashBuilder {
    blake: blake2::VarBlake2b,
}

impl Default for BlockHashBuilder {
    fn default() -> Self {
        Self {
            blake: blake2::VarBlake2b::new_keyed(&[], 32),
        }
    }
}

impl BlockHashBuilder {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn update(mut self, data: impl AsRef<[u8]>) -> Self {
        self.blake.update(data);
        self
    }

    pub fn build(self) -> BlockHash {
        let mut hash_bytes = [0u8; 32];
        self.blake.finalize_variable(|result| {
            hash_bytes.copy_from_slice(result);
        });
        BlockHash::from_bytes(hash_bytes)
    }
}

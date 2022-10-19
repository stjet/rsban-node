use std::slice;

use primitive_types::U256;

use crate::utils::Stream;

use super::RawKey;

#[derive(Clone, Copy, PartialEq, Eq, Default, Debug, Hash)]
pub struct PublicKey {
    value: [u8; 32], // big endian
}

impl PublicKey {
    pub const fn new() -> Self {
        Self { value: [0; 32] }
    }

    pub fn is_zero(&self) -> bool {
        self.value == [0; 32]
    }

    pub const fn from_bytes(value: [u8; 32]) -> Self {
        Self { value }
    }

    pub fn from_slice(value: &[u8]) -> Option<Self> {
        match value.try_into() {
            Ok(value) => Some(Self { value }),
            Err(_) => None,
        }
    }

    pub unsafe fn from_ptr(data: *const u8) -> Self {
        Self {
            value: slice::from_raw_parts(data, 32).try_into().unwrap(),
        }
    }

    pub fn number(&self) -> U256 {
        U256::from_big_endian(&self.value)
    }

    pub const fn serialized_size() -> usize {
        32
    }

    pub fn serialize(&self, stream: &mut dyn Stream) -> anyhow::Result<()> {
        stream.write_bytes(&self.value)
    }

    pub fn deserialize(stream: &mut dyn Stream) -> anyhow::Result<Self> {
        let mut result = PublicKey::new();
        stream.read_bytes(&mut result.value, 32)?;
        Ok(result)
    }

    pub fn as_bytes(&'_ self) -> &'_ [u8; 32] {
        &self.value
    }

    pub fn to_be_bytes(self) -> [u8; 32] {
        self.value
    }

    /// IV for Key encryption
    pub fn initialization_vector(&self) -> [u8; 16] {
        self.value[..16].try_into().unwrap()
    }
}

impl From<U256> for PublicKey {
    fn from(value: U256) -> Self {
        let mut key = Self::new();
        value.to_big_endian(&mut key.value);
        key
    }
}

impl TryFrom<&RawKey> for PublicKey {
    type Error = anyhow::Error;
    fn try_from(prv: &RawKey) -> Result<Self, Self::Error> {
        let secret = ed25519_dalek_blake2b::SecretKey::from_bytes(prv.as_bytes())
            .map_err(|_| anyhow!("could not extract secret key"))?;
        let public = ed25519_dalek_blake2b::PublicKey::from(&secret);
        Ok(PublicKey {
            value: public.to_bytes(),
        })
    }
}

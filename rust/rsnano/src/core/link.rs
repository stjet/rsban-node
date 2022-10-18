use std::ops::Deref;

use crate::utils::Stream;

use super::HashOrAccount;

#[derive(Clone, PartialEq, Eq, Default, Debug, Copy)]
pub struct Link {
    inner: HashOrAccount,
}

impl Link {
    pub fn new() -> Self {
        Self {
            inner: HashOrAccount::new(),
        }
    }

    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self {
            inner: HashOrAccount::from_bytes(bytes),
        }
    }

    pub const fn serialized_size() -> usize {
        32
    }

    pub fn deserialize(stream: &mut dyn Stream) -> anyhow::Result<Self> {
        HashOrAccount::deserialize(stream).map(|inner| Self { inner })
    }

    pub fn decode_hex(s: impl AsRef<str>) -> anyhow::Result<Self> {
        HashOrAccount::decode_hex(s).map(|inner| Self { inner })
    }

    pub fn is_zero(&self) -> bool {
        self.inner.is_zero()
    }
}

impl Deref for Link {
    type Target = HashOrAccount;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl From<u64> for Link {
    fn from(value: u64) -> Self {
        Self {
            inner: HashOrAccount::from(value),
        }
    }
}

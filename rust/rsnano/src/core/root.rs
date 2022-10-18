use super::{Account, BlockHash, HashOrAccount};
use crate::utils::Stream;
use std::{fmt::Display, ops::Deref};

#[derive(Clone, PartialEq, Eq, Default, Debug, Copy, Hash)]
pub struct Root {
    inner: HashOrAccount,
}

impl Root {
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self {
            inner: HashOrAccount::from_bytes(bytes),
        }
    }

    pub fn from_slice(bytes: &[u8]) -> Option<Self> {
        match HashOrAccount::from_slice(bytes) {
            Some(inner) => Some(Self { inner }),
            None => None,
        }
    }

    pub fn deserialize(stream: &mut dyn Stream) -> anyhow::Result<Self> {
        HashOrAccount::deserialize(stream).map(|inner| Root { inner })
    }

    pub fn serialized_size() -> usize {
        HashOrAccount::serialized_size()
    }
}

impl Deref for Root {
    type Target = HashOrAccount;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl From<u64> for Root {
    fn from(value: u64) -> Self {
        let mut bytes = [0; 32];
        bytes[..8].copy_from_slice(&value.to_le_bytes());
        Self::from_bytes(bytes)
    }
}

impl Display for Root {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.inner, f)
    }
}

impl From<&Account> for Root {
    fn from(hash: &Account) -> Self {
        Root::from_bytes(hash.to_bytes())
    }
}

impl From<&BlockHash> for Root {
    fn from(hash: &BlockHash) -> Self {
        Root::from_bytes(hash.to_bytes())
    }
}

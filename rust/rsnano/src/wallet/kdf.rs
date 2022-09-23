use argon2::{Variant, Version};

use crate::RawKey;

/// Key derivation function
pub struct KeyDerivationFunction {
    kdf_work: u32,
}

impl KeyDerivationFunction {
    pub fn new(kdf_work: u32) -> Self {
        Self { kdf_work }
    }

    pub fn hash_password(&self, password: &str, salt: &[u8; 32]) -> anyhow::Result<RawKey> {
        let config = argon2::Config {
            hash_length: 32,
            lanes: 1,
            mem_cost: self.kdf_work,
            thread_mode: argon2::ThreadMode::Sequential,
            time_cost: 1,
            variant: Variant::Argon2d,
            version: Version::Version10,
            ..Default::default()
        };

        let hash = argon2::hash_raw(password.as_bytes(), salt, &config)?;
        Ok(RawKey::from_bytes(hash.as_slice().try_into()?))
    }
}

use ctr::cipher::{KeyIvInit, StreamCipher};
use primitive_types::U256;
use rand::{thread_rng, Rng};
use std::fmt::Write;
use std::ops::BitXorAssign;

use crate::utils::{Deserialize, Serialize, Stream};

type Aes256Ctr = ctr::Ctr64BE<aes::Aes256>;

#[derive(Default, PartialEq, Eq, Debug, Copy, Clone)]
pub struct RawKey {
    bytes: [u8; 32],
}

impl RawKey {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self { bytes }
    }

    pub fn random() -> Self {
        Self::from_bytes(thread_rng().gen())
    }

    pub fn is_zero(&self) -> bool {
        self.bytes == [0; 32]
    }

    pub fn as_bytes(&'_ self) -> &'_ [u8; 32] {
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
        Ok(RawKey::from_bytes(bytes))
    }

    pub fn encrypt(&self, key: &RawKey, iv: &[u8; 16]) -> Self {
        let mut cipher = Aes256Ctr::new(&(*key.as_bytes()).into(), &(*iv).into());
        let mut buf = self.bytes;
        cipher.apply_keystream(&mut buf);
        RawKey { bytes: buf }
    }

    pub fn decrypt(&self, key: &RawKey, iv: &[u8; 16]) -> Self {
        self.encrypt(key, iv)
    }

    /// IV for Key encryption
    pub fn initialization_vector_low(&self) -> [u8; 16] {
        self.bytes[..16].try_into().unwrap()
    }

    /// IV for Key encryption
    pub fn initialization_vector_high(&self) -> [u8; 16] {
        self.bytes[16..].try_into().unwrap()
    }

    pub fn number(&self) -> U256 {
        U256::from_big_endian(&self.bytes)
    }
}

impl BitXorAssign for RawKey {
    fn bitxor_assign(&mut self, rhs: Self) {
        for (a, b) in self.bytes.iter_mut().zip(rhs.bytes) {
            *a ^= b;
        }
    }
}

impl From<u64> for RawKey {
    fn from(value: u64) -> Self {
        let mut bytes = [0; 32];
        bytes[24..].copy_from_slice(&value.to_be_bytes());
        Self::from_bytes(bytes)
    }
}

impl Serialize for RawKey {
    fn serialized_size() -> usize {
        32
    }

    fn serialize(&self, stream: &mut dyn Stream) -> anyhow::Result<()> {
        stream.write_bytes(self.as_bytes())
    }
}

impl Deserialize for RawKey {
    type Target = Self;

    fn deserialize(stream: &mut dyn Stream) -> anyhow::Result<Self::Target> {
        let mut buffer = [0; 32];
        stream.read_bytes(&mut buffer, 32)?;
        Ok(RawKey::from_bytes(buffer))
    }
}

#[cfg(test)]
mod tests {
    use crate::core::{KeyPair, PublicKey};

    use super::*;

    #[test]
    fn encrypt() {
        let clear_text = RawKey::from(1);
        let key = RawKey::from(2);
        let iv: u128 = 123;
        let encrypted = RawKey::encrypt(&clear_text, &key, &iv.to_be_bytes());
        let expected =
            RawKey::decode_hex("3ED412A6F9840EA148EAEE236AFD10983D8E11326B07DFB33C5E1C47000AF3FD")
                .unwrap();
        assert_eq!(encrypted, expected)
    }

    #[test]
    fn encrypt_and_decrypt() {
        let clear_text = RawKey::from(1);
        let key = RawKey::from(2);
        let iv: u128 = 123;
        let encrypted = clear_text.encrypt(&key, &iv.to_be_bytes());
        let decrypted = encrypted.decrypt(&key, &iv.to_be_bytes());
        assert_eq!(decrypted, clear_text)
    }

    #[test]
    fn key_encryption() {
        let keypair = KeyPair::new();
        let secret_key = RawKey::new();
        let iv = keypair.public_key().initialization_vector();
        let encrypted = keypair.private_key().encrypt(&secret_key, &iv);
        let decrypted = encrypted.decrypt(&secret_key, &iv);
        assert_eq!(keypair.private_key(), decrypted);
        let decrypted_pub = PublicKey::try_from(&decrypted).unwrap();
        assert_eq!(keypair.public_key(), decrypted_pub);
    }

    #[test]
    fn encrypt_produces_same_result_every_time() {
        let secret = RawKey::new();
        let number = RawKey::from(1);
        let iv = [1; 16];
        let encrypted1 = number.encrypt(&secret, &iv);
        let encrypted2 = number.encrypt(&secret, &iv);
        assert_eq!(encrypted1, encrypted2);
    }
}

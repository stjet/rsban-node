use std::convert::TryFrom;

use crate::utils::Stream;
use anyhow::Result;

#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub struct PublicKey {
    value: [u8; 32], // big endian
}

impl PublicKey {
    pub fn new() -> Self {
        Self { value: [8; 32] }
    }

    pub fn from_be_bytes(value: [u8; 32]) -> Self {
        Self { value }
    }

    pub const fn serialized_size() -> usize {
        32
    }

    pub fn serialize(&self, stream: &mut impl Stream) -> Result<()> {
        stream.write_bytes(&self.value)
    }

    pub fn deserialize(&mut self, stream: &mut impl Stream) -> Result<()> {
        let len = self.value.len();
        stream.read_bytes(&mut self.value, len)
    }

    pub fn as_bytes(&'_ self) -> &'_ [u8; 32] {
        &self.value
    }

    pub fn to_be_bytes(self) -> [u8; 32] {
        self.value
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub struct Account {
    public_key: PublicKey,
}

impl Account {
    pub fn new() -> Self {
        Self {
            public_key: PublicKey::new(),
        }
    }

    pub fn from_public_key(public_key: PublicKey) -> Self {
        Self { public_key }
    }

    pub fn from_be_bytes(bytes: [u8; 32]) -> Account {
        Self {
            public_key: PublicKey::from_be_bytes(bytes),
        }
    }

    pub const fn serialized_size() -> usize {
        PublicKey::serialized_size()
    }

    pub fn serialize(&self, stream: &mut impl Stream) -> Result<()> {
        self.public_key.serialize(stream)
    }

    pub fn deserialize(&mut self, stream: &mut impl Stream) -> Result<()> {
        self.public_key.deserialize(stream)
    }

    pub fn to_be_bytes(self) -> [u8; 32] {
        self.public_key.to_be_bytes()
    }
}

impl From<u64> for Account {
    fn from(value: u64) -> Self {
        let mut key = PublicKey::new();
        key.value[24..].copy_from_slice(&value.to_be_bytes());
        Account::from_public_key(key)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub struct BlockHash {
    value: [u8; 32], //big endian
}

impl BlockHash {
    pub fn new() -> Self {
        Self { value: [0; 32] }
    }

    pub fn is_zero(&self) -> bool {
        self.value == [0u8; 32]
    }

    pub fn from_be_bytes(value: [u8; 32]) -> Self {
        Self { value }
    }

    pub const fn serialized_size() -> usize {
        32
    }

    pub fn serialize(&self, stream: &mut impl Stream) -> Result<()> {
        stream.write_bytes(&self.value)
    }

    pub fn deserialize(&mut self, stream: &mut impl Stream) -> Result<()> {
        let len = self.value.len();
        stream.read_bytes(&mut self.value, len)
    }

    pub fn to_be_bytes(self) -> [u8; 32] {
        self.value
    }

    pub fn as_bytes(&'_ self) -> &'_ [u8; 32] {
        &self.value
    }
}

impl From<u64> for BlockHash {
    fn from(value: u64) -> Self {
        let mut result = Self { value: [0; 32] };

        result.value[24..].copy_from_slice(&value.to_be_bytes());

        result
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub struct Amount {
    value: u128, // native endian!
}

impl Amount {
    pub fn new(value: u128) -> Self {
        Self { value }
    }

    pub fn from_be_bytes(bytes: [u8; 16]) -> Self {
        Self {
            value: u128::from_be_bytes(bytes),
        }
    }

    pub const fn serialized_size() -> usize {
        std::mem::size_of::<u128>()
    }

    pub fn serialize(&self, stream: &mut impl Stream) -> Result<()> {
        stream.write_bytes(&self.value.to_be_bytes())
    }

    pub fn deserialize(&mut self, stream: &mut impl Stream) -> Result<()> {
        let mut buffer = [0u8; 16];
        let len = buffer.len();
        stream.read_bytes(&mut buffer, len)?;
        self.value = u128::from_be_bytes(buffer);
        Ok(())
    }

    pub fn to_be_bytes(self) -> [u8; 16] {
        self.value.to_be_bytes()
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Signature {
    bytes: [u8; 64],
}

impl Default for Signature {
    fn default() -> Self {
        Self { bytes: [0; 64] }
    }
}

impl Signature {
    pub fn new() -> Self {
        Self { bytes: [0u8; 64] }
    }

    pub fn from_bytes(bytes: [u8; 64]) -> Self {
        Self { bytes }
    }

    pub const fn serialized_size() -> usize {
        64
    }

    pub fn serialize(&self, stream: &mut impl Stream) -> Result<()> {
        stream.write_bytes(&self.bytes)
    }

    pub fn deserialize(stream: &mut impl Stream) -> Result<Signature> {
        let mut result = Signature { bytes: [0; 64] };

        stream.read_bytes(&mut result.bytes, 64)?;
        Ok(result)
    }

    pub fn as_bytes(&'_ self) -> &'_ [u8; 64] {
        &self.bytes
    }

    pub fn to_be_bytes(&self) -> [u8; 64] {
        self.bytes
    }

    #[cfg(test)]
    pub fn make_invalid(&mut self) {
        self.bytes[31] ^= 1;
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct Link {
    bytes: [u8; 32],
}

impl Link {
    pub fn new() -> Self {
        Self { bytes: [0u8; 32] }
    }

    pub fn from_be_bytes(bytes: [u8; 32]) -> Self {
        Self { bytes }
    }

    pub const fn serialized_size() -> usize {
        32
    }

    pub fn serialize(&self, stream: &mut impl Stream) -> Result<()> {
        stream.write_bytes(&self.bytes)
    }

    pub fn deserialize(&mut self, stream: &mut impl Stream) -> Result<()> {
        stream.read_bytes(&mut self.bytes, 32)?;
        Ok(())
    }

    pub fn to_be_bytes(&self) -> [u8; 32] {
        self.bytes
    }
}

pub struct RawKey {
    bytes: [u8; 32],
}

impl RawKey {
    pub fn new() -> Self {
        Self { bytes: [0u8; 32] }
    }

    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self { bytes }
    }

    pub fn as_bytes(&'_ self) -> &'_ [u8; 32] {
        &self.bytes
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

pub struct KeyPair {
    keypair: ed25519_dalek_blake2b::Keypair,
}

impl KeyPair {
    pub fn new() -> Self {
        let mut rng = rand::thread_rng();
        let keypair = ed25519_dalek_blake2b::Keypair::generate(&mut rng);
        Self { keypair }
    }

    pub fn public_key(&self) -> PublicKey {
        PublicKey::from_be_bytes(self.keypair.public.to_bytes())
    }

    pub fn private_key(&self) -> RawKey {
        RawKey::from_bytes(self.keypair.secret.to_bytes())
    }
}

pub fn sign_message(
    private_key: &RawKey,
    public_key: &PublicKey,
    data: &[u8],
) -> Result<Signature> {
    let secret = ed25519_dalek_blake2b::SecretKey::from_bytes(private_key.as_bytes())
        .map_err(|_| anyhow!("could not extract secret key"))?;
    let public = ed25519_dalek_blake2b::PublicKey::from_bytes(public_key.as_bytes())
        .map_err(|_| anyhow!("could not extract public key"))?;
    let expanded = ed25519_dalek_blake2b::ExpandedSecretKey::from(&secret);
    let signature = expanded.sign(data, &public);
    Ok(Signature::from_bytes(signature.to_bytes()))
}

pub fn validate_message(
    public_key: &PublicKey,
    message: &[u8],
    signature: &Signature,
) -> Result<()> {
    let public = ed25519_dalek_blake2b::PublicKey::from_bytes(public_key.as_bytes())
        .map_err(|_| anyhow!("could not extract public key"))?;
    let sig = ed25519_dalek_blake2b::Signature::new(signature.to_be_bytes());
    public
        .verify_strict(message, &sig)
        .map_err(|_| anyhow!("could not verify message"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ed25519_signing() -> Result<()> {
        let secret_key = ed25519_dalek_blake2b::SecretKey::from_bytes(&[0u8; 32]).unwrap();
        let public_key = ed25519_dalek_blake2b::PublicKey::from(&secret_key);
        let message = [0u8; 32];
        let expanded_prv_key = ed25519_dalek_blake2b::ExpandedSecretKey::from(&secret_key);
        let signature = expanded_prv_key.sign(&message, &public_key);
        public_key.verify_strict(&message, &signature).unwrap();

        let mut sig_bytes = signature.to_bytes();
        sig_bytes[32] ^= 0x1;
        let signature = ed25519_dalek_blake2b::Signature::new(sig_bytes);
        assert!(public_key.verify_strict(&message, &signature).is_err());

        Ok(())
    }

    #[test]
    fn sign_message_test() -> Result<()> {
        let keypair = KeyPair::new();
        let data = [0u8; 32];
        let signature = sign_message(&keypair.private_key(), &keypair.public_key(), &data)?;
        validate_message(&keypair.public_key(), &data, &signature)?;
        Ok(())
    }
}

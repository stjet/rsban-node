mod account;
mod account_info;
mod amount;
mod difficulty;
mod endpoint_key;
mod fan;

use std::convert::TryInto;
use std::fmt::Debug;
use std::mem::size_of;
use std::net::Ipv6Addr;

use crate::core::Root;
use crate::core::{BlockHash, PublicKey, RawKey, Signature};
use crate::hardened_constants::HardenedConstants;
use crate::utils::{Deserialize, MutStreamAdapter, Serialize, Stream};
use crate::Epoch;
use anyhow::Result;

pub use account::*;
pub use account_info::AccountInfo;
pub use amount::*;
use blake2::digest::{Update, VariableOutput};
use blake2::VarBlake2b;
pub use difficulty::*;
pub use endpoint_key::EndpointKey;
pub use fan::Fan;
use num::FromPrimitive;
use once_cell::sync::Lazy;
use primitive_types::U512;

#[derive(Default, Clone)]
pub struct QualifiedRoot {
    pub root: Root,
    pub previous: BlockHash,
}

impl QualifiedRoot {
    pub fn to_bytes(&self) -> [u8; 64] {
        let mut buffer = [0; 64];
        let mut stream = MutStreamAdapter::new(&mut buffer);
        self.serialize(&mut stream).unwrap();
        buffer
    }
}

impl Serialize for QualifiedRoot {
    fn serialized_size() -> usize {
        Root::serialized_size() + BlockHash::serialized_size()
    }

    fn serialize(&self, stream: &mut dyn Stream) -> anyhow::Result<()> {
        self.root.serialize(stream)?;
        self.previous.serialize(stream)
    }
}

impl Deserialize for QualifiedRoot {
    type Target = Self;
    fn deserialize(stream: &mut dyn Stream) -> anyhow::Result<QualifiedRoot> {
        let root = Root::deserialize(stream)?;
        let previous = BlockHash::deserialize(stream)?;
        Ok(QualifiedRoot { root, previous })
    }
}

impl From<U512> for QualifiedRoot {
    fn from(value: U512) -> Self {
        let mut bytes = [0; 64];
        value.to_big_endian(&mut bytes);
        let root = Root::from_slice(&bytes[..32]).unwrap();
        let previous = BlockHash::from_slice(&bytes[32..]).unwrap();
        QualifiedRoot { root, previous }
    }
}

#[derive(Default, PartialEq, Eq, Debug, Copy, Clone)]
pub struct WalletId([u8; 32]);

impl WalletId {
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub fn as_bytes(&'_ self) -> &'_ [u8; 32] {
        &self.0
    }

    pub fn decode_hex(s: impl AsRef<str>) -> Result<Self> {
        let mut bytes = [0u8; 32];
        hex::decode_to_slice(s.as_ref(), &mut bytes)?;
        Ok(Self::from_bytes(bytes))
    }
}

pub struct KeyPair {
    keypair: ed25519_dalek_blake2b::Keypair,
}

impl Default for KeyPair {
    fn default() -> Self {
        let mut rng = rand::thread_rng();
        let keypair = ed25519_dalek_blake2b::Keypair::generate(&mut rng);
        Self { keypair }
    }
}

impl KeyPair {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn zero() -> Self {
        Self::from_priv_key_bytes(&[0u8; 32]).unwrap()
    }

    pub fn from_priv_key_bytes(bytes: &[u8]) -> Result<Self> {
        let secret = ed25519_dalek_blake2b::SecretKey::from_bytes(bytes)
            .map_err(|_| anyhow!("could not load secret key"))?;
        let public = ed25519_dalek_blake2b::PublicKey::from(&secret);
        Ok(Self {
            keypair: ed25519_dalek_blake2b::Keypair { secret, public },
        })
    }

    pub fn from_priv_key_hex(s: impl AsRef<str>) -> Result<Self> {
        let mut bytes = [0u8; 32];
        hex::decode_to_slice(s.as_ref(), &mut bytes)?;
        Self::from_priv_key_bytes(&bytes)
    }

    pub fn public_key(&self) -> PublicKey {
        PublicKey::from_bytes(self.keypair.public.to_bytes())
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
    let sig = ed25519_dalek_blake2b::Signature::from_bytes(&signature.to_be_bytes())
        .map_err(|_| anyhow!("invalid signature bytes"))?;
    public
        .verify_strict(message, &sig)
        .map_err(|_| anyhow!("could not verify message"))?;
    Ok(())
}

pub fn validate_message_batch(
    messages: &[Vec<u8>],
    public_keys: &[PublicKey],
    signatures: &[Signature],
    valid: &mut [i32],
) {
    let len = messages.len();
    assert!(public_keys.len() == len && signatures.len() == len && valid.len() == len);
    for i in 0..len {
        valid[i] = match validate_message(&public_keys[i], &messages[i], &signatures[i]) {
            Ok(_) => 1,
            Err(_) => 0,
        }
    }
}

pub fn to_string_hex(i: u64) -> String {
    format!("{:016X}", i)
}

pub fn from_string_hex(s: impl AsRef<str>) -> Result<u64> {
    let result = u64::from_str_radix(s.as_ref(), 16)?;
    Ok(result)
}

pub static XRB_RATIO: Lazy<u128> = Lazy::new(|| str::parse("1000000000000000000000000").unwrap()); // 10^24
pub static KXRB_RATIO: Lazy<u128> =
    Lazy::new(|| str::parse("1000000000000000000000000000").unwrap()); // 10^27
pub static MXRB_RATIO: Lazy<u128> =
    Lazy::new(|| str::parse("1000000000000000000000000000000").unwrap()); // 10^30
pub static GXRB_RATIO: Lazy<u128> =
    Lazy::new(|| str::parse("1000000000000000000000000000000000").unwrap()); // 10^33

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

#[derive(Default, PartialEq, Eq, Debug)]
pub struct PendingKey {
    pub account: Account,
    pub hash: BlockHash,
}

impl PendingKey {
    pub fn new(account: Account, hash: BlockHash) -> Self {
        Self { account, hash }
    }

    pub fn to_bytes(&self) -> [u8; 64] {
        let mut result = [0; 64];
        result[..32].copy_from_slice(self.account.as_bytes());
        result[32..].copy_from_slice(self.hash.as_bytes());
        result
    }
}

impl Serialize for PendingKey {
    fn serialized_size() -> usize {
        Account::serialized_size() + BlockHash::serialized_size()
    }

    fn serialize(&self, stream: &mut dyn Stream) -> anyhow::Result<()> {
        self.account.serialize(stream)?;
        self.hash.serialize(stream)
    }
}

impl Deserialize for PendingKey {
    type Target = Self;

    fn deserialize(stream: &mut dyn Stream) -> anyhow::Result<Self::Target> {
        let account = Account::deserialize(stream)?;
        let hash = BlockHash::deserialize(stream)?;
        Ok(Self { account, hash })
    }
}

impl From<U512> for PendingKey {
    fn from(value: U512) -> Self {
        let mut buffer = [0; 64];
        value.to_big_endian(&mut buffer);
        PendingKey::new(
            Account::from_slice(&buffer[..32]).unwrap(),
            BlockHash::from_slice(&buffer[32..]).unwrap(),
        )
    }
}

#[derive(PartialEq, Eq, Debug)]
pub struct PendingInfo {
    pub source: Account,
    pub amount: Amount,
    pub epoch: Epoch,
}

impl Default for PendingInfo {
    fn default() -> Self {
        Self {
            source: Default::default(),
            amount: Default::default(),
            epoch: Epoch::Epoch0,
        }
    }
}

impl PendingInfo {
    pub fn new(source: Account, amount: Amount, epoch: Epoch) -> Self {
        Self {
            source,
            amount,
            epoch,
        }
    }

    pub fn to_bytes(&self) -> [u8; 49] {
        let mut bytes = [0; 49];
        bytes[..32].copy_from_slice(self.source.as_bytes());
        bytes[32..48].copy_from_slice(&self.amount.to_be_bytes());
        bytes[48] = self.epoch as u8;
        bytes
    }
}

impl Serialize for PendingInfo {
    fn serialized_size() -> usize {
        Account::serialized_size() + Amount::serialized_size() + size_of::<u8>()
    }

    fn serialize(&self, stream: &mut dyn Stream) -> anyhow::Result<()> {
        self.source.serialize(stream)?;
        self.amount.serialize(stream)?;
        stream.write_u8(self.epoch as u8)
    }
}

impl Deserialize for PendingInfo {
    type Target = Self;

    fn deserialize(stream: &mut dyn Stream) -> anyhow::Result<Self::Target> {
        let source = Account::deserialize(stream)?;
        let amount = Amount::deserialize(stream)?;
        let epoch =
            FromPrimitive::from_u8(stream.read_u8()?).ok_or_else(|| anyhow!("invalid epoch"))?;
        Ok(Self {
            source,
            amount,
            epoch,
        })
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

#[cfg(test)]
mod tests {
    use super::*;

    mod deterministic_key_tests {
        use crate::{core::RawKey, deterministic_key};

        #[test]
        fn test_deterministic_key() {
            let seed = RawKey::from(1);
            let key = deterministic_key(&seed, 3);
            assert_eq!(
                key,
                RawKey::decode_hex(
                    "89A518E3B70A0843DE8470F87FF851F9C980B1B2802267A05A089677B8FA1926"
                )
                .unwrap()
            );
        }
    }

    mod block_hash {
        use super::*;

        #[test]
        fn block_hash_encode_hex() {
            assert_eq!(
                BlockHash::new().encode_hex(),
                "0000000000000000000000000000000000000000000000000000000000000000"
            );
            assert_eq!(
                BlockHash::from(0x12ab).encode_hex(),
                "00000000000000000000000000000000000000000000000000000000000012AB"
            );
            assert_eq!(
                BlockHash::from_bytes([0xff; 32]).encode_hex(),
                "FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF"
            );
        }
    }

    mod signing {
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
            let signature = ed25519_dalek_blake2b::Signature::from_bytes(&sig_bytes).unwrap();
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

        #[test]
        fn signing_same_message_twice_produces_equal_signatures() -> Result<()> {
            // the C++ implementation adds random bytes and a padding when signing for extra security and for making side channel attacks more difficult.
            // Currently the Rust impl does not do that.
            // In C++ signing the same message twice will produce different signatures. In Rust we get the same signature.
            let keypair = KeyPair::new();
            let data = [1, 2, 3];
            let signature_a = sign_message(&keypair.private_key(), &keypair.public_key(), &data)?;
            let signature_b = sign_message(&keypair.private_key(), &keypair.public_key(), &data)?;
            assert_eq!(signature_a, signature_b);
            Ok(())
        }
    }

    mod raw_key_tests {
        use super::*;

        #[test]
        fn encrypt() {
            let clear_text = RawKey::from(1);
            let key = RawKey::from(2);
            let iv: u128 = 123;
            let encrypted = RawKey::encrypt(&clear_text, &key, &iv.to_be_bytes());
            let expected = RawKey::decode_hex(
                "3ED412A6F9840EA148EAEE236AFD10983D8E11326B07DFB33C5E1C47000AF3FD",
            )
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
}

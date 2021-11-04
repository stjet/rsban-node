use std::convert::TryFrom;
use std::fmt::Write;

use crate::utils::{Blake2b, RustBlake2b, Stream};
use anyhow::Result;
use primitive_types::U512;

#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub struct PublicKey {
    value: [u8; 32], // big endian
}

impl PublicKey {
    pub fn new() -> Self {
        Self { value: [0; 32] }
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

    pub fn from_bytes(bytes: [u8; 32]) -> Account {
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

    pub fn to_bytes(self) -> [u8; 32] {
        self.public_key.to_be_bytes()
    }

    pub fn as_bytes(&'_ self) -> &'_ [u8; 32] {
        self.public_key.as_bytes()
    }

    pub fn encode_account(&self) -> String {
        let mut number = U512::from_big_endian(self.public_key.as_bytes());
        let check = U512::from_little_endian(&self.account_checksum());
        number <<= 40;
        number = number | check;

        let mut result = String::with_capacity(65);

        for _i in 0..60 {
            let r = number.byte(0) & 0x1f_u8;
            number >>= 5;
            result.push(account_encode(r));
        }
        result.push_str("_onan"); // nano_
        result.chars().rev().collect()
    }

    fn account_checksum(&self) -> [u8; 5] {
        let mut blake = RustBlake2b::new();
        let mut check = [0u8; 5];
        blake.init(5).unwrap();
        blake.update(self.public_key.as_bytes()).unwrap();
        blake.finalize(&mut check).unwrap();
        check
    }

    pub fn decode_account(source: &str) -> Option<Account> {
        EncodedAccountStr(source)
            .to_u512()
            .map(|encoded| encoded.to_account())
            .flatten()
    }

    pub fn decode_hex(s: &str) -> Option<Self> {
        if s.is_empty() || s.len() > 64 {
            return None;
        }

        let mut bytes = [0u8; 32];
        match hex::decode_to_slice(s, &mut bytes) {
            Ok(_) => Some(Account::from_bytes(bytes)),
            Err(_) => None,
        }
    }
}

struct EncodedAccountU512(U512);

impl EncodedAccountU512 {
    fn account_bytes(&self) -> [u8; 32] {
        let mut bytes_512 = [0u8; 64];
        (self.0 >> 40).to_big_endian(&mut bytes_512);
        let mut bytes_256 = [0u8; 32];
        bytes_256.copy_from_slice(&bytes_512[32..]);
        bytes_256
    }

    fn checksum_bytes(&self) -> [u8; 5] {
        [
            self.0.byte(0),
            self.0.byte(1),
            self.0.byte(2),
            self.0.byte(3),
            self.0.byte(4),
        ]
    }

    fn to_account(&self) -> Option<Account> {
        let account = Account::from_bytes(self.account_bytes());
        if account.account_checksum() == self.checksum_bytes() {
            Some(account)
        } else {
            None
        }
    }
}

struct EncodedAccountStr<'a>(&'a str);
impl<'a> EncodedAccountStr<'a> {
    fn is_valid(&self) -> bool {
        self.0.len() > 4
            && self.has_valid_prefix()
            && self.is_length_valid()
            && self.is_first_digit_valid()
    }

    fn has_valid_prefix(&self) -> bool {
        self.has_xrb_prefix() || self.has_nano_prefix() || self.has_node_id_prefix()
    }

    fn has_xrb_prefix(&self) -> bool {
        self.0.starts_with("xrb_") || self.0.starts_with("xrb-")
    }

    fn has_nano_prefix(&self) -> bool {
        self.0.starts_with("nano_") || self.0.starts_with("nano-")
    }

    fn has_node_id_prefix(&self) -> bool {
        self.0.starts_with("node_")
    }

    fn is_length_valid(&self) -> bool {
        if self.has_xrb_prefix() && self.0.chars().count() != 64 {
            return false;
        }
        if self.has_nano_prefix() && self.0.chars().count() != 65 {
            return false;
        }
        true
    }

    fn prefix_len(&self) -> usize {
        if self.has_xrb_prefix() {
            4
        } else {
            5
        }
    }

    fn first_digit(&self) -> Option<char> {
        self.0.chars().nth(self.prefix_len())
    }

    fn is_first_digit_valid(&self) -> bool {
        match self.first_digit() {
            Some('1') | Some('3') => true,
            _ => false,
        }
    }

    fn chars_after_prefix(&'_ self) -> impl Iterator<Item = char> + '_ {
        self.0.chars().skip(self.prefix_len())
    }

    fn to_u512(&self) -> Option<EncodedAccountU512> {
        if !self.is_valid() {
            return None;
        }

        let mut number = U512::default();
        for character in self.chars_after_prefix() {
            match self.decode_byte(character) {
                Some(byte) => {
                    number <<= 5;
                    number = number + byte;
                }
                None => return None,
            }
        }
        Some(EncodedAccountU512(number))
    }

    fn decode_byte(&self, character: char) -> Option<u8> {
        if character.is_ascii() {
            let character = character as u8;
            if (0x30..0x80).contains(&character) {
                let byte: u8 = account_decode(character);
                if byte != b'~' {
                    return Some(byte);
                }
            }
        }

        None
    }
}

const ACCOUNT_LOOKUP: &[char] = &[
    '1', '3', '4', '5', '6', '7', '8', '9', 'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k',
    'm', 'n', 'o', 'p', 'q', 'r', 's', 't', 'u', 'w', 'x', 'y', 'z',
];

const ACCOUNT_REVERSE: &[char] = &[
    '~', '0', '~', '1', '2', '3', '4', '5', '6', '7', '~', '~', '~', '~', '~', '~', '~', '~', '~',
    '~', '~', '~', '~', '~', '~', '~', '~', '~', '~', '~', '~', '~', '~', '~', '~', '~', '~', '~',
    '~', '~', '~', '~', '~', '~', '~', '~', '~', '~', '~', '8', '9', ':', ';', '<', '=', '>', '?',
    '@', 'A', 'B', '~', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', '~', 'L', 'M', 'N', 'O', '~',
    '~', '~', '~', '~',
];

fn account_encode(value: u8) -> char {
    ACCOUNT_LOOKUP[value as usize]
}

fn account_decode(value: u8) -> u8 {
    let mut result = ACCOUNT_REVERSE[(value - 0x30) as usize] as u8;
    if result != b'~' {
        result -= 0x30;
    }
    result
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

    pub fn from_bytes(value: [u8; 32]) -> Self {
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

    pub fn encode_hex(&self) -> String {
        let mut result = String::with_capacity(64);
        for &byte in self.value.iter() {
            write!(&mut result, "{:02X}", byte).unwrap();
        }
        result
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

    mod account {
        use super::*;

        // original test: account.encode_zero
        #[test]
        fn encode_zero() {
            let account = Account::new();
            let encoded = account.encode_account();
            assert_eq!(
                encoded,
                "nano_1111111111111111111111111111111111111111111111111111hifc8npp"
            );
            let copy = Account::decode_account(&encoded).expect("decode failed");
            assert_eq!(account, copy);
        }

        // original test: account.encode_all
        #[test]
        fn encode_all() {
            let account = Account::from_bytes([0xFF; 32]);
            let encoded = account.encode_account();
            assert_eq!(
                encoded,
                "nano_3zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzc3yoon41"
            );
            let copy = Account::decode_account(&encoded).expect("decode failed");
            assert_eq!(account, copy);
        }

        // original test: account.encode_fail
        #[test]
        fn encode_fail() {
            let account = Account::new();
            let mut encoded = account.encode_account();
            encoded.replace_range(16..17, "x");
            assert!(Account::decode_account(&encoded).is_none());
        }

        #[test]
        fn encode_real_account() {
            let account = Account::decode_hex(
                "E7F5F39D52AC32ADF978BBCF6EA50C7A5FBBDDCADE965C542808ADAE9DEF6B20",
            )
            .unwrap();
            let encoded = account.encode_account();
            assert_eq!(
                encoded,
                "nano_3szoyggo7d3koqwqjgyhftkirykzqhgwoqnpdjc4i47fotgyyts1j8ab3mti"
            );
            assert_eq!(
                Account::decode_account(&encoded).expect("could not decode"),
                account
            );
        }
    }

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

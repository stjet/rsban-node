use crate::utils::Stream;
use anyhow::Result;

#[derive(Clone, PartialEq, Eq)]
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

    pub fn to_be_bytes(&self) -> [u8; 32] {
        self.value
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct Account {
    public_key: PublicKey,
}

impl Account {
    pub fn new(public_key: PublicKey) -> Self {
        Self { public_key }
    }

    pub fn from_be_bytes(bytes: [u8; 32]) -> Account {
        Self {
            public_key: PublicKey::from_be_bytes(bytes),
        }
    }

    pub fn serialized_size() -> usize {
        PublicKey::serialized_size()
    }

    pub fn serialize(&self, stream: &mut impl Stream) -> Result<()> {
        self.public_key.serialize(stream)
    }

    pub fn deserialize(&mut self, stream: &mut impl Stream) -> Result<()> {
        self.public_key.deserialize(stream)
    }

    pub fn to_be_bytes(&self) -> [u8; 32] {
        self.public_key.to_be_bytes()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct BlockHash {
    value: [u8; 32], //big endian
}

impl BlockHash {
    pub fn from_be_bytes(value: [u8; 32]) -> Self {
        Self { value }
    }

    pub fn serialized_size() -> usize {
        32
    }

    pub fn serialize(&self, stream: &mut impl Stream) -> Result<()> {
        stream.write_bytes(&self.value)
    }

    pub fn deserialize(&mut self, stream: &mut impl Stream) -> Result<()> {
        let len = self.value.len();
        stream.read_bytes(&mut self.value, len)
    }

    pub fn to_be_bytes(&self) -> [u8; 32] {
        self.value
    }
}

#[derive(Clone, PartialEq, Eq)]
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

    pub fn serialized_size() -> usize {
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

    pub fn to_be_bytes(&self) -> [u8; 16] {
        self.value.to_be_bytes()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct Signature {
    bytes: [u8; 64],
}

impl Signature {
    pub fn new() -> Self {
        Self { bytes: [0u8; 64] }
    }

    pub fn from_be_bytes(bytes: [u8; 64]) -> Self {
        Self { bytes }
    }

    pub fn serialize(&self, stream: &mut impl Stream) -> Result<()> {
        stream.write_bytes(&self.bytes)
    }

    pub fn deserialize(stream: &mut impl Stream) -> Result<Signature> {
        let mut result = Signature { bytes: [0; 64] };

        stream.read_bytes(&mut result.bytes, 64)?;
        Ok(result)
    }

    pub fn to_be_bytes(&self) -> [u8; 64] {
        self.bytes
    }
}

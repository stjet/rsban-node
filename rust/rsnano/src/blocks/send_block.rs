use crate::{
    numbers::{Account, Amount, BlockHash, Signature},
    utils::{Blake2b, Stream},
};
use anyhow::Result;

use super::BlockType;

#[derive(Clone, PartialEq, Eq)]
pub struct SendHashables {
    pub previous: BlockHash,
    pub destination: Account,
    pub balance: Amount,
}

impl SendHashables {
    pub const fn serialized_size() -> usize {
        BlockHash::serialized_size() + Account::serialized_size() + Amount::serialized_size()
    }

    pub fn serialize(&self, stream: &mut impl Stream) -> Result<()> {
        self.previous.serialize(stream)?;
        self.destination.serialize(stream)?;
        self.balance.serialize(stream)?;
        Ok(())
    }

    pub fn deserialize(stream: &mut impl Stream) -> Result<Self> {
        let mut buffer_32 = [0u8; 32];
        let mut buffer_16 = [0u8; 16];

        stream.read_bytes(&mut buffer_32, 32)?;
        let previous = BlockHash::from_be_bytes(buffer_32);

        stream.read_bytes(&mut buffer_32, 32)?;
        let destination = Account::from_be_bytes(buffer_32);

        stream.read_bytes(&mut buffer_16, 16)?;
        let balance = Amount::new(u128::from_be_bytes(buffer_16));

        Ok(Self {
            previous,
            destination,
            balance,
        })
    }

    fn clear(&mut self) {
        self.previous = BlockHash::new();
        self.destination = Account::new();
        self.balance = Amount::new(0);
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct SendBlock {
    pub hashables: SendHashables,
    pub signature: Signature,
    pub work: u64,
}

impl SendBlock {
    pub const fn serialized_size() -> usize {
        SendHashables::serialized_size() + Signature::serialized_size() + std::mem::size_of::<u64>()
    }

    pub fn serialize(&self, stream: &mut impl Stream) -> Result<()> {
        self.hashables.serialize(stream)?;
        self.signature.serialize(stream)?;
        stream.write_bytes(&self.work.to_ne_bytes())
    }

    pub fn deserialize(&mut self, stream: &mut impl Stream) -> Result<()> {
        self.hashables = SendHashables::deserialize(stream)?;
        self.signature = Signature::deserialize(stream)?;

        let mut buffer = [0u8; 8];
        stream.read_bytes(&mut buffer, 8)?;
        self.work = u64::from_ne_bytes(buffer);

        Ok(())
    }

    pub fn zero(&mut self) {
        self.work = 0;
        self.signature = Signature::new();
        self.hashables.clear();
    }

    pub fn set_destination(&mut self, destination: Account) {
        self.hashables.destination = destination;
    }

    pub fn set_previous(&mut self, previous: BlockHash) {
        self.hashables.previous = previous;
    }

    pub fn set_balance(&mut self, balance: Amount) {
        self.hashables.balance = balance;
    }

    pub fn hash(&self, blake2b: &mut impl Blake2b) -> Result<()> {
        blake2b.update(&self.hashables.previous.to_be_bytes())?;
        blake2b.update(&self.hashables.destination.to_be_bytes())?;
        blake2b.update(&self.hashables.balance.to_be_bytes())?;
        Ok(())
    }

    pub fn valid_predecessor(block_type: BlockType) -> bool {
        match block_type {
            BlockType::Send | BlockType::Receive | BlockType::Open | BlockType::Change => true,
            BlockType::NotABlock | BlockType::State | BlockType::Invalid => false,
        }
    }
}

use crate::{
    numbers::{Account, Amount, BlockHash, Link, Signature},
    utils::{Blake2b, Stream},
};
use anyhow::Result;

use super::BlockType;

#[derive(Clone, PartialEq, Eq)]
pub struct StateHashables {
    // Account# / public key that operates this account
    // Uses:
    // Bulk signature validation in advance of further ledger processing
    // Arranging uncomitted transactions by account
    pub account: Account,

    // Previous transaction in this chain
    pub previous: BlockHash,

    // Representative of this account
    pub representative: Account,

    // Current balance of this account
    // Allows lookup of account balance simply by looking at the head block
    pub balance: Amount,

    // Link field contains source block_hash if receiving, destination account if sending
    pub link: Link,
}

#[derive(Clone)]
pub struct StateBlock {
    pub work: u64,
    pub signature: Signature,
    pub hashables: StateHashables,
}

impl StateBlock {
    pub const fn serialized_size() -> usize {
        Account::serialized_size() // Account
            + BlockHash::serialized_size() // Previous
            + Account::serialized_size() // Representative
            + Amount::serialized_size() // Balance
            + Link::serialized_size() // Link
            + Signature::serialized_size()
            + std::mem::size_of::<u64>() // Work
    }

    pub fn hash(&self, blake2b: &mut impl Blake2b) -> Result<()> {
        let mut preamble = [0u8; 32];
        preamble[31] = BlockType::State as u8;
        blake2b.update(&preamble)?;
        blake2b.update(&self.hashables.account.to_bytes())?;
        blake2b.update(&self.hashables.previous.to_be_bytes())?;
        blake2b.update(&self.hashables.representative.to_bytes())?;
        blake2b.update(&self.hashables.balance.to_be_bytes())?;
        blake2b.update(&self.hashables.link.to_be_bytes())?;
        Ok(())
    }

    pub fn serialize(&self, stream: &mut impl Stream) -> Result<()> {
        self.hashables.account.serialize(stream)?;
        self.hashables.previous.serialize(stream)?;
        self.hashables.representative.serialize(stream)?;
        self.hashables.balance.serialize(stream)?;
        self.hashables.link.serialize(stream)?;
        self.signature.serialize(stream)?;
        stream.write_bytes(&self.work.to_be_bytes())?;
        Ok(())
    }

    pub fn deserialize(&mut self, stream: &mut impl Stream) -> Result<()> {
        self.hashables.account.deserialize(stream)?;
        self.hashables.previous.deserialize(stream)?;
        self.hashables.representative.deserialize(stream)?;
        self.hashables.balance.deserialize(stream)?;
        self.hashables.link.deserialize(stream)?;
        self.signature = Signature::deserialize(stream)?;
        let mut work_bytes = [0u8; 8];
        stream.read_bytes(&mut work_bytes, 8)?;
        self.work = u64::from_be_bytes(work_bytes);
        Ok(())
    }
}

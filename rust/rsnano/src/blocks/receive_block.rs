use crate::{
    numbers::{BlockHash, Signature},
    utils::Blake2b,
};
use anyhow::Result;

#[derive(Clone, PartialEq, Eq)]
pub struct ReceiveHashables {
    pub previous: BlockHash,
    pub source: BlockHash,
}

impl ReceiveHashables {
    const fn serialized_size() -> usize {
        BlockHash::serialized_size() + BlockHash::serialized_size()
    }
}
#[derive(Clone)]
pub struct ReceiveBlock {
    pub work: u64,
    pub signature: Signature,
    pub hashables: ReceiveHashables,
}
impl ReceiveBlock {
    pub fn hash(&self, blake2b: &mut impl Blake2b) -> Result<()> {
        blake2b.update(&self.hashables.previous.to_be_bytes())?;
        blake2b.update(&self.hashables.source.to_be_bytes())?;
        Ok(())
    }

    pub const fn serialized_size() -> usize {
        ReceiveHashables::serialized_size()
            + Signature::serialized_size()
            + std::mem::size_of::<u64>()
    }
}

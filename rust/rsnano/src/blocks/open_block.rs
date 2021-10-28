use crate::{numbers::{Account, BlockHash, Signature}, utils::{Blake2b, Stream}};
use anyhow::Result;

#[derive(Clone, PartialEq, Eq)]
pub struct OpenHashables {
    pub source: BlockHash,
    pub representative: Account,
    pub account: Account,
}

impl OpenHashables {
    const fn serialized_size() -> usize {
        BlockHash::serialized_size() + Account::serialized_size() + Account::serialized_size()
    }
}

#[derive(Clone)]
pub struct OpenBlock {
    pub work: u64,
    pub signature: Signature,
    pub hashables: OpenHashables,
}
impl OpenBlock {
    pub const fn serialized_size() -> usize {
        OpenHashables::serialized_size() + Signature::serialized_size() + std::mem::size_of::<u64>()
    }

    pub fn hash(&self, blake2b: &mut impl Blake2b) -> Result<()> {
        blake2b.update(&self.hashables.source.to_be_bytes())?;
        blake2b.update(&self.hashables.representative.to_be_bytes())?;
        blake2b.update(&self.hashables.account.to_be_bytes())?;
        Ok(())
    }

    pub fn serialize(&self, stream: &mut impl Stream) -> Result<()> {
        self.hashables.source.serialize(stream)?;
        self.hashables.representative.serialize(stream)?;
        self.hashables.account.serialize(stream)?;
        self.signature.serialize(stream)?;
        stream.write_bytes(&self.work.to_be_bytes())?;
        Ok(())
    }

    pub fn deserialize(&mut self, stream: &mut impl Stream) -> Result<()> {
        self.hashables.source.deserialize(stream)?;
        self.hashables.representative.deserialize(stream)?;
        self.hashables.account.deserialize(stream)?;
        self.signature = Signature::deserialize(stream)?;
        let mut work_bytes = [0u8;8];
        stream.read_bytes(&mut work_bytes, 8)?;
        self.work = u64::from_be_bytes(work_bytes);
        Ok(())
    }
}

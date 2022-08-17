use crate::{BlockHash, Account, Amount, Epoch, utils::{Stream, StreamExt}};
use anyhow::Result;
use num_traits::FromPrimitive;

pub struct AccountInfo{
    pub head: BlockHash,
	pub representative: Account,
	pub open_block: BlockHash,
	pub balance: Amount,
	/** Seconds since posix epoch */
	pub modified: u64,
	pub block_count: u64,
	pub epoch: Epoch
}

impl AccountInfo{
    pub fn serialize(&self, stream: &mut impl Stream) -> Result<()>{
        self.head.serialize(stream)?;
        self.representative.serialize(stream)?;
        self.open_block.serialize(stream)?;
        self.balance.serialize(stream)?;
        stream.write_u64_ne(self.modified)?;
        stream.write_u64_ne(self.block_count)?;
        stream.write_u8(self.epoch as u8)
    }

    pub fn deserialize(stream: &mut impl Stream) -> Result<AccountInfo>{
        Ok(Self{
            head: BlockHash::deserialize(stream)?,
            representative: Account::deserialize(stream)?,
            open_block: BlockHash::deserialize(stream)?,
            balance: Amount::deserialize(stream)?,
            modified: stream.read_u64_ne()?,
            block_count: stream.read_u64_ne()?,
            epoch: Epoch::from_u8(stream.read_u8()?).ok_or_else(||anyhow!("invalid epoch"))?
        })
    }
}
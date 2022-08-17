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
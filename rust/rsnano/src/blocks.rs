use anyhow::Result;
use num::FromPrimitive;

use crate::{block_details::BlockDetails, epoch::Epoch, numbers::{Account, Amount, BlockHash, PublicKey, Signature}, utils::Stream};

#[repr(u8)]
#[derive(PartialEq, Eq, Debug, Clone, Copy, FromPrimitive)]
pub enum BlockType {
    Invalid = 0,
    NotABlock = 1,
    Send = 2,
    Receive = 3,
    Open = 4,
    Change = 5,
    State = 6,
}

pub struct BlockSideband {
    pub height: u64,
    pub timestamp: u64,
    pub successor: BlockHash,
    pub account: Account,
    pub balance: Amount,
    pub details: BlockDetails,
    pub source_epoch: Epoch,
}

impl BlockSideband {
    pub fn new(
        account: Account,
        successor: BlockHash,
        balance: Amount,
        height: u64,
        timestamp: u64,
        details: BlockDetails,
        source_epoch: Epoch,
    ) -> Self {
        Self {
            height,
            timestamp,
            successor,
            account,
            balance,
            details,
            source_epoch,
        }
    }

    pub fn serialized_size(block_type: BlockType) -> usize {
        let mut size = BlockHash::serialized_size(); // successor

        if block_type != BlockType::State && block_type != BlockType::Open {
            size += Account::serialized_size(); // account
        }

        if block_type != BlockType::Open {
            size += std::mem::size_of::<u64>(); // height
        }

        if block_type == BlockType::Receive
            || block_type == BlockType::Change
            || block_type == BlockType::Open
        {
            size += Amount::serialized_size(); // balance
        }

        size += std::mem::size_of::<u64>(); // timestamp

        if block_type == BlockType::State {
            // block_details must not be larger than the epoch enum
            const_assert!(std::mem::size_of::<Epoch>() == BlockDetails::serialized_size());
            size += BlockDetails::serialized_size() + std::mem::size_of::<Epoch>();
        }

        size
    }

    pub fn serialize(&self, stream: &mut impl Stream, block_type: BlockType) -> Result<()> {
        self.successor.serialize(stream)?;

        if block_type != BlockType::State && block_type != BlockType::Open {
            self.account.serialize(stream)?;
        }

        if block_type != BlockType::Open {
            stream.write_bytes(&self.height.to_be_bytes())?;
        }

        if block_type == BlockType::Receive
            || block_type == BlockType::Change
            || block_type == BlockType::Open
        {
            self.balance.serialize(stream)?;
        }

        stream.write_bytes(&self.timestamp.to_be_bytes())?;

        if block_type == BlockType::State {
            self.details.serialize(stream)?;
            stream.write_u8(self.source_epoch as u8)?;
        }

        Ok(())
    }

    pub fn deserialize(&mut self, stream: &mut impl Stream, block_type: BlockType) -> Result<()> {
        self.successor.deserialize(stream)?;

        if block_type != BlockType::State && block_type != BlockType::Open {
            self.account.deserialize(stream)?;
        }

        let mut buffer = [0u8; 8];
        if block_type != BlockType::Open {
            stream.read_bytes(&mut buffer, 8)?;
            self.height = u64::from_be_bytes(buffer);
        } else {
            self.height = 1;
        }

        if block_type == BlockType::Receive
            || block_type == BlockType::Change
            || block_type == BlockType::Open
        {
            self.balance.deserialize(stream)?;
        }

        stream.read_bytes(&mut buffer, 8)?;
        self.timestamp = u64::from_be_bytes(buffer);

        if block_type == BlockType::State {
            self.details = BlockDetails::deserialize(stream)?;
            self.source_epoch = FromPrimitive::from_u8(stream.read_u8()?)
                .ok_or_else(|| anyhow!("invalid epoch value"))?;
        }

        Ok(())
    }
}

pub struct SendHashables {
    pub previous: BlockHash,
    pub destination: Account,
    pub balance: Amount,
}

impl SendHashables {
    pub fn deserialize(stream: &mut impl Stream) -> Result<Self> {
        let mut buffer_32 = [0u8; 32];
        let mut buffer_16 = [0u8; 16];

        stream.read_bytes(&mut buffer_32, 32)?;
        let previous = BlockHash::new(buffer_32);

        stream.read_bytes(&mut buffer_32, 32)?;
        let destination = Account::new(PublicKey::new(buffer_32));

        stream.read_bytes(&mut buffer_16, 16)?;
        let balance = Amount::new(u128::from_be_bytes(buffer_16));

        Ok(Self {
            previous,
            destination,
            balance,
        })
    }
}

pub struct SendBlock{
    pub hashables: SendHashables,
    pub signature: Signature,
    pub work: u64
}

impl SendBlock{
    pub fn serialize(&self, stream: &mut impl Stream) -> Result<()>{
        Ok(())
    }
}
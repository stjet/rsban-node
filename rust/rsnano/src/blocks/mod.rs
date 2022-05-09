#[cfg(test)]
mod block_builder;
mod block_details;
mod block_uniquer;
mod change_block;
mod open_block;
mod receive_block;
mod send_block;
mod state_block;

use std::sync::{Arc, RwLock};

use anyhow::Result;
#[cfg(test)]
pub use block_builder::*;
pub use block_details::*;
pub(crate) use block_uniquer::*;
pub use change_block::*;
use num::FromPrimitive;
pub use open_block::*;
pub use receive_block::*;
pub use send_block::*;
pub use state_block::*;

use crate::{
    Account, Amount, BlockHash, BlockHashBuilder, Epoch, Link, PropertyTreeReader,
    PropertyTreeWriter, Signature, Stream,
};

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

#[derive(Debug, Clone)]
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
        self.successor = BlockHash::deserialize(stream)?;

        if block_type != BlockType::State && block_type != BlockType::Open {
            self.account = Account::deserialize(stream)?;
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
            self.balance = Amount::deserialize(stream)?;
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

pub fn serialized_block_size(block_type: BlockType) -> usize {
    match block_type {
        BlockType::Invalid | BlockType::NotABlock => 0,
        BlockType::Send => SendBlock::serialized_size(),
        BlockType::Receive => ReceiveBlock::serialized_size(),
        BlockType::Open => OpenBlock::serialized_size(),
        BlockType::Change => ChangeBlock::serialized_size(),
        BlockType::State => StateBlock::serialized_size(),
    }
}

#[derive(Clone, Default, Debug)]
pub struct LazyBlockHash {
    // todo: Remove Arc<RwLock>? Maybe remove lazy hash calculation?
    hash: Arc<RwLock<BlockHash>>,
}

impl LazyBlockHash {
    pub fn new() -> Self {
        Self {
            hash: Arc::new(RwLock::new(BlockHash::new())),
        }
    }
    pub fn hash(&'_ self, factory: impl Into<BlockHash>) -> BlockHash {
        let mut value = self.hash.read().unwrap();
        if value.is_zero() {
            drop(value);
            let mut x = self.hash.write().unwrap();
            let block_hash: BlockHash = factory.into();
            *x = block_hash;
            drop(x);
            value = self.hash.read().unwrap();
        }

        *value
    }

    pub(crate) fn clear(&self) {
        let mut x = self.hash.write().unwrap();
        *x = BlockHash::new();
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum BlockEnum {
    Send(SendBlock),
    Receive(ReceiveBlock),
    Open(OpenBlock),
    Change(ChangeBlock),
    State(StateBlock),
}

impl BlockEnum {
    pub fn block_type(&self) -> BlockType {
        self.as_block().block_type()
    }

    pub fn as_block_mut(&mut self) -> &mut dyn Block {
        match self {
            BlockEnum::Send(b) => b,
            BlockEnum::Receive(b) => b,
            BlockEnum::Open(b) => b,
            BlockEnum::Change(b) => b,
            BlockEnum::State(b) => b,
        }
    }

    pub fn as_block(&self) -> &dyn Block {
        match self {
            BlockEnum::Send(b) => b,
            BlockEnum::Receive(b) => b,
            BlockEnum::Open(b) => b,
            BlockEnum::Change(b) => b,
            BlockEnum::State(b) => b,
        }
    }
}

pub trait Block {
    fn block_type(&self) -> BlockType;
    fn account(&self) -> &Account;

    /**
     * Contextual details about a block, some fields may or may not be set depending on block type.
     * This field is set via sideband_set in ledger processing or deserializing blocks from the database.
     * Otherwise it may be null (for example, an old block or fork).
     */
    fn sideband(&'_ self) -> Option<&'_ BlockSideband>;
    fn set_sideband(&mut self, sideband: BlockSideband);
    fn hash(&self) -> BlockHash;
    fn full_hash(&self) -> BlockHash {
        BlockHashBuilder::new()
            .update(self.hash().as_bytes())
            .update(self.block_signature().as_bytes())
            .update(self.work().to_ne_bytes())
            .build()
    }
    fn link(&self) -> Link;
    fn block_signature(&self) -> &Signature;
    fn set_block_signature(&mut self, signature: &Signature);
    fn work(&self) -> u64;
    fn set_work(&mut self, work: u64);
    fn previous(&self) -> &BlockHash;
    fn serialize(&self, stream: &mut dyn Stream) -> Result<()>;
    fn serialize_json(&self, writer: &mut dyn PropertyTreeWriter) -> Result<()>;
}

pub fn deserialize_block_json(ptree: &impl PropertyTreeReader) -> anyhow::Result<BlockEnum> {
    let block_type = ptree.get_string("type")?;
    match block_type.as_str() {
        "receive" => ReceiveBlock::deserialize_json(ptree).map(BlockEnum::Receive),
        "send" => SendBlock::deserialize_json(ptree).map(BlockEnum::Send),
        "open" => OpenBlock::deserialize_json(ptree).map(BlockEnum::Open),
        "change" => ChangeBlock::deserialize_json(ptree).map(BlockEnum::Change),
        "state" => StateBlock::deserialize_json(ptree).map(BlockEnum::State),
        _ => Err(anyhow!("unsupported block type")),
    }
}

mod block_details;
pub use block_details::BlockDetails;

mod block_sideband;
pub use block_sideband::BlockSideband;

mod change_block;
pub use change_block::{valid_change_block_predecessor, ChangeBlock, ChangeHashables};

mod open_block;
pub use open_block::{OpenBlock, OpenHashables};

mod receive_block;
pub use receive_block::{valid_receive_block_predecessor, ReceiveBlock, ReceiveHashables};

mod send_block;
pub use send_block::{valid_send_block_predecessor, SendBlock, SendHashables};

mod state_block;
pub use state_block::{StateBlock, StateHashables};

mod builders;
pub use builders::*;

use crate::{
    utils::{Deserialize, PropertyTreeReader, PropertyTreeWriter, Stream},
    Account, Amount, BlockHash, BlockHashBuilder, FullHash, Link, QualifiedRoot, Root, Signature,
    WorkVersion,
};
use num::FromPrimitive;
use std::{
    ops::{Deref, DerefMut},
    sync::{Arc, RwLock},
};

#[repr(u8)]
#[derive(PartialEq, Eq, Debug, Clone, Copy, FromPrimitive)]
pub enum BlockType {
    Invalid = 0,
    NotABlock = 1,
    LegacySend = 2,
    LegacyReceive = 3,
    LegacyOpen = 4,
    LegacyChange = 5,
    State = 6,
}

impl TryFrom<u8> for BlockType {
    type Error = anyhow::Error;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        FromPrimitive::from_u8(value).ok_or_else(|| anyhow!("invalid block type value"))
    }
}

#[derive(PartialEq, Eq, Debug, Clone, Copy)]
pub enum BlockSubType {
    Send,
    Receive,
    Open,
    Change,
    Epoch,
}

#[derive(Clone, Default, Debug)]
pub struct LazyBlockHash {
    // todo: Remove Arc<RwLock>? Maybe remove lazy hash calculation?
    hash: Arc<RwLock<BlockHash>>,
}

impl LazyBlockHash {
    pub fn new() -> Self {
        Self {
            hash: Arc::new(RwLock::new(BlockHash::zero())),
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

    pub fn clear(&self) {
        let mut x = self.hash.write().unwrap();
        *x = BlockHash::zero();
    }
}

pub trait Block: FullHash {
    fn block_type(&self) -> BlockType;
    fn account(&self) -> Account;

    /**
     * Contextual details about a block, some fields may or may not be set depending on block type.
     * This field is set via sideband_set in ledger processing or deserializing blocks from the database.
     * Otherwise it may be null (for example, an old block or fork).
     */
    fn sideband(&'_ self) -> Option<&'_ BlockSideband>;
    fn set_sideband(&mut self, sideband: BlockSideband);
    fn hash(&self) -> BlockHash;
    fn link(&self) -> Link;
    fn block_signature(&self) -> &Signature;
    fn set_block_signature(&mut self, signature: &Signature);
    fn work(&self) -> u64;
    fn set_work(&mut self, work: u64);
    fn previous(&self) -> BlockHash;
    fn serialize(&self, stream: &mut dyn Stream) -> anyhow::Result<()>;
    fn serialize_json(&self, writer: &mut dyn PropertyTreeWriter) -> anyhow::Result<()>;
    fn work_version(&self) -> WorkVersion {
        WorkVersion::Work1
    }
    fn root(&self) -> Root;
    fn visit(&self, visitor: &mut dyn BlockVisitor);
    fn visit_mut(&mut self, visitor: &mut dyn MutableBlockVisitor);
    fn balance(&self) -> Amount;
    fn source(&self) -> Option<BlockHash>;
    fn representative(&self) -> Option<Account>;
    fn destination(&self) -> Option<Account>;
    fn qualified_root(&self) -> QualifiedRoot {
        QualifiedRoot::new(self.root(), self.previous())
    }
    fn valid_predecessor(&self, block_type: BlockType) -> bool;
}

impl<T: Block> FullHash for T {
    fn full_hash(&self) -> BlockHash {
        BlockHashBuilder::new()
            .update(self.hash().as_bytes())
            .update(self.block_signature().as_bytes())
            .update(self.work().to_ne_bytes())
            .build()
    }
}

pub trait BlockVisitor {
    fn send_block(&mut self, block: &SendBlock);
    fn receive_block(&mut self, block: &ReceiveBlock);
    fn open_block(&mut self, block: &OpenBlock);
    fn change_block(&mut self, block: &ChangeBlock);
    fn state_block(&mut self, block: &StateBlock);
}

pub trait MutableBlockVisitor {
    fn send_block(&mut self, block: &mut SendBlock);
    fn receive_block(&mut self, block: &mut ReceiveBlock);
    fn open_block(&mut self, block: &mut OpenBlock);
    fn change_block(&mut self, block: &mut ChangeBlock);
    fn state_block(&mut self, block: &mut StateBlock);
}

pub fn serialized_block_size(block_type: BlockType) -> usize {
    match block_type {
        BlockType::Invalid | BlockType::NotABlock => 0,
        BlockType::LegacySend => SendBlock::serialized_size(),
        BlockType::LegacyReceive => ReceiveBlock::serialized_size(),
        BlockType::LegacyOpen => OpenBlock::serialized_size(),
        BlockType::LegacyChange => ChangeBlock::serialized_size(),
        BlockType::State => StateBlock::serialized_size(),
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum BlockEnum {
    LegacySend(SendBlock),
    LegacyReceive(ReceiveBlock),
    LegacyOpen(OpenBlock),
    LegacyChange(ChangeBlock),
    State(StateBlock),
}

impl BlockEnum {
    pub fn block_type(&self) -> BlockType {
        self.as_block().block_type()
    }

    pub fn as_block_mut(&mut self) -> &mut dyn Block {
        match self {
            BlockEnum::LegacySend(b) => b,
            BlockEnum::LegacyReceive(b) => b,
            BlockEnum::LegacyOpen(b) => b,
            BlockEnum::LegacyChange(b) => b,
            BlockEnum::State(b) => b,
        }
    }

    pub fn as_block(&self) -> &dyn Block {
        match self {
            BlockEnum::LegacySend(b) => b,
            BlockEnum::LegacyReceive(b) => b,
            BlockEnum::LegacyOpen(b) => b,
            BlockEnum::LegacyChange(b) => b,
            BlockEnum::State(b) => b,
        }
    }

    pub fn balance_calculated(&self) -> Amount {
        match self {
            BlockEnum::LegacySend(b) => b.balance(),
            BlockEnum::LegacyReceive(b) => b.sideband().unwrap().balance,
            BlockEnum::LegacyOpen(b) => b.sideband().unwrap().balance,
            BlockEnum::LegacyChange(b) => b.sideband().unwrap().balance,
            BlockEnum::State(b) => b.balance(),
        }
    }

    pub fn is_open(&self) -> bool {
        match &self {
            BlockEnum::LegacyOpen(_) => true,
            BlockEnum::State(state) => state.previous().is_zero(),
            _ => false,
        }
    }

    pub fn is_legacy(&self) -> bool {
        match self {
            BlockEnum::State(_) => false,
            _ => true,
        }
    }

    pub fn source_or_link(&self) -> BlockHash {
        self.source().unwrap_or_else(|| self.link().into())
    }

    pub fn destination_or_link(&self) -> Account {
        self.destination().unwrap_or_else(|| self.link().into())
    }

    pub fn account_calculated(&self) -> Account {
        let result = if self.account().is_zero() {
            self.sideband().unwrap().account
        } else {
            self.account()
        };

        result
    }
}

impl FullHash for BlockEnum {
    fn full_hash(&self) -> BlockHash {
        self.as_block().full_hash()
    }
}

impl Deref for BlockEnum {
    type Target = dyn Block;

    fn deref(&self) -> &Self::Target {
        match self {
            BlockEnum::LegacySend(b) => b,
            BlockEnum::LegacyReceive(b) => b,
            BlockEnum::LegacyOpen(b) => b,
            BlockEnum::LegacyChange(b) => b,
            BlockEnum::State(b) => b,
        }
    }
}

impl DerefMut for BlockEnum {
    fn deref_mut(&mut self) -> &mut Self::Target {
        match self {
            BlockEnum::LegacySend(b) => b,
            BlockEnum::LegacyReceive(b) => b,
            BlockEnum::LegacyOpen(b) => b,
            BlockEnum::LegacyChange(b) => b,
            BlockEnum::State(b) => b,
        }
    }
}

pub fn deserialize_block_json(ptree: &impl PropertyTreeReader) -> anyhow::Result<BlockEnum> {
    let block_type = ptree.get_string("type")?;
    match block_type.as_str() {
        "receive" => ReceiveBlock::deserialize_json(ptree).map(BlockEnum::LegacyReceive),
        "send" => SendBlock::deserialize_json(ptree).map(BlockEnum::LegacySend),
        "open" => OpenBlock::deserialize_json(ptree).map(BlockEnum::LegacyOpen),
        "change" => ChangeBlock::deserialize_json(ptree).map(BlockEnum::LegacyChange),
        "state" => StateBlock::deserialize_json(ptree).map(BlockEnum::State),
        _ => Err(anyhow!("unsupported block type")),
    }
}

pub fn serialize_block_enum(stream: &mut dyn Stream, block: &BlockEnum) -> anyhow::Result<()> {
    let block_type = block.block_type() as u8;
    stream.write_u8(block_type)?;
    block.serialize(stream)
}

pub fn deserialize_block_enum(stream: &mut dyn Stream) -> anyhow::Result<BlockEnum> {
    let block_type =
        BlockType::from_u8(stream.read_u8()?).ok_or_else(|| anyhow!("invalid block type"))?;
    deserialize_block_enum_with_type(block_type, stream)
}

pub fn deserialize_block_enum_with_type(
    block_type: BlockType,
    stream: &mut dyn Stream,
) -> anyhow::Result<BlockEnum> {
    let block = match block_type {
        BlockType::LegacyReceive => BlockEnum::LegacyReceive(ReceiveBlock::deserialize(stream)?),
        BlockType::LegacyOpen => BlockEnum::LegacyOpen(OpenBlock::deserialize(stream)?),
        BlockType::LegacyChange => BlockEnum::LegacyChange(ChangeBlock::deserialize(stream)?),
        BlockType::State => BlockEnum::State(StateBlock::deserialize(stream)?),
        BlockType::LegacySend => BlockEnum::LegacySend(SendBlock::deserialize(stream)?),
        BlockType::Invalid | BlockType::NotABlock => bail!("invalid block type"),
    };
    Ok(block)
}

pub struct BlockWithSideband {
    pub block: BlockEnum,
    pub sideband: BlockSideband,
}

impl Deserialize for BlockWithSideband {
    type Target = Self;
    fn deserialize(stream: &mut dyn Stream) -> anyhow::Result<Self> {
        let mut block = deserialize_block_enum(stream)?;
        let sideband = BlockSideband::from_stream(stream, block.block_type())?;
        block.as_block_mut().set_sideband(sideband.clone());
        Ok(BlockWithSideband { block, sideband })
    }
}

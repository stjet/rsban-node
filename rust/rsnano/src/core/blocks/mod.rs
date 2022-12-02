use std::sync::{Arc, RwLock};

use anyhow::Result;
use num::FromPrimitive;
use rsnano_core::{
    utils::{Deserialize, Stream},
    BlockEnum, BlockSideband, BlockType, ChangeBlock, OpenBlock, ReceiveBlock, SendBlock,
    StateBlock,
};

use super::Uniquer;

pub type BlockUniquer = Uniquer<BlockEnum>;

pub fn deserialize_block(
    block_type: BlockType,
    stream: &mut dyn Stream,
    uniquer: Option<&BlockUniquer>,
) -> Result<Arc<RwLock<BlockEnum>>> {
    let block = deserialize_block_enum_with_type(block_type, stream)?;

    let mut block = Arc::new(RwLock::new(block));

    if let Some(uniquer) = uniquer {
        block = uniquer.unique(&block)
    }

    Ok(block)
}

pub fn serialize_block_enum(stream: &mut dyn Stream, block: &BlockEnum) -> Result<()> {
    let block_type = block.block_type() as u8;
    stream.write_u8(block_type)?;
    block.as_block().serialize(stream)
}

pub fn deserialize_block_enum(stream: &mut dyn Stream) -> Result<BlockEnum> {
    let block_type =
        BlockType::from_u8(stream.read_u8()?).ok_or_else(|| anyhow!("invalid block type"))?;
    deserialize_block_enum_with_type(block_type, stream)
}

pub fn deserialize_block_enum_with_type(
    block_type: BlockType,
    stream: &mut dyn Stream,
) -> Result<BlockEnum> {
    let block = match block_type {
        BlockType::Receive => BlockEnum::Receive(ReceiveBlock::deserialize(stream)?),
        BlockType::Open => BlockEnum::Open(OpenBlock::deserialize(stream)?),
        BlockType::Change => BlockEnum::Change(ChangeBlock::deserialize(stream)?),
        BlockType::State => BlockEnum::State(StateBlock::deserialize(stream)?),
        BlockType::Send => BlockEnum::Send(SendBlock::deserialize(stream)?),
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

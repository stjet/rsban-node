use std::sync::{Arc, RwLock};

use anyhow::Result;

use rsnano_core::{deserialize_block_enum_with_type, utils::Stream, BlockEnum, BlockType};

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

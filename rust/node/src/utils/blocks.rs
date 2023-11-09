use anyhow::Result;
use rsnano_core::{deserialize_block_enum_with_type, utils::Stream, BlockEnum, BlockType};
use std::sync::Arc;

pub fn deserialize_block(block_type: BlockType, stream: &mut dyn Stream) -> Result<Arc<BlockEnum>> {
    let block = deserialize_block_enum_with_type(block_type, stream)?;
    Ok(Arc::new(block))
}

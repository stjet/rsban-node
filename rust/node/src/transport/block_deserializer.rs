use super::Socket;
use num_traits::FromPrimitive;
use rsnano_core::{serialized_block_size, utils::BufferReader, BlockEnum, BlockType};

pub async fn read_block(socket: &Socket) -> anyhow::Result<Option<BlockEnum>> {
    let mut buf = [0; 1];
    socket.read_raw(&mut buf, 1).await?;
    received_type(buf[0], &socket).await
}

async fn received_type(block_type_byte: u8, socket: &Socket) -> anyhow::Result<Option<BlockEnum>> {
    match BlockType::from_u8(block_type_byte) {
        None | Some(BlockType::Invalid) => Err(anyhow!("Invalid block type: {block_type_byte}")),
        Some(BlockType::NotABlock) => Ok(None),
        Some(block_type) => {
            let block_size = serialized_block_size(block_type);
            let mut buffer = [0; 256];
            socket.read_raw(&mut buffer, block_size).await?;
            let mut stream = BufferReader::new(&buffer[..block_size]);
            let block = BlockEnum::deserialize_block_type(block_type, &mut stream)?;
            Ok(Some(block))
        }
    }
}

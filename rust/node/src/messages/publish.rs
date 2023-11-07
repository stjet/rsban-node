use super::MessageVariant;
use crate::utils::{deserialize_block, BlockUniquer};
use anyhow::Result;
use bitvec::prelude::BitArray;
use num_traits::FromPrimitive;
use rsnano_core::{
    serialized_block_size,
    utils::{BufferWriter, Serialize, Stream},
    BlockEnum, BlockType,
};
use std::{
    fmt::{Debug, Display},
    sync::Arc,
};

#[derive(Clone, Eq)]
pub struct Publish {
    pub block: Arc<BlockEnum>,
    pub digest: u128,
}

impl Publish {
    const BLOCK_TYPE_MASK: u16 = 0x0f00;

    pub fn deserialize(
        stream: &mut impl Stream,
        extensions: BitArray<u16>,
        digest: u128,
        uniquer: Option<&BlockUniquer>,
    ) -> Result<Self> {
        let payload = Publish {
            block: deserialize_block(Self::block_type(extensions), stream, uniquer)?,
            digest,
        };

        Ok(payload)
    }

    pub fn serialized_size(extensions: BitArray<u16>) -> usize {
        serialized_block_size(Self::block_type(extensions))
    }

    fn block_type(extensions: BitArray<u16>) -> BlockType {
        let mut value = extensions & BitArray::new(Self::BLOCK_TYPE_MASK);
        value.shift_left(8);
        FromPrimitive::from_u16(value.data).unwrap_or(BlockType::Invalid)
    }
}

impl PartialEq for Publish {
    fn eq(&self, other: &Self) -> bool {
        self.block == other.block
    }
}

impl Serialize for Publish {
    fn serialize(&self, stream: &mut dyn Stream) -> Result<()> {
        self.block.serialize(stream)
    }

    fn serialize_safe(&self, writer: &mut dyn BufferWriter) {
        self.block.serialize_safe(writer);
    }
}

impl MessageVariant for Publish {
    fn header_extensions(&self, _payload_len: u16) -> BitArray<u16> {
        BitArray::new((self.block.block_type() as u16) << 8)
    }
}

impl Debug for Publish {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PublishPayload")
            .field("digest", &self.digest)
            .finish()
    }
}

impl Display for Publish {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "\n{}",
            self.block.to_json().map_err(|_| std::fmt::Error)?
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rsnano_core::{utils::MemoryStream, BlockBuilder};

    #[test]
    fn serialize() {
        let block = BlockBuilder::state().build();
        let block = Arc::new(block);
        let publish1 = Publish { block, digest: 123 };

        let mut stream = MemoryStream::new();
        publish1.serialize(&mut stream).unwrap();

        let extensions = publish1.header_extensions(0);
        let publish2 = Publish::deserialize(&mut stream, extensions, 123, None).unwrap();
        assert_eq!(publish1, publish2);
    }
}

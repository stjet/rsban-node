use crate::utils::{deserialize_block, BlockUniquer};
use anyhow::Result;
use bitvec::prelude::BitArray;
use rsnano_core::{
    utils::{Serialize, Stream},
    BlockEnum,
};
use std::{
    fmt::{Debug, Display},
    sync::Arc,
};

use super::{MessageHeader, MessageType, MessageVariant};

#[derive(Clone, Eq)]
pub struct Publish {
    pub block: Arc<BlockEnum>,
    pub digest: u128,
}

impl Publish {
    const BLOCK_TYPE_MASK: u16 = 0x0f00;

    pub fn deserialize(
        stream: &mut impl Stream,
        header: &MessageHeader,
        digest: u128,
        uniquer: Option<&BlockUniquer>,
    ) -> Result<Self> {
        debug_assert!(header.message_type == MessageType::Publish);
        let payload = Publish {
            block: deserialize_block(header.block_type(), stream, uniquer)?,
            digest,
        };

        Ok(payload)
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
    use rsnano_core::{utils::MemoryStream, BlockBuilder, BlockType};

    #[test]
    fn serialize() {
        let block = BlockBuilder::state().build();
        let block = Arc::new(block);
        let publish1 = Publish { block, digest: 123 };

        let mut stream = MemoryStream::new();
        publish1.serialize(&mut stream).unwrap();

        let mut header = MessageHeader::new(MessageType::Publish, Default::default());
        header.set_block_type(BlockType::State);

        let publish2 = Publish::deserialize(&mut stream, &header, 123, None).unwrap();
        assert_eq!(publish1, publish2);
    }
}

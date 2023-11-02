use crate::utils::{deserialize_block, BlockUniquer};
use anyhow::Result;
use rsnano_core::{utils::Stream, BlockEnum};
use std::{
    fmt::{Debug, Display},
    ops::Deref,
    sync::Arc,
};

use super::{MessageHeader, MessageType};

#[derive(Clone, Eq)]
pub struct PublishPayload {
    pub block: Option<Arc<BlockEnum>>, //TODO remove Option
    pub digest: u128,
}

impl PublishPayload {
    pub fn deserialize(
        stream: &mut impl Stream,
        header: &MessageHeader,
        digest: u128,
        uniquer: Option<&BlockUniquer>,
    ) -> Result<Self> {
        debug_assert!(header.message_type == MessageType::Publish);
        let payload = PublishPayload {
            block: Some(deserialize_block(header.block_type(), stream, uniquer)?),
            digest,
        };

        Ok(payload)
    }

    pub fn serialize(&self, stream: &mut dyn Stream) -> Result<()> {
        let block = self.block.as_ref().ok_or_else(|| anyhow!("no block"))?;
        block.serialize(stream)
    }
}

impl PartialEq for PublishPayload {
    fn eq(&self, other: &Self) -> bool {
        if self.block.is_some() != other.block.is_some() {
            return false;
        }

        if let Some(b1) = &self.block {
            if let Some(b2) = &other.block {
                if b1.deref() != b2.deref() {
                    return false;
                }
            }
        }

        self.digest == other.digest
    }
}

impl Debug for PublishPayload {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PublishPayload")
            .field("digest", &self.digest)
            .finish()
    }
}

impl Display for PublishPayload {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(block) = &self.block {
            write!(f, "\n{}", block.to_json().map_err(|_| std::fmt::Error)?)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use rsnano_core::{utils::MemoryStream, BlockBuilder, BlockType};

    use super::*;
    use crate::DEV_NETWORK_PARAMS;

    #[test]
    fn serialize() {
        let block = BlockBuilder::state().build();
        let block = Arc::new(block);
        let network = &DEV_NETWORK_PARAMS.network;
        let publish1 = PublishPayload {
            block: Some(block),
            digest: 123,
        };

        let mut stream = MemoryStream::new();
        publish1.serialize(&mut stream).unwrap();

        let mut header = MessageHeader::new(MessageType::Publish, &Default::default());
        header.set_block_type(BlockType::State);

        let publish2 = PublishPayload::deserialize(&mut stream, &header, 123, None).unwrap();
        assert_eq!(publish1, publish2);
    }
}

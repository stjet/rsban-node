use crate::{
    config::NetworkConstants,
    utils::{deserialize_block, BlockUniquer},
};
use anyhow::Result;
use rsnano_core::{
    serialized_block_size,
    utils::{Deserialize, Serialize, Stream},
    BlockEnum, BlockHash, BlockType, Root,
};
use std::{
    any::Any,
    fmt::{Debug, Display, Write},
    ops::Deref,
    sync::Arc,
};

use super::{Message, MessageHeader, MessageType, MessageVisitor};

#[derive(Clone)]
pub struct ConfirmReq {
    header: MessageHeader,
    block: Option<Arc<BlockEnum>>,
    roots_hashes: Vec<(BlockHash, Root)>,
}

impl ConfirmReq {
    pub fn with_block(constants: &NetworkConstants, block: Arc<BlockEnum>) -> Self {
        let mut header = MessageHeader::new(MessageType::ConfirmReq, &constants.protocol_info());
        header.set_block_type(block.block_type());

        Self {
            header,
            block: Some(block),
            roots_hashes: Vec::new(),
        }
    }

    pub fn with_roots_hashes(
        constants: &NetworkConstants,
        roots_hashes: Vec<(BlockHash, Root)>,
    ) -> Self {
        let mut header = MessageHeader::new(MessageType::ConfirmReq, &constants.protocol_info());
        // not_a_block (1) block type for hashes + roots request
        header.set_block_type(BlockType::NotABlock);

        debug_assert!(roots_hashes.len() < 16);
        header.set_count(roots_hashes.len() as u8);

        Self {
            header,
            block: None,
            roots_hashes,
        }
    }

    pub fn with_header(header: MessageHeader) -> Self {
        Self {
            header,
            block: None,
            roots_hashes: Vec::new(),
        }
    }

    pub fn from_stream(
        stream: &mut impl Stream,
        header: MessageHeader,
        uniquer: Option<&BlockUniquer>,
    ) -> Result<Self> {
        let mut msg = Self::with_header(header);
        msg.deserialize(stream, uniquer)?;
        Ok(msg)
    }

    pub fn block(&self) -> Option<&Arc<BlockEnum>> {
        self.block.as_ref()
    }

    pub fn roots_hashes(&self) -> &[(BlockHash, Root)] {
        &self.roots_hashes
    }

    pub fn deserialize(
        &mut self,
        stream: &mut impl Stream,
        uniquer: Option<&BlockUniquer>,
    ) -> Result<()> {
        debug_assert!(self.header().message_type == MessageType::ConfirmReq);

        if self.header().block_type() == BlockType::NotABlock {
            let count = self.header().count() as usize;
            for _ in 0..count {
                let block_hash = BlockHash::deserialize(stream)?;
                let root = Root::deserialize(stream)?;
                if !block_hash.is_zero() || !root.is_zero() {
                    self.roots_hashes.push((block_hash, root));
                }
            }

            if self.roots_hashes.is_empty() || self.roots_hashes.len() != count {
                bail!("roots hashes empty or incorrect count");
            }
        } else {
            self.block = Some(deserialize_block(
                self.header().block_type(),
                stream,
                uniquer,
            )?);
        }

        Ok(())
    }

    pub fn roots_string(&self) -> String {
        let mut result = String::new();
        for (hash, root) in self.roots_hashes() {
            write!(&mut result, "{}:{}, ", hash, root).unwrap();
        }
        result
    }

    pub fn serialized_size(block_type: BlockType, count: u8) -> usize {
        let mut result = 0;
        if block_type != BlockType::Invalid && block_type != BlockType::NotABlock {
            result = serialized_block_size(block_type);
        } else if block_type == BlockType::NotABlock {
            result = count as usize * (BlockHash::serialized_size() + Root::serialized_size());
        }
        result
    }
}

impl Message for ConfirmReq {
    fn header(&self) -> &MessageHeader {
        &self.header
    }

    fn set_header(&mut self, header: &MessageHeader) {
        self.header = header.clone();
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn serialize(&self, stream: &mut dyn Stream) -> Result<()> {
        self.header().serialize(stream)?;
        if self.header().block_type() == BlockType::NotABlock {
            debug_assert!(!self.roots_hashes().is_empty());
            // Write hashes & roots
            for (hash, root) in self.roots_hashes() {
                stream.write_bytes(hash.as_bytes())?;
                stream.write_bytes(root.as_bytes())?;
            }
        } else {
            match self.block() {
                Some(block) => {
                    block.serialize(stream)?;
                }
                None => bail!("block not set"),
            }
        }

        Ok(())
    }

    fn visit(&self, visitor: &mut dyn MessageVisitor) {
        visitor.confirm_req(self)
    }

    fn clone_box(&self) -> Box<dyn Message> {
        Box::new(self.clone())
    }

    fn message_type(&self) -> MessageType {
        MessageType::ConfirmReq
    }
}

impl PartialEq for ConfirmReq {
    fn eq(&self, other: &Self) -> bool {
        let mut equal = false;
        if let Some(block_a) = self.block() {
            if let Some(block_b) = other.block() {
                equal = block_a.deref().eq(&block_b.deref());
            }
        } else if !self.roots_hashes().is_empty() && !other.roots_hashes().is_empty() {
            equal = self.roots_hashes() == other.roots_hashes()
        }

        equal
    }
}

impl Debug for ConfirmReq {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConfirmReq")
            .field("header", &self.header)
            .field("roots_hashes", &self.roots_hashes)
            .finish()
    }
}

impl Display for ConfirmReq {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.header, f)?;
        if self.header.block_type() == BlockType::NotABlock {
            for (hash, root) in &self.roots_hashes {
                write!(f, "\n{}:{}", hash, root)?;
            }
        } else if let Some(block) = &self.block {
            write!(f, "\n{}", block.to_json().map_err(|_| std::fmt::Error)?)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rsnano_core::{utils::MemoryStream, StateBlockBuilder};

    #[test]
    fn serialize_block() -> Result<()> {
        let block = Arc::new(StateBlockBuilder::new().build());
        let constants = NetworkConstants::empty();
        let confirm_req1 = ConfirmReq::with_block(&constants, block);
        let confirm_req2 = serialize_and_deserialize(&confirm_req1)?;
        assert_eq!(confirm_req1, confirm_req2);
        Ok(())
    }

    #[test]
    fn serialze_roots_hashes() -> Result<()> {
        let constants = NetworkConstants::empty();
        let roots_hashes = vec![(BlockHash::from(1), Root::from(2))];
        let confirm_req1 = ConfirmReq::with_roots_hashes(&constants, roots_hashes);
        let confirm_req2 = serialize_and_deserialize(&confirm_req1)?;
        assert_eq!(confirm_req1, confirm_req2);
        Ok(())
    }

    #[test]
    fn serialze_many_roots_hashes() -> Result<()> {
        let constants = NetworkConstants::empty();
        let roots_hashes = (0..7)
            .into_iter()
            .map(|i| (BlockHash::from(i), Root::from(i + 1)))
            .collect();
        let confirm_req1 = ConfirmReq::with_roots_hashes(&constants, roots_hashes);
        let confirm_req2 = serialize_and_deserialize(&confirm_req1)?;
        assert_eq!(confirm_req1, confirm_req2);
        Ok(())
    }

    fn serialize_and_deserialize(confirm_req1: &ConfirmReq) -> Result<ConfirmReq, anyhow::Error> {
        let mut stream = MemoryStream::new();
        confirm_req1.serialize(&mut stream)?;
        let header = MessageHeader::from_stream(&mut stream)?;
        let mut confirm_req2 = ConfirmReq::with_header(header);
        confirm_req2.deserialize(&mut stream, None)?;
        Ok(confirm_req2)
    }
}

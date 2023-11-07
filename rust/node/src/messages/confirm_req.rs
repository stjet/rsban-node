use super::MessageVariant;
use crate::utils::{deserialize_block, BlockUniquer};
use anyhow::Result;
use bitvec::prelude::BitArray;
use num_traits::FromPrimitive;
use rsnano_core::{
    serialized_block_size,
    utils::{Deserialize, FixedSizeSerialize, MutStreamAdapter, Serialize, Stream},
    BlockEnum, BlockHash, BlockType, Root,
};
use std::{
    fmt::{Debug, Display, Write},
    sync::Arc,
};

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ConfirmReq {
    pub block: Option<Arc<BlockEnum>>,
    pub roots_hashes: Vec<(BlockHash, Root)>,
}

impl ConfirmReq {
    const BLOCK_TYPE_MASK: u16 = 0x0f00;
    const COUNT_MASK: u16 = 0xf000;

    fn block_type(extensions: BitArray<u16>) -> BlockType {
        let mut value = extensions & BitArray::new(Self::BLOCK_TYPE_MASK);
        value.shift_left(8);
        FromPrimitive::from_u16(value.data).unwrap_or(BlockType::Invalid)
    }

    pub fn count(extensions: BitArray<u16>) -> u8 {
        let mut value = extensions & BitArray::new(Self::COUNT_MASK);
        value.shift_left(12);
        value.data as u8
    }

    pub fn deserialize(
        stream: &mut impl Stream,
        extensions: BitArray<u16>,
        uniquer: Option<&BlockUniquer>,
    ) -> Result<Self> {
        let block_type = Self::block_type(extensions);
        if block_type == BlockType::NotABlock {
            Ok(Self {
                block: None,
                roots_hashes: Self::deserialize_roots(stream, extensions)?,
            })
        } else {
            Ok(Self {
                block: Some(deserialize_block(block_type, stream, uniquer)?),
                roots_hashes: Vec::new(),
            })
        }
    }

    fn deserialize_roots(
        stream: &mut impl Stream,
        extensions: BitArray<u16>,
    ) -> Result<Vec<(BlockHash, Root)>> {
        let count = Self::count(extensions) as usize;
        let mut roots_hashes = Vec::with_capacity(count);
        for _ in 0..count {
            let block_hash = BlockHash::deserialize(stream)?;
            let root = Root::deserialize(stream)?;
            if !block_hash.is_zero() || !root.is_zero() {
                roots_hashes.push((block_hash, root));
            }
        }

        if roots_hashes.is_empty() || roots_hashes.len() != count {
            bail!("roots hashes empty or incorrect count");
        }

        Ok(roots_hashes)
    }

    pub fn roots_string(&self) -> String {
        let mut result = String::new();
        for (hash, root) in &self.roots_hashes {
            write!(&mut result, "{}:{}, ", hash, root).unwrap();
        }
        result
    }

    pub fn serialized_size(extensions: BitArray<u16>) -> usize {
        let count = Self::count(extensions);
        let mut result = 0;
        let block_type = Self::block_type(extensions);
        if block_type != BlockType::Invalid && block_type != BlockType::NotABlock {
            result = serialized_block_size(block_type);
        } else if block_type == BlockType::NotABlock {
            result = count as usize * (BlockHash::serialized_size() + Root::serialized_size());
        }
        result
    }
}

impl Serialize for ConfirmReq {
    fn serialize(&self, stream: &mut dyn Stream) -> Result<()> {
        if let Some(block) = &self.block {
            block.serialize(stream)?;
        } else {
            // Write hashes & roots
            for (hash, root) in &self.roots_hashes {
                stream.write_bytes(hash.as_bytes())?;
                stream.write_bytes(root.as_bytes())?;
            }
        }
        Ok(())
    }

    fn serialize_safe(&self, stream: &mut MutStreamAdapter) {
        if let Some(block) = &self.block {
            block.serialize_safe(stream);
        } else {
            // Write hashes & roots
            for (hash, root) in &self.roots_hashes {
                stream.write_bytes_safe(hash.as_bytes());
                stream.write_bytes_safe(root.as_bytes());
            }
        }
    }
}

impl MessageVariant for ConfirmReq {
    fn header_extensions(&self, _payload_len: u16) -> BitArray<u16> {
        let block_type = match &self.block {
            Some(b) => b.block_type(),
            None => BlockType::NotABlock,
        };
        debug_assert!(self.roots_hashes.len() < 16);
        let mut extensions = BitArray::default();
        extensions |= BitArray::new((self.roots_hashes.len() as u16) << 12);
        extensions |= BitArray::new((block_type as u16) << 8);
        extensions
    }
}

impl Display for ConfirmReq {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(block) = &self.block {
            write!(f, "\n{}", block.to_json().map_err(|_| std::fmt::Error)?)?;
        } else {
            for (hash, root) in &self.roots_hashes {
                write!(f, "\n{}:{}", hash, root)?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::messages::{assert_deserializable, Message};

    use super::*;
    use rsnano_core::{LegacyReceiveBlockBuilder, StateBlockBuilder};

    #[test]
    fn serialize_block() {
        let block = Arc::new(StateBlockBuilder::new().build());
        let confirm_req = Message::ConfirmReq(ConfirmReq {
            block: Some(block),
            roots_hashes: Vec::new(),
        });
        assert_deserializable(&confirm_req);
    }

    #[test]
    fn serialize_roots_hashes() {
        let confirm_req = Message::ConfirmReq(ConfirmReq {
            block: None,
            roots_hashes: vec![(BlockHash::from(1), Root::from(2))],
        });
        assert_deserializable(&confirm_req);
    }

    #[test]
    fn serialize_many_roots_hashes() {
        let roots_hashes = (0..7)
            .into_iter()
            .map(|i| (BlockHash::from(i), Root::from(i + 1)))
            .collect();
        let confirm_req = Message::ConfirmReq(ConfirmReq {
            block: None,
            roots_hashes,
        });
        assert_deserializable(&confirm_req);
    }

    #[test]
    fn set_block_type_extension() {
        let block = Arc::new(StateBlockBuilder::new().build());
        let confirm_req = ConfirmReq {
            block: Some(block),
            roots_hashes: Vec::new(),
        };
        let extensions = confirm_req.header_extensions(0);
        assert_eq!(ConfirmReq::block_type(extensions), BlockType::State);
    }

    #[test]
    fn get_block_type_from_header() {
        let extensions = Default::default();
        assert_eq!(ConfirmReq::block_type(extensions), BlockType::Invalid);

        let block = Arc::new(LegacyReceiveBlockBuilder::new().build());
        let confirm_req = ConfirmReq {
            block: Some(block),
            roots_hashes: Vec::new(),
        };
        let extensions = confirm_req.header_extensions(0);
        assert_eq!(ConfirmReq::block_type(extensions), BlockType::LegacyReceive);
    }
}

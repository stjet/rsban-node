use super::MessageVariant;
use anyhow::Result;
use bitvec::prelude::BitArray;
use num_traits::FromPrimitive;
use rsnano_core::{
    serialized_block_size,
    utils::{BufferWriter, Deserialize, FixedSizeSerialize, Serialize, Stream},
    BlockEnum, BlockHash, BlockType, Root,
};
use serde::ser::{SerializeSeq, SerializeStruct};
use std::fmt::{Debug, Display, Write};

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ConfirmReq {
    pub block: Option<BlockEnum>,
    pub roots_hashes: Vec<(BlockHash, Root)>,
}

impl ConfirmReq {
    const BLOCK_TYPE_MASK: u16 = 0x0f00;
    const COUNT_MASK: u16 = 0xf000;

    pub fn create_test_instance_block() -> Self {
        Self {
            block: Some(BlockEnum::create_test_instance()),
            roots_hashes: Vec::new(),
        }
    }

    pub fn create_test_instance_roots() -> Self {
        Self {
            block: None,
            roots_hashes: vec![(BlockHash::from(123), Root::from(456))],
        }
    }

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

    pub fn deserialize(stream: &mut impl Stream, extensions: BitArray<u16>) -> Option<Self> {
        let block_type = Self::block_type(extensions);
        if block_type == BlockType::NotABlock {
            Some(Self {
                block: None,
                roots_hashes: Self::deserialize_roots(stream, extensions).ok()?,
            })
        } else {
            Some(Self {
                block: Some(BlockEnum::deserialize_block_type(block_type, stream).ok()?),
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
    fn serialize(&self, writer: &mut dyn BufferWriter) {
        if let Some(block) = &self.block {
            block.serialize_without_block_type(writer);
        } else {
            // Write hashes & roots
            for (hash, root) in &self.roots_hashes {
                writer.write_bytes_safe(hash.as_bytes());
                writer.write_bytes_safe(root.as_bytes());
            }
        }
    }
}

struct SerializableRootsHashes<'a>(&'a Vec<(BlockHash, Root)>);

impl<'a> serde::Serialize for SerializableRootsHashes<'a> {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut seq = serializer.serialize_seq(Some(self.0.len()))?;
        for item in self.0.iter() {
            seq.serialize_element(&SerializableRootHash(item))?;
        }
        seq.end()
    }
}

struct SerializableRootHash<'a>(&'a (BlockHash, Root));

impl<'a> serde::Serialize for SerializableRootHash<'a> {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut seq = serializer.serialize_struct("RootHash", 2)?;
        seq.serialize_field("hash", &self.0 .0)?;
        seq.serialize_field("root", &self.0 .1.encode_hex())?;
        seq.end()
    }
}

impl serde::Serialize for ConfirmReq {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state = serializer.serialize_struct("ConfirmReq", 6)?;
        if let Some(block) = &self.block {
            state.serialize_field("confirm_type", "block")?;
            state.serialize_field("block", block)?;
        } else {
            state.serialize_field("confirm_type", "roots_hashes")?;
            state.serialize_field("roots_hashes", &SerializableRootsHashes(&self.roots_hashes))?;
        }
        state.end()
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
    use super::*;
    use crate::{assert_deserializable, Message};
    use rsnano_core::{LegacyReceiveBlockBuilder, StateBlockBuilder};

    #[test]
    fn serialize_block() {
        let block = StateBlockBuilder::new().build();
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
        let block = StateBlockBuilder::new().build();
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

        let block = LegacyReceiveBlockBuilder::new().build();
        let confirm_req = ConfirmReq {
            block: Some(block),
            roots_hashes: Vec::new(),
        };
        let extensions = confirm_req.header_extensions(0);
        assert_eq!(ConfirmReq::block_type(extensions), BlockType::LegacyReceive);
    }

    #[test]
    fn serialize_json_block() {
        let serialized = serde_json::to_string_pretty(&Message::ConfirmReq(
            ConfirmReq::create_test_instance_block(),
        ))
        .unwrap();

        assert_eq!(
            serialized,
            r#"{
  "message_type": "confirm_req",
  "confirm_type": "block",
  "block": {
    "type": "state",
    "account": "nano_111111111111111111111111111111111111111111111111115uwdgas549",
    "previous": "00000000000000000000000000000000000000000000000000000000000001C8",
    "representative": "nano_11111111111111111111111111111111111111111111111111ros3kc7wyy",
    "balance": "420",
    "link": "000000000000000000000000000000000000000000000000000000000000006F",
    "link_as_account": "nano_111111111111111111111111111111111111111111111111115hkrzwewgm",
    "signature": "9C6E535FABB72F90E410B72192102BA13B77BDC58D77B94DF8B7A704D74698C5F9BCB01667A5D9788DB02AAFE8F46DCB898488487BB375283BC39CA61A678204",
    "work": "0000000000010F2C"
  }
}"#
        );
    }

    #[test]
    fn serialize_json_roots_hashes() {
        let serialized = serde_json::to_string_pretty(&Message::ConfirmReq(
            ConfirmReq::create_test_instance_roots(),
        ))
        .unwrap();

        assert_eq!(
            serialized,
            r#"{
  "message_type": "confirm_req",
  "confirm_type": "roots_hashes",
  "roots_hashes": [
    {
      "hash": "000000000000000000000000000000000000000000000000000000000000007B",
      "root": "00000000000000000000000000000000000000000000000000000000000001C8"
    }
  ]
}"#
        );
    }
}

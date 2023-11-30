use super::MessageVariant;
use bitvec::prelude::BitArray;
use num_traits::FromPrimitive;
use rsnano_core::{
    serialized_block_size,
    utils::{BufferWriter, Serialize, Stream},
    BlockEnum, BlockType,
};
use serde_derive::Serialize;
use std::fmt::{Debug, Display};

#[derive(Clone, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct Publish {
    pub block: BlockEnum,
    #[serde(skip_serializing)]
    pub digest: u128,
}

impl Publish {
    const BLOCK_TYPE_MASK: u16 = 0x0f00;

    pub fn create_test_instance() -> Self {
        Self {
            block: BlockEnum::create_test_instance(),
            digest: 0,
        }
    }

    pub fn deserialize(
        stream: &mut impl Stream,
        extensions: BitArray<u16>,
        digest: u128,
    ) -> Option<Self> {
        let payload = Publish {
            block: BlockEnum::deserialize_block_type(Self::block_type(extensions), stream).ok()?,
            digest,
        };

        Some(payload)
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
    fn serialize(&self, writer: &mut dyn BufferWriter) {
        self.block.serialize_without_block_type(writer);
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
    use crate::Message;

    use super::*;
    use rsnano_core::{utils::MemoryStream, BlockBuilder};

    #[test]
    fn serialize() {
        let block = BlockBuilder::state().build();
        let publish1 = Publish { block, digest: 123 };

        let mut stream = MemoryStream::new();
        publish1.serialize(&mut stream);

        let extensions = publish1.header_extensions(0);
        let publish2 = Publish::deserialize(&mut stream, extensions, 123).unwrap();
        assert_eq!(publish1, publish2);
    }

    #[test]
    fn serialize_json() {
        let serialized =
            serde_json::to_string_pretty(&Message::Publish(Publish::create_test_instance()))
                .unwrap();
        assert_eq!(
            serialized,
            r#"{
  "message_type": "publish",
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
}

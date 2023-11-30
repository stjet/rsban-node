use bitvec::prelude::BitArray;
use num_traits::FromPrimitive;
use rsnano_core::{
    utils::{BufferWriter, Deserialize, Serialize, Stream, StreamExt},
    Account, BlockEnum, BlockHash, BlockType,
};
use serde::ser::SerializeStruct;
use serde_derive::Serialize;
use std::{fmt::Display, mem::size_of};

use super::{AscPullPayloadId, MessageVariant};

#[derive(Clone, PartialEq, Eq, Debug, Serialize)]
#[serde(rename_all = "snake_case", tag = "pull_type")]
pub enum AscPullAckType {
    Blocks(BlocksAckPayload),
    AccountInfo(AccountInfoAckPayload),
}

#[derive(Clone, PartialEq, Eq, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct AscPullAck {
    pub id: u64,
    #[serde(flatten)]
    pub pull_type: AscPullAckType,
}

impl AscPullAck {
    pub fn create_test_instance_blocks() -> Self {
        Self {
            id: 12345,
            pull_type: AscPullAckType::Blocks(BlocksAckPayload(vec![
                BlockEnum::create_test_instance(),
            ])),
        }
    }

    pub fn create_test_instance_account() -> Self {
        Self {
            id: 12345,
            pull_type: AscPullAckType::AccountInfo(AccountInfoAckPayload::create_test_instance()),
        }
    }

    pub fn deserialize(stream: &mut impl Stream) -> Option<Self> {
        let pull_type_code = AscPullPayloadId::from_u8(stream.read_u8().ok()?)?;
        let id = stream.read_u64_be().ok()?;
        let pull_type = match pull_type_code {
            AscPullPayloadId::Blocks => {
                let mut payload = BlocksAckPayload::default();
                payload.deserialize(stream).ok()?;
                AscPullAckType::Blocks(payload)
            }
            AscPullPayloadId::AccountInfo => {
                let mut payload = AccountInfoAckPayload::default();
                payload.deserialize(stream).ok()?;
                AscPullAckType::AccountInfo(payload)
            }
        };

        Some(AscPullAck { id, pull_type })
    }

    pub fn payload_type(&self) -> AscPullPayloadId {
        match self.pull_type {
            AscPullAckType::Blocks(_) => AscPullPayloadId::Blocks,
            AscPullAckType::AccountInfo(_) => AscPullPayloadId::AccountInfo,
        }
    }

    fn serialize_pull_type(&self, writer: &mut dyn BufferWriter) {
        match &self.pull_type {
            AscPullAckType::Blocks(blocks) => blocks.serialize(writer),
            AscPullAckType::AccountInfo(account_info) => account_info.serialize(writer),
        }
    }

    pub fn serialized_size(extensions: BitArray<u16>) -> usize {
        let payload_length = extensions.data as usize;

        size_of::<u8>() // type code 
        + size_of::<u64>() // id
        + payload_length
    }
}

impl Serialize for AscPullAck {
    fn serialize(&self, writer: &mut dyn BufferWriter) {
        writer.write_u8_safe(self.payload_type() as u8);
        writer.write_u64_be_safe(self.id);
        self.serialize_pull_type(writer);
    }
}

impl MessageVariant for AscPullAck {
    fn header_extensions(&self, payload_len: u16) -> BitArray<u16> {
        BitArray::new(
            payload_len
            -1 // pull_type
            - 8, // ID
        )
    }
}

impl Display for AscPullAck {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.pull_type {
            AscPullAckType::Blocks(blocks) => {
                for block in blocks.blocks() {
                    write!(f, "{}", block.to_json().map_err(|_| std::fmt::Error)?)?;
                }
            }
            AscPullAckType::AccountInfo(info) => {
                write!(
                    f,
                    "\naccount public key:{} account open:{} account head:{} block count:{} confirmation frontier:{} confirmation height:{}",
                    info.account.encode_account(),
                    info.account_open,
                    info.account_head,
                    info.account_block_count,
                    info.account_conf_frontier,
                    info.account_conf_height,
                )?;
            }
        }
        Ok(())
    }
}

#[derive(Clone, Default, PartialEq, Eq, Debug)]
pub struct BlocksAckPayload(Vec<BlockEnum>);

impl BlocksAckPayload {
    pub fn new(blocks: Vec<BlockEnum>) -> Self {
        if blocks.len() > Self::MAX_BLOCKS {
            panic!(
                "too many blocks for BlocksAckPayload. Maximum is {}, but was {}",
                Self::MAX_BLOCKS,
                blocks.len()
            );
        }
        Self(blocks)
    }

    /* Header allows for 16 bit extensions; 65535 bytes / 500 bytes (block size with some future margin) ~ 131 */
    pub const MAX_BLOCKS: usize = 128;

    pub fn blocks(&self) -> &Vec<BlockEnum> {
        &self.0
    }

    pub fn deserialize(&mut self, stream: &mut dyn Stream) -> anyhow::Result<()> {
        while let Ok(current) = BlockEnum::deserialize(stream) {
            if self.0.len() >= Self::MAX_BLOCKS {
                bail!("too many blocks")
            }
            self.0.push(current);
        }
        Ok(())
    }
}

impl Serialize for BlocksAckPayload {
    fn serialize(&self, writer: &mut dyn BufferWriter) {
        for block in self.blocks() {
            block.serialize(writer);
        }
        // For convenience, end with null block terminator
        writer.write_u8_safe(BlockType::NotABlock as u8)
    }
}

impl serde::Serialize for BlocksAckPayload {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state = serializer.serialize_struct("Block", 6)?;
        state.serialize_field("blocks", &self.0)?;
        state.end()
    }
}

#[derive(Clone, Default, PartialEq, Eq, Debug, Serialize)]
pub struct AccountInfoAckPayload {
    pub account: Account,
    pub account_open: BlockHash,
    pub account_head: BlockHash,
    pub account_block_count: u64,
    pub account_conf_frontier: BlockHash,
    pub account_conf_height: u64,
}

impl AccountInfoAckPayload {
    pub fn deserialize(&mut self, stream: &mut dyn Stream) -> anyhow::Result<()> {
        self.account = Account::deserialize(stream)?;
        self.account_open = BlockHash::deserialize(stream)?;
        self.account_head = BlockHash::deserialize(stream)?;
        self.account_block_count = stream.read_u64_be()?;
        self.account_conf_frontier = BlockHash::deserialize(stream)?;
        self.account_conf_height = stream.read_u64_be()?;
        Ok(())
    }

    pub(crate) fn create_test_instance() -> AccountInfoAckPayload {
        Self {
            account: Account::from(1),
            account_open: BlockHash::from(2),
            account_head: BlockHash::from(3),
            account_block_count: 4,
            account_conf_frontier: BlockHash::from(5),
            account_conf_height: 3,
        }
    }
}

impl Serialize for AccountInfoAckPayload {
    fn serialize(&self, writer: &mut dyn BufferWriter) {
        self.account.serialize(writer);
        self.account_open.serialize(writer);
        self.account_head.serialize(writer);
        writer.write_u64_be_safe(self.account_block_count);
        self.account_conf_frontier.serialize(writer);
        writer.write_u64_be_safe(self.account_conf_height);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{assert_deserializable, Message};
    use rsnano_core::BlockBuilder;

    #[test]
    fn serialize_blocks() {
        let original = Message::AscPullAck(AscPullAck {
            id: 7,
            pull_type: AscPullAckType::Blocks(BlocksAckPayload::new(vec![
                BlockBuilder::state().build(),
                BlockBuilder::state().build(),
            ])),
        });

        assert_deserializable(&original);
    }

    #[test]
    fn serialize_account_info() {
        let original = Message::AscPullAck(AscPullAck {
            id: 7,
            pull_type: AscPullAckType::AccountInfo(AccountInfoAckPayload {
                account: Account::from(1),
                account_open: BlockHash::from(2),
                account_head: BlockHash::from(3),
                account_block_count: 4,
                account_conf_frontier: BlockHash::from(5),
                account_conf_height: 6,
            }),
        });

        assert_deserializable(&original);
    }

    #[test]
    fn display() {
        let ack = Message::AscPullAck(AscPullAck {
            id: 7,
            pull_type: AscPullAckType::AccountInfo(AccountInfoAckPayload {
                account: Account::from(1),
                account_open: BlockHash::from(2),
                account_head: BlockHash::from(3),
                account_block_count: 4,
                account_conf_frontier: BlockHash::from(5),
                account_conf_height: 6,
            }),
        });
        assert_eq!(ack.to_string(), "\naccount public key:nano_1111111111111111111111111111111111111111111111111113b8661hfk account open:0000000000000000000000000000000000000000000000000000000000000002 account head:0000000000000000000000000000000000000000000000000000000000000003 block count:4 confirmation frontier:0000000000000000000000000000000000000000000000000000000000000005 confirmation height:6");
    }

    #[test]
    fn serialize_json_blocks_ack() {
        let serialized = serde_json::to_string_pretty(&Message::AscPullAck(
            AscPullAck::create_test_instance_blocks(),
        ))
        .unwrap();
        assert_eq!(
            serialized,
            r#"{
  "message_type": "asc_pull_ack",
  "id": 12345,
  "pull_type": "blocks",
  "blocks": [
    {
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
  ]
}"#
        );
    }

    #[test]
    fn serialize_json_accounts_ack() {
        let serialized = serde_json::to_string_pretty(&Message::AscPullAck(
            AscPullAck::create_test_instance_account(),
        ))
        .unwrap();
        assert_eq!(
            serialized,
            r#"{
  "message_type": "asc_pull_ack",
  "id": 12345,
  "pull_type": "account_info",
  "account": "nano_1111111111111111111111111111111111111111111111111113b8661hfk",
  "account_open": "0000000000000000000000000000000000000000000000000000000000000002",
  "account_head": "0000000000000000000000000000000000000000000000000000000000000003",
  "account_block_count": 4,
  "account_conf_frontier": "0000000000000000000000000000000000000000000000000000000000000005",
  "account_conf_height": 3
}"#
        );
    }
}

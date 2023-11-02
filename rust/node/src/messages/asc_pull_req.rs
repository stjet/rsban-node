use num_traits::FromPrimitive;
use rsnano_core::{
    utils::{Deserialize, MemoryStream, Stream, StreamExt},
    HashOrAccount,
};
use std::{any::Any, fmt::Display, mem::size_of};

use super::{Message, MessageHeader, MessageType, MessageVisitor, ProtocolInfo};

/**
 * Type of requested asc pull data
 * - blocks:
 * - account_info:
 */
#[repr(u8)]
#[derive(Clone, FromPrimitive)]
pub enum AscPullPayloadId {
    Invalid = 0x0,
    Blocks = 0x1,
    AccountInfo = 0x2,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum AscPullReqPayload {
    Invalid,
    Blocks(BlocksReqPayload),
    AccountInfo(AccountInfoReqPayload),
}

#[derive(FromPrimitive, PartialEq, Eq, Clone, Copy, Debug, Default)]
pub enum HashType {
    #[default]
    Account = 0,
    Block = 1,
}

impl HashType {
    fn deserialize(stream: &mut dyn Stream) -> anyhow::Result<Self> {
        FromPrimitive::from_u8(stream.read_u8()?).ok_or_else(|| anyhow!("target_type missing"))
    }
}

#[derive(Default, Clone, PartialEq, Eq, Debug)]
pub struct BlocksReqPayload {
    pub start: HashOrAccount,
    pub count: u8,
    pub start_type: HashType,
}

impl BlocksReqPayload {
    fn serialize(&self, stream: &mut dyn Stream) -> anyhow::Result<()> {
        stream.write_bytes(self.start.as_bytes())?;
        stream.write_u8(self.count)?;
        stream.write_u8(self.start_type as u8)?;
        Ok(())
    }

    fn deserialize(&mut self, stream: &mut dyn Stream) -> anyhow::Result<()> {
        self.start = HashOrAccount::deserialize(stream)?;
        self.count = stream.read_u8()?;
        self.start_type = HashType::deserialize(stream)?;
        Ok(())
    }
}

#[derive(Default, Clone, PartialEq, Eq, Debug)]
pub struct AccountInfoReqPayload {
    pub target: HashOrAccount,
    pub target_type: HashType,
}

impl AccountInfoReqPayload {
    fn serialize(&self, stream: &mut dyn Stream) -> anyhow::Result<()> {
        stream.write_bytes(self.target.as_bytes())?;
        stream.write_u8(self.target_type as u8)
    }

    fn deserialize(&mut self, stream: &mut dyn Stream) -> anyhow::Result<()> {
        self.target = HashOrAccount::deserialize(stream)?;
        self.target_type = HashType::deserialize(stream)?;
        Ok(())
    }

    pub fn create_test_instance() -> Self {
        Self {
            target: HashOrAccount::from(42),
            target_type: HashType::Account,
        }
    }
}

/// Ascending bootstrap pull request
#[derive(Clone)]
pub struct AscPullReq {
    header: MessageHeader,
    payload: AscPullReqPayload,
    pub id: u64,
}

impl AscPullReq {
    pub fn new(protocol_info: &ProtocolInfo) -> Self {
        Self {
            header: MessageHeader::new(MessageType::AscPullReq, protocol_info),
            payload: AscPullReqPayload::Invalid,
            id: 0,
        }
    }

    pub fn new_asc_pull_req_blocks(
        protocol_info: &ProtocolInfo,
        id: u64,
        payload: BlocksReqPayload,
    ) -> Self {
        let mut msg = Self {
            header: MessageHeader::new(MessageType::AscPullReq, protocol_info),
            payload: AscPullReqPayload::Blocks(payload),
            id,
        };
        msg.update_header().unwrap();
        msg
    }

    pub fn new_asc_pull_req_accounts(
        protocol_info: &ProtocolInfo,
        id: u64,
        payload: AccountInfoReqPayload,
    ) -> Self {
        let mut msg = Self {
            header: MessageHeader::new(MessageType::AscPullReq, protocol_info),
            payload: AscPullReqPayload::AccountInfo(payload),
            id,
        };
        msg.update_header().unwrap();
        msg
    }

    pub fn with_header(header: MessageHeader) -> Self {
        Self {
            header,
            payload: AscPullReqPayload::Invalid,
            id: 0,
        }
    }

    pub fn deserialize(stream: &mut impl Stream, header: MessageHeader) -> anyhow::Result<Self> {
        debug_assert!(header.message_type == MessageType::AscPullReq);
        let mut msg = Self::with_header(header);
        let pull_type = AscPullPayloadId::from_u8(stream.read_u8()?)
            .ok_or_else(|| anyhow!("Unknown asc_pull_type"))?;
        msg.id = stream.read_u64_be()?;

        msg.payload = match pull_type {
            AscPullPayloadId::Blocks => {
                let mut payload = BlocksReqPayload::default();
                payload.deserialize(stream)?;
                AscPullReqPayload::Blocks(payload)
            }
            AscPullPayloadId::AccountInfo => {
                let mut payload = AccountInfoReqPayload::default();
                payload.deserialize(stream)?;
                AscPullReqPayload::AccountInfo(payload)
            }
            AscPullPayloadId::Invalid => bail!("Unknown asc_pull_type"),
        };
        Ok(msg)
    }

    pub fn payload(&self) -> &AscPullReqPayload {
        &self.payload
    }

    pub fn serialized_size(header: &MessageHeader) -> usize {
        let payload_len = header.extensions.data as usize;
        Self::partial_size() + payload_len
    }

    /**
     * Update payload size stored in header
     * IMPORTANT: Must be called after any update to the payload
     */
    fn update_header(&mut self) -> anyhow::Result<()> {
        let mut stream = MemoryStream::new();
        self.serialize_payload(&mut stream)?;
        let payload_len: u16 = stream.as_bytes().len().try_into()?;
        self.header.extensions.data = payload_len;
        Ok(())
    }

    fn serialize_payload(&self, stream: &mut dyn Stream) -> anyhow::Result<()> {
        match &self.payload {
            AscPullReqPayload::Invalid => bail!("invalid payload"),
            AscPullReqPayload::Blocks(blocks) => blocks.serialize(stream),
            AscPullReqPayload::AccountInfo(account_info) => account_info.serialize(stream),
        }
    }

    pub fn payload_type(&self) -> AscPullPayloadId {
        match self.payload {
            AscPullReqPayload::Invalid => AscPullPayloadId::Invalid,
            AscPullReqPayload::Blocks(_) => AscPullPayloadId::Blocks,
            AscPullReqPayload::AccountInfo(_) => AscPullPayloadId::AccountInfo,
        }
    }

    /** Size of message without payload */
    const fn partial_size() -> usize {
        size_of::<u8>() // pull type
        + size_of::<u64>() // id
    }
}

impl Message for AscPullReq {
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

    fn serialize(&self, stream: &mut dyn Stream) -> anyhow::Result<()> {
        self.header.serialize(stream)?;
        stream.write_u8(self.payload_type() as u8)?;
        stream.write_u64_be(self.id)?;
        self.serialize_payload(stream)
    }

    fn visit(&self, visitor: &mut dyn MessageVisitor) {
        visitor.asc_pull_req(self);
    }

    fn clone_box(&self) -> Box<dyn Message> {
        Box::new(self.clone())
    }

    fn message_type(&self) -> MessageType {
        MessageType::AscPullReq
    }
}

impl Display for AscPullReq {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "{}", self.header)?;
        match self.payload() {
            AscPullReqPayload::Invalid => write!(f, "missing payload")?,
            AscPullReqPayload::Blocks(blocks) => {
                write!(
                    f,
                    "acc:{} max block count:{} hash type: {}",
                    blocks.start, blocks.count, blocks.start_type as u8
                )?;
            }
            AscPullReqPayload::AccountInfo(info) => {
                write!(
                    f,
                    "target:{} hash type:{}",
                    info.target, info.target_type as u8
                )?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rsnano_core::utils::MemoryStream;

    #[test]
    fn serialize_header() -> anyhow::Result<()> {
        let original = AscPullReq::new_asc_pull_req_blocks(
            &ProtocolInfo::dev_network(),
            7,
            BlocksReqPayload {
                start: HashOrAccount::from(3),
                count: 111,
                start_type: HashType::Block,
            },
        );

        let mut stream = MemoryStream::new();
        original.serialize(&mut stream)?;

        let header = MessageHeader::deserialize(&mut stream)?;
        assert_eq!(header.message_type, MessageType::AscPullReq);
        Ok(())
    }

    #[test]
    fn missing_payload() {
        let original = AscPullReq::new(&ProtocolInfo::dev_network());
        let mut stream = MemoryStream::new();
        let result = original.serialize(&mut stream);
        match result {
            Ok(_) => panic!("serialize should fail"),
            Err(e) => assert_eq!(e.to_string(), "invalid payload"),
        }
    }

    #[test]
    fn serialize_blocks() -> anyhow::Result<()> {
        let original = AscPullReq::new_asc_pull_req_blocks(
            &ProtocolInfo::dev_network(),
            7,
            BlocksReqPayload {
                start: HashOrAccount::from(3),
                count: 111,
                start_type: HashType::Block,
            },
        );

        let mut stream = MemoryStream::new();
        original.serialize(&mut stream)?;

        let header = MessageHeader::deserialize(&mut stream)?;
        let message_out = AscPullReq::deserialize(&mut stream, header)?;
        assert_eq!(message_out.id, original.id);
        assert_eq!(message_out.payload(), original.payload());
        assert!(stream.at_end());
        Ok(())
    }

    #[test]
    fn serialize_account_info() -> anyhow::Result<()> {
        let original = AscPullReq::new_asc_pull_req_accounts(
            &ProtocolInfo::dev_network(),
            7,
            AccountInfoReqPayload {
                target: HashOrAccount::from(123),
                target_type: HashType::Block,
            },
        );

        let mut stream = MemoryStream::new();
        original.serialize(&mut stream)?;

        let header = MessageHeader::deserialize(&mut stream)?;
        let message_out = AscPullReq::deserialize(&mut stream, header)?;
        assert_eq!(message_out.id, original.id);
        assert_eq!(message_out.payload(), original.payload());
        assert!(stream.at_end());
        Ok(())
    }

    #[test]
    fn display_blocks_payload() {
        let req = AscPullReq::new_asc_pull_req_blocks(
            &ProtocolInfo::dev_network(),
            7,
            BlocksReqPayload {
                start: 1.into(),
                count: 2,
                start_type: HashType::Block,
            },
        );
        assert_eq!(req.to_string(), "NetID: 5241(dev), VerMaxUsingMin: 19/19/18, MsgType: 14(asc_pull_req), Extensions: 0022\nacc:0000000000000000000000000000000000000000000000000000000000000001 max block count:2 hash type: 1");
    }

    #[test]
    fn display_invalid_payload() {
        let mut req = AscPullReq::new(&ProtocolInfo::dev_network());
        req.id = 7;
        assert_eq!(req.to_string(), "NetID: 5241(dev), VerMaxUsingMin: 19/19/18, MsgType: 14(asc_pull_req), Extensions: 0000\nmissing payload");
    }

    #[test]
    fn display_account_info_payload() {
        let req = AscPullReq::new_asc_pull_req_accounts(
            &ProtocolInfo::dev_network(),
            7,
            AccountInfoReqPayload {
                target: HashOrAccount::from(123),
                target_type: HashType::Block,
            },
        );
        assert_eq!(req.to_string(), "NetID: 5241(dev), VerMaxUsingMin: 19/19/18, MsgType: 14(asc_pull_req), Extensions: 0021\ntarget:000000000000000000000000000000000000000000000000000000000000007B hash type:1");
    }
}

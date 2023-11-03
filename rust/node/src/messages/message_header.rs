use anyhow::Result;
use bitvec::prelude::*;
use num_traits::FromPrimitive;
use rsnano_core::{
    serialized_block_size,
    utils::{MemoryStream, Serialize, Stream},
    BlockType, Networks,
};
use std::{
    fmt::{Debug, Display},
    mem::size_of,
};

use super::*;

/// Message types are serialized to the network and existing values must thus never change as
/// types are added, removed and reordered in the enum.
#[repr(u8)]
#[derive(FromPrimitive, Clone, Copy, PartialEq, Eq)]
pub enum MessageType {
    Invalid = 0x0,
    NotAType = 0x1,
    Keepalive = 0x2,
    Publish = 0x3,
    ConfirmReq = 0x4,
    ConfirmAck = 0x5,
    BulkPull = 0x6,
    BulkPush = 0x7,
    FrontierReq = 0x8,
    /* deleted 0x9 */
    NodeIdHandshake = 0x0a,
    BulkPullAccount = 0x0b,
    TelemetryReq = 0x0c,
    TelemetryAck = 0x0d,
    AscPullReq = 0x0e,
    AscPullAck = 0x0f,
}

impl MessageType {
    pub fn as_str(&self) -> &'static str {
        match self {
            MessageType::Invalid => "invalid",
            MessageType::NotAType => "not_a_type",
            MessageType::Keepalive => "keepalive",
            MessageType::Publish => "publish",
            MessageType::ConfirmReq => "confirm_req",
            MessageType::ConfirmAck => "confirm_ack",
            MessageType::BulkPull => "bulk_pull",
            MessageType::BulkPush => "bulk_push",
            MessageType::FrontierReq => "frontier_req",
            MessageType::NodeIdHandshake => "node_id_handshake",
            MessageType::BulkPullAccount => "bulk_pull_account",
            MessageType::TelemetryReq => "telemetry_req",
            MessageType::TelemetryAck => "telemetry_ack",
            MessageType::AscPullReq => "asc_pull_req",
            MessageType::AscPullAck => "asc_pull_ack",
        }
    }
}

impl Debug for MessageType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

const BLOCK_TYPE_MASK: u16 = 0x0f00;
const COUNT_MASK: u16 = 0xf000;

pub struct ProtocolInfo {
    pub version_using: u8,
    pub version_max: u8,
    pub version_min: u8,
    pub network: Networks,
}

impl Default for ProtocolInfo {
    fn default() -> Self {
        Self {
            version_using: 0x13,
            version_max: 0x13,
            version_min: 0x12,
            network: Networks::NanoLiveNetwork,
        }
    }
}

impl ProtocolInfo {
    pub fn dev_network() -> Self {
        Self {
            network: Networks::NanoDevNetwork,
            ..Default::default()
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct MessageHeader {
    pub message_type: MessageType,
    pub version_using: u8,
    pub version_max: u8,
    pub version_min: u8,
    pub network: Networks,
    pub extensions: BitArray<u16>,
}

impl Default for MessageHeader {
    fn default() -> Self {
        Self {
            message_type: MessageType::Invalid,
            version_using: 0,
            version_max: 0,
            version_min: 0,
            network: Networks::NanoDevNetwork,
            extensions: BitArray::ZERO,
        }
    }
}

impl MessageHeader {
    pub const SERIALIZED_SIZE: usize = 8;

    pub fn new(message_type: MessageType, protocol: &ProtocolInfo) -> Self {
        Self {
            message_type,
            version_using: protocol.version_using,
            version_max: protocol.version_max,
            version_min: protocol.version_min,
            network: protocol.network,
            ..Default::default()
        }
    }

    pub fn new_with_payload_len(
        message_type: MessageType,
        protocol: &ProtocolInfo,
        payload: &impl Serialize,
    ) -> Self {
        let mut stream = MemoryStream::new();
        payload.serialize(&mut stream).unwrap(); // can't fail
        let payload_len: u16 = stream.bytes_written() as u16;

        Self {
            message_type,
            version_using: protocol.version_using,
            version_max: protocol.version_max,
            version_min: protocol.version_min,
            network: protocol.network,
            extensions: payload_len.into(),
        }
    }

    pub fn deserialize(stream: &mut impl Stream) -> Result<MessageHeader> {
        let mut header = Self::default();
        let mut buffer = [0; 2];

        stream.read_bytes(&mut buffer, 2)?;
        header.network = Networks::from_u16(u16::from_be_bytes(buffer))
            .ok_or_else(|| anyhow!("invalid network"))?;

        header.version_max = stream.read_u8()?;
        header.version_using = stream.read_u8()?;
        header.version_min = stream.read_u8()?;
        header.message_type = MessageType::from_u8(stream.read_u8()?)
            .ok_or_else(|| anyhow!("invalid message type"))?;

        stream.read_bytes(&mut buffer, 2)?;
        header.extensions.data = u16::from_le_bytes(buffer);
        Ok(header)
    }

    pub fn set_extension(&mut self, position: usize, value: bool) {
        self.extensions.set(position, value);
    }

    pub fn set_flag(&mut self, flag: u8) {
        // Flags from 8 are block_type & count
        debug_assert!(flag < 8);
        self.extensions.set(flag as usize, true);
    }

    pub fn block_type(&self) -> BlockType {
        let mut value = self.extensions & BitArray::new(BLOCK_TYPE_MASK);
        value.shift_left(8);
        BlockType::from_u16(value.data).unwrap_or(BlockType::Invalid)
    }

    pub fn set_block_type(&mut self, block_type: BlockType) {
        self.extensions &= BitArray::new(!BLOCK_TYPE_MASK);
        self.extensions |= BitArray::new((block_type as u16) << 8);
    }

    pub fn count(&self) -> u8 {
        let mut value = self.extensions & BitArray::new(COUNT_MASK);
        value.shift_left(12);
        value.data as u8
    }

    pub fn set_count(&mut self, count: u8) {
        debug_assert!(count < 16);
        self.extensions &= BitArray::new(!COUNT_MASK);
        self.extensions |= BitArray::new((count as u16) << 12)
    }

    pub const fn serialized_size() -> usize {
        size_of::<u8>() // version_using
        + size_of::<u8>() // version_min
        + size_of::<u8>() // version_max
        + size_of::<Networks>()
        + size_of::<MessageType>()
        + size_of::<u16>() // extensions
    }

    pub fn serialize(&self, stream: &mut dyn Stream) -> Result<()> {
        stream.write_bytes(&(self.network as u16).to_be_bytes())?;
        stream.write_u8(self.version_max)?;
        stream.write_u8(self.version_using)?;
        stream.write_u8(self.version_min)?;
        stream.write_u8(self.message_type as u8)?;
        stream.write_bytes(&self.extensions.data.to_le_bytes())?;
        Ok(())
    }

    const BULK_PULL_COUNT_PRESENT_FLAG: usize = 0;

    pub fn bulk_pull_is_count_present(&self) -> bool {
        self.message_type == MessageType::BulkPull
            && self.extensions[Self::BULK_PULL_COUNT_PRESENT_FLAG]
    }

    pub fn payload_length(&self) -> usize {
        match self.message_type {
            MessageType::Keepalive => KeepalivePayload::serialized_size(),
            MessageType::Publish => serialized_block_size(self.block_type()),
            MessageType::ConfirmReq => {
                ConfirmReqPayload::serialized_size(self.block_type(), self.count())
            }
            MessageType::ConfirmAck => ConfirmAckPayload::serialized_size(self.count()),
            MessageType::BulkPull => BulkPullPayload::serialized_size(self),
            MessageType::BulkPush | MessageType::TelemetryReq => 0,
            MessageType::FrontierReq => FrontierReqPayload::serialized_size(),
            MessageType::NodeIdHandshake => NodeIdHandshakePayload::serialized_size(self),
            MessageType::BulkPullAccount => BulkPullAccountPayload::serialized_size(),
            MessageType::TelemetryAck => TelemetryAck::size_from_header(self),
            MessageType::AscPullReq => AscPullReqPayload::serialized_size(self),
            MessageType::AscPullAck => AscPullAckPayload::serialized_size(self),
            MessageType::Invalid | MessageType::NotAType => {
                debug_assert!(false);
                0
            }
        }
    }

    pub fn is_valid_message_type(&self) -> bool {
        !matches!(
            self.message_type,
            MessageType::Invalid | MessageType::NotAType
        )
    }
}

impl Display for MessageHeader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!(
            "NetID: {:04X}({}), ",
            self.network as u16,
            self.network.as_str()
        ))?;
        f.write_fmt(format_args!(
            "VerMaxUsingMin: {}/{}/{}, ",
            self.version_max, self.version_using, self.version_min
        ))?;
        f.write_fmt(format_args!(
            "MsgType: {}({}), ",
            self.message_type as u8,
            self.message_type.as_str()
        ))?;
        f.write_fmt(format_args!("Extensions: {:04X}", self.extensions.data))
    }
}

impl Debug for MessageHeader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self, f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rsnano_core::utils::MemoryStream;

    #[test]
    fn message_header_to_string() {
        assert_eq!(
            test_header().to_string(),
            "NetID: 5241(dev), VerMaxUsingMin: 3/2/1, MsgType: 2(keepalive), Extensions: 000E"
        );
    }

    #[test]
    fn serialize_and_deserialize() -> Result<()> {
        let original = test_header();
        let mut stream = MemoryStream::new();
        original.serialize(&mut stream)?;
        let deserialized = MessageHeader::deserialize(&mut stream)?;
        assert_eq!(original, deserialized);
        Ok(())
    }

    #[test]
    fn block_type() {
        let mut header = test_header();
        assert_eq!(header.block_type(), BlockType::Invalid);
        header.set_block_type(BlockType::LegacyReceive);
        assert_eq!(header.block_type(), BlockType::LegacyReceive);
    }

    fn test_header() -> MessageHeader {
        let header = MessageHeader {
            message_type: MessageType::Keepalive,
            version_using: 2,
            version_max: 3,
            version_min: 1,
            network: Networks::NanoDevNetwork,
            extensions: BitArray::from(14),
        };
        header
    }

    #[test]
    fn serialize_header() -> Result<()> {
        let protocol_info = ProtocolInfo::dev_network();
        let mut header = MessageHeader::new(MessageType::Publish, &protocol_info);
        header.set_block_type(BlockType::State);

        let mut stream = MemoryStream::new();
        header.serialize(&mut stream)?;

        let bytes = stream.as_bytes();
        assert_eq!(bytes.len(), 8);
        assert_eq!(bytes[0], 0x52);
        assert_eq!(bytes[1], 0x41);
        assert_eq!(bytes[2], protocol_info.version_using);
        assert_eq!(bytes[3], protocol_info.version_max);
        assert_eq!(bytes[4], protocol_info.version_min);
        assert_eq!(bytes[5], 0x03); // publish
        assert_eq!(bytes[6], 0x00); // extensions
        assert_eq!(bytes[7], 0x06); // state block
        Ok(())
    }
}

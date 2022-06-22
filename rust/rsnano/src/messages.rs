use crate::{utils::Stream, NetworkConstants, Networks};
use anyhow::Result;
use bitvec::prelude::*;
use num_traits::FromPrimitive;
use std::{
    fmt::{Debug, Display},
    mem::size_of,
};

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
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct MessageHeader {
    message_type: MessageType,
    version_using: u8,
    version_max: u8,
    version_min: u8,
    network: Networks,
    extensions: BitArray<u16>,
}

impl MessageHeader {
    pub fn new(constants: &NetworkConstants, message_type: MessageType) -> Self {
        let version_using = constants.protocol_version;
        Self::with_version_using(constants, message_type, version_using)
    }

    pub fn empty() -> Self {
        Self {
            message_type: MessageType::Invalid,
            version_using: 0,
            version_max: 0,
            version_min: 0,
            network: Networks::NanoDevNetwork,
            extensions: BitArray::ZERO,
        }
    }

    pub fn from_stream(stream: &mut impl Stream) -> Result<MessageHeader> {
        let mut header = Self::empty();
        header.deserialize(stream)?;
        Ok(header)
    }

    pub fn with_version_using(
        constants: &NetworkConstants,
        message_type: MessageType,
        version_using: u8,
    ) -> Self {
        Self {
            message_type,
            version_using,
            version_max: constants.protocol_version,
            version_min: constants.protocol_version_min,
            network: constants.current_network,
            extensions: BitArray::ZERO,
        }
    }

    pub fn version_using(&self) -> u8 {
        self.version_using
    }

    pub fn version_max(&self) -> u8 {
        self.version_max
    }

    pub fn version_min(&self) -> u8 {
        self.version_min
    }

    pub fn network(&self) -> Networks {
        self.network
    }

    pub fn message_type(&self) -> MessageType {
        self.message_type
    }

    pub fn extensions(&self) -> u16 {
        self.extensions.data
    }

    pub fn set_extensions(&mut self, value: u16) {
        self.extensions.data = value;
    }

    pub fn test_extension(&self, position: usize) -> bool {
        self.extensions[position]
    }

    pub fn set_extension(&mut self, position: usize, value: bool) {
        self.extensions.set(position, value);
    }

    pub fn size() -> usize {
        size_of::<u8>() // version_using
        + size_of::<u8>() // version_min
        + size_of::<u8>() // version_max
        + size_of::<Networks>()
        + size_of::<MessageType>()
        + size_of::<u16>() // extensions
    }

    pub fn deserialize(&mut self, stream: &mut dyn Stream) -> Result<()> {
        let mut buffer = [0; 2];

        stream.read_bytes(&mut buffer, 2)?;
        self.network = Networks::from_u16(u16::from_be_bytes(buffer))
            .ok_or_else(|| anyhow!("invalid network"))?;

        self.version_max = stream.read_u8()?;
        self.version_using = stream.read_u8()?;
        self.version_min = stream.read_u8()?;
        self.message_type = MessageType::from_u8(stream.read_u8()?)
            .ok_or_else(|| anyhow!("invalid message type"))?;

        stream.read_bytes(&mut buffer, 2)?;
        self.extensions.data = u16::from_ne_bytes(buffer);
        Ok(())
    }

    pub fn serialize(&self, stream: &mut dyn Stream) -> Result<()> {
        stream.write_bytes(&(self.network() as u16).to_be_bytes())?;
        stream.write_u8(self.version_max())?;
        stream.write_u8(self.version_using())?;
        stream.write_u8(self.version_min())?;
        stream.write_u8(self.message_type() as u8)?;
        stream.write_bytes(&self.extensions().to_ne_bytes())?;
        Ok(())
    }
}

impl Display for MessageHeader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!(
            "NetID: {:04X}({}), ",
            self.network() as u16,
            self.network().as_str()
        ))?;
        f.write_fmt(format_args!(
            "VerMaxUsingMin: {}/{}/{}, ",
            self.version_max(),
            self.version_using(),
            self.version_min()
        ))?;
        f.write_fmt(format_args!(
            "MsgType: {}({}), ",
            self.message_type() as u8,
            self.message_type().as_str()
        ))?;
        f.write_fmt(format_args!("Extensions: {:04X}", self.extensions()))
    }
}

impl Debug for MessageHeader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self, f)
    }
}

#[cfg(test)]
mod tests {
    use crate::utils::TestStream;

    use super::*;

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
        let mut stream = TestStream::new();
        original.serialize(&mut stream)?;
        let deserialized = MessageHeader::from_stream(&mut stream)?;
        assert_eq!(original, deserialized);
        Ok(())
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
}

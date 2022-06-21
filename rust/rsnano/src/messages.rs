use crate::{utils::Stream, NetworkConstants};
use anyhow::Result;
use std::sync::Arc;

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
    pub fn as_str(&self) -> &str {
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

#[derive(Clone)]
pub struct MessageHeader {
    constants: Arc<NetworkConstants>,
    message_type: MessageType,
    version_using: u8,
}

impl MessageHeader {
    pub fn new(constants: Arc<NetworkConstants>, message_type: MessageType) -> Self {
        let version_using = constants.protocol_version;
        Self::with_version_using(constants, message_type, version_using)
    }

    pub fn with_version_using(
        constants: Arc<NetworkConstants>,
        message_type: MessageType,
        version_using: u8,
    ) -> Self {
        Self {
            constants,
            message_type,
            version_using,
        }
    }

    pub fn version_using(&self) -> u8 {
        self.version_using
    }

    pub fn size() -> usize {
        std::mem::size_of::<u8>() // version_using
    }

    pub(crate) fn deserialize(&mut self, stream: &mut dyn Stream) -> Result<()> {
        self.version_using = stream.read_u8()?;
        Ok(())
    }
}

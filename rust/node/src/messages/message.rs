use super::*;
use crate::stats::DetailType;
use bitvec::prelude::BitArray;
use rsnano_core::utils::{BufferWriter, Serialize, StreamAdapter};
use std::fmt::Display;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Message {
    Keepalive(Keepalive),
    Publish(Publish),
    AscPullAck(AscPullAck),
    AscPullReq(AscPullReq),
    BulkPull(BulkPull),
    BulkPullAccount(BulkPullAccount),
    BulkPush,
    ConfirmAck(ConfirmAck),
    ConfirmReq(ConfirmReq),
    FrontierReq(FrontierReq),
    NodeIdHandshake(NodeIdHandshake),
    TelemetryAck(TelemetryAck),
    TelemetryReq,
}

pub trait MessageVariant: Display + Serialize {
    fn header_extensions(&self, _payload_len: u16) -> BitArray<u16> {
        Default::default()
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ParseMessageError {
    Other,
    InsufficientWork,
    InvalidHeader,
    InvalidMessageType,
    InvalidMessage(MessageType),
    InvalidNetwork,
    OutdatedVersion,
    DuplicatePublishMessage,
    MessageSizeTooBig,
}

#[derive(Clone, PartialEq, Eq)]
pub struct DeserializedMessage {
    pub message: Message,
    pub protocol: ProtocolInfo,
}

impl DeserializedMessage {
    pub fn new(message: Message, protocol: ProtocolInfo) -> Self {
        Self { message, protocol }
    }
}

impl Message {
    pub const MAX_MESSAGE_SIZE: usize = 1024 * 65;

    pub fn message_type(&self) -> MessageType {
        match &self {
            Message::Keepalive(_) => MessageType::Keepalive,
            Message::Publish(_) => MessageType::Publish,
            Message::AscPullAck(_) => MessageType::AscPullAck,
            Message::AscPullReq(_) => MessageType::AscPullReq,
            Message::BulkPull(_) => MessageType::BulkPull,
            Message::BulkPullAccount(_) => MessageType::BulkPullAccount,
            Message::BulkPush => MessageType::BulkPush,
            Message::ConfirmAck(_) => MessageType::ConfirmAck,
            Message::ConfirmReq(_) => MessageType::ConfirmReq,
            Message::FrontierReq(_) => MessageType::FrontierReq,
            Message::NodeIdHandshake(_) => MessageType::NodeIdHandshake,
            Message::TelemetryAck(_) => MessageType::TelemetryAck,
            Message::TelemetryReq => MessageType::TelemetryReq,
        }
    }

    pub fn as_message_variant(&self) -> Option<&dyn MessageVariant> {
        match &self {
            Message::Keepalive(x) => Some(x),
            Message::Publish(x) => Some(x),
            Message::AscPullAck(x) => Some(x),
            Message::AscPullReq(x) => Some(x),
            Message::BulkPull(x) => Some(x),
            Message::BulkPullAccount(x) => Some(x),
            Message::ConfirmAck(x) => Some(x),
            Message::ConfirmReq(x) => Some(x),
            Message::FrontierReq(x) => Some(x),
            Message::NodeIdHandshake(x) => Some(x),
            Message::TelemetryAck(x) => Some(x),
            _ => None,
        }
    }

    pub fn serialize(&self, stream: &mut dyn BufferWriter) {
        if let Some(variant) = self.as_message_variant() {
            variant.serialize(stream);
        }
    }

    pub fn header_extensions(&self, payload_len: u16) -> BitArray<u16> {
        match self.as_message_variant() {
            Some(variant) => variant.header_extensions(payload_len),
            None => Default::default(),
        }
    }

    pub fn deserialize(payload_bytes: &[u8], header: &MessageHeader, digest: u128) -> Option<Self> {
        let mut stream = StreamAdapter::new(payload_bytes);
        let msg = match header.message_type {
            MessageType::Keepalive => Message::Keepalive(Keepalive::deserialize(&mut stream)?),
            MessageType::Publish => Message::Publish(Publish::deserialize(
                &mut stream,
                header.extensions,
                digest,
            )?),
            MessageType::AscPullAck => Message::AscPullAck(AscPullAck::deserialize(&mut stream)?),
            MessageType::AscPullReq => Message::AscPullReq(AscPullReq::deserialize(&mut stream)?),
            MessageType::BulkPull => {
                Message::BulkPull(BulkPull::deserialize(&mut stream, header.extensions)?)
            }
            MessageType::BulkPullAccount => {
                Message::BulkPullAccount(BulkPullAccount::deserialize(&mut stream)?)
            }
            MessageType::BulkPush => Message::BulkPush,
            MessageType::ConfirmAck => Message::ConfirmAck(ConfirmAck::deserialize(&mut stream)?),
            MessageType::ConfirmReq => {
                Message::ConfirmReq(ConfirmReq::deserialize(&mut stream, header.extensions)?)
            }
            MessageType::FrontierReq => {
                Message::FrontierReq(FrontierReq::deserialize(&mut stream, header.extensions)?)
            }
            MessageType::NodeIdHandshake => Message::NodeIdHandshake(NodeIdHandshake::deserialize(
                &mut stream,
                header.extensions,
            )?),
            MessageType::TelemetryAck => {
                Message::TelemetryAck(TelemetryAck::deserialize(&mut stream, header.extensions)?)
            }
            MessageType::TelemetryReq => Message::TelemetryReq,
            MessageType::Invalid | MessageType::NotAType => return None,
        };

        Some(msg)
    }
}

impl From<&Message> for DetailType {
    fn from(value: &Message) -> Self {
        value.message_type().into()
    }
}

impl Display for Message {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.as_message_variant() {
            Some(variant) => variant.fmt(f),
            None => Ok(()),
        }
    }
}

pub fn validate_header(
    header: &MessageHeader,
    expected_protocol: &ProtocolInfo,
) -> Result<(), ParseMessageError> {
    if header.protocol.network != expected_protocol.network {
        Err(ParseMessageError::InvalidNetwork)
    } else if header.protocol.version_using < expected_protocol.version_min {
        Err(ParseMessageError::OutdatedVersion)
    } else if !header.is_valid_message_type() {
        Err(ParseMessageError::InvalidHeader)
    } else if header.payload_length() > Message::MAX_MESSAGE_SIZE {
        Err(ParseMessageError::MessageSizeTooBig)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::voting::Vote;
    use rsnano_core::BlockBuilder;

    #[test]
    fn exact_confirm_ack() {
        let message = Message::ConfirmAck(ConfirmAck {
            vote: Arc::new(Vote::create_test_instance()),
        });
        test_deserializer(&message);
    }

    #[test]
    fn exact_confirm_req() {
        let block = BlockBuilder::legacy_send().build();
        let message = Message::ConfirmReq(ConfirmReq {
            block: Some(block),
            roots_hashes: Vec::new(),
        });
        test_deserializer(&message);
    }

    #[test]
    fn exact_publish() {
        let block = BlockBuilder::legacy_send().build();
        let message = Message::Publish(Publish { block, digest: 8 });
        test_deserializer(&message);
    }

    #[test]
    fn exact_keepalive() {
        test_deserializer(&Message::Keepalive(Keepalive::default()));
    }

    #[test]
    fn exact_frontier_req() {
        let message = Message::FrontierReq(FrontierReq::create_test_instance());
        test_deserializer(&message);
    }

    #[test]
    fn exact_telemetry_req() {
        test_deserializer(&Message::TelemetryReq);
    }

    #[test]
    fn exact_telemetry_ack() {
        let mut data = TelemetryData::default();
        data.unknown_data.push(0xFF);
        test_deserializer(&Message::TelemetryAck(TelemetryAck(Some(data))));
    }

    #[test]
    fn exact_bulk_pull() {
        let message = Message::BulkPull(BulkPull::create_test_instance());
        test_deserializer(&message);
    }

    #[test]
    fn exact_bulk_pull_account() {
        let message = Message::BulkPullAccount(BulkPullAccount::create_test_instance());
        test_deserializer(&message);
    }

    #[test]
    fn exact_bulk_push() {
        test_deserializer(&Message::BulkPush);
    }

    #[test]
    fn exact_node_id_handshake() {
        let message = Message::NodeIdHandshake(NodeIdHandshake {
            query: Some(NodeIdHandshakeQuery { cookie: [1; 32] }),
            response: None,
            is_v2: true,
        });
        test_deserializer(&message);
    }

    #[test]
    fn exact_asc_pull_req() {
        let message = Message::AscPullReq(AscPullReq {
            req_type: AscPullReqType::AccountInfo(AccountInfoReqPayload::create_test_instance()),
            id: 7,
        });
        test_deserializer(&message);
    }

    #[test]
    fn exact_asc_pull_ack() {
        let message = Message::AscPullAck(AscPullAck {
            id: 7,
            pull_type: AscPullAckType::AccountInfo(AccountInfoAckPayload::create_test_instance()),
        });
        test_deserializer(&message);
    }

    fn test_deserializer(original: &Message) {
        assert_deserializable(original);
    }
}

use super::NetworkFilter;
use crate::{messages::*, voting::VoteUniquer};
use rsnano_core::{utils::StreamAdapter, work::WorkThresholds};
use std::sync::Arc;

pub const MAX_MESSAGE_SIZE: usize = 1024 * 65;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ParseStatus {
    None,
    Success,
    InsufficientWork,
    InvalidHeader,
    InvalidMessageType,
    InvalidKeepaliveMessage,
    InvalidPublishMessage,
    InvalidConfirmReqMessage,
    InvalidConfirmAckMessage,
    InvalidNodeIdHandshakeMessage,
    InvalidTelemetryReqMessage,
    InvalidTelemetryAckMessage,
    InvalidBulkPullMessage,
    InvalidBulkPullAccountMessage,
    InvalidFrontierReqMessage,
    InvalidAscPullReqMessage,
    InvalidAscPullAckMessage,
    InvalidNetwork,
    OutdatedVersion,
    DuplicatePublishMessage,
    MessageSizeTooBig,
}

impl ParseStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Success => "success",
            Self::InsufficientWork => "insufficient_work",
            Self::InvalidHeader => "invalid_header",
            Self::InvalidMessageType => "invalid_message_type",
            Self::InvalidKeepaliveMessage => "invalid_keepalive_message",
            Self::InvalidPublishMessage => "invalid_publish_message",
            Self::InvalidConfirmReqMessage => "invalid_confirm_req_message",
            Self::InvalidConfirmAckMessage => "invalid_confirm_ack_message",
            Self::InvalidNodeIdHandshakeMessage => "invalid_node_id_handshake_message",
            Self::InvalidTelemetryReqMessage => "invalid_telemetry_req_message",
            Self::InvalidTelemetryAckMessage => "invalid_telemetry_ack_message",
            Self::InvalidBulkPullMessage => "invalid_bulk_pull_message",
            Self::InvalidBulkPullAccountMessage => "invalid_bulk_pull_account_message",
            Self::InvalidFrontierReqMessage => "invalid_frontier_req_message",
            Self::InvalidAscPullReqMessage => "invalid_asc_pull_req_message",
            Self::InvalidAscPullAckMessage => "invalid_asc_pull_ack_message",
            Self::InvalidNetwork => "invalid_network",
            Self::OutdatedVersion => "outdated_version",
            Self::DuplicatePublishMessage => "duplicate_publish_message",
            Self::MessageSizeTooBig => "message_size_too_big",
        }
    }
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

pub fn validate_header(
    header: &MessageHeader,
    expected_protocol: &ProtocolInfo,
) -> Result<(), ParseStatus> {
    if header.protocol.network != expected_protocol.network {
        Err(ParseStatus::InvalidNetwork)
    } else if header.protocol.version_using < expected_protocol.version_min {
        Err(ParseStatus::OutdatedVersion)
    } else if !header.is_valid_message_type() {
        Err(ParseStatus::InvalidHeader)
    } else if header.payload_length() > MAX_MESSAGE_SIZE {
        Err(ParseStatus::MessageSizeTooBig)
    } else {
        Ok(())
    }
}

pub struct MessageDeserializerImpl {
    work_thresholds: WorkThresholds,
    publish_filter: Arc<NetworkFilter>,
}

impl MessageDeserializerImpl {
    pub fn new(work_thresholds: WorkThresholds, publish_filter: Arc<NetworkFilter>) -> Self {
        Self {
            work_thresholds,
            publish_filter,
        }
    }

    pub fn deserialize(
        &self,
        header: MessageHeader,
        payload_bytes: &[u8],
    ) -> Result<DeserializedMessage, ParseStatus> {
        let digest = self.filter_duplicate_publish_messages(header.message_type, payload_bytes)?;

        let mut stream = StreamAdapter::new(payload_bytes);
        let result = Message::deserialize(&mut stream, &header, digest)
            .map_err(|_| Self::get_error(header.message_type));

        self.validate_work(&result)?;
        result.map(|r| DeserializedMessage::new(r, header.protocol))
    }

    fn get_error(message_type: MessageType) -> ParseStatus {
        match message_type {
            MessageType::Invalid | MessageType::NotAType => ParseStatus::InvalidHeader,
            MessageType::Keepalive => ParseStatus::InvalidKeepaliveMessage,
            MessageType::Publish => ParseStatus::InvalidPublishMessage,
            MessageType::ConfirmReq => ParseStatus::InvalidConfirmReqMessage,
            MessageType::ConfirmAck => ParseStatus::InvalidConfirmAckMessage,
            MessageType::BulkPull => ParseStatus::InvalidBulkPullMessage,
            MessageType::BulkPush => ParseStatus::None,
            MessageType::FrontierReq => ParseStatus::InvalidFrontierReqMessage,
            MessageType::NodeIdHandshake => ParseStatus::InvalidNodeIdHandshakeMessage,
            MessageType::BulkPullAccount => ParseStatus::InvalidBulkPullAccountMessage,
            MessageType::TelemetryReq => ParseStatus::InvalidTelemetryReqMessage,
            MessageType::TelemetryAck => ParseStatus::InvalidTelemetryReqMessage,
            MessageType::AscPullReq => ParseStatus::InvalidAscPullReqMessage,
            MessageType::AscPullAck => ParseStatus::InvalidAscPullAckMessage,
        }
    }

    fn filter_duplicate_publish_messages(
        &self,
        message_type: MessageType,
        payload_bytes: &[u8],
    ) -> Result<u128, ParseStatus> {
        if message_type == MessageType::Publish {
            // Early filtering to not waste time deserializing duplicate blocks
            let (digest, existed) = self.publish_filter.apply(payload_bytes);
            if existed {
                Err(ParseStatus::DuplicatePublishMessage)
            } else {
                Ok(digest)
            }
        } else {
            Ok(0)
        }
    }

    fn validate_work(&self, result: &Result<Message, ParseStatus>) -> Result<(), ParseStatus> {
        let block = match result {
            Ok(Message::Publish(msg)) => Some(&msg.block),
            Ok(Message::ConfirmReq(msg)) => msg.block.as_ref(),
            _ => None,
        };

        if let Some(block) = block {
            if self.work_thresholds.validate_entry_block(block) {
                return Err(ParseStatus::InsufficientWork);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{config::STUB_NETWORK_CONSTANTS, voting::Vote};
    use rsnano_core::{work::WORK_THRESHOLDS_STUB, BlockBuilder};

    #[test]
    fn exact_confirm_ack() {
        let message = Message::ConfirmAck(ConfirmAck {
            vote: Arc::new(Vote::create_test_instance()),
        });
        test_deserializer(&message);
    }

    #[test]
    fn exact_confirm_req() {
        let block = Arc::new(BlockBuilder::legacy_send().build());
        let message = Message::ConfirmReq(ConfirmReq {
            block: Some(block),
            roots_hashes: Vec::new(),
        });
        test_deserializer(&message);
    }

    #[test]
    fn exact_publish() {
        let block = Arc::new(BlockBuilder::legacy_send().build());
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
        let network_filter = Arc::new(NetworkFilter::new(1));

        let deserializer = Arc::new(MessageDeserializerImpl::new(
            WORK_THRESHOLDS_STUB.clone(),
            network_filter,
        ));

        let mut serializer = MessageSerializer::new(STUB_NETWORK_CONSTANTS.protocol_info());
        let serialized = serializer.serialize(original);
        let mut stream = StreamAdapter::new(serialized);
        let deserialized_header = MessageHeader::deserialize(&mut stream).unwrap();
        assert_eq!(
            deserialized_header.payload_length(),
            serialized.len() - MessageHeader::SERIALIZED_SIZE
        );

        let deserialized = deserializer
            .deserialize(deserialized_header, stream.remaining())
            .unwrap();

        assert_eq!(deserialized.message, *original);
    }
}

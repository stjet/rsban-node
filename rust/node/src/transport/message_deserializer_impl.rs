use super::NetworkFilter;
use crate::{config::NetworkConstants, messages::*, utils::BlockUniquer, voting::VoteUniquer};
use rsnano_core::utils::{MemoryStream, StreamAdapter};
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

#[derive(Clone)]
pub struct DeserializedMessage {
    pub message: Payload,
    pub protocol: ProtocolInfo,
}

impl DeserializedMessage {
    pub fn new(message: Payload, protocol: ProtocolInfo) -> Self {
        Self { message, protocol }
    }

    pub fn into_enum(&self) -> MessageEnum {
        let mut header = MessageHeader::new(self.message.message_type(), self.protocol);
        let mut stream = MemoryStream::new();
        self.message.serialize(&mut stream).unwrap();
        header.extensions = self
            .message
            .header_extensions(stream.bytes_written() as u16);
        MessageEnum {
            header,
            payload: self.message.clone(),
        }
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
    network_constants: NetworkConstants,
    publish_filter: Arc<NetworkFilter>,
    block_uniquer: Arc<BlockUniquer>,
    vote_uniquer: Arc<VoteUniquer>,
}

impl MessageDeserializerImpl {
    pub fn new(
        network_constants: NetworkConstants,
        publish_filter: Arc<NetworkFilter>,
        block_uniquer: Arc<BlockUniquer>,
        vote_uniquer: Arc<VoteUniquer>,
    ) -> Self {
        Self {
            network_constants,
            publish_filter,
            block_uniquer,
            vote_uniquer,
        }
    }

    pub fn deserialize(
        &self,
        header: MessageHeader,
        payload_bytes: &[u8],
    ) -> Result<DeserializedMessage, ParseStatus> {
        let digest = self.filter_duplicate_publish_messages(header.message_type, payload_bytes)?;

        let mut stream = StreamAdapter::new(payload_bytes);
        let result = Payload::deserialize(
            &mut stream,
            &header,
            digest,
            Some(&self.block_uniquer),
            Some(&self.vote_uniquer),
        )
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

    fn validate_work(&self, result: &Result<Payload, ParseStatus>) -> Result<(), ParseStatus> {
        let block = match result {
            Ok(Payload::Publish(msg)) => Some(&msg.block),
            Ok(Payload::ConfirmReq(msg)) => msg.block.as_ref(),
            _ => None,
        };

        if let Some(block) = block {
            if self.network_constants.work.validate_entry_block(block) {
                return Err(ParseStatus::InsufficientWork);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::ops::Deref;

    use super::*;
    use crate::{config::STUB_NETWORK_CONSTANTS, voting::Vote};
    use rsnano_core::BlockBuilder;

    #[test]
    fn exact_confirm_ack() {
        let message = Payload::ConfirmAck(ConfirmAckPayload {
            vote: Arc::new(Vote::create_test_instance()),
        });
        test_deserializer(&message);
    }

    #[test]
    fn exact_confirm_req() {
        let block = Arc::new(BlockBuilder::legacy_send().build());
        let message = Payload::ConfirmReq(ConfirmReqPayload {
            block: Some(block),
            roots_hashes: Vec::new(),
        });
        test_deserializer(&message);
    }

    #[test]
    fn exact_publish() {
        let block = Arc::new(BlockBuilder::legacy_send().build());
        let message = Payload::Publish(PublishPayload { block, digest: 8 });
        test_deserializer(&message);
    }

    #[test]
    fn exact_keepalive() {
        test_deserializer(&Payload::Keepalive(KeepalivePayload::default()));
    }

    #[test]
    fn exact_frontier_req() {
        let message = Payload::FrontierReq(FrontierReqPayload::create_test_instance());
        test_deserializer(&message);
    }

    #[test]
    fn exact_telemetry_req() {
        test_deserializer(&Payload::TelemetryReq(TelemetryReqPayload {}));
    }

    #[test]
    fn exact_telemetry_ack() {
        let mut data = TelemetryData::default();
        data.unknown_data.push(0xFF);

        test_deserializer(&Payload::TelemetryAck(data));
    }

    #[test]
    fn exact_bulk_pull() {
        let message = Payload::BulkPull(BulkPullPayload::create_test_instance());
        test_deserializer(&message);
    }

    #[test]
    fn exact_bulk_pull_account() {
        let message = Payload::BulkPullAccount(BulkPullAccountPayload::create_test_instance());
        test_deserializer(&message);
    }

    #[test]
    fn exact_bulk_push() {
        test_deserializer(&Payload::BulkPush(BulkPushPayload {}));
    }

    #[test]
    fn exact_node_id_handshake() {
        let message = Payload::NodeIdHandshake(NodeIdHandshakePayload {
            query: Some(NodeIdHandshakeQuery { cookie: [1; 32] }),
            response: None,
            is_v2: true,
        });
        test_deserializer(&message);
    }

    #[test]
    fn exact_asc_pull_req() {
        let message = Payload::AscPullReq(AscPullReqPayload {
            req_type: AscPullReqType::AccountInfo(AccountInfoReqPayload::create_test_instance()),
            id: 7,
        });
        test_deserializer(&message);
    }

    #[test]
    fn exact_asc_pull_ack() {
        let message = Payload::AscPullAck(AscPullAckPayload {
            id: 7,
            pull_type: AscPullAckType::AccountInfo(AccountInfoAckPayload::create_test_instance()),
        });
        test_deserializer(&message);
    }

    fn test_deserializer(original: &Payload) {
        let network_filter = Arc::new(NetworkFilter::new(1));
        let block_uniquer = Arc::new(BlockUniquer::new());
        let vote_uniquer = Arc::new(VoteUniquer::new());

        let deserializer = Arc::new(MessageDeserializerImpl::new(
            STUB_NETWORK_CONSTANTS.clone(),
            network_filter,
            block_uniquer,
            vote_uniquer,
        ));

        let mut serializer = MessageSerializer::new(STUB_NETWORK_CONSTANTS.protocol_info());
        let (header, payload) = serializer.serialize(original.deref()).unwrap();
        let mut stream = StreamAdapter::new(header);
        let deserialized_header = MessageHeader::deserialize(&mut stream).unwrap();

        let deserialized = deserializer
            .deserialize(deserialized_header, payload)
            .unwrap();

        assert_eq!(deserialized.message, *original);
    }
}

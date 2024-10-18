use crate::message_collection::MessageCollection;
use rsnano_messages::MessageType;
use std::sync::{Arc, RwLock};

pub(crate) struct MessageTypeFilter {
    messages: Arc<RwLock<MessageCollection>>,
}

impl MessageTypeFilter {
    pub(crate) fn new(messages: Arc<RwLock<MessageCollection>>) -> Self {
        Self { messages }
    }

    pub fn available_types(&self) -> impl Iterator<Item = &MessageType> {
        AVALABLE_TYPES.iter()
    }
}

const AVALABLE_TYPES: [MessageType; 12] = [
    MessageType::Keepalive,
    MessageType::Publish,
    MessageType::ConfirmReq,
    MessageType::ConfirmAck,
    MessageType::BulkPull,
    MessageType::BulkPush,
    MessageType::FrontierReq,
    MessageType::BulkPullAccount,
    MessageType::TelemetryReq,
    MessageType::TelemetryAck,
    MessageType::AscPullReq,
    MessageType::AscPullAck,
];

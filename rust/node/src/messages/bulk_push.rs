use super::{MessageHeader, MessageVariant};
use crate::messages::MessageType;
use rsnano_core::utils::{Serialize, Stream};
use std::fmt::Display;

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct BulkPushPayload;

impl BulkPushPayload {
    pub fn deserialize(_stream: &mut impl Stream, header: &MessageHeader) -> anyhow::Result<Self> {
        debug_assert!(header.message_type == MessageType::BulkPush);
        Ok(Self {})
    }
}

impl Serialize for BulkPushPayload {
    fn serialize(&self, _stream: &mut dyn Stream) -> anyhow::Result<()> {
        Ok(())
    }
}

impl MessageVariant for BulkPushPayload {
    fn message_type(&self) -> MessageType {
        MessageType::BulkPush
    }
}

impl Display for BulkPushPayload {
    fn fmt(&self, _f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Ok(())
    }
}

use super::{Message, MessageHeader, MessageType, MessageVisitor, ProtocolInfo};
use anyhow::Result;
use rsnano_core::utils::Stream;
use std::any::Any;

#[derive(Clone)]
pub struct BulkPush {
    header: MessageHeader,
}

impl BulkPush {
    pub fn new(protocol_info: &ProtocolInfo) -> Self {
        Self {
            header: MessageHeader::new(MessageType::BulkPush, protocol_info),
        }
    }

    pub fn with_header(header: MessageHeader) -> Self {
        Self { header }
    }

    pub fn deserialize(&mut self, _stream: &mut impl Stream) -> Result<()> {
        debug_assert!(self.header.message_type == MessageType::BulkPush);
        Ok(())
    }
}

impl Message for BulkPush {
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

    fn serialize(&self, stream: &mut dyn Stream) -> Result<()> {
        self.header.serialize(stream)
    }

    fn visit(&self, visitor: &mut dyn MessageVisitor) {
        visitor.bulk_push(self)
    }

    fn clone_box(&self) -> Box<dyn Message> {
        Box::new(self.clone())
    }

    fn message_type(&self) -> MessageType {
        MessageType::BulkPush
    }
}

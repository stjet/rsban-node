use super::{Message, MessageHeader, ProtocolInfo};
use crate::transport::MAX_MESSAGE_SIZE;
use rsnano_core::utils::MutStreamAdapter;

pub struct MessageSerializer {
    protocol: ProtocolInfo,
    buffer: [u8; Self::BUFFER_SIZE],
}

impl MessageSerializer {
    const BUFFER_SIZE: usize = MessageHeader::SERIALIZED_SIZE + MAX_MESSAGE_SIZE;
    pub fn new(protocol: ProtocolInfo) -> Self {
        Self {
            protocol,
            buffer: [0; Self::BUFFER_SIZE],
        }
    }

    pub fn serialize(&'_ mut self, message: &Message) -> anyhow::Result<&'_ [u8]> {
        let payload_len;
        {
            let mut payload_stream =
                MutStreamAdapter::new(&mut self.buffer[MessageHeader::SERIALIZED_SIZE..]);
            message.serialize(&mut payload_stream)?;
            payload_len = payload_stream.bytes_written();

            let mut header_stream =
                MutStreamAdapter::new(&mut self.buffer[..MessageHeader::SERIALIZED_SIZE]);
            let mut header = MessageHeader::new(message.message_type(), self.protocol);
            header.extensions = message.header_extensions(payload_len as u16);
            header.serialize(&mut header_stream)?;
        }
        Ok(&self.buffer[..MessageHeader::SERIALIZED_SIZE + payload_len])
    }
}

impl Default for MessageSerializer {
    fn default() -> Self {
        Self::new(ProtocolInfo::default())
    }
}

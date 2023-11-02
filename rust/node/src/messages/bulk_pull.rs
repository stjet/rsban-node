use anyhow::Result;
use rsnano_core::{
    utils::{Deserialize, FixedSizeSerialize, Serialize, Stream},
    BlockHash, HashOrAccount,
};
use std::{any::Any, fmt::Display, mem::size_of};

use super::{Message, MessageHeader, MessageType, MessageVisitor, ProtocolInfo};

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct BulkPullPayload {
    pub start: HashOrAccount,
    pub end: BlockHash,
    pub count: u32,
    pub ascending: bool,
}

impl BulkPullPayload {
    const COUNT_PRESENT_FLAG: usize = 0;
    const ASCENDING_FLAG: usize = 1;
    pub const EXTENDED_PARAMETERS_SIZE: usize = 8;

    pub fn create_test_instance() -> BulkPullPayload {
        Self {
            start: 1.into(),
            end: 2.into(),
            count: 3,
            ascending: true,
        }
    }

    pub fn serialized_size(header: &MessageHeader) -> usize {
        HashOrAccount::serialized_size()
            + BlockHash::serialized_size()
            + (if header.extensions[BulkPullPayload::COUNT_PRESENT_FLAG] {
                BulkPullPayload::EXTENDED_PARAMETERS_SIZE
            } else {
                0
            })
    }

    pub fn deserialize(stream: &mut impl Stream, header: &MessageHeader) -> Result<Self> {
        debug_assert!(header.message_type == MessageType::BulkPull);
        let start = HashOrAccount::deserialize(stream)?;
        let end = BlockHash::deserialize(stream)?;

        let count = if header.extensions[BulkPullPayload::COUNT_PRESENT_FLAG] {
            let mut extended_parameters_buffers = [0u8; BulkPullPayload::EXTENDED_PARAMETERS_SIZE];
            const_assert!(size_of::<u32>() < (BulkPullPayload::EXTENDED_PARAMETERS_SIZE - 1)); // "count must fit within buffer")

            stream.read_bytes(
                &mut extended_parameters_buffers,
                BulkPullPayload::EXTENDED_PARAMETERS_SIZE,
            )?;
            if extended_parameters_buffers[0] != 0 {
                bail!("extended parameters front was not 0");
            } else {
                u32::from_le_bytes(extended_parameters_buffers[1..5].try_into().unwrap())
            }
        } else {
            0
        };

        let ascending = header.extensions[BulkPullPayload::ASCENDING_FLAG];

        Ok(BulkPullPayload {
            start,
            end,
            count,
            ascending,
        })
    }
}

impl Serialize for BulkPullPayload {
    fn serialize(&self, stream: &mut dyn Stream) -> Result<()> {
        self.start.serialize(stream)?;
        self.end.serialize(stream)?;

        if self.count > 0 {
            let mut count_buffer = [0u8; BulkPullPayload::EXTENDED_PARAMETERS_SIZE];
            const_assert!(size_of::<u32>() < (BulkPullPayload::EXTENDED_PARAMETERS_SIZE - 1)); // count must fit within buffer

            count_buffer[1..5].copy_from_slice(&self.count.to_le_bytes());
            stream.write_bytes(&count_buffer)?;
        }
        Ok(())
    }
}

impl Display for BulkPullPayload {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "\nstart={} end={} cnt={}",
            self.start, self.end, self.count
        )
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct BulkPull {
    header: MessageHeader,
    pub payload: BulkPullPayload,
}

impl BulkPull {
    pub fn new_bulk_pull(protocol_info: &ProtocolInfo, payload: BulkPullPayload) -> Self {
        let mut header = MessageHeader::new(MessageType::BulkPull, protocol_info);
        header
            .extensions
            .set(BulkPullPayload::COUNT_PRESENT_FLAG, payload.count > 0);
        header
            .extensions
            .set(BulkPullPayload::ASCENDING_FLAG, payload.ascending);
        Self { header, payload }
    }

    pub fn deserialize(stream: &mut impl Stream, header: MessageHeader) -> Result<Self> {
        let payload = BulkPullPayload::deserialize(stream, &header)?;
        Ok(BulkPull { header, payload })
    }

    fn clone_box(&self) -> Box<dyn Message> {
        Box::new(self.clone())
    }
}

impl Message for BulkPull {
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
        self.header.serialize(stream)?;
        self.payload.serialize(stream)
    }

    fn visit(&self, visitor: &mut dyn MessageVisitor) {
        visitor.bulk_pull(self)
    }

    fn clone_box(&self) -> Box<dyn Message> {
        Box::new(self.clone())
    }

    fn message_type(&self) -> MessageType {
        MessageType::BulkPull
    }
}

impl Display for BulkPull {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.header.fmt(f)?;
        self.payload.fmt(f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rsnano_core::utils::MemoryStream;

    #[test]
    fn bulk_pull_serialization() -> Result<()> {
        let message_in = BulkPull::new_bulk_pull(
            &ProtocolInfo::dev_network(),
            BulkPullPayload::create_test_instance(),
        );
        let mut stream = MemoryStream::new();
        message_in.serialize(&mut stream)?;
        let header = MessageHeader::deserialize(&mut stream)?;
        let message_out = BulkPull::deserialize(&mut stream, header)?;
        assert_eq!(message_in, message_out);
        assert!(message_out.payload.ascending);
        Ok(())
    }
}

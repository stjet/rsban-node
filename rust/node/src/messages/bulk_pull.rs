use super::{MessageHeader, MessageType};
use anyhow::Result;
use rsnano_core::{
    utils::{Deserialize, FixedSizeSerialize, Serialize, Stream},
    BlockHash, HashOrAccount,
};
use std::{fmt::Display, mem::size_of};

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct BulkPullPayload {
    pub start: HashOrAccount,
    pub end: BlockHash,
    pub count: u32,
    pub ascending: bool,
}

impl BulkPullPayload {
    pub const COUNT_PRESENT_FLAG: usize = 0;
    pub const ASCENDING_FLAG: usize = 1;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::messages::{assert_deserializable, MessageEnum, ProtocolInfo};

    #[test]
    fn bulk_pull_serialization() {
        let message_in = MessageEnum::new_bulk_pull(
            &ProtocolInfo::dev_network(),
            BulkPullPayload::create_test_instance(),
        );
        assert_deserializable(&message_in);
    }
}

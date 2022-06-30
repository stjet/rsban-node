use crate::{utils::Stream, BlockHash, HashOrAccount, NetworkConstants};
use anyhow::Result;
use std::{any::Any, mem::size_of};

use super::{Message, MessageHeader, MessageType};

#[derive(Clone)]
pub struct BulkPull {
    header: MessageHeader,
    pub start: HashOrAccount,
    pub end: BlockHash,
    pub count: u32,
}

impl BulkPull {
    pub fn new(constants: &NetworkConstants) -> Self {
        Self {
            header: MessageHeader::new(constants, MessageType::BulkPull),
            start: HashOrAccount::new(),
            end: BlockHash::new(),
            count: 0,
        }
    }
    pub fn with_header(header: &MessageHeader) -> Self {
        Self {
            header: header.clone(),
            start: HashOrAccount::new(),
            end: BlockHash::new(),
            count: 0,
        }
    }

    fn clone_box(&self) -> Box<dyn Message> {
        Box::new(self.clone())
    }

    pub fn serialized_size(header: &MessageHeader) -> usize {
        HashOrAccount::serialized_size()
            + BlockHash::serialized_size()
            + (if BulkPull::is_count_present_in_header(header) {
                BulkPull::EXTENDED_PARAMETERS_SIZE
            } else {
                0
            })
    }

    const COUNT_PRESENT_FLAG: usize = 0;
    pub const EXTENDED_PARAMETERS_SIZE: usize = 8;

    pub fn is_count_present(&self) -> bool {
        Self::is_count_present_in_header(&self.header)
    }

    pub fn is_count_present_in_header(header: &MessageHeader) -> bool {
        header.test_extension(Self::COUNT_PRESENT_FLAG)
    }

    pub fn set_count_present(&mut self, present: bool) {
        self.header.set_extension(Self::COUNT_PRESENT_FLAG, present);
    }

    pub fn deserialize(&mut self, stream: &mut impl Stream) -> Result<()> {
        debug_assert!(self.header.message_type() == MessageType::BulkPull);

        self.start = HashOrAccount::deserialize(stream)?;
        self.end = BlockHash::deserialize(stream)?;

        if self.is_count_present() {
            let mut extended_parameters_buffers = [0u8; Self::EXTENDED_PARAMETERS_SIZE];
            const_assert!(size_of::<u32>() < (BulkPull::EXTENDED_PARAMETERS_SIZE - 1)); // "count must fit within buffer")

            stream.read_bytes(
                &mut extended_parameters_buffers,
                Self::EXTENDED_PARAMETERS_SIZE,
            )?;
            if extended_parameters_buffers[0] != 0 {
                bail!("extended parameters front was not 0");
            } else {
                self.count =
                    u32::from_le_bytes(extended_parameters_buffers[1..5].try_into().unwrap());
            }
        } else {
            self.count = 0;
        }
        Ok(())
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
        // Ensure the "count_present" flag is set if there
        // is a limit specifed.  Additionally, do not allow
        // the "count_present" flag with a value of 0, since
        // that is a sentinel which we use to mean "all blocks"
        // and that is the behavior of not having the flag set
        // so it is wasteful to do this.
        debug_assert!(
            (self.count == 0 && !self.is_count_present())
                || (self.count != 0 && self.is_count_present())
        );

        self.header.serialize(stream)?;
        self.start.serialize(stream)?;
        self.end.serialize(stream)?;

        if self.is_count_present() {
            let mut count_buffer = [0u8; Self::EXTENDED_PARAMETERS_SIZE];
            const_assert!(size_of::<u32>() < (BulkPull::EXTENDED_PARAMETERS_SIZE - 1)); // count must fit within buffer

            count_buffer[1..5].copy_from_slice(&self.count.to_le_bytes());
            stream.write_bytes(&count_buffer)?;
        }
        Ok(())
    }
}

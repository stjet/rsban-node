use crate::{utils::Stream, Account, NetworkConstants};
use anyhow::Result;
use std::{any::Any, mem::size_of};

use super::{Message, MessageHeader, MessageType};

#[derive(Clone)]
pub struct FrontierReq {
    header: MessageHeader,
    pub start: Account,
    pub age: u32,
    pub count: u32,
}

impl FrontierReq {
    pub fn new(constants: &NetworkConstants) -> Self {
        Self {
            header: MessageHeader::new(constants, MessageType::FrontierReq),
            start: *Account::zero(),
            age: 0,
            count: 0,
        }
    }

    pub fn with_header(header: &MessageHeader) -> Self {
        Self {
            header: header.clone(),
            start: *Account::zero(),
            age: 0,
            count: 0,
        }
    }

    pub fn serialized_size() -> usize {
        Account::serialized_size() 
        + size_of::<u32>() // age
        + size_of::<u32>() //count
    }

    pub fn deserialize(&mut self, stream: &mut impl Stream) -> Result<()> {
        debug_assert!(self.header.message_type() == MessageType::FrontierReq);
        self.start = Account::deserialize(stream)?;
        let mut buffer = [0u8; 4];
        stream.read_bytes(&mut buffer, 4)?;
        self.age = u32::from_le_bytes(buffer);
        stream.read_bytes(&mut buffer, 4)?;
        self.count = u32::from_le_bytes(buffer);
        Ok(())
    }
}

impl Message for FrontierReq {
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
}

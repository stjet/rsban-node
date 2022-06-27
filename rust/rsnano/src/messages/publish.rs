use crate::{deserialize_block, utils::Stream, BlockEnum, BlockUniquer, NetworkConstants};
use anyhow::Result;
use std::{
    any::Any,
    sync::{Arc, RwLock},
};

use super::{Message, MessageHeader, MessageType};

#[derive(Clone)]
pub struct Publish {
    header: MessageHeader,
    pub block: Option<Arc<RwLock<BlockEnum>>>, //todo remove Option
    pub digest: u128,
}

impl Publish {
    pub fn new(constants: &NetworkConstants, block: Arc<RwLock<BlockEnum>>) -> Self {
        let mut header = MessageHeader::new(constants, MessageType::Publish);
        header.set_block_type(block.read().unwrap().block_type());

        Self {
            header,
            block: Some(block),
            digest: 0,
        }
    }
    pub fn with_header(header: &MessageHeader, digest: u128) -> Self {
        Self {
            header: header.clone(),
            block: None,
            digest,
        }
    }

    pub fn serialize(&self, stream: &mut impl Stream) -> Result<()> {
        self.header().serialize(stream)?;
        let block = self.block.as_ref().ok_or_else(|| anyhow!("no block"))?;
        let lck = block.read().unwrap();
        lck.as_block().serialize(stream)
    }

    pub fn deserialize(
        &mut self,
        stream: &mut impl Stream,
        uniquer: Option<&BlockUniquer>,
    ) -> Result<()> {
        debug_assert!(self.header.message_type() == MessageType::Publish);
        self.block = Some(deserialize_block(
            self.header.block_type(),
            stream,
            uniquer,
        )?);
        Ok(())
    }
}

impl Message for Publish {
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

use crate::{
    messages::MessageType,
    utils::Stream,
    voting::{Vote, VoteUniquer},
    BlockType, NetworkConstants,
};
use anyhow::Result;
use std::{
    any::Any,
    sync::{Arc, RwLock},
};

use super::{Message, MessageHeader};

#[derive(Clone)]
pub struct ConfirmAck {
    header: MessageHeader,
    vote: Option<Arc<RwLock<Vote>>>,
}

impl ConfirmAck {
    pub fn new(constants: &NetworkConstants, vote: Arc<RwLock<Vote>>) -> Self {
        let mut header = MessageHeader::new(constants, MessageType::ConfirmAck);
        header.set_block_type(BlockType::NotABlock);
        let vote_lk = vote.read().unwrap();
        debug_assert!(vote_lk.hashes.len() < 16);
        header.set_count(vote_lk.hashes.len() as u8);
        drop(vote_lk);

        Self {
            header,
            vote: Some(vote),
        }
    }
    pub fn with_header(
        header: &MessageHeader,
        stream: &mut impl Stream,
        uniquer: Option<&VoteUniquer>,
    ) -> Result<Self> {
        let mut vote = Vote::null();
        vote.deserialize(stream)?;
        let mut vote = Arc::new(RwLock::new(vote));

        if let Some(uniquer) = uniquer {
            vote = uniquer.unique(&vote);
        }

        Ok(Self {
            header: header.clone(),
            vote: Some(vote),
        })
    }

    pub fn vote(&self) -> Option<&Arc<RwLock<Vote>>> {
        self.vote.as_ref()
    }

    pub fn serialized_size(count: usize) -> usize {
        Vote::serialized_size(count)
    }

    pub fn serialize(&self, stream: &mut impl Stream) -> Result<()> {
        debug_assert!(
            self.header().block_type() == BlockType::NotABlock
                || self.header().block_type() == BlockType::Send
                || self.header().block_type() == BlockType::Receive
                || self.header().block_type() == BlockType::Open
                || self.header.block_type() == BlockType::Change
                || self.header.block_type() == BlockType::State
        );
        self.header().serialize(stream)?;
        self.vote().unwrap().read().unwrap().serialize(stream)
    }
}

impl Message for ConfirmAck {
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

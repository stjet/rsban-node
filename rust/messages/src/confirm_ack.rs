use bitvec::prelude::BitArray;
use rsnano_core::{
    utils::{BufferWriter, Serialize, Stream},
    BlockType, Vote,
};
use std::{
    fmt::{Debug, Display},
    sync::Arc,
};

use super::{ConfirmReq, MessageVariant};

#[derive(Clone, Debug)]
pub struct ConfirmAck {
    pub vote: Arc<Vote>,
}

impl ConfirmAck {
    pub const HASHES_MAX: usize = 12;

    pub fn serialized_size(extensions: BitArray<u16>) -> usize {
        let count = ConfirmReq::count(extensions);
        Vote::serialized_size(count as usize)
    }

    pub fn deserialize(stream: &mut impl Stream) -> Option<Self> {
        let mut vote = Vote::null();
        vote.deserialize(stream).ok()?;
        let vote = Arc::new(vote);
        Some(ConfirmAck { vote })
    }

    pub fn create_test_instance() -> Self {
        Self {
            vote: Arc::new(Vote::create_test_instance()),
        }
    }
}

impl Serialize for ConfirmAck {
    fn serialize(&self, writer: &mut dyn BufferWriter) {
        self.vote.serialize(writer);
    }
}

impl serde::Serialize for ConfirmAck {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        todo!()
    }
}

impl MessageVariant for ConfirmAck {
    fn header_extensions(&self, _payload_len: u16) -> BitArray<u16> {
        let mut extensions = BitArray::default();
        extensions |= BitArray::new((BlockType::NotABlock as u16) << 8);
        debug_assert!(self.vote.hashes.len() < 16);
        extensions |= BitArray::new((self.vote.hashes.len() as u16) << 12);
        extensions
    }
}

impl PartialEq for ConfirmAck {
    fn eq(&self, other: &Self) -> bool {
        *self.vote == *other.vote
    }
}

impl Eq for ConfirmAck {}

impl Display for ConfirmAck {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "\n{}", self.vote.to_json().map_err(|_| std::fmt::Error)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{assert_deserializable, Message};
    use rsnano_core::{BlockHash, KeyPair};

    #[test]
    fn serialize() {
        let keys = KeyPair::new();
        let mut hashes = Vec::new();
        for i in 0..ConfirmAck::HASHES_MAX {
            hashes.push(BlockHash::from(i as u64))
        }
        let vote = Vote::new(keys.public_key().into(), &keys.private_key(), 0, 0, hashes);
        let vote = Arc::new(vote);
        let confirm = Message::ConfirmAck(ConfirmAck { vote });

        assert_deserializable(&confirm);
    }
}

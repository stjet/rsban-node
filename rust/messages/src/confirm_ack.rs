use super::{ConfirmReq, MessageVariant};
use bitvec::prelude::BitArray;
use rsnano_core::{
    utils::{BufferWriter, Serialize, Stream},
    Vote,
};
use std::fmt::{Debug, Display};

#[derive(Clone, Debug, serde::Serialize)]
pub struct ConfirmAck {
    vote: Vote,
}

impl ConfirmAck {
    pub const HASHES_MAX: usize = 255;

    pub fn new(vote: Vote) -> Self {
        assert!(vote.hashes.len() <= Self::HASHES_MAX);
        Self { vote }
    }

    pub fn vote(&self) -> &Vote {
        &self.vote
    }

    pub fn serialized_size(extensions: BitArray<u16>) -> usize {
        let count = ConfirmReq::count(extensions);
        Vote::serialized_size(count as usize)
    }

    pub fn deserialize(stream: &mut impl Stream) -> Option<Self> {
        let mut vote = Vote::null();
        vote.deserialize(stream).ok()?;
        Some(ConfirmAck::new(vote))
    }

    pub fn create_test_instance() -> Self {
        Self::new(Vote::create_test_instance())
    }
}

impl Serialize for ConfirmAck {
    fn serialize(&self, writer: &mut dyn BufferWriter) {
        self.vote.serialize(writer);
    }
}

impl MessageVariant for ConfirmAck {
    fn header_extensions(&self, _payload_len: u16) -> BitArray<u16> {
        let mut extensions = BitArray::default();
        extensions |= ConfirmReq::count_bits(self.vote.hashes.len() as u8);
        extensions
    }
}

impl PartialEq for ConfirmAck {
    fn eq(&self, other: &Self) -> bool {
        self.vote == other.vote
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
    fn serialize_v1() {
        let keys = KeyPair::new();
        let hashes = vec![BlockHash::from(1)];
        let vote = Vote::new(keys.public_key().into(), &keys.private_key(), 0, 0, hashes);
        let confirm = Message::ConfirmAck(ConfirmAck::new(vote));

        assert_deserializable(&confirm);
    }

    #[test]
    fn serialize_v2() {
        let keys = KeyPair::new();
        let mut hashes = Vec::new();
        for i in 0..ConfirmAck::HASHES_MAX {
            hashes.push(BlockHash::from(i as u64))
        }
        let vote = Vote::new(keys.public_key().into(), &keys.private_key(), 0, 0, hashes);
        let confirm = Message::ConfirmAck(ConfirmAck::new(vote));

        assert_deserializable(&confirm);
    }

    #[test]
    #[should_panic]
    fn panics_when_vote_contains_too_many_hashes() {
        let keys = KeyPair::new();
        let hashes = vec![BlockHash::from(1); 256];
        let vote = Vote::new(keys.public_key().into(), &keys.private_key(), 0, 0, hashes);
        Message::ConfirmAck(ConfirmAck::new(vote));
    }
}

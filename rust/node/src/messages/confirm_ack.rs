use crate::voting::{Vote, VoteUniquer};
use anyhow::Result;
use rsnano_core::utils::{Serialize, Stream};
use std::{
    fmt::{Debug, Display},
    sync::Arc,
};

#[derive(Clone, Debug)]
pub struct ConfirmAckPayload {
    pub vote: Arc<Vote>,
}

impl ConfirmAckPayload {
    pub const HASHES_MAX: usize = 12;

    pub fn serialized_size(count: u8) -> usize {
        Vote::serialized_size(count as usize)
    }

    pub fn deserialize(stream: &mut impl Stream, uniquer: Option<&VoteUniquer>) -> Result<Self> {
        let mut vote = Vote::null();
        vote.deserialize(stream)?;
        let mut vote = Arc::new(vote);

        if let Some(uniquer) = uniquer {
            vote = uniquer.unique(&vote);
        }

        Ok(ConfirmAckPayload { vote })
    }

    pub fn create_test_instance() -> Self {
        Self {
            vote: Arc::new(Vote::create_test_instance()),
        }
    }
}

impl Serialize for ConfirmAckPayload {
    fn serialize(&self, stream: &mut dyn Stream) -> anyhow::Result<()> {
        self.vote.serialize(stream)
    }
}

impl PartialEq for ConfirmAckPayload {
    fn eq(&self, other: &Self) -> bool {
        *self.vote == *other.vote
    }
}

impl Eq for ConfirmAckPayload {}

impl Display for ConfirmAckPayload {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "\n{}", self.vote.to_json().map_err(|_| std::fmt::Error)?)
    }
}

#[cfg(test)]
mod tests {
    use crate::messages::{assert_deserializable, MessageEnum};

    use super::*;
    use rsnano_core::{BlockHash, KeyPair};

    #[test]
    fn serialize() {
        let keys = KeyPair::new();
        let mut hashes = Vec::new();
        for i in 0..ConfirmAckPayload::HASHES_MAX {
            hashes.push(BlockHash::from(i as u64))
        }
        let vote = Vote::new(keys.public_key().into(), &keys.private_key(), 0, 0, hashes);
        let vote = Arc::new(vote);
        let confirm1 = MessageEnum::new_confirm_ack(&Default::default(), vote);

        assert_deserializable(&confirm1);
    }
}

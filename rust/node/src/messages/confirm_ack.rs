use crate::voting::{Vote, VoteUniquer};
use anyhow::Result;
use rsnano_core::utils::{Serialize, Stream};
use std::{
    fmt::{Debug, Display},
    ops::Deref,
    sync::Arc,
};

#[derive(Clone)]
pub struct ConfirmAckPayload {
    pub vote: Option<Arc<Vote>>,
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

        Ok(ConfirmAckPayload { vote: Some(vote) })
    }

    pub fn create_test_instance() -> Self {
        let vote = Arc::new(Vote::create_test_instance());
        Self { vote: Some(vote) }
    }
}

impl Serialize for ConfirmAckPayload {
    fn serialize(&self, stream: &mut dyn Stream) -> anyhow::Result<()> {
        self.vote.as_ref().unwrap().serialize(stream)
    }
}

impl PartialEq for ConfirmAckPayload {
    fn eq(&self, other: &Self) -> bool {
        if let Some(v1) = &self.vote {
            if let Some(v2) = &other.vote {
                return v1.deref() == v2.deref();
            }
        }
        false
    }
}

impl Eq for ConfirmAckPayload {}

impl Debug for ConfirmAckPayload {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut builder = f.debug_struct("ConfirmAckPayload");
        match &self.vote {
            Some(v) => {
                builder.field("vote", v.deref());
            }
            None => {
                builder.field("vote", &"None");
            }
        };
        builder.finish()
    }
}

impl Display for ConfirmAckPayload {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(vote) = &self.vote {
            write!(f, "\n{}", vote.to_json().map_err(|_| std::fmt::Error)?)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::messages::{Message, MessageEnum, MessageHeader};

    use super::*;
    use rsnano_core::{utils::MemoryStream, BlockHash, KeyPair};

    #[test]
    fn serialize() -> Result<()> {
        let keys = KeyPair::new();
        let mut hashes = Vec::new();
        for i in 0..ConfirmAckPayload::HASHES_MAX {
            hashes.push(BlockHash::from(i as u64))
        }
        let vote = Vote::new(keys.public_key().into(), &keys.private_key(), 0, 0, hashes);
        let vote = Arc::new(vote);
        let confirm1 = MessageEnum::new_confirm_ack(&Default::default(), vote);

        let mut stream = MemoryStream::new();
        confirm1.serialize(&mut stream)?;
        let header = MessageHeader::deserialize(&mut stream)?;
        let confirm2 = MessageEnum::deserialize(&mut stream, header, 0, None, None)?;
        assert_eq!(confirm1, confirm2);
        Ok(())
    }
}

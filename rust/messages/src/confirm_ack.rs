use super::{ConfirmReq, MessageVariant};
use bitvec::prelude::BitArray;
use rsnano_core::{
    utils::{BufferWriter, Serialize, Stream},
    BlockType, Vote,
};
use std::fmt::{Debug, Display};

#[derive(Clone, Debug, serde::Serialize)]
pub struct ConfirmAck {
    pub vote: Vote,
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
        Some(ConfirmAck { vote })
    }

    pub fn create_test_instance() -> Self {
        Self {
            vote: Vote::create_test_instance(),
        }
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
        extensions |= BitArray::new((BlockType::NotABlock as u16) << 8);
        debug_assert!(self.vote.hashes.len() < 16);
        extensions |= BitArray::new((self.vote.hashes.len() as u16) << 12);
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
    fn serialize() {
        let keys = KeyPair::new();
        let mut hashes = Vec::new();
        for i in 0..ConfirmAck::HASHES_MAX {
            hashes.push(BlockHash::from(i as u64))
        }
        let vote = Vote::new(keys.public_key().into(), &keys.private_key(), 0, 0, hashes);
        let confirm = Message::ConfirmAck(ConfirmAck { vote });

        assert_deserializable(&confirm);
    }

    #[test]
    fn serialize_json() {
        let serialized =
            serde_json::to_string_pretty(&Message::ConfirmAck(ConfirmAck::create_test_instance()))
                .unwrap();

        assert_eq!(
            serialized,
            r#"{
  "message_type": "confirm_ack",
  "vote": {
    "timestamp": 2,
    "voting_account": "nano_39y535msmkzb31bx73tdnf8iken5ucw9jt98re7nriduus6cgs6uonjdm8r5",
    "signature": "853CDAC926890F7447AD1CF8131867120878489AC13DB1445E6F5A120A3DDA42D2BB69B9294815DB086327CF8CA5FC66A9E9E707B382522E62C34A3042205F0A",
    "hashes": [
      "0000000000000000000000000000000000000000000000000000000000000005"
    ]
  }
}"#
        );
    }
}

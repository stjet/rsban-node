use std::time::Duration;

use anyhow::Error;

use crate::{Account, BlockHash, PropertyTreeWriter, Signature};

#[derive(Clone)]
pub(crate) struct Vote {
    pub timestamp: u64,

    // Account that's voting
    pub voting_account: Account,

    // Signature of timestamp + block hashes
    pub signature: Signature,

    // The hashes for which this vote directly covers
    pub hashes: Vec<BlockHash>,
}

impl Vote {
    pub(crate) fn null() -> Self {
        Self {
            timestamp: 0,
            voting_account: Account::new(),
            signature: Signature::new(),
            hashes: Vec::new(),
        }
    }

    pub(crate) fn new(
        account: Account,
        timestamp: u64,
        duration: u8,
        hashes: Vec<BlockHash>,
    ) -> Self {
        Self {
            voting_account: account,
            timestamp: packed_timestamp(timestamp, duration),
            signature: Signature::new(),
            hashes,
        }
    }

    /// Returns the timestamp of the vote (with the duration bits masked, set to zero)
    /// If it is a final vote, all the bits including duration bits are returned as they are, all FF
    pub(crate) fn timestamp(&self) -> u64 {
        if self.timestamp == u64::MAX {
            self.timestamp //final vote
        } else {
            self.timestamp & TIMESTAMP_MASK
        }
    }

    pub(crate) fn duration_bits(&self) -> u8 {
        // Duration field is specified in the 4 low-order bits of the timestamp.
        // This makes the timestamp have a minimum granularity of 16ms
        // The duration is specified as 2^(duration + 4) giving it a range of 16-524,288ms in power of two increments
        let result = self.timestamp & !TIMESTAMP_MASK;
        result as u8
    }

    pub(crate) fn duration(&self) -> Duration {
        Duration::from_millis(1 << (self.duration_bits() + 4))
    }

    pub(crate) fn vote_hashes_string(&self) -> String {
        let mut result = String::new();
        for h in self.hashes.iter() {
            result.push_str(&h.to_string());
            result.push_str(", ");
        }
        result
    }

    pub(crate) fn serialize_json(&self, writer: &mut dyn PropertyTreeWriter) -> Result<(), Error> {
        writer.put_string("account", &self.voting_account.encode_account())?;
        writer.put_string("signature", &self.signature.encode_hex())?;
        Ok(())
    }
}

impl PartialEq for Vote {
    fn eq(&self, other: &Self) -> bool {
        self.timestamp == other.timestamp
            && self.voting_account == other.voting_account
            && self.signature == other.signature
            && self.hashes == other.hashes
    }
}

const DURATION_MAX: u8 = 0x0F;
const TIMESTAMP_MAX: u64 = 0xFFFF_FFFF_FFFF_FFF0;
const TIMESTAMP_MASK: u64 = 0xFFFF_FFFF_FFFF_FFF0;

fn packed_timestamp(timestamp: u64, duration: u8) -> u64 {
    debug_assert!(duration <= DURATION_MAX);
    debug_assert!(timestamp != TIMESTAMP_MAX || duration == DURATION_MAX);
    (timestamp & TIMESTAMP_MASK) | (duration as u64)
}

use crate::Account;

#[derive(Clone)]
pub(crate) struct Vote {
    pub timestamp: u64,

    // Account that's voting
    pub voting_account: Account,
}

impl Vote {
    pub(crate) fn null() -> Self {
        Self {
            timestamp: 0,
            voting_account: Account::new(),
        }
    }

    pub(crate) fn new(account: Account, timestamp: u64, duration: u8) -> Self {
        Self {
            voting_account: account,
            timestamp: packed_timestamp(timestamp, duration),
        }
    }
}

impl PartialEq for Vote {
    fn eq(&self, other: &Self) -> bool {
        self.timestamp == other.timestamp && self.voting_account == other.voting_account
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

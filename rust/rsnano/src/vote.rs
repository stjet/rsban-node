#[derive(Clone, PartialEq, Eq)]
pub(crate) struct Vote {
    pub timestamp: u64,
}

impl Vote {
    pub(crate) fn null() -> Self {
        Self { timestamp: 0 }
    }

    pub(crate) fn new(timestamp: u64, duration: u8) -> Self {
        Self {
            timestamp: packed_timestamp(timestamp, duration),
        }
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

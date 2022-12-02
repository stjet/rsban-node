use rsnano_core::{Amount, BlockEnum};

use std::{
    sync::{Arc, RwLock},
    time::{Duration, SystemTime},
};

/**
 * Tag for the type of the election status
 */
#[repr(u8)]
#[derive(PartialEq, Eq, Debug, Clone, Copy, FromPrimitive, Default)]
pub enum ElectionStatusType {
    Ongoing = 0,
    ActiveConfirmedQuorum = 1,
    ActiveConfirmationHeight = 2,
    InactiveConfirmationHeight = 3,
    #[default]
    Stopped = 5,
}

/// Information on the status of an election
#[derive(Clone, Default)]
pub struct ElectionStatus {
    pub winner: Option<Arc<RwLock<BlockEnum>>>,
    pub tally: Amount,
    pub final_tally: Amount,
    pub confirmation_request_count: u32,
    pub block_count: u32,
    pub voter_count: u32,
    pub election_end: Option<SystemTime>,
    pub election_duration: Duration,
    pub election_status_type: ElectionStatusType,
}

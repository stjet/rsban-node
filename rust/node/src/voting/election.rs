use rsnano_core::{Account, BlockEnum, BlockHash, QualifiedRoot, Root};

use crate::utils::HardenedConstants;

use super::ElectionStatus;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::SystemTime,
};

pub struct Election {
    pub mutex: Mutex<ElectionData>,
    pub root: Root,
    pub qualified_root: QualifiedRoot,
}

impl Election {
    pub fn new(block: Arc<BlockEnum>) -> Self {
        let root = block.root();
        let qualified_root = block.qualified_root();

        let data = ElectionData {
            status: ElectionStatus {
                winner: Some(Arc::clone(&block)),
                election_end: Some(SystemTime::now()),
                block_count: 1,
                election_status_type: super::ElectionStatusType::Ongoing,
                ..Default::default()
            },
            last_votes: HashMap::from([(
                HardenedConstants::get().not_an_account,
                VoteInfo::new(0, block.hash()),
            )]),
            last_blocks: HashMap::from([(block.hash(), block)]),
            ..Default::default()
        };

        Self {
            mutex: Mutex::new(data),
            root,
            qualified_root,
        }
    }

    pub fn valid_change(expected: ElectionState, desired: ElectionState) -> bool {
        match expected {
            ElectionState::Passive => match desired {
                ElectionState::Active
                | ElectionState::Confirmed
                | ElectionState::ExpiredUnconfirmed => true,
                _ => false,
            },
            ElectionState::Active => match desired {
                ElectionState::Confirmed | ElectionState::ExpiredUnconfirmed => true,
                _ => false,
            },
            ElectionState::Confirmed => match desired {
                ElectionState::ExpiredConfirmed => true,
                _ => false,
            },
            _ => false,
        }
    }
}

#[derive(Default)]
pub struct ElectionData {
    pub status: ElectionStatus,
    pub last_blocks: HashMap<BlockHash, Arc<BlockEnum>>,
    pub last_votes: HashMap<Account, VoteInfo>,
}

#[derive(Clone)]
pub struct VoteInfo {
    pub time: SystemTime, // TODO use Instant
    pub timestamp: u64,
    pub hash: BlockHash,
}

impl VoteInfo {
    pub fn new(timestamp: u64, hash: BlockHash) -> Self {
        Self {
            time: SystemTime::now(),
            timestamp,
            hash,
        }
    }
}

impl Default for VoteInfo {
    fn default() -> Self {
        Self::new(0, BlockHash::zero())
    }
}

#[derive(FromPrimitive)]
#[repr(u8)]
pub enum ElectionState {
    Passive,   // only listening for incoming votes
    Active,    // actively request confirmations
    Confirmed, // confirmed but still listening for votes
    ExpiredConfirmed,
    ExpiredUnconfirmed,
}

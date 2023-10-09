use num_traits::FromPrimitive;
use rsnano_core::{Account, Amount, BlockEnum, BlockHash, QualifiedRoot, Root};

use crate::{stats::DetailType, utils::HardenedConstants};

use super::{ElectionStatus, ElectionStatusType};
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, AtomicU32, AtomicU8, Ordering},
        Arc, Mutex, RwLock,
    },
    time::{Duration, Instant, SystemTime},
};

pub struct Election {
    pub mutex: Mutex<ElectionData>,
    pub root: Root,
    pub qualified_root: QualifiedRoot,
    pub state_value: AtomicU8,
    pub is_quorum: AtomicBool,
    pub confirmation_request_count: AtomicU32,
    pub state_start: RwLock<Instant>,
    // These are modified while not holding the mutex from transition_time only
    last_block: RwLock<Instant>,
    pub behavior: ElectionBehavior,
    pub election_start: Instant,
    pub last_req: RwLock<Option<Instant>>,
    /** The last time vote for this election was generated */
    last_vote: RwLock<Option<Instant>>,
}

impl Election {
    pub fn new(block: Arc<BlockEnum>, behavior: ElectionBehavior) -> Self {
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
            state_value: AtomicU8::new(ElectionState::Passive as u8),
            is_quorum: AtomicBool::new(false),
            confirmation_request_count: AtomicU32::new(0),
            last_block: RwLock::new(Instant::now()),
            behavior,
            election_start: Instant::now(),
            state_start: RwLock::new(Instant::now()),
            last_req: RwLock::new(None),
            last_vote: RwLock::new(None),
        }
    }

    pub fn set_last_req(&self) {
        *self.last_req.write().unwrap() = Some(Instant::now());
    }

    pub fn last_req_elapsed(&self) -> Duration {
        match self.last_req.read().unwrap().as_ref() {
            Some(i) => i.elapsed(),
            None => Duration::from_secs(60 * 60 * 24 * 365), // Duration::MAX caused problems with C++
        }
    }

    pub fn set_last_vote(&self) {
        *self.last_vote.write().unwrap() = Some(Instant::now());
    }

    pub fn last_vote_elapsed(&self) -> Duration {
        match self.last_vote.read().unwrap().as_ref() {
            Some(i) => i.elapsed(),
            None => Duration::from_secs(60 * 60 * 24 * 365), // Duration::MAX caused problems with C++
        }
    }
    pub fn state_change(&self, expected: ElectionState, desired: ElectionState) -> Result<(), ()> {
        if Self::valid_change(expected, desired) {
            if self.compare_exhange_state(expected, desired) {
                *self.state_start.write().unwrap() = Instant::now();
                return Ok(());
            }
        }

        Err(())
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

    pub fn set_last_block(&self) {
        *self.last_block.write().unwrap() = Instant::now();
    }

    pub fn last_block_elapsed(&self) -> Duration {
        self.last_block.read().unwrap().elapsed()
    }

    pub fn state(&self) -> ElectionState {
        FromPrimitive::from_u8(self.state_value.load(Ordering::SeqCst)).unwrap()
    }

    pub fn swap_state(&self, new_state: ElectionState) -> ElectionState {
        FromPrimitive::from_u8(self.state_value.swap(new_state as u8, Ordering::SeqCst)).unwrap()
    }

    pub fn compare_exhange_state(&self, expected: ElectionState, desired: ElectionState) -> bool {
        self.state_value
            .compare_exchange(
                expected as u8,
                desired as u8,
                Ordering::SeqCst,
                Ordering::SeqCst,
            )
            .is_ok()
    }
}

#[derive(Default)]
pub struct ElectionData {
    pub status: ElectionStatus,
    pub last_blocks: HashMap<BlockHash, Arc<BlockEnum>>,
    pub last_votes: HashMap<Account, VoteInfo>,
    pub final_weight: Amount,
    pub last_tally: HashMap<BlockHash, Amount>,
}

impl ElectionData {
    pub fn update_status_to_confirmed(
        &mut self,
        status_type: ElectionStatusType,
        election: &Election,
    ) {
        self.status.election_end = Some(SystemTime::now());
        self.status.election_duration = election.election_start.elapsed();
        self.status.confirmation_request_count =
            election.confirmation_request_count.load(Ordering::SeqCst);
        self.status.block_count = self.last_blocks.len() as u32;
        self.status.voter_count = self.last_votes.len() as u32;
        self.status.election_status_type = status_type;
    }
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

#[derive(FromPrimitive, Copy, Clone, Debug)]
#[repr(u8)]
pub enum ElectionState {
    Passive,   // only listening for incoming votes
    Active,    // actively request confirmations
    Confirmed, // confirmed but still listening for votes
    ExpiredConfirmed,
    ExpiredUnconfirmed,
}

#[derive(FromPrimitive, Copy, Clone, Debug)]
#[repr(u8)]
pub enum ElectionBehavior {
    Normal,
    /**
     * Hinted elections:
     * - shorter timespan
     * - limited space inside AEC
     */
    Hinted,
    /**
     * Optimistic elections:
     * - shorter timespan
     * - limited space inside AEC
     * - more frequent confirmation requests
     */
    Optimistic,
}

impl From<ElectionBehavior> for DetailType {
    fn from(value: ElectionBehavior) -> Self {
        match value {
            ElectionBehavior::Normal => DetailType::Normal,
            ElectionBehavior::Hinted => DetailType::Hinted,
            ElectionBehavior::Optimistic => DetailType::Optimistic,
        }
    }
}

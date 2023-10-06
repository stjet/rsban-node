use rsnano_core::{BlockEnum, BlockHash, QualifiedRoot, Root};

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
        Self {
            mutex: Mutex::new(ElectionData::default()),
            root: block.root(),
            qualified_root: block.qualified_root(),
        }
    }
}

#[derive(Default)]
pub struct ElectionData {
    pub status: ElectionStatus,
    pub last_blocks: HashMap<BlockHash, Arc<BlockEnum>>,
}

#[derive(Clone)]
pub struct VoteInfo {
    pub time: SystemTime, // TODO use Instant
    pub timestamp: u64,
    pub hash: BlockHash,
}

impl Default for VoteInfo {
    fn default() -> Self {
        Self {
            time: SystemTime::now(),
            timestamp: 0,
            hash: BlockHash::zero(),
        }
    }
}

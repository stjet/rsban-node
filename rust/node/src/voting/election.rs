use rsnano_core::{BlockEnum, BlockHash, QualifiedRoot, Root};

use super::ElectionStatus;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
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

#[derive(Default, Clone)]
pub struct VoteInfo {
    pub time: i64,
    pub timestamp: u64,
    pub hash: BlockHash,
}

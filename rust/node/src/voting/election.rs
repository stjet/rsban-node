use rsnano_core::{BlockEnum, Root};

use super::ElectionStatus;
use std::sync::{Arc, Mutex};

pub struct Election {
    pub mutex: Mutex<ElectionStatus>,
    pub root: Root,
}

impl Election {
    pub fn new(block: Arc<BlockEnum>) -> Self {
        Self {
            mutex: Mutex::new(ElectionStatus::default()),
            root: block.root(),
        }
    }
}

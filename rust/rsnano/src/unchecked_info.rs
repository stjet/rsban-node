use std::{
    sync::{Arc, RwLock},
    time::{SystemTime, UNIX_EPOCH},
};

use crate::BlockEnum;

/// Information on an unchecked block
#[derive(Clone)]
pub(crate) struct UncheckedInfo {
    // todo: Remove Option as soon as no C++ code requires the empty constructor
    pub block: Option<Arc<RwLock<BlockEnum>>>,

    /// Seconds since posix epoch
    pub modified: u64,
}

impl UncheckedInfo {
    pub(crate) fn new(block: Arc<RwLock<BlockEnum>>) -> Self {
        Self {
            block: Some(block),
            modified: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        }
    }

    pub(crate) fn null() -> Self {
        Self {
            block: None,
            modified: 0,
        }
    }
}

use std::sync::{Arc, RwLock};

use crate::BlockEnum;

/// Information on an unchecked block
#[derive(Clone)]
pub(crate) struct UncheckedInfo {
    // todo: Remove Option as soon as no C++ code requires the empty constructor
    pub block: Option<Arc<RwLock<BlockEnum>>>,
}

impl UncheckedInfo {
    pub(crate) fn new(block: Arc<RwLock<BlockEnum>>) -> Self {
        Self { block: Some(block) }
    }

    pub(crate) fn null() -> Self {
        Self { block: None }
    }
}

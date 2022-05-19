use std::{
    sync::{Arc, RwLock},
    time::{SystemTime, UNIX_EPOCH},
};

use crate::{Account, BlockEnum, SignatureVerification};

/// Information on an unchecked block
#[derive(Clone)]
pub(crate) struct UncheckedInfo {
    // todo: Remove Option as soon as no C++ code requires the empty constructor
    pub block: Option<Arc<RwLock<BlockEnum>>>,

    /// Seconds since posix epoch
    pub modified: u64,
    pub account: Account,
    pub verified: SignatureVerification,
}

impl UncheckedInfo {
    pub(crate) fn new(
        block: Arc<RwLock<BlockEnum>>,
        account: &Account,
        verified: SignatureVerification,
    ) -> Self {
        Self {
            block: Some(block),
            modified: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            account: *account,
            verified,
        }
    }

    pub(crate) fn null() -> Self {
        Self {
            block: None,
            modified: 0,
            account: *Account::zero(),
            verified: SignatureVerification::Unknown,
        }
    }
}

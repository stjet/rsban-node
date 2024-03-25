use std::sync::{atomic::AtomicU64, Arc};

use crate::RepWeights;

pub struct LedgerCache {
    pub rep_weights: Arc<RepWeights>,
    pub cemented_count: AtomicU64,
    pub block_count: AtomicU64,
    pub pruned_count: AtomicU64,
    pub account_count: AtomicU64,
}

impl LedgerCache {
    pub fn new() -> Self {
        let rep_weights = Arc::new(RepWeights::new());
        Self {
            rep_weights,
            cemented_count: AtomicU64::new(0),
            block_count: AtomicU64::new(0),
            pruned_count: AtomicU64::new(0),
            account_count: AtomicU64::new(0),
        }
    }
}

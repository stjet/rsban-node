use rsnano_core::Amount;
use rsnano_store_lmdb::LmdbRepWeightStore;
use std::sync::{atomic::AtomicU64, Arc};

use crate::{RepWeightCache, RepWeightsUpdater};

pub struct LedgerCache {
    pub rep_weights_updater: RepWeightsUpdater,
    pub rep_weights: RepWeightCache,
    pub cemented_count: AtomicU64,
    pub block_count: AtomicU64,
    pub pruned_count: AtomicU64,
    pub account_count: AtomicU64,
}

impl LedgerCache {
    pub fn new(rep_weight_store: Arc<LmdbRepWeightStore>, min_weight: Amount) -> Self {
        let rep_weights_updater = RepWeightsUpdater::new(rep_weight_store, min_weight);
        Self {
            rep_weights: rep_weights_updater.cache(),
            rep_weights_updater,
            cemented_count: AtomicU64::new(0),
            block_count: AtomicU64::new(0),
            pruned_count: AtomicU64::new(0),
            account_count: AtomicU64::new(0),
        }
    }
}

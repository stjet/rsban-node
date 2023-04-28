use std::{
    sync::{atomic::AtomicBool, Arc},
    time::Duration,
};

use rsnano_core::{utils::Logger, BlockEnum};
use rsnano_ledger::{Ledger, WriteDatabaseQueue};

use crate::stats::Stats;

use super::{
    BatchWriteSizeManager, BlockCacheV2, BoundedMode, BoundedModeContainerInfo, CementCallbackRefs,
};

#[derive(FromPrimitive, Clone, PartialEq, Eq, Copy)]
pub enum ConfirmationHeightMode {
    Automatic,
    Unbounded,
    Bounded,
}

pub(super) struct AutomaticMode {
    pub bounded_mode: BoundedMode,
    pub mode: ConfirmationHeightMode,
    pub ledger: Arc<Ledger>,
}

impl AutomaticMode {
    pub(super) fn new(
        mode: ConfirmationHeightMode,
        ledger: Arc<Ledger>,
        logger: Arc<dyn Logger>,
        enable_timing_logging: bool,
        stats: Arc<Stats>,
        batch_separate_pending_min_time: Duration,
        write_database_queue: Arc<WriteDatabaseQueue>,
        stopped: Arc<AtomicBool>,
    ) -> Self {
        let bounded_mode = BoundedMode::new(
            ledger.clone(),
            write_database_queue.clone(),
            logger.clone(),
            enable_timing_logging,
            batch_separate_pending_min_time,
            stopped.clone(),
            stats.clone(),
        );

        Self {
            bounded_mode,
            mode,
            ledger,
        }
    }

    pub fn batch_write_size(&self) -> &Arc<BatchWriteSizeManager> {
        self.bounded_mode.batch_write_size()
    }

    pub fn has_pending_writes(&self) -> bool {
        self.bounded_mode.has_pending_writes()
    }

    pub fn write_pending_blocks(&mut self, callbacks: &mut CementCallbackRefs) {
        self.bounded_mode.write_pending_blocks(callbacks);
    }

    pub fn process(&mut self, block: Arc<BlockEnum>, callbacks: &mut CementCallbackRefs) {
        self.bounded_mode.process(&block, callbacks);
    }

    pub fn clear_process_vars(&mut self) {
        self.bounded_mode.clear_process_vars();
    }

    pub fn container_info(&self) -> BoundedModeContainerInfo {
        self.bounded_mode.container_info()
    }

    pub fn block_cache(&self) -> &Arc<BlockCacheV2> {
        self.bounded_mode.block_cache()
    }
}

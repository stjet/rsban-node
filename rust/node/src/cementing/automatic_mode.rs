use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

use rsnano_core::{
    utils::{ContainerInfoComponent, Logger},
    BlockEnum,
};
use rsnano_ledger::{Ledger, WriteDatabaseQueue};

use crate::stats::Stats;

use super::{
    block_cache::BlockCache, BatchWriteSizeManager, BoundedMode, BoundedModeContainerInfo,
    CementCallbackRefs, UnboundedMode, UnboundedModeContainerInfo,
};

#[derive(FromPrimitive, Clone, PartialEq, Eq, Copy)]
pub enum ConfirmationHeightMode {
    Automatic,
    Unbounded,
    Bounded,
}

/// When the uncemented count (block count - cemented count) is less than this use the unbounded processor
pub(super) const UNBOUNDED_CUTOFF: usize = 16384;

pub(super) struct AutomaticMode {
    pub bounded_mode: BoundedMode,
    pub unbounded_mode: UnboundedMode,
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

        let unbounded_mode = UnboundedMode::new(
            ledger.clone(),
            write_database_queue,
            logger,
            enable_timing_logging,
            batch_separate_pending_min_time,
            stopped,
            stats,
            bounded_mode.batch_write_size().clone(),
        );

        Self {
            bounded_mode,
            unbounded_mode,
            mode,
            ledger,
        }
    }

    pub fn batch_write_size(&self) -> &Arc<BatchWriteSizeManager> {
        self.bounded_mode.batch_write_size()
    }

    pub fn has_pending_writes(&self) -> bool {
        self.bounded_mode.has_pending_writes() || self.unbounded_mode.has_pending_writes()
    }

    pub fn write_pending_blocks(&mut self, callbacks: &mut CementCallbackRefs) {
        if self.bounded_mode.has_pending_writes() {
            self.bounded_mode.write_pending_blocks(callbacks);
        } else if self.unbounded_mode.has_pending_writes() {
            self.unbounded_mode.write_pending_blocks(callbacks);
        }
    }

    pub fn process(&mut self, block: Arc<BlockEnum>, callbacks: &mut CementCallbackRefs) {
        if self.should_use_unbounded_processor() {
            self.unbounded_mode.process(block, callbacks);
        } else {
            self.bounded_mode.process(&block, callbacks);
        }
    }

    pub fn clear_process_vars(&mut self) {
        self.bounded_mode.clear_process_vars();
        self.unbounded_mode.clear_process_vars();
    }

    pub fn container_info(&self) -> AutomaticModeContainerInfo {
        AutomaticModeContainerInfo {
            bounded_container_info: self.bounded_mode.container_info(),
            unbounded_container_info: self.unbounded_mode.container_info(),
        }
    }

    pub fn block_cache(&self) -> &Arc<BlockCache> {
        self.unbounded_mode.block_cache()
    }

    fn should_use_unbounded_processor(&self) -> bool {
        self.force_unbounded() || self.valid_unbounded()
    }

    fn valid_unbounded(&self) -> bool {
        self.mode == ConfirmationHeightMode::Automatic
            && self.are_blocks_within_automatic_unbounded_section()
            && !self.bounded_mode.has_pending_writes()
    }

    fn force_unbounded(&self) -> bool {
        self.unbounded_mode.has_pending_writes() || self.mode == ConfirmationHeightMode::Unbounded
    }

    fn are_blocks_within_automatic_unbounded_section(&self) -> bool {
        let block_count = self.ledger.cache.block_count.load(Ordering::SeqCst);
        let cemented_count = self.ledger.cache.cemented_count.load(Ordering::SeqCst);

        block_count < (UNBOUNDED_CUTOFF as u64)
            || block_count - (UNBOUNDED_CUTOFF as u64) < cemented_count
    }
}

pub(super) struct AutomaticModeContainerInfo {
    unbounded_container_info: UnboundedModeContainerInfo,
    bounded_container_info: BoundedModeContainerInfo,
}

impl AutomaticModeContainerInfo {
    pub fn collect(&self) -> Vec<ContainerInfoComponent> {
        vec![
            self.bounded_container_info.collect(),
            self.unbounded_container_info.collect(),
        ]
    }
}

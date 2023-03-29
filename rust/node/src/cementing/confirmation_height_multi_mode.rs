use std::sync::{atomic::Ordering, Arc};

use rsnano_core::{utils::ContainerInfoComponent, BlockEnum};
use rsnano_ledger::Ledger;

use super::{
    BoundedContainerInfo, ConfirmationHeightBounded, ConfirmationHeightUnbounded,
    UnboundedContainerInfo,
};

#[derive(FromPrimitive, Clone, PartialEq, Eq, Copy)]
pub enum ConfirmationHeightMode {
    Automatic,
    Unbounded,
    Bounded,
}

/// When the uncemented count (block count - cemented count) is less than this use the unbounded processor
const UNBOUNDED_CUTOFF: u64 = 16384;

pub(super) struct ConfirmationHeightMultiMode {
    pub bounded_processor: ConfirmationHeightBounded,
    pub unbounded_processor: ConfirmationHeightUnbounded,
    pub mode: ConfirmationHeightMode,
    pub ledger: Arc<Ledger>,
}

impl ConfirmationHeightMultiMode {
    pub fn pending_writes_empty(&self) -> bool {
        self.bounded_processor.pending_writes_empty()
            && self.unbounded_processor.pending_writes_empty()
    }

    pub fn write_pending_blocks(&mut self) {
        if !self.bounded_processor.pending_writes_empty() {
            self.bounded_processor.write_pending_blocks();
        } else if !self.unbounded_processor.pending_writes_empty() {
            self.unbounded_processor.write_pending_blocks();
        }
    }

    pub fn process(&mut self, block: Arc<BlockEnum>) {
        if self.should_use_unbounded_processor() {
            self.unbounded_processor.process(block);
        } else {
            self.bounded_processor.process(&block);
        }
    }

    pub fn clear_process_vars(&mut self) {
        self.bounded_processor.clear_process_vars();
        self.unbounded_processor.clear_process_vars();
    }

    pub fn container_info(&self) -> AutomaticContainerInfo {
        AutomaticContainerInfo {
            bounded_container_info: self.bounded_processor.container_info(),
            unbounded_container_info: self.unbounded_processor.container_info(),
        }
    }

    fn should_use_unbounded_processor(&self) -> bool {
        self.force_unbounded() || self.valid_unbounded()
    }

    fn valid_unbounded(&self) -> bool {
        self.mode == ConfirmationHeightMode::Automatic
            && self.are_blocks_within_automatic_unbounded_section()
            && self.bounded_processor.pending_writes_empty()
    }

    fn force_unbounded(&self) -> bool {
        !self.unbounded_processor.pending_writes_empty()
            || self.mode == ConfirmationHeightMode::Unbounded
    }

    fn are_blocks_within_automatic_unbounded_section(&self) -> bool {
        let block_count = self.ledger.cache.block_count.load(Ordering::SeqCst);
        let cemented_count = self.ledger.cache.cemented_count.load(Ordering::SeqCst);

        block_count < UNBOUNDED_CUTOFF || block_count - UNBOUNDED_CUTOFF < cemented_count
    }
}

pub(super) struct AutomaticContainerInfo {
    unbounded_container_info: UnboundedContainerInfo,
    bounded_container_info: BoundedContainerInfo,
}

impl AutomaticContainerInfo {
    pub fn collect(&self) -> Vec<ContainerInfoComponent> {
        vec![
            self.bounded_container_info.collect(),
            self.unbounded_container_info.collect(),
        ]
    }
}

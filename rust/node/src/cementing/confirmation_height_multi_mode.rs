use std::sync::{atomic::Ordering, Arc};

use rsnano_core::BlockEnum;
use rsnano_ledger::Ledger;

use super::{ConfirmationHeightBounded, ConfirmationHeightUnbounded};

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

    pub(super) fn should_use_unbounded_processor(&self) -> bool {
        self.force_unbounded() || self.valid_unbounded()
    }

    pub(super) fn valid_unbounded(&self) -> bool {
        self.mode == ConfirmationHeightMode::Automatic
            && self.are_blocks_within_automatic_unbounded_section()
            && self.bounded_processor.pending_writes_empty()
    }

    pub(super) fn force_unbounded(&self) -> bool {
        !self.unbounded_processor.pending_writes_empty()
            || self.mode == ConfirmationHeightMode::Unbounded
    }

    pub(super) fn are_blocks_within_automatic_unbounded_section(&self) -> bool {
        let block_count = self.ledger.cache.block_count.load(Ordering::SeqCst);
        let cemented_count = self.ledger.cache.cemented_count.load(Ordering::SeqCst);

        block_count < UNBOUNDED_CUTOFF || block_count - UNBOUNDED_CUTOFF < cemented_count
    }
}

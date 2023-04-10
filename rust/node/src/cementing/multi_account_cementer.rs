use std::sync::Arc;

use super::{
    bounded_mode::CementationLedgerAdapter, BatchWriteSizeManager, SingleAccountCementer,
    WriteDetails, WriteDetailsQueue,
};
use rsnano_core::{BlockEnum, ConfirmationHeightUpdate};

/// Writes all confirmation heights from the WriteDetailsQueue to the Ledger.
/// This happens in batches in order to increase performance.
pub(crate) struct MultiAccountCementer {
    /// Will contain all blocks that have been cemented (bounded by batch_write_size)
    /// and will get run through the cemented observer callback
    pub cemented_blocks: Vec<Arc<BlockEnum>>,

    account_cementer: SingleAccountCementer,
    current: Option<WriteDetails>,
}

impl MultiAccountCementer {
    pub fn new() -> Self {
        Self {
            cemented_blocks: Vec::new(),
            account_cementer: Default::default(),
            current: None,
        }
    }

    pub fn cement_next(
        &mut self,
        pending_writes: &mut WriteDetailsQueue,
        batch_write_size: &BatchWriteSizeManager,
        ledger_adapter: &CementationLedgerAdapter,
    ) -> anyhow::Result<Option<(ConfirmationHeightUpdate, bool)>> {
        if self.account_cementer.is_done() {
            self.load_next_pending(pending_writes, batch_write_size, ledger_adapter);
        }

        self.account_cementer.cement(
            &|hash| ledger_adapter.get_block(hash),
            &mut self.cemented_blocks,
        )
    }

    fn load_next_pending(
        &mut self,
        pending_writes: &mut WriteDetailsQueue,
        batch_write_size: &BatchWriteSizeManager,
        ledger_adapter: &CementationLedgerAdapter,
    ) {
        self.current = pending_writes.pop_front();
        if let Some(pending) = self.current.clone() {
            self.init_update_command_factory(batch_write_size, ledger_adapter, pending.clone());
        }
    }

    fn init_update_command_factory(
        &mut self,
        batch_write_size: &BatchWriteSizeManager,
        ledger_adapter: &CementationLedgerAdapter,
        pending: WriteDetails,
    ) {
        let confirmation_height_info =
            ledger_adapter.get_current_confirmation_height(&pending.account);

        self.account_cementer = SingleAccountCementer::new(
            pending,
            confirmation_height_info,
            batch_write_size.current_size_with_tolerance(),
        );
    }

    pub(crate) fn is_done(&self) -> bool {
        self.account_cementer.is_done()
    }

    pub fn unpublished_cemented_blocks(&self) -> usize{
        self.cemented_blocks.len()
    }

    pub fn publish_cemented_blocks(&mut self, block_cemented: &mut dyn FnMut(&Arc<BlockEnum>)) {
        for block in self.cemented_blocks.iter() {
            block_cemented(block);
        }

        self.cemented_blocks.clear();
    }
}

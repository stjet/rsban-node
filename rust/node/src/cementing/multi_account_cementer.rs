use std::sync::Arc;

use super::{BatchWriteSizeManager, SingleAccountCementer, WriteDetails, WriteDetailsQueue};
use rsnano_core::{
    Account, BlockEnum, BlockHash, ConfirmationHeightInfo, ConfirmationHeightUpdate,
};

pub(crate) trait CementationDataRequester {
    fn get_block(&self, block_hash: &BlockHash) -> Option<BlockEnum>;
    fn get_current_confirmation_height(&self, account: &Account) -> ConfirmationHeightInfo;
}

/// Writes all confirmation heights from the WriteDetailsQueue to the Ledger.
/// This happens in batches in order to increase performance.
pub(crate) struct MultiAccountCementer {
    /// Will contain all blocks that have been cemented (bounded by batch_write_size)
    /// and will get run through the cemented observer callback
    cemented_blocks: Vec<Arc<BlockEnum>>,
    account_cementer: SingleAccountCementer,
    current: Option<WriteDetails>,
    pub pending_writes: WriteDetailsQueue,
    pub batch_write_size: Arc<BatchWriteSizeManager>,
}

impl MultiAccountCementer {
    const PENDING_WRITES_MAX_SIZE: usize = 131072;

    pub fn new() -> Self {
        Self {
            cemented_blocks: Vec::new(),
            account_cementer: Default::default(),
            current: None,
            pending_writes: WriteDetailsQueue::new(),
            batch_write_size: Arc::new(BatchWriteSizeManager::new()),
        }
    }

    pub fn max_batch_write_size_reached(&self) -> bool {
        self.pending_writes.total_pending_blocks() >= self.batch_write_size.current_size()
    }

    pub fn max_pending_writes_reached(&self) -> bool {
        self.pending_writes.len() >= Self::PENDING_WRITES_MAX_SIZE
    }

    pub fn has_pending_writes(&self) -> bool {
        self.pending_writes.len() > 0
    }

    pub fn enqueue(&mut self, write_details: WriteDetails) {
        self.pending_writes.push_back(write_details);
    }

    pub fn cement_next<T: CementationDataRequester>(
        &mut self,
        data_requester: &T,
    ) -> anyhow::Result<Option<(ConfirmationHeightUpdate, bool)>> {
        if self.account_cementer.is_done() {
            self.load_next_pending(data_requester);
        }

        self.account_cementer.cement(
            &|hash| data_requester.get_block(hash),
            &mut self.cemented_blocks,
        )
    }

    fn load_next_pending<T: CementationDataRequester>(&mut self, data_requester: &T) {
        self.current = self.pending_writes.pop_front();
        if let Some(pending) = self.current.clone() {
            self.init_account_cementer(data_requester, pending.clone());
        }
    }

    fn init_account_cementer<T: CementationDataRequester>(
        &mut self,
        data_requester: &T,
        pending: WriteDetails,
    ) {
        let confirmation_height_info =
            data_requester.get_current_confirmation_height(&pending.account);

        self.account_cementer = SingleAccountCementer::new(
            pending,
            confirmation_height_info,
            self.batch_write_size.current_size_with_tolerance(),
        );
    }

    pub(crate) fn is_done(&self) -> bool {
        self.account_cementer.is_done() && self.pending_writes.is_empty()
    }

    pub fn unpublished_cemented_blocks(&self) -> usize {
        self.cemented_blocks.len()
    }

    pub fn publish_cemented_blocks(&mut self, block_cemented: &mut dyn FnMut(&Arc<BlockEnum>)) {
        for block in self.cemented_blocks.drain(..) {
            block_cemented(&block);
        }
    }
}

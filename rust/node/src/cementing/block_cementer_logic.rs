use std::{
    sync::{atomic::AtomicBool, Arc},
    time::Duration,
};

use rsnano_core::{utils::ContainerInfoComponent, BlockChainSection, BlockEnum, Epochs};

use super::{
    batch_write_size_manager::BatchWriteSizeManager, cementation_thread::CementCallbackRefs,
    ledger_data_requester::LedgerDataRequester, AccountsConfirmedMapContainerInfo, BlockCache,
    CementationQueueContainerInfo, CementationWalker, WriteBatcher,
};

pub(crate) struct BlockCementerContainerInfo {
    cementation_queue: CementationQueueContainerInfo,
    accounts_confirmed: AccountsConfirmedMapContainerInfo,
}

impl BlockCementerContainerInfo {
    pub fn collect(&self) -> ContainerInfoComponent {
        ContainerInfoComponent::Composite(
            "bounded_mode".to_owned(),
            vec![
                self.cementation_queue
                    .collect("cementation_queue".to_owned()),
                self.accounts_confirmed
                    .collect("accounts_confirmed".to_owned()),
            ],
        )
    }
}

pub(crate) struct BlockCementorLogic {
    pub(crate) cementation_walker: CementationWalker,
    pub(crate) write_batcher: WriteBatcher,
    minimum_batch_separation: Duration,
}

impl BlockCementorLogic {
    pub fn new(
        epochs: Epochs,
        stopped: Arc<AtomicBool>,
        minimum_batch_separation: Duration,
    ) -> Self {
        let cementation_walker = CementationWalker::builder()
            .epochs(epochs)
            .stopped(stopped)
            .build();

        Self {
            cementation_walker,
            write_batcher: Default::default(),
            minimum_batch_separation,
        }
    }

    pub fn initialize(&mut self, original_block: BlockEnum) {
        if !self.write_batcher.has_pending_writes() {
            self.clear_cached_accounts();
        }

        self.cementation_walker.initialize(original_block);
    }

    pub fn block_cache(&self) -> &Arc<BlockCache> {
        &self.cementation_walker.block_cache()
    }

    pub(crate) fn batch_write_size(&self) -> &Arc<BatchWriteSizeManager> {
        &self.write_batcher.batch_write_size
    }

    pub fn has_pending_writes(&self) -> bool {
        self.write_batcher.has_pending_writes()
    }

    pub fn clear_cached_accounts(&mut self) {
        self.cementation_walker.clear_all_cached_accounts();
    }

    pub fn is_write_queue_full(&self) -> bool {
        self.write_batcher.max_pending_writes_reached()
            || self.cementation_walker.is_accounts_cache_full()
    }

    fn is_min_processing_time_exceeded(&self, processing_time: Duration) -> bool {
        processing_time >= self.minimum_batch_separation
    }

    pub(crate) fn should_flush(
        &self,
        callbacks: &mut CementCallbackRefs,
        current_process_done: bool,
        processing_time: Duration,
    ) -> bool {
        let is_batch_full = self.write_batcher.max_batch_write_size_reached();

        // When there are a lot of pending confirmation height blocks, it is more efficient to
        // bulk some of them up to enable better write performance which becomes the bottleneck.
        let awaiting_processing = (callbacks.awaiting_processing_count)();
        let is_done_processing = current_process_done
            && (awaiting_processing == 0 || self.is_min_processing_time_exceeded(processing_time));

        let should_flush = is_done_processing || is_batch_full || self.is_write_queue_full();
        should_flush && !self.write_batcher.has_pending_writes()
    }

    pub fn is_current_block_done(&self) -> bool {
        self.cementation_walker.is_done()
    }

    pub fn was_block_already_cemented(&self) -> bool {
        self.cementation_walker.num_accounts_walked() == 0
    }

    pub fn next_write<T: LedgerDataRequester>(
        &mut self,
        data_requester: &T,
    ) -> Option<BlockChainSection> {
        self.write_batcher.next_write(data_requester)
    }

    pub fn cementation_written(&mut self, section_to_cement: &BlockChainSection) {
        self.cementation_walker
            .cementation_written(&section_to_cement.account, section_to_cement.top_height);
    }

    pub fn batch_written(
        &mut self,
        time_spent_cementing: Duration,
        callbacks: &mut CementCallbackRefs,
    ) {
        self.write_batcher
            .batch_write_size
            .adjust_size(time_spent_cementing);

        self.write_batcher
            .publish_cemented_blocks(callbacks.block_cemented);
    }

    pub(crate) fn container_info(&self) -> BlockCementerContainerInfo {
        BlockCementerContainerInfo {
            cementation_queue: self.write_batcher.container_info(),
            accounts_confirmed: self.cementation_walker.container_info(),
        }
    }

    pub fn unpublished_cemented_blocks_len(&self) -> usize {
        self.write_batcher.unpublished_cemented_blocks_len()
    }

    pub fn should_start_new_write_batch(&self) -> bool {
        true //todo implement!
    }
}

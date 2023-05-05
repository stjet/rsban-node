use std::{
    sync::{atomic::AtomicBool, Arc},
    time::Duration,
};

use rsnano_core::{utils::ContainerInfoComponent, BlockChainSection, BlockEnum, Epochs};

use super::{
    AccountsConfirmedMapContainerInfo, BatchWriteSizeManager, BlockCache, CementCallbackRefs,
    CementationQueueContainerInfo, CementationWalker, LedgerDataRequester, WriteBatcher,
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

pub(crate) struct BlockCementerLogic {
    pub(crate) cementation_walker: CementationWalker,
    pub(crate) write_batcher: WriteBatcher,
    minimum_batch_separation: Duration,
}

#[derive(PartialEq, Eq, Debug)]
pub(crate) enum FlushDecision {
    DontFlush,
    TryFlush(bool),
    ForceFlush(bool),
}

pub(crate) struct BlockCementerLogicOptions {
    pub epochs: Epochs,
    pub stopped: Arc<AtomicBool>,
    pub minimum_batch_separation: Duration,
}

impl Default for BlockCementerLogicOptions {
    fn default() -> Self {
        Self {
            epochs: Default::default(),
            stopped: Default::default(),
            minimum_batch_separation: Duration::from_millis(50),
        }
    }
}

impl BlockCementerLogic {
    pub fn new(options: BlockCementerLogicOptions) -> Self {
        let cementation_walker = CementationWalker::builder()
            .epochs(options.epochs)
            .stopped(options.stopped)
            .build();

        Self {
            cementation_walker,
            write_batcher: Default::default(),
            minimum_batch_separation: options.minimum_batch_separation,
        }
    }

    pub fn set_current_block(&mut self, original_block: BlockEnum) {
        debug_assert!(self.cementation_walker.is_done());
        self.cementation_walker.initialize(original_block);
    }

    pub fn process_current_block<T: LedgerDataRequester>(
        &mut self,
        data_requester: &mut T,
        callbacks: &mut CementCallbackRefs,
    ) -> bool {
        if let Some(section) = self.cementation_walker.next_cementation(data_requester) {
            self.write_batcher.enqueue(section);
            true
        } else {
            self.cementation_walker
                .notify_block_already_cemented(callbacks.block_already_cemented);
            false
        }
    }

    pub fn get_flush_decision(
        &self,
        awaiting_processing: u64,
        processing_time: Duration,
    ) -> FlushDecision {
        if self.should_flush(
            awaiting_processing,
            self.cementation_walker.is_done(),
            processing_time,
        ) {
            if self.is_write_queue_full() {
                FlushDecision::ForceFlush(!self.cementation_walker.is_done())
            } else {
                FlushDecision::TryFlush(!self.cementation_walker.is_done())
            }
        } else {
            FlushDecision::DontFlush
        }
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

    fn should_flush(
        &self,
        awaiting_processing: u64,
        current_process_done: bool,
        processing_time: Duration,
    ) -> bool {
        // When there are a lot of pending confirmation height blocks, it is more efficient to
        // bulk some of them up to enable better write performance which becomes the bottleneck.
        let is_done_processing = current_process_done
            && (awaiting_processing == 0 || self.is_min_processing_time_exceeded(processing_time));

        let should_flush = is_done_processing
            || self.write_batcher.max_batch_write_size_reached()
            || self.is_write_queue_full();
        should_flush && self.write_batcher.has_pending_writes()
    }

    pub fn next_write<T: LedgerDataRequester>(
        &mut self,
        data_requester: &T,
    ) -> Option<BlockChainSection> {
        self.write_batcher.next_write(data_requester)
    }

    pub fn section_cemented(&mut self, section_to_cement: &BlockChainSection) {
        self.cementation_walker
            .section_cemented(&section_to_cement.account, section_to_cement.top_height);
    }

    pub fn batch_completed(
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

impl Default for BlockCementerLogic {
    fn default() -> Self {
        Self::new(Default::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cementing::{CementCallbacks, LedgerDataRequesterStub};

    static TEST_MIN_BATCHSEPARATION: Duration = Duration::from_millis(50);

    #[test]
    fn flush_block_if_it_is_the_only_one() {
        let mut ledger_adapter = LedgerDataRequesterStub::new();
        let genesis_chain = ledger_adapter.add_genesis_block().legacy_send();
        ledger_adapter.add_uncemented(&genesis_chain);

        let mut logic = create_block_cementer_logic();
        logic.set_current_block(genesis_chain.latest_block().clone());
        let mut callbacks = CementCallbacks::default();
        assert!(logic.process_current_block(&mut ledger_adapter, &mut callbacks.as_refs()));
        assert_eq!(
            logic.get_flush_decision(0, Duration::ZERO),
            FlushDecision::TryFlush(false)
        );
        let next_write = logic.next_write(&ledger_adapter).unwrap();
        assert_eq!(next_write, genesis_chain.frontier_section());
        logic.section_cemented(&next_write);
        logic.batch_completed(Duration::ZERO, &mut callbacks.as_refs());
    }

    // flush_one_block_if_processing_duration_is_greater_than_minimum
    // dont_flush_if_processing_duration_is_below_minimum

    fn create_block_cementer_logic() -> BlockCementerLogic {
        BlockCementerLogic::new(BlockCementerLogicOptions {
            minimum_batch_separation: TEST_MIN_BATCHSEPARATION,
            ..Default::default()
        })
    }
}

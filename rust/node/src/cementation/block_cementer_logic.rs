use std::{
    sync::{atomic::AtomicBool, Arc},
    time::Duration,
};

use rsnano_core::{utils::ContainerInfoComponent, BlockChainSection, BlockEnum, Epochs};

use super::{
    batch_write_size_manager::BatchWriteSizeManagerOptions, AccountsConfirmedMapContainerInfo,
    BatchWriteSizeManager, BlockCache, CementCallbackRefs, CementationQueueContainerInfo,
    CementationWalker, LedgerDataRequester, WriteBatcher, WriteBatcherOptions,
};

pub struct BlockCementerContainerInfo {
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
    min_batch_separation: Duration,
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
    pub min_batch_separation: Duration,
    pub min_batch_size: usize,
    pub max_pending_writes: usize,
}

impl Default for BlockCementerLogicOptions {
    fn default() -> Self {
        Self {
            epochs: Default::default(),
            stopped: Default::default(),
            min_batch_separation: Duration::from_millis(50),
            min_batch_size: BatchWriteSizeManagerOptions::DEFAULT_MIN_SIZE,
            max_pending_writes: WriteBatcherOptions::DEFAULT_MAX_PENDING_WRITES,
        }
    }
}

impl BlockCementerLogic {
    pub fn new(options: BlockCementerLogicOptions) -> Self {
        let cementation_walker = CementationWalker::builder()
            .epochs(options.epochs)
            .stopped(options.stopped)
            .build();

        let write_batcher = WriteBatcher::new(WriteBatcherOptions {
            min_batch_size: options.min_batch_size,
            max_pending_writes: options.max_pending_writes,
            ..Default::default()
        });

        Self {
            cementation_walker,
            write_batcher,
            min_batch_separation: options.min_batch_separation,
        }
    }

    pub fn set_current_block(&mut self, original_block: BlockEnum) {
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

    fn is_write_queue_full(&self) -> bool {
        self.write_batcher.max_pending_writes_reached()
            || self.cementation_walker.is_accounts_cache_full()
    }

    fn is_min_processing_time_exceeded(&self, processing_time: Duration) -> bool {
        processing_time >= self.min_batch_separation
    }

    fn should_flush(
        &self,
        awaiting_processing: u64,
        current_process_done: bool,
        processing_time: Duration,
    ) -> bool {
        if !self.write_batcher.has_pending_writes() {
            return false;
        }

        // When there are a lot of pending confirmation height blocks, it is more efficient to
        // bulk some of them up to enable better write performance which becomes the bottleneck.
        let is_done_processing = current_process_done
            && (awaiting_processing == 0 || self.is_min_processing_time_exceeded(processing_time));

        is_done_processing
            || self.write_batcher.max_batch_size_reached()
            || self.is_write_queue_full()
    }

    pub fn next_write<T: LedgerDataRequester>(
        &mut self,
        data_requester: &mut T,
    ) -> Option<BlockChainSection> {
        let next_write = self.write_batcher.next_write(data_requester);
        if let Some(section) = &next_write {
            self.cementation_walker
                .section_cemented(&section.account, section.top_height);
        }
        next_write
    }

    pub fn batch_completed(
        &mut self,
        time_spent_cementing: Duration,
        callbacks: &mut CementCallbackRefs,
    ) {
        self.write_batcher
            .batch_completed(time_spent_cementing, callbacks.block_cemented);
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

    pub fn should_start_new_batch(&self) -> bool {
        self.write_batcher.should_start_new_batch()
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
    use crate::cementation::{CementCallbacks, LedgerDataRequesterStub};

    static TEST_MIN_BATCH_SEPARATION: Duration = Duration::from_millis(50);
    const TEST_MIN_BATCH_SIZE: usize = 100;

    #[test]
    fn flush_block_if_it_is_the_only_one() {
        let mut ledger_adapter = LedgerDataRequesterStub::new();
        let genesis_chain = ledger_adapter.add_genesis_block().legacy_send();
        ledger_adapter.add_uncemented(&genesis_chain);

        let mut logic = BlockCementerLogic::new(test_options());
        logic.set_current_block(genesis_chain.latest_block().clone());
        let mut callbacks = CementCallbacks::default();
        assert!(logic.process_current_block(&mut ledger_adapter, &mut callbacks.as_refs()));
        assert_eq!(
            logic.get_flush_decision(0, Duration::ZERO),
            FlushDecision::TryFlush(false)
        );
        let next_write = logic.next_write(&mut ledger_adapter).unwrap();
        assert_eq!(next_write, genesis_chain.frontier_section());
        assert_eq!(logic.should_start_new_batch(), false);
        logic.batch_completed(Duration::ZERO, &mut callbacks.as_refs());

        assert_eq!(
            logic.process_current_block(&mut ledger_adapter, &mut callbacks.as_refs()),
            false
        );
        assert_eq!(logic.unpublished_cemented_blocks_len(), 0);
        assert_eq!(logic.has_pending_writes(), false);
        assert_eq!(logic.batch_write_size().current_size(), TEST_MIN_BATCH_SIZE);
    }

    #[test]
    fn flush_two_blocks_in_one_batch() {
        let mut ledger_adapter = LedgerDataRequesterStub::new();
        let genesis_chain = ledger_adapter
            .add_genesis_block()
            .legacy_send()
            .legacy_send();
        ledger_adapter.add_uncemented(&genesis_chain);

        let mut logic = BlockCementerLogic::new(test_options());
        logic.set_current_block(genesis_chain.latest_block().clone());
        let mut callbacks = CementCallbacks::default();
        assert!(logic.process_current_block(&mut ledger_adapter, &mut callbacks.as_refs()));
        assert_eq!(
            logic.get_flush_decision(0, Duration::ZERO),
            FlushDecision::TryFlush(false)
        );
        let next_write = logic.next_write(&mut ledger_adapter).unwrap();
        assert_eq!(next_write, genesis_chain.section(2, 3));
        assert_eq!(logic.should_start_new_batch(), false);
        logic.batch_completed(Duration::ZERO, &mut callbacks.as_refs());

        assert_eq!(
            logic.process_current_block(&mut ledger_adapter, &mut callbacks.as_refs()),
            false
        );
        assert_eq!(logic.unpublished_cemented_blocks_len(), 0);
        assert_eq!(logic.has_pending_writes(), false);
        assert_eq!(logic.batch_write_size().current_size(), TEST_MIN_BATCH_SIZE);
    }

    #[test]
    fn dont_flush_if_there_are_more_blocks_awaiting_processing_and_processing_time_is_low() {
        let mut ledger_adapter = LedgerDataRequesterStub::new();
        let genesis_chain = ledger_adapter.add_genesis_block().legacy_send();
        ledger_adapter.add_uncemented(&genesis_chain);

        let mut logic = BlockCementerLogic::new(test_options());
        logic.set_current_block(genesis_chain.latest_block().clone());
        let mut callbacks = CementCallbacks::default();
        assert!(logic.process_current_block(&mut ledger_adapter, &mut callbacks.as_refs()));
        let still_awaiting_processing = 1;
        assert_eq!(
            logic.get_flush_decision(still_awaiting_processing, Duration::ZERO),
            FlushDecision::DontFlush
        );
    }

    #[test]
    fn flush_if_there_are_more_blocks_awaiting_processing_but_processing_time_is_high() {
        let mut ledger_adapter = LedgerDataRequesterStub::new();
        let genesis_chain = ledger_adapter.add_genesis_block().legacy_send();
        ledger_adapter.add_uncemented(&genesis_chain);

        let mut logic = BlockCementerLogic::new(test_options());
        logic.set_current_block(genesis_chain.latest_block().clone());
        let mut callbacks = CementCallbacks::default();
        assert!(logic.process_current_block(&mut ledger_adapter, &mut callbacks.as_refs()));

        let still_awaiting_processing = 1;
        let processing_time = TEST_MIN_BATCH_SEPARATION;
        assert_eq!(
            logic.get_flush_decision(still_awaiting_processing, processing_time),
            FlushDecision::TryFlush(false)
        );
    }

    #[test]
    fn flush_if_max_batch_size_reached() {
        let mut ledger_adapter = LedgerDataRequesterStub::new();
        let genesis_chain = ledger_adapter
            .add_genesis_block()
            .legacy_send()
            .legacy_send()
            .legacy_send();

        ledger_adapter.add_uncemented(&genesis_chain);

        let mut logic = BlockCementerLogic::new(BlockCementerLogicOptions {
            min_batch_size: 2,
            ..test_options()
        });

        logic.set_current_block(genesis_chain.latest_block().clone());
        let mut callbacks = CementCallbacks::default();
        assert!(logic.process_current_block(&mut ledger_adapter, &mut callbacks.as_refs()));
        let still_awaiting_processing = 1;
        assert_eq!(
            logic.get_flush_decision(still_awaiting_processing, Duration::ZERO),
            FlushDecision::TryFlush(false)
        );

        let next_write = logic.next_write(&mut ledger_adapter).unwrap();
        assert_eq!(next_write, genesis_chain.section(2, 3));
        assert_eq!(logic.should_start_new_batch(), true);
        logic.batch_completed(Duration::ZERO, &mut callbacks.as_refs());

        let next_write = logic.next_write(&mut ledger_adapter).unwrap();
        assert_eq!(next_write, genesis_chain.section(4, 4));
        assert_eq!(logic.should_start_new_batch(), false);
    }

    #[test]
    fn flush_if_write_queue_is_full() {
        let mut ledger_adapter = LedgerDataRequesterStub::new();
        let genesis_chain = ledger_adapter.add_genesis_block().legacy_send();

        ledger_adapter.add_uncemented(&genesis_chain);

        let mut logic = BlockCementerLogic::new(BlockCementerLogicOptions {
            max_pending_writes: 1,
            ..test_options()
        });

        logic.set_current_block(genesis_chain.latest_block().clone());
        let mut callbacks = CementCallbacks::default();
        assert!(logic.process_current_block(&mut ledger_adapter, &mut callbacks.as_refs()));
        let still_awaiting_processing = 1;
        assert_eq!(
            logic.get_flush_decision(still_awaiting_processing, Duration::ZERO),
            FlushDecision::ForceFlush(false)
        );

        let next_write = logic.next_write(&mut ledger_adapter).unwrap();
        assert_eq!(next_write, genesis_chain.section(2, 2));
        assert_eq!(logic.should_start_new_batch(), false);
    }

    #[test]
    fn flush_if_batch_is_full() {
        let mut ledger_adapter = LedgerDataRequesterStub::new();
        let genesis_chain = ledger_adapter
            .add_genesis_block()
            .legacy_send();
        let dest_chain = genesis_chain.open_last_destination();
        let genesis_chain = genesis_chain.legacy_send();
        ledger_adapter.add_uncemented(&genesis_chain);
        ledger_adapter.add_uncemented(&dest_chain);

        let mut logic = BlockCementerLogic::new(BlockCementerLogicOptions {
            min_batch_size: 3,
            ..test_options()
        });
        logic.set_current_block(dest_chain.latest_block().clone());
        let mut callbacks = CementCallbacks::default();
        assert_eq!(
            logic.process_current_block(&mut ledger_adapter, &mut callbacks.as_refs()),
            true
        );
        let still_awaiting_processing = 1;
        assert_eq!(
            logic.get_flush_decision(still_awaiting_processing, Duration::ZERO),
            FlushDecision::DontFlush
        );
        assert_eq!(
            logic.process_current_block(&mut ledger_adapter, &mut callbacks.as_refs()),
            true
        );
        assert_eq!(
            logic.get_flush_decision(still_awaiting_processing, Duration::ZERO),
            FlushDecision::DontFlush
        );
        assert_eq!(
            logic.process_current_block(&mut ledger_adapter, &mut callbacks.as_refs()),
            false
        );
        logic.set_current_block(genesis_chain.latest_block().clone());
        assert!(logic.process_current_block(&mut ledger_adapter, &mut callbacks.as_refs()));
        assert_eq!(
            logic.get_flush_decision(still_awaiting_processing, Duration::ZERO),
            FlushDecision::TryFlush(false)
        );

        let next_write = logic.next_write(&mut ledger_adapter).unwrap();
        assert_eq!(next_write, genesis_chain.section(2, 2));
        ledger_adapter.cement(genesis_chain.block(2));
        assert_eq!(logic.should_start_new_batch(), false);

        let next_write = logic.next_write(&mut ledger_adapter).unwrap();
        assert_eq!(next_write, dest_chain.section(1, 1));
        ledger_adapter.cement(dest_chain.block(1));
        assert_eq!(logic.should_start_new_batch(), false);

        let next_write = logic.next_write(&mut ledger_adapter).unwrap();
        assert_eq!(next_write, genesis_chain.section(3, 3));
        ledger_adapter.cement(genesis_chain.block(3));
        assert_eq!(logic.should_start_new_batch(), false);

        let next_write = logic.next_write(&mut ledger_adapter);
        assert_eq!(next_write, None);
    }

    fn test_options() -> BlockCementerLogicOptions {
        BlockCementerLogicOptions {
            min_batch_separation: TEST_MIN_BATCH_SEPARATION,
            min_batch_size: TEST_MIN_BATCH_SIZE,
            ..Default::default()
        }
    }
}
